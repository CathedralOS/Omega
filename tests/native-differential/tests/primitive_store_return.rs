//! Source-produced primitive writes retain caller storage and scalar return ABI.
use proof_admission::AdmissionProfile;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::OperationKind;

#[path = "primitive_store_return/publication.rs"]
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

const RESET: &str = "machine reset(value: &mut u64) -> u64 { value = 0; 0 }";
const REPLACE: &str = "machine replace(value: &mut u64, replacement: u64, returned: u64) -> u64 {
    value = replacement;
    returned
}";

// Nine scalar inputs put both the replacement and final reference on the stack
// on every hosted ABI; the independent return input remains register-passed.
const STACK_REPLACE: &str = "machine replace(value: &mut u64, returned: u64,
    argument1: u64, argument2: u64, argument3: u64, argument4: u64,
    argument5: u64, argument6: u64, argument7: u64, replacement: u64) -> u64 {
    value = replacement;
    returned
}";

fn produce(source: &str, entry: &str) -> CanonicalTerminalArtifact {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize store-return source");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse store-return source");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve store-return source");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type store-return source");
    let checked =
        typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check store-return source");
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, entry)
        .produce_artifact()
        .expect("publish store-return Terminal");
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
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("independently verify reloaded Terminal");
    let [machine] = module.machines.as_slice() else {
        panic!("only the authored callee belongs to this fixture")
    };
    let [parameter] = machine.structural_parameters.as_slice() else {
        panic!("one original primitive referent")
    };
    assert_eq!(
        parameter.access,
        terminal_psi::StructuralAccess::MutableBorrow
    );
    let stores = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::WriteOnlyPrimitiveStore { destination, value } => {
                Some((destination, value))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        stores.len(),
        1,
        "the mutation must survive source production"
    );
    assert_eq!(stores[0].0, parameter.place);
    assert!(
        machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .all(|operation| !matches!(
                operation.kind,
                OperationKind::EstablishPrimitiveLocal { .. }
                    | OperationKind::PrimitiveScalarRead { .. }
            )),
        "this milestone does not depend on activation-local primitive realization"
    );
    artifact
}

#[test]
fn reset_publishes_and_replays_on_four_targets() {
    publication::assert_four_targets(&produce(RESET, "reset"));
}

#[test]
fn reset_mutates_original_word_and_returns_zero_repeatedly() {
    publication::assert_host_execution(
        &produce(RESET, "reset"),
        include_str!("primitive_store_return/reset.c"),
    );
}

#[test]
fn runtime_store_input_and_scalar_result_publish_and_replay_on_four_targets() {
    publication::assert_four_targets(&produce(REPLACE, "replace"));
}

#[test]
fn runtime_store_input_and_scalar_result_remain_independent() {
    publication::assert_host_execution(
        &produce(REPLACE, "replace"),
        include_str!("primitive_store_return/replace.c"),
    );
}

#[test]
fn installation_rejects_substituted_primitive_reference_contracts() {
    publication::assert_corruptions(&produce(REPLACE, "replace"));
}

#[test]
fn stack_store_input_and_reference_publish_and_replay_on_four_targets() {
    publication::assert_four_targets(&produce(STACK_REPLACE, "replace"));
}

#[test]
fn stack_store_input_and_reference_preserve_scalar_return() {
    publication::assert_host_execution(
        &produce(STACK_REPLACE, "replace"),
        include_str!("primitive_store_return/stack_replace.c"),
    );
}
