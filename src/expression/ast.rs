//! AST representation and tree traversal operations
//!
//! This module contains the AST types and fold operations for boolean expressions.

use super::BoolExpr;
use std::sync::Arc;

/// Node type for expression tree folding
///
/// This enum represents the structure of an expression node without exposing
/// internal Arc types. It's used with [`BoolExpr::fold`] and [`BoolExpr::fold_with_context`]
/// to traverse and transform expression trees.
///
/// # Generic Parameter
///
/// - For [`BoolExpr::fold`]: `T` represents the accumulated result from child nodes (bottom-up)
/// - For [`BoolExpr::fold_with_context`]: `T` is `()` since context flows top-down via closures
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
    Variable(Arc<str>),
    /// Logical AND of two expressions
    And(Arc<BoolExprAst>, Arc<BoolExprAst>),
    /// Logical OR of two expressions
    Or(Arc<BoolExprAst>, Arc<BoolExprAst>),
    /// Logical NOT of an expression
    Not(Arc<BoolExprAst>),
    /// A constant value (true or false)
    Constant(bool),
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
    /// Uses two-level caching for both DNF and AST:
    /// 1. BoolExpr's own dnf_cache (strong reference, lives with this BoolExpr)
    /// 2. BddManager's dnf_cache and ast_cache (weak references, shared across BoolExprs)
    pub(super) fn to_ast_optimised(&self) -> Arc<BoolExprAst> {
        // Check AST cache first (fastest path)
        {
            let mgr = self.manager.read().unwrap();
            if let Some(weak) = mgr.ast_cache.get(&self.root) {
                if let Some(ast) = weak.upgrade() {
                    return ast;
                }
            }
        }

        // AST not cached, get DNF and factorise
        let dnf = self.get_or_create_dnf();

        // Convert DNF cubes to the format expected by factorisation
        let cube_terms: Vec<(std::collections::BTreeMap<Arc<str>, bool>, bool)> = dnf
            .cubes()
            .iter()
            .map(|cube| (cube.clone(), true))
            .collect();

        // Use factorisation to build a nice AST
        let ast = crate::expression::factorization::factorise_cubes_to_ast(cube_terms);

        // Cache the AST in the manager for sharing
        {
            let mut mgr = self.manager.write().unwrap();
            mgr.ast_cache.insert(self.root, Arc::downgrade(&ast));
        }

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

    /// Fold over an AST (helper for fold_impl)
    fn fold_ast<T, F>(ast: &BoolExprAst, f: &F) -> T
    where
        F: Fn(ExprNode<T>) -> T,
    {
        match ast {
            BoolExprAst::Variable(name) => f(ExprNode::Variable(name)),
            BoolExprAst::And(left, right) => {
                let left_result = Self::fold_ast(left, f);
                let right_result = Self::fold_ast(right, f);
                f(ExprNode::And(left_result, right_result))
            }
            BoolExprAst::Or(left, right) => {
                let left_result = Self::fold_ast(left, f);
                let right_result = Self::fold_ast(right, f);
                f(ExprNode::Or(left_result, right_result))
            }
            BoolExprAst::Not(inner) => {
                let inner_result = Self::fold_ast(inner, f);
                f(ExprNode::Not(inner_result))
            }
            BoolExprAst::Constant(val) => f(ExprNode::Constant(*val)),
        }
    }

    /// Fold with context parameter passed top-down through the tree
    ///
    /// Unlike [`fold`], which passes results bottom-up from children to parents,
    /// this method passes a context parameter top-down from parents to children.
    /// The function `f` receives the current node type, context from parent,
    /// and closures to recursively process children with modified context.
    ///
    /// This is useful for operations like applying De Morgan's laws where negations
    /// need to be pushed down through the tree.
    ///
    /// # Real-World Usage
    ///
    /// See `examples/threshold_gate_example.rs` and `examples/c_element_example.rs` for
    /// complete working examples that use `fold_with_context` to implement naive De Morgan
    /// expansion for performance comparison against BDD-based conversion.
    ///
    /// # Examples
    ///
    /// Count depth with context tracking current level:
    ///
    /// ```
    /// use espresso_logic::{BoolExpr, ExprNode};
    ///
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    /// let expr = a.and(&b).not();
    ///
    /// // Count depth with context tracking current level
    /// let max_depth = expr.fold_with_context(0, |node, depth, recurse_left, recurse_right| {
    ///     match node {
    ///         ExprNode::Variable(_) | ExprNode::Constant(_) => depth,
    ///         ExprNode::Not(_) => recurse_left(depth + 1),
    ///         ExprNode::And(_, _) | ExprNode::Or(_, _) => {
    ///             let left_depth = recurse_left(depth + 1);
    ///             let right_depth = recurse_right(depth + 1);
    ///             left_depth.max(right_depth)
    ///         }
    ///     }
    /// });
    /// ```
    ///
    /// Apply De Morgan's laws to push negations down:
    ///
    /// ```
    /// use espresso_logic::{BoolExpr, ExprNode};
    /// use std::collections::BTreeMap;
    /// use std::sync::Arc;
    ///
    /// fn to_dnf_naive(expr: &BoolExpr) -> Vec<BTreeMap<Arc<str>, bool>> {
    ///     expr.fold_with_context(false, |node, negate, recurse_left, recurse_right| {
    ///         match node {
    ///             ExprNode::Variable(name) => {
    ///                 let mut cube = BTreeMap::new();
    ///                 cube.insert(Arc::from(name), !negate);
    ///                 vec![cube]
    ///             }
    ///             ExprNode::Not(()) => recurse_left(!negate), // Flip negation
    ///             ExprNode::And((), ()) if negate => {
    ///                 // De Morgan: ~(A * B) = ~A + ~B
    ///                 let mut result = recurse_left(true);
    ///                 result.extend(recurse_right(true));
    ///                 result
    ///             }
    ///             ExprNode::Or((), ()) if negate => {
    ///                 // De Morgan: ~(A + B) = ~A * ~B (cross product)
    ///                 vec![] // Simplified for example
    ///             }
    ///             _ => vec![] // Other cases omitted
    ///         }
    ///     })
    /// }
    /// ```
    ///
    /// [`fold`]: BoolExpr::fold
    pub fn fold_with_context<C, T, F>(&self, context: C, f: F) -> T
    where
        C: Copy,
        F: Fn(
                ExprNode<()>,
                C,
                &dyn Fn(C) -> T, // recurse_left/inner
                &dyn Fn(C) -> T, // recurse_right
            ) -> T
            + Copy,
    {
        self.fold_with_context_impl(context, &f)
    }

    fn fold_with_context_impl<C, T, F>(&self, context: C, f: &F) -> T
    where
        C: Copy,
        F: Fn(ExprNode<()>, C, &dyn Fn(C) -> T, &dyn Fn(C) -> T) -> T,
    {
        let ast = self.get_or_create_ast();
        Self::fold_with_context_ast(&ast, context, f)
    }

    /// Fold with context over an AST (helper for fold_with_context_impl)
    fn fold_with_context_ast<C, T, F>(ast: &BoolExprAst, context: C, f: &F) -> T
    where
        C: Copy,
        F: Fn(ExprNode<()>, C, &dyn Fn(C) -> T, &dyn Fn(C) -> T) -> T,
    {
        match ast {
            BoolExprAst::Variable(name) => f(
                ExprNode::Variable(name),
                context,
                &|_| unreachable!(),
                &|_| unreachable!(),
            ),
            BoolExprAst::Constant(val) => f(
                ExprNode::Constant(*val),
                context,
                &|_| unreachable!(),
                &|_| unreachable!(),
            ),
            BoolExprAst::Not(inner) => {
                let recurse = |ctx: C| Self::fold_with_context_ast(inner, ctx, f);
                f(ExprNode::Not(()), context, &recurse, &|_| unreachable!())
            }
            BoolExprAst::And(left, right) => {
                let recurse_left = |ctx: C| Self::fold_with_context_ast(left, ctx, f);
                let recurse_right = |ctx: C| Self::fold_with_context_ast(right, ctx, f);
                f(
                    ExprNode::And((), ()),
                    context,
                    &recurse_left,
                    &recurse_right,
                )
            }
            BoolExprAst::Or(left, right) => {
                let recurse_left = |ctx: C| Self::fold_with_context_ast(left, ctx, f);
                let recurse_right = |ctx: C| Self::fold_with_context_ast(right, ctx, f);
                f(ExprNode::Or((), ()), context, &recurse_left, &recurse_right)
            }
        }
    }
}
