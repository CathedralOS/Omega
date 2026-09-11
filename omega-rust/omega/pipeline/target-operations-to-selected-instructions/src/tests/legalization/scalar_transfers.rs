//! Scalar Unit edges retain typed block roots and independently replay parallel transport.
use crate::{
    legalize_target_operations, select_instructions, validate_legalized_operations,
    validate_selected_instructions,
};
use abstract_operations::{
    AbstractBlockEntry, AbstractOperation, AbstractParameter, AbstractSuccessor, ValueBinding,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, OperationId,
    ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations::{
    TargetBooleanExpression, TargetControlTerminator, TargetUnitOperation,
    TargetUnitScalarArgumentSource,
};

fn block(identity: u64) -> BlockId {
    BlockId::new(identity).unwrap()
}
fn value(identity: u64) -> ValueId {
    ValueId::new(identity).unwrap()
}
fn edge(identity: u64) -> EdgeId {
    EdgeId::new(identity).unwrap()
}
fn integer() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}
fn parameters(first: u64) -> Vec<AbstractParameter> {
    vec![
        AbstractParameter {
            value: value(first),
            scalar_type: ScalarType::Boolean,
        },
        AbstractParameter {
            value: value(first + 1),
            scalar_type: integer(),
        },
    ]
}
fn successor(identity: u64, destination: u64, first: u64, argument: u64) -> AbstractSuccessor {
    AbstractSuccessor {
        structural_bindings: Vec::new(),
        psi_edge: edge(identity),
        target: block(destination),
        bindings: parameters(first)
            .iter()
            .enumerate()
            .map(|(index, parameter)| ValueBinding {
                parameter: parameter.value,
                argument: value(argument + index as u64),
                scalar_type: parameter.scalar_type,
            })
            .collect(),
        trivial_affine_discards: Vec::new(),
    }
}
fn jump(identity: u64, argument: u64) -> AbstractOperation {
    let successor = successor(identity, 4, 140, argument);
    AbstractOperation::Jump {
        structural_bindings: Vec::new(),
        psi_edge: successor.psi_edge,
        target: successor.target,
        bindings: successor.bindings,
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    }
}
fn call(identity: u64, argument: u64) -> AbstractOperation {
    AbstractOperation::CallUnit {
        psi_operation: OperationId::new(identity).unwrap(),
        callee: MachineId::new(2).unwrap(),
        arguments: vec![value(argument)],
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    }
}
fn fixture(
    native: NativeTarget,
    bits: u16,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut source, _, _) = super::unit_graph::fixture(native);
    source.functions[1].parameters[0].scalar_type = integer();
    let caller = &mut source.functions[0];
    caller.parameters = parameters(100);
    caller.block_entries = [
        (1, 0, None),
        (2, 1, Some(120)),
        (3, 3, Some(130)),
        (4, 5, Some(140)),
        (5, 7, None),
        (6, 8, None),
    ]
    .map(|(identity, operation_offset, first)| AbstractBlockEntry {
        structural_parameters: Vec::new(),
        block: block(identity),
        parameters: first.map(parameters).unwrap_or_default(),
        operation_offset,
    })
    .to_vec();
    caller.operations = vec![
        AbstractOperation::Conditional {
            condition: value(100),
            when_true: successor(10, 2, 120, 100),
            when_false: successor(11, 3, 130, 100),
        },
        call(11, 121),
        jump(12, 120),
        call(12, 131),
        jump(13, 130),
        call(13, 141),
        AbstractOperation::Conditional {
            condition: value(140),
            when_true: AbstractSuccessor {
                structural_bindings: Vec::new(),
                psi_edge: edge(14),
                target: block(5),
                bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
            when_false: AbstractSuccessor {
                structural_bindings: Vec::new(),
                psi_edge: edge(15),
                target: block(6),
                bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        },
        AbstractOperation::ReturnUnit {
            psi_edge: edge(16),
            cleanup_actions: Vec::new(),
        },
        AbstractOperation::ReturnUnit {
            psi_edge: edge(17),
            cleanup_actions: Vec::new(),
        },
    ];
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap());
    for function in &mut source.functions {
        for parameter in function.parameters.iter_mut().chain(
            function
                .block_entries
                .iter_mut()
                .flat_map(|block| &mut block.parameters),
        ) {
            if parameter.scalar_type == integer() {
                parameter.scalar_type = scalar_type;
            }
        }
        for operation in &mut function.operations {
            match operation {
                AbstractOperation::Jump { bindings, .. } => {
                    for binding in bindings {
                        if binding.scalar_type == integer() {
                            binding.scalar_type = scalar_type;
                        }
                    }
                }
                AbstractOperation::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for binding in when_true
                        .bindings
                        .iter_mut()
                        .chain(&mut when_false.bindings)
                    {
                        if binding.scalar_type == integer() {
                            binding.scalar_type = scalar_type;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let target =
        abstract_operations_to_target_operations::lower_to_target_operations(&source, native)
            .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (source, target, unit)
}

#[test]
fn typed_unit_transfers_select_and_replay_on_all_hosted_targets() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        for bits in [8, 64] {
            let (source, target, unit) = fixture(native, bits);
            let legal = legalize_target_operations(&target, &source, &unit).unwrap();
            validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = crate::selection_constraints(&legal, &environment);
            let selected = select_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .unwrap();
            for mutation in 0..3 {
                let mut changed = selected.plan().clone();
                let bridge = changed.functions[0].blocks.iter_mut().find(|block|
                    matches!(block.origin, selected_instructions::SelectedBlockOrigin::EdgeTransfer { edge: transfer, .. } if transfer == edge(12))).expect("prepared edge");
                let selected_instructions::SelectedTerminator::Jump { successor, .. } =
                    &mut bridge.terminator
                else {
                    panic!("jump");
                };
                match mutation {
                    0 => {
                        successor.bindings[0].transport =
                            selected_instructions::SelectedValueTransport::Unused
                    }
                    1 => successor.bindings.swap(0, 1),
                    _ => successor.bindings[1].semantic.argument = value(101),
                }
                assert!(
                    validate_selected_instructions(
                        &legal,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                        changed
                    )
                    .is_err(),
                    "selected mutation {mutation}: {native:?}"
                );
            }
        }
    }
}

#[test]
fn typed_unit_transfers_reject_owner_type_and_edge_substitution() {
    let (source, target, unit) = fixture(NativeTarget::linux_x64(), 64);
    let legal = legalize_target_operations(&target, &source, &unit).unwrap();
    for mutation in 0..8 {
        let mut changed = target.clone();
        let graph = &mut changed.functions[0].graph;
        match mutation {
            0 | 1 => {
                let TargetUnitOperation::Call {
                    scalar_arguments, ..
                } = &mut graph.blocks[1].operations[0]
                else {
                    panic!("call");
                };
                let TargetUnitScalarArgumentSource::BlockParameter(parameter) =
                    &mut scalar_arguments[0].source
                else {
                    panic!("block argument");
                };
                if mutation == 0 {
                    parameter.block = block(3);
                } else {
                    parameter.scalar_type = ScalarType::Boolean;
                }
            }
            2 | 3 => {
                let TargetControlTerminator::Conditional {
                    condition: TargetBooleanExpression::BlockParameter(parameter),
                    ..
                } = &mut graph.blocks[3].terminator
                else {
                    panic!("condition");
                };
                if mutation == 2 {
                    parameter.block = block(2);
                } else {
                    parameter.value = value(120);
                    parameter.block = block(2);
                }
            }
            4 => graph.blocks[3].parameters[0].scalar_type = integer(),
            _ => {
                let TargetControlTerminator::Jump { successor } = &mut graph.blocks[1].terminator
                else {
                    panic!("jump");
                };
                match mutation {
                    5 => successor.bindings[1].argument = value(131),
                    6 => successor.bindings.swap(0, 1),
                    _ => {
                        successor.bindings.pop();
                    }
                }
            }
        }
        assert!(
            legalize_target_operations(&changed, &source, &unit).is_err(),
            "target mutation {mutation}"
        );
        assert!(
            validate_legalized_operations(&changed, &source, &unit, legal.plan().clone()).is_err()
        );
    }
}

#[test]
fn computed_comparison_branch_and_edge_arguments_retain_materialized_value() {
    let native = NativeTarget::linux_x64();
    let (mut source, _, _) = fixture(native, 64);
    let caller = &mut source.functions[0];
    caller.operations.insert(
        0,
        AbstractOperation::IntegerLessThan {
            psi_operation: OperationId::new(30).unwrap(),
            result: value(150),
            left: value(101),
            right: value(101),
        },
    );
    for entry in caller.block_entries.iter_mut().skip(1) {
        entry.operation_offset += 1;
    }
    let AbstractOperation::Conditional { condition, .. } = &mut caller.operations[1] else {
        panic!("conditional");
    };
    *condition = value(150);
    // Sharing the predicate with outgoing arguments requires its ordinary Boolean value.
    for transfer_comparison in [false, true] {
        if transfer_comparison {
            let AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } = &mut source.functions[0].operations[1]
            else {
                panic!("conditional");
            };
            when_true.bindings[0].argument = value(150);
            when_false.bindings[0].argument = value(150);
        }
        let target =
            abstract_operations_to_target_operations::lower_to_target_operations(&source, native)
                .unwrap();
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legal, &environment);
        let selected = select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        let materializes = selected
            .plan()
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .any(|row| {
                matches!(
                    row.kind,
                    selected_instructions::SelectedInstructionKind::MaterializeBooleanU64LessThan
                )
            });
        assert_eq!(materializes, transfer_comparison);
    }
}
