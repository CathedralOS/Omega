//! Selected provider evidence: the closure of provider plans a selection actually
//! reached — which provider realizes each boundary requirement — and its
//! digest. The retained evidence beside it: `providers` records provider
//! plans, approvals and boundary-call audits; `authority` the terminal
//! authority dispositions and service permissions; `executable_scopes` the
//! executable bytes, services and behavior exclusions admitted into a scope;
//! `component_eras` the era ledger and progress manifests.
//! Selected requirement identities are interned when plans enter the closure,
//! before optional specialization or family relations are attached. Execution
//! receipts use this closure-local table, not a plan's unrelated coordinates.

pub(crate) mod authority;
pub(crate) mod component_eras;
pub(crate) use optimization_core::digest_encoder;
pub(crate) mod executable_scopes;
mod external_requirements;
pub(crate) mod providers;
pub use external_requirements::SelectedExternalRequirementApplication;

use crate::provider_evidence::digest_encoder::DomainDigestEncoder;
use crate::provider_plan::ProviderPlan;
use std::collections::BTreeSet;

/// Strong `[u8; 32]` evidence digests: `from_digest` constructs at the
/// visibility the caller's contract needs, `as_bytes` hands back the bytes.
macro_rules! evidence_digest {
    ($(#[$attr:meta])* $name:ident, from: $(#[$fdoc:meta])* $fv:vis) => {
        $(#[$attr])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            $(#[$fdoc])*
            $fv const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
    ($(#[$attr:meta])* $name:ident) => {
        $(#[$attr])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

pub(crate) use evidence_digest;

evidence_digest!(
    /// Collision-resistant identity of one complete selected-provider
    /// closure.
    ///
    /// This commits the exact selected plans and every attached closure
    /// fact. The existing compact normalized identity remains a
    /// compatibility/report coordinate and must not authorize artifact
    /// replay by itself.
    SelectedProviderClosureDigest
);

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
    external_requirement_applications: Vec<SelectedExternalRequirementApplication>,
    /// Exact family aliases of selected rows, closed against one Terminal artifact.
    family_realization_rows: Vec<SelectedTargetFamilyRealization>,
    /// Compact identities of the members settled by the toolchain itself
    /// (canonical-host mints). Report surfaces use this to label their
    /// provenance honestly instead of reporting them as ungranted
    /// dev-active packages.
    toolchain_settled_identities: BTreeSet<u64>,
    /// Closure-local terminal identity table every interned `*identity`
    /// field on the retained rows resolves against.
    identities: terminal_psi::TerminalIdentityTable,
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
            external_requirement_applications: Vec::new(),
            family_realization_rows: Vec::new(),
            toolchain_settled_identities: BTreeSet::new(),
            identities: terminal_psi::TerminalIdentityTable::default(),
        }
    }
}

impl SelectedProviderPlanFacts {
    /// Retain already-resolved provider plans without rejoining them through a
    /// readable plan name. Authored selection paths belong before this
    /// boundary; every plan here already carries its package-qualified slot,
    /// provider, requirement, and realization provenance.
    pub fn from_selected_plans(mut plans: Vec<ProviderPlan>) -> Result<Self, String> {
        plans.sort_by(ProviderPlan::selection_order);

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
                plan.schema.identity_text(plan.schema.trait_name),
            );
            if !boundary_slots.insert(slot) {
                return Err(format!(
                    "boundary slot `{}` has more than one selected provider plan",
                    plan.schema.identity_text(plan.schema.trait_name)
                ));
            }
        }

        let report_fingerprint = selected_plans_report_fingerprint(&plans);
        let mut requirement_identities = terminal_psi::TerminalIdentityTableBuilder::default();
        for plan in &plans {
            for row in &plan.rows {
                requirement_identities.intern(plan.identities.text(row.requirement_identity));
            }
        }
        Ok(Self {
            plans,
            report_fingerprint,
            execution_scope: crate::ExecutionScope::CallerAddressSpace,
            opaque_executable_admissions: Vec::new(),
            installation_reach_resolutions: Vec::new(),
            specialized_requirement_rows: Vec::new(),
            external_requirement_applications: Vec::new(),
            family_realization_rows: Vec::new(),
            toolchain_settled_identities: BTreeSet::new(),
            identities: requirement_identities.build(),
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
            || !self.external_requirement_applications.is_empty()
            || !self.family_realization_rows.is_empty()
        {
            return Err(
                "selected provider closure must join toolchain-settled plans before attaching installation reach, specialization, or family evidence"
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
                && existing.schema.identity_text(existing.schema.trait_name)
                    == plan.schema.identity_text(plan.schema.trait_name)
        }) {
            return Err(format!(
                "boundary slot `{}` has more than one selected provider plan",
                plan.schema.identity_text(plan.schema.trait_name)
            ));
        }
        let mut requirement_identities = self.identities.clone().into_builder();
        for row in &plan.rows {
            requirement_identities.intern(plan.identities.text(row.requirement_identity));
        }
        self.identities = requirement_identities.build();
        self.plans.push(plan);
        self.plans.sort_by(ProviderPlan::selection_order);
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

    pub fn plans(&self) -> &[ProviderPlan] {
        &self.plans
    }

    /// Extend an invocation's selection with exact demanded foreign callables.
    /// The caller projects already-derived candidates through executable scope;
    /// this does not classify mechanisms or grant package/runtime authority.
    pub fn with_direct_foreign_plans(mut self, plans: Vec<ProviderPlan>) -> Result<Self, String> {
        let checked = Self::from_selected_plans(plans)?;
        for plan in &checked.plans {
            if self.plans.iter().any(|existing| {
                existing.report_fingerprint() == plan.report_fingerprint()
                    || (existing.schema.trait_package_identity
                        == plan.schema.trait_package_identity
                        && existing.schema.identity_text(existing.schema.trait_name)
                            == plan.schema.identity_text(plan.schema.trait_name))
            }) {
                return Err(format!(
                    "direct foreign callable `{}` collides with an existing selected plan",
                    plan.name
                ));
            }
        }
        let mut identities = self.identities.clone().into_builder();
        for plan in &checked.plans {
            for row in &plan.rows {
                identities.intern(plan.identities.text(row.requirement_identity));
            }
        }
        self.identities = identities.build();
        self.plans.extend(checked.plans);
        self.plans.sort_by(ProviderPlan::selection_order);
        self.report_fingerprint = selected_closure_report_fingerprint(
            &self.plans,
            &self.installation_reach_resolutions,
            &self.specialized_requirement_rows,
            &self.family_realization_rows,
            &self.external_requirement_applications,
            &self.identities,
        );
        Ok(self)
    }

    /// Compatibility/report lookup for existing compact-ID consumers. Code
    /// making an admission or execution decision must use
    /// [`Self::plan_by_exact_evidence`] instead.
    pub(crate) fn plan_by_report_fingerprint(
        &self,
        report_fingerprint: u64,
    ) -> Option<&ProviderPlan> {
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
            encoder.string(
                self.identities
                    .text(resolution.requirement_identity)
                    .unwrap_or_default(),
            );
            encoder.u64(resolution.provider_plan_report_identity);
            encoder.strings(&resolution.upper_bound);
            encoder.strings(&resolution.resolved_row);
        }
        encoder.len(self.specialized_requirement_rows.len());
        for row in &self.specialized_requirement_rows {
            encoder.u64(row.provider_plan_report_identity);
            encoder.string(
                self.identities
                    .text(row.selected_requirement_identity)
                    .unwrap_or_default(),
            );
            encoder.string(
                self.identities
                    .text(row.selected_machine_identity)
                    .unwrap_or_default(),
            );
            encoder.string(
                self.identities
                    .text(row.requirement_identity)
                    .unwrap_or_default(),
            );
            encoder.string(
                self.identities
                    .text(row.machine_identity)
                    .unwrap_or_default(),
            );
        }
        encoder.len(self.external_requirement_applications.len());
        for row in &self.external_requirement_applications {
            encoder.u64(row.provider_plan_report_identity);
            encoder.bytes(row.provider_plan_digest.as_bytes());
            encoder.string(
                self.identities
                    .text(row.selected_requirement_identity)
                    .unwrap_or_default(),
            );
            encoder.string(
                self.identities
                    .text(row.requirement_identity)
                    .unwrap_or_default(),
            );
        }
        encoder.len(self.family_realization_rows.len());
        for row in &self.family_realization_rows {
            encoder.u64(u64::from(row.terminal.vocabulary_marker.get()));
            encoder.bytes(row.terminal.program_fingerprint.as_bytes());
            encoder.string(&row.target);
            encoder.u64(row.head.get());
            encoder.u64(row.member.get());
            encoder.string(self.identities.text(row.head_identity).unwrap_or_default());
            encoder.string(
                self.identities
                    .text(row.member_identity)
                    .unwrap_or_default(),
            );
            encoder.string(
                self.identities
                    .text(row.selected_requirement_identity)
                    .unwrap_or_default(),
            );
            encoder.u64(row.provider_plan_report_identity);
            encoder.bytes(row.provider_plan_digest.as_bytes());
        }
        encoder.finish()
    }

    pub const fn execution_scope(&self) -> crate::ExecutionScope {
        self.execution_scope
    }

    /// The closure-local terminal identity table every retained `*identity`
    /// field resolves against.
    pub const fn identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.identities
    }

    pub fn is_empty(&self) -> bool {
        self.plans.is_empty()
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
        resolution_identities: &terminal_psi::TerminalIdentityTable,
    ) -> Result<Self, String> {
        let mut identities = self.identities.clone().into_builder();
        for resolution in &mut resolutions {
            resolution.requirement_identity = identities.intern(
                resolution_identities
                    .text(resolution.requirement_identity)
                    .unwrap_or_default(),
            );
        }
        self.identities = identities.build();
        resolutions.sort_by_cached_key(|resolution| {
            (
                self.identities
                    .text(resolution.requirement_identity)
                    .unwrap_or_default()
                    .to_owned(),
                resolution.provider_plan_report_identity,
            )
        });
        for pair in resolutions.windows(2) {
            if pair[0].requirement_identity == pair[1].requirement_identity
                && pair[0].provider_plan_report_identity == pair[1].provider_plan_report_identity
            {
                return Err(format!(
                    "installation reach requirement `{}` has more than one selected resolution under provider plan {:#018x}",
                    self.identities
                        .text(pair[0].requirement_identity)
                        .unwrap_or_default(),
                    pair[0].provider_plan_report_identity
                ));
            }
        }
        for resolution in &mut resolutions {
            if resolution.requirement_identity.is_absent() {
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
                    self.identities
                        .text(resolution.requirement_identity)
                        .unwrap_or_default()
                ));
            }
            let Some(plan) =
                self.plan_by_report_fingerprint(resolution.provider_plan_report_identity)
            else {
                return Err(format!(
                    "installation reach resolution for `{}` names unselected provider plan {:#018x}",
                    self.identities
                        .text(resolution.requirement_identity)
                        .unwrap_or_default(),
                    resolution.provider_plan_report_identity
                ));
            };
            if !plan.rows.iter().any(|row| {
                Some(plan.identities.text(row.requirement_identity))
                    == self.identities.text(resolution.requirement_identity)
            }) {
                return Err(format!(
                    "installation reach resolution for `{}` is absent from selected provider plan `{}`",
                    self.identities
                        .text(resolution.requirement_identity)
                        .unwrap_or_default(),
                    plan.name
                ));
            }
        }
        self.installation_reach_resolutions = resolutions;
        self.report_fingerprint = selected_closure_report_fingerprint(
            &self.plans,
            &self.installation_reach_resolutions,
            &self.specialized_requirement_rows,
            &self.family_realization_rows,
            &self.external_requirement_applications,
            &self.identities,
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
    /// row and its specialized instance, or a leaf row (import, intrinsic,
    /// system call) that names the template as its own realization, and a
    /// closed requirement resolves exactly once across the whole closure.
    pub fn with_specialized_requirement_rows(
        mut self,
        mut rows: Vec<SelectedRequirementSpecialization>,
        row_identities: &terminal_psi::TerminalIdentityTable,
    ) -> Result<Self, String> {
        let mut identities = self.identities.clone().into_builder();
        for row in &mut rows {
            row.selected_requirement_identity = identities.intern(
                row_identities
                    .text(row.selected_requirement_identity)
                    .unwrap_or_default(),
            );
            row.selected_machine_identity = identities.intern(
                row_identities
                    .text(row.selected_machine_identity)
                    .unwrap_or_default(),
            );
            row.requirement_identity = identities.intern(
                row_identities
                    .text(row.requirement_identity)
                    .unwrap_or_default(),
            );
            row.machine_identity = identities.intern(
                row_identities
                    .text(row.machine_identity)
                    .unwrap_or_default(),
            );
        }
        self.identities = identities.build();
        rows.sort_by_cached_key(|row| {
            (
                self.identities
                    .text(row.requirement_identity)
                    .unwrap_or_default()
                    .to_owned(),
                row.provider_plan_report_identity,
                self.identities
                    .text(row.machine_identity)
                    .unwrap_or_default()
                    .to_owned(),
            )
        });
        for pair in rows.windows(2) {
            if pair[0].requirement_identity == pair[1].requirement_identity {
                return Err(format!(
                    "closed requirement `{}` has more than one selected specialization row",
                    self.identities
                        .text(pair[0].requirement_identity)
                        .unwrap_or_default()
                ));
            }
        }
        for row in &rows {
            if self
                .external_requirement_application(
                    self.identities
                        .text(row.requirement_identity)
                        .unwrap_or_default(),
                )
                .is_some()
            {
                return Err("checked specialization shadows a closed external requirement".into());
            }
            if row.selected_requirement_identity.is_absent()
                || row.selected_machine_identity.is_absent()
                || row.requirement_identity.is_absent()
                || row.machine_identity.is_absent()
            {
                return Err("selected requirement specialization has an empty identity".into());
            }
            let Some(plan) = self.plan_by_report_fingerprint(row.provider_plan_report_identity)
            else {
                return Err(format!(
                    "selected requirement specialization for `{}` names unselected provider plan {:#018x}",
                    self.identities
                        .text(row.requirement_identity)
                        .unwrap_or_default(),
                    row.provider_plan_report_identity
                ));
            };
            let bound = plan.rows.iter().any(|plan_row| {
                Some(plan.identities.text(plan_row.requirement_identity))
                    == self.identities.text(row.selected_requirement_identity)
                    && match &plan_row.binding {
                        crate::provider_plan::ProviderBinding::CheckedAdapter {
                            machine_identity,
                            ..
                        } => {
                            Some(plan.identity_text(*machine_identity))
                                == self.identities.text(row.selected_machine_identity)
                        }
                        // A leaf has no provider instance per closed tuple:
                        // its row names the template as its own realization.
                        crate::provider_plan::ProviderBinding::Import { .. }
                        | crate::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
                        | crate::provider_plan::ProviderBinding::Syscall { .. } => {
                            row.selected_machine_identity == row.selected_requirement_identity
                                && row.machine_identity == row.selected_requirement_identity
                        }
                        _ => false,
                    }
            });
            if !bound {
                return Err(format!(
                    "selected requirement specialization for `{}` is absent from selected provider plan `{}`",
                    self.identities
                        .text(row.requirement_identity)
                        .unwrap_or_default(),
                    plan.name
                ));
            }
        }
        self.specialized_requirement_rows = rows;
        self.validate_family_realization_rows(&self.family_realization_rows)?;
        self.report_fingerprint = selected_closure_report_fingerprint(
            &self.plans,
            &self.installation_reach_resolutions,
            &self.specialized_requirement_rows,
            &self.family_realization_rows,
            &self.external_requirement_applications,
            &self.identities,
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
        self.specialized_requirement_rows.iter().find(|row| {
            self.identities.text(row.requirement_identity) == Some(requirement_identity)
        })
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
                && self.identities.text(row.selected_requirement_identity)
                    == Some(selected_requirement_identity)
        })
    }

    /// Every Terminal-facing requirement identity one selected plan row
    /// serves: the roster's closed identities, or the row's own identity when
    /// its requirement was never specialized, plus exact family aliases.
    pub fn selected_row_requirement_identities<'a>(
        &'a self,
        provider_plan_report_identity: u64,
        row_requirement_identity: &'a str,
    ) -> Vec<&'a str> {
        let mut identities = self
            .specialized_requirement_rows_for_row(
                provider_plan_report_identity,
                row_requirement_identity,
            )
            .map(|row| {
                self.identities
                    .text(row.requirement_identity)
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        identities.extend(
            self.external_requirement_applications
                .iter()
                .filter(|row| {
                    row.provider_plan_report_identity == provider_plan_report_identity
                        && self.identities.text(row.selected_requirement_identity)
                            == Some(row_requirement_identity)
                })
                .filter_map(|row| self.identities.text(row.requirement_identity)),
        );
        if identities.is_empty() {
            identities.push(row_requirement_identity);
        }
        for row in &self.family_realization_rows {
            if row.provider_plan_report_identity == provider_plan_report_identity
                && self.identities.text(row.selected_requirement_identity)
                    == Some(row_requirement_identity)
            {
                identities.extend([
                    self.identities.text(row.head_identity).unwrap_or_default(),
                    self.identities
                        .text(row.member_identity)
                        .unwrap_or_default(),
                ]);
            }
        }
        identities.sort_unstable();
        identities.dedup();
        identities
    }

    /// Rejoin a Terminal identity to its selected row without changing the
    /// original call's contract. Family and specialization custody stay distinct.
    pub fn selected_requirement_identity<'a>(&'a self, requirement: &'a str) -> &'a str {
        if let Some(row) = self.family_realization_row(requirement) {
            return self
                .identities
                .text(row.selected_requirement_identity)
                .unwrap_or_default();
        }
        if let Some(row) = self.external_requirement_application(requirement) {
            return self
                .identities
                .text(row.selected_requirement_identity)
                .unwrap_or_default();
        }
        self.specialized_requirement_row(requirement)
            .map_or(requirement, |row| {
                self.identities
                    .text(row.selected_requirement_identity)
                    .unwrap_or_default()
            })
    }

    /// Exact plan-scoped row matching, including retained family and closed
    /// requirement coordinates. An alias cannot borrow a different plan's row.
    pub fn selected_row_matches_requirement(
        &self,
        requirement: &str,
        selected_requirement: &str,
        provider_plan_report_identity: u64,
    ) -> bool {
        if let Some(row) = self.family_realization_row(requirement) {
            return row.provider_plan_report_identity == provider_plan_report_identity
                && self.identities.text(row.selected_requirement_identity)
                    == Some(selected_requirement);
        }
        if let Some(row) = self.specialized_requirement_row(requirement) {
            return row.provider_plan_report_identity == provider_plan_report_identity
                && self.identities.text(row.selected_requirement_identity)
                    == Some(selected_requirement);
        }
        if let Some(row) = self.external_requirement_application(requirement) {
            return row.provider_plan_report_identity == provider_plan_report_identity
                && self.identities.text(row.selected_requirement_identity)
                    == Some(selected_requirement);
        }
        requirement == selected_requirement
    }

    /// The selected plan row serving one Terminal requirement identity, joined
    /// through the same family and closed-requirement coordinates as
    /// `selected_row_matches_requirement`. Selection resolves each requirement
    /// to at most one row and each schema roster names a requirement once, so
    /// the first match is the row; `None` means no provider was selected for
    /// the requirement at all.
    pub fn selected_requirement_row(
        &self,
        requirement: &str,
    ) -> Option<(&ProviderPlan, &crate::provider_plan::ProviderPlanRow)> {
        self.plans.iter().find_map(|plan| {
            plan.rows
                .iter()
                .find(|row| {
                    self.selected_row_matches_requirement(
                        requirement,
                        plan.identities.text(row.requirement_identity),
                        plan.report_fingerprint(),
                    )
                })
                .map(|row| (plan, row))
        })
    }

    /// Attach the source producer's exact realization-to-row relation. This
    /// checks closure consistency, not family membership or provider permission:
    /// the proposal producer must close IDs and identities against its artifact
    /// and selected target before execution consumes this retained relation.
    pub fn with_family_realization_rows(
        mut self,
        mut rows: Vec<SelectedTargetFamilyRealization>,
        row_identities: &terminal_psi::TerminalIdentityTable,
    ) -> Result<Self, String> {
        let mut identities = self.identities.clone().into_builder();
        for row in &mut rows {
            row.head_identity =
                identities.intern(row_identities.text(row.head_identity).unwrap_or_default());
            row.member_identity =
                identities.intern(row_identities.text(row.member_identity).unwrap_or_default());
            row.selected_requirement_identity = identities.intern(
                row_identities
                    .text(row.selected_requirement_identity)
                    .unwrap_or_default(),
            );
        }
        self.identities = identities.build();
        rows.sort_by_cached_key(|row| {
            self.identities
                .text(row.head_identity)
                .unwrap_or_default()
                .to_owned()
        });
        self.validate_family_realization_rows(&rows)?;
        self.family_realization_rows = rows;
        self.report_fingerprint = selected_closure_report_fingerprint(
            &self.plans,
            &self.installation_reach_resolutions,
            &self.specialized_requirement_rows,
            &self.family_realization_rows,
            &self.external_requirement_applications,
            &self.identities,
        );
        Ok(self)
    }

    pub fn family_realization_rows(&self) -> &[SelectedTargetFamilyRealization] {
        &self.family_realization_rows
    }

    pub fn family_realization_row(
        &self,
        identity: &str,
    ) -> Option<&SelectedTargetFamilyRealization> {
        self.family_realization_rows.iter().find(|row| {
            self.identities.text(row.head_identity) == Some(identity)
                || self.identities.text(row.member_identity) == Some(identity)
        })
    }

    fn validate_family_realization_rows(
        &self,
        rows: &[SelectedTargetFamilyRealization],
    ) -> Result<(), String> {
        let mut identities = BTreeSet::new();
        let mut boundaries = BTreeSet::new();
        for row in rows {
            if row.target.is_empty()
                || row.head_identity.is_absent()
                || row.member_identity.is_absent()
                || row.selected_requirement_identity.is_absent()
                || !identities.insert(
                    self.identities
                        .text(row.head_identity)
                        .unwrap_or_default()
                        .to_owned(),
                )
                || !identities.insert(
                    self.identities
                        .text(row.member_identity)
                        .unwrap_or_default()
                        .to_owned(),
                )
                || !boundaries.insert(row.head)
                || !boundaries.insert(row.member)
            {
                return Err(
                    "selected family realization has empty or ambiguous coordinates".into(),
                );
            }
            if rows
                .first()
                .is_some_and(|first| first.terminal != row.terminal || first.target != row.target)
            {
                return Err(
                    "selected family realizations name different artifacts or targets".into(),
                );
            }
            let Some(plan) = self.plan_by_report_fingerprint(row.provider_plan_report_identity)
            else {
                return Err("selected family realization names an unselected plan".into());
            };
            if plan.identity_digest() != row.provider_plan_digest
                || plan.target != row.target
                || plan
                    .rows
                    .iter()
                    .filter(|candidate| {
                        Some(plan.identities.text(candidate.requirement_identity))
                            == self.identities.text(row.selected_requirement_identity)
                    })
                    .count()
                    != 1
            {
                return Err(
                    "selected family realization disagrees with its exact plan, target, or row"
                        .into(),
                );
            }
            // A direct row or closed requirement must never be shadowed by
            // a family alias which would redirect it to unrelated authority.
            for alias in [&row.head_identity, &row.member_identity] {
                if self.plans.iter().any(|candidate| {
                    candidate.rows.iter().any(|candidate_row| {
                        Some(
                            candidate
                                .identities
                                .text(candidate_row.requirement_identity),
                        ) == self.identities.text(*alias)
                            && (candidate.report_fingerprint() != row.provider_plan_report_identity
                                || Some(
                                    candidate
                                        .identities
                                        .text(candidate_row.requirement_identity),
                                ) != self.identities.text(row.selected_requirement_identity))
                    })
                }) || self
                    .external_requirement_applications
                    .iter()
                    .any(|application| {
                        application.requirement_identity == *alias
                            && (application.provider_plan_report_identity
                                != row.provider_plan_report_identity
                                || application.selected_requirement_identity
                                    != row.selected_requirement_identity)
                    })
                    || self
                        .specialized_requirement_rows
                        .iter()
                        .any(|specialization| {
                            specialization.requirement_identity == *alias
                                && (specialization.provider_plan_report_identity
                                    != row.provider_plan_report_identity
                                    || specialization.selected_requirement_identity
                                        != row.selected_requirement_identity)
                        })
                {
                    return Err(
                        "selected family alias shadows a different selected requirement".into(),
                    );
                }
            }
        }
        Ok(())
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
            .filter(move |row| {
                self.identities.text(row.selected_requirement_identity)
                    == Some(requirement_identity)
            })
            .map(|row| {
                self.identities
                    .text(row.requirement_identity)
                    .unwrap_or_default()
            })
            .chain(
                self.external_requirement_applications
                    .iter()
                    .filter(|row| {
                        self.identities.text(row.selected_requirement_identity)
                            == Some(requirement_identity)
                    })
                    .filter_map(|row| self.identities.text(row.requirement_identity)),
            )
            .collect()
    }

    /// Requirement lookup for identities realized by exactly one selected
    /// plan. A shared requirement identity — distinct root traits may inherit
    /// the same requirement, so several plans can each carry its row — is
    /// ambiguous here and yields `None`; scoped callers use
    /// [`Self::installation_reach_resolution_for_plan`].
    pub(crate) fn installation_reach_resolution(
        &self,
        requirement_identity: &str,
    ) -> Option<&InstallationReachResolution> {
        if let Some(row) = self.family_realization_row(requirement_identity) {
            return self.installation_reach_resolution_for_plan(
                row.provider_plan_report_identity,
                requirement_identity,
            );
        }
        let requirement_identity = self.selected_requirement_identity(requirement_identity);
        let mut matches = self
            .installation_reach_resolutions
            .iter()
            .filter(|resolution| {
                self.identities.text(resolution.requirement_identity) == Some(requirement_identity)
            });
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
    pub(crate) fn installation_reach_resolution_for_plan(
        &self,
        provider_plan_report_identity: u64,
        requirement_identity: &str,
    ) -> Option<&InstallationReachResolution> {
        if self
            .family_realization_row(requirement_identity)
            .is_some_and(|row| row.provider_plan_report_identity != provider_plan_report_identity)
        {
            return None;
        }
        let requirement_identity = self.selected_requirement_identity(requirement_identity);
        self.installation_reach_resolutions
            .iter()
            .find(|resolution| {
                resolution.provider_plan_report_identity == provider_plan_report_identity
                    && self.identities.text(resolution.requirement_identity)
                        == Some(requirement_identity)
            })
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
                requirement_identity,
            ) {
                resolved.extend(row.resolved_row.iter().cloned());
                continue;
            }
            if let Some(row) = self.installation_reach_resolution(requirement_identity) {
                resolved.extend(row.resolved_row.iter().cloned());
                continue;
            }
            if self
                .installation_reach_resolutions
                .iter()
                .any(|resolution| {
                    self.identities.text(resolution.requirement_identity) == Some(selected_identity)
                })
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallationReachResolution {
    /// Index into the owning closure's `identities` table.
    pub requirement_identity: terminal_psi::TerminalIdentityIndex,
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
    /// generic requirement template, as an index into the owning closure's
    /// `identities` table.
    pub selected_requirement_identity: terminal_psi::TerminalIdentityIndex,
    /// The exact provider machine identity the selected row's checked-adapter
    /// binding names: the authored generic provider template. A leaf row
    /// (import, intrinsic, system call) has no provider machine and names the
    /// requirement template itself.
    pub selected_machine_identity: terminal_psi::TerminalIdentityIndex,
    /// The closed requirement identity Terminal boundary declarations carry.
    pub requirement_identity: terminal_psi::TerminalIdentityIndex,
    /// The specialized provider instance identity Terminal provider
    /// candidates carry as `candidate_identity`; the requirement template
    /// for a leaf row, which serves every closed application unchanged.
    pub machine_identity: terminal_psi::TerminalIdentityIndex,
}

/// A provider row's exact target-family execution coordinates. The selected
/// requirement is the authority address; head/member identify the original
/// callable contract and selected realization in one canonical artifact.
/// These records do not grant execution or establish a checked conformance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedTargetFamilyRealization {
    pub terminal: terminal_psi::TerminalPsiIdentity,
    pub target: String,
    pub head: semantic_vocabulary::BoundaryMachineId,
    pub member: semantic_vocabulary::BoundaryMachineId,
    /// Indices into the owning closure's `identities` table.
    pub head_identity: terminal_psi::TerminalIdentityIndex,
    pub member_identity: terminal_psi::TerminalIdentityIndex,
    pub selected_requirement_identity: terminal_psi::TerminalIdentityIndex,
    pub provider_plan_report_identity: u64,
    pub provider_plan_digest: crate::provider_plan::ProviderPlanDigest,
}

fn selected_plans_report_fingerprint(plans: &[ProviderPlan]) -> u64 {
    let mut hash = crate::fnv1a::Fnv1a::new();
    hash.u64(plans.len() as u64);
    for plan in plans {
        hash.u64(plan.report_fingerprint());
    }
    hash.finish()
}

fn selected_closure_report_fingerprint(
    plans: &[ProviderPlan],
    resolutions: &[InstallationReachResolution],
    specialization_rows: &[SelectedRequirementSpecialization],
    family_rows: &[SelectedTargetFamilyRealization],
    external_applications: &[SelectedExternalRequirementApplication],
    identities: &terminal_psi::TerminalIdentityTable,
) -> u64 {
    let mut hash = crate::fnv1a::Fnv1a::resume(selected_plans_report_fingerprint(plans));
    for resolution in resolutions {
        hash.bytes(
            identities
                .text(resolution.requirement_identity)
                .unwrap_or_default()
                .as_bytes(),
        );
        hash.u64(resolution.provider_plan_report_identity);
        hash.u64(resolution.upper_bound.len() as u64);
        for service in &resolution.upper_bound {
            hash.bytes_terminated(service.as_bytes(), 0);
        }
        hash.u64(resolution.resolved_row.len() as u64);
        for service in &resolution.resolved_row {
            hash.bytes_terminated(service.as_bytes(), 0);
        }
    }
    for row in specialization_rows {
        hash.u64(row.provider_plan_report_identity);
        hash.bytes_terminated(
            identities
                .text(row.selected_requirement_identity)
                .unwrap_or_default()
                .as_bytes(),
            0,
        );
        hash.bytes_terminated(
            identities
                .text(row.selected_machine_identity)
                .unwrap_or_default()
                .as_bytes(),
            0,
        );
        hash.bytes_terminated(
            identities
                .text(row.requirement_identity)
                .unwrap_or_default()
                .as_bytes(),
            0,
        );
        hash.bytes_terminated(
            identities
                .text(row.machine_identity)
                .unwrap_or_default()
                .as_bytes(),
            0,
        );
    }
    for row in external_applications {
        hash.u64(row.provider_plan_report_identity);
        hash.bytes(row.provider_plan_digest.as_bytes());
        hash.bytes_terminated(
            identities
                .text(row.selected_requirement_identity)
                .unwrap_or_default()
                .as_bytes(),
            0,
        );
        hash.bytes_terminated(
            identities
                .text(row.requirement_identity)
                .unwrap_or_default()
                .as_bytes(),
            0,
        );
    }
    for row in family_rows {
        hash.u64(u64::from(row.terminal.vocabulary_marker.get()));
        hash.bytes(row.terminal.program_fingerprint.as_bytes());
        hash.bytes_terminated(row.target.as_bytes(), 0);
        hash.u64(row.head.get());
        hash.u64(row.member.get());
        hash.bytes_terminated(
            identities
                .text(row.head_identity)
                .unwrap_or_default()
                .as_bytes(),
            0,
        );
        hash.bytes_terminated(
            identities
                .text(row.member_identity)
                .unwrap_or_default()
                .as_bytes(),
            0,
        );
        hash.bytes_terminated(
            identities
                .text(row.selected_requirement_identity)
                .unwrap_or_default()
                .as_bytes(),
            0,
        );
        hash.u64(row.provider_plan_report_identity);
        hash.bytes(row.provider_plan_digest.as_bytes());
    }
    hash.finish()
}

struct SelectedProviderClosureDigestEncoder(DomainDigestEncoder);

impl crate::provider_evidence::providers::provider_plan::DigestFeed
    for SelectedProviderClosureDigestEncoder
{
    fn raw(&mut self, bytes: &[u8]) {
        self.0.raw(bytes);
    }
}

impl SelectedProviderClosureDigestEncoder {
    fn new() -> Self {
        // v3 adds the separate closed Import application roster to this grammar.
        Self(DomainDigestEncoder::new(
            b"omega.selected-provider-closure.sha256.v3\0",
        ))
    }

    fn finish(self) -> SelectedProviderClosureDigest {
        SelectedProviderClosureDigest::from_digest(self.0.finish())
    }

    fn byte(&mut self, value: u8) {
        self.0.byte(value);
    }

    fn len(&mut self, value: usize) {
        self.0.len(value);
    }

    fn u64(&mut self, value: u64) {
        self.0.u64(value);
    }

    fn i64(&mut self, value: i64) {
        self.0.i64(value);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.0.bytes(bytes);
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
        self.string(&admission.requirement);
        self.opaque_binding(&admission.binding);
        self.string(&admission.executable);
        self.string(&admission.implementation_evidence);
        self.execution_scope(admission.execution_scope);
        self.len(admission.containment.len());
        for evidence in &admission.containment {
            self.byte(match evidence.guarantee {
                crate::ContainmentGuarantee::MemoryIsolation => 0,
                crate::ContainmentGuarantee::ForcibleTermination => 1,
                crate::ContainmentGuarantee::FaultContainment => 2,
                crate::ContainmentGuarantee::BoundedResources => 3,
            });
            self.string(&evidence.evidence);
        }
        self.optional_string(admission.executable_closure_evidence.as_deref());
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
                        self.bytes(&library);
                        self.bytes(&export);
                    }
                    crate::ForeignLocatorCandidate::PeByOrdinal { library, ordinal } => {
                        self.byte(1);
                        self.bytes(&library);
                        self.0.u16(*ordinal);
                    }
                    crate::ForeignLocatorCandidate::ElfVersioned {
                        object,
                        symbol,
                        version,
                    } => {
                        self.byte(2);
                        self.bytes(&object);
                        self.bytes(&symbol);
                        self.bytes(&version);
                    }
                    crate::ForeignLocatorCandidate::MachODylibSymbol {
                        install_name,
                        symbol,
                    } => {
                        self.byte(3);
                        self.bytes(install_name);
                        self.bytes(&symbol);
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
