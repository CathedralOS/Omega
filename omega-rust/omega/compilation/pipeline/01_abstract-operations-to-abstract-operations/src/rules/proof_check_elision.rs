//! Optimizer module role: executable entrance. Proof-check elision, cataloged by the exact scalar identity being proved.

use abstract_operations::IntegerArithmeticOperator;
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationRuleContract,
    OptimizationSafetyClass,
};
use optimization_unit::{
    IntegerConstantRewrite, NodeLocation, PsiOptimizationUnit, PsiRewriteCandidate,
};
use semantic_vocabulary::{IntegerCarrier, IntegerSign, IntegerType, IntegerValue};

use crate::rules::PROOF_CHECK_ELISION_PASS_NAME;
use crate::rules::accepted_obligations::accepted_obligation_fact;
use crate::rules::catalog::BuiltInRuleRegistration;
use crate::rules::control_flow_cleanup::node_realized_provenance;
use crate::{AnalysisProduct, RuleAnalysisView, RuleProposalError};

mod divide_by_one;
mod identity_rewrite;
mod multiply_by_zero;
mod negative_one_shift_right;
mod proof_certified_dead_scalar_elimination;
mod remainder_by_one;
mod scalar_identities;
mod self_divide;
mod self_remainder;
mod self_subtract;
mod signed_remainder_by_negative_one;
mod zero_dividend;
mod zero_value_shift;

/// The contract every proof-check-elision rule states by hand — pass,
/// version, invalidation set and safety class are fixed; only the rule
/// identity and its required analyses differ.
fn proof_check_elision_rule_contract(
    identity: &'static [u8],
    analyses: AnalysisSet,
) -> OptimizationRuleContract {
    crate::rules::standard_rule_contract(
        identity,
        PROOF_CHECK_ELISION_PASS_NAME,
        1,
        analyses,
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
        OptimizationSafetyClass::ProofCertified,
    )
}

pub(crate) use divide_by_one::LiveProofCertifiedIntegerDivideByOneEliminationRule;
pub(crate) use multiply_by_zero::LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule;
pub(crate) use negative_one_shift_right::LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule;
pub(crate) use proof_certified_dead_scalar_elimination::ProofCertifiedDeadScalarEliminationRule;
pub(crate) use remainder_by_one::LiveProofCertifiedIntegerRemainderByOneEliminationRule;
pub(crate) use scalar_identities::LiveProofCertifiedIntegerIdentityEliminationRule;
pub(crate) use self_divide::LiveProofCertifiedIntegerSelfDivideEliminationRule;
pub(crate) use self_remainder::LiveProofCertifiedIntegerSelfRemainderEliminationRule;
pub(crate) use self_subtract::LiveProofCertifiedExactIntegerSelfSubtractEliminationRule;
pub(crate) use signed_remainder_by_negative_one::LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule;
pub(crate) use zero_dividend::LiveProofCertifiedIntegerZeroDividendEliminationRule;
pub(crate) use zero_value_shift::LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule;

/// The exact local rule order for this pass.
pub(super) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![
        BuiltInRuleRegistration::new(0, ProofCertifiedDeadScalarEliminationRule),
        BuiltInRuleRegistration::new(1, LiveProofCertifiedIntegerIdentityEliminationRule),
        BuiltInRuleRegistration::new(2, LiveProofCertifiedIntegerDivideByOneEliminationRule),
        BuiltInRuleRegistration::new(
            3,
            LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule,
        ),
        BuiltInRuleRegistration::new(4, LiveProofCertifiedIntegerZeroDividendEliminationRule),
        BuiltInRuleRegistration::new(
            5,
            LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule,
        ),
        BuiltInRuleRegistration::new(6, LiveProofCertifiedExactIntegerSelfSubtractEliminationRule),
        BuiltInRuleRegistration::new(7, LiveProofCertifiedIntegerSelfRemainderEliminationRule),
        BuiltInRuleRegistration::new(8, LiveProofCertifiedIntegerSelfDivideEliminationRule),
        BuiltInRuleRegistration::new(9, LiveProofCertifiedIntegerRemainderByOneEliminationRule),
        BuiltInRuleRegistration::new(
            10,
            LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule,
        ),
        BuiltInRuleRegistration::new(
            11,
            LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule,
        ),
    ]
}

/// The shared proposal the remainder-by-constant rules run: find each
/// fixed-carrier integer `Remainder` whose right operand is the rule's
/// constant on a live, provably-pure node, then offer the literal
/// proof-certified replacement rewriting it to zero. `signed_only` adds
/// the signed-carrier gate; `expected_constant` is the rule's constant
/// match over the scalar type and the literal it found.
fn propose_proof_certified_remainder_by_constant(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    signed_only: bool,
    expected_constant: impl Fn(IntegerType, IntegerValue) -> bool,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    required_analysis!(analyses, AnalysisKind::ScalarConstants => Some(AnalysisProduct::ScalarConstants(constants)));
    let (use_definitions, effects) = analyses.use_definitions_and_effects()?;
    let mut candidates: Vec<PsiRewriteCandidate> = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let Some(operands) = node
                    .operation
                    .integer_arithmetic_operands(IntegerArithmeticOperator::Remainder)
                else {
                    continue;
                };
                if operands.left == operands.right
                    || operands.scalar_type.carrier() != IntegerCarrier::Fixed
                    || (signed_only && operands.scalar_type.sign() != IntegerSign::Signed)
                    || !use_definitions.uses.iter().any(|(machine, use_site)| {
                        *machine == function.machine && use_site.value == operands.result
                    })
                {
                    continue;
                }
                let Some((constant, constant_fact)) =
                    constants.literal_integer_constant(function.machine, operands.right)
                else {
                    continue;
                };
                if !expected_constant(operands.scalar_type, constant)
                    || !operands.scalar_type.admits(constant)
                {
                    continue;
                }
                let node_index =
                    u32::try_from(node_index).expect("optimization node index fits u32");
                let effect = effects.node(function.machine, block.id, node_index);
                if effect.is_none_or(|row| !row.is_pure_scalar_baseline(unit.identity)) {
                    continue;
                }
                let Ok(obligation_fact) =
                    accepted_obligation_fact(unit, function.machine, operands.psi_operation)
                else {
                    continue;
                };
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                };
                candidates.push(
                    PsiRewriteCandidate::new_literal_proof_certified_integer_constant_replacement(
                        unit.identity,
                        contract,
                        vec![block.id],
                        vec![node_realized_provenance(location, node)],
                        constant_fact,
                        obligation_fact,
                        -1,
                        IntegerConstantRewrite {
                            location,
                            source_operation: operands.psi_operation,
                            result: operands.result,
                            scalar_type: operands.scalar_type,
                            constant: operands.scalar_type.zero(),
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}
