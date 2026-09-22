//! Source-produced locals keep their address, writes, reads and reinitialization.

// The front end these fixtures run is named here rather than re-sequenced at
// every site.
#[path = "common/front_end.rs"]
mod front_end;

use proof_admission::AdmissionProfile;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

#[path = "primitive_locals/boolean_control.rs"]
mod boolean_control;
#[path = "primitive_locals/literals.rs"]
mod literals;
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
#[path = "primitive_locals/widths.rs"]
mod widths;

fn produce(source: &str, entry: &str) -> CanonicalTerminalArtifact {
    let checked = crate::front_end::checked_program(source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name(entry),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    assert_eq!(
        terminal_codec::encode_module(&module).unwrap(),
        artifact.semantic_bytes()
    );
    assert_eq!(
        terminal_codec::encode_proof_section(&module, &proof).unwrap(),
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
