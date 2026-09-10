//! Case targets are source blocks, including returning arms and continuations.
use super::*;
use abstract_operations::{
    AbstractBoundaryResult, AbstractStructuralCasePayloadBinding, AbstractStructuralCaseSuccessor,
};
use semantic_vocabulary::{PlaceId, StructuralCaseId, StructuralFieldId, StructuralTypeId};
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
                    identity: "Byte".into(),
                    fields: vec![StructuralFieldDeclaration {
                        id: field,
                        identity: "value".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(scalar_type),
                    }],
                },
                StructuralCaseDeclaration {
                    id: StructuralCaseId::new(2).unwrap(),
                    identity: "End".into(),
                    fields: Vec::new(),
                },
            ],
        },
    }];
    let mut read = plan.boundary_machines[0].clone();
    read.id = BoundaryMachineId::new(902).unwrap();
    read.identity = "Console::read_byte()->ByteRead".into();
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
                    psi_edge: edge(1),
                    target: block(20),
                    case: StructuralCaseId::new(1).unwrap(),
                    payloads: vec![AbstractStructuralCasePayloadBinding {
                        parameter: value(20),
                        field,
                        scalar_type,
                    }],
                    trivial_affine_discards: vec![source],
                },
                AbstractStructuralCaseSuccessor {
                    psi_edge: edge(2),
                    target: block(30),
                    case: StructuralCaseId::new(2).unwrap(),
                    payloads: Vec::new(),
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
    crate::lower_to_target_operations_with_provider_executions(
        plan,
        NativeTarget::linux_x64(),
        &bindings,
    )
}

#[test]
fn structural_case_graph_preserves_reordered_blocks_and_ordinary_continuation() {
    let plan = fixture();
    let lowered = lower(&plan).unwrap();
    let TargetOperation::ControlGraph(graph) = &lowered.functions[0].operation else {
        panic!("ordinary graph")
    };
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
    assert_eq!((cases[0].target, cases[1].target), (block(20), block(30)));
    assert_eq!(
        cases[0].payloads[0].parameter,
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
    let TargetOperation::ControlGraph(graph) = &lowered.functions[0].operation else {
        panic!("graph")
    };
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
    cases[0].target = block(50);
    cases[0].psi_edge = edge(7);
    cases[0].payloads[0].parameter = value(50);
    cases[1].target = block(60);
    cases[1].psi_edge = edge(8);
    let scalar_type = cases[0].payloads[0].scalar_type;
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
    let TargetOperation::ControlGraph(graph) = &lowered.functions[0].operation else {
        panic!("graph")
    };
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
    let payload = cases[0].payloads[0];
    cases[0]
        .payloads
        .push(AbstractStructuralCasePayloadBinding {
            parameter: value(21),
            ..payload
        });
    assert!(lower(&plan).is_ok());
}

#[test]
fn structural_case_graph_rejects_changed_payload_telescope_source_and_cleanup() {
    for mutation in 0..7 {
        let mut plan = fixture();
        let AbstractOperation::StructuralCase { source, cases } =
            &mut plan.functions[0].operations[2]
        else {
            panic!("case")
        };
        match mutation {
            0 => cases.swap(0, 1),
            1 => cases[0].payloads[0].parameter = value(99),
            2 => cases[0].payloads[0].scalar_type = ScalarType::Boolean,
            3 => cases[0].payloads[0].field = StructuralFieldId::new(99).unwrap(),
            4 => *source = PlaceId::new(99).unwrap(),
            5 => cases[0].trivial_affine_discards.push(*source),
            _ => cases[0].target = block(30),
        }
        assert!(lower(&plan).is_err(), "corruption {mutation}");
    }
}

fn lower_owned(
    plan: &AbstractOperationPlan,
) -> Result<target_operations::TargetOperationPlan, crate::LoweringError> {
    crate::lower_to_target_operations_with_provider_executions(
        plan,
        NativeTarget::linux_x64(),
        &[crate::AdmittedBoundarySettlement {
            boundary: plan.boundary_machines[0].id,
            execution: crate::AdmittedBoundaryExecution::CompilerBuiltin(
                target_operations::CompilerBuiltinExecution::HostedWriteByteI32,
            ),
            realization: target_operations::HostedWriteByteI32Realization.into(),
        }],
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
        result_case: StructuralCaseId::new(2).unwrap(),
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
    let TargetOperation::ControlGraph(graph) = &lowered.functions[0].operation else {
        panic!("graph");
    };
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
    let TargetOperation::ControlGraph(graph) = &lowered.functions[0].operation else {
        panic!("graph");
    };
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
