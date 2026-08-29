use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use omega_optimization_unit::{
    BooleanConstantRewrite, IntegerEvaluationWitness, NodeLocation, ProvenanceDisposition,
    ProvenanceRewrite, PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
    ValueRangeSupport,
};
use psi_core::{IntegerType, IntegerValue};

use crate::{
    AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
    ScalarConstantAnalysis, ValueRangeAnalysis,
};

use super::{super::SCCP_PASS_NAME, constant_evaluation::integer_value_type};
use crate::rules::passes::support::literal_integer_constant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::rules::passes) enum IntegerRangeComparisonKind {
    RangeEqualConstant,
    ConstantEqualRange,
    RangeLessThanConstant,
    ConstantLessThanRange,
    RangeLessOrEqualConstant,
    ConstantLessOrEqualRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::rules::passes) enum IntegerRangePairComparisonKind {
    Equal,
    LessThan,
    LessOrEqual,
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

macro_rules! integer_range_pair_comparison_rule {
    ($name:ident, $identity:literal, $kind:expr) => {
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        impl $name {
            pub fn contract() -> OptimizationRuleContract {
                OptimizationRuleContract::new(
                    OptimizationRuleIdentity::from_canonical_bytes($identity),
                    OptimizationPassIdentity::from_canonical_bytes(SCCP_PASS_NAME),
                    1,
                    AnalysisSet::new([AnalysisKind::ValueRanges]),
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
                let Some(AnalysisProduct::ValueRanges(ranges)) =
                    analyses.get(AnalysisKind::ValueRanges)
                else {
                    return Err(RuleProposalError::MissingAnalysis(
                        AnalysisKind::ValueRanges,
                    ));
                };
                propose_integer_range_pair_comparison(unit, ranges, Self::contract(), $kind)
            }
        }
    };
}

integer_range_pair_comparison_rule!(
    IntegerEqualRangeRangeRule,
    b"omega.psi-rule.integer-equal-range-range.v1",
    IntegerRangePairComparisonKind::Equal
);
integer_range_pair_comparison_rule!(
    IntegerLessThanRangeRangeRule,
    b"omega.psi-rule.integer-less-than-range-range.v1",
    IntegerRangePairComparisonKind::LessThan
);
integer_range_pair_comparison_rule!(
    IntegerLessOrEqualRangeRangeRule,
    b"omega.psi-rule.integer-less-or-equal-range-range.v1",
    IntegerRangePairComparisonKind::LessOrEqual
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

pub(in crate::rules::passes) fn evaluate_integer_range_comparison(
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

fn propose_integer_range_pair_comparison(
    unit: &PsiOptimizationUnit,
    ranges: &ValueRangeAnalysis,
    contract: OptimizationRuleContract,
    kind: IntegerRangePairComparisonKind,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let (psi_operation, result, left, right) = match (kind, &node.operation) {
                    (
                        IntegerRangePairComparisonKind::Equal,
                        O::IntegerEqual {
                            psi_operation,
                            result,
                            left,
                            right,
                        },
                    )
                    | (
                        IntegerRangePairComparisonKind::LessThan,
                        O::IntegerLessThan {
                            psi_operation,
                            result,
                            left,
                            right,
                        },
                    )
                    | (
                        IntegerRangePairComparisonKind::LessOrEqual,
                        O::IntegerLessOrEqual {
                            psi_operation,
                            result,
                            left,
                            right,
                        },
                    ) => (*psi_operation, *result, *left, *right),
                    _ => continue,
                };
                let Some(scalar_type) = integer_value_type(function, left) else {
                    continue;
                };
                if integer_value_type(function, right) != Some(scalar_type) {
                    continue;
                }
                let node_index =
                    u32::try_from(node_index).expect("optimization node indices are u32");
                let proof_range = |value| {
                    ranges.facts.iter().find(|fact| {
                        fact.valid_in.machine == function.machine
                            && fact.value == value
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
                    })
                };
                let (Some(left_range), Some(right_range)) = (proof_range(left), proof_range(right))
                else {
                    continue;
                };
                let Some(constant) = evaluate_integer_range_pair_comparison(
                    kind,
                    scalar_type,
                    left == right,
                    left_range.minimum,
                    left_range.maximum,
                    right_range.minimum,
                    right_range.maximum,
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
                        IntegerEvaluationWitness::RangeAgainstRange {
                            left_range_fact: left_range.identity,
                            right_range_fact: right_range.identity,
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

pub(in crate::rules::passes) fn evaluate_integer_range_pair_comparison(
    kind: IntegerRangePairComparisonKind,
    scalar_type: IntegerType,
    same_value: bool,
    left_minimum: IntegerValue,
    left_maximum: IntegerValue,
    right_minimum: IntegerValue,
    right_maximum: IntegerValue,
) -> Option<bool> {
    if same_value {
        return Some(!matches!(kind, IntegerRangePairComparisonKind::LessThan));
    }
    let left_maximum_to_right_minimum = scalar_type.compare(left_maximum, right_minimum)?;
    let left_minimum_to_right_maximum = scalar_type.compare(left_minimum, right_maximum)?;
    match kind {
        IntegerRangePairComparisonKind::Equal => {
            let both_equal_singletons = scalar_type.compare(left_minimum, left_maximum)?.is_eq()
                && scalar_type.compare(right_minimum, right_maximum)?.is_eq()
                && scalar_type.compare(left_minimum, right_minimum)?.is_eq();
            both_equal_singletons.then_some(true).or_else(|| {
                (left_maximum_to_right_minimum.is_lt() || left_minimum_to_right_maximum.is_gt())
                    .then_some(false)
            })
        }
        IntegerRangePairComparisonKind::LessThan => left_maximum_to_right_minimum
            .is_lt()
            .then_some(true)
            .or_else(|| (!left_minimum_to_right_maximum.is_lt()).then_some(false)),
        IntegerRangePairComparisonKind::LessOrEqual => (!left_maximum_to_right_minimum.is_gt())
            .then_some(true)
            .or_else(|| left_minimum_to_right_maximum.is_gt().then_some(false)),
    }
}
