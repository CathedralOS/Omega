use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::{
    AnalysisKind, OptimizationRuleContract, OptimizationSafetyClass, ScalarConstantFactIdentity,
};
use omega_optimization_unit::{
    IntegerConstantRewrite, IntegerEvaluationWitness, NodeLocation, ProvenanceDisposition,
    ProvenanceRewrite, PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
};
use psi_core::{IntegerValue, MachineId, OperationId, ValueId};

use crate::rules::passes::support::accepted_obligation_fact;
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::super::integer_evaluation_contract;
use super::integer_constant;

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
