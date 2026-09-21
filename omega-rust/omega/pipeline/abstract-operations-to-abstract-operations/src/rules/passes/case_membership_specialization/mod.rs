//! Case-membership specialization pass entrance and rule registration.

use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::{
    CaseMembershipSpecializationRewrite, FoldedCaseMembershipRow, PsiOptimizationUnit,
    PsiRewriteCandidate,
};

use crate::representation_specialization;
use crate::rules::catalog::BuiltInRuleRegistration;
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::REPRESENTATION_SPECIALIZATION_PASS_NAME;

/// Folds a `StructuralCaseMembership` observation to a `BooleanConstant`
/// carrying the verdict the unit itself proves: an `EstablishScalarCase`
/// producer on the observed operation-result place, or a declared closed
/// `Sum`/`Mixed` roster of exactly one case at the position the
/// observation's path resolves to. Each folded node keeps its operation
/// custody identity, result value, successors, definitions, uses, ownership
/// events, and fuel settlement; only the operation and the recomputed unit
/// identity differ. Machines holding an authenticated cyclic component stay
/// frozen.
#[derive(Debug, Clone, Copy, Default)]
pub struct CaseMembershipSpecializationRule;

impl CaseMembershipSpecializationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.case-membership-specialization.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(REPRESENTATION_SPECIALIZATION_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::StronglyConnectedComponents]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::ScalarConstants,
                AnalysisKind::ValueRanges,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::StructuralIdentity,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for CaseMembershipSpecializationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        let Some(AnalysisProduct::StronglyConnectedComponents(scc)) =
            analyses.get(AnalysisKind::StronglyConnectedComponents)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::StronglyConnectedComponents,
            ));
        };
        let frozen = super::support::frozen_machines(unit, scc);
        let mut candidates = Vec::new();
        for function in &unit.functions {
            if frozen.contains(&function.machine) {
                continue;
            }
            for declaration in &function.structural_places {
                let Some(plan) =
                    representation_specialization::propose::plan(unit, function, declaration.id)
                else {
                    continue;
                };
                if plan.memberships.is_empty() {
                    continue;
                }
                let memberships = plan
                    .memberships
                    .iter()
                    .map(|row| FoldedCaseMembershipRow {
                        site: row.site(),
                        psi_operation: row.psi_operation(),
                        result: row.result(),
                        source: row.source(),
                        producer: row.producer(),
                        observed_case: row.observed_case(),
                        proven_case: row.proven_case(),
                        outcome: row.outcome(),
                    })
                    .collect::<Vec<_>>();
                let Ok(provenance) =
                    representation_specialization::validate::provenance_rows(function, &plan)
                else {
                    continue;
                };
                let mut affected_blocks = memberships
                    .iter()
                    .map(|row| row.site.block)
                    .collect::<Vec<_>>();
                affected_blocks.sort_unstable();
                affected_blocks.dedup();
                let patch = CaseMembershipSpecializationRewrite {
                    machine: plan.machine,
                    place: plan.place,
                    producer: plan.producer,
                    memberships,
                };
                let predicted_cost_delta =
                    -i64::try_from(patch.memberships.len()).unwrap_or(i64::MAX);
                candidates.push(
                    PsiRewriteCandidate::new_case_membership_specialization(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        provenance,
                        predicted_cost_delta,
                        patch,
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}

/// The exact local rule order for this pass.
pub(in crate::rules) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![BuiltInRuleRegistration::new(
        0,
        CaseMembershipSpecializationRule,
    )]
}
