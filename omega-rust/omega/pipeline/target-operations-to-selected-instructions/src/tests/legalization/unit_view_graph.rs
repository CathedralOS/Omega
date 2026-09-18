//! Shared Unit graphs retain observations, Boolean homes and original reference calls.
use crate::{
    legalize_target_operations, select_instructions, validate_legalized_operations,
    validate_selected_instructions,
};
use abstract_operations::{
    AbstractBlockEntry, AbstractOperation, AbstractParameter, AbstractResult, AbstractSuccessor,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, OperationId,
    PlaceId, ScalarType, StructuralTypeId, ValueId,
};
use target::NativeTarget;
use target_operations::{
    TargetBooleanExpression, TargetControlTerminator, TargetIntegerExpression,
    TargetScalarExpression, TargetUnitOperation,
};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
};

fn fixture(
    native: NativeTarget,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let place = PlaceId::new(10).unwrap();
    let structural_type = StructuralTypeId::new(10).unwrap();
    source
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: structural_type,
            identity: "shared-view".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        });
    let caller = &mut source.functions[0];
    caller.parameters = vec![AbstractParameter {
        value: ValueId::new(100).unwrap(),
        scalar_type,
    }];
    caller.structural_parameters = vec![StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    let mut callee = caller.clone();
    callee.machine = MachineId::new(2).unwrap();
    callee.entry = BlockId::new(20).unwrap();
    callee.block_entries[0].block = callee.entry;
    callee.parameters[0].value = ValueId::new(200).unwrap();
    callee.structural_parameters[0].place = PlaceId::new(20).unwrap();
    callee.operations = vec![AbstractOperation::ReturnUnit {
        psi_edge: EdgeId::new(20).unwrap(),
        cleanup_actions: Vec::new(),
    }];
    caller.block_entries = [(1, 0), (2, 3), (3, 5)]
        .map(|(block, operation_offset)| AbstractBlockEntry {
            structural_parameters: Vec::new(),
            block: BlockId::new(block).unwrap(),
            parameters: Vec::new(),
            operation_offset,
        })
        .to_vec();
    let successor = |identity, target| AbstractSuccessor {
        structural_bindings: Vec::new(),
        psi_edge: EdgeId::new(identity).unwrap(),
        target: BlockId::new(target).unwrap(),
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    caller.operations = vec![
        AbstractOperation::ByteSequenceLength {
            psi_operation: OperationId::new(10).unwrap(),
            result: AbstractResult {
                value: ValueId::new(101).unwrap(),
                scalar_type,
            },
            source: place,
        },
        AbstractOperation::IntegerLessThan {
            psi_operation: OperationId::new(11).unwrap(),
            result: ValueId::new(102).unwrap(),
            left: ValueId::new(100).unwrap(),
            right: ValueId::new(101).unwrap(),
        },
        AbstractOperation::Conditional {
            condition: ValueId::new(102).unwrap(),
            when_true: successor(10, 2),
            when_false: successor(11, 3),
        },
        AbstractOperation::CallUnit {
            psi_operation: OperationId::new(12).unwrap(),
            callee: callee.machine,
            arguments: vec![ValueId::new(100).unwrap()],
            structural_arguments: vec![StructuralArgument {
                place,
                access: StructuralAccess::SharedBorrow,
                path: Vec::new(),
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(12).unwrap(),
            cleanup_actions: Vec::new(),
        },
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(13).unwrap(),
            cleanup_actions: Vec::new(),
        },
    ];
    source.functions.push(callee);
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
    )
    .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (source, target, unit)
}

/// One borrowed view whose root arrives through control flow: a non-entry
/// block structural parameter lends the caller's own view to an ordinary call.
/// `block_access` is the block parameter's own access; the argument it lends
/// stays exclusive, so a shared root must not satisfy it.
fn block_parameter_source(
    block_access: StructuralAccess,
) -> abstract_operations::AbstractOperationPlan {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let place = PlaceId::new(10).unwrap();
    let lent = PlaceId::new(11).unwrap();
    let structural_type = StructuralTypeId::new(10).unwrap();
    source
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: structural_type,
            identity: "exclusive-view".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        });
    let caller = &mut source.functions[0];
    caller.parameters = vec![AbstractParameter {
        value: ValueId::new(100).unwrap(),
        scalar_type,
    }];
    caller.structural_parameters = vec![StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    let mut callee = caller.clone();
    callee.machine = MachineId::new(2).unwrap();
    callee.entry = BlockId::new(20).unwrap();
    callee.block_entries[0].block = callee.entry;
    callee.parameters[0].value = ValueId::new(200).unwrap();
    callee.structural_parameters[0].place = PlaceId::new(20).unwrap();
    callee.operations = vec![AbstractOperation::ReturnUnit {
        psi_edge: EdgeId::new(20).unwrap(),
        cleanup_actions: Vec::new(),
    }];
    caller.block_entries = vec![
        AbstractBlockEntry {
            structural_parameters: Vec::new(),
            block: BlockId::new(1).unwrap(),
            parameters: Vec::new(),
            operation_offset: 0,
        },
        AbstractBlockEntry {
            structural_parameters: vec![StructuralParameterDeclaration {
                place: lent,
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: block_access,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            block: BlockId::new(2).unwrap(),
            parameters: Vec::new(),
            operation_offset: 3,
        },
        AbstractBlockEntry {
            structural_parameters: Vec::new(),
            block: BlockId::new(3).unwrap(),
            parameters: Vec::new(),
            operation_offset: 5,
        },
    ];
    let successor = |identity, target, lend| AbstractSuccessor {
        structural_bindings: if lend {
            vec![abstract_operations::AbstractStructuralBinding {
                parameter: lent,
                argument: StructuralArgument {
                    place,
                    access: block_access,
                    path: Vec::new(),
                },
            }]
        } else {
            Vec::new()
        },
        psi_edge: EdgeId::new(identity).unwrap(),
        target: BlockId::new(target).unwrap(),
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    caller.operations = vec![
        AbstractOperation::ByteSequenceLength {
            psi_operation: OperationId::new(10).unwrap(),
            result: AbstractResult {
                value: ValueId::new(101).unwrap(),
                scalar_type,
            },
            source: place,
        },
        AbstractOperation::IntegerLessThan {
            psi_operation: OperationId::new(11).unwrap(),
            result: ValueId::new(102).unwrap(),
            left: ValueId::new(100).unwrap(),
            right: ValueId::new(101).unwrap(),
        },
        AbstractOperation::Conditional {
            condition: ValueId::new(102).unwrap(),
            when_true: successor(10, 2, true),
            when_false: successor(11, 3, false),
        },
        AbstractOperation::CallUnit {
            psi_operation: OperationId::new(12).unwrap(),
            callee: callee.machine,
            arguments: vec![ValueId::new(100).unwrap()],
            structural_arguments: vec![StructuralArgument {
                place: lent,
                access: StructuralAccess::MutableBorrow,
                path: Vec::new(),
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(12).unwrap(),
            cleanup_actions: Vec::new(),
        },
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(13).unwrap(),
            cleanup_actions: Vec::new(),
        },
    ];
    source.functions.push(callee);
    source
}

/// An exclusive borrow reaches an ordinary call from a block parameter, not
/// only from the machine entrance: control flow establishes the same custody
/// the entrance publishes, and the argument retains that block-parameter
/// source through legalization and its independent replay. A shared root
/// cannot satisfy the exclusive argument, so the physical view alone never
/// authorizes the access.
#[test]
fn an_exclusive_view_lends_through_a_block_parameter_and_a_shared_root_rejects() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = block_parameter_source(StructuralAccess::MutableBorrow);
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .unwrap();
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
        let argument = target.functions[0]
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::Call { arguments, .. } => arguments.first(),
                _ => None,
            })
            .expect("lent call argument");
        assert_eq!(
            argument.source,
            target_operations::TargetStructuralArgumentSource::BlockParameter {
                block: BlockId::new(2).unwrap(),
                place: PlaceId::new(11).unwrap(),
            },
            "{native:?}"
        );
        assert_eq!(
            argument.access,
            StructuralAccess::MutableBorrow,
            "{native:?}"
        );
        let shared = block_parameter_source(StructuralAccess::SharedBorrow);
        assert!(
            abstract_operations_to_target_operations::lower_to_target_operations(
                &shared,
                abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
            )
            .is_err(),
            "a shared block parameter cannot lend an exclusive argument: {native:?}"
        );
    }
}

#[test]
fn shared_unit_graph_observations_and_boolean_homes_replay_on_all_hosted_targets() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let (source, target, unit) = fixture(native);
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
        assert!(selected.plan().functions[0].local_storage_slots.is_empty());
        for mutation in 0..11 {
            let mut changed = target.clone();
            let graph = &mut changed.functions[0].graph;
            match mutation {
                0 => graph.parameters[0].access = StructuralAccess::Owned,
                1 => {
                    graph.parameters[0].placement.shape =
                        calling_conventions::ValueShape::integer(16, 8)
                }
                2 => {
                    let TargetUnitOperation::ScalarDefinition { result_home, .. } =
                        &mut graph.blocks[0].operations[0]
                    else {
                        panic!("length");
                    };
                    result_home.source_value = ValueId::new(999).unwrap();
                }
                3 => {
                    let TargetUnitOperation::ScalarDefinition {
                        expression:
                            TargetScalarExpression::Integer {
                                expression:
                                    TargetIntegerExpression::ByteSequenceLength { source, .. },
                                ..
                            },
                        ..
                    } = &mut graph.blocks[0].operations[0]
                    else {
                        panic!("length");
                    };
                    *source = PlaceId::new(999).unwrap();
                }
                4 => {
                    let TargetUnitOperation::ScalarDefinition {
                        expression:
                            TargetScalarExpression::Boolean(TargetBooleanExpression::IntegerLessThan {
                                right,
                                ..
                            }),
                        ..
                    } = &mut graph.blocks[0].operations[1]
                    else {
                        panic!("compare");
                    };
                    let TargetIntegerExpression::ScalarHome(home) = right.as_mut() else {
                        panic!("length home");
                    };
                    home.defining_operation = OperationId::new(999).unwrap();
                }
                5 => {
                    let TargetControlTerminator::Conditional {
                        condition: TargetBooleanExpression::ScalarHome(home),
                        ..
                    } = &mut graph.blocks[0].terminator
                    else {
                        panic!("Boolean home");
                    };
                    home.shape = calling_conventions::ValueShape::integer(8, 8);
                }
                6 => graph.blocks[0].operations.swap(0, 1),
                7 => {
                    graph.blocks[0].operations.pop();
                }
                8 | 9 => {
                    let TargetUnitOperation::ScalarDefinition { result_home, .. } =
                        &mut graph.blocks[0].operations[0]
                    else {
                        panic!("length");
                    };
                    if mutation == 8 {
                        result_home.scalar_type = ScalarType::Boolean;
                    } else {
                        result_home.shape = calling_conventions::ValueShape::integer(1, 1);
                    }
                }
                _ => {
                    let TargetControlTerminator::Conditional {
                        condition: TargetBooleanExpression::ScalarHome(home),
                        ..
                    } = &mut graph.blocks[0].terminator
                    else {
                        panic!("Boolean home");
                    };
                    home.source_value = ValueId::new(101).unwrap();
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
        let mut changed = selected.plan().clone();
        changed.functions[0].calls.clear();
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
