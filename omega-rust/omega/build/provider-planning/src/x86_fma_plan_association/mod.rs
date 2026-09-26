//! Exact join between source-selected nearest-FMA demand and one admitted x86
//! deployment provider.
//!
//! This is checked custody for later native lowering. It neither selects an
//! instruction nor claims native execution evidence.

use std::collections::BTreeSet;

use crate::{CompilerIntrinsicExecutionIdentity, SelectedProviderReviewProvenance};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanDigest};
use symbols::{BuiltinFunction, SymbolHandle};
use target::{
    AdmittedX86ScalarFmaProvider, TargetProfile, X86FeatureRequirement, X86ScalarFmaSlot,
};

/// One exact checked association from a source-selected `ProviderPlan` to the
/// build-admitted x86 scalar FMA provider.
///
/// The complete selected plan and admitted provider remain private immutable
/// evidence. Compact report coordinates are exposed only for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedX86ScalarFmaPlanAssociation {
    selected_plan: ProviderPlan,
    selected_builtin: BuiltinFunction,
    requirement_operator: SymbolHandle,
    slot: X86ScalarFmaSlot,
    admitted_provider: AdmittedX86ScalarFmaProvider,
}

impl CheckedX86ScalarFmaPlanAssociation {
    pub const fn selected_plan(&self) -> &ProviderPlan {
        &self.selected_plan
    }

    pub fn selected_plan_digest(&self) -> ProviderPlanDigest {
        self.selected_plan.identity_digest()
    }

    pub fn selected_plan_report_identity(&self) -> u64 {
        self.selected_plan.report_fingerprint()
    }

    pub const fn selected_builtin(&self) -> BuiltinFunction {
        self.selected_builtin
    }

    pub const fn requirement_operator(&self) -> SymbolHandle {
        self.requirement_operator
    }

    pub const fn slot(&self) -> X86ScalarFmaSlot {
        self.slot
    }

    pub const fn admitted_provider(&self) -> AdmittedX86ScalarFmaProvider {
        self.admitted_provider
    }

    /// Rejoin this association to the exact selected closure and provider.
    /// Future native lowering must use this predicate or consume the complete
    /// checked compilation rather than trusting compact plan coordinates.
    pub fn matches_checked_inputs(
        &self,
        selected: &effects::SelectedProviderPlanFacts,
        admitted_provider: AdmittedX86ScalarFmaProvider,
    ) -> bool {
        let expected_requirement = X86FeatureRequirement::scalar_fma(admitted_provider.profile());
        admitted_provider == self.admitted_provider
            && admitted_provider.has_canonical_identity()
            && expected_requirement == Some(admitted_provider.requirement())
            && admitted_provider.admits(admitted_provider.requirement(), self.slot)
            && slot_for_builtin(self.selected_builtin) == Some(self.slot)
            && self.selected_plan.target == admitted_provider.profile().target_name()
            && matches!(
                self.selected_plan.rows.as_slice(),
                [row] if matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. })
            )
            && selected
                .plan_by_exact_evidence(
                    self.selected_plan.report_fingerprint(),
                    &self.selected_plan,
                )
                .is_some()
    }
}

pub fn bind_checked_x86_scalar_fma_plan_associations(
    checked: &CheckedTrees,
    selected: &effects::SelectedProviderPlanFacts,
    provenance: &[SelectedProviderReviewProvenance],
    admitted_provider: Option<AdmittedX86ScalarFmaProvider>,
    selected_profile: Option<TargetProfile>,
) -> Result<Vec<CheckedX86ScalarFmaPlanAssociation>, Vec<Diagnostic>> {
    let mut diagnostics = validate_provenance_alignment(selected, provenance);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut demands = Vec::new();
    // Both source spellings join this target's selected plan by requirement
    // identity: a named boundary-operator use or a direct top-level
    // boundary-requirement call. The lowered occurrence's
    // `requirement_operator` carries the requirement symbol either way, so
    // demand joins on that one view.
    let named_uses = checked
        .facts
        .operators
        .named_uses()
        .map(|operator_use| (operator_use.selected_operator_symbol, operator_use.origin))
        .chain(
            checked
                .facts
                .operators
                .named_requirement_uses()
                .map(|requirement_use| (requirement_use.requirement_symbol, requirement_use.origin)),
        );
    for (requirement_symbol, origin) in named_uses {
        match crate::selected_use_plan(checked, selected.plans(), requirement_symbol, origin) {
            None => continue,
            Some((plan_index, _)) => {
                let retained = &provenance[plan_index];
                let mut fma_rows = retained
                    .row_compiler_intrinsic_executions
                    .iter()
                    .enumerate()
                    .filter_map(|(row_index, execution)| {
                        let Some(CompilerIntrinsicExecutionIdentity::BuiltinFunction(builtin)) =
                            execution
                        else {
                            return None;
                        };
                        slot_for_builtin(*builtin).map(|slot| (row_index, *builtin, slot))
                    });
                let Some((row_index, builtin, slot)) = fma_rows.next() else {
                    continue;
                };
                if fma_rows.next().is_some() {
                    diagnostics.push(Diagnostic::error(format!(
                        "selected ProviderPlan `{}` retains more than one nearest-even scalar FMA execution row",
                        retained.plan.name,
                    )));
                    continue;
                }
                if retained.provider.row_requirements[row_index] != requirement_symbol {
                    diagnostics.push(Diagnostic::error(format!(
                        "selected ProviderPlan `{}` does not bind this exact named FMA use to its retained requirement symbol",
                        retained.plan.name,
                    )));
                    continue;
                }

                // A targetless check realizes nothing, so it owes no x86
                // deployment demand; realization for an exact profile does.
                let Some(profile) = selected_profile else {
                    continue;
                };
                if X86FeatureRequirement::scalar_fma(profile).is_none()
                    || retained.plan.target != profile.target_name()
                {
                    // AArch64, portable/software, and differently targeted
                    // FMA realizations are not x86 deployment demands.
                    continue;
                }
                demands.push((
                    plan_index,
                    row_index,
                    builtin,
                    requirement_symbol,
                    slot,
                ));
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    if demands.is_empty() {
        return Ok(Vec::new());
    }

    let Some(profile) = selected_profile else {
        unreachable!("an x86 FMA demand cannot survive without an exact profile")
    };
    let Some(expected_requirement) = X86FeatureRequirement::scalar_fma(profile) else {
        unreachable!("only exact x86 profiles enter the demand set")
    };
    let Some(admitted_provider) = admitted_provider else {
        return Err(demands
            .iter()
            .map(|(plan_index, _, _, _, slot)| {
                Diagnostic::error(format!(
                    "selected x86 scalar FMA ProviderPlan `{}` requires explicit AVX+FMA3 admission for slot `{}`",
                    selected.plans()[*plan_index].name,
                    slot.requirement_identity(),
                ))
            })
            .collect());
    };
    if !admitted_provider.has_canonical_identity()
        || admitted_provider.profile() != profile
        || admitted_provider.requirement() != expected_requirement
    {
        return Err(vec![Diagnostic::error(format!(
            "x86 scalar FMA admission does not match exact selected profile `{}`",
            profile.target_name(),
        ))]);
    }

    let mut binary32 = FmaSlotDemands::default();
    let mut binary64 = FmaSlotDemands::default();
    for (plan_index, row_index, builtin, requirement_operator, slot) in demands {
        let plan = &selected.plans()[plan_index];
        let retained = &provenance[plan_index];
        let row = &plan.rows[row_index];
        if !matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. }) {
            diagnostics.push(Diagnostic::error(format!(
                "selected x86 scalar FMA ProviderPlan `{}` is not a compiler intrinsic",
                plan.name,
            )));
            continue;
        }
        if !admitted_provider.admits(expected_requirement, slot) {
            diagnostics.push(Diagnostic::error(format!(
                "x86 scalar FMA provider does not admit selected slot `{}`",
                slot.requirement_identity(),
            )));
            continue;
        }

        let plan_digest = plan.identity_digest();
        let slot_demands = match slot {
            X86ScalarFmaSlot::Binary32 => &mut binary32,
            X86ScalarFmaSlot::Binary64 => &mut binary64,
        };
        if slot_demands.first_digest == Some(plan_digest)
            || slot_demands.conflicting_digests.contains(&plan_digest)
        {
            continue;
        }
        if slot_demands.first_digest.is_some() {
            slot_demands.conflicting_digests.insert(plan_digest);
            diagnostics.push(Diagnostic::error(format!(
                "more than one exact selected ProviderPlan claims x86 scalar FMA slot `{}`",
                slot.requirement_identity(),
            )));
            continue;
        }
        slot_demands.first_digest = Some(plan_digest);
        if retained.plan != *plan {
            diagnostics.push(Diagnostic::error(format!(
                "selected x86 scalar FMA ProviderPlan `{}` was substituted after review",
                plan.name,
            )));
            continue;
        }
        slot_demands.association = Some(CheckedX86ScalarFmaPlanAssociation {
            selected_plan: plan.clone(),
            selected_builtin: builtin,
            requirement_operator,
            slot,
            admitted_provider,
        });
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok([binary32.association, binary64.association]
        .into_iter()
        .flatten()
        .collect())
}

// Slot cardinality bounds successful associations, not rejected input. The
// digest set allocates only after a distinct plan conflicts with a slot.
#[derive(Default)]
struct FmaSlotDemands {
    first_digest: Option<ProviderPlanDigest>,
    conflicting_digests: BTreeSet<ProviderPlanDigest>,
    association: Option<CheckedX86ScalarFmaPlanAssociation>,
}

fn validate_provenance_alignment(
    selected: &effects::SelectedProviderPlanFacts,
    provenance: &[SelectedProviderReviewProvenance],
) -> Vec<Diagnostic> {
    if selected.plans().len() != provenance.len() {
        return vec![Diagnostic::error(
            "selected provider plans are not aligned with compiler-owned review provenance",
        )];
    }
    selected
        .plans()
        .iter()
        .zip(provenance)
        .filter(|&(plan, retained)| retained.plan != *plan
                || retained.provider.row_requirements.len() != plan.rows.len()
                || retained.provider.row_realizations.len() != plan.rows.len()
                || retained.row_compiler_intrinsic_executions.len() != plan.rows.len()).map(|(plan, _retained)| Diagnostic::error(format!(
                    "selected provider plan `{}` has incomplete or misaligned compiler-owned review provenance",
                    plan.name,
                )))
        .collect()
}

const fn slot_for_builtin(builtin: BuiltinFunction) -> Option<X86ScalarFmaSlot> {
    match builtin {
        BuiltinFunction::FloatFusedMultiplyAddF32 => Some(X86ScalarFmaSlot::Binary32),
        BuiltinFunction::FloatFusedMultiplyAddF64 => Some(X86ScalarFmaSlot::Binary64),
        _ => None,
    }
}
