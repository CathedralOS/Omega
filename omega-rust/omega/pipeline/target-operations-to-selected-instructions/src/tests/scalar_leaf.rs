//! Scalar leaf production and independent corruption controls.

use crate::{
    legalize_target_operations, select_instructions, selection_constraints,
    validate_legalized_operations, validate_selected_instructions,
};
use abstract_operations::{
    AbstractFunctionResult, AbstractOperation, AbstractOperationPlan, AbstractParameter,
    AbstractResult,
};
use legalized_operations::{LegalizedFunction, LegalizedLeafValue};
use semantic_vocabulary::{
    EdgeId, IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
};
use target_operations::{TargetOperation, TargetOperationPlan};

fn fixture(
    immediate: Option<u64>,
    native_target: target::NativeTarget,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut abstracted, _, previous_unit) = super::fixtures::plain_unit::plain_unit_fixture();
    let function = &mut abstracted.functions[0];
    let source_value = ValueId::new(2).unwrap();
    let result = ValueId::new(3).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: result,
        scalar_type,
    });
    function.operations.clear();
    match immediate {
        Some(value) => function
            .operations
            .push(AbstractOperation::IntegerConstant {
                psi_operation: OperationId::new(1).unwrap(),
                result: source_value,
                scalar_type,
                value: IntegerValue::Unsigned(u128::from(value)),
            }),
        None => function.parameters.push(AbstractParameter {
            value: source_value,
            scalar_type,
        }),
    }
    function.operations.push(AbstractOperation::Return {
        psi_edge: EdgeId::new(1).unwrap(),
        result,
        value: source_value,
        scalar_type,
        cleanup_actions: Vec::new(),
    });
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &abstracted,
        native_target,
    )
    .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &abstracted,
        previous_unit.fuel_schedule,
    )
    .unwrap();
    (abstracted, target, unit)
}

#[test]
fn publication_classification_reuses_the_existing_scalar_input() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let (abstracted, targeted, unit) = fixture(Some(17), target);
        assert!(crate::legalization::accepts_fragment_publication_input(
            &targeted,
            &abstracted,
            &unit
        ));
        let mut changed = targeted.clone();
        changed.functions[0].fixed_integer_scalar_abi = None;
        assert!(!crate::legalization::accepts_fragment_publication_input(
            &changed,
            &abstracted,
            &unit
        ));
        let mut changed = abstracted.clone();
        let extra_block = changed.functions[0].block_entries[0].clone();
        changed.functions[0].block_entries.push(extra_block);
        assert!(!crate::legalization::accepts_fragment_publication_input(
            &targeted, &changed, &unit
        ));
    }
}

#[test]
fn scalar_leaf_constants_and_parameters_select_without_fabricated_control() {
    for native_target in [
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        for immediate in [Some(0), Some(7), Some(u64::MAX), None] {
            let (abstracted, target, unit) = fixture(immediate, native_target);
            let legalized = legalize_target_operations(&target, &abstracted, &unit).unwrap();
            let LegalizedFunction::Leaf(leaf) = &legalized.plan().functions[0] else {
                panic!("single scalar leaf")
            };
            assert_eq!(
                leaf.abi,
                target.functions[0]
                    .fixed_integer_scalar_abi
                    .clone()
                    .unwrap()
            );
            let environment =
                register_environment::baseline_target_register_environment(native_target).unwrap();
            let constraints = selection_constraints(&legalized, &environment);
            let selected = select_instructions(
                &legalized,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            assert_eq!(selected.plan().functions[0].blocks.len(), 1);
            assert_eq!(selected.plan().functions[0].blocks[0].instructions.len(), 1);
            assert_eq!(
                selected.plan().functions[0].virtual_registers.len(),
                1 + usize::from(immediate.is_none())
            );
            if immediate.is_none() {
                assert_eq!(
                    selected.plan().functions[0].blocks[0].instructions[0].kind,
                    selected_instructions::SelectedInstructionKind::CopyI64
                );
                assert!(
                    selected.plan().functions[0].virtual_registers[1]
                        .entry_fixed_view
                        .is_none()
                );
            }
            assert_eq!(
                selected.plan().functions[0].provenance,
                target.functions[0].provenance
            );
            assert_eq!(
                selected.plan().functions[0].virtual_registers[0]
                    .entry_fixed_view
                    .is_some(),
                immediate.is_none()
            );
        }
    }
}

#[test]
fn scalar_leaf_legalization_rejects_changed_literal_abi_and_return_register() {
    let (abstracted, target, unit) = fixture(Some(7), target::NativeTarget::windows_x64());
    let legalized = legalize_target_operations(&target, &abstracted, &unit).unwrap();
    let original_identity = legalized.receipt().identity();
    assert_ne!(
        legalized.receipt().validator(),
        crate::legalization_validator_identity_v21_legacy()
    );
    assert_ne!(
        original_identity,
        legalized_operations::legalized_operation_plan_identity_v21_legacy(legalized.plan())
    );
    for corruption in 0..4 {
        let mut proposed = legalized.plan().clone();
        let LegalizedFunction::Leaf(leaf) = &mut proposed.functions[0] else {
            unreachable!()
        };
        match corruption {
            0 => {
                let LegalizedLeafValue::Immediate { value, .. } = &mut leaf.leaf.value else {
                    unreachable!()
                };
                *value = IntegerValue::Unsigned(9);
            }
            1 => leaf.abi.result.value = ValueId::new(99).unwrap(),
            2 => {
                let calling_conventions::ValueLocation::Register { register, .. } =
                    &mut leaf.abi.result.placement.locations[0]
                else {
                    unreachable!()
                };
                *register = target_operations::MachineRegister::X86Rcx;
            }
            _ => leaf.provenance.operations.clear(),
        }
        assert_ne!(
            original_identity,
            legalized_operations::legalized_operation_plan_identity(&proposed)
        );
        assert!(validate_legalized_operations(&target, &abstracted, &unit, proposed).is_err());
    }
    let mut corrupted_target = target.clone();
    let TargetOperation::ReturnIntegerImmediate { value, .. } =
        &mut corrupted_target.functions[0].operation
    else {
        unreachable!()
    };
    *value = IntegerValue::Unsigned(8);
    assert!(legalize_target_operations(&corrupted_target, &abstracted, &unit).is_err());
    let mut corrupted_target = target.clone();
    let abi = corrupted_target.functions[0]
        .fixed_integer_scalar_abi
        .as_mut()
        .unwrap();
    let calling_conventions::ValueLocation::Register { register, .. } =
        &mut abi.result.placement.locations[0]
    else {
        unreachable!()
    };
    *register = target_operations::MachineRegister::X86Rcx;
    // Even changing both ABI copies cannot authorize a noncanonical result register.
    abi.call_plan.result = Some(abi.result.placement.clone());
    assert!(legalize_target_operations(&corrupted_target, &abstracted, &unit).is_err());
    assert!(
        validate_legalized_operations(
            &corrupted_target,
            &abstracted,
            &unit,
            legalized.plan().clone()
        )
        .is_err()
    );
}

#[test]
fn scalar_leaf_parameter_replay_binds_index_and_incoming_register() {
    let (abstracted, target, unit) = fixture(None, target::NativeTarget::windows_x64());
    let legalized = legalize_target_operations(&target, &abstracted, &unit).unwrap();
    for change_index in [false, true] {
        let mut proposed = legalized.plan().clone();
        let LegalizedFunction::Leaf(leaf) = &mut proposed.functions[0] else {
            unreachable!()
        };
        let LegalizedLeafValue::EntryParameter {
            parameter_index,
            register,
            ..
        } = &mut leaf.leaf.value
        else {
            unreachable!()
        };
        if change_index {
            *parameter_index = 1;
        } else {
            *register = target_operations::MachineRegister::X86Rdx;
        }
        assert!(validate_legalized_operations(&target, &abstracted, &unit, proposed).is_err());
    }
}

#[test]
fn scalar_leaf_selected_replay_rejects_literal_precolor_and_return_changes() {
    for immediate in [Some(7), None] {
        let (abstracted, target, unit) = fixture(immediate, target::NativeTarget::windows_x64());
        let legalized = legalize_target_operations(&target, &abstracted, &unit).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(target.target).unwrap();
        let constraints = selection_constraints(&legalized, &environment);
        let selected = select_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        if immediate.is_none() {
            for corrupt_copy in [true, false] {
                let mut proposed = selected.plan().clone();
                if corrupt_copy {
                    proposed.functions[0].blocks[0].instructions[0].operands[1].virtual_register =
                        selected_instructions::VirtualRegisterId(0);
                } else {
                    let selected_instructions::SelectedTerminator::Return { instruction, .. } =
                        &mut proposed.functions[0].blocks[0].terminator
                    else {
                        unreachable!()
                    };
                    instruction.operands[0].virtual_register =
                        selected_instructions::VirtualRegisterId(0);
                }
                assert!(
                    validate_selected_instructions(
                        &legalized,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                        proposed
                    )
                    .is_err()
                );
            }
        }
        let mut proposed = selected.plan().clone();
        if immediate.is_some() {
            proposed.functions[0].blocks[0].instructions[0].kind =
                selected_instructions::SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(8),
                };
        } else {
            proposed.functions[0].virtual_registers[0].entry_fixed_view = None;
        }
        assert!(
            validate_selected_instructions(
                &legalized,
                &constraints,
                environment.physical(),
                environment.constraints(),
                proposed
            )
            .is_err()
        );
        let mut proposed = selected.plan().clone();
        let selected_instructions::SelectedTerminator::Return { instruction, .. } =
            &mut proposed.functions[0].blocks[0].terminator
        else {
            unreachable!()
        };
        instruction.operands[0].fixed_view =
            environment.fixed_register_view(target_operations::MachineRegister::X86Rcx);
        assert!(
            validate_selected_instructions(
                &legalized,
                &constraints,
                environment.physical(),
                environment.constraints(),
                proposed
            )
            .is_err()
        );
    }
}
