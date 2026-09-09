//! Independent replay rejects changed write operands, bounds, effects, and bytes.
use super::*;
use legalized_operations::LegalizedScalarInstructionKind;
use selected_instructions::{SelectedInstructionKind, SelectedMemoryAccessRole};
use target_operations_to_selected_instructions::{
    legalize_target_operations, selection_constraints, stage_optimized_instruction_selection,
    validate_legalized_operations, validate_selected_instructions,
};

#[test]
fn acyclic_mutable_write_cannot_substitute_another_available_byte() {
    use abstract_operations_to_abstract_operations::validation::{
        validate_transformed_psi_optimization_unit, validate_verified_psi_optimization_unit,
    };
    let source = FILL
        .replacen("byte: u8)", "byte: u8, other: u8)", 1)
        .replace("scan(out, position + 1, byte)", "done()");
    let lowered = lower_writer(&source, "fill");
    let input = terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
        &terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    let verified = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap();
    validate_verified_psi_optimization_unit(&verified).unwrap();
    let (input, mut changed) = verified.into_parts();
    let function = &mut changed.functions[0];
    let other = function.parameters[1].value;
    let node = function
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
        .find(|node| {
            matches!(
                node.operation,
                abstract_operations::AbstractOperation::ByteSequenceWrite { .. }
            )
        })
        .unwrap();
    let abstract_operations::AbstractOperation::ByteSequenceWrite { value, .. } =
        &mut node.operation
    else {
        unreachable!()
    };
    assert_ne!(*value, other);
    for usage in &mut node.uses {
        if usage.value == *value {
            usage.value = other;
        }
    }
    *value = other;
    changed.identity = optimization_unit::recompute_psi_optimization_unit_identity(&changed);
    let result = validate_transformed_psi_optimization_unit(&input, &changed);
    assert!(matches!(
        result,
        Err(optimization_unit_semantics::OptimizationUnitValidationError::OperationObligationOwnerMismatch { .. })
    ), "substituted byte: {result:?}");
}

#[test]
fn mutable_write_legalization_rejects_substituted_custody() {
    let lowered = writer(false);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let compiled = byte_view_target(&lowered.semantic_module, &lowered.proof_bundle, target);
        let native = compiled.target_operations();
        let abstracted = compiled.optimized().plan();
        let unit = compiled.optimized();
        let admitted = legalize_target_operations(native, abstracted, unit).unwrap();
        assert!(
            legalize_target_operations(native, abstracted, unit.unit()).is_err(),
            "a detached cyclic unit cannot replace verified source custody"
        );
        for mutation in 0..6 {
            let mut proposed = native.clone();
            let target_operations::TargetOperation::ControlGraph(graph) =
                &mut proposed.functions[0].operation
            else {
                panic!("fill control graph")
            };
            let operation = graph
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find(|operation| {
                    matches!(
                        operation,
                        target_operations::TargetUnitOperation::ByteSequenceWrite { .. }
                    )
                })
                .unwrap();
            let target_operations::TargetUnitOperation::ByteSequenceWrite {
                psi_operation,
                destination,
                index,
                value,
                length,
                obligation,
                ..
            } = operation
            else {
                unreachable!()
            };
            match mutation {
                0 => destination.access = terminal_psi::StructuralAccess::SharedBorrow,
                1 => destination.place = PlaceId::new(9999).unwrap(),
                2 => std::mem::swap(index, value),
                3 => *length = ValueId::new(9999).unwrap(),
                4 => *obligation = ObligationId::new(9999).unwrap(),
                5 => *psi_operation = OperationId::new(9999).unwrap(),
                _ => unreachable!(),
            }
            assert!(
                legalize_target_operations(&proposed, abstracted, unit).is_err(),
                "target mutation {mutation} on {target:?}"
            );
        }
        for mutation in 0..8 {
            let mut proposed = admitted.plan().clone();
            let row = proposed
                .scalar_functions
                .iter_mut()
                .flat_map(|function| &mut function.blocks)
                .flat_map(|block| &mut block.instructions)
                .find(|row| {
                    matches!(
                        row.kind,
                        LegalizedScalarInstructionKind::ByteSequenceWrite { .. }
                    )
                })
                .unwrap();
            let LegalizedScalarInstructionKind::ByteSequenceWrite {
                destination,
                index,
                value,
                length,
                obligation,
                accepted_fact,
            } = &mut row.kind
            else {
                unreachable!()
            };
            match mutation {
                0 => *destination = PlaceId::new(9999).unwrap(),
                1 => *index = *length,
                2 => *value = *index,
                3 => *length = *index,
                4 => *obligation = ObligationId::new(9999).unwrap(),
                5 => {
                    *accepted_fact =
                        optimization_core::AcceptedObligationFactIdentity::from_bytes([0; 32])
                }
                6 => row.fuel.clear(),
                7 => row.operation = OperationId::new(9999).unwrap(),
                _ => unreachable!(),
            }
            assert!(
                validate_legalized_operations(native, abstracted, unit, proposed).is_err(),
                "mutation {mutation} on {target:?}"
            );
        }
    }
}

#[test]
fn mutable_write_selection_rejects_changed_address_width_value_and_proof() {
    let lowered = writer(false);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let compiled = byte_view_target(&lowered.semantic_module, &lowered.proof_bundle, target);
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
        for mutation in 0..7 {
            let mut proposed = staged.selected().plan().clone();
            let function = proposed
                .functions
                .iter_mut()
                .find(|function| function.machine == lowered.semantic_module.entry)
                .unwrap();
            let memory = function
                .memory_accesses
                .iter_mut()
                .find(|access| {
                    matches!(
                        access.role,
                        SelectedMemoryAccessRole::WriteByteSequence { .. }
                    )
                })
                .unwrap();
            let instruction = function
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find(|instruction| instruction.id == memory.instruction)
                .unwrap();
            match mutation {
                0 => {
                    instruction.kind = SelectedInstructionKind::Store {
                        byte_offset: 0,
                        byte_size: 8,
                    }
                }
                1 => {
                    instruction.kind = SelectedInstructionKind::Store {
                        byte_offset: 1,
                        byte_size: 1,
                    }
                }
                2 => instruction.operands.swap(0, 1),
                3 => instruction.provenance.fuel.clear(),
                4 => memory.place = PlaceId::new(9999).unwrap(),
                5 => memory.byte_count = 8,
                6 => {
                    let SelectedMemoryAccessRole::WriteByteSequence { accepted_fact, .. } =
                        &mut memory.role
                    else {
                        unreachable!()
                    };
                    *accepted_fact =
                        optimization_core::AcceptedObligationFactIdentity::from_bytes([0; 32]);
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
