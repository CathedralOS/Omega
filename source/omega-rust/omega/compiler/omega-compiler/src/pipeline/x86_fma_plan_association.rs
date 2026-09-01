//! Exact join between source-selected nearest-FMA demand and one admitted x86
//! deployment provider.
//!
//! This is checked custody for later native lowering. It neither selects an
//! instruction nor claims native execution evidence.

use std::collections::{BTreeMap, BTreeSet};

use omega_effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanDigest};
use omega_provider_planning::plans::{
    CompilerIntrinsicExecutionIdentity, SelectedProviderReviewProvenance,
};
use omega_target::{
    AdmittedX86ScalarFmaProvider, TargetProfile, X86FeatureRequirement, X86ScalarFmaSlot,
};
use psi_checked_trees::{CheckedNamedOperatorUseFact, CheckedTrees};
use psi_diagnostics::Diagnostic;
use psi_symbols::BuiltinFunction;

/// One exact checked association from a source-selected `ProviderPlan` to the
/// build-admitted x86 scalar FMA provider.
///
/// The complete selected plan and admitted provider remain private immutable
/// evidence. Compact report coordinates are exposed only for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedX86ScalarFmaPlanAssociation {
    selected_plan: ProviderPlan,
    selected_builtin: BuiltinFunction,
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
        selected: &omega_effects::SelectedProviderPlanFacts,
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

pub(super) fn bind_checked_x86_scalar_fma_plan_associations(
    checked: &CheckedTrees,
    selected: &omega_effects::SelectedProviderPlanFacts,
    provenance: &[SelectedProviderReviewProvenance],
    admitted_provider: Option<AdmittedX86ScalarFmaProvider>,
    selected_profile: Option<TargetProfile>,
) -> Result<Vec<CheckedX86ScalarFmaPlanAssociation>, Vec<Diagnostic>> {
    let mut diagnostics = validate_provenance_alignment(selected, provenance);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut demands = Vec::new();
    for operator_use in checked.facts.operators.named_uses() {
        match exact_selected_plan_index(operator_use, selected) {
            Ok(None) => continue,
            Ok(Some(plan_index)) => {
                let retained = &provenance[plan_index];
                let fma_rows = retained
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
                    })
                    .collect::<Vec<_>>();
                if fma_rows.is_empty() {
                    continue;
                }
                let [(row_index, builtin, slot)] = fma_rows.as_slice() else {
                    diagnostics.push(Diagnostic::error(format!(
                        "selected ProviderPlan `{}` retains more than one nearest-even scalar FMA execution row",
                        retained.plan.name,
                    )));
                    continue;
                };
                if retained.provider.row_requirements[*row_index]
                    != operator_use.selected_operator_symbol
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "selected ProviderPlan `{}` does not bind this exact named FMA use to its retained requirement symbol",
                        retained.plan.name,
                    )));
                    continue;
                }

                let Some(profile) = selected_profile else {
                    if TargetProfile::from_canonical_target_name(&retained.plan.target)
                        .ok()
                        .and_then(X86FeatureRequirement::scalar_fma)
                        .is_some()
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "selected x86 scalar FMA ProviderPlan `{}` requires an exact deployment profile and admitted AVX+FMA3 provider",
                            retained.plan.name,
                        )));
                    }
                    continue;
                };
                if X86FeatureRequirement::scalar_fma(profile).is_none()
                    || retained.plan.target != profile.target_name()
                {
                    // AArch64, portable/software, and differently targeted
                    // FMA realizations are not x86 deployment demands.
                    continue;
                }
                demands.push((plan_index, *row_index, *builtin, *slot));
            }
            Err(diagnostic) => diagnostics.push(diagnostic),
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
            .map(|(plan_index, _, _, slot)| {
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

    let mut exact_demands = BTreeSet::new();
    let mut plan_by_slot = BTreeMap::new();
    let mut associations = Vec::new();
    for (plan_index, row_index, builtin, slot) in demands {
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
        if !exact_demands.insert((plan_digest, slot)) {
            continue;
        }
        if let Some(existing_digest) = plan_by_slot.insert(slot, plan_digest) {
            if existing_digest != plan_digest {
                diagnostics.push(Diagnostic::error(format!(
                    "more than one exact selected ProviderPlan claims x86 scalar FMA slot `{}`",
                    slot.requirement_identity(),
                )));
                continue;
            }
        }
        if retained.plan != *plan {
            diagnostics.push(Diagnostic::error(format!(
                "selected x86 scalar FMA ProviderPlan `{}` was substituted after review",
                plan.name,
            )));
            continue;
        }
        associations.push(CheckedX86ScalarFmaPlanAssociation {
            selected_plan: plan.clone(),
            selected_builtin: builtin,
            slot,
            admitted_provider,
        });
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    associations.sort_by_key(CheckedX86ScalarFmaPlanAssociation::slot);
    Ok(associations)
}

fn validate_provenance_alignment(
    selected: &omega_effects::SelectedProviderPlanFacts,
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
        .filter_map(|(plan, retained)| {
            (retained.plan != *plan
                || retained.provider.row_requirements.len() != plan.rows.len()
                || retained.provider.row_realizations.len() != plan.rows.len()
                || retained.row_compiler_intrinsic_executions.len() != plan.rows.len())
            .then(|| {
                Diagnostic::error(format!(
                    "selected provider plan `{}` has incomplete or misaligned compiler-owned review provenance",
                    plan.name,
                ))
            })
        })
        .collect()
}

fn exact_selected_plan_index(
    operator_use: &CheckedNamedOperatorUseFact,
    selected: &omega_effects::SelectedProviderPlanFacts,
) -> Result<Option<usize>, Diagnostic> {
    let report_identity = operator_use.provider_plan_report_fingerprint;
    let commitment = operator_use.provider_plan_commitment;
    if report_identity == 0 && commitment.is_empty() {
        return Ok(None);
    }
    if report_identity == 0 || commitment.is_empty() {
        return Err(Diagnostic::error(
            "named operator use retains only one half of exact selected ProviderPlan evidence",
        ));
    }

    let matches = selected
        .plans()
        .iter()
        .enumerate()
        .filter(|(_, plan)| {
            plan.report_fingerprint() == report_identity
                && plan.identity_digest().as_bytes() == commitment.as_bytes()
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let [index] = matches.as_slice() else {
        return Err(Diagnostic::error(format!(
            "named operator use does not rejoin exactly one selected ProviderPlan at report identity {report_identity:#018x}",
        )));
    };
    let plan = &selected.plans()[*index];
    if selected
        .plan_by_exact_evidence(report_identity, plan)
        .is_none()
    {
        return Err(Diagnostic::error(format!(
            "named operator use rejoined a substituted ProviderPlan at report identity {report_identity:#018x}",
        )));
    }
    Ok(Some(*index))
}

const fn slot_for_builtin(builtin: BuiltinFunction) -> Option<X86ScalarFmaSlot> {
    match builtin {
        BuiltinFunction::FloatFusedMultiplyAddF32 => Some(X86ScalarFmaSlot::Binary32),
        BuiltinFunction::FloatFusedMultiplyAddF64 => Some(X86ScalarFmaSlot::Binary64),
        _ => None,
    }
}
