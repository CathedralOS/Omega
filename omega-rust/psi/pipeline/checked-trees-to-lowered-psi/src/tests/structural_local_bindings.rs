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

#[test]
fn partial_literal_local_lowers_its_zero_fields_and_verifies() {
    // A mutable partial literal whose omitted members are a scalar leaf and a
    // nested record: the declared-order record plan replays both zeros at
    // emission, so the lowered module carries the nested record establishment
    // plus the enclosing record, then the &mut method call, and the whole
    // module verifies.
    let checked = crate::front_end::checked_program(
        "data Inner [copy] { checksum: u64; }
         data Info [copy] { id: u64; inner: Inner; len: u64; }
         machine Info::new(id: u64) -> Info {
             let mut info: Info = Info { id: id };
             info.set_len(id);
             info
         }
         machine Info::set_len(&mut self, v: u64) {
             self.len = v;
         }",
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Info::new"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("the partial literal's zero members lower")
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
    assert!(
        kinds
            .iter()
            .filter(|kind| kind.as_str() == "EstablishRecord")
            .count()
            >= 2,
        "the omitted nested record mints its own establishment before the enclosing record: {kinds:?}"
    );
    terminal_verifier::verify_module(
        &module,
        &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
}

#[test]
fn fixed_array_membered_result_verification_is_the_residual() {
    // The omitted closed array mints its zero through the same route an
    // authored array member would, but the verifier's plain/constructible
    // result classification recurses only into Record members: a record with
    // a fixed-array member is not a plain result type, so the return fails
    // StructuralResultMustBeOwned before any operation validation.
    let checked = crate::front_end::checked_program(
        "data Info [copy] { id: u64; tag: [u8; 4]; len: u64; }
         machine Info::new(id: u64) -> Info {
             let info: Info = Info { id: id, len: 0 };
             info
         }",
    );
    let error = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Info::new"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect_err("a fixed-array membered result still lacks a plain classification");
    assert!(
        format!("{error:?}").contains("StructuralResultMustBeOwned"),
        "the closed-array result wall is StructuralResultMustBeOwned: {error:?}"
    );
}

#[test]
fn empty_literal_local_lowers_zero_scalar_members_and_verifies() {
    let checked = crate::front_end::checked_program(
        "data Pair { a: u64; b: u64; }
         machine make() -> Pair {
             let p: Pair = Pair {};
             p
         }",
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("make"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("the empty literal's zero members lower")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
}
