//! A structural local bound by its initializer's call lowers as that call's
//! owned result and verifies independently.

use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

#[test]
fn call_bound_structural_local_lowers_and_verifies_beside_a_store() {
    let checked = crate::front_end::checked_program(
        "data Inner { value: u64; }
         data Holder { inner: Inner; count: u64; }
         data Main { total: u64; }
         machine make() -> Holder { Holder { inner: Inner { value: 1 }, count: 2 } }
         machine Main::main(&mut self) { let h: Holder = make(); self.total = 5; }",
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Main::main"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("the bound call result and the following store lower")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let kinds = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .map(|operation| {
            let kind = format!("{:?}", operation.kind);
            kind.split([' ', '{', '(']).next().unwrap().to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            "CallStructuralWithScalarArguments",
            "IntegerConstant",
            "StructuralScalarFieldStore",
        ],
        "the entry performs the binding call, then the store's literal and the store"
    );
    terminal_verifier::verify_module(
        &module,
        &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
}
