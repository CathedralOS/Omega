//! Case targets are source blocks, including returning arms and continuations.
//! The hosted read leaf fixes EOF at tag zero and the byte payload at tag one;
//! reorder executable blocks without changing that external result convention.
use super::super::super::{BoundaryMachineId, TargetUnitOperation};

use super::{
    AbstractBlockEntry, AbstractOperation, AbstractOperationPlan, AbstractParameter, IntegerValue,
    NativeTarget, ScalarType, block, edge, operation, value,
};
use abstract_operations::{
    AbstractBoundaryResult, AbstractStructuralCasePayloadBinding, AbstractStructuralCaseSuccessor,
};
use semantic_vocabulary::{
    BlockId, OperationId, PlaceId, StructuralCaseId, StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BoundaryMachineResult, BoundaryStructuralResultDeclaration, StructuralCaseDeclaration,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralTypeDeclaration, StructuralTypeShape,
};

fn fixture() -> AbstractOperationPlan {
    let mut plan = super::super::returning_byte_parameter::fixture();
    let scalar_type = plan.functions[0].parameters[0].scalar_type;
    let structural_type = StructuralTypeId::new(20).unwrap();
    let source = PlaceId::new(20).unwrap();
    let field = StructuralFieldId::new(1).unwrap();
    plan.structural_types = vec![StructuralTypeDeclaration {
        id: structural_type,
        identity: "ByteRead".into(),
        shape: StructuralTypeShape::Sum {
            cases: vec![
                StructuralCaseDeclaration {
                    id: StructuralCaseId::new(1).unwrap(),
                    identity: "Eof".into(),
                    fields: Vec::new(),
                },
                StructuralCaseDeclaration {
                    id: StructuralCaseId::new(2).unwrap(),
                    identity: "Byte".into(),
                    fields: vec![StructuralFieldDeclaration {
                        id: field,
                        identity: "value".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(scalar_type),
                    }],
                },
            ],
        },
    }]
    .into();
    let mut read = plan.boundary_machines[0].clone();
    read.id = BoundaryMachineId::new(902).unwrap();
    read.identity = "Console::read_byte()->ByteRead".into();
    read.parameter_order.clear();
    read.scalar_parameters.clear();
    read.result = BoundaryMachineResult::Structural(BoundaryStructuralResultDeclaration {
        structural_type,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
    });
    plan.boundary_machines.push(read);
    let function = &mut plan.functions[0];
    function.parameters.clear();
    function.entry = block(10);
    // The empty arm precedes the payload arm physically; a fourth block joins
    // both returning arms. Neither offsets nor targets fit the retired template.
    function.block_entries = [(10, 0), (30, 3), (20, 4), (40, 6)]
        .into_iter()
        .map(|(identity, operation_offset)| AbstractBlockEntry {
            block: block(identity),
            operation_offset,
            structural_parameters: Vec::new(),
            parameters: if identity == 20 {
                vec![AbstractParameter {
                    value: value(20),
                    scalar_type,
                }]
            } else {
                Vec::new()
            },
        })
        .collect();
    let return_operation = AbstractOperation::ReturnUnit {
        psi_edge: edge(6),
        cleanup_actions: Vec::new(),
    };
    let jump = |identity| AbstractOperation::Jump {
        psi_edge: edge(identity),
        target: block(40),
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    function.operations = vec![
        AbstractOperation::IntegerConstant {
            psi_operation: operation(1),
            result: value(1),
            scalar_type,
            value: IntegerValue::Signed(33),
        },
        AbstractOperation::BoundaryCall {
            psi_operation: operation(2),
            result: AbstractBoundaryResult::Structural(StructuralOperationResult {
                place: source,
                structural_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            boundary: plan.boundary_machines[1].id,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        },
        AbstractOperation::StructuralCase {
            source,
            cases: vec![
                AbstractStructuralCaseSuccessor {
                    psi_edge: edge(2),
                    target: block(30),
                    case: StructuralCaseId::new(1).unwrap(),
                    payloads: Vec::new(),
                    trivial_affine_discards: vec![source],
                },
                AbstractStructuralCaseSuccessor {
                    psi_edge: edge(1),
                    target: block(20),
                    case: StructuralCaseId::new(2).unwrap(),
                    payloads: vec![AbstractStructuralCasePayloadBinding {
                        parameter: value(20),
                        field,
                        scalar_type,
                    }],
                    trivial_affine_discards: vec![source],
                },
            ],
        },
        jump(3),
        AbstractOperation::BoundaryCall {
            psi_operation: operation(3),
            result: AbstractBoundaryResult::Unit,
            boundary: plan.boundary_machines[0].id,
            arguments: vec![value(20)],
            structural_arguments: Vec::new(),
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        },
        jump(4),
        AbstractOperation::BoundaryCall {
            psi_operation: operation(4),
            result: AbstractBoundaryResult::Unit,
            boundary: plan.boundary_machines[0].id,
            arguments: vec![value(1)],
            structural_arguments: Vec::new(),
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        },
        return_operation,
    ];
    plan
}

fn lower(
    plan: &AbstractOperationPlan,
) -> Result<target_operations::TargetOperationPlan, crate::LoweringError> {
    let mut bindings = vec![
        crate::AdmittedBoundarySettlement {
            boundary: plan.boundary_machines[0].id,
            execution: crate::AdmittedBoundaryExecution::CompilerBuiltin(
                target_operations::CompilerBuiltinExecution::HostedWriteByteI32,
            ),
            realization: target_operations::HostedWriteByteI32Realization.into(),
        },
        crate::AdmittedBoundarySettlement {
            boundary: plan.boundary_machines[1].id,
            execution: crate::AdmittedBoundaryExecution::CompilerBuiltin(
                target_operations::CompilerBuiltinExecution::HostedReadByte,
            ),
            realization: target_operations::HostedReadByteRealization.into(),
        },
    ];
    if let Some(exit) = plan.boundary_machines.get(2) {
        bindings.push(crate::AdmittedBoundarySettlement {
            boundary: exit.id,
            execution: crate::AdmittedBoundaryExecution::CompilerBuiltin(
                target_operations::CompilerBuiltinExecution::HostedExitProcessI32,
            ),
            realization: target_operations::HostedExitProcessI32Realization.into(),
        });
    }
    crate::lower_to_target_operations(
        plan,
        crate::TargetLoweringRequest {
            target: NativeTarget::linux_x64(),
            settlements: &bindings,
            installation: None,
            ieee_float_fma: &[],
        },
    )
}

#[test]
fn structural_case_graph_preserves_reordered_blocks_and_ordinary_continuation() {
    let plan = fixture();
    let lowered = lower(&plan).unwrap();
    let graph = &lowered.functions[0].graph;
    assert_eq!(
        graph
            .blocks
            .iter()
            .map(|block| block.block)
            .collect::<Vec<_>>(),
        vec![block(10), block(30), block(20), block(40)]
    );
    let target_operations::TargetControlTerminator::StructuralCase { source, cases } =
        &graph.blocks[0].terminator
    else {
        panic!("structural case")
    };
    assert_eq!(
        source.operation_result().map(|(operation, _)| operation),
        Some(operation(2))
    );
    assert_eq!((cases[0].target, cases[1].target), (block(30), block(20)));
    assert_eq!((cases[0].case_tag, cases[1].case_tag), (0, 1));
    assert_eq!(
        cases[1].payloads[0].parameter,
        target_operations::TargetScalarBlockValue {
            block: block(20),
            value: value(20),
            scalar_type: plan.functions[0].block_entries[2].parameters[0].scalar_type
        }
    );
    assert_eq!(cases[0].trivial_affine_discards, vec![source.place()]);
    assert!(matches!(
        graph.blocks[0].operations[0],
        TargetUnitOperation::IntegerConstant { psi_operation, result, value: IntegerValue::Signed(33), .. } if psi_operation == operation(1) && result == value(1)
    ));
    assert!(
        matches!(graph.blocks[3].terminator, target_operations::TargetControlTerminator::Return { psi_edge, .. } if psi_edge == edge(6))
    );
}

#[test]
fn structural_case_graph_retains_nominal_return_after_exit_and_rejects_later_work() {
    let mut plan = fixture();
    let mut exit = plan.boundary_machines[0].clone();
    exit.id = BoundaryMachineId::new(903).unwrap();
    exit.identity = "Console::exit_process(i32)->Unit".into();
    let AbstractOperation::BoundaryCall { boundary, .. } = &mut plan.functions[0].operations[6]
    else {
        panic!("call")
    };
    *boundary = exit.id;
    plan.boundary_machines.push(exit);
    let lowered = lower(&plan).unwrap();
    let graph = &lowered.functions[0].graph;
    assert!(
        matches!(graph.blocks[3].terminator, target_operations::TargetControlTerminator::Return { psi_edge, .. } if psi_edge == edge(6))
    );
    let mut later_constant = plan.functions[0].operations[0].clone();
    let AbstractOperation::IntegerConstant {
        result,
        psi_operation,
        ..
    } = &mut later_constant
    else {
        panic!("constant")
    };
    *result = value(99);
    *psi_operation = operation(99);
    plan.functions[0].operations.insert(7, later_constant);
    assert!(matches!(
        lower(&plan),
        Err(crate::LoweringError::InvalidHostedExitProcessShape(_))
    ));
}

#[test]
fn structural_case_graph_retains_source_until_later_dispatch_cleanup() {
    let mut plan = fixture();
    let function = &mut plan.functions[0];
    let mut later_dispatch = function.operations[2].clone();
    let AbstractOperation::StructuralCase { cases, .. } = &mut function.operations[2] else {
        panic!("case")
    };
    for case in cases {
        case.trivial_affine_discards.clear();
    }
    let AbstractOperation::StructuralCase { cases, .. } = &mut later_dispatch else {
        panic!("case")
    };
    cases[1].target = block(50);
    cases[1].psi_edge = edge(7);
    cases[1].payloads[0].parameter = value(50);
    cases[0].target = block(60);
    cases[0].psi_edge = edge(8);
    let scalar_type = cases[1].payloads[0].scalar_type;
    function.operations[7] = later_dispatch;
    for (identity, offset) in [(50, 8), (60, 9)] {
        function.block_entries.push(AbstractBlockEntry {
            block: block(identity),
            operation_offset: offset,
            structural_parameters: Vec::new(),
            parameters: if identity == 50 {
                vec![AbstractParameter {
                    value: value(50),
                    scalar_type,
                }]
            } else {
                Vec::new()
            },
        });
        function.operations.push(AbstractOperation::ReturnUnit {
            psi_edge: edge(identity),
            cleanup_actions: Vec::new(),
        });
    }
    let lowered = lower(&plan).unwrap();
    let graph = &lowered.functions[0].graph;
    for (position, expected_count) in [(0, 0), (3, 1)] {
        let target_operations::TargetControlTerminator::StructuralCase { cases, .. } =
            &graph.blocks[position].terminator
        else {
            panic!("case")
        };
        assert!(
            cases
                .iter()
                .all(|case| case.trivial_affine_discards.len() == expected_count)
        );
    }
}

#[test]
fn structural_case_graph_allows_repeated_field_projection() {
    let mut plan = fixture();
    let parameter = plan.functions[0].block_entries[2].parameters[0];
    plan.functions[0].block_entries[2]
        .parameters
        .push(AbstractParameter {
            value: value(21),
            ..parameter
        });
    let AbstractOperation::StructuralCase { cases, .. } = &mut plan.functions[0].operations[2]
    else {
        panic!("case")
    };
    let payload = cases[1].payloads[0];
    cases[1]
        .payloads
        .push(AbstractStructuralCasePayloadBinding {
            parameter: value(21),
            ..payload
        });
    assert!(lower(&plan).is_ok());
}

#[test]
fn structural_case_graph_rejects_changed_payload_telescope_source_and_cleanup() {
    let valid = fixture();
    lower(&valid).expect("the unmodified graph must reach case lowering");
    for mutation in 0..7 {
        let mut plan = valid.clone();
        let AbstractOperation::StructuralCase { source, cases } =
            &mut plan.functions[0].operations[2]
        else {
            panic!("case")
        };
        match mutation {
            0 => cases.swap(0, 1),
            1 => cases[1].payloads[0].parameter = value(99),
            2 => cases[1].payloads[0].scalar_type = ScalarType::Boolean,
            3 => cases[1].payloads[0].field = StructuralFieldId::new(99).unwrap(),
            4 => *source = PlaceId::new(99).unwrap(),
            5 => cases[0].trivial_affine_discards.push(*source),
            _ => cases[1].target = block(30),
        }
        assert!(
            matches!(lower(&plan), Err(crate::LoweringError::UnsupportedControlFlow(machine))
                if machine == valid.entry),
            "case corruption {mutation} must fail graph checking, not boundary admission",
        );
    }
}

#[test]
fn hosted_byte_read_rejects_reordered_result_even_with_matching_dispatch() {
    let mut plan = fixture();
    lower(&plan).expect("the declared EOF/byte result is admitted");
    let StructuralTypeShape::Sum { cases } = &mut plan.structural_types.make_mut()[0].shape else {
        panic!("sum");
    };
    cases.swap(0, 1);
    let AbstractOperation::StructuralCase { cases, .. } = &mut plan.functions[0].operations[2]
    else {
        panic!("case");
    };
    cases.swap(0, 1);
    assert!(matches!(
        lower(&plan),
        Err(crate::LoweringError::BoundaryRealizationMismatch(boundary))
            if boundary == plan.boundary_machines[1].id
    ));
}

fn lower_owned(
    plan: &AbstractOperationPlan,
) -> Result<target_operations::TargetOperationPlan, crate::LoweringError> {
    crate::lower_to_target_operations(
        plan,
        crate::TargetLoweringRequest {
            target: NativeTarget::linux_x64(),
            settlements: &[crate::AdmittedBoundarySettlement {
                boundary: plan.boundary_machines[0].id,
                execution: crate::AdmittedBoundaryExecution::CompilerBuiltin(
                    target_operations::CompilerBuiltinExecution::HostedWriteByteI32,
                ),
                realization: target_operations::HostedWriteByteI32Realization.into(),
            }],
            installation: None,
            ieee_float_fma: &[],
        },
    )
}

fn owned_arrival_fixture() -> AbstractOperationPlan {
    let mut plan = fixture();
    plan.boundary_machines.truncate(1);
    let function = &mut plan.functions[0];
    // The arrival contract is independent of external byte-read realization.
    // Establish an ordinary nominal value before transferring its owned place.
    let AbstractOperation::BoundaryCall {
        psi_operation,
        result: AbstractBoundaryResult::Structural(result),
        ..
    } = function.operations[1].clone()
    else {
        panic!("structural result");
    };
    function.operations[1] = AbstractOperation::EstablishScalarCase {
        psi_operation,
        result,
        result_case: StructuralCaseId::new(1).unwrap(),
        fields: Vec::new(),
    };
    let parameter = terminal_psi::StructuralParameterDeclaration {
        place: PlaceId::new(21).unwrap(),
        position: 0,
        is_self: false,
        structural_type: StructuralTypeId::new(20).unwrap(),
        access: terminal_psi::StructuralAccess::Owned,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let AbstractOperation::StructuralCase { source, cases } = &mut function.operations[2] else {
        panic!("original case dispatch");
    };
    *source = parameter.place;
    for case in cases {
        case.trivial_affine_discards = vec![parameter.place];
    }
    function.operations.insert(
        2,
        AbstractOperation::Jump {
            psi_edge: edge(20),
            target: block(15),
            bindings: Vec::new(),
            structural_bindings: vec![abstract_operations::AbstractStructuralBinding {
                parameter: parameter.place,
                argument: terminal_psi::StructuralArgument {
                    place: PlaceId::new(20).unwrap(),
                    path: Vec::new(),
                    access: terminal_psi::StructuralAccess::Owned,
                },
            }],
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
    );
    for entry in &mut function.block_entries[1..] {
        entry.operation_offset += 1;
    }
    function.block_entries.insert(
        1,
        AbstractBlockEntry {
            block: block(15),
            operation_offset: 3,
            parameters: Vec::new(),
            structural_parameters: vec![parameter],
        },
    );
    plan
}

#[test]
fn owned_sum_arrival_uses_its_actual_block_declaration_and_complete_layout() {
    let source = owned_arrival_fixture();
    let lowered =
        lower_owned(&source).expect("owned result transfers into an observed sum parameter");
    let graph = &lowered.functions[0].graph;
    let target_operations::TargetControlTerminator::StructuralCase { source: home, .. } =
        &graph.blocks[1].terminator
    else {
        panic!("case");
    };
    assert_eq!(
        home.origin,
        target_operations::TargetStructuralHomeOrigin::BlockParameter {
            block: block(15),
            declaration: source.functions[0].block_entries[1].structural_parameters[0].clone(),
        }
    );
    assert!(home.operation_result().is_none());
    assert_eq!(home.place(), PlaceId::new(21).unwrap());
    assert_eq!(home.layout.sum().unwrap().cases.len(), 2);
    let target_operations::TargetControlTerminator::Jump { successor } =
        &graph.blocks[0].terminator
    else {
        panic!("jump");
    };
    let AbstractOperation::Jump {
        structural_bindings,
        ..
    } = &source.functions[0].operations[2]
    else {
        panic!("source jump");
    };
    assert_eq!(&successor.structural_bindings, structural_bindings);
}

#[test]
fn owned_sum_arrival_rejects_substituted_destination_and_unavailable_source() {
    for mutation in 0..7 {
        let mut source = owned_arrival_fixture();
        let function = &mut source.functions[0];
        match mutation {
            0 => {
                function.block_entries[1].structural_parameters[0].multiplicity =
                    StructuralMultiplicity::Unrestricted
            }
            1 => {
                function.block_entries[1].structural_parameters[0].access =
                    terminal_psi::StructuralAccess::SharedBorrow
            }
            2 => {
                function.block_entries[1].structural_parameters[0].structural_type =
                    StructuralTypeId::new(99).unwrap()
            }
            3 => function.block_entries[1].structural_parameters[0].position = 1,
            4 => function.block_entries[1].structural_parameters[0].is_self = true,
            5 => {
                let AbstractOperation::Jump {
                    structural_bindings,
                    ..
                } = &mut function.operations[2]
                else {
                    panic!("jump");
                };
                structural_bindings[0].argument.place = PlaceId::new(99).unwrap();
            }
            _ => {
                let AbstractOperation::Jump {
                    structural_bindings,
                    ..
                } = &mut function.operations[2]
                else {
                    panic!("jump");
                };
                structural_bindings.push(structural_bindings[0].clone());
            }
        }
        assert!(
            lower_owned(&source).is_err(),
            "arrival corruption {mutation}"
        );
    }
}

#[test]
fn owned_sum_diamond_retains_destination_identity_and_rejects_sibling_sources() {
    let mut source = owned_arrival_fixture();
    let function = &mut source.functions[0];
    function.parameters.push(AbstractParameter {
        value: value(90),
        scalar_type: ScalarType::Boolean,
    });
    let first_constructor = function.operations[1].clone();
    let first_jump = function.operations[2].clone();
    let mut second_constructor = first_constructor.clone();
    let AbstractOperation::EstablishScalarCase {
        psi_operation,
        result,
        ..
    } = &mut second_constructor
    else {
        panic!("constructor");
    };
    *psi_operation = operation(90);
    result.place = PlaceId::new(22).unwrap();
    let mut second_jump = first_jump.clone();
    let AbstractOperation::Jump {
        psi_edge,
        structural_bindings,
        ..
    } = &mut second_jump
    else {
        panic!("jump");
    };
    *psi_edge = edge(90);
    structural_bindings[0].argument.place = PlaceId::new(22).unwrap();
    let successor = |identity| abstract_operations::AbstractSuccessor {
        psi_edge: edge(identity),
        target: block(identity),
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    function.operations.splice(
        1..3,
        [
            AbstractOperation::Conditional {
                condition: value(90),
                when_true: successor(11),
                when_false: successor(12),
            },
            first_constructor,
            first_jump,
            second_constructor,
            second_jump,
        ],
    );
    for entry in &mut function.block_entries[1..] {
        entry.operation_offset += 3;
    }
    function.block_entries.splice(
        1..1,
        [(11, 2), (12, 4)].map(|(identity, operation_offset)| AbstractBlockEntry {
            block: block(identity),
            operation_offset,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
        }),
    );
    let lowered =
        lower_owned(&source).expect("two independently established values join one owned home");
    let graph = &lowered.functions[0].graph;
    let target_operations::TargetControlTerminator::StructuralCase { source: home, .. } =
        &graph.blocks[3].terminator
    else {
        panic!("case");
    };
    assert_eq!(
        home.origin,
        target_operations::TargetStructuralHomeOrigin::BlockParameter {
            block: block(15),
            declaration: source.functions[0].block_entries[3].structural_parameters[0].clone(),
        }
    );
    for (operation_index, sibling) in [(3, 22), (5, 20)] {
        let mut forged = source.clone();
        let AbstractOperation::Jump {
            structural_bindings,
            ..
        } = &mut forged.functions[0].operations[operation_index]
        else {
            panic!("jump");
        };
        structural_bindings[0].argument.place = PlaceId::new(sibling).unwrap();
        assert!(
            lower_owned(&forged).is_err(),
            "sibling source {sibling} cannot dominate this edge"
        );
    }
}

#[test]
fn hosted_read_and_write_settlements_replay_and_reject_forged_rows() {
    let plan = fixture();
    let scalar_type = plan.functions[0].block_entries[2].parameters[0].scalar_type;
    let bindings = [
        crate::AdmittedBoundarySettlement {
            boundary: plan.boundary_machines[0].id,
            execution: crate::AdmittedBoundaryExecution::CompilerBuiltin(
                target_operations::CompilerBuiltinExecution::HostedWriteByteI32,
            ),
            realization: target_operations::HostedWriteByteI32Realization.into(),
        },
        crate::AdmittedBoundarySettlement {
            boundary: plan.boundary_machines[1].id,
            execution: crate::AdmittedBoundaryExecution::CompilerBuiltin(
                target_operations::CompilerBuiltinExecution::HostedReadByte,
            ),
            realization: target_operations::HostedReadByteRealization.into(),
        },
    ];
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let lowered = crate::lower_to_target_operations(
            &plan,
            crate::TargetLoweringRequest {
                target: native,
                settlements: &bindings,
                installation: None,
                ieee_float_fma: &[],
            },
        )
        .unwrap();
        crate::validate_abstract_to_target_translation(&plan, native, &lowered)
            .expect("honest hosted read and write settlements replay");

        // The hosted byte read settles its structural result into the
        // independently reconstructed conventional-sum home; every retained
        // coordinate of that row replays.
        for mutation in 0..14 {
            let mut changed = lowered.clone();
            {
                let TargetUnitOperation::BoundarySettlement {
                    psi_operation,
                    boundary,
                    result,
                    execution,
                    realization,
                    scalar_arguments,
                    runtime_scalar_arguments,
                    arguments,
                    completion_claim_sources,
                    completion_receipts,
                    ..
                } = &mut changed.functions[0].graph.blocks[0].operations[1]
                else {
                    panic!("hosted read settlement row")
                };
                match mutation {
                    0 => *psi_operation = OperationId::new(909).unwrap(),
                    1 => *boundary = BoundaryMachineId::new(909).unwrap(),
                    2 => *result = target_operations::TargetBoundaryResult::Unit,
                    3 => {
                        let target_operations::TargetBoundaryResult::Structural(home) = result
                        else {
                            panic!("structural result")
                        };
                        home.layout = target_operations::TargetStructuralHomeLayout::Aggregate(
                            calling_conventions::ValueShape::integer(4, 4),
                        );
                    }
                    4 => {
                        let target_operations::TargetBoundaryResult::Structural(home) = result
                        else {
                            panic!("structural result")
                        };
                        let target_operations::TargetStructuralHomeLayout::Sum(layout) =
                            &mut home.layout
                        else {
                            panic!("sum layout")
                        };
                        layout.tag_byte_offset = 4;
                    }
                    5 => {
                        let target_operations::TargetBoundaryResult::Structural(home) = result
                        else {
                            panic!("structural result")
                        };
                        let target_operations::TargetStructuralHomeOrigin::OperationResult {
                            result: declared,
                            ..
                        } = &mut home.origin
                        else {
                            panic!("operation result origin")
                        };
                        declared.multiplicity = StructuralMultiplicity::Unrestricted;
                    }
                    6 => {
                        let target_operations::TargetBoundaryResult::Structural(home) = result
                        else {
                            panic!("structural result")
                        };
                        let target_operations::TargetStructuralHomeOrigin::OperationResult {
                            result: declared,
                            ..
                        } = &mut home.origin
                        else {
                            panic!("operation result origin")
                        };
                        declared.place = PlaceId::new(909).unwrap();
                    }
                    7 => {
                        *realization = target_operations::BoundaryRealization::HostedWriteByteI32(
                            target_operations::HostedWriteByteI32Realization,
                        )
                    }
                    8 => {
                        *execution = target_operations::BoundaryExecutionBinding::CompilerBuiltin(
                            target_operations::CompilerBuiltinExecution::HostedExitProcessI32,
                        )
                    }
                    9 => runtime_scalar_arguments.push(
                        target_operations::TargetUnitScalarCallArgument {
                            parameter_index: 0,
                            source: target_operations::TargetUnitScalarArgumentSource::Parameter {
                                parameter_index: 0,
                                source_value: value(1),
                                scalar_type,
                            },
                            placement: calling_conventions::ValuePlacement {
                                shape: calling_conventions::ValueShape::integer(4, 4),
                                locations: vec![calling_conventions::ValueLocation::Register {
                                    register: calling_conventions::MachineRegister::X86Rax,
                                    value_byte_offset: 0,
                                    byte_size: 4,
                                }],
                            },
                        },
                    ),
                    10 => arguments.push(terminal_psi::StructuralArgument {
                        place: PlaceId::new(909).unwrap(),
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                    }),
                    11 => scalar_arguments.push(target_operations::BoundaryScalarArgument {
                        source_value: value(1),
                        scalar_type,
                        immediate: IntegerValue::Signed(1),
                        destination: calling_conventions::MachineRegister::X86Rax,
                    }),
                    12 => {
                        completion_claim_sources.push(abstract_operations::CompletionClaimSource {
                            claim: semantic_vocabulary::ClaimId::new(909).unwrap(),
                            entry: None,
                            content: None,
                        })
                    }
                    _ => completion_receipts.push(terminal_psi::CompletionReceipt {
                        claim: semantic_vocabulary::ClaimId::new(909).unwrap(),
                        argument_index: 0,
                    }),
                }
            }
            let forged_key = (mutation == 0).then(|| OperationId::new(909).unwrap());
            assert_eq!(
                crate::validate_abstract_to_target_translation(&plan, native, &changed),
                Err(
                    crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
                        machine: plan.entry,
                        operation: forged_key.unwrap_or_else(|| operation(2)),
                    }
                ),
                "accepted forged read settlement mutation {mutation} on {native:?}"
            );
        }

        // The hosted byte read's `[empty, byte]` contract replays the
        // declaration's own case rows: a carrier missing the empty case or
        // carrying the wrong payload type has no honest settlement.
        for mutation in 0..3 {
            let mut changed = plan.clone();
            let mut declarations = changed.structural_types.to_vec();
            let StructuralTypeShape::Sum { cases } = &mut declarations[0].shape else {
                panic!("sum carrier")
            };
            match mutation {
                0 => {
                    cases.remove(0);
                }
                1 => {
                    cases[1].fields[0].field_type =
                        StructuralFieldType::Scalar(ScalarType::Integer(
                            semantic_vocabulary::IntegerType::new(
                                semantic_vocabulary::IntegerSign::Unsigned,
                                32,
                            )
                            .unwrap(),
                        ));
                }
                _ => cases[1].fields.push(StructuralFieldDeclaration {
                    id: StructuralFieldId::new(2).unwrap(),
                    identity: "extra".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(scalar_type),
                }),
            }
            changed.structural_types = declarations.into();
            // A changed carrier declaration may also fail the earlier
            // structural-type roster replay; either layer rejects it.
            assert!(
                crate::validate_abstract_to_target_translation(&changed, native, &lowered).is_err(),
                "accepted read declaration mutation {mutation}"
            );
        }

        // The block-parameter write forwards its payload through a
        // `BlockParameter` source; every coordinate of that source replays.
        for mutation in 0..4 {
            let mut changed = lowered.clone();
            let TargetUnitOperation::BoundarySettlement {
                runtime_scalar_arguments,
                ..
            } = &mut changed.functions[0].graph.blocks[2].operations[0]
            else {
                panic!("write settlement row")
            };
            match mutation {
                0..=2 => {
                    let target_operations::TargetUnitScalarArgumentSource::BlockParameter(
                        parameter,
                    ) = &mut runtime_scalar_arguments[0].source
                    else {
                        panic!("block parameter source")
                    };
                    match mutation {
                        0 => parameter.block = BlockId::new(909).unwrap(),
                        1 => parameter.value = ValueId::new(909).unwrap(),
                        _ => parameter.scalar_type = ScalarType::Boolean,
                    }
                }
                _ => {
                    runtime_scalar_arguments[0].source =
                        target_operations::TargetUnitScalarArgumentSource::Parameter {
                            parameter_index: 0,
                            source_value: value(20),
                            scalar_type,
                        };
                }
            }
            assert_eq!(
                crate::validate_abstract_to_target_translation(&plan, native, &changed),
                Err(
                    crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
                        machine: plan.entry,
                        operation: operation(3),
                    }
                ),
                "accepted forged block-parameter source mutation {mutation}"
            );
        }

        // The continuation write forwards the constant through an
        // `IntegerImmediate` source bound to its defining operation.
        let mut changed = lowered.clone();
        let TargetUnitOperation::BoundarySettlement {
            runtime_scalar_arguments,
            ..
        } = &mut changed.functions[0].graph.blocks[3].operations[0]
        else {
            panic!("write settlement row")
        };
        let target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
            defining_operation,
            ..
        } = &mut runtime_scalar_arguments[0].source
        else {
            panic!("immediate source")
        };
        *defining_operation = OperationId::new(909).unwrap();
        assert_eq!(
            crate::validate_abstract_to_target_translation(&plan, native, &changed),
            Err(
                crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
                    machine: plan.entry,
                    operation: operation(4),
                }
            ),
            "accepted a forged immediate defining operation"
        );
    }
}
