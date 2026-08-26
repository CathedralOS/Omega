use std::{collections::BTreeSet, sync::Arc};

use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, Optimization, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
    OptimizationSelections, ScalarConstantFactIdentity,
};
use omega_optimization_unit::{
    BlockParameterIncomingBinding, BooleanConstantRewrite, IntegerConstantRewrite,
    IntegerEvaluationWitness, NodeLocation, ProvenanceRewrite, PsiOptimizationUnit,
    PsiRewriteCandidate, RedundantBlockParameterRewrite, RedundantBlockParameterWitness,
};
use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
use psi_core::{BlockId, IntegerValue, MachineId, OperationId, ValueId};

use crate::{
    AnalysisProduct, OrderedRuleRegistry, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
    RuleRegistryError, ScalarConstant, ScalarConstantAnalysis,
};

const SCCP_PASS_NAME: &[u8] = b"omega.psi-pass.sparse-conditional-constant-propagation.v1";
const COPY_PROPAGATION_PASS_NAME: &[u8] = b"omega.psi-pass.copy-propagation.v1";

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
                            output: location,
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
                            output: location,
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
                            output: location,
                            sources: node.provenance.clone(),
                            fuel: node.fuel.clone(),
                        }],
                        IntegerEvaluationWitness::Unary { operand_fact },
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
                            output: location,
                            sources: node.provenance.clone(),
                            fuel: node.fuel.clone(),
                        }],
                        IntegerEvaluationWitness::Binary {
                            left_fact,
                            right_fact,
                        },
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
                        let changes_binding =
                            node.successors.iter().any(|edge| edge.target == block.id);
                        if changes_use || changes_binding {
                            affected_blocks.insert(source.id);
                            provenance.push(ProvenanceRewrite {
                                output: NodeLocation {
                                    machine: function.machine,
                                    block: source.id,
                                    node: u32::try_from(node_index)
                                        .expect("unit node index fits u32"),
                                },
                                sources: node.provenance.clone(),
                                fuel: node.fuel.clone(),
                            });
                        }
                    }
                }
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
    if let Some(unsupported) = selections.as_slice().iter().find(|optimization| {
        !matches!(
            optimization,
            Optimization::SparseConditionalConstantPropagation | Optimization::CopyPropagation
        )
    }) {
        return Err(RuleRegistryError::UnsupportedOptimization(*unsupported));
    }
    if selections.contains(Optimization::SparseConditionalConstantPropagation)
        && selections.contains(Optimization::CopyPropagation)
    {
        return Err(RuleRegistryError::UnsupportedOptimizationCombination);
    }
    let mut rules = Vec::<Arc<dyn PsiOptimizationRule>>::new();
    if selections.contains(Optimization::SparseConditionalConstantPropagation) {
        rules.push(Arc::new(ExactIntegerAddConstantsRule));
        rules.push(Arc::new(ExactIntegerSubtractConstantsRule));
        rules.push(Arc::new(ExactIntegerMultiplyConstantsRule));
        rules.push(Arc::new(WrappingIntegerAddConstantsRule));
        rules.push(Arc::new(WrappingIntegerSubtractConstantsRule));
        rules.push(Arc::new(WrappingIntegerMultiplyConstantsRule));
        rules.push(Arc::new(SaturatingIntegerAddConstantsRule));
        rules.push(Arc::new(SaturatingIntegerSubtractConstantsRule));
        rules.push(Arc::new(SaturatingIntegerMultiplyConstantsRule));
        rules.push(Arc::new(ExactIntegerDivideConstantsRule));
        rules.push(Arc::new(ExactIntegerRemainderConstantsRule));
        rules.push(Arc::new(WrappingIntegerDivideConstantsRule));
        rules.push(Arc::new(WrappingIntegerRemainderConstantsRule));
        rules.push(Arc::new(SaturatingIntegerDivideConstantsRule));
        rules.push(Arc::new(SaturatingIntegerRemainderConstantsRule));
        rules.push(Arc::new(ExactIntegerShiftLeftConstantsRule));
        rules.push(Arc::new(ExactIntegerShiftRightConstantsRule));
        rules.push(Arc::new(WrappingIntegerShiftLeftConstantsRule));
        rules.push(Arc::new(WrappingIntegerShiftRightConstantsRule));
        rules.push(Arc::new(ExactIntegerCastConstantsRule));
        rules.push(Arc::new(IntegerWidenConstantsRule));
        rules.push(Arc::new(IntegerBitwiseNotConstantsRule));
        rules.push(Arc::new(IntegerBitwiseAndConstantsRule));
        rules.push(Arc::new(IntegerBitwiseOrConstantsRule));
        rules.push(Arc::new(IntegerBitwiseXorConstantsRule));
        rules.push(Arc::new(BooleanNotConstantsRule));
        rules.push(Arc::new(BooleanEqualConstantsRule));
        rules.push(Arc::new(IntegerEqualConstantsRule));
        rules.push(Arc::new(IntegerLessThanConstantsRule));
        rules.push(Arc::new(IntegerLessOrEqualConstantsRule));
    }
    if selections.contains(Optimization::CopyPropagation) {
        rules.push(Arc::new(RedundantBlockParameterRule));
    }
    OrderedRuleRegistry::new(rules)
}

#[cfg(test)]
pub(crate) mod tests {
    use omega_optimization_unit::{OptimizationFact, reconstruct_psi_optimization_unit_seed};
    use omega_optimization_validation::{
        OptimizationUnitValidationError, validate_boolean_evaluation_candidate,
        validate_integer_evaluation_candidate, validate_redundant_block_parameter_candidate,
    };
    use omega_terminal_abstract_operations::{
        TerminalAbstractBlockEntry, TerminalAbstractFunction, TerminalAbstractFunctionResult,
        TerminalAbstractOperation, TerminalAbstractOperationPlan, TerminalAbstractParameter,
        TerminalAbstractResult, TerminalAbstractSuccessor, TerminalValueBinding,
    };
    use psi_core::{
        BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, ObligationId,
        OperationId, ScalarType, ValueId,
    };
    use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::*;
    use crate::compute_analysis;

    fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
        constructor(raw).expect("nonzero test identity")
    }

    pub(crate) fn exact_add_unit() -> PsiOptimizationUnit {
        exact_chain_unit(false)
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
            TerminalAbstractOperation::IntegerConstant {
                psi_operation: id(306, OperationId::new),
                result: left,
                scalar_type: ScalarType::Integer(integer),
                value: IntegerValue::Unsigned(7),
            },
            TerminalAbstractOperation::IntegerConstant {
                psi_operation: id(307, OperationId::new),
                result: right,
                scalar_type: ScalarType::Integer(integer),
                value: IntegerValue::Unsigned(8),
            },
            TerminalAbstractOperation::ExactIntegerAdd {
                psi_operation: id(308, OperationId::new),
                obligation: id(309, ObligationId::new),
                result: sum,
                scalar_type: integer,
                left,
                right,
            },
        ];
        if include_multiply {
            operations.push(TerminalAbstractOperation::ExactIntegerMultiply {
                psi_operation: id(312, OperationId::new),
                obligation: id(313, ObligationId::new),
                result: product,
                scalar_type: integer,
                left: sum,
                right,
            });
        }
        operations.push(TerminalAbstractOperation::Return {
            psi_edge: id(310, EdgeId::new),
            result,
            value: result,
            scalar_type: ScalarType::Integer(integer),
            cleanup_actions: Vec::new(),
        });
        reconstruct_psi_optimization_unit_seed(
            &TerminalAbstractOperationPlan {
                terminal_psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([13; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![TerminalAbstractFunction {
                    machine,
                    attachment: None,
                    entry: block,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: result,
                        scalar_type: ScalarType::Integer(integer),
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![TerminalAbstractBlockEntry {
                        block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations,
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    pub(crate) fn propagated_block_parameter_unit() -> PsiOptimizationUnit {
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
        let binding = |argument| TerminalValueBinding {
            parameter,
            argument,
            scalar_type,
        };
        reconstruct_psi_optimization_unit_seed(
            &TerminalAbstractOperationPlan {
                terminal_psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([21; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![TerminalAbstractFunction {
                    machine,
                    attachment: None,
                    entry,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: result,
                        scalar_type,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![
                        TerminalAbstractBlockEntry {
                            block: entry,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        },
                        TerminalAbstractBlockEntry {
                            block: when_true,
                            parameters: Vec::new(),
                            operation_offset: 2,
                        },
                        TerminalAbstractBlockEntry {
                            block: when_false,
                            parameters: Vec::new(),
                            operation_offset: 4,
                        },
                        TerminalAbstractBlockEntry {
                            block: merge,
                            parameters: vec![TerminalAbstractParameter {
                                value: parameter,
                                scalar_type,
                            }],
                            operation_offset: 6,
                        },
                    ],
                    operations: vec![
                        TerminalAbstractOperation::BooleanConstant {
                            psi_operation: id(611, OperationId::new),
                            result: condition,
                            value: true,
                        },
                        TerminalAbstractOperation::Conditional {
                            condition,
                            when_true: TerminalAbstractSuccessor {
                                psi_edge: id(612, EdgeId::new),
                                target: when_true,
                                bindings: Vec::new(),
                            },
                            when_false: TerminalAbstractSuccessor {
                                psi_edge: id(613, EdgeId::new),
                                target: when_false,
                                bindings: Vec::new(),
                            },
                        },
                        TerminalAbstractOperation::IntegerConstant {
                            psi_operation: id(614, OperationId::new),
                            result: true_value,
                            scalar_type,
                            value: IntegerValue::Unsigned(7),
                        },
                        TerminalAbstractOperation::Jump {
                            psi_edge: id(615, EdgeId::new),
                            target: merge,
                            bindings: vec![binding(true_value)],
                        },
                        TerminalAbstractOperation::IntegerConstant {
                            psi_operation: id(616, OperationId::new),
                            result: false_value,
                            scalar_type,
                            value: IntegerValue::Unsigned(8),
                        },
                        TerminalAbstractOperation::Jump {
                            psi_edge: id(617, EdgeId::new),
                            target: merge,
                            bindings: vec![binding(false_value)],
                        },
                        TerminalAbstractOperation::IntegerBitwiseNot {
                            psi_operation: id(618, OperationId::new),
                            result,
                            scalar_type: integer,
                            operand: parameter,
                        },
                        TerminalAbstractOperation::Return {
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
        let binding = |argument| TerminalValueBinding {
            parameter,
            argument,
            scalar_type,
        };
        reconstruct_psi_optimization_unit_seed(
            &TerminalAbstractOperationPlan {
                terminal_psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([22; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![TerminalAbstractFunction {
                    machine,
                    attachment: None,
                    entry,
                    parameters: vec![
                        TerminalAbstractParameter {
                            value: condition,
                            scalar_type: ScalarType::Boolean,
                        },
                        TerminalAbstractParameter {
                            value: shared,
                            scalar_type,
                        },
                        TerminalAbstractParameter {
                            value: alternate,
                            scalar_type,
                        },
                    ],
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: result,
                        scalar_type,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![
                        TerminalAbstractBlockEntry {
                            block: entry,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        },
                        TerminalAbstractBlockEntry {
                            block: merge,
                            parameters: vec![TerminalAbstractParameter {
                                value: parameter,
                                scalar_type,
                            }],
                            operation_offset: 1,
                        },
                    ],
                    operations: vec![
                        TerminalAbstractOperation::Conditional {
                            condition,
                            when_true: TerminalAbstractSuccessor {
                                psi_edge: id(709, EdgeId::new),
                                target: merge,
                                bindings: vec![binding(shared)],
                            },
                            when_false: TerminalAbstractSuccessor {
                                psi_edge: id(710, EdgeId::new),
                                target: merge,
                                bindings: vec![binding(if redundant { shared } else { alternate })],
                            },
                        },
                        TerminalAbstractOperation::ExactIntegerAdd {
                            psi_operation: id(711, OperationId::new),
                            obligation: id(713, ObligationId::new),
                            result,
                            scalar_type: integer,
                            left: parameter,
                            right: alternate,
                        },
                        TerminalAbstractOperation::Return {
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
        unit.identity = omega_optimization_core::OptimizationUnitIdentity::from_canonical_bytes(
            if saturating {
                b"saturating-add-fixture"
            } else {
                b"wrapping-add-fixture"
            },
        );
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
        unit.identity =
            omega_optimization_core::OptimizationUnitIdentity::from_canonical_bytes(match kind {
                BitwiseFixtureKind::And => b"bitwise-and-fixture",
                BitwiseFixtureKind::Or => b"bitwise-or-fixture",
                BitwiseFixtureKind::Xor => b"bitwise-xor-fixture",
            });
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
        unit.identity =
            omega_optimization_core::OptimizationUnitIdentity::from_canonical_bytes(match kind {
                ShiftFixtureKind::ExactLeft => b"exact-shift-left-fixture",
                ShiftFixtureKind::ExactRight => b"exact-shift-right-fixture",
                ShiftFixtureKind::WrappingLeft => b"wrapping-shift-left-fixture",
                ShiftFixtureKind::WrappingRight => b"wrapping-shift-right-fixture",
            });
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
        unit.identity = omega_optimization_core::OptimizationUnitIdentity::from_canonical_bytes(
            if zero_divisor {
                b"zero-divisor-fixture"
            } else {
                b"exact-divide-fixture"
            },
        );
        unit
    }

    fn exact_cast_unit(value: u128) -> PsiOptimizationUnit {
        let machine = id(321, MachineId::new);
        let block = id(322, BlockId::new);
        let operand = id(323, ValueId::new);
        let result = id(324, ValueId::new);
        let source_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
        let target_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        reconstruct_psi_optimization_unit_seed(
            &TerminalAbstractOperationPlan {
                terminal_psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([14; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![TerminalAbstractFunction {
                    machine,
                    attachment: None,
                    entry: block,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: result,
                        scalar_type: ScalarType::Integer(target_type),
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![TerminalAbstractBlockEntry {
                        block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![
                        TerminalAbstractOperation::IntegerConstant {
                            psi_operation: id(325, OperationId::new),
                            result: operand,
                            scalar_type: ScalarType::Integer(source_type),
                            value: IntegerValue::Unsigned(value),
                        },
                        TerminalAbstractOperation::IntegerExactCast {
                            psi_operation: id(326, OperationId::new),
                            obligation: id(327, ObligationId::new),
                            result,
                            source_type,
                            target_type,
                            operand,
                        },
                        TerminalAbstractOperation::Return {
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
        .unwrap()
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
            TerminalAbstractOperation::IntegerWiden {
                psi_operation: id(336, OperationId::new),
                result,
                source_type,
                target_type,
                operand,
            }
        } else {
            TerminalAbstractOperation::IntegerBitwiseNot {
                psi_operation: id(336, OperationId::new),
                result,
                scalar_type: source_type,
                operand,
            }
        };
        reconstruct_psi_optimization_unit_seed(
            &TerminalAbstractOperationPlan {
                terminal_psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([15; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![TerminalAbstractFunction {
                    machine,
                    attachment: None,
                    entry: block,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: result,
                        scalar_type: ScalarType::Integer(target_type),
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![TerminalAbstractBlockEntry {
                        block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![
                        TerminalAbstractOperation::IntegerConstant {
                            psi_operation: id(335, OperationId::new),
                            result: operand,
                            scalar_type: ScalarType::Integer(source_type),
                            value: IntegerValue::Unsigned(15),
                        },
                        unary,
                        TerminalAbstractOperation::Return {
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
            TerminalAbstractOperation::BooleanEqual {
                psi_operation: id(348, OperationId::new),
                result,
                left,
                right,
            }
        } else {
            TerminalAbstractOperation::BooleanNot {
                psi_operation: id(348, OperationId::new),
                result,
                operand: left,
            }
        };
        reconstruct_psi_optimization_unit_seed(
            &TerminalAbstractOperationPlan {
                terminal_psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([16; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![TerminalAbstractFunction {
                    machine,
                    attachment: None,
                    entry: block,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: result,
                        scalar_type: ScalarType::Boolean,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![TerminalAbstractBlockEntry {
                        block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![
                        TerminalAbstractOperation::BooleanConstant {
                            psi_operation: id(346, OperationId::new),
                            result: left,
                            value: true,
                        },
                        TerminalAbstractOperation::BooleanConstant {
                            psi_operation: id(347, OperationId::new),
                            result: right,
                            value: false,
                        },
                        operation,
                        TerminalAbstractOperation::Return {
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
            ComparisonFixtureKind::Equal => TerminalAbstractOperation::IntegerEqual {
                psi_operation: id(358, OperationId::new),
                result,
                left,
                right,
            },
            ComparisonFixtureKind::LessThan => TerminalAbstractOperation::IntegerLessThan {
                psi_operation: id(358, OperationId::new),
                result,
                left,
                right,
            },
            ComparisonFixtureKind::LessOrEqual => TerminalAbstractOperation::IntegerLessOrEqual {
                psi_operation: id(358, OperationId::new),
                result,
                left,
                right,
            },
        };
        reconstruct_psi_optimization_unit_seed(
            &TerminalAbstractOperationPlan {
                terminal_psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([17; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![TerminalAbstractFunction {
                    machine,
                    attachment: None,
                    entry: block,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: result,
                        scalar_type: ScalarType::Boolean,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![TerminalAbstractBlockEntry {
                        block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![
                        TerminalAbstractOperation::IntegerConstant {
                            psi_operation: id(356, OperationId::new),
                            result: left,
                            scalar_type: ScalarType::Integer(scalar_type),
                            value: IntegerValue::Unsigned(7),
                        },
                        TerminalAbstractOperation::IntegerConstant {
                            psi_operation: id(357, OperationId::new),
                            result: right,
                            scalar_type: ScalarType::Integer(scalar_type),
                            value: IntegerValue::Unsigned(8),
                        },
                        operation,
                        TerminalAbstractOperation::Return {
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
        let products = vec![constants];
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        assert_eq!(registry.len(), 30);
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
            TerminalAbstractOperation::IntegerConstant {
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
                TerminalAbstractOperation::IntegerConstant {
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
                TerminalAbstractOperation::IntegerConstant {
                    value: IntegerValue::Unsigned(value),
                    ..
                } if value == expected
            ));
        }
    }

    #[test]
    fn propagated_block_parameter_fact_is_independently_reconstructed() {
        let unit = propagated_block_parameter_unit();
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let candidates = IntegerBitwiseNotConstantsRule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap();
        assert_eq!(candidates.len(), 1);
        let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
        assert!(matches!(
            accepted.unit().functions[0].blocks[3].nodes[0].operation,
            TerminalAbstractOperation::IntegerConstant {
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
            TerminalAbstractOperation::IntegerConstant {
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
                TerminalAbstractOperation::IntegerConstant {
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
            IntegerEvaluationWitness::Unary { .. }
        ));
        let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
        let target_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[1].operation,
            TerminalAbstractOperation::IntegerConstant {
                scalar_type: ScalarType::Integer(scalar_type),
                value: IntegerValue::Unsigned(250),
                ..
            } if scalar_type == target_type
        ));

        let IntegerEvaluationWitness::Unary { operand_fact } =
            candidates[0].scalar_evaluation_witness().unwrap()
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
            IntegerEvaluationWitness::Binary {
                left_fact: operand_fact,
                right_fact: operand_fact,
            },
            -1,
            patch,
        )
        .unwrap();
        assert_eq!(binary_witness.consumed_facts().len(), 1);
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
                TerminalAbstractOperation::IntegerConstant {
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
                TerminalAbstractOperation::BooleanConstant { value: false, .. }
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
                TerminalAbstractOperation::BooleanConstant { value, .. } if value == expected
            ));
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
        let unsupported = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
        assert!(matches!(
            built_in_psi_registry(&unsupported),
            Err(RuleRegistryError::UnsupportedOptimization(
                Optimization::ControlFlowCleanup
            ))
        ));
        let copy = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
        assert_eq!(built_in_psi_registry(&copy).unwrap().len(), 1);
        let unsupported_combination = OptimizationSelections::new([
            Optimization::SparseConditionalConstantPropagation,
            Optimization::CopyPropagation,
        ])
        .unwrap();
        assert!(matches!(
            built_in_psi_registry(&unsupported_combination),
            Err(RuleRegistryError::UnsupportedOptimizationCombination)
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
