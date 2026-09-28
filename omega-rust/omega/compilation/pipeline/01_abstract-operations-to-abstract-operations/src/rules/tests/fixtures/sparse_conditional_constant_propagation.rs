//! SCCP fixture programs.

use super::{id, with_synthetic_accepted_obligations};
use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperation as O, AbstractOperationPlan, AbstractResult,
};
use optimization_unit::{PsiOptimizationUnit, reconstruct_psi_optimization_unit_seed};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    ObligationId, OperationId, ScalarType, ValueId,
};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};
mod binary;
mod boolean;
mod range;
mod unary;

pub(crate) use binary::{BinaryConstantFixtureKind, binary_constant_unit};
pub(crate) use boolean::{
    BooleanFixtureKind, ComparisonFixtureKind, boolean_constant_unit,
    integer_comparison_constant_unit,
};
pub(crate) use range::{
    ProofRangeKind, range_constant_comparison_unit, range_pair_comparison_unit,
};
pub(crate) use unary::{UnaryConstantFixtureKind, unary_constant_unit};

pub(crate) fn exact_cast_unit(value: u128) -> PsiOptimizationUnit {
    let machine = id(321, MachineId::new);
    let block = id(322, BlockId::new);
    let operand = id(323, ValueId::new);
    let result = id(324, ValueId::new);
    let source_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let target_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([14; 32]),
            },
            entry: machine,
            structural_types: Vec::new().into(),
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
                    scalar_type: ScalarType::Integer(target_type),
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: id(325, OperationId::new),
                        result: operand,
                        scalar_type: ScalarType::Integer(source_type),
                        value: IntegerValue::Unsigned(value),
                    },
                    AbstractOperation::IntegerExactCast {
                        psi_operation: id(326, OperationId::new),
                        obligation: id(327, ObligationId::new),
                        result,
                        source_type,
                        target_type,
                        operand,
                    },
                    AbstractOperation::Return {
                        psi_edge: id(328, EdgeId::new),
                        result,
                        value: result,
                        scalar_type: ScalarType::Integer(target_type),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    with_synthetic_accepted_obligations(unit)
}
