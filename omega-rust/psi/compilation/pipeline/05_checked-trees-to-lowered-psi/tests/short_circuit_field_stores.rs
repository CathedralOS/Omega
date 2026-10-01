//! A field store whose value is a short-circuit Boolean evaluates it through
//! its own decision blocks and stores the rejoined result: a content
//! comparison's reads stay guarded by the length test that dominates them.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_psi::{OperationKind, Terminator};

const SOURCE: &str = r#"
    domain [u8; 4]::Utf8 requires valid_utf8(self);
    data Main { name: [u8; 4] in Utf8; count: u32; ok: bool; }
    machine Main::check(&mut self) {
        self.ok = self.name != "omg" && self.count < 3;
    }
"#;

#[test]
fn a_short_circuit_boolean_field_store_rejoins_its_decision() {
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::check"),
    )
    .unwrap_or_else(|error| panic!("{error:?}"));
    let module = terminal_codec::decode_module(
        &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
    )
    .unwrap();
    let proof = terminal_codec::decode_proof_bundle(
        &terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap(),
    )
    .unwrap();
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("the staged decision and its store verify independently");
    let machine = &module.machines[0];
    // The decision branches on its first test before the store runs.
    assert!(
        machine
            .blocks
            .iter()
            .any(|block| matches!(block.terminator, Terminator::Conditional { .. })),
        "the short-circuit value is a decision, not one block"
    );
    let stored = machine
        .blocks
        .iter()
        .find_map(|block| {
            block
                .operations
                .iter()
                .find_map(|operation| match &operation.kind {
                    OperationKind::StructuralScalarFieldStore { value, .. } => {
                        Some((block, *value))
                    }
                    _ => None,
                })
        })
        .expect("the field store");
    let (block, value) = stored;
    assert!(
        block
            .parameters
            .iter()
            .any(|parameter| parameter.id == value),
        "the store takes the decision's rejoined result"
    );
}
