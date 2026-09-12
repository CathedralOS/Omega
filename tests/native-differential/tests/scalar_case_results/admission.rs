//! Fresh aggregate custody cannot be replaced by scalar/Unit records or detached bytes.
use super::*;

#[test]
fn indirect_aggregate_returns_remain_an_explicit_realization_limit() {
    let artifact = produce("choose");
    let selections = OptimizationSelections::new([]).unwrap();
    let optimized = optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .unwrap();
    let target = NativeTarget::windows_x64();
    let compiled = abstract_operations_to_target_operations::lower_optimized_to_target_operations(
        optimized, target,
    );
    if let Ok(compiled) = compiled {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        assert!(
            target_operations_to_selected_instructions::stage_optimized_instruction_selection(
                compiled,
                environment
            )
            .is_err(),
            "a Windows indirect aggregate cannot silently use the direct register-return implementation"
        );
    }
}

pub(super) fn installation_cannot_change_call_or_result(
    record: &image_emission::InstallationRecord,
    image: &image_emission::ExecutableImage,
) {
    let reject = |changed: &image_emission::InstallationRecord| {
        assert!(image_emission::validate_installation_record(changed, image).is_err());
    };
    let result_function = record
        .functions()
        .iter()
        .position(|function| {
            function
                .parameter_abi
                .as_ref()
                .is_some_and(|abi| abi.call_plan.result.is_some())
                || function.scalar_abi.is_some()
                || function.mixed_structural_scalar_abi.is_some()
        })
        .expect("aggregate result retains its complete call plan");
    for mutation in 0..5 {
        let mut changed = record.clone();
        let function = &mut changed.functions_mut_for_test()[result_function];
        if function.parameter_abi.is_none() {
            if mutation == 0 {
                function.scalar_abi = None;
                function.mixed_structural_scalar_abi = None;
            } else {
                let plan = if let Some(abi) = &mut function.scalar_abi {
                    &mut abi.call_plan
                } else {
                    &mut function
                        .mixed_structural_scalar_abi
                        .as_mut()
                        .unwrap()
                        .call_plan
                };
                match mutation {
                    1 => plan.result = None,
                    2 => plan.result.as_mut().unwrap().shape.byte_size = 8,
                    3 => plan.result.as_mut().unwrap().locations.clear(),
                    4 => {
                        // Zero-argument source functions must undergo a real
                        // substitution too; clearing their roster changes nothing.
                        if plan.parameters.is_empty() {
                            plan.parameters.push(plan.result.clone().unwrap());
                        } else {
                            plan.parameters.clear();
                        }
                    }
                    _ => unreachable!(),
                }
            }
            reject(&changed);
            continue;
        }
        match mutation {
            0 => function.parameter_abi = None,
            1 => function.parameter_abi.as_mut().unwrap().call_plan.result = None,
            2 => {
                function
                    .parameter_abi
                    .as_mut()
                    .unwrap()
                    .call_plan
                    .result
                    .as_mut()
                    .unwrap()
                    .shape
                    .byte_size = 8
            }
            3 => {
                let locations = &mut function
                    .parameter_abi
                    .as_mut()
                    .unwrap()
                    .call_plan
                    .result
                    .as_mut()
                    .unwrap()
                    .locations;
                if locations.len() > 1 {
                    locations.swap(0, 1);
                } else {
                    // A tag-only sum has one fragment; removing it still tests
                    // the exact result-location roster without inventing a second.
                    assert_eq!(locations.len(), 1);
                    locations.clear();
                }
            }
            4 => function.parameter_abi.as_mut().unwrap().parameters.clear(),
            _ => unreachable!(),
        }
        reject(&changed);
    }
    for (function_index, function) in record.functions().iter().enumerate() {
        if function.unit_call_stacks.is_empty() {
            continue;
        }
        for mutation in 0..6 {
            let mut changed = record.clone();
            let function = &mut changed.functions_mut_for_test()[function_index];
            match mutation {
                0 => function.unit_call_stacks.clear(),
                1 => function.unit_call_stacks[0].target = function.machine,
                2 => {
                    function.unit_call_stacks[0].owner = target_operations::CallSiteOwner::Operation(
                        semantic_vocabulary::OperationId::new(9999).unwrap(),
                    )
                }
                3 => function.unit_call_stacks[0].text_offset += 1,
                4 => function.unit_call_stacks[0].caller_live_bytes += 16,
                5 => function.unit_call_stacks.push(function.unit_call_stacks[0]),
                _ => unreachable!(),
            }
            reject(&changed);
        }
    }
    let mut old_format = image_emission::encode_installation_record(record).unwrap();
    old_format[8..10]
        .copy_from_slice(&(image_emission::INSTALLATION_FORMAT_MARKER - 1).to_le_bytes());
    assert!(image_emission::decode_installation_record(&old_format).is_err());
}

#[test]
fn aggregate_selection_rejects_substituted_result_and_dispatch() {
    use selected_instructions::{SelectedBlockOrigin, SelectedInstructionKind, SelectedTerminator};
    use target_operations_to_selected_instructions::{
        selection_constraints, stage_optimized_instruction_selection,
        validate_selected_instructions,
    };
    let artifact = produce_source("collect", include_str!("borrowed.omg"));
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let selections = OptimizationSelections::new([]).unwrap();
        let optimized = optimize_artifact_sections(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &AdmissionProfile::default(),
            compiler_baseline_request_v1(&selections),
        )
        .unwrap();
        let compiled =
            abstract_operations_to_target_operations::lower_optimized_to_target_operations(
                optimized, target,
            )
            .unwrap();
        let legalized = target_operations_to_selected_instructions::legalize_target_operations(
            compiled.target_operations(),
            compiled.optimized().plan(),
            compiled.optimized(),
        )
        .unwrap();
        for mutation in 0..5 {
            let mut proposed = legalized.plan().clone();
            let constructor = proposed.scalar_functions.iter_mut().flat_map(|function| &mut function.blocks)
                .flat_map(|block| &mut block.instructions).find(|row| matches!(&row.kind,
                    legalized_operations::LegalizedScalarInstructionKind::EstablishScalarCase { fields, .. } if !fields.is_empty())).unwrap();
            let legalized_operations::LegalizedScalarInstructionKind::EstablishScalarCase {
                result,
                fields,
                result_case,
                layout,
            } = &mut constructor.kind
            else {
                unreachable!()
            };
            match mutation {
                0 => fields[0].value = semantic_vocabulary::ValueId::new(9999).unwrap(),
                1 => fields[0].field = semantic_vocabulary::StructuralFieldId::new(9999).unwrap(),
                2 => *result_case = semantic_vocabulary::StructuralCaseId::new(9999).unwrap(),
                3 => layout.shape.byte_size = 8,
                4 => result.place = semantic_vocabulary::PlaceId::new(9999).unwrap(),
                _ => unreachable!(),
            }
            assert!(
                target_operations_to_selected_instructions::validate_legalized_operations(
                    compiled.target_operations(),
                    compiled.optimized().plan(),
                    compiled.optimized(),
                    proposed,
                )
                .is_err(),
                "constructor mutation {mutation} on {target:?}"
            );
        }
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let staged = stage_optimized_instruction_selection(compiled, environment).unwrap();
        let environment = staged.register_environment();
        let constraints = selection_constraints(staged.legalized(), environment);
        let validate = |raw| {
            validate_selected_instructions(
                staged.legalized(),
                &constraints,
                environment.physical(),
                environment.constraints(),
                raw,
            )
        };
        validate(staged.selected().plan().clone()).unwrap();
        for function in &staged.legalized().plan().scalar_functions {
            let selected = staged
                .selected()
                .plan()
                .functions
                .iter()
                .find(|row| row.machine == function.machine)
                .unwrap();
            for constructor in function.blocks.iter().flat_map(|block| &block.instructions).filter(|row|
                matches!(row.kind, legalized_operations::LegalizedScalarInstructionKind::EstablishScalarCase { .. })) {
                assert!(!constructor.fuel.is_empty());
                let charged: Vec<_> = selected.blocks.iter().flat_map(|block| &block.instructions)
                    .filter(|instruction| instruction.provenance.operations.contains(&constructor.operation))
                    .flat_map(|instruction| &instruction.provenance.fuel).cloned().collect();
                assert_eq!(charged, constructor.fuel, "one source constructor charge, independent of carrier stores");
            }
        }
        for mutation in 0..8 {
            let mut proposed = staged.selected().plan().clone();
            match mutation {
                0..=2 => {
                    let call = proposed
                        .functions
                        .iter_mut()
                        .flat_map(|function| &mut function.calls)
                        .find(|row| row.call.structural_result.is_some())
                        .unwrap();
                    match mutation {
                        0 => call.call.structural_result = None,
                        1 => {
                            call.call.structural_result.as_mut().unwrap().place =
                                semantic_vocabulary::PlaceId::new(9999).unwrap()
                        }
                        _ => call
                            .call
                            .result_placement
                            .as_mut()
                            .unwrap()
                            .locations
                            .swap(0, 1),
                    }
                }
                3 | 4 => {
                    let instruction = proposed
                        .functions
                        .iter_mut()
                        .flat_map(|function| &mut function.blocks)
                        .find_map(|block| match &mut block.terminator {
                            SelectedTerminator::Return { instruction, .. }
                                if matches!(
                                    instruction.kind,
                                    SelectedInstructionKind::ReturnAggregate { .. }
                                ) =>
                            {
                                Some(instruction)
                            }
                            _ => None,
                        })
                        .unwrap();
                    if mutation == 3 {
                        instruction.kind =
                            SelectedInstructionKind::ReturnAggregate { fragment_count: 1 };
                    } else {
                        instruction.operands.swap(0, 1);
                    }
                }
                5 => {
                    let block = proposed
                        .functions
                        .iter_mut()
                        .flat_map(|function| &mut function.blocks)
                        .find(|block| {
                            matches!(block.origin, SelectedBlockOrigin::CaseDispatch { .. })
                        })
                        .unwrap();
                    let SelectedBlockOrigin::CaseDispatch { case_ordinal, .. } = &mut block.origin
                    else {
                        unreachable!()
                    };
                    *case_ordinal += 1;
                }
                6 => {
                    let instruction = proposed
                        .functions
                        .iter_mut()
                        .flat_map(|function| &mut function.blocks)
                        .flat_map(|block| &mut block.instructions)
                        .find(|instruction| {
                            matches!(
                                instruction.kind,
                                SelectedInstructionKind::CallAggregate { .. }
                            )
                        })
                        .unwrap();
                    instruction.kind = SelectedInstructionKind::CallUnit {
                        callee: semantic_vocabulary::MachineId::new(9999).unwrap(),
                    };
                }
                7 => {
                    let home = proposed
                        .functions
                        .iter_mut()
                        .flat_map(|function| &mut function.local_storage_slots)
                        .next()
                        .unwrap();
                    home.byte_size += 8;
                }
                _ => unreachable!(),
            }
            assert!(
                validate(proposed).is_err(),
                "mutation {mutation} on {target:?}"
            );
        }
    }
}
