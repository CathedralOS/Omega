//! Optimizer module role: executable entrance. Proof-check elision, cataloged by the exact scalar identity being proved.

use abstract_operations::{AbstractOperation as O, IntegerArithmeticOperator};
use optimization_core::{AnalysisKind, OptimizationRuleContract};
use optimization_unit::{
    IntegerConstantRewrite, NodeLocation, ProvenanceDisposition, ProvenanceRewrite,
    PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
};
use semantic_vocabulary::{IntegerCarrier, IntegerSign, IntegerType, IntegerValue};

use crate::rules::accepted_obligations::accepted_obligation_fact;
use crate::rules::catalog::BuiltInRuleRegistration;
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

pub use divide_by_one::LiveProofCertifiedIntegerDivideByOneEliminationRule;
pub use multiply_by_zero::LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule;
pub use negative_one_shift_right::LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule;
pub use proof_certified_dead_scalar_elimination::ProofCertifiedDeadScalarEliminationRule;
pub use remainder_by_one::LiveProofCertifiedIntegerRemainderByOneEliminationRule;
pub use scalar_identities::LiveProofCertifiedIntegerIdentityEliminationRule;
pub use self_divide::LiveProofCertifiedIntegerSelfDivideEliminationRule;
pub use self_remainder::LiveProofCertifiedIntegerSelfRemainderEliminationRule;
pub use self_subtract::LiveProofCertifiedExactIntegerSelfSubtractEliminationRule;
pub use signed_remainder_by_negative_one::LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule;
pub use zero_dividend::LiveProofCertifiedIntegerZeroDividendEliminationRule;
pub use zero_value_shift::LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule;

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
    let Some(AnalysisProduct::ScalarConstants(constants)) =
        analyses.get(AnalysisKind::ScalarConstants)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ScalarConstants,
        ));
    };
    let (use_definitions, effects) = analyses.use_definitions_and_effects()?;
    let mut candidates: Vec<PsiRewriteCandidate> = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let (psi_operation, result, scalar_type, left, right) = match &node.operation {
                    O::IntegerArithmetic {
                        psi_operation,
                        result,
                        scalar_type,
                        left,
                        right,
                        arithmetic,
                        ..
                    } if arithmetic.operator() == IntegerArithmeticOperator::Remainder => {
                        (psi_operation, result, scalar_type, left, right)
                    }
                    _ => continue,
                };
                if left == right
                    || scalar_type.carrier() != IntegerCarrier::Fixed
                    || (signed_only && scalar_type.sign() != IntegerSign::Signed)
                    || !use_definitions.uses.iter().any(|(machine, use_site)| {
                        *machine == function.machine && use_site.value == *result
                    })
                {
                    continue;
                }
                let Some((constant, constant_fact)) =
                    constants.literal_integer_constant(function.machine, *right)
                else {
                    continue;
                };
                if !expected_constant(*scalar_type, constant) || !scalar_type.admits(constant) {
                    continue;
                }
                let node_index =
                    u32::try_from(node_index).expect("optimization node index fits u32");
                let effect = effects.node(function.machine, block.id, node_index);
                if effect.is_none_or(|row| !row.is_pure_scalar_baseline(unit.identity)) {
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
                        contract,
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
                            constant: scalar_type.zero(),
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}
