//! Optimizer module role: executable entrance. Sparse conditional constant
//! propagation: the pass's exact local rule order over its six rule
//! families.
//!
//! `integer_binary_constants`, `integer_unary_constants`,
//! `integer_cast_constants` and `boolean_constants` fold operations whose
//! operands are scalar constants; `range_against_constant` and
//! `range_against_range` decide comparisons from closed integer ranges. Each
//! family file is a table of rules over one traversal. This file owns the
//! two rule contracts the families share, the scalar-constant fact lookup,
//! and the closed comparison vocabulary. The parent stage catalog selects
//! this pass; `built_in_registrations` is the only local rule-order point.

/// One constant-fold rule: its type, canonical identity, safety class and
/// the closed operation kind its family's `propose` folds. Expanded at each
/// family's table below; `contract` and `propose` resolve in the invoking
/// family.
macro_rules! constant_fold_rules {
    ($($name:ident = ($identity:literal, $safety:ident, $kind:expr);)*) => {$(
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        impl $name {
            pub fn contract() -> OptimizationRuleContract {
                contract($identity, OptimizationSafetyClass::$safety)
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
                propose(unit, analyses, Self::contract(), $kind)
            }
        }
    )*};
}

pub(crate) mod boolean_constants;
pub(crate) mod integer_binary_constants;
pub(crate) mod integer_binary_shapes;
pub(crate) mod integer_cast_constants;
pub(crate) mod integer_unary_constants;
pub(crate) mod range_against_constant;
pub(crate) mod range_against_range;

pub use boolean_constants::{
    BooleanEqualConstantsRule, BooleanNotConstantsRule, IntegerEqualConstantsRule,
    IntegerLessOrEqualConstantsRule, IntegerLessThanConstantsRule,
};
pub use integer_binary_constants::{
    ExactIntegerAddConstantsRule, ExactIntegerDivideConstantsRule,
    ExactIntegerMultiplyConstantsRule, ExactIntegerRemainderConstantsRule,
    ExactIntegerShiftLeftConstantsRule, ExactIntegerShiftRightConstantsRule,
    ExactIntegerSubtractConstantsRule, IntegerBitwiseAndConstantsRule,
    IntegerBitwiseOrConstantsRule, IntegerBitwiseXorConstantsRule,
    SaturatingIntegerAddConstantsRule, SaturatingIntegerDivideConstantsRule,
    SaturatingIntegerMultiplyConstantsRule, SaturatingIntegerRemainderConstantsRule,
    SaturatingIntegerSubtractConstantsRule, WrappingIntegerAddConstantsRule,
    WrappingIntegerDivideConstantsRule, WrappingIntegerMultiplyConstantsRule,
    WrappingIntegerRemainderConstantsRule, WrappingIntegerShiftLeftConstantsRule,
    WrappingIntegerShiftRightConstantsRule, WrappingIntegerSubtractConstantsRule,
};
pub use integer_cast_constants::ExactIntegerCastConstantsRule;
pub use integer_unary_constants::{IntegerBitwiseNotConstantsRule, IntegerWidenConstantsRule};
pub use range_against_constant::{
    IntegerEqualConstantRangeRule, IntegerEqualRangeConstantRule,
    IntegerLessOrEqualConstantRangeRule, IntegerLessOrEqualRangeConstantRule,
    IntegerLessThanConstantRangeRule, IntegerLessThanRangeConstantRule,
};
pub use range_against_range::{
    IntegerEqualRangeRangeRule, IntegerLessOrEqualRangeRangeRule, IntegerLessThanRangeRangeRule,
};

use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
    ScalarConstantFactIdentity,
};
use semantic_vocabulary::{IntegerValue, MachineId, ValueId};

use crate::rules::SCCP_PASS_NAME;
use crate::rules::catalog::BuiltInRuleRegistration;
use crate::{ScalarConstant, ScalarConstantAnalysis};

/// The exact local rule order for this pass.
pub(super) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![
        BuiltInRuleRegistration::new(0, ExactIntegerAddConstantsRule),
        BuiltInRuleRegistration::new(1, ExactIntegerSubtractConstantsRule),
        BuiltInRuleRegistration::new(2, ExactIntegerMultiplyConstantsRule),
        BuiltInRuleRegistration::new(3, WrappingIntegerAddConstantsRule),
        BuiltInRuleRegistration::new(4, WrappingIntegerSubtractConstantsRule),
        BuiltInRuleRegistration::new(5, WrappingIntegerMultiplyConstantsRule),
        BuiltInRuleRegistration::new(6, SaturatingIntegerAddConstantsRule),
        BuiltInRuleRegistration::new(7, SaturatingIntegerSubtractConstantsRule),
        BuiltInRuleRegistration::new(8, SaturatingIntegerMultiplyConstantsRule),
        BuiltInRuleRegistration::new(9, ExactIntegerDivideConstantsRule),
        BuiltInRuleRegistration::new(10, ExactIntegerRemainderConstantsRule),
        BuiltInRuleRegistration::new(11, WrappingIntegerDivideConstantsRule),
        BuiltInRuleRegistration::new(12, WrappingIntegerRemainderConstantsRule),
        BuiltInRuleRegistration::new(13, SaturatingIntegerDivideConstantsRule),
        BuiltInRuleRegistration::new(14, SaturatingIntegerRemainderConstantsRule),
        BuiltInRuleRegistration::new(15, ExactIntegerShiftLeftConstantsRule),
        BuiltInRuleRegistration::new(16, ExactIntegerShiftRightConstantsRule),
        BuiltInRuleRegistration::new(17, WrappingIntegerShiftLeftConstantsRule),
        BuiltInRuleRegistration::new(18, WrappingIntegerShiftRightConstantsRule),
        BuiltInRuleRegistration::new(19, ExactIntegerCastConstantsRule),
        BuiltInRuleRegistration::new(20, IntegerWidenConstantsRule),
        BuiltInRuleRegistration::new(21, IntegerBitwiseNotConstantsRule),
        BuiltInRuleRegistration::new(22, IntegerBitwiseAndConstantsRule),
        BuiltInRuleRegistration::new(23, IntegerBitwiseOrConstantsRule),
        BuiltInRuleRegistration::new(24, IntegerBitwiseXorConstantsRule),
        BuiltInRuleRegistration::new(25, BooleanNotConstantsRule),
        BuiltInRuleRegistration::new(26, BooleanEqualConstantsRule),
        BuiltInRuleRegistration::new(27, IntegerEqualConstantsRule),
        BuiltInRuleRegistration::new(28, IntegerLessThanConstantsRule),
        BuiltInRuleRegistration::new(29, IntegerLessOrEqualConstantsRule),
        BuiltInRuleRegistration::new(30, IntegerLessThanRangeConstantRule),
        BuiltInRuleRegistration::new(31, IntegerLessThanConstantRangeRule),
        BuiltInRuleRegistration::new(32, IntegerLessOrEqualRangeConstantRule),
        BuiltInRuleRegistration::new(33, IntegerLessOrEqualConstantRangeRule),
        BuiltInRuleRegistration::new(34, IntegerEqualRangeConstantRule),
        BuiltInRuleRegistration::new(35, IntegerEqualConstantRangeRule),
        BuiltInRuleRegistration::new(36, IntegerEqualRangeRangeRule),
        BuiltInRuleRegistration::new(37, IntegerLessThanRangeRangeRule),
        BuiltInRuleRegistration::new(38, IntegerLessOrEqualRangeRangeRule),
    ]
}

pub(super) fn constant_evaluation_contract(
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

/// The contract shared by the range-comparison families: proof-certified,
/// over `analyses`, invalidating use-definition facts.
pub(super) fn range_comparison_contract(
    identity: &'static [u8],
    analyses: AnalysisSet,
) -> OptimizationRuleContract {
    OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(identity),
        OptimizationPassIdentity::from_canonical_bytes(SCCP_PASS_NAME),
        1,
        analyses,
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition]),
        OptimizationSafetyClass::ProofCertified,
    )
    .expect("built-in rule has nonzero version")
}

pub(super) fn integer_constant(
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

pub(super) fn integer_value_type(
    function: &optimization_unit::PsiOptimizationFunction,
    value: ValueId,
) -> Option<semantic_vocabulary::IntegerType> {
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
                    semantic_vocabulary::ScalarType::Integer(integer) => Some(integer),
                    semantic_vocabulary::ScalarType::Boolean
                    | semantic_vocabulary::ScalarType::IeeeFloat(_) => None,
                })
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntegerRangeComparisonKind {
    RangeEqualConstant,
    ConstantEqualRange,
    RangeLessThanConstant,
    ConstantLessThanRange,
    RangeLessOrEqualConstant,
    ConstantLessOrEqualRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntegerRangePairComparisonKind {
    Equal,
    LessThan,
    LessOrEqual,
}
