//! Stated `requires` ordering premises certify relational conclusions the
//! structural selector judgment cannot reach.
//!
//! A premise is exact relational evidence only: it retains the
//! `ContractProofFact` row it was decomposed from and its normalized
//! immutable-bound operands, replays positionally against the formation
//! scope's re-derived contracts, and never mints, extends, or transfers loan
//! authority.

use super::{checked_source, sole_certificate};
use crate::tests::{
    Lexer, ResolutionRequest, SymbolHandle, lower_symbol_resolved_trees, parse_syntax_trees,
    resolve,
};

/// `cut <= last` makes `[0, cut)` and `[last, 4)` provably disjoint for two
/// distinct symbolic parameters; the structural judgment alone cannot order
/// them.
const DISJOINT_WINDOWS: &str = r#"
    data Main { items: [i32; 4]; }

    machine Main::split(&mut self, cut: u64, last: u64) -> u64
        requires cut <= 4 && cut <= last && last <= 4;
    {
        let left: &mut [i32] = self.items[0..cut];
        let right: &mut [i32] = self.items[last..4];
        left.len + right.len
    }
"#;

/// `cut == last` proves the two shared windows denote the same extent.
const EQUAL_WINDOWS: &str = r#"
    data Main { items: [i32; 4]; }

    machine Main::split(&mut self, cut: u64, last: u64) -> u64
        requires cut <= 4 && cut == last && last <= 4;
    {
        let left: &[i32] = self.items[0..cut];
        let right: &[i32] = self.items[0..last];
        left.len + right.len
    }
"#;

/// Three stated relations each prove one containment bound: `a < b` makes the
/// inner window non-empty, `0 <= a` lowers its start, `b <= outer` caps its
/// end.
const CONTAINED_WINDOWS: &str = r#"
    data Main { items: [i32; 4]; }

    machine Main::split(&mut self, outer: u64, a: u64, b: u64) -> u64
        requires 0 <= a && a < b && b <= outer && b <= 4 && outer <= 4;
    {
        let whole: &[i32] = self.items[0..outer];
        let inner: &[i32] = self.items[a..b];
        whole.len + inner.len
    }
"#;

fn parameter_symbol(checked: &checked_trees::CheckedTrees, name: &str) -> SymbolHandle {
    checked
        .typed
        .machines()
        .iter()
        .flat_map(|machine| checked.typed.machine_states(machine))
        .flat_map(|state| checked.typed.state_parameters(state))
        .find(|parameter| parameter.name.as_str() == name)
        .map(|parameter| parameter.symbol)
        .expect("fixture parameter")
}

fn requires_fact(
    checked: &checked_trees::CheckedTrees,
) -> arena::Handle<checked_trees::ContractProofFact> {
    checked
        .facts
        .proof
        .contract_facts
        .iter()
        .find_map(|(handle, row)| {
            (row.kind == checked_trees::ContractProofFactKind::Requires).then_some(handle)
        })
        .expect("fixture requires contract fact")
}

fn symbol_value(
    checked: &checked_trees::CheckedTrees,
    name: &str,
) -> checked_trees::BorrowCompatibilitySelectorValue {
    checked_trees::BorrowCompatibilitySelectorValue::Symbol(parameter_symbol(checked, name))
}

fn integer_value(value: i64) -> checked_trees::BorrowCompatibilitySelectorValue {
    checked_trees::BorrowCompatibilitySelectorValue::Integer(value)
}

fn premise_tokens(
    certificate: &checked_trees::CheckedBorrowCompatibilityCertificate,
) -> Vec<(
    checked_trees::BorrowCompatibilityPremiseRelation,
    checked_trees::BorrowCompatibilitySelectorValue,
    checked_trees::BorrowCompatibilitySelectorValue,
)> {
    certificate
        .premises
        .iter()
        .map(|premise| (premise.relation, premise.left, premise.right))
        .collect()
}

fn assert_recording_rejects(
    checked: &mut checked_trees::CheckedTrees,
    message: &str,
) -> Vec<diagnostics::Diagnostic> {
    let before = checked.facts.borrow.compatibility_certificates.clone();
    let before_mutations = checked.facts.borrow.mutation_certificates.clone();
    let diagnostics =
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect_err("tampered premised evidence must reject replay");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(message)),
        "expected `{message}`: {diagnostics:#?}"
    );
    assert_eq!(
        checked.facts.borrow.compatibility_certificates, before,
        "failed replay must preserve the retained proof ledger",
    );
    assert_eq!(
        checked.facts.borrow.mutation_certificates, before_mutations,
        "failed replay must preserve the retained mutation ledger",
    );
    diagnostics
}

fn typed_source(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize premise fixture");
    let syntax = parse_syntax_trees(&tokens).expect("parse premise fixture");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve premise fixture");
    lower_symbol_resolved_trees(&resolved).expect("type premise fixture")
}

fn assert_borrow_conflict(source: &str) {
    let Err(diagnostics) = crate::lower_typed_trees(typed_source(source)) else {
        panic!("premise-insufficient windows must reject: {source}");
    };
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("creates local borrow")
                && diagnostic.message.contains("is still active")
        }),
        "expected a borrow conflict, not an earlier failure: {diagnostics:#?}"
    );
}

#[test]
fn stated_ordering_premise_certifies_disjoint_symbolic_windows() {
    let checked = checked_source(DISJOINT_WINDOWS);
    let certificate = sole_certificate(&checked);

    assert_eq!(
        certificate.derivation,
        checked_trees::BorrowCompatibilityDerivation::Premised
    );
    assert_eq!(
        premise_tokens(&certificate),
        vec![(
            checked_trees::BorrowCompatibilityPremiseRelation::LessOrEqual,
            symbol_value(&checked, "cut"),
            symbol_value(&checked, "last"),
        )],
        "only the `cut <= last` conjunct is consumed; unrelated conjuncts stay unrecorded",
    );
    assert_eq!(certificate.premises[0].fact, requires_fact(&checked));
    assert!(certificate.conclusion.disjoint);
    assert!(certificate.conclusion.non_interfering);
    assert_eq!(
        certificate.conclusion.containment,
        checked_trees::CapturedPlaceContainment::None
    );
    assert!(
        checked
            .facts
            .borrow
            .compatibility_certificate_matches_resources(&certificate)
    );
}

#[test]
fn stated_equality_premise_certifies_same_extent() {
    let checked = checked_source(EQUAL_WINDOWS);
    let certificate = sole_certificate(&checked);

    assert_eq!(
        certificate.derivation,
        checked_trees::BorrowCompatibilityDerivation::Premised
    );
    assert_eq!(
        premise_tokens(&certificate),
        vec![(
            checked_trees::BorrowCompatibilityPremiseRelation::Equal,
            symbol_value(&checked, "cut"),
            symbol_value(&checked, "last"),
        )]
    );
    assert!(!certificate.conclusion.disjoint);
    assert_eq!(
        certificate.conclusion.containment,
        checked_trees::CapturedPlaceContainment::Same
    );
    assert!(certificate.conclusion.non_interfering);
}

#[test]
fn stated_premises_certify_multi_token_containment() {
    let checked = checked_source(CONTAINED_WINDOWS);
    let certificate = sole_certificate(&checked);

    assert_eq!(
        certificate.derivation,
        checked_trees::BorrowCompatibilityDerivation::Premised
    );
    assert_eq!(
        premise_tokens(&certificate),
        vec![
            (
                checked_trees::BorrowCompatibilityPremiseRelation::StrictlyBefore,
                symbol_value(&checked, "a"),
                symbol_value(&checked, "b"),
            ),
            (
                checked_trees::BorrowCompatibilityPremiseRelation::LessOrEqual,
                integer_value(0),
                symbol_value(&checked, "a"),
            ),
            (
                checked_trees::BorrowCompatibilityPremiseRelation::LessOrEqual,
                symbol_value(&checked, "b"),
                symbol_value(&checked, "outer"),
            ),
        ],
        "each containment bound consumes its exact stated relation in consult order",
    );
    assert!(!certificate.conclusion.disjoint);
    assert_eq!(
        certificate.conclusion.containment,
        // The forming `[a, b)` window sits inside the already-active
        // `[0, outer)` window.
        checked_trees::CapturedPlaceContainment::RightContainsLeft
    );
    assert!(certificate.conclusion.non_interfering);
}

#[test]
fn premised_certificate_replays_through_checked_recording() {
    let mut checked = checked_source(DISJOINT_WINDOWS);
    let before = checked.facts.borrow.compatibility_certificates.clone();

    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect("retained premised certificate must replay its exact requires tokens");
    assert_eq!(
        checked.facts.borrow.compatibility_certificates, before,
        "idempotent replay republishes the identical premised certificate",
    );
}

#[test]
fn unconsulted_requires_leaves_a_structural_certificate() {
    let checked = checked_source(
        r#"
        data Main { items: [i32; 4]; }

        machine Main::split(&mut self, cut: u64) -> u64 requires cut <= 4;
        {
            let left: &mut [i32] = self.items[0..1];
            let right: &mut [i32] = self.items[2..4];
            left.len + right.len
        }
    "#,
    );
    let certificate = sole_certificate(&checked);

    assert_eq!(
        certificate.derivation,
        checked_trees::BorrowCompatibilityDerivation::Structural
    );
    assert!(
        certificate.premises.is_empty(),
        "a stated premise the structural judgment never needed is not evidence"
    );
}

#[test]
fn reversed_ordering_premise_does_not_prove_disjointness() {
    assert_borrow_conflict(
        r#"
        data Main { items: [i32; 4]; }

        machine Main::split(&mut self, cut: u64, last: u64) -> u64
            requires cut <= 4 && last <= cut && last <= 4;
        {
            let left: &mut [i32] = self.items[0..cut];
            let right: &mut [i32] = self.items[last..4];
            left.len + right.len
        }
    "#,
    );
}

#[test]
fn equality_premise_does_not_license_a_second_exclusive_loan() {
    // `cut == last` makes the windows the same extent; the same extent is
    // exactly what two exclusive loans cannot share.
    assert_borrow_conflict(
        r#"
        data Main { items: [i32; 4]; }

        machine Main::split(&mut self, cut: u64, last: u64) -> u64
            requires cut <= 4 && cut == last && last <= 4;
        {
            let left: &mut [i32] = self.items[0..cut];
            let right: &mut [i32] = self.items[0..last];
            left.len + right.len
        }
    "#,
    );
}

#[test]
fn ordering_premise_does_not_discharge_overlapping_third_loan() {
    // The certified `cut <= last` pair stays disjoint, but a third exclusive
    // window overlapping `left` still conflicts: premise evidence cannot
    // mint new disjointness or replace per-loan resource replay.
    assert_borrow_conflict(
        r#"
        data Main { items: [i32; 4]; }

        machine Main::split(&mut self, cut: u64, last: u64) -> u64
            requires cut <= 4 && cut <= last && last <= 4;
        {
            let left: &mut [i32] = self.items[0..cut];
            let right: &mut [i32] = self.items[last..4];
            let again: &mut [i32] = self.items[0..last];
            left.len + right.len + again.len
        }
    "#,
    );
}

#[test]
fn foreign_machine_requires_offers_no_premise() {
    // `Helper`'s `x <= y` belongs to `Helper`'s entry scope; it cannot order
    // `Main::split`'s parameters.
    assert_borrow_conflict(
        r#"
        data Helper { }

        machine Helper::bounded(&mut self, x: u64, y: u64) -> u64 requires x <= y;
        {
            x
        }

        data Main { items: [i32; 4]; }

        machine Main::split(&mut self, cut: u64, last: u64) -> u64
            requires cut <= 4 && last <= 4;
        {
            let left: &mut [i32] = self.items[0..cut];
            let right: &mut [i32] = self.items[last..4];
            left.len + right.len
        }
    "#,
    );
}

#[test]
fn mutable_premise_subject_offers_no_premise() {
    // `last` is mutable, so `cut <= last` decomposes to no normalized bound
    // and the windows stay unordered.
    let Err(_) = crate::lower_typed_trees(typed_source(
        r#"
        data Main { items: [i32; 4]; }

        machine Main::split(&mut self, cut: u64, mut last: u64) -> u64
            requires cut <= 4 && cut <= last && last <= 4;
        {
            let left: &mut [i32] = self.items[0..cut];
            let right: &mut [i32] = self.items[last..4];
            left.len + right.len
        }
    "#,
    )) else {
        panic!("a premise over a mutable subject cannot certify disjointness");
    };
}

#[test]
fn rejects_retained_premise_relation_tamper() {
    let mut checked = checked_source(DISJOINT_WINDOWS);
    let row = checked
        .facts
        .borrow
        .compatibility_certificates
        .iter()
        .next()
        .expect("certificate")
        .0;
    checked
        .facts
        .borrow
        .compatibility_certificates
        .get_mut(row)
        .premises[0]
        .relation = checked_trees::BorrowCompatibilityPremiseRelation::Equal;

    assert_recording_rejects(&mut checked, "premise tokens drifted");
}

#[test]
fn rejects_retained_premise_operand_retarget() {
    let mut checked = checked_source(DISJOINT_WINDOWS);
    let row = checked
        .facts
        .borrow
        .compatibility_certificates
        .iter()
        .next()
        .expect("certificate")
        .0;
    checked
        .facts
        .borrow
        .compatibility_certificates
        .get_mut(row)
        .premises[0]
        .right = integer_value(4);

    assert_recording_rejects(&mut checked, "premise tokens drifted");
}

#[test]
fn rejects_retained_premise_fact_retarget() {
    let mut checked = checked_source(DISJOINT_WINDOWS);
    let row = checked
        .facts
        .borrow
        .compatibility_certificates
        .iter()
        .next()
        .expect("certificate")
        .0;
    checked
        .facts
        .borrow
        .compatibility_certificates
        .get_mut(row)
        .premises[0]
        .fact = arena::Handle::invalid();

    assert_recording_rejects(&mut checked, "premise tokens drifted");
}

#[test]
fn rejects_transposed_retained_premise_tokens() {
    let mut checked = checked_source(CONTAINED_WINDOWS);
    let row = checked
        .facts
        .borrow
        .compatibility_certificates
        .iter()
        .next()
        .expect("certificate")
        .0;
    checked
        .facts
        .borrow
        .compatibility_certificates
        .get_mut(row)
        .premises
        .swap(0, 2);

    assert_recording_rejects(&mut checked, "premise tokens drifted");
}

#[test]
fn rejects_missing_retained_premise_tokens() {
    let mut checked = checked_source(CONTAINED_WINDOWS);
    let row = checked
        .facts
        .borrow
        .compatibility_certificates
        .iter()
        .next()
        .expect("certificate")
        .0;
    checked
        .facts
        .borrow
        .compatibility_certificates
        .get_mut(row)
        .premises
        .pop();

    assert_recording_rejects(&mut checked, "premise tokens drifted");
}

#[test]
fn rejects_extra_retained_premise_tokens() {
    let mut checked = checked_source(DISJOINT_WINDOWS);
    let row = checked
        .facts
        .borrow
        .compatibility_certificates
        .iter()
        .next()
        .expect("certificate")
        .0;
    let certificate = checked.facts.borrow.compatibility_certificates.get_mut(row);
    let extra = certificate.premises[0];
    certificate.premises.push(extra);

    assert_recording_rejects(&mut checked, "premise tokens drifted");
}

#[test]
fn rejects_premised_derivation_with_empty_ledger() {
    let mut checked = checked_source(DISJOINT_WINDOWS);
    let row = checked
        .facts
        .borrow
        .compatibility_certificates
        .iter()
        .next()
        .expect("certificate")
        .0;
    let certificate = checked.facts.borrow.compatibility_certificates.get_mut(row);
    certificate.premises.clear();

    assert_recording_rejects(&mut checked, "derivation drifted");
}

#[test]
fn rejects_structural_derivation_with_retained_ledger() {
    let mut checked = checked_source(DISJOINT_WINDOWS);
    let row = checked
        .facts
        .borrow
        .compatibility_certificates
        .iter()
        .next()
        .expect("certificate")
        .0;
    let certificate = checked.facts.borrow.compatibility_certificates.get_mut(row);
    certificate.derivation = checked_trees::BorrowCompatibilityDerivation::Structural;

    assert_recording_rejects(&mut checked, "derivation drifted");
}

#[test]
fn rejects_structural_derivation_with_stripped_ledger() {
    // With the ledger erased and the class downgraded, replay re-derives the
    // available premise but the recorded consult position is missing.
    let mut checked = checked_source(DISJOINT_WINDOWS);
    let row = checked
        .facts
        .borrow
        .compatibility_certificates
        .iter()
        .next()
        .expect("certificate")
        .0;
    let certificate = checked.facts.borrow.compatibility_certificates.get_mut(row);
    certificate.premises.clear();
    certificate.derivation = checked_trees::BorrowCompatibilityDerivation::Structural;

    assert_recording_rejects(&mut checked, "premise tokens drifted");
}

#[test]
fn rejects_stale_requires_that_no_longer_states_the_relation() {
    let mut checked = checked_source(DISJOINT_WINDOWS);
    // Flip the recorded `cut <= last` conjunct to `cut >= last` in the typed
    // requires expression: the re-derived premise set still offers an
    // ordering fact, but not the one the certificate consumed.
    let mut flipped = false;
    let fact_handles = checked
        .typed
        .proof_facts
        .iter()
        .filter_map(|(_, fact)| match fact {
            typed_trees::domain::ProofFact::Expression(expression) => Some(*expression),
            _ => None,
        })
        .collect::<Vec<_>>();
    for fact_expression in fact_handles {
        let mut stack = vec![fact_expression];
        while let Some(handle) = stack.pop() {
            let checked_trees::expression::ExpressionNode::Binary(binary) =
                checked.typed.expression_table.expression(handle)
            else {
                continue;
            };
            let (left, operator, right) = (binary.left, binary.operator, binary.right);
            stack.push(left);
            stack.push(right);
            if operator != checked_trees::expression::BinaryOperator::LessOrEqual {
                continue;
            }
            let right_is_name = matches!(
                checked.typed.expression_table.expression(right),
                checked_trees::expression::ExpressionNode::Name(_)
            );
            if right_is_name {
                let checked_trees::expression::ExpressionNode::Binary(binary) =
                    checked.typed.expression_table.expression_mut(handle)
                else {
                    unreachable!("binary node remains binary");
                };
                binary.operator = checked_trees::expression::BinaryOperator::GreaterOrEqual;
                flipped = true;
            }
        }
    }
    assert!(flipped, "fixture must find the `cut <= last` conjunct");

    assert_recording_rejects(&mut checked, "premise tokens drifted");
}

/// `i + j < cut` states the ordering on a two-term bound: the summed index
/// normalizes to one canonical pair whose terms separate the write's point
/// index from the `[cut, 4)` window.
const SUMMED_INDEX: &str = r#"
    data Main { items: [i32; 4]; }

    machine Main::split(&mut self, i: u64 [0..2], j: u64 [0..2], cut: u64 [0..4]) -> u64
        requires i + j < cut;
    {
        let left: &mut [i32] = self.items[cut..4];
        self.items[i + j] = 7;
        left.len
    }
"#;

fn sum_value(
    checked: &checked_trees::CheckedTrees,
    first: &str,
    second: &str,
) -> checked_trees::BorrowCompatibilitySelectorValue {
    checked_trees::BorrowCompatibilitySelectorValue::SymbolSum {
        first: parameter_symbol(checked, first),
        second: parameter_symbol(checked, second),
        offset: 0,
    }
}

fn sole_mutation_certificate(
    checked: &checked_trees::CheckedTrees,
) -> checked_trees::CheckedBorrowMutationCertificate {
    let certificates = checked
        .facts
        .borrow
        .mutation_certificates
        .iter()
        .map(|(_, certificate)| certificate.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        certificates.len(),
        1,
        "only the write-against-active-window admission is certified"
    );
    certificates.into_iter().next().expect("sole certificate")
}

#[test]
fn stated_ordering_premise_certifies_summed_index_bounds() {
    let checked = checked_source(SUMMED_INDEX);
    let certificate = sole_mutation_certificate(&checked);

    assert_eq!(
        certificate.derivation,
        checked_trees::BorrowCompatibilityDerivation::Premised
    );
    assert_eq!(
        certificate
            .premises
            .iter()
            .map(|premise| (premise.relation, premise.left, premise.right))
            .collect::<Vec<_>>(),
        vec![(
            checked_trees::BorrowCompatibilityPremiseRelation::StrictlyBefore,
            sum_value(&checked, "i", "j"),
            symbol_value(&checked, "cut"),
        )],
        "the retained premise is the stated `i + j < cut` conjunct on the canonical pair",
    );
    assert_eq!(certificate.premises[0].fact, requires_fact(&checked));
    assert!(
        checked
            .facts
            .borrow
            .mutation_certificate_matches_resources(&certificate)
    );
}

#[test]
fn summed_premise_certificate_replays_through_checked_recording() {
    let mut checked = checked_source(SUMMED_INDEX);
    let before = checked.facts.borrow.mutation_certificates.clone();

    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect("retained summed-premise certificate must replay its exact tokens");
    assert_eq!(
        checked.facts.borrow.mutation_certificates, before,
        "idempotent replay republishes the identical summed-premise certificate",
    );
}

#[test]
fn rejects_retained_sum_operand_retarget() {
    // Reordering the recorded pair breaks its canonical member order, so the
    // re-derived premise tokens drift.
    let mut checked = checked_source(SUMMED_INDEX);
    let row = checked
        .facts
        .borrow
        .mutation_certificates
        .iter()
        .next()
        .expect("certificate")
        .0;
    let certificate = checked.facts.borrow.mutation_certificates.get_mut(row);
    let checked_trees::BorrowCompatibilitySelectorValue::SymbolSum { first, second, .. } =
        &mut certificate.premises[0].left
    else {
        panic!("the summed premise records a two-symbol bound");
    };
    std::mem::swap(first, second);

    assert_recording_rejects(&mut checked, "premise tokens drifted");
}
