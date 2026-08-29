#[cfg(test)]
use crate::provider_plan::ProviderPlan;
use crate::provider_plan::ProviderPlanDigest;
use crate::{
    ContainmentGuarantee, ExecutableEntryOrigin, ExecutableIdentity, ExecutableTcbEntry,
    ExecutableTcbManifest, ExecutionScope, ImplementationEvidence, IncompleteCause,
    ProviderIdentity, ScopeCompleteness, SelectedProviderRequirement,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncompleteScopePolicy {
    PermitAndMark,
    Reject,
}

/// One exact non-local entry allowed by an artifact profile.
///
/// Provider, plan, selected requirement (when row-backed), executable,
/// implementation evidence, origin, and scope all participate. Required
/// containment names axes, while the manifest entry retains the independently
/// admitted evidence identity for each axis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactExecutableTcbAllowance {
    pub provider_identity: ProviderIdentity,
    pub provider_plan_report_identity: u64,
    pub provider_plan_digest: ProviderPlanDigest,
    pub selected_requirement: Option<SelectedProviderRequirement>,
    pub executable_identity: ExecutableIdentity,
    pub implementation_evidence: ImplementationEvidence,
    pub origin: ExecutableEntryOrigin,
    pub execution_scope: ExecutionScope,
    pub required_containment: Vec<ContainmentGuarantee>,
}

/// Ordinary build/deployment policy over one scope-relative TCB manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableTcbProfile {
    pub name: String,
    pub scope: ExecutionScope,
    /// Checked code in the artifact being evaluated can be admitted as a
    /// class. Compiler-known and opaque entries always require exact rules.
    pub allow_static_current_artifact_checked_bodies: bool,
    pub exact_allowances: Vec<ExactExecutableTcbAllowance>,
    pub incomplete_scope: IncompleteScopePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableTcbProfileAcceptance {
    profile: ExecutableTcbProfile,
    manifest: ExecutableTcbManifest,
    /// Nonempty only for the explicit `PermitAndMark` policy.
    marked_incomplete_causes: Vec<IncompleteCause>,
}

impl ExecutableTcbProfileAcceptance {
    pub const fn profile(&self) -> &ExecutableTcbProfile {
        &self.profile
    }

    pub const fn scope(&self) -> ExecutionScope {
        self.profile.scope
    }

    pub const fn manifest(&self) -> &ExecutableTcbManifest {
        &self.manifest
    }

    pub fn known_entry_count(&self) -> usize {
        self.manifest.known_entries.len()
    }

    pub fn marked_incomplete_causes(&self) -> &[IncompleteCause] {
        &self.marked_incomplete_causes
    }

    pub fn is_marked_incomplete(&self) -> bool {
        !self.marked_incomplete_causes.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableTcbProfileRejection {
    pub profile: String,
    pub violations: Vec<ExecutableTcbProfileViolation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutableTcbProfileViolation {
    EmptyProfileIdentity,
    DuplicateExactAllowance {
        provider_plan_report_identity: u64,
        executable_identity: ExecutableIdentity,
    },
    ScopeMismatch {
        profile: ExecutionScope,
        manifest: ExecutionScope,
    },
    EntryNotAllowed {
        provider_plan_report_identity: u64,
        executable_identity: ExecutableIdentity,
    },
    MissingContainment {
        provider_plan_report_identity: u64,
        executable_identity: ExecutableIdentity,
        guarantee: ContainmentGuarantee,
    },
    IncompleteScope {
        scope: ExecutionScope,
        causes: Vec<IncompleteCause>,
    },
}

pub fn evaluate_executable_tcb_profile(
    manifest: &ExecutableTcbManifest,
    profile: &ExecutableTcbProfile,
) -> Result<ExecutableTcbProfileAcceptance, ExecutableTcbProfileRejection> {
    let mut violations = validate_profile(profile);
    let manifest_scope = manifest_scope(&manifest.completeness);
    if manifest_scope != profile.scope {
        violations.push(ExecutableTcbProfileViolation::ScopeMismatch {
            profile: profile.scope,
            manifest: manifest_scope,
        });
    }

    for entry in &manifest.known_entries {
        if is_allowed_local_checked_entry(entry, profile) {
            continue;
        }
        let Some(allowance) = profile.exact_allowances.iter().find(|allowance| {
            allowance.provider_identity == entry.provider_identity
                && allowance.provider_plan_report_identity == entry.provider_plan_report_identity
                && allowance.provider_plan_digest == entry.provider_plan_digest
                && allowance.selected_requirement == entry.selected_requirement
                && allowance.executable_identity == entry.executable_identity
                && allowance.implementation_evidence == entry.implementation_evidence
                && allowance.origin == entry.origin
                && allowance.execution_scope == entry.execution_scope
        }) else {
            violations.push(ExecutableTcbProfileViolation::EntryNotAllowed {
                provider_plan_report_identity: entry.provider_plan_report_identity,
                executable_identity: entry.executable_identity.clone(),
            });
            continue;
        };
        for guarantee in &allowance.required_containment {
            if !entry
                .containment
                .iter()
                .any(|evidence| evidence.guarantee == *guarantee)
            {
                violations.push(ExecutableTcbProfileViolation::MissingContainment {
                    provider_plan_report_identity: entry.provider_plan_report_identity,
                    executable_identity: entry.executable_identity.clone(),
                    guarantee: *guarantee,
                });
            }
        }
    }

    let marked_incomplete_causes = match &manifest.completeness {
        ScopeCompleteness::Complete { .. } => Vec::new(),
        ScopeCompleteness::Incomplete { scope, causes, .. } => {
            if profile.incomplete_scope == IncompleteScopePolicy::Reject {
                violations.push(ExecutableTcbProfileViolation::IncompleteScope {
                    scope: *scope,
                    causes: causes.clone(),
                });
                Vec::new()
            } else {
                causes.clone()
            }
        }
    };

    if violations.is_empty() {
        Ok(ExecutableTcbProfileAcceptance {
            profile: profile.clone(),
            manifest: manifest.clone(),
            marked_incomplete_causes,
        })
    } else {
        Err(ExecutableTcbProfileRejection {
            profile: profile.name.clone(),
            violations,
        })
    }
}

fn validate_profile(profile: &ExecutableTcbProfile) -> Vec<ExecutableTcbProfileViolation> {
    let mut violations = Vec::new();
    if profile.name.trim().is_empty() {
        violations.push(ExecutableTcbProfileViolation::EmptyProfileIdentity);
    }
    for (index, allowance) in profile.exact_allowances.iter().enumerate() {
        if profile.exact_allowances[..index]
            .iter()
            .any(|earlier| same_allowance_subject(earlier, allowance))
        {
            violations.push(ExecutableTcbProfileViolation::DuplicateExactAllowance {
                provider_plan_report_identity: allowance.provider_plan_report_identity,
                executable_identity: allowance.executable_identity.clone(),
            });
        }
    }
    violations
}

fn same_allowance_subject(
    left: &ExactExecutableTcbAllowance,
    right: &ExactExecutableTcbAllowance,
) -> bool {
    left.provider_identity == right.provider_identity
        && left.provider_plan_report_identity == right.provider_plan_report_identity
        && left.provider_plan_digest == right.provider_plan_digest
        && left.selected_requirement == right.selected_requirement
        && left.executable_identity == right.executable_identity
        && left.implementation_evidence == right.implementation_evidence
        && left.origin == right.origin
        && left.execution_scope == right.execution_scope
}

fn is_allowed_local_checked_entry(
    entry: &ExecutableTcbEntry,
    profile: &ExecutableTcbProfile,
) -> bool {
    profile.allow_static_current_artifact_checked_bodies
        && entry.origin == ExecutableEntryOrigin::StaticSelection
        && entry.execution_scope == profile.scope
        && matches!(
            (&entry.executable_identity, &entry.implementation_evidence),
            (
                ExecutableIdentity::CurrentArtifactMachine(executable),
                ImplementationEvidence::CheckedBody { machine },
            ) if executable == machine
        )
}

const fn manifest_scope(completeness: &ScopeCompleteness) -> ExecutionScope {
    match completeness {
        ScopeCompleteness::Complete { scope, .. } | ScopeCompleteness::Incomplete { scope, .. } => {
            *scope
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContainmentEvidence, OpaqueInProcessBinding};

    fn opaque_entry() -> ExecutableTcbEntry {
        ExecutableTcbEntry {
            provider_identity: ProviderIdentity::NominalType("PlatformProvider".into()),
            provider_plan_report_identity: 7,
            provider_plan_digest: ProviderPlan::default().identity_digest(),
            selected_requirement: None,
            executable_identity: ExecutableIdentity::PinnedOpaqueArtifact(
                "platform-baseline:window-v1".into(),
            ),
            implementation_evidence: ImplementationEvidence::AdmittedOpaque {
                receipt_identity: "receipt:window-v1".into(),
            },
            origin: ExecutableEntryOrigin::StaticSelection,
            execution_scope: ExecutionScope::CallerAddressSpace,
            containment: vec![ContainmentEvidence {
                guarantee: ContainmentGuarantee::FaultContainment,
                evidence_identity: "receipt:fault-v1".into(),
            }],
        }
    }

    fn allowance(entry: &ExecutableTcbEntry) -> ExactExecutableTcbAllowance {
        ExactExecutableTcbAllowance {
            provider_identity: entry.provider_identity.clone(),
            provider_plan_report_identity: entry.provider_plan_report_identity,
            provider_plan_digest: entry.provider_plan_digest,
            selected_requirement: entry.selected_requirement.clone(),
            executable_identity: entry.executable_identity.clone(),
            implementation_evidence: entry.implementation_evidence.clone(),
            origin: entry.origin,
            execution_scope: entry.execution_scope,
            required_containment: vec![ContainmentGuarantee::FaultContainment],
        }
    }

    fn profile(entry: &ExecutableTcbEntry) -> ExecutableTcbProfile {
        ExecutableTcbProfile {
            name: "safety".into(),
            scope: ExecutionScope::CallerAddressSpace,
            allow_static_current_artifact_checked_bodies: true,
            exact_allowances: vec![allowance(entry)],
            incomplete_scope: IncompleteScopePolicy::Reject,
        }
    }

    fn complete_manifest(entry: ExecutableTcbEntry) -> ExecutableTcbManifest {
        ExecutableTcbManifest {
            known_entries: vec![entry],
            completeness: ScopeCompleteness::Complete {
                scope: ExecutionScope::CallerAddressSpace,
                selected_provider_closure_report_identity: 9,
                opaque_closure_evidence: Vec::new(),
                runtime_closure_evidence: Vec::new(),
            },
        }
    }

    #[test]
    fn exact_allowance_and_containment_accept_before_installation() {
        let entry = opaque_entry();
        let accepted =
            evaluate_executable_tcb_profile(&complete_manifest(entry.clone()), &profile(&entry))
                .expect("exact profile");

        assert_eq!(accepted.known_entry_count(), 1);
        assert!(!accepted.is_marked_incomplete());
    }

    #[test]
    fn identity_evidence_and_containment_drift_reject() {
        let entry = opaque_entry();
        let mut compact_equal_wrong_digest = profile(&entry);
        let mut structurally_different_plan = ProviderPlan::default();
        structurally_different_plan.name = "different-plan".into();
        compact_equal_wrong_digest.exact_allowances[0].provider_plan_digest =
            structurally_different_plan.identity_digest();
        let rejected = evaluate_executable_tcb_profile(
            &complete_manifest(entry.clone()),
            &compact_equal_wrong_digest,
        )
        .expect_err("compact-equal allowance with the wrong plan digest");
        assert!(matches!(
            rejected.violations.as_slice(),
            [ExecutableTcbProfileViolation::EntryNotAllowed { .. }]
        ));

        let mut wrong_identity = profile(&entry);
        wrong_identity.exact_allowances[0].executable_identity =
            ExecutableIdentity::PinnedOpaqueArtifact("platform-baseline:other".into());
        let rejected =
            evaluate_executable_tcb_profile(&complete_manifest(entry.clone()), &wrong_identity)
                .expect_err("identity drift");
        assert!(matches!(
            rejected.violations.as_slice(),
            [ExecutableTcbProfileViolation::EntryNotAllowed { .. }]
        ));

        let mut missing_containment = profile(&entry);
        missing_containment.exact_allowances[0].required_containment =
            vec![ContainmentGuarantee::BoundedResources];
        let rejected =
            evaluate_executable_tcb_profile(&complete_manifest(entry), &missing_containment)
                .expect_err("missing containment");
        assert!(matches!(
            rejected.violations.as_slice(),
            [ExecutableTcbProfileViolation::MissingContainment { .. }]
        ));
    }

    #[test]
    fn exact_allowance_rejects_selected_requirement_identity_drift() {
        let mut entry = opaque_entry();
        entry.selected_requirement = Some(SelectedProviderRequirement {
            method: "convert".into(),
            requirement_identity: "named-callable:path=Convert::convert;result=Ordinary".into(),
        });
        let mut drifted = profile(&entry);
        drifted.exact_allowances[0]
            .selected_requirement
            .as_mut()
            .expect("selected requirement allowance")
            .method = "readable-label-drift".into();
        evaluate_executable_tcb_profile(&complete_manifest(entry.clone()), &drifted)
            .expect("readable method is not the selected overload identity");
        drifted.exact_allowances[0]
            .selected_requirement
            .as_mut()
            .expect("selected requirement allowance")
            .requirement_identity = "named-callable:path=Convert::convert;result=Saturating".into();

        let rejected = evaluate_executable_tcb_profile(&complete_manifest(entry), &drifted)
            .expect_err("same readable method cannot authorize another overload identity");
        assert!(matches!(
            rejected.violations.as_slice(),
            [ExecutableTcbProfileViolation::EntryNotAllowed { .. }]
        ));
    }

    #[test]
    fn static_allowance_cannot_launder_a_runtime_admission_origin() {
        let mut entry = opaque_entry();
        entry.origin = ExecutableEntryOrigin::OmegaRuntimeAdmission;
        let mut runtime_profile = profile(&entry);
        evaluate_executable_tcb_profile(&complete_manifest(entry.clone()), &runtime_profile)
            .expect("exact runtime-origin allowance");

        runtime_profile.exact_allowances[0].origin = ExecutableEntryOrigin::StaticSelection;
        let rejected = evaluate_executable_tcb_profile(&complete_manifest(entry), &runtime_profile)
            .expect_err("static allowance must not accept runtime admission");
        assert!(matches!(
            rejected.violations.as_slice(),
            [ExecutableTcbProfileViolation::EntryNotAllowed { .. }]
        ));
    }

    #[test]
    fn incomplete_scope_is_rejected_or_permitted_and_marked() {
        let entry = opaque_entry();
        let cause = IncompleteCause::SelectedOpaqueProvider {
            provider_identity: entry.provider_identity.clone(),
            provider_plan_report_identity: entry.provider_plan_report_identity,
            provider_plan_digest: entry.provider_plan_digest,
            method: "open".into(),
            requirement_identity: "Window::open".into(),
            binding: OpaqueInProcessBinding::StringBackedImportBootstrap {
                library: "platform".into(),
                symbol: "open".into(),
            },
        };
        let manifest = ExecutableTcbManifest {
            known_entries: vec![entry.clone()],
            completeness: ScopeCompleteness::Incomplete {
                scope: ExecutionScope::CallerAddressSpace,
                causes: vec![cause.clone()],
                opaque_closure_evidence: Vec::new(),
                runtime_closure_evidence: Vec::new(),
            },
        };
        let rejected = evaluate_executable_tcb_profile(&manifest, &profile(&entry))
            .expect_err("safety profile rejects incomplete scope");
        assert!(rejected.violations.iter().any(|violation| matches!(
            violation,
            ExecutableTcbProfileViolation::IncompleteScope { .. }
        )));

        let mut development = profile(&entry);
        development.name = "development".into();
        development.incomplete_scope = IncompleteScopePolicy::PermitAndMark;
        let accepted = evaluate_executable_tcb_profile(&manifest, &development)
            .expect("development profile marks incompleteness");
        assert_eq!(accepted.marked_incomplete_causes(), &[cause]);
    }

    #[test]
    fn local_checked_body_requires_only_the_explicit_class_rule() {
        let checked = ExecutableTcbEntry {
            provider_identity: ProviderIdentity::NominalType("CheckedProvider".into()),
            provider_plan_report_identity: 11,
            provider_plan_digest: ProviderPlan::default().identity_digest(),
            selected_requirement: Some(SelectedProviderRequirement {
                method: "run".into(),
                requirement_identity: "named-callable(path:CheckedService::run)".into(),
            }),
            executable_identity: ExecutableIdentity::CurrentArtifactMachine(
                "CheckedProvider::run".into(),
            ),
            implementation_evidence: ImplementationEvidence::CheckedBody {
                machine: "CheckedProvider::run".into(),
            },
            origin: ExecutableEntryOrigin::StaticSelection,
            execution_scope: ExecutionScope::CallerAddressSpace,
            containment: Vec::new(),
        };
        let profile = ExecutableTcbProfile {
            name: "checked".into(),
            scope: ExecutionScope::CallerAddressSpace,
            allow_static_current_artifact_checked_bodies: true,
            exact_allowances: Vec::new(),
            incomplete_scope: IncompleteScopePolicy::Reject,
        };

        evaluate_executable_tcb_profile(&complete_manifest(checked), &profile)
            .expect("checked current-artifact class is explicitly allowed");
    }
}
