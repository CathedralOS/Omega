use super::super::{id, with_synthetic_accepted_obligations};
use omega_abstract_operations::AbstractOperation as O;
use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult,
};
use omega_optimization_unit::{
    OptimizationFact, PsiOptimizationUnit, recompute_psi_optimization_unit_identity,
    reconstruct_psi_optimization_unit_seed,
};
use omega_optimization_validation::validate_psi_optimization_unit;
use psi_core::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, ObligationId,
    OperationId, ScalarType, ValueId,
};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

pub(crate) fn local_cse_unit() -> PsiOptimizationUnit {
    scalar_local_cse_unit(false)
}

pub(crate) fn proof_certified_local_cse_unit() -> PsiOptimizationUnit {
    scalar_local_cse_unit(true)
}

pub(crate) fn compatible_policy_local_cse_unit() -> PsiOptimizationUnit {
    let mut unit = proof_certified_local_cse_unit();
    let node = &mut unit.functions[0].blocks[0].nodes[0];
    let O::ExactIntegerAdd {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
        ..
    } = node.operation
    else {
        unreachable!("proof CSE leader is exact add")
    };
    node.operation = O::WrappingIntegerAdd {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
    };
    unit.functions[0].facts.retain(|fact| {
        !matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                if *support == psi_operation)
    });
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    validate_psi_optimization_unit(&unit).unwrap();
    unit
}

pub(crate) fn scalar_local_cse_unit(proof_certified: bool) -> PsiOptimizationUnit {
    let machine = id(1_301, MachineId::new);
    let block = id(1_302, BlockId::new);
    let left = id(1_303, ValueId::new);
    let right = id(1_304, ValueId::new);
    let leader = id(1_305, ValueId::new);
    let redundant = id(1_306, ValueId::new);
    let equal = id(1_307, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let binary = |psi_operation, obligation, result, left, right| {
        if proof_certified {
            AbstractOperation::ExactIntegerAdd {
                psi_operation,
                obligation,
                result,
                scalar_type: integer,
                left,
                right,
            }
        } else {
            AbstractOperation::WrappingIntegerAdd {
                psi_operation,
                result,
                scalar_type: integer,
                left,
                right,
            }
        }
    };
    let unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([41; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: vec![
                    AbstractParameter {
                        value: left,
                        scalar_type: ScalarType::Integer(integer),
                    },
                    AbstractParameter {
                        value: right,
                        scalar_type: ScalarType::Integer(integer),
                    },
                ],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: equal,
                    scalar_type: ScalarType::Boolean,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    binary(
                        id(1_308, OperationId::new),
                        id(1_312, ObligationId::new),
                        leader,
                        left,
                        right,
                    ),
                    binary(
                        id(1_309, OperationId::new),
                        id(1_313, ObligationId::new),
                        redundant,
                        right,
                        left,
                    ),
                    AbstractOperation::IntegerEqual {
                        psi_operation: id(1_310, OperationId::new),
                        result: equal,
                        left: leader,
                        right: redundant,
                    },
                    AbstractOperation::Return {
                        psi_edge: id(1_311, EdgeId::new),
                        result: equal,
                        value: equal,
                        scalar_type: ScalarType::Boolean,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    if proof_certified {
        with_synthetic_accepted_obligations(unit)
    } else {
        unit
    }
}
