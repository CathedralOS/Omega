//! Field-value specialization pass entrance and rule registration.

use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::{
    FieldValueSpecializationRewrite, FoldedFieldValueRow, PsiOptimizationUnit, PsiRewriteCandidate,
};

use crate::rules::REPRESENTATION_SPECIALIZATION_PASS_NAME;
use crate::rules::catalog::BuiltInRuleRegistration;
use crate::{
    AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
    field_value_specialization,
};

/// Folds a `BooleanStructuralField`/`IntegerStructuralField` observation to
/// a `BooleanConstant`/`IntegerConstant` carrying the stored value the unit
/// itself proves: an `EstablishRecord` producer on the observed
/// operation-result place at an empty path, an `EstablishScalarCase`
/// producer whose `result_case` matches at a lone `Case` path, or a declared
/// `BoundedInteger` singleton bound at the position the observation's path
/// resolves to. Each folded node keeps its operation custody identity,
/// result value, successors, definitions, uses, ownership events, and fuel
/// settlement; only the operation and the recomputed unit identity differ.
/// Machines holding an authenticated cyclic component stay frozen.
#[derive(Debug, Clone, Copy, Default)]
pub struct FieldValueSpecializationRule;

impl FieldValueSpecializationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.field-value-specialization.v1",
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

impl PsiOptimizationRule for FieldValueSpecializationRule {
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
                    field_value_specialization::propose::plan(unit, function, declaration.id)
                else {
                    continue;
                };
                if plan.reads.is_empty() {
                    continue;
                }
                let reads = plan
                    .reads
                    .iter()
                    .map(|row| FoldedFieldValueRow {
                        site: row.site(),
                        psi_operation: row.psi_operation(),
                        result: row.result(),
                        source: row.source(),
                        path: row.path().to_vec(),
                        field: row.field(),
                        producer: row.producer(),
                        value: row.value(),
                    })
                    .collect::<Vec<_>>();
                let Ok(provenance) =
                    field_value_specialization::validate::provenance_rows(function, &plan)
                else {
                    continue;
                };
                let mut affected_blocks =
                    reads.iter().map(|row| row.site.block).collect::<Vec<_>>();
                affected_blocks.sort_unstable();
                affected_blocks.dedup();
                let patch = FieldValueSpecializationRewrite {
                    machine: plan.machine,
                    place: plan.place,
                    producer: plan.producer,
                    reads,
                };
                let predicted_cost_delta = -i64::try_from(patch.reads.len()).unwrap_or(i64::MAX);
                candidates.push(
                    PsiRewriteCandidate::new_field_value_specialization(
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
pub(super) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![BuiltInRuleRegistration::new(
        1,
        FieldValueSpecializationRule,
    )]
}
