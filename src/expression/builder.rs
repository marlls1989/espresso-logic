//! The auxiliary builder behind [`BoolExpr::build`].
//!
//! Composing a [`BoolExpr`] with the operators concatenates whole reverse-Polish token streams on every
//! combine, so assembling one expression from many pieces copies a growing buffer repeatedly. The
//! builder removes that cost: it accumulates nodes into one central arena, each addition O(1), and
//! serialises the arena to a single token stream once at the end.
//!
//! [`BoolExpr::build`] hands a closure a shared [`ExprBuilder`] reference and expects back an
//! [`Expr`] handle for the root. An `Expr` is a [`Copy`] index into the builder's arena that carries the
//! builder reference, so the operators (`&`, `|`, `^`, `!`) compose handles directly — no `&` or
//! [`clone`](Clone::clone) at the call site. The handle is branded with the builder's lifetime, so it
//! cannot escape the closure or be mixed with another build: both are compile errors.

use super::rpn::Token;
use super::BoolExpr;
use crate::Symbol;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::ops::{BitAnd, BitOr, BitXor, Not};
use std::sync::Arc;

/// One node in the build arena. Operands are indices into the same arena.
enum BuildNode {
    Var(Symbol),
    Const(bool),
    Not(u32),
    And(u32, u32),
    Or(u32, u32),
    Xor(u32, u32),
    /// An existing expression spliced in verbatim; its tokens are emitted as-is at serialisation.
    Graft(BoolExpr),
}

/// The central node arena a [`BoolExpr::build`] closure composes into.
///
/// Created only by [`BoolExpr::build`], which hands the closure a shared reference. The arena is the one
/// growing structure; the [`Expr`] handles returned by its methods are featherweight indices into it.
/// Methods take `&self` (interior mutability) so a single shared reference threads through the whole
/// closure.
#[derive(Default)]
pub struct ExprBuilder {
    nodes: RefCell<Vec<BuildNode>>,
}

impl ExprBuilder {
    /// Append a node and return its arena index.
    fn push(&self, node: BuildNode) -> u32 {
        let mut nodes = self.nodes.borrow_mut();
        let id =
            u32::try_from(nodes.len()).expect("expression builder node count exceeds u32::MAX");
        nodes.push(node);
        id
    }

    /// A variable leaf with the given name.
    pub fn var<S: AsRef<str>>(&self, name: S) -> Expr<'_> {
        Expr::new(self, self.push(BuildNode::Var(Symbol::from(name.as_ref()))))
    }

    /// A constant leaf (`true` or `false`).
    pub fn constant(&self, value: bool) -> Expr<'_> {
        Expr::new(self, self.push(BuildNode::Const(value)))
    }

    /// Splice an existing [`BoolExpr`] into the build as a single handle.
    ///
    /// The expression's tokens are emitted verbatim when the build is serialised; holding it here is a
    /// refcount bump (its tokens are an [`Arc`]).
    pub fn graft(&self, expr: &BoolExpr) -> Expr<'_> {
        Expr::new(self, self.push(BuildNode::Graft(expr.clone())))
    }
}

/// A handle into an [`ExprBuilder`]'s arena — the value a [`BoolExpr::build`] closure composes with.
///
/// [`Copy`], so an operand can be named more than once without [`clone`](Clone::clone), and it carries
/// the builder reference so the operators compose handles in place. The lifetime `'b` brands the handle
/// to one build: it is invariant, so a handle cannot outlive the closure or be combined with a handle
/// from another build.
///
/// A handle cannot escape its closure:
///
/// ```compile_fail
/// use espresso_logic::BoolExpr;
///
/// let mut stash = None;
/// BoolExpr::build(|b| {
///     let a = b.var("a");
///     stash = Some(a); // error: `a` does not live long enough — the brand confines it
///     a
/// });
/// ```
pub struct Expr<'b> {
    builder: &'b ExprBuilder,
    id: u32,
    /// Invariant in `'b`, so the brand neither widens nor narrows.
    _brand: PhantomData<fn(&'b ()) -> &'b ()>,
}

impl<'b> Clone for Expr<'b> {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for Expr<'_> {}

impl<'b> Expr<'b> {
    fn new(builder: &'b ExprBuilder, id: u32) -> Self {
        Expr {
            builder,
            id,
            _brand: PhantomData,
        }
    }

    /// Combine two handles under a binary node constructor.
    fn combine(self, op: fn(u32, u32) -> BuildNode, rhs: Expr<'b>) -> Expr<'b> {
        // The shared `'b` already ties both handles to one builder; this guards against a future change
        // that lets the lifetimes coincide across builders.
        debug_assert!(
            std::ptr::eq(self.builder, rhs.builder),
            "Expr handles from different builders"
        );
        Expr::new(self.builder, self.builder.push(op(self.id, rhs.id)))
    }
}

impl<'b> BitAnd for Expr<'b> {
    type Output = Expr<'b>;
    fn bitand(self, rhs: Expr<'b>) -> Expr<'b> {
        self.combine(BuildNode::And, rhs)
    }
}

impl<'b> BitOr for Expr<'b> {
    type Output = Expr<'b>;
    fn bitor(self, rhs: Expr<'b>) -> Expr<'b> {
        self.combine(BuildNode::Or, rhs)
    }
}

impl<'b> BitXor for Expr<'b> {
    type Output = Expr<'b>;
    fn bitxor(self, rhs: Expr<'b>) -> Expr<'b> {
        self.combine(BuildNode::Xor, rhs)
    }
}

impl<'b> Not for Expr<'b> {
    type Output = Expr<'b>;
    fn not(self) -> Expr<'b> {
        Expr::new(self.builder, self.builder.push(BuildNode::Not(self.id)))
    }
}

impl BoolExpr {
    /// Build an expression through an auxiliary [`ExprBuilder`].
    ///
    /// The closure receives a shared [`ExprBuilder`] and returns the [`Expr`] handle for the result.
    /// Composition inside the closure adds one arena node per operation; the arena is serialised to a
    /// single reverse-Polish token stream once, here, so building a large expression allocates one
    /// `Arc<[Token]>` rather than one per operator. The handle's lifetime confines it to the closure.
    ///
    /// ```
    /// use espresso_logic::BoolExpr;
    ///
    /// // (a ^ b) & !c, composed from handles — no `&` or `.clone()`.
    /// let f = BoolExpr::build(|b| {
    ///     let a = b.var("a");
    ///     let c = b.var("c");
    ///     (a ^ b.var("b")) & !c
    /// });
    /// assert_eq!(f, BoolExpr::parse("(a ^ b) & !c").unwrap());
    /// ```
    #[must_use]
    pub fn build<F>(f: F) -> BoolExpr
    where
        F: for<'b> FnOnce(&'b ExprBuilder) -> Expr<'b>,
    {
        let builder = ExprBuilder::default();
        let root = f(&builder).id;
        let nodes = builder.nodes.into_inner();
        BoolExpr::from_tokens(serialise(&nodes, root))
    }
}

/// Serialise the arena, from `root`, into a reverse-Polish token stream.
///
/// An iterative postorder walk (explicit work-stack, so a deep build cannot overflow the call stack):
/// each node emits its operands' tokens before its own operator, exactly the order the binary operators
/// produce, and a [`Graft`](BuildNode::Graft) splices its tokens verbatim. A handle reused in two places
/// is re-emitted at each use, since the token stream is a tree.
fn serialise(nodes: &[BuildNode], root: u32) -> Arc<[Token]> {
    enum Step {
        /// Walk this node, emitting its operands before its own operator.
        Visit(u32),
        /// Emit an operator token once both operands have been emitted.
        Emit(Token),
    }

    // Size for the real token total: every non-graft node emits one token, while a `Graft` splices its
    // whole sub-expression. (A handle reused in two places is re-emitted, so this can still
    // under-estimate, but it restores the single allocation for the common graft-without-reuse path.)
    let capacity: usize = nodes
        .iter()
        .map(|node| match node {
            BuildNode::Graft(expr) => expr.tokens().len(),
            _ => 1,
        })
        .sum();
    let mut tokens: Vec<Token> = Vec::with_capacity(capacity);
    let mut work = vec![Step::Visit(root)];

    while let Some(step) = work.pop() {
        match step {
            Step::Visit(id) => match &nodes[id as usize] {
                BuildNode::Var(name) => tokens.push(Token::Var(name.clone())),
                BuildNode::Const(value) => tokens.push(Token::Const(*value)),
                BuildNode::Graft(expr) => tokens.extend_from_slice(expr.tokens()),
                BuildNode::Not(inner) => {
                    work.push(Step::Emit(Token::Not));
                    work.push(Step::Visit(*inner));
                }
                BuildNode::And(left, right) => {
                    work.push(Step::Emit(Token::And));
                    work.push(Step::Visit(*right));
                    work.push(Step::Visit(*left));
                }
                BuildNode::Or(left, right) => {
                    work.push(Step::Emit(Token::Or));
                    work.push(Step::Visit(*right));
                    work.push(Step::Visit(*left));
                }
                BuildNode::Xor(left, right) => {
                    work.push(Step::Emit(Token::Xor));
                    work.push(Step::Visit(*right));
                    work.push(Step::Visit(*left));
                }
            },
            Step::Emit(token) => tokens.push(token),
        }
    }

    tokens.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_is_token_identical_to_operator_composition() {
        let built = BoolExpr::build(|b| {
            let a = b.var("a");
            let c = b.var("c");
            (a ^ b.var("b")) & !c
        });
        let composed = (BoolExpr::var("a") ^ BoolExpr::var("b")) & !BoolExpr::var("c");
        assert_eq!(built, composed);
    }

    #[test]
    fn constant_leaves_build() {
        let f = BoolExpr::build(|b| b.var("a") | b.constant(true));
        assert_eq!(f, BoolExpr::var("a") | BoolExpr::constant(true));
    }

    #[test]
    fn graft_splices_an_existing_expression_verbatim() {
        let sub = BoolExpr::parse("a & b").unwrap();
        let built = BoolExpr::build(|b| b.graft(&sub) | b.var("c"));
        assert_eq!(built, sub.clone() | BoolExpr::var("c"));
    }

    #[test]
    fn a_handle_may_be_reused_without_cloning() {
        // `a` is `Copy`, so naming it twice needs no `.clone()`; the token stream is still a tree.
        let built = BoolExpr::build(|b| {
            let a = b.var("a");
            a & a
        });
        assert_eq!(built, BoolExpr::var("a") & BoolExpr::var("a"));
    }

    #[test]
    fn built_expression_is_equivalent_to_operator_form_as_a_bdd() {
        let built = BoolExpr::build(|b| {
            let a = b.var("a");
            let c = b.var("c");
            (a ^ b.var("b")) & !c
        });
        let composed = (BoolExpr::var("a") ^ BoolExpr::var("b")) & !BoolExpr::var("c");

        let builder = crate::bdd_builder!();
        assert!(builder
            .build(&built)
            .equivalent_to(&builder.build(&composed)));
    }

    #[test]
    fn deep_build_does_not_overflow_and_stays_linear() {
        // A long AND chain: one arena node per term, serialised iteratively. Building the same shape
        // with the operators would copy a growing token stream on every combine.
        const N: usize = 50_000;
        let names: Vec<String> = (0..N).map(|i| format!("v{i}")).collect();
        let built = BoolExpr::build(|b| {
            let mut acc = b.var(&names[0]);
            for name in &names[1..] {
                acc = acc & b.var(name);
            }
            acc
        });
        // N variables + (N - 1) AND tokens.
        assert_eq!(built.tokens().len(), N + (N - 1));
        assert_eq!(built.variables().len(), N);
    }
}
