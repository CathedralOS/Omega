//! Unit control uses the ordinary scalar graph on every hosted ABI.
use crate::{
    legalize_target_operations, select_instructions, validate_legalized_operations,
    validate_selected_instructions,
};
use abstract_operations::{
    AbstractBlockEntry, AbstractOperation, AbstractParameter, AbstractSuccessor,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations::{TargetControlTerminator, TargetOperation, TargetUnitOperation};

fn targets() -> [NativeTarget; 4] {
    [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ]
}
fn block(value: u64) -> BlockId {
    BlockId::new(value).unwrap()
}
fn edge(value: u64) -> EdgeId {
    EdgeId::new(value).unwrap()
}
fn value(identity: u64) -> ValueId {
    ValueId::new(identity).unwrap()
}
fn successor(identity: u64, target: u64) -> AbstractSuccessor {
    AbstractSuccessor {
        structural_bindings: Vec::new(),
        psi_edge: edge(identity),
        target: block(target),
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
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
fn jump(identity: u64) -> AbstractOperation {
    AbstractOperation::Jump {
        structural_bindings: Vec::new(),
        psi_edge: edge(identity),
        target: block(4),
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    }
}
pub(super) fn fixture(
    native: NativeTarget,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let mut callee = source.functions[0].clone();
    callee.machine = MachineId::new(2).unwrap();
    callee.entry = block(20);
    callee.block_entries[0].block = callee.entry;
    callee.parameters = vec![AbstractParameter {
        value: value(200),
        scalar_type,
    }];
    callee.operations = vec![AbstractOperation::ReturnUnit {
        psi_edge: edge(20),
        cleanup_actions: Vec::new(),
    }];
    let caller = &mut source.functions[0];
    caller.parameters = vec![
        AbstractParameter {
            value: value(100),
            scalar_type: ScalarType::Boolean,
        },
        AbstractParameter {
            value: value(101),
            scalar_type,
        },
    ];
    caller.block_entries = [(1, 0), (2, 1), (3, 4), (4, 6)]
        .map(|(identity, offset)| AbstractBlockEntry {
            structural_parameters: Vec::new(),
            block: block(identity),
            parameters: Vec::new(),
            operation_offset: offset,
        })
        .to_vec();
    caller.operations = vec![
        AbstractOperation::Conditional {
            condition: value(100),
            when_true: successor(10, 2),
            when_false: successor(11, 3),
        },
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(10).unwrap(),
            result: value(102),
            scalar_type,
            value: IntegerValue::Signed(33),
        },
        call(11, 102),
        jump(12),
        call(12, 101),
        jump(13),
        call(13, 101),
        AbstractOperation::ReturnUnit {
            psi_edge: edge(14),
            cleanup_actions: Vec::new(),
        },
    ];
    source.functions.push(callee);
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
fn unit_graph_calls_branch_and_rejoin_on_all_hosted_targets() {
    for native in targets() {
        let (source, target, unit) = fixture(native);
        assert!(matches!(
            target.functions[0].operation,
            TargetOperation::ControlGraph(_)
        ));
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
        let caller = &legal.plan().scalar_functions[0];
        assert_eq!(caller.blocks.len(), 4);
        assert!(caller.call_plan.result.is_none());
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
        let caller = &selected.plan().functions[0];
        assert_eq!(caller.blocks.len(), 4);
        assert_eq!(
            caller
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter(|row| matches!(
                    row.kind,
                    selected_instructions::SelectedInstructionKind::CallUnit { .. }
                ))
                .count(),
            3
        );
        for mutation in 0..3 {
            let mut changed = selected.plan().clone();
            let blocks = &mut changed.functions[0].blocks;
            match mutation {
                0 => {
                    blocks.swap(1, 2);
                }
                1 => {
                    blocks[3].instructions.clear();
                }
                _ => {
                    blocks[0].terminator = blocks[1].terminator.clone();
                }
            }
            assert!(
                validate_selected_instructions(
                    &legal,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    changed
                )
                .is_err()
            );
        }
    }
}

#[test]
fn unit_graph_replay_rejects_cfg_and_source_substitution() {
    for native in targets() {
        let (source, target, unit) = fixture(native);
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        for mutation in 0..12 {
            let mut changed = target.clone();
            let TargetOperation::ControlGraph(graph) = &mut changed.functions[0].operation else {
                panic!("graph");
            };
            match mutation {
                0 => graph.entry = block(2),
                1 => graph.blocks.swap(1, 2),
                2 => {
                    graph.blocks.pop();
                }
                3 => {
                    graph.blocks[1].operations.remove(0);
                }
                4 => graph.blocks[3].terminator = graph.blocks[1].terminator.clone(),
                5 => {
                    graph.scalar_parameters[0].scalar_type = graph.scalar_parameters[1].scalar_type
                }
                6 => {
                    let branch_source = match &graph.blocks[1].operations[1] {
                        TargetUnitOperation::Call {
                            scalar_arguments, ..
                        } => scalar_arguments[0].source,
                        _ => panic!("call"),
                    };
                    let TargetUnitOperation::Call {
                        scalar_arguments, ..
                    } = &mut graph.blocks[3].operations[0]
                    else {
                        panic!("call");
                    };
                    scalar_arguments[0].source = branch_source;
                }
                _ => {
                    let TargetControlTerminator::Conditional {
                        condition_source,
                        condition,
                        when_true,
                        when_false,
                    } = &mut graph.blocks[0].terminator
                    else {
                        panic!("conditional");
                    };
                    match mutation {
                        7 => std::mem::swap(when_true, when_false),
                        8 => when_true.target = block(3),
                        9 => when_true.psi_edge = edge(11),
                        10 => *condition_source = value(101),
                        _ => {
                            let target_operations::TargetBooleanExpression::Parameter {
                                parameter_index,
                                ..
                            } = condition
                            else {
                                panic!("parameter");
                            };
                            *parameter_index = 1;
                        }
                    }
                }
            }
            assert!(
                legalize_target_operations(&changed, &source, &unit).is_err(),
                "target mutation {mutation}: {native:?}"
            );
            assert!(
                validate_legalized_operations(&changed, &source, &unit, legal.plan().clone())
                    .is_err()
            );
        }
        for mutation in 0..3 {
            let mut changed = legal.plan().clone();
            let blocks = &mut changed.scalar_functions[0].blocks;
            match mutation {
                0 => {
                    blocks.swap(1, 2);
                }
                1 => {
                    blocks[3].instructions.clear();
                }
                _ => {
                    blocks[0].terminator = blocks[1].terminator.clone();
                }
            }
            assert!(validate_legalized_operations(&target, &source, &unit, changed).is_err());
        }
    }
}
