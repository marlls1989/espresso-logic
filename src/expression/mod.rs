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
pub(crate) mod factorization;
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
    /// Primary representation: BDD (canonical form)
    bdd: Bdd,
    /// Cached AST representation (reconstructed lazily when needed for display/fold)
    pub(crate) ast_cache: OnceLock<Arc<BoolExprAst>>,
}

impl BoolExpr {
    /// Create a variable expression with the given name
    pub fn variable(name: &str) -> Self {
        BoolExpr {
            bdd: Bdd::variable(name),
            ast_cache: OnceLock::new(),
        }
    }

    /// Create a constant expression (true or false)
    pub fn constant(value: bool) -> Self {
        BoolExpr {
            bdd: Bdd::constant(value),
            ast_cache: OnceLock::new(),
        }
    }

    /// Get or create the AST representation from the BDD
    ///
    /// This is called internally when AST is needed (for display, fold, etc.)
    ///
    /// Uses factorization-based reconstruction for beautiful, compact expressions.
    fn get_or_create_ast(&self) -> Arc<BoolExprAst> {
        // Check if we have a cached AST
        if let Some(ast) = self.ast_cache.get() {
            return Arc::clone(ast);
        }

        // Need to reconstruct from BDD using factorization
        let ast = self.bdd.to_ast_optimised();

        // Try to store it (may fail if another thread beat us to it, that's fine)
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

    /// Collect all variables used in this expression in alphabetical order
    ///
    /// Returns a `BTreeSet` which maintains variables in sorted order.
    /// This ordering is used when converting to covers for minimization.
    pub fn collect_variables(&self) -> BTreeSet<Arc<str>> {
        // Use BDD-native traversal (no need to reconstruct AST)
        self.bdd.collect_variables()
    }

    /// Convert this boolean expression to a Binary Decision Diagram ([`Bdd`])
    ///
    /// BDDs provide a canonical representation of boolean functions and support
    /// efficient operations. Since BoolExpr now uses BDD as primary storage,
    /// this method simply returns a clone of the internal BDD.
    ///
    /// # Performance
    ///
    /// This operation is O(1) - just a clone of an Arc-based structure.
    ///
    /// # Use in Minimization
    ///
    /// When a [`BoolExpr`] is minimized, its BDD is used to extract cubes
    /// which are then minimized by the Espresso algorithm.
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
    /// let bdd = expr.to_bdd();  // O(1) operation
    /// println!("BDD has {} nodes", bdd.node_count());
    /// ```
    ///
    /// [`Bdd`]: crate::bdd::Bdd
    /// [`Cover`]: crate::Cover
    pub fn to_bdd(&self) -> Bdd {
        self.bdd.clone()
    }

    /// Logical AND: create a new expression that is the conjunction of this and another
    pub fn and(&self, other: &BoolExpr) -> BoolExpr {
        BoolExpr {
            bdd: self.bdd.and(&other.bdd),
            ast_cache: OnceLock::new(),
        }
    }

    /// Logical OR: create a new expression that is the disjunction of this and another
    pub fn or(&self, other: &BoolExpr) -> BoolExpr {
        BoolExpr {
            bdd: self.bdd.or(&other.bdd),
            ast_cache: OnceLock::new(),
        }
    }

    /// Logical NOT: create a new expression that is the negation of this one
    pub fn not(&self) -> BoolExpr {
        BoolExpr {
            bdd: self.bdd.not(),
            ast_cache: OnceLock::new(),
        }
    }
}

/// PartialEq implementation that compares BDDs for canonical equality
///
/// Since BDDs are canonical, two BoolExprs are equal if and only if
/// they represent the same logical function.
impl PartialEq for BoolExpr {
    fn eq(&self, other: &Self) -> bool {
        self.bdd == other.bdd
    }
}

impl Eq for BoolExpr {}

#[cfg(test)]
mod tests;
