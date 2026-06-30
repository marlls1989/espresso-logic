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

use super::rpn::Token;
use super::{BoolExpr, BoolExprAst};
use crate::Symbol;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// Represents a product term (cube) as a set of literals
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ProductTerm {
    /// Literals in this product term: variable name -> polarity (true = positive, false = negative)
    literals: BTreeMap<Symbol, bool>,
}

impl ProductTerm {
    /// Create a new product term from literals
    fn new(literals: BTreeMap<Symbol, bool>) -> Self {
        ProductTerm { literals }
    }

    /// Check if this term contains a given literal
    fn contains_literal(&self, var: &Symbol, polarity: bool) -> bool {
        self.literals.get(var) == Some(&polarity)
    }

    /// Remove a literal from this term
    fn remove_literal(&mut self, var: &Symbol) -> Option<bool> {
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
                let v = Arc::new(BoolExprAst::Variable(var.clone()));
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
fn find_common_divisors(terms: &[ProductTerm]) -> Vec<(Symbol, bool)> {
    if terms.len() < 2 {
        return Vec::new();
    }

    // Count occurrences of each literal
    let mut literal_counts: HashMap<(Symbol, bool), usize> = HashMap::new();
    for term in terms {
        for (var, &polarity) in &term.literals {
            *literal_counts.entry((var.clone(), polarity)).or_insert(0) += 1;
        }
    }

    // Find literals that appear in at least 2 terms
    let mut common: Vec<(Symbol, bool)> = literal_counts
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
    var: &Symbol,
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
///
/// Driven **iteratively** with an explicit work-stack rather than recursion, so a wide term set
/// (which factors into a deep chain of subproblems) can't overflow the call stack. Each `Solve`
/// either resolves a subproblem to a leaf AST directly or, when a profitable factor exists, schedules
/// its `with`/`without` halves and a `Combine` that reassembles them — exactly mirroring the former
/// recursive shape (`var * factor(with) [ + factor(without) ]`).
fn factorise_once(terms: Vec<ProductTerm>) -> Arc<BoolExprAst> {
    enum Work {
        /// Factorise this set of terms.
        Solve(Vec<ProductTerm>),
        /// Reassemble after the children of a factored node are on the result stack. `has_without`
        /// records whether a `without` branch was scheduled (and so sits under the `with` result).
        Combine {
            var_ast: Arc<BoolExprAst>,
            has_without: bool,
        },
    }

    let mut work = vec![Work::Solve(terms)];
    let mut results: Vec<Arc<BoolExprAst>> = Vec::new();

    while let Some(item) = work.pop() {
        match item {
            Work::Solve(terms) => {
                if terms.is_empty() {
                    results.push(Arc::new(BoolExprAst::Constant(false)));
                    continue;
                }
                if terms.len() == 1 {
                    results.push(terms[0].to_ast());
                    continue;
                }

                // Find the best literal to factor out (greedy choice)
                if let Some((var, polarity)) = find_best_factor(&terms) {
                    let (with_literal, without_literal) = factor_literal(terms, &var, polarity);

                    // The factored-out literal: var (or ~var).
                    let var_ast = Arc::new(BoolExprAst::Variable(var.clone()));
                    let var_ast = if polarity {
                        var_ast
                    } else {
                        Arc::new(BoolExprAst::Not(var_ast))
                    };

                    let has_without = !without_literal.is_empty();
                    // Schedule Combine first (pops last). Push `with` then `without` so `without`
                    // pops/solves first and lands *under* the `with` result on the stack — Combine
                    // reads `with` (top) then `without`.
                    work.push(Work::Combine {
                        var_ast,
                        has_without,
                    });
                    work.push(Work::Solve(with_literal));
                    if has_without {
                        work.push(Work::Solve(without_literal));
                    }
                } else {
                    // No common factors, just OR all terms together
                    results.push(SopForm::new(terms).to_ast());
                }
            }
            Work::Combine {
                var_ast,
                has_without,
            } => {
                let factored_terms = results.pop().expect("factor `with` branch result");
                let factored_part = Arc::new(BoolExprAst::And(var_ast, factored_terms));
                if has_without {
                    let remaining = results.pop().expect("factor `without` branch result");
                    // Put unfactored terms first for neater appearance: a*b + q*(a+b)
                    results.push(Arc::new(BoolExprAst::Or(remaining, factored_part)));
                } else {
                    results.push(factored_part);
                }
            }
        }
    }

    results.pop().expect("factorise_once produced a result")
}

/// Find the best literal to factor out (greedy heuristic)
///
/// Prefers literals that appear in the most terms, with tie-breaker
/// favouring lexicographically later variables (e.g., 'q' over 'a').
fn find_best_factor(terms: &[ProductTerm]) -> Option<(Symbol, bool)> {
    let common = find_common_divisors(terms);
    if common.is_empty() {
        return None;
    }

    // Score each potential factor
    let mut best_factor = None;
    let mut best_score = 0;
    let mut best_var_name: Symbol = Symbol::from("");

    for (var, polarity) in common {
        // Score: number of terms we can factor (higher is better). `find_common_divisors` only
        // returns literals shared by ≥2 terms, so this count is always ≥2.
        let score = terms
            .iter()
            .filter(|t| t.contains_literal(&var, polarity))
            .count();

        // Tie-breaker: prefer lexicographically later variables
        if score > best_score || (score == best_score && var.as_ref() > best_var_name.as_ref()) {
            best_score = score;
            best_factor = Some((var.clone(), polarity));
            best_var_name = var;
        }
    }

    best_factor
}

/// Count the number of operators in an expression (as a rough size metric)
#[cfg(test)]
fn count_operators(expr: &BoolExpr) -> usize {
    use super::ExprNode;

    expr.fold(|node: ExprNode<usize>| -> usize {
        match node {
            ExprNode::Constant(_) | ExprNode::Variable(_) => 0,
            ExprNode::Not(inner_count) => inner_count + 1,
            ExprNode::And(left_count, right_count)
            | ExprNode::Or(left_count, right_count)
            | ExprNode::Xor(left_count, right_count) => left_count + right_count + 1,
        }
    })
}

/// Convert cubes directly to factored AST
///
/// This is the core factorization function that returns just the AST.
/// Used by BDD-to-AST conversion to produce beautiful expressions.
pub(crate) fn factorise_cubes_to_ast(
    cubes: Vec<(BTreeMap<Symbol, bool>, bool)>,
) -> Arc<BoolExprAst> {
    // Convert to ProductTerm format
    let terms: Vec<ProductTerm> = cubes
        .into_iter()
        .filter(|(_, include)| *include)
        .map(|(literals, _)| ProductTerm::new(literals))
        .collect();

    // Factorise and return AST. `factorise_once` already maps an empty term set to the `false`
    // constant, so no separate empty-case guard is needed here.
    factorise_once(terms)
}

/// Convert cubes (from a Cover) directly to a factored [`BoolExpr`].
///
/// Takes a list of product terms and applies algebraic factorisation to produce a more compact
/// multi-level representation.
///
/// This is the main entry point for the factorisation module, designed to work with cubes extracted
/// from Espresso output. The factored algebra is built as a [`BoolExprAst`], then **lowered directly
/// to the owned reverse-Polish [`BoolExpr`]** — it never round-trips through a BDD, so Espresso's
/// minimised cube structure is preserved exactly (re-canonicalising would discard it).
pub(crate) fn factorise_cubes(cubes: Vec<(BTreeMap<Symbol, bool>, bool)>) -> BoolExpr {
    let factored_ast = factorise_cubes_to_ast(cubes);
    ast_to_expr(&factored_ast)
}

/// Number of nodes in an AST, which equals the number of reverse-Polish tokens it lowers to (one token
/// per node). Used to pre-size the lowering buffer. Iterative so a deep tree can't overflow the stack.
fn count_nodes(ast: &BoolExprAst) -> usize {
    let mut stack = vec![ast];
    let mut count = 0;
    while let Some(node) = stack.pop() {
        count += 1;
        match node {
            BoolExprAst::Not(inner) => stack.push(inner),
            BoolExprAst::And(left, right)
            | BoolExprAst::Or(left, right)
            | BoolExprAst::Xor(left, right) => {
                stack.push(left);
                stack.push(right);
            }
            BoolExprAst::Variable(_) | BoolExprAst::Constant(_) => {}
        }
    }
    count
}

/// Lower a factored [`BoolExprAst`] directly to an owned [`BoolExpr`] (a reverse-Polish token stream).
///
/// Emits the AST's postorder token serialisation in a single pass into one pre-sized buffer. The
/// earlier form folded bottom-up through the binary `and`/`or` composition helpers, each of which
/// concatenates its *whole* left operand; folding the left-deep chain of k product terms (and the
/// OR-of-products over k cubes) therefore copied an ever-larger accumulator at every combine, O(k²)
/// total. Appending each node's single token once is O(k). The postfix order is unchanged — `binary`
/// already produced `left ++ right ++ [op]`, exactly postorder — so the token stream, its rendering,
/// and the canonical result are byte-for-byte identical; only the cost changes.
///
/// The traversal is iterative (an explicit work-stack) so a deep factorised AST can't overflow the
/// call stack. No BDD is built at any point — this is the load-bearing "direct factorisation" path for
/// `Cover::to_expr`.
fn ast_to_expr(ast: &BoolExprAst) -> BoolExpr {
    enum Step<'a> {
        /// Walk this node, emitting its children before its own operator.
        Visit(&'a BoolExprAst),
        /// Emit an operator token once both operands have been emitted.
        Emit(Token),
    }

    let mut tokens: Vec<Token> = Vec::with_capacity(count_nodes(ast));
    let mut work = vec![Step::Visit(ast)];

    while let Some(step) = work.pop() {
        match step {
            // For an operator node, schedule its `Emit` first (popped last) and push operands in
            // reverse so the left operand is walked before the right — yielding `left right op`.
            Step::Visit(node) => match node {
                BoolExprAst::Variable(name) => tokens.push(Token::Var(name.clone())),
                BoolExprAst::Constant(value) => tokens.push(Token::Const(*value)),
                BoolExprAst::Not(inner) => {
                    work.push(Step::Emit(Token::Not));
                    work.push(Step::Visit(inner));
                }
                BoolExprAst::And(left, right) => {
                    work.push(Step::Emit(Token::And));
                    work.push(Step::Visit(right));
                    work.push(Step::Visit(left));
                }
                BoolExprAst::Or(left, right) => {
                    work.push(Step::Emit(Token::Or));
                    work.push(Step::Visit(right));
                    work.push(Step::Visit(left));
                }
                BoolExprAst::Xor(left, right) => {
                    work.push(Step::Emit(Token::Xor));
                    work.push(Step::Visit(right));
                    work.push(Step::Visit(left));
                }
            },
            Step::Emit(token) => tokens.push(token),
        }
    }

    BoolExpr::from_tokens(tokens.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether two expressions denote the same Boolean function. `BoolExpr` is syntactic, so
    /// equivalence is checked through the canonical BDD layer (`equivalent_to` is an O(1) canonical
    /// comparison once both are built into one builder).
    fn equiv(a: &BoolExpr, b: &BoolExpr) -> bool {
        let builder = crate::bdd_builder!();
        builder.build(a).equivalent_to(&builder.build(b))
    }

    #[test]
    fn test_common_divisor_extraction() {
        // Create terms: a*b, a*c
        let mut term1_lits = BTreeMap::new();
        term1_lits.insert(Symbol::from("a"), true);
        term1_lits.insert(Symbol::from("b"), true);
        let term1 = ProductTerm::new(term1_lits);

        let mut term2_lits = BTreeMap::new();
        term2_lits.insert(Symbol::from("a"), true);
        term2_lits.insert(Symbol::from("c"), true);
        let term2 = ProductTerm::new(term2_lits);

        let terms = vec![term1, term2];
        let common = find_common_divisors(&terms);

        assert!(common.contains(&(Symbol::from("a"), true)));
    }

    #[test]
    fn test_factor_literal() {
        // Create terms: a*b, a*c, d
        let mut term1_lits = BTreeMap::new();
        term1_lits.insert(Symbol::from("a"), true);
        term1_lits.insert(Symbol::from("b"), true);
        let term1 = ProductTerm::new(term1_lits);

        let mut term2_lits = BTreeMap::new();
        term2_lits.insert(Symbol::from("a"), true);
        term2_lits.insert(Symbol::from("c"), true);
        let term2 = ProductTerm::new(term2_lits);

        let mut term3_lits = BTreeMap::new();
        term3_lits.insert(Symbol::from("d"), true);
        let term3 = ProductTerm::new(term3_lits);

        let terms = vec![term1, term2, term3];
        let (with_a, without_a) = factor_literal(terms, &Symbol::from("a"), true);

        assert_eq!(with_a.len(), 2); // b and c
        assert_eq!(without_a.len(), 1); // d
    }

    #[test]
    fn test_simple_factorisation() {
        // Test: a*b + a*c should factorise to a*(b+c)
        // Build product terms directly
        let mut term1 = BTreeMap::new();
        term1.insert(Symbol::from("a"), true);
        term1.insert(Symbol::from("b"), true);

        let mut term2 = BTreeMap::new();
        term2.insert(Symbol::from("a"), true);
        term2.insert(Symbol::from("c"), true);

        let cubes = vec![(term1, true), (term2, true)];
        let factored = factorise_cubes(cubes.clone());

        // Build unfactored version for comparison
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let unfactored = a.and(&b).or(&a.and(&c));

        // Should be logically equivalent
        assert!(equiv(&unfactored, &factored));

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
        term1.insert(Symbol::from("a"), true);
        term1.insert(Symbol::from("b"), true);

        let mut term2 = BTreeMap::new();
        term2.insert(Symbol::from("a"), true);
        term2.insert(Symbol::from("c"), true);

        let cubes = vec![(term1, true), (term2, true)];
        let factored = factorise_cubes(cubes);

        // Build original SOP
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let original = a.and(&b).or(&a.and(&c));

        // Factorisation must preserve the Boolean function.
        assert!(equiv(&original, &factored));
    }

    #[test]
    fn test_no_factorisation_needed() {
        // Test cases where no factorisation is possible

        // Single term: just 'a'
        let mut term1 = BTreeMap::new();
        term1.insert(Symbol::from("a"), true);
        let cubes = vec![(term1, true)];
        let factored = factorise_cubes(cubes);

        let a = BoolExpr::variable("a");
        assert!(equiv(&a, &factored));

        // Empty cubes (constant false)
        let factored_false = factorise_cubes(vec![]);
        assert!(equiv(&factored_false, &BoolExpr::constant(false)));

        // Tautology (empty product term = constant true)
        let cubes_true = vec![(BTreeMap::new(), true)];
        let factored_true = factorise_cubes(cubes_true);
        assert!(equiv(&factored_true, &BoolExpr::constant(true)));
    }

    #[test]
    fn test_complex_factorisation() {
        // Test: a*b*c + a*b*d should factor to a*b*(c + d)
        let mut term1 = BTreeMap::new();
        term1.insert(Symbol::from("a"), true);
        term1.insert(Symbol::from("b"), true);
        term1.insert(Symbol::from("c"), true);

        let mut term2 = BTreeMap::new();
        term2.insert(Symbol::from("a"), true);
        term2.insert(Symbol::from("b"), true);
        term2.insert(Symbol::from("d"), true);

        let cubes = vec![(term1, true), (term2, true)];
        let factored = factorise_cubes(cubes);

        // Build original SOP
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");
        let d = BoolExpr::variable("d");
        let original = a.and(&b).and(&c).or(&a.and(&b).and(&d));

        // Should be logically equivalent
        assert!(equiv(&original, &factored));

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
