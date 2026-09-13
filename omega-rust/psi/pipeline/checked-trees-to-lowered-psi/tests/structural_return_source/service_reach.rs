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
fn nominal_linear_callback_result_accepts_mixed_scalar_arguments() {
    for body in [
        "let forwarded: Region in Owned = Selected(region); Main::with_markers(7, forwarded, 9)",
        "let forwarded: Region in Owned = Selected(region); Main::with_markers(Main::marker(7), forwarded, Main::marker(9))",
        "let forwarded: Region in Owned = Selected(region); let marked: Region in Owned = Main::with_markers(Main::marker(7), forwarded, Main::marker(9)); Main::forward(marked)",
        "let forwarded: Region in Owned = Main::with_markers(7, region, 9); let returned: Region in Owned = Main::with_markers(11, forwarded, 13); Main::forward(returned)",
    ] {
        let source = mixed_callback_source(body);
        let checked = checked(&source);
        let artifact =
            terminal_production::TerminalProductionRequest::new(&checked, "Main::demand")
                .produce_artifact()
                .expect("publish scalar arguments around a live linear callback result");
        drop(checked);
        execute_identity(&artifact, 32);
        // Computed operands cross ordinary continuation parameters; their
        // exact source roots are covered by the mutation test below.
        if body.contains("Main::marker") {
            continue;
        }
        let module = decode_module(artifact.semantic_bytes()).expect("reload mixed operands");
        let mut actual_pairs = Vec::new();
        for machine in &module.machines {
            for block in &machine.blocks {
                for (call_index, operation) in block.operations.iter().enumerate() {
                    let terminal_psi::OperationKind::CallStructuralWithScalarArguments {
                        arguments,
                        claim_transfers,
                        ..
                    } = &operation.kind
                    else {
                        continue;
                    };
                    assert_eq!(arguments.len(), 2);
                    assert_eq!(
                        claim_transfers[0].argument_index, 0,
                        "the structural index is not its authored parameter position"
                    );
                    let values = arguments
                        .iter()
                        .map(|argument| {
                            let mut value = *argument;
                            let mut available = &block.operations[..call_index];
                            loop {
                                let (source_index, source) = available
                                    .iter()
                                    .enumerate()
                                    .find(|(_, candidate)| {
                                        candidate
                                            .result
                                            .scalar()
                                            .is_some_and(|result| result.id == value)
                                    })
                                    .expect("each scalar producer precedes its consumer");
                                match &source.kind {
                                    terminal_psi::OperationKind::IntegerConstant { value } => {
                                        break *value;
                                    }
                                    terminal_psi::OperationKind::Call { arguments, .. } => {
                                        assert_eq!(arguments.len(), 1);
                                        value = arguments[0];
                                        available = &available[..source_index];
                                    }
                                    _ => panic!("literal or identity marker call"),
                                }
                            }
                        })
                        .collect::<Vec<_>>();
                    actual_pairs.push(values);
                }
            }
        }
        let mut expected = vec![vec![
            semantic_vocabulary::IntegerValue::Unsigned(7),
            semantic_vocabulary::IntegerValue::Unsigned(9),
        ]];
        if body.contains("11, forwarded, 13") {
            expected.push(vec![
                semantic_vocabulary::IntegerValue::Unsigned(11),
                semantic_vocabulary::IntegerValue::Unsigned(13),
            ]);
        }
        assert_eq!(actual_pairs, expected);
    }
}

fn mixed_callback_source(body: &str) -> String {
    NOMINAL_CALLBACK.replace("{ Selected(region) }", &format!("{{ {body} }}"))
        + "machine Main::with_markers(before: u64, region: Region in Owned, after: u64) -> Region in Owned { region }\n machine Main::marker(value: u64) -> u64 { value }"
}

#[test]
fn mixed_linear_call_rejects_changed_source_operand_positions() {
    for body in [
        "let forwarded: Region in Owned = Selected(region); Main::with_markers(7, forwarded, 9)",
        "let forwarded: Region in Owned = Selected(region); Main::with_markers(Main::marker(7), forwarded, Main::marker(9))",
    ] {
        let checked = checked(&mixed_callback_source(body));
        let _artifact =
            terminal_production::TerminalProductionRequest::new(&checked, "Main::demand")
                .produce_artifact()
                .expect("unchanged mixed call publishes");
        for mutation in ["scalar_order", "scalar_arity", "claim_position"] {
            let mut invalid = checked.clone();
            let operation = invalid.facts.flow.terminal_unit_effects.composed_machines
            .iter_mut()
            .flat_map(|machine| &mut machine.states)
            .flat_map(|state| &mut state.operations)
            .find(|operation| matches!(operation,
                checked_trees::CheckedUnitEffectOperationPlan::StructuralCall { scalar_arguments, .. }
                if scalar_arguments.len() == 2))
            .expect("mixed call in the shared graph");
            let checked_trees::CheckedUnitEffectOperationPlan::StructuralCall {
                scalar_arguments,
                custody,
                ..
            } = operation
            else {
                unreachable!()
            };
            match mutation {
                "scalar_order" => scalar_arguments.swap(0, 1),
                "scalar_arity" => {
                    scalar_arguments.pop();
                }
                "claim_position" => custody.claim_transfers[0].argument_index = 1,
                _ => unreachable!(),
            }
            assert!(
                terminal_production::TerminalProductionRequest::new(&invalid, "Main::demand")
                    .produce_artifact()
                    .is_err(),
                "changed {mutation} must reject at source replay"
            );
        }
    }
}

#[test]
fn mixed_linear_call_replay_rejects_stale_values_and_claims() {
    let checked = checked(&mixed_callback_source(
        "let first: Region in Owned = Main::with_markers(7, region, 9); Main::with_markers(11, first, 13)",
    ));
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::demand")
        .produce_artifact()
        .expect("publish mixed producer and consumer");
    drop(checked);
    execute_identity(&artifact, 32);
    let module = decode_module(artifact.semantic_bytes()).expect("reload without source");
    for mutation in [
        "scalar_arity",
        "future_scalar",
        "stale_input",
        "own_result",
        "returned_claim",
        "qualification",
        "multiplicity",
        "content_guarantee",
        "scalar_type",
    ] {
        let mut invalid = module.clone();
        if mutation == "content_guarantee" || mutation == "scalar_type" {
            let leaf = invalid
                .machines
                .iter_mut()
                .find(|machine| {
                    machine.parameters.len() == 2 && !machine.content_identity_reshuffles.is_empty()
                })
                .expect("mixed callee's checked content guarantee");
            if mutation == "content_guarantee" {
                leaf.content_identity_reshuffles.clear();
            } else {
                leaf.parameters[0].scalar_type = semantic_vocabulary::ScalarType::Boolean;
            }
        } else {
            let caller = invalid
                .machines
                .iter_mut()
                .find(|machine| {
                    machine.blocks.iter().flat_map(|block| &block.operations)
                    .filter(|operation| matches!(operation.kind,
                        terminal_psi::OperationKind::CallStructuralWithScalarArguments { .. }))
                    .count() == 2
                })
                .expect("two mixed calls in authored order");
            let calls = caller
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .filter(|operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::CallStructuralWithScalarArguments { .. }
                    )
                })
                .collect::<Vec<_>>();
            let [producer, consumer]: [&mut terminal_psi::Operation; 2] = calls.try_into().unwrap();
            let terminal_psi::OperationKind::CallStructuralWithScalarArguments {
                arguments: produced_arguments,
                ..
            } = &mut producer.kind
            else {
                unreachable!()
            };
            let terminal_psi::OperationKind::CallStructuralWithScalarArguments {
                arguments,
                structural_arguments,
                returned_claim_transfers,
                ..
            } = &mut consumer.kind
            else {
                unreachable!()
            };
            match mutation {
                "scalar_arity" => {
                    arguments.pop();
                }
                "future_scalar" => produced_arguments[0] = arguments[0],
                "stale_input" => {
                    structural_arguments[0].place = caller.structural_parameters[0].place
                }
                "own_result" => {
                    structural_arguments[0].place = consumer.result.structural().unwrap().place
                }
                "returned_claim" => returned_claim_transfers.clear(),
                "qualification" | "multiplicity" => {
                    let terminal_psi::OperationResult::Structural(result) = &mut consumer.result
                    else {
                        unreachable!()
                    };
                    if mutation == "qualification" {
                        result.qualifications.clear();
                    } else {
                        result.multiplicity = terminal_psi::StructuralMultiplicity::Affine;
                    }
                }
                _ => unreachable!(),
            }
        }
        assert!(
            terminal_verifier::validate_module(&invalid).is_err(),
            "source-free {mutation} must reject independently of an old proof fingerprint"
        );
    }
}

#[test]
fn nominal_linear_callback_result_feeds_an_ordinary_call() {
    for body in [
        "let forwarded: Region in Owned = Selected(region); Main::forward(forwarded)",
        "let marker: u64 = 7; let forwarded: Region in Owned = Selected(region); let returned: Region in Owned = Main::forward(forwarded); returned",
    ] {
        let source = NOMINAL_CALLBACK.replace("{ Selected(region) }", &format!("{{ {body} }}"));
        let checked = checked(&source);
        let artifact =
            terminal_production::TerminalProductionRequest::new(&checked, "Main::demand")
                .produce_artifact()
                .expect("publish successive calls carrying one qualified linear claim");
        drop(checked);
        execute_identity(&artifact, 8);
        let mut module = decode_module(artifact.semantic_bytes()).expect("reload result handoff");
        let caller = module
            .machines
            .iter_mut()
            .find(|machine| {
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter(|operation| {
                        matches!(
                            operation.kind,
                            terminal_psi::OperationKind::CallStructural { .. }
                        )
                    })
                    .count()
                    == 2
            })
            .expect("two ordered structural invocations");
        let calls = caller
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .filter(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::CallStructural { .. }
                )
            })
            .collect::<Vec<_>>();
        let [producer, consumer]: [&mut terminal_psi::Operation; 2] =
            calls.try_into().expect("two calls");
        let terminal_psi::OperationResult::Structural(produced) = &producer.result else {
            unreachable!()
        };
        let terminal_psi::OperationResult::Structural(returned) = &consumer.result else {
            unreachable!()
        };
        assert_eq!(produced.qualifications, returned.qualifications);
        assert_eq!(produced.claims, returned.claims);
        assert_ne!(produced.place, returned.place);
        let terminal_psi::OperationKind::CallStructural {
            structural_arguments,
            ..
        } = &mut consumer.kind
        else {
            unreachable!()
        };
        assert_eq!(structural_arguments[0].place, produced.place);
        structural_arguments[0].place = caller.structural_parameters[0].place;
        assert!(
            terminal_verifier::validate_module(&module).is_err(),
            "source-free replay must reject moving the consumed input instead of its result"
        );
    }
}

#[test]
fn nominal_linear_callback_result_cannot_be_moved_twice() {
    for consumed in ["region", "forwarded"] {
        let source = NOMINAL_CALLBACK.replace(
            "{ Selected(region) }",
            &format!("{{ let forwarded: Region in Owned = Selected(region); let returned: Region in Owned = Main::forward(forwarded); Main::forward({consumed}) }}"),
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        assert!(
            lower_typed_trees(typed).is_err(),
            "moving {consumed} twice must reject"
        );
    }
}

#[test]
fn nominal_linear_callback_result_frontier_rejects_stale_and_future_places() {
    let source = NOMINAL_CALLBACK.replace(
        "{ Selected(region) }",
        "{ let first: Region in Owned = Selected(region); let second: Region in Owned = Main::forward(first); Main::forward(second) }",
    );
    let checked = checked(&source);
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::demand")
        .produce_artifact()
        .expect("publish three successive calls");
    drop(checked);
    execute_identity(&artifact, 10);
    let module = decode_module(artifact.semantic_bytes()).expect("reload three-call frontier");
    for mutation in [
        "stale_result",
        "own_result",
        "future_result",
        "qualification",
        "claim",
        "missing_content_guarantee",
    ] {
        let mut invalid = module.clone();
        if mutation == "missing_content_guarantee" {
            let leaf = invalid
                .machines
                .iter_mut()
                .find(|machine| {
                    !machine.content_identity_reshuffles.is_empty()
                        && machine
                            .blocks
                            .iter()
                            .all(|block| block.operations.is_empty())
                })
                .expect("identity-forwarding leaf");
            leaf.content_identity_reshuffles.clear();
            assert!(
                terminal_verifier::validate_module(&invalid).is_err(),
                "claim identity alone cannot replace the callee's content guarantee"
            );
            continue;
        }
        let caller = invalid
            .machines
            .iter_mut()
            .find(|machine| {
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter(|operation| {
                        matches!(
                            operation.kind,
                            terminal_psi::OperationKind::CallStructural { .. }
                        )
                    })
                    .count()
                    == 3
            })
            .expect("three-call caller");
        let calls = caller
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .filter(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::CallStructural { .. }
                )
            })
            .collect::<Vec<_>>();
        let [first, second, third]: [&mut terminal_psi::Operation; 3] =
            calls.try_into().expect("three calls");
        let first_place = first.result.structural().unwrap().place;
        let second_place = second.result.structural().unwrap().place;
        let third_place = third.result.structural().unwrap().place;
        match mutation {
            "qualification" | "claim" => {
                let terminal_psi::OperationResult::Structural(result) = &mut second.result else {
                    unreachable!()
                };
                if mutation == "qualification" {
                    result.qualifications.clear();
                } else {
                    result.claims.clear();
                }
            }
            _ => {
                let (consumer, source) = match mutation {
                    "stale_result" => (third, first_place),
                    "own_result" => (second, second_place),
                    "future_result" => (second, third_place),
                    _ => unreachable!(),
                };
                let terminal_psi::OperationKind::CallStructural {
                    structural_arguments,
                    ..
                } = &mut consumer.kind
                else {
                    unreachable!()
                };
                structural_arguments[0].place = source;
            }
        }
        assert!(
            terminal_verifier::validate_module(&invalid).is_err(),
            "source-free {mutation} must reject"
        );
    }
}

#[test]
fn nominal_linear_callback_result_rejects_stale_consumer_custody() {
    let source = NOMINAL_CALLBACK.replace(
        "{ Selected(region) }",
        "{ let forwarded: Region in Owned = Selected(region); Main::forward(forwarded) }",
    );
    let checked = checked(&source);
    let _artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::demand")
        .produce_artifact()
        .expect("unmodified result handoff publishes");
    for mutation in [
        "moved_parameter",
        "future_result",
        "missing_transfer",
        "missing_return",
    ] {
        let mut invalid = checked.clone();
        let operation = invalid.facts.flow.terminal_unit_effects.composed_machines
            .iter_mut()
            .flat_map(|machine| &mut machine.states)
            .flat_map(|state| &mut state.operations)
            .find(|operation| matches!(operation,
                checked_trees::CheckedUnitEffectOperationPlan::StructuralCall { structural_arguments, .. }
                if structural_arguments.iter().any(|argument| argument.source_structural_result_binding_ordinal().is_some())))
            .expect("ordinary result consumer");
        let checked_trees::CheckedUnitEffectOperationPlan::StructuralCall {
            structural_arguments,
            result,
            custody,
            ..
        } = operation
        else {
            unreachable!()
        };
        match mutation {
            "moved_parameter" => {
                structural_arguments[0].source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: 0,
                    }
            }
            "future_result" => {
                structural_arguments[0].source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: result.binding_ordinal,
                    }
            }
            "missing_transfer" => custody.claim_transfers.clear(),
            "missing_return" => custody.returned_claim_transfers.clear(),
            _ => unreachable!(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&invalid, "Main::demand")
                .produce_artifact()
                .is_err(),
            "stale consumer {mutation} must reject"
        );
    }
}

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
        execute_identity(&artifact, 8);

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

fn execute_identity(artifact: &terminal_codec::CanonicalTerminalArtifact, fuel: u64) {
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
    let mut meter = TerminalFuelMeter::with_allowance(fuel);
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
        execute_identity(&artifact, 8);

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
