//! Operator overloading for boolean expressions

use super::BoolExpr;
use std::ops::{Add, Mul, Not};

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
