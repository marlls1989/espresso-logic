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
//! let xor = expr!("a" * "b" + !"a" * !"b");
//! println!("{}", xor);  // Output: a * b + ~a * ~b
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
//! use espresso_logic::{BoolExpr, expr};
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
//! // Minimize it (consumes redundant)
//! let minimized = redundant.minimize()?;
//! println!("Minimized: {}", minimized);  // Output: a * b
//!
//! // Check logical equivalence
//! let redundant2 = expr!(a * b + a * b * c);
//! assert!(redundant2.equivalent_to(&minimized));
//! # Ok(())
//! # }
//! ```

use crate::error::{ExpressionParseError, MinimizationError, ParseBoolExprError};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::{Add, Mul, Not};
use std::sync::Arc;

// Lalrpop-generated parser module (generated in OUT_DIR at build time)
#[allow(clippy::all)]
mod parser {
    #![allow(clippy::all)]
    #![allow(dead_code)]
    #![allow(unused_variables)]
    #![allow(unused_imports)]
    #![allow(non_snake_case)]
    #![allow(non_camel_case_types)]
    #![allow(non_upper_case_globals)]
    include!(concat!(env!("OUT_DIR"), "/expression/bool_expr.rs"));
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
#[derive(Clone, PartialEq, Eq)]
pub struct BoolExpr {
    inner: Arc<BoolExprInner>,
}

impl BoolExpr {
    /// Create a variable expression with the given name
    pub fn variable(name: &str) -> Self {
        BoolExpr {
            inner: Arc::new(BoolExprInner::Variable(Arc::from(name))),
        }
    }

    /// Create a constant expression (true or false)
    pub fn constant(value: bool) -> Self {
        BoolExpr {
            inner: Arc::new(BoolExprInner::Constant(value)),
        }
    }

    /// Parse a boolean expression from a string
    ///
    /// Supports standard boolean operators:
    /// - `+` or `|` for OR
    /// - `*` or `&` for AND  
    /// - `~` or `!` for NOT
    /// - Parentheses for grouping
    /// - Constants: `0`, `1`, `true`, `false`
    pub fn parse(input: &str) -> Result<Self, ParseBoolExprError> {
        parser::ExprParser::new().parse(input).map_err(|e| {
            let message = e.to_string();
            // Try to extract position from lalrpop error message
            let position = extract_position_from_error(&message);
            ExpressionParseError::InvalidSyntax {
                message,
                input: input.to_string(),
                position,
            }
            .into()
        })
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

    /// Convert this expression to Disjunctive Normal Form (DNF)
    ///
    /// Returns a vector of product terms, where each term is a map from variable to its literal value
    /// (true for positive literal, false for negative literal).
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
    /// let dnf = expr.to_dnf();
    /// assert_eq!(dnf.len(), 1); // One product term: a * b
    /// ```
    pub fn to_dnf(&self) -> Vec<BTreeMap<Arc<str>, bool>> {
        to_dnf_impl(self)
    }

    /// Logical AND: create a new expression that is the conjunction of this and another
    pub fn and(&self, other: &BoolExpr) -> BoolExpr {
        BoolExpr {
            inner: Arc::new(BoolExprInner::And(self.clone(), other.clone())),
        }
    }

    /// Logical OR: create a new expression that is the disjunction of this and another
    pub fn or(&self, other: &BoolExpr) -> BoolExpr {
        BoolExpr {
            inner: Arc::new(BoolExprInner::Or(self.clone(), other.clone())),
        }
    }

    /// Logical NOT: create a new expression that is the negation of this one
    pub fn not(&self) -> BoolExpr {
        BoolExpr {
            inner: Arc::new(BoolExprInner::Not(self.clone())),
        }
    }

    /// Get a reference to the inner expression (internal use)
    pub(crate) fn inner(&self) -> &BoolExprInner {
        &self.inner
    }

    /// Minimize this boolean expression using Espresso
    ///
    /// This is a convenience method that creates a `Cover`, adds the expression to it,
    /// minimizes it, and returns the minimized expression.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{BoolExpr, expr};
    ///
    /// # fn main() -> std::io::Result<()> {
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    /// let c = BoolExpr::variable("c");
    ///
    /// // Redundant expression
    /// let expr = expr!(a * b + a * b * c);
    ///
    /// // Minimize it
    /// let minimized = expr.minimize()?;
    ///
    /// // minimized should be simpler (just a * b)
    /// # Ok(())
    /// # }
    /// ```
    pub fn minimize(self) -> Result<BoolExpr, MinimizationError> {
        use crate::{Cover, CoverType};
        let mut cover = Cover::new(CoverType::F);
        // This shouldn't fail because we're using a new cover with a unique output name
        cover
            .add_expr(self, "out")
            .expect("Adding expression to new cover should not fail");
        cover.minimize()?;
        // This shouldn't fail because we just created the output "out"
        Ok(cover
            .to_expr("out")
            .expect("Converting output to expression should not fail"))
    }

    /// Minimize this boolean expression using exact minimization
    ///
    /// This method uses exact minimization which guarantees minimal results,
    /// unlike the heuristic [`minimize()`](Self::minimize) method.
    ///
    /// # Performance vs Quality Trade-off
    ///
    /// - **`minimize()`**: Fast heuristic, near-optimal results (~99% optimal in practice)
    /// - **`minimize_exact()`**: Slower but guaranteed minimal results (exact solution)
    ///
    /// Use `minimize_exact()` when:
    /// - You need provably minimal results (e.g., for equivalency checking)
    /// - The expression is small enough that exact solving is feasible
    /// - Quality is more important than speed
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{BoolExpr, expr};
    ///
    /// # fn main() -> std::io::Result<()> {
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    /// let c = BoolExpr::variable("c");
    ///
    /// // Redundant expression
    /// let expr = expr!(a * b + a * b * c);
    ///
    /// // Minimize exactly for guaranteed minimal result
    /// let minimized = expr.minimize_exact()?;
    ///
    /// // minimized is guaranteed to be minimal (a * b)
    /// # Ok(())
    /// # }
    /// ```
    pub fn minimize_exact(self) -> Result<BoolExpr, MinimizationError> {
        use crate::{Cover, CoverType};
        let mut cover = Cover::new(CoverType::F);
        // This shouldn't fail because we're using a new cover with a unique output name
        cover
            .add_expr(self, "out")
            .expect("Adding expression to new cover should not fail");
        cover.minimize_exact()?;
        // This shouldn't fail because we just created the output "out"
        Ok(cover
            .to_expr("out")
            .expect("Converting output to expression should not fail"))
    }

    /// Check if two boolean expressions are logically equivalent
    ///
    /// This method uses exact minimization to efficiently check logical equivalence.
    /// It combines both expressions into a single cover with two outputs, minimizes
    /// exactly once, and checks if all cubes have identical output patterns.
    ///
    /// # Performance
    ///
    /// This method is much more efficient than exhaustive truth table comparison:
    /// - **Old approach**: O(2^n) where n is the number of variables (exponential)
    /// - **New approach**: O(m × k) where m is cubes and k is variables (polynomial)
    ///
    /// For expressions with many variables, this is dramatically faster.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::BoolExpr;
    ///
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    ///
    /// // These are logically equivalent
    /// let expr1 = a.and(&b);
    /// let expr2 = b.and(&a);  // Commutative
    /// assert!(expr1.equivalent_to(&expr2));
    ///
    /// // These are not
    /// let expr3 = a.or(&b);
    /// assert!(!expr1.equivalent_to(&expr3));
    /// ```
    pub fn equivalent_to(&self, other: &BoolExpr) -> bool {
        use crate::{Cover, CoverType};
        use std::collections::HashMap;

        // Handle constant expressions specially
        let self_vars = self.collect_variables();
        let other_vars = other.collect_variables();

        if self_vars.is_empty() && other_vars.is_empty() {
            // Both are constants - just evaluate
            return self.evaluate(&HashMap::new()) == other.evaluate(&HashMap::new());
        }

        // Create a cover with both expressions as separate outputs
        let mut cover = Cover::new(CoverType::F);

        // Add both expressions - if this fails, they're not equivalent
        if cover.add_expr(self.clone(), "expr1").is_err() {
            return false;
        }
        if cover.add_expr(other.clone(), "expr2").is_err() {
            return false;
        }

        // Minimize exactly once - if this fails, assume not equivalent
        if cover.minimize_exact().is_err() {
            return false;
        }

        // Check if all cubes have identical output patterns for both outputs
        // After exact minimization, if the expressions are equivalent, every cube
        // will have the same value for both outputs (both 0 or both 1)
        for cube in cover.cubes() {
            let outputs = cube.outputs();
            if outputs.len() >= 2 && outputs[0] != outputs[1] {
                return false;
            }
        }

        true
    }

    /// Evaluate the boolean expression given an assignment of variables to values
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::BoolExpr;
    /// use std::collections::HashMap;
    /// use std::sync::Arc;
    ///
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    /// let expr = a.and(&b);
    ///
    /// let mut assignment = HashMap::new();
    /// assignment.insert(Arc::from("a"), true);
    /// assignment.insert(Arc::from("b"), true);
    ///
    /// assert_eq!(expr.evaluate(&assignment), true);
    ///
    /// assignment.insert(Arc::from("b"), false);
    /// assert_eq!(expr.evaluate(&assignment), false);
    /// ```
    pub fn evaluate(&self, assignment: &std::collections::HashMap<Arc<str>, bool>) -> bool {
        match self.inner.as_ref() {
            BoolExprInner::Variable(name) => *assignment.get(name).unwrap_or(&false),
            BoolExprInner::Constant(val) => *val,
            BoolExprInner::And(left, right) => {
                left.evaluate(assignment) && right.evaluate(assignment)
            }
            BoolExprInner::Or(left, right) => {
                left.evaluate(assignment) || right.evaluate(assignment)
            }
            BoolExprInner::Not(expr) => !expr.evaluate(assignment),
        }
    }
}

/// Helper function to extract position information from lalrpop error messages
///
/// Lalrpop errors often contain position information in the form "at line X column Y"
/// or similar patterns. This function attempts to extract that information.
fn extract_position_from_error(error_msg: &str) -> Option<usize> {
    // Try to find patterns like "at 5" or "position 5" or similar
    // Lalrpop errors typically have format like "Unrecognized token `+` at line 1 column 7"

    // Look for "column N" pattern
    if let Some(col_idx) = error_msg.find("column ") {
        let after_col = &error_msg[col_idx + 7..];
        if let Some(end_idx) = after_col.find(|c: char| !c.is_ascii_digit()) {
            if let Ok(col) = after_col[..end_idx].parse::<usize>() {
                return Some(col.saturating_sub(1)); // Convert to 0-indexed
            }
        }
    }

    // Look for position after "at " pattern (some formats use byte offset)
    if let Some(at_idx) = error_msg.rfind(" at ") {
        let after_at = &error_msg[at_idx + 4..];
        // Skip if it looks like "at line" or "at column"
        if !after_at.starts_with("line") && !after_at.starts_with("column") {
            if let Some(end_idx) = after_at.find(|c: char| !c.is_ascii_digit()) {
                if let Ok(pos) = after_at[..end_idx].parse::<usize>() {
                    return Some(pos);
                }
            }
        }
    }

    None
}

// The expr! macro is now provided by the espresso-logic-macros procedural macro crate
// and re-exported from the main crate lib.rs

/// Debug formatting for boolean expressions
///
/// Formats the expression in a readable form using standard boolean notation:
/// - Variables: shown as-is (e.g., `a`)
/// - AND: `*` operator (e.g., `a * b`)
/// - OR: `+` operator (e.g., `a + b`)
/// - NOT: `~` prefix (e.g., `~a`)
/// - Constants: `1` for true, `0` for false
///
/// Parentheses are only added when necessary based on operator precedence (NOT > AND > OR).
///
/// # Examples
///
/// ```
/// use espresso_logic::BoolExpr;
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let c = BoolExpr::variable("c");
/// let expr = a.and(&b).or(&c);
///
/// println!("{:?}", expr);  // Outputs: a * b + c (no unnecessary parentheses)
/// ```
impl fmt::Debug for BoolExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_with_context(f, OpContext::None)
    }
}

/// Context for formatting expressions with minimal parentheses
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpContext {
    None, // Top level or inside parentheses
    And,  // Inside an AND operation
    Or,   // Inside an OR operation
    Not,  // Inside a NOT operation
}

impl BoolExpr {
    /// Format with operator precedence context to minimize parentheses
    fn fmt_with_context(&self, f: &mut fmt::Formatter<'_>, ctx: OpContext) -> fmt::Result {
        match self.inner.as_ref() {
            BoolExprInner::Variable(name) => write!(f, "{}", name),
            BoolExprInner::Constant(val) => write!(f, "{}", if *val { "1" } else { "0" }),

            BoolExprInner::And(left, right) => {
                // AND needs parens if inside a NOT
                let needs_parens = ctx == OpContext::Not;

                if needs_parens {
                    write!(f, "(")?;
                }

                left.fmt_with_context(f, OpContext::And)?;
                write!(f, " * ")?;
                right.fmt_with_context(f, OpContext::And)?;

                if needs_parens {
                    write!(f, ")")?;
                }
                Ok(())
            }

            BoolExprInner::Or(left, right) => {
                // OR needs parens if inside AND or NOT (lower precedence)
                let needs_parens = ctx == OpContext::And || ctx == OpContext::Not;

                if needs_parens {
                    write!(f, "(")?;
                }

                left.fmt_with_context(f, OpContext::Or)?;
                write!(f, " + ")?;
                right.fmt_with_context(f, OpContext::Or)?;

                if needs_parens {
                    write!(f, ")")?;
                }
                Ok(())
            }

            BoolExprInner::Not(expr) => {
                write!(f, "~")?;
                // NOT needs parens around compound expressions (AND/OR)
                // but NOT of NOT or variables/constants don't need parens
                match expr.inner.as_ref() {
                    BoolExprInner::Variable(_)
                    | BoolExprInner::Constant(_)
                    | BoolExprInner::Not(_) => expr.fmt_with_context(f, OpContext::Not),
                    _ => {
                        write!(f, "(")?;
                        expr.fmt_with_context(f, OpContext::None)?;
                        write!(f, ")")
                    }
                }
            }
        }
    }
}

/// Display formatting for boolean expressions
///
/// Delegates to the `Debug` implementation. Use `{}` or `{:?}` interchangeably.
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
/// println!("{}", expr);  // Same as println!("{:?}", expr)
/// ```
impl fmt::Display for BoolExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

// Operator overloading
// Implemented for both owned and borrowed types
// The expr! macro wraps expressions to enable clean `a * b + !a * !b` syntax

/// Logical AND operator for references: `&a * &b`
///
/// Implements the `*` operator for boolean expressions using references.
/// This is the most efficient form as it avoids unnecessary cloning.
///
/// # Examples
///
/// ```
/// use espresso_logic::BoolExpr;
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let result = &a * &b;  // Equivalent to a.and(&b)
/// ```
impl Mul for &BoolExpr {
    type Output = BoolExpr;

    fn mul(self, rhs: &BoolExpr) -> BoolExpr {
        self.and(rhs)
    }
}

/// Logical AND operator: `a * b` (delegates to reference version)
///
/// Implements the `*` operator for owned boolean expressions.
/// Note: Using references (`&a * &b`) is preferred for efficiency.
///
/// # Examples
///
/// ```
/// use espresso_logic::BoolExpr;
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// // Both work, but references are preferred
/// let result1 = a.clone() * b.clone();
/// let result2 = &a * &b;
/// ```
impl Mul for BoolExpr {
    type Output = BoolExpr;

    fn mul(self, rhs: BoolExpr) -> BoolExpr {
        self.and(&rhs)
    }
}

/// Logical OR operator for references: `&a + &b`
///
/// Implements the `+` operator for boolean expressions using references.
/// This is the most efficient form as it avoids unnecessary cloning.
///
/// # Examples
///
/// ```
/// use espresso_logic::BoolExpr;
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let result = &a + &b;  // Equivalent to a.or(&b)
/// ```
impl Add for &BoolExpr {
    type Output = BoolExpr;

    fn add(self, rhs: &BoolExpr) -> BoolExpr {
        self.or(rhs)
    }
}

/// Logical OR operator: `a + b` (delegates to reference version)
///
/// Implements the `+` operator for owned boolean expressions.
/// Note: Using references (`&a + &b`) is preferred for efficiency.
///
/// # Examples
///
/// ```
/// use espresso_logic::BoolExpr;
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// // Both work, but references are preferred
/// let result1 = a.clone() + b.clone();
/// let result2 = &a + &b;
/// ```
impl Add for BoolExpr {
    type Output = BoolExpr;

    fn add(self, rhs: BoolExpr) -> BoolExpr {
        self.or(&rhs)
    }
}

/// Logical NOT operator for references: `!&a`
///
/// Implements the `!` operator for boolean expressions using references.
/// This is the most efficient form as it avoids unnecessary cloning.
///
/// # Examples
///
/// ```
/// use espresso_logic::BoolExpr;
///
/// let a = BoolExpr::variable("a");
/// let result = !&a;  // Equivalent to a.not()
/// ```
impl Not for &BoolExpr {
    type Output = BoolExpr;

    fn not(self) -> BoolExpr {
        BoolExpr::not(self)
    }
}

/// Logical NOT operator: `!a` (delegates to reference version)
///
/// Implements the `!` operator for owned boolean expressions.
/// Note: Using references (`!&a`) is preferred for efficiency when the
/// original expression is still needed.
///
/// # Examples
///
/// ```
/// use espresso_logic::BoolExpr;
///
/// let a = BoolExpr::variable("a");
/// // Both work, but references are preferred if you need 'a' later
/// let result1 = !a.clone();
/// let result2 = !&a;
/// ```
impl Not for BoolExpr {
    type Output = BoolExpr;

    fn not(self) -> BoolExpr {
        BoolExpr::not(&self)
    }
}

// ============================================================================
// DNF Conversion - Helper Functions
// ============================================================================

/// Convert a boolean expression to Disjunctive Normal Form (DNF)
/// Returns a vector of product terms, where each term is a map from variable to its literal value
/// (true for positive literal, false for negative literal)
fn to_dnf_impl(expr: &BoolExpr) -> Vec<BTreeMap<Arc<str>, bool>> {
    match expr.inner() {
        BoolExprInner::Constant(true) => {
            // True constant = one product term with no literals (tautology)
            vec![BTreeMap::new()]
        }
        BoolExprInner::Constant(false) => {
            // False constant = no product terms (empty sum)
            vec![]
        }
        BoolExprInner::Variable(name) => {
            // Single variable = one product with positive literal
            let mut term = BTreeMap::new();
            term.insert(Arc::clone(name), true);
            vec![term]
        }
        BoolExprInner::Not(inner) => {
            // NOT is handled recursively with De Morgan's laws
            to_dnf_not_impl(inner)
        }
        BoolExprInner::And(left, right) => {
            // AND: cross product of terms from each side
            let left_dnf = to_dnf_impl(left);
            let right_dnf = to_dnf_impl(right);

            let mut result = Vec::new();
            for left_term in &left_dnf {
                for right_term in &right_dnf {
                    // Merge terms, checking for contradictions (x AND ~x)
                    if let Some(merged) = merge_product_terms(left_term, right_term) {
                        result.push(merged);
                    }
                }
            }
            result
        }
        BoolExprInner::Or(left, right) => {
            // OR: union of terms from each side
            let mut left_dnf = to_dnf_impl(left);
            let right_dnf = to_dnf_impl(right);
            left_dnf.extend(right_dnf);
            left_dnf
        }
    }
}

/// Convert NOT expression to DNF using De Morgan's laws
fn to_dnf_not_impl(expr: &BoolExpr) -> Vec<BTreeMap<Arc<str>, bool>> {
    match expr.inner() {
        BoolExprInner::Constant(val) => {
            // NOT of constant
            to_dnf_impl(&BoolExpr::constant(!val))
        }
        BoolExprInner::Variable(name) => {
            // NOT of variable = one product with negative literal
            let mut term = BTreeMap::new();
            term.insert(Arc::clone(name), false);
            vec![term]
        }
        BoolExprInner::Not(inner) => {
            // Double negation
            to_dnf_impl(inner)
        }
        BoolExprInner::And(left, right) => {
            // De Morgan: ~(A * B) = ~A + ~B
            let not_left = left.not();
            let not_right = right.not();
            to_dnf_impl(&not_left.or(&not_right))
        }
        BoolExprInner::Or(left, right) => {
            // De Morgan: ~(A + B) = ~A * ~B
            let not_left = left.not();
            let not_right = right.not();
            to_dnf_impl(&not_left.and(&not_right))
        }
    }
}

/// Merge two product terms (AND them together)
/// Returns None if they contradict (e.g., x AND ~x)
fn merge_product_terms(
    left: &BTreeMap<Arc<str>, bool>,
    right: &BTreeMap<Arc<str>, bool>,
) -> Option<BTreeMap<Arc<str>, bool>> {
    let mut result = left.clone();

    for (var, &polarity) in right {
        if let Some(&existing) = result.get(var) {
            if existing != polarity {
                // Contradiction: x AND ~x = false
                return None;
            }
        } else {
            result.insert(Arc::clone(var), polarity);
        }
    }

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr;

    #[test]
    fn test_variable_creation() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let a2 = BoolExpr::variable("a");

        // Variables are compared by structure
        assert_eq!(a, a2);
        assert_ne!(a, b);
    }

    #[test]
    fn test_constant_creation() {
        let t = BoolExpr::constant(true);
        let f = BoolExpr::constant(false);

        assert_eq!(t, BoolExpr::constant(true));
        assert_ne!(t, f);
    }

    #[test]
    fn test_method_api() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");

        // Test AND method - no clones in user code!
        let and_expr = a.and(&b);
        match and_expr.inner() {
            BoolExprInner::And(_, _) => {}
            _ => panic!("Expected And expression"),
        }

        // Test OR method - can still use a and b
        let or_expr = a.or(&b);
        match or_expr.inner() {
            BoolExprInner::Or(_, _) => {}
            _ => panic!("Expected Or expression"),
        }

        // Test NOT method
        let not_expr = a.not();
        match not_expr.inner() {
            BoolExprInner::Not(_) => {}
            _ => panic!("Expected Not expression"),
        }
    }

    #[test]
    fn test_complex_expression() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");

        // Build complex expression: (a AND b) OR (NOT a AND c)
        let expr = a.and(&b).or(&a.not().and(&c));

        match expr.inner() {
            BoolExprInner::Or(_, _) => {}
            _ => panic!("Expected Or at top level"),
        }
    }

    #[test]
    fn test_collect_variables() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");

        // Using method API
        let expr = a.and(&b).or(&c);
        let vars = expr.collect_variables();

        assert_eq!(vars.len(), 3);
        let var_names: Vec<String> = vars.iter().map(|s| s.to_string()).collect();
        assert_eq!(var_names, vec!["a", "b", "c"]); // Should be alphabetical
    }

    // ========== Display Formatting Tests ==========

    #[test]
    fn test_display_simple_variable() {
        let a = BoolExpr::variable("a");
        assert_eq!(format!("{}", a), "a");
        assert_eq!(format!("{:?}", a), "a");
    }

    #[test]
    fn test_display_constants() {
        let t = BoolExpr::constant(true);
        let f = BoolExpr::constant(false);
        assert_eq!(format!("{}", t), "1");
        assert_eq!(format!("{}", f), "0");
    }

    #[test]
    fn test_display_simple_and() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let expr = a.and(&b);

        // Simple AND should have no parentheses
        assert_eq!(format!("{}", expr), "a * b");
    }

    #[test]
    fn test_display_simple_or() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let expr = a.or(&b);

        // Simple OR should have no parentheses
        assert_eq!(format!("{}", expr), "a + b");
    }

    #[test]
    fn test_display_simple_not() {
        let a = BoolExpr::variable("a");
        let expr = a.not();

        // NOT of variable should have no parentheses
        assert_eq!(format!("{}", expr), "~a");
    }

    #[test]
    fn test_display_and_then_or() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let expr = a.and(&b).or(&c);

        // AND has higher precedence than OR, so no parentheses needed
        assert_eq!(format!("{}", expr), "a * b + c");
    }

    #[test]
    fn test_display_or_then_and() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let expr = a.or(&b).and(&c);

        // OR has lower precedence, needs parentheses when inside AND
        assert_eq!(format!("{}", expr), "(a + b) * c");
    }

    #[test]
    fn test_display_multiple_and() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let expr = a.and(&b).and(&c);

        // Chained AND operations, no parentheses needed
        assert_eq!(format!("{}", expr), "a * b * c");
    }

    #[test]
    fn test_display_multiple_or() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let expr = a.or(&b).or(&c);

        // Chained OR operations, no parentheses needed
        assert_eq!(format!("{}", expr), "a + b + c");
    }

    #[test]
    fn test_display_not_of_and() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let expr = a.and(&b).not();

        // NOT of compound expression needs parentheses
        assert_eq!(format!("{}", expr), "~(a * b)");
    }

    #[test]
    fn test_display_not_of_or() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let expr = a.or(&b).not();

        // NOT of compound expression needs parentheses
        assert_eq!(format!("{}", expr), "~(a + b)");
    }

    #[test]
    fn test_display_xor_like() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        // XOR-like: a*b + ~a*~b
        let expr = a.and(&b).or(&a.not().and(&b.not()));

        // No unnecessary parentheses
        assert_eq!(format!("{}", expr), "a * b + ~a * ~b");
    }

    #[test]
    fn test_display_xnor_like() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        // XOR-like (not XNOR): a*~b + ~a*b
        // Build using reference NOT operator
        let expr = a.and(&(!&b)).or(&(!&a).and(&b));

        // No unnecessary parentheses
        assert_eq!(format!("{}", expr), "a * ~b + ~a * b");
    }

    #[test]
    fn test_display_complex_nested() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let d = BoolExpr::variable("d");

        // (a + b) * (c + d)
        let expr = a.or(&b).and(&c.or(&d));

        // Both ORs need parentheses when inside AND
        assert_eq!(format!("{}", expr), "(a + b) * (c + d)");
    }

    #[test]
    fn test_display_nested_or_in_and() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");

        // a * (b + c)
        let expr = a.and(&b.or(&c));

        // OR needs parentheses when inside AND
        assert_eq!(format!("{}", expr), "a * (b + c)");
    }

    #[test]
    fn test_display_nested_and_in_or() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");

        // a + b * c
        let expr = a.or(&b.and(&c));

        // AND has higher precedence, no parentheses needed
        assert_eq!(format!("{}", expr), "a + b * c");
    }

    #[test]
    fn test_display_double_negation() {
        let a = BoolExpr::variable("a");
        let expr = a.not().not();

        // Double negation
        assert_eq!(format!("{}", expr), "~~a");
    }

    #[test]
    fn test_display_not_in_and() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let expr = a.not().and(&b);

        // NOT has highest precedence, no extra parens
        assert_eq!(format!("{}", expr), "~a * b");
    }

    #[test]
    fn test_display_not_in_or() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let expr = a.not().or(&b);

        // NOT has highest precedence, no extra parens
        assert_eq!(format!("{}", expr), "~a + b");
    }

    #[test]
    fn test_display_majority_function() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");

        // Majority: a*b + b*c + a*c
        let expr = a.and(&b).or(&b.and(&c)).or(&a.and(&c));

        // Clean formatting with no unnecessary parentheses
        assert_eq!(format!("{}", expr), "a * b + b * c + a * c");
    }

    #[test]
    fn test_display_with_constants() {
        let a = BoolExpr::variable("a");
        let t = BoolExpr::constant(true);
        let f = BoolExpr::constant(false);

        assert_eq!(format!("{}", a.and(&t)), "a * 1");
        assert_eq!(format!("{}", a.or(&f)), "a + 0");
        assert_eq!(format!("{}", t.not()), "~1");
    }

    #[test]
    fn test_display_deeply_nested() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let d = BoolExpr::variable("d");

        // ((a + b) * c) + d - should minimize parens
        let expr = a.or(&b).and(&c).or(&d);
        assert_eq!(format!("{}", expr), "(a + b) * c + d");

        // a * ((b + c) * d)
        let expr2 = a.and(&b.or(&c).and(&d));
        assert_eq!(format!("{}", expr2), "a * (b + c) * d");
    }

    #[test]
    fn test_display_not_of_complex() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");

        // ~(a*b + c)
        let expr = a.and(&b).or(&c).not();
        assert_eq!(format!("{}", expr), "~(a * b + c)");

        // ~((a + b) * c)
        let expr2 = a.or(&b).and(&c).not();
        assert_eq!(format!("{}", expr2), "~((a + b) * c)");
    }

    // ========== Roundtrip Tests (Display -> Parse -> Display) ==========

    #[test]
    fn test_roundtrip_simple_and() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let expr = a.and(&b);

        let display = format!("{}", expr);
        let parsed = BoolExpr::parse(&display).unwrap();
        let display2 = format!("{}", parsed);

        assert_eq!(display, "a * b");
        assert_eq!(display, display2);
    }

    #[test]
    fn test_roundtrip_simple_or() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let expr = a.or(&b);

        let display = format!("{}", expr);
        let parsed = BoolExpr::parse(&display).unwrap();
        let display2 = format!("{}", parsed);

        assert_eq!(display, "a + b");
        assert_eq!(display, display2);
    }

    #[test]
    fn test_roundtrip_and_then_or() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let expr = a.and(&b).or(&c);

        let display = format!("{}", expr);
        let parsed = BoolExpr::parse(&display).unwrap();
        let display2 = format!("{}", parsed);

        assert_eq!(display, "a * b + c");
        assert_eq!(display, display2);
    }

    #[test]
    fn test_roundtrip_or_then_and() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let expr = a.or(&b).and(&c);

        let display = format!("{}", expr);
        let parsed = BoolExpr::parse(&display).unwrap();
        let display2 = format!("{}", parsed);

        assert_eq!(display, "(a + b) * c");
        assert_eq!(display, display2);
    }

    #[test]
    fn test_roundtrip_not_of_and() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let expr = a.and(&b).not();

        let display = format!("{}", expr);
        let parsed = BoolExpr::parse(&display).unwrap();
        let display2 = format!("{}", parsed);

        assert_eq!(display, "~(a * b)");
        assert_eq!(display, display2);
    }

    #[test]
    fn test_roundtrip_not_of_or() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let expr = a.or(&b).not();

        let display = format!("{}", expr);
        let parsed = BoolExpr::parse(&display).unwrap();
        let display2 = format!("{}", parsed);

        assert_eq!(display, "~(a + b)");
        assert_eq!(display, display2);
    }

    #[test]
    fn test_roundtrip_xor_like() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let expr = a.and(&b).or(&a.not().and(&b.not()));

        let display = format!("{}", expr);
        let parsed = BoolExpr::parse(&display).unwrap();
        let display2 = format!("{}", parsed);

        assert_eq!(display, "a * b + ~a * ~b");
        assert_eq!(display, display2);
    }

    #[test]
    fn test_roundtrip_complex_nested() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let d = BoolExpr::variable("d");
        let expr = a.or(&b).and(&c.or(&d));

        let display = format!("{}", expr);
        let parsed = BoolExpr::parse(&display).unwrap();
        let display2 = format!("{}", parsed);

        assert_eq!(display, "(a + b) * (c + d)");
        assert_eq!(display, display2);
    }

    #[test]
    fn test_roundtrip_majority() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let expr = a.and(&b).or(&b.and(&c)).or(&a.and(&c));

        let display = format!("{}", expr);
        let parsed = BoolExpr::parse(&display).unwrap();
        let display2 = format!("{}", parsed);

        assert_eq!(display, "a * b + b * c + a * c");
        assert_eq!(display, display2);
    }

    #[test]
    fn test_roundtrip_double_negation() {
        let a = BoolExpr::variable("a");
        let expr = a.not().not();

        let display = format!("{}", expr);
        let parsed = BoolExpr::parse(&display).unwrap();
        let display2 = format!("{}", parsed);

        assert_eq!(display, "~~a");
        assert_eq!(display, display2);
    }

    #[test]
    fn test_roundtrip_deeply_nested() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let d = BoolExpr::variable("d");
        let expr = a.or(&b).and(&c).or(&d);

        let display = format!("{}", expr);
        let parsed = BoolExpr::parse(&display).unwrap();
        let display2 = format!("{}", parsed);

        assert_eq!(display, "(a + b) * c + d");
        assert_eq!(display, display2);
    }

    #[test]
    fn test_roundtrip_with_constants() {
        let a = BoolExpr::variable("a");
        let t = BoolExpr::constant(true);
        let expr = a.and(&t);

        let display = format!("{}", expr);
        let parsed = BoolExpr::parse(&display).unwrap();
        let display2 = format!("{}", parsed);

        assert_eq!(display, "a * 1");
        assert_eq!(display, display2);
    }

    // ========== Macro Tests (expr! macro) ==========

    #[test]
    fn test_operator_overloading_and() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");

        let manual = a.and(&b);
        let with_ops = &a * &b;

        assert_eq!(manual, with_ops);
        assert_eq!(format!("{}", with_ops), "a * b");
    }

    #[test]
    fn test_operator_overloading_or() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");

        let manual = a.or(&b);
        let with_ops = &a + &b;

        assert_eq!(manual, with_ops);
        assert_eq!(format!("{}", with_ops), "a + b");
    }

    #[test]
    fn test_operator_overloading_not() {
        let a = BoolExpr::variable("a");

        let manual = a.not();

        let a2 = BoolExpr::variable("a");
        let with_ops = !&a2;

        assert_eq!(manual, with_ops);
        assert_eq!(format!("{}", with_ops), "~a");
    }

    #[test]
    fn test_operator_overloading_and_then_or() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");

        let manual = a.and(&b).or(&c);
        let with_ops = (&a * &b).or(&c);

        assert_eq!(manual, with_ops);
        assert_eq!(format!("{}", with_ops), "a * b + c");
    }

    #[test]
    fn test_operator_overloading_xor_pattern() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");

        let manual = a.and(&b).or(&a.not().and(&b.not()));

        let a2 = BoolExpr::variable("a");
        let b2 = BoolExpr::variable("b");
        let with_ops = &a2 * &b2 + &(!&a2) * &(!&b2);

        assert_eq!(manual, with_ops);
        assert_eq!(format!("{}", with_ops), "a * b + ~a * ~b");
    }

    #[test]
    fn test_operator_overloading_with_parens() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");

        let manual = a.or(&b).and(&c);
        let with_ops = (&a + &b).and(&c);

        assert_eq!(manual, with_ops);
        assert_eq!(format!("{}", with_ops), "(a + b) * c");
    }

    #[test]
    fn test_operator_overloading_not_of_expression() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");

        let manual = a.and(&b).not();
        let with_ops = !(&a * &b);

        assert_eq!(manual, with_ops);
        assert_eq!(format!("{}", with_ops), "~(a * b)");
    }

    #[test]
    fn test_operator_overloading_complex_nested() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let d = BoolExpr::variable("d");

        let manual = a.or(&b).and(&c.or(&d));
        let with_ops = (&a + &b) * (&c + &d);

        assert_eq!(manual, with_ops);
        assert_eq!(format!("{}", with_ops), "(a + b) * (c + d)");
    }

    #[test]
    fn test_operator_overloading_multiple_not() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");

        let manual = a.not().and(&b.not());

        let a2 = BoolExpr::variable("a");
        let b2 = BoolExpr::variable("b");
        let with_ops = (!&a2) * (!&b2);

        assert_eq!(manual, with_ops);
        assert_eq!(format!("{}", with_ops), "~a * ~b");
    }

    #[test]
    fn test_operator_overloading_three_way_and() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");

        let manual = a.and(&b).and(&c);
        let with_ops = (&a * &b).and(&c);

        assert_eq!(manual, with_ops);
        assert_eq!(format!("{}", with_ops), "a * b * c");
    }

    // ========== Combined Roundtrip + Operator Tests ==========

    #[test]
    fn test_operator_roundtrip_xor() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");

        // Build with operators
        let expr_built = (&a) * (&b) + (&(!&a)) * (&(!&b));
        let display = format!("{}", expr_built);

        // Parse it back
        let parsed = BoolExpr::parse(&display).unwrap();
        let display2 = format!("{}", parsed);

        // Should be stable
        assert_eq!(display, "a * b + ~a * ~b");
        assert_eq!(display, display2);
    }

    #[test]
    fn test_operator_roundtrip_complex() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let d = BoolExpr::variable("d");

        // Build with operators
        let expr_built = (&a + &b) * (&c + &d);
        let display = format!("{}", expr_built);

        // Parse it back
        let parsed = BoolExpr::parse(&display).unwrap();
        let display2 = format!("{}", parsed);

        // Should be stable
        assert_eq!(display, "(a + b) * (c + d)");
        assert_eq!(display, display2);
    }

    #[test]
    fn test_parse_display_operator_equivalence() {
        // All three methods should produce equivalent results
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");

        // Manual construction
        let manual = a.and(&b).or(&c);

        // Operator construction
        let with_ops = (&a * &b).or(&c);

        // Parse from string
        let from_parse = BoolExpr::parse("a * b + c").unwrap();

        // Macro construction
        let from_macro = expr!(a * b + c);

        // All four should produce the same structure
        assert_eq!(manual, with_ops);
        assert_eq!(manual, from_parse);
        assert_eq!(manual, from_macro);
        assert_eq!(with_ops, from_parse);

        // All should display the same
        let display1 = format!("{}", manual);
        let display2 = format!("{}", with_ops);
        let display3 = format!("{}", from_parse);
        let display4 = format!("{}", from_macro);

        assert_eq!(display1, "a * b + c");
        assert_eq!(display1, display2);
        assert_eq!(display1, display3);
        assert_eq!(display1, display4);
    }

    // ========== Procedural Macro Tests (expr!) ==========

    #[test]
    fn test_expr_macro_simple_and() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");

        let macro_expr = expr!(a * b);
        let manual = a.and(&b);

        assert_eq!(macro_expr, manual);
        assert_eq!(format!("{}", macro_expr), "a * b");
    }

    #[test]
    fn test_expr_macro_simple_or() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");

        let macro_expr = expr!(a + b);
        let manual = a.or(&b);

        assert_eq!(macro_expr, manual);
        assert_eq!(format!("{}", macro_expr), "a + b");
    }

    #[test]
    fn test_expr_macro_simple_not() {
        let a = BoolExpr::variable("a");

        let macro_expr = expr!(!a);
        let manual = a.not();

        assert_eq!(macro_expr, manual);
        assert_eq!(format!("{}", macro_expr), "~a");
    }

    #[test]
    fn test_expr_macro_and_then_or() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");

        let macro_expr = expr!(a * b + c);
        let manual = a.and(&b).or(&c);

        assert_eq!(macro_expr, manual);
        assert_eq!(format!("{}", macro_expr), "a * b + c");
    }

    #[test]
    fn test_expr_macro_xor_pattern() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");

        let macro_expr = expr!(a * b + !a * !b);
        let manual = a.and(&b).or(&a.not().and(&b.not()));

        assert_eq!(macro_expr, manual);
        assert_eq!(format!("{}", macro_expr), "a * b + ~a * ~b");
    }

    #[test]
    fn test_expr_macro_with_parens() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");

        let macro_expr = expr!((a + b) * c);
        let manual = a.or(&b).and(&c);

        assert_eq!(macro_expr, manual);
        assert_eq!(format!("{}", macro_expr), "(a + b) * c");
    }

    #[test]
    fn test_expr_macro_not_of_expression() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");

        let macro_expr = expr!(!(a * b));
        let manual = a.and(&b).not();

        assert_eq!(macro_expr, manual);
        assert_eq!(format!("{}", macro_expr), "~(a * b)");
    }

    #[test]
    fn test_expr_macro_complex_nested() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let d = BoolExpr::variable("d");

        let macro_expr = expr!((a + b) * (c + d));
        let manual = a.or(&b).and(&c.or(&d));

        assert_eq!(macro_expr, manual);
        assert_eq!(format!("{}", macro_expr), "(a + b) * (c + d)");
    }

    #[test]
    fn test_expr_macro_multiple_not() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");

        let macro_expr = expr!(!a * !b);
        let manual = a.not().and(&b.not());

        assert_eq!(macro_expr, manual);
        assert_eq!(format!("{}", macro_expr), "~a * ~b");
    }

    #[test]
    fn test_expr_macro_three_way_and() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");

        let macro_expr = expr!(a * b * c);
        let manual = a.and(&b).and(&c);

        assert_eq!(macro_expr, manual);
        assert_eq!(format!("{}", macro_expr), "a * b * c");
    }

    #[test]
    fn test_expr_macro_three_way_or() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");

        let macro_expr = expr!(a + b + c);
        let manual = a.or(&b).or(&c);

        assert_eq!(macro_expr, manual);
        assert_eq!(format!("{}", macro_expr), "a + b + c");
    }

    #[test]
    fn test_expr_macro_majority_function() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");

        let macro_expr = expr!(a * b + b * c + a * c);
        let manual = a.and(&b).or(&b.and(&c)).or(&a.and(&c));

        assert_eq!(macro_expr, manual);
        assert_eq!(format!("{}", macro_expr), "a * b + b * c + a * c");
    }

    #[test]
    fn test_expr_macro_double_negation() {
        let a = BoolExpr::variable("a");

        let macro_expr = expr!(!!a);
        let manual = a.not().not();

        assert_eq!(macro_expr, manual);
        assert_eq!(format!("{}", macro_expr), "~~a");
    }

    #[test]
    fn test_expr_macro_deeply_nested() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let d = BoolExpr::variable("d");

        let macro_expr = expr!((a + b) * c + d);
        let manual = a.or(&b).and(&c).or(&d);

        assert_eq!(macro_expr, manual);
        assert_eq!(format!("{}", macro_expr), "(a + b) * c + d");
    }

    #[test]
    fn test_expr_macro_equivalence_with_manual() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");

        // Macro version
        let macro_expr = expr!(a * b + !a * !b);

        // Manual version
        let manual_expr = a.and(&b).or(&a.not().and(&b.not()));

        // Should be structurally equal
        assert_eq!(macro_expr, manual_expr);
        assert_eq!(format!("{}", macro_expr), format!("{}", manual_expr));
    }

    #[test]
    fn test_expr_macro_roundtrip() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");

        let expr = expr!(a * b + !c);
        let display = format!("{}", expr);

        // Parse it back
        let parsed = BoolExpr::parse(&display).unwrap();
        let display2 = format!("{}", parsed);

        // Should be stable
        assert_eq!(display, display2);
        assert!(expr.equivalent_to(&parsed));
    }

    #[test]
    fn test_expr_macro_with_sub_expressions() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");

        // Build sub-expressions
        let sub1 = expr!(a * b);
        let sub2 = expr!(c + !a);

        // Combine them
        let combined = expr!(sub1 + sub2);

        // Should work correctly
        let manual = a.and(&b).or(&c.or(&a.not()));
        assert_eq!(combined, manual);
    }

    // ========== String Literal Tests (automatic variable creation) ==========

    #[test]
    fn test_expr_macro_string_simple_and() {
        let macro_expr = expr!("a" * "b");

        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let manual = a.and(&b);

        assert_eq!(macro_expr, manual);
        assert_eq!(format!("{}", macro_expr), "a * b");
    }

    #[test]
    fn test_expr_macro_string_simple_or() {
        let macro_expr = expr!("a" + "b");

        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let manual = a.or(&b);

        assert_eq!(macro_expr, manual);
        assert_eq!(format!("{}", macro_expr), "a + b");
    }

    #[test]
    fn test_expr_macro_string_xor() {
        let macro_expr = expr!("a" * "b" + !"a" * !"b");

        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let manual = a.and(&b).or(&a.not().and(&b.not()));

        assert_eq!(macro_expr, manual);
        assert_eq!(format!("{}", macro_expr), "a * b + ~a * ~b");
    }

    #[test]
    fn test_expr_macro_string_complex() {
        let macro_expr = expr!(("a" + "b") * ("c" + "d"));

        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let d = BoolExpr::variable("d");
        let manual = a.or(&b).and(&c.or(&d));

        assert_eq!(macro_expr, manual);
        assert_eq!(format!("{}", macro_expr), "(a + b) * (c + d)");
    }

    #[test]
    fn test_expr_macro_mixed_string_and_var() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");

        // Mix existing variables with string literals
        let macro_expr = expr!(a * "c" + b);

        let c = BoolExpr::variable("c");
        let manual = a.and(&c).or(&b);

        assert_eq!(macro_expr, manual);
        assert_eq!(format!("{}", macro_expr), "a * c + b");
    }

    #[test]
    fn test_expr_macro_string_no_variable_declaration() {
        // Most concise syntax - no variable declarations needed!
        let expr = expr!("x" * "y" + "z");

        assert_eq!(format!("{}", expr), "x * y + z");

        // Verify it works correctly
        let vars = expr.collect_variables();
        assert_eq!(vars.len(), 3);
    }

    // ========== Semantic Equivalence Tests ==========

    #[test]
    fn test_commutative_and() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");

        let expr1 = a.and(&b);
        let expr2 = b.and(&a);

        // Structurally different
        assert_ne!(expr1, expr2);
        // But logically equivalent
        assert!(expr1.equivalent_to(&expr2));
    }

    #[test]
    fn test_commutative_or() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");

        let expr1 = a.or(&b);
        let expr2 = b.or(&a);

        // Structurally different
        assert_ne!(expr1, expr2);
        // But logically equivalent
        assert!(expr1.equivalent_to(&expr2));
    }

    #[test]
    fn test_double_negation() {
        let a = BoolExpr::variable("a");

        let expr1 = a.clone();
        let expr2 = a.not().not();

        // Structurally different
        assert_ne!(expr1, expr2);
        // But logically equivalent
        assert!(expr1.equivalent_to(&expr2));
    }

    #[test]
    fn test_not_equivalent() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");

        let and_expr = a.and(&b);
        let or_expr = a.or(&b);

        // Different operations should not be equivalent
        assert_ne!(and_expr, or_expr);
        assert!(!and_expr.equivalent_to(&or_expr));
    }
}
