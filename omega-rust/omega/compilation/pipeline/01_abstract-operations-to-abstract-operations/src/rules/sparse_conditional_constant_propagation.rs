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
pub(crate) use integer_cast_constants::ExactIntegerCastConstantsRule;
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
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationRuleContract,
    OptimizationSafetyClass, ScalarConstantFactIdentity,
};
use optimization_unit::{
    IntegerConstantRewrite, IntegerEvaluationWitness, NodeLocation, PsiOptimizationUnit,
    PsiRewriteCandidate,
};
use semantic_vocabulary::{BlockId, IntegerType, IntegerValue, MachineId, OperationId, ValueId};

use crate::rules::SCCP_PASS_NAME;
use crate::rules::catalog::BuiltInRuleRegistration;
use crate::rules::control_flow_cleanup::node_realized_provenance;
use crate::{
    AnalysisProduct, RuleAnalysisView, RuleProposalError, ScalarConstantAnalysis,
    ScalarConstantValue,
};

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
    rule_name: &'static [u8],
    safety_class: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    crate::rules::standard_rule_contract(
        rule_name,
        SCCP_PASS_NAME,
        1,
        AnalysisSet::new([AnalysisKind::ScalarConstants]),
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition]),
        safety_class,
    )
}

/// One integer constant-fold row: the exact operation and result the rewrite
/// replaces, the folded constant, and the evaluation witness the rule's own
/// classification admits.
pub(super) struct IntegerConstantFold {
    pub(super) source_operation: OperationId,
    pub(super) result: ValueId,
    pub(super) scalar_type: IntegerType,
    pub(super) constant: IntegerValue,
    pub(super) witness: IntegerEvaluationWitness,
}

/// The family's one integer constant-fold traversal: every node is offered to
/// the invoking rule's own classification and folding, and each accepted fold
/// emits its `IntegerConstantRewrite` candidate with the node's in-place
/// provenance. `fold` owns the rule's operand lookup, safety gate and witness.
pub(super) fn propose_integer_constant_folds(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    fold: impl Fn(
        &PsiOptimizationUnit,
        MachineId,
        &optimization_unit::OptimizationNode,
        &ScalarConstantAnalysis,
    ) -> Result<Option<IntegerConstantFold>, RuleProposalError>,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    required_analysis!(analyses, AnalysisKind::ScalarConstants => Some(AnalysisProduct::ScalarConstants(constants)));
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let Some(row) = fold(unit, function.machine, node, constants)? else {
                    continue;
                };
                let location = node_location(
                    function.machine,
                    block.id,
                    u32::try_from(node_index).expect("optimization node indices are u32"),
                );
                candidates.push(
                    PsiRewriteCandidate::new_integer_evaluation(
                        unit.identity,
                        contract,
                        vec![block.id],
                        Vec::new(),
                        vec![node_realized_provenance(location, node)],
                        row.witness,
                        -1,
                        IntegerConstantRewrite {
                            location,
                            source_operation: row.source_operation,
                            result: row.result,
                            scalar_type: row.scalar_type,
                            constant: row.constant,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}

/// The contract shared by the range-comparison families: proof-certified,
/// over `analyses`, invalidating use-definition facts.
pub(super) fn range_comparison_contract(
    identity: &'static [u8],
    analyses: AnalysisSet,
) -> OptimizationRuleContract {
    crate::rules::standard_rule_contract(
        identity,
        SCCP_PASS_NAME,
        1,
        analyses,
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition]),
        OptimizationSafetyClass::ProofCertified,
    )
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
                ScalarConstantValue::Integer(value) => {
                    fact.identity.map(|identity| (value, identity))
                }
                ScalarConstantValue::Boolean(_) => None,
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
        .chain(function.block_parameters())
        .chain(function.node_definitions())
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

/// One node-occurrence location shared by every candidate-producing match arm
/// in this family.
pub(super) fn node_location(machine: MachineId, block: BlockId, node: u32) -> NodeLocation {
    NodeLocation {
        machine,
        block,
        node,
    }
}
