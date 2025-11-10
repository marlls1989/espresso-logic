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

use crate::expression::bdd::Bdd;
use crate::expression::BoolExpr;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Disjunctive Normal Form representation of a boolean function
///
/// A DNF is a sum (OR) of products (AND), where each product term is called a "cube".
/// Each cube is represented as a map from variable names to their polarity:
/// - `true` means the variable appears positively (e.g., `a`)
/// - `false` means the variable appears negatively (e.g., `~a`)
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
    /// Product terms (cubes), each mapping variables to their polarity
    cubes: Vec<BTreeMap<Arc<str>, bool>>,
    /// Cached list of all variables (sorted alphabetically)
    variables: Vec<Arc<str>>,
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
            cubes: Vec::new(),
            variables: Vec::new(),
        }
    }

    /// Create a DNF from a vector of cubes
    ///
    /// This is primarily for internal use. Users should typically convert
    /// from [`BoolExpr`] or [`Bdd`] instead.
    ///
    /// [`BoolExpr`]: crate::expression::BoolExpr
    /// [`Bdd`]: crate::expression::bdd::Bdd
    pub fn from_cubes(cubes: Vec<BTreeMap<Arc<str>, bool>>) -> Self {
        // Collect all variables from cubes using BTreeSet for sorting
        let mut var_set = std::collections::BTreeSet::new();
        for cube in &cubes {
            for var in cube.keys() {
                var_set.insert(Arc::clone(var));
            }
        }
        // Convert to Vec for efficient slicing
        let variables: Vec<_> = var_set.into_iter().collect();

        Dnf { cubes, variables }
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
        self.cubes.is_empty()
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
        self.cubes.len()
    }

    /// Get an iterator over the cubes
    ///
    /// Each cube is a map from variable names to their polarity.
    pub fn iter(&self) -> impl Iterator<Item = &BTreeMap<Arc<str>, bool>> {
        self.cubes.iter()
    }

    /// Get a reference to the cubes
    ///
    /// This provides direct access to the underlying cube representation.
    pub fn cubes(&self) -> &[BTreeMap<Arc<str>, bool>] {
        &self.cubes
    }

    /// Get the cached variables (sorted alphabetically)
    ///
    /// Returns a slice of all variables that appear in the DNF.
    pub fn variables(&self) -> &[Arc<str>] {
        &self.variables
    }
}

impl Default for Dnf {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Blanket Conversions TO Dnf
// ============================================================================

/// Convert BoolExpr to DNF (via BDD for efficiency)
///
/// Ensures conversions go through BDD for canonical form and optimizations.
impl From<BoolExpr> for Dnf {
    fn from(expr: BoolExpr) -> Self {
        let bdd: Bdd = expr.into();
        let cubes = bdd.to_cubes();
        Dnf::from_cubes(cubes)
    }
}

/// Convert &BoolExpr to DNF (via BDD for efficiency)
impl From<&BoolExpr> for Dnf {
    fn from(expr: &BoolExpr) -> Self {
        let bdd: Bdd = expr.into();
        let cubes = bdd.to_cubes();
        Dnf::from_cubes(cubes)
    }
}

/// Convert Bdd to DNF (direct cube extraction)
impl From<Bdd> for Dnf {
    fn from(bdd: Bdd) -> Self {
        let cubes = bdd.to_cubes();
        Dnf::from_cubes(cubes)
    }
}

/// Convert &Bdd to DNF (direct cube extraction)
impl From<&Bdd> for Dnf {
    fn from(bdd: &Bdd) -> Self {
        let cubes = bdd.to_cubes();
        Dnf::from_cubes(cubes)
    }
}

// ============================================================================
// Conversions FROM Dnf
// ============================================================================

/// Convert DNF to BoolExpr
///
/// Reconstructs a boolean expression in DNF form (OR of AND terms).
///
/// # Examples
///
/// ```
/// use espresso_logic::{BoolExpr, Dnf};
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let expr = a.and(&b);
///
/// let dnf = Dnf::from(&expr);
/// let expr2 = BoolExpr::from(dnf);
///
/// assert!(expr.equivalent_to(&expr2));
/// ```
impl From<Dnf> for BoolExpr {
    fn from(dnf: Dnf) -> Self {
        if dnf.is_empty() {
            return BoolExpr::constant(false);
        }

        // Convert each cube to a product term
        let mut terms = Vec::new();
        for cube in dnf.cubes {
            if cube.is_empty() {
                // Empty cube means tautology (all variables are don't-care)
                terms.push(BoolExpr::constant(true));
            } else {
                // Build AND of all literals in this cube
                let factors: Vec<BoolExpr> = cube
                    .iter()
                    .map(|(var, &polarity)| {
                        let v = BoolExpr::variable(var);
                        if polarity {
                            v
                        } else {
                            v.not()
                        }
                    })
                    .collect();

                let product = factors.into_iter().reduce(|acc, f| acc.and(&f)).unwrap();
                terms.push(product);
            }
        }

        // OR all terms together
        terms.into_iter().reduce(|acc, t| acc.or(&t)).unwrap()
    }
}

/// Convert DNF directly to BDD
///
/// This implementation builds the BDD directly from cubes without going through
/// BoolExpr, which is more efficient.
///
/// # Examples
///
/// ```
/// use espresso_logic::{BoolExpr, Bdd, Dnf};
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let expr = a.or(&b);
///
/// let dnf = Dnf::from(&expr);
/// let bdd = Bdd::from(dnf);
///
/// // Should produce equivalent BDD
/// assert_eq!(bdd, expr.to_bdd());
/// ```
impl From<Dnf> for Bdd {
    fn from(dnf: Dnf) -> Self {
        if dnf.is_empty() {
            return Bdd::constant(false);
        }

        // Start with FALSE and OR each cube
        let mut result = Bdd::constant(false);

        for cube in dnf.cubes {
            if cube.is_empty() {
                // Empty cube = tautology, entire function is TRUE
                return Bdd::constant(true);
            }

            // Build conjunction for this cube
            let mut cube_bdd = Bdd::constant(true);
            for (var, &polarity) in &cube {
                let var_bdd = Bdd::variable(var);
                let literal_bdd = if polarity { var_bdd } else { var_bdd.not() };
                cube_bdd = cube_bdd.and(&literal_bdd);
            }

            // OR this cube with the accumulator
            result = result.or(&cube_bdd);
        }

        result
    }
}

// ============================================================================
// Blanket Minimizable Implementation
// ============================================================================

/// Blanket implementation of Minimizable for any type convertible to/from Dnf
///
/// This automatically provides minimization for `BoolExpr`, `Bdd`, and any other
/// type that implements the necessary conversions.
///
/// # Workflow
///
/// 1. Convert expression to Dnf (via BDD for canonical form)
/// 2. Convert Dnf to Cover
/// 3. Minimize the Cover using Espresso
/// 4. Convert minimized Cover back to Dnf
/// 5. Convert Dnf back to original type
///
/// The DNF serves as the intermediary representation between boolean expressions
/// and covers, ensuring all conversions go through the efficient BDD path.
impl<T> crate::cover::Minimizable for T
where
    for<'a> &'a T: Into<Dnf>,
    T: From<Dnf>,
{
    fn minimize_with_config(
        &self,
        config: &crate::EspressoConfig,
    ) -> Result<Self, crate::error::MinimizationError> {
        // Convert to Dnf (goes through BDD for canonical representation)
        let dnf: Dnf = self.into();

        // Use cached variables (already sorted alphabetically)
        let var_list = dnf.variables();
        let var_refs: Vec<&str> = var_list.iter().map(|s| s.as_ref()).collect();

        // Create cover with proper dimensions and labels
        let mut cover = crate::Cover::with_labels(crate::CoverType::F, &var_refs, &["out"]);

        // Add cubes to cover
        for cube in dnf.cubes() {
            let mut inputs = vec![None; var_list.len()];
            for (i, var) in var_list.iter().enumerate() {
                if let Some(&polarity) = cube.get(var) {
                    inputs[i] = Some(polarity);
                }
            }
            cover.add_cube(&inputs, &[Some(true)]);
        }

        // Minimize the cover
        let minimized_cover = cover.minimize_with_config(config)?;

        // Convert back to Dnf then to T
        let minimized_dnf = cover_to_dnf(&minimized_cover);
        Ok(T::from(minimized_dnf))
    }

    fn minimize_exact_with_config(
        &self,
        config: &crate::EspressoConfig,
    ) -> Result<Self, crate::error::MinimizationError> {
        // Convert to Dnf (goes through BDD for canonical representation)
        let dnf: Dnf = self.into();

        // Use cached variables (already sorted alphabetically)
        let var_list = dnf.variables();
        let var_refs: Vec<&str> = var_list.iter().map(|s| s.as_ref()).collect();

        // Create cover with proper dimensions and labels
        let mut cover = crate::Cover::with_labels(crate::CoverType::F, &var_refs, &["out"]);

        // Add cubes to cover
        for cube in dnf.cubes() {
            let mut inputs = vec![None; var_list.len()];
            for (i, var) in var_list.iter().enumerate() {
                if let Some(&polarity) = cube.get(var) {
                    inputs[i] = Some(polarity);
                }
            }
            cover.add_cube(&inputs, &[Some(true)]);
        }

        // Minimize the cover using exact algorithm
        let minimized_cover = cover.minimize_exact_with_config(config)?;

        // Convert back to Dnf then to T
        let minimized_dnf = cover_to_dnf(&minimized_cover);
        Ok(T::from(minimized_dnf))
    }
}

/// Helper function to convert a Cover back to Dnf
fn cover_to_dnf(cover: &crate::Cover) -> Dnf {
    let mut cubes = Vec::new();

    for cube in cover.cubes() {
        let mut product = BTreeMap::new();

        // Get input labels
        let input_labels = cover.input_labels();

        for (i, &literal) in cube.inputs().iter().enumerate() {
            if let Some(polarity) = literal {
                let var_name = if i < input_labels.len() {
                    Arc::clone(&input_labels[i])
                } else {
                    Arc::from(format!("x{}", i).as_str())
                };
                product.insert(var_name, polarity);
            }
        }

        cubes.push(product);
    }

    Dnf::from_cubes(cubes)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let bdd = expr.to_bdd();

        let dnf = Dnf::from(&bdd);
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

        assert_eq!(bdd, expr.to_bdd());
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
        let bdd = expr.to_bdd();

        let dnf = Dnf::from(&bdd);
        let bdd2 = Bdd::from(dnf);

        assert_eq!(bdd, bdd2);
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
