//! Disjunctive Normal Form (DNF) representation for boolean functions
//!
//! This module provides the [`Dnf`] type, which represents boolean functions in
//! Disjunctive Normal Form (sum of products). DNF serves as the intermediary
//! representation between boolean expressions and covers in the minimization pipeline.
//!
//! # Role in Minimization
//!
//! The DNF type is central to the minimization workflow:
//! 1. Boolean expressions ([`BoolExpr`]) are converted to BDD for canonical representation
//! 2. BDDs are converted to DNF (extracting cubes)
//! 3. DNF is used to populate [`Cover`] with product terms
//! 4. Espresso minimizes the cover
//! 5. Minimized cover is converted back through DNF to boolean expressions
//!
//! # Efficient Conversion Path
//!
//! All conversions to DNF go through BDD to ensure:
//! - Canonical representation (equivalent expressions produce identical DNF)
//! - Automatic optimizations (redundancy elimination, simplification)
//! - Efficient cube extraction
//!
//! [`BoolExpr`]: crate::expression::BoolExpr
//! [`Cover`]: crate::Cover
use std::collections::BTreeMap;
use std::sync::Arc;

/// Inner data for Dnf (wrapped in Arc for cheap cloning)
#[derive(Debug, PartialEq, Eq)]
struct DnfInner {
    /// Product terms (cubes), each mapping variables to their polarity
    cubes: Vec<BTreeMap<Arc<str>, bool>>,
    /// Cached list of all variables (sorted alphabetically)
    variables: Vec<Arc<str>>,
}

/// Disjunctive Normal Form representation of a boolean function
///
/// A DNF is a sum (OR) of products (AND), where each product term is called a "cube".
/// Each cube is represented as a map from variable names to their polarity:
/// - `true` means the variable appears positively (e.g., `a`)
/// - `false` means the variable appears negatively (e.g., `~a`)
///
/// Uses `Arc` internally for efficient cloning.
///
/// # Examples
///
/// ```
/// use espresso_logic::{BoolExpr, Dnf};
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let expr = a.and(&b).or(&a.not().and(&b.not()));
///
/// // Convert to DNF
/// let dnf = Dnf::from(&expr);
///
/// // DNF contains two cubes: {a: true, b: true} and {a: false, b: false}
/// assert_eq!(dnf.len(), 2);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dnf {
    inner: Arc<DnfInner>,
}

impl Dnf {
    /// Create a new empty DNF (represents FALSE)
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::Dnf;
    ///
    /// let dnf = Dnf::new();
    /// assert!(dnf.is_empty());
    /// ```
    pub fn new() -> Self {
        Dnf {
            inner: Arc::new(DnfInner {
                cubes: Vec::new(),
                variables: Vec::new(),
            }),
        }
    }

    /// Create a DNF from a slice of cubes
    ///
    /// This is primarily for internal use. Users should typically convert
    /// from [`BoolExpr`] or [`Bdd`] instead.
    ///
    /// [`BoolExpr`]: crate::expression::BoolExpr
    /// [`Bdd`]: crate::expression::Bdd
    pub fn from_cubes(cubes: &[BTreeMap<Arc<str>, bool>]) -> Self {
        // Collect all variables from cubes using BTreeSet for sorting
        let mut var_set = std::collections::BTreeSet::new();
        for cube in cubes {
            for var in cube.keys() {
                var_set.insert(Arc::clone(var));
            }
        }
        // Convert to Vec for efficient slicing
        let variables: Vec<_> = var_set.into_iter().collect();

        Dnf {
            inner: Arc::new(DnfInner {
                cubes: cubes.to_vec(),
                variables,
            }),
        }
    }

    /// Check if the DNF is empty (represents FALSE)
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{BoolExpr, Dnf};
    ///
    /// let f = BoolExpr::constant(false);
    /// let dnf = Dnf::from(&f);
    /// assert!(dnf.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.inner.cubes.is_empty()
    }

    /// Get the number of cubes (product terms) in the DNF
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{BoolExpr, Dnf};
    ///
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    /// let expr = a.or(&b);
    ///
    /// let dnf = Dnf::from(&expr);
    /// assert_eq!(dnf.len(), 2); // Two cubes: a and b
    /// ```
    pub fn len(&self) -> usize {
        self.inner.cubes.len()
    }

    /// Get an iterator over the cubes
    ///
    /// Each cube is a map from variable names to their polarity.
    pub fn iter(&self) -> impl Iterator<Item = &BTreeMap<Arc<str>, bool>> {
        self.inner.cubes.iter()
    }

    /// Get a reference to the cubes
    ///
    /// This provides direct access to the underlying cube representation.
    pub fn cubes(&self) -> &[BTreeMap<Arc<str>, bool>] {
        &self.inner.cubes
    }

    /// Get the cached variables (sorted alphabetically)
    ///
    /// Returns a slice of all variables that appear in the DNF.
    pub fn variables(&self) -> &[Arc<str>] {
        &self.inner.variables
    }
}

impl Default for Dnf {
    fn default() -> Self {
        Self::new()
    }
}
// Note: Conversion trait implementations (From<BoolExpr> for Dnf, From<Dnf> for BoolExpr)
// have been moved to src/expression/conversions.rs

// ============================================================================
// Blanket Minimizable Implementation
// ============================================================================

// Note: Blanket implementation of Minimizable for types convertible to/from Dnf
// has been moved to cover/minimisation.rs

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expression::{Bdd, BoolExpr};

    #[test]
    fn test_dnf_creation() {
        let dnf = Dnf::new();
        assert!(dnf.is_empty());
        assert_eq!(dnf.len(), 0);
    }

    #[test]
    fn test_dnf_from_bool_expr() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let expr = a.and(&b);

        let dnf = Dnf::from(&expr);
        assert_eq!(dnf.len(), 1); // One cube: a AND b
    }

    #[test]
    fn test_dnf_from_bdd() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let expr = a.or(&b);

        let dnf = Dnf::from(&expr);
        assert_eq!(dnf.len(), 2); // Two cubes
    }

    #[test]
    fn test_dnf_to_bool_expr() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let expr = a.and(&b);

        let dnf = Dnf::from(&expr);
        let expr2 = BoolExpr::from(dnf);

        assert!(expr.equivalent_to(&expr2));
    }

    #[test]
    fn test_dnf_to_bdd() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let expr = a.or(&b);

        let dnf = Dnf::from(&expr);
        let bdd = Bdd::from(dnf);

        assert_eq!(bdd, expr);
    }

    #[test]
    fn test_roundtrip_bool_expr() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let expr = a.and(&b).or(&c);

        let dnf = Dnf::from(&expr);
        let expr2 = BoolExpr::from(dnf);

        assert!(expr.equivalent_to(&expr2));
    }

    #[test]
    fn test_roundtrip_bdd() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let expr = a.and(&b);

        let dnf = Dnf::from(&expr);
        let bdd2 = Bdd::from(dnf);

        assert_eq!(expr, bdd2);
    }

    #[test]
    fn test_empty_dnf() {
        let f = BoolExpr::constant(false);
        let dnf = Dnf::from(&f);

        assert!(dnf.is_empty());
        assert_eq!(dnf.len(), 0);

        let expr = BoolExpr::from(dnf);
        assert!(f.equivalent_to(&expr));
    }

    #[test]
    fn test_tautology_dnf() {
        let t = BoolExpr::constant(true);
        let dnf = Dnf::from(&t);

        assert!(!dnf.is_empty());
        // Tautology is one empty cube
        assert_eq!(dnf.len(), 1);
        assert!(dnf.cubes()[0].is_empty());
    }
}
