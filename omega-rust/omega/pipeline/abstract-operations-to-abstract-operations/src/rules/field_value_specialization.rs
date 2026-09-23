//! Field-value specialization pass entrance and rule registration.

use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::{
    FieldValueResolution, PsiOptimizationUnit, PsiRewriteCandidate, ScalarSubstitution,
};

use crate::rules::REPRESENTATION_SPECIALIZATION_PASS_NAME;
use crate::rules::catalog::BuiltInRuleRegistration;
use crate::{
    AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
    field_value_specialization,
};

/// Resolves a `BooleanStructuralField`/`IntegerStructuralField` observation
/// whose stored value the unit itself proves: an `EstablishRecord` producer
/// on the observed operation-result place at an empty path, an
/// `EstablishScalarCase` producer whose `result_case` matches at a lone
/// `Case` path, or a declared `BoundedInteger` singleton bound at the
/// position the observation's path resolves to. A constant initializer or
/// bound folds the read to a `BooleanConstant`/`IntegerConstant` in place,
/// keeping the read's custody; a proven nonconstant initializer substitutes
/// itself at every use of the read's result and retires the observation
/// node, fusing the read's custody into the following node. Machines holding
/// an authenticated cyclic component stay frozen.
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
                let Some(patch) =
                    field_value_specialization::propose::plan(unit, function, declaration.id)
                else {
                    continue;
                };
                if patch.reads.is_empty() {
                    continue;
                }
                // Every `Forward` row carries exactly one substitution — the
                // read's result rebinds to the proven initializer — and the
                // candidate's region and custody ledger come from the same
                // accounting the independent replay recomputes.
                let mut substitutions = patch
                    .reads
                    .iter()
                    .filter_map(|row| match &row.resolution {
                        FieldValueResolution::Forward(forwarded) => Some(ScalarSubstitution {
                            from: row.result,
                            to: forwarded.initializer,
                            scalar_type: forwarded.scalar_type,
                        }),
                        FieldValueResolution::Constant(_) => None,
                    })
                    .collect::<Vec<_>>();
                substitutions.sort();
                let Some((affected_blocks, provenance)) =
                    field_value_specialization::accounting::plan_accounting(function, &patch)
                else {
                    continue;
                };
                let predicted_cost_delta = -i64::try_from(patch.reads.len()).unwrap_or(i64::MAX);
                candidates.push(
                    PsiRewriteCandidate::new_field_value_specialization(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        substitutions,
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
