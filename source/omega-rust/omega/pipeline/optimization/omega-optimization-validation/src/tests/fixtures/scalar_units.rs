//! Minimal scalar and call fixture units.

use super::*;

pub(crate) fn unit() -> PsiOptimizationUnit {
    let machine = id(1, MachineId::new);
    let block = id(2, BlockId::new);
    let result = id(3, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("valid width");
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([11; 32]),
        },
        entry: machine,
        structural_types: Vec::new(),
        placed_view_inputs: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: result,
                scalar_type: ScalarType::Integer(integer),
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::IntegerConstant {
                    psi_operation: id(4, OperationId::new),
                    result,
                    scalar_type: ScalarType::Integer(integer),
                    value: IntegerValue::Unsigned(7),
                },
                AbstractOperation::Return {
                    psi_edge: id(5, EdgeId::new),
                    result,
                    value: result,
                    scalar_type: ScalarType::Integer(integer),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    };
    reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .expect("valid unit")
}

fn write_only_store_plan(store_before_value: bool) -> AbstractOperationPlan {
    let machine = id(51, MachineId::new);
    let block = id(52, BlockId::new);
    let value = id(53, ValueId::new);
    let place = id(54, PlaceId::new);
    let structural_type = id(55, StructuralTypeId::new);
    let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let destination = psi_terminal::StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: psi_terminal::StructuralMultiplicity::Unrestricted,
        access: psi_terminal::StructuralAccess::WriteOnlyBorrow,
        qualifications: Vec::new(),
    };
    let constant = AbstractOperation::IntegerConstant {
        psi_operation: id(56, OperationId::new),
        result: value,
        scalar_type,
        value: IntegerValue::Signed(2),
    };
    let store = AbstractOperation::WriteOnlyPrimitiveStore {
        psi_operation: id(57, OperationId::new),
        destination: destination.clone(),
        value: AbstractResult { value, scalar_type },
    };
    let mut operations = if store_before_value {
        vec![store, constant]
    } else {
        vec![constant, store]
    };
    operations.push(AbstractOperation::ReturnUnit {
        psi_edge: id(58, EdgeId::new),
        cleanup_actions: Vec::new(),
    });
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([51; 32]),
        },
        entry: machine,
        structural_types: vec![psi_terminal::StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::i32".into(),
            shape: psi_terminal::StructuralTypeShape::PrimitiveScalar(scalar_type),
        }],
        placed_view_inputs: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: vec![destination],
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations,
        }],
    }
}

pub(crate) fn write_only_store_unit() -> PsiOptimizationUnit {
    reconstruct_psi_optimization_unit_seed(
        &write_only_store_plan(false),
        TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("valid write-only store fixture")
}

pub(crate) fn write_only_store_before_value_unit() -> PsiOptimizationUnit {
    reconstruct_psi_optimization_unit_seed(
        &write_only_store_plan(true),
        TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("structural reconstruction permits independent dominance validation")
}

pub(crate) fn exact_add_unit() -> PsiOptimizationUnit {
    let machine = id(201, MachineId::new);
    let block = id(202, BlockId::new);
    let left = id(203, ValueId::new);
    let right = id(204, ValueId::new);
    let result = id(205, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([12; 32]),
        },
        entry: machine,
        structural_types: Vec::new(),
        placed_view_inputs: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: result,
                scalar_type: ScalarType::Integer(integer),
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::IntegerConstant {
                    psi_operation: id(206, OperationId::new),
                    result: left,
                    scalar_type: ScalarType::Integer(integer),
                    value: IntegerValue::Unsigned(7),
                },
                AbstractOperation::IntegerConstant {
                    psi_operation: id(207, OperationId::new),
                    result: right,
                    scalar_type: ScalarType::Integer(integer),
                    value: IntegerValue::Unsigned(8),
                },
                AbstractOperation::ExactIntegerAdd {
                    psi_operation: id(208, OperationId::new),
                    obligation: id(209, psi_core::ObligationId::new),
                    result,
                    scalar_type: integer,
                    left,
                    right,
                },
                AbstractOperation::Return {
                    psi_edge: id(210, EdgeId::new),
                    result,
                    value: result,
                    scalar_type: ScalarType::Integer(integer),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    };
    let unit = reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap())
        .unwrap();
    omega_optimization_unit::attach_accepted_obligation_facts(
        unit.clone(),
        vec![omega_optimization_unit::AcceptedObligationFact::new(
            unit.psi,
            [23; 32],
            machine,
            id(208, OperationId::new),
            id(209, psi_core::ObligationId::new),
            b"validation-test-obligation".to_vec(),
        )],
    )
    .unwrap()
}

pub(crate) fn scalar_call_unit() -> PsiOptimizationUnit {
    let caller = id(301, MachineId::new);
    let callee = id(302, MachineId::new);
    let caller_block = id(303, BlockId::new);
    let callee_block = id(304, BlockId::new);
    let argument = id(305, ValueId::new);
    let caller_result = id(306, ValueId::new);
    let parameter = id(307, ValueId::new);
    let callee_result = id(308, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([13; 32]),
        },
        entry: caller,
        structural_types: Vec::new(),
        placed_view_inputs: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller,
                attachment: None,
                entry: caller_block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: caller_result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: caller_block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: id(309, OperationId::new),
                        result: argument,
                        scalar_type,
                        value: IntegerValue::Unsigned(7),
                    },
                    AbstractOperation::Call {
                        psi_operation: id(310, OperationId::new),
                        result: caller_result,
                        scalar_type,
                        callee,
                        arguments: vec![argument],
                    },
                    AbstractOperation::Return {
                        psi_edge: id(311, EdgeId::new),
                        result: caller_result,
                        value: caller_result,
                        scalar_type,
                        cleanup_actions: Vec::new(),
                    },
                ],
            },
            AbstractFunction {
                machine: callee,
                attachment: None,
                entry: callee_block,
                parameters: vec![AbstractParameter {
                    value: parameter,
                    scalar_type,
                }],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: callee_result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: callee_block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![AbstractOperation::Return {
                    psi_edge: id(312, EdgeId::new),
                    result: callee_result,
                    value: parameter,
                    scalar_type,
                    cleanup_actions: Vec::new(),
                }],
            },
        ],
    };
    reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap()).unwrap()
}

pub(crate) fn scalar_boundary_call_unit() -> PsiOptimizationUnit {
    let machine = id(321, MachineId::new);
    let boundary = id(322, BoundaryMachineId::new);
    let block = id(323, BlockId::new);
    let argument = id(324, ValueId::new);
    let result = id(325, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([14; 32]),
        },
        entry: machine,
        structural_types: Vec::new(),
        placed_view_inputs: Vec::new(),
        boundary_machines: vec![psi_terminal::BoundaryMachineDeclaration {
            id: boundary,
            identity: "validation::scalar-boundary".into(),
            attachment: None,
            scalar_parameters: vec![scalar_type],
            structural_parameters: Vec::new(),
            result: Some(scalar_type),
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }],
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: result,
                scalar_type,
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::IntegerConstant {
                    psi_operation: id(326, OperationId::new),
                    result: argument,
                    scalar_type,
                    value: IntegerValue::Unsigned(7),
                },
                AbstractOperation::BoundaryCall {
                    psi_operation: id(327, OperationId::new),
                    result: Some(AbstractResult {
                        value: result,
                        scalar_type,
                    }),
                    boundary,
                    arguments: vec![argument],
                    structural_arguments: Vec::new(),
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                },
                AbstractOperation::Return {
                    psi_edge: id(328, EdgeId::new),
                    result,
                    value: result,
                    scalar_type,
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    };
    reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap()).unwrap()
}

pub(crate) fn structural_call_unit() -> PsiOptimizationUnit {
    let caller = id(331, MachineId::new);
    let callee = id(332, MachineId::new);
    let caller_block = id(333, BlockId::new);
    let callee_block = id(334, BlockId::new);
    let caller_place = id(335, PlaceId::new);
    let callee_place = id(336, PlaceId::new);
    let structural_type = id(337, psi_core::StructuralTypeId::new);
    let parameter = |place, position| psi_terminal::StructuralParameterDeclaration {
        place,
        position,
        is_self: false,
        structural_type,
        multiplicity: psi_terminal::StructuralMultiplicity::Unrestricted,
        access: psi_terminal::StructuralAccess::Owned,
        qualifications: Vec::new(),
    };
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([15; 32]),
        },
        entry: caller,
        structural_types: vec![psi_terminal::StructuralTypeDeclaration {
            id: structural_type,
            identity: "validation::structural-call-argument".into(),
            shape: psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ),
        }],
        placed_view_inputs: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller,
                attachment: None,
                entry: caller_block,
                parameters: Vec::new(),
                structural_parameters: vec![parameter(caller_place, 0)],
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: caller_block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::CallUnit {
                        psi_operation: id(338, OperationId::new),
                        callee,
                        structural_arguments: vec![psi_terminal::StructuralArgument {
                            place: caller_place,
                            path: Vec::new(),
                            access: psi_terminal::StructuralAccess::Owned,
                        }],
                        claim_transfers: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(339, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    },
                ],
            },
            AbstractFunction {
                machine: callee,
                attachment: None,
                entry: callee_block,
                parameters: Vec::new(),
                structural_parameters: vec![parameter(callee_place, 0)],
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: callee_block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![AbstractOperation::ReturnUnit {
                    psi_edge: id(340, EdgeId::new),
                    cleanup_actions: Vec::new(),
                }],
            },
        ],
    };
    reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap()).unwrap()
}
