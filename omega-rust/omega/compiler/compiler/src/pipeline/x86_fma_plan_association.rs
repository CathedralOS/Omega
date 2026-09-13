//! Exact join between source-selected nearest-FMA demand and one admitted x86
//! deployment provider.
//!
//! This is checked custody for later native lowering. It neither selects an
//! instruction nor claims native execution evidence.

use std::collections::BTreeSet;

use checked_trees::{CheckedNamedOperatorUseFact, CheckedTrees};
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanDigest};
use provider_planning::plans::{
    CompilerIntrinsicExecutionIdentity, SelectedProviderReviewProvenance,
};
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

    pub fn matches_lowered_occurrence(
        &self,
        occurrence: &lowered_psi::LoweredSelectedIeeeFloatFmaOccurrence,
        selected: &effects::SelectedProviderPlanFacts,
        admitted_provider: AdmittedX86ScalarFmaProvider,
    ) -> bool {
        self.matches_checked_inputs(selected, admitted_provider)
            && occurrence.requirement_operator == self.requirement_operator
            && occurrence.provider_plan_report_fingerprint
                == self.selected_plan.report_fingerprint()
            && occurrence.provider_plan_commitment.as_bytes()
                == self.selected_plan.identity_digest().as_bytes()
            && occurrence.format
                == match self.slot {
                    X86ScalarFmaSlot::Binary32 => semantic_vocabulary::IeeeFloatFormat::Binary32,
                    X86ScalarFmaSlot::Binary64 => semantic_vocabulary::IeeeFloatFormat::Binary64,
                }
    }
}

pub(super) fn bind_checked_x86_scalar_fma_plan_associations(
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

    let mut plan_evidence = Vec::new();
    let mut demands = Vec::new();
    for operator_use in checked.facts.operators.named_uses() {
        match exact_selected_plan_index(operator_use, selected, &mut plan_evidence) {
            Ok(None) => continue,
            Ok(Some(plan_index)) => {
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
                if retained.provider.row_requirements[row_index]
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
                demands.push((
                    plan_index,
                    row_index,
                    builtin,
                    operator_use.selected_operator_symbol,
                    slot,
                ));
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

        let plan_digest = *plan_evidence[plan_index]
            .digest
            .get_or_insert_with(|| plan.identity_digest());
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

// Invocation-local facts about immutable selected plans. Strong identities and
// exact selected-closure rejoins are prepared only for demanded report matches.
struct SelectedPlanEvidence {
    report_identity: u64,
    digest: Option<ProviderPlanDigest>,
    exact_rejoin: Option<bool>,
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

fn exact_selected_plan_index(
    operator_use: &CheckedNamedOperatorUseFact,
    selected: &effects::SelectedProviderPlanFacts,
    evidence: &mut Vec<SelectedPlanEvidence>,
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

    if evidence.is_empty() {
        evidence.extend(selected.plans().iter().map(|plan| SelectedPlanEvidence {
            report_identity: plan.report_fingerprint(),
            digest: None,
            exact_rejoin: None,
        }));
    }

    let mut matches = selected
        .plans()
        .iter()
        .zip(evidence.iter_mut())
        .enumerate()
        .filter_map(|(plan_index, (plan, evidence))| {
            if evidence.report_identity != report_identity {
                return None;
            }
            let digest = *evidence
                .digest
                .get_or_insert_with(|| plan.identity_digest());
            (digest.as_bytes() == commitment.as_bytes()).then_some(plan_index)
        });
    let first = matches.next();
    let Some(plan_index) = first.filter(|_| matches.next().is_none()) else {
        return Err(Diagnostic::error(format!(
            "named operator use does not rejoin exactly one selected ProviderPlan at report identity {report_identity:#018x}",
        )));
    };
    let plan = &selected.plans()[plan_index];
    if !*evidence[plan_index].exact_rejoin.get_or_insert_with(|| {
        selected
            .plan_by_exact_evidence(report_identity, plan)
            .is_some()
    }) {
        return Err(Diagnostic::error(format!(
            "named operator use rejoined a substituted ProviderPlan at report identity {report_identity:#018x}",
        )));
    }
    Ok(Some(plan_index))
}

const fn slot_for_builtin(builtin: BuiltinFunction) -> Option<X86ScalarFmaSlot> {
    match builtin {
        BuiltinFunction::FloatFusedMultiplyAddF32 => Some(X86ScalarFmaSlot::Binary32),
        BuiltinFunction::FloatFusedMultiplyAddF64 => Some(X86ScalarFmaSlot::Binary64),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use checked_trees::CheckedProviderPlanCommitment;
    use effects::SelectedProviderPlanFacts;
    use effects::provider_plan::{ProviderPlanRow, ServiceMethod, ServiceSchema};
    use provider_planning::plans::{
        ProviderPlanProvenance, ProviderSchemaDeclaration, ProviderSelectionProvenance,
    };

    fn selected_plans(
        definitions: &[(&str, BuiltinFunction)],
    ) -> (
        SelectedProviderPlanFacts,
        Vec<SelectedProviderReviewProvenance>,
    ) {
        let selected = SelectedProviderPlanFacts::from_selected_plans(
            definitions
                .iter()
                .map(|(name, _)| ProviderPlan {
                    name: (*name).to_owned(),
                    target: "linux_x86_64".to_owned(),
                    schema: ServiceSchema {
                        trait_name: (*name).to_owned(),
                        methods: vec![ServiceMethod {
                            name: "fma".to_owned(),
                            requirement_owner: (*name).to_owned(),
                            requirement_identity: format!("{name}::fma"),
                            ..ServiceMethod::default()
                        }],
                        ..ServiceSchema::default()
                    },
                    rows: vec![ProviderPlanRow {
                        method: "fma".to_owned(),
                        requirement_identity: format!("{name}::fma"),
                        requirement_lifetime_partition: Vec::new(),
                        binding: ProviderBinding::CompilerIntrinsic {
                            machine: format!("{name}::realization"),
                        },
                    }],
                    ..ProviderPlan::default()
                })
                .collect(),
        )
        .expect("distinct fully covering selected plan fixtures");
        let provenance = selected
            .plans()
            .iter()
            .map(|plan| {
                let builtin = definitions
                    .iter()
                    .find(|(name, _)| *name == plan.name)
                    .expect("fixture builtin")
                    .1;
                SelectedProviderReviewProvenance {
                    plan: plan.clone(),
                    provider: ProviderPlanProvenance {
                        schema: ProviderSchemaDeclaration::BoundaryTrait(SymbolHandle::invalid()),
                        provider_type: None,
                        row_requirements: vec![SymbolHandle::invalid()],
                        row_realizations: vec![SymbolHandle::invalid()],
                        row_target_machine_origins: Vec::new(),
                    },
                    selected_by: ProviderSelectionProvenance::UniqueCoveringCandidate,
                    row_compiler_intrinsic_executions: vec![Some(
                        CompilerIntrinsicExecutionIdentity::BuiltinFunction(builtin),
                    )],
                }
            })
            .collect();
        (selected, provenance)
    }

    fn operator_use(plan: &ProviderPlan) -> CheckedNamedOperatorUseFact {
        CheckedNamedOperatorUseFact {
            provider_plan_report_fingerprint: plan.report_fingerprint(),
            provider_plan_commitment: CheckedProviderPlanCommitment::from_digest(
                *plan.identity_digest().as_bytes(),
            ),
            ..CheckedNamedOperatorUseFact::default()
        }
    }

    fn demands(selected: &SelectedProviderPlanFacts, names: &[&str]) -> CheckedTrees {
        let mut checked = CheckedTrees::default();
        for name in names {
            let plan = selected
                .plans()
                .iter()
                .find(|plan| plan.name == *name)
                .expect("selected demand fixture");
            checked
                .facts
                .operators
                .named_uses
                .append(operator_use(plan));
        }
        checked
    }

    fn admitted(profile: TargetProfile) -> AdmittedX86ScalarFmaProvider {
        AdmittedX86ScalarFmaProvider::from_deployment_claim(
            profile,
            &target::X86_SCALAR_FMA_REQUIRED_FEATURES,
        )
        .expect("canonical deployment fixture")
    }

    #[test]
    fn repeated_demands_publish_both_slots_in_slot_order() {
        let (selected, provenance) = selected_plans(&[
            ("a32", BuiltinFunction::FloatFusedMultiplyAddF32),
            ("b64", BuiltinFunction::FloatFusedMultiplyAddF64),
        ]);
        let checked = demands(&selected, &["b64", "a32", "b64", "a32"]);
        let provider = admitted(TargetProfile::LinuxX64);
        let associations = bind_checked_x86_scalar_fma_plan_associations(
            &checked,
            &selected,
            &provenance,
            Some(provider),
            Some(TargetProfile::LinuxX64),
        )
        .expect("repeated demand deduplicates without changing slot order");
        assert_eq!(
            associations
                .iter()
                .map(|association| association.slot())
                .collect::<Vec<_>>(),
            X86ScalarFmaSlot::ALL
        );
        assert!(
            associations
                .iter()
                .all(|association| association.matches_checked_inputs(&selected, provider))
        );
    }

    #[test]
    fn conflicting_plans_are_reported_once_each_in_demand_order() {
        let (selected, provenance) = selected_plans(&[
            ("a32", BuiltinFunction::FloatFusedMultiplyAddF32),
            ("b32", BuiltinFunction::FloatFusedMultiplyAddF32),
            ("c32", BuiltinFunction::FloatFusedMultiplyAddF32),
            ("d64", BuiltinFunction::FloatFusedMultiplyAddF64),
            ("e64", BuiltinFunction::FloatFusedMultiplyAddF64),
        ]);
        let checked = demands(
            &selected,
            &["a32", "b32", "a32", "d64", "e64", "b32", "c32", "e64"],
        );
        let diagnostics = bind_checked_x86_scalar_fma_plan_associations(
            &checked,
            &selected,
            &provenance,
            Some(admitted(TargetProfile::LinuxX64)),
            Some(TargetProfile::LinuxX64),
        )
        .expect_err("two slots do not bound the number of distinct conflicting plans");
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>(),
            [
                "more than one exact selected ProviderPlan claims x86 scalar FMA slot `F32::fused_multiply_add`",
                "more than one exact selected ProviderPlan claims x86 scalar FMA slot `F64::fused_multiply_add`",
                "more than one exact selected ProviderPlan claims x86 scalar FMA slot `F32::fused_multiply_add`",
            ]
        );
    }

    #[test]
    fn plan_digest_preparation_is_demanded_and_reused_without_accepting_stale_evidence() {
        let (selected, _) = selected_plans(&[
            ("a32", BuiltinFunction::FloatFusedMultiplyAddF32),
            ("b64", BuiltinFunction::FloatFusedMultiplyAddF64),
        ]);
        let mut evidence = Vec::new();
        let mut occurrence = operator_use(&selected.plans()[1]);
        for _ in 0..64 {
            assert_eq!(
                exact_selected_plan_index(&occurrence, &selected, &mut evidence)
                    .expect("exact repeated occurrence"),
                Some(1)
            );
        }
        assert!(evidence[0].digest.is_none());
        assert!(evidence[0].exact_rejoin.is_none());
        assert_eq!(
            evidence[1].digest,
            Some(selected.plans()[1].identity_digest())
        );
        assert_eq!(evidence[1].exact_rejoin, Some(true));
        occurrence.provider_plan_commitment = CheckedProviderPlanCommitment::from_digest([7; 32]);
        assert!(
            exact_selected_plan_index(&occurrence, &selected, &mut evidence)
                .expect_err("a cached plan does not authorize changed occurrence evidence")
                .message
                .contains("exactly one selected ProviderPlan")
        );
    }

    #[test]
    fn invalid_provenance_rejects_even_without_demand() {
        let (selected, provenance) =
            selected_plans(&[("a32", BuiltinFunction::FloatFusedMultiplyAddF32)]);
        for checked in [CheckedTrees::default(), demands(&selected, &["a32", "a32"])] {
            assert!(
                bind_checked_x86_scalar_fma_plan_associations(&checked, &selected, &[], None, None)
                    .expect_err("undemanded provenance still aligns")[0]
                    .message
                    .contains("not aligned")
            );
            for substitute in [false, true] {
                let mut malformed = provenance.clone();
                if substitute {
                    malformed[0].plan.name.push_str(".substituted");
                } else {
                    malformed[0].provider.row_requirements.clear();
                }
                assert!(
                    bind_checked_x86_scalar_fma_plan_associations(
                        &checked, &selected, &malformed, None, None
                    )
                    .expect_err("complete provenance precedes demand processing")[0]
                        .message
                        .contains("incomplete or misaligned")
                );
            }
        }
    }

    #[test]
    fn missing_and_wrong_deployment_keep_per_occurrence_diagnostics() {
        let (selected, provenance) = selected_plans(&[
            ("a32", BuiltinFunction::FloatFusedMultiplyAddF32),
            ("b64", BuiltinFunction::FloatFusedMultiplyAddF64),
        ]);
        let checked = demands(&selected, &["b64", "a32", "b64"]);
        let diagnostics = bind_checked_x86_scalar_fma_plan_associations(
            &checked,
            &selected,
            &provenance,
            None,
            Some(TargetProfile::LinuxX64),
        )
        .expect_err("missing admission diagnostics retain repeated source demand");
        assert_eq!(diagnostics.len(), 3);
        assert!(diagnostics[0].message.contains("`b64`"));
        assert!(diagnostics[1].message.contains("`a32`"));
        assert_eq!(diagnostics[0].message, diagnostics[2].message);
        let diagnostics = bind_checked_x86_scalar_fma_plan_associations(
            &checked,
            &selected,
            &provenance,
            Some(admitted(TargetProfile::WindowsX64)),
            Some(TargetProfile::LinuxX64),
        )
        .expect_err("matching architecture does not authorize another deployment");
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("does not match exact selected profile")
        );
    }
}
