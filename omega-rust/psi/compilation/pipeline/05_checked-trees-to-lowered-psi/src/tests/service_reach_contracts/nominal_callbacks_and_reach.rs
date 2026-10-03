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

#[test]
fn closed_reach_retains_type_selected_cleanup_through_generic_forwarding() {
    let checked = crate::front_end::checked_program(
        r#"
        pub boundary trait Console { machine ping() reaches Console terminates; }
        pub data Token { value: bool; }
        pub data Plain { value: bool; }
        machine Token::drop(token: &mut Token) reaches Console { Console::ping(); }
        machine retire<T>(value: T) {}
        machine forward<T>(value: T) { retire(value); }
        pub machine enter(first: Token, second: Plain) reaches Console invokes Console; { forward(first); forward(second); }
    "#,
    );
    let mut hooked = 0;
    let mut plain = 0;
    for specialization in &checked.machine_specializations {
        let template = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == specialization.template)
            .unwrap();
        if !matches!(template.name.as_str(), "retire" | "forward") {
            continue;
        }
        let row = checked
            .facts
            .service_reaches
            .for_machine(specialization.instance)
            .unwrap();
        let cleanup = checked
            .facts
            .service_reaches
            .rows
            .services(row.cleanup_contribution);
        if specialization
            .type_argument_identities
            .iter()
            .any(|identity| identity.contains("Token"))
        {
            assert_eq!(cleanup.len(), 1);
            hooked += 1;
        } else {
            assert!(
                cleanup.is_empty(),
                "plain specialization cannot inherit another instance's cleanup"
            );
            plain += 1;
        }
    }
    assert_eq!((hooked, plain), (2, 2));
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("type-selected cleanup composes with the original closed reach dependency")
    .into_artifact();
    drop(checked);
    let module = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("reload closed cleanup reach");
    terminal_verifier::validate_module(&module).expect("source-free closed reach replay");
    let owner = module
        .machines
        .iter()
        .find(|machine| {
            machine
                .closed_reach_application
                .as_ref()
                .is_some_and(|application| !application.fixed.is_empty())
        })
        .expect("effectful generic cleanup retains its closed application");
    let application = owner.closed_reach_application.as_ref().unwrap();
    assert!(application.dependencies.is_empty());
    assert_eq!(application.fixed, owner.published_service_ceiling);
    let mut invalid = module.clone();
    invalid
        .machines
        .iter_mut()
        .find(|machine| machine.id == owner.id)
        .unwrap()
        .closed_reach_application
        .as_mut()
        .unwrap()
        .fixed
        .clear();
    assert!(
        terminal_verifier::validate_module(&invalid).is_err(),
        "omitted cleanup contribution cannot pass retained ceiling replay"
    );
}
