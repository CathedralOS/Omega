//! Real shared-return value flow, including unused incoming arm parameters.

use crate::{
    legalize_target_operations, select_instructions, selection_constraints,
    validate_legalized_operations, validate_selected_instructions,
};
use abstract_operations::{
    AbstractBlockEntry, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan,
    AbstractParameter, AbstractResult, AbstractSuccessor, ValueBinding,
};
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
};

fn value(raw: u64) -> ValueId {
    ValueId::new(raw).unwrap()
}
fn block(raw: u64) -> BlockId {
    BlockId::new(raw).unwrap()
}
fn edge(raw: u64) -> EdgeId {
    EdgeId::new(raw).unwrap()
}
fn operation(raw: u64) -> OperationId {
    OperationId::new(raw).unwrap()
}

fn fixture(
    target: target::NativeTarget,
) -> (
    AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut abstracted, _, previous) = super::fixtures::plain_unit::plain_unit_fixture();
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let parameter = |raw| AbstractParameter {
        value: value(raw),
        scalar_type: scalar,
    };
    let binding = |destination, argument| ValueBinding {
        parameter: value(destination),
        argument: value(argument),
        scalar_type: scalar,
    };
    let function = &mut abstracted.functions[0];
    function.parameters = vec![parameter(1), parameter(2)];
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: value(11),
        scalar_type: scalar,
    });
    function.block_entries = vec![
        AbstractBlockEntry {
            block: block(1),
            parameters: Vec::new(),
            operation_offset: 0,
        },
        AbstractBlockEntry {
            block: block(2),
            parameters: vec![parameter(3)],
            operation_offset: 2,
        },
        AbstractBlockEntry {
            block: block(3),
            parameters: vec![parameter(5), parameter(6)],
            operation_offset: 3,
        },
        AbstractBlockEntry {
            block: block(4),
            parameters: vec![parameter(7), parameter(8)],
            operation_offset: 5,
        },
    ];
    function.operations = vec![
        AbstractOperation::IntegerEqual {
            psi_operation: operation(1),
            result: value(4),
            left: value(1),
            right: value(2),
        },
        AbstractOperation::Conditional {
            condition: value(4),
            when_true: AbstractSuccessor {
                psi_edge: edge(1),
                target: block(3),
                bindings: vec![binding(5, 1), binding(6, 2)],
                trivial_affine_discards: Vec::new(),
            },
            when_false: AbstractSuccessor {
                psi_edge: edge(2),
                target: block(4),
                bindings: vec![binding(7, 1), binding(8, 2)],
                trivial_affine_discards: Vec::new(),
            },
        },
        AbstractOperation::Return {
            psi_edge: edge(3),
            result: value(11),
            value: value(3),
            scalar_type: scalar,
            cleanup_actions: Vec::new(),
        },
        AbstractOperation::IntegerConstant {
            psi_operation: operation(2),
            result: value(9),
            scalar_type: scalar,
            value: IntegerValue::Unsigned(7),
        },
        AbstractOperation::Jump {
            psi_edge: edge(4),
            target: block(2),
            bindings: vec![binding(3, 9)],
            trivial_affine_discards: Vec::new(),
        },
        AbstractOperation::IntegerConstant {
            psi_operation: operation(3),
            result: value(10),
            scalar_type: scalar,
            value: IntegerValue::Unsigned(0),
        },
        AbstractOperation::Jump {
            psi_edge: edge(5),
            target: block(2),
            bindings: vec![binding(3, 10)],
            trivial_affine_discards: Vec::new(),
        },
    ];
    let target =
        abstract_operations_to_target_operations::lower_to_target_operations(&abstracted, target)
            .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &abstracted,
        previous.fuel_schedule,
    )
    .unwrap();
    (abstracted, target, unit)
}

#[test]
fn shared_return_selection_preserves_real_blocks_and_binding_edges() {
    for target in [
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (abstracted, target_plan, unit) = fixture(target);
        let legalized = legalize_target_operations(&target_plan, &abstracted, &unit).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = selection_constraints(&legalized, &environment);
        let selected = select_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let function = &selected.plan().functions[0];
        assert_eq!(
            function
                .blocks
                .iter()
                .map(|block| block.source_block)
                .collect::<Vec<_>>(),
            [block(1), block(3), block(4), block(2)]
        );
        assert_eq!(function.virtual_registers.len(), 5);
        assert!(
            matches!(function.virtual_registers[4].origin, selected_instructions::VirtualRegisterOrigin::BlockParameter { source_value, .. } if source_value == value(3))
        );
        for arm in &function.blocks[1..3] {
            let selected_instructions::SelectedTerminator::Jump { successor, .. } = &arm.terminator
            else {
                panic!("actual jump")
            };
            assert_eq!(successor.source_target, block(2));
            assert_eq!(successor.bindings[0].parameter, value(3));
        }
    }
}

#[test]
fn shared_return_legalization_rejects_substituted_bindings_and_join_identity() {
    let (abstracted, target, unit) = fixture(target::NativeTarget::linux_x64());
    let legalized = legalize_target_operations(&target, &abstracted, &unit).unwrap();
    for change in 0..5 {
        let mut proposed = legalized.plan().clone();
        let legalized_operations::LegalizedFunction::SharedReturnConditional(function) =
            &mut proposed.functions[0]
        else {
            panic!("common return")
        };
        match change {
            0 => function.when_true.transfer_binding.argument = value(10),
            1 => function.when_true.branch_bindings[0].argument = value(2),
            2 => function.return_parameter.value = value(9),
            3 => function.when_false.transfer_fuel.clear(),
            _ => function.when_true.parameters.clear(),
        }
        assert_ne!(
            legalized_operations::legalized_operation_plan_identity(&proposed),
            legalized_operations::legalized_operation_plan_identity(legalized.plan())
        );
        assert!(validate_legalized_operations(&target, &abstracted, &unit, proposed).is_err());
    }
}

#[test]
fn shared_return_selection_rejects_substituted_transfer_and_parameter_home() {
    let native_target = target::NativeTarget::linux_x64();
    let (abstracted, target, unit) = fixture(native_target);
    let legalized = legalize_target_operations(&target, &abstracted, &unit).unwrap();
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
    for change in 0..5 {
        let mut plan = selected.plan().clone();
        let function = &mut plan.functions[0];
        match change {
            0 => {
                let selected_instructions::SelectedTerminator::Jump { successor, .. } =
                    &mut function.blocks[1].terminator
                else {
                    unreachable!()
                };
                successor.bindings[0].argument = value(10);
            }
            1 => {
                let selected_instructions::SelectedTerminator::Jump { successor, .. } =
                    &mut function.blocks[1].terminator
                else {
                    unreachable!()
                };
                successor.source_target = block(4);
            }
            2 => {
                function.virtual_registers[4].origin =
                    selected_instructions::VirtualRegisterOrigin::BlockParameter {
                        source_value: value(3),
                        block: selected_instructions::SelectedBlockId(2),
                        parameter_index: 0,
                    }
            }
            3 => {
                let selected_instructions::SelectedTerminator::ConditionalBranch {
                    when_zero, ..
                } = &mut function.blocks[0].terminator
                else {
                    unreachable!()
                };
                when_zero.bindings.clear();
            }
            _ => {
                function.blocks[2].instructions[0].kind =
                    selected_instructions::SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(1),
                    }
            }
        }
        assert_ne!(
            crate::selected_instruction_plan_identity(&plan),
            crate::selected_instruction_plan_identity(selected.plan())
        );
        assert!(
            validate_selected_instructions(
                &legalized,
                &constraints,
                environment.physical(),
                environment.constraints(),
                plan
            )
            .is_err()
        );
    }
}
