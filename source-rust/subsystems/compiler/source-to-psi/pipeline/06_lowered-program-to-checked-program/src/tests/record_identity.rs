//! Identity of two records that have storage, from the identities of their
//! fields.
//!
//! A record whose fields all have a kernel carrier is the product of those
//! carriers, and an immutable input of it is the tuple of what its fields
//! held at entry. `a = b` at `Point` is proved field by field from the entry
//! requirements `a.x = b.x` and `a.y = b.y`; the kernel converts each tuple
//! back into its record. When one field identity is missing, the refusal
//! names that field ([record identity](wiki/spec/proofs/contracts.md#record-identity-from-field-identities)).

use super::*;

const FROM_FIELDS: &str =
    include_str!("../../tests/fixtures/record_identity/pass/point_from_fields/main.omg");
const MISSING_Y: &str =
    include_str!("../../tests/fixtures/record_identity/fail/point_missing_y/main.omg");

fn judged(source: &str) -> RouteVerdict {
    let typed = super::build_purpose::frontier_with_core(
        source,
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        &[],
    );
    let prepared = crate::prepare_fixture(typed);
    let scope = source_provenance::DependencyScope::Product;
    let compilation = crate::lowering::prepare_compilation(
        &prepared,
        super::build_purpose::roster(&[], &[Some(super::build_purpose::package())], scope),
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
    let mut parts = crate::judge_compilation(compilation.program).into_iter();
    let verdict = parts.next().expect("the program's judgment").unwrap();
    assert!(parts.next().is_none(), "one judged part");
    verdict
}

#[test]
fn a_record_is_identical_when_every_field_is() {
    let verdict = judged(FROM_FIELDS);
    let RouteVerdict::Accepted(closure) = verdict else {
        panic!("the record identity must be proved: {verdict:?}")
    };
    let guarantees = verdicts(closure.elaboration(), closure.discharge(), 1, guarantee);
    let [(_, Ok(Discharged::Identity(judgment)))] = guarantees.as_slice() else {
        panic!("expected one judged Identity, got {guarantees:?}")
    };
    // Both entry requirements are cited, and nothing else.
    assert_eq!(judgment.citations.len(), 2);
    let MathematicalCoreDecision::Judged(receipt) = judgment.decision;
    // The two requirements are the proof's hypotheses; it assumes only the
    // `i32` carrier and the four values the fields held at entry.
    assert_eq!((receipt.context_depth, receipt.assumption_closure), (2, 5));
    let mut checked =
        terminal_codec::decode_mathematical_certificate(&judgment.certificate).unwrap();
    proof_admission::verify_mathematical_certificate(
        &mut checked.arena,
        &checked.certificate,
        &mut Default::default(),
    )
    .unwrap();
    // The statement is kernel Id at the record, and the record is declared
    // as the product of its two `i32` fields.
    use proof_admission::Term;
    let Term::Id { ty, .. } = checked.arena.get(checked.certificate.expected) else {
        panic!("the contract must denote kernel Id")
    };
    let Term::Constant { declaration, .. } = checked.arena.get(ty) else {
        panic!("the record is a declared constant")
    };
    let Some(Term::Sigma { domain, codomain }) = checked.certificate.signature
        [declaration as usize]
        .body
        .map(|body| checked.arena.get(body))
    else {
        panic!("the record is the product of its fields")
    };
    assert!(checked.arena.structurally_equal(domain, codomain));
}

#[test]
fn a_record_whose_second_field_is_unproved_is_refused_at_that_field() {
    assert_eq!(
        MISSING_Y,
        FROM_FIELDS.replace("requires\n    core\\Identity::<i32>(a.y, b.y)\n", ""),
        "the twins differ only in the `y` requirement"
    );
    let verdict = judged(MISSING_Y);
    let RouteVerdict::Rejected {
        unproved,
        unestablished,
        ..
    } = verdict
    else {
        panic!("the record identity must be refused: {verdict:?}")
    };
    assert!(unestablished.is_empty(), "{unestablished:?}");
    let [refused] = unproved.as_slice() else {
        panic!("one unproved obligation: {unproved:?}")
    };
    assert_eq!(refused.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        refused.statement,
        "cannot prove ensures contract for exit from same_point at statement 0: \
         core\\Identity::<Point>(a, b)"
    );
    assert_eq!(
        refused.detail,
        "no available equality proves the stated Identity: \
         nothing proves the identity of field `y`"
    );
}

/// The same pair with `x` a record of its own: a requirement over the whole
/// field supplies it, and without one the refusal names the scalar inside.
fn nested(source: &str) -> String {
    format!(
        "data Coordinate {{\n    value: i32;\n}}\n\n{}",
        source.replace("    x: i32;", "    x: Coordinate;").replace(
            "Identity::<i32>(a.x, b.x)",
            "Identity::<Coordinate>(a.x, b.x)"
        )
    )
}

#[test]
fn a_record_field_of_a_record_is_identical_by_its_own_fields() {
    let verdict = judged(&nested(FROM_FIELDS));
    let RouteVerdict::Accepted(closure) = verdict else {
        panic!("the nested record identity must be proved: {verdict:?}")
    };
    let guarantees = verdicts(closure.elaboration(), closure.discharge(), 1, guarantee);
    let [(_, Ok(Discharged::Identity(judgment)))] = guarantees.as_slice() else {
        panic!("expected one judged Identity, got {guarantees:?}")
    };
    assert_eq!(judgment.citations.len(), 2);

    let without_x =
        nested(FROM_FIELDS).replace("requires\n    core\\Identity::<Coordinate>(a.x, b.x)\n", "");
    let RouteVerdict::Rejected { unproved, .. } = judged(&without_x) else {
        panic!("the nested record identity must be refused without `x`")
    };
    let [refused] = unproved.as_slice() else {
        panic!("one unproved obligation: {unproved:?}")
    };
    assert_eq!(refused.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        refused.detail,
        "no available equality proves the stated Identity: \
         nothing proves the identity of field `x.value`"
    );
}
