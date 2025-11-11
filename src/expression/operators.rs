//! Operator overloading and boolean operations for boolean expressions

use super::manager::{FALSE_NODE, TRUE_NODE};
use super::BoolExpr;
use std::ops::{Add, Mul, Not};
use std::sync::{Arc, OnceLock};

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

// Boolean operation methods
impl BoolExpr {
    /// Logical AND: create a new expression that is the conjunction of this and another
    ///
    /// Computes the conjunction using the BDD ITE operation:
    /// `and(f, g) = ite(f, g, false)`
    pub fn and(&self, other: &BoolExpr) -> BoolExpr {
        // Use ITE: and(f, g) = ite(f, g, false)
        let manager = Arc::clone(&self.manager);
        let result = manager
            .write()
            .unwrap()
            .ite(self.root, other.root, FALSE_NODE);
        BoolExpr {
            manager,
            root: result,
            dnf_cache: OnceLock::new(),
            ast_cache: OnceLock::new(),
        }
    }

    /// Logical OR: create a new expression that is the disjunction of this and another
    ///
    /// Computes the disjunction using the BDD ITE operation:
    /// `or(f, g) = ite(f, true, g)`
    pub fn or(&self, other: &BoolExpr) -> BoolExpr {
        // Use ITE: or(f, g) = ite(f, true, g)
        let manager = Arc::clone(&self.manager);
        let result = manager
            .write()
            .unwrap()
            .ite(self.root, TRUE_NODE, other.root);
        BoolExpr {
            manager,
            root: result,
            dnf_cache: OnceLock::new(),
            ast_cache: OnceLock::new(),
        }
    }

    /// Logical NOT: create a new expression that is the negation of this one
    ///
    /// Computes the negation using the BDD ITE operation:
    /// `not(f) = ite(f, false, true)`
    pub fn not(&self) -> BoolExpr {
        // Use ITE: not(f) = ite(f, false, true)
        let manager = Arc::clone(&self.manager);
        let result = manager
            .write()
            .unwrap()
            .ite(self.root, FALSE_NODE, TRUE_NODE);
        BoolExpr {
            manager,
            root: result,
            dnf_cache: OnceLock::new(),
            ast_cache: OnceLock::new(),
        }
    }
}
