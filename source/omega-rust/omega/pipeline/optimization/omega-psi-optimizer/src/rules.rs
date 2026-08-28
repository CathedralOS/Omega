use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, Optimization, OptimizationExecutionPhase,
    OptimizationPassIdentity, OptimizationRuleContract, OptimizationRuleIdentity,
    OptimizationSafetyClass, OptimizationSelections, ScalarConstantFactIdentity,
};
use omega_optimization_unit::{
    AdjacentBlockMergeRewrite, BlockParameterIncomingBinding, BooleanConstantRewrite,
    ConstantConditionalRewrite, DeadScalarNodeRewrite, DominatingScalarCommonSubexpressionRewrite,
    IntegerConstantRewrite, IntegerEvaluationWitness, LinearEmptyBlockRewrite,
    LocalScalarCommonSubexpressionRewrite, NodeLocation, NonAdjacentBlockMergeRewrite,
    OptimizationFact, OwnershipFrontierSite, OwnershipFrontierWitness, OwnershipFrontierWitnessRow,
    PathQualifiedEmptyBlockRewrite, PhiTranslatedScalarGvnRewrite, PhiTranslatedScalarIncoming,
    ProofCertifiedScalarIdentityKind, ProofCertifiedScalarIdentityRewrite, ProvenanceDisposition,
    ProvenanceRewrite, PrunedMachineCustody, PsiOptimizationUnit, PsiProvenance,
    PsiRealizationSite, PsiRewriteCandidate, RedundantBlockParameterRewrite,
    RedundantBlockParameterWitness, ScalarSubstitution, SharedJumpFusionRewrite,
    UnreachablePrivateMachinesRewrite, ValueRangeSupport,
};
use psi_core::{
    BlockId, IntegerCarrier, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, ValueId,
};

use crate::{
    AnalysisProduct, OrderedRuleRegistry, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
    RuleRegistryError, ScalarConstant, ScalarConstantAnalysis, ValueRangeAnalysis,
};

const SCCP_PASS_NAME: &[u8] = b"omega.psi-pass.sparse-conditional-constant-propagation.v4";
const CONTROL_FLOW_CLEANUP_PASS_NAME: &[u8] = b"omega.psi-pass.control-flow-cleanup.v12";
const COPY_PROPAGATION_PASS_NAME: &[u8] = b"omega.psi-pass.copy-propagation.v1";
const DEAD_PURE_SCALAR_PASS_NAME: &[u8] = b"omega.psi-pass.dead-pure-scalar-elimination.v2";
const PROOF_CHECK_ELISION_PASS_NAME: &[u8] = b"omega.psi-pass.proof-check-elision.v11";
const GLOBAL_VALUE_NUMBERING_PASS_NAME: &[u8] = b"omega.psi-pass.global-value-numbering.v7";

#[derive(Debug, Clone, Copy, Default)]
pub struct ConstantConditionalFoldRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct UnreachablePrivateMachinePruneRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct DeadScalarLiteralEliminationRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct DeadUnconditionallyTotalScalarEliminationRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct ProofCertifiedDeadScalarEliminationRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedIntegerIdentityEliminationRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedIntegerDivideByOneEliminationRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedIntegerZeroDividendEliminationRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedExactIntegerSelfSubtractEliminationRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedIntegerSelfRemainderEliminationRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedIntegerSelfDivideEliminationRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedIntegerRemainderByOneEliminationRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct SameBlockTotalScalarCseRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct DominatorTotalScalarGvnRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct SameBlockProofCertifiedScalarCseRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct DominatorProofCertifiedScalarGvnRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct SameBlockProofCertifiedCompatiblePolicyScalarCseRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct DominatorProofCertifiedCompatiblePolicyScalarGvnRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct PhiTranslatedObligationFreeScalarGvnRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct PhiTranslatedProofCertifiedScalarGvnRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TotalScalarExpressionKey {
    BooleanConstant(bool),
    IntegerConstant(ScalarType, IntegerValue),
    BooleanNot(ValueId),
    BooleanEqual(ValueId, ValueId),
    IntegerEqual(IntegerType, ValueId, ValueId),
    IntegerLessThan(IntegerType, ValueId, ValueId),
    IntegerLessOrEqual(IntegerType, ValueId, ValueId),
    IntegerBitwiseNot(IntegerType, ValueId),
    IntegerWiden(IntegerType, IntegerType, ValueId),
    IntegerBitwiseAnd(IntegerType, ValueId, ValueId),
    IntegerBitwiseOr(IntegerType, ValueId, ValueId),
    IntegerBitwiseXor(IntegerType, ValueId, ValueId),
    WrappingShiftLeft(IntegerType, IntegerType, ValueId, ValueId),
    WrappingShiftRight(IntegerType, IntegerType, ValueId, ValueId),
    WrappingAdd(IntegerType, ValueId, ValueId),
    WrappingSubtract(IntegerType, ValueId, ValueId),
    WrappingMultiply(IntegerType, ValueId, ValueId),
    SaturatingAdd(IntegerType, ValueId, ValueId),
    SaturatingSubtract(IntegerType, ValueId, ValueId),
    SaturatingMultiply(IntegerType, ValueId, ValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum CompatiblePolicyScalarExpressionKey {
    ShiftLeft(IntegerType, IntegerType, ValueId, ValueId),
    ShiftRight(IntegerType, IntegerType, ValueId, ValueId),
    Add(IntegerType, ValueId, ValueId),
    Subtract(IntegerType, ValueId, ValueId),
    Multiply(IntegerType, ValueId, ValueId),
}

impl CompatiblePolicyScalarExpressionKey {
    fn references_any(self, values: &BTreeSet<ValueId>) -> bool {
        match self {
            Self::ShiftLeft(_, _, left, right)
            | Self::ShiftRight(_, _, left, right)
            | Self::Add(_, left, right)
            | Self::Subtract(_, left, right)
            | Self::Multiply(_, left, right) => values.contains(&left) || values.contains(&right),
        }
    }

    fn translate(self, values: &BTreeMap<ValueId, ValueId>) -> Self {
        let value = |operand: ValueId| values.get(&operand).copied().unwrap_or(operand);
        match self {
            Self::ShiftLeft(value_type, count_type, operand, count) => {
                Self::ShiftLeft(value_type, count_type, value(operand), value(count))
            }
            Self::ShiftRight(value_type, count_type, operand, count) => {
                Self::ShiftRight(value_type, count_type, value(operand), value(count))
            }
            Self::Add(scalar_type, left, right) => {
                let (left, right) = canonical_pair(value(left), value(right));
                Self::Add(scalar_type, left, right)
            }
            Self::Subtract(scalar_type, left, right) => {
                Self::Subtract(scalar_type, value(left), value(right))
            }
            Self::Multiply(scalar_type, left, right) => {
                let (left, right) = canonical_pair(value(left), value(right));
                Self::Multiply(scalar_type, left, right)
            }
        }
    }
}

impl TotalScalarExpressionKey {
    fn references_any(self, values: &BTreeSet<ValueId>) -> bool {
        match self {
            Self::BooleanConstant(_) | Self::IntegerConstant(_, _) => false,
            Self::BooleanNot(value)
            | Self::IntegerBitwiseNot(_, value)
            | Self::IntegerWiden(_, _, value) => values.contains(&value),
            Self::BooleanEqual(left, right)
            | Self::IntegerEqual(_, left, right)
            | Self::IntegerLessThan(_, left, right)
            | Self::IntegerLessOrEqual(_, left, right)
            | Self::IntegerBitwiseAnd(_, left, right)
            | Self::IntegerBitwiseOr(_, left, right)
            | Self::IntegerBitwiseXor(_, left, right)
            | Self::WrappingAdd(_, left, right)
            | Self::WrappingSubtract(_, left, right)
            | Self::WrappingMultiply(_, left, right)
            | Self::SaturatingAdd(_, left, right)
            | Self::SaturatingSubtract(_, left, right)
            | Self::SaturatingMultiply(_, left, right) => {
                values.contains(&left) || values.contains(&right)
            }
            Self::WrappingShiftLeft(_, _, value, count)
            | Self::WrappingShiftRight(_, _, value, count) => {
                values.contains(&value) || values.contains(&count)
            }
        }
    }

    fn translate(self, values: &BTreeMap<ValueId, ValueId>) -> Option<Self> {
        let value = |operand: ValueId| Some(values.get(&operand).copied().unwrap_or(operand));
        let commutative = |left: ValueId, right: ValueId| {
            let left = value(left)?;
            let right = value(right)?;
            Some(canonical_pair(left, right))
        };
        Some(match self {
            Self::BooleanConstant(constant) => Self::BooleanConstant(constant),
            Self::IntegerConstant(scalar_type, constant) => {
                Self::IntegerConstant(scalar_type, constant)
            }
            Self::BooleanNot(operand) => Self::BooleanNot(value(operand)?),
            Self::BooleanEqual(left, right) => {
                let (left, right) = commutative(left, right)?;
                Self::BooleanEqual(left, right)
            }
            Self::IntegerEqual(scalar_type, left, right) => {
                let (left, right) = commutative(left, right)?;
                Self::IntegerEqual(scalar_type, left, right)
            }
            Self::IntegerLessThan(scalar_type, left, right) => {
                Self::IntegerLessThan(scalar_type, value(left)?, value(right)?)
            }
            Self::IntegerLessOrEqual(scalar_type, left, right) => {
                Self::IntegerLessOrEqual(scalar_type, value(left)?, value(right)?)
            }
            Self::IntegerBitwiseNot(scalar_type, operand) => {
                Self::IntegerBitwiseNot(scalar_type, value(operand)?)
            }
            Self::IntegerWiden(source_type, target_type, operand) => {
                Self::IntegerWiden(source_type, target_type, value(operand)?)
            }
            Self::IntegerBitwiseAnd(scalar_type, left, right) => {
                let (left, right) = commutative(left, right)?;
                Self::IntegerBitwiseAnd(scalar_type, left, right)
            }
            Self::IntegerBitwiseOr(scalar_type, left, right) => {
                let (left, right) = commutative(left, right)?;
                Self::IntegerBitwiseOr(scalar_type, left, right)
            }
            Self::IntegerBitwiseXor(scalar_type, left, right) => {
                let (left, right) = commutative(left, right)?;
                Self::IntegerBitwiseXor(scalar_type, left, right)
            }
            Self::WrappingShiftLeft(value_type, count_type, operand, count) => {
                Self::WrappingShiftLeft(value_type, count_type, value(operand)?, value(count)?)
            }
            Self::WrappingShiftRight(value_type, count_type, operand, count) => {
                Self::WrappingShiftRight(value_type, count_type, value(operand)?, value(count)?)
            }
            Self::WrappingAdd(scalar_type, left, right) => {
                let (left, right) = commutative(left, right)?;
                Self::WrappingAdd(scalar_type, left, right)
            }
            Self::WrappingSubtract(scalar_type, left, right) => {
                Self::WrappingSubtract(scalar_type, value(left)?, value(right)?)
            }
            Self::WrappingMultiply(scalar_type, left, right) => {
                let (left, right) = commutative(left, right)?;
                Self::WrappingMultiply(scalar_type, left, right)
            }
            Self::SaturatingAdd(scalar_type, left, right) => {
                let (left, right) = commutative(left, right)?;
                Self::SaturatingAdd(scalar_type, left, right)
            }
            Self::SaturatingSubtract(scalar_type, left, right) => {
                Self::SaturatingSubtract(scalar_type, value(left)?, value(right)?)
            }
            Self::SaturatingMultiply(scalar_type, left, right) => {
                let (left, right) = commutative(left, right)?;
                Self::SaturatingMultiply(scalar_type, left, right)
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ProofCertifiedScalarExpressionKey {
    ExactCast(IntegerType, IntegerType, ValueId),
    ExactShiftLeft(IntegerType, IntegerType, ValueId, ValueId),
    ExactShiftRight(IntegerType, IntegerType, ValueId, ValueId),
    ExactAdd(IntegerType, ValueId, ValueId),
    ExactSubtract(IntegerType, ValueId, ValueId),
    ExactMultiply(IntegerType, ValueId, ValueId),
    ExactDivide(IntegerType, ValueId, ValueId),
    ExactRemainder(IntegerType, ValueId, ValueId),
    WrappingDivide(IntegerType, ValueId, ValueId),
    WrappingRemainder(IntegerType, ValueId, ValueId),
    SaturatingDivide(IntegerType, ValueId, ValueId),
    SaturatingRemainder(IntegerType, ValueId, ValueId),
}

impl ProofCertifiedScalarExpressionKey {
    fn references_any(self, values: &BTreeSet<ValueId>) -> bool {
        match self {
            Self::ExactCast(_, _, operand) => values.contains(&operand),
            Self::ExactShiftLeft(_, _, left, right)
            | Self::ExactShiftRight(_, _, left, right)
            | Self::ExactAdd(_, left, right)
            | Self::ExactSubtract(_, left, right)
            | Self::ExactMultiply(_, left, right)
            | Self::ExactDivide(_, left, right)
            | Self::ExactRemainder(_, left, right)
            | Self::WrappingDivide(_, left, right)
            | Self::WrappingRemainder(_, left, right)
            | Self::SaturatingDivide(_, left, right)
            | Self::SaturatingRemainder(_, left, right) => {
                values.contains(&left) || values.contains(&right)
            }
        }
    }

    fn translate(self, values: &BTreeMap<ValueId, ValueId>) -> Self {
        let value = |operand: ValueId| values.get(&operand).copied().unwrap_or(operand);
        match self {
            Self::ExactCast(source_type, target_type, operand) => {
                Self::ExactCast(source_type, target_type, value(operand))
            }
            Self::ExactShiftLeft(value_type, count_type, operand, count) => {
                Self::ExactShiftLeft(value_type, count_type, value(operand), value(count))
            }
            Self::ExactShiftRight(value_type, count_type, operand, count) => {
                Self::ExactShiftRight(value_type, count_type, value(operand), value(count))
            }
            Self::ExactAdd(scalar_type, left, right) => {
                let (left, right) = canonical_pair(value(left), value(right));
                Self::ExactAdd(scalar_type, left, right)
            }
            Self::ExactSubtract(scalar_type, left, right) => {
                Self::ExactSubtract(scalar_type, value(left), value(right))
            }
            Self::ExactMultiply(scalar_type, left, right) => {
                let (left, right) = canonical_pair(value(left), value(right));
                Self::ExactMultiply(scalar_type, left, right)
            }
            Self::ExactDivide(scalar_type, left, right) => {
                Self::ExactDivide(scalar_type, value(left), value(right))
            }
            Self::ExactRemainder(scalar_type, left, right) => {
                Self::ExactRemainder(scalar_type, value(left), value(right))
            }
            Self::WrappingDivide(scalar_type, left, right) => {
                Self::WrappingDivide(scalar_type, value(left), value(right))
            }
            Self::WrappingRemainder(scalar_type, left, right) => {
                Self::WrappingRemainder(scalar_type, value(left), value(right))
            }
            Self::SaturatingDivide(scalar_type, left, right) => {
                Self::SaturatingDivide(scalar_type, value(left), value(right))
            }
            Self::SaturatingRemainder(scalar_type, left, right) => {
                Self::SaturatingRemainder(scalar_type, value(left), value(right))
            }
        }
    }
}

fn canonical_pair(left: ValueId, right: ValueId) -> (ValueId, ValueId) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

fn total_scalar_expression(
    operation: &O,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Option<(TotalScalarExpressionKey, OperationId, ValueId, ScalarType)> {
    let boolean = ScalarType::Boolean;
    let integer_operand_type = |value: ValueId| match value_types.get(&value) {
        Some(ScalarType::Integer(scalar_type)) => Some(*scalar_type),
        _ => None,
    };
    let row = match operation {
        O::BooleanConstant {
            psi_operation,
            result,
            value,
        } => (
            TotalScalarExpressionKey::BooleanConstant(*value),
            *psi_operation,
            *result,
            boolean,
        ),
        O::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } => (
            TotalScalarExpressionKey::IntegerConstant(*scalar_type, *value),
            *psi_operation,
            *result,
            *scalar_type,
        ),
        O::BooleanNot {
            psi_operation,
            result,
            operand,
        } => (
            TotalScalarExpressionKey::BooleanNot(*operand),
            *psi_operation,
            *result,
            boolean,
        ),
        O::BooleanEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let (left, right) = canonical_pair(*left, *right);
            (
                TotalScalarExpressionKey::BooleanEqual(left, right),
                *psi_operation,
                *result,
                boolean,
            )
        }
        O::IntegerEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let scalar_type = integer_operand_type(*left)?;
            if integer_operand_type(*right)? != scalar_type {
                return None;
            }
            let (left, right) = canonical_pair(*left, *right);
            (
                TotalScalarExpressionKey::IntegerEqual(scalar_type, left, right),
                *psi_operation,
                *result,
                boolean,
            )
        }
        O::IntegerLessThan {
            psi_operation,
            result,
            left,
            right,
        } => {
            let scalar_type = integer_operand_type(*left)?;
            if integer_operand_type(*right)? != scalar_type {
                return None;
            }
            (
                TotalScalarExpressionKey::IntegerLessThan(scalar_type, *left, *right),
                *psi_operation,
                *result,
                boolean,
            )
        }
        O::IntegerLessOrEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let scalar_type = integer_operand_type(*left)?;
            if integer_operand_type(*right)? != scalar_type {
                return None;
            }
            (
                TotalScalarExpressionKey::IntegerLessOrEqual(scalar_type, *left, *right),
                *psi_operation,
                *result,
                boolean,
            )
        }
        O::IntegerBitwiseNot {
            psi_operation,
            result,
            scalar_type,
            operand,
        } => (
            TotalScalarExpressionKey::IntegerBitwiseNot(*scalar_type, *operand),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::IntegerWiden {
            psi_operation,
            result,
            source_type,
            target_type,
            operand,
        } => (
            TotalScalarExpressionKey::IntegerWiden(*source_type, *target_type, *operand),
            *psi_operation,
            *result,
            ScalarType::Integer(*target_type),
        ),
        O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = canonical_pair(*left, *right);
            (
                TotalScalarExpressionKey::IntegerBitwiseAnd(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = canonical_pair(*left, *right);
            (
                TotalScalarExpressionKey::IntegerBitwiseOr(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = canonical_pair(*left, *right);
            (
                TotalScalarExpressionKey::IntegerBitwiseXor(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            TotalScalarExpressionKey::WrappingShiftLeft(*value_type, *count_type, *value, *count),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
        ),
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            TotalScalarExpressionKey::WrappingShiftRight(*value_type, *count_type, *value, *count),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
        ),
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = canonical_pair(*left, *right);
            (
                TotalScalarExpressionKey::WrappingAdd(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            TotalScalarExpressionKey::WrappingSubtract(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = canonical_pair(*left, *right);
            (
                TotalScalarExpressionKey::WrappingMultiply(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = canonical_pair(*left, *right);
            (
                TotalScalarExpressionKey::SaturatingAdd(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            TotalScalarExpressionKey::SaturatingSubtract(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = canonical_pair(*left, *right);
            (
                TotalScalarExpressionKey::SaturatingMultiply(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        _ => return None,
    };
    Some(row)
}

fn proof_certified_scalar_expression(
    operation: &O,
) -> Option<(
    ProofCertifiedScalarExpressionKey,
    OperationId,
    ValueId,
    ScalarType,
)> {
    let row = match operation {
        O::IntegerExactCast {
            psi_operation,
            result,
            source_type,
            target_type,
            operand,
            ..
        } => (
            ProofCertifiedScalarExpressionKey::ExactCast(*source_type, *target_type, *operand),
            *psi_operation,
            *result,
            ScalarType::Integer(*target_type),
        ),
        O::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => (
            ProofCertifiedScalarExpressionKey::ExactShiftLeft(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
        ),
        O::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => (
            ProofCertifiedScalarExpressionKey::ExactShiftRight(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
        ),
        O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => {
            let (left, right) = canonical_pair(*left, *right);
            (
                ProofCertifiedScalarExpressionKey::ExactAdd(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            ProofCertifiedScalarExpressionKey::ExactSubtract(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::ExactIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => {
            let (left, right) = canonical_pair(*left, *right);
            (
                ProofCertifiedScalarExpressionKey::ExactMultiply(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::ExactIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            ProofCertifiedScalarExpressionKey::ExactDivide(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::ExactIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            ProofCertifiedScalarExpressionKey::ExactRemainder(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::WrappingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            ProofCertifiedScalarExpressionKey::WrappingDivide(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::WrappingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            ProofCertifiedScalarExpressionKey::WrappingRemainder(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::SaturatingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            ProofCertifiedScalarExpressionKey::SaturatingDivide(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::SaturatingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            ProofCertifiedScalarExpressionKey::SaturatingRemainder(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        _ => return None,
    };
    Some(row)
}

fn compatible_policy_scalar_leader(
    operation: &O,
) -> Option<(
    CompatiblePolicyScalarExpressionKey,
    OperationId,
    ValueId,
    ScalarType,
)> {
    let row = match operation {
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            CompatiblePolicyScalarExpressionKey::ShiftLeft(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
        ),
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            CompatiblePolicyScalarExpressionKey::ShiftRight(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
        ),
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | O::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = canonical_pair(*left, *right);
            (
                CompatiblePolicyScalarExpressionKey::Add(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | O::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            CompatiblePolicyScalarExpressionKey::Subtract(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | O::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = canonical_pair(*left, *right);
            (
                CompatiblePolicyScalarExpressionKey::Multiply(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        _ => return None,
    };
    Some(row)
}

fn compatible_policy_scalar_redundant(
    operation: &O,
) -> Option<(
    CompatiblePolicyScalarExpressionKey,
    OperationId,
    ValueId,
    ScalarType,
)> {
    let row = match operation {
        O::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => (
            CompatiblePolicyScalarExpressionKey::ShiftLeft(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
        ),
        O::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => (
            CompatiblePolicyScalarExpressionKey::ShiftRight(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
        ),
        O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => {
            let (left, right) = canonical_pair(*left, *right);
            (
                CompatiblePolicyScalarExpressionKey::Add(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            CompatiblePolicyScalarExpressionKey::Subtract(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::ExactIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => {
            let (left, right) = canonical_pair(*left, *right);
            (
                CompatiblePolicyScalarExpressionKey::Multiply(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        _ => return None,
    };
    Some(row)
}

impl SameBlockTotalScalarCseRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.same-block-obligation-free-total-scalar-cse.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(GLOBAL_VALUE_NUMBERING_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ExactOperationSemantics,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for SameBlockTotalScalarCseRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::EffectSummaries(effects)) =
            analyses.get(AnalysisKind::EffectSummaries)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::EffectSummaries,
            ));
        };
        let mut candidates = Vec::new();
        for function in &unit.functions {
            let value_types = function
                .parameters
                .iter()
                .map(|row| (row.value, row.scalar_type))
                .chain(function.blocks.iter().flat_map(|block| {
                    block
                        .parameters
                        .iter()
                        .map(|row| (row.value, row.scalar_type))
                }))
                .chain(function.blocks.iter().flat_map(|block| {
                    block.nodes.iter().flat_map(|node| {
                        node.definitions
                            .iter()
                            .map(|row| (row.value, row.scalar_type))
                    })
                }))
                .collect::<BTreeMap<_, _>>();
            for block in &function.blocks {
                let mut leaders = BTreeMap::new();
                for (index, node) in block.nodes.iter().enumerate() {
                    let Some((key, operation, result, scalar_type)) =
                        total_scalar_expression(&node.operation, &value_types)
                    else {
                        continue;
                    };
                    let node_index =
                        u32::try_from(index).expect("optimization node index fits u32");
                    let pure = effects.nodes.iter().any(|row| {
                        row.revision == unit.identity
                            && row.machine == function.machine
                            && row.block == block.id
                            && row.node == node_index
                            && row.class == crate::EffectClass::PureScalar
                            && row.observable == crate::EffectKnowledge::No
                            && row.structural_state == crate::EffectKnowledge::No
                            && row.crash == crate::EffectKnowledge::No
                            && row.suspension == crate::EffectKnowledge::No
                    });
                    if !pure {
                        continue;
                    }
                    let Some((leader, leader_operation, leader_result, leader_type)) =
                        leaders.get(&key).copied()
                    else {
                        leaders.insert(key, (node_index, operation, result, scalar_type));
                        continue;
                    };
                    if leader_type != scalar_type
                        || !use_definitions.uses.iter().any(|(machine, use_site)| {
                            *machine == function.machine && use_site.value == result
                        })
                    {
                        continue;
                    }
                    let Some(receiver) = block.nodes.get(index + 1) else {
                        continue;
                    };
                    if receiver
                        .provenance
                        .iter()
                        .any(|source| node.provenance.contains(source))
                    {
                        continue;
                    }
                    let leader_location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: leader,
                    };
                    let redundant_location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    };
                    let Some((affected_blocks, provenance)) =
                        local_cse_accounting(function, redundant_location, result)
                    else {
                        continue;
                    };
                    let patch = LocalScalarCommonSubexpressionRewrite {
                        leader: leader_location,
                        redundant: redundant_location,
                        leader_operation,
                        redundant_operation: operation,
                        leader_result,
                        redundant_result: result,
                        scalar_type,
                    };
                    candidates.push(
                        PsiRewriteCandidate::new_local_scalar_common_subexpression(
                            unit.identity,
                            Self::contract(),
                            affected_blocks,
                            provenance,
                            -1,
                            patch,
                        )
                        .map_err(RuleProposalError::InvalidCandidate)?,
                    );
                }
            }
        }
        Ok(candidates)
    }
}

impl DominatorTotalScalarGvnRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.dominator-obligation-free-total-scalar-gvn.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(GLOBAL_VALUE_NUMBERING_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ExactOperationSemantics,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for DominatorTotalScalarGvnRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        if analyses.get(AnalysisKind::ControlFlowGraph).is_none() {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ControlFlowGraph,
            ));
        }
        let Some(AnalysisProduct::Dominators(dominators)) = analyses.get(AnalysisKind::Dominators)
        else {
            return Err(RuleProposalError::MissingAnalysis(AnalysisKind::Dominators));
        };
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::EffectSummaries(effects)) =
            analyses.get(AnalysisKind::EffectSummaries)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::EffectSummaries,
            ));
        };
        let mut candidates = Vec::new();
        for function in &unit.functions {
            let machine_dominators = dominators
                .functions
                .iter()
                .find(|(machine, _)| *machine == function.machine)
                .map(|(_, rows)| rows.as_slice())
                .unwrap_or_default();
            let value_types = function
                .parameters
                .iter()
                .map(|row| (row.value, row.scalar_type))
                .chain(function.blocks.iter().flat_map(|block| {
                    block
                        .parameters
                        .iter()
                        .map(|row| (row.value, row.scalar_type))
                }))
                .chain(function.blocks.iter().flat_map(|block| {
                    block.nodes.iter().flat_map(|node| {
                        node.definitions
                            .iter()
                            .map(|row| (row.value, row.scalar_type))
                    })
                }))
                .collect::<BTreeMap<_, _>>();
            let mut expressions = Vec::new();
            for block in &function.blocks {
                for (index, node) in block.nodes.iter().enumerate() {
                    let node_index =
                        u32::try_from(index).expect("optimization node index fits u32");
                    let Some((key, operation, result, scalar_type)) =
                        total_scalar_expression(&node.operation, &value_types)
                    else {
                        continue;
                    };
                    if !exact_pure_scalar_effect(
                        unit,
                        effects,
                        function.machine,
                        block.id,
                        node_index,
                    ) {
                        continue;
                    }
                    expressions.push((
                        key,
                        NodeLocation {
                            machine: function.machine,
                            block: block.id,
                            node: node_index,
                        },
                        operation,
                        result,
                        scalar_type,
                    ));
                }
            }
            for (key, redundant, redundant_operation, redundant_result, scalar_type) in &expressions
            {
                if !use_definitions.uses.iter().any(|(machine, use_site)| {
                    *machine == function.machine && use_site.value == *redundant_result
                }) {
                    continue;
                }
                let Some(redundant_block) = function
                    .blocks
                    .iter()
                    .find(|block| block.id == redundant.block)
                else {
                    continue;
                };
                let redundant_index = usize::try_from(redundant.node).expect("u32 fits usize");
                let Some(redundant_node) = redundant_block.nodes.get(redundant_index) else {
                    continue;
                };
                let Some(receiver) = redundant_block.nodes.get(redundant_index + 1) else {
                    continue;
                };
                if receiver
                    .provenance
                    .iter()
                    .any(|source| redundant_node.provenance.contains(source))
                {
                    continue;
                }
                let leader = expressions
                    .iter()
                    .filter(|(candidate_key, location, _, _, candidate_type)| {
                        candidate_key == key
                            && *candidate_type == *scalar_type
                            && location.block != redundant.block
                            && block_dominates(machine_dominators, location.block, redundant.block)
                    })
                    .min_by_key(|(_, location, _, _, _)| {
                        let depth = machine_dominators
                            .iter()
                            .find(|(block, _)| *block == location.block)
                            .map_or(usize::MAX, |(_, rows)| rows.len());
                        (depth, *location)
                    });
                let Some((_, leader, leader_operation, leader_result, _)) = leader else {
                    continue;
                };
                let replacement_definition = omega_optimization_unit::ValueDefinition {
                    value: *leader_result,
                    scalar_type: *scalar_type,
                    site: omega_optimization_unit::ValueDefinitionSite::Node {
                        block: leader.block,
                        node: leader.node,
                    },
                };
                if !use_definitions
                    .uses
                    .iter()
                    .filter(|(machine, use_site)| {
                        *machine == function.machine && use_site.value == *redundant_result
                    })
                    .all(|(_, use_site)| match replacement_definition.site {
                        omega_optimization_unit::ValueDefinitionSite::Node { block, node }
                            if block == use_site.block =>
                        {
                            node < use_site.node
                        }
                        omega_optimization_unit::ValueDefinitionSite::Node { block, .. } => {
                            block_dominates(machine_dominators, block, use_site.block)
                        }
                        _ => false,
                    })
                {
                    continue;
                }
                let Some((affected_blocks, provenance)) =
                    local_cse_accounting(function, *redundant, *redundant_result)
                else {
                    continue;
                };
                let patch = DominatingScalarCommonSubexpressionRewrite {
                    leader: *leader,
                    redundant: *redundant,
                    leader_operation: *leader_operation,
                    redundant_operation: *redundant_operation,
                    leader_result: *leader_result,
                    redundant_result: *redundant_result,
                    scalar_type: *scalar_type,
                };
                candidates.push(
                    PsiRewriteCandidate::new_dominating_scalar_common_subexpression(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        provenance,
                        -1,
                        patch,
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}

impl SameBlockProofCertifiedScalarCseRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.same-block-proof-certified-total-scalar-cse.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(GLOBAL_VALUE_NUMBERING_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for SameBlockProofCertifiedScalarCseRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::EffectSummaries(effects)) =
            analyses.get(AnalysisKind::EffectSummaries)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::EffectSummaries,
            ));
        };
        let mut candidates = Vec::new();
        for function in &unit.functions {
            for block in &function.blocks {
                let mut leaders = BTreeMap::new();
                for (index, node) in block.nodes.iter().enumerate() {
                    let Some((key, operation, result, scalar_type)) =
                        proof_certified_scalar_expression(&node.operation)
                    else {
                        continue;
                    };
                    let Some(obligation_fact) =
                        accepted_obligation_fact(unit, function.machine, operation).ok()
                    else {
                        continue;
                    };
                    let node_index =
                        u32::try_from(index).expect("optimization node index fits u32");
                    if !exact_pure_scalar_effect(
                        unit,
                        effects,
                        function.machine,
                        block.id,
                        node_index,
                    ) {
                        continue;
                    }
                    let Some((leader, leader_operation, leader_result, leader_type)) =
                        leaders.get(&key).copied()
                    else {
                        leaders.insert(key, (node_index, operation, result, scalar_type));
                        continue;
                    };
                    if leader_type != scalar_type
                        || !use_definitions.uses.iter().any(|(machine, use_site)| {
                            *machine == function.machine && use_site.value == result
                        })
                    {
                        continue;
                    }
                    let Some(receiver) = block.nodes.get(index + 1) else {
                        continue;
                    };
                    if receiver
                        .provenance
                        .iter()
                        .any(|source| node.provenance.contains(source))
                    {
                        continue;
                    }
                    let leader_location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: leader,
                    };
                    let redundant_location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    };
                    let Some((affected_blocks, provenance)) =
                        local_cse_accounting(function, redundant_location, result)
                    else {
                        continue;
                    };
                    candidates.push(
                        PsiRewriteCandidate::new_proof_certified_local_scalar_common_subexpression(
                            unit.identity,
                            Self::contract(),
                            affected_blocks,
                            provenance,
                            obligation_fact,
                            -1,
                            LocalScalarCommonSubexpressionRewrite {
                                leader: leader_location,
                                redundant: redundant_location,
                                leader_operation,
                                redundant_operation: operation,
                                leader_result,
                                redundant_result: result,
                                scalar_type,
                            },
                        )
                        .map_err(RuleProposalError::InvalidCandidate)?,
                    );
                }
            }
        }
        Ok(candidates)
    }
}

impl DominatorProofCertifiedScalarGvnRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.dominator-proof-certified-total-scalar-gvn.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(GLOBAL_VALUE_NUMBERING_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for DominatorProofCertifiedScalarGvnRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        if analyses.get(AnalysisKind::ControlFlowGraph).is_none() {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ControlFlowGraph,
            ));
        }
        let Some(AnalysisProduct::Dominators(dominators)) = analyses.get(AnalysisKind::Dominators)
        else {
            return Err(RuleProposalError::MissingAnalysis(AnalysisKind::Dominators));
        };
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::EffectSummaries(effects)) =
            analyses.get(AnalysisKind::EffectSummaries)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::EffectSummaries,
            ));
        };
        let mut candidates = Vec::new();
        for function in &unit.functions {
            let machine_dominators = dominators
                .functions
                .iter()
                .find(|(machine, _)| *machine == function.machine)
                .map(|(_, rows)| rows.as_slice())
                .unwrap_or_default();
            let mut expressions = Vec::new();
            for block in &function.blocks {
                for (index, node) in block.nodes.iter().enumerate() {
                    let node_index =
                        u32::try_from(index).expect("optimization node index fits u32");
                    let Some((key, operation, result, scalar_type)) =
                        proof_certified_scalar_expression(&node.operation)
                    else {
                        continue;
                    };
                    let Some(obligation_fact) =
                        accepted_obligation_fact(unit, function.machine, operation).ok()
                    else {
                        continue;
                    };
                    if !exact_pure_scalar_effect(
                        unit,
                        effects,
                        function.machine,
                        block.id,
                        node_index,
                    ) {
                        continue;
                    }
                    expressions.push((
                        key,
                        NodeLocation {
                            machine: function.machine,
                            block: block.id,
                            node: node_index,
                        },
                        operation,
                        result,
                        scalar_type,
                        obligation_fact,
                    ));
                }
            }
            for (
                key,
                redundant,
                redundant_operation,
                redundant_result,
                scalar_type,
                obligation_fact,
            ) in &expressions
            {
                if !use_definitions.uses.iter().any(|(machine, use_site)| {
                    *machine == function.machine && use_site.value == *redundant_result
                }) {
                    continue;
                }
                let Some(redundant_block) = function
                    .blocks
                    .iter()
                    .find(|block| block.id == redundant.block)
                else {
                    continue;
                };
                let redundant_index = usize::try_from(redundant.node).expect("u32 fits usize");
                let Some(redundant_node) = redundant_block.nodes.get(redundant_index) else {
                    continue;
                };
                let Some(receiver) = redundant_block.nodes.get(redundant_index + 1) else {
                    continue;
                };
                if receiver
                    .provenance
                    .iter()
                    .any(|source| redundant_node.provenance.contains(source))
                {
                    continue;
                }
                let leader = expressions
                    .iter()
                    .filter(|(candidate_key, location, _, _, candidate_type, _)| {
                        candidate_key == key
                            && *candidate_type == *scalar_type
                            && location.block != redundant.block
                            && block_dominates(machine_dominators, location.block, redundant.block)
                    })
                    .min_by_key(|(_, location, _, _, _, _)| {
                        let depth = machine_dominators
                            .iter()
                            .find(|(block, _)| *block == location.block)
                            .map_or(usize::MAX, |(_, rows)| rows.len());
                        (depth, *location)
                    });
                let Some((_, leader, leader_operation, leader_result, _, _)) = leader else {
                    continue;
                };
                let replacement_definition = omega_optimization_unit::ValueDefinition {
                    value: *leader_result,
                    scalar_type: *scalar_type,
                    site: omega_optimization_unit::ValueDefinitionSite::Node {
                        block: leader.block,
                        node: leader.node,
                    },
                };
                if !use_definitions
                    .uses
                    .iter()
                    .filter(|(machine, use_site)| {
                        *machine == function.machine && use_site.value == *redundant_result
                    })
                    .all(|(_, use_site)| match replacement_definition.site {
                        omega_optimization_unit::ValueDefinitionSite::Node { block, node }
                            if block == use_site.block =>
                        {
                            node < use_site.node
                        }
                        omega_optimization_unit::ValueDefinitionSite::Node { block, .. } => {
                            block_dominates(machine_dominators, block, use_site.block)
                        }
                        _ => false,
                    })
                {
                    continue;
                }
                let Some((affected_blocks, provenance)) =
                    local_cse_accounting(function, *redundant, *redundant_result)
                else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_proof_certified_dominating_scalar_common_subexpression(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        provenance,
                        *obligation_fact,
                        -1,
                        DominatingScalarCommonSubexpressionRewrite {
                            leader: *leader,
                            redundant: *redundant,
                            leader_operation: *leader_operation,
                            redundant_operation: *redundant_operation,
                            leader_result: *leader_result,
                            redundant_result: *redundant_result,
                            scalar_type: *scalar_type,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}

impl SameBlockProofCertifiedCompatiblePolicyScalarCseRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.same-block-proof-certified-compatible-policy-scalar-cse.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(GLOBAL_VALUE_NUMBERING_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for SameBlockProofCertifiedCompatiblePolicyScalarCseRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::EffectSummaries(effects)) =
            analyses.get(AnalysisKind::EffectSummaries)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::EffectSummaries,
            ));
        };
        let mut candidates = Vec::new();
        for function in &unit.functions {
            for block in &function.blocks {
                let mut leaders = BTreeMap::new();
                for (index, node) in block.nodes.iter().enumerate() {
                    let node_index =
                        u32::try_from(index).expect("optimization node index fits u32");
                    if let Some((key, operation, result, scalar_type)) =
                        compatible_policy_scalar_leader(&node.operation)
                        && exact_pure_scalar_effect(
                            unit,
                            effects,
                            function.machine,
                            block.id,
                            node_index,
                        )
                        && !function.facts.iter().any(|fact| {
                            matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                                if *support == operation)
                        })
                    {
                        leaders.entry(key).or_insert((
                            node_index,
                            operation,
                            result,
                            scalar_type,
                        ));
                    }
                    let Some((key, operation, result, scalar_type)) =
                        compatible_policy_scalar_redundant(&node.operation)
                    else {
                        continue;
                    };
                    let Some(obligation_fact) =
                        accepted_obligation_fact(unit, function.machine, operation).ok()
                    else {
                        continue;
                    };
                    if !exact_pure_scalar_effect(
                        unit,
                        effects,
                        function.machine,
                        block.id,
                        node_index,
                    ) || !use_definitions.uses.iter().any(|(machine, use_site)| {
                        *machine == function.machine && use_site.value == result
                    }) {
                        continue;
                    }
                    let Some((leader, leader_operation, leader_result, leader_type)) =
                        leaders.get(&key).copied()
                    else {
                        continue;
                    };
                    if leader_type != scalar_type {
                        continue;
                    }
                    let Some(receiver) = block.nodes.get(index + 1) else {
                        continue;
                    };
                    if receiver
                        .provenance
                        .iter()
                        .any(|source| node.provenance.contains(source))
                    {
                        continue;
                    }
                    let redundant = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    };
                    let Some((affected_blocks, provenance)) =
                        local_cse_accounting(function, redundant, result)
                    else {
                        continue;
                    };
                    candidates.push(
                        PsiRewriteCandidate::new_proof_certified_local_scalar_common_subexpression(
                            unit.identity,
                            Self::contract(),
                            affected_blocks,
                            provenance,
                            obligation_fact,
                            -1,
                            LocalScalarCommonSubexpressionRewrite {
                                leader: NodeLocation {
                                    machine: function.machine,
                                    block: block.id,
                                    node: leader,
                                },
                                redundant,
                                leader_operation,
                                redundant_operation: operation,
                                leader_result,
                                redundant_result: result,
                                scalar_type,
                            },
                        )
                        .map_err(RuleProposalError::InvalidCandidate)?,
                    );
                }
            }
        }
        Ok(candidates)
    }
}

impl DominatorProofCertifiedCompatiblePolicyScalarGvnRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.dominator-proof-certified-compatible-policy-scalar-gvn.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(GLOBAL_VALUE_NUMBERING_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for DominatorProofCertifiedCompatiblePolicyScalarGvnRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        if analyses.get(AnalysisKind::ControlFlowGraph).is_none() {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ControlFlowGraph,
            ));
        }
        let Some(AnalysisProduct::Dominators(dominators)) = analyses.get(AnalysisKind::Dominators)
        else {
            return Err(RuleProposalError::MissingAnalysis(AnalysisKind::Dominators));
        };
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::EffectSummaries(effects)) =
            analyses.get(AnalysisKind::EffectSummaries)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::EffectSummaries,
            ));
        };
        let mut candidates = Vec::new();
        for function in &unit.functions {
            let machine_dominators = dominators
                .functions
                .iter()
                .find(|(machine, _)| *machine == function.machine)
                .map(|(_, rows)| rows.as_slice())
                .unwrap_or_default();
            let mut leaders = Vec::new();
            let mut redundants = Vec::new();
            for block in &function.blocks {
                for (index, node) in block.nodes.iter().enumerate() {
                    let node_index =
                        u32::try_from(index).expect("optimization node index fits u32");
                    let location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    };
                    if let Some((key, operation, result, scalar_type)) =
                        compatible_policy_scalar_leader(&node.operation)
                        && exact_pure_scalar_effect(
                            unit,
                            effects,
                            function.machine,
                            block.id,
                            node_index,
                        )
                        && !function.facts.iter().any(|fact| {
                            matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                                if *support == operation)
                        })
                    {
                        leaders.push((key, location, operation, result, scalar_type));
                    }
                    if let Some((key, operation, result, scalar_type)) =
                        compatible_policy_scalar_redundant(&node.operation)
                        && let Ok(obligation_fact) =
                            accepted_obligation_fact(unit, function.machine, operation)
                        && exact_pure_scalar_effect(
                            unit,
                            effects,
                            function.machine,
                            block.id,
                            node_index,
                        )
                    {
                        redundants.push((
                            key,
                            location,
                            operation,
                            result,
                            scalar_type,
                            obligation_fact,
                        ));
                    }
                }
            }
            for (key, redundant, redundant_operation, redundant_result, scalar_type, fact) in
                redundants
            {
                if !use_definitions.uses.iter().any(|(machine, use_site)| {
                    *machine == function.machine && use_site.value == redundant_result
                }) {
                    continue;
                }
                let Some(redundant_block) = function
                    .blocks
                    .iter()
                    .find(|block| block.id == redundant.block)
                else {
                    continue;
                };
                let redundant_index = usize::try_from(redundant.node).expect("u32 fits usize");
                let Some(redundant_node) = redundant_block.nodes.get(redundant_index) else {
                    continue;
                };
                let Some(receiver) = redundant_block.nodes.get(redundant_index + 1) else {
                    continue;
                };
                if receiver
                    .provenance
                    .iter()
                    .any(|source| redundant_node.provenance.contains(source))
                {
                    continue;
                }
                let leader = leaders
                    .iter()
                    .filter(|(candidate_key, location, _, _, candidate_type)| {
                        *candidate_key == key
                            && *candidate_type == scalar_type
                            && location.block != redundant.block
                            && block_dominates(machine_dominators, location.block, redundant.block)
                    })
                    .min_by_key(|(_, location, _, _, _)| {
                        let depth = machine_dominators
                            .iter()
                            .find(|(block, _)| *block == location.block)
                            .map_or(usize::MAX, |(_, rows)| rows.len());
                        (depth, *location)
                    });
                let Some((_, leader, leader_operation, leader_result, _)) = leader else {
                    continue;
                };
                if !use_definitions
                    .uses
                    .iter()
                    .filter(|(machine, use_site)| {
                        *machine == function.machine && use_site.value == redundant_result
                    })
                    .all(|(_, use_site)| {
                        if leader.block == use_site.block {
                            leader.node < use_site.node
                        } else {
                            block_dominates(machine_dominators, leader.block, use_site.block)
                        }
                    })
                {
                    continue;
                }
                let Some((affected_blocks, provenance)) =
                    local_cse_accounting(function, redundant, redundant_result)
                else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_proof_certified_dominating_scalar_common_subexpression(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        provenance,
                        fact,
                        -1,
                        DominatingScalarCommonSubexpressionRewrite {
                            leader: *leader,
                            redundant,
                            leader_operation: *leader_operation,
                            redundant_operation,
                            leader_result: *leader_result,
                            redundant_result,
                            scalar_type,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}

impl PhiTranslatedObligationFreeScalarGvnRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.phi-translated-obligation-free-total-scalar-gvn.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(GLOBAL_VALUE_NUMBERING_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ExactOperationSemantics,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for PhiTranslatedObligationFreeScalarGvnRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        if analyses.get(AnalysisKind::ControlFlowGraph).is_none() {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ControlFlowGraph,
            ));
        }
        let Some(AnalysisProduct::Dominators(dominators)) = analyses.get(AnalysisKind::Dominators)
        else {
            return Err(RuleProposalError::MissingAnalysis(AnalysisKind::Dominators));
        };
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::EffectSummaries(effects)) =
            analyses.get(AnalysisKind::EffectSummaries)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::EffectSummaries,
            ));
        };

        let mut candidates = Vec::new();
        for function in &unit.functions {
            let machine_dominators = dominators
                .functions
                .iter()
                .find(|(machine, _)| *machine == function.machine)
                .map(|(_, rows)| rows.as_slice())
                .unwrap_or_default();
            let value_types = function
                .parameters
                .iter()
                .map(|row| (row.value, row.scalar_type))
                .chain(function.blocks.iter().flat_map(|block| {
                    block
                        .parameters
                        .iter()
                        .map(|row| (row.value, row.scalar_type))
                }))
                .chain(function.blocks.iter().flat_map(|block| {
                    block.nodes.iter().flat_map(|node| {
                        node.definitions
                            .iter()
                            .map(|row| (row.value, row.scalar_type))
                    })
                }))
                .collect::<BTreeMap<_, _>>();
            let mut expressions = Vec::new();
            for block in &function.blocks {
                for (index, node) in block.nodes.iter().enumerate() {
                    let node_index =
                        u32::try_from(index).expect("optimization node index fits u32");
                    let Some((key, operation, result, scalar_type)) =
                        total_scalar_expression(&node.operation, &value_types)
                    else {
                        continue;
                    };
                    if exact_pure_scalar_effect(
                        unit,
                        effects,
                        function.machine,
                        block.id,
                        node_index,
                    ) {
                        expressions.push((
                            key,
                            NodeLocation {
                                machine: function.machine,
                                block: block.id,
                                node: node_index,
                            },
                            operation,
                            result,
                            scalar_type,
                        ));
                    }
                }
            }

            for (key, redundant, redundant_operation, redundant_result, scalar_type) in &expressions
            {
                let Some(block) = function
                    .blocks
                    .iter()
                    .find(|block| block.id == redundant.block)
                else {
                    continue;
                };
                let parameter_values = block
                    .parameters
                    .iter()
                    .map(|parameter| parameter.value)
                    .collect::<BTreeSet<_>>();
                if parameter_values.is_empty()
                    || !key.references_any(&parameter_values)
                    || !use_definitions.uses.iter().any(|(machine, use_site)| {
                        *machine == function.machine && use_site.value == *redundant_result
                    })
                {
                    continue;
                }
                let redundant_index = usize::try_from(redundant.node).expect("u32 fits usize");
                let Some(redundant_node) = block.nodes.get(redundant_index) else {
                    continue;
                };
                let Some(receiver) = block.nodes.get(redundant_index + 1) else {
                    continue;
                };
                if receiver
                    .provenance
                    .iter()
                    .any(|source| redundant_node.provenance.contains(source))
                {
                    continue;
                }

                let mut incoming = Vec::new();
                let mut complete = true;
                for source in &function.blocks {
                    for (owner_index, owner) in source.nodes.iter().enumerate() {
                        let owner_index =
                            u32::try_from(owner_index).expect("optimization node index fits u32");
                        for edge in owner
                            .successors
                            .iter()
                            .filter(|edge| edge.target == block.id)
                        {
                            if edge.bindings.len() != block.parameters.len() {
                                complete = false;
                                continue;
                            }
                            let mut translation = BTreeMap::new();
                            for (parameter, binding) in block.parameters.iter().zip(&edge.bindings)
                            {
                                if binding.parameter != parameter.value
                                    || binding.scalar_type != parameter.scalar_type
                                    || value_types.get(&binding.argument)
                                        != Some(&binding.scalar_type)
                                {
                                    complete = false;
                                    break;
                                }
                                translation.insert(parameter.value, binding.argument);
                            }
                            if !complete {
                                continue;
                            }
                            let Some(translated_key) = key.translate(&translation) else {
                                complete = false;
                                continue;
                            };
                            let leader = expressions
                                .iter()
                                .filter(|(candidate_key, location, _, _, candidate_type)| {
                                    candidate_key == &translated_key
                                        && candidate_type == scalar_type
                                        && ((location.block == source.id
                                            && location.node < owner_index)
                                            || (location.block != source.id
                                                && block_dominates(
                                                    machine_dominators,
                                                    location.block,
                                                    source.id,
                                                )))
                                })
                                .min_by_key(|(_, location, _, _, _)| {
                                    let depth = machine_dominators
                                        .iter()
                                        .find(|(candidate, _)| *candidate == location.block)
                                        .map_or(usize::MAX, |(_, rows)| rows.len());
                                    (depth, *location)
                                });
                            let Some((_, leader, leader_operation, leader_result, _)) = leader
                            else {
                                complete = false;
                                continue;
                            };
                            incoming.push(PhiTranslatedScalarIncoming {
                                source: source.id,
                                edge: edge.psi_edge,
                                leader: *leader,
                                leader_operation: *leader_operation,
                                leader_result: *leader_result,
                            });
                        }
                    }
                }
                if !complete || incoming.len() < 2 {
                    continue;
                }
                incoming.sort_by_key(|row| (row.edge, row.source));
                let Some((affected_blocks, provenance)) =
                    phi_translated_cse_accounting(function, *redundant, &incoming)
                else {
                    continue;
                };
                let parameter_position = u32::try_from(block.parameters.len())
                    .expect("optimization block parameter count fits u32");
                candidates.push(
                    PsiRewriteCandidate::new_phi_translated_scalar_common_subexpression(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        provenance,
                        -1,
                        PhiTranslatedScalarGvnRewrite {
                            redundant: *redundant,
                            redundant_operation: *redundant_operation,
                            redundant_result: *redundant_result,
                            scalar_type: *scalar_type,
                            parameter_position,
                            incoming,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}

impl PhiTranslatedProofCertifiedScalarGvnRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.phi-translated-proof-certified-total-scalar-gvn.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(GLOBAL_VALUE_NUMBERING_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for PhiTranslatedProofCertifiedScalarGvnRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        if analyses.get(AnalysisKind::ControlFlowGraph).is_none() {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ControlFlowGraph,
            ));
        }
        let Some(AnalysisProduct::Dominators(dominators)) = analyses.get(AnalysisKind::Dominators)
        else {
            return Err(RuleProposalError::MissingAnalysis(AnalysisKind::Dominators));
        };
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::EffectSummaries(effects)) =
            analyses.get(AnalysisKind::EffectSummaries)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::EffectSummaries,
            ));
        };

        let mut candidates = Vec::new();
        for function in &unit.functions {
            let machine_dominators = dominators
                .functions
                .iter()
                .find(|(machine, _)| *machine == function.machine)
                .map(|(_, rows)| rows.as_slice())
                .unwrap_or_default();
            let value_types = function
                .parameters
                .iter()
                .map(|row| (row.value, row.scalar_type))
                .chain(function.blocks.iter().flat_map(|block| {
                    block
                        .parameters
                        .iter()
                        .map(|row| (row.value, row.scalar_type))
                }))
                .chain(function.blocks.iter().flat_map(|block| {
                    block.nodes.iter().flat_map(|node| {
                        node.definitions
                            .iter()
                            .map(|row| (row.value, row.scalar_type))
                    })
                }))
                .collect::<BTreeMap<_, _>>();
            let mut expressions = Vec::new();
            for block in &function.blocks {
                for (index, node) in block.nodes.iter().enumerate() {
                    let node_index =
                        u32::try_from(index).expect("optimization node index fits u32");
                    let Some((key, operation, result, scalar_type)) =
                        proof_certified_scalar_expression(&node.operation)
                    else {
                        continue;
                    };
                    let Some(obligation_fact) =
                        accepted_obligation_fact(unit, function.machine, operation).ok()
                    else {
                        continue;
                    };
                    if exact_pure_scalar_effect(
                        unit,
                        effects,
                        function.machine,
                        block.id,
                        node_index,
                    ) {
                        expressions.push((
                            key,
                            NodeLocation {
                                machine: function.machine,
                                block: block.id,
                                node: node_index,
                            },
                            operation,
                            result,
                            scalar_type,
                            obligation_fact,
                        ));
                    }
                }
            }

            for (
                key,
                redundant,
                redundant_operation,
                redundant_result,
                scalar_type,
                obligation_fact,
            ) in &expressions
            {
                let Some(block) = function
                    .blocks
                    .iter()
                    .find(|block| block.id == redundant.block)
                else {
                    continue;
                };
                let parameter_values = block
                    .parameters
                    .iter()
                    .map(|parameter| parameter.value)
                    .collect::<BTreeSet<_>>();
                if parameter_values.is_empty()
                    || !key.references_any(&parameter_values)
                    || !use_definitions.uses.iter().any(|(machine, use_site)| {
                        *machine == function.machine && use_site.value == *redundant_result
                    })
                {
                    continue;
                }
                let redundant_index = usize::try_from(redundant.node).expect("u32 fits usize");
                let Some(redundant_node) = block.nodes.get(redundant_index) else {
                    continue;
                };
                let Some(receiver) = block.nodes.get(redundant_index + 1) else {
                    continue;
                };
                if receiver
                    .provenance
                    .iter()
                    .any(|source| redundant_node.provenance.contains(source))
                {
                    continue;
                }

                let mut incoming = Vec::new();
                let mut complete = true;
                for source in &function.blocks {
                    for (owner_index, owner) in source.nodes.iter().enumerate() {
                        let owner_index =
                            u32::try_from(owner_index).expect("optimization node index fits u32");
                        for edge in owner
                            .successors
                            .iter()
                            .filter(|edge| edge.target == block.id)
                        {
                            if edge.bindings.len() != block.parameters.len() {
                                complete = false;
                                continue;
                            }
                            let mut translation = BTreeMap::new();
                            for (parameter, binding) in block.parameters.iter().zip(&edge.bindings)
                            {
                                if binding.parameter != parameter.value
                                    || binding.scalar_type != parameter.scalar_type
                                    || value_types.get(&binding.argument)
                                        != Some(&binding.scalar_type)
                                {
                                    complete = false;
                                    break;
                                }
                                translation.insert(parameter.value, binding.argument);
                            }
                            if !complete {
                                continue;
                            }
                            let translated_key = key.translate(&translation);
                            let leader = expressions
                                .iter()
                                .filter(|(candidate_key, location, _, _, candidate_type, _)| {
                                    candidate_key == &translated_key
                                        && candidate_type == scalar_type
                                        && ((location.block == source.id
                                            && location.node < owner_index)
                                            || (location.block != source.id
                                                && block_dominates(
                                                    machine_dominators,
                                                    location.block,
                                                    source.id,
                                                )))
                                })
                                .min_by_key(|(_, location, _, _, _, _)| {
                                    let depth = machine_dominators
                                        .iter()
                                        .find(|(candidate, _)| *candidate == location.block)
                                        .map_or(usize::MAX, |(_, rows)| rows.len());
                                    (depth, *location)
                                });
                            let Some((_, leader, leader_operation, leader_result, _, _)) = leader
                            else {
                                complete = false;
                                continue;
                            };
                            incoming.push(PhiTranslatedScalarIncoming {
                                source: source.id,
                                edge: edge.psi_edge,
                                leader: *leader,
                                leader_operation: *leader_operation,
                                leader_result: *leader_result,
                            });
                        }
                    }
                }
                if !complete || incoming.len() < 2 {
                    continue;
                }
                incoming.sort_by_key(|row| (row.edge, row.source));
                let Some((affected_blocks, provenance)) =
                    phi_translated_cse_accounting(function, *redundant, &incoming)
                else {
                    continue;
                };
                let parameter_position = u32::try_from(block.parameters.len())
                    .expect("optimization block parameter count fits u32");
                candidates.push(
                    PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        provenance,
                        *obligation_fact,
                        -1,
                        PhiTranslatedScalarGvnRewrite {
                            redundant: *redundant,
                            redundant_operation: *redundant_operation,
                            redundant_result: *redundant_result,
                            scalar_type: *scalar_type,
                            parameter_position,
                            incoming,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}

impl PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.phi-translated-proof-certified-compatible-policy-scalar-gvn.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(GLOBAL_VALUE_NUMBERING_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        if analyses.get(AnalysisKind::ControlFlowGraph).is_none() {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ControlFlowGraph,
            ));
        }
        let Some(AnalysisProduct::Dominators(dominators)) = analyses.get(AnalysisKind::Dominators)
        else {
            return Err(RuleProposalError::MissingAnalysis(AnalysisKind::Dominators));
        };
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::EffectSummaries(effects)) =
            analyses.get(AnalysisKind::EffectSummaries)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::EffectSummaries,
            ));
        };

        let mut candidates = Vec::new();
        for function in &unit.functions {
            let machine_dominators = dominators
                .functions
                .iter()
                .find(|(machine, _)| *machine == function.machine)
                .map(|(_, rows)| rows.as_slice())
                .unwrap_or_default();
            let value_types = function
                .parameters
                .iter()
                .map(|row| (row.value, row.scalar_type))
                .chain(function.blocks.iter().flat_map(|block| {
                    block
                        .parameters
                        .iter()
                        .map(|row| (row.value, row.scalar_type))
                }))
                .chain(function.blocks.iter().flat_map(|block| {
                    block.nodes.iter().flat_map(|node| {
                        node.definitions
                            .iter()
                            .map(|row| (row.value, row.scalar_type))
                    })
                }))
                .collect::<BTreeMap<_, _>>();
            let mut leaders = Vec::new();
            let mut redundants = Vec::new();
            for block in &function.blocks {
                for (index, node) in block.nodes.iter().enumerate() {
                    let node_index =
                        u32::try_from(index).expect("optimization node index fits u32");
                    let location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    };
                    if let Some((key, operation, result, scalar_type)) =
                        compatible_policy_scalar_leader(&node.operation)
                        && exact_pure_scalar_effect(
                            unit,
                            effects,
                            function.machine,
                            block.id,
                            node_index,
                        )
                        && !function.facts.iter().any(|fact| {
                            matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                                if *support == operation)
                        })
                    {
                        leaders.push((key, location, operation, result, scalar_type));
                    }
                    if let Some((key, operation, result, scalar_type)) =
                        compatible_policy_scalar_redundant(&node.operation)
                        && let Ok(obligation_fact) =
                            accepted_obligation_fact(unit, function.machine, operation)
                        && exact_pure_scalar_effect(
                            unit,
                            effects,
                            function.machine,
                            block.id,
                            node_index,
                        )
                    {
                        redundants.push((
                            key,
                            location,
                            operation,
                            result,
                            scalar_type,
                            obligation_fact,
                        ));
                    }
                }
            }

            for (
                key,
                redundant,
                redundant_operation,
                redundant_result,
                scalar_type,
                obligation_fact,
            ) in redundants
            {
                let Some(join) = function
                    .blocks
                    .iter()
                    .find(|block| block.id == redundant.block)
                else {
                    continue;
                };
                let parameter_values = join
                    .parameters
                    .iter()
                    .map(|parameter| parameter.value)
                    .collect::<BTreeSet<_>>();
                if join.id == function.entry
                    || parameter_values.is_empty()
                    || !key.references_any(&parameter_values)
                    || !use_definitions.uses.iter().any(|(machine, use_site)| {
                        *machine == function.machine && use_site.value == redundant_result
                    })
                {
                    continue;
                }
                let redundant_index = usize::try_from(redundant.node).expect("u32 fits usize");
                let Some(redundant_node) = join.nodes.get(redundant_index) else {
                    continue;
                };
                let Some(receiver) = join.nodes.get(redundant_index + 1) else {
                    continue;
                };
                if receiver
                    .provenance
                    .iter()
                    .any(|source| redundant_node.provenance.contains(source))
                {
                    continue;
                }

                let mut incoming = Vec::new();
                let mut complete = true;
                for source in &function.blocks {
                    for (owner_index, owner) in source.nodes.iter().enumerate() {
                        let owner_index =
                            u32::try_from(owner_index).expect("optimization node index fits u32");
                        for edge in owner
                            .successors
                            .iter()
                            .filter(|edge| edge.target == join.id)
                        {
                            if edge.bindings.len() != join.parameters.len() {
                                complete = false;
                                continue;
                            }
                            let mut translation = BTreeMap::new();
                            for (parameter, binding) in join.parameters.iter().zip(&edge.bindings) {
                                if binding.parameter != parameter.value
                                    || binding.scalar_type != parameter.scalar_type
                                    || value_types.get(&binding.argument)
                                        != Some(&binding.scalar_type)
                                {
                                    complete = false;
                                    break;
                                }
                                translation.insert(parameter.value, binding.argument);
                            }
                            if !complete {
                                continue;
                            }
                            let translated_key = key.translate(&translation);
                            let leader = leaders
                                .iter()
                                .filter(|(candidate_key, location, _, _, candidate_type)| {
                                    candidate_key == &translated_key
                                        && candidate_type == &scalar_type
                                        && ((location.block == source.id
                                            && location.node < owner_index)
                                            || (location.block != source.id
                                                && block_dominates(
                                                    machine_dominators,
                                                    location.block,
                                                    source.id,
                                                )))
                                })
                                .min_by_key(|(_, location, _, _, _)| {
                                    let depth = machine_dominators
                                        .iter()
                                        .find(|(candidate, _)| *candidate == location.block)
                                        .map_or(usize::MAX, |(_, rows)| rows.len());
                                    (depth, *location)
                                });
                            let Some((_, leader, leader_operation, leader_result, _)) = leader
                            else {
                                complete = false;
                                continue;
                            };
                            incoming.push(PhiTranslatedScalarIncoming {
                                source: source.id,
                                edge: edge.psi_edge,
                                leader: *leader,
                                leader_operation: *leader_operation,
                                leader_result: *leader_result,
                            });
                        }
                    }
                }
                if !complete || incoming.len() < 2 {
                    continue;
                }
                incoming.sort_by_key(|row| (row.edge, row.source));
                let Some((affected_blocks, provenance)) =
                    phi_translated_cse_accounting(function, redundant, &incoming)
                else {
                    continue;
                };
                let parameter_position = u32::try_from(join.parameters.len())
                    .expect("optimization block parameter count fits u32");
                candidates.push(
                    PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        provenance,
                        obligation_fact,
                        -1,
                        PhiTranslatedScalarGvnRewrite {
                            redundant,
                            redundant_operation,
                            redundant_result,
                            scalar_type,
                            parameter_position,
                            incoming,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}

fn exact_pure_scalar_effect(
    unit: &PsiOptimizationUnit,
    effects: &crate::EffectSummaryAnalysis,
    machine: MachineId,
    block: BlockId,
    node: u32,
) -> bool {
    effects.nodes.iter().any(|row| {
        row.revision == unit.identity
            && row.machine == machine
            && row.block == block
            && row.node == node
            && row.class == crate::EffectClass::PureScalar
            && row.observable == crate::EffectKnowledge::No
            && row.structural_state == crate::EffectKnowledge::No
            && row.crash == crate::EffectKnowledge::No
            && row.suspension == crate::EffectKnowledge::No
    })
}

#[derive(Debug, Clone, Copy)]
enum DeadScalarFamily {
    Literal,
    UnconditionallyTotal,
    ProofCertified,
}

impl DeadScalarLiteralEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.dead-unused-scalar-literal-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(DEAD_PURE_SCALAR_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::ValueLiveness, AnalysisKind::EffectSummaries]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ExactOperationSemantics,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for DeadScalarLiteralEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_dead_scalar_nodes(unit, analyses, Self::contract(), DeadScalarFamily::Literal)
    }
}

impl DeadUnconditionallyTotalScalarEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.dead-unused-unconditionally-total-scalar-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(DEAD_PURE_SCALAR_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::ValueLiveness, AnalysisKind::EffectSummaries]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ExactOperationSemantics,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for DeadUnconditionallyTotalScalarEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_dead_scalar_nodes(
            unit,
            analyses,
            Self::contract(),
            DeadScalarFamily::UnconditionallyTotal,
        )
    }
}

impl ProofCertifiedDeadScalarEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.dead-unused-proof-certified-scalar-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::ValueLiveness, AnalysisKind::EffectSummaries]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for ProofCertifiedDeadScalarEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_dead_scalar_nodes(
            unit,
            analyses,
            Self::contract(),
            DeadScalarFamily::ProofCertified,
        )
    }
}

impl LiveProofCertifiedIntegerIdentityEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-integer-identity-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ScalarConstants,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for LiveProofCertifiedIntegerIdentityEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_proof_certified_scalar_identities(
            unit,
            analyses,
            Self::contract(),
            proof_certified_scalar_identity_shapes,
        )
    }
}

impl LiveProofCertifiedIntegerDivideByOneEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-integer-divide-by-one-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ScalarConstants,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for LiveProofCertifiedIntegerDivideByOneEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_proof_certified_scalar_identities(
            unit,
            analyses,
            Self::contract(),
            proof_certified_integer_divide_by_one_shapes,
        )
    }
}

impl LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-exact-integer-multiply-by-zero-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ScalarConstants,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_proof_certified_scalar_identities(
            unit,
            analyses,
            Self::contract(),
            proof_certified_exact_integer_multiply_by_zero_shapes,
        )
    }
}

impl LiveProofCertifiedIntegerZeroDividendEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-integer-zero-dividend-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ScalarConstants,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for LiveProofCertifiedIntegerZeroDividendEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_proof_certified_scalar_identities(
            unit,
            analyses,
            Self::contract(),
            proof_certified_integer_zero_dividend_shapes,
        )
    }
}

impl LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-exact-integer-zero-value-shift-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ScalarConstants,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_proof_certified_scalar_identities(
            unit,
            analyses,
            Self::contract(),
            proof_certified_exact_integer_zero_value_shift_shapes,
        )
    }
}

impl LiveProofCertifiedExactIntegerSelfSubtractEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-exact-integer-self-subtract-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for LiveProofCertifiedExactIntegerSelfSubtractEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::EffectSummaries(effects)) =
            analyses.get(AnalysisKind::EffectSummaries)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::EffectSummaries,
            ));
        };
        let mut candidates = Vec::new();
        for function in &unit.functions {
            for block in &function.blocks {
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let O::ExactIntegerSubtract {
                        psi_operation,
                        obligation: _,
                        result,
                        scalar_type,
                        left,
                        right,
                    } = &node.operation
                    else {
                        continue;
                    };
                    if left != right
                        || !use_definitions.uses.iter().any(|(machine, use_site)| {
                            *machine == function.machine && use_site.value == *result
                        })
                    {
                        continue;
                    }
                    let node_index =
                        u32::try_from(node_index).expect("optimization node index fits u32");
                    let effect = effects.nodes.iter().find(|row| {
                        row.machine == function.machine
                            && row.block == block.id
                            && row.node == node_index
                    });
                    if effect.is_none_or(|row| {
                        row.revision != unit.identity
                            || row.class != crate::EffectClass::PureScalar
                            || row.observable != crate::EffectKnowledge::No
                            || row.structural_state != crate::EffectKnowledge::No
                            || row.crash != crate::EffectKnowledge::No
                            || row.suspension != crate::EffectKnowledge::No
                    }) {
                        continue;
                    }
                    let Ok(obligation_fact) =
                        accepted_obligation_fact(unit, function.machine, *psi_operation)
                    else {
                        continue;
                    };
                    let location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    };
                    let site = PsiRealizationSite::Node(location);
                    candidates.push(
                        PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
                            unit.identity,
                            Self::contract(),
                            vec![block.id],
                            vec![ProvenanceRewrite {
                                input: site,
                                disposition: ProvenanceDisposition::RealizedAt(site),
                                sources: node.provenance.clone(),
                                fuel: node.fuel.clone(),
                            }],
                            obligation_fact,
                            -1,
                            IntegerConstantRewrite {
                                location,
                                source_operation: *psi_operation,
                                result: *result,
                                scalar_type: *scalar_type,
                                constant: integer_zero(*scalar_type),
                            },
                        )
                        .map_err(RuleProposalError::InvalidCandidate)?,
                    );
                }
            }
        }
        Ok(candidates)
    }
}

impl LiveProofCertifiedIntegerSelfRemainderEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-integer-self-remainder-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for LiveProofCertifiedIntegerSelfRemainderEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::EffectSummaries(effects)) =
            analyses.get(AnalysisKind::EffectSummaries)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::EffectSummaries,
            ));
        };
        let mut candidates = Vec::new();
        for function in &unit.functions {
            for block in &function.blocks {
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let (psi_operation, result, scalar_type, left, right) = match &node.operation {
                        O::ExactIntegerRemainder {
                            psi_operation,
                            result,
                            scalar_type,
                            left,
                            right,
                            ..
                        }
                        | O::WrappingIntegerRemainder {
                            psi_operation,
                            result,
                            scalar_type,
                            left,
                            right,
                            ..
                        }
                        | O::SaturatingIntegerRemainder {
                            psi_operation,
                            result,
                            scalar_type,
                            left,
                            right,
                            ..
                        } => (psi_operation, result, scalar_type, left, right),
                        _ => continue,
                    };
                    if left != right
                        || !use_definitions.uses.iter().any(|(machine, use_site)| {
                            *machine == function.machine && use_site.value == *result
                        })
                    {
                        continue;
                    }
                    let node_index =
                        u32::try_from(node_index).expect("optimization node index fits u32");
                    let effect = effects.nodes.iter().find(|row| {
                        row.machine == function.machine
                            && row.block == block.id
                            && row.node == node_index
                    });
                    if effect.is_none_or(|row| {
                        row.revision != unit.identity
                            || row.class != crate::EffectClass::PureScalar
                            || row.observable != crate::EffectKnowledge::No
                            || row.structural_state != crate::EffectKnowledge::No
                            || row.crash != crate::EffectKnowledge::No
                            || row.suspension != crate::EffectKnowledge::No
                    }) {
                        continue;
                    }
                    let Ok(obligation_fact) =
                        accepted_obligation_fact(unit, function.machine, *psi_operation)
                    else {
                        continue;
                    };
                    let location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    };
                    let site = PsiRealizationSite::Node(location);
                    candidates.push(
                        PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
                            unit.identity,
                            Self::contract(),
                            vec![block.id],
                            vec![ProvenanceRewrite {
                                input: site,
                                disposition: ProvenanceDisposition::RealizedAt(site),
                                sources: node.provenance.clone(),
                                fuel: node.fuel.clone(),
                            }],
                            obligation_fact,
                            -1,
                            IntegerConstantRewrite {
                                location,
                                source_operation: *psi_operation,
                                result: *result,
                                scalar_type: *scalar_type,
                                constant: integer_zero(*scalar_type),
                            },
                        )
                        .map_err(RuleProposalError::InvalidCandidate)?,
                    );
                }
            }
        }
        Ok(candidates)
    }
}

impl LiveProofCertifiedIntegerSelfDivideEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-integer-self-divide-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for LiveProofCertifiedIntegerSelfDivideEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::EffectSummaries(effects)) =
            analyses.get(AnalysisKind::EffectSummaries)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::EffectSummaries,
            ));
        };
        let mut candidates = Vec::new();
        for function in &unit.functions {
            for block in &function.blocks {
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let (psi_operation, result, scalar_type, left, right) = match &node.operation {
                        O::ExactIntegerDivide {
                            psi_operation,
                            result,
                            scalar_type,
                            left,
                            right,
                            ..
                        }
                        | O::WrappingIntegerDivide {
                            psi_operation,
                            result,
                            scalar_type,
                            left,
                            right,
                            ..
                        }
                        | O::SaturatingIntegerDivide {
                            psi_operation,
                            result,
                            scalar_type,
                            left,
                            right,
                            ..
                        } => (psi_operation, result, scalar_type, left, right),
                        _ => continue,
                    };
                    if left != right
                        || scalar_type.carrier() != IntegerCarrier::Fixed
                        || (scalar_type.sign() == IntegerSign::Signed && scalar_type.bits() == 1)
                        || !use_definitions.uses.iter().any(|(machine, use_site)| {
                            *machine == function.machine && use_site.value == *result
                        })
                    {
                        continue;
                    }
                    let node_index =
                        u32::try_from(node_index).expect("optimization node index fits u32");
                    let effect = effects.nodes.iter().find(|row| {
                        row.machine == function.machine
                            && row.block == block.id
                            && row.node == node_index
                    });
                    if effect.is_none_or(|row| {
                        row.revision != unit.identity
                            || row.class != crate::EffectClass::PureScalar
                            || row.observable != crate::EffectKnowledge::No
                            || row.structural_state != crate::EffectKnowledge::No
                            || row.crash != crate::EffectKnowledge::No
                            || row.suspension != crate::EffectKnowledge::No
                    }) {
                        continue;
                    }
                    let Ok(obligation_fact) =
                        accepted_obligation_fact(unit, function.machine, *psi_operation)
                    else {
                        continue;
                    };
                    let location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    };
                    let site = PsiRealizationSite::Node(location);
                    candidates.push(
                        PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
                            unit.identity,
                            Self::contract(),
                            vec![block.id],
                            vec![ProvenanceRewrite {
                                input: site,
                                disposition: ProvenanceDisposition::RealizedAt(site),
                                sources: node.provenance.clone(),
                                fuel: node.fuel.clone(),
                            }],
                            obligation_fact,
                            -1,
                            IntegerConstantRewrite {
                                location,
                                source_operation: *psi_operation,
                                result: *result,
                                scalar_type: *scalar_type,
                                constant: integer_one(*scalar_type),
                            },
                        )
                        .map_err(RuleProposalError::InvalidCandidate)?,
                    );
                }
            }
        }
        Ok(candidates)
    }
}

impl LiveProofCertifiedIntegerRemainderByOneEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-integer-remainder-by-one-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ScalarConstants,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for LiveProofCertifiedIntegerRemainderByOneEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        let Some(AnalysisProduct::ScalarConstants(constants)) =
            analyses.get(AnalysisKind::ScalarConstants)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ScalarConstants,
            ));
        };
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::EffectSummaries(effects)) =
            analyses.get(AnalysisKind::EffectSummaries)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::EffectSummaries,
            ));
        };
        let mut candidates = Vec::new();
        for function in &unit.functions {
            for block in &function.blocks {
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let (psi_operation, result, scalar_type, left, right) = match &node.operation {
                        O::ExactIntegerRemainder {
                            psi_operation,
                            result,
                            scalar_type,
                            left,
                            right,
                            ..
                        }
                        | O::WrappingIntegerRemainder {
                            psi_operation,
                            result,
                            scalar_type,
                            left,
                            right,
                            ..
                        }
                        | O::SaturatingIntegerRemainder {
                            psi_operation,
                            result,
                            scalar_type,
                            left,
                            right,
                            ..
                        } => (psi_operation, result, scalar_type, left, right),
                        _ => continue,
                    };
                    // Keep the earlier self-remainder rule as the sole owner
                    // of the overlapping `1 % 1`/`x % x` shape.
                    if left == right
                        || scalar_type.carrier() != IntegerCarrier::Fixed
                        || !use_definitions.uses.iter().any(|(machine, use_site)| {
                            *machine == function.machine && use_site.value == *result
                        })
                    {
                        continue;
                    }
                    let Some((constant, constant_fact)) =
                        literal_integer_constant(constants, function.machine, *right)
                    else {
                        continue;
                    };
                    if constant != integer_one(*scalar_type) || !scalar_type.admits(constant) {
                        continue;
                    }
                    let node_index =
                        u32::try_from(node_index).expect("optimization node index fits u32");
                    let effect = effects.nodes.iter().find(|row| {
                        row.machine == function.machine
                            && row.block == block.id
                            && row.node == node_index
                    });
                    if effect.is_none_or(|row| {
                        row.revision != unit.identity
                            || row.class != crate::EffectClass::PureScalar
                            || row.observable != crate::EffectKnowledge::No
                            || row.structural_state != crate::EffectKnowledge::No
                            || row.crash != crate::EffectKnowledge::No
                            || row.suspension != crate::EffectKnowledge::No
                    }) {
                        continue;
                    }
                    let Ok(obligation_fact) =
                        accepted_obligation_fact(unit, function.machine, *psi_operation)
                    else {
                        continue;
                    };
                    let location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    };
                    let site = PsiRealizationSite::Node(location);
                    candidates.push(
                        PsiRewriteCandidate::new_literal_proof_certified_integer_constant_replacement(
                            unit.identity,
                            Self::contract(),
                            vec![block.id],
                            vec![ProvenanceRewrite {
                                input: site,
                                disposition: ProvenanceDisposition::RealizedAt(site),
                                sources: node.provenance.clone(),
                                fuel: node.fuel.clone(),
                            }],
                            constant_fact,
                            obligation_fact,
                            -1,
                            IntegerConstantRewrite {
                                location,
                                source_operation: *psi_operation,
                                result: *result,
                                scalar_type: *scalar_type,
                                constant: integer_zero(*scalar_type),
                            },
                        )
                        .map_err(RuleProposalError::InvalidCandidate)?,
                    );
                }
            }
        }
        Ok(candidates)
    }
}

impl LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-signed-integer-remainder-by-negative-one-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ScalarConstants,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        let Some(AnalysisProduct::ScalarConstants(constants)) =
            analyses.get(AnalysisKind::ScalarConstants)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ScalarConstants,
            ));
        };
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::EffectSummaries(effects)) =
            analyses.get(AnalysisKind::EffectSummaries)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::EffectSummaries,
            ));
        };
        let mut candidates = Vec::new();
        for function in &unit.functions {
            for block in &function.blocks {
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let (psi_operation, result, scalar_type, left, right) = match &node.operation {
                        O::ExactIntegerRemainder {
                            psi_operation,
                            result,
                            scalar_type,
                            left,
                            right,
                            ..
                        }
                        | O::WrappingIntegerRemainder {
                            psi_operation,
                            result,
                            scalar_type,
                            left,
                            right,
                            ..
                        }
                        | O::SaturatingIntegerRemainder {
                            psi_operation,
                            result,
                            scalar_type,
                            left,
                            right,
                            ..
                        } => (psi_operation, result, scalar_type, left, right),
                        _ => continue,
                    };
                    if left == right
                        || scalar_type.carrier() != IntegerCarrier::Fixed
                        || scalar_type.sign() != IntegerSign::Signed
                        || !use_definitions.uses.iter().any(|(machine, use_site)| {
                            *machine == function.machine && use_site.value == *result
                        })
                    {
                        continue;
                    }
                    let Some((constant, constant_fact)) =
                        literal_integer_constant(constants, function.machine, *right)
                    else {
                        continue;
                    };
                    if constant != IntegerValue::Signed(-1) || !scalar_type.admits(constant) {
                        continue;
                    }
                    let node_index =
                        u32::try_from(node_index).expect("optimization node index fits u32");
                    let effect = effects.nodes.iter().find(|row| {
                        row.machine == function.machine
                            && row.block == block.id
                            && row.node == node_index
                    });
                    if effect.is_none_or(|row| {
                        row.revision != unit.identity
                            || row.class != crate::EffectClass::PureScalar
                            || row.observable != crate::EffectKnowledge::No
                            || row.structural_state != crate::EffectKnowledge::No
                            || row.crash != crate::EffectKnowledge::No
                            || row.suspension != crate::EffectKnowledge::No
                    }) {
                        continue;
                    }
                    let Ok(obligation_fact) =
                        accepted_obligation_fact(unit, function.machine, *psi_operation)
                    else {
                        continue;
                    };
                    let location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    };
                    let site = PsiRealizationSite::Node(location);
                    candidates.push(
                        PsiRewriteCandidate::new_literal_proof_certified_integer_constant_replacement(
                            unit.identity,
                            Self::contract(),
                            vec![block.id],
                            vec![ProvenanceRewrite {
                                input: site,
                                disposition: ProvenanceDisposition::RealizedAt(site),
                                sources: node.provenance.clone(),
                                fuel: node.fuel.clone(),
                            }],
                            constant_fact,
                            obligation_fact,
                            -1,
                            IntegerConstantRewrite {
                                location,
                                source_operation: *psi_operation,
                                result: *result,
                                scalar_type: *scalar_type,
                                constant: integer_zero(*scalar_type),
                            },
                        )
                        .map_err(RuleProposalError::InvalidCandidate)?,
                    );
                }
            }
        }
        Ok(candidates)
    }
}

fn propose_proof_certified_scalar_identities(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    shapes_for_operation: fn(&O) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)>,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let Some(AnalysisProduct::ScalarConstants(constants)) =
        analyses.get(AnalysisKind::ScalarConstants)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ScalarConstants,
        ));
    };
    let Some(AnalysisProduct::UseDefinition(use_definitions)) =
        analyses.get(AnalysisKind::UseDefinition)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::UseDefinition,
        ));
    };
    let Some(AnalysisProduct::EffectSummaries(effects)) =
        analyses.get(AnalysisKind::EffectSummaries)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::EffectSummaries,
        ));
    };
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let shapes = shapes_for_operation(&node.operation);
                if shapes.is_empty() {
                    continue;
                }
                let node_index =
                    u32::try_from(node_index).expect("optimization node index fits u32");
                let effect = effects.nodes.iter().find(|row| {
                    row.machine == function.machine
                        && row.block == block.id
                        && row.node == node_index
                });
                if effect.is_none_or(|row| {
                    row.revision != unit.identity
                        || row.class != crate::EffectClass::PureScalar
                        || row.observable != crate::EffectKnowledge::No
                        || row.structural_state != crate::EffectKnowledge::No
                        || row.crash != crate::EffectKnowledge::No
                        || row.suspension != crate::EffectKnowledge::No
                }) {
                    continue;
                }
                let Some((patch_shape, constant_fact)) =
                    shapes.into_iter().find_map(|(shape, expected)| {
                        let (actual, fact) = literal_integer_constant(
                            constants,
                            function.machine,
                            shape.identity_operand,
                        )?;
                        (actual == expected).then_some((shape, fact))
                    })
                else {
                    continue;
                };
                if !use_definitions.uses.iter().any(|(machine, use_site)| {
                    *machine == function.machine && use_site.value == patch_shape.result
                }) {
                    continue;
                }
                let Ok(obligation_fact) =
                    accepted_obligation_fact(unit, function.machine, patch_shape.source_operation)
                else {
                    continue;
                };
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                };
                let Some((affected_blocks, provenance)) =
                    local_cse_accounting(function, location, patch_shape.result)
                else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_proof_certified_scalar_identity(
                        unit.identity,
                        contract,
                        affected_blocks,
                        provenance,
                        constant_fact,
                        obligation_fact,
                        -1,
                        ProofCertifiedScalarIdentityRewrite {
                            location,
                            source_operation: patch_shape.source_operation,
                            result: patch_shape.result,
                            replacement: patch_shape.replacement,
                            scalar_type: patch_shape.scalar_type,
                            identity: patch_shape.identity,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}

#[derive(Debug, Clone, Copy)]
struct ProofCertifiedScalarIdentityShape {
    source_operation: OperationId,
    result: ValueId,
    replacement: ValueId,
    identity_operand: ValueId,
    scalar_type: IntegerType,
    identity: ProofCertifiedScalarIdentityKind,
}

fn proof_certified_scalar_identity_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    match operation {
        O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => vec![
            (
                ProofCertifiedScalarIdentityShape {
                    source_operation: *psi_operation,
                    result: *result,
                    replacement: *right,
                    identity_operand: *left,
                    scalar_type: *scalar_type,
                    identity: ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroLeft,
                },
                integer_zero(*scalar_type),
            ),
            (
                ProofCertifiedScalarIdentityShape {
                    source_operation: *psi_operation,
                    result: *result,
                    replacement: *left,
                    identity_operand: *right,
                    scalar_type: *scalar_type,
                    identity: ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroRight,
                },
                integer_zero(*scalar_type),
            ),
        ],
        O::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => vec![(
            ProofCertifiedScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *left,
                identity_operand: *right,
                scalar_type: *scalar_type,
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerSubtractZeroRight,
            },
            integer_zero(*scalar_type),
        )],
        O::ExactIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => vec![
            (
                ProofCertifiedScalarIdentityShape {
                    source_operation: *psi_operation,
                    result: *result,
                    replacement: *right,
                    identity_operand: *left,
                    scalar_type: *scalar_type,
                    identity: ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneLeft,
                },
                integer_one(*scalar_type),
            ),
            (
                ProofCertifiedScalarIdentityShape {
                    source_operation: *psi_operation,
                    result: *result,
                    replacement: *left,
                    identity_operand: *right,
                    scalar_type: *scalar_type,
                    identity: ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneRight,
                },
                integer_one(*scalar_type),
            ),
        ],
        O::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count,
            value,
            count_type,
            ..
        } => vec![(
            ProofCertifiedScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *value,
                identity_operand: *count,
                scalar_type: *value_type,
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroCount,
            },
            integer_zero(*count_type),
        )],
        O::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count,
            value,
            count_type,
            ..
        } => vec![(
            ProofCertifiedScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *value,
                identity_operand: *count,
                scalar_type: *value_type,
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroCount,
            },
            integer_zero(*count_type),
        )],
        _ => Vec::new(),
    }
}

fn proof_certified_integer_divide_by_one_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let (source_operation, result, scalar_type, left, right, identity) = match operation {
        O::ExactIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight,
        ),
        O::WrappingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight,
        ),
        O::SaturatingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideOneRight,
        ),
        _ => return Vec::new(),
    };
    vec![(
        ProofCertifiedScalarIdentityShape {
            source_operation,
            result,
            replacement: left,
            identity_operand: right,
            scalar_type,
            identity,
        },
        integer_one(scalar_type),
    )]
}

fn proof_certified_exact_integer_multiply_by_zero_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let O::ExactIntegerMultiply {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
        ..
    } = operation
    else {
        return Vec::new();
    };
    vec![
        (
            ProofCertifiedScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *left,
                identity_operand: *left,
                scalar_type: *scalar_type,
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroLeft,
            },
            integer_zero(*scalar_type),
        ),
        (
            ProofCertifiedScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *right,
                identity_operand: *right,
                scalar_type: *scalar_type,
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight,
            },
            integer_zero(*scalar_type),
        ),
    ]
}

fn proof_certified_integer_zero_dividend_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let (source_operation, result, scalar_type, left, identity) = match operation {
        O::ExactIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            ProofCertifiedScalarIdentityKind::ExactIntegerDivideZeroLeft,
        ),
        O::WrappingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            ProofCertifiedScalarIdentityKind::WrappingIntegerDivideZeroLeft,
        ),
        O::SaturatingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideZeroLeft,
        ),
        O::ExactIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            ProofCertifiedScalarIdentityKind::ExactIntegerRemainderZeroLeft,
        ),
        O::WrappingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            ProofCertifiedScalarIdentityKind::WrappingIntegerRemainderZeroLeft,
        ),
        O::SaturatingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            ProofCertifiedScalarIdentityKind::SaturatingIntegerRemainderZeroLeft,
        ),
        _ => return Vec::new(),
    };
    vec![(
        ProofCertifiedScalarIdentityShape {
            source_operation,
            result,
            replacement: left,
            identity_operand: left,
            scalar_type,
            identity,
        },
        integer_zero(scalar_type),
    )]
}

fn proof_certified_exact_integer_zero_value_shift_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let (source_operation, result, scalar_type, value, identity) = match operation {
        O::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            value,
            ..
        } => (
            *psi_operation,
            *result,
            *value_type,
            *value,
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroValue,
        ),
        O::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            value,
            ..
        } => (
            *psi_operation,
            *result,
            *value_type,
            *value,
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroValue,
        ),
        _ => return Vec::new(),
    };
    vec![(
        ProofCertifiedScalarIdentityShape {
            source_operation,
            result,
            replacement: value,
            identity_operand: value,
            scalar_type,
            identity,
        },
        integer_zero(scalar_type),
    )]
}

fn integer_zero(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        psi_core::IntegerSign::Signed => IntegerValue::Signed(0),
        psi_core::IntegerSign::Unsigned => IntegerValue::Unsigned(0),
    }
}

fn integer_one(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        psi_core::IntegerSign::Signed => IntegerValue::Signed(1),
        psi_core::IntegerSign::Unsigned => IntegerValue::Unsigned(1),
    }
}

fn propose_dead_scalar_nodes(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    family: DeadScalarFamily,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let Some(AnalysisProduct::ValueLiveness(liveness)) = analyses.get(AnalysisKind::ValueLiveness)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ValueLiveness,
        ));
    };
    let Some(AnalysisProduct::EffectSummaries(effects)) =
        analyses.get(AnalysisKind::EffectSummaries)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::EffectSummaries,
        ));
    };
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let Some((source_operation, result, scalar_type)) =
                    dead_scalar_shape(&node.operation, family)
                else {
                    continue;
                };
                let Some(next) = block.nodes.get(node_index + 1) else {
                    continue;
                };
                if next
                    .provenance
                    .iter()
                    .any(|source| node.provenance.contains(source))
                {
                    continue;
                }
                let node_index =
                    u32::try_from(node_index).expect("optimization node index fits u32");
                let live = liveness
                    .blocks
                    .iter()
                    .find(|row| row.machine == function.machine && row.block == block.id)
                    .and_then(|row| row.nodes.iter().find(|row| row.node == node_index));
                let effect = effects.nodes.iter().find(|row| {
                    row.machine == function.machine
                        && row.block == block.id
                        && row.node == node_index
                });
                if live.is_none_or(|row| row.exit.contains(&result))
                    || effect.is_none_or(|row| {
                        row.revision != unit.identity
                            || row.class != crate::EffectClass::PureScalar
                            || row.observable != crate::EffectKnowledge::No
                            || row.structural_state != crate::EffectKnowledge::No
                            || row.crash != crate::EffectKnowledge::No
                            || row.suspension != crate::EffectKnowledge::No
                    })
                {
                    continue;
                }
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                };
                let Some((affected_blocks, provenance)) =
                    dead_scalar_node_accounting(function, location)
                else {
                    continue;
                };
                let patch = DeadScalarNodeRewrite {
                    location,
                    source_operation,
                    result,
                    scalar_type,
                };
                let candidate = if matches!(family, DeadScalarFamily::ProofCertified) {
                    PsiRewriteCandidate::new_proof_certified_dead_scalar_node(
                        unit.identity,
                        contract,
                        affected_blocks,
                        provenance,
                        accepted_obligation_fact(unit, function.machine, source_operation)?,
                        -1,
                        patch,
                    )
                } else {
                    PsiRewriteCandidate::new_dead_scalar_node(
                        unit.identity,
                        contract,
                        affected_blocks,
                        provenance,
                        -1,
                        patch,
                    )
                };
                candidates.push(candidate.map_err(RuleProposalError::InvalidCandidate)?);
            }
        }
    }
    Ok(candidates)
}

fn dead_scalar_shape(
    operation: &O,
    family: DeadScalarFamily,
) -> Option<(OperationId, ValueId, psi_core::ScalarType)> {
    match (family, operation) {
        (
            DeadScalarFamily::Literal,
            O::IntegerConstant {
                psi_operation,
                result,
                scalar_type,
                ..
            },
        ) => Some((*psi_operation, *result, *scalar_type)),
        (
            DeadScalarFamily::Literal,
            O::BooleanConstant {
                psi_operation,
                result,
                ..
            },
        ) => Some((*psi_operation, *result, psi_core::ScalarType::Boolean)),
        (
            DeadScalarFamily::UnconditionallyTotal,
            O::BooleanNot {
                psi_operation,
                result,
                ..
            }
            | O::BooleanEqual {
                psi_operation,
                result,
                ..
            }
            | O::IntegerEqual {
                psi_operation,
                result,
                ..
            }
            | O::IntegerLessThan {
                psi_operation,
                result,
                ..
            }
            | O::IntegerLessOrEqual {
                psi_operation,
                result,
                ..
            },
        ) => Some((*psi_operation, *result, psi_core::ScalarType::Boolean)),
        (
            DeadScalarFamily::UnconditionallyTotal,
            O::IntegerBitwiseNot {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::IntegerBitwiseAnd {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::IntegerBitwiseOr {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::IntegerBitwiseXor {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                ..
            },
        ) => Some((
            *psi_operation,
            *result,
            psi_core::ScalarType::Integer(*scalar_type),
        )),
        (
            DeadScalarFamily::UnconditionallyTotal,
            O::IntegerWiden {
                psi_operation,
                result,
                target_type,
                ..
            },
        ) => Some((
            *psi_operation,
            *result,
            psi_core::ScalarType::Integer(*target_type),
        )),
        (
            DeadScalarFamily::UnconditionallyTotal,
            O::WrappingIntegerShiftLeft {
                psi_operation,
                result,
                value_type,
                ..
            }
            | O::WrappingIntegerShiftRight {
                psi_operation,
                result,
                value_type,
                ..
            },
        ) => Some((
            *psi_operation,
            *result,
            psi_core::ScalarType::Integer(*value_type),
        )),
        (
            DeadScalarFamily::ProofCertified,
            O::IntegerExactCast {
                psi_operation,
                result,
                target_type,
                ..
            },
        ) => Some((
            *psi_operation,
            *result,
            psi_core::ScalarType::Integer(*target_type),
        )),
        (
            DeadScalarFamily::ProofCertified,
            O::ExactIntegerShiftLeft {
                psi_operation,
                result,
                value_type,
                ..
            }
            | O::ExactIntegerShiftRight {
                psi_operation,
                result,
                value_type,
                ..
            },
        ) => Some((
            *psi_operation,
            *result,
            psi_core::ScalarType::Integer(*value_type),
        )),
        (
            DeadScalarFamily::ProofCertified,
            O::ExactIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::ExactIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::ExactIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::ExactIntegerDivide {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::ExactIntegerRemainder {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerDivide {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerRemainder {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerDivide {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerRemainder {
                psi_operation,
                result,
                scalar_type,
                ..
            },
        ) => Some((
            *psi_operation,
            *result,
            psi_core::ScalarType::Integer(*scalar_type),
        )),
        _ => None,
    }
}

impl UnreachablePrivateMachinePruneRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.unreachable-private-machine-pruning.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(CONTROL_FLOW_CLEANUP_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::CallGraph]),
            AnalysisInvalidationSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::CallGraph,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::StructuralIdentity,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for UnreachablePrivateMachinePruneRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        let Some(AnalysisProduct::CallGraph(call_graph)) = analyses.get(AnalysisKind::CallGraph)
        else {
            return Err(RuleProposalError::MissingAnalysis(AnalysisKind::CallGraph));
        };
        let machines = rule_unreachable_private_machine_complement(unit, call_graph);
        if machines.is_empty() {
            return Ok(Vec::new());
        }
        let ordinals = rule_active_source_ordinals(unit);
        let custody = machines
            .iter()
            .map(|machine| PrunedMachineCustody {
                machine: *machine,
                source_ordinal: ordinals[machine],
            })
            .collect::<Vec<_>>();
        let Some(provenance) = rule_pruned_machine_provenance(unit, &machines) else {
            return Ok(Vec::new());
        };
        Ok(vec![
            PsiRewriteCandidate::new_unreachable_private_machines(
                unit.identity,
                Self::contract(),
                provenance,
                -i64::try_from(machines.len()).unwrap_or(i64::MAX),
                UnreachablePrivateMachinesRewrite { machines: custody },
            )
            .map_err(RuleProposalError::InvalidCandidate)?,
        ])
    }
}

fn rule_active_source_ordinals(unit: &PsiOptimizationUnit) -> BTreeMap<MachineId, u32> {
    let pruned = unit
        .pruned_machines
        .iter()
        .map(|row| (row.source_ordinal, row.machine))
        .collect::<BTreeMap<_, _>>();
    let mut active = unit.functions.iter();
    let mut result = BTreeMap::new();
    let total = unit.functions.len() + unit.pruned_machines.len();
    for ordinal in 0..total {
        let ordinal = u32::try_from(ordinal).expect("function ordinal fits u32");
        if !pruned.contains_key(&ordinal) {
            let function = active
                .next()
                .expect("validated roster has active source member");
            result.insert(function.machine, ordinal);
        }
    }
    result
}

fn rule_unreachable_private_machine_complement(
    unit: &PsiOptimizationUnit,
    call_graph: &crate::CallGraphAnalysis,
) -> Vec<MachineId> {
    let active = unit
        .functions
        .iter()
        .map(|function| function.machine)
        .collect::<BTreeSet<_>>();
    let mut reachable = BTreeSet::from([unit.entry]);
    reachable.extend(
        unit.provider_candidates
            .iter()
            .map(|candidate| candidate.candidate),
    );
    reachable.extend(
        unit.functions
            .iter()
            .filter(|function| function.attachment.is_some())
            .map(|function| function.machine),
    );
    let mut references = call_graph
        .callees
        .iter()
        .map(|(machine, callees)| (*machine, callees.iter().copied().collect::<BTreeSet<_>>()))
        .collect::<BTreeMap<_, _>>();
    for function in &unit.functions {
        let function_references = references.entry(function.machine).or_default();
        for operation in function
            .blocks
            .iter()
            .flat_map(|block| block.nodes.iter().map(|node| &node.operation))
        {
            match operation {
                O::Return {
                    cleanup_actions, ..
                }
                | O::ReturnUnit {
                    cleanup_actions, ..
                } => {
                    function_references.extend(cleanup_actions.iter().filter_map(|action| {
                        match action {
                            psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                                Some(cleanup.cleanup_machine)
                            }
                            _ => None,
                        }
                    }));
                }
                _ => {}
            }
        }
    }
    let mut work = reachable.iter().copied().collect::<Vec<_>>();
    while let Some(machine) = work.pop() {
        for callee in references.get(&machine).into_iter().flatten().copied() {
            if active.contains(&callee) && reachable.insert(callee) {
                work.push(callee);
            }
        }
    }
    active.difference(&reachable).copied().collect()
}

fn rule_pruned_machine_provenance(
    unit: &PsiOptimizationUnit,
    machines: &[MachineId],
) -> Option<Vec<ProvenanceRewrite>> {
    let machines = machines.iter().copied().collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for function in unit
        .functions
        .iter()
        .filter(|function| machines.contains(&function.machine))
    {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let input = PsiRealizationSite::Node(NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).ok()?,
                });
                if !node.provenance.is_empty() {
                    rows.push(ProvenanceRewrite {
                        input,
                        disposition: ProvenanceDisposition::ProvenUnreachableAt(input),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
                for edge in &node.successors {
                    let input = PsiRealizationSite::Edge {
                        machine: function.machine,
                        edge: edge.psi_edge,
                    };
                    if !edge.provenance.is_empty() {
                        rows.push(ProvenanceRewrite {
                            input,
                            disposition: ProvenanceDisposition::ProvenUnreachableAt(input),
                            sources: edge.provenance.clone(),
                            fuel: edge.fuel.clone(),
                        });
                    }
                }
            }
        }
    }
    rows.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some(rows)
}

impl ConstantConditionalFoldRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.constant-conditional-fold.v5",
            ),
            OptimizationPassIdentity::from_canonical_bytes(CONTROL_FLOW_CLEANUP_PASS_NAME),
            5,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::ScalarConstants,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::CallGraph,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ExactOperationSemantics,
        )
        .expect("built-in rule has nonzero version")
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LinearEmptyBlockThreadRule;

impl LinearEmptyBlockThreadRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.linear-empty-block-thread.v2",
            ),
            OptimizationPassIdentity::from_canonical_bytes(CONTROL_FLOW_CLEANUP_PASS_NAME),
            2,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::UseDefinition,
                AnalysisKind::OwnershipFrontiers,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::StructuralIdentity,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for LinearEmptyBlockThreadRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        if analyses.get(AnalysisKind::ControlFlowGraph).is_none() {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ControlFlowGraph,
            ));
        }
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::OwnershipFrontiers(frontiers)) =
            analyses.get(AnalysisKind::OwnershipFrontiers)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::OwnershipFrontiers,
            ));
        };

        let mut candidates = Vec::new();
        for function in &unit.functions {
            for empty in &function.blocks {
                if empty.id == function.entry || empty.nodes.len() != 1 {
                    continue;
                }
                let O::Jump {
                    psi_edge: outgoing_edge,
                    target,
                    bindings: outgoing_bindings,
                    trivial_affine_discards: outgoing_discards,
                } = &empty.nodes[0].operation
                else {
                    continue;
                };
                let incoming = function
                    .blocks
                    .iter()
                    .flat_map(|block| {
                        block
                            .nodes
                            .iter()
                            .enumerate()
                            .filter_map(move |(node, candidate)| {
                                candidate
                                    .successors
                                    .iter()
                                    .any(|edge| edge.target == empty.id)
                                    .then_some((block, node, candidate))
                            })
                    })
                    .collect::<Vec<_>>();
                let [(predecessor_block, predecessor_node_index, predecessor_node)] =
                    incoming.as_slice()
                else {
                    continue;
                };
                let O::Jump {
                    psi_edge: incoming_edge,
                    target: predecessor_target,
                    bindings: incoming_bindings,
                    trivial_affine_discards: incoming_discards,
                } = &predecessor_node.operation
                else {
                    continue;
                };
                if !incoming_discards.is_empty()
                    || !outgoing_discards.is_empty()
                    || *predecessor_target != empty.id
                    || empty.parameters.iter().any(|parameter| {
                        use_definitions.uses.iter().any(|(machine, use_site)| {
                            *machine == function.machine
                                && use_site.value == parameter.value
                                && (use_site.block != empty.id || use_site.node != 0)
                        })
                    })
                    || !linear_thread_ownership_is_identity(
                        unit,
                        function,
                        frontiers,
                        *incoming_edge,
                        empty.id,
                        *outgoing_edge,
                        *target,
                    )
                {
                    continue;
                }
                let Some(_) = compose_linear_thread_bindings(
                    &empty.parameters,
                    incoming_bindings,
                    outgoing_bindings,
                ) else {
                    continue;
                };
                let predecessor = NodeLocation {
                    machine: function.machine,
                    block: predecessor_block.id,
                    node: u32::try_from(*predecessor_node_index)
                        .expect("optimization node indices are u32"),
                };
                let empty_location = NodeLocation {
                    machine: function.machine,
                    block: empty.id,
                    node: 0,
                };
                let Some((affected_blocks, provenance)) =
                    linear_thread_accounting(function, predecessor, empty_location)
                else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_linear_empty_block(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        provenance,
                        -3,
                        LinearEmptyBlockRewrite {
                            predecessor,
                            incoming_edge: *incoming_edge,
                            empty: empty_location,
                            outgoing_edge: *outgoing_edge,
                            target: *target,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PathQualifiedEmptyBlockThreadRule;

impl PathQualifiedEmptyBlockThreadRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.path-qualified-empty-block-thread.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(CONTROL_FLOW_CLEANUP_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::UseDefinition,
                AnalysisKind::OwnershipFrontiers,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::StructuralIdentity,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for PathQualifiedEmptyBlockThreadRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        if analyses.get(AnalysisKind::ControlFlowGraph).is_none() {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ControlFlowGraph,
            ));
        }
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::OwnershipFrontiers(frontiers)) =
            analyses.get(AnalysisKind::OwnershipFrontiers)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::OwnershipFrontiers,
            ));
        };
        let mut candidates = Vec::new();
        for function in &unit.functions {
            for empty in &function.blocks {
                if empty.id == function.entry || empty.nodes.len() != 1 {
                    continue;
                }
                let O::Jump {
                    psi_edge: outgoing_edge,
                    target,
                    bindings: outgoing_bindings,
                    trivial_affine_discards: outgoing_discards,
                } = &empty.nodes[0].operation
                else {
                    continue;
                };
                let incoming = function
                    .blocks
                    .iter()
                    .flat_map(|block| {
                        block
                            .nodes
                            .iter()
                            .enumerate()
                            .flat_map(move |(node_index, node)| {
                                node.successors
                                    .iter()
                                    .filter(move |edge| edge.target == empty.id)
                                    .map(move |edge| (block, node_index, node, edge))
                            })
                    })
                    .collect::<Vec<_>>();
                if !outgoing_discards.is_empty()
                    || incoming.is_empty()
                    || (incoming.len() == 1 && matches!(incoming[0].2.operation, O::Jump { .. }))
                    || incoming
                        .iter()
                        .any(|(_, _, _, edge)| !edge.trivial_affine_discards.is_empty())
                    || empty.parameters.iter().any(|parameter| {
                        use_definitions.uses.iter().any(|(machine, use_site)| {
                            *machine == function.machine
                                && use_site.value == parameter.value
                                && (use_site.block != empty.id || use_site.node != 0)
                        })
                    })
                {
                    continue;
                }
                if incoming.iter().any(|(_, _, _, edge)| {
                    compose_linear_thread_bindings(
                        &empty.parameters,
                        &edge.bindings,
                        outgoing_bindings,
                    )
                    .is_none()
                        || !linear_thread_ownership_is_identity(
                            unit,
                            function,
                            frontiers,
                            edge.psi_edge,
                            empty.id,
                            *outgoing_edge,
                            *target,
                        )
                }) {
                    continue;
                }
                let empty_location = NodeLocation {
                    machine: function.machine,
                    block: empty.id,
                    node: 0,
                };
                let incoming_edges = incoming
                    .iter()
                    .map(|(_, _, _, edge)| edge.psi_edge)
                    .collect::<Vec<_>>();
                let Some((affected_blocks, provenance)) =
                    path_thread_accounting(function, empty_location, &incoming_edges)
                else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_path_qualified_empty_block(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        provenance,
                        -3,
                        PathQualifiedEmptyBlockRewrite {
                            empty: empty_location,
                            outgoing_edge: *outgoing_edge,
                            target: *target,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AdjacentBlockMergeRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct NonAdjacentBlockMergeRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct SharedJumpFusionRule;

impl SharedJumpFusionRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.shared-terminal-jump-fusion.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(CONTROL_FLOW_CLEANUP_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::OwnershipFrontiers,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::StructuralIdentity,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for SharedJumpFusionRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        if analyses.get(AnalysisKind::ControlFlowGraph).is_none() {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ControlFlowGraph,
            ));
        }
        let Some(AnalysisProduct::OwnershipFrontiers(frontiers)) =
            analyses.get(AnalysisKind::OwnershipFrontiers)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::OwnershipFrontiers,
            ));
        };
        let mut candidates = Vec::new();
        for function in &unit.functions {
            for predecessor in &function.blocks {
                let Some((predecessor_index, predecessor_node)) = predecessor
                    .nodes
                    .len()
                    .checked_sub(1)
                    .map(|index| (index, &predecessor.nodes[index]))
                else {
                    continue;
                };
                let O::Jump {
                    psi_edge: incoming_edge,
                    target: target_id,
                    bindings,
                    trivial_affine_discards,
                } = &predecessor_node.operation
                else {
                    continue;
                };
                let Some(target) = function.blocks.iter().find(|block| block.id == *target_id)
                else {
                    continue;
                };
                let [terminal] = target.nodes.as_slice() else {
                    continue;
                };
                if !trivial_affine_discards.is_empty()
                    || target.id == function.entry
                    || !terminal.successors.is_empty()
                    || !matches!(terminal.provenance.first(), Some(PsiProvenance::Edge(_)))
                    || !matches!(
                        terminal.operation,
                        O::Return { .. }
                            | O::ReturnUnit { .. }
                            | O::ReturnStructural { .. }
                            | O::Crash { .. }
                    )
                {
                    continue;
                }
                let incoming_count = function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.nodes)
                    .flat_map(|node| &node.successors)
                    .filter(|edge| edge.target == target.id)
                    .count();
                if incoming_count < 2
                    || !adjacent_merge_ownership_is_identity(
                        unit,
                        function,
                        frontiers,
                        *incoming_edge,
                        target.id,
                    )
                {
                    continue;
                }
                let Some(mut substitutions) = target
                    .parameters
                    .iter()
                    .zip(bindings)
                    .map(|(parameter, binding)| {
                        (binding.parameter == parameter.value
                            && binding.scalar_type == parameter.scalar_type)
                            .then_some(ScalarSubstitution {
                                from: parameter.value,
                                to: binding.argument,
                                scalar_type: parameter.scalar_type,
                            })
                    })
                    .collect::<Option<Vec<_>>>()
                    .filter(|_| target.parameters.len() == bindings.len())
                else {
                    continue;
                };
                substitutions.sort();
                let predecessor_location = NodeLocation {
                    machine: function.machine,
                    block: predecessor.id,
                    node: u32::try_from(predecessor_index)
                        .expect("optimization node index fits u32"),
                };
                let Some((affected_blocks, provenance)) = shared_terminal_fusion_accounting(
                    function,
                    predecessor_location,
                    *incoming_edge,
                    target.id,
                ) else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_shared_jump_fusion(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        substitutions,
                        provenance,
                        -1,
                        SharedJumpFusionRewrite {
                            predecessor: predecessor_location,
                            incoming_edge: *incoming_edge,
                            target: target.id,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}

impl AdjacentBlockMergeRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.adjacent-single-predecessor-block-merge.v5",
            ),
            OptimizationPassIdentity::from_canonical_bytes(CONTROL_FLOW_CLEANUP_PASS_NAME),
            5,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::UseDefinition,
                AnalysisKind::OwnershipFrontiers,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::StructuralIdentity,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for AdjacentBlockMergeRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        if analyses.get(AnalysisKind::ControlFlowGraph).is_none() {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ControlFlowGraph,
            ));
        }
        let Some(AnalysisProduct::Dominators(dominators)) = analyses.get(AnalysisKind::Dominators)
        else {
            return Err(RuleProposalError::MissingAnalysis(AnalysisKind::Dominators));
        };
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::OwnershipFrontiers(frontiers)) =
            analyses.get(AnalysisKind::OwnershipFrontiers)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::OwnershipFrontiers,
            ));
        };
        let mut candidates = Vec::new();
        for function in &unit.functions {
            let machine_dominators = dominators
                .functions
                .iter()
                .find(|(machine, _)| *machine == function.machine)
                .map(|(_, rows)| rows.as_slice())
                .unwrap_or_default();
            for adjacent in function.blocks.windows(2) {
                let [predecessor, target] = adjacent else {
                    unreachable!("two-block window")
                };
                let eligible_first = target.nodes.first().is_some_and(|node| {
                    (node.successors.is_empty()
                        && (matches!(node.provenance.first(), Some(PsiProvenance::Operation(_)))
                            || (matches!(node.provenance.first(), Some(PsiProvenance::Edge(_)))
                                && matches!(
                                    node.operation,
                                    O::Return { .. }
                                        | O::ReturnUnit { .. }
                                        | O::ReturnStructural { .. }
                                        | O::Crash { .. }
                                ))))
                        || (matches!(node.operation, O::Conditional { .. })
                            && node.successors.len() == 2
                            && node.provenance.is_empty())
                });
                if target.id == function.entry || !eligible_first {
                    continue;
                }
                let Some((predecessor_index, predecessor_node)) = predecessor
                    .nodes
                    .len()
                    .checked_sub(1)
                    .map(|index| (index, &predecessor.nodes[index]))
                else {
                    continue;
                };
                let O::Jump {
                    psi_edge: incoming_edge,
                    target: jump_target,
                    bindings,
                    trivial_affine_discards,
                } = &predecessor_node.operation
                else {
                    continue;
                };
                if !trivial_affine_discards.is_empty()
                    || *jump_target != target.id
                    || function
                        .blocks
                        .iter()
                        .flat_map(|block| &block.nodes)
                        .flat_map(|node| &node.successors)
                        .filter(|edge| edge.target == target.id)
                        .count()
                        != 1
                {
                    continue;
                }
                let Some(ownership_witness) = adjacent_merge_ownership_witness(
                    unit,
                    function,
                    frontiers,
                    *incoming_edge,
                    target.id,
                ) else {
                    continue;
                };
                let Some(mut substitutions) = target
                    .parameters
                    .iter()
                    .zip(bindings)
                    .map(|(parameter, binding)| {
                        (binding.parameter == parameter.value
                            && binding.scalar_type == parameter.scalar_type
                            && replacement_dominates_parameter_uses(
                                function.machine,
                                binding.argument,
                                parameter.value,
                                machine_dominators,
                                use_definitions,
                            ))
                        .then_some(ScalarSubstitution {
                            from: parameter.value,
                            to: binding.argument,
                            scalar_type: parameter.scalar_type,
                        })
                    })
                    .collect::<Option<Vec<_>>>()
                    .filter(|_| target.parameters.len() == bindings.len())
                else {
                    continue;
                };
                substitutions.sort();
                let predecessor_location = NodeLocation {
                    machine: function.machine,
                    block: predecessor.id,
                    node: u32::try_from(predecessor_index)
                        .expect("optimization node index fits u32"),
                };
                let Some((affected_blocks, provenance)) = adjacent_merge_accounting(
                    function,
                    predecessor_location,
                    target.id,
                    &substitutions,
                ) else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_adjacent_block_merge(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        substitutions,
                        provenance,
                        ownership_witness,
                        -2,
                        AdjacentBlockMergeRewrite {
                            predecessor: predecessor_location,
                            incoming_edge: *incoming_edge,
                            target: target.id,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}

impl NonAdjacentBlockMergeRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.non-adjacent-unique-predecessor-block-merge.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(CONTROL_FLOW_CLEANUP_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::UseDefinition,
                AnalysisKind::OwnershipFrontiers,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::StructuralIdentity,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for NonAdjacentBlockMergeRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        if analyses.get(AnalysisKind::ControlFlowGraph).is_none() {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ControlFlowGraph,
            ));
        }
        let Some(AnalysisProduct::Dominators(dominators)) = analyses.get(AnalysisKind::Dominators)
        else {
            return Err(RuleProposalError::MissingAnalysis(AnalysisKind::Dominators));
        };
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::OwnershipFrontiers(frontiers)) =
            analyses.get(AnalysisKind::OwnershipFrontiers)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::OwnershipFrontiers,
            ));
        };

        let mut candidates = Vec::new();
        for function in &unit.functions {
            let machine_dominators = dominators
                .functions
                .iter()
                .find(|(machine, _)| *machine == function.machine)
                .map(|(_, rows)| rows.as_slice())
                .unwrap_or_default();
            for (predecessor_position, predecessor) in function.blocks.iter().enumerate() {
                let Some((predecessor_index, predecessor_node)) = predecessor
                    .nodes
                    .len()
                    .checked_sub(1)
                    .map(|index| (index, &predecessor.nodes[index]))
                else {
                    continue;
                };
                let O::Jump {
                    psi_edge: incoming_edge,
                    target: target_id,
                    bindings,
                    trivial_affine_discards,
                } = &predecessor_node.operation
                else {
                    continue;
                };
                let Some((target_position, target)) = function
                    .blocks
                    .iter()
                    .enumerate()
                    .find(|(_, block)| block.id == *target_id)
                else {
                    continue;
                };
                if !trivial_affine_discards.is_empty()
                    || target.id == function.entry
                    || target_position == predecessor_position.saturating_add(1)
                    || !non_adjacent_merge_target_is_nonempty(target)
                    || !block_dominates(machine_dominators, predecessor.id, target.id)
                    || function
                        .blocks
                        .iter()
                        .flat_map(|block| &block.nodes)
                        .flat_map(|node| &node.successors)
                        .filter(|edge| edge.target == target.id)
                        .count()
                        != 1
                    || !adjacent_merge_ownership_is_identity(
                        unit,
                        function,
                        frontiers,
                        *incoming_edge,
                        target.id,
                    )
                {
                    continue;
                }
                let Some(mut substitutions) = target
                    .parameters
                    .iter()
                    .zip(bindings)
                    .map(|(parameter, binding)| {
                        (binding.parameter == parameter.value
                            && binding.scalar_type == parameter.scalar_type
                            && replacement_dominates_parameter_uses(
                                function.machine,
                                binding.argument,
                                parameter.value,
                                machine_dominators,
                                use_definitions,
                            ))
                        .then_some(ScalarSubstitution {
                            from: parameter.value,
                            to: binding.argument,
                            scalar_type: parameter.scalar_type,
                        })
                    })
                    .collect::<Option<Vec<_>>>()
                    .filter(|_| target.parameters.len() == bindings.len())
                else {
                    continue;
                };
                substitutions.sort();
                let predecessor_location = NodeLocation {
                    machine: function.machine,
                    block: predecessor.id,
                    node: u32::try_from(predecessor_index)
                        .expect("optimization node index fits u32"),
                };
                let Some((affected_blocks, provenance)) = non_adjacent_merge_accounting(
                    function,
                    predecessor_location,
                    target.id,
                    &substitutions,
                ) else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_non_adjacent_block_merge(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        substitutions,
                        provenance,
                        -2,
                        NonAdjacentBlockMergeRewrite {
                            predecessor: predecessor_location,
                            incoming_edge: *incoming_edge,
                            target: target.id,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}

fn non_adjacent_merge_target_is_nonempty(
    target: &omega_optimization_unit::OptimizationBlock,
) -> bool {
    !target.nodes.is_empty()
        && !matches!(target.nodes.as_slice(), [node] if matches!(node.operation, O::Jump { .. }))
}

impl PsiOptimizationRule for ConstantConditionalFoldRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        let Some(AnalysisProduct::ScalarConstants(constants)) =
            analyses.get(AnalysisKind::ScalarConstants)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ScalarConstants,
            ));
        };
        if analyses.get(AnalysisKind::ControlFlowGraph).is_none() {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ControlFlowGraph,
            ));
        }
        let mut candidates = Vec::new();
        for function in &unit.functions {
            for block in &function.blocks {
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let O::Conditional {
                        condition,
                        when_true,
                        when_false,
                    } = &node.operation
                    else {
                        continue;
                    };
                    let Some((constant, condition_fact)) =
                        boolean_constant(constants, function.machine, *condition)
                    else {
                        continue;
                    };
                    let (selected, rejected) = if constant {
                        (when_true, when_false)
                    } else {
                        (when_false, when_true)
                    };
                    let location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: u32::try_from(node_index).expect("optimization node indices are u32"),
                    };
                    let Some(reachable) =
                        reachable_blocks_after_fold(function, block.id, selected.psi_edge)
                    else {
                        continue;
                    };
                    let Some((affected_blocks, provenance)) = conditional_fold_accounting(
                        function,
                        location,
                        selected.psi_edge,
                        rejected.psi_edge,
                        &reachable,
                    ) else {
                        continue;
                    };
                    candidates.push(
                        PsiRewriteCandidate::new_constant_conditional(
                            unit.identity,
                            Self::contract(),
                            affected_blocks,
                            provenance,
                            condition_fact,
                            -1,
                            ConstantConditionalRewrite {
                                location,
                                condition: *condition,
                                constant,
                                selected_edge: selected.psi_edge,
                                rejected_edge: rejected.psi_edge,
                            },
                        )
                        .map_err(RuleProposalError::InvalidCandidate)?,
                    );
                }
            }
        }
        Ok(candidates)
    }
}

fn reachable_blocks_after_fold(
    function: &omega_optimization_unit::PsiOptimizationFunction,
    source: BlockId,
    selected_edge: psi_core::EdgeId,
) -> Option<BTreeSet<BlockId>> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block_id) = pending.pop() {
        if !reachable.insert(block_id) {
            continue;
        }
        let block = function.blocks.iter().find(|block| block.id == block_id)?;
        for edge in block.nodes.iter().flat_map(|node| &node.successors) {
            if block_id != source || edge.psi_edge == selected_edge {
                pending.push(edge.target);
            }
        }
    }
    Some(reachable)
}

fn conditional_fold_accounting(
    function: &omega_optimization_unit::PsiOptimizationFunction,
    decision: NodeLocation,
    selected_edge: psi_core::EdgeId,
    rejected_edge: psi_core::EdgeId,
    reachable: &BTreeSet<BlockId>,
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let decision_node = function
        .blocks
        .iter()
        .find(|block| block.id == decision.block)?
        .nodes
        .get(usize::try_from(decision.node).ok()?)?;
    let selected = decision_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == selected_edge)?;
    let rejected = decision_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == rejected_edge)?;
    let selected_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: selected_edge,
    };
    let rejected_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: rejected_edge,
    };
    let removed = function
        .blocks
        .iter()
        .map(|block| block.id)
        .filter(|block| !reachable.contains(block))
        .collect::<BTreeSet<_>>();
    let mut affected = BTreeSet::from([decision.block]);
    affected.extend(removed.iter().copied());
    let mut realized = vec![ProvenanceRewrite {
        input: selected_site,
        disposition: ProvenanceDisposition::RealizedAt(selected_site),
        sources: selected.provenance.clone(),
        fuel: selected.fuel.clone(),
    }];
    let mut unreachable = vec![ProvenanceRewrite {
        input: rejected_site,
        disposition: ProvenanceDisposition::ProvenUnreachableAt(rejected_site),
        sources: rejected.provenance.clone(),
        fuel: rejected.fuel.clone(),
    }];
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        if removed.contains(&block.id) {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).ok()?,
                };
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    unreachable.push(ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::ProvenUnreachableAt(site),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
                for edge in &node.successors {
                    let site = PsiRealizationSite::Edge {
                        machine: function.machine,
                        edge: edge.psi_edge,
                    };
                    unreachable.push(ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::ProvenUnreachableAt(site),
                        sources: edge.provenance.clone(),
                        fuel: edge.fuel.clone(),
                    });
                }
            }
            continue;
        }
        for (node_index, node) in block.nodes.iter().enumerate() {
            let location = NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            };
            let effect_changes = node.effect.input != expected_effect
                || node.effect.output != expected_effect.checked_add(1)?;
            if effect_changes && location != decision {
                affected.insert(block.id);
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    realized.push(ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::RealizedAt(site),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
            }
            expected_effect = expected_effect.checked_add(1)?;
        }
    }
    realized.extend(unreachable);
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}

fn compose_linear_thread_bindings(
    parameters: &[omega_optimization_unit::ValueDefinition],
    incoming: &[omega_abstract_operations::ValueBinding],
    outgoing: &[omega_abstract_operations::ValueBinding],
) -> Option<Vec<omega_abstract_operations::ValueBinding>> {
    if parameters.len() != incoming.len() {
        return None;
    }
    let replacements = parameters
        .iter()
        .zip(incoming)
        .map(|(parameter, binding)| {
            (binding.parameter == parameter.value && binding.scalar_type == parameter.scalar_type)
                .then_some((parameter.value, (binding.argument, binding.scalar_type)))
        })
        .collect::<Option<BTreeMap<_, _>>>()?;
    Some(
        outgoing
            .iter()
            .map(|binding| {
                replacements
                    .get(&binding.argument)
                    .map_or(*binding, |(argument, scalar_type)| {
                        omega_abstract_operations::ValueBinding {
                            parameter: binding.parameter,
                            argument: *argument,
                            scalar_type: *scalar_type,
                        }
                    })
            })
            .collect(),
    )
}

fn linear_thread_ownership_is_identity(
    unit: &PsiOptimizationUnit,
    function: &omega_optimization_unit::PsiOptimizationFunction,
    frontiers: &crate::OwnershipFrontierAnalysis,
    incoming: psi_core::EdgeId,
    empty: BlockId,
    outgoing: psi_core::EdgeId,
    target: BlockId,
) -> bool {
    let sites = [
        OwnershipFrontierSite::EdgeEntry(incoming),
        OwnershipFrontierSite::EdgeExit(incoming),
        OwnershipFrontierSite::BlockEntry(empty),
        OwnershipFrontierSite::EdgeEntry(outgoing),
        OwnershipFrontierSite::EdgeExit(outgoing),
        OwnershipFrontierSite::BlockEntry(target),
    ];
    let facts = sites.map(|site| frontiers.fact(function.machine, site));
    if facts.iter().all(Option::is_none) {
        return function.structural_parameters.is_empty()
            && function.entry_claim_declarations.is_empty()
            && function.declared_places.is_empty();
    }
    facts.iter().all(|fact| {
        fact.is_some_and(|fact| fact.revision == unit.identity && fact.machine == function.machine)
    }) && facts
        .windows(2)
        .all(|pair| pair[0].unwrap().snapshot == pair[1].unwrap().snapshot)
}

fn linear_thread_accounting(
    function: &omega_optimization_unit::PsiOptimizationFunction,
    predecessor: NodeLocation,
    empty: NodeLocation,
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let predecessor_node = function
        .blocks
        .iter()
        .find(|block| block.id == predecessor.block)?
        .nodes
        .get(usize::try_from(predecessor.node).ok()?)?;
    let empty_node = function
        .blocks
        .iter()
        .find(|block| block.id == empty.block)?
        .nodes
        .get(usize::try_from(empty.node).ok()?)?;
    let predecessor_edge = predecessor_node.successors.first()?;
    let empty_edge = empty_node.successors.first()?;
    let output_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: predecessor_edge.psi_edge,
    };
    let predecessor_site = output_site;
    let empty_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: empty_edge.psi_edge,
    };

    let mut affected = BTreeSet::from([predecessor.block, empty.block]);
    let mut realized = vec![
        ProvenanceRewrite {
            input: predecessor_site,
            disposition: ProvenanceDisposition::RealizedAt(output_site),
            sources: predecessor_edge.provenance.clone(),
            fuel: predecessor_edge.fuel.clone(),
        },
        ProvenanceRewrite {
            input: empty_site,
            disposition: ProvenanceDisposition::RealizedAt(output_site),
            sources: empty_edge.provenance.clone(),
            fuel: empty_edge.fuel.clone(),
        },
    ];
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        if block.id == empty.block {
            continue;
        }
        for (node_index, node) in block.nodes.iter().enumerate() {
            let location = NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            };
            let effect_changes = node.effect.input != expected_effect
                || node.effect.output != expected_effect.checked_add(1)?;
            if effect_changes && location != predecessor {
                affected.insert(block.id);
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    realized.push(ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::RealizedAt(site),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
            }
            expected_effect = expected_effect.checked_add(1)?;
        }
    }
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}

fn path_thread_accounting(
    function: &omega_optimization_unit::PsiOptimizationFunction,
    empty: NodeLocation,
    incoming_edges: &[psi_core::EdgeId],
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let empty_node = function
        .blocks
        .iter()
        .find(|block| block.id == empty.block)?
        .nodes
        .get(usize::try_from(empty.node).ok()?)?;
    let outgoing = empty_node.successors.first()?;
    let outgoing_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: outgoing.psi_edge,
    };
    let incoming_set = incoming_edges.iter().copied().collect::<BTreeSet<_>>();
    if incoming_set.len() != incoming_edges.len() || incoming_set.is_empty() {
        return None;
    }
    let mut affected = BTreeSet::from([empty.block]);
    let mut realized = Vec::new();
    for block in &function.blocks {
        for node in &block.nodes {
            for edge in &node.successors {
                if !incoming_set.contains(&edge.psi_edge) || edge.target != empty.block {
                    continue;
                }
                affected.insert(block.id);
                let site = PsiRealizationSite::Edge {
                    machine: function.machine,
                    edge: edge.psi_edge,
                };
                realized.push(ProvenanceRewrite {
                    input: site,
                    disposition: ProvenanceDisposition::RealizedAt(site),
                    sources: edge.provenance.clone(),
                    fuel: edge.fuel.clone(),
                });
                realized.push(ProvenanceRewrite {
                    input: outgoing_site,
                    disposition: ProvenanceDisposition::RealizedAt(site),
                    sources: outgoing.provenance.clone(),
                    fuel: outgoing.fuel.clone(),
                });
            }
        }
    }
    if realized.len() != incoming_edges.len().checked_mul(2)? {
        return None;
    }
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        if block.id == empty.block {
            continue;
        }
        for (node_index, node) in block.nodes.iter().enumerate() {
            let location = NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            };
            let effect_changes = node.effect.input != expected_effect
                || node.effect.output != expected_effect.checked_add(1)?;
            if effect_changes {
                affected.insert(block.id);
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    realized.push(ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::RealizedAt(site),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
            }
            expected_effect = expected_effect.checked_add(1)?;
        }
    }
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}

fn adjacent_merge_ownership_is_identity(
    unit: &PsiOptimizationUnit,
    function: &omega_optimization_unit::PsiOptimizationFunction,
    frontiers: &crate::OwnershipFrontierAnalysis,
    incoming: psi_core::EdgeId,
    target: BlockId,
) -> bool {
    adjacent_merge_ownership_witness(unit, function, frontiers, incoming, target).is_some()
}

fn adjacent_merge_ownership_witness(
    unit: &PsiOptimizationUnit,
    function: &omega_optimization_unit::PsiOptimizationFunction,
    frontiers: &crate::OwnershipFrontierAnalysis,
    incoming: psi_core::EdgeId,
    target: BlockId,
) -> Option<OwnershipFrontierWitness> {
    let sites = [
        OwnershipFrontierSite::EdgeEntry(incoming),
        OwnershipFrontierSite::EdgeExit(incoming),
        OwnershipFrontierSite::BlockEntry(target),
    ];
    let facts = sites.map(|site| frontiers.fact(function.machine, site));
    if facts.iter().all(Option::is_none) {
        return (function.structural_parameters.is_empty()
            && function.entry_claim_declarations.is_empty()
            && function.declared_places.is_empty())
        .then_some(OwnershipFrontierWitness { rows: Vec::new() });
    }
    if !facts.iter().all(|fact| {
        fact.is_some_and(|fact| fact.revision == unit.identity && fact.machine == function.machine)
    }) || !facts
        .windows(2)
        .all(|pair| pair[0].unwrap().snapshot == pair[1].unwrap().snapshot)
    {
        return None;
    }
    let mut rows = facts
        .into_iter()
        .map(|fact| {
            let fact = fact.expect("complete ownership frontier fact set");
            OwnershipFrontierWitnessRow {
                site: fact.site,
                fact: fact.identity,
            }
        })
        .collect::<Vec<_>>();
    rows.sort_unstable_by_key(|row| row.site);
    Some(OwnershipFrontierWitness { rows })
}

fn adjacent_merge_accounting(
    function: &omega_optimization_unit::PsiOptimizationFunction,
    predecessor: NodeLocation,
    target: BlockId,
    substitutions: &[ScalarSubstitution],
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let predecessor_position = function
        .blocks
        .iter()
        .position(|block| block.id == predecessor.block)?;
    let target_position = function
        .blocks
        .iter()
        .position(|block| block.id == target)?;
    if target_position != predecessor_position.checked_add(1)? {
        return None;
    }
    let predecessor_node = function.blocks[predecessor_position]
        .nodes
        .get(usize::try_from(predecessor.node).ok()?)?;
    let incoming = predecessor_node.successors.first()?;
    let target_block = &function.blocks[target_position];
    let incoming_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: incoming.psi_edge,
    };
    let mut affected = BTreeSet::from([predecessor.block, target]);
    let first = target_block.nodes.first()?;
    let mut realized = if !first.provenance.is_empty() {
        vec![ProvenanceRewrite {
            input: incoming_site,
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(
                NodeLocation {
                    machine: function.machine,
                    block: predecessor.block,
                    node: predecessor.node,
                },
            )),
            sources: incoming.provenance.clone(),
            fuel: incoming.fuel.clone(),
        }]
    } else if !first.successors.is_empty() {
        first
            .successors
            .iter()
            .map(|successor| ProvenanceRewrite {
                input: incoming_site,
                disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Edge {
                    machine: function.machine,
                    edge: successor.psi_edge,
                }),
                sources: incoming.provenance.clone(),
                fuel: incoming.fuel.clone(),
            })
            .collect()
    } else {
        return None;
    };
    for (node_index, node) in target_block.nodes.iter().enumerate() {
        if node.provenance.is_empty() {
            continue;
        }
        let input = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: target,
            node: u32::try_from(node_index).ok()?,
        });
        let output = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: predecessor.block,
            node: predecessor
                .node
                .checked_add(u32::try_from(node_index).ok()?)?,
        });
        realized.push(ProvenanceRewrite {
            input,
            disposition: ProvenanceDisposition::RealizedAt(output),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }
    for block in function.blocks.iter().skip(target_position + 1) {
        affected.insert(block.id);
        for (node_index, node) in block.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            });
            realized.push(ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    let substituted_values = substitutions
        .iter()
        .map(|row| row.from)
        .collect::<BTreeSet<_>>();
    for block in &function.blocks {
        if affected.contains(&block.id) {
            continue;
        }
        let changed_nodes = block
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| {
                node.uses
                    .iter()
                    .any(|row| substituted_values.contains(&row.value))
            })
            .collect::<Vec<_>>();
        if changed_nodes.is_empty() {
            continue;
        }
        affected.insert(block.id);
        for (node_index, node) in changed_nodes {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            });
            realized.push(ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}

fn non_adjacent_merge_accounting(
    function: &omega_optimization_unit::PsiOptimizationFunction,
    predecessor: NodeLocation,
    target: BlockId,
    substitutions: &[ScalarSubstitution],
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let predecessor_position = function
        .blocks
        .iter()
        .position(|block| block.id == predecessor.block)?;
    let target_position = function
        .blocks
        .iter()
        .position(|block| block.id == target)?;
    if target_position == predecessor_position.checked_add(1)? {
        return None;
    }
    let predecessor_block = &function.blocks[predecessor_position];
    let predecessor_node = predecessor_block
        .nodes
        .get(usize::try_from(predecessor.node).ok()?)?;
    let incoming = predecessor_node.successors.first()?;
    let target_block = &function.blocks[target_position];
    let first = target_block.nodes.first()?;
    let incoming_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: incoming.psi_edge,
    };
    let mut realized = if first.successors.is_empty() {
        vec![ProvenanceRewrite {
            input: incoming_site,
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(predecessor)),
            sources: incoming.provenance.clone(),
            fuel: incoming.fuel.clone(),
        }]
    } else {
        first
            .successors
            .iter()
            .map(|successor| ProvenanceRewrite {
                input: incoming_site,
                disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Edge {
                    machine: function.machine,
                    edge: successor.psi_edge,
                }),
                sources: incoming.provenance.clone(),
                fuel: incoming.fuel.clone(),
            })
            .collect()
    };

    for (node_index, node) in target_block.nodes.iter().enumerate() {
        if node.provenance.is_empty() {
            continue;
        }
        let input = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: target,
            node: u32::try_from(node_index).ok()?,
        });
        let output = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: predecessor.block,
            node: predecessor
                .node
                .checked_add(u32::try_from(node_index).ok()?)?,
        });
        realized.push(ProvenanceRewrite {
            input,
            disposition: ProvenanceDisposition::RealizedAt(output),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }

    let mut input_effect = 0u64;
    let mut input_starts = BTreeMap::new();
    for block in &function.blocks {
        input_starts.insert(block.id, input_effect);
        input_effect = input_effect.checked_add(u64::try_from(block.nodes.len()).ok()?)?;
    }
    let mut output_effect = 0u64;
    let mut effect_shifted = BTreeSet::new();
    for block in &function.blocks {
        if block.id == target {
            continue;
        }
        if input_starts.get(&block.id).copied()? != output_effect {
            effect_shifted.insert(block.id);
        }
        let output_nodes = if block.id == predecessor.block {
            block
                .nodes
                .len()
                .checked_sub(1)?
                .checked_add(target_block.nodes.len())?
        } else {
            block.nodes.len()
        };
        output_effect = output_effect.checked_add(u64::try_from(output_nodes).ok()?)?;
    }

    let substituted_values = substitutions
        .iter()
        .map(|row| row.from)
        .collect::<BTreeSet<_>>();
    let mut affected = BTreeSet::from([predecessor.block, target]);
    affected.extend(effect_shifted.iter().copied());
    for block in &function.blocks {
        if block.id == target {
            continue;
        }
        let mut changed_uses = BTreeSet::new();
        for (node_index, node) in block.nodes.iter().enumerate() {
            if node
                .uses
                .iter()
                .any(|row| substituted_values.contains(&row.value))
            {
                changed_uses.insert(node_index);
                affected.insert(block.id);
            }
        }
        for (node_index, node) in block.nodes.iter().enumerate() {
            if node.provenance.is_empty()
                || (!effect_shifted.contains(&block.id) && !changed_uses.contains(&node_index))
            {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            });
            realized.push(ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}

fn shared_terminal_fusion_accounting(
    function: &omega_optimization_unit::PsiOptimizationFunction,
    predecessor: NodeLocation,
    incoming_edge: psi_core::EdgeId,
    target: BlockId,
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let predecessor_block = function
        .blocks
        .iter()
        .find(|block| block.id == predecessor.block)?;
    let incoming = predecessor_block
        .nodes
        .get(usize::try_from(predecessor.node).ok()?)?
        .successors
        .iter()
        .find(|edge| edge.psi_edge == incoming_edge)?;
    let target_block = function.blocks.iter().find(|block| block.id == target)?;
    let [terminal] = target_block.nodes.as_slice() else {
        return None;
    };
    let input_edge = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: incoming_edge,
    };
    let input_terminal = PsiRealizationSite::Node(NodeLocation {
        machine: function.machine,
        block: target,
        node: 0,
    });
    let output_clone = PsiRealizationSite::Node(predecessor);
    let mut provenance = vec![
        ProvenanceRewrite {
            input: input_edge,
            disposition: ProvenanceDisposition::RealizedAt(output_clone),
            sources: incoming.provenance.clone(),
            fuel: incoming.fuel.clone(),
        },
        ProvenanceRewrite {
            input: input_terminal,
            disposition: ProvenanceDisposition::RealizedAt(output_clone),
            sources: terminal.provenance.clone(),
            fuel: terminal.fuel.clone(),
        },
        ProvenanceRewrite {
            input: input_terminal,
            disposition: ProvenanceDisposition::RealizedAt(input_terminal),
            sources: terminal.provenance.clone(),
            fuel: terminal.fuel.clone(),
        },
    ];
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    let mut affected = vec![predecessor.block, target];
    affected.sort();
    affected.dedup();
    Some((affected, provenance))
}

fn dead_scalar_node_accounting(
    function: &omega_optimization_unit::PsiOptimizationFunction,
    location: NodeLocation,
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let block_position = function
        .blocks
        .iter()
        .position(|block| block.id == location.block)?;
    let node_position = usize::try_from(location.node).ok()?;
    let block = &function.blocks[block_position];
    let removed = block.nodes.get(node_position)?;
    block.nodes.get(node_position.checked_add(1)?)?;
    let output_receiver = PsiRealizationSite::Node(location);
    let mut provenance = vec![ProvenanceRewrite {
        input: PsiRealizationSite::Node(location),
        disposition: ProvenanceDisposition::RealizedAt(output_receiver),
        sources: removed.provenance.clone(),
        fuel: removed.fuel.clone(),
    }];
    for (index, node) in block.nodes.iter().enumerate().skip(node_position + 1) {
        if node.provenance.is_empty() {
            continue;
        }
        let old = NodeLocation {
            machine: function.machine,
            block: block.id,
            node: u32::try_from(index).ok()?,
        };
        let new = NodeLocation {
            node: old.node.checked_sub(1)?,
            ..old
        };
        provenance.push(ProvenanceRewrite {
            input: PsiRealizationSite::Node(old),
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(new)),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }
    let mut affected = vec![block.id];
    for later in function.blocks.iter().skip(block_position + 1) {
        affected.push(later.id);
        for (index, node) in later.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: later.id,
                node: u32::try_from(index).ok()?,
            });
            provenance.push(ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    affected.sort();
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected, provenance))
}

fn local_cse_accounting(
    function: &omega_optimization_unit::PsiOptimizationFunction,
    redundant: NodeLocation,
    redundant_result: ValueId,
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let block_position = function
        .blocks
        .iter()
        .position(|block| block.id == redundant.block)?;
    let node_position = usize::try_from(redundant.node).ok()?;
    let block = &function.blocks[block_position];
    let removed = block.nodes.get(node_position)?;
    block.nodes.get(node_position.checked_add(1)?)?;
    let mut provenance = vec![ProvenanceRewrite {
        input: PsiRealizationSite::Node(redundant),
        disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(redundant)),
        sources: removed.provenance.clone(),
        fuel: removed.fuel.clone(),
    }];
    for (index, node) in block.nodes.iter().enumerate().skip(node_position + 1) {
        if node.provenance.is_empty() {
            continue;
        }
        let old = NodeLocation {
            machine: function.machine,
            block: block.id,
            node: u32::try_from(index).ok()?,
        };
        let new = NodeLocation {
            node: old.node.checked_sub(1)?,
            ..old
        };
        provenance.push(ProvenanceRewrite {
            input: PsiRealizationSite::Node(old),
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(new)),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }
    let mut affected = vec![block.id];
    for later in function.blocks.iter().skip(block_position + 1) {
        affected.push(later.id);
        for (index, node) in later.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: later.id,
                node: u32::try_from(index).ok()?,
            });
            provenance.push(ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    for use_block in &function.blocks {
        if affected.contains(&use_block.id)
            || !use_block
                .nodes
                .iter()
                .flat_map(|node| &node.uses)
                .any(|row| row.value == redundant_result)
        {
            continue;
        }
        affected.push(use_block.id);
        for (index, node) in use_block.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: use_block.id,
                node: u32::try_from(index).ok()?,
            });
            provenance.push(ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    affected.sort();
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected, provenance))
}

fn phi_translated_cse_accounting(
    function: &omega_optimization_unit::PsiOptimizationFunction,
    redundant: NodeLocation,
    incoming: &[PhiTranslatedScalarIncoming],
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let block_position = function
        .blocks
        .iter()
        .position(|block| block.id == redundant.block)?;
    let node_position = usize::try_from(redundant.node).ok()?;
    let block = &function.blocks[block_position];
    let removed = block.nodes.get(node_position)?;
    block.nodes.get(node_position.checked_add(1)?)?;
    let mut affected = incoming
        .iter()
        .map(|row| row.source)
        .chain([block.id])
        .collect::<BTreeSet<_>>();
    let mut provenance = vec![ProvenanceRewrite {
        input: PsiRealizationSite::Node(redundant),
        disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(redundant)),
        sources: removed.provenance.clone(),
        fuel: removed.fuel.clone(),
    }];
    for row in incoming {
        let source = function
            .blocks
            .iter()
            .find(|block| block.id == row.source)?;
        let edge = source
            .nodes
            .iter()
            .flat_map(|node| &node.successors)
            .find(|edge| edge.psi_edge == row.edge && edge.target == redundant.block)?;
        if !edge.provenance.is_empty() {
            let site = PsiRealizationSite::Edge {
                machine: function.machine,
                edge: edge.psi_edge,
            };
            provenance.push(ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: edge.provenance.clone(),
                fuel: edge.fuel.clone(),
            });
        }
    }
    for (index, node) in block.nodes.iter().enumerate().skip(node_position + 1) {
        if node.provenance.is_empty() {
            continue;
        }
        let old = NodeLocation {
            machine: function.machine,
            block: block.id,
            node: u32::try_from(index).ok()?,
        };
        let new = NodeLocation {
            node: old.node.checked_sub(1)?,
            ..old
        };
        provenance.push(ProvenanceRewrite {
            input: PsiRealizationSite::Node(old),
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(new)),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }
    for later in function.blocks.iter().skip(block_position + 1) {
        affected.insert(later.id);
        for (index, node) in later.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: later.id,
                node: u32::try_from(index).ok()?,
            });
            provenance.push(ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), provenance))
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RedundantBlockParameterRule;

impl RedundantBlockParameterRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.redundant-block-parameter.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(COPY_PROPAGATION_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::UseDefinition,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::StructuralIdentity,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for RedundantBlockParameterRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_redundant_block_parameters(unit, analyses, Self::contract())
    }
}

fn integer_evaluation_contract(
    rule_name: &[u8],
    safety_class: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(rule_name),
        OptimizationPassIdentity::from_canonical_bytes(SCCP_PASS_NAME),
        1,
        AnalysisSet::new([AnalysisKind::ScalarConstants]),
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition]),
        safety_class,
    )
    .expect("built-in rule has nonzero version")
}

macro_rules! integer_evaluation_rule {
    ($name:ident, $rule_name:literal, $kind:expr, $safety:expr) => {
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        impl $name {
            pub fn contract() -> OptimizationRuleContract {
                integer_evaluation_contract($rule_name, $safety)
            }
        }

        impl PsiOptimizationRule for $name {
            fn contract(&self) -> OptimizationRuleContract {
                Self::contract()
            }

            fn propose(
                &self,
                unit: &PsiOptimizationUnit,
                analyses: RuleAnalysisView<'_>,
            ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
                propose_integer_binary_constants(unit, analyses, Self::contract(), $kind)
            }
        }
    };
}

integer_evaluation_rule!(
    ExactIntegerAddConstantsRule,
    b"omega.psi-rule.exact-integer-add-constants.v1",
    IntegerBinaryKind::ExactAdd,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    ExactIntegerSubtractConstantsRule,
    b"omega.psi-rule.exact-integer-subtract-constants.v1",
    IntegerBinaryKind::ExactSubtract,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    ExactIntegerMultiplyConstantsRule,
    b"omega.psi-rule.exact-integer-multiply-constants.v1",
    IntegerBinaryKind::ExactMultiply,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    WrappingIntegerAddConstantsRule,
    b"omega.psi-rule.wrapping-integer-add-constants.v1",
    IntegerBinaryKind::WrappingAdd,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_evaluation_rule!(
    WrappingIntegerSubtractConstantsRule,
    b"omega.psi-rule.wrapping-integer-subtract-constants.v1",
    IntegerBinaryKind::WrappingSubtract,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_evaluation_rule!(
    WrappingIntegerMultiplyConstantsRule,
    b"omega.psi-rule.wrapping-integer-multiply-constants.v1",
    IntegerBinaryKind::WrappingMultiply,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_evaluation_rule!(
    SaturatingIntegerAddConstantsRule,
    b"omega.psi-rule.saturating-integer-add-constants.v1",
    IntegerBinaryKind::SaturatingAdd,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_evaluation_rule!(
    SaturatingIntegerSubtractConstantsRule,
    b"omega.psi-rule.saturating-integer-subtract-constants.v1",
    IntegerBinaryKind::SaturatingSubtract,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_evaluation_rule!(
    SaturatingIntegerMultiplyConstantsRule,
    b"omega.psi-rule.saturating-integer-multiply-constants.v1",
    IntegerBinaryKind::SaturatingMultiply,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_evaluation_rule!(
    ExactIntegerDivideConstantsRule,
    b"omega.psi-rule.exact-integer-divide-constants.v1",
    IntegerBinaryKind::ExactDivide,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    ExactIntegerRemainderConstantsRule,
    b"omega.psi-rule.exact-integer-remainder-constants.v1",
    IntegerBinaryKind::ExactRemainder,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    WrappingIntegerDivideConstantsRule,
    b"omega.psi-rule.wrapping-integer-divide-constants.v1",
    IntegerBinaryKind::WrappingDivide,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    WrappingIntegerRemainderConstantsRule,
    b"omega.psi-rule.wrapping-integer-remainder-constants.v1",
    IntegerBinaryKind::WrappingRemainder,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    SaturatingIntegerDivideConstantsRule,
    b"omega.psi-rule.saturating-integer-divide-constants.v1",
    IntegerBinaryKind::SaturatingDivide,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    SaturatingIntegerRemainderConstantsRule,
    b"omega.psi-rule.saturating-integer-remainder-constants.v1",
    IntegerBinaryKind::SaturatingRemainder,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    ExactIntegerShiftLeftConstantsRule,
    b"omega.psi-rule.exact-integer-shift-left-constants.v1",
    IntegerBinaryKind::ExactShiftLeft,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    ExactIntegerShiftRightConstantsRule,
    b"omega.psi-rule.exact-integer-shift-right-constants.v1",
    IntegerBinaryKind::ExactShiftRight,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    WrappingIntegerShiftLeftConstantsRule,
    b"omega.psi-rule.wrapping-integer-shift-left-constants.v1",
    IntegerBinaryKind::WrappingShiftLeft,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_evaluation_rule!(
    WrappingIntegerShiftRightConstantsRule,
    b"omega.psi-rule.wrapping-integer-shift-right-constants.v1",
    IntegerBinaryKind::WrappingShiftRight,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_evaluation_rule!(
    IntegerBitwiseAndConstantsRule,
    b"omega.psi-rule.integer-bitwise-and-constants.v1",
    IntegerBinaryKind::BitwiseAnd,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_evaluation_rule!(
    IntegerBitwiseOrConstantsRule,
    b"omega.psi-rule.integer-bitwise-or-constants.v1",
    IntegerBinaryKind::BitwiseOr,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_evaluation_rule!(
    IntegerBitwiseXorConstantsRule,
    b"omega.psi-rule.integer-bitwise-xor-constants.v1",
    IntegerBinaryKind::BitwiseXor,
    OptimizationSafetyClass::ExactOperationSemantics
);

macro_rules! boolean_evaluation_rule {
    ($name:ident, $rule_name:literal, $kind:expr) => {
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        impl $name {
            pub fn contract() -> OptimizationRuleContract {
                integer_evaluation_contract(
                    $rule_name,
                    OptimizationSafetyClass::ExactOperationSemantics,
                )
            }
        }

        impl PsiOptimizationRule for $name {
            fn contract(&self) -> OptimizationRuleContract {
                Self::contract()
            }

            fn propose(
                &self,
                unit: &PsiOptimizationUnit,
                analyses: RuleAnalysisView<'_>,
            ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
                propose_boolean_constants(unit, analyses, Self::contract(), $kind)
            }
        }
    };
}

boolean_evaluation_rule!(
    BooleanNotConstantsRule,
    b"omega.psi-rule.boolean-not-constants.v1",
    BooleanEvaluationKind::Not
);
boolean_evaluation_rule!(
    BooleanEqualConstantsRule,
    b"omega.psi-rule.boolean-equal-constants.v1",
    BooleanEvaluationKind::Equal
);
boolean_evaluation_rule!(
    IntegerEqualConstantsRule,
    b"omega.psi-rule.integer-equal-constants.v1",
    BooleanEvaluationKind::IntegerEqual
);
boolean_evaluation_rule!(
    IntegerLessThanConstantsRule,
    b"omega.psi-rule.integer-less-than-constants.v1",
    BooleanEvaluationKind::IntegerLessThan
);
boolean_evaluation_rule!(
    IntegerLessOrEqualConstantsRule,
    b"omega.psi-rule.integer-less-or-equal-constants.v1",
    BooleanEvaluationKind::IntegerLessOrEqual
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntegerRangeComparisonKind {
    RangeEqualConstant,
    ConstantEqualRange,
    RangeLessThanConstant,
    ConstantLessThanRange,
    RangeLessOrEqualConstant,
    ConstantLessOrEqualRange,
}

macro_rules! integer_range_comparison_rule {
    ($name:ident, $identity:literal, $kind:expr) => {
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        impl $name {
            pub fn contract() -> OptimizationRuleContract {
                OptimizationRuleContract::new(
                    OptimizationRuleIdentity::from_canonical_bytes($identity),
                    OptimizationPassIdentity::from_canonical_bytes(SCCP_PASS_NAME),
                    1,
                    AnalysisSet::new([AnalysisKind::ScalarConstants, AnalysisKind::ValueRanges]),
                    AnalysisInvalidationSet::new([AnalysisKind::UseDefinition]),
                    OptimizationSafetyClass::ProofCertified,
                )
                .expect("built-in rule has nonzero version")
            }
        }

        impl PsiOptimizationRule for $name {
            fn contract(&self) -> OptimizationRuleContract {
                Self::contract()
            }

            fn propose(
                &self,
                unit: &PsiOptimizationUnit,
                analyses: RuleAnalysisView<'_>,
            ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
                let Some(AnalysisProduct::ScalarConstants(constants)) =
                    analyses.get(AnalysisKind::ScalarConstants)
                else {
                    return Err(RuleProposalError::MissingAnalysis(
                        AnalysisKind::ScalarConstants,
                    ));
                };
                let Some(AnalysisProduct::ValueRanges(ranges)) =
                    analyses.get(AnalysisKind::ValueRanges)
                else {
                    return Err(RuleProposalError::MissingAnalysis(
                        AnalysisKind::ValueRanges,
                    ));
                };
                propose_integer_range_comparison(unit, constants, ranges, Self::contract(), $kind)
            }
        }
    };
}

integer_range_comparison_rule!(
    IntegerEqualRangeConstantRule,
    b"omega.psi-rule.integer-equal-range-constant.v1",
    IntegerRangeComparisonKind::RangeEqualConstant
);
integer_range_comparison_rule!(
    IntegerEqualConstantRangeRule,
    b"omega.psi-rule.integer-equal-constant-range.v1",
    IntegerRangeComparisonKind::ConstantEqualRange
);
integer_range_comparison_rule!(
    IntegerLessThanRangeConstantRule,
    b"omega.psi-rule.integer-less-than-range-constant.v1",
    IntegerRangeComparisonKind::RangeLessThanConstant
);
integer_range_comparison_rule!(
    IntegerLessThanConstantRangeRule,
    b"omega.psi-rule.integer-less-than-constant-range.v1",
    IntegerRangeComparisonKind::ConstantLessThanRange
);
integer_range_comparison_rule!(
    IntegerLessOrEqualRangeConstantRule,
    b"omega.psi-rule.integer-less-or-equal-range-constant.v1",
    IntegerRangeComparisonKind::RangeLessOrEqualConstant
);
integer_range_comparison_rule!(
    IntegerLessOrEqualConstantRangeRule,
    b"omega.psi-rule.integer-less-or-equal-constant-range.v1",
    IntegerRangeComparisonKind::ConstantLessOrEqualRange
);

fn propose_integer_range_comparison(
    unit: &PsiOptimizationUnit,
    constants: &ScalarConstantAnalysis,
    ranges: &ValueRangeAnalysis,
    contract: OptimizationRuleContract,
    kind: IntegerRangeComparisonKind,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let (psi_operation, result, left, right) = match (kind, &node.operation) {
                    (
                        IntegerRangeComparisonKind::RangeEqualConstant
                        | IntegerRangeComparisonKind::ConstantEqualRange,
                        O::IntegerEqual {
                            psi_operation,
                            result,
                            left,
                            right,
                        },
                    )
                    | (
                        IntegerRangeComparisonKind::RangeLessThanConstant
                        | IntegerRangeComparisonKind::ConstantLessThanRange,
                        O::IntegerLessThan {
                            psi_operation,
                            result,
                            left,
                            right,
                        },
                    )
                    | (
                        IntegerRangeComparisonKind::RangeLessOrEqualConstant
                        | IntegerRangeComparisonKind::ConstantLessOrEqualRange,
                        O::IntegerLessOrEqual {
                            psi_operation,
                            result,
                            left,
                            right,
                        },
                    ) => (*psi_operation, *result, *left, *right),
                    _ => continue,
                };
                let (range_value, constant_operand) = match kind {
                    IntegerRangeComparisonKind::RangeEqualConstant
                    | IntegerRangeComparisonKind::RangeLessThanConstant
                    | IntegerRangeComparisonKind::RangeLessOrEqualConstant => (left, right),
                    IntegerRangeComparisonKind::ConstantEqualRange
                    | IntegerRangeComparisonKind::ConstantLessThanRange
                    | IntegerRangeComparisonKind::ConstantLessOrEqualRange => (right, left),
                };
                let Some((constant_value, constant_fact)) =
                    literal_integer_constant(constants, function.machine, constant_operand)
                else {
                    continue;
                };
                let Some(scalar_type) = integer_value_type(function, range_value) else {
                    continue;
                };
                if integer_value_type(function, constant_operand) != Some(scalar_type) {
                    continue;
                }
                let node_index =
                    u32::try_from(node_index).expect("optimization node indices are u32");
                let Some(range) = ranges.facts.iter().find(|fact| {
                    fact.valid_in.machine == function.machine
                        && fact.value == range_value
                        && fact.scalar_type == scalar_type
                        && matches!(
                            fact.support,
                            ValueRangeSupport::AcceptedOperationProof { .. }
                        )
                        && ranges.fact_applies_at(
                            fact,
                            unit,
                            function.machine,
                            block.id,
                            node_index,
                        )
                }) else {
                    continue;
                };
                let Some(constant) = evaluate_integer_range_comparison(
                    kind,
                    scalar_type,
                    range.minimum,
                    range.maximum,
                    constant_value,
                ) else {
                    continue;
                };
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                };
                candidates.push(
                    PsiRewriteCandidate::new_boolean_evaluation(
                        unit.identity,
                        contract,
                        vec![block.id],
                        Vec::new(),
                        vec![ProvenanceRewrite {
                            input: PsiRealizationSite::Node(location),
                            disposition: ProvenanceDisposition::RealizedAt(
                                PsiRealizationSite::Node(location),
                            ),
                            sources: node.provenance.clone(),
                            fuel: node.fuel.clone(),
                        }],
                        IntegerEvaluationWitness::RangeAgainstConstant {
                            range_fact: range.identity,
                            constant_fact,
                        },
                        -1,
                        BooleanConstantRewrite {
                            location,
                            source_operation: psi_operation,
                            result,
                            constant,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}

fn evaluate_integer_range_comparison(
    kind: IntegerRangeComparisonKind,
    scalar_type: IntegerType,
    minimum: IntegerValue,
    maximum: IntegerValue,
    constant: IntegerValue,
) -> Option<bool> {
    let minimum_to_constant = scalar_type.compare(minimum, constant)?;
    let maximum_to_constant = scalar_type.compare(maximum, constant)?;
    match kind {
        IntegerRangeComparisonKind::RangeEqualConstant
        | IntegerRangeComparisonKind::ConstantEqualRange => (minimum_to_constant.is_eq()
            && maximum_to_constant.is_eq())
        .then_some(true)
        .or_else(|| (minimum_to_constant.is_gt() || maximum_to_constant.is_lt()).then_some(false)),
        IntegerRangeComparisonKind::RangeLessThanConstant => maximum_to_constant
            .is_lt()
            .then_some(true)
            .or_else(|| (!minimum_to_constant.is_lt()).then_some(false)),
        IntegerRangeComparisonKind::ConstantLessThanRange => minimum_to_constant
            .is_gt()
            .then_some(true)
            .or_else(|| (!maximum_to_constant.is_gt()).then_some(false)),
        IntegerRangeComparisonKind::RangeLessOrEqualConstant => (!maximum_to_constant.is_gt())
            .then_some(true)
            .or_else(|| minimum_to_constant.is_gt().then_some(false)),
        IntegerRangeComparisonKind::ConstantLessOrEqualRange => (!minimum_to_constant.is_lt())
            .then_some(true)
            .or_else(|| maximum_to_constant.is_lt().then_some(false)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BooleanEvaluationKind {
    Not,
    Equal,
    IntegerEqual,
    IntegerLessThan,
    IntegerLessOrEqual,
}

fn propose_boolean_constants(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    kind: BooleanEvaluationKind,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let Some(AnalysisProduct::ScalarConstants(constants)) =
        analyses.get(AnalysisKind::ScalarConstants)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ScalarConstants,
        ));
    };
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let (source_operation, result, constant, witness) = match (&node.operation, kind) {
                    (
                        O::BooleanNot {
                            psi_operation,
                            result,
                            operand,
                        },
                        BooleanEvaluationKind::Not,
                    ) => {
                        let Some((operand, operand_fact)) =
                            boolean_constant(constants, function.machine, *operand)
                        else {
                            continue;
                        };
                        (
                            *psi_operation,
                            *result,
                            !operand,
                            IntegerEvaluationWitness::Unary { operand_fact },
                        )
                    }
                    (
                        O::BooleanEqual {
                            psi_operation,
                            result,
                            left,
                            right,
                        },
                        BooleanEvaluationKind::Equal,
                    ) => {
                        let Some((left, left_fact)) =
                            boolean_constant(constants, function.machine, *left)
                        else {
                            continue;
                        };
                        let Some((right, right_fact)) =
                            boolean_constant(constants, function.machine, *right)
                        else {
                            continue;
                        };
                        (
                            *psi_operation,
                            *result,
                            left == right,
                            IntegerEvaluationWitness::Binary {
                                left_fact,
                                right_fact,
                            },
                        )
                    }
                    (
                        O::IntegerEqual {
                            psi_operation,
                            result,
                            left,
                            right,
                        },
                        BooleanEvaluationKind::IntegerEqual,
                    )
                    | (
                        O::IntegerLessThan {
                            psi_operation,
                            result,
                            left,
                            right,
                        },
                        BooleanEvaluationKind::IntegerLessThan,
                    )
                    | (
                        O::IntegerLessOrEqual {
                            psi_operation,
                            result,
                            left,
                            right,
                        },
                        BooleanEvaluationKind::IntegerLessOrEqual,
                    ) => {
                        let Some((left_value, left_fact)) =
                            integer_constant(constants, function.machine, *left)
                        else {
                            continue;
                        };
                        let Some((right_value, right_fact)) =
                            integer_constant(constants, function.machine, *right)
                        else {
                            continue;
                        };
                        let Some(left_type) = integer_value_type(function, *left) else {
                            continue;
                        };
                        if integer_value_type(function, *right) != Some(left_type) {
                            continue;
                        }
                        let Some(ordering) = left_type.compare(left_value, right_value) else {
                            continue;
                        };
                        let constant = match kind {
                            BooleanEvaluationKind::IntegerEqual => ordering.is_eq(),
                            BooleanEvaluationKind::IntegerLessThan => ordering.is_lt(),
                            BooleanEvaluationKind::IntegerLessOrEqual => !ordering.is_gt(),
                            _ => unreachable!(),
                        };
                        (
                            *psi_operation,
                            *result,
                            constant,
                            IntegerEvaluationWitness::Binary {
                                left_fact,
                                right_fact,
                            },
                        )
                    }
                    _ => continue,
                };
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).expect("optimization node indices are u32"),
                };
                candidates.push(
                    PsiRewriteCandidate::new_boolean_evaluation(
                        unit.identity,
                        contract,
                        vec![block.id],
                        Vec::new(),
                        vec![ProvenanceRewrite {
                            input: PsiRealizationSite::Node(location),
                            disposition: ProvenanceDisposition::RealizedAt(
                                PsiRealizationSite::Node(location),
                            ),
                            sources: node.provenance.clone(),
                            fuel: node.fuel.clone(),
                        }],
                        witness,
                        -1,
                        BooleanConstantRewrite {
                            location,
                            source_operation,
                            result,
                            constant,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ExactIntegerCastConstantsRule;

impl ExactIntegerCastConstantsRule {
    pub fn contract() -> OptimizationRuleContract {
        integer_evaluation_contract(
            b"omega.psi-rule.exact-integer-cast-constants.v1",
            OptimizationSafetyClass::ProofCertified,
        )
    }
}

impl PsiOptimizationRule for ExactIntegerCastConstantsRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_exact_integer_cast_constants(unit, analyses, Self::contract())
    }
}

macro_rules! integer_unary_rule {
    ($name:ident, $rule_name:literal, $kind:expr) => {
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        impl $name {
            pub fn contract() -> OptimizationRuleContract {
                integer_evaluation_contract(
                    $rule_name,
                    OptimizationSafetyClass::ExactOperationSemantics,
                )
            }
        }

        impl PsiOptimizationRule for $name {
            fn contract(&self) -> OptimizationRuleContract {
                Self::contract()
            }

            fn propose(
                &self,
                unit: &PsiOptimizationUnit,
                analyses: RuleAnalysisView<'_>,
            ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
                propose_integer_unary_constants(unit, analyses, Self::contract(), $kind)
            }
        }
    };
}

integer_unary_rule!(
    IntegerWidenConstantsRule,
    b"omega.psi-rule.integer-widen-constants.v1",
    IntegerUnaryKind::Widen
);
integer_unary_rule!(
    IntegerBitwiseNotConstantsRule,
    b"omega.psi-rule.integer-bitwise-not-constants.v1",
    IntegerUnaryKind::BitwiseNot
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntegerUnaryKind {
    Widen,
    BitwiseNot,
}

fn propose_integer_unary_constants(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    kind: IntegerUnaryKind,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let Some(AnalysisProduct::ScalarConstants(constants)) =
        analyses.get(AnalysisKind::ScalarConstants)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ScalarConstants,
        ));
    };
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let (source_operation, result, source_type, target_type, operand) =
                    match (&node.operation, kind) {
                        (
                            O::IntegerWiden {
                                psi_operation,
                                result,
                                source_type,
                                target_type,
                                operand,
                            },
                            IntegerUnaryKind::Widen,
                        ) => (
                            *psi_operation,
                            *result,
                            *source_type,
                            *target_type,
                            *operand,
                        ),
                        (
                            O::IntegerBitwiseNot {
                                psi_operation,
                                result,
                                scalar_type,
                                operand,
                            },
                            IntegerUnaryKind::BitwiseNot,
                        ) => (
                            *psi_operation,
                            *result,
                            *scalar_type,
                            *scalar_type,
                            *operand,
                        ),
                        _ => continue,
                    };
                let Some((operand_value, operand_fact)) =
                    integer_constant(constants, function.machine, operand)
                else {
                    #[cfg(test)]
                    eprintln!("linear thread refused accounting");
                    continue;
                };
                let constant = match kind {
                    IntegerUnaryKind::Widen => {
                        source_type.widen_value_to(target_type, operand_value)
                    }
                    IntegerUnaryKind::BitwiseNot => source_type.bitwise_not(operand_value),
                };
                let Some(constant) = constant else {
                    continue;
                };
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).expect("optimization node indices are u32"),
                };
                candidates.push(
                    PsiRewriteCandidate::new_integer_evaluation(
                        unit.identity,
                        contract,
                        vec![block.id],
                        Vec::new(),
                        vec![ProvenanceRewrite {
                            input: PsiRealizationSite::Node(location),
                            disposition: ProvenanceDisposition::RealizedAt(
                                PsiRealizationSite::Node(location),
                            ),
                            sources: node.provenance.clone(),
                            fuel: node.fuel.clone(),
                        }],
                        IntegerEvaluationWitness::Unary { operand_fact },
                        -1,
                        IntegerConstantRewrite {
                            location,
                            source_operation,
                            result,
                            scalar_type: target_type,
                            constant,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}

fn propose_exact_integer_cast_constants(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let Some(AnalysisProduct::ScalarConstants(constants)) =
        analyses.get(AnalysisKind::ScalarConstants)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ScalarConstants,
        ));
    };
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let O::IntegerExactCast {
                    psi_operation,
                    result,
                    source_type,
                    target_type,
                    operand,
                    ..
                } = node.operation
                else {
                    continue;
                };
                let Some((operand_value, operand_fact)) =
                    integer_constant(constants, function.machine, operand)
                else {
                    continue;
                };
                let Some(constant) = source_type.exact_cast_value_to(target_type, operand_value)
                else {
                    continue;
                };
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).expect("optimization node indices are u32"),
                };
                candidates.push(
                    PsiRewriteCandidate::new_integer_evaluation(
                        unit.identity,
                        contract,
                        vec![block.id],
                        Vec::new(),
                        vec![ProvenanceRewrite {
                            input: PsiRealizationSite::Node(location),
                            disposition: ProvenanceDisposition::RealizedAt(
                                PsiRealizationSite::Node(location),
                            ),
                            sources: node.provenance.clone(),
                            fuel: node.fuel.clone(),
                        }],
                        proof_certified_unary_witness(
                            unit,
                            function.machine,
                            psi_operation,
                            operand_fact,
                        )?,
                        -1,
                        IntegerConstantRewrite {
                            location,
                            source_operation: psi_operation,
                            result,
                            scalar_type: target_type,
                            constant,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}

fn propose_integer_binary_constants(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    kind: IntegerBinaryKind,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let Some(AnalysisProduct::ScalarConstants(constants)) =
        analyses.get(AnalysisKind::ScalarConstants)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ScalarConstants,
        ));
    };
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let Some(shape) = integer_binary_shape(&node.operation) else {
                    continue;
                };
                if shape.kind != kind {
                    continue;
                }
                let Some((left_value, left_fact)) =
                    integer_constant(constants, function.machine, shape.left)
                else {
                    continue;
                };
                let Some((right_value, right_fact)) =
                    integer_constant(constants, function.machine, shape.right)
                else {
                    continue;
                };
                let Some(constant) = shape.evaluate(left_value, right_value) else {
                    continue;
                };
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).expect("optimization node indices are u32"),
                };
                candidates.push(
                    PsiRewriteCandidate::new_integer_evaluation(
                        unit.identity,
                        contract,
                        vec![block.id],
                        Vec::new(),
                        vec![ProvenanceRewrite {
                            input: PsiRealizationSite::Node(location),
                            disposition: ProvenanceDisposition::RealizedAt(
                                PsiRealizationSite::Node(location),
                            ),
                            sources: node.provenance.clone(),
                            fuel: node.fuel.clone(),
                        }],
                        integer_binary_witness(
                            unit,
                            function.machine,
                            shape.source,
                            contract.safety_class(),
                            left_fact,
                            right_fact,
                        )?,
                        -1,
                        IntegerConstantRewrite {
                            location,
                            source_operation: shape.source,
                            result: shape.result,
                            scalar_type: shape.scalar_type,
                            constant,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}

fn accepted_obligation_fact(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    operation: OperationId,
) -> Result<omega_optimization_core::AcceptedObligationFactIdentity, RuleProposalError> {
    let obligation = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .and_then(|function| {
            function.facts.iter().find_map(|fact| match fact {
                OptimizationFact::OperationObligationReference {
                    obligation,
                    support,
                } if *support == operation => Some(*obligation),
                _ => None,
            })
        });
    let Some(obligation) = obligation else {
        return Err(RuleProposalError::MissingAcceptedObligation {
            machine,
            operation,
            obligation: None,
        });
    };
    unit.accepted_obligation_facts
        .iter()
        .find(|fact| {
            fact.machine == machine && fact.operation == operation && fact.obligation == obligation
        })
        .map(|fact| fact.identity)
        .ok_or(RuleProposalError::MissingAcceptedObligation {
            machine,
            operation,
            obligation: Some(obligation),
        })
}

fn proof_certified_unary_witness(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    operation: OperationId,
    operand_fact: ScalarConstantFactIdentity,
) -> Result<IntegerEvaluationWitness, RuleProposalError> {
    Ok(IntegerEvaluationWitness::ProofCertifiedUnary {
        operand_fact,
        obligation_fact: accepted_obligation_fact(unit, machine, operation)?,
    })
}

fn integer_binary_witness(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    operation: OperationId,
    safety: OptimizationSafetyClass,
    left_fact: ScalarConstantFactIdentity,
    right_fact: ScalarConstantFactIdentity,
) -> Result<IntegerEvaluationWitness, RuleProposalError> {
    if safety == OptimizationSafetyClass::ProofCertified {
        Ok(IntegerEvaluationWitness::ProofCertifiedBinary {
            left_fact,
            right_fact,
            obligation_fact: accepted_obligation_fact(unit, machine, operation)?,
        })
    } else {
        Ok(IntegerEvaluationWitness::Binary {
            left_fact,
            right_fact,
        })
    }
}

struct IntegerBinaryShape {
    source: OperationId,
    result: ValueId,
    scalar_type: psi_core::IntegerType,
    left: ValueId,
    right: ValueId,
    count_type: Option<psi_core::IntegerType>,
    kind: IntegerBinaryKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntegerBinaryKind {
    ExactAdd,
    ExactSubtract,
    ExactMultiply,
    WrappingAdd,
    WrappingSubtract,
    WrappingMultiply,
    SaturatingAdd,
    SaturatingSubtract,
    SaturatingMultiply,
    ExactDivide,
    ExactRemainder,
    WrappingDivide,
    WrappingRemainder,
    SaturatingDivide,
    SaturatingRemainder,
    ExactShiftLeft,
    ExactShiftRight,
    WrappingShiftLeft,
    WrappingShiftRight,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
}

impl IntegerBinaryShape {
    fn evaluate(&self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        match self.kind {
            IntegerBinaryKind::ExactAdd => self.scalar_type.exact_add(left, right),
            IntegerBinaryKind::ExactSubtract => self.scalar_type.exact_sub(left, right),
            IntegerBinaryKind::ExactMultiply => self.scalar_type.exact_mul(left, right),
            IntegerBinaryKind::WrappingAdd => self.scalar_type.wrapping_add(left, right),
            IntegerBinaryKind::WrappingSubtract => self.scalar_type.wrapping_sub(left, right),
            IntegerBinaryKind::WrappingMultiply => self.scalar_type.wrapping_mul(left, right),
            IntegerBinaryKind::SaturatingAdd => self.scalar_type.saturating_add(left, right),
            IntegerBinaryKind::SaturatingSubtract => self.scalar_type.saturating_sub(left, right),
            IntegerBinaryKind::SaturatingMultiply => self.scalar_type.saturating_mul(left, right),
            IntegerBinaryKind::ExactDivide => self.scalar_type.exact_div(left, right),
            IntegerBinaryKind::ExactRemainder => self.scalar_type.exact_rem(left, right),
            IntegerBinaryKind::WrappingDivide => self.scalar_type.wrapping_div(left, right),
            IntegerBinaryKind::WrappingRemainder => self.scalar_type.wrapping_rem(left, right),
            IntegerBinaryKind::SaturatingDivide => self.scalar_type.saturating_div(left, right),
            IntegerBinaryKind::SaturatingRemainder => self.scalar_type.saturating_rem(left, right),
            IntegerBinaryKind::ExactShiftLeft => self.scalar_type.exact_shift_left(
                left,
                self.count_type.expect("shift count type"),
                right,
            ),
            IntegerBinaryKind::ExactShiftRight => self.scalar_type.exact_shift_right(
                left,
                self.count_type.expect("shift count type"),
                right,
            ),
            IntegerBinaryKind::WrappingShiftLeft => self.scalar_type.wrapping_shift_left(
                left,
                self.count_type.expect("shift count type"),
                right,
            ),
            IntegerBinaryKind::WrappingShiftRight => self.scalar_type.wrapping_shift_right(
                left,
                self.count_type.expect("shift count type"),
                right,
            ),
            IntegerBinaryKind::BitwiseAnd => self.scalar_type.bitwise_and(left, right),
            IntegerBinaryKind::BitwiseOr => self.scalar_type.bitwise_or(left, right),
            IntegerBinaryKind::BitwiseXor => self.scalar_type.bitwise_xor(left, right),
        }
    }
}

fn integer_binary_shape(operation: &O) -> Option<IntegerBinaryShape> {
    let shift = match operation {
        O::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => Some((
            *psi_operation,
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
            IntegerBinaryKind::ExactShiftLeft,
        )),
        O::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => Some((
            *psi_operation,
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
            IntegerBinaryKind::ExactShiftRight,
        )),
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => Some((
            *psi_operation,
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
            IntegerBinaryKind::WrappingShiftLeft,
        )),
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => Some((
            *psi_operation,
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
            IntegerBinaryKind::WrappingShiftRight,
        )),
        _ => None,
    };
    if let Some((source, result, scalar_type, count_type, left, right, kind)) = shift {
        return Some(IntegerBinaryShape {
            source,
            result,
            scalar_type,
            left,
            right,
            count_type: Some(count_type),
            kind,
        });
    }
    let (source, result, scalar_type, left, right, kind) = match operation {
        O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::ExactAdd,
        ),
        O::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::ExactSubtract,
        ),
        O::ExactIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::ExactMultiply,
        ),
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::WrappingAdd,
        ),
        O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::WrappingSubtract,
        ),
        O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::WrappingMultiply,
        ),
        O::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::SaturatingAdd,
        ),
        O::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::SaturatingSubtract,
        ),
        O::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::SaturatingMultiply,
        ),
        O::ExactIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::ExactDivide,
        ),
        O::ExactIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::ExactRemainder,
        ),
        O::WrappingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::WrappingDivide,
        ),
        O::WrappingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::WrappingRemainder,
        ),
        O::SaturatingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::SaturatingDivide,
        ),
        O::SaturatingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::SaturatingRemainder,
        ),
        O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::BitwiseAnd,
        ),
        O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::BitwiseOr,
        ),
        O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::BitwiseXor,
        ),
        _ => return None,
    };
    Some(IntegerBinaryShape {
        source,
        result,
        scalar_type,
        left,
        right,
        count_type: None,
        kind,
    })
}

fn integer_constant(
    constants: &ScalarConstantAnalysis,
    machine: MachineId,
    value: ValueId,
) -> Option<(IntegerValue, ScalarConstantFactIdentity)> {
    constants.facts.iter().find_map(|fact| {
        (fact.valid_in.machine == machine && fact.value == value)
            .then_some(fact)
            .and_then(|fact| match fact.constant {
                ScalarConstant::Integer(value) => fact.identity.map(|identity| (value, identity)),
                ScalarConstant::Boolean(_) => None,
            })
    })
}

fn literal_integer_constant(
    constants: &ScalarConstantAnalysis,
    machine: MachineId,
    value: ValueId,
) -> Option<(IntegerValue, ScalarConstantFactIdentity)> {
    constants.facts.iter().find_map(|fact| {
        (fact.valid_in.machine == machine
            && fact.value == value
            && fact.support.literal_operation().is_some())
        .then_some(fact)
        .and_then(|fact| match fact.constant {
            ScalarConstant::Integer(value) => fact.identity.map(|identity| (value, identity)),
            ScalarConstant::Boolean(_) => None,
        })
    })
}

fn boolean_constant(
    constants: &ScalarConstantAnalysis,
    machine: MachineId,
    value: ValueId,
) -> Option<(bool, ScalarConstantFactIdentity)> {
    constants.facts.iter().find_map(|fact| {
        (fact.valid_in.machine == machine && fact.value == value)
            .then_some(fact)
            .and_then(|fact| match fact.constant {
                ScalarConstant::Boolean(value) => fact.identity.map(|identity| (value, identity)),
                ScalarConstant::Integer(_) => None,
            })
    })
}

fn integer_value_type(
    function: &omega_optimization_unit::PsiOptimizationFunction,
    value: ValueId,
) -> Option<psi_core::IntegerType> {
    function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| &block.parameters))
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .flat_map(|node| &node.definitions),
        )
        .find_map(|definition| {
            (definition.value == value)
                .then_some(definition.scalar_type)
                .and_then(|scalar_type| match scalar_type {
                    psi_core::ScalarType::Integer(integer) => Some(integer),
                    psi_core::ScalarType::Boolean => None,
                })
        })
}

fn propose_redundant_block_parameters(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let Some(AnalysisProduct::ControlFlowGraph(_)) = analyses.get(AnalysisKind::ControlFlowGraph)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ControlFlowGraph,
        ));
    };
    let Some(AnalysisProduct::Dominators(dominators)) = analyses.get(AnalysisKind::Dominators)
    else {
        return Err(RuleProposalError::MissingAnalysis(AnalysisKind::Dominators));
    };
    let Some(AnalysisProduct::UseDefinition(use_definitions)) =
        analyses.get(AnalysisKind::UseDefinition)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::UseDefinition,
        ));
    };

    let mut candidates = Vec::new();
    for function in &unit.functions {
        let machine_dominators = dominators
            .functions
            .iter()
            .find(|(machine, _)| *machine == function.machine)
            .map(|(_, rows)| rows.as_slice())
            .unwrap_or_default();
        for block in function
            .blocks
            .iter()
            .filter(|block| block.id != function.entry)
        {
            for (position, parameter) in block.parameters.iter().enumerate() {
                let mut incoming = Vec::new();
                for source in &function.blocks {
                    for node in &source.nodes {
                        for edge in &node.successors {
                            if edge.target != block.id {
                                continue;
                            }
                            let Some(binding) = edge.bindings.get(position) else {
                                continue;
                            };
                            incoming.push(BlockParameterIncomingBinding {
                                source: source.id,
                                edge: edge.psi_edge,
                                argument: binding.argument,
                            });
                        }
                    }
                }
                incoming.sort_by_key(|row| (row.edge, row.source));
                let Some(replacement) = incoming.first().map(|row| row.argument) else {
                    continue;
                };
                if replacement == parameter.value
                    || incoming.iter().any(|row| row.argument != replacement)
                    || !replacement_dominates_parameter_uses(
                        function.machine,
                        replacement,
                        parameter.value,
                        machine_dominators,
                        use_definitions,
                    )
                {
                    continue;
                }

                let mut affected_blocks = BTreeSet::from([block.id]);
                let mut provenance = Vec::new();
                for source in &function.blocks {
                    for (node_index, node) in source.nodes.iter().enumerate() {
                        let changes_use = node
                            .uses
                            .iter()
                            .any(|use_site| use_site.value == parameter.value);
                        for edge in node
                            .successors
                            .iter()
                            .filter(|edge| edge.target == block.id)
                        {
                            affected_blocks.insert(source.id);
                            let site = PsiRealizationSite::Edge {
                                machine: function.machine,
                                edge: edge.psi_edge,
                            };
                            provenance.push(ProvenanceRewrite {
                                input: site,
                                disposition: ProvenanceDisposition::RealizedAt(site),
                                sources: edge.provenance.clone(),
                                fuel: edge.fuel.clone(),
                            });
                        }
                        if changes_use {
                            affected_blocks.insert(source.id);
                            if !node.provenance.is_empty() {
                                let site = PsiRealizationSite::Node(NodeLocation {
                                    machine: function.machine,
                                    block: source.id,
                                    node: u32::try_from(node_index)
                                        .expect("unit node index fits u32"),
                                });
                                provenance.push(ProvenanceRewrite {
                                    input: site,
                                    disposition: ProvenanceDisposition::RealizedAt(site),
                                    sources: node.provenance.clone(),
                                    fuel: node.fuel.clone(),
                                });
                            }
                        }
                    }
                }
                provenance.sort_by_key(|row| {
                    (
                        row.input,
                        row.disposition.canonical_tag(),
                        row.disposition.site(),
                    )
                });
                candidates.push(
                    PsiRewriteCandidate::new_redundant_block_parameter(
                        unit.identity,
                        contract,
                        affected_blocks.into_iter().collect(),
                        provenance,
                        RedundantBlockParameterWitness { incoming },
                        -1,
                        RedundantBlockParameterRewrite {
                            machine: function.machine,
                            block: block.id,
                            position: u32::try_from(position)
                                .expect("unit parameter position fits u32"),
                            parameter: parameter.value,
                            replacement,
                            scalar_type: parameter.scalar_type,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}

fn replacement_dominates_parameter_uses(
    machine: MachineId,
    replacement: ValueId,
    parameter: ValueId,
    dominators: &[(BlockId, Vec<BlockId>)],
    use_definitions: &crate::UseDefinitionAnalysis,
) -> bool {
    let Some((_, definition)) = use_definitions
        .definitions
        .iter()
        .find(|(owner, definition)| *owner == machine && definition.value == replacement)
    else {
        return false;
    };
    use_definitions
        .uses
        .iter()
        .filter(|(owner, use_site)| *owner == machine && use_site.value == parameter)
        .all(|(_, use_site)| match definition.site {
            omega_optimization_unit::ValueDefinitionSite::FunctionParameter(_) => true,
            omega_optimization_unit::ValueDefinitionSite::BlockParameter {
                block: defining,
                ..
            } => block_dominates(dominators, defining, use_site.block),
            omega_optimization_unit::ValueDefinitionSite::Node {
                block: defining,
                node,
            } if defining == use_site.block => node < use_site.node,
            omega_optimization_unit::ValueDefinitionSite::Node {
                block: defining, ..
            } => block_dominates(dominators, defining, use_site.block),
        })
}

fn block_dominates(
    dominators: &[(BlockId, Vec<BlockId>)],
    dominator: BlockId,
    block: BlockId,
) -> bool {
    dominators
        .iter()
        .find(|(candidate, _)| *candidate == block)
        .is_some_and(|(_, rows)| rows.contains(&dominator))
}

pub fn built_in_psi_registry(
    selections: &OptimizationSelections,
) -> Result<OrderedRuleRegistry, RuleRegistryError> {
    let mut registries = built_in_psi_registries(selections)?;
    if registries.len() > 1 {
        return Err(RuleRegistryError::UnsupportedOptimizationCombination);
    }
    Ok(registries
        .pop()
        .unwrap_or_else(|| OrderedRuleRegistry::new(Vec::new()).expect("empty registry is valid")))
}

/// Build the canonical pass-group schedule for an exact named selection set.
///
/// Selection declaration order is not pass order. The explicit schedule below
/// runs semantic constant propagation before CFG cleanup, structural copy
/// cleanup, local/global value numbering,
/// proof-certified check/work elision, and dead pure scalar
/// elimination. Each returned registry continues to own exactly one pass
/// identity.
pub fn built_in_psi_registries(
    selections: &OptimizationSelections,
) -> Result<Vec<OrderedRuleRegistry>, RuleRegistryError> {
    let psi_selections = selections.for_phase(OptimizationExecutionPhase::Psi);
    if let Some(unsupported) = psi_selections.as_slice().iter().find(|optimization| {
        !matches!(
            optimization,
            Optimization::SparseConditionalConstantPropagation
                | Optimization::ControlFlowCleanup
                | Optimization::CopyPropagation
                | Optimization::GlobalValueNumbering
                | Optimization::DeadPureScalarElimination
                | Optimization::ProofCheckElision
        )
    }) {
        return Err(RuleRegistryError::UnsupportedOptimization(*unsupported));
    }
    let mut registries = Vec::new();
    if psi_selections.contains(Optimization::SparseConditionalConstantPropagation) {
        registries.push(registry_for_optimization(
            Optimization::SparseConditionalConstantPropagation,
        )?);
    }
    if psi_selections.contains(Optimization::ControlFlowCleanup) {
        registries.push(registry_for_optimization(Optimization::ControlFlowCleanup)?);
    }
    if psi_selections.contains(Optimization::CopyPropagation) {
        registries.push(registry_for_optimization(Optimization::CopyPropagation)?);
    }
    if psi_selections.contains(Optimization::GlobalValueNumbering) {
        registries.push(registry_for_optimization(
            Optimization::GlobalValueNumbering,
        )?);
    }
    if psi_selections.contains(Optimization::ProofCheckElision) {
        registries.push(registry_for_optimization(Optimization::ProofCheckElision)?);
    }
    if psi_selections.contains(Optimization::DeadPureScalarElimination) {
        registries.push(registry_for_optimization(
            Optimization::DeadPureScalarElimination,
        )?);
    }
    Ok(registries)
}

fn registry_for_optimization(
    optimization: Optimization,
) -> Result<OrderedRuleRegistry, RuleRegistryError> {
    assemble_built_in_registry(built_in_rule_registrations(optimization))
}

#[derive(Debug, Clone)]
struct BuiltInRuleRegistration {
    schedule_ordinal: u16,
    rule: Arc<dyn PsiOptimizationRule>,
}

fn built_in_rule_registrations(optimization: Optimization) -> Vec<BuiltInRuleRegistration> {
    let mut registrations = Vec::new();
    macro_rules! register {
        ($ordinal:literal, $rule:expr) => {
            registrations.push(BuiltInRuleRegistration {
                schedule_ordinal: $ordinal,
                rule: Arc::new($rule),
            });
        };
    }
    if optimization == Optimization::SparseConditionalConstantPropagation {
        register!(0, ExactIntegerAddConstantsRule);
        register!(1, ExactIntegerSubtractConstantsRule);
        register!(2, ExactIntegerMultiplyConstantsRule);
        register!(3, WrappingIntegerAddConstantsRule);
        register!(4, WrappingIntegerSubtractConstantsRule);
        register!(5, WrappingIntegerMultiplyConstantsRule);
        register!(6, SaturatingIntegerAddConstantsRule);
        register!(7, SaturatingIntegerSubtractConstantsRule);
        register!(8, SaturatingIntegerMultiplyConstantsRule);
        register!(9, ExactIntegerDivideConstantsRule);
        register!(10, ExactIntegerRemainderConstantsRule);
        register!(11, WrappingIntegerDivideConstantsRule);
        register!(12, WrappingIntegerRemainderConstantsRule);
        register!(13, SaturatingIntegerDivideConstantsRule);
        register!(14, SaturatingIntegerRemainderConstantsRule);
        register!(15, ExactIntegerShiftLeftConstantsRule);
        register!(16, ExactIntegerShiftRightConstantsRule);
        register!(17, WrappingIntegerShiftLeftConstantsRule);
        register!(18, WrappingIntegerShiftRightConstantsRule);
        register!(19, ExactIntegerCastConstantsRule);
        register!(20, IntegerWidenConstantsRule);
        register!(21, IntegerBitwiseNotConstantsRule);
        register!(22, IntegerBitwiseAndConstantsRule);
        register!(23, IntegerBitwiseOrConstantsRule);
        register!(24, IntegerBitwiseXorConstantsRule);
        register!(25, BooleanNotConstantsRule);
        register!(26, BooleanEqualConstantsRule);
        register!(27, IntegerEqualConstantsRule);
        register!(28, IntegerLessThanConstantsRule);
        register!(29, IntegerLessOrEqualConstantsRule);
        register!(30, IntegerLessThanRangeConstantRule);
        register!(31, IntegerLessThanConstantRangeRule);
        register!(32, IntegerLessOrEqualRangeConstantRule);
        register!(33, IntegerLessOrEqualConstantRangeRule);
        register!(34, IntegerEqualRangeConstantRule);
        register!(35, IntegerEqualConstantRangeRule);
    }
    if optimization == Optimization::ControlFlowCleanup {
        register!(0, ConstantConditionalFoldRule);
        register!(1, LinearEmptyBlockThreadRule);
        register!(2, PathQualifiedEmptyBlockThreadRule);
        register!(3, AdjacentBlockMergeRule);
        register!(4, SharedJumpFusionRule);
        register!(5, UnreachablePrivateMachinePruneRule);
        register!(6, NonAdjacentBlockMergeRule);
    }
    if optimization == Optimization::CopyPropagation {
        register!(0, RedundantBlockParameterRule);
    }
    if optimization == Optimization::GlobalValueNumbering {
        register!(0, SameBlockTotalScalarCseRule);
        register!(1, SameBlockProofCertifiedScalarCseRule);
        register!(2, DominatorTotalScalarGvnRule);
        register!(3, DominatorProofCertifiedScalarGvnRule);
        register!(4, PhiTranslatedObligationFreeScalarGvnRule);
        register!(5, PhiTranslatedProofCertifiedScalarGvnRule);
        register!(6, SameBlockProofCertifiedCompatiblePolicyScalarCseRule);
        register!(7, DominatorProofCertifiedCompatiblePolicyScalarGvnRule);
        register!(8, PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule);
    }
    if optimization == Optimization::DeadPureScalarElimination {
        register!(0, DeadScalarLiteralEliminationRule);
        register!(1, DeadUnconditionallyTotalScalarEliminationRule);
    }
    if optimization == Optimization::ProofCheckElision {
        register!(0, ProofCertifiedDeadScalarEliminationRule);
        register!(1, LiveProofCertifiedIntegerIdentityEliminationRule);
        register!(2, LiveProofCertifiedIntegerDivideByOneEliminationRule);
        register!(
            3,
            LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
        );
        register!(4, LiveProofCertifiedIntegerZeroDividendEliminationRule);
        register!(
            5,
            LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule
        );
        register!(6, LiveProofCertifiedExactIntegerSelfSubtractEliminationRule);
        register!(7, LiveProofCertifiedIntegerSelfRemainderEliminationRule);
        register!(8, LiveProofCertifiedIntegerSelfDivideEliminationRule);
        register!(9, LiveProofCertifiedIntegerRemainderByOneEliminationRule);
        register!(
            10,
            LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule
        );
    }
    registrations
}

fn assemble_built_in_registry(
    mut registrations: Vec<BuiltInRuleRegistration>,
) -> Result<OrderedRuleRegistry, RuleRegistryError> {
    registrations.sort_by_key(|registration| registration.schedule_ordinal);
    for (expected, registration) in registrations.iter().enumerate() {
        let expected = u16::try_from(expected).expect("built-in rule schedule fits u16");
        assert_eq!(
            registration.schedule_ordinal, expected,
            "built-in rule schedule ordinals must be unique and contiguous"
        );
    }
    OrderedRuleRegistry::new(
        registrations
            .into_iter()
            .map(|registration| registration.rule),
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use omega_abstract_operations::{
        AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
        AbstractOperationPlan, AbstractParameter, AbstractResult, AbstractSuccessor, ValueBinding,
    };
    use omega_optimization_core::{OptimizationFactReference, OptimizationValidatorIdentity};
    use omega_optimization_unit::{
        AcceptedObligationFact, OptimizationFact, OptimizationNode, OwnershipFrontierFact,
        OwnershipFrontierSnapshot, PsiProvenance, PsiRewriteCandidateError, PsiRewritePatch,
        attach_accepted_obligation_facts, recompute_psi_optimization_unit_identity,
        reconstruct_psi_optimization_unit_seed,
    };
    use omega_optimization_validation::{
        OptimizationUnitValidationError, validate_adjacent_block_merge_candidate,
        validate_boolean_evaluation_candidate, validate_constant_conditional_candidate,
        validate_dead_scalar_node_candidate,
        validate_dominating_scalar_common_subexpression_candidate,
        validate_integer_evaluation_candidate, validate_linear_empty_block_candidate,
        validate_local_scalar_common_subexpression_candidate,
        validate_non_adjacent_block_merge_candidate, validate_path_qualified_empty_block_candidate,
        validate_phi_translated_scalar_common_subexpression_candidate,
        validate_proof_certified_exact_integer_self_subtract_candidate,
        validate_proof_certified_integer_remainder_by_one_candidate,
        validate_proof_certified_integer_self_divide_candidate,
        validate_proof_certified_integer_self_remainder_candidate,
        validate_proof_certified_scalar_identity_candidate,
        validate_proof_certified_signed_integer_remainder_by_negative_one_candidate,
        validate_psi_optimization_unit, validate_redundant_block_parameter_candidate,
        validate_shared_jump_fusion_candidate, validate_unreachable_private_machines_candidate,
    };
    use psi_core::{
        BlockId, BoundaryMachineId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType,
        MachineId, ObligationId, OperationId, PlaceId, ScalarType, ServiceId, StructuralTypeId,
        ValueId,
    };
    use psi_terminal::{
        SemanticFingerprint, ServiceDeclaration, TerminalPsiIdentity, VocabularyMarker,
    };

    use super::*;
    use crate::compute_analysis;

    fn shuffle_built_in_registrations(
        registrations: &mut [BuiltInRuleRegistration],
        mut state: u64,
    ) {
        for upper in (1..registrations.len()).rev() {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let index = usize::try_from(
                state % u64::try_from(upper + 1).expect("registration count fits u64"),
            )
            .expect("shuffle index fits usize");
            registrations.swap(upper, index);
        }
    }

    pub(crate) fn randomized_built_in_registries(
        optimization: Optimization,
    ) -> Vec<OrderedRuleRegistry> {
        (1..=32)
            .map(|seed| {
                let mut registrations = built_in_rule_registrations(optimization);
                shuffle_built_in_registrations(&mut registrations, seed);
                assemble_built_in_registry(registrations)
                    .expect("shuffling cannot alter a valid built-in schedule")
            })
            .collect()
    }

    fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
        constructor(raw).expect("nonzero test identity")
    }

    fn with_synthetic_accepted_obligations(unit: PsiOptimizationUnit) -> PsiOptimizationUnit {
        let facts = unit
            .functions
            .iter()
            .flat_map(|function| {
                function.facts.iter().filter_map(|fact| match fact {
                    OptimizationFact::OperationObligationReference {
                        obligation,
                        support,
                    } => Some(AcceptedObligationFact::new(
                        unit.psi,
                        [29; 32],
                        function.machine,
                        *support,
                        *obligation,
                        obligation.get().to_le_bytes().to_vec(),
                    )),
                    _ => None,
                })
            })
            .collect();
        attach_accepted_obligation_facts(unit, facts).unwrap()
    }

    pub(crate) fn exact_add_unit() -> PsiOptimizationUnit {
        exact_chain_unit(false)
    }

    pub(crate) fn live_exact_add_zero_unit() -> PsiOptimizationUnit {
        let mut unit = exact_add_unit();
        let O::IntegerConstant { value, .. } = &mut unit.functions[0].blocks[0].nodes[1].operation
        else {
            unreachable!()
        };
        *value = IntegerValue::Unsigned(0);
        for fact in &mut unit.functions[0].facts {
            if let OptimizationFact::IntegerConstant {
                value, constant, ..
            } = fact
                && *value == id(304, ValueId::new)
            {
                *constant = IntegerValue::Unsigned(0);
            }
        }
        unit.identity = recompute_psi_optimization_unit_identity(&unit);
        validate_psi_optimization_unit(&unit).unwrap();
        unit
    }

    pub(crate) fn live_divide_by_one_unit(
        integer: IntegerType,
        make_operation: impl FnOnce(
            OperationId,
            ObligationId,
            ValueId,
            IntegerType,
            ValueId,
            ValueId,
        ) -> AbstractOperation,
    ) -> PsiOptimizationUnit {
        live_proof_binary_identity_unit(integer, integer_one(integer), false, make_operation)
    }

    pub(crate) fn live_exact_multiply_by_zero_unit(
        integer: IntegerType,
        zero_left: bool,
    ) -> PsiOptimizationUnit {
        live_proof_binary_identity_unit(
            integer,
            integer_zero(integer),
            zero_left,
            |psi_operation, obligation, result, scalar_type, left, right| {
                AbstractOperation::ExactIntegerMultiply {
                    psi_operation,
                    obligation,
                    result,
                    scalar_type,
                    left,
                    right,
                }
            },
        )
    }

    pub(crate) fn live_zero_dividend_unit(
        integer: IntegerType,
        make_operation: impl FnOnce(
            OperationId,
            ObligationId,
            ValueId,
            IntegerType,
            ValueId,
            ValueId,
        ) -> AbstractOperation,
    ) -> PsiOptimizationUnit {
        live_proof_binary_identity_unit(integer, integer_zero(integer), true, make_operation)
    }

    pub(crate) fn live_remainder_by_one_unit(
        integer: IntegerType,
        policy: SelfRemainderPolicy,
    ) -> PsiOptimizationUnit {
        live_proof_binary_identity_unit(
            integer,
            integer_one(integer),
            false,
            |psi_operation, obligation, result, scalar_type, left, right| match policy {
                SelfRemainderPolicy::Exact => AbstractOperation::ExactIntegerRemainder {
                    psi_operation,
                    obligation,
                    result,
                    scalar_type,
                    left,
                    right,
                },
                SelfRemainderPolicy::Wrapping => AbstractOperation::WrappingIntegerRemainder {
                    psi_operation,
                    obligation,
                    result,
                    scalar_type,
                    left,
                    right,
                },
                SelfRemainderPolicy::Saturating => AbstractOperation::SaturatingIntegerRemainder {
                    psi_operation,
                    obligation,
                    result,
                    scalar_type,
                    left,
                    right,
                },
            },
        )
    }

    pub(crate) fn live_signed_remainder_by_negative_one_unit(
        integer: IntegerType,
        policy: SelfRemainderPolicy,
    ) -> PsiOptimizationUnit {
        live_proof_binary_identity_unit(
            integer,
            IntegerValue::Signed(-1),
            false,
            |psi_operation, obligation, result, scalar_type, left, right| match policy {
                SelfRemainderPolicy::Exact => AbstractOperation::ExactIntegerRemainder {
                    psi_operation,
                    obligation,
                    result,
                    scalar_type,
                    left,
                    right,
                },
                SelfRemainderPolicy::Wrapping => AbstractOperation::WrappingIntegerRemainder {
                    psi_operation,
                    obligation,
                    result,
                    scalar_type,
                    left,
                    right,
                },
                SelfRemainderPolicy::Saturating => AbstractOperation::SaturatingIntegerRemainder {
                    psi_operation,
                    obligation,
                    result,
                    scalar_type,
                    left,
                    right,
                },
            },
        )
    }

    pub(crate) fn live_exact_zero_value_shift_unit(
        integer: IntegerType,
        left_shift: bool,
    ) -> PsiOptimizationUnit {
        live_proof_binary_identity_unit(
            integer,
            integer_zero(integer),
            true,
            |psi_operation, obligation, result, value_type, value, count| {
                if left_shift {
                    AbstractOperation::ExactIntegerShiftLeft {
                        psi_operation,
                        obligation,
                        result,
                        value_type,
                        count_type: integer,
                        value,
                        count,
                    }
                } else {
                    AbstractOperation::ExactIntegerShiftRight {
                        psi_operation,
                        obligation,
                        result,
                        value_type,
                        count_type: integer,
                        value,
                        count,
                    }
                }
            },
        )
    }

    pub(crate) fn live_exact_self_subtract_unit(integer: IntegerType) -> PsiOptimizationUnit {
        let machine = id(331, MachineId::new);
        let block = id(332, BlockId::new);
        let operand = id(333, ValueId::new);
        let result = id(334, ValueId::new);
        let operation = id(335, OperationId::new);
        let obligation = id(336, ObligationId::new);
        let scalar_type = ScalarType::Integer(integer);
        let unit = reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([33; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![AbstractFunction {
                    machine,
                    attachment: None,
                    entry: block,
                    parameters: vec![AbstractParameter {
                        value: operand,
                        scalar_type,
                    }],
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
                        AbstractOperation::ExactIntegerSubtract {
                            psi_operation: operation,
                            obligation,
                            result,
                            scalar_type: integer,
                            left: operand,
                            right: operand,
                        },
                        AbstractOperation::Return {
                            psi_edge: id(337, EdgeId::new),
                            result,
                            value: result,
                            scalar_type,
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

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum SelfRemainderPolicy {
        Exact,
        Wrapping,
        Saturating,
    }

    pub(crate) fn live_self_remainder_unit(
        integer: IntegerType,
        policy: SelfRemainderPolicy,
    ) -> PsiOptimizationUnit {
        let machine = id(341, MachineId::new);
        let block = id(342, BlockId::new);
        let operand = id(343, ValueId::new);
        let result = id(344, ValueId::new);
        let operation = id(345, OperationId::new);
        let obligation = id(346, ObligationId::new);
        let scalar_type = ScalarType::Integer(integer);
        let operation = match policy {
            SelfRemainderPolicy::Exact => AbstractOperation::ExactIntegerRemainder {
                psi_operation: operation,
                obligation,
                result,
                scalar_type: integer,
                left: operand,
                right: operand,
            },
            SelfRemainderPolicy::Wrapping => AbstractOperation::WrappingIntegerRemainder {
                psi_operation: operation,
                obligation,
                result,
                scalar_type: integer,
                left: operand,
                right: operand,
            },
            SelfRemainderPolicy::Saturating => AbstractOperation::SaturatingIntegerRemainder {
                psi_operation: operation,
                obligation,
                result,
                scalar_type: integer,
                left: operand,
                right: operand,
            },
        };
        let unit = reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([34; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![AbstractFunction {
                    machine,
                    attachment: None,
                    entry: block,
                    parameters: vec![AbstractParameter {
                        value: operand,
                        scalar_type,
                    }],
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
                        operation,
                        AbstractOperation::Return {
                            psi_edge: id(347, EdgeId::new),
                            result,
                            value: result,
                            scalar_type,
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

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum SelfDividePolicy {
        Exact,
        Wrapping,
        Saturating,
    }

    pub(crate) fn live_self_divide_unit(
        integer: IntegerType,
        policy: SelfDividePolicy,
    ) -> PsiOptimizationUnit {
        let machine = id(351, MachineId::new);
        let block = id(352, BlockId::new);
        let operand = id(353, ValueId::new);
        let result = id(354, ValueId::new);
        let operation = id(355, OperationId::new);
        let obligation = id(356, ObligationId::new);
        let scalar_type = ScalarType::Integer(integer);
        let operation = match policy {
            SelfDividePolicy::Exact => AbstractOperation::ExactIntegerDivide {
                psi_operation: operation,
                obligation,
                result,
                scalar_type: integer,
                left: operand,
                right: operand,
            },
            SelfDividePolicy::Wrapping => AbstractOperation::WrappingIntegerDivide {
                psi_operation: operation,
                obligation,
                result,
                scalar_type: integer,
                left: operand,
                right: operand,
            },
            SelfDividePolicy::Saturating => AbstractOperation::SaturatingIntegerDivide {
                psi_operation: operation,
                obligation,
                result,
                scalar_type: integer,
                left: operand,
                right: operand,
            },
        };
        let unit = reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([35; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![AbstractFunction {
                    machine,
                    attachment: None,
                    entry: block,
                    parameters: vec![AbstractParameter {
                        value: operand,
                        scalar_type,
                    }],
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
                        operation,
                        AbstractOperation::Return {
                            psi_edge: id(357, EdgeId::new),
                            result,
                            value: result,
                            scalar_type,
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

    fn live_proof_binary_identity_unit(
        integer: IntegerType,
        literal_value: IntegerValue,
        literal_left: bool,
        make_operation: impl FnOnce(
            OperationId,
            ObligationId,
            ValueId,
            IntegerType,
            ValueId,
            ValueId,
        ) -> AbstractOperation,
    ) -> PsiOptimizationUnit {
        let machine = id(321, MachineId::new);
        let block = id(322, BlockId::new);
        let other = id(323, ValueId::new);
        let literal = id(324, ValueId::new);
        let computed = id(325, ValueId::new);
        let literal_operation = id(326, OperationId::new);
        let binary_operation = id(327, OperationId::new);
        let obligation = id(328, ObligationId::new);
        let scalar_type = ScalarType::Integer(integer);
        let (left, right) = if literal_left {
            (literal, other)
        } else {
            (other, literal)
        };
        let unit = reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([31; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![AbstractFunction {
                    machine,
                    attachment: None,
                    entry: block,
                    parameters: vec![AbstractParameter {
                        value: other,
                        scalar_type,
                    }],
                    structural_parameters: Vec::new(),
                    result: AbstractFunctionResult::Scalar(AbstractResult {
                        value: computed,
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
                            psi_operation: literal_operation,
                            result: literal,
                            scalar_type,
                            value: literal_value,
                        },
                        make_operation(
                            binary_operation,
                            obligation,
                            computed,
                            integer,
                            left,
                            right,
                        ),
                        AbstractOperation::Return {
                            psi_edge: id(329, EdgeId::new),
                            result: computed,
                            value: computed,
                            scalar_type,
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

    pub(crate) fn dependent_exact_chain_unit() -> PsiOptimizationUnit {
        exact_chain_unit(true)
    }

    fn exact_chain_unit(include_multiply: bool) -> PsiOptimizationUnit {
        let machine = id(301, MachineId::new);
        let block = id(302, BlockId::new);
        let left = id(303, ValueId::new);
        let right = id(304, ValueId::new);
        let sum = id(305, ValueId::new);
        let product = id(311, ValueId::new);
        let result = if include_multiply { product } else { sum };
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let mut operations = vec![
            AbstractOperation::IntegerConstant {
                psi_operation: id(306, OperationId::new),
                result: left,
                scalar_type: ScalarType::Integer(integer),
                value: IntegerValue::Unsigned(7),
            },
            AbstractOperation::IntegerConstant {
                psi_operation: id(307, OperationId::new),
                result: right,
                scalar_type: ScalarType::Integer(integer),
                value: IntegerValue::Unsigned(8),
            },
            AbstractOperation::ExactIntegerAdd {
                psi_operation: id(308, OperationId::new),
                obligation: id(309, ObligationId::new),
                result: sum,
                scalar_type: integer,
                left,
                right,
            },
        ];
        if include_multiply {
            operations.push(AbstractOperation::ExactIntegerMultiply {
                psi_operation: id(312, OperationId::new),
                obligation: id(313, ObligationId::new),
                result: product,
                scalar_type: integer,
                left: sum,
                right,
            });
        }
        operations.push(AbstractOperation::Return {
            psi_edge: id(310, EdgeId::new),
            result,
            value: result,
            scalar_type: ScalarType::Integer(integer),
            cleanup_actions: Vec::new(),
        });
        let unit = reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([13; 32]),
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
                        scalar_type: ScalarType::Integer(integer),
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![AbstractBlockEntry {
                        block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations,
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        with_synthetic_accepted_obligations(unit)
    }

    pub(crate) fn propagated_block_parameter_unit(constant: bool) -> PsiOptimizationUnit {
        let machine = id(601, MachineId::new);
        let entry = id(602, BlockId::new);
        let when_true = id(603, BlockId::new);
        let when_false = id(604, BlockId::new);
        let merge = id(605, BlockId::new);
        let condition = id(606, ValueId::new);
        let true_value = id(607, ValueId::new);
        let false_value = id(608, ValueId::new);
        let parameter = id(609, ValueId::new);
        let result = id(610, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let scalar_type = ScalarType::Integer(integer);
        let binding = |argument| ValueBinding {
            parameter,
            argument,
            scalar_type,
        };
        reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([21; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![AbstractFunction {
                    machine,
                    attachment: None,
                    entry,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: AbstractFunctionResult::Scalar(AbstractResult {
                        value: result,
                        scalar_type,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![
                        AbstractBlockEntry {
                            block: entry,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        },
                        AbstractBlockEntry {
                            block: when_true,
                            parameters: Vec::new(),
                            operation_offset: 2,
                        },
                        AbstractBlockEntry {
                            block: when_false,
                            parameters: Vec::new(),
                            operation_offset: 4,
                        },
                        AbstractBlockEntry {
                            block: merge,
                            parameters: vec![AbstractParameter {
                                value: parameter,
                                scalar_type,
                            }],
                            operation_offset: 6,
                        },
                    ],
                    operations: vec![
                        AbstractOperation::BooleanConstant {
                            psi_operation: id(611, OperationId::new),
                            result: condition,
                            value: constant,
                        },
                        AbstractOperation::Conditional {
                            condition,
                            when_true: AbstractSuccessor {
                                psi_edge: id(612, EdgeId::new),
                                target: when_true,
                                bindings: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: AbstractSuccessor {
                                psi_edge: id(613, EdgeId::new),
                                target: when_false,
                                bindings: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                        AbstractOperation::IntegerConstant {
                            psi_operation: id(614, OperationId::new),
                            result: true_value,
                            scalar_type,
                            value: IntegerValue::Unsigned(7),
                        },
                        AbstractOperation::Jump {
                            psi_edge: id(615, EdgeId::new),
                            target: merge,
                            bindings: vec![binding(true_value)],
                            trivial_affine_discards: Vec::new(),
                        },
                        AbstractOperation::IntegerConstant {
                            psi_operation: id(616, OperationId::new),
                            result: false_value,
                            scalar_type,
                            value: IntegerValue::Unsigned(8),
                        },
                        AbstractOperation::Jump {
                            psi_edge: id(617, EdgeId::new),
                            target: merge,
                            bindings: vec![binding(false_value)],
                            trivial_affine_discards: Vec::new(),
                        },
                        AbstractOperation::IntegerBitwiseNot {
                            psi_operation: id(618, OperationId::new),
                            result,
                            scalar_type: integer,
                            operand: parameter,
                        },
                        AbstractOperation::Return {
                            psi_edge: id(619, EdgeId::new),
                            result,
                            value: result,
                            scalar_type,
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    fn constant_conditional_dead_service_unit() -> PsiOptimizationUnit {
        let mut unit = propagated_block_parameter_unit(true);
        let service = id(620, ServiceId::new);
        let operation = id(621, OperationId::new);
        unit.services = vec![ServiceDeclaration {
            id: service,
            identity: "validation::dead-branch-service".into(),
            parents: Vec::new(),
        }]
        .into();
        unit.functions[0].published_service_ceiling = vec![service];
        let rejected = unit.functions[0]
            .blocks
            .iter_mut()
            .find(|block| block.id == id(604, BlockId::new))
            .expect("constant fixture retains its rejected branch");
        rejected.nodes.insert(
            1,
            OptimizationNode {
                operation: AbstractOperation::PortWrite {
                    psi_operation: operation,
                    service,
                    port: 0x3f8,
                    value: 0x41,
                },
                provenance: vec![PsiProvenance::Operation(operation)],
                fuel: vec![omega_optimization_unit::FuelSettlement {
                    site: PsiProvenance::Operation(operation),
                    units: 1,
                }],
                effect: omega_optimization_unit::EffectLink {
                    input: 0,
                    output: 0,
                },
                definitions: Vec::new(),
                uses: Vec::new(),
                successors: Vec::new(),
                ownership: Vec::new(),
            },
        );
        let mut effect = 0u64;
        for block in &mut unit.functions[0].blocks {
            for (node_index, node) in block.nodes.iter_mut().enumerate() {
                let node_index = u32::try_from(node_index).expect("fixture node index fits u32");
                for definition in &mut node.definitions {
                    if let omega_optimization_unit::ValueDefinitionSite::Node {
                        block: site_block,
                        node: site_node,
                    } = &mut definition.site
                    {
                        *site_block = block.id;
                        *site_node = node_index;
                    }
                }
                for value_use in &mut node.uses {
                    value_use.block = block.id;
                    value_use.node = node_index;
                }
                node.effect = omega_optimization_unit::EffectLink {
                    input: effect,
                    output: effect + 1,
                };
                effect += 1;
            }
        }
        unit.root_service_reach.concrete = vec![service];
        unit.identity = recompute_psi_optimization_unit_identity(&unit);
        unit
    }

    pub(crate) fn linear_empty_block_unit() -> PsiOptimizationUnit {
        let machine = id(901, MachineId::new);
        let entry = id(902, BlockId::new);
        let empty = id(903, BlockId::new);
        let target = id(904, BlockId::new);
        let left = id(905, ValueId::new);
        let right = id(906, ValueId::new);
        let first = id(907, ValueId::new);
        let second = id(908, ValueId::new);
        let target_first = id(909, ValueId::new);
        let target_second = id(910, ValueId::new);
        let scalar_type = ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 8).expect("valid fixture integer"),
        );
        let parameter = |value| AbstractParameter { value, scalar_type };
        let binding = |parameter, argument| ValueBinding {
            parameter,
            argument,
            scalar_type,
        };
        reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([31; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![AbstractFunction {
                    machine,
                    attachment: None,
                    entry,
                    parameters: vec![
                        AbstractParameter {
                            value: left,
                            scalar_type,
                        },
                        AbstractParameter {
                            value: right,
                            scalar_type,
                        },
                    ],
                    structural_parameters: Vec::new(),
                    result: AbstractFunctionResult::Unit,
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![
                        AbstractBlockEntry {
                            block: entry,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        },
                        AbstractBlockEntry {
                            block: empty,
                            parameters: vec![parameter(first), parameter(second)],
                            operation_offset: 1,
                        },
                        AbstractBlockEntry {
                            block: target,
                            parameters: vec![parameter(target_first), parameter(target_second)],
                            operation_offset: 2,
                        },
                    ],
                    operations: vec![
                        AbstractOperation::Jump {
                            psi_edge: id(911, EdgeId::new),
                            target: empty,
                            bindings: vec![binding(first, left), binding(second, right)],
                            trivial_affine_discards: Vec::new(),
                        },
                        AbstractOperation::Jump {
                            psi_edge: id(912, EdgeId::new),
                            target,
                            bindings: vec![
                                binding(target_first, second),
                                binding(target_second, first),
                            ],
                            trivial_affine_discards: Vec::new(),
                        },
                        AbstractOperation::ReturnUnit {
                            psi_edge: id(913, EdgeId::new),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    pub(crate) fn path_qualified_empty_block_unit() -> PsiOptimizationUnit {
        let machine = id(921, MachineId::new);
        let entry = id(922, BlockId::new);
        let left_block = id(923, BlockId::new);
        let right_block = id(924, BlockId::new);
        let empty = id(925, BlockId::new);
        let target = id(926, BlockId::new);
        let condition = id(927, ValueId::new);
        reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([32; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![AbstractFunction {
                    machine,
                    attachment: None,
                    entry,
                    parameters: vec![AbstractParameter {
                        value: condition,
                        scalar_type: ScalarType::Boolean,
                    }],
                    structural_parameters: Vec::new(),
                    result: AbstractFunctionResult::Unit,
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![
                        AbstractBlockEntry {
                            block: entry,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        },
                        AbstractBlockEntry {
                            block: left_block,
                            parameters: Vec::new(),
                            operation_offset: 1,
                        },
                        AbstractBlockEntry {
                            block: right_block,
                            parameters: Vec::new(),
                            operation_offset: 2,
                        },
                        AbstractBlockEntry {
                            block: empty,
                            parameters: Vec::new(),
                            operation_offset: 3,
                        },
                        AbstractBlockEntry {
                            block: target,
                            parameters: Vec::new(),
                            operation_offset: 4,
                        },
                    ],
                    operations: vec![
                        AbstractOperation::Conditional {
                            condition,
                            when_true: AbstractSuccessor {
                                psi_edge: id(931, EdgeId::new),
                                target: left_block,
                                bindings: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: AbstractSuccessor {
                                psi_edge: id(932, EdgeId::new),
                                target: right_block,
                                bindings: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                        AbstractOperation::Jump {
                            psi_edge: id(933, EdgeId::new),
                            target: empty,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        AbstractOperation::Jump {
                            psi_edge: id(934, EdgeId::new),
                            target: empty,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        AbstractOperation::Jump {
                            psi_edge: id(935, EdgeId::new),
                            target,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        AbstractOperation::ReturnUnit {
                            psi_edge: id(936, EdgeId::new),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    pub(crate) fn shared_terminal_unit() -> PsiOptimizationUnit {
        let machine = id(921, MachineId::new);
        let entry = id(922, BlockId::new);
        let left_block = id(923, BlockId::new);
        let right_block = id(924, BlockId::new);
        let target = id(926, BlockId::new);
        let condition = id(927, ValueId::new);
        reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([38; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![AbstractFunction {
                    machine,
                    attachment: None,
                    entry,
                    parameters: vec![AbstractParameter {
                        value: condition,
                        scalar_type: ScalarType::Boolean,
                    }],
                    structural_parameters: Vec::new(),
                    result: AbstractFunctionResult::Unit,
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![
                        AbstractBlockEntry {
                            block: entry,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        },
                        AbstractBlockEntry {
                            block: left_block,
                            parameters: Vec::new(),
                            operation_offset: 1,
                        },
                        AbstractBlockEntry {
                            block: right_block,
                            parameters: Vec::new(),
                            operation_offset: 2,
                        },
                        AbstractBlockEntry {
                            block: target,
                            parameters: Vec::new(),
                            operation_offset: 3,
                        },
                    ],
                    operations: vec![
                        AbstractOperation::Conditional {
                            condition,
                            when_true: AbstractSuccessor {
                                psi_edge: id(931, EdgeId::new),
                                target: left_block,
                                bindings: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: AbstractSuccessor {
                                psi_edge: id(932, EdgeId::new),
                                target: right_block,
                                bindings: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                        AbstractOperation::Jump {
                            psi_edge: id(933, EdgeId::new),
                            target,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        AbstractOperation::Jump {
                            psi_edge: id(934, EdgeId::new),
                            target,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        AbstractOperation::ReturnUnit {
                            psi_edge: id(936, EdgeId::new),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

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

    fn scalar_local_cse_unit(proof_certified: bool) -> PsiOptimizationUnit {
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

    pub(crate) fn dominator_gvn_unit() -> PsiOptimizationUnit {
        scalar_dominator_gvn_unit(false)
    }

    pub(crate) fn proof_certified_dominator_gvn_unit() -> PsiOptimizationUnit {
        scalar_dominator_gvn_unit(true)
    }

    pub(crate) fn compatible_policy_dominator_gvn_unit() -> PsiOptimizationUnit {
        let mut unit = proof_certified_dominator_gvn_unit();
        let node = &mut unit.functions[0].blocks[1].nodes[0];
        let O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } = node.operation
        else {
            unreachable!("proof GVN leader is exact add")
        };
        node.operation = O::SaturatingIntegerAdd {
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

    fn scalar_dominator_gvn_unit(proof_certified: bool) -> PsiOptimizationUnit {
        let machine = id(1_341, MachineId::new);
        let dominated = id(1_342, BlockId::new);
        let entry = id(1_343, BlockId::new);
        let left = id(1_344, ValueId::new);
        let right = id(1_345, ValueId::new);
        let leader = id(1_346, ValueId::new);
        let redundant = id(1_347, ValueId::new);
        let equal = id(1_348, ValueId::new);
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
                    program_fingerprint: SemanticFingerprint::from_bytes([42; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![AbstractFunction {
                    machine,
                    attachment: None,
                    entry,
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
                    block_entries: vec![
                        AbstractBlockEntry {
                            block: dominated,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        },
                        AbstractBlockEntry {
                            block: entry,
                            parameters: Vec::new(),
                            operation_offset: 3,
                        },
                    ],
                    operations: vec![
                        binary(
                            id(1_351, OperationId::new),
                            id(1_354, ObligationId::new),
                            redundant,
                            right,
                            left,
                        ),
                        AbstractOperation::IntegerEqual {
                            psi_operation: id(1_352, OperationId::new),
                            result: equal,
                            left: leader,
                            right: redundant,
                        },
                        AbstractOperation::Return {
                            psi_edge: id(1_353, EdgeId::new),
                            result: equal,
                            value: equal,
                            scalar_type: ScalarType::Boolean,
                            cleanup_actions: Vec::new(),
                        },
                        binary(
                            id(1_349, OperationId::new),
                            id(1_355, ObligationId::new),
                            leader,
                            left,
                            right,
                        ),
                        AbstractOperation::Jump {
                            psi_edge: id(1_350, EdgeId::new),
                            target: dominated,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
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

    pub(crate) fn diamond_dominator_gvn_unit() -> PsiOptimizationUnit {
        let machine = id(1_401, MachineId::new);
        let join = id(1_402, BlockId::new);
        let left_block = id(1_403, BlockId::new);
        let entry = id(1_404, BlockId::new);
        let right_block = id(1_405, BlockId::new);
        let condition = id(1_406, ValueId::new);
        let operand = id(1_407, ValueId::new);
        let outer_first = id(1_408, ValueId::new);
        let outer_second = id(1_409, ValueId::new);
        let inner_first = id(1_410, ValueId::new);
        let inner_second = id(1_411, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([43; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![AbstractFunction {
                    machine,
                    attachment: None,
                    entry,
                    parameters: vec![
                        AbstractParameter {
                            value: condition,
                            scalar_type: ScalarType::Boolean,
                        },
                        AbstractParameter {
                            value: operand,
                            scalar_type: ScalarType::Integer(integer),
                        },
                    ],
                    structural_parameters: Vec::new(),
                    result: AbstractFunctionResult::Scalar(AbstractResult {
                        value: inner_second,
                        scalar_type: ScalarType::Integer(integer),
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![
                        AbstractBlockEntry {
                            block: join,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        },
                        AbstractBlockEntry {
                            block: left_block,
                            parameters: Vec::new(),
                            operation_offset: 3,
                        },
                        AbstractBlockEntry {
                            block: entry,
                            parameters: Vec::new(),
                            operation_offset: 4,
                        },
                        AbstractBlockEntry {
                            block: right_block,
                            parameters: Vec::new(),
                            operation_offset: 7,
                        },
                    ],
                    operations: vec![
                        AbstractOperation::IntegerBitwiseNot {
                            psi_operation: id(1_412, OperationId::new),
                            result: inner_first,
                            scalar_type: integer,
                            operand,
                        },
                        AbstractOperation::IntegerBitwiseNot {
                            psi_operation: id(1_413, OperationId::new),
                            result: inner_second,
                            scalar_type: integer,
                            operand: inner_first,
                        },
                        AbstractOperation::Return {
                            psi_edge: id(1_414, EdgeId::new),
                            result: inner_second,
                            value: inner_second,
                            scalar_type: ScalarType::Integer(integer),
                            cleanup_actions: Vec::new(),
                        },
                        AbstractOperation::Jump {
                            psi_edge: id(1_415, EdgeId::new),
                            target: join,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        AbstractOperation::IntegerBitwiseNot {
                            psi_operation: id(1_416, OperationId::new),
                            result: outer_first,
                            scalar_type: integer,
                            operand,
                        },
                        AbstractOperation::IntegerBitwiseNot {
                            psi_operation: id(1_417, OperationId::new),
                            result: outer_second,
                            scalar_type: integer,
                            operand: outer_first,
                        },
                        AbstractOperation::Conditional {
                            condition,
                            when_true: AbstractSuccessor {
                                psi_edge: id(1_418, EdgeId::new),
                                target: left_block,
                                bindings: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: AbstractSuccessor {
                                psi_edge: id(1_419, EdgeId::new),
                                target: right_block,
                                bindings: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                        AbstractOperation::Jump {
                            psi_edge: id(1_420, EdgeId::new),
                            target: join,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    fn sibling_only_gvn_unit() -> PsiOptimizationUnit {
        let machine = id(1_441, MachineId::new);
        let join = id(1_442, BlockId::new);
        let left_block = id(1_443, BlockId::new);
        let entry = id(1_444, BlockId::new);
        let right_block = id(1_445, BlockId::new);
        let condition = id(1_446, ValueId::new);
        let operand = id(1_447, ValueId::new);
        let sibling = id(1_448, ValueId::new);
        let redundant = id(1_449, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([44; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![AbstractFunction {
                    machine,
                    attachment: None,
                    entry,
                    parameters: vec![
                        AbstractParameter {
                            value: condition,
                            scalar_type: ScalarType::Boolean,
                        },
                        AbstractParameter {
                            value: operand,
                            scalar_type: ScalarType::Integer(integer),
                        },
                    ],
                    structural_parameters: Vec::new(),
                    result: AbstractFunctionResult::Scalar(AbstractResult {
                        value: redundant,
                        scalar_type: ScalarType::Integer(integer),
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![
                        AbstractBlockEntry {
                            block: join,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        },
                        AbstractBlockEntry {
                            block: left_block,
                            parameters: Vec::new(),
                            operation_offset: 2,
                        },
                        AbstractBlockEntry {
                            block: entry,
                            parameters: Vec::new(),
                            operation_offset: 4,
                        },
                        AbstractBlockEntry {
                            block: right_block,
                            parameters: Vec::new(),
                            operation_offset: 5,
                        },
                    ],
                    operations: vec![
                        AbstractOperation::IntegerBitwiseNot {
                            psi_operation: id(1_450, OperationId::new),
                            result: redundant,
                            scalar_type: integer,
                            operand,
                        },
                        AbstractOperation::Return {
                            psi_edge: id(1_451, EdgeId::new),
                            result: redundant,
                            value: redundant,
                            scalar_type: ScalarType::Integer(integer),
                            cleanup_actions: Vec::new(),
                        },
                        AbstractOperation::IntegerBitwiseNot {
                            psi_operation: id(1_452, OperationId::new),
                            result: sibling,
                            scalar_type: integer,
                            operand,
                        },
                        AbstractOperation::Jump {
                            psi_edge: id(1_453, EdgeId::new),
                            target: join,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        AbstractOperation::Conditional {
                            condition,
                            when_true: AbstractSuccessor {
                                psi_edge: id(1_454, EdgeId::new),
                                target: left_block,
                                bindings: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: AbstractSuccessor {
                                psi_edge: id(1_455, EdgeId::new),
                                target: right_block,
                                bindings: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                        AbstractOperation::Jump {
                            psi_edge: id(1_456, EdgeId::new),
                            target: join,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    #[derive(Clone, Copy)]
    enum PhiTranslatedRightArm {
        Matching,
        Missing,
        MismatchedType,
    }

    pub(crate) fn phi_translated_gvn_unit() -> PsiOptimizationUnit {
        phi_translated_gvn_fixture(PhiTranslatedRightArm::Matching, false, false)
    }

    pub(crate) fn proof_certified_phi_translated_gvn_unit() -> PsiOptimizationUnit {
        phi_translated_gvn_fixture(PhiTranslatedRightArm::Matching, true, false)
    }

    pub(crate) fn compatible_policy_phi_translated_gvn_unit() -> PsiOptimizationUnit {
        phi_translated_gvn_fixture(PhiTranslatedRightArm::Matching, false, true)
    }

    fn phi_translated_gvn_fixture(
        right_arm: PhiTranslatedRightArm,
        proof_certified: bool,
        compatible_policy: bool,
    ) -> PsiOptimizationUnit {
        let machine = id(1_701, MachineId::new);
        let join = id(1_702, BlockId::new);
        let left_block = id(1_703, BlockId::new);
        let entry = id(1_704, BlockId::new);
        let right_block = id(1_705, BlockId::new);
        let condition = id(1_706, ValueId::new);
        let left_input = id(1_707, ValueId::new);
        let right_input = id(1_708, ValueId::new);
        let join_input = id(1_709, ValueId::new);
        let redundant = id(1_710, ValueId::new);
        let left_leader = id(1_711, ValueId::new);
        let right_leader = id(1_712, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let wide = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
        let redundant_expression = if proof_certified || compatible_policy {
            AbstractOperation::ExactIntegerAdd {
                psi_operation: id(1_713, OperationId::new),
                obligation: id(1_721, ObligationId::new),
                result: redundant,
                scalar_type: integer,
                left: join_input,
                right: join_input,
            }
        } else {
            AbstractOperation::IntegerBitwiseNot {
                psi_operation: id(1_713, OperationId::new),
                result: redundant,
                scalar_type: integer,
                operand: join_input,
            }
        };
        let left_expression = if proof_certified {
            AbstractOperation::ExactIntegerAdd {
                psi_operation: id(1_715, OperationId::new),
                obligation: id(1_722, ObligationId::new),
                result: left_leader,
                scalar_type: integer,
                left: left_input,
                right: left_input,
            }
        } else if compatible_policy {
            AbstractOperation::WrappingIntegerAdd {
                psi_operation: id(1_715, OperationId::new),
                result: left_leader,
                scalar_type: integer,
                left: left_input,
                right: left_input,
            }
        } else {
            AbstractOperation::IntegerBitwiseNot {
                psi_operation: id(1_715, OperationId::new),
                result: left_leader,
                scalar_type: integer,
                operand: left_input,
            }
        };
        let right_expression = if proof_certified {
            AbstractOperation::ExactIntegerAdd {
                psi_operation: id(1_716, OperationId::new),
                obligation: id(1_723, ObligationId::new),
                result: right_leader,
                scalar_type: integer,
                left: right_input,
                right: right_input,
            }
        } else if compatible_policy {
            match right_arm {
                PhiTranslatedRightArm::Matching => AbstractOperation::SaturatingIntegerAdd {
                    psi_operation: id(1_716, OperationId::new),
                    result: right_leader,
                    scalar_type: integer,
                    left: right_input,
                    right: right_input,
                },
                PhiTranslatedRightArm::Missing => AbstractOperation::WrappingIntegerSubtract {
                    psi_operation: id(1_716, OperationId::new),
                    result: right_leader,
                    scalar_type: integer,
                    left: right_input,
                    right: right_input,
                },
                PhiTranslatedRightArm::MismatchedType => AbstractOperation::IntegerWiden {
                    psi_operation: id(1_716, OperationId::new),
                    result: right_leader,
                    source_type: integer,
                    target_type: wide,
                    operand: right_input,
                },
            }
        } else {
            match right_arm {
                PhiTranslatedRightArm::Matching => AbstractOperation::IntegerBitwiseNot {
                    psi_operation: id(1_716, OperationId::new),
                    result: right_leader,
                    scalar_type: integer,
                    operand: right_input,
                },
                PhiTranslatedRightArm::Missing => AbstractOperation::WrappingIntegerAdd {
                    psi_operation: id(1_716, OperationId::new),
                    result: right_leader,
                    scalar_type: integer,
                    left: right_input,
                    right: right_input,
                },
                PhiTranslatedRightArm::MismatchedType => AbstractOperation::IntegerWiden {
                    psi_operation: id(1_716, OperationId::new),
                    result: right_leader,
                    source_type: integer,
                    target_type: wide,
                    operand: right_input,
                },
            }
        };
        let unit = reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([47; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![AbstractFunction {
                    machine,
                    attachment: None,
                    entry,
                    parameters: vec![
                        AbstractParameter {
                            value: condition,
                            scalar_type: ScalarType::Boolean,
                        },
                        AbstractParameter {
                            value: left_input,
                            scalar_type: ScalarType::Integer(integer),
                        },
                        AbstractParameter {
                            value: right_input,
                            scalar_type: ScalarType::Integer(integer),
                        },
                    ],
                    structural_parameters: Vec::new(),
                    result: AbstractFunctionResult::Scalar(AbstractResult {
                        value: redundant,
                        scalar_type: ScalarType::Integer(integer),
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![
                        AbstractBlockEntry {
                            block: join,
                            parameters: vec![AbstractParameter {
                                value: join_input,
                                scalar_type: ScalarType::Integer(integer),
                            }],
                            operation_offset: 0,
                        },
                        AbstractBlockEntry {
                            block: left_block,
                            parameters: Vec::new(),
                            operation_offset: 2,
                        },
                        AbstractBlockEntry {
                            block: entry,
                            parameters: Vec::new(),
                            operation_offset: 4,
                        },
                        AbstractBlockEntry {
                            block: right_block,
                            parameters: Vec::new(),
                            operation_offset: 5,
                        },
                    ],
                    operations: vec![
                        redundant_expression,
                        AbstractOperation::Return {
                            psi_edge: id(1_714, EdgeId::new),
                            result: redundant,
                            value: redundant,
                            scalar_type: ScalarType::Integer(integer),
                            cleanup_actions: Vec::new(),
                        },
                        left_expression,
                        AbstractOperation::Jump {
                            psi_edge: id(1_720, EdgeId::new),
                            target: join,
                            bindings: vec![ValueBinding {
                                parameter: join_input,
                                argument: left_input,
                                scalar_type: ScalarType::Integer(integer),
                            }],
                            trivial_affine_discards: Vec::new(),
                        },
                        AbstractOperation::Conditional {
                            condition,
                            when_true: AbstractSuccessor {
                                psi_edge: id(1_718, EdgeId::new),
                                target: left_block,
                                bindings: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: AbstractSuccessor {
                                psi_edge: id(1_719, EdgeId::new),
                                target: right_block,
                                bindings: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                        right_expression,
                        AbstractOperation::Jump {
                            psi_edge: id(1_717, EdgeId::new),
                            target: join,
                            bindings: vec![ValueBinding {
                                parameter: join_input,
                                argument: right_input,
                                scalar_type: ScalarType::Integer(integer),
                            }],
                            trivial_affine_discards: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        if proof_certified || compatible_policy {
            with_synthetic_accepted_obligations(unit)
        } else {
            unit
        }
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

    fn discard_scalar_function_result(mut unit: PsiOptimizationUnit) -> PsiOptimizationUnit {
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

    fn adjacent_conditional_merge_unit() -> PsiOptimizationUnit {
        let machine = id(1_101, MachineId::new);
        let entry = id(1_102, BlockId::new);
        let decision = id(1_103, BlockId::new);
        let left = id(1_104, BlockId::new);
        let right = id(1_105, BlockId::new);
        let condition = id(1_106, ValueId::new);
        let forwarded = id(1_107, ValueId::new);
        reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([37; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![AbstractFunction {
                    machine,
                    attachment: None,
                    entry,
                    parameters: vec![AbstractParameter {
                        value: condition,
                        scalar_type: ScalarType::Boolean,
                    }],
                    structural_parameters: Vec::new(),
                    result: AbstractFunctionResult::Unit,
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![
                        AbstractBlockEntry {
                            block: entry,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        },
                        AbstractBlockEntry {
                            block: decision,
                            parameters: vec![AbstractParameter {
                                value: forwarded,
                                scalar_type: ScalarType::Boolean,
                            }],
                            operation_offset: 1,
                        },
                        AbstractBlockEntry {
                            block: left,
                            parameters: Vec::new(),
                            operation_offset: 2,
                        },
                        AbstractBlockEntry {
                            block: right,
                            parameters: Vec::new(),
                            operation_offset: 3,
                        },
                    ],
                    operations: vec![
                        AbstractOperation::Jump {
                            psi_edge: id(1_110, EdgeId::new),
                            target: decision,
                            bindings: vec![ValueBinding {
                                parameter: forwarded,
                                argument: condition,
                                scalar_type: ScalarType::Boolean,
                            }],
                            trivial_affine_discards: Vec::new(),
                        },
                        AbstractOperation::Conditional {
                            condition: forwarded,
                            when_true: AbstractSuccessor {
                                psi_edge: id(1_111, EdgeId::new),
                                target: left,
                                bindings: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: AbstractSuccessor {
                                psi_edge: id(1_112, EdgeId::new),
                                target: right,
                                bindings: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                        AbstractOperation::ReturnUnit {
                            psi_edge: id(1_113, EdgeId::new),
                            cleanup_actions: Vec::new(),
                        },
                        AbstractOperation::ReturnUnit {
                            psi_edge: id(1_114, EdgeId::new),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    pub(crate) fn non_adjacent_merge_unit(target_before_predecessor: bool) -> PsiOptimizationUnit {
        let machine = id(1_501, MachineId::new);
        let entry = id(1_502, BlockId::new);
        let descendant = id(1_503, BlockId::new);
        let target = id(1_504, BlockId::new);
        let sibling = id(1_505, BlockId::new);
        let predecessor = id(1_506, BlockId::new);
        let condition = id(1_507, ValueId::new);
        let incoming = id(1_508, ValueId::new);
        let target_parameter = id(1_509, ValueId::new);
        let target_result = id(1_510, ValueId::new);
        let descendant_result = id(1_511, ValueId::new);
        let predecessor_value = id(1_520, ValueId::new);

        let entry_operation = AbstractOperation::Conditional {
            condition,
            when_true: AbstractSuccessor {
                psi_edge: id(1_512, EdgeId::new),
                target: predecessor,
                bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
            when_false: AbstractSuccessor {
                psi_edge: id(1_513, EdgeId::new),
                target: sibling,
                bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        };
        let descendant_operations = vec![
            AbstractOperation::BooleanEqual {
                psi_operation: id(1_514, OperationId::new),
                result: descendant_result,
                left: target_parameter,
                right: target_result,
            },
            AbstractOperation::Return {
                psi_edge: id(1_515, EdgeId::new),
                result: descendant_result,
                value: descendant_result,
                scalar_type: ScalarType::Boolean,
                cleanup_actions: Vec::new(),
            },
        ];
        let target_operations = vec![
            AbstractOperation::BooleanNot {
                psi_operation: id(1_516, OperationId::new),
                result: target_result,
                operand: target_parameter,
            },
            AbstractOperation::Jump {
                psi_edge: id(1_517, EdgeId::new),
                target: descendant,
                bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        ];
        let sibling_operation = AbstractOperation::Return {
            psi_edge: id(1_518, EdgeId::new),
            result: descendant_result,
            value: incoming,
            scalar_type: ScalarType::Boolean,
            cleanup_actions: Vec::new(),
        };
        let predecessor_operations = vec![
            AbstractOperation::BooleanNot {
                psi_operation: id(1_521, OperationId::new),
                result: predecessor_value,
                operand: incoming,
            },
            AbstractOperation::Jump {
                psi_edge: id(1_519, EdgeId::new),
                target,
                bindings: vec![ValueBinding {
                    parameter: target_parameter,
                    argument: predecessor_value,
                    scalar_type: ScalarType::Boolean,
                }],
                trivial_affine_discards: Vec::new(),
            },
        ];

        let mut block_entries = Vec::new();
        let mut operations = Vec::new();
        let mut push_block = |block, parameters, block_operations: Vec<_>| {
            block_entries.push(AbstractBlockEntry {
                block,
                parameters,
                operation_offset: operations.len(),
            });
            operations.extend(block_operations);
        };
        push_block(entry, Vec::new(), vec![entry_operation]);
        if target_before_predecessor {
            push_block(descendant, Vec::new(), descendant_operations);
            push_block(
                target,
                vec![AbstractParameter {
                    value: target_parameter,
                    scalar_type: ScalarType::Boolean,
                }],
                target_operations,
            );
            push_block(sibling, Vec::new(), vec![sibling_operation]);
            push_block(predecessor, Vec::new(), predecessor_operations);
        } else {
            push_block(predecessor, Vec::new(), predecessor_operations);
            push_block(sibling, Vec::new(), vec![sibling_operation]);
            push_block(
                target,
                vec![AbstractParameter {
                    value: target_parameter,
                    scalar_type: ScalarType::Boolean,
                }],
                target_operations,
            );
            push_block(descendant, Vec::new(), descendant_operations);
        }

        reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([44; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![AbstractFunction {
                    machine,
                    attachment: None,
                    entry,
                    parameters: vec![
                        AbstractParameter {
                            value: condition,
                            scalar_type: ScalarType::Boolean,
                        },
                        AbstractParameter {
                            value: incoming,
                            scalar_type: ScalarType::Boolean,
                        },
                    ],
                    structural_parameters: Vec::new(),
                    result: AbstractFunctionResult::Scalar(AbstractResult {
                        value: descendant_result,
                        scalar_type: ScalarType::Boolean,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries,
                    operations,
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    pub(crate) fn constant_conditional_same_target_unit(constant: bool) -> PsiOptimizationUnit {
        let machine = id(651, MachineId::new);
        let entry = id(652, BlockId::new);
        let merge = id(653, BlockId::new);
        let condition = id(654, ValueId::new);
        reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([23; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![AbstractFunction {
                    machine,
                    attachment: None,
                    entry,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: AbstractFunctionResult::Unit,
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![
                        AbstractBlockEntry {
                            block: entry,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        },
                        AbstractBlockEntry {
                            block: merge,
                            parameters: Vec::new(),
                            operation_offset: 2,
                        },
                    ],
                    operations: vec![
                        AbstractOperation::BooleanConstant {
                            psi_operation: id(655, OperationId::new),
                            result: condition,
                            value: constant,
                        },
                        AbstractOperation::Conditional {
                            condition,
                            when_true: AbstractSuccessor {
                                psi_edge: id(656, EdgeId::new),
                                target: merge,
                                bindings: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: AbstractSuccessor {
                                psi_edge: id(657, EdgeId::new),
                                target: merge,
                                bindings: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                        AbstractOperation::ReturnUnit {
                            psi_edge: id(658, EdgeId::new),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    pub(crate) fn redundant_block_parameter_unit(redundant: bool) -> PsiOptimizationUnit {
        let machine = id(701, MachineId::new);
        let entry = id(702, BlockId::new);
        let merge = id(703, BlockId::new);
        let condition = id(704, ValueId::new);
        let shared = id(705, ValueId::new);
        let alternate = id(706, ValueId::new);
        let parameter = id(707, ValueId::new);
        let result = id(708, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let scalar_type = ScalarType::Integer(integer);
        let binding = |argument| ValueBinding {
            parameter,
            argument,
            scalar_type,
        };
        reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([22; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![AbstractFunction {
                    machine,
                    attachment: None,
                    entry,
                    parameters: vec![
                        AbstractParameter {
                            value: condition,
                            scalar_type: ScalarType::Boolean,
                        },
                        AbstractParameter {
                            value: shared,
                            scalar_type,
                        },
                        AbstractParameter {
                            value: alternate,
                            scalar_type,
                        },
                    ],
                    structural_parameters: Vec::new(),
                    result: AbstractFunctionResult::Scalar(AbstractResult {
                        value: result,
                        scalar_type,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![
                        AbstractBlockEntry {
                            block: entry,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        },
                        AbstractBlockEntry {
                            block: merge,
                            parameters: vec![AbstractParameter {
                                value: parameter,
                                scalar_type,
                            }],
                            operation_offset: 1,
                        },
                    ],
                    operations: vec![
                        AbstractOperation::Conditional {
                            condition,
                            when_true: AbstractSuccessor {
                                psi_edge: id(709, EdgeId::new),
                                target: merge,
                                bindings: vec![binding(shared)],
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: AbstractSuccessor {
                                psi_edge: id(710, EdgeId::new),
                                target: merge,
                                bindings: vec![binding(if redundant { shared } else { alternate })],
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                        AbstractOperation::ExactIntegerAdd {
                            psi_operation: id(711, OperationId::new),
                            obligation: id(713, ObligationId::new),
                            result,
                            scalar_type: integer,
                            left: parameter,
                            right: alternate,
                        },
                        AbstractOperation::Return {
                            psi_edge: id(712, EdgeId::new),
                            result,
                            value: result,
                            scalar_type,
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    fn policy_add_unit(saturating: bool) -> PsiOptimizationUnit {
        let mut unit = exact_add_unit();
        let function = &mut unit.functions[0];
        let block = &mut function.blocks[0];
        let O::IntegerConstant { value, .. } = &mut block.nodes[0].operation else {
            unreachable!()
        };
        *value = IntegerValue::Unsigned(250);
        let O::IntegerConstant { value, .. } = &mut block.nodes[1].operation else {
            unreachable!()
        };
        *value = IntegerValue::Unsigned(10);
        let O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } = block.nodes[2].operation
        else {
            unreachable!()
        };
        block.nodes[2].operation = if saturating {
            O::SaturatingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            }
        } else {
            O::WrappingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            }
        };
        let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[0] else {
            unreachable!()
        };
        *constant = IntegerValue::Unsigned(250);
        let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[1] else {
            unreachable!()
        };
        *constant = IntegerValue::Unsigned(10);
        function.facts.truncate(2);
        unit.identity = recompute_psi_optimization_unit_identity(&unit);
        unit
    }

    pub(crate) fn wrapping_add_unit() -> PsiOptimizationUnit {
        policy_add_unit(false)
    }

    #[derive(Clone, Copy)]
    enum BitwiseFixtureKind {
        And,
        Or,
        Xor,
    }

    fn bitwise_unit(kind: BitwiseFixtureKind) -> PsiOptimizationUnit {
        let mut unit = exact_add_unit();
        let function = &mut unit.functions[0];
        let block = &mut function.blocks[0];
        let O::IntegerConstant { value, .. } = &mut block.nodes[0].operation else {
            unreachable!()
        };
        *value = IntegerValue::Unsigned(0b1010);
        let O::IntegerConstant { value, .. } = &mut block.nodes[1].operation else {
            unreachable!()
        };
        *value = IntegerValue::Unsigned(0b1100);
        let O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } = block.nodes[2].operation
        else {
            unreachable!()
        };
        block.nodes[2].operation = match kind {
            BitwiseFixtureKind::And => O::IntegerBitwiseAnd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            BitwiseFixtureKind::Or => O::IntegerBitwiseOr {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            BitwiseFixtureKind::Xor => O::IntegerBitwiseXor {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
        };
        let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[0] else {
            unreachable!()
        };
        *constant = IntegerValue::Unsigned(0b1010);
        let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[1] else {
            unreachable!()
        };
        *constant = IntegerValue::Unsigned(0b1100);
        function.facts.truncate(2);
        unit.identity = recompute_psi_optimization_unit_identity(&unit);
        unit
    }

    #[derive(Clone, Copy)]
    enum ShiftFixtureKind {
        ExactLeft,
        ExactRight,
        WrappingLeft,
        WrappingRight,
    }

    fn shift_unit(kind: ShiftFixtureKind, value: u128, count: u128) -> PsiOptimizationUnit {
        let mut unit = exact_add_unit();
        let function = &mut unit.functions[0];
        let block = &mut function.blocks[0];
        let O::IntegerConstant {
            value: left_value, ..
        } = &mut block.nodes[0].operation
        else {
            unreachable!()
        };
        *left_value = IntegerValue::Unsigned(value);
        let O::IntegerConstant {
            value: right_value, ..
        } = &mut block.nodes[1].operation
        else {
            unreachable!()
        };
        *right_value = IntegerValue::Unsigned(count);
        let O::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } = block.nodes[2].operation
        else {
            unreachable!()
        };
        block.nodes[2].operation = match kind {
            ShiftFixtureKind::ExactLeft => O::ExactIntegerShiftLeft {
                psi_operation,
                obligation,
                result,
                value_type: scalar_type,
                count_type: scalar_type,
                value: left,
                count: right,
            },
            ShiftFixtureKind::ExactRight => O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                result,
                value_type: scalar_type,
                count_type: scalar_type,
                value: left,
                count: right,
            },
            ShiftFixtureKind::WrappingLeft => O::WrappingIntegerShiftLeft {
                psi_operation,
                result,
                value_type: scalar_type,
                count_type: scalar_type,
                value: left,
                count: right,
            },
            ShiftFixtureKind::WrappingRight => O::WrappingIntegerShiftRight {
                psi_operation,
                result,
                value_type: scalar_type,
                count_type: scalar_type,
                value: left,
                count: right,
            },
        };
        let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[0] else {
            unreachable!()
        };
        *constant = IntegerValue::Unsigned(value);
        let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[1] else {
            unreachable!()
        };
        *constant = IntegerValue::Unsigned(count);
        if matches!(
            kind,
            ShiftFixtureKind::WrappingLeft | ShiftFixtureKind::WrappingRight
        ) {
            function.facts.truncate(2);
        }
        unit.identity = recompute_psi_optimization_unit_identity(&unit);
        unit
    }

    fn exact_divide_unit(zero_divisor: bool) -> PsiOptimizationUnit {
        let mut unit = exact_add_unit();
        let function = &mut unit.functions[0];
        let block = &mut function.blocks[0];
        let O::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } = block.nodes[2].operation
        else {
            unreachable!()
        };
        block.nodes[2].operation = O::ExactIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        };
        if zero_divisor {
            let O::IntegerConstant { value, .. } = &mut block.nodes[1].operation else {
                unreachable!()
            };
            *value = IntegerValue::Unsigned(0);
            let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[1] else {
                unreachable!()
            };
            *constant = IntegerValue::Unsigned(0);
        }
        unit.identity = recompute_psi_optimization_unit_identity(&unit);
        unit
    }

    fn exact_cast_unit(value: u128) -> PsiOptimizationUnit {
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

    fn goal_free_unary_unit(widen: bool) -> PsiOptimizationUnit {
        let machine = id(331, MachineId::new);
        let block = id(332, BlockId::new);
        let operand = id(333, ValueId::new);
        let result = id(334, ValueId::new);
        let source_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let target_type = if widen {
            IntegerType::new(IntegerSign::Unsigned, 16).unwrap()
        } else {
            source_type
        };
        let unary = if widen {
            AbstractOperation::IntegerWiden {
                psi_operation: id(336, OperationId::new),
                result,
                source_type,
                target_type,
                operand,
            }
        } else {
            AbstractOperation::IntegerBitwiseNot {
                psi_operation: id(336, OperationId::new),
                result,
                scalar_type: source_type,
                operand,
            }
        };
        reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([15; 32]),
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
                            psi_operation: id(335, OperationId::new),
                            result: operand,
                            scalar_type: ScalarType::Integer(source_type),
                            value: IntegerValue::Unsigned(15),
                        },
                        unary,
                        AbstractOperation::Return {
                            psi_edge: id(337, EdgeId::new),
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
        .unwrap()
    }

    pub(crate) fn boolean_unit(equal: bool) -> PsiOptimizationUnit {
        let machine = id(341, MachineId::new);
        let block = id(342, BlockId::new);
        let left = id(343, ValueId::new);
        let right = id(344, ValueId::new);
        let result = id(345, ValueId::new);
        let operation = if equal {
            AbstractOperation::BooleanEqual {
                psi_operation: id(348, OperationId::new),
                result,
                left,
                right,
            }
        } else {
            AbstractOperation::BooleanNot {
                psi_operation: id(348, OperationId::new),
                result,
                operand: left,
            }
        };
        reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([16; 32]),
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
                        AbstractOperation::BooleanConstant {
                            psi_operation: id(346, OperationId::new),
                            result: left,
                            value: true,
                        },
                        AbstractOperation::BooleanConstant {
                            psi_operation: id(347, OperationId::new),
                            result: right,
                            value: false,
                        },
                        operation,
                        AbstractOperation::Return {
                            psi_edge: id(349, EdgeId::new),
                            result,
                            value: result,
                            scalar_type: ScalarType::Boolean,
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    #[derive(Clone, Copy)]
    enum ComparisonFixtureKind {
        Equal,
        LessThan,
        LessOrEqual,
    }

    fn integer_comparison_unit(kind: ComparisonFixtureKind) -> PsiOptimizationUnit {
        let machine = id(351, MachineId::new);
        let block = id(352, BlockId::new);
        let left = id(353, ValueId::new);
        let right = id(354, ValueId::new);
        let result = id(355, ValueId::new);
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let operation = match kind {
            ComparisonFixtureKind::Equal => AbstractOperation::IntegerEqual {
                psi_operation: id(358, OperationId::new),
                result,
                left,
                right,
            },
            ComparisonFixtureKind::LessThan => AbstractOperation::IntegerLessThan {
                psi_operation: id(358, OperationId::new),
                result,
                left,
                right,
            },
            ComparisonFixtureKind::LessOrEqual => AbstractOperation::IntegerLessOrEqual {
                psi_operation: id(358, OperationId::new),
                result,
                left,
                right,
            },
        };
        reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([17; 32]),
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
                        AbstractOperation::IntegerConstant {
                            psi_operation: id(356, OperationId::new),
                            result: left,
                            scalar_type: ScalarType::Integer(scalar_type),
                            value: IntegerValue::Unsigned(7),
                        },
                        AbstractOperation::IntegerConstant {
                            psi_operation: id(357, OperationId::new),
                            result: right,
                            scalar_type: ScalarType::Integer(scalar_type),
                            value: IntegerValue::Unsigned(8),
                        },
                        operation,
                        AbstractOperation::Return {
                            psi_edge: id(359, EdgeId::new),
                            result,
                            value: result,
                            scalar_type: ScalarType::Boolean,
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn selected_builtin_proposes_one_independently_validated_exact_fold() {
        let unit = exact_add_unit();
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let ranges = compute_analysis(&unit, AnalysisKind::ValueRanges).unwrap();
        let products = vec![constants, ranges];
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        assert_eq!(registry.len(), 36);
        let mut dispatched = 0usize;
        let mut candidates = Vec::new();
        for rule in registry.iter() {
            dispatched += 1;
            candidates.extend(
                rule.propose(&unit, RuleAnalysisView::new(&products))
                    .unwrap(),
            );
        }
        assert_eq!(dispatched, registry.len());
        assert_eq!(candidates.len(), 1);
        let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[2].operation,
            AbstractOperation::IntegerConstant {
                value: IntegerValue::Unsigned(15),
                ..
            }
        ));
    }

    #[test]
    fn wrapping_and_saturating_rules_use_their_exact_declared_policies() {
        for (unit, saturating) in [(wrapping_add_unit(), false), (policy_add_unit(true), true)] {
            let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
            let products = vec![constants];
            let candidates = if saturating {
                SaturatingIntegerAddConstantsRule
                    .propose(&unit, RuleAnalysisView::new(&products))
                    .unwrap()
            } else {
                WrappingIntegerAddConstantsRule
                    .propose(&unit, RuleAnalysisView::new(&products))
                    .unwrap()
            };
            assert_eq!(candidates.len(), 1);
            assert_eq!(
                candidates[0].safety_class(),
                OptimizationSafetyClass::ExactOperationSemantics
            );
            let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
            let expected = if saturating { 255 } else { 4 };
            assert!(matches!(
                accepted.unit().functions[0].blocks[0].nodes[2].operation,
                AbstractOperation::IntegerConstant {
                    value: IntegerValue::Unsigned(value),
                    ..
                } if value == expected
            ));
        }
    }

    #[test]
    fn binary_bitwise_rules_fold_with_typed_psi_semantics() {
        let cases: [(BitwiseFixtureKind, &dyn PsiOptimizationRule, u128); 3] = [
            (BitwiseFixtureKind::And, &IntegerBitwiseAndConstantsRule, 8),
            (BitwiseFixtureKind::Or, &IntegerBitwiseOrConstantsRule, 14),
            (BitwiseFixtureKind::Xor, &IntegerBitwiseXorConstantsRule, 6),
        ];
        for (kind, rule, expected) in cases {
            let unit = bitwise_unit(kind);
            let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
            let candidates = rule
                .propose(&unit, RuleAnalysisView::new(&[constants]))
                .unwrap();
            assert_eq!(candidates.len(), 1);
            assert_eq!(
                candidates[0].safety_class(),
                OptimizationSafetyClass::ExactOperationSemantics
            );
            assert!(matches!(
                candidates[0].scalar_evaluation_witness().unwrap(),
                IntegerEvaluationWitness::Binary { .. }
            ));
            let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
            assert!(matches!(
                accepted.unit().functions[0].blocks[0].nodes[2].operation,
                AbstractOperation::IntegerConstant {
                    value: IntegerValue::Unsigned(value),
                    ..
                } if value == expected
            ));
        }
    }

    #[test]
    fn propagated_block_parameter_fact_is_independently_reconstructed() {
        let unit = propagated_block_parameter_unit(true);
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let candidates = IntegerBitwiseNotConstantsRule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap();
        assert_eq!(candidates.len(), 1);
        let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
        assert!(matches!(
            accepted.unit().functions[0].blocks[3].nodes[0].operation,
            AbstractOperation::IntegerConstant {
                value: IntegerValue::Unsigned(248),
                ..
            }
        ));
    }

    #[test]
    fn proof_bearing_division_folds_only_when_the_declared_operation_is_defined() {
        let unit = exact_divide_unit(false);
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let candidates = ExactIntegerDivideConstantsRule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap();
        assert_eq!(candidates.len(), 1);
        let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[2].operation,
            AbstractOperation::IntegerConstant {
                value: IntegerValue::Unsigned(0),
                ..
            }
        ));

        let zero = exact_divide_unit(true);
        let constants = compute_analysis(&zero, AnalysisKind::ScalarConstants).unwrap();
        assert!(
            ExactIntegerDivideConstantsRule
                .propose(&zero, RuleAnalysisView::new(&[constants]))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn exact_and_wrapping_shift_rules_use_psi_integer_semantics() {
        let cases: [(ShiftFixtureKind, &dyn PsiOptimizationRule, u128, u128, u128); 4] = [
            (
                ShiftFixtureKind::ExactLeft,
                &ExactIntegerShiftLeftConstantsRule,
                7,
                2,
                28,
            ),
            (
                ShiftFixtureKind::ExactRight,
                &ExactIntegerShiftRightConstantsRule,
                7,
                2,
                1,
            ),
            (
                ShiftFixtureKind::WrappingLeft,
                &WrappingIntegerShiftLeftConstantsRule,
                250,
                2,
                232,
            ),
            (
                ShiftFixtureKind::WrappingRight,
                &WrappingIntegerShiftRightConstantsRule,
                250,
                2,
                62,
            ),
        ];
        for (kind, rule, value, count, expected) in cases {
            let unit = shift_unit(kind, value, count);
            let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
            let candidates = rule
                .propose(&unit, RuleAnalysisView::new(&[constants]))
                .unwrap();
            assert_eq!(candidates.len(), 1);
            let expected_safety = if matches!(
                kind,
                ShiftFixtureKind::ExactLeft | ShiftFixtureKind::ExactRight
            ) {
                OptimizationSafetyClass::ProofCertified
            } else {
                OptimizationSafetyClass::ExactOperationSemantics
            };
            assert_eq!(candidates[0].safety_class(), expected_safety);
            let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
            assert!(matches!(
                accepted.unit().functions[0].blocks[0].nodes[2].operation,
                AbstractOperation::IntegerConstant {
                    value: IntegerValue::Unsigned(value),
                    ..
                } if value == expected
            ));
        }
    }

    #[test]
    fn exact_shift_left_declines_an_overflowing_constant_evaluation() {
        let unit = shift_unit(ShiftFixtureKind::ExactLeft, 250, 2);
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        assert!(
            ExactIntegerShiftLeftConstantsRule
                .propose(&unit, RuleAnalysisView::new(&[constants]))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn exact_cast_rule_uses_unary_evidence_and_target_integer_semantics() {
        let unit = exact_cast_unit(250);
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let candidates = ExactIntegerCastConstantsRule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].safety_class(),
            OptimizationSafetyClass::ProofCertified
        );
        assert!(matches!(
            candidates[0].scalar_evaluation_witness().unwrap(),
            IntegerEvaluationWitness::ProofCertifiedUnary { .. }
        ));
        let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
        let target_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[1].operation,
            AbstractOperation::IntegerConstant {
                scalar_type: ScalarType::Integer(scalar_type),
                value: IntegerValue::Unsigned(250),
                ..
            } if scalar_type == target_type
        ));

        let IntegerEvaluationWitness::ProofCertifiedUnary {
            operand_fact,
            obligation_fact,
        } = candidates[0].scalar_evaluation_witness().unwrap()
        else {
            unreachable!()
        };
        let omega_optimization_unit::PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) =
            candidates[0].patch()
        else {
            unreachable!()
        };
        let binary_witness = PsiRewriteCandidate::new_integer_evaluation(
            unit.identity,
            ExactIntegerCastConstantsRule::contract(),
            vec![unit.functions[0].blocks[0].id],
            Vec::new(),
            candidates[0].provenance().to_vec(),
            IntegerEvaluationWitness::ProofCertifiedBinary {
                left_fact: operand_fact,
                right_fact: operand_fact,
                obligation_fact,
            },
            -1,
            patch,
        )
        .unwrap();
        assert_eq!(binary_witness.consumed_facts().len(), 2);
        assert_ne!(binary_witness.identity(), candidates[0].identity());
        assert!(matches!(
            validate_integer_evaluation_candidate(&unit, &binary_witness),
            Err(omega_optimization_validation::OptimizationUnitValidationError::CandidateOperandFactMismatch)
        ));
    }

    #[test]
    fn exact_cast_rule_declines_a_constant_outside_the_target_domain() {
        let unit = exact_cast_unit(300);
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        assert!(
            ExactIntegerCastConstantsRule
                .propose(&unit, RuleAnalysisView::new(&[constants]))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn widen_and_bitwise_not_rules_reuse_typed_unary_evidence() {
        let cases: [(bool, &dyn PsiOptimizationRule, u128, u16); 2] = [
            (true, &IntegerWidenConstantsRule, 15, 16),
            (false, &IntegerBitwiseNotConstantsRule, 240, 8),
        ];
        for (widen, rule, expected, expected_bits) in cases {
            let unit = goal_free_unary_unit(widen);
            let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
            let candidates = rule
                .propose(&unit, RuleAnalysisView::new(&[constants]))
                .unwrap();
            assert_eq!(candidates.len(), 1);
            assert_eq!(
                candidates[0].safety_class(),
                OptimizationSafetyClass::ExactOperationSemantics
            );
            assert!(matches!(
                candidates[0].scalar_evaluation_witness().unwrap(),
                IntegerEvaluationWitness::Unary { .. }
            ));
            let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
            assert!(matches!(
                accepted.unit().functions[0].blocks[0].nodes[1].operation,
                AbstractOperation::IntegerConstant {
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(value),
                    ..
                } if value == expected && scalar_type.bits() == expected_bits
            ));
        }
    }

    #[test]
    fn boolean_not_and_equal_use_typed_boolean_patches() {
        let cases: [(bool, &dyn PsiOptimizationRule); 2] = [
            (false, &BooleanNotConstantsRule),
            (true, &BooleanEqualConstantsRule),
        ];
        for (equal, rule) in cases {
            let unit = boolean_unit(equal);
            let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
            let candidates = rule
                .propose(&unit, RuleAnalysisView::new(&[constants]))
                .unwrap();
            assert_eq!(candidates.len(), 1);
            assert!(matches!(
                candidates[0].patch(),
                omega_optimization_unit::PsiRewritePatch::ReplaceBooleanOperationWithConstant(_)
            ));
            let accepted = validate_boolean_evaluation_candidate(&unit, &candidates[0]).unwrap();
            assert!(matches!(
                accepted.unit().functions[0].blocks[0].nodes[2].operation,
                AbstractOperation::BooleanConstant { value: false, .. }
            ));
        }
    }

    #[test]
    fn integer_comparison_rules_reconstruct_operand_types_and_boolean_results() {
        let cases: [(ComparisonFixtureKind, &dyn PsiOptimizationRule, bool); 3] = [
            (
                ComparisonFixtureKind::Equal,
                &IntegerEqualConstantsRule,
                false,
            ),
            (
                ComparisonFixtureKind::LessThan,
                &IntegerLessThanConstantsRule,
                true,
            ),
            (
                ComparisonFixtureKind::LessOrEqual,
                &IntegerLessOrEqualConstantsRule,
                true,
            ),
        ];
        for (kind, rule, expected) in cases {
            let unit = integer_comparison_unit(kind);
            let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
            let candidates = rule
                .propose(&unit, RuleAnalysisView::new(&[constants]))
                .unwrap();
            assert_eq!(candidates.len(), 1);
            let accepted = validate_boolean_evaluation_candidate(&unit, &candidates[0]).unwrap();
            assert!(matches!(
                accepted.unit().functions[0].blocks[0].nodes[2].operation,
                AbstractOperation::BooleanConstant { value, .. } if value == expected
            ));
        }
    }

    #[test]
    fn integer_range_equality_proves_singleton_outside_and_declines_overlap() {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        for kind in [
            IntegerRangeComparisonKind::RangeEqualConstant,
            IntegerRangeComparisonKind::ConstantEqualRange,
        ] {
            assert_eq!(
                evaluate_integer_range_comparison(
                    kind,
                    scalar_type,
                    IntegerValue::Unsigned(7),
                    IntegerValue::Unsigned(7),
                    IntegerValue::Unsigned(7),
                ),
                Some(true)
            );
            assert_eq!(
                evaluate_integer_range_comparison(
                    kind,
                    scalar_type,
                    IntegerValue::Unsigned(7),
                    IntegerValue::Unsigned(9),
                    IntegerValue::Unsigned(6),
                ),
                Some(false)
            );
            assert_eq!(
                evaluate_integer_range_comparison(
                    kind,
                    scalar_type,
                    IntegerValue::Unsigned(7),
                    IntegerValue::Unsigned(9),
                    IntegerValue::Unsigned(10),
                ),
                Some(false)
            );
            assert_eq!(
                evaluate_integer_range_comparison(
                    kind,
                    scalar_type,
                    IntegerValue::Unsigned(7),
                    IntegerValue::Unsigned(9),
                    IntegerValue::Unsigned(8),
                ),
                None
            );
        }
    }

    #[test]
    fn sccp_registry_appends_both_integer_range_equality_orientations() {
        let registry =
            registry_for_optimization(Optimization::SparseConditionalConstantPropagation).unwrap();
        let contracts = registry.contracts().collect::<Vec<_>>();
        assert_eq!(contracts.len(), 36);
        assert_eq!(
            contracts[34].identity(),
            IntegerEqualRangeConstantRule::contract().identity()
        );
        assert_eq!(
            contracts[35].identity(),
            IntegerEqualConstantRangeRule::contract().identity()
        );
        assert!(contracts.iter().all(|contract| {
            contract.pass() == OptimizationPassIdentity::from_canonical_bytes(SCCP_PASS_NAME)
        }));
    }

    #[test]
    fn built_in_schedule_is_independent_of_registration_arrival_order() {
        for optimization in [
            Optimization::SparseConditionalConstantPropagation,
            Optimization::ControlFlowCleanup,
            Optimization::GlobalValueNumbering,
            Optimization::DeadPureScalarElimination,
        ] {
            let expected = registry_for_optimization(optimization).unwrap();
            let expected_contracts = expected.contracts().collect::<Vec<_>>();

            for registry in randomized_built_in_registries(optimization) {
                assert_eq!(registry.identity(), expected.identity());
                assert_eq!(registry.contracts().collect::<Vec<_>>(), expected_contracts);
            }
        }
    }

    #[test]
    fn absent_selection_registers_nothing_and_missing_analysis_fails_closed() {
        let unit = exact_add_unit();
        assert!(
            built_in_psi_registry(&OptimizationSelections::default())
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            ExactIntegerAddConstantsRule.propose(&unit, RuleAnalysisView::new(&[])),
            Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ScalarConstants
            ))
        );
        let cleanup = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
        assert_eq!(built_in_psi_registry(&cleanup).unwrap().len(), 7);
        let copy = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
        assert_eq!(built_in_psi_registry(&copy).unwrap().len(), 1);
        let gvn = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
        let gvn = built_in_psi_registry(&gvn).unwrap();
        assert_eq!(gvn.len(), 9);
        assert_eq!(
            gvn.contracts()
                .map(|contract| contract.identity())
                .collect::<Vec<_>>(),
            [
                SameBlockTotalScalarCseRule::contract().identity(),
                SameBlockProofCertifiedScalarCseRule::contract().identity(),
                DominatorTotalScalarGvnRule::contract().identity(),
                DominatorProofCertifiedScalarGvnRule::contract().identity(),
                PhiTranslatedObligationFreeScalarGvnRule::contract().identity(),
                PhiTranslatedProofCertifiedScalarGvnRule::contract().identity(),
                SameBlockProofCertifiedCompatiblePolicyScalarCseRule::contract().identity(),
                DominatorProofCertifiedCompatiblePolicyScalarGvnRule::contract().identity(),
                PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract().identity(),
            ]
        );
        assert!(gvn.contracts().all(|contract| {
            contract.pass()
                == OptimizationPassIdentity::from_canonical_bytes(GLOBAL_VALUE_NUMBERING_PASS_NAME)
        }));
        let dead = OptimizationSelections::new([Optimization::DeadPureScalarElimination]).unwrap();
        assert_eq!(built_in_psi_registry(&dead).unwrap().len(), 2);
        let proof = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
        let proof = built_in_psi_registry(&proof).unwrap();
        assert_eq!(proof.len(), 11);
        assert_eq!(
            proof
                .contracts()
                .map(|contract| contract.identity())
                .collect::<Vec<_>>(),
            [
                ProofCertifiedDeadScalarEliminationRule::contract().identity(),
                LiveProofCertifiedIntegerIdentityEliminationRule::contract().identity(),
                LiveProofCertifiedIntegerDivideByOneEliminationRule::contract().identity(),
                LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule::contract().identity(),
                LiveProofCertifiedIntegerZeroDividendEliminationRule::contract().identity(),
                LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule::contract().identity(),
                LiveProofCertifiedExactIntegerSelfSubtractEliminationRule::contract().identity(),
                LiveProofCertifiedIntegerSelfRemainderEliminationRule::contract().identity(),
                LiveProofCertifiedIntegerSelfDivideEliminationRule::contract().identity(),
                LiveProofCertifiedIntegerRemainderByOneEliminationRule::contract().identity(),
                LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule::contract()
                    .identity(),
            ]
        );
        let unsupported_combination = OptimizationSelections::new([
            Optimization::SparseConditionalConstantPropagation,
            Optimization::CopyPropagation,
        ])
        .unwrap();
        assert!(matches!(
            built_in_psi_registry(&unsupported_combination),
            Err(RuleRegistryError::UnsupportedOptimizationCombination)
        ));

        let lower_only =
            OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate])
                .unwrap();
        assert!(built_in_psi_registry(&lower_only).unwrap().is_empty());
        assert!(built_in_psi_registries(&lower_only).unwrap().is_empty());

        let sccp =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let mixed = OptimizationSelections::new([
            Optimization::SparseConditionalConstantPropagation,
            Optimization::SelectedIncomingU12ExactAddImmediate,
        ])
        .unwrap();
        let sccp_registries = built_in_psi_registries(&sccp).unwrap();
        let mixed_registries = built_in_psi_registries(&mixed).unwrap();
        assert_eq!(mixed_registries.len(), 1);
        assert_eq!(
            mixed_registries[0].identity(),
            sccp_registries[0].identity()
        );
        assert_eq!(
            mixed_registries[0].contracts().collect::<Vec<_>>(),
            sccp_registries[0].contracts().collect::<Vec<_>>()
        );
    }

    #[test]
    fn unreachable_private_machine_pruning_is_atomic_canonical_and_idempotent() {
        let mut unit = linear_empty_block_unit();
        let mut private = unit.functions[0].clone();
        private.machine = MachineId::new(99).unwrap();
        unit.functions.push(private);
        unit.identity = recompute_psi_optimization_unit_identity(&unit);
        validate_psi_optimization_unit(&unit).unwrap();

        let call_graph = compute_analysis(&unit, AnalysisKind::CallGraph).unwrap();
        let candidates = UnreachablePrivateMachinePruneRule
            .propose(&unit, RuleAnalysisView::new(&[call_graph]))
            .unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].affected_machines(),
            [MachineId::new(99).unwrap()]
        );
        assert!(candidates[0].provenance().iter().all(|row| {
            row.input.machine() == MachineId::new(99).unwrap()
                && row.disposition == ProvenanceDisposition::ProvenUnreachableAt(row.input)
        }));

        let accepted =
            validate_unreachable_private_machines_candidate(&unit, &candidates[0]).unwrap();
        assert_eq!(accepted.unit().functions.len(), 1);
        assert_eq!(
            accepted.unit().pruned_machines,
            [PrunedMachineCustody {
                machine: MachineId::new(99).unwrap(),
                source_ordinal: 1,
            }]
        );
        assert_eq!(
            accepted.unit().accepted_obligation_facts,
            unit.accepted_obligation_facts
        );
        assert_eq!(
            accepted.unit().ownership_frontier_facts,
            unit.ownership_frontier_facts
        );

        let call_graph = compute_analysis(accepted.unit(), AnalysisKind::CallGraph).unwrap();
        assert!(
            UnreachablePrivateMachinePruneRule
                .propose(accepted.unit(), RuleAnalysisView::new(&[call_graph]))
                .unwrap()
                .is_empty()
        );

        let PsiRewritePatch::PruneUnreachablePrivateMachines(patch) = candidates[0].patch() else {
            unreachable!("pruning rule emits its typed patch")
        };
        let mut incomplete = candidates[0].provenance().to_vec();
        incomplete.pop();
        let forged = PsiRewriteCandidate::new_unreachable_private_machines(
            unit.identity,
            UnreachablePrivateMachinePruneRule::contract(),
            incomplete,
            -1,
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_unreachable_private_machines_candidate(&unit, &forged),
            Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
        );
    }

    #[test]
    fn private_machine_roots_include_calls_attachments_cleanup_and_prune_recursive_islands() {
        let mut unit = linear_empty_block_unit();
        let template = unit.functions[0].clone();
        for machine in [99, 100, 101, 102, 103, 104] {
            let mut function = template.clone();
            function.machine = MachineId::new(machine).unwrap();
            unit.functions.push(function);
        }
        unit.functions[0].blocks[0].nodes[0].operation = O::CallUnit {
            psi_operation: OperationId::new(9_001).unwrap(),
            callee: MachineId::new(99).unwrap(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
        };
        unit.functions[2].attachment = Some(StructuralTypeId::new(9_002).unwrap());
        unit.provider_candidates
            .push(psi_terminal::ProviderCandidateConformance {
                boundary: BoundaryMachineId::new(9_006).unwrap(),
                requirement_identity: "root-test-requirement".into(),
                provider_identity: "root-test-provider".into(),
                candidate_identity: "root-test-candidate".into(),
                candidate: MachineId::new(102).unwrap(),
                signature: psi_terminal::ProviderUnitSignature {
                    parameters: Vec::new(),
                },
                refinement: psi_terminal::ProviderUnitRefinement {
                    positional_parameters: Vec::new(),
                    required_domains: Vec::new(),
                    realized_service_ceiling: Vec::new(),
                },
            });
        unit.functions[1].blocks[0].nodes[0].operation = O::ReturnUnit {
            psi_edge: EdgeId::new(9_003).unwrap(),
            cleanup_actions: vec![psi_terminal::TerminalAffineCleanupAction::InvokeNominal(
                psi_terminal::NominalAffineCleanup {
                    place: PlaceId::new(9_004).unwrap(),
                    structural_type: StructuralTypeId::new(9_005).unwrap(),
                    cleanup_machine: MachineId::new(101).unwrap(),
                    cleanup_receiver: None,
                    requirement_obligations: Vec::new(),
                },
            )],
        };
        unit.functions[5].blocks[0].nodes[0].operation = O::CallUnit {
            psi_operation: OperationId::new(9_007).unwrap(),
            callee: MachineId::new(104).unwrap(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
        };
        unit.functions[6].blocks[0].nodes[0].operation = O::CallUnit {
            psi_operation: OperationId::new(9_008).unwrap(),
            callee: MachineId::new(103).unwrap(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
        };

        let analysis = compute_analysis(&unit, AnalysisKind::CallGraph).unwrap();
        let AnalysisProduct::CallGraph(call_graph) = analysis else {
            unreachable!("requested call graph analysis")
        };
        assert_eq!(
            rule_unreachable_private_machine_complement(&unit, &call_graph),
            [MachineId::new(103).unwrap(), MachineId::new(104).unwrap()]
        );
    }

    #[test]
    fn constant_conditional_fold_binds_selected_edge_fact_and_fuel() {
        for constant in [false, true] {
            let unit = constant_conditional_same_target_unit(constant);
            let contract = ConstantConditionalFoldRule::contract();
            let mut manager = crate::AnalysisManager::new(&unit);
            let products = manager
                .require_all(&unit, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let candidates = ConstantConditionalFoldRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap();
            assert_eq!(candidates.len(), 1);
            assert_eq!(candidates[0].consumed_facts().len(), 1);
            let omega_optimization_unit::PsiRewritePatch::FoldConstantConditional(patch) =
                candidates[0].patch()
            else {
                unreachable!()
            };
            assert_eq!(patch.constant, constant);
            let realized = candidates[0]
                .provenance()
                .iter()
                .find(|row| row.disposition.is_realized())
                .expect("conditional fold carries selected-edge custody");
            let proven_unreachable = candidates[0]
                .provenance()
                .iter()
                .find(|row| !row.disposition.is_realized())
                .expect("conditional fold carries rejected-edge custody");
            let realized_site = PsiRealizationSite::Edge {
                machine: patch.location.machine,
                edge: patch.selected_edge,
            };
            let unreachable_site = PsiRealizationSite::Edge {
                machine: patch.location.machine,
                edge: patch.rejected_edge,
            };
            assert_eq!(
                realized.disposition,
                ProvenanceDisposition::RealizedAt(realized_site)
            );
            assert_eq!(
                realized.sources,
                [omega_optimization_unit::PsiProvenance::Edge(
                    patch.selected_edge
                )]
            );
            assert_eq!(
                proven_unreachable.disposition,
                ProvenanceDisposition::ProvenUnreachableAt(unreachable_site)
            );
            assert_eq!(
                proven_unreachable.sources,
                [omega_optimization_unit::PsiProvenance::Edge(
                    patch.rejected_edge
                )]
            );
            let accepted = validate_constant_conditional_candidate(&unit, &candidates[0]).unwrap();
            assert_eq!(accepted.provenance(), candidates[0].provenance());
            assert_eq!(
                accepted.validator(),
                omega_optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
                    b"omega.validator.constant-conditional-fold.v4"
                )
            );
            let node = &accepted.unit().functions[0].blocks[0].nodes[1];
            assert!(matches!(
                node.operation,
                AbstractOperation::Jump { psi_edge, .. } if psi_edge == patch.selected_edge
            ));
            assert_eq!(
                node.successors[0].provenance,
                [omega_optimization_unit::PsiProvenance::Edge(
                    patch.selected_edge
                )]
            );
            assert!(node.provenance.is_empty());
            assert!(node.fuel.is_empty());
            assert_eq!(node.successors[0].fuel.len(), 1);
            assert_eq!(
                node.successors[0].fuel[0].site,
                omega_optimization_unit::PsiProvenance::Edge(patch.selected_edge)
            );
        }
    }

    #[test]
    fn constant_conditional_fold_atomically_prunes_the_unreachable_branch_region() {
        let unit = propagated_block_parameter_unit(true);
        let contract = ConstantConditionalFoldRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidate = ConstantConditionalFoldRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .pop()
            .expect("constant branch produces an atomic prune candidate");
        assert_eq!(
            candidate.affected_blocks(),
            [
                id(602, BlockId::new),
                id(604, BlockId::new),
                id(605, BlockId::new),
            ]
        );
        assert_eq!(
            candidate
                .provenance()
                .iter()
                .filter(|row| row.disposition.is_realized())
                .count(),
            3
        );
        assert_eq!(
            candidate
                .provenance()
                .iter()
                .filter(|row| !row.disposition.is_realized())
                .count(),
            3
        );
        let accepted = validate_constant_conditional_candidate(&unit, &candidate).unwrap();
        let output = accepted.unit();
        assert_eq!(
            output.functions[0]
                .blocks
                .iter()
                .map(|block| block.id)
                .collect::<Vec<_>>(),
            [
                id(602, BlockId::new),
                id(603, BlockId::new),
                id(605, BlockId::new),
            ]
        );
        assert_eq!(output.functions[0].facts.len(), 2);
        assert_eq!(output.functions[0].blocks[2].nodes[0].effect.input, 4);
        assert_eq!(output.functions[0].blocks[2].nodes[1].effect.output, 6);
        assert_eq!(accepted.provenance(), candidate.provenance());
    }

    #[test]
    fn constant_conditional_fold_atomically_refreshes_current_root_service_reach() {
        let unit = constant_conditional_dead_service_unit();
        validate_psi_optimization_unit(&unit)
            .expect("dead-branch service belongs to the source revision root reach");
        assert_eq!(unit.root_service_reach.concrete, [id(620, ServiceId::new)]);
        let contract = ConstantConditionalFoldRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidate = ConstantConditionalFoldRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .pop()
            .expect("constant branch produces an atomic prune candidate");
        let accepted = validate_constant_conditional_candidate(&unit, &candidate)
            .expect("accepted fold refreshes its derived root reach");
        assert!(accepted.unit().root_service_reach.concrete.is_empty());
        assert!(
            accepted
                .unit()
                .root_service_reach
                .installation_dependencies
                .is_empty()
        );
        validate_psi_optimization_unit(accepted.unit())
            .expect("fold output has exact current-revision root reach");
    }

    #[test]
    fn adjacent_block_merge_substitutes_parameters_and_rehomes_edge_custody() {
        let unit = propagated_block_parameter_unit(true);
        let fold_contract = ConstantConditionalFoldRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, fold_contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let fold = ConstantConditionalFoldRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .pop()
            .unwrap();
        let folded = validate_constant_conditional_candidate(&unit, &fold)
            .unwrap()
            .into_unit();

        let contract = AdjacentBlockMergeRule::contract();
        let mut manager = crate::AnalysisManager::new(&folded);
        let products = manager
            .require_all(&folded, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidates = AdjacentBlockMergeRule
            .propose(&folded, RuleAnalysisView::new(&products))
            .unwrap();
        let candidate = candidates
            .iter()
            .find(|candidate| {
                matches!(
                    candidate.patch(),
                    PsiRewritePatch::MergeAdjacentBlock(patch)
                        if patch.predecessor.block == id(603, BlockId::new)
                            && patch.target == id(605, BlockId::new)
                )
            })
            .expect("selected arm can merge with its unique adjacent target");
        assert_eq!(candidate.substitutions().len(), 1);
        let accepted = validate_adjacent_block_merge_candidate(&folded, candidate).unwrap();
        let output = accepted.unit();
        assert_eq!(output.functions[0].blocks.len(), 2);
        let merged = &output.functions[0].blocks[1];
        assert_eq!(merged.nodes.len(), 3);
        assert!(matches!(
            merged.nodes[1].operation,
            AbstractOperation::IntegerBitwiseNot { operand, .. }
                if operand == id(607, ValueId::new)
        ));
        assert_eq!(
            merged.nodes[1].provenance,
            [
                PsiProvenance::Operation(id(618, OperationId::new)),
                PsiProvenance::Edge(id(615, EdgeId::new)),
            ]
        );

        let PsiRewritePatch::MergeAdjacentBlock(patch) = candidate.patch() else {
            unreachable!()
        };
        let mut corrupted_provenance = candidate.provenance().to_vec();
        let incoming = PsiRealizationSite::Edge {
            machine: patch.predecessor.machine,
            edge: patch.incoming_edge,
        };
        let row = corrupted_provenance
            .iter_mut()
            .find(|row| row.input == incoming)
            .unwrap();
        row.disposition =
            ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(NodeLocation {
                machine: patch.predecessor.machine,
                block: patch.target,
                node: 0,
            }));
        corrupted_provenance.sort_by_key(|row| {
            (
                row.input,
                row.disposition.canonical_tag(),
                row.disposition.site(),
            )
        });
        let corrupted = PsiRewriteCandidate::new_adjacent_block_merge(
            folded.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.substitutions().to_vec(),
            corrupted_provenance,
            candidate.ownership_frontier_witness().unwrap().clone(),
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_adjacent_block_merge_candidate(&folded, &corrupted),
            Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
        );
    }

    #[test]
    fn adjacent_block_merge_fuses_a_direct_terminal_exit_without_erasing_it() {
        let unit = linear_empty_block_unit();
        let contract = AdjacentBlockMergeRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = AdjacentBlockMergeRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .expect("the adjacent return target is the sole eligible merge");
        assert!(candidate.consumed_facts().is_empty());
        let accepted = validate_adjacent_block_merge_candidate(&unit, &candidate).unwrap();
        let output = accepted.unit();
        assert_eq!(output.functions[0].blocks.len(), 2);
        let terminal = &output.functions[0].blocks[1].nodes[0];
        assert!(matches!(terminal.operation, O::ReturnUnit { .. }));
        assert_eq!(
            terminal.provenance,
            [
                PsiProvenance::Edge(id(913, EdgeId::new)),
                PsiProvenance::Edge(id(912, EdgeId::new)),
            ]
        );
        let incoming = PsiRealizationSite::Edge {
            machine: id(901, MachineId::new),
            edge: id(912, EdgeId::new),
        };
        assert!(accepted.provenance().iter().any(|row| {
            row.input == incoming
                && row.disposition
                    == ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(NodeLocation {
                        machine: id(901, MachineId::new),
                        block: id(903, BlockId::new),
                        node: 0,
                    }))
        }));
    }

    #[test]
    fn adjacent_block_merge_carries_exact_ownership_frontier_custody() {
        let mut unit = linear_empty_block_unit();
        let machine = id(901, MachineId::new);
        let incoming = id(912, EdgeId::new);
        let target = id(904, BlockId::new);
        let snapshot = OwnershipFrontierSnapshot {
            claims: Vec::new(),
            owned_places: Vec::new(),
            partial_custody: Vec::new(),
        };
        unit.ownership_frontier_facts = [
            OwnershipFrontierSite::EdgeEntry(id(911, EdgeId::new)),
            OwnershipFrontierSite::EdgeExit(id(911, EdgeId::new)),
            OwnershipFrontierSite::EdgeEntry(incoming),
            OwnershipFrontierSite::EdgeExit(incoming),
            OwnershipFrontierSite::BlockEntry(target),
        ]
        .into_iter()
        .map(|site| OwnershipFrontierFact::new(unit.psi, machine, site, snapshot.clone()))
        .collect();
        unit.ownership_frontier_facts
            .sort_by_key(|fact| (fact.machine, fact.site));
        unit.identity = recompute_psi_optimization_unit_identity(&unit);
        validate_psi_optimization_unit(&unit).unwrap();

        let contract = AdjacentBlockMergeRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidate = AdjacentBlockMergeRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .into_iter()
            .find(|candidate| {
                matches!(
                    candidate.patch(),
                    PsiRewritePatch::MergeAdjacentBlock(patch)
                        if patch.incoming_edge == incoming && patch.target == target
                )
            })
            .expect("ownership-certified adjacent merge is proposed");
        assert_eq!(candidate.consumed_facts().len(), 3);
        assert!(
            candidate
                .consumed_facts()
                .iter()
                .all(|fact| matches!(fact, OptimizationFactReference::OwnershipFrontier(_)))
        );
        validate_adjacent_block_merge_candidate(&unit, &candidate).unwrap();

        let PsiRewritePatch::MergeAdjacentBlock(patch) = candidate.patch() else {
            unreachable!()
        };
        let missing_custody = PsiRewriteCandidate::new_adjacent_block_merge(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.substitutions().to_vec(),
            candidate.provenance().to_vec(),
            OwnershipFrontierWitness { rows: Vec::new() },
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_adjacent_block_merge_candidate(&unit, &missing_custody),
            Err(OptimizationUnitValidationError::CandidateObservationMismatch)
        );

        let mut forged_witness = candidate.ownership_frontier_witness().unwrap().clone();
        forged_witness.rows[0].fact =
            omega_optimization_core::OwnershipFrontierFactIdentity::from_canonical_bytes(
                b"forged-adjacent-merge-ownership-fact",
            );
        let forged_custody = PsiRewriteCandidate::new_adjacent_block_merge(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.substitutions().to_vec(),
            candidate.provenance().to_vec(),
            forged_witness,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_adjacent_block_merge_candidate(&unit, &forged_custody),
            Err(OptimizationUnitValidationError::CandidateObservationMismatch)
        );

        let mut reordered_witness = candidate.ownership_frontier_witness().unwrap().clone();
        reordered_witness.rows.reverse();
        assert_eq!(
            PsiRewriteCandidate::new_adjacent_block_merge(
                unit.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                candidate.substitutions().to_vec(),
                candidate.provenance().to_vec(),
                reordered_witness,
                candidate.predicted_cost_delta(),
                patch,
            ),
            Err(PsiRewriteCandidateError::NonCanonicalOwnershipFrontierWitness)
        );
    }

    #[test]
    fn adjacent_conditional_merge_fans_incoming_custody_to_exact_arms() {
        let unit = adjacent_conditional_merge_unit();
        let contract = AdjacentBlockMergeRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = AdjacentBlockMergeRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .expect("only the adjacent conditional target is eligible");
        let accepted = validate_adjacent_block_merge_candidate(&unit, &candidate).unwrap();
        let output = accepted.unit();
        assert_eq!(output.functions[0].blocks.len(), 3);
        let node = &output.functions[0].blocks[0].nodes[0];
        assert!(matches!(
            node.operation,
            AbstractOperation::Conditional { condition, .. }
                if condition == id(1_106, ValueId::new)
        ));
        for (edge, direct) in [
            (&node.successors[0], id(1_111, EdgeId::new)),
            (&node.successors[1], id(1_112, EdgeId::new)),
        ] {
            assert_eq!(
                edge.provenance,
                [
                    PsiProvenance::Edge(direct),
                    PsiProvenance::Edge(id(1_110, EdgeId::new)),
                ]
            );
        }
        let incoming = PsiRealizationSite::Edge {
            machine: id(1_101, MachineId::new),
            edge: id(1_110, EdgeId::new),
        };
        assert_eq!(
            accepted
                .provenance()
                .iter()
                .filter(|row| row.input == incoming)
                .count(),
            2
        );

        let PsiRewritePatch::MergeAdjacentBlock(patch) = candidate.patch() else {
            unreachable!()
        };
        let mut corrupted_provenance = candidate.provenance().to_vec();
        corrupted_provenance
            .iter_mut()
            .find(|row| {
                row.input == incoming
                    && row.disposition.site()
                        == (PsiRealizationSite::Edge {
                            machine: id(1_101, MachineId::new),
                            edge: id(1_112, EdgeId::new),
                        })
            })
            .unwrap()
            .disposition =
            ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(NodeLocation {
                machine: id(1_101, MachineId::new),
                block: id(1_103, BlockId::new),
                node: 0,
            }));
        corrupted_provenance.sort_by_key(|row| {
            (
                row.input,
                row.disposition.canonical_tag(),
                row.disposition.site(),
            )
        });
        let corrupted = PsiRewriteCandidate::new_adjacent_block_merge(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.substitutions().to_vec(),
            corrupted_provenance,
            candidate.ownership_frontier_witness().unwrap().clone(),
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_adjacent_block_merge_candidate(&unit, &corrupted),
            Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
        );
    }

    #[test]
    fn non_adjacent_merge_supports_both_roster_directions_and_global_uses() {
        for target_before_predecessor in [false, true] {
            let unit = non_adjacent_merge_unit(target_before_predecessor);
            validate_psi_optimization_unit(&unit).unwrap();
            let contract = NonAdjacentBlockMergeRule::contract();
            let mut manager = crate::AnalysisManager::new(&unit);
            let products = manager
                .require_all(&unit, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let candidates = NonAdjacentBlockMergeRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap();
            let candidate = candidates
                .iter()
                .find(|candidate| {
                    matches!(
                        candidate.patch(),
                        PsiRewritePatch::MergeNonAdjacentBlock(patch)
                            if patch.target == id(1_504, BlockId::new)
                    )
                })
                .expect("predecessor-to-target merge is proposed in either roster direction");
            assert_eq!(
                candidate.affected_blocks(),
                [
                    id(1_503, BlockId::new),
                    id(1_504, BlockId::new),
                    id(1_505, BlockId::new),
                    id(1_506, BlockId::new),
                ]
            );
            assert!(
                AdjacentBlockMergeRule
                    .propose(&unit, RuleAnalysisView::new(&products))
                    .unwrap()
                    .iter()
                    .all(|row| !matches!(
                        row.patch(),
                        PsiRewritePatch::MergeAdjacentBlock(patch)
                            if patch.target == id(1_504, BlockId::new)
                    ))
            );

            let accepted = validate_non_adjacent_block_merge_candidate(&unit, candidate).unwrap();
            let output = accepted.unit();
            assert_eq!(output.functions[0].blocks.len(), 4);
            assert!(
                output.functions[0]
                    .blocks
                    .iter()
                    .all(|block| block.id != id(1_504, BlockId::new))
            );
            let predecessor = output.functions[0]
                .blocks
                .iter()
                .find(|block| block.id == id(1_506, BlockId::new))
                .unwrap();
            assert_eq!(predecessor.nodes.len(), 3);
            assert!(matches!(
                predecessor.nodes[1].operation,
                O::BooleanNot {
                    operand,
                    result,
                    ..
                } if operand == id(1_520, ValueId::new)
                    && result == id(1_510, ValueId::new)
            ));
            assert_eq!(
                predecessor.nodes[1].definitions[0].site,
                omega_optimization_unit::ValueDefinitionSite::Node {
                    block: id(1_506, BlockId::new),
                    node: 1,
                }
            );
            let descendant = output.functions[0]
                .blocks
                .iter()
                .find(|block| block.id == id(1_503, BlockId::new))
                .unwrap();
            assert!(matches!(
                descendant.nodes[0].operation,
                O::BooleanEqual { left, right, .. }
                    if left == id(1_520, ValueId::new)
                        && right == id(1_510, ValueId::new)
            ));
            let incoming = PsiRealizationSite::Edge {
                machine: id(1_501, MachineId::new),
                edge: id(1_519, EdgeId::new),
            };
            assert!(accepted.provenance().iter().any(|row| {
                row.input == incoming
                    && row.disposition
                        == ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(
                            NodeLocation {
                                machine: id(1_501, MachineId::new),
                                block: id(1_506, BlockId::new),
                                node: 1,
                            },
                        ))
            }));

            let PsiRewritePatch::MergeNonAdjacentBlock(patch) = candidate.patch() else {
                unreachable!()
            };
            let mut incomplete = candidate.provenance().to_vec();
            let omitted = incomplete
                .iter()
                .position(|row| row.input != incoming)
                .expect("fixture has non-incoming custody");
            incomplete.remove(omitted);
            let corrupted = PsiRewriteCandidate::new_non_adjacent_block_merge(
                unit.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                candidate.substitutions().to_vec(),
                incomplete,
                candidate.predicted_cost_delta(),
                patch,
            )
            .unwrap();
            assert_eq!(
                validate_non_adjacent_block_merge_candidate(&unit, &corrupted),
                Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
            );
        }
    }

    #[test]
    fn adjacent_merge_rewrites_target_parameter_uses_in_dominated_successors() {
        let mut unit = non_adjacent_merge_unit(false);
        let sibling = unit.functions[0].blocks.remove(2);
        unit.functions[0].blocks.insert(3, sibling);
        let mut effect = 0u64;
        for block in &mut unit.functions[0].blocks {
            for node in &mut block.nodes {
                node.effect = omega_optimization_unit::EffectLink {
                    input: effect,
                    output: effect + 1,
                };
                effect += 1;
            }
        }
        unit.identity = recompute_psi_optimization_unit_identity(&unit);
        validate_psi_optimization_unit(&unit).unwrap();

        let contract = AdjacentBlockMergeRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidate = AdjacentBlockMergeRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .into_iter()
            .find(|candidate| {
                matches!(
                    candidate.patch(),
                    PsiRewritePatch::MergeAdjacentBlock(patch)
                        if patch.target == id(1_504, BlockId::new)
                )
            })
            .expect("forward-adjacent parameterized target is merged");
        let accepted = validate_adjacent_block_merge_candidate(&unit, &candidate).unwrap();
        let descendant = accepted.unit().functions[0]
            .blocks
            .iter()
            .find(|block| block.id == id(1_503, BlockId::new))
            .unwrap();
        assert!(matches!(
            descendant.nodes[0].operation,
            O::BooleanEqual { left, right, .. }
                if left == id(1_520, ValueId::new)
                    && right == id(1_510, ValueId::new)
        ));
    }

    #[test]
    fn shared_terminal_jump_fusion_clones_one_path_and_retains_exact_custody() {
        let threaded = shared_terminal_unit();
        let contract = SharedJumpFusionRule::contract();
        let mut manager = crate::AnalysisManager::new(&threaded);
        let products = manager
            .require_all(&threaded, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidates = SharedJumpFusionRule
            .propose(&threaded, RuleAnalysisView::new(&products))
            .unwrap();
        assert_eq!(candidates.len(), 2);
        let candidate = candidates
            .iter()
            .find(|candidate| {
                matches!(
                    candidate.patch(),
                    PsiRewritePatch::FuseSharedTerminalJump(patch)
                        if patch.predecessor.block == id(923, BlockId::new)
                )
            })
            .expect("left incoming path has an exact fusion candidate");
        let target_before = threaded.functions[0]
            .blocks
            .iter()
            .find(|block| block.id == id(926, BlockId::new))
            .unwrap()
            .clone();
        let accepted = validate_shared_jump_fusion_candidate(&threaded, candidate).unwrap();
        let output = accepted.unit();
        let clone = &output.functions[0]
            .blocks
            .iter()
            .find(|block| block.id == id(923, BlockId::new))
            .unwrap()
            .nodes[0];
        assert!(matches!(clone.operation, O::ReturnUnit { .. }));
        assert_eq!(
            clone.provenance,
            [
                PsiProvenance::Edge(id(936, EdgeId::new)),
                PsiProvenance::Edge(id(933, EdgeId::new)),
            ]
        );
        assert_eq!(
            output.functions[0]
                .blocks
                .iter()
                .find(|block| block.id == id(926, BlockId::new))
                .unwrap(),
            &target_before
        );
        let terminal_input = PsiRealizationSite::Node(NodeLocation {
            machine: id(921, MachineId::new),
            block: id(926, BlockId::new),
            node: 0,
        });
        assert_eq!(
            accepted
                .provenance()
                .iter()
                .filter(|row| row.input == terminal_input)
                .count(),
            2
        );

        let mut nonterminal_duplicate = output.clone();
        let duplicated = PsiProvenance::Edge(id(936, EdgeId::new));
        let nonterminal = &mut nonterminal_duplicate.functions[0]
            .blocks
            .iter_mut()
            .find(|block| block.id == id(923, BlockId::new))
            .unwrap()
            .nodes[0];
        nonterminal.provenance.push(duplicated);
        nonterminal
            .fuel
            .push(omega_optimization_unit::FuelSettlement {
                site: duplicated,
                units: 1,
            });
        nonterminal_duplicate.identity =
            recompute_psi_optimization_unit_identity(&nonterminal_duplicate);
        assert_eq!(
            validate_psi_optimization_unit(&nonterminal_duplicate),
            Err(OptimizationUnitValidationError::DuplicateProvenance(
                duplicated
            ))
        );

        let PsiRewritePatch::FuseSharedTerminalJump(patch) = candidate.patch() else {
            unreachable!()
        };
        let mut incomplete = candidate.provenance().to_vec();
        incomplete
            .retain(|row| row.input != terminal_input || row.disposition.site() != terminal_input);
        let forged = PsiRewriteCandidate::new_shared_jump_fusion(
            threaded.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.substitutions().to_vec(),
            incomplete,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_shared_jump_fusion_candidate(&threaded, &forged),
            Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
        );
    }

    #[test]
    fn same_block_cse_uses_earliest_typed_leader_and_moves_custody_forward() {
        let unit = local_cse_unit();
        let contract = SameBlockTotalScalarCseRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = SameBlockTotalScalarCseRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .expect("swapped commutative operands have one exact CSE candidate");
        assert!(matches!(
            candidate.patch(),
            PsiRewritePatch::EliminateLocalScalarCommonSubexpression(_)
        ));
        assert_eq!(
            candidate.substitutions(),
            [ScalarSubstitution {
                from: id(1_306, ValueId::new),
                to: id(1_305, ValueId::new),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).unwrap()
                )
            }]
        );
        let accepted =
            validate_local_scalar_common_subexpression_candidate(&unit, &candidate).unwrap();
        let output = accepted.unit();
        let nodes = &output.functions[0].blocks[0].nodes;
        assert_eq!(nodes.len(), 3);
        assert!(
            matches!(nodes[1].operation, O::IntegerEqual { left, right, .. } if left == id(1_305, ValueId::new) && right == left)
        );
        assert_eq!(
            nodes[1].provenance,
            [
                PsiProvenance::Operation(id(1_310, OperationId::new)),
                PsiProvenance::Operation(id(1_309, OperationId::new))
            ]
        );
        assert_eq!(accepted.provenance().len(), 3);
        assert!(
            output.functions[0].blocks[0]
                .nodes
                .iter()
                .flat_map(|node| &node.uses)
                .all(|row| row.value != id(1_306, ValueId::new))
        );

        let mut manager = crate::AnalysisManager::new(output);
        let products = manager
            .require_all(output, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            SameBlockTotalScalarCseRule
                .propose(output, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );

        let PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch) = candidate.patch()
        else {
            unreachable!()
        };
        let mut provenance = candidate.provenance().to_vec();
        provenance[0].disposition =
            ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(patch.leader));
        provenance.sort_by_key(|row| {
            (
                row.input,
                row.disposition.canonical_tag(),
                row.disposition.site(),
            )
        });
        let forged = PsiRewriteCandidate::new_local_scalar_common_subexpression(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            provenance,
            -1,
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_local_scalar_common_subexpression_candidate(&unit, &forged),
            Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
        );
    }

    #[test]
    fn proof_certified_same_block_cse_consumes_the_redundant_operations_fact() {
        let unit = proof_certified_local_cse_unit();
        let ordinary_contract = SameBlockTotalScalarCseRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, ordinary_contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            SameBlockTotalScalarCseRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );

        let contract = SameBlockProofCertifiedScalarCseRule::contract();
        assert_eq!(
            contract.safety_class(),
            OptimizationSafetyClass::ProofCertified
        );
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = SameBlockProofCertifiedScalarCseRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .expect("swapped exact-add operands produce one proof-certified CSE candidate");
        let redundant_fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| fact.operation == id(1_309, OperationId::new))
            .expect("fixture retains the redundant operation fact")
            .identity;
        assert_eq!(
            candidate.accepted_obligation_witness(),
            Some(redundant_fact)
        );
        assert_eq!(
            candidate.consumed_facts(),
            [
                omega_optimization_core::OptimizationFactReference::AcceptedObligation(
                    redundant_fact,
                )
            ]
        );
        let PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch) = candidate.patch()
        else {
            unreachable!()
        };
        assert_eq!(patch.leader_operation, id(1_308, OperationId::new));
        assert_eq!(patch.redundant_operation, id(1_309, OperationId::new));
        let accepted =
            validate_local_scalar_common_subexpression_candidate(&unit, &candidate).unwrap();
        assert_eq!(accepted.unit().functions[0].blocks[0].nodes.len(), 3);
        assert_eq!(
            accepted.unit().accepted_obligation_facts,
            unit.accepted_obligation_facts
        );
        assert!(accepted.unit().functions[0].facts.iter().any(|fact| {
            matches!(
                fact,
                OptimizationFact::OperationObligationReference { support, .. }
                    if *support == id(1_308, OperationId::new)
            )
        }));
        assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
            !matches!(
                fact,
                OptimizationFact::OperationObligationReference { support, .. }
                    if *support == id(1_309, OperationId::new)
            )
        }));

        let forged = PsiRewriteCandidate::new_proof_certified_local_scalar_common_subexpression(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                b"foreign proof-certified local CSE fact",
            ),
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_local_scalar_common_subexpression_candidate(&unit, &forged),
            Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        );

        let mut missing_leader = unit.clone();
        missing_leader
            .accepted_obligation_facts
            .retain(|fact| fact.operation != id(1_308, OperationId::new));
        missing_leader.identity = recompute_psi_optimization_unit_identity(&missing_leader);
        let uses = compute_analysis(&missing_leader, AnalysisKind::UseDefinition).unwrap();
        let effects = compute_analysis(&missing_leader, AnalysisKind::EffectSummaries).unwrap();
        assert!(
            SameBlockProofCertifiedScalarCseRule
                .propose(&missing_leader, RuleAnalysisView::new(&[uses, effects]))
                .unwrap()
                .is_empty()
        );
        let forged_without_leader_fact =
            PsiRewriteCandidate::new_proof_certified_local_scalar_common_subexpression(
                missing_leader.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                redundant_fact,
                candidate.predicted_cost_delta(),
                patch,
            )
            .unwrap();
        assert_eq!(
            validate_local_scalar_common_subexpression_candidate(
                &missing_leader,
                &forged_without_leader_fact,
            ),
            Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        );

        let mut missing_redundant = unit.clone();
        missing_redundant
            .accepted_obligation_facts
            .retain(|fact| fact.operation != id(1_309, OperationId::new));
        missing_redundant.identity = recompute_psi_optimization_unit_identity(&missing_redundant);
        let uses = compute_analysis(&missing_redundant, AnalysisKind::UseDefinition).unwrap();
        let effects = compute_analysis(&missing_redundant, AnalysisKind::EffectSummaries).unwrap();
        assert!(
            SameBlockProofCertifiedScalarCseRule
                .propose(&missing_redundant, RuleAnalysisView::new(&[uses, effects]))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn proof_certified_dominator_gvn_consumes_cross_block_redundant_evidence() {
        let unit = proof_certified_dominator_gvn_unit();
        let contract = DominatorProofCertifiedScalarGvnRule::contract();
        assert_eq!(
            contract.safety_class(),
            OptimizationSafetyClass::ProofCertified
        );
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = DominatorProofCertifiedScalarGvnRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .expect("the entry exact add dominates one proof-certified duplicate");
        let redundant_fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| fact.operation == id(1_351, OperationId::new))
            .expect("fixture retains the dominated operation fact")
            .identity;
        assert_eq!(
            candidate.accepted_obligation_witness(),
            Some(redundant_fact)
        );
        let PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) = candidate.patch()
        else {
            unreachable!()
        };
        assert_eq!(patch.leader.block, id(1_343, BlockId::new));
        assert_eq!(patch.redundant.block, id(1_342, BlockId::new));
        assert_eq!(patch.leader_operation, id(1_349, OperationId::new));
        assert_eq!(patch.redundant_operation, id(1_351, OperationId::new));
        let accepted =
            validate_dominating_scalar_common_subexpression_candidate(&unit, &candidate).unwrap();
        assert_eq!(accepted.unit().functions[0].blocks[0].nodes.len(), 2);
        assert_eq!(
            accepted.unit().accepted_obligation_facts,
            unit.accepted_obligation_facts
        );
        assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
            !matches!(
                fact,
                OptimizationFact::OperationObligationReference { support, .. }
                    if *support == id(1_351, OperationId::new)
            )
        }));

        let forged =
            PsiRewriteCandidate::new_proof_certified_dominating_scalar_common_subexpression(
                unit.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                    b"foreign proof-certified dominator GVN fact",
                ),
                candidate.predicted_cost_delta(),
                patch,
            )
            .unwrap();
        assert_eq!(
            validate_dominating_scalar_common_subexpression_candidate(&unit, &forged),
            Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        );
    }

    #[test]
    fn proof_certified_cse_expression_vocabulary_is_closed_and_exact() {
        let seed = proof_certified_local_cse_unit();
        let O::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } = seed.functions[0].blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        let operations = [
            O::IntegerExactCast {
                psi_operation,
                obligation,
                result,
                source_type: scalar_type,
                target_type: scalar_type,
                operand: left,
            },
            O::ExactIntegerShiftLeft {
                psi_operation,
                obligation,
                result,
                value_type: scalar_type,
                count_type: scalar_type,
                value: left,
                count: right,
            },
            O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                result,
                value_type: scalar_type,
                count_type: scalar_type,
                value: left,
                count: right,
            },
            O::ExactIntegerAdd {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            O::ExactIntegerSubtract {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            O::ExactIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            O::ExactIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            O::WrappingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            O::WrappingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            O::SaturatingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            O::SaturatingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
        ];
        for operation in &operations {
            assert!(
                proof_certified_scalar_expression(operation).is_some(),
                "closed proof-bearing shape must have an expression key: {operation:?}"
            );
        }
        assert!(
            proof_certified_scalar_expression(&O::WrappingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            })
            .is_none()
        );

        let exact_add = proof_certified_scalar_expression(&operations[3]).unwrap().0;
        let swapped_add = proof_certified_scalar_expression(&O::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left: right,
            right: left,
        })
        .unwrap()
        .0;
        assert_eq!(exact_add, swapped_add);
        let subtract = proof_certified_scalar_expression(&operations[4]).unwrap().0;
        let swapped_subtract = proof_certified_scalar_expression(&O::ExactIntegerSubtract {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left: right,
            right: left,
        })
        .unwrap()
        .0;
        assert_ne!(subtract, swapped_subtract);
    }

    #[test]
    fn compatible_policy_keys_cover_only_exact_total_counterparts_with_correct_ordering() {
        let value_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        let count_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let left = id(20_001, ValueId::new);
        let right = id(20_002, ValueId::new);
        let leader_operation = id(20_003, OperationId::new);
        let redundant_operation = id(20_004, OperationId::new);
        let leader_result = id(20_005, ValueId::new);
        let redundant_result = id(20_006, ValueId::new);
        let obligation = id(20_007, ObligationId::new);

        for (leader, redundant) in [
            (
                O::WrappingIntegerAdd {
                    psi_operation: leader_operation,
                    result: leader_result,
                    scalar_type: value_type,
                    left,
                    right,
                },
                O::ExactIntegerAdd {
                    psi_operation: redundant_operation,
                    obligation,
                    result: redundant_result,
                    scalar_type: value_type,
                    left: right,
                    right: left,
                },
            ),
            (
                O::SaturatingIntegerAdd {
                    psi_operation: leader_operation,
                    result: leader_result,
                    scalar_type: value_type,
                    left,
                    right,
                },
                O::ExactIntegerAdd {
                    psi_operation: redundant_operation,
                    obligation,
                    result: redundant_result,
                    scalar_type: value_type,
                    left,
                    right,
                },
            ),
            (
                O::WrappingIntegerSubtract {
                    psi_operation: leader_operation,
                    result: leader_result,
                    scalar_type: value_type,
                    left,
                    right,
                },
                O::ExactIntegerSubtract {
                    psi_operation: redundant_operation,
                    obligation,
                    result: redundant_result,
                    scalar_type: value_type,
                    left,
                    right,
                },
            ),
            (
                O::SaturatingIntegerMultiply {
                    psi_operation: leader_operation,
                    result: leader_result,
                    scalar_type: value_type,
                    left,
                    right,
                },
                O::ExactIntegerMultiply {
                    psi_operation: redundant_operation,
                    obligation,
                    result: redundant_result,
                    scalar_type: value_type,
                    left: right,
                    right: left,
                },
            ),
            (
                O::WrappingIntegerShiftLeft {
                    psi_operation: leader_operation,
                    result: leader_result,
                    value_type,
                    count_type,
                    value: left,
                    count: right,
                },
                O::ExactIntegerShiftLeft {
                    psi_operation: redundant_operation,
                    obligation,
                    result: redundant_result,
                    value_type,
                    count_type,
                    value: left,
                    count: right,
                },
            ),
            (
                O::WrappingIntegerShiftRight {
                    psi_operation: leader_operation,
                    result: leader_result,
                    value_type,
                    count_type,
                    value: left,
                    count: right,
                },
                O::ExactIntegerShiftRight {
                    psi_operation: redundant_operation,
                    obligation,
                    result: redundant_result,
                    value_type,
                    count_type,
                    value: left,
                    count: right,
                },
            ),
        ] {
            assert_eq!(
                compatible_policy_scalar_leader(&leader).map(|row| row.0),
                compatible_policy_scalar_redundant(&redundant).map(|row| row.0)
            );
        }

        let reversed_subtract = O::ExactIntegerSubtract {
            psi_operation: redundant_operation,
            obligation,
            result: redundant_result,
            scalar_type: value_type,
            left: right,
            right: left,
        };
        let subtract_leader = O::WrappingIntegerSubtract {
            psi_operation: leader_operation,
            result: leader_result,
            scalar_type: value_type,
            left,
            right,
        };
        assert_ne!(
            compatible_policy_scalar_leader(&subtract_leader).map(|row| row.0),
            compatible_policy_scalar_redundant(&reversed_subtract).map(|row| row.0)
        );
        assert!(
            compatible_policy_scalar_redundant(&O::ExactIntegerDivide {
                psi_operation: redundant_operation,
                obligation,
                result: redundant_result,
                scalar_type: value_type,
                left,
                right,
            })
            .is_none()
        );
    }

    #[test]
    fn compatible_policy_key_matrix_binds_sign_width_family_policy_and_operand_order() {
        #[derive(Clone, Copy)]
        enum ArithmeticFamily {
            Add,
            Subtract,
            Multiply,
        }
        #[derive(Clone, Copy)]
        enum LeaderPolicy {
            Wrapping,
            Saturating,
        }

        let operation = id(20_101, OperationId::new);
        let result = id(20_102, ValueId::new);
        let obligation = id(20_103, ObligationId::new);
        let left = id(20_104, ValueId::new);
        let right = id(20_105, ValueId::new);
        for sign in [IntegerSign::Unsigned, IntegerSign::Signed] {
            for bits in [8, 32, 128] {
                let scalar_type = IntegerType::new(sign, bits).unwrap();
                let foreign_domain = IntegerType::new(sign, bits / 2 + 1).unwrap();
                for family in [
                    ArithmeticFamily::Add,
                    ArithmeticFamily::Subtract,
                    ArithmeticFamily::Multiply,
                ] {
                    let redundant = match family {
                        ArithmeticFamily::Add => O::ExactIntegerAdd {
                            psi_operation: operation,
                            obligation,
                            result,
                            scalar_type,
                            left,
                            right,
                        },
                        ArithmeticFamily::Subtract => O::ExactIntegerSubtract {
                            psi_operation: operation,
                            obligation,
                            result,
                            scalar_type,
                            left,
                            right,
                        },
                        ArithmeticFamily::Multiply => O::ExactIntegerMultiply {
                            psi_operation: operation,
                            obligation,
                            result,
                            scalar_type,
                            left,
                            right,
                        },
                    };
                    let redundant_key = compatible_policy_scalar_redundant(&redundant).unwrap().0;
                    for policy in [LeaderPolicy::Wrapping, LeaderPolicy::Saturating] {
                        let leader = match (family, policy) {
                            (ArithmeticFamily::Add, LeaderPolicy::Wrapping) => {
                                O::WrappingIntegerAdd {
                                    psi_operation: operation,
                                    result,
                                    scalar_type,
                                    left,
                                    right,
                                }
                            }
                            (ArithmeticFamily::Add, LeaderPolicy::Saturating) => {
                                O::SaturatingIntegerAdd {
                                    psi_operation: operation,
                                    result,
                                    scalar_type,
                                    left,
                                    right,
                                }
                            }
                            (ArithmeticFamily::Subtract, LeaderPolicy::Wrapping) => {
                                O::WrappingIntegerSubtract {
                                    psi_operation: operation,
                                    result,
                                    scalar_type,
                                    left,
                                    right,
                                }
                            }
                            (ArithmeticFamily::Subtract, LeaderPolicy::Saturating) => {
                                O::SaturatingIntegerSubtract {
                                    psi_operation: operation,
                                    result,
                                    scalar_type,
                                    left,
                                    right,
                                }
                            }
                            (ArithmeticFamily::Multiply, LeaderPolicy::Wrapping) => {
                                O::WrappingIntegerMultiply {
                                    psi_operation: operation,
                                    result,
                                    scalar_type,
                                    left,
                                    right,
                                }
                            }
                            (ArithmeticFamily::Multiply, LeaderPolicy::Saturating) => {
                                O::SaturatingIntegerMultiply {
                                    psi_operation: operation,
                                    result,
                                    scalar_type,
                                    left,
                                    right,
                                }
                            }
                        };
                        assert_eq!(
                            compatible_policy_scalar_leader(&leader).unwrap().0,
                            redundant_key
                        );
                    }
                    let swapped = match family {
                        ArithmeticFamily::Add => O::ExactIntegerAdd {
                            psi_operation: operation,
                            obligation,
                            result,
                            scalar_type,
                            left: right,
                            right: left,
                        },
                        ArithmeticFamily::Subtract => O::ExactIntegerSubtract {
                            psi_operation: operation,
                            obligation,
                            result,
                            scalar_type,
                            left: right,
                            right: left,
                        },
                        ArithmeticFamily::Multiply => O::ExactIntegerMultiply {
                            psi_operation: operation,
                            obligation,
                            result,
                            scalar_type,
                            left: right,
                            right: left,
                        },
                    };
                    let swapped_key = compatible_policy_scalar_redundant(&swapped).unwrap().0;
                    assert_eq!(
                        redundant_key == swapped_key,
                        !matches!(family, ArithmeticFamily::Subtract)
                    );
                    let foreign = O::WrappingIntegerAdd {
                        psi_operation: operation,
                        result,
                        scalar_type: foreign_domain,
                        left,
                        right,
                    };
                    assert_ne!(
                        compatible_policy_scalar_leader(&foreign).unwrap().0,
                        redundant_key
                    );
                }

                for shift_left in [true, false] {
                    let count_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
                    let leader = if shift_left {
                        O::WrappingIntegerShiftLeft {
                            psi_operation: operation,
                            result,
                            value_type: scalar_type,
                            count_type,
                            value: left,
                            count: right,
                        }
                    } else {
                        O::WrappingIntegerShiftRight {
                            psi_operation: operation,
                            result,
                            value_type: scalar_type,
                            count_type,
                            value: left,
                            count: right,
                        }
                    };
                    let redundant = if shift_left {
                        O::ExactIntegerShiftLeft {
                            psi_operation: operation,
                            obligation,
                            result,
                            value_type: scalar_type,
                            count_type,
                            value: left,
                            count: right,
                        }
                    } else {
                        O::ExactIntegerShiftRight {
                            psi_operation: operation,
                            obligation,
                            result,
                            value_type: scalar_type,
                            count_type,
                            value: left,
                            count: right,
                        }
                    };
                    assert_eq!(
                        compatible_policy_scalar_leader(&leader).unwrap().0,
                        compatible_policy_scalar_redundant(&redundant).unwrap().0
                    );
                    let reversed = if shift_left {
                        O::ExactIntegerShiftLeft {
                            psi_operation: operation,
                            obligation,
                            result,
                            value_type: scalar_type,
                            count_type,
                            value: right,
                            count: left,
                        }
                    } else {
                        O::ExactIntegerShiftRight {
                            psi_operation: operation,
                            obligation,
                            result,
                            value_type: scalar_type,
                            count_type,
                            value: right,
                            count: left,
                        }
                    };
                    assert_ne!(
                        compatible_policy_scalar_leader(&leader).unwrap().0,
                        compatible_policy_scalar_redundant(&reversed).unwrap().0
                    );
                }
            }
        }
    }

    #[test]
    fn compatible_policy_local_and_dominator_gvn_consume_only_redundant_proof_custody() {
        let cases: [(
            PsiOptimizationUnit,
            OptimizationRuleContract,
            &dyn PsiOptimizationRule,
            bool,
        ); 2] = [
            (
                compatible_policy_local_cse_unit(),
                SameBlockProofCertifiedCompatiblePolicyScalarCseRule::contract(),
                &SameBlockProofCertifiedCompatiblePolicyScalarCseRule,
                false,
            ),
            (
                compatible_policy_dominator_gvn_unit(),
                DominatorProofCertifiedCompatiblePolicyScalarGvnRule::contract(),
                &DominatorProofCertifiedCompatiblePolicyScalarGvnRule,
                true,
            ),
        ];
        for (unit, contract, rule, dominating) in cases {
            let mut manager = crate::AnalysisManager::new(&unit);
            let products = manager
                .require_all(&unit, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let [candidate] = rule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .try_into()
                .expect("one compatible-policy redundant expression");
            assert_eq!(candidate.consumed_facts().len(), 1);
            let accepted = if dominating {
                validate_dominating_scalar_common_subexpression_candidate(&unit, &candidate)
                    .unwrap()
            } else {
                validate_local_scalar_common_subexpression_candidate(&unit, &candidate).unwrap()
            };
            assert_eq!(
                accepted.unit().accepted_obligation_facts,
                unit.accepted_obligation_facts
            );
            let PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch) = candidate.patch()
            else {
                if let PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) =
                    candidate.patch()
                {
                    assert!(!accepted.unit().functions[0].facts.iter().any(|fact| {
                        matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                            if *support == patch.redundant_operation)
                    }));
                    continue;
                }
                unreachable!("compatible GVN uses a scalar CSE patch")
            };
            assert!(!accepted.unit().functions[0].facts.iter().any(|fact| {
                matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                    if *support == patch.redundant_operation)
            }));
        }
    }

    #[test]
    fn compatible_policy_gvn_declines_missing_evidence_and_rejects_corruption() {
        let unit = compatible_policy_local_cse_unit();
        let contract = SameBlockProofCertifiedCompatiblePolicyScalarCseRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = SameBlockProofCertifiedCompatiblePolicyScalarCseRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .unwrap();
        let PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch) = candidate.patch()
        else {
            unreachable!()
        };

        let mut wrong_patch = patch;
        wrong_patch.scalar_type = ScalarType::Boolean;
        let forged = PsiRewriteCandidate::new_proof_certified_local_scalar_common_subexpression(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            candidate.accepted_obligation_witness().unwrap(),
            -1,
            wrong_patch,
        )
        .unwrap();
        assert_eq!(
            validate_local_scalar_common_subexpression_candidate(&unit, &forged),
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        );

        let foreign_fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| fact.operation == patch.leader_operation)
            .unwrap()
            .identity;
        let forged = PsiRewriteCandidate::new_proof_certified_local_scalar_common_subexpression(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            foreign_fact,
            -1,
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_local_scalar_common_subexpression_candidate(&unit, &forged),
            Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        );

        let mut provenance = candidate.provenance().to_vec();
        provenance[0].fuel[0].units = provenance[0].fuel[0].units.saturating_add(1);
        let forged = PsiRewriteCandidate::new_proof_certified_local_scalar_common_subexpression(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            provenance,
            candidate.accepted_obligation_witness().unwrap(),
            -1,
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_local_scalar_common_subexpression_candidate(&unit, &forged),
            Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
        );

        let mut missing = unit.clone();
        missing
            .accepted_obligation_facts
            .retain(|fact| fact.operation != patch.redundant_operation);
        missing.identity = recompute_psi_optimization_unit_identity(&missing);
        let mut manager = crate::AnalysisManager::new(&missing);
        let products = manager
            .require_all(&missing, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            SameBlockProofCertifiedCompatiblePolicyScalarCseRule
                .propose(&missing, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
        let exact_only = proof_certified_local_cse_unit();
        let mut manager = crate::AnalysisManager::new(&exact_only);
        let products = manager
            .require_all(&exact_only, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            SameBlockProofCertifiedCompatiblePolicyScalarCseRule
                .propose(&exact_only, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn dominator_gvn_reuses_a_canonical_cross_block_total_scalar_expression() {
        let unit = dominator_gvn_unit();
        let local_contract = SameBlockTotalScalarCseRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let local_products = manager
            .require_all(&unit, local_contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            SameBlockTotalScalarCseRule
                .propose(&unit, RuleAnalysisView::new(&local_products))
                .unwrap()
                .is_empty()
        );

        let contract = DominatorTotalScalarGvnRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = DominatorTotalScalarGvnRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .expect("entry expression strictly dominates one cross-block duplicate");
        let PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) = candidate.patch()
        else {
            unreachable!()
        };
        assert_eq!(patch.leader.block, id(1_343, BlockId::new));
        assert_eq!(patch.redundant.block, id(1_342, BlockId::new));
        let accepted =
            validate_dominating_scalar_common_subexpression_candidate(&unit, &candidate).unwrap();
        let output = accepted.unit();
        assert_eq!(output.functions[0].blocks[0].nodes.len(), 2);
        assert!(
            matches!(output.functions[0].blocks[0].nodes[0].operation, O::IntegerEqual { left, right, .. } if left == id(1_346, ValueId::new) && right == left)
        );
        assert_eq!(
            output.functions[0].blocks[0].nodes[0].provenance,
            [
                PsiProvenance::Operation(id(1_352, OperationId::new)),
                PsiProvenance::Operation(id(1_351, OperationId::new))
            ]
        );
        assert!(
            output.functions[0]
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .flat_map(|node| &node.uses)
                .all(|row| row.value != id(1_347, ValueId::new))
        );

        let mut forged_patch = patch;
        forged_patch.leader.node = 1;
        forged_patch.leader_operation = id(1_350, OperationId::new);
        let forged = PsiRewriteCandidate::new_dominating_scalar_common_subexpression(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            -1,
            forged_patch,
        )
        .unwrap();
        assert_eq!(
            validate_dominating_scalar_common_subexpression_candidate(&unit, &forged),
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        );
    }

    #[test]
    fn dominator_gvn_cascades_through_a_non_topological_diamond_to_fixed_point() {
        let mut unit = diamond_dominator_gvn_unit();
        let contract = DominatorTotalScalarGvnRule::contract();
        for (expected_redundant, expected_leader) in [
            (id(1_410, ValueId::new), id(1_408, ValueId::new)),
            (id(1_411, ValueId::new), id(1_409, ValueId::new)),
        ] {
            let mut manager = crate::AnalysisManager::new(&unit);
            let products = manager
                .require_all(&unit, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let [candidate] = DominatorTotalScalarGvnRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .try_into()
                .expect("one newly exposed cross-block value number");
            let PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) =
                candidate.patch()
            else {
                unreachable!()
            };
            assert_eq!(patch.redundant_result, expected_redundant);
            assert_eq!(patch.leader_result, expected_leader);
            assert_eq!(
                candidate.affected_blocks(),
                [
                    id(1_402, BlockId::new),
                    id(1_403, BlockId::new),
                    id(1_404, BlockId::new),
                    id(1_405, BlockId::new)
                ]
            );
            unit = validate_dominating_scalar_common_subexpression_candidate(&unit, &candidate)
                .unwrap()
                .into_unit();
        }
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            DominatorTotalScalarGvnRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
        let join = &unit.functions[0].blocks[0];
        assert_eq!(join.nodes.len(), 1);
        assert!(
            matches!(join.nodes[0].operation, O::Return { value, .. } if value == id(1_409, ValueId::new))
        );
        assert_eq!(
            join.nodes[0].provenance,
            [
                PsiProvenance::Edge(id(1_414, EdgeId::new)),
                PsiProvenance::Operation(id(1_413, OperationId::new)),
                PsiProvenance::Operation(id(1_412, OperationId::new))
            ]
        );
    }

    #[test]
    fn dominator_gvn_rejects_an_equivalent_sibling_expression_at_a_join() {
        let unit = sibling_only_gvn_unit();
        let contract = DominatorTotalScalarGvnRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            DominatorTotalScalarGvnRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );

        let function = &unit.functions[0];
        let leader = NodeLocation {
            machine: function.machine,
            block: id(1_443, BlockId::new),
            node: 0,
        };
        let redundant = NodeLocation {
            machine: function.machine,
            block: id(1_442, BlockId::new),
            node: 0,
        };
        let (affected, provenance) =
            local_cse_accounting(function, redundant, id(1_449, ValueId::new)).unwrap();
        let forged = PsiRewriteCandidate::new_dominating_scalar_common_subexpression(
            unit.identity,
            contract,
            affected,
            provenance,
            -1,
            DominatingScalarCommonSubexpressionRewrite {
                leader,
                redundant,
                leader_operation: id(1_452, OperationId::new),
                redundant_operation: id(1_450, OperationId::new),
                leader_result: id(1_448, ValueId::new),
                redundant_result: id(1_449, ValueId::new),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                ),
            },
        )
        .unwrap();
        assert_eq!(
            validate_dominating_scalar_common_subexpression_candidate(&unit, &forged),
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        );
    }

    fn phi_translated_candidates(unit: &PsiOptimizationUnit) -> Vec<PsiRewriteCandidate> {
        let contract = PhiTranslatedObligationFreeScalarGvnRule::contract();
        let mut manager = crate::AnalysisManager::new(unit);
        let products = manager
            .require_all(unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        PhiTranslatedObligationFreeScalarGvnRule
            .propose(unit, RuleAnalysisView::new(&products))
            .unwrap()
    }

    fn proof_certified_phi_translated_candidates(
        unit: &PsiOptimizationUnit,
    ) -> Vec<PsiRewriteCandidate> {
        let contract = PhiTranslatedProofCertifiedScalarGvnRule::contract();
        let mut manager = crate::AnalysisManager::new(unit);
        let products = manager
            .require_all(unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        PhiTranslatedProofCertifiedScalarGvnRule
            .propose(unit, RuleAnalysisView::new(&products))
            .unwrap()
    }

    fn compatible_policy_phi_translated_candidates(
        unit: &PsiOptimizationUnit,
    ) -> Vec<PsiRewriteCandidate> {
        let contract = PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract();
        let mut manager = crate::AnalysisManager::new(unit);
        let products = manager
            .require_all(unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule
            .propose(unit, RuleAnalysisView::new(&products))
            .unwrap()
    }

    #[test]
    fn phi_translated_gvn_preserves_result_identity_and_reaches_fixed_point() {
        let unit = phi_translated_gvn_unit();
        let [candidate] = phi_translated_candidates(&unit)
            .try_into()
            .expect("both predecessor translations have available leaders");
        let PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) =
            candidate.patch()
        else {
            unreachable!()
        };
        assert_eq!(patch.parameter_position, 1);
        assert_eq!(patch.redundant_result, id(1_710, ValueId::new));
        assert_eq!(
            patch
                .incoming
                .iter()
                .map(|row| (row.edge, row.source, row.leader_result))
                .collect::<Vec<_>>(),
            [
                (
                    id(1_717, EdgeId::new),
                    id(1_705, BlockId::new),
                    id(1_712, ValueId::new),
                ),
                (
                    id(1_720, EdgeId::new),
                    id(1_703, BlockId::new),
                    id(1_711, ValueId::new),
                ),
            ]
        );
        assert!(candidate.substitutions().is_empty());
        assert!(candidate.consumed_facts().is_empty());

        let accepted =
            validate_phi_translated_scalar_common_subexpression_candidate(&unit, &candidate)
                .unwrap();
        let output = accepted.unit();
        let join = &output.functions[0].blocks[0];
        assert_eq!(join.parameters.len(), 2);
        assert_eq!(join.parameters[1].value, id(1_710, ValueId::new));
        assert_eq!(join.nodes.len(), 1);
        assert!(
            matches!(join.nodes[0].operation, O::Return { value, .. } if value == id(1_710, ValueId::new))
        );
        for (source, leader) in [
            (id(1_703, BlockId::new), id(1_711, ValueId::new)),
            (id(1_705, BlockId::new), id(1_712, ValueId::new)),
        ] {
            let edge = output.functions[0]
                .blocks
                .iter()
                .find(|block| block.id == source)
                .unwrap()
                .nodes
                .last()
                .unwrap()
                .successors
                .first()
                .unwrap();
            assert_eq!(edge.bindings.len(), 2);
            assert_eq!(edge.bindings[1].parameter, id(1_710, ValueId::new));
            assert_eq!(edge.bindings[1].argument, leader);
        }
        assert!(phi_translated_candidates(output).is_empty());

        let mut corrupted_patch = patch;
        corrupted_patch.incoming[0].leader_result = id(1_711, ValueId::new);
        let corrupted = PsiRewriteCandidate::new_phi_translated_scalar_common_subexpression(
            unit.identity,
            PhiTranslatedObligationFreeScalarGvnRule::contract(),
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            candidate.predicted_cost_delta(),
            corrupted_patch,
        )
        .unwrap();
        assert_eq!(
            validate_phi_translated_scalar_common_subexpression_candidate(&unit, &corrupted),
            Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)
        );
    }

    #[test]
    fn phi_translated_gvn_requires_a_typed_leader_on_every_incoming_arm() {
        for right_arm in [
            PhiTranslatedRightArm::Missing,
            PhiTranslatedRightArm::MismatchedType,
        ] {
            let unit = phi_translated_gvn_fixture(right_arm, false, false);
            assert!(phi_translated_candidates(&unit).is_empty());
        }
    }

    #[test]
    fn phi_translated_gvn_candidate_rejects_noncanonical_incoming_order() {
        let unit = phi_translated_gvn_unit();
        let [candidate] = phi_translated_candidates(&unit).try_into().unwrap();
        let PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(mut patch) =
            candidate.patch()
        else {
            unreachable!()
        };
        patch.incoming.reverse();
        assert_eq!(
            PsiRewriteCandidate::new_phi_translated_scalar_common_subexpression(
                unit.identity,
                PhiTranslatedObligationFreeScalarGvnRule::contract(),
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                candidate.predicted_cost_delta(),
                patch,
            ),
            Err(omega_optimization_unit::PsiRewriteCandidateError::PatchDecisionPointMismatch)
        );
    }

    #[test]
    fn proof_certified_phi_translation_consumes_only_redundant_evidence() {
        let unit = proof_certified_phi_translated_gvn_unit();
        assert!(phi_translated_candidates(&unit).is_empty());
        let [candidate] = proof_certified_phi_translated_candidates(&unit)
            .try_into()
            .expect("all three exact-add operations retain accepted evidence");
        let redundant_fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| fact.operation == id(1_713, OperationId::new))
            .unwrap()
            .identity;
        assert_eq!(
            candidate.accepted_obligation_witness(),
            Some(redundant_fact)
        );
        assert_eq!(
            candidate.consumed_facts(),
            [
                omega_optimization_core::OptimizationFactReference::AcceptedObligation(
                    redundant_fact,
                ),
            ]
        );
        let PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) =
            candidate.patch()
        else {
            unreachable!()
        };
        assert_eq!(
            patch
                .incoming
                .iter()
                .map(|row| row.leader_operation)
                .collect::<Vec<_>>(),
            [id(1_716, OperationId::new), id(1_715, OperationId::new),]
        );
        let accepted =
            validate_phi_translated_scalar_common_subexpression_candidate(&unit, &candidate)
                .unwrap();
        assert_eq!(
            accepted.validator(),
            omega_optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.phi-translated-proof-certified-total-scalar-gvn.v1",
            )
        );
        assert_eq!(
            accepted.unit().accepted_obligation_facts,
            unit.accepted_obligation_facts
        );
        assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
            !matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                if *support == id(1_713, OperationId::new))
        }));
        assert!(proof_certified_phi_translated_candidates(accepted.unit()).is_empty());

        let foreign =
            PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
                unit.identity,
                PhiTranslatedProofCertifiedScalarGvnRule::contract(),
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                    b"foreign proof phi fact",
                ),
                candidate.predicted_cost_delta(),
                patch,
            )
            .unwrap();
        assert_eq!(
            validate_phi_translated_scalar_common_subexpression_candidate(&unit, &foreign),
            Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        );
    }

    #[test]
    fn proof_certified_phi_translation_requires_every_leader_fact() {
        let original = proof_certified_phi_translated_gvn_unit();
        let [candidate] = proof_certified_phi_translated_candidates(&original)
            .try_into()
            .unwrap();
        let PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) =
            candidate.patch()
        else {
            unreachable!()
        };
        let redundant_fact = candidate.accepted_obligation_witness().unwrap();
        let mut unit = original;
        unit.accepted_obligation_facts
            .retain(|fact| fact.operation != id(1_716, OperationId::new));
        unit.identity = recompute_psi_optimization_unit_identity(&unit);
        assert!(proof_certified_phi_translated_candidates(&unit).is_empty());
        let detached_leader =
            PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
                unit.identity,
                PhiTranslatedProofCertifiedScalarGvnRule::contract(),
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                redundant_fact,
                candidate.predicted_cost_delta(),
                patch,
            )
            .unwrap();
        assert_eq!(
            validate_phi_translated_scalar_common_subexpression_candidate(&unit, &detached_leader,),
            Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        );
    }

    #[test]
    fn compatible_policy_phi_translation_preserves_result_and_consumes_only_redundant_evidence() {
        let unit = compatible_policy_phi_translated_gvn_unit();
        let [candidate] = compatible_policy_phi_translated_candidates(&unit)
            .try_into()
            .expect("wrapping and saturating arm leaders are compatible");
        assert!(phi_translated_candidates(&unit).is_empty());
        assert!(proof_certified_phi_translated_candidates(&unit).is_empty());
        let redundant_fact = unit.accepted_obligation_facts[0].identity;
        assert_eq!(
            candidate.accepted_obligation_witness(),
            Some(redundant_fact)
        );
        assert_eq!(candidate.substitutions(), []);
        assert_eq!(candidate.consumed_facts().len(), 1);
        let accepted =
            validate_phi_translated_scalar_common_subexpression_candidate(&unit, &candidate)
                .unwrap();
        assert_eq!(
            accepted.validator(),
            omega_optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.phi-translated-proof-certified-compatible-policy-scalar-gvn.v1",
            )
        );
        assert_eq!(
            accepted.unit().accepted_obligation_facts,
            unit.accepted_obligation_facts
        );
        assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
            !matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                if *support == id(1_713, OperationId::new))
        }));
        let join = &accepted.unit().functions[0].blocks[0];
        assert_eq!(
            join.parameters.last().unwrap().value,
            id(1_710, ValueId::new)
        );
        assert_eq!(join.nodes.len(), 1);
        assert!(compatible_policy_phi_translated_candidates(accepted.unit()).is_empty());
    }

    #[test]
    fn compatible_policy_phi_translation_declines_incomplete_arms_and_rejects_corruption() {
        for right_arm in [
            PhiTranslatedRightArm::Missing,
            PhiTranslatedRightArm::MismatchedType,
        ] {
            let unit = phi_translated_gvn_fixture(right_arm, false, true);
            assert!(compatible_policy_phi_translated_candidates(&unit).is_empty());
        }

        let original = compatible_policy_phi_translated_gvn_unit();
        let [candidate] = compatible_policy_phi_translated_candidates(&original)
            .try_into()
            .unwrap();
        let PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) =
            candidate.patch()
        else {
            unreachable!()
        };

        let mut missing_fact = original.clone();
        missing_fact.accepted_obligation_facts.clear();
        missing_fact.identity = recompute_psi_optimization_unit_identity(&missing_fact);
        assert!(compatible_policy_phi_translated_candidates(&missing_fact).is_empty());

        let foreign_fact =
            PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
                original.identity,
                PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract(),
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                    b"foreign compatible phi fact",
                ),
                candidate.predicted_cost_delta(),
                patch.clone(),
            )
            .unwrap();
        assert_eq!(
            validate_phi_translated_scalar_common_subexpression_candidate(&original, &foreign_fact,),
            Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        );

        let mut unavailable_patch = patch.clone();
        unavailable_patch.incoming[0].leader.node = 1;
        let unavailable =
            PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
                original.identity,
                PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract(),
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                candidate.accepted_obligation_witness().unwrap(),
                candidate.predicted_cost_delta(),
                unavailable_patch,
            )
            .unwrap();
        assert_eq!(
            validate_phi_translated_scalar_common_subexpression_candidate(&original, &unavailable),
            Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)
        );

        let mut detached_patch = patch.clone();
        detached_patch.incoming[0].leader_operation = id(20_201, OperationId::new);
        detached_patch.incoming[0].leader_result = id(20_202, ValueId::new);
        let detached =
            PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
                original.identity,
                PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract(),
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                candidate.accepted_obligation_witness().unwrap(),
                candidate.predicted_cost_delta(),
                detached_patch,
            )
            .unwrap();
        assert_eq!(
            validate_phi_translated_scalar_common_subexpression_candidate(&original, &detached),
            Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)
        );

        let mut reordered_patch = patch;
        reordered_patch.incoming.reverse();
        assert_eq!(
            PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
                original.identity,
                PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract(),
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                candidate.accepted_obligation_witness().unwrap(),
                candidate.predicted_cost_delta(),
                reordered_patch,
            ),
            Err(omega_optimization_unit::PsiRewriteCandidateError::PatchDecisionPointMismatch)
        );
    }

    #[test]
    fn dead_scalar_literals_rehome_operation_custody_without_tombstones() {
        let unit = dead_scalar_literals_unit();
        let contract = DeadScalarLiteralEliminationRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidates = DeadScalarLiteralEliminationRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap();
        assert_eq!(candidates.len(), 2);
        let first = candidates
            .iter()
            .find(|candidate| candidate.node_decision_point().unwrap().node == 0)
            .unwrap();
        let accepted = validate_dead_scalar_node_candidate(&unit, first).unwrap();
        assert_eq!(accepted.unit().functions[0].blocks[0].nodes.len(), 2);
        assert_eq!(accepted.unit().functions[0].facts.len(), 1);
        assert_eq!(
            accepted.unit().functions[0].blocks[0].nodes[0].provenance,
            [
                PsiProvenance::Operation(id(1_206, OperationId::new)),
                PsiProvenance::Operation(id(1_205, OperationId::new)),
            ]
        );
        assert!(
            accepted
                .provenance()
                .iter()
                .all(|row| row.disposition.is_realized())
        );

        let next_unit = accepted.into_unit();
        let mut manager = crate::AnalysisManager::new(&next_unit);
        let products = manager
            .require_all(&next_unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [second] = DeadScalarLiteralEliminationRule
            .propose(&next_unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .expect("only the inherited integer literal remains dead");
        let final_unit = validate_dead_scalar_node_candidate(&next_unit, &second)
            .unwrap()
            .into_unit();
        let terminal = &final_unit.functions[0].blocks[0].nodes[0];
        assert!(matches!(terminal.operation, O::ReturnUnit { .. }));
        assert_eq!(
            terminal.provenance,
            [
                PsiProvenance::Edge(id(1_207, EdgeId::new)),
                PsiProvenance::Operation(id(1_206, OperationId::new)),
                PsiProvenance::Operation(id(1_205, OperationId::new)),
            ]
        );

        let mut used = unit.clone();
        used.functions[0].blocks[0].nodes[2].operation = O::Return {
            psi_edge: id(1_207, EdgeId::new),
            result: id(1_204, ValueId::new),
            value: id(1_204, ValueId::new),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
            cleanup_actions: Vec::new(),
        };
        used.functions[0].result = AbstractFunctionResult::Scalar(AbstractResult {
            value: id(1_204, ValueId::new),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
        });
        used.functions[0].blocks[0].nodes[2].uses = vec![omega_optimization_unit::ValueUse {
            value: id(1_204, ValueId::new),
            block: id(1_202, BlockId::new),
            node: 2,
        }];
        used.identity = recompute_psi_optimization_unit_identity(&used);
        validate_psi_optimization_unit(&used).unwrap();
        let liveness = compute_analysis(&used, AnalysisKind::ValueLiveness).unwrap();
        let effects = compute_analysis(&used, AnalysisKind::EffectSummaries).unwrap();
        let proposed = DeadScalarLiteralEliminationRule
            .propose(&used, RuleAnalysisView::new(&[liveness, effects]))
            .unwrap();
        assert_eq!(proposed.len(), 1);
        assert_eq!(proposed[0].node_decision_point().unwrap().node, 0);
    }

    #[test]
    fn dead_total_scalar_rule_removes_wrapping_add_but_not_proof_bearing_exact_add() {
        let unit = dead_wrapping_add_unit();
        let contract = DeadUnconditionallyTotalScalarEliminationRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = DeadUnconditionallyTotalScalarEliminationRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .expect("only the unused wrapping add is in this rule family");
        assert_eq!(candidate.node_decision_point().unwrap().node, 2);
        let accepted = validate_dead_scalar_node_candidate(&unit, &candidate).unwrap();
        assert_eq!(accepted.unit().functions[0].blocks[0].nodes.len(), 3);
        assert_eq!(
            accepted.unit().functions[0].blocks[0].nodes[2]
                .provenance
                .len(),
            2
        );

        let PsiRewritePatch::RemoveDeadScalarNode(patch) = candidate.patch() else {
            unreachable!()
        };
        let wrong_family = PsiRewriteCandidate::new_dead_scalar_node(
            unit.identity,
            DeadScalarLiteralEliminationRule::contract(),
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_dead_scalar_node_candidate(&unit, &wrong_family),
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        );

        let exact = dead_exact_add_unit();
        validate_psi_optimization_unit(&exact).unwrap();
        let liveness = compute_analysis(&exact, AnalysisKind::ValueLiveness).unwrap();
        let effects = compute_analysis(&exact, AnalysisKind::EffectSummaries).unwrap();
        assert!(
            DeadUnconditionallyTotalScalarEliminationRule
                .propose(&exact, RuleAnalysisView::new(&[liveness, effects]))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn proof_check_elision_binds_accepted_evidence_and_retains_its_catalog() {
        let unit = dead_exact_add_unit();
        validate_psi_optimization_unit(&unit).unwrap();
        let contract = ProofCertifiedDeadScalarEliminationRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = ProofCertifiedDeadScalarEliminationRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .expect("the unused proof-certified exact add is the sole candidate");
        assert_eq!(candidate.node_decision_point().unwrap().node, 2);
        assert_eq!(
            candidate.accepted_obligation_witness(),
            Some(unit.accepted_obligation_facts[0].identity)
        );
        assert_eq!(
            candidate.consumed_facts(),
            [
                omega_optimization_core::OptimizationFactReference::AcceptedObligation(
                    unit.accepted_obligation_facts[0].identity,
                )
            ]
        );
        let accepted = validate_dead_scalar_node_candidate(&unit, &candidate).unwrap();
        assert_eq!(accepted.unit().functions[0].blocks[0].nodes.len(), 3);
        assert_eq!(
            accepted.unit().accepted_obligation_facts,
            unit.accepted_obligation_facts
        );
        assert!(
            accepted.unit().functions[0]
                .facts
                .iter()
                .all(|fact| !matches!(fact, OptimizationFact::OperationObligationReference { .. }))
        );

        let PsiRewritePatch::RemoveDeadScalarNode(patch) = candidate.patch() else {
            unreachable!()
        };
        let forged = PsiRewriteCandidate::new_proof_certified_dead_scalar_node(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                b"foreign accepted obligation",
            ),
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_dead_scalar_node_candidate(&unit, &forged),
            Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        );

        let mut bare = unit.clone();
        bare.accepted_obligation_facts.clear();
        bare.identity = recompute_psi_optimization_unit_identity(&bare);
        let liveness = compute_analysis(&bare, AnalysisKind::ValueLiveness).unwrap();
        let effects = compute_analysis(&bare, AnalysisKind::EffectSummaries).unwrap();
        assert!(matches!(
            ProofCertifiedDeadScalarEliminationRule
                .propose(&bare, RuleAnalysisView::new(&[liveness, effects])),
            Err(RuleProposalError::MissingAcceptedObligation { .. })
        ));
    }

    #[test]
    fn live_proof_certified_identity_elision_substitutes_and_replays_exact_custody() {
        let unit = live_exact_add_zero_unit();
        let contract = LiveProofCertifiedIntegerIdentityEliminationRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = LiveProofCertifiedIntegerIdentityEliminationRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .expect("x + 0 has one canonical proof-certified identity rewrite");
        let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch()
        else {
            unreachable!()
        };
        assert_eq!(
            patch.identity,
            ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroRight
        );
        assert_eq!(patch.replacement, id(303, ValueId::new));
        assert_eq!(candidate.consumed_facts().len(), 2);
        assert!(candidate.consumed_facts().iter().any(|fact| matches!(
            fact,
            omega_optimization_core::OptimizationFactReference::AcceptedObligation(_)
        )));
        assert!(candidate.consumed_facts().iter().any(|fact| matches!(
            fact,
            omega_optimization_core::OptimizationFactReference::ScalarConstant(_)
        )));

        let accepted =
            validate_proof_certified_scalar_identity_candidate(&unit, &candidate).unwrap();
        assert_eq!(
            accepted.validator(),
            OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.live-proof-certified-integer-identity-elimination.v1"
            )
        );
        assert_eq!(
            accepted.unit().accepted_obligation_facts,
            unit.accepted_obligation_facts
        );
        assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
            !matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                if *support == id(308, OperationId::new))
        }));
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[2].operation,
            O::Return { value, .. } if value == id(303, ValueId::new)
        ));
        assert!(
            accepted
                .provenance()
                .iter()
                .all(|row| row.disposition.is_realized())
        );

        let mut manager = crate::AnalysisManager::new(accepted.unit());
        let products = manager
            .require_all(accepted.unit(), contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedIntegerIdentityEliminationRule
                .propose(accepted.unit(), RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );

        let (constant_fact, obligation_fact) =
            candidate.proof_certified_scalar_identity_witness().unwrap();
        let forged_kind = PsiRewriteCandidate::new_proof_certified_scalar_identity(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            constant_fact,
            obligation_fact,
            candidate.predicted_cost_delta(),
            ProofCertifiedScalarIdentityRewrite {
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerSubtractZeroRight,
                ..patch
            },
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_scalar_identity_candidate(&unit, &forged_kind),
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        );
        let foreign_constant = PsiRewriteCandidate::new_proof_certified_scalar_identity(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            ScalarConstantFactIdentity::from_canonical_bytes(b"foreign identity constant"),
            obligation_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_scalar_identity_candidate(&unit, &foreign_constant),
            Err(OptimizationUnitValidationError::CandidateOperandFactMismatch)
        );
    }

    #[test]
    fn live_proof_certified_identity_vocabulary_is_closed_and_canonical() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let left = id(303, ValueId::new);
        let right = id(304, ValueId::new);
        let result = id(305, ValueId::new);
        let source = id(308, OperationId::new);
        let obligation = id(309, ObligationId::new);
        let rows = [
            (
                IntegerValue::Unsigned(0),
                IntegerValue::Unsigned(7),
                O::ExactIntegerAdd {
                    psi_operation: source,
                    obligation,
                    result,
                    scalar_type: integer,
                    left,
                    right,
                },
                ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroLeft,
            ),
            (
                IntegerValue::Unsigned(7),
                IntegerValue::Unsigned(0),
                O::ExactIntegerAdd {
                    psi_operation: source,
                    obligation,
                    result,
                    scalar_type: integer,
                    left,
                    right,
                },
                ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroRight,
            ),
            (
                IntegerValue::Unsigned(7),
                IntegerValue::Unsigned(0),
                O::ExactIntegerSubtract {
                    psi_operation: source,
                    obligation,
                    result,
                    scalar_type: integer,
                    left,
                    right,
                },
                ProofCertifiedScalarIdentityKind::ExactIntegerSubtractZeroRight,
            ),
            (
                IntegerValue::Unsigned(1),
                IntegerValue::Unsigned(7),
                O::ExactIntegerMultiply {
                    psi_operation: source,
                    obligation,
                    result,
                    scalar_type: integer,
                    left,
                    right,
                },
                ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneLeft,
            ),
            (
                IntegerValue::Unsigned(7),
                IntegerValue::Unsigned(1),
                O::ExactIntegerMultiply {
                    psi_operation: source,
                    obligation,
                    result,
                    scalar_type: integer,
                    left,
                    right,
                },
                ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneRight,
            ),
            (
                IntegerValue::Unsigned(7),
                IntegerValue::Unsigned(0),
                O::ExactIntegerShiftLeft {
                    psi_operation: source,
                    obligation,
                    result,
                    value_type: integer,
                    count_type: integer,
                    value: left,
                    count: right,
                },
                ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroCount,
            ),
            (
                IntegerValue::Unsigned(7),
                IntegerValue::Unsigned(0),
                O::ExactIntegerShiftRight {
                    psi_operation: source,
                    obligation,
                    result,
                    value_type: integer,
                    count_type: integer,
                    value: left,
                    count: right,
                },
                ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroCount,
            ),
        ];
        for (left_constant, right_constant, operation, expected) in rows {
            let mut unit = exact_add_unit();
            for (index, constant) in [left_constant, right_constant].into_iter().enumerate() {
                let O::IntegerConstant { value, .. } =
                    &mut unit.functions[0].blocks[0].nodes[index].operation
                else {
                    unreachable!()
                };
                *value = constant;
                let defined = [left, right][index];
                for fact in &mut unit.functions[0].facts {
                    if let OptimizationFact::IntegerConstant {
                        value,
                        constant: row,
                        ..
                    } = fact
                        && *value == defined
                    {
                        *row = constant;
                    }
                }
            }
            unit.functions[0].blocks[0].nodes[2].operation = operation;
            unit.identity = recompute_psi_optimization_unit_identity(&unit);
            validate_psi_optimization_unit(&unit).unwrap();
            let contract = LiveProofCertifiedIntegerIdentityEliminationRule::contract();
            let mut manager = crate::AnalysisManager::new(&unit);
            let products = manager
                .require_all(&unit, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let [candidate] = LiveProofCertifiedIntegerIdentityEliminationRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .try_into()
                .unwrap();
            let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch()
            else {
                unreachable!()
            };
            assert_eq!(patch.identity, expected);
            validate_proof_certified_scalar_identity_candidate(&unit, &candidate).unwrap();
        }

        let mut missing_proof = live_exact_add_zero_unit();
        missing_proof.accepted_obligation_facts.clear();
        missing_proof.identity = recompute_psi_optimization_unit_identity(&missing_proof);
        let contract = LiveProofCertifiedIntegerIdentityEliminationRule::contract();
        let mut manager = crate::AnalysisManager::new(&missing_proof);
        let products = manager
            .require_all(&missing_proof, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedIntegerIdentityEliminationRule
                .propose(&missing_proof, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn proof_certified_divide_by_one_covers_every_policy_and_integer_sign() {
        for integer in [
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            IntegerType::new(IntegerSign::Signed, 8).unwrap(),
        ] {
            for (policy, expected) in [
                (
                    0,
                    ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight,
                ),
                (
                    1,
                    ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight,
                ),
                (
                    2,
                    ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideOneRight,
                ),
            ] {
                let unit = live_divide_by_one_unit(
                    integer,
                    |psi_operation, obligation, result, scalar_type, left, right| match policy {
                        0 => O::ExactIntegerDivide {
                            psi_operation,
                            obligation,
                            result,
                            scalar_type,
                            left,
                            right,
                        },
                        1 => O::WrappingIntegerDivide {
                            psi_operation,
                            obligation,
                            result,
                            scalar_type,
                            left,
                            right,
                        },
                        2 => O::SaturatingIntegerDivide {
                            psi_operation,
                            obligation,
                            result,
                            scalar_type,
                            left,
                            right,
                        },
                        _ => unreachable!(),
                    },
                );
                let contract = LiveProofCertifiedIntegerDivideByOneEliminationRule::contract();
                let mut manager = crate::AnalysisManager::new(&unit);
                let products = manager
                    .require_all(&unit, contract.required_analyses())
                    .unwrap()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                let [candidate] = LiveProofCertifiedIntegerDivideByOneEliminationRule
                    .propose(&unit, RuleAnalysisView::new(&products))
                    .unwrap()
                    .try_into()
                    .expect("each typed divide by literal one has one candidate");
                let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) =
                    candidate.patch()
                else {
                    unreachable!()
                };
                assert_eq!(patch.identity, expected);
                assert_eq!(patch.replacement, id(323, ValueId::new));
                assert_eq!(candidate.consumed_facts().len(), 2);
                let accepted =
                    validate_proof_certified_scalar_identity_candidate(&unit, &candidate).unwrap();
                assert_eq!(
                    accepted.validator(),
                    OptimizationValidatorIdentity::from_canonical_bytes(
                        b"omega.validator.live-proof-certified-integer-divide-by-one-elimination.v1"
                    )
                );
                assert_eq!(
                    accepted.unit().accepted_obligation_facts,
                    unit.accepted_obligation_facts
                );
                assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
                    !matches!(fact, OptimizationFact::OperationObligationReference { .. })
                }));
                assert!(matches!(
                    accepted.unit().functions[0].blocks[0].nodes[1].operation,
                    O::Return { value, .. } if value == id(323, ValueId::new)
                ));
                assert_eq!(
                    accepted.unit().functions[0].blocks[0].nodes[1]
                        .provenance
                        .len(),
                    2
                );
                assert_eq!(
                    accepted.unit().functions[0].blocks[0].nodes[1].fuel.len(),
                    2
                );

                let mut manager = crate::AnalysisManager::new(accepted.unit());
                let products = manager
                    .require_all(accepted.unit(), contract.required_analyses())
                    .unwrap()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                assert!(
                    LiveProofCertifiedIntegerDivideByOneEliminationRule
                        .propose(accepted.unit(), RuleAnalysisView::new(&products))
                        .unwrap()
                        .is_empty()
                );
            }
        }
    }

    #[test]
    fn proof_certified_divide_by_one_declines_missing_evidence_and_rejects_corruption() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let make_exact =
            |psi_operation, obligation, result, scalar_type, left, right| O::ExactIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            };
        let unit = live_divide_by_one_unit(integer, make_exact);
        let contract = LiveProofCertifiedIntegerDivideByOneEliminationRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = LiveProofCertifiedIntegerDivideByOneEliminationRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .unwrap();
        let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch()
        else {
            unreachable!()
        };
        let (constant_fact, obligation_fact) =
            candidate.proof_certified_scalar_identity_witness().unwrap();

        let forged_kind = PsiRewriteCandidate::new_proof_certified_scalar_identity(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            constant_fact,
            obligation_fact,
            candidate.predicted_cost_delta(),
            ProofCertifiedScalarIdentityRewrite {
                identity: ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight,
                ..patch
            },
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_scalar_identity_candidate(&unit, &forged_kind),
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        );
        let forged_old_family = PsiRewriteCandidate::new_proof_certified_scalar_identity(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            constant_fact,
            obligation_fact,
            candidate.predicted_cost_delta(),
            ProofCertifiedScalarIdentityRewrite {
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneRight,
                ..patch
            },
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_scalar_identity_candidate(&unit, &forged_old_family),
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        );
        let foreign_constant = PsiRewriteCandidate::new_proof_certified_scalar_identity(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            ScalarConstantFactIdentity::from_canonical_bytes(b"foreign divide-one literal"),
            obligation_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_scalar_identity_candidate(&unit, &foreign_constant),
            Err(OptimizationUnitValidationError::CandidateOperandFactMismatch)
        );
        let foreign_obligation = PsiRewriteCandidate::new_proof_certified_scalar_identity(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            constant_fact,
            omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                b"foreign divide-one proof",
            ),
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_scalar_identity_candidate(&unit, &foreign_obligation),
            Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        );

        let mut wrong_literal = live_divide_by_one_unit(integer, make_exact);
        let O::IntegerConstant { value, .. } =
            &mut wrong_literal.functions[0].blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        *value = IntegerValue::Unsigned(0);
        let OptimizationFact::IntegerConstant { constant, .. } =
            &mut wrong_literal.functions[0].facts[0]
        else {
            unreachable!()
        };
        *constant = IntegerValue::Unsigned(0);
        wrong_literal.identity = recompute_psi_optimization_unit_identity(&wrong_literal);
        validate_psi_optimization_unit(&wrong_literal).unwrap();
        let mut manager = crate::AnalysisManager::new(&wrong_literal);
        let products = manager
            .require_all(&wrong_literal, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedIntegerDivideByOneEliminationRule
                .propose(&wrong_literal, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );

        let mut missing_proof = live_divide_by_one_unit(integer, make_exact);
        missing_proof.accepted_obligation_facts.clear();
        missing_proof.identity = recompute_psi_optimization_unit_identity(&missing_proof);
        let mut manager = crate::AnalysisManager::new(&missing_proof);
        let products = manager
            .require_all(&missing_proof, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedIntegerDivideByOneEliminationRule
                .propose(&missing_proof, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn proof_certified_exact_multiply_by_zero_covers_both_sides_and_integer_signs() {
        for integer in [
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            IntegerType::new(IntegerSign::Signed, 8).unwrap(),
        ] {
            for (zero_left, expected) in [
                (
                    true,
                    ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroLeft,
                ),
                (
                    false,
                    ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight,
                ),
            ] {
                let unit = live_exact_multiply_by_zero_unit(integer, zero_left);
                let contract =
                    LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule::contract();
                let mut manager = crate::AnalysisManager::new(&unit);
                let products = manager
                    .require_all(&unit, contract.required_analyses())
                    .unwrap()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                let [candidate] = LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
                    .propose(&unit, RuleAnalysisView::new(&products))
                    .unwrap()
                    .try_into()
                    .expect("each exact multiplication with one literal zero has one candidate");
                let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) =
                    candidate.patch()
                else {
                    unreachable!()
                };
                assert_eq!(patch.identity, expected);
                assert_eq!(patch.replacement, id(324, ValueId::new));
                assert_eq!(candidate.consumed_facts().len(), 2);
                let accepted =
                    validate_proof_certified_scalar_identity_candidate(&unit, &candidate).unwrap();
                assert_eq!(
                    accepted.validator(),
                    OptimizationValidatorIdentity::from_canonical_bytes(
                        b"omega.validator.live-proof-certified-exact-integer-multiply-by-zero-elimination.v1"
                    )
                );
                assert_eq!(
                    accepted.unit().accepted_obligation_facts,
                    unit.accepted_obligation_facts
                );
                assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
                    !matches!(fact, OptimizationFact::OperationObligationReference { .. })
                }));
                assert!(matches!(
                    accepted.unit().functions[0].blocks[0].nodes[1].operation,
                    O::Return { value, .. } if value == id(324, ValueId::new)
                ));
                assert_eq!(
                    accepted.unit().functions[0].blocks[0].nodes[1]
                        .provenance
                        .len(),
                    2
                );
                assert_eq!(
                    accepted.unit().functions[0].blocks[0].nodes[1].fuel.len(),
                    2
                );

                let mut manager = crate::AnalysisManager::new(accepted.unit());
                let products = manager
                    .require_all(accepted.unit(), contract.required_analyses())
                    .unwrap()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                assert!(
                    LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
                        .propose(accepted.unit(), RuleAnalysisView::new(&products))
                        .unwrap()
                        .is_empty()
                );
            }
        }
    }

    #[test]
    fn proof_certified_zero_dividend_covers_divide_remainder_policies_and_signs() {
        let rows = [
            ProofCertifiedScalarIdentityKind::ExactIntegerDivideZeroLeft,
            ProofCertifiedScalarIdentityKind::WrappingIntegerDivideZeroLeft,
            ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideZeroLeft,
            ProofCertifiedScalarIdentityKind::ExactIntegerRemainderZeroLeft,
            ProofCertifiedScalarIdentityKind::WrappingIntegerRemainderZeroLeft,
            ProofCertifiedScalarIdentityKind::SaturatingIntegerRemainderZeroLeft,
        ];
        for integer in [
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            IntegerType::new(IntegerSign::Signed, 8).unwrap(),
        ] {
            for (policy, expected) in rows.into_iter().enumerate() {
                let unit = live_zero_dividend_unit(
                    integer,
                    |psi_operation, obligation, result, scalar_type, left, right| match policy {
                        0 => O::ExactIntegerDivide {
                            psi_operation,
                            obligation,
                            result,
                            scalar_type,
                            left,
                            right,
                        },
                        1 => O::WrappingIntegerDivide {
                            psi_operation,
                            obligation,
                            result,
                            scalar_type,
                            left,
                            right,
                        },
                        2 => O::SaturatingIntegerDivide {
                            psi_operation,
                            obligation,
                            result,
                            scalar_type,
                            left,
                            right,
                        },
                        3 => O::ExactIntegerRemainder {
                            psi_operation,
                            obligation,
                            result,
                            scalar_type,
                            left,
                            right,
                        },
                        4 => O::WrappingIntegerRemainder {
                            psi_operation,
                            obligation,
                            result,
                            scalar_type,
                            left,
                            right,
                        },
                        5 => O::SaturatingIntegerRemainder {
                            psi_operation,
                            obligation,
                            result,
                            scalar_type,
                            left,
                            right,
                        },
                        _ => unreachable!(),
                    },
                );
                let contract = LiveProofCertifiedIntegerZeroDividendEliminationRule::contract();
                let mut manager = crate::AnalysisManager::new(&unit);
                let products = manager
                    .require_all(&unit, contract.required_analyses())
                    .unwrap()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                let [candidate] = LiveProofCertifiedIntegerZeroDividendEliminationRule
                    .propose(&unit, RuleAnalysisView::new(&products))
                    .unwrap()
                    .try_into()
                    .expect("each proof-certified zero dividend has one candidate");
                let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) =
                    candidate.patch()
                else {
                    unreachable!()
                };
                assert_eq!(patch.identity, expected);
                assert_eq!(patch.replacement, id(324, ValueId::new));
                assert_eq!(candidate.consumed_facts().len(), 2);
                assert!(
                    candidate
                        .consumed_facts()
                        .iter()
                        .any(|fact| matches!(fact, OptimizationFactReference::ScalarConstant(_)))
                );
                assert!(
                    candidate.consumed_facts().iter().any(|fact| matches!(
                        fact,
                        OptimizationFactReference::AcceptedObligation(_)
                    ))
                );

                let accepted =
                    validate_proof_certified_scalar_identity_candidate(&unit, &candidate).unwrap();
                assert_eq!(
                    accepted.validator(),
                    OptimizationValidatorIdentity::from_canonical_bytes(
                        b"omega.validator.live-proof-certified-integer-zero-dividend-elimination.v1"
                    )
                );
                assert_eq!(
                    accepted.unit().accepted_obligation_facts,
                    unit.accepted_obligation_facts
                );
                assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
                    !matches!(fact, OptimizationFact::OperationObligationReference { .. })
                }));
                assert!(matches!(
                    accepted.unit().functions[0].blocks[0].nodes[1].operation,
                    O::Return { value, .. } if value == id(324, ValueId::new)
                ));
                assert_eq!(
                    accepted.unit().functions[0].blocks[0].nodes[1]
                        .provenance
                        .len(),
                    2
                );
                assert_eq!(
                    accepted.unit().functions[0].blocks[0].nodes[1].fuel.len(),
                    2
                );

                let mut manager = crate::AnalysisManager::new(accepted.unit());
                let products = manager
                    .require_all(accepted.unit(), contract.required_analyses())
                    .unwrap()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                assert!(
                    LiveProofCertifiedIntegerZeroDividendEliminationRule
                        .propose(accepted.unit(), RuleAnalysisView::new(&products))
                        .unwrap()
                        .is_empty()
                );
            }
        }
    }

    #[test]
    fn proof_certified_zero_dividend_declines_ineligible_shapes_and_rejects_corruption() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let make_exact =
            |psi_operation, obligation, result, scalar_type, left, right| O::ExactIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            };
        let unit = live_zero_dividend_unit(integer, make_exact);
        let contract = LiveProofCertifiedIntegerZeroDividendEliminationRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = LiveProofCertifiedIntegerZeroDividendEliminationRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .unwrap();
        let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch()
        else {
            unreachable!()
        };
        let (constant_fact, obligation_fact) =
            candidate.proof_certified_scalar_identity_witness().unwrap();

        for identity in [
            ProofCertifiedScalarIdentityKind::WrappingIntegerDivideZeroLeft,
            ProofCertifiedScalarIdentityKind::ExactIntegerRemainderZeroLeft,
            ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight,
        ] {
            let forged = PsiRewriteCandidate::new_proof_certified_scalar_identity(
                unit.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                constant_fact,
                obligation_fact,
                candidate.predicted_cost_delta(),
                ProofCertifiedScalarIdentityRewrite { identity, ..patch },
            )
            .unwrap();
            assert_eq!(
                validate_proof_certified_scalar_identity_candidate(&unit, &forged),
                Err(OptimizationUnitValidationError::CandidatePatchMismatch)
            );
        }
        let wrong_replacement = PsiRewriteCandidate::new_proof_certified_scalar_identity(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            constant_fact,
            obligation_fact,
            candidate.predicted_cost_delta(),
            ProofCertifiedScalarIdentityRewrite {
                replacement: id(323, ValueId::new),
                ..patch
            },
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_scalar_identity_candidate(&unit, &wrong_replacement),
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        );
        let foreign_constant = PsiRewriteCandidate::new_proof_certified_scalar_identity(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            ScalarConstantFactIdentity::from_canonical_bytes(b"foreign zero-dividend literal"),
            obligation_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_scalar_identity_candidate(&unit, &foreign_constant),
            Err(OptimizationUnitValidationError::CandidateOperandFactMismatch)
        );
        let foreign_obligation = PsiRewriteCandidate::new_proof_certified_scalar_identity(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            constant_fact,
            omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                b"foreign zero-dividend proof",
            ),
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_scalar_identity_candidate(&unit, &foreign_obligation),
            Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        );

        let mut nonzero = live_zero_dividend_unit(integer, make_exact);
        let O::IntegerConstant { value, .. } =
            &mut nonzero.functions[0].blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        *value = IntegerValue::Unsigned(1);
        let OptimizationFact::IntegerConstant { constant, .. } = &mut nonzero.functions[0].facts[0]
        else {
            unreachable!()
        };
        *constant = IntegerValue::Unsigned(1);
        nonzero.identity = recompute_psi_optimization_unit_identity(&nonzero);
        let mut manager = crate::AnalysisManager::new(&nonzero);
        let products = manager
            .require_all(&nonzero, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedIntegerZeroDividendEliminationRule
                .propose(&nonzero, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );

        let mut missing_proof = live_zero_dividend_unit(integer, make_exact);
        missing_proof.accepted_obligation_facts.clear();
        missing_proof.identity = recompute_psi_optimization_unit_identity(&missing_proof);
        let mut manager = crate::AnalysisManager::new(&missing_proof);
        let products = manager
            .require_all(&missing_proof, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedIntegerZeroDividendEliminationRule
                .propose(&missing_proof, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );

        let mut missing_active_reference = live_zero_dividend_unit(integer, make_exact);
        missing_active_reference.functions[0]
            .facts
            .retain(|fact| !matches!(fact, OptimizationFact::OperationObligationReference { .. }));
        missing_active_reference.identity =
            recompute_psi_optimization_unit_identity(&missing_active_reference);
        let mut manager = crate::AnalysisManager::new(&missing_active_reference);
        let products = manager
            .require_all(&missing_active_reference, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedIntegerZeroDividendEliminationRule
                .propose(&missing_active_reference, RuleAnalysisView::new(&products),)
                .unwrap()
                .is_empty()
        );

        let dead = discard_scalar_function_result(live_zero_dividend_unit(integer, make_exact));
        let mut manager = crate::AnalysisManager::new(&dead);
        let products = manager
            .require_all(&dead, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedIntegerZeroDividendEliminationRule
                .propose(&dead, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );

        let right_zero =
            live_proof_binary_identity_unit(integer, IntegerValue::Unsigned(0), false, make_exact);
        let mut manager = crate::AnalysisManager::new(&right_zero);
        let products = manager
            .require_all(&right_zero, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedIntegerZeroDividendEliminationRule
                .propose(&right_zero, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn proof_certified_exact_zero_value_shift_covers_directions_and_integer_signs() {
        for integer in [
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            IntegerType::new(IntegerSign::Signed, 8).unwrap(),
        ] {
            for (left_shift, expected) in [
                (
                    true,
                    ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroValue,
                ),
                (
                    false,
                    ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroValue,
                ),
            ] {
                let unit = live_exact_zero_value_shift_unit(integer, left_shift);
                let contract =
                    LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule::contract();
                let mut manager = crate::AnalysisManager::new(&unit);
                let products = manager
                    .require_all(&unit, contract.required_analyses())
                    .unwrap()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                let [candidate] = LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule
                    .propose(&unit, RuleAnalysisView::new(&products))
                    .unwrap()
                    .try_into()
                    .expect("each live proof-certified zero-value shift has one candidate");
                let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) =
                    candidate.patch()
                else {
                    unreachable!()
                };
                assert_eq!(patch.identity, expected);
                assert_eq!(patch.replacement, id(324, ValueId::new));
                assert_eq!(candidate.consumed_facts().len(), 2);
                assert!(
                    candidate
                        .consumed_facts()
                        .iter()
                        .any(|fact| matches!(fact, OptimizationFactReference::ScalarConstant(_)))
                );
                assert!(
                    candidate.consumed_facts().iter().any(|fact| matches!(
                        fact,
                        OptimizationFactReference::AcceptedObligation(_)
                    ))
                );

                let accepted =
                    validate_proof_certified_scalar_identity_candidate(&unit, &candidate).unwrap();
                assert_eq!(
                    accepted.validator(),
                    OptimizationValidatorIdentity::from_canonical_bytes(
                        b"omega.validator.live-proof-certified-exact-integer-zero-value-shift-elimination.v1"
                    )
                );
                assert_eq!(
                    accepted.unit().accepted_obligation_facts,
                    unit.accepted_obligation_facts
                );
                assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
                    !matches!(fact, OptimizationFact::OperationObligationReference { .. })
                }));
                assert!(matches!(
                    accepted.unit().functions[0].blocks[0].nodes[1].operation,
                    O::Return { value, .. } if value == id(324, ValueId::new)
                ));
                assert_eq!(
                    accepted.unit().functions[0].blocks[0].nodes[1]
                        .provenance
                        .len(),
                    2
                );
                assert_eq!(
                    accepted.unit().functions[0].blocks[0].nodes[1].fuel.len(),
                    2
                );

                let mut manager = crate::AnalysisManager::new(accepted.unit());
                let products = manager
                    .require_all(accepted.unit(), contract.required_analyses())
                    .unwrap()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                assert!(
                    LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule
                        .propose(accepted.unit(), RuleAnalysisView::new(&products))
                        .unwrap()
                        .is_empty()
                );
            }
        }
    }

    #[test]
    fn proof_certified_exact_zero_value_shift_declines_and_rejects_corruption() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let unit = live_exact_zero_value_shift_unit(integer, true);
        let contract = LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .unwrap();
        let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch()
        else {
            unreachable!()
        };
        let (constant_fact, obligation_fact) =
            candidate.proof_certified_scalar_identity_witness().unwrap();

        for identity in [
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroValue,
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroCount,
            ProofCertifiedScalarIdentityKind::ExactIntegerDivideZeroLeft,
        ] {
            let forged = PsiRewriteCandidate::new_proof_certified_scalar_identity(
                unit.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                constant_fact,
                obligation_fact,
                candidate.predicted_cost_delta(),
                ProofCertifiedScalarIdentityRewrite { identity, ..patch },
            )
            .unwrap();
            assert_eq!(
                validate_proof_certified_scalar_identity_candidate(&unit, &forged),
                Err(OptimizationUnitValidationError::CandidatePatchMismatch)
            );
        }

        let wrong_replacement = PsiRewriteCandidate::new_proof_certified_scalar_identity(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            constant_fact,
            obligation_fact,
            candidate.predicted_cost_delta(),
            ProofCertifiedScalarIdentityRewrite {
                replacement: id(323, ValueId::new),
                ..patch
            },
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_scalar_identity_candidate(&unit, &wrong_replacement),
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        );
        let foreign_constant = PsiRewriteCandidate::new_proof_certified_scalar_identity(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            ScalarConstantFactIdentity::from_canonical_bytes(b"foreign zero-shift literal"),
            obligation_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_scalar_identity_candidate(&unit, &foreign_constant),
            Err(OptimizationUnitValidationError::CandidateOperandFactMismatch)
        );
        let foreign_obligation = PsiRewriteCandidate::new_proof_certified_scalar_identity(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            constant_fact,
            omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                b"foreign zero-shift proof",
            ),
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_scalar_identity_candidate(&unit, &foreign_obligation),
            Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        );
        let mut corrupted_provenance = candidate.provenance().to_vec();
        corrupted_provenance[0].fuel[0].units += 1;
        let corrupted_provenance = PsiRewriteCandidate::new_proof_certified_scalar_identity(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            corrupted_provenance,
            constant_fact,
            obligation_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_scalar_identity_candidate(&unit, &corrupted_provenance),
            Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
        );

        let mut nonzero = live_exact_zero_value_shift_unit(integer, true);
        let O::IntegerConstant { value, .. } =
            &mut nonzero.functions[0].blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        *value = IntegerValue::Unsigned(1);
        let OptimizationFact::IntegerConstant { constant, .. } = &mut nonzero.functions[0].facts[0]
        else {
            unreachable!()
        };
        *constant = IntegerValue::Unsigned(1);
        nonzero.identity = recompute_psi_optimization_unit_identity(&nonzero);
        let mut manager = crate::AnalysisManager::new(&nonzero);
        let products = manager
            .require_all(&nonzero, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule
                .propose(&nonzero, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );

        let mut missing_proof = live_exact_zero_value_shift_unit(integer, true);
        missing_proof.accepted_obligation_facts.clear();
        missing_proof.identity = recompute_psi_optimization_unit_identity(&missing_proof);
        let mut manager = crate::AnalysisManager::new(&missing_proof);
        let products = manager
            .require_all(&missing_proof, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule
                .propose(&missing_proof, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );

        let mut missing_active_reference = live_exact_zero_value_shift_unit(integer, true);
        missing_active_reference.functions[0]
            .facts
            .retain(|fact| !matches!(fact, OptimizationFact::OperationObligationReference { .. }));
        missing_active_reference.identity =
            recompute_psi_optimization_unit_identity(&missing_active_reference);
        let mut manager = crate::AnalysisManager::new(&missing_active_reference);
        let products = manager
            .require_all(&missing_active_reference, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule
                .propose(&missing_active_reference, RuleAnalysisView::new(&products),)
                .unwrap()
                .is_empty()
        );

        let dead = discard_scalar_function_result(live_exact_zero_value_shift_unit(integer, true));
        let mut manager = crate::AnalysisManager::new(&dead);
        let products = manager
            .require_all(&dead, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule
                .propose(&dead, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );

        let zero_count_only = live_proof_binary_identity_unit(
            integer,
            IntegerValue::Unsigned(0),
            false,
            |psi_operation, obligation, result, value_type, value, count| {
                O::ExactIntegerShiftLeft {
                    psi_operation,
                    obligation,
                    result,
                    value_type,
                    count_type: integer,
                    value,
                    count,
                }
            },
        );
        let mut manager = crate::AnalysisManager::new(&zero_count_only);
        let products = manager
            .require_all(&zero_count_only, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule
                .propose(&zero_count_only, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );

        let wrapping = live_proof_binary_identity_unit(
            integer,
            IntegerValue::Unsigned(0),
            true,
            |psi_operation, _obligation, result, value_type, value, count| {
                O::WrappingIntegerShiftLeft {
                    psi_operation,
                    result,
                    value_type,
                    count_type: integer,
                    value,
                    count,
                }
            },
        );
        let mut manager = crate::AnalysisManager::new(&wrapping);
        let products = manager
            .require_all(&wrapping, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule
                .propose(&wrapping, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn proof_certified_exact_self_subtract_materializes_typed_zero_with_exact_custody() {
        for integer in [
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            IntegerType::new(IntegerSign::Signed, 8).unwrap(),
        ] {
            let unit = live_exact_self_subtract_unit(integer);
            let contract = LiveProofCertifiedExactIntegerSelfSubtractEliminationRule::contract();
            let original_node = unit.functions[0].blocks[0].nodes[0].clone();
            let mut manager = crate::AnalysisManager::new(&unit);
            let products = manager
                .require_all(&unit, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let [candidate] = LiveProofCertifiedExactIntegerSelfSubtractEliminationRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .try_into()
                .expect("live exact self-subtract has one candidate");
            let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch()
            else {
                unreachable!()
            };
            assert_eq!(patch.location.node, 0);
            assert_eq!(patch.source_operation, id(335, OperationId::new));
            assert_eq!(patch.result, id(334, ValueId::new));
            assert_eq!(patch.scalar_type, integer);
            assert_eq!(patch.constant, integer_zero(integer));
            assert!(candidate.substitutions().is_empty());
            assert_eq!(candidate.affected_blocks(), [id(332, BlockId::new)]);
            assert_eq!(candidate.consumed_facts().len(), 1);
            assert!(matches!(
                candidate.consumed_facts()[0],
                OptimizationFactReference::AcceptedObligation(_)
            ));

            let accepted =
                validate_proof_certified_exact_integer_self_subtract_candidate(&unit, &candidate)
                    .unwrap();
            assert_eq!(
                accepted.validator(),
                OptimizationValidatorIdentity::from_canonical_bytes(
                    b"omega.validator.live-proof-certified-exact-integer-self-subtract-elimination.v1"
                )
            );
            assert_eq!(
                accepted.unit().accepted_obligation_facts,
                unit.accepted_obligation_facts
            );
            let output_node = &accepted.unit().functions[0].blocks[0].nodes[0];
            assert!(matches!(
                output_node.operation,
                O::IntegerConstant {
                    psi_operation,
                    result,
                    scalar_type: ScalarType::Integer(output_type),
                    value,
                } if psi_operation == id(335, OperationId::new)
                    && result == id(334, ValueId::new)
                    && output_type == integer
                    && value == integer_zero(integer)
            ));
            assert_eq!(output_node.provenance, original_node.provenance);
            assert_eq!(output_node.fuel, original_node.fuel);
            assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
                !matches!(fact, OptimizationFact::OperationObligationReference { .. })
            }));
            assert!(accepted.unit().functions[0].facts.iter().any(|fact| {
                matches!(
                    fact,
                    OptimizationFact::IntegerConstant { value, constant, support }
                        if *value == id(334, ValueId::new)
                            && *constant == integer_zero(integer)
                            && *support == id(335, OperationId::new)
                )
            }));

            let mut manager = crate::AnalysisManager::new(accepted.unit());
            let products = manager
                .require_all(accepted.unit(), contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            assert!(
                LiveProofCertifiedExactIntegerSelfSubtractEliminationRule
                    .propose(accepted.unit(), RuleAnalysisView::new(&products))
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn proof_certified_exact_self_subtract_declines_and_rejects_corruption() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let unit = live_exact_self_subtract_unit(integer);
        let contract = LiveProofCertifiedExactIntegerSelfSubtractEliminationRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = LiveProofCertifiedExactIntegerSelfSubtractEliminationRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .unwrap();
        let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch() else {
            unreachable!()
        };
        let obligation_fact = candidate.accepted_obligation_witness().unwrap();

        for forged_patch in [
            IntegerConstantRewrite {
                constant: IntegerValue::Unsigned(1),
                ..patch
            },
            IntegerConstantRewrite {
                source_operation: id(338, OperationId::new),
                ..patch
            },
            IntegerConstantRewrite {
                result: id(339, ValueId::new),
                ..patch
            },
            IntegerConstantRewrite {
                scalar_type: IntegerType::new(IntegerSign::Signed, 8).unwrap(),
                constant: IntegerValue::Signed(0),
                ..patch
            },
        ] {
            let forged = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
                unit.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                obligation_fact,
                candidate.predicted_cost_delta(),
                forged_patch,
            )
            .unwrap();
            assert!(
                validate_proof_certified_exact_integer_self_subtract_candidate(&unit, &forged)
                    .is_err()
            );
        }
        let foreign_fact = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                b"foreign self-subtract proof",
            ),
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_exact_integer_self_subtract_candidate(&unit, &foreign_fact),
            Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        );
        let mut corrupt_provenance = candidate.provenance().to_vec();
        corrupt_provenance[0].fuel[0].units += 1;
        let corrupt_provenance =
            PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
                unit.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                corrupt_provenance,
                obligation_fact,
                candidate.predicted_cost_delta(),
                patch,
            )
            .unwrap();
        assert_eq!(
            validate_proof_certified_exact_integer_self_subtract_candidate(
                &unit,
                &corrupt_provenance,
            ),
            Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
        );
        let substituted = PsiRewriteCandidate::new_integer_evaluation(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            vec![ScalarSubstitution {
                from: patch.result,
                to: id(333, ValueId::new),
                scalar_type: ScalarType::Integer(integer),
            }],
            candidate.provenance().to_vec(),
            IntegerEvaluationWitness::ProofCertifiedUnary {
                operand_fact: ScalarConstantFactIdentity::from_canonical_bytes(
                    b"foreign self-subtract operand",
                ),
                obligation_fact,
            },
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_exact_integer_self_subtract_candidate(&unit, &substituted),
            Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)
        );
        let foreign_contract = OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(b"foreign self-subtract rule"),
            OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
            1,
            contract.required_analyses(),
            contract.invalidated_analyses(),
            OptimizationSafetyClass::ProofCertified,
        )
        .unwrap();
        let foreign_rule = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
            unit.identity,
            foreign_contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            obligation_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_exact_integer_self_subtract_candidate(&unit, &foreign_rule),
            Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)
        );
        let terminal_location = NodeLocation {
            machine: patch.location.machine,
            block: patch.location.block,
            node: 1,
        };
        let terminal = &unit.functions[0].blocks[0].nodes[1];
        let terminal_site = PsiRealizationSite::Node(terminal_location);
        let wrong_location = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            vec![ProvenanceRewrite {
                input: terminal_site,
                disposition: ProvenanceDisposition::RealizedAt(terminal_site),
                sources: terminal.provenance.clone(),
                fuel: terminal.fuel.clone(),
            }],
            obligation_fact,
            candidate.predicted_cost_delta(),
            IntegerConstantRewrite {
                location: terminal_location,
                ..patch
            },
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_exact_integer_self_subtract_candidate(&unit, &wrong_location),
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        );

        let mut missing_catalog = unit.clone();
        missing_catalog.accepted_obligation_facts.clear();
        missing_catalog.identity = recompute_psi_optimization_unit_identity(&missing_catalog);
        let missing_catalog_candidate =
            PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
                missing_catalog.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                obligation_fact,
                candidate.predicted_cost_delta(),
                patch,
            )
            .unwrap();
        assert_eq!(
            validate_proof_certified_exact_integer_self_subtract_candidate(
                &missing_catalog,
                &missing_catalog_candidate,
            ),
            Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        );

        let mut wrong_policy = unit.clone();
        wrong_policy.functions[0].blocks[0].nodes[0].operation = O::WrappingIntegerSubtract {
            psi_operation: patch.source_operation,
            result: patch.result,
            scalar_type: integer,
            left: id(333, ValueId::new),
            right: id(333, ValueId::new),
        };
        wrong_policy.functions[0]
            .facts
            .retain(|fact| !matches!(fact, OptimizationFact::OperationObligationReference { .. }));
        wrong_policy.identity = recompute_psi_optimization_unit_identity(&wrong_policy);
        validate_psi_optimization_unit(&wrong_policy).unwrap();
        let wrong_policy_candidate =
            PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
                wrong_policy.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                obligation_fact,
                candidate.predicted_cost_delta(),
                patch,
            )
            .unwrap();
        assert_eq!(
            validate_proof_certified_exact_integer_self_subtract_candidate(
                &wrong_policy,
                &wrong_policy_candidate,
            ),
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        );

        let unequal = live_proof_binary_identity_unit(
            integer,
            IntegerValue::Unsigned(1),
            false,
            |psi_operation, obligation, result, scalar_type, left, right| O::ExactIntegerSubtract {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
        );
        for ineligible in [
            unequal,
            live_proof_binary_identity_unit(
                integer,
                IntegerValue::Unsigned(1),
                false,
                |psi_operation, _obligation, result, scalar_type, left, right| {
                    O::WrappingIntegerSubtract {
                        psi_operation,
                        result,
                        scalar_type,
                        left,
                        right,
                    }
                },
            ),
            live_proof_binary_identity_unit(
                integer,
                IntegerValue::Unsigned(1),
                false,
                |psi_operation, _obligation, result, scalar_type, left, right| {
                    O::SaturatingIntegerSubtract {
                        psi_operation,
                        result,
                        scalar_type,
                        left,
                        right,
                    }
                },
            ),
            discard_scalar_function_result(live_exact_self_subtract_unit(integer)),
        ] {
            let mut manager = crate::AnalysisManager::new(&ineligible);
            let products = manager
                .require_all(&ineligible, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            assert!(
                LiveProofCertifiedExactIntegerSelfSubtractEliminationRule
                    .propose(&ineligible, RuleAnalysisView::new(&products))
                    .unwrap()
                    .is_empty()
            );
        }

        for mut missing in [
            live_exact_self_subtract_unit(integer),
            live_exact_self_subtract_unit(integer),
        ]
        .into_iter()
        .enumerate()
        {
            if missing.0 == 0 {
                missing.1.accepted_obligation_facts.clear();
            } else {
                missing.1.functions[0].facts.retain(|fact| {
                    !matches!(fact, OptimizationFact::OperationObligationReference { .. })
                });
            }
            missing.1.identity = recompute_psi_optimization_unit_identity(&missing.1);
            let mut manager = crate::AnalysisManager::new(&missing.1);
            let products = manager
                .require_all(&missing.1, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            assert!(
                LiveProofCertifiedExactIntegerSelfSubtractEliminationRule
                    .propose(&missing.1, RuleAnalysisView::new(&products))
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn proof_certified_self_remainder_materializes_typed_zero_for_every_policy_and_sign() {
        for integer in [
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            IntegerType::new(IntegerSign::Signed, 8).unwrap(),
        ] {
            for policy in [
                SelfRemainderPolicy::Exact,
                SelfRemainderPolicy::Wrapping,
                SelfRemainderPolicy::Saturating,
            ] {
                let unit = live_self_remainder_unit(integer, policy);
                let contract = LiveProofCertifiedIntegerSelfRemainderEliminationRule::contract();
                let original_node = unit.functions[0].blocks[0].nodes[0].clone();
                let mut manager = crate::AnalysisManager::new(&unit);
                let products = manager
                    .require_all(&unit, contract.required_analyses())
                    .unwrap()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                let [candidate] = LiveProofCertifiedIntegerSelfRemainderEliminationRule
                    .propose(&unit, RuleAnalysisView::new(&products))
                    .unwrap()
                    .try_into()
                    .expect("one live same-operand remainder candidate");
                let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch()
                else {
                    unreachable!()
                };
                assert_eq!(
                    patch.location,
                    NodeLocation {
                        machine: id(341, MachineId::new),
                        block: id(342, BlockId::new),
                        node: 0,
                    }
                );
                assert_eq!(patch.source_operation, id(345, OperationId::new));
                assert_eq!(patch.result, id(344, ValueId::new));
                assert_eq!(patch.scalar_type, integer);
                assert_eq!(patch.constant, integer_zero(integer));
                assert_eq!(candidate.predicted_cost_delta(), -1);
                assert!(candidate.substitutions().is_empty());
                assert_eq!(candidate.affected_blocks(), [id(342, BlockId::new)]);
                assert_eq!(candidate.consumed_facts().len(), 1);
                assert!(matches!(
                    candidate.consumed_facts()[0],
                    OptimizationFactReference::AcceptedObligation(_)
                ));

                let accepted =
                    validate_proof_certified_integer_self_remainder_candidate(&unit, &candidate)
                        .unwrap();
                assert_eq!(
                    accepted.validator(),
                    OptimizationValidatorIdentity::from_canonical_bytes(
                        b"omega.validator.live-proof-certified-integer-self-remainder-elimination.v1"
                    )
                );
                assert_eq!(
                    accepted.unit().accepted_obligation_facts,
                    unit.accepted_obligation_facts
                );
                let output_node = &accepted.unit().functions[0].blocks[0].nodes[0];
                assert!(matches!(
                    output_node.operation,
                    O::IntegerConstant {
                        psi_operation,
                        result,
                        scalar_type: ScalarType::Integer(output_type),
                        value,
                    } if psi_operation == id(345, OperationId::new)
                        && result == id(344, ValueId::new)
                        && output_type == integer
                        && value == integer_zero(integer)
                ));
                assert_eq!(output_node.provenance, original_node.provenance);
                assert_eq!(output_node.fuel, original_node.fuel);
                assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
                    !matches!(fact, OptimizationFact::OperationObligationReference { .. })
                }));
                assert!(accepted.unit().functions[0].facts.iter().any(|fact| {
                    matches!(
                        fact,
                        OptimizationFact::IntegerConstant { value, constant, support }
                            if *value == id(344, ValueId::new)
                                && *constant == integer_zero(integer)
                                && *support == id(345, OperationId::new)
                    )
                }));

                let mut manager = crate::AnalysisManager::new(accepted.unit());
                let products = manager
                    .require_all(accepted.unit(), contract.required_analyses())
                    .unwrap()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                assert!(
                    LiveProofCertifiedIntegerSelfRemainderEliminationRule
                        .propose(accepted.unit(), RuleAnalysisView::new(&products))
                        .unwrap()
                        .is_empty()
                );
            }
        }
    }

    #[test]
    fn proof_certified_self_remainder_declines_and_rejects_corruption() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let unit = live_self_remainder_unit(integer, SelfRemainderPolicy::Exact);
        let contract = LiveProofCertifiedIntegerSelfRemainderEliminationRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = LiveProofCertifiedIntegerSelfRemainderEliminationRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .unwrap();
        let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch() else {
            unreachable!()
        };
        let obligation_fact = candidate.accepted_obligation_witness().unwrap();

        for forged_patch in [
            IntegerConstantRewrite {
                constant: IntegerValue::Unsigned(1),
                ..patch
            },
            IntegerConstantRewrite {
                source_operation: id(348, OperationId::new),
                ..patch
            },
            IntegerConstantRewrite {
                result: id(349, ValueId::new),
                ..patch
            },
            IntegerConstantRewrite {
                scalar_type: IntegerType::new(IntegerSign::Signed, 8).unwrap(),
                constant: IntegerValue::Signed(0),
                ..patch
            },
        ] {
            let forged = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
                unit.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                obligation_fact,
                candidate.predicted_cost_delta(),
                forged_patch,
            )
            .unwrap();
            assert!(
                validate_proof_certified_integer_self_remainder_candidate(&unit, &forged).is_err()
            );
        }

        let foreign_fact = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                b"foreign self-remainder proof",
            ),
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_integer_self_remainder_candidate(&unit, &foreign_fact),
            Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        );

        for corrupt_provenance in [
            {
                let mut rows = candidate.provenance().to_vec();
                rows[0].fuel[0].units += 1;
                rows
            },
            {
                let mut rows = candidate.provenance().to_vec();
                rows[0].sources[0] = PsiProvenance::Operation(id(348, OperationId::new));
                rows[0].fuel[0].site = rows[0].sources[0];
                rows
            },
        ] {
            let forged = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
                unit.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                corrupt_provenance,
                obligation_fact,
                candidate.predicted_cost_delta(),
                patch,
            )
            .unwrap();
            assert_eq!(
                validate_proof_certified_integer_self_remainder_candidate(&unit, &forged),
                Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
            );
        }

        let foreign_contract = OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(b"foreign self-remainder rule"),
            OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
            1,
            contract.required_analyses(),
            contract.invalidated_analyses(),
            OptimizationSafetyClass::ProofCertified,
        )
        .unwrap();
        let foreign_rule = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
            unit.identity,
            foreign_contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            obligation_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_integer_self_remainder_candidate(&unit, &foreign_rule),
            Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)
        );

        let terminal_location = NodeLocation {
            machine: patch.location.machine,
            block: patch.location.block,
            node: 1,
        };
        let terminal = &unit.functions[0].blocks[0].nodes[1];
        let terminal_site = PsiRealizationSite::Node(terminal_location);
        assert!(
            PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
                unit.identity,
                contract,
                vec![id(350, BlockId::new)],
                candidate.provenance().to_vec(),
                obligation_fact,
                candidate.predicted_cost_delta(),
                patch,
            )
            .is_err()
        );
        let wrong_location = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            vec![ProvenanceRewrite {
                input: terminal_site,
                disposition: ProvenanceDisposition::RealizedAt(terminal_site),
                sources: terminal.provenance.clone(),
                fuel: terminal.fuel.clone(),
            }],
            obligation_fact,
            candidate.predicted_cost_delta(),
            IntegerConstantRewrite {
                location: terminal_location,
                ..patch
            },
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_integer_self_remainder_candidate(&unit, &wrong_location),
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        );

        let substituted = PsiRewriteCandidate::new_integer_evaluation(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            vec![ScalarSubstitution {
                from: patch.result,
                to: id(343, ValueId::new),
                scalar_type: ScalarType::Integer(integer),
            }],
            candidate.provenance().to_vec(),
            IntegerEvaluationWitness::ProofCertifiedUnary {
                operand_fact: ScalarConstantFactIdentity::from_canonical_bytes(
                    b"foreign self-remainder operand",
                ),
                obligation_fact,
            },
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_integer_self_remainder_candidate(&unit, &substituted),
            Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)
        );

        let mut missing_catalog = unit.clone();
        missing_catalog.accepted_obligation_facts.clear();
        missing_catalog.identity = recompute_psi_optimization_unit_identity(&missing_catalog);
        let missing_catalog_candidate =
            PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
                missing_catalog.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                obligation_fact,
                candidate.predicted_cost_delta(),
                patch,
            )
            .unwrap();
        assert_eq!(
            validate_proof_certified_integer_self_remainder_candidate(
                &missing_catalog,
                &missing_catalog_candidate,
            ),
            Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        );

        let unequal = live_proof_binary_identity_unit(
            integer,
            IntegerValue::Unsigned(1),
            false,
            |psi_operation, obligation, result, scalar_type, left, right| {
                O::ExactIntegerRemainder {
                    psi_operation,
                    obligation,
                    result,
                    scalar_type,
                    left,
                    right,
                }
            },
        );
        let mut foreign_operation = unit.clone();
        foreign_operation.functions[0].blocks[0].nodes[0].operation = O::ExactIntegerDivide {
            psi_operation: patch.source_operation,
            obligation: id(346, ObligationId::new),
            result: patch.result,
            scalar_type: integer,
            left: id(343, ValueId::new),
            right: id(343, ValueId::new),
        };
        foreign_operation.identity = recompute_psi_optimization_unit_identity(&foreign_operation);
        validate_psi_optimization_unit(&foreign_operation).unwrap();
        let foreign_operation_candidate =
            PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
                foreign_operation.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                obligation_fact,
                candidate.predicted_cost_delta(),
                patch,
            )
            .unwrap();
        assert_eq!(
            validate_proof_certified_integer_self_remainder_candidate(
                &foreign_operation,
                &foreign_operation_candidate,
            ),
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        );

        for ineligible in [
            unequal,
            discard_scalar_function_result(live_self_remainder_unit(
                integer,
                SelfRemainderPolicy::Exact,
            )),
        ] {
            let mut manager = crate::AnalysisManager::new(&ineligible);
            let products = manager
                .require_all(&ineligible, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            assert!(
                LiveProofCertifiedIntegerSelfRemainderEliminationRule
                    .propose(&ineligible, RuleAnalysisView::new(&products))
                    .unwrap()
                    .is_empty()
            );
        }

        for (remove_catalog, mut missing) in [
            (
                true,
                live_self_remainder_unit(integer, SelfRemainderPolicy::Exact),
            ),
            (
                false,
                live_self_remainder_unit(integer, SelfRemainderPolicy::Exact),
            ),
        ] {
            if remove_catalog {
                missing.accepted_obligation_facts.clear();
            } else {
                missing.functions[0].facts.retain(|fact| {
                    !matches!(fact, OptimizationFact::OperationObligationReference { .. })
                });
            }
            missing.identity = recompute_psi_optimization_unit_identity(&missing);
            let mut manager = crate::AnalysisManager::new(&missing);
            let products = manager
                .require_all(&missing, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            assert!(
                LiveProofCertifiedIntegerSelfRemainderEliminationRule
                    .propose(&missing, RuleAnalysisView::new(&products))
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn proof_certified_self_divide_materializes_typed_one_for_every_policy_and_sign() {
        for integer in [
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            IntegerType::new(IntegerSign::Signed, 8).unwrap(),
        ] {
            for policy in [
                SelfDividePolicy::Exact,
                SelfDividePolicy::Wrapping,
                SelfDividePolicy::Saturating,
            ] {
                let unit = live_self_divide_unit(integer, policy);
                let contract = LiveProofCertifiedIntegerSelfDivideEliminationRule::contract();
                let original_node = unit.functions[0].blocks[0].nodes[0].clone();
                let accepted_catalog = unit.accepted_obligation_facts.clone();
                let mut manager = crate::AnalysisManager::new(&unit);
                let products = manager
                    .require_all(&unit, contract.required_analyses())
                    .unwrap()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                let [candidate] = LiveProofCertifiedIntegerSelfDivideEliminationRule
                    .propose(&unit, RuleAnalysisView::new(&products))
                    .unwrap()
                    .try_into()
                    .expect("one live same-operand division candidate");
                let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch()
                else {
                    unreachable!()
                };
                assert_eq!(
                    patch.location,
                    NodeLocation {
                        machine: id(351, MachineId::new),
                        block: id(352, BlockId::new),
                        node: 0,
                    }
                );
                assert_eq!(patch.source_operation, id(355, OperationId::new));
                assert_eq!(patch.result, id(354, ValueId::new));
                assert_eq!(patch.scalar_type, integer);
                assert_eq!(patch.constant, integer_one(integer));
                assert_eq!(candidate.predicted_cost_delta(), -1);
                assert!(candidate.substitutions().is_empty());
                assert_eq!(candidate.affected_blocks(), [id(352, BlockId::new)]);
                assert_eq!(candidate.consumed_facts().len(), 1);
                assert!(matches!(
                    candidate.consumed_facts()[0],
                    OptimizationFactReference::AcceptedObligation(_)
                ));

                let accepted =
                    validate_proof_certified_integer_self_divide_candidate(&unit, &candidate)
                        .unwrap();
                assert_eq!(
                    accepted.validator(),
                    OptimizationValidatorIdentity::from_canonical_bytes(
                        b"omega.validator.live-proof-certified-integer-self-divide-elimination.v1"
                    )
                );
                assert_eq!(accepted.unit().accepted_obligation_facts, accepted_catalog);
                let output_node = &accepted.unit().functions[0].blocks[0].nodes[0];
                assert!(matches!(
                    output_node.operation,
                    O::IntegerConstant {
                        psi_operation,
                        result,
                        scalar_type: ScalarType::Integer(output_type),
                        value,
                    } if psi_operation == id(355, OperationId::new)
                        && result == id(354, ValueId::new)
                        && output_type == integer
                        && value == integer_one(integer)
                ));
                assert_eq!(output_node.provenance, original_node.provenance);
                assert_eq!(output_node.fuel, original_node.fuel);
                assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
                    !matches!(fact, OptimizationFact::OperationObligationReference { .. })
                }));
                assert!(accepted.unit().functions[0].facts.iter().any(|fact| {
                    matches!(
                        fact,
                        OptimizationFact::IntegerConstant { value, constant, support }
                            if *value == id(354, ValueId::new)
                                && *constant == integer_one(integer)
                                && *support == id(355, OperationId::new)
                    )
                }));

                let mut manager = crate::AnalysisManager::new(accepted.unit());
                let products = manager
                    .require_all(accepted.unit(), contract.required_analyses())
                    .unwrap()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                assert!(
                    LiveProofCertifiedIntegerSelfDivideEliminationRule
                        .propose(accepted.unit(), RuleAnalysisView::new(&products))
                        .unwrap()
                        .is_empty()
                );
            }
        }
    }

    #[test]
    fn proof_certified_self_divide_declines_ineligible_shapes_and_rejects_corruption() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let unit = live_self_divide_unit(integer, SelfDividePolicy::Exact);
        let contract = LiveProofCertifiedIntegerSelfDivideEliminationRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = LiveProofCertifiedIntegerSelfDivideEliminationRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .unwrap();
        let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch() else {
            unreachable!()
        };
        let obligation_fact = candidate.accepted_obligation_witness().unwrap();

        for forged_patch in [
            IntegerConstantRewrite {
                constant: IntegerValue::Unsigned(0),
                ..patch
            },
            IntegerConstantRewrite {
                source_operation: id(358, OperationId::new),
                ..patch
            },
            IntegerConstantRewrite {
                result: id(359, ValueId::new),
                ..patch
            },
            IntegerConstantRewrite {
                scalar_type: IntegerType::new(IntegerSign::Signed, 8).unwrap(),
                constant: IntegerValue::Signed(1),
                ..patch
            },
        ] {
            let forged = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
                unit.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                obligation_fact,
                candidate.predicted_cost_delta(),
                forged_patch,
            )
            .unwrap();
            assert!(
                validate_proof_certified_integer_self_divide_candidate(&unit, &forged).is_err()
            );
        }

        let foreign_fact = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                b"foreign self-divide proof",
            ),
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_integer_self_divide_candidate(&unit, &foreign_fact),
            Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        );

        let forged_cost = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            obligation_fact,
            0,
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_integer_self_divide_candidate(&unit, &forged_cost),
            Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)
        );

        let mut corrupt_provenance = candidate.provenance().to_vec();
        corrupt_provenance[0].fuel[0].units += 1;
        let forged = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            corrupt_provenance,
            obligation_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_integer_self_divide_candidate(&unit, &forged),
            Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
        );

        let unequal = live_proof_binary_identity_unit(
            integer,
            IntegerValue::Unsigned(1),
            false,
            |psi_operation, obligation, result, scalar_type, left, right| O::ExactIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
        );
        let signed_one_bit = live_self_divide_unit(
            IntegerType::new(IntegerSign::Signed, 1).unwrap(),
            SelfDividePolicy::Exact,
        );
        let address =
            live_self_divide_unit(IntegerType::address(64).unwrap(), SelfDividePolicy::Exact);
        for ineligible in [
            unequal,
            signed_one_bit,
            address,
            discard_scalar_function_result(live_self_divide_unit(integer, SelfDividePolicy::Exact)),
        ] {
            let mut manager = crate::AnalysisManager::new(&ineligible);
            let products = manager
                .require_all(&ineligible, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            assert!(
                LiveProofCertifiedIntegerSelfDivideEliminationRule
                    .propose(&ineligible, RuleAnalysisView::new(&products))
                    .unwrap()
                    .is_empty()
            );
        }

        for (remove_catalog, mut missing) in [
            (
                true,
                live_self_divide_unit(integer, SelfDividePolicy::Exact),
            ),
            (
                false,
                live_self_divide_unit(integer, SelfDividePolicy::Exact),
            ),
        ] {
            if remove_catalog {
                missing.accepted_obligation_facts.clear();
            } else {
                missing.functions[0].facts.retain(|fact| {
                    !matches!(fact, OptimizationFact::OperationObligationReference { .. })
                });
            }
            missing.identity = recompute_psi_optimization_unit_identity(&missing);
            let mut manager = crate::AnalysisManager::new(&missing);
            let products = manager
                .require_all(&missing, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            assert!(
                LiveProofCertifiedIntegerSelfDivideEliminationRule
                    .propose(&missing, RuleAnalysisView::new(&products))
                    .unwrap()
                    .is_empty()
            );
        }

        let signed_one_bit = live_self_divide_unit(
            IntegerType::new(IntegerSign::Signed, 1).unwrap(),
            SelfDividePolicy::Exact,
        );
        let node = &signed_one_bit.functions[0].blocks[0].nodes[0];
        let signed_one_bit_patch = IntegerConstantRewrite {
            location: NodeLocation {
                machine: signed_one_bit.functions[0].machine,
                block: signed_one_bit.functions[0].blocks[0].id,
                node: 0,
            },
            source_operation: id(355, OperationId::new),
            result: id(354, ValueId::new),
            scalar_type: IntegerType::new(IntegerSign::Signed, 1).unwrap(),
            constant: IntegerValue::Signed(1),
        };
        let site = PsiRealizationSite::Node(signed_one_bit_patch.location);
        let forged_signed_one_bit =
            PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
                signed_one_bit.identity,
                contract,
                vec![signed_one_bit_patch.location.block],
                vec![ProvenanceRewrite {
                    input: site,
                    disposition: ProvenanceDisposition::RealizedAt(site),
                    sources: node.provenance.clone(),
                    fuel: node.fuel.clone(),
                }],
                signed_one_bit.accepted_obligation_facts[0].identity,
                -1,
                signed_one_bit_patch,
            )
            .unwrap();
        assert_eq!(
            validate_proof_certified_integer_self_divide_candidate(
                &signed_one_bit,
                &forged_signed_one_bit,
            ),
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        );
    }

    #[test]
    fn proof_certified_remainder_by_one_materializes_typed_zero_for_every_policy_and_sign() {
        for integer in [
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            IntegerType::new(IntegerSign::Signed, 8).unwrap(),
        ] {
            for policy in [
                SelfRemainderPolicy::Exact,
                SelfRemainderPolicy::Wrapping,
                SelfRemainderPolicy::Saturating,
            ] {
                let unit = live_remainder_by_one_unit(integer, policy);
                let contract = LiveProofCertifiedIntegerRemainderByOneEliminationRule::contract();
                let original_node = unit.functions[0].blocks[0].nodes[1].clone();
                let accepted_catalog = unit.accepted_obligation_facts.clone();
                let mut manager = crate::AnalysisManager::new(&unit);
                let products = manager
                    .require_all(&unit, contract.required_analyses())
                    .unwrap()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                let [candidate] = LiveProofCertifiedIntegerRemainderByOneEliminationRule
                    .propose(&unit, RuleAnalysisView::new(&products))
                    .unwrap()
                    .try_into()
                    .expect("one live remainder-by-one candidate");
                let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch()
                else {
                    unreachable!()
                };
                assert_eq!(patch.location.machine, id(321, MachineId::new));
                assert_eq!(patch.location.block, id(322, BlockId::new));
                assert_eq!(patch.location.node, 1);
                assert_eq!(patch.source_operation, id(327, OperationId::new));
                assert_eq!(patch.result, id(325, ValueId::new));
                assert_eq!(patch.scalar_type, integer);
                assert_eq!(patch.constant, integer_zero(integer));
                assert_eq!(candidate.predicted_cost_delta(), -1);
                assert!(candidate.substitutions().is_empty());
                assert_eq!(candidate.affected_blocks(), [id(322, BlockId::new)]);
                assert_eq!(candidate.consumed_facts().len(), 2);
                assert!(
                    candidate
                        .consumed_facts()
                        .iter()
                        .any(|fact| matches!(fact, OptimizationFactReference::ScalarConstant(_)))
                );
                assert!(
                    candidate.consumed_facts().iter().any(|fact| matches!(
                        fact,
                        OptimizationFactReference::AcceptedObligation(_)
                    ))
                );

                let accepted =
                    validate_proof_certified_integer_remainder_by_one_candidate(&unit, &candidate)
                        .unwrap();
                assert_eq!(
                    accepted.validator(),
                    OptimizationValidatorIdentity::from_canonical_bytes(
                        b"omega.validator.live-proof-certified-integer-remainder-by-one-elimination.v1"
                    )
                );
                assert_eq!(accepted.unit().accepted_obligation_facts, accepted_catalog);
                let output_node = &accepted.unit().functions[0].blocks[0].nodes[1];
                assert!(matches!(
                    output_node.operation,
                    O::IntegerConstant {
                        psi_operation,
                        result,
                        scalar_type: ScalarType::Integer(output_type),
                        value,
                    } if psi_operation == id(327, OperationId::new)
                        && result == id(325, ValueId::new)
                        && output_type == integer
                        && value == integer_zero(integer)
                ));
                assert_eq!(output_node.provenance, original_node.provenance);
                assert_eq!(output_node.fuel, original_node.fuel);
                assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
                    !matches!(fact, OptimizationFact::OperationObligationReference { .. })
                }));
                assert!(accepted.unit().functions[0].facts.iter().any(|fact| {
                    matches!(
                        fact,
                        OptimizationFact::IntegerConstant { value, constant, support }
                            if *value == id(325, ValueId::new)
                                && *constant == integer_zero(integer)
                                && *support == id(327, OperationId::new)
                    )
                }));

                let mut manager = crate::AnalysisManager::new(accepted.unit());
                let products = manager
                    .require_all(accepted.unit(), contract.required_analyses())
                    .unwrap()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                assert!(
                    LiveProofCertifiedIntegerRemainderByOneEliminationRule
                        .propose(accepted.unit(), RuleAnalysisView::new(&products))
                        .unwrap()
                        .is_empty()
                );
            }
        }
    }

    #[test]
    fn proof_certified_remainder_by_one_declines_closed_shapes_and_rejects_corruption() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let contract = LiveProofCertifiedIntegerRemainderByOneEliminationRule::contract();
        let unit = live_remainder_by_one_unit(integer, SelfRemainderPolicy::Exact);
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = LiveProofCertifiedIntegerRemainderByOneEliminationRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .unwrap();
        let mut propagated_products = products.clone();
        let AnalysisProduct::ScalarConstants(constants) = propagated_products
            .iter_mut()
            .find(|product| product.kind() == AnalysisKind::ScalarConstants)
            .unwrap()
        else {
            unreachable!()
        };
        constants
            .facts
            .iter_mut()
            .find(|fact| fact.value == id(324, ValueId::new))
            .unwrap()
            .support
            .edges
            .push(id(329, EdgeId::new));
        assert!(
            LiveProofCertifiedIntegerRemainderByOneEliminationRule
                .propose(&unit, RuleAnalysisView::new(&propagated_products))
                .unwrap()
                .is_empty(),
            "a propagated-one fact is not a direct literal witness"
        );
        let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch() else {
            unreachable!()
        };
        let (constant_fact, obligation_fact) =
            candidate.proof_certified_scalar_identity_witness().unwrap();

        for forged_patch in [
            IntegerConstantRewrite {
                constant: IntegerValue::Unsigned(1),
                ..patch
            },
            IntegerConstantRewrite {
                source_operation: id(330, OperationId::new),
                ..patch
            },
            IntegerConstantRewrite {
                result: id(330, ValueId::new),
                ..patch
            },
            IntegerConstantRewrite {
                scalar_type: IntegerType::new(IntegerSign::Signed, 8).unwrap(),
                constant: IntegerValue::Signed(0),
                ..patch
            },
        ] {
            let forged =
                PsiRewriteCandidate::new_literal_proof_certified_integer_constant_replacement(
                    unit.identity,
                    contract,
                    candidate.affected_blocks().to_vec(),
                    candidate.provenance().to_vec(),
                    constant_fact,
                    obligation_fact,
                    candidate.predicted_cost_delta(),
                    forged_patch,
                )
                .unwrap();
            assert!(
                validate_proof_certified_integer_remainder_by_one_candidate(&unit, &forged)
                    .is_err()
            );
        }

        for (bad_constant, bad_obligation, expected) in [
            (
                ScalarConstantFactIdentity::from_canonical_bytes(b"foreign remainder-one literal"),
                obligation_fact,
                OptimizationUnitValidationError::CandidateOperandFactMismatch,
            ),
            (
                constant_fact,
                omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                    b"foreign remainder-one proof",
                ),
                OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
            ),
        ] {
            let forged =
                PsiRewriteCandidate::new_literal_proof_certified_integer_constant_replacement(
                    unit.identity,
                    contract,
                    candidate.affected_blocks().to_vec(),
                    candidate.provenance().to_vec(),
                    bad_constant,
                    bad_obligation,
                    candidate.predicted_cost_delta(),
                    patch,
                )
                .unwrap();
            assert_eq!(
                validate_proof_certified_integer_remainder_by_one_candidate(&unit, &forged),
                Err(expected)
            );
        }

        let mut corrupt_provenance = candidate.provenance().to_vec();
        corrupt_provenance[0].fuel[0].units += 1;
        let forged = PsiRewriteCandidate::new_literal_proof_certified_integer_constant_replacement(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            corrupt_provenance,
            constant_fact,
            obligation_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_integer_remainder_by_one_candidate(&unit, &forged),
            Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
        );

        let mut foreign_operation = unit.clone();
        foreign_operation.functions[0].blocks[0].nodes[1].operation = O::ExactIntegerDivide {
            psi_operation: id(327, OperationId::new),
            obligation: id(328, ObligationId::new),
            result: id(325, ValueId::new),
            scalar_type: integer,
            left: id(323, ValueId::new),
            right: id(324, ValueId::new),
        };
        foreign_operation.identity = recompute_psi_optimization_unit_identity(&foreign_operation);
        validate_psi_optimization_unit(&foreign_operation).unwrap();
        let foreign_candidate =
            PsiRewriteCandidate::new_literal_proof_certified_integer_constant_replacement(
                foreign_operation.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                constant_fact,
                obligation_fact,
                candidate.predicted_cost_delta(),
                patch,
            )
            .unwrap();
        assert_eq!(
            validate_proof_certified_integer_remainder_by_one_candidate(
                &foreign_operation,
                &foreign_candidate,
            ),
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        );

        let mut non_one = unit.clone();
        let O::IntegerConstant { value, .. } =
            &mut non_one.functions[0].blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        *value = IntegerValue::Unsigned(2);
        for fact in &mut non_one.functions[0].facts {
            if let OptimizationFact::IntegerConstant {
                value, constant, ..
            } = fact
                && *value == id(324, ValueId::new)
            {
                *constant = IntegerValue::Unsigned(2);
            }
        }
        non_one.identity = recompute_psi_optimization_unit_identity(&non_one);
        validate_psi_optimization_unit(&non_one).unwrap();

        let mut missing_reference = unit.clone();
        missing_reference.functions[0]
            .facts
            .retain(|fact| !matches!(fact, OptimizationFact::OperationObligationReference { .. }));
        missing_reference.identity = recompute_psi_optimization_unit_identity(&missing_reference);
        let mut missing_catalog = unit.clone();
        missing_catalog.accepted_obligation_facts.clear();
        missing_catalog.identity = recompute_psi_optimization_unit_identity(&missing_catalog);
        for ineligible in [
            non_one,
            missing_reference,
            missing_catalog,
            discard_scalar_function_result(unit.clone()),
            live_self_remainder_unit(integer, SelfRemainderPolicy::Exact),
        ] {
            let mut manager = crate::AnalysisManager::new(&ineligible);
            let products = manager
                .require_all(&ineligible, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            assert!(
                LiveProofCertifiedIntegerRemainderByOneEliminationRule
                    .propose(&ineligible, RuleAnalysisView::new(&products))
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn proof_certified_signed_remainder_by_negative_one_materializes_zero_for_every_policy() {
        let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        for policy in [
            SelfRemainderPolicy::Exact,
            SelfRemainderPolicy::Wrapping,
            SelfRemainderPolicy::Saturating,
        ] {
            let unit = live_signed_remainder_by_negative_one_unit(integer, policy);
            let contract =
                LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule::contract();
            let original_node = unit.functions[0].blocks[0].nodes[1].clone();
            let accepted_catalog = unit.accepted_obligation_facts.clone();
            let mut manager = crate::AnalysisManager::new(&unit);
            let products = manager
                .require_all(&unit, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let [candidate] = LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .try_into()
                .expect("one live signed remainder-by-negative-one candidate");
            let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch()
            else {
                unreachable!()
            };
            assert_eq!(patch.location.machine, id(321, MachineId::new));
            assert_eq!(patch.location.block, id(322, BlockId::new));
            assert_eq!(patch.location.node, 1);
            assert_eq!(patch.source_operation, id(327, OperationId::new));
            assert_eq!(patch.result, id(325, ValueId::new));
            assert_eq!(patch.scalar_type, integer);
            assert_eq!(patch.constant, IntegerValue::Signed(0));
            assert_eq!(candidate.predicted_cost_delta(), -1);
            assert!(candidate.substitutions().is_empty());
            assert_eq!(candidate.consumed_facts().len(), 2);
            assert!(
                candidate
                    .consumed_facts()
                    .iter()
                    .any(|fact| matches!(fact, OptimizationFactReference::ScalarConstant(_)))
            );
            assert!(
                candidate
                    .consumed_facts()
                    .iter()
                    .any(|fact| matches!(fact, OptimizationFactReference::AcceptedObligation(_)))
            );

            let accepted =
                validate_proof_certified_signed_integer_remainder_by_negative_one_candidate(
                    &unit, &candidate,
                )
                .unwrap();
            assert_eq!(
                accepted.validator(),
                OptimizationValidatorIdentity::from_canonical_bytes(
                    b"omega.validator.live-proof-certified-signed-integer-remainder-by-negative-one-elimination.v1"
                )
            );
            assert_eq!(accepted.unit().accepted_obligation_facts, accepted_catalog);
            let output_node = &accepted.unit().functions[0].blocks[0].nodes[1];
            assert!(matches!(
                output_node.operation,
                O::IntegerConstant {
                    psi_operation,
                    result,
                    scalar_type: ScalarType::Integer(output_type),
                    value: IntegerValue::Signed(0),
                } if psi_operation == id(327, OperationId::new)
                    && result == id(325, ValueId::new)
                    && output_type == integer
            ));
            assert_eq!(output_node.provenance, original_node.provenance);
            assert_eq!(output_node.fuel, original_node.fuel);
            assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
                !matches!(fact, OptimizationFact::OperationObligationReference { .. })
            }));
            assert!(accepted.unit().functions[0].facts.iter().any(|fact| {
                matches!(
                    fact,
                    OptimizationFact::IntegerConstant {
                        value,
                        constant: IntegerValue::Signed(0),
                        support,
                    } if *value == id(325, ValueId::new)
                        && *support == id(327, OperationId::new)
                )
            }));

            let mut manager = crate::AnalysisManager::new(accepted.unit());
            let products = manager
                .require_all(accepted.unit(), contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            assert!(
                LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule
                    .propose(accepted.unit(), RuleAnalysisView::new(&products))
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn proof_certified_signed_remainder_by_negative_one_declines_and_rejects_corruption() {
        let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        let contract =
            LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule::contract();
        let unit = live_signed_remainder_by_negative_one_unit(integer, SelfRemainderPolicy::Exact);
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .unwrap();
        let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch() else {
            unreachable!()
        };
        let (constant_fact, obligation_fact) =
            candidate.proof_certified_scalar_identity_witness().unwrap();

        for forged_patch in [
            IntegerConstantRewrite {
                constant: IntegerValue::Signed(1),
                ..patch
            },
            IntegerConstantRewrite {
                source_operation: id(330, OperationId::new),
                ..patch
            },
            IntegerConstantRewrite {
                result: id(330, ValueId::new),
                ..patch
            },
            IntegerConstantRewrite {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).unwrap(),
                ..patch
            },
        ] {
            let forged =
                PsiRewriteCandidate::new_literal_proof_certified_integer_constant_replacement(
                    unit.identity,
                    contract,
                    candidate.affected_blocks().to_vec(),
                    candidate.provenance().to_vec(),
                    constant_fact,
                    obligation_fact,
                    candidate.predicted_cost_delta(),
                    forged_patch,
                )
                .unwrap();
            assert!(
                validate_proof_certified_signed_integer_remainder_by_negative_one_candidate(
                    &unit, &forged,
                )
                .is_err()
            );
        }

        for (bad_constant, bad_obligation, expected) in [
            (
                ScalarConstantFactIdentity::from_canonical_bytes(b"foreign negative-one literal"),
                obligation_fact,
                OptimizationUnitValidationError::CandidateOperandFactMismatch,
            ),
            (
                constant_fact,
                omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                    b"foreign negative-one proof",
                ),
                OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
            ),
        ] {
            let forged =
                PsiRewriteCandidate::new_literal_proof_certified_integer_constant_replacement(
                    unit.identity,
                    contract,
                    candidate.affected_blocks().to_vec(),
                    candidate.provenance().to_vec(),
                    bad_constant,
                    bad_obligation,
                    candidate.predicted_cost_delta(),
                    patch,
                )
                .unwrap();
            assert_eq!(
                validate_proof_certified_signed_integer_remainder_by_negative_one_candidate(
                    &unit, &forged,
                ),
                Err(expected)
            );
        }

        let mut corrupt_provenance = candidate.provenance().to_vec();
        corrupt_provenance[0].fuel[0].units += 1;
        let forged = PsiRewriteCandidate::new_literal_proof_certified_integer_constant_replacement(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            corrupt_provenance,
            constant_fact,
            obligation_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_signed_integer_remainder_by_negative_one_candidate(
                &unit, &forged,
            ),
            Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
        );

        let mut propagated_products = products.clone();
        let AnalysisProduct::ScalarConstants(constants) = propagated_products
            .iter_mut()
            .find(|product| product.kind() == AnalysisKind::ScalarConstants)
            .unwrap()
        else {
            unreachable!()
        };
        constants
            .facts
            .iter_mut()
            .find(|fact| fact.value == id(324, ValueId::new))
            .unwrap()
            .support
            .edges
            .push(id(329, EdgeId::new));
        assert!(
            LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule
                .propose(&unit, RuleAnalysisView::new(&propagated_products))
                .unwrap()
                .is_empty(),
            "a propagated negative-one fact is not a direct literal witness"
        );

        let mut non_negative_one = unit.clone();
        let O::IntegerConstant { value, .. } =
            &mut non_negative_one.functions[0].blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        *value = IntegerValue::Signed(-2);
        for fact in &mut non_negative_one.functions[0].facts {
            if let OptimizationFact::IntegerConstant {
                value, constant, ..
            } = fact
                && *value == id(324, ValueId::new)
            {
                *constant = IntegerValue::Signed(-2);
            }
        }
        non_negative_one.identity = recompute_psi_optimization_unit_identity(&non_negative_one);
        validate_psi_optimization_unit(&non_negative_one).unwrap();

        let mut missing_reference = unit.clone();
        missing_reference.functions[0]
            .facts
            .retain(|fact| !matches!(fact, OptimizationFact::OperationObligationReference { .. }));
        missing_reference.identity = recompute_psi_optimization_unit_identity(&missing_reference);
        let mut missing_catalog = unit.clone();
        missing_catalog.accepted_obligation_facts.clear();
        missing_catalog.identity = recompute_psi_optimization_unit_identity(&missing_catalog);
        for ineligible in [
            non_negative_one,
            missing_reference,
            missing_catalog,
            discard_scalar_function_result(unit.clone()),
            live_self_remainder_unit(integer, SelfRemainderPolicy::Exact),
            live_remainder_by_one_unit(
                IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                SelfRemainderPolicy::Exact,
            ),
        ] {
            let mut manager = crate::AnalysisManager::new(&ineligible);
            let products = manager
                .require_all(&ineligible, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            assert!(
                LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule
                    .propose(&ineligible, RuleAnalysisView::new(&products))
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn proof_certified_exact_zero_product_is_canonical_and_closed() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let contract = LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule::contract();
        let mut both_zero = exact_add_unit();
        for node in &mut both_zero.functions[0].blocks[0].nodes[..2] {
            let O::IntegerConstant { value, .. } = &mut node.operation else {
                unreachable!()
            };
            *value = IntegerValue::Unsigned(0);
        }
        for fact in &mut both_zero.functions[0].facts {
            if let OptimizationFact::IntegerConstant { constant, .. } = fact {
                *constant = IntegerValue::Unsigned(0);
            }
        }
        both_zero.functions[0].blocks[0].nodes[2].operation = O::ExactIntegerMultiply {
            psi_operation: id(308, OperationId::new),
            obligation: id(309, ObligationId::new),
            result: id(305, ValueId::new),
            scalar_type: integer,
            left: id(303, ValueId::new),
            right: id(304, ValueId::new),
        };
        both_zero.identity = recompute_psi_optimization_unit_identity(&both_zero);
        validate_psi_optimization_unit(&both_zero).unwrap();
        let mut manager = crate::AnalysisManager::new(&both_zero);
        let products = manager
            .require_all(&both_zero, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
            .propose(&both_zero, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .expect("zero times zero has one canonical candidate");
        let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch()
        else {
            unreachable!()
        };
        assert_eq!(
            patch.identity,
            ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroLeft
        );
        assert_eq!(patch.replacement, id(303, ValueId::new));
        validate_proof_certified_scalar_identity_candidate(&both_zero, &candidate).unwrap();

        for wrapping in [true, false] {
            let unit = live_proof_binary_identity_unit(
                integer,
                IntegerValue::Unsigned(0),
                false,
                |psi_operation, _obligation, result, scalar_type, left, right| {
                    if wrapping {
                        O::WrappingIntegerMultiply {
                            psi_operation,
                            result,
                            scalar_type,
                            left,
                            right,
                        }
                    } else {
                        O::SaturatingIntegerMultiply {
                            psi_operation,
                            result,
                            scalar_type,
                            left,
                            right,
                        }
                    }
                },
            );
            let mut manager = crate::AnalysisManager::new(&unit);
            let products = manager
                .require_all(&unit, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            assert!(
                LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
                    .propose(&unit, RuleAnalysisView::new(&products))
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn proof_certified_exact_multiply_by_zero_declines_and_rejects_corruption() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let unit = live_exact_multiply_by_zero_unit(integer, true);
        let contract = LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .unwrap();
        let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch()
        else {
            unreachable!()
        };
        let (constant_fact, obligation_fact) =
            candidate.proof_certified_scalar_identity_witness().unwrap();

        for identity in [
            ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight,
            ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneLeft,
            ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight,
        ] {
            let forged = PsiRewriteCandidate::new_proof_certified_scalar_identity(
                unit.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                constant_fact,
                obligation_fact,
                candidate.predicted_cost_delta(),
                ProofCertifiedScalarIdentityRewrite { identity, ..patch },
            )
            .unwrap();
            assert_eq!(
                validate_proof_certified_scalar_identity_candidate(&unit, &forged),
                Err(OptimizationUnitValidationError::CandidatePatchMismatch)
            );
        }
        let wrong_replacement = PsiRewriteCandidate::new_proof_certified_scalar_identity(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            constant_fact,
            obligation_fact,
            candidate.predicted_cost_delta(),
            ProofCertifiedScalarIdentityRewrite {
                replacement: id(323, ValueId::new),
                ..patch
            },
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_scalar_identity_candidate(&unit, &wrong_replacement),
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        );
        let foreign_constant = PsiRewriteCandidate::new_proof_certified_scalar_identity(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            ScalarConstantFactIdentity::from_canonical_bytes(b"foreign zero-product literal"),
            obligation_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_scalar_identity_candidate(&unit, &foreign_constant),
            Err(OptimizationUnitValidationError::CandidateOperandFactMismatch)
        );
        let foreign_obligation = PsiRewriteCandidate::new_proof_certified_scalar_identity(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            constant_fact,
            omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                b"foreign zero-product proof",
            ),
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_scalar_identity_candidate(&unit, &foreign_obligation),
            Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        );

        let nonzero = live_divide_by_one_unit(
            integer,
            |psi_operation, obligation, result, scalar_type, left, right| O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
        );
        let mut manager = crate::AnalysisManager::new(&nonzero);
        let products = manager
            .require_all(&nonzero, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
                .propose(&nonzero, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );

        let mut missing_proof = live_exact_multiply_by_zero_unit(integer, false);
        missing_proof.accepted_obligation_facts.clear();
        missing_proof.identity = recompute_psi_optimization_unit_identity(&missing_proof);
        let mut manager = crate::AnalysisManager::new(&missing_proof);
        let products = manager
            .require_all(&missing_proof, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
                .propose(&missing_proof, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );

        let mut unused = live_exact_multiply_by_zero_unit(integer, false);
        let O::Return { value, .. } = &mut unused.functions[0].blocks[0].nodes[2].operation else {
            unreachable!()
        };
        *value = id(324, ValueId::new);
        unused.functions[0].blocks[0].nodes[2].uses[0].value = id(324, ValueId::new);
        unused.identity = recompute_psi_optimization_unit_identity(&unused);
        validate_psi_optimization_unit(&unused).unwrap();
        let mut manager = crate::AnalysisManager::new(&unused);
        let products = manager
            .require_all(&unused, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
                .propose(&unused, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );

        let mut detached = unit.clone();
        detached.functions[0]
            .facts
            .retain(|fact| !matches!(fact, OptimizationFact::OperationObligationReference { .. }));
        detached.identity = recompute_psi_optimization_unit_identity(&detached);
        assert!(matches!(
            validate_psi_optimization_unit(&detached),
            Err(OptimizationUnitValidationError::FactIndexMismatch(_))
        ));
    }

    #[test]
    fn proof_check_elision_covers_the_closed_proof_bearing_scalar_vocabulary() {
        let seed = dead_exact_add_unit();
        let O::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } = seed.functions[0].blocks[0].nodes[2].operation
        else {
            unreachable!()
        };
        let operations = vec![
            O::ExactIntegerAdd {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            O::ExactIntegerSubtract {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            O::ExactIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            O::ExactIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            O::WrappingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            O::WrappingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            O::SaturatingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            O::SaturatingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            O::ExactIntegerShiftLeft {
                psi_operation,
                obligation,
                result,
                value_type: scalar_type,
                count_type: scalar_type,
                value: left,
                count: right,
            },
            O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                result,
                value_type: scalar_type,
                count_type: scalar_type,
                value: left,
                count: right,
            },
        ];
        for operation in operations {
            let mut unit = seed.clone();
            unit.functions[0].blocks[0].nodes[2].operation = operation;
            unit.identity = recompute_psi_optimization_unit_identity(&unit);
            validate_psi_optimization_unit(&unit).unwrap();
            let liveness = compute_analysis(&unit, AnalysisKind::ValueLiveness).unwrap();
            let effects = compute_analysis(&unit, AnalysisKind::EffectSummaries).unwrap();
            let [candidate] = ProofCertifiedDeadScalarEliminationRule
                .propose(&unit, RuleAnalysisView::new(&[liveness, effects]))
                .unwrap()
                .try_into()
                .expect("each exact binary proof shape proposes once");
            validate_dead_scalar_node_candidate(&unit, &candidate).unwrap();
        }

        let cast = discard_scalar_function_result(exact_cast_unit(7));
        validate_psi_optimization_unit(&cast).unwrap();
        let liveness = compute_analysis(&cast, AnalysisKind::ValueLiveness).unwrap();
        let effects = compute_analysis(&cast, AnalysisKind::EffectSummaries).unwrap();
        let [candidate] = ProofCertifiedDeadScalarEliminationRule
            .propose(&cast, RuleAnalysisView::new(&[liveness, effects]))
            .unwrap()
            .try_into()
            .expect("the exact cast proposes once");
        let PsiRewritePatch::RemoveDeadScalarNode(patch) = candidate.patch() else {
            unreachable!()
        };
        assert_eq!(
            patch.scalar_type,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap())
        );
        validate_dead_scalar_node_candidate(&cast, &candidate).unwrap();
    }

    #[test]
    fn constant_conditional_pruning_is_symmetric_and_rebases_all_later_blocks() {
        let unit = propagated_block_parameter_unit(false);
        let contract = ConstantConditionalFoldRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidate = ConstantConditionalFoldRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .pop()
            .unwrap();

        assert_eq!(
            candidate.affected_blocks(),
            [
                id(602, BlockId::new),
                id(603, BlockId::new),
                id(604, BlockId::new),
                id(605, BlockId::new),
            ]
        );
        assert_eq!(
            candidate
                .provenance()
                .iter()
                .filter(|row| row.disposition.is_realized())
                .count(),
            4
        );
        assert_eq!(
            candidate
                .provenance()
                .iter()
                .filter(|row| !row.disposition.is_realized())
                .count(),
            3
        );
        let accepted = validate_constant_conditional_candidate(&unit, &candidate).unwrap();
        let output = accepted.unit();
        assert_eq!(
            output.functions[0]
                .blocks
                .iter()
                .map(|block| block.id)
                .collect::<Vec<_>>(),
            [
                id(602, BlockId::new),
                id(604, BlockId::new),
                id(605, BlockId::new),
            ]
        );
        assert_eq!(output.functions[0].facts.len(), 2);
        assert_eq!(output.functions[0].blocks[1].nodes[0].effect.input, 2);
        assert_eq!(output.functions[0].blocks[2].nodes[1].effect.output, 6);
    }

    #[test]
    fn linear_empty_block_thread_composes_bindings_and_realizes_both_edges() {
        let unit = linear_empty_block_unit();
        let contract = LinearEmptyBlockThreadRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidate = LinearEmptyBlockThreadRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .pop()
            .expect("linear jump block is threadable");
        assert_eq!(
            candidate.affected_blocks(),
            [
                id(902, BlockId::new),
                id(903, BlockId::new),
                id(904, BlockId::new),
            ]
        );
        assert_eq!(candidate.provenance().len(), 3);
        assert!(
            candidate
                .provenance()
                .iter()
                .all(|row| row.disposition.is_realized())
        );
        assert_eq!(
            candidate
                .provenance()
                .iter()
                .filter(|row| {
                    matches!(row.input, PsiRealizationSite::Edge { .. })
                        && row.disposition.site()
                            == PsiRealizationSite::Edge {
                                machine: id(901, MachineId::new),
                                edge: id(911, psi_core::EdgeId::new),
                            }
                })
                .count(),
            2
        );

        let accepted = validate_linear_empty_block_candidate(&unit, &candidate).unwrap();
        assert_eq!(
            accepted.validator(),
            omega_optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.linear-empty-block-thread.v2"
            )
        );
        let output = accepted.unit();
        assert_eq!(
            output.functions[0]
                .blocks
                .iter()
                .map(|block| block.id)
                .collect::<Vec<_>>(),
            [id(902, BlockId::new), id(904, BlockId::new)]
        );
        let O::Jump {
            psi_edge,
            target,
            bindings,
            ..
        } = &output.functions[0].blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        assert_eq!(*psi_edge, id(911, EdgeId::new));
        assert_eq!(*target, id(904, BlockId::new));
        assert_eq!(bindings[0].argument, id(906, ValueId::new));
        assert_eq!(bindings[1].argument, id(905, ValueId::new));
        assert!(output.functions[0].blocks[0].nodes[0].provenance.is_empty());
        assert!(output.functions[0].blocks[0].nodes[0].fuel.is_empty());
        assert_eq!(
            output.functions[0].blocks[0].nodes[0].successors[0]
                .provenance
                .len(),
            2
        );
        assert_eq!(
            output.functions[0].blocks[0].nodes[0].successors[0]
                .fuel
                .len(),
            2
        );
        assert_eq!(output.functions[0].blocks[1].nodes[0].effect.input, 1);
        assert_eq!(output.functions[0].blocks[1].nodes[0].effect.output, 2);
    }

    #[test]
    fn linear_empty_block_validator_rejects_incomplete_fused_custody() {
        let unit = linear_empty_block_unit();
        let contract = LinearEmptyBlockThreadRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidate = LinearEmptyBlockThreadRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .pop()
            .unwrap();
        let omega_optimization_unit::PsiRewritePatch::ThreadLinearEmptyBlock(patch) =
            candidate.patch()
        else {
            unreachable!()
        };
        let mut provenance = candidate.provenance().to_vec();
        let incoming = provenance
            .iter()
            .find(|row| {
                row.input
                    == PsiRealizationSite::Edge {
                        machine: patch.predecessor.machine,
                        edge: patch.incoming_edge,
                    }
            })
            .expect("incoming occurrence is present")
            .clone();
        let outgoing = provenance
            .iter_mut()
            .find(|row| {
                row.input
                    == PsiRealizationSite::Edge {
                        machine: patch.predecessor.machine,
                        edge: patch.outgoing_edge,
                    }
            })
            .expect("outgoing occurrence is present");
        outgoing.sources = incoming.sources;
        outgoing.fuel = incoming.fuel;
        let incomplete = PsiRewriteCandidate::new_linear_empty_block(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            provenance,
            -3,
            patch,
        )
        .unwrap();
        assert!(matches!(
            validate_linear_empty_block_candidate(&unit, &incomplete),
            Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
        ));
    }

    #[test]
    fn path_qualified_empty_block_thread_fans_out_only_on_incoming_edge_antichain() {
        let unit = path_qualified_empty_block_unit();
        let contract = PathQualifiedEmptyBlockThreadRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidate = PathQualifiedEmptyBlockThreadRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .pop()
            .expect("two mutually exclusive incoming edges are threadable");
        let PsiRewritePatch::ThreadPathQualifiedEmptyBlock(patch) = candidate.patch() else {
            unreachable!()
        };
        let outgoing_site = PsiRealizationSite::Edge {
            machine: patch.empty.machine,
            edge: patch.outgoing_edge,
        };
        let fanout = candidate
            .provenance()
            .iter()
            .filter(|row| row.input == outgoing_site)
            .collect::<Vec<_>>();
        assert_eq!(fanout.len(), 2);
        assert_ne!(fanout[0].disposition.site(), fanout[1].disposition.site());
        assert!(fanout.iter().all(|row| row.disposition.is_realized()));

        let accepted = validate_path_qualified_empty_block_candidate(&unit, &candidate).unwrap();
        assert_eq!(
            accepted.validator(),
            OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.path-qualified-empty-block-thread.v1"
            )
        );
        let function = &accepted.unit().functions[0];
        assert_eq!(function.blocks.len(), 4);
        assert!(
            !function
                .blocks
                .iter()
                .any(|block| block.id == patch.empty.block)
        );
        for edge_id in [id(933, EdgeId::new), id(934, EdgeId::new)] {
            let edge = function
                .blocks
                .iter()
                .flat_map(|block| block.nodes.iter())
                .flat_map(|node| node.successors.iter())
                .find(|edge| edge.psi_edge == edge_id)
                .expect("incoming edge survives");
            assert_eq!(edge.target, patch.target);
            assert_eq!(
                edge.provenance,
                [
                    PsiProvenance::Edge(edge_id),
                    PsiProvenance::Edge(patch.outgoing_edge),
                ]
            );
        }

        let mut coexecuted = accepted.unit().clone();
        let source = PsiProvenance::Edge(patch.outgoing_edge);
        coexecuted.functions[0].blocks[0].nodes[0].successors[0]
            .provenance
            .push(source);
        coexecuted.functions[0].blocks[0].nodes[0].successors[0]
            .fuel
            .push(omega_optimization_unit::FuelSettlement {
                site: source,
                units: 1,
            });
        coexecuted.identity = recompute_psi_optimization_unit_identity(&coexecuted);
        assert_eq!(
            omega_optimization_validation::validate_psi_optimization_unit(&coexecuted),
            Err(OptimizationUnitValidationError::CoExecutableProvenanceOccurrences(source))
        );
    }

    #[test]
    fn constant_conditional_validator_rejects_edge_and_fuel_corruption() {
        let unit = constant_conditional_same_target_unit(true);
        let contract = ConstantConditionalFoldRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidate = ConstantConditionalFoldRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .pop()
            .unwrap();
        let omega_optimization_unit::PsiRewritePatch::FoldConstantConditional(patch) =
            candidate.patch()
        else {
            unreachable!()
        };
        let condition_fact = candidate
            .scalar_evaluation_witness()
            .and_then(IntegerEvaluationWitness::unary_operand)
            .unwrap();
        assert!(matches!(
            PsiRewriteCandidate::new_constant_conditional(
                unit.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                candidate.provenance()[..1].to_vec(),
                condition_fact,
                -1,
                patch,
            ),
            Err(omega_optimization_unit::PsiRewriteCandidateError::PatchDecisionPointMismatch)
        ));

        let mut duplicate_source = candidate.provenance().to_vec();
        let source = duplicate_source[0].sources[0];
        let fuel = duplicate_source[0].fuel[0];
        duplicate_source[0].sources.push(source);
        duplicate_source[0].fuel.push(fuel);
        assert!(matches!(
            PsiRewriteCandidate::new_constant_conditional(
                unit.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                duplicate_source,
                condition_fact,
                -1,
                patch,
            ),
            Err(omega_optimization_unit::PsiRewriteCandidateError::NonCanonicalProvenance)
        ));

        let mut zero_fuel = candidate.provenance().to_vec();
        zero_fuel[1].fuel[0].units = 0;
        assert!(matches!(
            PsiRewriteCandidate::new_constant_conditional(
                unit.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                zero_fuel,
                condition_fact,
                -1,
                patch,
            ),
            Err(omega_optimization_unit::PsiRewriteCandidateError::FuelProvenanceMismatch)
        ));

        let selected_site = PsiRealizationSite::Edge {
            machine: patch.location.machine,
            edge: patch.selected_edge,
        };
        let rejected_site = PsiRealizationSite::Edge {
            machine: patch.location.machine,
            edge: patch.rejected_edge,
        };
        let mut swapped_provenance = candidate.provenance().to_vec();
        for row in &mut swapped_provenance {
            if row.input == selected_site {
                row.disposition = ProvenanceDisposition::ProvenUnreachableAt(selected_site);
            } else if row.input == rejected_site {
                row.disposition = ProvenanceDisposition::RealizedAt(rejected_site);
            }
        }
        let swapped = PsiRewriteCandidate::new_constant_conditional(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            swapped_provenance,
            condition_fact,
            -1,
            ConstantConditionalRewrite {
                selected_edge: patch.rejected_edge,
                rejected_edge: patch.selected_edge,
                ..patch
            },
        )
        .unwrap();
        assert!(matches!(
            validate_constant_conditional_candidate(&unit, &swapped),
            Err(OptimizationUnitValidationError::CandidateEvaluationMismatch)
        ));

        let mut provenance = candidate.provenance().to_vec();
        provenance[0].fuel[0].units += 1;
        let wrong_fuel = PsiRewriteCandidate::new_constant_conditional(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            provenance,
            condition_fact,
            -1,
            patch,
        )
        .unwrap();
        assert!(matches!(
            validate_constant_conditional_candidate(&unit, &wrong_fuel),
            Err(OptimizationUnitValidationError::CandidateFuelMismatch)
        ));

        let mut provenance = candidate.provenance().to_vec();
        provenance[1].fuel[0].units += 1;
        let wrong_unreachable_fuel = PsiRewriteCandidate::new_constant_conditional(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            provenance,
            condition_fact,
            -1,
            patch,
        )
        .unwrap();
        assert!(matches!(
            validate_constant_conditional_candidate(&unit, &wrong_unreachable_fuel),
            Err(OptimizationUnitValidationError::CandidateFuelMismatch)
        ));
    }

    #[test]
    fn constant_conditional_validator_rejects_incomplete_prune_custody_and_region() {
        let unit = propagated_block_parameter_unit(true);
        let contract = ConstantConditionalFoldRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidate = ConstantConditionalFoldRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .pop()
            .unwrap();
        let omega_optimization_unit::PsiRewritePatch::FoldConstantConditional(patch) =
            candidate.patch()
        else {
            unreachable!()
        };
        let condition_fact = candidate
            .scalar_evaluation_witness()
            .and_then(IntegerEvaluationWitness::unary_operand)
            .unwrap();
        let dead_block = id(604, BlockId::new);
        let rebased_merge = id(605, BlockId::new);

        let mut incomplete_provenance = candidate.provenance().to_vec();
        let removed = incomplete_provenance
            .iter()
            .position(|row| {
                !row.disposition.is_realized()
                    && matches!(
                        row.disposition.site(),
                        PsiRealizationSite::Node(location) if location.block == dead_block
                    )
            })
            .expect("dead nodes carry unreachable custody");
        incomplete_provenance.remove(removed);
        let incomplete_custody = PsiRewriteCandidate::new_constant_conditional(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            incomplete_provenance,
            condition_fact,
            -1,
            patch,
        )
        .unwrap();
        assert!(matches!(
            validate_constant_conditional_candidate(&unit, &incomplete_custody),
            Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
        ));

        let incomplete_region = PsiRewriteCandidate::new_constant_conditional(
            unit.identity,
            contract,
            candidate
                .affected_blocks()
                .iter()
                .copied()
                .filter(|block| *block != rebased_merge)
                .collect(),
            candidate.provenance().to_vec(),
            condition_fact,
            -1,
            patch,
        )
        .unwrap();
        assert!(matches!(
            validate_constant_conditional_candidate(&unit, &incomplete_region),
            Err(OptimizationUnitValidationError::CandidateReachabilityMismatch)
        ));
    }

    #[test]
    fn redundant_block_parameter_rule_binds_both_exact_conditional_edges() {
        let unit = redundant_block_parameter_unit(true);
        let contract = RedundantBlockParameterRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidates = RedundantBlockParameterRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap();
        assert_eq!(candidates.len(), 1);
        let witness = candidates[0].redundant_block_parameter_witness().unwrap();
        assert_eq!(witness.incoming.len(), 2);
        assert_eq!(witness.incoming[0].source, witness.incoming[1].source);
        assert_ne!(witness.incoming[0].edge, witness.incoming[1].edge);
        assert!(candidates[0].consumed_facts().is_empty());

        let accepted = validate_redundant_block_parameter_candidate(&unit, &candidates[0]).unwrap();
        assert_eq!(
            accepted.validator(),
            omega_optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.redundant-block-parameter.v2"
            )
        );
        let output = accepted.unit();
        assert!(output.functions[0].blocks[1].parameters.is_empty());
        let O::Conditional {
            when_true,
            when_false,
            ..
        } = &output.functions[0].blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        assert!(when_true.bindings.is_empty());
        assert!(when_false.bindings.is_empty());
        let O::ExactIntegerAdd {
            obligation, left, ..
        } = output.functions[0].blocks[1].nodes[0].operation
        else {
            unreachable!()
        };
        assert_eq!(left, unit.functions[0].parameters[1].value);
        assert_eq!(obligation, id(713, ObligationId::new));
        assert_eq!(output.functions[0].facts, unit.functions[0].facts);
        for (before, after) in unit.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .zip(
                output.functions[0]
                    .blocks
                    .iter()
                    .flat_map(|block| &block.nodes),
            )
        {
            assert_eq!(after.provenance, before.provenance);
            assert_eq!(after.fuel, before.fuel);
            assert_eq!(after.effect, before.effect);
            assert_eq!(after.ownership, before.ownership);
        }
    }

    #[test]
    fn differing_bindings_decline_and_incomplete_edge_witness_rejects() {
        let unit = redundant_block_parameter_unit(false);
        let contract = RedundantBlockParameterRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            RedundantBlockParameterRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );

        let unit = redundant_block_parameter_unit(true);
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidate = RedundantBlockParameterRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .pop()
            .unwrap();
        let omega_optimization_unit::PsiRewritePatch::RemoveRedundantBlockParameter(patch) =
            candidate.patch()
        else {
            unreachable!()
        };
        let incomplete = PsiRewriteCandidate::new_redundant_block_parameter(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            RedundantBlockParameterWitness {
                incoming: candidate
                    .redundant_block_parameter_witness()
                    .unwrap()
                    .incoming[..1]
                    .to_vec(),
            },
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_ne!(incomplete.identity(), candidate.identity());
        assert_eq!(
            validate_redundant_block_parameter_candidate(&unit, &incomplete),
            Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)
        );
    }
}
