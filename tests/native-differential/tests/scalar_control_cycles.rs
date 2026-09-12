//! Prerequisite companions for the owned scalar walk: ordinary scalar cycles.
//! Owned-record realization and primitive-local storage remain separate work.

use proof_admission::{AdmissionProfile, EvidenceRoute, PrimitiveJudgment};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{OperationKind, TerminalRankedScc};
use terminal_psi_to_abstract_operations::{
    ArtifactLoweringError, lower_artifact_sections_for_native_realization,
};

#[path = "scalar_control_cycles/publication.rs"]
mod publication;

#[path = "scalar_control_cycles/optimization.rs"]
mod optimization;

#[path = "scalar_control_cycles/exact_add.rs"]
mod exact_add;

#[path = "scalar_control_cycles/value_reuse.rs"]
mod value_reuse;

#[path = "scalar_control_cycles/bitwise.rs"]
mod bitwise;

#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(target_os = "macos", target_arch = "aarch64")
))]
#[path = "common/native_function.rs"]
#[allow(dead_code)]
mod native_function;

const SELECTED_CALL: &str = include_str!("scalar_control_cycles/selected_call.omg");
const SWAPPED_ARGUMENTS: &str = include_str!("scalar_control_cycles/swapped_arguments.omg");
const SELECTED_CALL_DRIVER: &str = include_str!("scalar_control_cycles/selected_call.c");
const SWAPPED_ARGUMENTS_DRIVER: &str = include_str!("scalar_control_cycles/swapped_arguments.c");
const NATURAL_RANK: &str = "terminates by remaining -> Nat::Descending in 0..6;\n";

fn produce_candidate(
    source: &str,
    entry: &str,
) -> Result<CanonicalTerminalArtifact, terminal_production::TerminalArtifactProductionError> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap_or_else(|error| panic!("tokenize scalar cycle: {error:?}\n{source}"));
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .unwrap_or_else(|error| panic!("parse scalar cycle: {error:?}\n{source}"));
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .unwrap_or_else(|error| panic!("resolve scalar cycle: {error:?}\n{source}"));
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .unwrap_or_else(|error| panic!("type scalar cycle: {error:?}\n{source}"));
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|error| panic!("check scalar cycle: {error:#?}\n{source}"));
    terminal_production::TerminalProductionRequest::new(&checked, entry).produce_artifact()
}

fn produce(source: &str, ranked: bool) -> CanonicalTerminalArtifact {
    produce_for_entry(source, "countdown", ranked)
}

fn produce_for_entry(source: &str, entry_name: &str, ranked: bool) -> CanonicalTerminalArtifact {
    let artifact = produce_candidate(source, entry_name)
        .unwrap_or_else(|error| panic!("publish source scalar cycle: {error:#?}\n{source}"));
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    assert_eq!(
        terminal_codec::encode_module(&module).unwrap(),
        artifact.semantic_bytes()
    );
    assert_eq!(
        terminal_codec::encode_proof_bundle(&proof).unwrap(),
        artifact.proof_bytes()
    );
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_eq!(
        terminal_verifier::control_cycle_members(entry)
            .unwrap()
            .len(),
        1
    );
    assert!(entry.structural_parameters.is_empty());
    if ranked {
        assert!(
            entry
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    matches!(operation.kind, OperationKind::ExactIntegerSubtract { .. })
                }),
            "the authored decrement remains executable"
        );
    }
    if ranked {
        assert!(
            matches!(&entry.ranked_scc, Some(TerminalRankedScc::Natural(components)) if components.len() == 1)
        );
    } else {
        assert!(
            module
                .machines
                .iter()
                .all(|machine| machine.ranked_scc.is_none())
        );
    }
    let verified = terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("independently verify canonical source-produced cycle");
    assert_eq!(proof.control_cycles.len(), usize::from(ranked));
    assert_eq!(
        verified.accepted_control_cycles().len(),
        usize::from(ranked)
    );
    let abstracted = lower_artifact_sections_for_native_realization(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
    )
    .expect("Natural and unranked cycles use the same verified native entrance");
    assert_eq!(
        abstracted.plan().functions.len(),
        module.machines.len(),
        "native lowering retains the verified function roster"
    );
    artifact
}

fn unranked_selected_call() -> String {
    assert_eq!(SELECTED_CALL.matches(NATURAL_RANK).count(), 1);
    SELECTED_CALL.replacen(NATURAL_RANK, "", 1)
}

#[test]
fn natural_countdown_selected_scalar_call_publishes_and_replays_on_four_targets() {
    publication::assert_four_targets(&produce(SELECTED_CALL, true), 1);
}

#[test]
fn natural_countdown_selected_scalar_call_executes_zero_and_several_iterations() {
    publication::assert_host_execution(&produce(SELECTED_CALL, true), 1, SELECTED_CALL_DRIVER);
}

#[test]
fn natural_countdown_simultaneous_backedge_arguments_publish_and_replay_on_four_targets() {
    publication::assert_four_targets(&produce(SWAPPED_ARGUMENTS, true), 1);
}

#[test]
fn natural_countdown_simultaneous_backedge_arguments_execute_exact_exchanges() {
    publication::assert_host_execution(
        &produce(SWAPPED_ARGUMENTS, true),
        1,
        SWAPPED_ARGUMENTS_DRIVER,
    );
}

#[test]
fn unranked_finite_countdown_selected_scalar_call_publishes_and_replays_on_four_targets() {
    publication::assert_four_targets(&produce(&unranked_selected_call(), false), 1);
}

#[test]
fn unranked_finite_countdown_selected_scalar_call_executes_zero_and_several_iterations() {
    publication::assert_host_execution(
        &produce(&unranked_selected_call(), false),
        1,
        SELECTED_CALL_DRIVER,
    );
}

#[test]
fn native_entrance_rejects_corrupted_natural_cycle_evidence() {
    let artifact = produce(SELECTED_CALL, true);
    let original = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    assert!(original.control_cycles[0].certificate.edges.len() > 1);
    for corruption in [
        "missing component",
        "missing edges",
        "substituted edge proof",
        "foreign relation",
    ] {
        let mut proof = original.clone();
        match corruption {
            "missing component" => proof.control_cycles.clear(),
            "missing edges" => proof.control_cycles[0].certificate.edges.clear(),
            "substituted edge proof" => {
                proof.control_cycles[0].certificate.edges[0].evidence =
                    EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth);
            }
            "foreign relation" => {
                let relation = &mut proof.control_cycles[0].certificate.ranking_relation;
                let replacement = if relation.get() == 1 { 2 } else { 1 };
                *relation = semantic_vocabulary::RankingRelationId::new(replacement).unwrap();
            }
            _ => unreachable!(),
        }
        let changed = terminal_codec::encode_proof_bundle(&proof).unwrap();
        assert_ne!(changed, artifact.proof_bytes(), "{corruption}");
        assert!(
            matches!(
                lower_artifact_sections_for_native_realization(
                    artifact.semantic_bytes(),
                    &changed,
                    &AdmissionProfile::default(),
                ),
                Err(ArtifactLoweringError::Verification(_))
            ),
            "native entrance must reject {corruption} during verification"
        );
    }
}
