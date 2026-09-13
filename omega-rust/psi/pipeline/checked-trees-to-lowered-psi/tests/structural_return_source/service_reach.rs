use super::*;

const NOMINAL_CALLBACK: &str = r#"
    data ByteUnit {}
    data CountedQuantity<Unit> { magnitude: u64; }
    trait Content<A> { machine project(subject: &Self) -> A; }
    data Region [linear] { length: u64; }
    domain Region::Owned;
    machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
    satisfies Content<CountedQuantity<ByteUnit>>::project
    { CountedQuantity { magnitude: region.length } }
    trait RegionForward {
        machine call(region: Region in Region::Owned) -> Region in Region::Owned;
    }
    data Main {}
    machine Main::forward(region: Region in Owned) -> Region in Owned
    satisfies RegionForward::call
    { region }
    machine Main::through_call<machine Selected>(region: Region in Owned) -> Region in Owned
    where machine Selected satisfies RegionForward::call;
    { Selected(region) }
    machine Main::demand(region: Region in Owned) -> Region in Owned {
        Main::through_call<Main::forward>(region)
    }
"#;

#[test]
fn nominal_linear_callback_publishes_and_executes_with_exact_reach() {
    for prefix in ["", "let marker: u64 = 7;"] {
        let source = NOMINAL_CALLBACK.replace(
            "{ Selected(region) }",
            &format!("{{ {prefix} Selected(region) }}"),
        );
        let checked = checked(&source);
        let [specialization] = checked.machine_specializations.as_slice() else {
            panic!("one exact nominal callback application");
        };
        let commitment = specialization.commitment.as_bytes();
        let artifact =
            terminal_production::TerminalProductionRequest::new(&checked, "Main::demand")
                .produce_artifact()
                .expect("publish transitive nominal linear callback");
        drop(checked);
        let module =
            decode_module(artifact.semantic_bytes()).expect("reload source-free semantics");
        let applications = module
            .machines
            .iter()
            .filter_map(|machine| machine.closed_reach_application.as_ref())
            .collect::<Vec<_>>();
        let [application] = applications.as_slice() else {
            panic!("one retained reach application");
        };
        assert_eq!(application.specialization_commitment, commitment);
        assert_eq!(application.dependencies, [0]);
        let [terminal_psi::ClosedReachParameter::Machine(binding)] =
            application.telescope.as_slice()
        else {
            panic!("nominal machine parameter");
        };
        assert!(
            binding
                .nominal_requirement
                .as_ref()
                .is_some_and(|identity| identity.contains("RegionForward"))
        );
        assert!(binding.selected_identity.contains("Main::forward"));
        assert!(binding.callee.is_some());
        assert_eq!(application.calls.len(), 1);
        execute_identity(&artifact);

        let mut invalid = module.clone();
        let call = invalid
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| match &mut operation.kind {
                terminal_psi::OperationKind::CallStructuralWithScalarArguments {
                    returned_claim_transfers,
                    ..
                }
                | terminal_psi::OperationKind::CallStructural {
                    returned_claim_transfers,
                    ..
                } => Some(returned_claim_transfers),
                _ => None,
            })
            .expect("shared ordinary structural call");
        call.clear();
        let proof =
            terminal_codec::decode_proof_bundle(artifact.proof_bytes()).expect("reload proof");
        assert!(
            terminal_verifier::verify_module(&invalid, &proof, &AdmissionProfile::default())
                .is_err(),
            "closed reach never substitutes for returned claim custody"
        );
    }
}

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

#[test]
fn nominal_linear_callback_rejects_stale_checked_call_custody() {
    let checked = checked(NOMINAL_CALLBACK);
    let root = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::demand")
        .unwrap()
        .symbol;
    for mutation in [
        "statement",
        "ordinal",
        "qualifications",
        "input_claim",
        "returned_claim",
        "result_binding",
    ] {
        let mut invalid = checked.clone();
        let plan = invalid
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .find(|plan| plan.machine == root)
            .expect("ordinary shared graph plan");
        let operation = plan
            .states
            .iter_mut()
            .flat_map(|state| &mut state.operations)
            .find(|operation| {
                matches!(
                    operation,
                    checked_trees::CheckedUnitEffectOperationPlan::StructuralCall { .. }
                )
            })
            .expect("ordinary structural call");
        let checked_trees::CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            custody,
            result,
            ..
        } = operation
        else {
            unreachable!()
        };
        match mutation {
            "statement" => coordinate.statement_index += 1,
            "ordinal" => coordinate.call_ordinal += 1,
            "qualifications" => custody.result_qualifications.clear(),
            "input_claim" => custody.claim_transfers.clear(),
            "returned_claim" => custody.returned_claim_transfers.clear(),
            "result_binding" => result.binding_ordinal += 1,
            _ => unreachable!(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&invalid, "Main::demand")
                .produce_artifact()
                .is_err(),
            "stale checked {mutation} must reject"
        );
    }
}

#[test]
fn nominal_linear_callback_rejects_changed_source_claim_lineage() {
    let checked = checked(NOMINAL_CALLBACK);
    let root = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::demand")
        .unwrap()
        .symbol;
    for source in [
        language_semantics::PermissionEventKind::Establish,
        language_semantics::PermissionEventKind::Transfer,
    ] {
        let mut invalid = checked.clone();
        let event = invalid
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .find(|(_, event)| {
                event.machine_symbol == root
                    && event.kind == source
                    && event.multiplicity == Multiplicity::Linear
            })
            .map(|(handle, _)| handle)
            .expect("exact source claim event");
        invalid
            .facts
            .flow
            .ownership
            .permissions
            .get_mut(event)
            .claim_identity = PermissionClaimIdentity::Unknown;
        assert!(
            terminal_production::TerminalProductionRequest::new(&invalid, "Main::demand")
                .produce_artifact()
                .is_err(),
            "changed source {source:?} lineage must reject"
        );
    }
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
    let caller = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::through_call")
        .unwrap()
        .symbol;
    for mutation in ["statement", "ordinal", "target"] {
        let mut invalid = checked.clone();
        let plan = invalid
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .find(|plan| plan.machine == caller)
            .expect("shared caller plan");
        let [
            checked_trees::CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate,
                target_machine,
                target_state,
                custody,
                ..
            },
        ] = plan.states[0].operations.as_mut_slice()
        else {
            panic!("one shared call");
        };
        match mutation {
            "statement" => coordinate.statement_index += 1,
            "ordinal" => coordinate.call_ordinal += 1,
            "target" => {
                *target_machine = alternate.symbol;
                *target_state = alternate_state;
                custody.returned_claim_transfers[0].callee_claim = alternate_claim;
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
