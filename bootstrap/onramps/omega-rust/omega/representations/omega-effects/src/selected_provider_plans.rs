use crate::provider_plan::ProviderPlan;
use std::collections::BTreeSet;

/// The exact provider plans selected by the compiler for one checked program.
///
/// Candidates remain ordinary policy values. This carrier retains only the
/// fully covering candidates selected for the concrete target, in canonical
/// name order, so later provider execution and generated-machine lowering do
/// not have to rediscover selection from source declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderPlanFacts {
    plans: Vec<ProviderPlan>,
    normalized_identity: u64,
    execution_scope: crate::ExecutionScope,
    opaque_executable_admissions: Vec<crate::ValidatedOpaqueExecutableAdmission>,
    installation_reach_resolutions: Vec<InstallationReachResolution>,
}

impl Default for SelectedProviderPlanFacts {
    fn default() -> Self {
        Self {
            plans: Vec::new(),
            normalized_identity: fingerprint_selected_plans(&[]),
            execution_scope: crate::ExecutionScope::CallerAddressSpace,
            opaque_executable_admissions: Vec::new(),
            installation_reach_resolutions: Vec::new(),
        }
    }
}

impl SelectedProviderPlanFacts {
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
        let mut identities = BTreeSet::new();
        let mut boundary_slots = BTreeSet::new();
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
            let errors = plan.validate_against_schema();
            if !errors.is_empty() {
                return Err(format!(
                    "selected provider plan `{name}` is not fully covering: {}",
                    errors.join("; ")
                ));
            }
            let identity = plan.identity_fingerprint();
            if identity == 0 {
                return Err(format!(
                    "selected provider plan `{name}` produced the reserved zero identity"
                ));
            }
            if !identities.insert(identity) {
                return Err(format!(
                    "selected provider plan `{name}` collides with another selected plan at identity {identity:#018x}"
                ));
            }
            if !boundary_slots.insert(plan.schema.trait_name.as_str()) {
                return Err(format!(
                    "boundary slot `{}` has more than one selected provider plan",
                    plan.schema.trait_name
                ));
            }
            plans.push((*plan).clone());
        }

        let normalized_identity = fingerprint_selected_plans(&plans);
        Ok(Self {
            plans,
            normalized_identity,
            execution_scope: crate::ExecutionScope::CallerAddressSpace,
            opaque_executable_admissions: Vec::new(),
            installation_reach_resolutions: Vec::new(),
        })
    }

    pub fn plans(&self) -> &[ProviderPlan] {
        &self.plans
    }

    pub fn plan_by_name(&self, name: &str) -> Option<&ProviderPlan> {
        self.plans.iter().find(|plan| plan.name == name)
    }

    pub fn plan_by_identity(&self, identity: u64) -> Option<&ProviderPlan> {
        self.plans
            .iter()
            .find(|plan| plan.identity_fingerprint() == identity)
    }

    pub const fn normalized_identity(&self) -> u64 {
        self.normalized_identity
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
                    candidate.provider_plan_identity,
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
                candidate.provider_plan_identity,
                candidate.method.clone(),
                candidate.requirement_identity.clone(),
            );
            if !occupied.insert(key) {
                return Err(format!(
                    "opaque executable admission duplicates selected row `{}` / `{}` in provider plan {:#018x}",
                    candidate.method,
                    candidate.requirement_identity,
                    candidate.provider_plan_identity
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
            left.provider_plan_identity
                .cmp(&right.provider_plan_identity)
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
                    left.provider_plan_identity
                        .cmp(&right.provider_plan_identity)
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
            let Some(plan) = self.plan_by_identity(resolution.provider_plan_identity) else {
                return Err(format!(
                    "installation reach resolution for `{}` names unselected provider plan {:#018x}",
                    resolution.requirement_identity, resolution.provider_plan_identity
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
        self.normalized_identity = fingerprint_selected_plans_and_reaches(
            &self.plans,
            &self.installation_reach_resolutions,
        );
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
            self.normalized_identity,
            self.execution_scope,
            &self.opaque_executable_admissions,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallationReachResolution {
    pub requirement_identity: String,
    pub provider_plan_identity: u64,
    pub upper_bound: Vec<String>,
    pub resolved_row: Vec<String>,
}

fn fingerprint_selected_plans(plans: &[ProviderPlan]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in (plans.len() as u64).to_le_bytes().into_iter().chain(
        plans
            .iter()
            .flat_map(|plan| plan.identity_fingerprint().to_le_bytes()),
    ) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn fingerprint_selected_plans_and_reaches(
    plans: &[ProviderPlan],
    resolutions: &[InstallationReachResolution],
) -> u64 {
    let mut hash = fingerprint_selected_plans(plans);
    for resolution in resolutions {
        for byte in resolution
            .requirement_identity
            .as_bytes()
            .iter()
            .copied()
            .chain(resolution.provider_plan_identity.to_le_bytes())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_plan::{ProviderBinding, ProviderPlanRow, ServiceMethod, ServiceSchema};

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
                    calling_plan_fingerprint: None,
                }],
            },
            rows: vec![ProviderPlanRow {
                method: method.into(),
                requirement_identity: format!("{name}Service::{method}"),
                binding: ProviderBinding::CheckedAdapter {
                    machine: format!("{name}Provider::{method}"),
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
                .plan_by_identity(alpha.identity_fingerprint())
                .map(|plan| plan.name.as_str()),
            Some("Alpha")
        );
    }

    #[test]
    fn installation_reach_resolution_is_exact_bounded_selected_evidence() {
        let plan = candidate("Interrupt", "complete");
        let plan_identity = plan.identity_fingerprint();
        let selected = SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&plan),
            std::slice::from_ref(&plan.name),
        )
        .expect("selected provider");
        let base_identity = selected.normalized_identity();
        let requirement_identity = plan.schema.methods[0].requirement_identity.clone();
        let resolved = selected
            .with_installation_reach_resolutions(vec![InstallationReachResolution {
                requirement_identity: requirement_identity.clone(),
                provider_plan_identity: plan_identity,
                upper_bound: vec!["PortIo".into(), "MachineControl".into()],
                resolved_row: vec!["PortIo".into()],
            }])
            .expect("selected row refines its bound");

        assert_ne!(resolved.normalized_identity(), base_identity);
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
            provider_plan_identity: plan_identity,
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
        opaque_leaf.rows[0].binding = ProviderBinding::Import {
            library: "vendor-storage".into(),
            symbol: "read_raw".into(),
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
                provider_plan_identity,
                binding: crate::OpaqueInProcessBinding::Import { .. },
                ..
            } if *provider_plan_identity == opaque_leaf.identity_fingerprint()
        ));
    }

    #[test]
    fn pinned_opaque_entry_remains_incomplete_without_executable_closure_evidence() {
        let mut opaque = candidate("Opaque", "read");
        opaque.rows[0].binding = ProviderBinding::Import {
            library: "vendor-storage".into(),
            symbol: "read".into(),
        };
        let plan_identity = opaque.identity_fingerprint();
        let selected = SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&opaque),
            std::slice::from_ref(&opaque.name),
        )
        .expect("selected opaque provider")
        .with_opaque_executable_admissions([crate::OpaqueExecutableAdmissionCandidate {
            provider_plan_identity: plan_identity,
            method: "read".into(),
            requirement_identity: opaque.schema.methods[0].requirement_identity.clone(),
            binding: crate::OpaqueInProcessBinding::Import {
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
        opaque.rows[0].binding = ProviderBinding::Import {
            library: "platform".into(),
            symbol: "read".into(),
        };
        let plan_identity = opaque.identity_fingerprint();
        let selected = SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&opaque),
            std::slice::from_ref(&opaque.name),
        )
        .expect("selected opaque provider")
        .with_opaque_executable_admissions([crate::OpaqueExecutableAdmissionCandidate {
            provider_plan_identity: plan_identity,
            method: "read".into(),
            requirement_identity: opaque.schema.methods[0].requirement_identity.clone(),
            binding: crate::OpaqueInProcessBinding::Import {
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
        closed.rows[0].binding = ProviderBinding::Import {
            library: "closed-platform".into(),
            symbol: "read".into(),
        };
        let mut open = candidate("Open", "write");
        open.rows[0].binding = ProviderBinding::Import {
            library: "open-vendor".into(),
            symbol: "write".into(),
        };
        let closed_identity = closed.identity_fingerprint();
        let selected = SelectedProviderPlanFacts::from_selection(
            &[closed.clone(), open.clone()],
            &[closed.name.clone(), open.name.clone()],
        )
        .expect("two distinct selected slots")
        .with_opaque_executable_admissions([crate::OpaqueExecutableAdmissionCandidate {
            provider_plan_identity: closed_identity,
            method: "read".into(),
            requirement_identity: closed.schema.methods[0].requirement_identity.clone(),
            binding: crate::OpaqueInProcessBinding::Import {
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
                provider_plan_identity,
                ..
            } if *provider_plan_identity == open.identity_fingerprint()
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
        opaque.rows[0].binding = ProviderBinding::Import {
            library: "platform".into(),
            symbol: "read".into(),
        };
        let plan_identity = opaque.identity_fingerprint();
        let selected = SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&opaque),
            std::slice::from_ref(&opaque.name),
        )
        .expect("selected opaque provider");
        let candidate = crate::OpaqueExecutableAdmissionCandidate {
            provider_plan_identity: plan_identity,
            method: "read".into(),
            requirement_identity: opaque.schema.methods[0].requirement_identity.clone(),
            binding: crate::OpaqueInProcessBinding::Import {
                library: "other".into(),
                symbol: "read".into(),
            },
            executable_identity: "sha256:0123456789abcdef".into(),
            implementation_evidence_identity: "receipt:opaque-v1".into(),
            execution_scope: crate::ExecutionScope::CallerAddressSpace,
            containment: Vec::new(),
            executable_closure_evidence_identity: None,
        };
        assert!(
            selected
                .clone()
                .with_opaque_executable_admissions([candidate.clone()])
                .expect_err("binding drift")
                .contains("binding drift")
        );

        let mut candidate = candidate;
        candidate.binding = crate::OpaqueInProcessBinding::Import {
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
                selected_provider_closure_identity,
                ..
            } if selected_provider_closure_identity == selected.normalized_identity()
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
            binding: ProviderBinding::CheckedAdapter {
                machine: "ConvertProvider::convert".into(),
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
