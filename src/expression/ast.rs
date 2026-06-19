//! AST representation and tree traversal operations
//!
//! This module contains the AST types and fold operations for boolean expressions.

use super::BoolExpr;
use crate::Symbol;
use std::sync::Arc;

/// Node type for expression tree folding
///
/// This enum represents the structure of an expression node without exposing
/// internal Arc types. It's used with [`BoolExpr::fold`] and [`BoolExpr::fold_with_context`]
/// to traverse and transform expression trees.
///
/// # Generic Parameter
///
/// - For [`BoolExpr::fold`]: `T` is the accumulated result carried up from child nodes (bottom-up).
/// - For [`BoolExpr::fold_with_context`]: the `descend` closure sees `ExprNode<()>` (shape only, on
///   the way down) and the `combine` closure sees `ExprNode<T>` (child results, on the way up).
///
/// # Examples
///
/// See [`BoolExpr::fold`] and [`BoolExpr::fold_with_context`] for detailed usage examples.
///
/// [`BoolExpr::fold`]: BoolExpr::fold
/// [`BoolExpr::fold_with_context`]: BoolExpr::fold_with_context
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprNode<'a, T> {
    /// A variable with the given name
    Variable(&'a str),
    /// Logical AND with results from left and right subtrees
    And(T, T),
    /// Logical OR with results from left and right subtrees
    Or(T, T),
    /// Logical NOT with result from inner subtree
    Not(T),
    /// A constant boolean value
    Constant(bool),
}

/// AST representation of a boolean expression
///
/// Pure AST tree structure - holds Arc<BoolExprAst> children, not BoolExpr.
/// This allows the AST to be reconstructed from BDD without circular dependencies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BoolExprAst {
    /// A named variable
    Variable(Symbol),
    /// Logical AND of two expressions
    And(Arc<BoolExprAst>, Arc<BoolExprAst>),
    /// Logical OR of two expressions
    Or(Arc<BoolExprAst>, Arc<BoolExprAst>),
    /// Logical NOT of an expression
    Not(Arc<BoolExprAst>),
    /// A constant value (true or false)
    Constant(bool),
}

/// Drop a `BoolExprAst` iteratively so a very deep tree can't overflow the stack.
///
/// A factorised AST can be a long `And`/`Or` chain; the compiler-derived recursive drop of its nested
/// `Arc<BoolExprAst>` children would recurse to the tree's full depth and overflow on such inputs (the
/// same depth hazard the traversals were rewritten to avoid). Instead, move each node's children onto a
/// heap work-stack, leaving a shared childless placeholder behind, so every node that actually drops has
/// only leaf children and cannot recurse further.
impl Drop for BoolExprAst {
    fn drop(&mut self) {
        // One process-wide placeholder `Constant`; replacing a child with a clone of it is a refcount
        // bump (no allocation), and the static keeps a reference so the placeholder's own drop never
        // fires through these clones.
        fn placeholder() -> Arc<BoolExprAst> {
            static P: std::sync::OnceLock<Arc<BoolExprAst>> = std::sync::OnceLock::new();
            Arc::clone(P.get_or_init(|| Arc::new(BoolExprAst::Constant(false))))
        }
        fn take_children(node: &mut BoolExprAst, stack: &mut Vec<Arc<BoolExprAst>>) {
            match node {
                BoolExprAst::Not(a) => stack.push(std::mem::replace(a, placeholder())),
                BoolExprAst::And(a, b) | BoolExprAst::Or(a, b) => {
                    stack.push(std::mem::replace(a, placeholder()));
                    stack.push(std::mem::replace(b, placeholder()));
                }
                BoolExprAst::Variable(_) | BoolExprAst::Constant(_) => {}
            }
        }

        let mut stack: Vec<Arc<BoolExprAst>> = Vec::new();
        take_children(self, &mut stack);
        while let Some(child) = stack.pop() {
            // Only the sole owner dismantles further; a shared child just decrements its refcount. The
            // unwrapped node's children were just taken, so its drop here sees only placeholders.
            if let Ok(mut node) = Arc::try_unwrap(child) {
                take_children(&mut node, &mut stack);
            }
        }
    }
}

impl BoolExpr {
    /// Get or create the AST representation from the BDD
    ///
    /// This is called internally when AST is needed (for display, fold, etc.)
    ///
    /// Uses factorisation-based reconstruction for beautiful, compact expressions.
    pub(super) fn get_or_create_ast(&self) -> Arc<BoolExprAst> {
        // Check if we have a cached AST
        if let Some(ast) = self.ast_cache.get() {
            return Arc::clone(ast);
        }

        // Need to reconstruct from BDD using factorisation
        let ast = self.to_ast_optimised();

        // Try to store it (may fail if another thread beat us to it, that's fine)
        let _ = self.ast_cache.set(Arc::clone(&ast));

        ast
    }

    /// Convert BDD to optimised AST representation using factorisation
    ///
    /// Extracts cubes from the BDD and applies algebraic factorisation to produce
    /// a compact, readable expression.
    ///
    /// Uses local caching for both DNF and AST.
    pub(super) fn to_ast_optimised(&self) -> Arc<BoolExprAst> {
        // Check local AST cache first
        if let Some(ast) = self.ast_cache.get() {
            return Arc::clone(ast);
        }

        // AST not cached, get cubes and factorise
        let cubes = self.get_or_create_cubes();

        // Convert cubes to the (literals, include) format expected by factorisation
        let cube_terms: Vec<(std::collections::BTreeMap<Symbol, bool>, bool)> = cubes
            .iter()
            .map(|cube| (super::minterm_literals(cube), true))
            .collect();

        // Use factorisation to build a nice AST
        let ast = crate::expression::factorization::factorise_cubes_to_ast(cube_terms);

        // Cache locally
        let _ = self.ast_cache.set(Arc::clone(&ast));

        ast
    }

    /// Fold the expression tree depth-first from leaves to root
    ///
    /// This method traverses the expression tree recursively, calling the provided
    /// function `f` on each node. The function receives an [`ExprNode`] containing
    /// the node type and accumulated results from child nodes.
    ///
    /// This is useful for implementing custom expression transformations and analyses
    /// without needing access to private expression internals.
    ///
    /// # Examples
    ///
    /// Count the number of operations in an expression:
    ///
    /// ```
    /// use espresso_logic::{BoolExpr, ExprNode};
    ///
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    /// let expr = a.and(&b);
    ///
    /// let op_count = expr.fold(|node| match node {
    ///     ExprNode::Variable(_) | ExprNode::Constant(_) => 0,
    ///     ExprNode::And(l, r) | ExprNode::Or(l, r) => l + r + 1,
    ///     ExprNode::Not(inner) => inner + 1,
    /// });
    ///
    /// assert_eq!(op_count, 1); // Just AND
    /// ```
    pub fn fold<T, F>(&self, f: F) -> T
    where
        F: Fn(ExprNode<T>) -> T + Copy,
    {
        self.fold_impl(&f)
    }

    fn fold_impl<T, F>(&self, f: &F) -> T
    where
        F: Fn(ExprNode<T>) -> T,
    {
        let ast = self.get_or_create_ast();
        Self::fold_ast(&ast, f)
    }

    /// Fold over an AST bottom-up (helper for `fold_impl`).
    ///
    /// Iterative postorder over an explicit work-stack (so a deep AST can't overflow the call
    /// stack): each node is `Enter`ed once — pushing an `Exit` marker then its children — and the
    /// closure `f` runs at `Exit` time, consuming the children's results off a result stack. Result-
    /// and work-stack depth are O(AST height).
    fn fold_ast<T, F>(ast: &BoolExprAst, f: &F) -> T
    where
        F: Fn(ExprNode<T>) -> T,
    {
        enum Frame<'a> {
            Enter(&'a BoolExprAst),
            ExitAnd,
            ExitOr,
            ExitNot,
        }
        let mut work = vec![Frame::Enter(ast)];
        let mut results: Vec<T> = Vec::new();
        while let Some(frame) = work.pop() {
            match frame {
                Frame::Enter(node) => match node {
                    BoolExprAst::Variable(name) => results.push(f(ExprNode::Variable(name))),
                    BoolExprAst::Constant(val) => results.push(f(ExprNode::Constant(*val))),
                    // Push Exit, then children so they pop (and produce results) first; left pushed
                    // last so it pops first → results end up [.., left, right].
                    BoolExprAst::And(left, right) => {
                        work.push(Frame::ExitAnd);
                        work.push(Frame::Enter(right));
                        work.push(Frame::Enter(left));
                    }
                    BoolExprAst::Or(left, right) => {
                        work.push(Frame::ExitOr);
                        work.push(Frame::Enter(right));
                        work.push(Frame::Enter(left));
                    }
                    BoolExprAst::Not(inner) => {
                        work.push(Frame::ExitNot);
                        work.push(Frame::Enter(inner));
                    }
                },
                Frame::ExitAnd => {
                    let right = results.pop().expect("And right result");
                    let left = results.pop().expect("And left result");
                    results.push(f(ExprNode::And(left, right)));
                }
                Frame::ExitOr => {
                    let right = results.pop().expect("Or right result");
                    let left = results.pop().expect("Or left result");
                    results.push(f(ExprNode::Or(left, right)));
                }
                Frame::ExitNot => {
                    let inner = results.pop().expect("Not inner result");
                    results.push(f(ExprNode::Not(inner)));
                }
            }
        }
        results.pop().expect("fold produced a result")
    }

    /// Fold with a context that flows **top-down** through the tree.
    ///
    /// Unlike [`fold`], which only carries results bottom-up from children to parents, this carries
    /// information in *both* directions, split across two closures:
    ///
    /// - **`descend`** runs on the way *down*. Given an internal node's shape ([`ExprNode<()>`] —
    ///   `And`/`Or`/`Not`) and its own context, it returns the `(left, right)` contexts to hand to
    ///   that node's children. (For `Not` only the left context is used; it is never called on a
    ///   leaf.) This is where, e.g., a negation flag gets flipped or a depth counter incremented.
    /// - **`combine`** runs on the way *back up*. Given a node whose children already hold their
    ///   folded results ([`ExprNode<T>`]) plus that node's own context, it produces this node's
    ///   result. This is where the per-node value — and any context-dependent reshaping, like
    ///   choosing AND vs OR under a De Morgan negation — is decided.
    ///
    /// This split is what lets the fold run **iteratively** (explicit work-stack, no recursion), so
    /// arbitrarily deep expressions can't overflow the call stack. It replaces the older
    /// continuation-passing form (which handed the closure raw `recurse` callbacks); the same
    /// problems are expressible, but the top-down and bottom-up halves are now separated.
    ///
    /// # Examples
    ///
    /// Track depth top-down and take the maximum bottom-up:
    ///
    /// ```
    /// use espresso_logic::{BoolExpr, ExprNode};
    ///
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    /// let expr = a.and(&b).not();
    ///
    /// let max_depth = expr.fold_with_context(
    ///     0,
    ///     // descend: every child sits one level deeper than its parent.
    ///     |_node, &depth| (depth + 1, depth + 1),
    ///     // combine: leaves report their own depth; internal nodes take the deeper child.
    ///     |node, depth| match node {
    ///         ExprNode::Variable(_) | ExprNode::Constant(_) => depth,
    ///         ExprNode::Not(inner) => inner,
    ///         ExprNode::And(l, r) | ExprNode::Or(l, r) => l.max(r),
    ///     },
    /// );
    /// // The fold runs over the canonical (BDD-reconstructed) tree, so the exact depth depends on
    /// // that form; for a two-variable expression it is at least one level deep.
    /// assert!(max_depth >= 1);
    /// ```
    ///
    /// Push negations down with De Morgan's laws: `descend` flips the flag through `Not`, and
    /// `combine` turns an `And` under negation into an `Or` (and vice versa):
    ///
    /// ```
    /// use espresso_logic::{Symbol, BoolExpr, ExprNode};
    /// use std::collections::BTreeMap;
    ///
    /// // Lower an expression to a (very naive) DNF: a list of cubes, each a map of literal->polarity.
    /// fn to_dnf_naive(expr: &BoolExpr) -> Vec<BTreeMap<Symbol, bool>> {
    ///     expr.fold_with_context(
    ///         false, // root context: not negated
    ///         // descend: Not flips the negation for its child; And/Or pass it straight through
    ///         // (De Morgan negates the operands, the reshaping itself happens in `combine`).
    ///         |node, &negate| match node {
    ///             ExprNode::Not(()) => (!negate, !negate),
    ///             _ => (negate, negate),
    ///         },
    ///         // combine: build cubes bottom-up, choosing the operator by the negation flag.
    ///         |node, negate| match node {
    ///             ExprNode::Variable(name) => {
    ///                 let mut cube = BTreeMap::new();
    ///                 cube.insert(Symbol::from(name), !negate);
    ///                 vec![cube]
    ///             }
    ///             ExprNode::Constant(_) => vec![],
    ///             ExprNode::Not(inner) => inner, // flag was already flipped on the way down
    ///             // OR (or AND under negation): union the cube lists.
    ///             ExprNode::Or(mut l, r) | ExprNode::And(mut l, r) if negate => {
    ///                 l.extend(r);
    ///                 l
    ///             }
    ///             // AND (or OR under negation): cross-product the cube lists.
    ///             ExprNode::And(l, r) | ExprNode::Or(l, r) => {
    ///                 let mut out = Vec::new();
    ///                 for lc in &l {
    ///                     for rc in &r {
    ///                         let mut merged = lc.clone();
    ///                         merged.extend(rc.clone());
    ///                         out.push(merged);
    ///                     }
    ///                 }
    ///                 out
    ///             }
    ///         },
    ///     )
    /// }
    ///
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    /// // ~(a*b) is satisfiable, so its DNF has at least one cube. (The exact cube list depends on
    /// // the canonical form the fold walks, so we only check it is non-empty here.)
    /// let dnf = to_dnf_naive(&a.and(&b).not());
    /// assert!(!dnf.is_empty());
    /// ```
    ///
    /// [`fold`]: BoolExpr::fold
    /// [`ExprNode<()>`]: ExprNode
    /// [`ExprNode<T>`]: ExprNode
    pub fn fold_with_context<C, T, D, G>(&self, root_context: C, descend: D, combine: G) -> T
    where
        D: Fn(ExprNode<()>, &C) -> (C, C),
        G: Fn(ExprNode<T>, C) -> T,
    {
        let ast = self.get_or_create_ast();
        Self::fold_with_context_ast(&ast, root_context, &descend, &combine)
    }

    /// Iterative top-down/bottom-up fold over an AST (helper for [`fold_with_context`]).
    ///
    /// Walks the tree on an explicit work-stack so depth is bounded by heap, not the call stack.
    /// Each node is `Enter`ed once carrying its context: leaves combine immediately, internal nodes
    /// call `descend` to derive their children's contexts, push an `Exit` marker holding their own
    /// context, then push their children. The `Exit` marker fires `combine` once the children's
    /// results sit on the `results` stack. Both stacks are O(AST height) deep.
    ///
    /// [`fold_with_context`]: BoolExpr::fold_with_context
    fn fold_with_context_ast<C, T, D, G>(
        ast: &BoolExprAst,
        root_context: C,
        descend: &D,
        combine: &G,
    ) -> T
    where
        D: Fn(ExprNode<()>, &C) -> (C, C),
        G: Fn(ExprNode<T>, C) -> T,
    {
        enum Work<'a, C> {
            Enter(&'a BoolExprAst, C),
            ExitAnd(C),
            ExitOr(C),
            ExitNot(C),
        }
        let mut work = vec![Work::Enter(ast, root_context)];
        let mut results: Vec<T> = Vec::new();
        while let Some(frame) = work.pop() {
            match frame {
                Work::Enter(node, ctx) => match node {
                    BoolExprAst::Variable(name) => {
                        results.push(combine(ExprNode::Variable(name), ctx));
                    }
                    BoolExprAst::Constant(val) => {
                        results.push(combine(ExprNode::Constant(*val), ctx));
                    }
                    BoolExprAst::Not(inner) => {
                        let (child_ctx, _) = descend(ExprNode::Not(()), &ctx);
                        work.push(Work::ExitNot(ctx));
                        work.push(Work::Enter(inner, child_ctx));
                    }
                    // Push Exit, then children so they pop (and produce results) first; left pushed
                    // last so it pops first → results end up [.., left, right].
                    BoolExprAst::And(left, right) => {
                        let (left_ctx, right_ctx) = descend(ExprNode::And((), ()), &ctx);
                        work.push(Work::ExitAnd(ctx));
                        work.push(Work::Enter(right, right_ctx));
                        work.push(Work::Enter(left, left_ctx));
                    }
                    BoolExprAst::Or(left, right) => {
                        let (left_ctx, right_ctx) = descend(ExprNode::Or((), ()), &ctx);
                        work.push(Work::ExitOr(ctx));
                        work.push(Work::Enter(right, right_ctx));
                        work.push(Work::Enter(left, left_ctx));
                    }
                },
                Work::ExitAnd(ctx) => {
                    let right = results.pop().expect("And right result");
                    let left = results.pop().expect("And left result");
                    results.push(combine(ExprNode::And(left, right), ctx));
                }
                Work::ExitOr(ctx) => {
                    let right = results.pop().expect("Or right result");
                    let left = results.pop().expect("Or left result");
                    results.push(combine(ExprNode::Or(left, right), ctx));
                }
                Work::ExitNot(ctx) => {
                    let inner = results.pop().expect("Not inner result");
                    results.push(combine(ExprNode::Not(inner), ctx));
                }
            }
        }
        results.pop().expect("fold_with_context produced a result")
    }
}
