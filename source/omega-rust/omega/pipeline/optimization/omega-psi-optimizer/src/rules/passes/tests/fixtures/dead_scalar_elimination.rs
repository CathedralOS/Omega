//! Dead-scalar fixture programs.

use super::*;

pub(crate) fn dead_scalar_literals_unit() -> PsiOptimizationUnit {
    let machine = id(1_201, MachineId::new);
    let block = id(1_202, BlockId::new);
    let boolean = id(1_203, ValueId::new);
    let integer_value = id(1_204, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([39; 32]),
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
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::BooleanConstant {
                        psi_operation: id(1_205, OperationId::new),
                        result: boolean,
                        value: true,
                    },
                    AbstractOperation::IntegerConstant {
                        psi_operation: id(1_206, OperationId::new),
                        result: integer_value,
                        scalar_type: ScalarType::Integer(integer),
                        value: psi_core::IntegerValue::Unsigned(7),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(1_207, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

pub(crate) fn dead_wrapping_add_unit() -> PsiOptimizationUnit {
    let machine = id(1_211, MachineId::new);
    let block = id(1_212, BlockId::new);
    let left = id(1_213, ValueId::new);
    let right = id(1_214, ValueId::new);
    let sum = id(1_215, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([40; 32]),
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
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: id(1_216, OperationId::new),
                        result: left,
                        scalar_type: ScalarType::Integer(integer),
                        value: IntegerValue::Unsigned(250),
                    },
                    AbstractOperation::IntegerConstant {
                        psi_operation: id(1_217, OperationId::new),
                        result: right,
                        scalar_type: ScalarType::Integer(integer),
                        value: IntegerValue::Unsigned(10),
                    },
                    AbstractOperation::WrappingIntegerAdd {
                        psi_operation: id(1_218, OperationId::new),
                        result: sum,
                        scalar_type: integer,
                        left,
                        right,
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(1_219, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

pub(crate) fn dead_exact_add_unit() -> PsiOptimizationUnit {
    discard_scalar_function_result(exact_add_unit())
}

pub(crate) fn discard_scalar_function_result(mut unit: PsiOptimizationUnit) -> PsiOptimizationUnit {
    let return_node = unit.functions[0].blocks[0]
        .nodes
        .last_mut()
        .expect("fixture has a return node");
    let O::Return {
        psi_edge,
        cleanup_actions,
        ..
    } = &return_node.operation
    else {
        unreachable!()
    };
    return_node.operation = O::ReturnUnit {
        psi_edge: *psi_edge,
        cleanup_actions: cleanup_actions.clone(),
    };
    return_node.uses.clear();
    unit.functions[0].result = AbstractFunctionResult::Unit;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    unit
}
