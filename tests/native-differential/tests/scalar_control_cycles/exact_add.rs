//! A nonzero addition remains observable through the selected scalar call.

use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::OperationKind;

const SOURCE: &str = include_str!("count_up.omg");
const RANGE_ONLY_SOURCE: &str = include_str!("selected_call_exact_add.omg");
const DRIVER: &str = include_str!("count_up.c");

#[test]
fn range_only_loop_bound_requires_checked_invariant_evidence() {
    // Keep the original customer visible until establishment/preservation is
    // checked. Invocation requirements alone cannot constrain later iterations.
    assert!(matches!(
        super::produce_candidate(RANGE_ONLY_SOURCE, "countdown"),
        Err(
            terminal_production::TerminalArtifactProductionError::Lowering(
                checked_trees_to_lowered_psi::LoweringError::OperationProofUnavailable(_)
            )
        )
    ));
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
