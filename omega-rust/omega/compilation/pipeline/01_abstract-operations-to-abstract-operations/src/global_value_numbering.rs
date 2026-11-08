/*
Global value numbering (`global_value_numbering`): same-block, dominator, and
phi-translated common-subexpression elimination across total, proof-certified,
and compatible-policy expression keys (`expression_keys`), join-parameter
provenance accounting (`phi_translated_accounting`), and obligation-free
total-scalar identity rules (`total_scalar_identities`).
*/

use std::collections::BTreeMap;

use optimization_core::{
    AcceptedObligationFactIdentity, AnalysisInvalidationSet, AnalysisKind, AnalysisSet,
    OptimizationRuleContract, OptimizationSafetyClass,
};
use optimization_unit::{
    DominatingScalarCommonSubexpressionRewrite, LocalScalarCommonSubexpressionRewrite,
    NodeLocation, OptimizationFact, PhiTranslatedScalarGvnRewrite, ProvenanceRewrite,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiRewriteCandidate, ValueDefinition,
    ValueDefinitionSite,
};
use semantic_vocabulary::{BlockId, MachineId, OperationId, ScalarType, ValueId};

use expression_keys::{ScalarExpressionKey, ScalarExpressionRow};

use crate::rules::catalog::BuiltInRuleRegistration;
use crate::{
    AnalysisProduct, EffectKnowledge, EffectSummaryAnalysis, RuleProposalError,
    analyses::block_dominates,
    rules::{GLOBAL_VALUE_NUMBERING_PASS_NAME, accepted_obligations, standard_rule_contract},
};
use abstract_operations::AbstractOperation;

pub(crate) mod dominating_scalar_gvn {
    use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

    use super::propose_dominating;
    use crate::rules::global_value_numbering::dominating_contract;
    use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

    psi_rule_contract!(
        DominatorTotalScalarGvnRule,
        dominating_contract(
            b"omega.psi-rule.dominator-obligation-free-total-scalar-gvn.v1",
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        propose = propose_dominating,
        super::TOTAL_CSE_KEYS,
    );

    psi_rule_contract!(
        DominatorProofCertifiedScalarGvnRule,
        dominating_contract(
            b"omega.psi-rule.dominator-proof-certified-total-scalar-gvn.v1",
            OptimizationSafetyClass::ProofCertified,
        ),
        propose = propose_dominating,
        super::PROOF_CERTIFIED_CSE_KEYS,
    );

    psi_rule_contract!(
        DominatorProofCertifiedCompatiblePolicyScalarGvnRule,
        dominating_contract(
            b"omega.psi-rule.dominator-proof-certified-compatible-policy-scalar-gvn.v1",
            OptimizationSafetyClass::ProofCertified,
        ),
        propose = propose_dominating,
        super::COMPATIBLE_POLICY_CSE_KEYS,
    );
}
pub(crate) mod expression_keys {
    pub(crate) mod compatible_policy {
        use abstract_operations::{
            AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
            IntegerShiftDirection, IntegerShiftPolicy,
        };
        use semantic_vocabulary::{IntegerType, ScalarType, ValueId};

        use super::{CompatiblePolicyScalarExpressionKey, ScalarExpressionRow, canonical_pair};

        pub(crate) fn compatible_policy_scalar_leader(
            operation: &O,
        ) -> Option<ScalarExpressionRow<CompatiblePolicyScalarExpressionKey>> {
            compatible_policy_row(
                operation,
                |policy| policy == IntegerShiftPolicy::Wrapping,
                |policy| {
                    matches!(
                        policy,
                        IntegerOverflowPolicy::Wrapping | IntegerOverflowPolicy::Saturating
                    )
                },
            )
        }

        pub(crate) fn compatible_policy_scalar_redundant(
            operation: &O,
        ) -> Option<ScalarExpressionRow<CompatiblePolicyScalarExpressionKey>> {
            compatible_policy_row(
                operation,
                |policy| matches!(policy, IntegerShiftPolicy::Exact { .. }),
                |policy| policy == IntegerOverflowPolicy::Exact,
            )
        }

        fn compatible_policy_row(
            operation: &O,
            admits_shift: impl Fn(IntegerShiftPolicy) -> bool,
            admits_arithmetic: impl Fn(IntegerOverflowPolicy) -> bool,
        ) -> Option<ScalarExpressionRow<CompatiblePolicyScalarExpressionKey>> {
            let row = match operation {
                O::IntegerShift {
                    psi_operation,
                    result,
                    value_type,
                    count_type,
                    policy,
                    direction,
                    value,
                    count,
                } if admits_shift(*policy) => (
                    shift_key(*direction, *value_type, *count_type, *value, *count),
                    *psi_operation,
                    *result,
                    ScalarType::Integer(*value_type),
                ),
                O::IntegerArithmetic {
                    psi_operation,
                    result,
                    scalar_type,
                    arithmetic,
                    left,
                    right,
                } if admits_arithmetic(arithmetic.policy()) => (
                    arithmetic_key(arithmetic.operator(), *scalar_type, *left, *right)?,
                    *psi_operation,
                    *result,
                    ScalarType::Integer(*scalar_type),
                ),
                _ => return None,
            };
            Some(row)
        }

        fn shift_key(
            direction: IntegerShiftDirection,
            value_type: IntegerType,
            count_type: IntegerType,
            value: ValueId,
            count: ValueId,
        ) -> CompatiblePolicyScalarExpressionKey {
            match direction {
                IntegerShiftDirection::Left => CompatiblePolicyScalarExpressionKey::ShiftLeft(
                    value_type, count_type, value, count,
                ),
                IntegerShiftDirection::Right => CompatiblePolicyScalarExpressionKey::ShiftRight(
                    value_type, count_type, value, count,
                ),
            }
        }

        fn arithmetic_key(
            operator: IntegerArithmeticOperator,
            scalar_type: IntegerType,
            left: ValueId,
            right: ValueId,
        ) -> Option<CompatiblePolicyScalarExpressionKey> {
            let (low, high) = canonical_pair(left, right);
            match operator {
                IntegerArithmeticOperator::Add => Some(CompatiblePolicyScalarExpressionKey::Add(
                    scalar_type,
                    low,
                    high,
                )),
                IntegerArithmeticOperator::Subtract => Some(
                    CompatiblePolicyScalarExpressionKey::Subtract(scalar_type, left, right),
                ),
                IntegerArithmeticOperator::Multiply => Some(
                    CompatiblePolicyScalarExpressionKey::Multiply(scalar_type, low, high),
                ),
                IntegerArithmeticOperator::Divide | IntegerArithmeticOperator::Remainder => None,
            }
        }
    }
    pub(crate) mod proof_certified {
        use abstract_operations::{
            AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
            IntegerShiftDirection, IntegerShiftPolicy,
        };
        use semantic_vocabulary::ScalarType;

        use super::{ProofCertifiedScalarExpressionKey, ScalarExpressionRow, canonical_pair};

        pub(crate) fn proof_certified_scalar_expression(
            operation: &O,
        ) -> Option<ScalarExpressionRow<ProofCertifiedScalarExpressionKey>> {
            let row = match operation {
                O::IntegerExactCast {
                    psi_operation,
                    result,
                    source_type,
                    target_type,
                    operand,
                    ..
                } => (
                    ProofCertifiedScalarExpressionKey::ExactCast(
                        *source_type,
                        *target_type,
                        *operand,
                    ),
                    *psi_operation,
                    *result,
                    ScalarType::Integer(*target_type),
                ),
                O::IntegerShift {
                    psi_operation,
                    result,
                    value_type,
                    count_type,
                    policy: IntegerShiftPolicy::Exact { .. },
                    direction,
                    value,
                    count,
                } => (
                    match direction {
                        IntegerShiftDirection::Left => {
                            ProofCertifiedScalarExpressionKey::ExactShiftLeft(
                                *value_type,
                                *count_type,
                                *value,
                                *count,
                            )
                        }
                        IntegerShiftDirection::Right => {
                            ProofCertifiedScalarExpressionKey::ExactShiftRight(
                                *value_type,
                                *count_type,
                                *value,
                                *count,
                            )
                        }
                    },
                    *psi_operation,
                    *result,
                    ScalarType::Integer(*value_type),
                ),
                O::IntegerArithmetic {
                    psi_operation,
                    result,
                    scalar_type,
                    arithmetic,
                    left,
                    right,
                } => {
                    use IntegerArithmeticOperator as Operator;
                    use IntegerOverflowPolicy as Policy;
                    use ProofCertifiedScalarExpressionKey as Key;
                    let (low, high) = canonical_pair(*left, *right);
                    let key = match (arithmetic.policy(), arithmetic.operator()) {
                        (Policy::Exact, Operator::Add) => Key::ExactAdd(*scalar_type, low, high),
                        (Policy::Exact, Operator::Subtract) => {
                            Key::ExactSubtract(*scalar_type, *left, *right)
                        }
                        (Policy::Exact, Operator::Multiply) => {
                            Key::ExactMultiply(*scalar_type, low, high)
                        }
                        (Policy::Exact, Operator::Divide) => {
                            Key::ExactDivide(*scalar_type, *left, *right)
                        }
                        (Policy::Exact, Operator::Remainder) => {
                            Key::ExactRemainder(*scalar_type, *left, *right)
                        }
                        (Policy::Wrapping, Operator::Divide) => {
                            Key::WrappingDivide(*scalar_type, *left, *right)
                        }
                        (Policy::Wrapping, Operator::Remainder) => {
                            Key::WrappingRemainder(*scalar_type, *left, *right)
                        }
                        (Policy::Saturating, Operator::Divide) => {
                            Key::SaturatingDivide(*scalar_type, *left, *right)
                        }
                        (Policy::Saturating, Operator::Remainder) => {
                            Key::SaturatingRemainder(*scalar_type, *left, *right)
                        }
                        (
                            Policy::Wrapping | Policy::Saturating,
                            Operator::Add | Operator::Subtract | Operator::Multiply,
                        ) => return None,
                    };
                    (
                        key,
                        *psi_operation,
                        *result,
                        ScalarType::Integer(*scalar_type),
                    )
                }
                _ => return None,
            };
            Some(row)
        }
    }
    pub(crate) mod total {
        use std::collections::BTreeMap;

        use abstract_operations::{
            AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
            IntegerShiftDirection, IntegerShiftPolicy,
        };
        use semantic_vocabulary::{ScalarType, ValueId};

        use super::{ScalarExpressionRow, TotalScalarExpressionKey, canonical_pair};

        pub(crate) fn total_scalar_expression(
            operation: &O,
            value_types: &BTreeMap<ValueId, ScalarType>,
        ) -> Option<ScalarExpressionRow<TotalScalarExpressionKey>> {
            let boolean = ScalarType::Boolean;
            let integer_operand_type = |value: ValueId| match value_types.get(&value) {
                Some(ScalarType::Integer(scalar_type)) => Some(*scalar_type),
                _ => None,
            };
            let same_integer_operand_type = |left: ValueId, right: ValueId| {
                let scalar_type = integer_operand_type(left)?;
                (integer_operand_type(right)? == scalar_type).then_some(scalar_type)
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
                    let scalar_type = same_integer_operand_type(*left, *right)?;
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
                    let scalar_type = same_integer_operand_type(*left, *right)?;
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
                    let scalar_type = same_integer_operand_type(*left, *right)?;
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
                O::IntegerShift {
                    psi_operation,
                    result,
                    value_type,
                    count_type,
                    policy: IntegerShiftPolicy::Wrapping,
                    direction,
                    value,
                    count,
                } => (
                    match direction {
                        IntegerShiftDirection::Left => TotalScalarExpressionKey::WrappingShiftLeft(
                            *value_type,
                            *count_type,
                            *value,
                            *count,
                        ),
                        IntegerShiftDirection::Right => {
                            TotalScalarExpressionKey::WrappingShiftRight(
                                *value_type,
                                *count_type,
                                *value,
                                *count,
                            )
                        }
                    },
                    *psi_operation,
                    *result,
                    ScalarType::Integer(*value_type),
                ),
                O::IntegerArithmetic {
                    psi_operation,
                    result,
                    scalar_type,
                    arithmetic,
                    left,
                    right,
                } => {
                    use IntegerArithmeticOperator as Operator;
                    use IntegerOverflowPolicy as Policy;
                    use TotalScalarExpressionKey as Key;
                    let (low, high) = canonical_pair(*left, *right);
                    let key = match (arithmetic.policy(), arithmetic.operator()) {
                        (Policy::Wrapping, Operator::Add) => {
                            Key::WrappingAdd(*scalar_type, low, high)
                        }
                        (Policy::Wrapping, Operator::Subtract) => {
                            Key::WrappingSubtract(*scalar_type, *left, *right)
                        }
                        (Policy::Wrapping, Operator::Multiply) => {
                            Key::WrappingMultiply(*scalar_type, low, high)
                        }
                        (Policy::Saturating, Operator::Add) => {
                            Key::SaturatingAdd(*scalar_type, low, high)
                        }
                        (Policy::Saturating, Operator::Subtract) => {
                            Key::SaturatingSubtract(*scalar_type, *left, *right)
                        }
                        (Policy::Saturating, Operator::Multiply) => {
                            Key::SaturatingMultiply(*scalar_type, low, high)
                        }
                        _ => return None,
                    };
                    (
                        key,
                        *psi_operation,
                        *result,
                        ScalarType::Integer(*scalar_type),
                    )
                }
                _ => return None,
            };
            Some(row)
        }
    }

    use semantic_vocabulary::{OperationId, ScalarType, ValueId};

    pub(crate) use compatible_policy::{
        compatible_policy_scalar_leader, compatible_policy_scalar_redundant,
    };
    pub(crate) use proof_certified::proof_certified_scalar_expression;
    use semantic_vocabulary::{IntegerType, IntegerValue};
    use std::collections::{BTreeMap, BTreeSet};
    pub(crate) use total::total_scalar_expression;

    pub(crate) type ScalarExpressionRow<K> = (K, OperationId, ValueId, ScalarType);

    pub(crate) trait ScalarExpressionKey: Copy {
        fn references_parameter(self, values: &BTreeSet<ValueId>) -> bool;

        fn translated(self, values: &BTreeMap<ValueId, ValueId>) -> Option<Self>;
    }

    impl ScalarExpressionKey for TotalScalarExpressionKey {
        fn references_parameter(self, values: &BTreeSet<ValueId>) -> bool {
            self.references_any(values)
        }

        fn translated(self, values: &BTreeMap<ValueId, ValueId>) -> Option<Self> {
            self.translate(values)
        }
    }

    impl ScalarExpressionKey for ProofCertifiedScalarExpressionKey {
        fn references_parameter(self, values: &BTreeSet<ValueId>) -> bool {
            self.references_any(values)
        }

        fn translated(self, values: &BTreeMap<ValueId, ValueId>) -> Option<Self> {
            Some(self.translate(values))
        }
    }

    impl ScalarExpressionKey for CompatiblePolicyScalarExpressionKey {
        fn references_parameter(self, values: &BTreeSet<ValueId>) -> bool {
            self.references_any(values)
        }

        fn translated(self, values: &BTreeMap<ValueId, ValueId>) -> Option<Self> {
            Some(self.translate(values))
        }
    }

    fn canonical_pair(left: ValueId, right: ValueId) -> (ValueId, ValueId) {
        if left <= right {
            (left, right)
        } else {
            (right, left)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) enum TotalScalarExpressionKey {
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
    pub(crate) enum CompatiblePolicyScalarExpressionKey {
        ShiftLeft(IntegerType, IntegerType, ValueId, ValueId),
        ShiftRight(IntegerType, IntegerType, ValueId, ValueId),
        Add(IntegerType, ValueId, ValueId),
        Subtract(IntegerType, ValueId, ValueId),
        Multiply(IntegerType, ValueId, ValueId),
    }

    impl CompatiblePolicyScalarExpressionKey {
        pub(crate) fn references_any(self, values: &BTreeSet<ValueId>) -> bool {
            match self {
                Self::ShiftLeft(_, _, left, right)
                | Self::ShiftRight(_, _, left, right)
                | Self::Add(_, left, right)
                | Self::Subtract(_, left, right)
                | Self::Multiply(_, left, right) => {
                    values.contains(&left) || values.contains(&right)
                }
            }
        }

        pub(crate) fn translate(self, values: &BTreeMap<ValueId, ValueId>) -> Self {
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
        pub(crate) fn references_any(self, values: &BTreeSet<ValueId>) -> bool {
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
                | Self::SaturatingMultiply(_, left, right)
                | Self::WrappingShiftLeft(_, _, left, right)
                | Self::WrappingShiftRight(_, _, left, right) => {
                    values.contains(&left) || values.contains(&right)
                }
            }
        }

        pub(crate) fn translate(self, values: &BTreeMap<ValueId, ValueId>) -> Option<Self> {
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
    pub(crate) enum ProofCertifiedScalarExpressionKey {
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
        pub(crate) fn references_any(self, values: &BTreeSet<ValueId>) -> bool {
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

        pub(crate) fn translate(self, values: &BTreeMap<ValueId, ValueId>) -> Self {
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
}
pub(crate) mod phi_translated_accounting {
    use std::collections::BTreeSet;

    use crate::analyses::block_dominates;
    use crate::rules::control_flow_cleanup::{
        node_realized_provenance, provenance_realized_at, record_block_provenance,
    };
    use optimization_unit::{
        NodeLocation, PhiTranslatedScalarIncoming, ProvenanceRewrite, PsiOptimizationFunction,
        PsiRealizationSite,
    };
    use semantic_vocabulary::{BlockId, OperationId, ScalarType, ValueId};

    pub(crate) fn phi_translated_cse_accounting(
        function: &PsiOptimizationFunction,
        redundant: NodeLocation,
        incoming: &[PhiTranslatedScalarIncoming],
    ) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
        let block_position = function.block_index(redundant.block)?;
        let node_position = usize::try_from(redundant.node).ok()?;
        let block = &function.blocks[block_position];
        let removed = block.nodes.get(node_position)?;
        block.nodes.get(node_position.checked_add(1)?)?;
        let mut affected = incoming
            .iter()
            .map(|row| row.source)
            .chain([block.id])
            .collect::<BTreeSet<_>>();
        let mut provenance = vec![node_realized_provenance(redundant, removed)];
        for row in incoming {
            let source = function.block(row.source)?;
            let edge = source
                .successors()
                .find(|edge| edge.psi_edge == row.edge && edge.target == redundant.block)?;
            if !edge.provenance.is_empty() {
                let site = PsiRealizationSite::Edge {
                    machine: function.machine,
                    edge: edge.psi_edge,
                };
                provenance.push(provenance_realized_at(
                    site,
                    site,
                    &edge.provenance,
                    &edge.work,
                ));
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
            provenance.push(provenance_realized_at(
                PsiRealizationSite::Node(old),
                PsiRealizationSite::Node(new),
                &node.provenance,
                &node.work,
            ));
        }
        for later in function.blocks.iter().skip(block_position + 1) {
            affected.insert(later.id);
            record_block_provenance(function.machine, later, &mut provenance)?;
        }
        provenance.sort_by_key(ProvenanceRewrite::canonical_sort_key);
        Some((affected.into_iter().collect(), provenance))
    }

    use std::collections::BTreeMap;

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn collect_phi_translated_incoming<K>(
        function: &PsiOptimizationFunction,
        redundant: NodeLocation,
        redundant_result: ValueId,
        scalar_type: ScalarType,
        use_definitions: &crate::UseDefinitionAnalysis,
        value_types: &BTreeMap<ValueId, ScalarType>,
        machine_dominators: &[(BlockId, Vec<BlockId>)],
        leaders: &[(K, NodeLocation, OperationId, ValueId, ScalarType)],
        key_references_parameters: impl Fn(&BTreeSet<ValueId>) -> bool,
        translate: impl Fn(&BTreeMap<ValueId, ValueId>) -> Option<K>,
    ) -> Option<(
        Vec<PhiTranslatedScalarIncoming>,
        Vec<BlockId>,
        Vec<ProvenanceRewrite>,
        u32,
    )>
    where
        K: Copy + PartialEq,
    {
        let block = function.block(redundant.block)?;
        let parameter_values = block
            .parameters
            .iter()
            .map(|parameter| parameter.value)
            .collect::<BTreeSet<_>>();
        if parameter_values.is_empty()
            || !key_references_parameters(&parameter_values)
            || !use_definitions.uses.iter().any(|(machine, use_site)| {
                *machine == function.machine && use_site.value == redundant_result
            })
        {
            return None;
        }
        let redundant_index = usize::try_from(redundant.node).expect("u32 fits usize");
        let redundant_node = block.nodes.get(redundant_index)?;
        let receiver = block.nodes.get(redundant_index + 1)?;
        if receiver
            .provenance
            .iter()
            .any(|source| redundant_node.provenance.contains(source))
        {
            return None;
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
                    for (parameter, binding) in block.parameters.iter().zip(&edge.bindings) {
                        if binding.parameter != parameter.value
                            || binding.scalar_type != parameter.scalar_type
                            || value_types.get(&binding.argument) != Some(&binding.scalar_type)
                        {
                            complete = false;
                            break;
                        }
                        translation.insert(parameter.value, binding.argument);
                    }
                    if !complete {
                        continue;
                    }
                    let Some(translated_key) = translate(&translation) else {
                        complete = false;
                        continue;
                    };
                    let leader = leaders
                        .iter()
                        .filter(|(candidate_key, location, _, _, candidate_type)| {
                            candidate_key == &translated_key
                                && *candidate_type == scalar_type
                                && ((location.block == source.id && location.node < owner_index)
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
                    let Some((_, leader, leader_operation, leader_result, _)) = leader else {
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
            return None;
        }
        incoming.sort_by_key(|row| (row.edge, row.source));
        let (affected_blocks, provenance) =
            phi_translated_cse_accounting(function, redundant, &incoming)?;
        let parameter_position = u32::try_from(block.parameters.len())
            .expect("optimization block parameter count fits u32");
        Some((incoming, affected_blocks, provenance, parameter_position))
    }
}
pub(crate) mod phi_translated_scalar_gvn {
    use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

    use super::propose_phi_translated;
    use crate::rules::global_value_numbering::phi_translated_contract;
    use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

    psi_rule_contract!(
        PhiTranslatedObligationFreeScalarGvnRule,
        phi_translated_contract(
            b"omega.psi-rule.phi-translated-obligation-free-total-scalar-gvn.v1",
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        propose = propose_phi_translated,
        super::total_cse_phi_rows,
    );

    psi_rule_contract!(
        PhiTranslatedProofCertifiedScalarGvnRule,
        phi_translated_contract(
            b"omega.psi-rule.phi-translated-proof-certified-total-scalar-gvn.v1",
            OptimizationSafetyClass::ProofCertified,
        ),
        propose = propose_phi_translated,
        super::proof_certified_cse_phi_rows,
    );

    psi_rule_contract!(
        PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule,
        phi_translated_contract(
            b"omega.psi-rule.phi-translated-proof-certified-compatible-policy-scalar-gvn.v1",
            OptimizationSafetyClass::ProofCertified,
        ),
        propose = propose_phi_translated,
        super::compatible_policy_cse_phi_rows,
    );
}
pub(crate) mod same_block_scalar_cse {
    use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

    use super::propose_same_block;
    use crate::rules::global_value_numbering::same_block_contract;
    use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

    psi_rule_contract!(
        SameBlockTotalScalarCseRule,
        same_block_contract(
            b"omega.psi-rule.same-block-obligation-free-total-scalar-cse.v1",
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        propose = propose_same_block,
        super::TOTAL_CSE_KEYS,
    );

    psi_rule_contract!(
        SameBlockProofCertifiedScalarCseRule,
        same_block_contract(
            b"omega.psi-rule.same-block-proof-certified-total-scalar-cse.v1",
            OptimizationSafetyClass::ProofCertified,
        ),
        propose = propose_same_block,
        super::PROOF_CERTIFIED_CSE_KEYS,
    );

    psi_rule_contract!(
        SameBlockProofCertifiedCompatiblePolicyScalarCseRule,
        same_block_contract(
            b"omega.psi-rule.same-block-proof-certified-compatible-policy-scalar-cse.v1",
            OptimizationSafetyClass::ProofCertified,
        ),
        propose = propose_same_block,
        super::COMPATIBLE_POLICY_CSE_KEYS,
    );
}
pub(crate) mod total_scalar_identities {
    use abstract_operations::{
        AbstractOperation, IntegerArithmeticOperator, IntegerOverflowPolicy,
    };
    use optimization_core::{
        AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationRuleContract,
        OptimizationSafetyClass,
    };
    use optimization_unit::{
        NodeLocation, PsiOptimizationUnit, PsiRewriteCandidate, TotalScalarIdentityKind,
        TotalScalarIdentityRewrite,
    };
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
    };

    use crate::rules::GLOBAL_VALUE_NUMBERING_PASS_NAME;
    use crate::rules::global_value_numbering::exact_pure_scalar_effect;
    use crate::rules::node_elision::node_elision_accounting;
    use crate::{AnalysisProduct, RuleAnalysisView, RuleProposalError};

    pub(crate) fn propose_total_scalar_identities(
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
        contract: OptimizationRuleContract,
        classify: fn(&AbstractOperation) -> Vec<TotalScalarIdentityShape>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        required_analysis!(analyses, AnalysisKind::ScalarConstants => Some(AnalysisProduct::ScalarConstants(constants)));
        let (use_definitions, effects) = analyses.use_definitions_and_effects()?;

        let mut candidates = Vec::new();
        for function in &unit.functions {
            let value_types = super::function_value_types(function);
            for block in &function.blocks {
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let shapes = classify(&node.operation);
                    if shapes.is_empty() {
                        continue;
                    }
                    let node_index =
                        u32::try_from(node_index).expect("optimization node index fits u32");
                    if !exact_pure_scalar_effect(
                        unit,
                        effects,
                        function.machine,
                        block.id,
                        node_index,
                    ) {
                        continue;
                    }
                    let Some((shape, constant_fact)) = shapes.into_iter().find_map(|shape| {
                        if value_types.get(&shape.law_operand)
                            != Some(&ScalarType::Integer(shape.law_operand_type))
                        {
                            return None;
                        }
                        let (actual, fact) = constants
                            .literal_integer_constant(function.machine, shape.law_operand)?;
                        (actual == shape.expected_law_value).then_some((shape, fact))
                    }) else {
                        continue;
                    };
                    if !use_definitions.uses.iter().any(|(machine, use_site)| {
                        *machine == function.machine && use_site.value == shape.result
                    }) {
                        continue;
                    }
                    let location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    };
                    let Some((affected_blocks, provenance)) =
                        node_elision_accounting(function, location, shape.result)
                    else {
                        continue;
                    };
                    candidates.push(
                        PsiRewriteCandidate::new_total_scalar_identity(
                            unit.identity,
                            contract,
                            affected_blocks,
                            provenance,
                            constant_fact,
                            -1,
                            TotalScalarIdentityRewrite {
                                location,
                                source_operation: shape.source_operation,
                                result: shape.result,
                                replacement: shape.replacement,
                                scalar_type: shape.scalar_type,
                                identity: shape.identity,
                            },
                        )
                        .map_err(RuleProposalError::InvalidCandidate)?,
                    );
                }
            }
        }
        Ok(candidates)
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct TotalScalarIdentityShape {
        pub source_operation: OperationId,
        pub result: ValueId,
        pub replacement: ValueId,
        pub law_operand: ValueId,
        pub scalar_type: IntegerType,
        pub law_operand_type: IntegerType,
        pub identity: TotalScalarIdentityKind,
        pub expected_law_value: IntegerValue,
    }

    const REQUIRED_ANALYSES: [AnalysisKind; 3] = [
        AnalysisKind::ScalarConstants,
        AnalysisKind::UseDefinition,
        AnalysisKind::EffectSummaries,
    ];

    const INVALIDATED_ANALYSES: [AnalysisKind; 2] =
        [AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries];

    pub(crate) fn exact_total_scalar_identity(identity: &'static [u8]) -> OptimizationRuleContract {
        crate::rules::standard_rule_contract(
            identity,
            GLOBAL_VALUE_NUMBERING_PASS_NAME,
            1,
            AnalysisSet::new(REQUIRED_ANALYSES),
            AnalysisInvalidationSet::new(INVALIDATED_ANALYSES),
            OptimizationSafetyClass::ExactOperationSemantics,
        )
    }

    pub(crate) fn all_ones(scalar_type: IntegerType) -> IntegerValue {
        match scalar_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(-1),
            IntegerSign::Unsigned => scalar_type.maximum_value(),
        }
    }

    pub(crate) fn multiply_zero_annihilation_shapes(
        operation: &AbstractOperation,
        policy: IntegerOverflowPolicy,
        left_zero: TotalScalarIdentityKind,
        right_zero: TotalScalarIdentityKind,
    ) -> Vec<TotalScalarIdentityShape> {
        let AbstractOperation::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            arithmetic,
            ..
        } = operation
        else {
            return Vec::new();
        };
        if !arithmetic.is(policy, IntegerArithmeticOperator::Multiply) {
            return Vec::new();
        }
        vec![
            TotalScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *left,
                law_operand: *left,
                scalar_type: *scalar_type,
                law_operand_type: *scalar_type,
                identity: left_zero,
                expected_law_value: scalar_type.zero(),
            },
            TotalScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *right,
                law_operand: *right,
                scalar_type: *scalar_type,
                law_operand_type: *scalar_type,
                identity: right_zero,
                expected_law_value: scalar_type.zero(),
            },
        ]
    }

    pub(crate) struct NeutralArithmeticKinds {
        pub add_zero_left: TotalScalarIdentityKind,
        pub add_zero_right: TotalScalarIdentityKind,
        pub subtract_zero_right: TotalScalarIdentityKind,
        pub multiply_one_left: TotalScalarIdentityKind,
        pub multiply_one_right: TotalScalarIdentityKind,
    }

    pub(crate) fn neutral_arithmetic_shapes(
        operation: &AbstractOperation,
        policy: IntegerOverflowPolicy,
        kinds: NeutralArithmeticKinds,
    ) -> Vec<TotalScalarIdentityShape> {
        let Some(operands) = operation.integer_arithmetic() else {
            return Vec::new();
        };
        if operands.arithmetic.policy() != policy {
            return Vec::new();
        }
        let zero = operands.scalar_type.zero();
        let one = operands.scalar_type.one();
        let laws = match operands.arithmetic.operator() {
            IntegerArithmeticOperator::Add => vec![
                (operands.right, operands.left, kinds.add_zero_left, zero),
                (operands.left, operands.right, kinds.add_zero_right, zero),
            ],
            IntegerArithmeticOperator::Subtract => vec![(
                operands.left,
                operands.right,
                kinds.subtract_zero_right,
                zero,
            )],
            IntegerArithmeticOperator::Multiply => vec![
                (operands.right, operands.left, kinds.multiply_one_left, one),
                (operands.left, operands.right, kinds.multiply_one_right, one),
            ],
            IntegerArithmeticOperator::Divide | IntegerArithmeticOperator::Remainder => Vec::new(),
        };
        laws.into_iter()
            .map(|(replacement, law_operand, identity, expected_law_value)| {
                TotalScalarIdentityShape {
                    source_operation: operands.psi_operation,
                    result: operands.result,
                    replacement,
                    law_operand,
                    scalar_type: operands.scalar_type,
                    law_operand_type: operands.scalar_type,
                    identity,
                    expected_law_value,
                }
            })
            .collect()
    }
}
pub(crate) mod total_scalar_identity_rules {
    use abstract_operations::{
        AbstractOperation as O, IntegerOverflowPolicy, IntegerShiftDirection, IntegerShiftPolicy,
    };
    use optimization_core::OptimizationRuleContract;
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate, TotalScalarIdentityKind};
    use semantic_vocabulary::{IntegerType, IntegerValue, OperationId, ValueId};

    use crate::rules::global_value_numbering::total_scalar_identities::{
        NeutralArithmeticKinds, TotalScalarIdentityShape, all_ones, exact_total_scalar_identity,
        multiply_zero_annihilation_shapes, neutral_arithmetic_shapes,
        propose_total_scalar_identities,
    };
    use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

    macro_rules! total_scalar_identity_rules {
        ($($(#[$meta:meta])* $name:ident = ($identity:literal, $classify:path);)*) => {$(
            psi_rule_contract!(
                $(#[$meta])*
                $name,
                exact_total_scalar_identity($identity),
                propose = propose_total_scalar_identities,
                $classify,
            );
        )*};
    }

    total_scalar_identity_rules! {
        WrappingNeutralArithmeticIdentityRule = (
            b"omega.psi-rule.live-obligation-free-wrapping-integer-neutral-arithmetic-identity-elimination.v1",
            wrapping_neutral_arithmetic_classify
        );
        SaturatingNeutralArithmeticIdentityRule = (
            b"omega.psi-rule.live-obligation-free-saturating-integer-neutral-arithmetic-identity-elimination.v1",
            saturating_neutral_arithmetic_classify
        );
        WrappingShiftZeroCountIdentityRule = (
            b"omega.psi-rule.live-obligation-free-wrapping-integer-shift-zero-count-elimination.v1",
            wrapping_shift_zero_count_classify
        );
        WrappingMultiplyZeroAnnihilationRule = (
            b"omega.psi-rule.live-obligation-free-wrapping-integer-multiply-zero-annihilation.v1",
            wrapping_multiply_zero_annihilation_classify
        );
        SaturatingMultiplyZeroAnnihilationRule = (
            b"omega.psi-rule.live-obligation-free-saturating-integer-multiply-zero-annihilation.v1",
            saturating_multiply_zero_annihilation_classify
        );
        BitwiseNeutralLiteralIdentityRule = (
            b"omega.psi-rule.live-obligation-free-integer-bitwise-neutral-literal-elimination.v1",
            bitwise_neutral_literal_classify
        );
        BitwiseAbsorbingLiteralIdentityRule = (
            b"omega.psi-rule.live-obligation-free-integer-bitwise-absorbing-literal-elimination.v1",
            bitwise_absorbing_literal_classify
        );
    }
    pub(crate) fn wrapping_neutral_arithmetic_classify(
        operation: &O,
    ) -> Vec<TotalScalarIdentityShape> {
        use TotalScalarIdentityKind as Kind;
        neutral_arithmetic_shapes(
            operation,
            IntegerOverflowPolicy::Wrapping,
            NeutralArithmeticKinds {
                add_zero_left: Kind::WrappingIntegerAddZeroLeft,
                add_zero_right: Kind::WrappingIntegerAddZeroRight,
                subtract_zero_right: Kind::WrappingIntegerSubtractZeroRight,
                multiply_one_left: Kind::WrappingIntegerMultiplyOneLeft,
                multiply_one_right: Kind::WrappingIntegerMultiplyOneRight,
            },
        )
    }

    pub(crate) fn saturating_neutral_arithmetic_classify(
        operation: &O,
    ) -> Vec<TotalScalarIdentityShape> {
        use TotalScalarIdentityKind as Kind;
        neutral_arithmetic_shapes(
            operation,
            IntegerOverflowPolicy::Saturating,
            NeutralArithmeticKinds {
                add_zero_left: Kind::SaturatingIntegerAddZeroLeft,
                add_zero_right: Kind::SaturatingIntegerAddZeroRight,
                subtract_zero_right: Kind::SaturatingIntegerSubtractZeroRight,
                multiply_one_left: Kind::SaturatingIntegerMultiplyOneLeft,
                multiply_one_right: Kind::SaturatingIntegerMultiplyOneRight,
            },
        )
    }

    pub(crate) fn wrapping_shift_zero_count_classify(
        operation: &O,
    ) -> Vec<TotalScalarIdentityShape> {
        let Some(operands) = operation.integer_shift() else {
            return Vec::new();
        };
        if !matches!(operands.policy, IntegerShiftPolicy::Wrapping) {
            return Vec::new();
        }
        let identity = match operands.direction {
            IntegerShiftDirection::Left => {
                TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount
            }
            IntegerShiftDirection::Right => {
                TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount
            }
        };
        vec![TotalScalarIdentityShape {
            source_operation: operands.psi_operation,
            result: operands.result,
            replacement: operands.value,
            law_operand: operands.count,
            scalar_type: operands.value_type,
            law_operand_type: operands.count_type,
            identity,
            expected_law_value: operands.count_type.zero(),
        }]
    }

    pub(crate) fn wrapping_multiply_zero_annihilation_classify(
        operation: &O,
    ) -> Vec<TotalScalarIdentityShape> {
        multiply_zero_annihilation_shapes(
            operation,
            IntegerOverflowPolicy::Wrapping,
            TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft,
            TotalScalarIdentityKind::WrappingIntegerMultiplyZeroRight,
        )
    }

    pub(crate) fn saturating_multiply_zero_annihilation_classify(
        operation: &O,
    ) -> Vec<TotalScalarIdentityShape> {
        multiply_zero_annihilation_shapes(
            operation,
            IntegerOverflowPolicy::Saturating,
            TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft,
            TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroRight,
        )
    }

    pub(crate) fn bitwise_neutral_literal_classify(operation: &O) -> Vec<TotalScalarIdentityShape> {
        use TotalScalarIdentityKind as Kind;
        match operation {
            O::IntegerBitwiseAnd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => bitwise_pair_shapes(
                *psi_operation,
                *result,
                *scalar_type,
                [
                    (*right, *left, Kind::IntegerBitwiseAndAllOnesLeft),
                    (*left, *right, Kind::IntegerBitwiseAndAllOnesRight),
                ],
                all_ones(*scalar_type),
            ),
            O::IntegerBitwiseOr {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => bitwise_pair_shapes(
                *psi_operation,
                *result,
                *scalar_type,
                [
                    (*right, *left, Kind::IntegerBitwiseOrZeroLeft),
                    (*left, *right, Kind::IntegerBitwiseOrZeroRight),
                ],
                scalar_type.zero(),
            ),
            O::IntegerBitwiseXor {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => bitwise_pair_shapes(
                *psi_operation,
                *result,
                *scalar_type,
                [
                    (*right, *left, Kind::IntegerBitwiseXorZeroLeft),
                    (*left, *right, Kind::IntegerBitwiseXorZeroRight),
                ],
                scalar_type.zero(),
            ),
            _ => Vec::new(),
        }
    }

    fn bitwise_pair_shapes(
        source_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        candidates: [(ValueId, ValueId, TotalScalarIdentityKind); 2],
        law_value: IntegerValue,
    ) -> Vec<TotalScalarIdentityShape> {
        candidates
            .into_iter()
            .map(
                |(replacement, law_operand, identity)| TotalScalarIdentityShape {
                    source_operation,
                    result,
                    replacement,
                    law_operand,
                    scalar_type,
                    law_operand_type: scalar_type,
                    identity,
                    expected_law_value: law_value,
                },
            )
            .collect()
    }

    pub(crate) fn bitwise_absorbing_literal_classify(
        operation: &O,
    ) -> Vec<TotalScalarIdentityShape> {
        use TotalScalarIdentityKind as Kind;
        match operation {
            O::IntegerBitwiseAnd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => bitwise_pair_shapes(
                *psi_operation,
                *result,
                *scalar_type,
                [
                    (*left, *left, Kind::IntegerBitwiseAndZeroLeft),
                    (*right, *right, Kind::IntegerBitwiseAndZeroRight),
                ],
                scalar_type.zero(),
            ),
            O::IntegerBitwiseOr {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => bitwise_pair_shapes(
                *psi_operation,
                *result,
                *scalar_type,
                [
                    (*left, *left, Kind::IntegerBitwiseOrAllOnesLeft),
                    (*right, *right, Kind::IntegerBitwiseOrAllOnesRight),
                ],
                all_ones(*scalar_type),
            ),
            _ => Vec::new(),
        }
    }
}

pub(crate) use dominating_scalar_gvn::{
    DominatorProofCertifiedCompatiblePolicyScalarGvnRule, DominatorProofCertifiedScalarGvnRule,
    DominatorTotalScalarGvnRule,
};
pub(crate) use phi_translated_scalar_gvn::{
    PhiTranslatedObligationFreeScalarGvnRule,
    PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule,
    PhiTranslatedProofCertifiedScalarGvnRule,
};
pub(crate) use same_block_scalar_cse::{
    SameBlockProofCertifiedCompatiblePolicyScalarCseRule, SameBlockProofCertifiedScalarCseRule,
    SameBlockTotalScalarCseRule,
};
pub(crate) use total_scalar_identity_rules::{
    BitwiseAbsorbingLiteralIdentityRule, BitwiseNeutralLiteralIdentityRule,
    SaturatingMultiplyZeroAnnihilationRule, SaturatingNeutralArithmeticIdentityRule,
    WrappingMultiplyZeroAnnihilationRule, WrappingNeutralArithmeticIdentityRule,
    WrappingShiftZeroCountIdentityRule,
};

pub(crate) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![
        BuiltInRuleRegistration::new(0, SameBlockTotalScalarCseRule),
        BuiltInRuleRegistration::new(1, SameBlockProofCertifiedScalarCseRule),
        BuiltInRuleRegistration::new(2, DominatorTotalScalarGvnRule),
        BuiltInRuleRegistration::new(3, DominatorProofCertifiedScalarGvnRule),
        BuiltInRuleRegistration::new(4, PhiTranslatedObligationFreeScalarGvnRule),
        BuiltInRuleRegistration::new(5, PhiTranslatedProofCertifiedScalarGvnRule),
        BuiltInRuleRegistration::new(6, SameBlockProofCertifiedCompatiblePolicyScalarCseRule),
        BuiltInRuleRegistration::new(7, DominatorProofCertifiedCompatiblePolicyScalarGvnRule),
        BuiltInRuleRegistration::new(8, PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule),
        BuiltInRuleRegistration::new(9, WrappingNeutralArithmeticIdentityRule),
        BuiltInRuleRegistration::new(10, WrappingShiftZeroCountIdentityRule),
        BuiltInRuleRegistration::new(11, WrappingMultiplyZeroAnnihilationRule),
        BuiltInRuleRegistration::new(12, SaturatingNeutralArithmeticIdentityRule),
        BuiltInRuleRegistration::new(13, SaturatingMultiplyZeroAnnihilationRule),
        BuiltInRuleRegistration::new(14, BitwiseNeutralLiteralIdentityRule),
        BuiltInRuleRegistration::new(15, BitwiseAbsorbingLiteralIdentityRule),
    ]
}

pub(crate) fn same_block_contract(
    rule_name: &'static [u8],
    safety: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    standard_rule_contract(
        rule_name,
        GLOBAL_VALUE_NUMBERING_PASS_NAME,
        1,
        AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
        safety,
    )
}

pub(crate) fn dominating_contract(
    rule_name: &'static [u8],
    safety: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    standard_rule_contract(
        rule_name,
        GLOBAL_VALUE_NUMBERING_PASS_NAME,
        1,
        AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::Dominators,
            AnalysisKind::UseDefinition,
            AnalysisKind::EffectSummaries,
        ]),
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
        safety,
    )
}

pub(crate) fn phi_translated_contract(
    rule_name: &'static [u8],
    safety: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    dominating_contract(rule_name, safety)
}

pub(crate) fn exact_pure_scalar_effect(
    unit: &PsiOptimizationUnit,
    effects: &EffectSummaryAnalysis,
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
            && row.observable == EffectKnowledge::No
            && row.structural_state == EffectKnowledge::No
            && row.crash == EffectKnowledge::No
            && row.suspension == EffectKnowledge::No
    })
}

fn function_value_types(function: &PsiOptimizationFunction) -> BTreeMap<ValueId, ScalarType> {
    function
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
        .collect()
}

pub(crate) struct SameBlockScalarCseMatch<E> {
    pub affected_blocks: Vec<BlockId>,
    pub provenance: Vec<ProvenanceRewrite>,
    pub extras: E,
    pub patch: LocalScalarCommonSubexpressionRewrite,
}

pub(crate) fn propose_same_block_scalar_cse<K, E>(
    unit: &PsiOptimizationUnit,
    analyses: crate::RuleAnalysisView<'_>,
    leader: impl Fn(
        &ScalarCseValueTypes,
        &PsiOptimizationUnit,
        &PsiOptimizationFunction,
        &AbstractOperation,
    ) -> Option<ScalarExpressionRow<K>>,
    redundant: impl Fn(
        &ScalarCseValueTypes,
        &PsiOptimizationUnit,
        &PsiOptimizationFunction,
        &AbstractOperation,
    ) -> Option<(ScalarExpressionRow<K>, E)>,
    emit: impl Fn(SameBlockScalarCseMatch<E>) -> Result<PsiRewriteCandidate, RuleProposalError>,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError>
where
    K: Ord,
    E: Copy,
{
    use crate::rules::node_elision::node_elision_accounting;

    let (use_definitions, effects) = analyses.use_definitions_and_effects()?;
    let mut candidates = Vec::new();
    for function in &unit.functions {
        let value_types = function_value_types(function);
        for block in &function.blocks {
            let mut leaders = BTreeMap::new();
            for (index, node) in block.nodes.iter().enumerate() {
                let leader_row = leader(&value_types, unit, function, &node.operation);
                let redundant_row = redundant(&value_types, unit, function, &node.operation);
                if leader_row.is_none() && redundant_row.is_none() {
                    continue;
                }
                let node_index = u32::try_from(index).expect("optimization node index fits u32");
                if !exact_pure_scalar_effect(unit, effects, function.machine, block.id, node_index)
                {
                    continue;
                }
                let mut establishes_leader = false;
                if let Some(row) = leader_row
                    && !leaders.contains_key(&row.0)
                {
                    leaders.insert(row.0, (node_index, row.1, row.2, row.3));
                    establishes_leader = true;
                }
                if establishes_leader {
                    continue;
                }
                let Some((row, extras)) = redundant_row else {
                    continue;
                };
                let (key, operation, result, scalar_type) = row;
                let Some((leader, leader_operation, leader_result, leader_type)) =
                    leaders.get(&key).copied()
                else {
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
                let leader_location = optimization_unit::NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: leader,
                };
                let redundant_location = optimization_unit::NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                };
                let Some((affected_blocks, provenance)) =
                    node_elision_accounting(function, redundant_location, result)
                else {
                    continue;
                };
                candidates.push(emit(SameBlockScalarCseMatch {
                    affected_blocks,
                    provenance,
                    extras,
                    patch: LocalScalarCommonSubexpressionRewrite {
                        leader: leader_location,
                        redundant: redundant_location,
                        leader_operation,
                        redundant_operation: operation,
                        leader_result,
                        redundant_result: result,
                        scalar_type,
                    },
                })?);
            }
        }
    }
    Ok(candidates)
}

pub(crate) struct DominatingScalarGvnMatch<E> {
    pub affected_blocks: Vec<BlockId>,
    pub provenance: Vec<ProvenanceRewrite>,
    pub extras: E,
    pub patch: DominatingScalarCommonSubexpressionRewrite,
}

pub(crate) fn propose_dominating_scalar_gvn<K, E>(
    unit: &PsiOptimizationUnit,
    analyses: crate::RuleAnalysisView<'_>,
    leader: impl Fn(
        &ScalarCseValueTypes,
        &PsiOptimizationUnit,
        &PsiOptimizationFunction,
        &AbstractOperation,
    ) -> Option<ScalarExpressionRow<K>>,
    redundant: impl Fn(
        &ScalarCseValueTypes,
        &PsiOptimizationUnit,
        &PsiOptimizationFunction,
        &AbstractOperation,
    ) -> Option<(ScalarExpressionRow<K>, E)>,
    emit: impl Fn(DominatingScalarGvnMatch<E>) -> Result<PsiRewriteCandidate, RuleProposalError>,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError>
where
    K: Ord,
    E: Copy,
{
    use crate::rules::node_elision::node_elision_accounting;

    required_analysis!(analyses, AnalysisKind::Dominators => Some(AnalysisProduct::Dominators(dominators)));
    let (use_definitions, effects) = analyses.use_definitions_and_effects()?;
    let mut candidates = Vec::new();
    for function in &unit.functions {
        let machine_dominators = dominators.rows(function.machine);
        let value_types = function_value_types(function);
        let mut leaders = Vec::new();
        let mut expressions = Vec::new();
        for block in &function.blocks {
            for (index, node) in block.nodes.iter().enumerate() {
                let leader_row = leader(&value_types, unit, function, &node.operation);
                let redundant_row = redundant(&value_types, unit, function, &node.operation);
                if leader_row.is_none() && redundant_row.is_none() {
                    continue;
                }
                let node_index = u32::try_from(index).expect("optimization node index fits u32");
                if !exact_pure_scalar_effect(unit, effects, function.machine, block.id, node_index)
                {
                    continue;
                }
                let location = optimization_unit::NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                };
                if let Some(row) = leader_row {
                    leaders.push((row.0, location, row.1, row.2, row.3));
                }
                if let Some((row, extras)) = redundant_row {
                    expressions.push((row.0, location, row.1, row.2, row.3, extras));
                }
            }
        }
        for (key, redundant, redundant_operation, redundant_result, scalar_type, extras) in
            &expressions
        {
            if !use_definitions.uses.iter().any(|(machine, use_site)| {
                *machine == function.machine && use_site.value == *redundant_result
            }) {
                continue;
            }
            let Some(redundant_block) = function.block(redundant.block) else {
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
            let replacement_definition = ValueDefinition {
                value: *leader_result,
                scalar_type: *scalar_type,
                site: ValueDefinitionSite::Node {
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
                    ValueDefinitionSite::Node { block, node } if block == use_site.block => {
                        node < use_site.node
                    }
                    ValueDefinitionSite::Node { block, .. } => {
                        block_dominates(machine_dominators, block, use_site.block)
                    }
                    _ => false,
                })
            {
                continue;
            }
            let Some((affected_blocks, provenance)) =
                node_elision_accounting(function, *redundant, *redundant_result)
            else {
                continue;
            };
            candidates.push(emit(DominatingScalarGvnMatch {
                affected_blocks,
                provenance,
                extras: *extras,
                patch: DominatingScalarCommonSubexpressionRewrite {
                    leader: *leader,
                    redundant: *redundant,
                    leader_operation: *leader_operation,
                    redundant_operation: *redundant_operation,
                    leader_result: *leader_result,
                    redundant_result: *redundant_result,
                    scalar_type: *scalar_type,
                },
            })?);
        }
    }
    Ok(candidates)
}

pub(crate) type PhiTranslatedScalarGvnRows<K, E> = (
    Vec<(K, NodeLocation, OperationId, ValueId, ScalarType)>,
    Vec<(K, NodeLocation, OperationId, ValueId, ScalarType, E)>,
);

pub(crate) struct PhiTranslatedScalarGvnMatch<E> {
    pub affected_blocks: Vec<BlockId>,
    pub provenance: Vec<ProvenanceRewrite>,
    pub extras: E,
    pub patch: PhiTranslatedScalarGvnRewrite,
}

pub(crate) fn propose_phi_translated_scalar_gvn<K, E>(
    unit: &PsiOptimizationUnit,
    analyses: crate::RuleAnalysisView<'_>,
    rows: impl Fn(
        &PsiOptimizationUnit,
        &PsiOptimizationFunction,
        &EffectSummaryAnalysis,
        &BTreeMap<ValueId, ScalarType>,
    ) -> PhiTranslatedScalarGvnRows<K, E>,
    emit: impl Fn(PhiTranslatedScalarGvnMatch<E>) -> Result<PsiRewriteCandidate, RuleProposalError>,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError>
where
    K: ScalarExpressionKey + Ord,
    E: Copy,
{
    required_analysis!(analyses, AnalysisKind::ControlFlowGraph => Some(_));
    required_analysis!(analyses, AnalysisKind::Dominators => Some(AnalysisProduct::Dominators(dominators)));
    let (use_definitions, effects) = analyses.use_definitions_and_effects()?;
    let mut candidates = Vec::new();
    for function in &unit.functions {
        let machine_dominators = dominators.rows(function.machine);
        let value_types = function_value_types(function);
        let (leaders, redundants) = rows(unit, function, effects, &value_types);
        for (key, redundant, redundant_operation, redundant_result, scalar_type, extras) in
            redundants
        {
            let Some((incoming, affected_blocks, provenance, parameter_position)) =
                phi_translated_accounting::collect_phi_translated_incoming(
                    function,
                    redundant,
                    redundant_result,
                    scalar_type,
                    use_definitions,
                    &value_types,
                    machine_dominators,
                    &leaders,
                    |parameters| key.references_parameter(parameters),
                    |translation| key.translated(translation),
                )
            else {
                continue;
            };
            candidates.push(emit(PhiTranslatedScalarGvnMatch {
                affected_blocks,
                provenance,
                extras,
                patch: PhiTranslatedScalarGvnRewrite {
                    redundant,
                    redundant_operation,
                    redundant_result,
                    scalar_type,
                    parameter_position,
                    incoming,
                },
            })?);
        }
    }
    Ok(candidates)
}

pub(crate) fn collect_scalar_rows<K, E>(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    effects: &EffectSummaryAnalysis,
    leader: impl Fn(&AbstractOperation) -> Option<ScalarExpressionRow<K>>,
    redundant: impl Fn(&AbstractOperation) -> Option<(ScalarExpressionRow<K>, E)>,
) -> PhiTranslatedScalarGvnRows<K, E> {
    let mut leaders = Vec::new();
    let mut redundants = Vec::new();
    for block in &function.blocks {
        for (index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(index).expect("optimization node index fits u32");
            if !exact_pure_scalar_effect(unit, effects, function.machine, block.id, node_index) {
                continue;
            }
            let location = NodeLocation {
                machine: function.machine,
                block: block.id,
                node: node_index,
            };
            if let Some((key, operation, result, scalar_type)) = leader(&node.operation) {
                leaders.push((key, location, operation, result, scalar_type));
            }
            if let Some(((key, operation, result, scalar_type), extras)) =
                redundant(&node.operation)
            {
                redundants.push((key, location, operation, result, scalar_type, extras));
            }
        }
    }
    (leaders, redundants)
}

pub(crate) type ScalarCsePayload = Option<AcceptedObligationFactIdentity>;

pub(crate) type ScalarCseValueTypes = BTreeMap<ValueId, ScalarType>;

pub(crate) struct ScalarCseKeys<K> {
    pub leader: fn(
        &ScalarCseValueTypes,
        &PsiOptimizationUnit,
        &PsiOptimizationFunction,
        &AbstractOperation,
    ) -> Option<ScalarExpressionRow<K>>,
    pub redundant: fn(
        &ScalarCseValueTypes,
        &PsiOptimizationUnit,
        &PsiOptimizationFunction,
        &AbstractOperation,
    ) -> Option<(ScalarExpressionRow<K>, ScalarCsePayload)>,
}

pub(crate) const TOTAL_CSE_KEYS: ScalarCseKeys<expression_keys::TotalScalarExpressionKey> =
    ScalarCseKeys {
        leader: |value_types, _unit, _function, operation| {
            expression_keys::total_scalar_expression(operation, value_types)
        },
        redundant: |value_types, _unit, _function, operation| {
            expression_keys::total_scalar_expression(operation, value_types).map(|row| (row, None))
        },
    };

pub(crate) const PROOF_CERTIFIED_CSE_KEYS: ScalarCseKeys<
    expression_keys::ProofCertifiedScalarExpressionKey,
> = ScalarCseKeys {
    leader: |_value_types, unit, function, operation| {
        let row = expression_keys::proof_certified_scalar_expression(operation)?;
        accepted_obligations::accepted_obligation_fact(unit, function.machine, row.1).ok()?;
        Some(row)
    },
    redundant: |_value_types, unit, function, operation| {
        let row = expression_keys::proof_certified_scalar_expression(operation)?;
        let fact =
            accepted_obligations::accepted_obligation_fact(unit, function.machine, row.1).ok()?;
        Some((row, Some(fact)))
    },
};

pub(crate) const COMPATIBLE_POLICY_CSE_KEYS: ScalarCseKeys<
    expression_keys::CompatiblePolicyScalarExpressionKey,
> = ScalarCseKeys {
    leader: |_value_types, _unit, function, operation| {
        let row = expression_keys::compatible_policy_scalar_leader(operation)?;
        if function.facts.iter().any(|fact| {
            matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                if *support == row.1)
        }) {
            return None;
        }
        Some(row)
    },
    redundant: |_value_types, unit, function, operation| {
        let row = expression_keys::compatible_policy_scalar_redundant(operation)?;
        let fact =
            accepted_obligations::accepted_obligation_fact(unit, function.machine, row.1).ok()?;
        Some((row, Some(fact)))
    },
};

pub(crate) fn propose_same_block<K>(
    unit: &PsiOptimizationUnit,
    analyses: crate::RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    keys: ScalarCseKeys<K>,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError>
where
    K: Ord,
{
    propose_same_block_scalar_cse(unit, analyses, keys.leader, keys.redundant, |same| {
        match same.extras {
            Some(obligation_fact) => {
                PsiRewriteCandidate::new_proof_certified_local_scalar_common_subexpression(
                    unit.identity,
                    contract,
                    same.affected_blocks,
                    same.provenance,
                    obligation_fact,
                    -1,
                    same.patch,
                )
            }
            None => PsiRewriteCandidate::new_local_scalar_common_subexpression(
                unit.identity,
                contract,
                same.affected_blocks,
                same.provenance,
                -1,
                same.patch,
            ),
        }
        .map_err(RuleProposalError::InvalidCandidate)
    })
}

pub(crate) fn propose_dominating<K>(
    unit: &PsiOptimizationUnit,
    analyses: crate::RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    keys: ScalarCseKeys<K>,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError>
where
    K: Ord,
{
    propose_dominating_scalar_gvn(unit, analyses, keys.leader, keys.redundant, |same| {
        match same.extras {
            Some(obligation_fact) => {
                PsiRewriteCandidate::new_proof_certified_dominating_scalar_common_subexpression(
                    unit.identity,
                    contract,
                    same.affected_blocks,
                    same.provenance,
                    obligation_fact,
                    -1,
                    same.patch,
                )
            }
            None => PsiRewriteCandidate::new_dominating_scalar_common_subexpression(
                unit.identity,
                contract,
                same.affected_blocks,
                same.provenance,
                -1,
                same.patch,
            ),
        }
        .map_err(RuleProposalError::InvalidCandidate)
    })
}

pub(crate) fn propose_phi_translated<K>(
    unit: &PsiOptimizationUnit,
    analyses: crate::RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    rows: fn(
        &PsiOptimizationUnit,
        &PsiOptimizationFunction,
        &EffectSummaryAnalysis,
        &ScalarCseValueTypes,
    ) -> PhiTranslatedScalarGvnRows<K, ScalarCsePayload>,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError>
where
    K: ScalarExpressionKey + Ord,
{
    propose_phi_translated_scalar_gvn(unit, analyses, rows, |found| {
        match found.extras {
            Some(obligation_fact) => {
                PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
                    unit.identity,
                    contract,
                    found.affected_blocks,
                    found.provenance,
                    obligation_fact,
                    -1,
                    found.patch,
                )
            }
            None => PsiRewriteCandidate::new_phi_translated_scalar_common_subexpression(
                unit.identity,
                contract,
                found.affected_blocks,
                found.provenance,
                -1,
                found.patch,
            ),
        }
        .map_err(RuleProposalError::InvalidCandidate)
    })
}

pub(crate) fn total_cse_phi_rows(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    effects: &EffectSummaryAnalysis,
    value_types: &ScalarCseValueTypes,
) -> PhiTranslatedScalarGvnRows<expression_keys::TotalScalarExpressionKey, ScalarCsePayload> {
    let keys = TOTAL_CSE_KEYS;
    collect_scalar_rows(
        unit,
        function,
        effects,
        |operation| (keys.leader)(value_types, unit, function, operation),
        |operation| (keys.redundant)(value_types, unit, function, operation),
    )
}

pub(crate) fn proof_certified_cse_phi_rows(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    effects: &EffectSummaryAnalysis,
    value_types: &ScalarCseValueTypes,
) -> PhiTranslatedScalarGvnRows<expression_keys::ProofCertifiedScalarExpressionKey, ScalarCsePayload>
{
    let keys = PROOF_CERTIFIED_CSE_KEYS;
    collect_scalar_rows(
        unit,
        function,
        effects,
        |operation| (keys.leader)(value_types, unit, function, operation),
        |operation| (keys.redundant)(value_types, unit, function, operation),
    )
}

pub(crate) fn compatible_policy_cse_phi_rows(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    effects: &EffectSummaryAnalysis,
    value_types: &ScalarCseValueTypes,
) -> PhiTranslatedScalarGvnRows<
    expression_keys::CompatiblePolicyScalarExpressionKey,
    ScalarCsePayload,
> {
    let keys = COMPATIBLE_POLICY_CSE_KEYS;
    let (leaders, redundants) = collect_scalar_rows(
        unit,
        function,
        effects,
        |operation| (keys.leader)(value_types, unit, function, operation),
        |operation| (keys.redundant)(value_types, unit, function, operation),
    );
    let redundants = redundants
        .into_iter()
        .filter(|row| row.1.block != function.entry)
        .collect();
    (leaders, redundants)
}
