//! Algebraic factorisation for boolean expressions
//!
//! This module implements greedy algebraic factorisation algorithms to convert
//! Sum-of-Products (SOP) expressions into more compact multi-level logic forms.
//!
//! The factorisation process:
//! 1. Extracts common divisors (literals that appear in multiple terms)
//! 2. Applies kernel extraction to identify factorable subexpressions
//! 3. Performs algebraic division to factor out common subexpressions
//! 4. Iterates until no further improvement is possible
//!
//! Complexity: O(n² × m) where n = number of product terms, m = literals per term

use super::{Bdd, BoolExpr, BoolExprAst};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// Represents a product term (cube) as a set of literals
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ProductTerm {
    /// Literals in this product term: variable name -> polarity (true = positive, false = negative)
    literals: BTreeMap<Arc<str>, bool>,
}

impl ProductTerm {
    /// Create a new product term from literals
    fn new(literals: BTreeMap<Arc<str>, bool>) -> Self {
        ProductTerm { literals }
    }

    /// Check if this term contains a given literal
    fn contains_literal(&self, var: &Arc<str>, polarity: bool) -> bool {
        self.literals.get(var) == Some(&polarity)
    }

    /// Remove a literal from this term
    fn remove_literal(&mut self, var: &Arc<str>) -> Option<bool> {
        self.literals.remove(var)
    }

    /// Check if this term is empty (represents constant true)
    fn is_empty(&self) -> bool {
        self.literals.is_empty()
    }

    /// Convert to AST
    fn to_ast(&self) -> Arc<BoolExprAst> {
        if self.is_empty() {
            return Arc::new(BoolExprAst::Constant(true));
        }

        let factors: Vec<Arc<BoolExprAst>> = self
            .literals
            .iter()
            .map(|(var, &polarity)| {
                let v = Arc::new(BoolExprAst::Variable(Arc::clone(var)));
                if polarity {
                    v
                } else {
                    Arc::new(BoolExprAst::Not(v))
                }
            })
            .collect();

        factors
            .into_iter()
            .reduce(|acc, f| Arc::new(BoolExprAst::And(acc, f)))
            .unwrap()
    }
}

/// Represents a Sum-of-Products expression
#[derive(Debug, Clone)]
struct SopForm {
    terms: Vec<ProductTerm>,
}

impl SopForm {
    /// Create a new SOP form from product terms
    fn new(terms: Vec<ProductTerm>) -> Self {
        SopForm { terms }
    }

    /// Convert to AST
    fn to_ast(&self) -> Arc<BoolExprAst> {
        if self.terms.is_empty() {
            return Arc::new(BoolExprAst::Constant(false));
        }

        let asts: Vec<Arc<BoolExprAst>> = self.terms.iter().map(|t| t.to_ast()).collect();
        asts.into_iter()
            .reduce(|acc, e| Arc::new(BoolExprAst::Or(acc, e)))
            .unwrap()
    }
}

/// Extract common divisors from a set of product terms
///
/// Finds literals that appear in multiple terms and can be factored out.
fn find_common_divisors(terms: &[ProductTerm]) -> Vec<(Arc<str>, bool)> {
    if terms.len() < 2 {
        return Vec::new();
    }

    // Count occurrences of each literal
    let mut literal_counts: HashMap<(Arc<str>, bool), usize> = HashMap::new();
    for term in terms {
        for (var, &polarity) in &term.literals {
            *literal_counts
                .entry((Arc::clone(var), polarity))
                .or_insert(0) += 1;
        }
    }

    // Find literals that appear in at least 2 terms
    let mut common: Vec<(Arc<str>, bool)> = literal_counts
        .into_iter()
        .filter(|(_, count)| *count >= 2)
        .map(|((var, polarity), _)| (var, polarity))
        .collect();

    // Sort for deterministic results
    common.sort();
    common
}

/// Factor out a single literal from terms
///
/// Given terms and a literal, splits terms into:
/// - Terms containing the literal (with literal removed)
/// - Terms not containing the literal
fn factor_literal(
    terms: Vec<ProductTerm>,
    var: &Arc<str>,
    polarity: bool,
) -> (Vec<ProductTerm>, Vec<ProductTerm>) {
    let mut with_literal = Vec::new();
    let mut without_literal = Vec::new();

    for mut term in terms {
        if term.contains_literal(var, polarity) {
            term.remove_literal(var);
            with_literal.push(term);
        } else {
            without_literal.push(term);
        }
    }

    (with_literal, without_literal)
}

/// Apply single-level factorisation to a set of terms
///
/// Finds common literals and factors them out, producing expressions like:
/// `a*b + a*c → a*(b + c)`
///
/// Uses greedy heuristic: factors out the first profitable literal found.
fn factorise_once(terms: Vec<ProductTerm>) -> Arc<BoolExprAst> {
    if terms.is_empty() {
        return Arc::new(BoolExprAst::Constant(false));
    }

    if terms.len() == 1 {
        return terms[0].to_ast();
    }

    // Find the best literal to factor out (greedy choice)
    if let Some((var, polarity)) = find_best_factor(&terms) {
        let (with_literal, without_literal) = factor_literal(terms, &var, polarity);

        // Create the factored part: var * (factorised_with_literal)
        let var_ast = Arc::new(BoolExprAst::Variable(Arc::clone(&var)));
        let var_ast = if polarity {
            var_ast
        } else {
            Arc::new(BoolExprAst::Not(var_ast))
        };

        // Recursively factorise the terms with the literal removed
        let factored_terms = factorise_once(with_literal);
        let factored_part = Arc::new(BoolExprAst::And(var_ast, factored_terms));

        // Handle remaining terms
        if without_literal.is_empty() {
            factored_part
        } else {
            let remaining = factorise_once(without_literal);
            // Put unfactored terms first for neater appearance: a*b + q*(a+b)
            Arc::new(BoolExprAst::Or(remaining, factored_part))
        }
    } else {
        // No common factors, just OR all terms together
        SopForm::new(terms).to_ast()
    }
}

/// Find the best literal to factor out (greedy heuristic)
///
/// Prefers literals that appear in the most terms, with tie-breaker
/// favouring lexicographically later variables (e.g., 'q' over 'a').
fn find_best_factor(terms: &[ProductTerm]) -> Option<(Arc<str>, bool)> {
    let common = find_common_divisors(terms);
    if common.is_empty() {
        return None;
    }

    // Score each potential factor
    let mut best_factor = None;
    let mut best_score = 0;
    let mut best_var_name: Arc<str> = Arc::from("");

    for (var, polarity) in common {
        let terms_with_literal = terms
            .iter()
            .filter(|t| t.contains_literal(&var, polarity))
            .count();

        // Only factor if at least 2 terms have this literal
        if terms_with_literal < 2 {
            continue;
        }

        // Score: number of terms we can factor (higher is better)
        let score = terms_with_literal;

        // Tie-breaker: prefer lexicographically later variables
        if score > best_score || (score == best_score && var.as_ref() > best_var_name.as_ref()) {
            best_score = score;
            best_factor = Some((Arc::clone(&var), polarity));
            best_var_name = var;
        }
    }

    best_factor
}

/// Apply single-pass factorisation, returning AST directly
fn factorise_multipass(terms: Vec<ProductTerm>, _max_iterations: usize) -> Arc<BoolExprAst> {
    if terms.is_empty() {
        return Arc::new(BoolExprAst::Constant(false));
    }

    // Single pass only - working directly with AST preserves factored structure
    factorise_once(terms)
}

/// Count the number of operators in an expression (as a rough size metric)
#[cfg(test)]
fn count_operators(expr: &BoolExpr) -> usize {
    use super::ExprNode;

    expr.fold(|node: ExprNode<usize>| -> usize {
        match node {
            ExprNode::Constant(_) | ExprNode::Variable(_) => 0,
            ExprNode::Not(inner_count) => inner_count + 1,
            ExprNode::And(left_count, right_count) | ExprNode::Or(left_count, right_count) => {
                left_count + right_count + 1
            }
        }
    })
}

/// Convert cubes directly to factored AST
///
/// This is the core factorization function that returns just the AST.
/// Used by BDD-to-AST conversion to produce beautiful expressions.
pub(crate) fn factorise_cubes_to_ast(
    cubes: Vec<(BTreeMap<Arc<str>, bool>, bool)>,
) -> Arc<BoolExprAst> {
    // Convert to ProductTerm format
    let terms: Vec<ProductTerm> = cubes
        .into_iter()
        .filter(|(_, include)| *include)
        .map(|(literals, _)| ProductTerm::new(literals))
        .collect();

    // Factorise and return AST
    factorise_multipass(terms, 3)
}

/// Convert cubes (from Cover) directly to factored expression
///
/// Takes a list of product terms and applies algebraic factorisation to
/// produce a more compact multi-level representation.
///
/// This is the main entry point for the factorization module, designed to work
/// with cubes extracted from Espresso output.
///
/// Works entirely with AST to preserve the factored structure, then converts
/// to BoolExpr at the very end.
pub(crate) fn factorise_cubes(cubes: Vec<(BTreeMap<Arc<str>, bool>, bool)>) -> BoolExpr {
    // Get factored AST
    let factored_ast = factorise_cubes_to_ast(cubes);

    // Convert AST to BoolExpr by building BDD
    let bdd = ast_to_bdd(&factored_ast);

    // Create BoolExpr with both BDD and pre-computed AST
    let expr = bdd; // Bdd is now just a type alias for BoolExpr
                    // Cache the factored AST so it's used for display instead of regenerating
    let _ = expr.ast_cache.set(factored_ast);
    expr
}

/// Convert AST to BDD for semantic operations
fn ast_to_bdd(ast: &BoolExprAst) -> Bdd {
    match ast {
        BoolExprAst::Constant(val) => Bdd::constant(*val),
        BoolExprAst::Variable(name) => Bdd::variable(name),
        BoolExprAst::Not(inner) => ast_to_bdd(inner).not(),
        BoolExprAst::And(left, right) => ast_to_bdd(left).and(&ast_to_bdd(right)),
        BoolExprAst::Or(left, right) => ast_to_bdd(left).or(&ast_to_bdd(right)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_common_divisor_extraction() {
        // Create terms: a*b, a*c
        let mut term1_lits = BTreeMap::new();
        term1_lits.insert(Arc::from("a"), true);
        term1_lits.insert(Arc::from("b"), true);
        let term1 = ProductTerm::new(term1_lits);

        let mut term2_lits = BTreeMap::new();
        term2_lits.insert(Arc::from("a"), true);
        term2_lits.insert(Arc::from("c"), true);
        let term2 = ProductTerm::new(term2_lits);

        let terms = vec![term1, term2];
        let common = find_common_divisors(&terms);

        assert!(common.contains(&(Arc::from("a"), true)));
    }

    #[test]
    fn test_factor_literal() {
        // Create terms: a*b, a*c, d
        let mut term1_lits = BTreeMap::new();
        term1_lits.insert(Arc::from("a"), true);
        term1_lits.insert(Arc::from("b"), true);
        let term1 = ProductTerm::new(term1_lits);

        let mut term2_lits = BTreeMap::new();
        term2_lits.insert(Arc::from("a"), true);
        term2_lits.insert(Arc::from("c"), true);
        let term2 = ProductTerm::new(term2_lits);

        let mut term3_lits = BTreeMap::new();
        term3_lits.insert(Arc::from("d"), true);
        let term3 = ProductTerm::new(term3_lits);

        let terms = vec![term1, term2, term3];
        let (with_a, without_a) = factor_literal(terms, &Arc::from("a"), true);

        assert_eq!(with_a.len(), 2); // b and c
        assert_eq!(without_a.len(), 1); // d
    }

    #[test]
    fn test_simple_factorisation() {
        // Test: a*b + a*c should factorise to a*(b+c)
        // Build product terms directly
        let mut term1 = BTreeMap::new();
        term1.insert(Arc::from("a"), true);
        term1.insert(Arc::from("b"), true);

        let mut term2 = BTreeMap::new();
        term2.insert(Arc::from("a"), true);
        term2.insert(Arc::from("c"), true);

        let cubes = vec![(term1, true), (term2, true)];
        let factored = factorise_cubes(cubes.clone());

        // Build unfactored version for comparison
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let unfactored = a.and(&b).or(&a.and(&c));

        // Should be logically equivalent
        assert!(unfactored.equivalent_to(&factored));

        // Factored version should have fewer operators
        let unfactored_ops = count_operators(&unfactored);
        let factored_ops = count_operators(&factored);
        assert!(
            factored_ops <= unfactored_ops,
            "Factored has {} ops, unfactored has {} ops",
            factored_ops,
            unfactored_ops
        );
    }

    #[test]
    fn test_factorisation_equivalence() {
        // Test that factorisation preserves logical equivalence for a*b + a*c
        let mut term1 = BTreeMap::new();
        term1.insert(Arc::from("a"), true);
        term1.insert(Arc::from("b"), true);

        let mut term2 = BTreeMap::new();
        term2.insert(Arc::from("a"), true);
        term2.insert(Arc::from("c"), true);

        let cubes = vec![(term1, true), (term2, true)];
        let factored = factorise_cubes(cubes);

        // Build original SOP
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let original = a.and(&b).or(&a.and(&c));

        // Verify equivalence by testing all input combinations
        for a_val in [false, true] {
            for b_val in [false, true] {
                for c_val in [false, true] {
                    let mut assignment = HashMap::new();
                    assignment.insert(Arc::from("a"), a_val);
                    assignment.insert(Arc::from("b"), b_val);
                    assignment.insert(Arc::from("c"), c_val);

                    let original_result = original.evaluate(&assignment);
                    let factored_result = factored.evaluate(&assignment);

                    assert_eq!(
                        original_result, factored_result,
                        "Factorisation changed logic for a={}, b={}, c={}",
                        a_val, b_val, c_val
                    );
                }
            }
        }
    }

    #[test]
    fn test_no_factorisation_needed() {
        // Test cases where no factorisation is possible

        // Single term: just 'a'
        let mut term1 = BTreeMap::new();
        term1.insert(Arc::from("a"), true);
        let cubes = vec![(term1, true)];
        let factored = factorise_cubes(cubes);

        let a = BoolExpr::variable("a");
        assert!(a.equivalent_to(&factored));

        // Empty cubes (constant false)
        let factored_false = factorise_cubes(vec![]);
        assert!(factored_false.equivalent_to(&BoolExpr::constant(false)));

        // Tautology (empty product term = constant true)
        let cubes_true = vec![(BTreeMap::new(), true)];
        let factored_true = factorise_cubes(cubes_true);
        assert!(factored_true.equivalent_to(&BoolExpr::constant(true)));
    }

    #[test]
    fn test_complex_factorisation() {
        // Test: a*b*c + a*b*d should factor to a*b*(c + d)
        let mut term1 = BTreeMap::new();
        term1.insert(Arc::from("a"), true);
        term1.insert(Arc::from("b"), true);
        term1.insert(Arc::from("c"), true);

        let mut term2 = BTreeMap::new();
        term2.insert(Arc::from("a"), true);
        term2.insert(Arc::from("b"), true);
        term2.insert(Arc::from("d"), true);

        let cubes = vec![(term1, true), (term2, true)];
        let factored = factorise_cubes(cubes);

        // Build original SOP
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let d = BoolExpr::variable("d");
        let original = a.and(&b).and(&c).or(&a.and(&b).and(&d));

        // Should be logically equivalent
        assert!(original.equivalent_to(&factored));

        // Should have fewer operators
        let original_ops = count_operators(&original);
        let factored_ops = count_operators(&factored);
        assert!(
            factored_ops <= original_ops,
            "Factored has {} ops, original has {} ops",
            factored_ops,
            original_ops
        );
    }
}
