//! The closure of provider plans a selection actually reached, and its digest.

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
}

impl Default for SelectedProviderPlanFacts {
    fn default() -> Self {
        Self {
            plans: Vec::new(),
            report_fingerprint: selected_plans_report_fingerprint(&[]),
            execution_scope: crate::ExecutionScope::CallerAddressSpace,
            opaque_executable_admissions: Vec::new(),
            installation_reach_resolutions: Vec::new(),
        }
    }
}

impl SelectedProviderPlanFacts {
    /// Retain already-resolved provider plans without rejoining them through a
    /// readable plan name. Authored selection paths belong before this
    /// boundary; every plan here already carries its package-qualified slot,
    /// provider, requirement, and realization provenance.
    pub fn from_selected_plans(mut plans: Vec<ProviderPlan>) -> Result<Self, String> {
        plans.sort_by(|left, right| {
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
        });

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
        })
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
        let mut matches = self
            .plans
            .iter()
            .filter(|plan| plan.report_fingerprint() == report_identity);
        let selected = matches.next()?;
        if matches.next().is_some() || selected != exact_plan {
            return None;
        }
        Some(selected)
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
                crate::executable_tcb_manifest::validate_opaque_executable_admission(
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
            if pair[0].requirement_identity == pair[1].requirement_identity {
                return Err(format!(
                    "installation reach requirement `{}` has more than one selected resolution",
                    pair[0].requirement_identity
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
        self.report_fingerprint =
            selected_closure_report_fingerprint(&self.plans, &self.installation_reach_resolutions);
        Ok(self)
    }

    pub fn installation_reach_resolutions(&self) -> &[InstallationReachResolution] {
        &self.installation_reach_resolutions
    }

    pub fn installation_reach_resolution(
        &self,
        requirement_identity: &str,
    ) -> Option<&InstallationReachResolution> {
        self.installation_reach_resolutions
            .iter()
            .find(|resolution| resolution.requirement_identity == requirement_identity)
    }

    /// Resolve one root closure from its concrete reach plus exact bounded
    /// requirement dependencies. Absence rejects; an upper bound is never
    /// silently used as the selected row.
    pub fn resolve_installation_reach(
        &self,
        concrete_reach: &[String],
        requirement_identities: &[String],
    ) -> Result<Vec<String>, String> {
        let mut resolved = concrete_reach.to_vec();
        for requirement_identity in requirement_identities {
            let Some(row) = self.installation_reach_resolution(requirement_identity) else {
                return Err(format!(
                    "installation reach requirement `{requirement_identity}` remains unresolved at final admission"
                ));
            };
            resolved.extend(row.resolved_row.iter().cloned());
        }
        resolved.sort();
        resolved.dedup();
        Ok(resolved)
    }

    /// Derive caller-address-space TCB facts from the selected closure, never
    /// from source service reach or the unselected candidate set.
    pub fn executable_tcb_manifest(&self) -> crate::ExecutableTcbManifest {
        crate::executable_tcb_manifest::derive_static_manifest(
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
            crate::OpaqueInProcessBinding::StringBackedImportBootstrap { library, symbol } => {
                self.byte(1);
                self.string(library);
                self.string(symbol);
            }
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
mod tests {
    use super::*;
    use crate::provider_plan::{
        EvaluatedBindingEvaluationDigest, EvaluatedBindingMaterializationDigest,
        EvaluatedBindingProducerClosureDigest, EvaluatedBindingReceipt, EvaluatedBindingUsage,
        EvaluatedForeignImport, ProviderBinding, ProviderPlanRow, ServiceMethod, ServiceSchema,
    };

    fn evaluated_import(
        locator: crate::NormalizedForeignLocator,
        seed: u8,
    ) -> EvaluatedForeignImport {
        let usage =
            EvaluatedBindingUsage::from_evaluator(7, 1, 10, 1_000, 0, 0, 4, 12, 3, 0).unwrap();
        let receipt = EvaluatedBindingReceipt::from_evaluation(
            None,
            format!("fixture::producer::{seed}"),
            EvaluatedBindingProducerClosureDigest::from_bytes([seed; 32]).unwrap(),
            1,
            usage,
            EvaluatedBindingEvaluationDigest::from_bytes([seed.wrapping_add(1); 32]).unwrap(),
            1,
            EvaluatedBindingMaterializationDigest::from_bytes([seed.wrapping_add(2); 32]).unwrap(),
            locator.identity_digest(),
        )
        .unwrap();
        EvaluatedForeignImport::from_retained_evidence(locator, receipt).unwrap()
    }

    fn candidate(name: &str, method: &str) -> ProviderPlan {
        ProviderPlan {
            name: name.into(),
            provider_type: format!("{name}Provider"),
            provider_type_package_identity: None,
            target: "x86_64-unknown-none".into(),
            schema: ServiceSchema {
                trait_name: format!("{name}Service"),
                trait_package_identity: None,
                methods: vec![ServiceMethod {
                    name: method.into(),
                    requirement_owner: format!("{name}Service"),
                    requirement_owner_package_identity: None,
                    requirement_identity: format!("{name}Service::{method}"),
                    parameter_count: 0,
                    parameter_type_identities: Vec::new(),
                    entry_claims: Vec::new(),
                    has_result: false,
                    result_type_identity: None,
                    result_claims: Vec::new(),
                    service_reach: vec![format!("{name}Service")],
                    synchronous_invocations: Vec::new(),
                    may_suspend: false,
                    may_block: false,
                    terminates_guarantee: false,
                    termination_premises: Vec::new(),
                    calling_plan_report_fingerprint: None,
                    calling_plan_commitment: None,
                }],
            },
            rows: vec![ProviderPlanRow {
                method: method.into(),
                requirement_identity: format!("{name}Service::{method}"),
                requirement_lifetime_partition: Vec::new(),
                binding: ProviderBinding::CheckedAdapter {
                    machine_identity: format!("{name}Provider::{method}"),
                    machine_package_identity: None,
                },
            }],
            origin_package_identity: None,
            origin_package: "test".into(),
        }
    }

    #[test]
    fn selected_plans_are_retained_in_canonical_order() {
        let alpha = candidate("Alpha", "read");
        let beta = candidate("Beta", "write");
        let candidates = vec![beta.clone(), alpha.clone()];

        let first = SelectedProviderPlanFacts::from_selection(
            &candidates,
            &["Beta".into(), "Alpha".into()],
        )
        .expect("valid selection");
        let second = SelectedProviderPlanFacts::from_selection(
            &candidates,
            &["Alpha".into(), "Beta".into()],
        )
        .expect("valid selection");

        assert_eq!(first, second);
        assert_eq!(
            first
                .plans()
                .iter()
                .map(|plan| plan.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Alpha", "Beta"]
        );
        assert_eq!(
            first
                .plan_by_report_fingerprint(alpha.report_fingerprint())
                .map(|plan| plan.name.as_str()),
            Some("Alpha")
        );
    }

    #[test]
    fn exact_plan_lookup_rejects_a_substitute_claiming_the_same_report_identity() {
        let selected_plan = candidate("Alpha", "read");
        let mut substituted_plan = selected_plan.clone();
        substituted_plan.rows[0].binding = ProviderBinding::CheckedAdapter {
            machine_identity: "AlphaProvider::substituted_read".into(),
            machine_package_identity: None,
        };
        let report_identity = selected_plan.report_fingerprint();
        let selected = SelectedProviderPlanFacts::from_selected_plans(vec![selected_plan.clone()])
            .expect("selected provider plan");

        assert!(
            selected
                .plan_by_exact_evidence(report_identity, &substituted_plan)
                .is_none(),
            "claiming a collision-equal compact report identity cannot replace exact plan evidence"
        );
        assert_eq!(
            selected.plan_by_exact_evidence(report_identity, &selected_plan),
            Some(&selected_plan)
        );
    }

    #[test]
    fn selected_closure_digest_distinguishes_compact_equal_calling_plans() {
        let mut first = candidate("Alpha", "read");
        first.schema.methods[0].calling_plan_report_fingerprint = Some(0x1234);
        first.schema.methods[0].calling_plan_commitment =
            Some(crate::provider_plan::BoundaryCallingPlanCommitment::from_digest([0x11; 32]));
        let mut substituted = first.clone();
        substituted.schema.methods[0].calling_plan_commitment =
            Some(crate::provider_plan::BoundaryCallingPlanCommitment::from_digest([0x22; 32]));

        assert_eq!(first.report_fingerprint(), substituted.report_fingerprint());
        assert_ne!(first.identity_digest(), substituted.identity_digest());
        let first = SelectedProviderPlanFacts::from_selected_plans(vec![first])
            .expect("first exact selected closure");
        let substituted = SelectedProviderPlanFacts::from_selected_plans(vec![substituted])
            .expect("substituted exact selected closure");
        assert_eq!(first.report_fingerprint(), substituted.report_fingerprint());
        assert_ne!(first.identity_digest(), substituted.identity_digest());
    }

    #[test]
    fn resolved_selection_retains_same_spelled_plans_from_distinct_packages() {
        let first_package = psi_core::PackageKeyIdentity::from_digest([0x31; 32])
            .expect("nonzero package identity");
        let second_package = psi_core::PackageKeyIdentity::from_digest([0x32; 32])
            .expect("nonzero package identity");
        let mut first = candidate("Shared", "run");
        first.schema.trait_package_identity = Some(first_package);
        let mut second = first.clone();
        second.schema.trait_package_identity = Some(second_package);

        let selected =
            SelectedProviderPlanFacts::from_selected_plans(vec![second.clone(), first.clone()])
                .expect("package-qualified slots remain distinct after resolution");

        assert_eq!(selected.plans().len(), 2);
        assert!(selected.plans().contains(&first));
        assert!(selected.plans().contains(&second));
        assert!(
            SelectedProviderPlanFacts::from_selected_plans(vec![first.clone(), first.clone()])
                .expect_err("the same resolved plan cannot be retained twice")
                .contains("appears more than once")
        );
        assert!(
            SelectedProviderPlanFacts::from_selection(&[first, second], &["Shared".to_owned()])
                .expect_err("legacy name-only selection cannot choose between packages")
                .contains("matches 2 candidates")
        );
    }

    #[test]
    fn identity_lookup_distinguishes_same_readable_name_across_packages() {
        let first_package = psi_core::PackageKeyIdentity::from_digest([0x41; 32])
            .expect("nonzero package identity");
        let second_package = psi_core::PackageKeyIdentity::from_digest([0x42; 32])
            .expect("nonzero package identity");
        let mut first = candidate("Shared", "run");
        first.origin_package_identity = Some(first_package);
        first.provider_type_package_identity = Some(first_package);
        first.schema.trait_package_identity = Some(first_package);
        first.schema.methods[0].requirement_owner_package_identity = Some(first_package);
        let ProviderBinding::CheckedAdapter {
            machine_package_identity,
            ..
        } = &mut first.rows[0].binding
        else {
            panic!("test candidate must use a checked adapter");
        };
        *machine_package_identity = Some(first_package);
        let mut second = first.clone();
        second.origin_package_identity = Some(second_package);
        second.provider_type_package_identity = Some(second_package);
        second.schema.trait_package_identity = Some(second_package);
        second.schema.methods[0].requirement_owner_package_identity = Some(second_package);
        let ProviderBinding::CheckedAdapter {
            machine_package_identity,
            ..
        } = &mut second.rows[0].binding
        else {
            panic!("test candidate must use a checked adapter");
        };
        *machine_package_identity = Some(second_package);

        let first_identity = first.report_fingerprint();
        let second_identity = second.report_fingerprint();
        assert_ne!(first_identity, second_identity);

        let selected = SelectedProviderPlanFacts::from_selected_plans(vec![first, second])
            .expect("package-qualified same-readable-name plans remain independently addressable");

        assert_eq!(
            selected
                .plan_by_report_fingerprint(first_identity)
                .and_then(|plan| plan.origin_package_identity),
            Some(first_package)
        );
        assert_eq!(
            selected
                .plan_by_report_fingerprint(second_identity)
                .and_then(|plan| plan.origin_package_identity),
            Some(second_package)
        );
    }

    #[test]
    fn installation_reach_resolution_is_exact_bounded_selected_evidence() {
        let plan = candidate("Interrupt", "complete");
        let plan_identity = plan.report_fingerprint();
        let selected = SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&plan),
            std::slice::from_ref(&plan.name),
        )
        .expect("selected provider");
        let base_identity = selected.report_fingerprint();
        let requirement_identity = plan.schema.methods[0].requirement_identity.clone();
        let resolved = selected
            .with_installation_reach_resolutions(vec![InstallationReachResolution {
                requirement_identity: requirement_identity.clone(),
                provider_plan_report_identity: plan_identity,
                upper_bound: vec!["PortIo".into(), "MachineControl".into()],
                resolved_row: vec!["PortIo".into()],
            }])
            .expect("selected row refines its bound");

        assert_ne!(resolved.report_fingerprint(), base_identity);
        let row = resolved
            .installation_reach_resolution(&requirement_identity)
            .expect("exact requirement resolution");
        assert_eq!(row.upper_bound, ["MachineControl", "PortIo"]);
        assert_eq!(row.resolved_row, ["PortIo"]);
        assert_eq!(
            resolved
                .resolve_installation_reach(
                    &["InterruptCompletion".into(), "MachineControl".into()],
                    std::slice::from_ref(&requirement_identity),
                )
                .expect("selected row closes the root"),
            ["InterruptCompletion", "MachineControl", "PortIo"]
        );
        assert!(
            resolved
                .resolve_installation_reach(&[], &["Missing::requirement".into()])
                .expect_err("final admission rejects unresolved rows")
                .contains("remains unresolved at final admission")
        );

        let outside = SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&plan),
            std::slice::from_ref(&plan.name),
        )
        .expect("selected provider")
        .with_installation_reach_resolutions(vec![InstallationReachResolution {
            requirement_identity,
            provider_plan_report_identity: plan_identity,
            upper_bound: vec!["MachineControl".into()],
            resolved_row: vec!["FilesystemHost".into()],
        }])
        .expect_err("resolved row outside the bound must reject");
        assert!(outside.contains("exceeds its published upper bound"));
    }

    #[test]
    fn absent_duplicate_and_partial_selections_reject() {
        let complete = candidate("Complete", "run");
        assert!(
            SelectedProviderPlanFacts::from_selection(
                std::slice::from_ref(&complete),
                &["Missing".into()]
            )
            .expect_err("missing candidate must reject")
            .contains("absent")
        );
        assert!(
            SelectedProviderPlanFacts::from_selection(
                std::slice::from_ref(&complete),
                &["Complete".into(), "Complete".into()]
            )
            .expect_err("duplicate selection must reject")
            .contains("more than once")
        );

        let mut partial = candidate("Partial", "run");
        partial.rows.clear();
        assert!(
            SelectedProviderPlanFacts::from_selection(&[partial], &["Partial".into()])
                .expect_err("partial selected plan must reject")
                .contains("not fully covering")
        );

        let first = candidate("First", "run");
        let mut second = candidate("Second", "run");
        second.schema.trait_name = first.schema.trait_name.clone();
        assert!(
            SelectedProviderPlanFacts::from_selection(
                &[first, second],
                &["First".into(), "Second".into()]
            )
            .expect_err("one boundary slot cannot retain two selected plans")
            .contains("more than one selected provider plan")
        );
    }

    #[test]
    fn selection_rejects_name_only_requirement_rows() {
        let mut incomplete = candidate("Incomplete", "run");
        incomplete.rows[0].requirement_identity.clear();
        assert!(
            SelectedProviderPlanFacts::from_selection(
                std::slice::from_ref(&incomplete),
                std::slice::from_ref(&incomplete.name),
            )
            .expect_err("name-only provider rows must not enter the selected closure")
            .contains("no exact requirement identity")
        );

        let mut incomplete = candidate("IncompleteSchema", "run");
        incomplete.schema.methods[0].requirement_identity.clear();
        assert!(
            SelectedProviderPlanFacts::from_selection(
                std::slice::from_ref(&incomplete),
                std::slice::from_ref(&incomplete.name),
            )
            .expect_err("name-only provider schema methods must not enter the selected closure")
            .contains("no exact requirement identity")
        );
    }

    #[test]
    fn opaque_selection_survives_a_checked_wrapper_as_attributed_incompleteness() {
        let checked_wrapper = candidate("CheckedWrapper", "read");
        let mut opaque_leaf = candidate("OpaqueLeaf", "read_raw");
        opaque_leaf.schema.trait_name = "RawStorage".into();
        opaque_leaf.target = "windows_x86_64".into();
        let locator = crate::normalize_foreign_locator(
            crate::ForeignLocatorCandidate::PeByName {
                library: b"vendor-storage.dll".to_vec(),
                export: b"read_raw".to_vec(),
            },
            omega_target::TargetProfile::WindowsX64,
        )
        .expect("normalized opaque import");
        opaque_leaf.rows[0].binding = ProviderBinding::Import {
            evaluated: evaluated_import(locator.clone(), 11),
        };
        let selected = SelectedProviderPlanFacts::from_selection(
            &[checked_wrapper.clone(), opaque_leaf.clone()],
            &[checked_wrapper.name.clone(), opaque_leaf.name.clone()],
        )
        .expect("both transitive selections are exact");

        let manifest = selected.executable_tcb_manifest();
        assert_eq!(manifest.known_entries.len(), 1);
        let crate::ScopeCompleteness::Incomplete { causes, .. } = manifest.completeness else {
            panic!("opaque in-process selection must make the scope incomplete");
        };
        assert_eq!(causes.len(), 1);
        assert!(matches!(
            &causes[0],
            crate::IncompleteCause::SelectedOpaqueProvider {
                provider_plan_report_identity,
                binding: crate::OpaqueInProcessBinding::Import {
                    evaluated: retained,
                },
                ..
            } if *provider_plan_report_identity == opaque_leaf.report_fingerprint()
                && retained.locator() == &locator
        ));
    }

    #[test]
    fn selected_plan_identity_changes_with_normalized_import_coordinates() {
        fn selected(export: &[u8]) -> SelectedProviderPlanFacts {
            let mut plan = candidate("OpaqueLeaf", "read_raw");
            plan.target = "windows_x86_64".into();
            let locator = crate::normalize_foreign_locator(
                crate::ForeignLocatorCandidate::PeByName {
                    library: b"vendor-storage.dll".to_vec(),
                    export: export.to_vec(),
                },
                omega_target::TargetProfile::WindowsX64,
            )
            .expect("normalized selected import");
            plan.rows[0].binding = ProviderBinding::Import {
                evaluated: evaluated_import(locator, 21),
            };
            SelectedProviderPlanFacts::from_selection(
                std::slice::from_ref(&plan),
                std::slice::from_ref(&plan.name),
            )
            .expect("selected normalized import")
        }

        assert_ne!(
            selected(b"read_raw").report_fingerprint(),
            selected(b"write_raw").report_fingerprint(),
        );
    }

    #[test]
    fn selected_plan_identity_retains_macho_install_name_and_symbol() {
        fn selected(install_name: &[u8], symbol: &[u8]) -> SelectedProviderPlanFacts {
            let mut plan = candidate("OpaqueLeaf", "read_raw");
            plan.target = "macos_arm64".into();
            let locator = crate::normalize_foreign_locator(
                crate::ForeignLocatorCandidate::MachODylibSymbol {
                    install_name: install_name.to_vec(),
                    symbol: symbol.to_vec(),
                },
                omega_target::TargetProfile::MacosArm64,
            )
            .expect("normalized selected Mach-O import");
            plan.rows[0].binding = ProviderBinding::Import {
                evaluated: evaluated_import(locator, 31),
            };
            SelectedProviderPlanFacts::from_selection(
                std::slice::from_ref(&plan),
                std::slice::from_ref(&plan.name),
            )
            .expect("selected normalized Mach-O import")
        }

        let baseline = selected(b"/usr/lib/libSystem.B.dylib", b"_read");
        assert_ne!(
            baseline.identity_digest(),
            selected(b"/usr/lib/libobjc.A.dylib", b"_read").identity_digest(),
        );
        assert_ne!(
            baseline.identity_digest(),
            selected(b"/usr/lib/libSystem.B.dylib", b"_write").identity_digest(),
        );
    }

    #[test]
    fn pinned_opaque_entry_remains_incomplete_without_executable_closure_evidence() {
        let mut opaque = candidate("Opaque", "read");
        opaque.rows[0].binding = ProviderBinding::StringBackedImportBootstrap {
            library: "vendor-storage".into(),
            symbol: "read".into(),
        };
        let plan_identity = opaque.report_fingerprint();
        let selected = SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&opaque),
            std::slice::from_ref(&opaque.name),
        )
        .expect("selected opaque provider")
        .with_opaque_executable_admissions([crate::OpaqueExecutableAdmissionCandidate {
            provider_plan_report_identity: plan_identity,
            provider_plan_digest: opaque.identity_digest(),
            method: "read".into(),
            requirement_identity: opaque.schema.methods[0].requirement_identity.clone(),
            binding: crate::OpaqueInProcessBinding::StringBackedImportBootstrap {
                library: "vendor-storage".into(),
                symbol: "read".into(),
            },
            executable_identity: "sha256:0123456789abcdef".into(),
            implementation_evidence_identity: "receipt:vendor-storage-v1".into(),
            execution_scope: crate::ExecutionScope::CallerAddressSpace,
            containment: vec![crate::ContainmentEvidence {
                guarantee: crate::ContainmentGuarantee::FaultContainment,
                evidence_identity: "receipt:fault-boundary-v1".into(),
            }],
            executable_closure_evidence_identity: None,
        }])
        .expect("exact opaque admission");

        let manifest = selected.executable_tcb_manifest();
        assert_eq!(manifest.known_entries.len(), 1);
        assert!(matches!(
            manifest.known_entries[0].executable_identity,
            crate::ExecutableIdentity::PinnedOpaqueArtifact(ref identity)
                if identity == "sha256:0123456789abcdef"
        ));
        assert!(matches!(
            manifest.completeness,
            crate::ScopeCompleteness::Incomplete { ref causes, .. } if causes.len() == 1
        ));
    }

    #[test]
    fn exact_closure_and_containment_receipts_complete_the_opaque_scope() {
        let mut opaque = candidate("Opaque", "read");
        opaque.rows[0].binding = ProviderBinding::StringBackedImportBootstrap {
            library: "platform".into(),
            symbol: "read".into(),
        };
        let plan_identity = opaque.report_fingerprint();
        let selected = SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&opaque),
            std::slice::from_ref(&opaque.name),
        )
        .expect("selected opaque provider")
        .with_opaque_executable_admissions([crate::OpaqueExecutableAdmissionCandidate {
            provider_plan_report_identity: plan_identity,
            provider_plan_digest: opaque.identity_digest(),
            method: "read".into(),
            requirement_identity: opaque.schema.methods[0].requirement_identity.clone(),
            binding: crate::OpaqueInProcessBinding::StringBackedImportBootstrap {
                library: "platform".into(),
                symbol: "read".into(),
            },
            executable_identity: "platform-baseline:read-v1".into(),
            implementation_evidence_identity: "receipt:platform-read-v1".into(),
            execution_scope: crate::ExecutionScope::CallerAddressSpace,
            containment: vec![
                crate::ContainmentEvidence {
                    guarantee: crate::ContainmentGuarantee::BoundedResources,
                    evidence_identity: "receipt:quota-v1".into(),
                },
                crate::ContainmentEvidence {
                    guarantee: crate::ContainmentGuarantee::MemoryIsolation,
                    evidence_identity: "receipt:memory-v1".into(),
                },
            ],
            executable_closure_evidence_identity: Some("receipt:closed-loader-v1".into()),
        }])
        .expect("exact opaque admission");

        let manifest = selected.executable_tcb_manifest();
        assert_eq!(manifest.known_entries[0].containment.len(), 2);
        let crate::ScopeCompleteness::Complete {
            opaque_closure_evidence,
            ..
        } = manifest.completeness
        else {
            panic!("closed executable envelope should complete the scope");
        };
        assert_eq!(opaque_closure_evidence.len(), 1);
        assert_eq!(
            opaque_closure_evidence[0].evidence_identity,
            "receipt:closed-loader-v1"
        );
    }

    #[test]
    fn exact_closure_evidence_survives_an_unrelated_incomplete_row() {
        let mut closed = candidate("Closed", "read");
        closed.rows[0].binding = ProviderBinding::StringBackedImportBootstrap {
            library: "closed-platform".into(),
            symbol: "read".into(),
        };
        let mut open = candidate("Open", "write");
        open.rows[0].binding = ProviderBinding::StringBackedImportBootstrap {
            library: "open-vendor".into(),
            symbol: "write".into(),
        };
        let closed_identity = closed.report_fingerprint();
        let selected = SelectedProviderPlanFacts::from_selection(
            &[closed.clone(), open.clone()],
            &[closed.name.clone(), open.name.clone()],
        )
        .expect("two distinct selected slots")
        .with_opaque_executable_admissions([crate::OpaqueExecutableAdmissionCandidate {
            provider_plan_report_identity: closed_identity,
            provider_plan_digest: closed.identity_digest(),
            method: "read".into(),
            requirement_identity: closed.schema.methods[0].requirement_identity.clone(),
            binding: crate::OpaqueInProcessBinding::StringBackedImportBootstrap {
                library: "closed-platform".into(),
                symbol: "read".into(),
            },
            executable_identity: "platform-baseline:closed-read-v1".into(),
            implementation_evidence_identity: "receipt:closed-read-v1".into(),
            execution_scope: crate::ExecutionScope::CallerAddressSpace,
            containment: Vec::new(),
            executable_closure_evidence_identity: Some("receipt:closed-loader-v1".into()),
        }])
        .expect("closed row admission");

        let manifest = selected.executable_tcb_manifest();
        let crate::ScopeCompleteness::Incomplete {
            causes,
            opaque_closure_evidence,
            ..
        } = manifest.completeness
        else {
            panic!("unadmitted opaque row keeps scope incomplete");
        };
        assert_eq!(causes.len(), 1);
        assert!(matches!(
            &causes[0],
            crate::IncompleteCause::SelectedOpaqueProvider {
                provider_plan_report_identity,
                ..
            } if *provider_plan_report_identity == open.report_fingerprint()
        ));
        assert_eq!(opaque_closure_evidence.len(), 1);
        assert_eq!(
            opaque_closure_evidence[0].evidence_identity,
            "receipt:closed-loader-v1"
        );
    }

    #[test]
    fn opaque_admission_rejects_binding_drift_and_duplicate_containment_axes() {
        let mut opaque = candidate("Opaque", "read");
        opaque.rows[0].binding = ProviderBinding::StringBackedImportBootstrap {
            library: "platform".into(),
            symbol: "read".into(),
        };
        let plan_identity = opaque.report_fingerprint();
        let selected = SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&opaque),
            std::slice::from_ref(&opaque.name),
        )
        .expect("selected opaque provider");
        let candidate = crate::OpaqueExecutableAdmissionCandidate {
            provider_plan_report_identity: plan_identity,
            provider_plan_digest: opaque.identity_digest(),
            method: "read".into(),
            requirement_identity: opaque.schema.methods[0].requirement_identity.clone(),
            binding: crate::OpaqueInProcessBinding::StringBackedImportBootstrap {
                library: "other".into(),
                symbol: "read".into(),
            },
            executable_identity: "sha256:0123456789abcdef".into(),
            implementation_evidence_identity: "receipt:opaque-v1".into(),
            execution_scope: crate::ExecutionScope::CallerAddressSpace,
            containment: Vec::new(),
            executable_closure_evidence_identity: None,
        };
        let mut compact_equal_wrong_digest = candidate.clone();
        let mut substituted_plan = opaque.clone();
        substituted_plan.name = "compact-equal-substitution".into();
        compact_equal_wrong_digest.provider_plan_digest = substituted_plan.identity_digest();
        assert!(
            selected
                .clone()
                .with_opaque_executable_admissions([compact_equal_wrong_digest])
                .expect_err("compact report identity cannot select a different exact plan")
                .contains("unselected provider plan")
        );
        assert!(
            selected
                .clone()
                .with_opaque_executable_admissions([candidate.clone()])
                .expect_err("binding drift")
                .contains("binding drift")
        );

        let mut candidate = candidate;
        candidate.binding = crate::OpaqueInProcessBinding::StringBackedImportBootstrap {
            library: "platform".into(),
            symbol: "read".into(),
        };
        candidate.containment = vec![
            crate::ContainmentEvidence {
                guarantee: crate::ContainmentGuarantee::FaultContainment,
                evidence_identity: "receipt:fault-a".into(),
            },
            crate::ContainmentEvidence {
                guarantee: crate::ContainmentGuarantee::FaultContainment,
                evidence_identity: "receipt:fault-b".into(),
            },
        ];
        assert!(
            selected
                .with_opaque_executable_admissions([candidate])
                .expect_err("duplicate containment axis")
                .contains("repeats one containment guarantee")
        );
    }

    #[test]
    fn checked_and_intrinsic_entries_are_derived_only_from_selected_plans() {
        let checked = candidate("Checked", "run");
        let mut intrinsic = candidate("Intrinsic", "halt");
        intrinsic.schema.trait_name = "MachineControl".into();
        intrinsic.rows[0].binding = ProviderBinding::CompilerIntrinsic {
            machine: "MachineControl::halt".into(),
        };
        let unselected = candidate("Unselected", "skip");
        let selected = SelectedProviderPlanFacts::from_selection(
            &[checked.clone(), intrinsic.clone(), unselected],
            &[intrinsic.name.clone(), checked.name.clone()],
        )
        .expect("selected closure");

        let manifest = selected.executable_tcb_manifest();
        assert_eq!(manifest.known_entries.len(), 2);
        assert!(matches!(
            manifest.completeness,
            crate::ScopeCompleteness::Complete {
                selected_provider_closure_report_identity,
                ..
            } if selected_provider_closure_report_identity == selected.report_fingerprint()
        ));
        assert!(manifest.known_entries.iter().all(|entry| {
            entry.origin == crate::ExecutableEntryOrigin::StaticSelection
                && entry.execution_scope == crate::ExecutionScope::CallerAddressSpace
                && entry.selected_requirement.is_some()
        }));
    }

    #[test]
    fn executable_manifest_keeps_same_named_overload_rows_distinct() {
        let mut overloaded = candidate("Convert", "convert");
        let first_identity = "named-callable:path=ConvertService::convert;result=Ordinary";
        let second_identity = "named-callable:path=ConvertService::convert;result=Saturating";
        let mut second_method = overloaded.schema.methods[0].clone();
        overloaded.schema.methods[0].requirement_identity = first_identity.into();
        second_method.requirement_identity = second_identity.into();
        overloaded.schema.methods.push(second_method);
        overloaded.rows[0].requirement_identity = first_identity.into();
        overloaded.rows.push(ProviderPlanRow {
            method: "convert".into(),
            requirement_identity: second_identity.into(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::CheckedAdapter {
                machine_identity: "ConvertProvider::convert".into(),
                machine_package_identity: None,
            },
        });

        let selected =
            SelectedProviderPlanFacts::from_selection(&[overloaded], &["Convert".into()])
                .expect("same-named exact overload rows cover distinct requirements");
        let manifest = selected.executable_tcb_manifest();
        assert_eq!(
            manifest.known_entries.len(),
            2,
            "one shared executable must not collapse distinct selected requirement rows"
        );
        let mut identities = manifest
            .known_entries
            .iter()
            .map(|entry| {
                let requirement = entry
                    .selected_requirement
                    .as_ref()
                    .expect("static selected row identity");
                assert_eq!(requirement.method, "convert");
                requirement.requirement_identity.as_str()
            })
            .collect::<Vec<_>>();
        identities.sort_unstable();
        assert_eq!(identities, [first_identity, second_identity]);
    }
}
