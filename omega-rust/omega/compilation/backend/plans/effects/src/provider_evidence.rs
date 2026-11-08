/*
We retain the selected provider closure (`SelectedProviderPlanFacts`), its
domain-separated SHA-256 digest (`SelectedProviderClosureDigest`), and the
compiler-private recovery candidate (`SelectedProviderPlanCandidate`). When
plans enter the closure, we validate unique plan identities and boundary slots
and intern requirement identities into a closure-local `TerminalIdentityTable`
before attaching installation reach, template specialization rows, closed
external requirement applications, or target-family realization coordinates.
*/

pub(crate) mod authority;
pub(crate) mod component_eras;
pub(crate) use optimization_core::digest_encoder;
pub(crate) mod executable_scopes;
pub(crate) mod providers;

use crate::provider_evidence::digest_encoder::DomainDigestEncoder;
use crate::provider_evidence::providers::provider_plan::DigestFeed;
use crate::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanDigest};
use std::collections::BTreeSet;
use terminal_psi::{TerminalIdentityIndex, TerminalIdentityTable};

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

evidence_digest!(SelectedProviderClosureDigest);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssumedBindingContractSelection {
    pub required: String,
    pub supplied: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedExternalRequirementApplication {
    pub provider_plan_report_identity: u64,
    pub provider_plan_digest: ProviderPlanDigest,
    pub selected_requirement_identity: TerminalIdentityIndex,
    pub requirement_identity: TerminalIdentityIndex,
}

#[derive(Clone, PartialEq, Eq)]
pub struct SelectedProviderPlanCandidate {
    pub plans: Vec<ProviderPlan>,
    pub assumed_bindings: Vec<AssumedBindingContractSelection>,
    pub report_fingerprint: u64,
    pub installation_reach_resolutions: Vec<InstallationReachResolution>,
    pub specialized_requirement_rows: Vec<SelectedRequirementSpecialization>,
    pub external_requirement_applications: Vec<SelectedExternalRequirementApplication>,
    pub family_realization_rows: Vec<SelectedTargetFamilyRealization>,
    pub toolchain_settled_identities: BTreeSet<u64>,
    pub identities: terminal_psi::TerminalIdentityTable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderPlanFacts {
    plans: Vec<ProviderPlan>,
    assumed_bindings: Vec<AssumedBindingContractSelection>,
    report_fingerprint: u64,
    installation_reach_resolutions: Vec<InstallationReachResolution>,
    specialized_requirement_rows: Vec<SelectedRequirementSpecialization>,
    external_requirement_applications: Vec<SelectedExternalRequirementApplication>,
    family_realization_rows: Vec<SelectedTargetFamilyRealization>,
    toolchain_settled_identities: BTreeSet<u64>,
    identities: terminal_psi::TerminalIdentityTable,
}

impl Default for SelectedProviderPlanFacts {
    fn default() -> Self {
        Self {
            plans: Vec::new(),
            assumed_bindings: Vec::new(),
            report_fingerprint: selected_plans_report_fingerprint(&[]),
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
    pub fn recovery_candidate(&self) -> SelectedProviderPlanCandidate {
        let Self {
            plans,
            assumed_bindings,
            report_fingerprint,
            installation_reach_resolutions,
            specialized_requirement_rows,
            external_requirement_applications,
            family_realization_rows,
            toolchain_settled_identities,
            identities,
        } = self;
        SelectedProviderPlanCandidate {
            plans: plans.clone(),
            assumed_bindings: assumed_bindings.clone(),
            report_fingerprint: *report_fingerprint,
            installation_reach_resolutions: installation_reach_resolutions.clone(),
            specialized_requirement_rows: specialized_requirement_rows.clone(),
            external_requirement_applications: external_requirement_applications.clone(),
            family_realization_rows: family_realization_rows.clone(),
            toolchain_settled_identities: toolchain_settled_identities.clone(),
            identities: identities.clone(),
        }
    }

    pub fn with_assumed_bindings(mut self, rows: Vec<AssumedBindingContractSelection>) -> Self {
        self.assumed_bindings = rows;
        self
    }

    pub fn assumed_supplier_identity<'a>(
        &'a self,
        requirement: &'a str,
    ) -> Result<&'a str, String> {
        let mut current = requirement;
        for _ in 0..=self.assumed_bindings.len() {
            if let Some(row) = self
                .assumed_bindings
                .iter()
                .find(|row| row.required == current)
            {
                current = &row.supplied;
            } else {
                return Ok(current);
            }
        }
        Err("cyclic assumed binding supply".to_owned())
    }

    pub fn assumed_bindings(&self) -> &[AssumedBindingContractSelection] {
        &self.assumed_bindings
    }

    pub fn assumed_binding_coordinates(
        &self,
        module: &terminal_psi::TerminalModule,
    ) -> Result<
        Vec<(
            semantic_vocabulary::BoundaryMachineId,
            semantic_vocabulary::BoundaryMachineId,
        )>,
        String,
    > {
        let find = |identity: &str| {
            let mut rows = module
                .boundary_machines
                .iter()
                .filter(|row| module.identities.text(row.identity) == Some(identity));
            let row = rows
                .next()
                .ok_or("assumed binding requirement absent from canonical artifact")?;
            if rows.next().is_some() {
                return Err("assumed binding requirement is ambiguous");
            }
            Ok(row.id)
        };
        self.assumed_bindings
            .iter()
            .map(|row| Ok((find(&row.required)?, find(&row.supplied)?)))
            .collect()
    }

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
            let slot = plan.schema.selection_slot_identity();
            if !boundary_slots.insert(slot) {
                return Err(format!(
                    "boundary slot `{}` has more than one selected provider plan",
                    plan.schema
                        .identity_text(plan.schema.declaration.trait_name)
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
            assumed_bindings: Vec::new(),
            report_fingerprint,
            installation_reach_resolutions: Vec::new(),
            specialized_requirement_rows: Vec::new(),
            external_requirement_applications: Vec::new(),
            family_realization_rows: Vec::new(),
            toolchain_settled_identities: BTreeSet::new(),
            identities: requirement_identities.build(),
        })
    }

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
            existing.schema.selection_slot_identity() == plan.schema.selection_slot_identity()
        }) {
            return Err(format!(
                "boundary slot `{}` has more than one selected provider plan",
                plan.schema
                    .identity_text(plan.schema.declaration.trait_name)
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
        self.refresh_report_fingerprint();
        Ok(self)
    }

    pub fn is_toolchain_settled(&self, plan: &ProviderPlan) -> bool {
        self.toolchain_settled_identities
            .contains(&plan.report_fingerprint())
            && self.plans.iter().any(|member| member == plan)
    }

    pub fn plans(&self) -> &[ProviderPlan] {
        &self.plans
    }

    pub fn with_direct_boundary_plans(mut self, plans: Vec<ProviderPlan>) -> Result<Self, String> {
        let checked = Self::from_selected_plans(plans)?;
        for plan in &checked.plans {
            if self.plans.iter().any(|existing| {
                existing.report_fingerprint() == plan.report_fingerprint()
                    || (existing.schema.declaration.trait_package_identity
                        == plan.schema.declaration.trait_package_identity
                        && existing
                            .schema
                            .identity_text(existing.schema.declaration.trait_name)
                            == plan
                                .schema
                                .identity_text(plan.schema.declaration.trait_name))
            }) {
                return Err(format!(
                    "direct boundary callable `{}` collides with an existing selected plan",
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
        self.refresh_report_fingerprint();
        Ok(self)
    }

    pub(crate) fn plan_by_report_fingerprint(
        &self,
        report_fingerprint: u64,
    ) -> Option<&ProviderPlan> {
        self.plans
            .iter()
            .find(|plan| plan.report_fingerprint() == report_fingerprint)
    }

    pub fn plan_by_exact_evidence(
        &self,
        report_identity: u64,
        exact_plan: &ProviderPlan,
    ) -> Option<&ProviderPlan> {
        let selected = self.plans.iter().find(|plan| *plan == exact_plan)?;
        (selected.report_fingerprint() == report_identity).then_some(selected)
    }

    pub const fn report_fingerprint(&self) -> u64 {
        self.report_fingerprint
    }

    fn refresh_report_fingerprint(&mut self) {
        self.report_fingerprint = selected_closure_report_fingerprint(
            &self.plans,
            &self.installation_reach_resolutions,
            &self.specialized_requirement_rows,
            &self.family_realization_rows,
            &self.external_requirement_applications,
            &self.identities,
        );
    }

    pub const fn compatibility_report_identity(&self) -> u64 {
        self.report_fingerprint
    }

    pub fn identity_digest(&self) -> SelectedProviderClosureDigest {
        let mut encoder = SelectedProviderClosureDigestEncoder::new();
        encoder.len(self.plans.len());
        for plan in &self.plans {
            encoder.bytes(plan.identity_digest().as_bytes());
        }
        encoder.len(self.assumed_bindings.len());
        for row in &self.assumed_bindings {
            encoder.string(&row.required);
            encoder.string(&row.supplied);
        }
        encoder.execution_scope(self.execution_scope());
        encoder.len(0);

        encoder.len(self.installation_reach_resolutions.len());
        for resolution in &self.installation_reach_resolutions {
            encoder.string(self.identity_text(resolution.requirement_identity));
            encoder.u64(resolution.provider_plan_report_identity);
            encoder.strings(&resolution.upper_bound);
            encoder.strings(&resolution.resolved_row);
        }
        encoder.len(self.specialized_requirement_rows.len());
        for row in &self.specialized_requirement_rows {
            encoder.u64(row.provider_plan_report_identity);
            encoder.string(self.identity_text(row.selected_requirement_identity));
            encoder.string(self.identity_text(row.selected_machine_identity));
            encoder.string(self.identity_text(row.requirement_identity));
            encoder.string(self.identity_text(row.machine_identity));
        }
        encoder.len(self.external_requirement_applications.len());
        for row in &self.external_requirement_applications {
            encoder.u64(row.provider_plan_report_identity);
            encoder.bytes(row.provider_plan_digest.as_bytes());
            encoder.string(self.identity_text(row.selected_requirement_identity));
            encoder.string(self.identity_text(row.requirement_identity));
        }
        encoder.len(self.family_realization_rows.len());
        for row in &self.family_realization_rows {
            encoder.u64(u64::from(row.terminal.vocabulary_marker.get()));
            encoder.bytes(row.terminal.program_fingerprint.as_bytes());
            encoder.string(&row.target);
            encoder.u64(row.head.get());
            encoder.u64(row.member.get());
            encoder.string(self.identity_text(row.head_identity));
            encoder.string(self.identity_text(row.member_identity));
            encoder.string(self.identity_text(row.selected_requirement_identity));
            encoder.u64(row.provider_plan_report_identity);
            encoder.bytes(row.provider_plan_digest.as_bytes());
        }
        encoder.finish()
    }

    pub const fn execution_scope(&self) -> crate::ExecutionScope {
        crate::ExecutionScope::CallerAddressSpace
    }

    pub const fn identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.identities
    }

    pub fn identity_text(&self, identity: terminal_psi::TerminalIdentityIndex) -> &str {
        identity_text(&self.identities, identity)
    }

    pub fn is_empty(&self) -> bool {
        self.plans.is_empty()
    }

    pub fn with_installation_reach_resolutions(
        mut self,
        mut resolutions: Vec<InstallationReachResolution>,
        resolution_identities: &terminal_psi::TerminalIdentityTable,
    ) -> Result<Self, String> {
        let mut identities = self.identities.clone().into_builder();
        for resolution in &mut resolutions {
            resolution.requirement_identity = identities.intern(identity_text(
                resolution_identities,
                resolution.requirement_identity,
            ));
        }
        self.identities = identities.build();
        resolutions.sort_by_cached_key(|resolution| {
            (
                self.identity_text(resolution.requirement_identity)
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
                    self.identity_text(pair[0].requirement_identity),
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
                    self.identity_text(resolution.requirement_identity)
                ));
            }
            let Some(plan) =
                self.plan_by_report_fingerprint(resolution.provider_plan_report_identity)
            else {
                return Err(format!(
                    "installation reach resolution for `{}` names unselected provider plan {:#018x}",
                    self.identity_text(resolution.requirement_identity),
                    resolution.provider_plan_report_identity
                ));
            };
            if !plan.rows.iter().any(|row| {
                Some(plan.identities.text(row.requirement_identity))
                    == self.identities.text(resolution.requirement_identity)
            }) {
                return Err(format!(
                    "installation reach resolution for `{}` is absent from selected provider plan `{}`",
                    self.identity_text(resolution.requirement_identity),
                    plan.name
                ));
            }
        }
        self.installation_reach_resolutions = resolutions;
        self.refresh_report_fingerprint();
        Ok(self)
    }

    pub fn installation_reach_resolutions(&self) -> &[InstallationReachResolution] {
        &self.installation_reach_resolutions
    }

    pub fn with_specialized_requirement_rows(
        mut self,
        mut rows: Vec<SelectedRequirementSpecialization>,
        row_identities: &terminal_psi::TerminalIdentityTable,
    ) -> Result<Self, String> {
        let mut identities = self.identities.clone().into_builder();
        for row in &mut rows {
            row.selected_requirement_identity = identities.intern(identity_text(
                row_identities,
                row.selected_requirement_identity,
            ));
            row.selected_machine_identity =
                identities.intern(identity_text(row_identities, row.selected_machine_identity));
            row.requirement_identity =
                identities.intern(identity_text(row_identities, row.requirement_identity));
            row.machine_identity =
                identities.intern(identity_text(row_identities, row.machine_identity));
        }
        self.identities = identities.build();
        rows.sort_by_cached_key(|row| {
            (
                self.identity_text(row.requirement_identity).to_owned(),
                row.provider_plan_report_identity,
                self.identity_text(row.machine_identity).to_owned(),
            )
        });
        for pair in rows.windows(2) {
            if pair[0].requirement_identity == pair[1].requirement_identity {
                return Err(format!(
                    "closed requirement `{}` has more than one selected specialization row",
                    self.identity_text(pair[0].requirement_identity)
                ));
            }
        }
        for row in &rows {
            if self
                .external_requirement_application(self.identity_text(row.requirement_identity))
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
                    self.identity_text(row.requirement_identity),
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
                    self.identity_text(row.requirement_identity),
                    plan.name
                ));
            }
        }
        self.specialized_requirement_rows = rows;
        self.validate_family_realization_rows(&self.family_realization_rows)?;
        self.refresh_report_fingerprint();
        Ok(self)
    }

    pub fn specialized_requirement_rows(&self) -> &[SelectedRequirementSpecialization] {
        &self.specialized_requirement_rows
    }

    pub fn without_checked_specializations(mut self) -> Self {
        self.specialized_requirement_rows.retain(|specialization| {
            !self.plans.iter().any(|plan| {
                plan.report_fingerprint() == specialization.provider_plan_report_identity
                    && plan.rows.iter().any(|row| {
                        matches!(
                            row.binding,
                            crate::provider_plan::ProviderBinding::CheckedAdapter { .. }
                        ) && Some(plan.identities.text(row.requirement_identity))
                            == self
                                .identities
                                .text(specialization.selected_requirement_identity)
                    })
            })
        });
        self.refresh_report_fingerprint();
        self
    }

    pub fn specialized_requirement_row(
        &self,
        requirement_identity: &str,
    ) -> Option<&SelectedRequirementSpecialization> {
        let index = self.identities.index_of(requirement_identity)?;
        self.specialized_requirement_rows
            .iter()
            .find(|row| row.requirement_identity == index)
    }

    pub fn specialized_requirement_rows_for_row<'a>(
        &'a self,
        provider_plan_report_identity: u64,
        selected_requirement_identity: &'a str,
    ) -> impl Iterator<Item = &'a SelectedRequirementSpecialization> + 'a {
        let selected_index = self.identities.index_of(selected_requirement_identity);
        self.specialized_requirement_rows.iter().filter(move |row| {
            row.provider_plan_report_identity == provider_plan_report_identity
                && Some(row.selected_requirement_identity) == selected_index
        })
    }

    pub fn selected_row_requirement_identities<'a>(
        &'a self,
        provider_plan_report_identity: u64,
        row_requirement_identity: &'a str,
    ) -> Vec<&'a str> {
        let row_index = self.identities.index_of(row_requirement_identity);
        let mut identities = self
            .specialized_requirement_rows_for_row(
                provider_plan_report_identity,
                row_requirement_identity,
            )
            .map(|row| self.identity_text(row.requirement_identity))
            .collect::<Vec<_>>();
        identities.extend(
            self.external_requirement_applications
                .iter()
                .filter(|row| {
                    row.provider_plan_report_identity == provider_plan_report_identity
                        && Some(row.selected_requirement_identity) == row_index
                })
                .filter_map(|row| self.identities.text(row.requirement_identity)),
        );
        if identities.is_empty() {
            identities.push(row_requirement_identity);
        }
        for row in &self.family_realization_rows {
            if row.provider_plan_report_identity == provider_plan_report_identity
                && Some(row.selected_requirement_identity) == row_index
            {
                identities.extend([
                    self.identity_text(row.head_identity),
                    self.identity_text(row.member_identity),
                ]);
            }
        }
        identities.sort_unstable();
        identities.dedup();
        identities
    }

    pub fn selected_requirement_identity<'a>(&'a self, requirement: &'a str) -> &'a str {
        if let Some(row) = self.family_realization_row(requirement) {
            return self.identity_text(row.selected_requirement_identity);
        }
        if let Some(row) = self.external_requirement_application(requirement) {
            return self.identity_text(row.selected_requirement_identity);
        }
        self.specialized_requirement_row(requirement)
            .map_or(requirement, |row| {
                self.identity_text(row.selected_requirement_identity)
            })
    }

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

    pub fn selected_requirement_row(
        &self,
        requirement: &str,
    ) -> Option<(&ProviderPlan, &crate::provider_plan::ProviderPlanRow)> {
        let (target_requirement, target_plan_fingerprint) =
            if let Some(row) = self.family_realization_row(requirement) {
                (
                    self.identities.text(row.selected_requirement_identity)?,
                    Some(row.provider_plan_report_identity),
                )
            } else if let Some(row) = self.specialized_requirement_row(requirement) {
                (
                    self.identities.text(row.selected_requirement_identity)?,
                    Some(row.provider_plan_report_identity),
                )
            } else if let Some(row) = self.external_requirement_application(requirement) {
                (
                    self.identities.text(row.selected_requirement_identity)?,
                    Some(row.provider_plan_report_identity),
                )
            } else {
                (requirement, None)
            };
        self.plans.iter().find_map(|plan| {
            if let Some(expected_fp) = target_plan_fingerprint
                && plan.report_fingerprint() != expected_fp
            {
                return None;
            }
            let target_id = plan.identities.id_at(target_requirement)?;
            plan.rows
                .iter()
                .find(|row| row.requirement_identity == target_id)
                .map(|row| (plan, row))
        })
    }

    pub fn external_requirement_applications(&self) -> &[SelectedExternalRequirementApplication] {
        &self.external_requirement_applications
    }

    pub fn external_requirement_application(
        &self,
        requirement: &str,
    ) -> Option<&SelectedExternalRequirementApplication> {
        let index = self.identities.index_of(requirement)?;
        self.external_requirement_applications
            .iter()
            .find(|row| row.requirement_identity == index)
    }

    pub fn with_external_requirement_applications(
        mut self,
        mut rows: Vec<SelectedExternalRequirementApplication>,
        row_identities: &TerminalIdentityTable,
    ) -> Result<Self, String> {
        let mut identities = self.identities.clone().into_builder();
        for row in &mut rows {
            row.selected_requirement_identity = identities.intern(identity_text(
                row_identities,
                row.selected_requirement_identity,
            ));
            row.requirement_identity =
                identities.intern(identity_text(row_identities, row.requirement_identity));
        }
        self.identities = identities.build();
        rows.sort_by_cached_key(|row| self.identity_text(row.requirement_identity).to_owned());
        for pair in rows.windows(2) {
            if pair[0].requirement_identity == pair[1].requirement_identity {
                return Err(
                    "closed external requirement has multiple selected applications".into(),
                );
            }
        }
        for row in &rows {
            if row.selected_requirement_identity.is_absent()
                || row.requirement_identity.is_absent()
                || row.selected_requirement_identity == row.requirement_identity
            {
                return Err(
                    "closed external requirement lost its distinct source identities".into(),
                );
            }
            let plan = self
                .plan_by_report_fingerprint(row.provider_plan_report_identity)
                .ok_or("closed external requirement names an unselected plan")?;
            if plan.identity_digest() != row.provider_plan_digest {
                return Err("closed external requirement substituted its selected plan".into());
            }
            let mut matching = plan.rows.iter().filter(|selected| {
                Some(plan.identities.text(selected.requirement_identity))
                    == self.identities.text(row.selected_requirement_identity)
            });
            let selected = matching
                .next()
                .ok_or("closed external requirement lost its selected row")?;
            if matching.next().is_some()
                || !matches!(selected.binding, ProviderBinding::Import { .. })
            {
                return Err(
                    "closed external requirement requires exactly one selected Import row".into(),
                );
            }
            let requirement = self.identity_text(row.requirement_identity);
            if self.specialized_requirement_row(requirement).is_some()
                || self.family_realization_row(requirement).is_some()
                || self.plans.iter().any(|candidate| {
                    candidate.rows.iter().any(|selected| {
                        candidate.identities.text(selected.requirement_identity) == requirement
                    })
                })
            {
                return Err(
                    "closed external requirement shadows another selected coordinate".into(),
                );
            }
        }
        self.external_requirement_applications = rows;
        self.refresh_report_fingerprint();
        Ok(self)
    }

    pub fn with_family_realization_rows(
        mut self,
        mut rows: Vec<SelectedTargetFamilyRealization>,
        row_identities: &terminal_psi::TerminalIdentityTable,
    ) -> Result<Self, String> {
        let mut identities = self.identities.clone().into_builder();
        for row in &mut rows {
            row.head_identity = identities.intern(identity_text(row_identities, row.head_identity));
            row.member_identity =
                identities.intern(identity_text(row_identities, row.member_identity));
            row.selected_requirement_identity = identities.intern(identity_text(
                row_identities,
                row.selected_requirement_identity,
            ));
        }
        self.identities = identities.build();
        rows.sort_by_cached_key(|row| self.identity_text(row.head_identity).to_owned());
        self.validate_family_realization_rows(&rows)?;
        self.family_realization_rows = rows;
        self.refresh_report_fingerprint();
        Ok(self)
    }

    pub fn family_realization_rows(&self) -> &[SelectedTargetFamilyRealization] {
        &self.family_realization_rows
    }

    pub fn family_realization_row(
        &self,
        identity: &str,
    ) -> Option<&SelectedTargetFamilyRealization> {
        let index = self.identities.index_of(identity)?;
        self.family_realization_rows
            .iter()
            .find(|row| row.head_identity == index || row.member_identity == index)
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
                || !identities.insert(self.identity_text(row.head_identity).to_owned())
                || !identities.insert(self.identity_text(row.member_identity).to_owned())
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
        let req_index = self.identities.index_of(requirement_identity)?;
        let mut matches = self
            .installation_reach_resolutions
            .iter()
            .filter(|resolution| resolution.requirement_identity == req_index);
        let resolution = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        Some(resolution)
    }

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
        let req_index = self.identities.index_of(requirement_identity)?;
        self.installation_reach_resolutions
            .iter()
            .find(|resolution| {
                resolution.provider_plan_report_identity == provider_plan_report_identity
                    && resolution.requirement_identity == req_index
            })
    }

    pub fn resolve_installation_reach_for_plan(
        &self,
        concrete_reach: &[String],
        requirement_identities: &[String],
        provider_plan_report_identity: u64,
    ) -> Result<Vec<String>, String> {
        let mut resolved = concrete_reach.to_vec();
        for requirement_identity in requirement_identities {
            let row = self.resolve_installation_requirement(
                requirement_identity,
                provider_plan_report_identity,
            )?;
            resolved.extend(row.resolved_row.iter().cloned());
        }
        resolved.sort();
        resolved.dedup();
        Ok(resolved)
    }

    pub fn resolve_installation_requirement(
        &self,
        requirement_identity: &str,
        provider_plan_report_identity: u64,
    ) -> Result<&InstallationReachResolution, String> {
        if let Some(row) = self.installation_reach_resolution_for_plan(
            provider_plan_report_identity,
            requirement_identity,
        ) {
            return Ok(row);
        }
        if let Some(row) = self.installation_reach_resolution(requirement_identity) {
            return Ok(row);
        }
        let selected_identity = self.selected_requirement_identity(requirement_identity);
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
        Err(format!(
            "installation reach requirement `{requirement_identity}` remains unresolved at final admission"
        ))
    }
}

impl SelectedProviderPlanCandidate {
    pub fn validate(self) -> Result<SelectedProviderPlanFacts, String> {
        let mut ordinary = Vec::new();
        let mut toolchain = Vec::new();
        for plan in &self.plans {
            if self
                .toolchain_settled_identities
                .contains(&plan.report_fingerprint())
            {
                toolchain.push(plan.clone());
            } else {
                ordinary.push(plan.clone());
            }
        }
        let mut recovered = SelectedProviderPlanFacts::from_selected_plans(ordinary)?;
        for plan in toolchain {
            recovered = recovered.with_toolchain_settled_plan(plan)?;
        }
        if recovered.plans != self.plans
            || recovered.toolchain_settled_identities != self.toolchain_settled_identities
            || recovered
                .identities
                .entries()
                .iter()
                .any(|identity| self.identities.index_of(identity).is_none())
        {
            return Err(
                "recovered selected provider roster changed order, membership or identity custody"
                    .to_owned(),
            );
        }
        recovered.identities = self.identities.clone();
        recovered = recovered.with_assumed_bindings(self.assumed_bindings.clone());
        for row in &self.assumed_bindings {
            recovered.assumed_supplier_identity(&row.required)?;
        }
        recovered = recovered.with_installation_reach_resolutions(
            self.installation_reach_resolutions.clone(),
            &self.identities,
        )?;
        recovered = recovered.with_specialized_requirement_rows(
            self.specialized_requirement_rows.clone(),
            &self.identities,
        )?;
        recovered = recovered.with_external_requirement_applications(
            self.external_requirement_applications.clone(),
            &self.identities,
        )?;
        recovered = recovered
            .with_family_realization_rows(self.family_realization_rows.clone(), &self.identities)?;
        if recovered.recovery_candidate() != self {
            return Err(
                "recovered provider closure is not canonical or lost a retained field".to_owned(),
            );
        }
        Ok(recovered)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallationReachResolution {
    pub requirement_identity: terminal_psi::TerminalIdentityIndex,
    pub provider_plan_report_identity: u64,
    pub upper_bound: Vec<String>,
    pub resolved_row: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedRequirementSpecialization {
    pub provider_plan_report_identity: u64,
    pub selected_requirement_identity: terminal_psi::TerminalIdentityIndex,
    pub selected_machine_identity: terminal_psi::TerminalIdentityIndex,
    pub requirement_identity: terminal_psi::TerminalIdentityIndex,
    pub machine_identity: terminal_psi::TerminalIdentityIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedTargetFamilyRealization {
    pub terminal: terminal_psi::TerminalPsiIdentity,
    pub target: String,
    pub head: semantic_vocabulary::BoundaryMachineId,
    pub member: semantic_vocabulary::BoundaryMachineId,
    pub head_identity: terminal_psi::TerminalIdentityIndex,
    pub member_identity: terminal_psi::TerminalIdentityIndex,
    pub selected_requirement_identity: terminal_psi::TerminalIdentityIndex,
    pub provider_plan_report_identity: u64,
    pub provider_plan_digest: crate::provider_plan::ProviderPlanDigest,
}

pub(crate) fn identity_text(
    identities: &terminal_psi::TerminalIdentityTable,
    index: terminal_psi::TerminalIdentityIndex,
) -> &str {
    identities.text(index).unwrap_or_default()
}

fn selected_plans_report_fingerprint(plans: &[ProviderPlan]) -> u64 {
    let mut hash = semantic_vocabulary::Fnv1a::new();
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
    let mut hash = semantic_vocabulary::Fnv1a::resume(selected_plans_report_fingerprint(plans));
    for resolution in resolutions {
        hash.bytes(identity_text(identities, resolution.requirement_identity).as_bytes());
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
            identity_text(identities, row.selected_requirement_identity).as_bytes(),
            0,
        );
        hash.bytes_terminated(
            identity_text(identities, row.selected_machine_identity).as_bytes(),
            0,
        );
        hash.bytes_terminated(
            identity_text(identities, row.requirement_identity).as_bytes(),
            0,
        );
        hash.bytes_terminated(
            identity_text(identities, row.machine_identity).as_bytes(),
            0,
        );
    }
    for row in external_applications {
        hash.u64(row.provider_plan_report_identity);
        hash.bytes(row.provider_plan_digest.as_bytes());
        hash.bytes_terminated(
            identity_text(identities, row.selected_requirement_identity).as_bytes(),
            0,
        );
        hash.bytes_terminated(
            identity_text(identities, row.requirement_identity).as_bytes(),
            0,
        );
    }
    for row in family_rows {
        hash.u64(u64::from(row.terminal.vocabulary_marker.get()));
        hash.bytes(row.terminal.program_fingerprint.as_bytes());
        hash.bytes_terminated(row.target.as_bytes(), 0);
        hash.u64(row.head.get());
        hash.u64(row.member.get());
        hash.bytes_terminated(identity_text(identities, row.head_identity).as_bytes(), 0);
        hash.bytes_terminated(identity_text(identities, row.member_identity).as_bytes(), 0);
        hash.bytes_terminated(
            identity_text(identities, row.selected_requirement_identity).as_bytes(),
            0,
        );
        hash.u64(row.provider_plan_report_identity);
        hash.bytes(row.provider_plan_digest.as_bytes());
    }
    hash.finish()
}

struct SelectedProviderClosureDigestEncoder(DomainDigestEncoder);

impl DigestFeed for SelectedProviderClosureDigestEncoder {
    fn raw(&mut self, bytes: &[u8]) {
        self.0.raw(bytes);
    }
}

impl SelectedProviderClosureDigestEncoder {
    fn new() -> Self {
        Self(DomainDigestEncoder::new(
            b"omega.selected-provider-closure.sha256.v3\0",
        ))
    }

    fn finish(self) -> SelectedProviderClosureDigest {
        SelectedProviderClosureDigest::from_digest(self.0.finish())
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
}

#[cfg(test)]
mod installation_tests {
    use super::*;

    #[test]
    fn installation_dependency_selection_preserves_plan_and_family_custody() {
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let requirement = identities.intern("Requirement::visit");
        let head = identities.intern("Family::head");
        let member = identities.intern("Family::member");
        let mut selected = SelectedProviderPlanFacts {
            identities: identities.build(),
            installation_reach_resolutions: [1, 2]
                .map(|plan| InstallationReachResolution {
                    requirement_identity: requirement,
                    provider_plan_report_identity: plan,
                    upper_bound: vec!["X".into(), "Y".into()],
                    resolved_row: vec![if plan == 1 { "X" } else { "Y" }.into()],
                })
                .to_vec(),
            ..SelectedProviderPlanFacts::default()
        };
        assert_eq!(
            selected
                .resolve_installation_requirement("Requirement::visit", 1)
                .unwrap()
                .resolved_row,
            ["X"]
        );
        assert!(
            selected
                .resolve_installation_requirement("Requirement::visit", 3)
                .unwrap_err()
                .contains("more than one")
        );
        assert!(
            selected
                .resolve_installation_requirement("Missing", 1)
                .unwrap_err()
                .contains("unresolved")
        );
        selected
            .family_realization_rows
            .push(SelectedTargetFamilyRealization {
                terminal: terminal_psi::TerminalPsiIdentity {
                    vocabulary_marker: terminal_psi::VocabularyMarker,
                    program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([1; 32]),
                },
                target: "macos_arm64".into(),
                head: semantic_vocabulary::BoundaryMachineId::new(1).unwrap(),
                member: semantic_vocabulary::BoundaryMachineId::new(2).unwrap(),
                head_identity: head,
                member_identity: member,
                selected_requirement_identity: requirement,
                provider_plan_report_identity: 2,
                provider_plan_digest: crate::provider_plan::ProviderPlanDigest::from_digest(
                    [2; 32],
                ),
            });
        assert_eq!(
            selected
                .resolve_installation_requirement("Family::head", 1)
                .unwrap()
                .provider_plan_report_identity,
            2
        );
        assert_eq!(
            selected
                .resolve_installation_reach_for_plan(&[], &["Family::member".into()], 1)
                .unwrap(),
            ["Y"]
        );
        selected.installation_reach_resolutions.remove(0);
        assert_eq!(
            selected
                .resolve_installation_requirement("Requirement::visit", 3)
                .unwrap()
                .resolved_row,
            ["Y"]
        );
    }
}
