//! Operator overloading and boolean operations for boolean expressions

use super::BoolExpr;
use std::ops::{Add, BitXor, Mul, Not};

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
/// assert!(result.equivalent_to(&a.and(&b)));
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

/// Logical XOR operator for references: `&a ^ &b`
///
/// Implements the `^` operator for boolean expressions using references.
/// This is the most efficient form as it avoids unnecessary cloning.
///
/// # Examples
///
/// ```
/// use espresso_logic::BoolExpr;
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let result = &a ^ &b;  // Equivalent to a.xor(&b)
/// assert!(result.equivalent_to(&a.xor(&b)));
/// ```
impl BitXor for &BoolExpr {
    type Output = BoolExpr;

    fn bitxor(self, rhs: &BoolExpr) -> BoolExpr {
        self.xor(rhs)
    }
}

/// Logical XOR operator: `a ^ b` (delegates to reference version)
///
/// Implements the `^` operator for owned boolean expressions.
/// Note: Using references (`&a ^ &b`) is preferred for efficiency.
///
/// # Examples
///
/// ```
/// use espresso_logic::BoolExpr;
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// // Both work, but references are preferred
/// let result1 = a.clone() ^ b.clone();
/// let result2 = &a ^ &b;
/// ```
impl BitXor for BoolExpr {
    type Output = BoolExpr;

    fn bitxor(self, rhs: BoolExpr) -> BoolExpr {
        self.xor(&rhs)
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
/// assert!(result.equivalent_to(&a.not()));
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

// Boolean operation methods
impl BoolExpr {
    /// Logical AND: create a new expression that is the conjunction of this and another
    ///
    /// Computes the conjunction using the BDD ITE operation:
    /// `and(f, g) = ite(f, g, false)`
    #[must_use]
    pub fn and(&self, other: &BoolExpr) -> BoolExpr {
        // and(f, g) = ite(f, g, false). A thin shim over `build`, the single manager-acquisition point;
        // `graft` debug-asserts both operands belong to that (global) manager.
        BoolExpr::build(|b| {
            let f = b.graft(self);
            let g = b.graft(other);
            b.and(f, g)
        })
    }

    /// Logical OR: create a new expression that is the disjunction of this and another
    ///
    /// Computes the disjunction using the BDD ITE operation:
    /// `or(f, g) = ite(f, true, g)`
    #[must_use]
    pub fn or(&self, other: &BoolExpr) -> BoolExpr {
        // or(f, g) = ite(f, true, g). Thin shim over `build` (see `and`).
        BoolExpr::build(|b| {
            let f = b.graft(self);
            let g = b.graft(other);
            b.or(f, g)
        })
    }

    /// Logical NOT: create a new expression that is the negation of this one
    ///
    /// Computes the negation using the BDD ITE operation:
    /// `not(f) = ite(f, false, true)`
    #[must_use]
    pub fn not(&self) -> BoolExpr {
        // not(f) = ite(f, false, true). Thin shim over `build` (see `and`).
        BoolExpr::build(|b| {
            let f = b.graft(self);
            b.not(f)
        })
    }

    /// Logical XOR: create a new expression that is the exclusive-or of this and another
    ///
    /// Computes the exclusive-or using the BDD ITE operation:
    /// `xor(f, g) = ite(f, ¬g, g)` — equivalently `f*¬g + ¬f*g`.
    #[must_use]
    pub fn xor(&self, other: &BoolExpr) -> BoolExpr {
        // xor(f, g) = ite(f, !g, g). Thin shim over `build` (see `and`).
        BoolExpr::build(|b| {
            let f = b.graft(self);
            let g = b.graft(other);
            b.xor(f, g)
        })
    }
}
