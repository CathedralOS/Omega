use super::{QualificationEvidence, QualificationProvenance};
use symbols::SymbolHandle;

fn receipt() -> QualificationEvidence {
    QualificationEvidence::from_admitted_requirement(
        SymbolHandle::from_parts(10, 1),
        SymbolHandle::from_parts(11, 1),
        arena::Handle::from_parts(12, 1),
    )
}

fn input(generation: u32) -> QualificationEvidence {
    receipt().at_invocation_input(
        SymbolHandle::from_parts(20, generation),
        SymbolHandle::from_parts(21, 1),
    )
}

#[test]
fn an_arrival_preserves_authority_without_independent_receipt_credit() {
    let local = receipt();
    let input = input(1);
    assert!(local.same_authority(input));
    assert_eq!(local.issuer, input.issuer);
    assert_eq!(local.source_symbol, input.source_symbol);
    assert_eq!(local.requirement_symbol, input.requirement_symbol);
    assert_eq!(local.origin, input.origin);
    assert!(!input.is_independent_receipt());
    assert_eq!(local.common_consequence(input), Some(input));
    assert_eq!(input.common_consequence(local), Some(input));
}

#[test]
fn an_available_independent_proof_survives_input_proofs() {
    let local = receipt();
    let input = input(1);
    assert_eq!(local.available_alternative(input), Some(local));
    assert_eq!(input.available_alternative(local), Some(local));
    assert!(local.is_independent_receipt());
}

#[test]
fn distinct_input_generations_and_unknown_provenance_fail_closed() {
    let first = input(1);
    let second = input(2);
    assert_ne!(first, second);
    let joined = first.common_consequence(second).unwrap();
    assert_eq!(joined.provenance, QualificationProvenance::MixedInputs);
    assert!(!joined.is_independent_receipt());
    assert_eq!(first.common_consequence(first), Some(first));
    let mut unknown = receipt();
    unknown.provenance = QualificationProvenance::Unknown;
    assert_eq!(first.common_consequence(unknown), Some(unknown));
    assert!(!QualificationEvidence::default().is_independent_receipt());
    assert_eq!(
        receipt().at_invocation_input(SymbolHandle::invalid(), SymbolHandle::invalid()),
        unknown
    );
}

#[test]
fn branch_consequences_preserve_input_dependency_and_available_local_proof() {
    let local = receipt();
    let input = input(1);
    let branch = local.common_consequence(input).unwrap();
    assert!(!branch.is_independent_receipt());
    assert_eq!(branch.available_alternative(local), Some(local));
    let mut other_authority = local;
    other_authority.issuer = arena::Handle::from_parts(12, 2);
    assert!(!local.same_authority(other_authority));
    assert_eq!(local.common_consequence(other_authority), None);
    assert_eq!(local.available_alternative(other_authority), None);
}
