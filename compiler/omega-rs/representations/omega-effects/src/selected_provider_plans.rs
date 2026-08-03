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
    opaque_executable_admissions: Vec<crate::ValidatedOpaqueExecutableAdmission>,
}

impl Default for SelectedProviderPlanFacts {
    fn default() -> Self {
        Self {
            plans: Vec::new(),
            normalized_identity: fingerprint_selected_plans(&[]),
            opaque_executable_admissions: Vec::new(),
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
            opaque_executable_admissions: Vec::new(),
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

    /// Derive caller-address-space TCB facts from the selected closure, never
    /// from source service reach or the unselected candidate set.
    pub fn executable_tcb_manifest(&self) -> crate::ExecutableTcbManifest {
        crate::executable_tcb_manifest::derive_static_manifest(
            &self.plans,
            self.normalized_identity,
            &self.opaque_executable_admissions,
        )
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_plan::{ProviderBinding, ProviderPlanRow, ServiceMethod, ServiceSchema};

    fn candidate(name: &str, method: &str) -> ProviderPlan {
        ProviderPlan {
            name: name.into(),
            provider_type: format!("{name}Provider"),
            target: "x86_64-unknown-none".into(),
            schema: ServiceSchema {
                trait_name: format!("{name}Service"),
                methods: vec![ServiceMethod {
                    name: method.into(),
                    requirement_owner: format!("{name}Service"),
                    requirement_identity: String::new(),
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
                    calling_plan_fingerprint: None,
                }],
            },
            rows: vec![ProviderPlanRow {
                method: method.into(),
                requirement_identity: String::new(),
                binding: ProviderBinding::CheckedAdapter {
                    machine: format!("{name}Provider::{method}"),
                },
            }],
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
        assert_eq!(
            causes[0].provider_plan_identity,
            opaque_leaf.identity_fingerprint()
        );
        assert!(matches!(
            causes[0].binding,
            crate::OpaqueInProcessBinding::Import { .. }
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
            requirement_identity: String::new(),
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
            requirement_identity: String::new(),
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
            requirement_identity: String::new(),
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
        assert_eq!(
            causes[0].provider_plan_identity,
            open.identity_fingerprint()
        );
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
            requirement_identity: String::new(),
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
            name: "MachineControl::halt".into(),
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
        }));
    }
}
