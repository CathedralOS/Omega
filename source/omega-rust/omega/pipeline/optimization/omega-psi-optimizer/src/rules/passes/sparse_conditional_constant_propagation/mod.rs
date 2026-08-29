use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
    ScalarConstantFactIdentity,
};
use omega_optimization_unit::{
    BooleanConstantRewrite, IntegerConstantRewrite, IntegerEvaluationWitness, NodeLocation,
    ProvenanceDisposition, ProvenanceRewrite, PsiOptimizationUnit, PsiRealizationSite,
    PsiRewriteCandidate,
};
use psi_core::{IntegerValue, MachineId, OperationId, ValueId};

use crate::{
    AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError, ScalarConstant,
    ScalarConstantAnalysis,
};

use super::{
    SCCP_PASS_NAME,
    support::{accepted_obligation_fact, boolean_constant, literal_integer_constant},
};

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

pub(in crate::rules::passes) mod range_comparisons;

pub use range_comparisons::{
    IntegerEqualConstantRangeRule, IntegerEqualRangeConstantRule, IntegerEqualRangeRangeRule,
    IntegerLessOrEqualConstantRangeRule, IntegerLessOrEqualRangeConstantRule,
    IntegerLessOrEqualRangeRangeRule, IntegerLessThanConstantRangeRule,
    IntegerLessThanRangeConstantRule, IntegerLessThanRangeRangeRule,
};
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
