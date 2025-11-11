//! Boolean expression types with operator overloading and parsing support
//!
//! This module provides a boolean expression representation that can be constructed
//! programmatically using operator overloading, the `expr!` macro, or parsed from strings.
//! Expressions can be minimized using the Espresso algorithm.
//!
//! # Main Types
//!
//! - [`BoolExpr`] - A boolean expression that supports three construction methods:
//!   1. Method API: `a.and(&b).or(&c)`
//!   2. Operator overloading: `&a * &b + &c`
//!   3. **`expr!` macro**: `expr!(a * b + c)` - Recommended!
//! - [`Bdd`] - Binary Decision Diagram for canonical representation and efficient
//!   operations. Used internally for efficient cover generation during minimization.
//!
//! # Quick Start
//!
//! ## Using the `expr!` Macro (Recommended)
//!
//! The `expr!` macro provides the cleanest syntax with three usage styles:
//!
//! ```
//! use espresso_logic::{BoolExpr, expr};
//!
//! // Style 1: String literals (most concise - no variable declarations!)
//! let xor = expr!("a" * !"b" + !"a" * "b");
//! println!("{}", xor);  // Output: a * ~b + ~a * b
//!
//! // Style 2: Existing BoolExpr variables
//! let a = BoolExpr::variable("a");
//! let b = BoolExpr::variable("b");
//! let expr = expr!(a * b + !a * !b);
//!
//! // Style 3: Mix both
//! let result = expr!(a * "temp" + b);
//! ```
//!
//! ## Parsing from Strings
//!
//! ```
//! use espresso_logic::BoolExpr;
//!
//! # fn main() -> std::io::Result<()> {
//! let expr = BoolExpr::parse("a * b + ~a * ~b")?;
//! let complex = BoolExpr::parse("(a + b) * (c + d)")?;
//! println!("{}", expr);  // Minimal parentheses: a * b + ~a * ~b
//! # Ok(())
//! # }
//! ```
//!
//! ## Minimizing and Evaluating
//!
//! ```
//! use espresso_logic::{BoolExpr, expr, Minimizable};
//! use std::collections::HashMap;
//! use std::sync::Arc;
//!
//! # fn main() -> std::io::Result<()> {
//! let a = BoolExpr::variable("a");
//! let b = BoolExpr::variable("b");
//! let c = BoolExpr::variable("c");
//!
//! // Redundant expression
//! let redundant = expr!(a * b + a * b * c);
//!
//! // Evaluate with specific values
//! let mut assignment = HashMap::new();
//! assignment.insert(Arc::from("a"), true);
//! assignment.insert(Arc::from("b"), true);
//! assignment.insert(Arc::from("c"), false);
//! let result = redundant.evaluate(&assignment);
//! assert_eq!(result, true);
//!
//! // Minimize it (returns new minimized instance)
//! let minimized = redundant.minimize()?;
//! println!("Minimized: {}", minimized);  // Output: a * b
//!
//! // Check logical equivalence
//! let redundant2 = expr!(a * b + a * b * c);
//! assert!(redundant2.equivalent_to(&minimized));
//! # Ok(())
//! # }
//! ```

// Submodules
mod conversions;
mod display;
pub mod error;
mod eval;
mod operators;
mod parser;

pub use error::{ExpressionParseError, ParseBoolExprError};

use crate::bdd::Bdd;
use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

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

/// Inner representation of a boolean expression
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BoolExprInner {
    /// A named variable
    Variable(Arc<str>),
    /// Logical AND of two expressions
    And(BoolExpr, BoolExpr),
    /// Logical OR of two expressions
    Or(BoolExpr, BoolExpr),
    /// Logical NOT of an expression
    Not(BoolExpr),
    /// A constant value (true or false)
    Constant(bool),
}

/// A boolean expression that can be manipulated programmatically
///
/// Uses `Arc` internally for efficient cloning. Provides a fluent method-based API
/// and an `expr!` macro for clean syntax.
///
/// # Examples
///
/// # Examples
///
/// ## Method-based API
/// ```
/// use espresso_logic::BoolExpr;
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let expr = a.and(&b).or(&a.not().and(&b.not()));
/// ```
///
/// ## Using operator overloading (requires explicit &)
/// ```  
/// use espresso_logic::BoolExpr;
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let expr = &a * &b + &(&a).not() * &(&b).not();
/// ```
#[derive(Clone)]
pub struct BoolExpr {
    pub(crate) inner: Arc<BoolExprInner>,
    /// Cached BDD representation (computed lazily on first access)
    pub(crate) bdd_cache: Arc<OnceLock<Bdd>>,
}

impl BoolExpr {
    /// Create a variable expression with the given name
    pub fn variable(name: &str) -> Self {
        BoolExpr {
            inner: Arc::new(BoolExprInner::Variable(Arc::from(name))),
            bdd_cache: Arc::new(OnceLock::new()),
        }
    }

    /// Create a constant expression (true or false)
    pub fn constant(value: bool) -> Self {
        BoolExpr {
            inner: Arc::new(BoolExprInner::Constant(value)),
            bdd_cache: Arc::new(OnceLock::new()),
        }
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
    /// let expr = a.and(&b).or(&a.not());
    ///
    /// let op_count = expr.fold(|node| match node {
    ///     ExprNode::Variable(_) | ExprNode::Constant(_) => 0,
    ///     ExprNode::And(l, r) | ExprNode::Or(l, r) => l + r + 1,
    ///     ExprNode::Not(inner) => inner + 1,
    /// });
    ///
    /// assert_eq!(op_count, 3); // AND, OR, NOT
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
        match self.inner.as_ref() {
            BoolExprInner::Variable(name) => f(ExprNode::Variable(name)),
            BoolExprInner::And(left, right) => {
                let left_result = left.fold_impl(f);
                let right_result = right.fold_impl(f);
                f(ExprNode::And(left_result, right_result))
            }
            BoolExprInner::Or(left, right) => {
                let left_result = left.fold_impl(f);
                let right_result = right.fold_impl(f);
                f(ExprNode::Or(left_result, right_result))
            }
            BoolExprInner::Not(inner) => {
                let inner_result = inner.fold_impl(f);
                f(ExprNode::Not(inner_result))
            }
            BoolExprInner::Constant(val) => f(ExprNode::Constant(*val)),
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
        match self.inner.as_ref() {
            BoolExprInner::Variable(name) => f(
                ExprNode::Variable(name),
                context,
                &|_| unreachable!(),
                &|_| unreachable!(),
            ),
            BoolExprInner::Constant(val) => f(
                ExprNode::Constant(*val),
                context,
                &|_| unreachable!(),
                &|_| unreachable!(),
            ),
            BoolExprInner::Not(inner) => {
                let recurse = |ctx: C| inner.fold_with_context_impl(ctx, f);
                f(ExprNode::Not(()), context, &recurse, &|_| unreachable!())
            }
            BoolExprInner::And(left, right) => {
                let recurse_left = |ctx: C| left.fold_with_context_impl(ctx, f);
                let recurse_right = |ctx: C| right.fold_with_context_impl(ctx, f);
                f(
                    ExprNode::And((), ()),
                    context,
                    &recurse_left,
                    &recurse_right,
                )
            }
            BoolExprInner::Or(left, right) => {
                let recurse_left = |ctx: C| left.fold_with_context_impl(ctx, f);
                let recurse_right = |ctx: C| right.fold_with_context_impl(ctx, f);
                f(ExprNode::Or((), ()), context, &recurse_left, &recurse_right)
            }
        }
    }

    /// Collect all variables used in this expression in alphabetical order
    ///
    /// Returns a `BTreeSet` which maintains variables in sorted order.
    /// This ordering is used when converting to covers for minimization.
    pub fn collect_variables(&self) -> BTreeSet<Arc<str>> {
        let mut vars = BTreeSet::new();
        self.collect_variables_impl(&mut vars);
        vars
    }

    /// Convert this boolean expression to a Binary Decision Diagram ([`Bdd`])
    ///
    /// BDDs provide a canonical representation of boolean functions and support
    /// efficient operations. This conversion walks the expression tree and builds
    /// the BDD bottom-up.
    ///
    /// # Caching
    ///
    /// **The BDD is lazily cached on first computation for O(1) subsequent access.**
    ///
    /// - First call to `to_bdd()` computes and caches the BDD at expression level
    /// - Subsequent calls return the cached BDD instantly (O(1))
    /// - During expression composition, subexpression BDD caches are automatically leveraged
    /// - When the same subexpression appears multiple times, its BDD is computed only once
    /// - This prevents redundant conversions during complex transformations
    ///
    /// **Important (v3.1):** Minimization returns a NEW `BoolExpr` with empty expression-level cache.
    /// The global BDD manager caches (ITE cache, unique table) persist as long as any Bdd exists,
    /// but the expression-level cache is lost. **Always minimize late (after all composition) to
    /// maximize expression-level cache hits.**
    ///
    /// # Performance Benefits
    ///
    /// During expression composition, when the same subexpression appears multiple times,
    /// its BDD is computed only once and reused. This prevents redundant conversions during
    /// complex transformations and compositions, providing significant performance gains for
    /// large expressions.
    ///
    /// # Use in Minimization
    ///
    /// When a [`BoolExpr`] is minimized, it is first converted to a [`Bdd`],
    /// then cubes are extracted from the BDD to create a [`Cover`], which is
    /// then minimized by the Espresso algorithm. BDDs enable efficient cover
    /// generation with automatic optimizations.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::BoolExpr;
    ///
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    /// let expr = a.and(&b);
    ///
    /// let bdd1 = expr.to_bdd();  // Computes and caches
    /// let bdd2 = expr.to_bdd();  // Returns cached (O(1))
    /// println!("BDD has {} nodes", bdd1.node_count());
    /// ```
    ///
    /// [`Bdd`]: crate::bdd::Bdd
    /// [`Cover`]: crate::Cover
    pub fn to_bdd(&self) -> Bdd {
        self.bdd_cache
            .get_or_init(|| {
                match self.inner.as_ref() {
                    BoolExprInner::Constant(val) => Bdd::constant(*val),
                    BoolExprInner::Variable(name) => Bdd::variable(name),
                    BoolExprInner::And(left, right) => {
                        // Use to_bdd() on subexpressions to leverage their caches
                        let left_bdd = left.to_bdd();
                        let right_bdd = right.to_bdd();
                        left_bdd.and(&right_bdd)
                    }
                    BoolExprInner::Or(left, right) => {
                        // Use to_bdd() on subexpressions to leverage their caches
                        let left_bdd = left.to_bdd();
                        let right_bdd = right.to_bdd();
                        left_bdd.or(&right_bdd)
                    }
                    BoolExprInner::Not(inner) => {
                        // Use to_bdd() on subexpression to leverage its cache
                        let inner_bdd = inner.to_bdd();
                        inner_bdd.not()
                    }
                }
            })
            .clone()
    }

    fn collect_variables_impl(&self, vars: &mut BTreeSet<Arc<str>>) {
        match self.inner.as_ref() {
            BoolExprInner::Variable(name) => {
                vars.insert(Arc::clone(name));
            }
            BoolExprInner::And(left, right) | BoolExprInner::Or(left, right) => {
                left.collect_variables_impl(vars);
                right.collect_variables_impl(vars);
            }
            BoolExprInner::Not(expr) => {
                expr.collect_variables_impl(vars);
            }
            BoolExprInner::Constant(_) => {}
        }
    }

    /// Logical AND: create a new expression that is the conjunction of this and another
    pub fn and(&self, other: &BoolExpr) -> BoolExpr {
        BoolExpr {
            inner: Arc::new(BoolExprInner::And(self.clone(), other.clone())),
            bdd_cache: Arc::new(OnceLock::new()),
        }
    }

    /// Logical OR: create a new expression that is the disjunction of this and another
    pub fn or(&self, other: &BoolExpr) -> BoolExpr {
        BoolExpr {
            inner: Arc::new(BoolExprInner::Or(self.clone(), other.clone())),
            bdd_cache: Arc::new(OnceLock::new()),
        }
    }

    /// Logical NOT: create a new expression that is the negation of this one
    pub fn not(&self) -> BoolExpr {
        BoolExpr {
            inner: Arc::new(BoolExprInner::Not(self.clone())),
            bdd_cache: Arc::new(OnceLock::new()),
        }
    }
}

/// Manual PartialEq implementation that only compares the expression structure,
/// not the cached BDD (cache is an optimization, not part of the logical value)
impl PartialEq for BoolExpr {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for BoolExpr {}

#[cfg(test)]
mod tests;
