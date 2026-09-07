//! Partial-result cleanup stays on its native continuation before later calls.

use super::*;
use abstract_operations_to_target_operations::validate_abstract_to_target_translation;
use assigned_target_operations::AssignedUnitOperation;
use machine_code::UnitContinuationRecord;
use target_operations::TargetUnitOperation;

#[test]
fn projected_result_cleanup_reaches_native_continuation() {
    check("Pair", ".right", 16, false);
}

#[test]
fn indexed_result_cleanup_reaches_native_continuation() {
    check("[Token; 2]", "[1]", 16, false);
}

#[test]
fn empty_result_complements_retain_native_continuation_boundaries() {
    check("[Token; 1]", "[0]", 8, false);
}

#[test]
fn successive_native_producers_retire_only_their_own_result() {
    check("Pair", ".right", 16, true);
    check("[Token; 1]", "[0]", 8, true);
}

fn check(carrier: &str, projection: &str, byte_size: u16, repeated: bool) {
    let second_parameter = if repeated {
        format!(", second: {carrier}")
    } else {
        String::new()
    };
    let second_consumer = if repeated {
        format!("Sink::take(Root::forward(second){projection}); Sink::done();")
    } else {
        String::new()
    };
    let source = format!(
        "data Token {{ value: u64; }}
        data Pair {{ left: Token; right: Token; }}
        data Root {{}} data Sink {{}}
        machine Root::forward(value: {carrier}) -> {carrier} {{ value }}
        machine Sink::take(value: Token) {{}}
        machine Sink::done() {{}}
        machine Root::enter(value: {carrier}{second_parameter}) {{
            Sink::take(Root::forward(value){projection});
            Sink::done(); {second_consumer}
        }}"
    );
    let tokens = Lexer::new(&source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = lower_typed_trees(typed).unwrap();
    let terminal = lower_machine(&checked, "Root::enter").unwrap();
    let semantic = encode_module(&terminal.semantic_module).unwrap();
    let proof = encode_proof_bundle(&terminal.proof_bundle).unwrap();
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default()).unwrap();
    let entry = terminal.semantic_module.entry;
    let expected = plan
        .functions
        .iter()
        .find(|function| function.machine == entry)
        .unwrap()
        .operations
        .iter()
        .filter_map(|operation| match operation {
            AbstractOperation::Jump {
                psi_edge,
                target,
                residual_affine_discards,
                ..
            } => Some((
                *psi_edge,
                *target,
                residual_affine_discards
                    .iter()
                    .cloned()
                    .map(TerminalAffineCleanupAction::DiscardResidual)
                    .collect::<Vec<_>>(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(expected.len(), 1 + usize::from(repeated));
    for case in target_cases() {
        if byte_size == 16 && case.policy == CallingPolicy::MicrosoftX64 {
            assert!(lower_to_target_operations(&plan, case.target).is_err());
            eprintln!("unsupported 16-byte direct-result ABI: {:?}", case.target);
            continue;
        }
        let target = lower_to_target_operations(&plan, case.target).unwrap_or_else(|error| {
            panic!("native continuation {:?}: {error:?}\n{source}", case.target)
        });
        validate_abstract_to_target_translation(&plan, case.target, &target).unwrap();
        let target_index = target
            .functions
            .iter()
            .position(|function| function.machine == entry)
            .unwrap();
        let TargetOperation::UnitBody(body) = &target.functions[target_index].operation else {
            panic!("Unit caller")
        };
        let continuation_ordinals = body
            .operations
            .iter()
            .enumerate()
            .filter_map(|(ordinal, operation)| {
                matches!(operation, TargetUnitOperation::Continue { .. }).then_some(ordinal)
            })
            .collect::<Vec<_>>();
        assert_eq!(continuation_ordinals.len(), expected.len());
        let mut changed_carrier = target.clone();
        changed_carrier.functions[target_index].operation = target
            .functions
            .iter()
            .find(|function| !matches!(function.operation, TargetOperation::UnitBody(_)))
            .expect("ordinary structural producer has a non-Unit carrier")
            .operation
            .clone();
        assert!(
            validate_abstract_to_target_translation(&plan, case.target, &changed_carrier).is_err()
        );
        for mutation in 0..4 {
            let mut changed = target.clone();
            let TargetOperation::UnitBody(body) = &mut changed.functions[target_index].operation
            else {
                unreachable!()
            };
            let ordinal = continuation_ordinals[0];
            let TargetUnitOperation::Continue {
                psi_edge,
                source_block,
                target_block,
                cleanup_actions,
                ..
            } = &mut body.operations[ordinal]
            else {
                unreachable!()
            };
            match mutation {
                0 => *target_block = *source_block,
                1 => *psi_edge = semantic_vocabulary::EdgeId::new(999).unwrap(),
                2 => cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(
                    body.parameters[0].place,
                )),
                3 => {
                    body.operations.remove(ordinal);
                }
                _ => unreachable!(),
            }
            assert!(
                validate_abstract_to_target_translation(&plan, case.target, &changed).is_err(),
                "target mutation {mutation}"
            );
            assert!(
                assign_registers(&changed).is_err(),
                "assignment mutation {mutation}"
            );
        }
        let assigned = assign_registers(&target).unwrap();
        let assigned_index = assigned
            .functions
            .iter()
            .position(|function| function.machine == entry)
            .unwrap();
        for mutation in 0..3 {
            let mut changed = assigned.clone();
            let AssignedOperation::UnitBody(body) =
                &mut changed.functions[assigned_index].operation
            else {
                unreachable!()
            };
            let AssignedUnitOperation::Continue {
                source_block,
                target_block,
                cleanup_actions,
                ..
            } = &mut body.operations[continuation_ordinals[0]]
            else {
                unreachable!()
            };
            match mutation {
                0 => *target_block = *source_block,
                1 => cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(
                    body.parameters[0].place,
                )),
                2 => {
                    body.operations.remove(continuation_ordinals[0]);
                }
                _ => unreachable!(),
            }
            assert!(
                emit_machine_code(&changed).is_err(),
                "assigned mutation {mutation}"
            );
        }
        let emitted = emit_machine_code(&assigned).unwrap();
        let caller_index = emitted
            .functions
            .iter()
            .position(|function| function.machine == entry)
            .unwrap();
        let caller = &emitted.functions[caller_index];
        let disposer = caller.internal_unit_calls[1].target;
        for machine in [entry, disposer] {
            let mut changed = emitted.clone();
            changed
                .functions
                .iter_mut()
                .find(|function| function.machine == machine)
                .unwrap()
                .scalar_abi = Some(canonical_scalar_abi(case.target));
            assert!(
                build_object_artifact(&changed).is_err(),
                "Unit machine cannot acquire scalar ABI: {machine:?}"
            );
        }
        assert_eq!(caller.unit_continuations.len(), expected.len());
        assert!(
            caller
                .unit_affine_cleanup
                .as_ref()
                .unwrap()
                .actions
                .is_empty()
        );
        for (record, (edge, target_block, actions)) in
            caller.unit_continuations.iter().zip(&expected)
        {
            assert_eq!(record.cleanup.psi_edge, *edge);
            assert_eq!(record.target_block, *target_block);
            assert_eq!(record.cleanup.actions, *actions);
            assert_eq!(record.cleanup.byte_count, 0);
            assert!(record.cleanup.locals.is_empty());
            assert_eq!(
                record.successor_operation_ordinal,
                record.operation_ordinal + 1
            );
            let attribution = caller
                .semantic_code_attribution
                .iter()
                .find(|site| site.site == SemanticCodeSite::Edge(*edge))
                .unwrap();
            assert_eq!(attribution.operation_ordinal, record.operation_ordinal);
            assert_eq!(attribution.code_offset, record.cleanup.code_offset);
            assert_eq!(attribution.byte_count, 0);
            let successor = caller
                .internal_unit_calls
                .iter()
                .find(|call| call.operation_ordinal == record.successor_operation_ordinal)
                .unwrap();
            assert_eq!(record.cleanup.code_offset, successor.code_offset);
        }
        let mutation_count = if byte_size == 16 { 11 } else { 8 };
        for mutation in 0..mutation_count {
            let mut changed = emitted.clone();
            let caller = &mut changed.functions[caller_index];
            corrupt_continuations(
                &mut caller.unit_continuations,
                mutation,
                caller.unit_affine_cleanup.as_ref().unwrap(),
            );
            assert!(
                build_object_artifact(&changed).is_err(),
                "emitted mutation {mutation} {:?}",
                case.target
            );
        }
        let object = build_object_artifact(&emitted).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        validate_installation_record(&installation, &image).unwrap();
        let decoded =
            decode_installation_record(&encode_installation_record(&installation).unwrap())
                .unwrap();
        validate_installation_record(&decoded, &image).unwrap();
        let installed_index = installation
            .functions()
            .iter()
            .position(|function| function.machine == entry)
            .unwrap();
        for machine in [entry, disposer] {
            let mut changed = installation.clone();
            changed
                .functions_mut_for_test()
                .iter_mut()
                .find(|function| function.machine == machine)
                .unwrap()
                .scalar_abi = Some(canonical_scalar_abi(case.target));
            assert!(validate_installation_record(&changed, &image).is_err());
            assert!(
                encode_installation_record(&changed).is_err(),
                "canonical Unit metadata rejects scalar ABI: {machine:?}"
            );
        }
        for mutation in 0..mutation_count {
            let mut changed = installation.clone();
            let caller = &mut changed.functions_mut_for_test()[installed_index];
            corrupt_continuations(
                &mut caller.unit_continuations,
                mutation,
                caller.unit_affine_cleanup.as_ref().unwrap(),
            );
            assert!(
                validate_installation_record(&changed, &image).is_err(),
                "installed mutation {mutation}"
            );
            assert!(
                encode_installation_record(&changed).is_err(),
                "canonical installed mutation {mutation}"
            );
        }
        #[cfg(unix)]
        if case.target == NativeTarget::host() {
            super::affine_call_result_host::execute_parameters(
                &image,
                object.entry_function().text_offset,
                byte_size == 16,
                repeated,
            );
        }
    }
}

fn canonical_scalar_abi(target: NativeTarget) -> target_operations::ScalarFunctionAbi {
    let call_plan = calling_conventions::evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &calling_conventions::CallSignature {
            parameters: Vec::new(),
            result: Some(ValueShape::integer(8, 8)),
        },
    )
    .unwrap();
    target_operations::ScalarFunctionAbi {
        parameters: Vec::new(),
        result: target_operations::ScalarAbiValue {
            value: semantic_vocabulary::ValueId::new(999).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            placement: call_plan.result.clone().unwrap(),
        },
        call_plan,
    }
}

fn corrupt_continuations(
    continuations: &mut Vec<UnitContinuationRecord>,
    mutation: usize,
    final_cleanup: &machine_code::UnitAffineCleanupRecord,
) {
    match mutation {
        0 => continuations.clear(),
        1 => continuations.insert(0, continuations[0].clone()),
        2 => continuations[0].cleanup.psi_edge = final_cleanup.psi_edge,
        3 => continuations[0].cleanup.code_offset = final_cleanup.code_offset,
        4 => continuations[0].cleanup.byte_count = 1,
        5 => continuations[0].successor_operation_ordinal += 1,
        6 => continuations[0].target_block = continuations[0].source_block,
        7 => continuations[0]
            .cleanup
            .actions
            .push(TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(999).unwrap(),
            )),
        8 => continuations[0].cleanup.actions.clear(),
        9 | 10 => {
            let TerminalAffineCleanupAction::DiscardResidual(residual) =
                &mut continuations[0].cleanup.actions[0]
            else {
                unreachable!()
            };
            if mutation == 9 {
                residual.path.clear();
            } else {
                residual.structural_type = semantic_vocabulary::StructuralTypeId::new(999).unwrap();
            }
        }
        _ => unreachable!(),
    }
}
