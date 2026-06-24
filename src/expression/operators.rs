//! Operator overloading and boolean operations for boolean expressions

use super::builder::build_in;
use super::context::Brand;
use super::BoolExpr;
use std::ops::{Add, BitXor, Mul, Not};

/// Logical AND operator for references: `&a * &b`
///
/// Implements the `*` operator for boolean expressions using references.
/// This form avoids cloning the operands.
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
impl<B: Brand> Mul for &BoolExpr<B> {
    type Output = BoolExpr<B>;

    fn mul(self, rhs: &BoolExpr<B>) -> BoolExpr<B> {
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
impl<B: Brand> Mul for BoolExpr<B> {
    type Output = BoolExpr<B>;

    fn mul(self, rhs: BoolExpr<B>) -> BoolExpr<B> {
        self.and(&rhs)
    }
}

/// Logical OR operator for references: `&a + &b`
///
/// Implements the `+` operator for boolean expressions using references.
/// This form avoids cloning the operands.
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
impl<B: Brand> Add for &BoolExpr<B> {
    type Output = BoolExpr<B>;

    fn add(self, rhs: &BoolExpr<B>) -> BoolExpr<B> {
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
impl<B: Brand> Add for BoolExpr<B> {
    type Output = BoolExpr<B>;

    fn add(self, rhs: BoolExpr<B>) -> BoolExpr<B> {
        self.or(&rhs)
    }
}

/// Logical XOR operator for references: `&a ^ &b`
///
/// Implements the `^` operator for boolean expressions using references.
/// This form avoids cloning the operands.
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
impl<B: Brand> BitXor for &BoolExpr<B> {
    type Output = BoolExpr<B>;

    fn bitxor(self, rhs: &BoolExpr<B>) -> BoolExpr<B> {
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
impl<B: Brand> BitXor for BoolExpr<B> {
    type Output = BoolExpr<B>;

    fn bitxor(self, rhs: BoolExpr<B>) -> BoolExpr<B> {
        self.xor(&rhs)
    }
}

/// Logical NOT operator for references: `!&a`
///
/// Implements the `!` operator for boolean expressions using references.
/// This form avoids cloning the operands.
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
impl<B: Brand> Not for &BoolExpr<B> {
    type Output = BoolExpr<B>;

    fn not(self) -> BoolExpr<B> {
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
impl<B: Brand> Not for BoolExpr<B> {
    type Output = BoolExpr<B>;

    fn not(self) -> BoolExpr<B> {
        BoolExpr::not(&self)
    }
}

// Boolean operation methods
impl<B: Brand> BoolExpr<B> {
    /// Logical AND: create a new expression that is the conjunction of this and another
    ///
    /// Computes the conjunction using the BDD ITE operation:
    /// `and(f, g) = ite(f, g, false)`
    #[must_use]
    pub fn and(&self, other: &BoolExpr<B>) -> BoolExpr<B> {
        // and(f, g) = ite(f, g, false). A thin shim over the builder, the single manager-acquisition
        // point; `graft` debug-asserts both operands belong to this expression's manager.
        build_in(self.store_cloned(), |b| {
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
    pub fn or(&self, other: &BoolExpr<B>) -> BoolExpr<B> {
        // or(f, g) = ite(f, true, g). Thin shim over the builder (see `and`).
        build_in(self.store_cloned(), |b| {
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
    pub fn not(&self) -> BoolExpr<B> {
        // not(f) = ite(f, false, true). Thin shim over the builder (see `and`).
        build_in(self.store_cloned(), |b| {
            let f = b.graft(self);
            b.not(f)
        })
    }

    /// Logical XOR: create a new expression that is the exclusive-or of this and another
    ///
    /// Computes the exclusive-or using the BDD ITE operation:
    /// `xor(f, g) = ite(f, ¬g, g)` — equivalently `f*¬g + ¬f*g`.
    #[must_use]
    pub fn xor(&self, other: &BoolExpr<B>) -> BoolExpr<B> {
        // xor(f, g) = ite(f, !g, g). Thin shim over the builder (see `and`).
        build_in(self.store_cloned(), |b| {
            let f = b.graft(self);
            let g = b.graft(other);
            b.xor(f, g)
        })
    }
}
