//! The allowance a scope is judged against, and what an exact violation of it looks
//! like.

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
