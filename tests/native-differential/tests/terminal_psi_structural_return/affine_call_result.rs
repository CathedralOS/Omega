//! Native calls retain the produced owner until its explicit whole-root discard.

use super::*;
use assigned_target_operations::AssignedUnitOperation;
use target_operations::TargetUnitOperation;

#[cfg(unix)]
#[path = "affine_call_result/host.rs"]
mod host;

#[test]
fn affine_call_result_without_scalar_arguments_reaches_native_cleanup() {
    check_call_result(false, false, false);
}

#[test]
fn wide_affine_call_result_retains_direct_aggregate_custody() {
    check_call_result(true, false, false);
}

#[test]
fn fixed_array_affine_call_results_retain_native_cleanup_custody() {
    check_call_result(false, true, false);
    check_call_result(true, true, false);
}

#[test]
fn scalar_prefixed_affine_call_result_preserves_the_existing_native_route() {
    check_call_result(false, false, true);
}

fn check_call_result(wide: bool, array: bool, scalar_prefix: bool) {
    let fields = if wide {
        "left: Token; right: Token;"
    } else {
        "value: u64;"
    };
    let carrier = if array {
        if wide { "[Token; 2]" } else { "[Token; 1]" }
    } else {
        "Payload"
    };
    let side = if scalar_prefix { "side: u64, " } else { "" };
    let actual = if scalar_prefix { "7u64, " } else { "" };
    let source = format!(
        "data Token {{ value: u64; }} data Payload {{ {fields} }}
         machine forward({side}value: {carrier}) -> {carrier} {{ value }}
         machine enter(value: {carrier}) {{ let result: {carrier} = forward({actual}value); }}"
    );
    let tokens = Lexer::new(&source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = lower_typed_trees(typed).unwrap();
    let terminal = lower_machine(&checked, "enter").unwrap();
    let semantic = encode_module(&terminal.semantic_module).unwrap();
    let proof = encode_proof_bundle(&terminal.proof_bundle).unwrap();
    drop(checked);
    drop(terminal);
    let module = decode_module(&semantic).unwrap();
    let proof_bundle = terminal_codec::decode_proof_bundle(&proof).unwrap();
    verify_module(&module, &proof_bundle, &AdmissionProfile::default()).unwrap();
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let input = caller.structural_parameters[0].place;
    let call = caller.blocks[0]
        .operations
        .iter()
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::CallStructuralWithScalarArguments { .. }
            )
        })
        .unwrap();
    let terminal_psi::OperationResult::Structural(produced) = &call.result else {
        panic!("structural call result")
    };
    let OperationKind::CallStructuralWithScalarArguments {
        callee,
        structural_arguments,
        claim_transfers,
        ..
    } = &call.kind
    else {
        unreachable!()
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].place, input);
    assert!(structural_arguments[0].path.is_empty());
    assert_eq!(
        structural_arguments[0].access,
        terminal_psi::StructuralAccess::Owned
    );
    assert!(claim_transfers.is_empty());
    assert!(produced.claims.is_empty());
    assert_ne!(produced.place, input);
    assert_eq!(
        caller
            .structural_places
            .iter()
            .find(|place| place.id == produced.place)
            .unwrap()
            .kind,
        StructuralPlaceKind::OperationResult {
            producer: call.id,
            structural_type: produced.structural_type
        }
    );
    let Terminator::ReturnUnit {
        edge,
        trivial_affine_discards,
    } = &caller.blocks[0].terminator
    else {
        panic!("whole-result discard")
    };
    assert_eq!(trivial_affine_discards, &[produced.place]);
    let cleanup = vec![TerminalAffineCleanupAction::DiscardRoot(produced.place)];
    let callee_machine = module
        .machines
        .iter()
        .find(|machine| machine.id == *callee)
        .unwrap();
    let TerminalMachineResult::Structural(callee_result) = &callee_machine.result else {
        panic!("identity result")
    };
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default()).unwrap();
    let abstract_caller = plan
        .functions
        .iter()
        .find(|function| function.machine == module.entry)
        .unwrap();
    assert!(
        matches!(abstract_caller.operations.last(), Some(AbstractOperation::ReturnUnit { psi_edge, cleanup_actions })
        if psi_edge == edge && cleanup_actions == &cleanup)
    );
    assert!(
        abstract_caller
            .operations
            .iter()
            .any(|operation| matches!(operation,
        AbstractOperation::CallStructural { psi_operation, result, callee: actual, .. }
        if *psi_operation == call.id && result == produced && actual == callee))
    );
    for case in target_cases() {
        if wide && case.policy == CallingPolicy::MicrosoftX64 {
            assert!(lower_to_target_operations(&plan, case.target).is_err());
            continue;
        }
        let target = lower_to_target_operations(&plan, case.target)
            .unwrap_or_else(|error| panic!("{:?}: {error:?}", case.target));
        let caller_index = target
            .functions
            .iter()
            .position(|function| function.machine == module.entry)
            .unwrap();
        let TargetOperation::UnitBody(body) = &target.functions[caller_index].operation else {
            panic!("existing Unit body")
        };
        let producer_index = body
            .operations
            .iter()
            .position(|operation| {
                matches!(operation, TargetUnitOperation::StructuralResultCall { .. })
            })
            .unwrap();
        let TargetUnitOperation::StructuralResultCall {
            psi_operation,
            result,
            callee: target_callee,
            callee_result: target_result,
            call_plan,
            arguments,
            scalar_arguments,
            claim_transfers,
            returned_claim_transfers,
            ..
        } = &body.operations[producer_index]
        else {
            unreachable!()
        };
        assert_eq!(*psi_operation, call.id);
        assert_eq!(result, produced);
        assert_eq!(target_callee, callee);
        assert_eq!(target_result, callee_result);
        assert_eq!(scalar_arguments.len(), usize::from(scalar_prefix));
        assert!(claim_transfers.is_empty() && returned_claim_transfers.is_empty());
        assert_eq!(arguments.len(), 1);
        assert_eq!(arguments[0].place, input);
        assert_eq!(
            arguments[0].shape,
            ValueShape::integer(if wide { 16 } else { 8 }, 8)
        );
        assert!(arguments[0].path.is_empty());
        assert_eq!(arguments[0].source_byte_offset, 0);
        let result_placement = call_plan.result.as_ref().unwrap();
        assert_eq!(result_placement.locations.len(), if wide { 2 } else { 1 });
        assert!(
            matches!(body.operations.last(), Some(TargetUnitOperation::Return { psi_edge, cleanup_actions })
            if psi_edge == edge && cleanup_actions == &cleanup)
        );
        for mutation in 0..5 {
            let mut changed = target.clone();
            let TargetOperation::UnitBody(body) = &mut changed.functions[caller_index].operation
            else {
                unreachable!()
            };
            if mutation < 3 {
                let TargetUnitOperation::Return {
                    cleanup_actions, ..
                } = body.operations.last_mut().unwrap()
                else {
                    unreachable!()
                };
                corrupt_cleanup(cleanup_actions, input, mutation);
            } else {
                let TargetUnitOperation::StructuralResultCall {
                    result, call_plan, ..
                } = &mut body.operations[producer_index]
                else {
                    unreachable!()
                };
                if mutation == 3 {
                    result.place = input;
                } else {
                    call_plan.result.as_mut().unwrap().locations.clear();
                }
                if mutation == 3 {
                    let TargetUnitOperation::Return {
                        cleanup_actions, ..
                    } = body.operations.last_mut().unwrap()
                    else {
                        unreachable!()
                    };
                    cleanup_actions[0] = TerminalAffineCleanupAction::DiscardRoot(input);
                }
            }
            let rejected = match assign_registers(&changed) {
                Err(_) => true,
                Ok(assigned) if mutation < 3 => {
                    // Assignment carries the ordered cleanup row; emission
                    // independently replays its whole-body ownership frontier.
                    emit_machine_code(&assigned).is_err()
                }
                Ok(_) => false,
            };
            assert!(rejected, "target mutation {mutation}");
        }
        let assigned = assign_registers(&target).unwrap();
        let AssignedOperation::UnitBody(body) = &assigned.functions[caller_index].operation else {
            panic!("assigned Unit body")
        };
        let AssignedUnitOperation::StructuralResultCall { result, copies, .. } =
            &body.operations[producer_index]
        else {
            panic!("assigned producer")
        };
        assert_eq!(result, produced);
        assert_eq!(copies[0].place, input);
        assert_eq!(copies[0].source, arguments[0].source);
        assert_eq!(copies[0].destination, arguments[0].destination);
        assert!(
            matches!(body.operations.last(), Some(AssignedUnitOperation::Return { psi_edge, cleanup_actions })
            if psi_edge == edge && cleanup_actions == &cleanup)
        );
        for mutation in 0..4 {
            let mut changed = assigned.clone();
            let AssignedOperation::UnitBody(body) = &mut changed.functions[caller_index].operation
            else {
                unreachable!()
            };
            if mutation < 3 {
                let AssignedUnitOperation::Return {
                    cleanup_actions, ..
                } = body.operations.last_mut().unwrap()
                else {
                    unreachable!()
                };
                corrupt_cleanup(cleanup_actions, input, mutation);
            } else {
                let AssignedUnitOperation::StructuralResultCall { result, .. } =
                    &mut body.operations[producer_index]
                else {
                    unreachable!()
                };
                result.place = input;
                let AssignedUnitOperation::Return {
                    cleanup_actions, ..
                } = body.operations.last_mut().unwrap()
                else {
                    unreachable!()
                };
                cleanup_actions[0] = TerminalAffineCleanupAction::DiscardRoot(input);
            }
            assert!(
                emit_machine_code(&changed).is_err(),
                "assigned mutation {mutation}"
            );
        }
        let emitted = emit_machine_code(&assigned).unwrap();
        let emitted_index = emitted
            .functions
            .iter()
            .position(|function| function.machine == module.entry)
            .unwrap();
        let function = &emitted.functions[emitted_index];
        let native_call = &function.internal_unit_calls[0];
        assert_eq!(native_call.target, *callee);
        assert_eq!(native_call.operation_ordinal, producer_index);
        let native_result = native_call.structural_result.as_ref().unwrap();
        assert_eq!(native_result.operation_result, *produced);
        assert_eq!(native_result.function_result, *callee_result);
        assert!(
            native_result.returned_claim_transfers.is_empty()
                && native_result.returned_claims.is_empty()
        );
        assert_eq!(native_result.callee_result_placement, *result_placement);
        let native_cleanup = function.unit_affine_cleanup.as_ref().unwrap();
        assert_eq!(native_cleanup.psi_edge, *edge);
        assert_eq!(native_cleanup.actions, cleanup);
        // The return range includes frame/link restoration and the return
        // instruction, not a runtime destructor for the affine result.
        let return_site = function
            .semantic_code_attribution
            .iter()
            .find(|site| site.site == SemanticCodeSite::Edge(*edge))
            .unwrap();
        assert_eq!(native_cleanup.code_offset, return_site.code_offset);
        assert_eq!(native_cleanup.byte_count, return_site.byte_count);
        assert!(function.provenance.operations.contains(&call.id));
        assert!(function.provenance.edges.contains(edge));
        for mutation in 0..4 {
            let mut changed = emitted.clone();
            let function = &mut changed.functions[emitted_index];
            if mutation < 3 {
                corrupt_cleanup(
                    &mut function.unit_affine_cleanup.as_mut().unwrap().actions,
                    input,
                    mutation,
                );
            } else {
                function.internal_unit_calls[0]
                    .structural_result
                    .as_mut()
                    .unwrap()
                    .operation_result
                    .place = input;
                function.unit_affine_cleanup.as_mut().unwrap().actions[0] =
                    TerminalAffineCleanupAction::DiscardRoot(input);
            }
            assert!(
                build_object_artifact(&changed).is_err(),
                "object replay mutation {mutation}"
            );
        }
        let object = build_object_artifact(&emitted).unwrap();
        assert_eq!(
            object.entry_function().unit_affine_cleanup.as_ref(),
            Some(native_cleanup)
        );
        let container = emit_object_container(&object);
        assert_eq!(container.psi, plan.psi);
        let image = emit_executable_image(&object, 3).unwrap();
        image_emission::validate_executable_image(&object, &image).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        validate_installation_record(&installation, &image).unwrap();
        let installed_call = installation
            .internal_unit_calls()
            .iter()
            .find(|call| call.machine == module.entry)
            .unwrap();
        assert_eq!(
            installed_call.custody.structural_result.as_ref(),
            Some(native_result)
        );
        let mut changed = installation.clone();
        changed
            .internal_unit_calls_mut_for_test()
            .iter_mut()
            .find(|call| call.machine == module.entry)
            .unwrap()
            .custody
            .structural_result
            .as_mut()
            .unwrap()
            .operation_result
            .place = input;
        assert!(validate_installation_record(&changed, &image).is_err());
        if let Ok(encoded) = encode_installation_record(&changed)
            && let Ok(decoded) = decode_installation_record(&encoded)
        {
            assert!(validate_installation_record(&decoded, &image).is_err());
        }
        assert_eq!(
            decode_installation_record(&encode_installation_record(&installation).unwrap()),
            Ok(installation)
        );
        #[cfg(unix)]
        if case.target == NativeTarget::host() {
            host::execute(&image, object.entry_function().text_offset, wide);
        }
    }
}

fn corrupt_cleanup(actions: &mut Vec<TerminalAffineCleanupAction>, input: PlaceId, mutation: u32) {
    match mutation {
        0 => actions.clear(),
        1 => actions[0] = TerminalAffineCleanupAction::DiscardRoot(input),
        2 => actions.push(actions[0].clone()),
        _ => unreachable!(),
    }
}
