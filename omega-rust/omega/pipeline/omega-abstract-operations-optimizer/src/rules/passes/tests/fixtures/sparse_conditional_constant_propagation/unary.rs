//! Unary integer constant-evaluation fixtures by exact operation identity.

use super::*;

#[derive(Debug, Clone, Copy)]
pub(crate) enum UnaryConstantFixtureKind {
    ExactCast,
    Widen,
    BitwiseNot,
}

impl UnaryConstantFixtureKind {
    pub(crate) fn proof_certified(self) -> bool {
        matches!(self, Self::ExactCast)
    }

    fn operation(
        self,
        psi_operation: OperationId,
        obligation: ObligationId,
        result: ValueId,
        source_type: IntegerType,
        target_type: IntegerType,
        operand: ValueId,
    ) -> AbstractOperation {
        match self {
            Self::ExactCast => O::IntegerExactCast {
                psi_operation,
                obligation,
                result,
                source_type,
                target_type,
                operand,
            },
            Self::Widen => O::IntegerWiden {
                psi_operation,
                result,
                source_type,
                target_type,
                operand,
            },
            Self::BitwiseNot => O::IntegerBitwiseNot {
                psi_operation,
                result,
                scalar_type: source_type,
                operand,
            },
        }
    }
}

pub(crate) fn unary_constant_unit(
    kind: UnaryConstantFixtureKind,
    source_type: IntegerType,
    target_type: IntegerType,
    constant: IntegerValue,
) -> PsiOptimizationUnit {
    let machine = id(361, MachineId::new);
    let block = id(362, BlockId::new);
    let operand = id(363, ValueId::new);
    let result = id(364, ValueId::new);
    let operation = kind.operation(
        id(366, OperationId::new),
        id(367, ObligationId::new),
        result,
        source_type,
        target_type,
        operand,
    );
    let unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([36; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
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
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: id(365, OperationId::new),
                        result: operand,
                        scalar_type: ScalarType::Integer(source_type),
                        value: constant,
                    },
                    operation,
                    AbstractOperation::Return {
                        psi_edge: id(368, EdgeId::new),
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
