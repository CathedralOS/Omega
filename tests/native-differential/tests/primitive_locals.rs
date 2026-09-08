//! Source-produced locals keep their address, writes, reads and reinitialization.
use proof_admission::AdmissionProfile;
use terminal_codec::CanonicalTerminalArtifact;

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
#[path = "primitive_locals/publication.rs"]
mod publication;

fn produce(source: &str, entry: &str) -> CanonicalTerminalArtifact {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    let artifact = terminal_production::produce_terminal_artifact(&checked, entry).unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    assert_eq!(
        terminal_codec::encode_module(&module).unwrap(),
        artifact.semantic_bytes()
    );
    assert_eq!(
        terminal_codec::encode_proof_bundle(&proof).unwrap(),
        artifact.proof_bytes()
    );
    assert!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::EstablishPrimitiveLocal { .. }
            ))
    );
    artifact
}

#[test]
fn unchanged_walk_publishes_on_four_targets() {
    publication::assert_four_targets(&produce(include_str!("primitive_locals/walk.omg"), "walk"));
}

#[test]
fn unchanged_walk_executes_zero_and_several_iterations() {
    publication::assert_host_execution(
        &produce(include_str!("primitive_locals/walk.omg"), "walk"),
        include_str!("primitive_locals/walk.c"),
    );
}

#[test]
fn caller_observes_callee_write_and_independent_scalar_result() {
    let artifact = produce(include_str!("primitive_locals/observe.omg"), "observe");
    publication::assert_four_targets(&artifact);
    publication::assert_host_execution(&artifact, include_str!("primitive_locals/observe.c"));
}

#[test]
fn local_is_reinitialized_before_each_loop_borrow() {
    let artifact = produce(
        include_str!("primitive_locals/reinitialize.omg"),
        "reinitialize",
    );
    publication::assert_four_targets(&artifact);
    publication::assert_host_execution(&artifact, include_str!("primitive_locals/reinitialize.c"));
}

#[test]
fn direct_local_replacement_is_a_fresh_read_not_the_initializer() {
    let source = "machine reset(value: &mut u64) -> u64 { value = 0; 0 }
    machine replace(replacement: u64) -> u64 {
        let mut scratch: u64 = 91;
        let ignored: u64 = reset(&mut scratch);
        scratch = replacement;
        scratch
    }";
    let artifact = produce(source, "replace");
    publication::assert_four_targets(&artifact);
    publication::assert_host_execution(&artifact, include_str!("primitive_locals/replace.c"));
}
