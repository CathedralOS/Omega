use super::*;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn execute_identity(artifact: &terminal_codec::CanonicalTerminalArtifact) {
    let module = decode_module(artifact.semantic_bytes()).expect("reload semantics");
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let parameter = &caller.structural_parameters[0];
    let argument = TerminalStructuralValue {
        opaque_identity: 0x5eed,
        structural_type: parameter.structural_type,
        qualifications: parameter.qualifications.clone(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
        &[],
        std::slice::from_ref(&argument),
    )
    .expect("verify and execute without source custody");
    let mut meter = TerminalFuelMeter::with_allowance(8);
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
            TerminalStructuralResult {
                value: argument,
                claims: vec![caller.entry_claims[0].claim],
            }
        ))
    );
}

#[test]
fn structural_call_retains_generic_callee_reach_after_publication() {
    for count in [2, 7] {
        let source = SOURCE
            .replace(
                "Main::forward(region:",
                "Main::forward<const Count: u64>(region:",
            )
            .replace(
                "Main::forward(region)",
                &format!("Main::forward<{count}>(region)"),
            );
        let checked = checked(&source);
        let [specialization] = checked.machine_specializations.as_slice() else {
            panic!("one exact generic callee application");
        };
        let expected_commitment = specialization.commitment.as_bytes();
        let expected_argument = specialization.const_argument_identities[0].clone();
        let artifact =
            terminal_production::TerminalProductionRequest::new(&checked, "Main::through_call")
                .produce_artifact()
                .expect("publish linear structural call");
        drop(checked);
        let module =
            decode_module(artifact.semantic_bytes()).expect("reload without source custody");
        let caller = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let terminal_psi::OperationKind::CallStructural { callee, .. } =
            caller.blocks[0].operations[0].kind
        else {
            panic!("ordinary structural call remains present");
        };
        let target = module
            .machines
            .iter()
            .find(|machine| machine.id == callee)
            .unwrap();
        let application = target
            .closed_reach_application
            .as_ref()
            .expect("generic callee retains its reach application");
        assert_eq!(application.specialization_commitment, expected_commitment);
        assert_eq!(
            application.telescope,
            [terminal_psi::ClosedReachParameter::Const {
                argument: expected_argument
            }]
        );
        assert!(application.fixed.is_empty());
        assert!(application.dependencies.is_empty());
        execute_identity(&artifact);

        let mut invalid = module.clone();
        let caller = invalid
            .machines
            .iter_mut()
            .find(|machine| machine.id == invalid.entry)
            .unwrap();
        let terminal_psi::OperationKind::CallStructural {
            returned_claim_transfers,
            ..
        } = &mut caller.blocks[0].operations[0].kind
        else {
            panic!("structural call");
        };
        returned_claim_transfers.clear();
        let proof =
            terminal_codec::decode_proof_bundle(artifact.proof_bytes()).expect("reload proof");
        assert!(
            terminal_verifier::verify_module(&invalid, &proof, &AdmissionProfile::default())
                .is_err(),
            "retained reach must not replace the ordinary returned-claim checks"
        );
    }
}

#[test]
fn structural_call_rejects_stale_source_coordinates_and_same_shaped_targets() {
    let checked = checked(
        &(SOURCE.to_owned()
            + "\nmachine Main::alternative(region: Region in Owned) -> Region in Owned { region }"),
    );
    let alternate = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::alternative")
        .unwrap();
    let alternate_state = checked.machine_states(alternate)[0].symbol;
    let alternate_claim = checked
        .facts
        .flow
        .terminal_structural_returns
        .for_machine(alternate.symbol)
        .expect("same-shaped alternate leaf")
        .transferred_claim;
    for mutation in ["statement", "ordinal", "target"] {
        let mut invalid = checked.clone();
        let plan = &mut invalid.facts.flow.terminal_structural_call_returns.machines[0];
        match mutation {
            "statement" => plan.call.coordinate.statement_index += 1,
            "ordinal" => plan.call.coordinate.call_ordinal += 1,
            "target" => {
                plan.call.target_machine = alternate.symbol;
                plan.call.target_state = alternate_state;
                plan.call.callee_returned_claim = alternate_claim;
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&invalid, "Main::through_call")
                .produce_artifact()
                .is_err(),
            "stale structural call {mutation} must reject"
        );
    }
}
