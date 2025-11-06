//! ExprCover - A cover type for boolean expressions
//!
//! This module provides ExprCover, which wraps a boolean expression and implements
//! the Cover trait. It converts expressions to cubes for minimization and can
//! convert minimized cubes back to expressions.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::cover::{Cube, CubeType, Minimizable, PLAType};
use crate::pla::PLASerializable;

use super::{BoolExpr, BoolExprInner};

/// A cover representation of a boolean expression
///
/// ExprCover wraps a BoolExpr and implements the Cover trait, allowing
/// expressions to be minimized using Espresso. After minimization, the
/// cover can be converted back to a BoolExpr.
///
/// # Examples
///
/// ```
/// use espresso_logic::{BoolExpr, ExprCover, Cover, expr};
///
/// # fn main() -> std::io::Result<()> {
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let c = BoolExpr::variable("c");
/// let expr = expr!(a * b + a * b * c);
///
/// let mut cover = ExprCover::from_expr(expr);
/// cover.minimize()?;
/// let minimized_expr = cover.to_expr();
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct ExprCover {
    /// Variables in alphabetical order
    variables: Vec<Arc<str>>,
    /// Cubes representing the expression
    cubes: Vec<Cube>,
}

impl ExprCover {
    /// Create a cover from a boolean expression
    pub fn from_expr(expr: BoolExpr) -> Self {
        let variables: Vec<Arc<str>> = expr.collect_variables().into_iter().collect();
        let dnf = to_dnf(&expr);
        let cubes = dnf_to_cubes(dnf, &variables);

        ExprCover { variables, cubes }
    }

    /// Convert the cover back to a boolean expression
    pub fn to_expr(&self) -> BoolExpr {
        // All cubes in ExprCover should be F-type (filtered in set_cubes)
        let cube_refs: Vec<&Cube> = self.cubes.iter().collect();
        cubes_to_expr_refs(&cube_refs, &self.variables)
    }

    /// Get the variables in this cover
    pub fn variables(&self) -> &[Arc<str>] {
        &self.variables
    }
}

/// Convert a boolean expression to Disjunctive Normal Form (DNF)
/// Returns a vector of product terms, where each term is a map from variable to its literal value
/// (true for positive literal, false for negative literal)
fn to_dnf(expr: &BoolExpr) -> Vec<BTreeMap<Arc<str>, bool>> {
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
            to_dnf_not(inner)
        }
        BoolExprInner::And(left, right) => {
            // AND: cross product of terms from each side
            let left_dnf = to_dnf(left);
            let right_dnf = to_dnf(right);

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
            let mut left_dnf = to_dnf(left);
            let right_dnf = to_dnf(right);
            left_dnf.extend(right_dnf);
            left_dnf
        }
    }
}

/// Convert NOT expression to DNF using De Morgan's laws
fn to_dnf_not(expr: &BoolExpr) -> Vec<BTreeMap<Arc<str>, bool>> {
    match expr.inner() {
        BoolExprInner::Constant(val) => {
            // NOT of constant
            to_dnf(&BoolExpr::constant(!val))
        }
        BoolExprInner::Variable(name) => {
            // NOT of variable = one product with negative literal
            let mut term = BTreeMap::new();
            term.insert(Arc::clone(name), false);
            vec![term]
        }
        BoolExprInner::Not(inner) => {
            // Double negation
            to_dnf(inner)
        }
        BoolExprInner::And(left, right) => {
            // De Morgan: ~(A * B) = ~A + ~B
            let not_left = left.not();
            let not_right = right.not();
            to_dnf(&not_left.or(&not_right))
        }
        BoolExprInner::Or(left, right) => {
            // De Morgan: ~(A + B) = ~A * ~B
            let not_left = left.not();
            let not_right = right.not();
            to_dnf(&not_left.and(&not_right))
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

/// Convert DNF representation to cubes for Espresso
fn dnf_to_cubes(dnf: Vec<BTreeMap<Arc<str>, bool>>, variables: &[Arc<str>]) -> Vec<Cube> {
    let mut cubes = Vec::new();

    for product_term in dnf {
        // Build input vector based on variable order
        let inputs: Vec<Option<bool>> = variables
            .iter()
            .map(|var| product_term.get(var).copied())
            .collect();

        // Single output that's always true (ON-set)
        let outputs = vec![true];

        cubes.push(Cube::new(inputs, outputs, CubeType::F));
    }

    cubes
}

/// Convert cube references back to a boolean expression
fn cubes_to_expr_refs(cubes: &[&Cube], variables: &[Arc<str>]) -> BoolExpr {
    if cubes.is_empty() {
        return BoolExpr::constant(false);
    }

    let mut terms = Vec::new();

    for cube in cubes.iter() {
        // Check if output is ON (should always be true for F-type)
        if !cube.outputs.is_empty() && cube.outputs[0] {
            // Build product term for this cube
            let mut factors = Vec::new();

            for (i, var) in variables.iter().enumerate() {
                match cube.inputs.get(i) {
                    Some(Some(true)) => {
                        // Positive literal
                        factors.push(BoolExpr::variable(var));
                    }
                    Some(Some(false)) => {
                        // Negative literal
                        factors.push(BoolExpr::variable(var).not());
                    }
                    Some(None) | None => {
                        // Don't care - skip this variable
                    }
                }
            }

            // AND all factors together
            if factors.is_empty() {
                // No literals means tautology (true)
                terms.push(BoolExpr::constant(true));
            } else {
                let product = factors.into_iter().reduce(|acc, f| acc.and(&f)).unwrap();
                terms.push(product);
            }
        }
    }

    // OR all terms together
    if terms.is_empty() {
        BoolExpr::constant(false)
    } else {
        terms.into_iter().reduce(|acc, t| acc.or(&t)).unwrap()
    }
}

impl Minimizable for ExprCover {
    fn num_inputs(&self) -> usize {
        self.variables.len()
    }

    fn num_outputs(&self) -> usize {
        1
    }

    fn cover_type(&self) -> PLAType {
        PLAType::F
    }

    fn internal_cubes_iter<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cube> + 'a> {
        Box::new(self.cubes.iter())
    }

    fn set_cubes(&mut self, cubes: Vec<Cube>) {
        // For F-type covers, only store F-type cubes (ON-set)
        // Filter out any D or R cubes that Espresso might have generated
        self.cubes = cubes
            .into_iter()
            .filter(|cube| cube.cube_type == CubeType::F)
            .collect();
    }
}

impl PLASerializable for ExprCover {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_dnf_variable() {
        let a = BoolExpr::variable("a");
        let dnf = to_dnf(&a);

        assert_eq!(dnf.len(), 1);
        assert_eq!(dnf[0].len(), 1);
        assert_eq!(dnf[0].get(&Arc::from("a")), Some(&true));
    }

    #[test]
    fn test_to_dnf_not() {
        let a = BoolExpr::variable("a");
        let not_a = a.not();
        let dnf = to_dnf(&not_a);

        assert_eq!(dnf.len(), 1);
        assert_eq!(dnf[0].len(), 1);
        assert_eq!(dnf[0].get(&Arc::from("a")), Some(&false));
    }

    #[test]
    fn test_to_dnf_and() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let and_expr = a.and(&b);
        let dnf = to_dnf(&and_expr);

        assert_eq!(dnf.len(), 1);
        assert_eq!(dnf[0].len(), 2);
        assert_eq!(dnf[0].get(&Arc::from("a")), Some(&true));
        assert_eq!(dnf[0].get(&Arc::from("b")), Some(&true));
    }

    #[test]
    fn test_to_dnf_or() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let or_expr = a.or(&b);
        let dnf = to_dnf(&or_expr);

        assert_eq!(dnf.len(), 2);
    }

    #[test]
    fn test_to_dnf_complex() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        // (a * b) + (~a * ~b) - XNOR
        let expr = a.and(&b).or(&a.not().and(&b.not()));
        let dnf = to_dnf(&expr);

        // Should have 2 product terms
        assert_eq!(dnf.len(), 2);
    }

    #[test]
    fn test_dnf_to_cubes() {
        let a_arc: Arc<str> = Arc::from("a");
        let b_arc: Arc<str> = Arc::from("b");
        let variables = vec![a_arc.clone(), b_arc.clone()];

        // Create DNF: a*b
        let mut term = BTreeMap::new();
        term.insert(a_arc, true);
        term.insert(b_arc, true);
        let dnf = vec![term];

        let cubes = dnf_to_cubes(dnf, &variables);

        assert_eq!(cubes.len(), 1);
        assert_eq!(cubes[0].inputs[0], Some(true));
        assert_eq!(cubes[0].inputs[1], Some(true));
    }

    #[test]
    fn test_expr_cover_trait() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let expr = a.and(&b);

        let cover = ExprCover::from_expr(expr);
        assert_eq!(cover.num_inputs(), 2);
        assert_eq!(cover.num_outputs(), 1);
        assert_eq!(cover.cover_type(), PLAType::F);
    }
}
