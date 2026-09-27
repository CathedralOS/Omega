//! Selected provider evidence: the closure of provider plans a selection actually
//! reached — which provider realizes each boundary requirement — and its
//! digest. The retained evidence beside it: `providers` records provider
//! plans, approvals and boundary-call audits; `authority` the terminal
//! authority dispositions and service permissions; `executable_scopes` the
//! executable bytes and services admitted into a scope; `component_eras` the
//! era ledger and progress manifests.

pub(crate) mod authority;
pub(crate) mod component_eras;
pub(crate) mod executable_scopes;
pub(crate) mod providers;

use crate::provider_plan::ProviderPlan;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// Collision-resistant identity of one complete selected-provider closure.
///
/// This commits the exact selected plans and every attached closure fact. The
/// existing compact normalized identity remains a compatibility/report
/// coordinate and must not authorize artifact replay by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SelectedProviderClosureDigest([u8; 32]);

impl SelectedProviderClosureDigest {
    const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// The exact provider plans selected by the compiler for one checked program.
///
/// Candidates remain ordinary policy values. This carrier retains only the
/// fully covering candidates selected for the concrete target, in canonical
/// name order, so later provider execution and generated-machine lowering do
/// not have to rediscover selection from source declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderPlanFacts {
    plans: Vec<ProviderPlan>,
    /// Compact report coordinate. Authority retains `plans` and is exposed by
    /// `identity_digest()`.
    report_fingerprint: u64,
    execution_scope: crate::ExecutionScope,
    opaque_executable_admissions: Vec<crate::ValidatedOpaqueExecutableAdmission>,
    installation_reach_resolutions: Vec<InstallationReachResolution>,
    /// Template-keyed selected rows rebound to their exact closed requirement
    /// instances once specialization evidence exists.
    specialized_requirement_rows: Vec<SelectedRequirementSpecialization>,
    /// Compact identities of the members settled by the toolchain itself
    /// (canonical-host mints). Report surfaces use this to label their
    /// provenance honestly instead of reporting them as ungranted
    /// dev-active packages.
    toolchain_settled_identities: BTreeSet<u64>,
}

impl Default for SelectedProviderPlanFacts {
    fn default() -> Self {
        Self {
            plans: Vec::new(),
            report_fingerprint: selected_plans_report_fingerprint(&[]),
            execution_scope: crate::ExecutionScope::CallerAddressSpace,
            opaque_executable_admissions: Vec::new(),
            installation_reach_resolutions: Vec::new(),
            specialized_requirement_rows: Vec::new(),
            toolchain_settled_identities: BTreeSet::new(),
        }
    }
}

impl SelectedProviderPlanFacts {
    /// Retain already-resolved provider plans without rejoining them through a
    /// readable plan name. Authored selection paths belong before this
    /// boundary; every plan here already carries its package-qualified slot,
    /// provider, requirement, and realization provenance.
    pub fn from_selected_plans(mut plans: Vec<ProviderPlan>) -> Result<Self, String> {
        plans.sort_by(selected_plan_order);

        let mut identities = BTreeSet::new();
        let mut boundary_slots = BTreeSet::new();
        let mut previous: Option<&ProviderPlan> = None;
        for plan in &plans {
            if previous.is_some_and(|previous| previous == plan) {
                return Err(format!(
                    "selected provider plan `{}` appears more than once",
                    plan.name
                ));
            }
            previous = Some(plan);
            let errors = plan.validate_against_schema();
            if !errors.is_empty() {
                return Err(format!(
                    "selected provider plan `{}` is not fully covering: {}",
                    plan.name,
                    errors.join("; ")
                ));
            }
            let identity = plan.report_fingerprint();
            if identity == 0 {
                return Err(format!(
                    "selected provider plan `{}` produced the reserved zero identity",
                    plan.name
                ));
            }
            if !identities.insert(identity) {
                return Err(format!(
                    "selected provider plan `{}` collides with another selected plan at identity {identity:#018x}",
                    plan.name
                ));
            }
            let slot = (
                plan.schema.trait_package_identity,
                plan.schema.trait_name.as_str(),
            );
            if !boundary_slots.insert(slot) {
                return Err(format!(
                    "boundary slot `{}` has more than one selected provider plan",
                    plan.schema.trait_name
                ));
            }
        }

        let report_fingerprint = selected_plans_report_fingerprint(&plans);
        Ok(Self {
            plans,
            report_fingerprint,
            execution_scope: crate::ExecutionScope::CallerAddressSpace,
            opaque_executable_admissions: Vec::new(),
            installation_reach_resolutions: Vec::new(),
            specialized_requirement_rows: Vec::new(),
            toolchain_settled_identities: BTreeSet::new(),
        })
    }

    /// Join one toolchain-settled canonical-host plan into this selected
    /// closure.
    ///
    /// Provider settlement mints these plans for boundary services whose
    /// canonical host is toolchain-owned (the accepted `FilesystemHost`
    /// binding): no package may author `satisfies` conformances or
    /// `select_provider` rows for that slot, so the toolchain retains the
    /// per-target realization rows itself. The minted plan carries the exact
    /// checked service schema and selected target, but it deliberately covers
    /// only the leaves the toolchain can bind honestly on that target, so the
    /// full-coverage check inside [`Self::from_selected_plans`] does not
    /// apply. Demand-completeness still belongs to closure review: a demanded
    /// leaf with no minted row resolves to zero selected rows and rejects
    /// there, never here. Uniqueness, nonzero identity, and boundary-slot
    /// invariants are unchanged.
    pub fn with_toolchain_settled_plan(mut self, plan: ProviderPlan) -> Result<Self, String> {
        if !self.installation_reach_resolutions.is_empty()
            || !self.specialized_requirement_rows.is_empty()
        {
            return Err(
                "selected provider closure must join toolchain-settled plans before attaching installation reach or specialization evidence"
                    .into(),
            );
        }
        let errors = plan.validate_candidate_against_schema();
        if !errors.is_empty() {
            return Err(format!(
                "toolchain-settled provider plan `{}` is malformed: {}",
                plan.name,
                errors.join("; ")
            ));
        }
        let identity = plan.report_fingerprint();
        if identity == 0 {
            return Err(format!(
                "toolchain-settled provider plan `{}` produced the reserved zero identity",
                plan.name
            ));
        }
        if self
            .plans
            .iter()
            .any(|existing| existing.report_fingerprint() == identity)
        {
            return Err(format!(
                "toolchain-settled provider plan `{}` collides with a selected plan at identity {identity:#018x}",
                plan.name
            ));
        }
        if self.plans.iter().any(|existing| {
            existing.schema.trait_package_identity == plan.schema.trait_package_identity
                && existing.schema.trait_name == plan.schema.trait_name
        }) {
            return Err(format!(
                "boundary slot `{}` has more than one selected provider plan",
                plan.schema.trait_name
            ));
        }
        self.plans.push(plan);
        self.plans.sort_by(selected_plan_order);
        self.toolchain_settled_identities.insert(identity);
        self.report_fingerprint = selected_plans_report_fingerprint(&self.plans);
        Ok(self)
    }

    /// Whether this exact selected member was settled by the toolchain
    /// rather than authored conformance selection. Exact structural
    /// membership is required so a fingerprint-shaped substitute cannot
    /// claim toolchain provenance.
    pub fn is_toolchain_settled(&self, plan: &ProviderPlan) -> bool {
        self.toolchain_settled_identities
            .contains(&plan.report_fingerprint())
            && self.plans.iter().any(|member| member == plan)
    }

    /// Compatibility constructor for focused tests and legacy callers. New
    /// compiler paths resolve authored names once and call
    /// [`Self::from_selected_plans`] directly.
    pub fn from_selection(
        candidates: &[ProviderPlan],
        selected_names: &[String],
    ) -> Result<Self, String> {
        let mut names = BTreeSet::new();
        for name in selected_names {
            if !names.insert(name.as_str()) {
                return Err(format!(
                    "selected provider plan `{name}` appears more than once"
                ));
            }
        }

        let mut plans = Vec::with_capacity(names.len());
        for name in names {
            let matches = candidates
                .iter()
                .filter(|candidate| candidate.name == name)
                .collect::<Vec<_>>();
            let [plan] = matches.as_slice() else {
                return Err(match matches.len() {
                    0 => format!(
                        "selected provider plan `{name}` is absent from the validated candidate set"
                    ),
                    count => format!(
                        "selected provider plan `{name}` matches {count} candidates; selection must identify exactly one plan"
                    ),
                });
            };
            plans.push((*plan).clone());
        }
        Self::from_selected_plans(plans)
    }

    pub fn plans(&self) -> &[ProviderPlan] {
        &self.plans
    }

    /// Compatibility/report lookup for existing compact-ID consumers. Code
    /// making an admission or execution decision must use
    /// [`Self::plan_by_exact_evidence`] instead.
    pub fn plan_by_report_fingerprint(&self, report_fingerprint: u64) -> Option<&ProviderPlan> {
        self.plans
            .iter()
            .find(|plan| plan.report_fingerprint() == report_fingerprint)
    }

    /// Rejoin a compact provider-plan report identity only when the caller
    /// also retains the complete selected plan evidence. The fingerprint is a
    /// compatibility coordinate; exact structural equality is the authority
    /// check, so a collision-equal substitute cannot select another plan.
    pub fn plan_by_exact_evidence(
        &self,
        report_identity: u64,
        exact_plan: &ProviderPlan,
    ) -> Option<&ProviderPlan> {
        // Every constructor rejects a repeated plan and a fingerprint
        // collision, so the one member structurally equal to `exact_plan` is
        // the only member that can carry its fingerprint: comparing plans
        // (which fails fast on the name) and rendering one fingerprint
        // replaces rendering every member's.
        let selected = self.plans.iter().find(|plan| *plan == exact_plan)?;
        (selected.report_fingerprint() == report_identity).then_some(selected)
    }

    pub const fn report_fingerprint(&self) -> u64 {
        self.report_fingerprint
    }

    /// Non-authoritative compatibility/report identity retained for existing
    /// artifact formats and diagnostics.
    pub const fn compatibility_report_identity(&self) -> u64 {
        self.report_fingerprint
    }

    /// Domain-separated SHA-256 commitment to the complete exact selected
    /// closure. Artifact replay must use this value or retain the complete
    /// closure rather than relying on [`Self::compatibility_report_identity`].
    pub fn identity_digest(&self) -> SelectedProviderClosureDigest {
        let mut encoder = SelectedProviderClosureDigestEncoder::new();
        encoder.len(self.plans.len());
        for plan in &self.plans {
            encoder.bytes(plan.identity_digest().as_bytes());
        }
        encoder.execution_scope(self.execution_scope);

        encoder.len(self.opaque_executable_admissions.len());
        for admission in &self.opaque_executable_admissions {
            encoder.opaque_executable_admission(admission.candidate());
        }

        encoder.len(self.installation_reach_resolutions.len());
        for resolution in &self.installation_reach_resolutions {
            encoder.string(&resolution.requirement_identity);
            encoder.u64(resolution.provider_plan_report_identity);
            encoder.strings(&resolution.upper_bound);
            encoder.strings(&resolution.resolved_row);
        }
        encoder.len(self.specialized_requirement_rows.len());
        for row in &self.specialized_requirement_rows {
            encoder.u64(row.provider_plan_report_identity);
            encoder.string(&row.selected_requirement_identity);
            encoder.string(&row.selected_machine_identity);
            encoder.string(&row.requirement_identity);
            encoder.string(&row.machine_identity);
        }
        encoder.finish()
    }

    pub const fn execution_scope(&self) -> crate::ExecutionScope {
        self.execution_scope
    }

    /// Re-scope one selected closure before attaching opaque admissions. The
    /// provider-plan identity is unchanged; execution scope is artifact
    /// installation context rather than source/provider identity.
    pub fn with_execution_scope(
        mut self,
        execution_scope: crate::ExecutionScope,
    ) -> Result<Self, String> {
        if !self.opaque_executable_admissions.is_empty() {
            return Err(
                "selected provider closure must choose its execution scope before opaque executable admissions"
                    .into(),
            );
        }
        if matches!(execution_scope, crate::ExecutionScope::IsolatedProvider(0)) {
            return Err("isolated execution scope has the reserved zero identity".into());
        }
        self.execution_scope = execution_scope;
        Ok(self)
    }

    pub fn is_empty(&self) -> bool {
        self.plans.is_empty()
    }

    /// Bind trusted opaque-executable evidence to exact rows in this selected
    /// closure. Loader names are checked only for row drift; they never become
    /// executable identity.
    pub fn with_opaque_executable_admissions(
        mut self,
        candidates: impl IntoIterator<Item = crate::OpaqueExecutableAdmissionCandidate>,
    ) -> Result<Self, String> {
        let mut occupied = self
            .opaque_executable_admissions
            .iter()
            .map(|admission| {
                let candidate = admission.candidate();
                (
                    candidate.provider_plan_report_identity,
                    candidate.provider_plan_digest,
                    candidate.method.clone(),
                    candidate.requirement_identity.clone(),
                )
            })
            .collect::<BTreeSet<_>>();
        for candidate in candidates {
            if candidate.execution_scope != self.execution_scope {
                return Err(format!(
                    "opaque executable admission scope {:?} does not match selected closure scope {:?}",
                    candidate.execution_scope, self.execution_scope
                ));
            }
            let key = (
                candidate.provider_plan_report_identity,
                candidate.provider_plan_digest,
                candidate.method.clone(),
                candidate.requirement_identity.clone(),
            );
            if !occupied.insert(key) {
                return Err(format!(
                    "opaque executable admission duplicates selected row `{}` / `{}` in provider plan {:#018x}",
                    candidate.method,
                    candidate.requirement_identity,
                    candidate.provider_plan_report_identity
                ));
            }
            self.opaque_executable_admissions.push(
                crate::provider_evidence::executable_scopes::executable_tcb_manifest::validate_opaque_executable_admission(
                    &self.plans,
                    candidate,
                )?,
            );
        }
        self.opaque_executable_admissions.sort_by(|left, right| {
            let left = left.candidate();
            let right = right.candidate();
            left.provider_plan_report_identity
                .cmp(&right.provider_plan_report_identity)
                .then_with(|| left.method.cmp(&right.method))
                .then_with(|| left.requirement_identity.cmp(&right.requirement_identity))
        });
        Ok(self)
    }

    pub fn opaque_executable_admissions(&self) -> &[crate::ValidatedOpaqueExecutableAdmission] {
        &self.opaque_executable_admissions
    }

    /// Attach checked realization reach to provider-selected bounded
    /// requirements. The requirement ceiling stays in the provider schema;
    /// this row is derived implementation evidence used by root composition.
    /// One requirement identity resolves once per selected plan — distinct
    /// root traits inheriting the same requirement each retain their own row —
    /// so the roster key is the (requirement identity, provider plan report
    /// identity) pair.
    pub fn with_installation_reach_resolutions(
        mut self,
        mut resolutions: Vec<InstallationReachResolution>,
    ) -> Result<Self, String> {
        resolutions.sort_by(|left, right| {
            left.requirement_identity
                .cmp(&right.requirement_identity)
                .then_with(|| {
                    left.provider_plan_report_identity
                        .cmp(&right.provider_plan_report_identity)
                })
        });
        for pair in resolutions.windows(2) {
            if pair[0].requirement_identity == pair[1].requirement_identity
                && pair[0].provider_plan_report_identity == pair[1].provider_plan_report_identity
            {
                return Err(format!(
                    "installation reach requirement `{}` has more than one selected resolution under provider plan {:#018x}",
                    pair[0].requirement_identity, pair[0].provider_plan_report_identity
                ));
            }
        }
        for resolution in &mut resolutions {
            if resolution.requirement_identity.is_empty() {
                return Err(
                    "installation reach resolution has an empty requirement identity".into(),
                );
            }
            resolution.upper_bound.sort();
            resolution.upper_bound.dedup();
            resolution.resolved_row.sort();
            resolution.resolved_row.dedup();
            if resolution
                .resolved_row
                .iter()
                .any(|service| !resolution.upper_bound.contains(service))
            {
                return Err(format!(
                    "installation reach resolution for `{}` exceeds its published upper bound",
                    resolution.requirement_identity
                ));
            }
            let Some(plan) =
                self.plan_by_report_fingerprint(resolution.provider_plan_report_identity)
            else {
                return Err(format!(
                    "installation reach resolution for `{}` names unselected provider plan {:#018x}",
                    resolution.requirement_identity, resolution.provider_plan_report_identity
                ));
            };
            if !plan
                .rows
                .iter()
                .any(|row| row.requirement_identity == resolution.requirement_identity)
            {
                return Err(format!(
                    "installation reach resolution for `{}` is absent from selected provider plan `{}`",
                    resolution.requirement_identity, plan.name
                ));
            }
        }
        self.installation_reach_resolutions = resolutions;
        self.report_fingerprint = selected_closure_report_fingerprint(
            &self.plans,
            &self.installation_reach_resolutions,
            &self.specialized_requirement_rows,
        );
        Ok(self)
    }

    pub fn installation_reach_resolutions(&self) -> &[InstallationReachResolution] {
        &self.installation_reach_resolutions
    }

    /// Attach the template→closed requirement join derived from retained
    /// specialization evidence. Provider plans derive before specialization
    /// settlement, so a generic requirement's selected row retains the
    /// authored template identities; Terminal boundary declarations and
    /// provider candidates instead name the exact closed identities. These
    /// rows rebind the selected closure to them without rewriting any
    /// selected row: each record must name a selected plan's checked-adapter
    /// row, and a closed requirement resolves exactly once across the whole
    /// closure.
    pub fn with_specialized_requirement_rows(
        mut self,
        mut rows: Vec<SelectedRequirementSpecialization>,
    ) -> Result<Self, String> {
        rows.sort_by(|left, right| {
            left.requirement_identity
                .cmp(&right.requirement_identity)
                .then_with(|| {
                    left.provider_plan_report_identity
                        .cmp(&right.provider_plan_report_identity)
                })
                .then_with(|| left.machine_identity.cmp(&right.machine_identity))
        });
        for pair in rows.windows(2) {
            if pair[0].requirement_identity == pair[1].requirement_identity {
                return Err(format!(
                    "closed requirement `{}` has more than one selected specialization row",
                    pair[0].requirement_identity
                ));
            }
        }
        for row in &rows {
            if row.selected_requirement_identity.is_empty()
                || row.selected_machine_identity.is_empty()
                || row.requirement_identity.is_empty()
                || row.machine_identity.is_empty()
            {
                return Err("selected requirement specialization has an empty identity".into());
            }
            let Some(plan) = self.plan_by_report_fingerprint(row.provider_plan_report_identity)
            else {
                return Err(format!(
                    "selected requirement specialization for `{}` names unselected provider plan {:#018x}",
                    row.requirement_identity, row.provider_plan_report_identity
                ));
            };
            let bound = plan.rows.iter().any(|plan_row| {
                plan_row.requirement_identity == row.selected_requirement_identity
                    && matches!(
                        &plan_row.binding,
                        crate::provider_plan::ProviderBinding::CheckedAdapter {
                            machine_identity,
                            ..
                        } if *machine_identity == row.selected_machine_identity
                    )
            });
            if !bound {
                return Err(format!(
                    "selected requirement specialization for `{}` is absent from selected provider plan `{}`",
                    row.requirement_identity, plan.name
                ));
            }
        }
        self.specialized_requirement_rows = rows;
        self.report_fingerprint = selected_closure_report_fingerprint(
            &self.plans,
            &self.installation_reach_resolutions,
            &self.specialized_requirement_rows,
        );
        Ok(self)
    }

    /// The specialization roster: each row rebinds one template-keyed
    /// selected row to one closed requirement instance and its specialized
    /// provider instance.
    pub fn specialized_requirement_rows(&self) -> &[SelectedRequirementSpecialization] {
        &self.specialized_requirement_rows
    }

    /// The roster row joining one closed requirement identity to its selected
    /// plan row. A closed requirement belongs to one requirement template's
    /// boundary slot, so it resolves once across the whole closure.
    pub fn specialized_requirement_row(
        &self,
        requirement_identity: &str,
    ) -> Option<&SelectedRequirementSpecialization> {
        self.specialized_requirement_rows
            .iter()
            .find(|row| row.requirement_identity == requirement_identity)
    }

    /// The roster rows under one selected plan row — every closed requirement
    /// instance that template-keyed row serves through a specialized provider
    /// instance.
    pub fn specialized_requirement_rows_for_row<'a>(
        &'a self,
        provider_plan_report_identity: u64,
        selected_requirement_identity: &'a str,
    ) -> impl Iterator<Item = &'a SelectedRequirementSpecialization> + 'a {
        self.specialized_requirement_rows.iter().filter(move |row| {
            row.provider_plan_report_identity == provider_plan_report_identity
                && row.selected_requirement_identity == selected_requirement_identity
        })
    }

    /// Every Terminal-facing requirement identity one selected plan row
    /// serves: the roster's closed identities, or the row's own identity when
    /// its requirement was never specialized.
    pub fn selected_row_requirement_identities<'a>(
        &'a self,
        provider_plan_report_identity: u64,
        row_requirement_identity: &'a str,
    ) -> Vec<&'a str> {
        let specialized = self
            .specialized_requirement_rows_for_row(
                provider_plan_report_identity,
                row_requirement_identity,
            )
            .map(|row| row.requirement_identity.as_str())
            .collect::<Vec<_>>();
        if specialized.is_empty() {
            vec![row_requirement_identity]
        } else {
            specialized
        }
    }

    /// The requirement identity the selected row retains for one queried
    /// requirement: a roster row rebinds its closed Terminal identity to the
    /// template identity its selected row carries. Unspecialized identities
    /// pass through unchanged.
    pub fn selected_requirement_identity<'a>(&'a self, requirement: &'a str) -> &'a str {
        self.specialized_requirement_row(requirement)
            .map_or(requirement, |row| {
                row.selected_requirement_identity.as_str()
            })
    }

    /// Every closed requirement identity the roster binds to one retained
    /// requirement identity, across every selected plan. Empty when no
    /// roster row rebinds it — the identity is either not generic or was
    /// never demanded under a closed instance.
    pub fn closed_requirement_identities<'a>(
        &'a self,
        requirement_identity: &'a str,
    ) -> Vec<&'a str> {
        self.specialized_requirement_rows
            .iter()
            .filter(move |row| row.selected_requirement_identity == requirement_identity)
            .map(|row| row.requirement_identity.as_str())
            .collect()
    }

    /// Requirement lookup for identities realized by exactly one selected
    /// plan. A shared requirement identity — distinct root traits may inherit
    /// the same requirement, so several plans can each carry its row — is
    /// ambiguous here and yields `None`; scoped callers use
    /// [`Self::installation_reach_resolution_for_plan`].
    pub fn installation_reach_resolution(
        &self,
        requirement_identity: &str,
    ) -> Option<&InstallationReachResolution> {
        let requirement_identity = self.selected_requirement_identity(requirement_identity);
        let mut matches = self
            .installation_reach_resolutions
            .iter()
            .filter(|resolution| resolution.requirement_identity == requirement_identity);
        let resolution = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        Some(resolution)
    }

    /// Resolve one requirement identity under the selected provider plan that
    /// realized it for this root. Root traits inheriting a shared requirement
    /// each retain a row under their own selected plan, so the pair
    /// (requirement identity, provider plan report identity) is the exact
    /// resolution address.
    pub fn installation_reach_resolution_for_plan(
        &self,
        provider_plan_report_identity: u64,
        requirement_identity: &str,
    ) -> Option<&InstallationReachResolution> {
        let requirement_identity = self.selected_requirement_identity(requirement_identity);
        self.installation_reach_resolutions
            .iter()
            .find(|resolution| {
                resolution.provider_plan_report_identity == provider_plan_report_identity
                    && resolution.requirement_identity == requirement_identity
            })
    }

    /// Resolve one root closure from its concrete reach plus exact bounded
    /// requirement dependencies. Absence rejects; an upper bound is never
    /// silently used as the selected row. A requirement identity realized by
    /// more than one selected plan is ambiguous unscoped and rejects —
    /// resolve it through the owning root's plan with
    /// [`Self::resolve_installation_reach_for_plan`].
    pub fn resolve_installation_reach(
        &self,
        concrete_reach: &[String],
        requirement_identities: &[String],
    ) -> Result<Vec<String>, String> {
        let mut resolved = concrete_reach.to_vec();
        for requirement_identity in requirement_identities {
            let selected_identity = self.selected_requirement_identity(requirement_identity);
            if let Some(row) = self.installation_reach_resolution(selected_identity) {
                resolved.extend(row.resolved_row.iter().cloned());
                continue;
            }
            if self
                .installation_reach_resolutions
                .iter()
                .any(|resolution| resolution.requirement_identity == *selected_identity)
            {
                return Err(format!(
                    "installation reach requirement `{requirement_identity}` is realized by more than one selected provider plan; resolve it through the root's selected provider plan"
                ));
            }
            return Err(format!(
                "installation reach requirement `{requirement_identity}` remains unresolved at final admission"
            ));
        }
        resolved.sort();
        resolved.dedup();
        Ok(resolved)
    }

    /// Resolve one root closure from its concrete reach plus exact bounded
    /// requirement dependencies, addressed through the root's own selected
    /// provider plan. A requirement the root's plan realizes binds that
    /// plan's row; a requirement realized by exactly one other selected plan
    /// still resolves unscoped. Any other multiplicity rejects.
    pub fn resolve_installation_reach_for_plan(
        &self,
        concrete_reach: &[String],
        requirement_identities: &[String],
        provider_plan_report_identity: u64,
    ) -> Result<Vec<String>, String> {
        let mut resolved = concrete_reach.to_vec();
        for requirement_identity in requirement_identities {
            let selected_identity = self.selected_requirement_identity(requirement_identity);
            if let Some(row) = self.installation_reach_resolution_for_plan(
                provider_plan_report_identity,
                selected_identity,
            ) {
                resolved.extend(row.resolved_row.iter().cloned());
                continue;
            }
            if let Some(row) = self.installation_reach_resolution(selected_identity) {
                resolved.extend(row.resolved_row.iter().cloned());
                continue;
            }
            if self
                .installation_reach_resolutions
                .iter()
                .any(|resolution| resolution.requirement_identity == *selected_identity)
            {
                return Err(format!(
                    "installation reach requirement `{requirement_identity}` is realized by more than one selected provider plan, none under provider plan {provider_plan_report_identity:#018x}"
                ));
            }
            return Err(format!(
                "installation reach requirement `{requirement_identity}` remains unresolved at final admission"
            ));
        }
        resolved.sort();
        resolved.dedup();
        Ok(resolved)
    }

    /// Derive caller-address-space TCB facts from the selected closure, never
    /// from source service reach or the unselected candidate set.
    pub fn executable_tcb_manifest(&self) -> crate::ExecutableTcbManifest {
        crate::provider_evidence::executable_scopes::executable_tcb_manifest::derive_static_manifest(
            &self.plans,
            self.report_fingerprint,
            self.execution_scope,
            &self.opaque_executable_admissions,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallationReachResolution {
    pub requirement_identity: String,
    /// Compact report coordinate; the owning selected closure retains the
    /// exact plan and its strong digest.
    pub provider_plan_report_identity: u64,
    pub upper_bound: Vec<String>,
    pub resolved_row: Vec<String>,
}

/// The post-specialization realization of one template-keyed selected plan
/// row: which exact closed requirement instance it serves and which
/// specialized provider instance realizes it.
///
/// Provider plans derive before specialization settlement, so a selected row
/// for a generic requirement retains the authored template identities.
/// Terminal boundary declarations and provider candidates instead carry the
/// closed identities specialization minted. This row is the exact join
/// between the two — derived by replaying the retained specialization
/// receipts and the provider's recorded closed applications, never display
/// names or runtime signatures. The selected row itself is never rewritten.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedRequirementSpecialization {
    /// Compact report coordinate of the owning selected plan; the retained
    /// closure and its strong digest remain the replay authority.
    pub provider_plan_report_identity: u64,
    /// The exact requirement identity the selected row retains: the authored
    /// generic requirement template.
    pub selected_requirement_identity: String,
    /// The exact provider machine identity the selected row's checked-adapter
    /// binding names: the authored generic provider template.
    pub selected_machine_identity: String,
    /// The closed requirement identity Terminal boundary declarations carry.
    pub requirement_identity: String,
    /// The specialized provider instance identity Terminal provider
    /// candidates carry as `candidate_identity`.
    pub machine_identity: String,
}

fn selected_plan_order(left: &ProviderPlan, right: &ProviderPlan) -> std::cmp::Ordering {
    left.name
        .cmp(&right.name)
        .then_with(|| {
            left.origin_package_identity
                .cmp(&right.origin_package_identity)
        })
        .then_with(|| {
            left.provider_type_package_identity
                .cmp(&right.provider_type_package_identity)
        })
        .then_with(|| {
            left.schema
                .trait_package_identity
                .cmp(&right.schema.trait_package_identity)
        })
        .then_with(|| left.report_fingerprint().cmp(&right.report_fingerprint()))
}

fn selected_plans_report_fingerprint(plans: &[ProviderPlan]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in (plans.len() as u64).to_le_bytes().into_iter().chain(
        plans
            .iter()
            .flat_map(|plan| plan.report_fingerprint().to_le_bytes()),
    ) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn selected_closure_report_fingerprint(
    plans: &[ProviderPlan],
    resolutions: &[InstallationReachResolution],
    specialization_rows: &[SelectedRequirementSpecialization],
) -> u64 {
    let mut hash = selected_plans_report_fingerprint(plans);
    for resolution in resolutions {
        for byte in resolution
            .requirement_identity
            .as_bytes()
            .iter()
            .copied()
            .chain(resolution.provider_plan_report_identity.to_le_bytes())
            .chain((resolution.upper_bound.len() as u64).to_le_bytes())
            .chain(
                resolution
                    .upper_bound
                    .iter()
                    .flat_map(|service| service.as_bytes().iter().copied().chain([0])),
            )
            .chain((resolution.resolved_row.len() as u64).to_le_bytes())
            .chain(
                resolution
                    .resolved_row
                    .iter()
                    .flat_map(|service| service.as_bytes().iter().copied().chain([0])),
            )
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    for row in specialization_rows {
        for byte in row
            .provider_plan_report_identity
            .to_le_bytes()
            .into_iter()
            .chain(row.selected_requirement_identity.as_bytes().iter().copied())
            .chain([0])
            .chain(row.selected_machine_identity.as_bytes().iter().copied())
            .chain([0])
            .chain(row.requirement_identity.as_bytes().iter().copied())
            .chain([0])
            .chain(row.machine_identity.as_bytes().iter().copied())
            .chain([0])
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}

struct SelectedProviderClosureDigestEncoder(Sha256);

impl SelectedProviderClosureDigestEncoder {
    fn new() -> Self {
        let mut digest = Sha256::new();
        digest.update(b"omega.selected-provider-closure.sha256.v1\0");
        Self(digest)
    }

    fn finish(self) -> SelectedProviderClosureDigest {
        SelectedProviderClosureDigest::from_digest(self.0.finalize().into())
    }

    fn byte(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn len(&mut self, value: usize) {
        self.0.update((value as u64).to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.0.update(value.to_le_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.0.update(value.to_le_bytes());
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.len(bytes.len());
        self.0.update(bytes);
    }

    fn string(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    fn strings(&mut self, values: &[String]) {
        self.len(values.len());
        for value in values {
            self.string(value);
        }
    }

    fn optional_string(&mut self, value: Option<&str>) {
        match value {
            Some(value) => {
                self.byte(1);
                self.string(value);
            }
            None => self.byte(0),
        }
    }

    fn execution_scope(&mut self, scope: crate::ExecutionScope) {
        match scope {
            crate::ExecutionScope::CallerAddressSpace => self.byte(0),
            crate::ExecutionScope::IsolatedProvider(identity) => {
                self.byte(1);
                self.u64(identity);
            }
        }
    }

    fn opaque_executable_admission(
        &mut self,
        admission: &crate::OpaqueExecutableAdmissionCandidate,
    ) {
        self.u64(admission.provider_plan_report_identity);
        self.bytes(admission.provider_plan_digest.as_bytes());
        self.string(&admission.method);
        self.string(&admission.requirement_identity);
        self.opaque_binding(&admission.binding);
        self.string(&admission.executable_identity);
        self.string(&admission.implementation_evidence_identity);
        self.execution_scope(admission.execution_scope);
        self.len(admission.containment.len());
        for evidence in &admission.containment {
            self.byte(match evidence.guarantee {
                crate::ContainmentGuarantee::MemoryIsolation => 0,
                crate::ContainmentGuarantee::ForcibleTermination => 1,
                crate::ContainmentGuarantee::FaultContainment => 2,
                crate::ContainmentGuarantee::BoundedResources => 3,
            });
            self.string(&evidence.evidence_identity);
        }
        self.optional_string(admission.executable_closure_evidence_identity.as_deref());
    }

    fn opaque_binding(&mut self, binding: &crate::OpaqueInProcessBinding) {
        match binding {
            crate::OpaqueInProcessBinding::Import { evaluated } => {
                let locator = evaluated.locator();
                self.byte(0);
                self.string(locator.target().target_name());
                match locator.locator() {
                    crate::ForeignLocatorCandidate::PeByName { library, export } => {
                        self.byte(0);
                        self.bytes(library);
                        self.bytes(export);
                    }
                    crate::ForeignLocatorCandidate::PeByOrdinal { library, ordinal } => {
                        self.byte(1);
                        self.bytes(library);
                        self.0.update(ordinal.to_le_bytes());
                    }
                    crate::ForeignLocatorCandidate::ElfVersioned {
                        object,
                        symbol,
                        version,
                    } => {
                        self.byte(2);
                        self.bytes(object);
                        self.bytes(symbol);
                        self.bytes(version);
                    }
                    crate::ForeignLocatorCandidate::MachODylibSymbol {
                        install_name,
                        symbol,
                    } => {
                        self.byte(3);
                        self.bytes(install_name);
                        self.bytes(symbol);
                    }
                }
                self.bytes(&evaluated.receipt().identity_digest());
            }
            // Tag byte 1 belonged to the retired string-backed import
            // bootstrap and stays unassigned.
            crate::OpaqueInProcessBinding::VtableSlot { index } => {
                self.byte(2);
                self.i64(*index);
            }
            crate::OpaqueInProcessBinding::VtableField { table, field } => {
                self.byte(3);
                self.string(table);
                self.string(field);
            }
            crate::OpaqueInProcessBinding::TableFunction { table, field } => {
                self.byte(4);
                self.string(table);
                self.string(field);
            }
        }
    }
}

#[cfg(test)]
mod tests;
