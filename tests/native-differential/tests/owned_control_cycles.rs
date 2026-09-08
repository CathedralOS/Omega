//! Whole owned arrivals retain semantics even when runtime code never reads them.

use proof_admission::AdmissionProfile;
use terminal_codec::CanonicalTerminalArtifact;

#[path = "scalar_control_cycles/publication.rs"]
mod publication;

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

const SOURCE: &str = include_str!("owned_control_cycles/countdown.omg");
const DRIVER: &str = include_str!("owned_control_cycles/countdown.c");
const RANK: &str =
    "terminates by remaining -> Nat::Descending in 0..(limits.limit % limits.divisor + 6);\n";

fn produce(source: &str) -> CanonicalTerminalArtifact {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    let artifact = terminal_production::produce_terminal_artifact(&checked, "countdown").unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    assert_eq!(
        terminal_codec::encode_module(&module).unwrap(),
        artifact.semantic_bytes()
    );
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert!(!entry.structural_parameters.is_empty());
    assert!(
        entry
            .blocks
            .iter()
            .any(|block| !block.structural_parameters.is_empty())
    );
    artifact
}

#[test]
fn natural_owned_arrivals_publish_on_four_targets() {
    publication::assert_four_targets(&produce(SOURCE), 1);
}

#[test]
fn natural_owned_arrivals_execute_zero_and_several_iterations() {
    publication::assert_host_execution(&produce(SOURCE), 1, DRIVER);
}

#[test]
fn unranked_owned_arrivals_publish_on_four_targets() {
    assert_eq!(SOURCE.matches(RANK).count(), 1);
    publication::assert_four_targets(&produce(&SOURCE.replacen(RANK, "", 1)), 1);
}

#[test]
fn whole_owned_backedge_swaps_publish_on_four_targets() {
    publication::assert_four_targets(
        &produce(include_str!("owned_control_cycles/swapped.omg")),
        1,
    );
}

#[test]
fn whole_owned_backedge_swaps_execute_zero_and_several_iterations() {
    publication::assert_host_execution(
        &produce(include_str!("owned_control_cycles/swapped.omg")),
        1,
        include_str!("owned_control_cycles/swapped.c"),
    );
}
