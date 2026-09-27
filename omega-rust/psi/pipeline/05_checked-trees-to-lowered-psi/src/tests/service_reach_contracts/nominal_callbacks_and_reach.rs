use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::OperationKind;

#[test]
fn direct_boundary_calls_transfer_both_owned_claims() {
    let checked = crate::front_end::checked_program(
        r#"
        pub data Extent [linear] { value: u64; }
        pub boundary trait Sink { machine take(first: Extent, second: Extent); }
        pub data Root {}
        pub machine Root::enter(first: Extent, second: Extent)
        reaches Sink invokes Sink;
        { Sink::take(first, second); }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Root::enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("publish direct boundary claim transfer")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).expect("reload claims");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry");
    assert_eq!(entry.entry_claims.len(), 2);
    let OperationKind::BoundaryCall {
        completion_receipts,
        ..
    } = &entry.blocks[0].operations[0].kind
    else {
        panic!("boundary call");
    };
    assert_eq!(completion_receipts.len(), 2);
    for mutation in 0..4 {
        let mut invalid = module.clone();
        let entry = invalid
            .machines
            .iter_mut()
            .find(|machine| machine.id == invalid.entry)
            .expect("entry");
        let OperationKind::BoundaryCall {
            completion_receipts,
            ..
        } = &mut entry.blocks[0].operations[0].kind
        else {
            panic!("boundary call");
        };
        match mutation {
            0 => {
                completion_receipts.pop();
            }
            1 => completion_receipts[1] = completion_receipts[0],
            2 => completion_receipts[0].argument_index = 1,
            _ => completion_receipts.swap(0, 1),
        }
        assert!(
            terminal_verifier::validate_module(&invalid).is_err(),
            "missing, duplicate, wrong-position and reordered receipts reject: {mutation}"
        );
    }
}
