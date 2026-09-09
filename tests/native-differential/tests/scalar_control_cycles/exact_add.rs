//! A nonzero addition remains observable through the selected scalar call.

use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::OperationKind;

const SOURCE: &str = include_str!("count_up.omg");
const RANGE_ONLY_SOURCE: &str = include_str!("selected_call_exact_add.omg");
const DRIVER: &str = include_str!("count_up.c");
const RANGE_ONLY_DRIVER: &str = include_str!("selected_call_exact_add.c");

#[test]
fn range_only_loop_bound_publishes_with_checked_invariant_evidence() {
    let artifact = super::produce_for_entry(RANGE_ONLY_SOURCE, "countdown", true);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert_eq!(module.scalar_range_invariants.len(), 1);
    assert_eq!(module.scalar_range_invariants[0].arrivals.len(), 2);
    super::publication::assert_four_targets(&artifact, 1);
}

#[test]
fn range_only_loop_bound_executes_zero_and_several_iterations() {
    let artifact = super::produce_for_entry(RANGE_ONLY_SOURCE, "countdown", true);
    super::publication::assert_host_execution(&artifact, 1, RANGE_ONLY_DRIVER);
}

#[test]
fn unranked_range_invariant_supports_the_same_arithmetic_without_progress_evidence() {
    let source = RANGE_ONLY_SOURCE.replace(super::NATURAL_RANK, "");
    let artifact = super::produce_for_entry(&source, "countdown", false);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert_eq!(module.scalar_range_invariants.len(), 1);
    super::publication::assert_host_execution(&artifact, 1, RANGE_ONLY_DRIVER);
}

#[test]
fn native_entrance_rejects_missing_or_substituted_scalar_invariant_certificates() {
    let artifact = super::produce_for_entry(RANGE_ONLY_SOURCE, "countdown", true);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let original = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    for arrival in &module.scalar_range_invariants[0].arrivals {
        for substitute in [false, true] {
            let mut proof = original.clone();
            if substitute {
                proof
                    .evidence
                    .iter_mut()
                    .find(|entry| entry.obligation == arrival.obligation)
                    .unwrap()
                    .route = proof_admission::EvidenceRoute::KernelDerived(
                    proof_admission::PrimitiveJudgment::Truth,
                );
            } else {
                proof
                    .evidence
                    .retain(|entry| entry.obligation != arrival.obligation);
            }
            let bytes = terminal_codec::encode_proof_bundle(&proof).unwrap();
            assert!(matches!(
                super::lower_artifact_sections_for_native_realization(
                    artifact.semantic_bytes(),
                    &bytes,
                    &super::AdmissionProfile::default(),
                ),
                Err(super::ArtifactLoweringError::Verification(_))
            ));
        }
    }
}

fn artifact() -> CanonicalTerminalArtifact {
    let artifact = super::produce_for_entry(SOURCE, "count_up", false);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_eq!(
        entry
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, OperationKind::ExactIntegerAdd { .. }))
            .count(),
        1,
        "the selected call retains its authored exact addition"
    );
    artifact
}

#[test]
fn unranked_count_up_selected_exact_add_publishes_and_replays_on_four_targets() {
    super::publication::assert_four_targets(&artifact(), 1);
}

#[test]
fn unranked_count_up_selected_exact_add_executes_zero_and_several_iterations() {
    super::publication::assert_host_execution(&artifact(), 1, DRIVER);
}
