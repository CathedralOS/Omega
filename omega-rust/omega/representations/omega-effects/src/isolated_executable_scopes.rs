#[cfg(test)]
use crate::provider_plan::ProviderPlan;
use crate::provider_plan::ProviderPlanDigest;
use crate::{
    ContainmentEvidence, ExecutableEntryOrigin, ExecutableIdentity, ExecutableTcbEntry,
    ExecutableTcbManifest, ExecutionScope, ImplementationEvidence, ProviderIdentity,
    ScopeCompleteness,
};

/// Deployment admission joining one caller-visible endpoint to the separate
/// executable manifest for the isolated provider scope behind it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsolatedExecutableScopeCandidate {
    pub provider_identity: ProviderIdentity,
    pub provider_plan_report_identity: u64,
    pub provider_plan_digest: ProviderPlanDigest,
    pub endpoint_identity: String,
    pub endpoint_receipt_identity: String,
    pub isolated_manifest_receipt_identity: String,
    pub isolated_scope_identity: u64,
    pub containment: Vec<ContainmentEvidence>,
    pub isolated_manifest: ExecutableTcbManifest,
}

/// Sealed endpoint/manifest pair retained by a scope set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedIsolatedExecutableScope {
    endpoint: ExecutableTcbEntry,
    manifest_receipt_identity: String,
    manifest: ExecutableTcbManifest,
}

impl AdmittedIsolatedExecutableScope {
    pub const fn endpoint(&self) -> &ExecutableTcbEntry {
        &self.endpoint
    }

    pub const fn manifest_receipt_identity(&self) -> &str {
        self.manifest_receipt_identity.as_str()
    }

    pub const fn manifest(&self) -> &ExecutableTcbManifest {
        &self.manifest
    }

    pub const fn scope(&self) -> ExecutionScope {
        manifest_scope(&self.manifest)
    }
}

/// One root/caller manifest plus separately evaluated isolated-scope
/// manifests. Child incompleteness never contaminates the parent's scope; the
/// parent retains the exact endpoint and containment evidence instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableTcbManifestSet {
    root: ExecutableTcbManifest,
    isolated: Vec<AdmittedIsolatedExecutableScope>,
}

impl ExecutableTcbManifestSet {
    pub fn new(root: ExecutableTcbManifest) -> Result<Self, String> {
        validate_manifest_scope(&root)?;
        Ok(Self {
            root,
            isolated: Vec::new(),
        })
    }

    pub const fn root(&self) -> &ExecutableTcbManifest {
        &self.root
    }

    pub fn isolated(&self) -> &[AdmittedIsolatedExecutableScope] {
        &self.isolated
    }

    pub fn attach_isolated_scope(
        &mut self,
        mut candidate: IsolatedExecutableScopeCandidate,
    ) -> Result<(), String> {
        validate_candidate(&mut candidate)?;
        let isolated_scope = ExecutionScope::IsolatedProvider(candidate.isolated_scope_identity);
        if manifest_scope(&candidate.isolated_manifest) != isolated_scope {
            return Err(format!(
                "isolated executable manifest scope {:?} does not match admitted scope {:?}",
                manifest_scope(&candidate.isolated_manifest),
                isolated_scope
            ));
        }
        validate_manifest_scope(&candidate.isolated_manifest)?;
        if self
            .isolated
            .iter()
            .any(|admitted| admitted.scope() == isolated_scope)
        {
            return Err(format!(
                "isolated executable scope {:#018x} is attached more than once",
                candidate.isolated_scope_identity
            ));
        }

        let parent_scope = manifest_scope(&self.root);
        let endpoint = ExecutableTcbEntry {
            provider_identity: candidate.provider_identity,
            provider_plan_report_identity: candidate.provider_plan_report_identity,
            provider_plan_digest: candidate.provider_plan_digest,
            selected_requirement: None,
            executable_identity: ExecutableIdentity::IsolatedProviderEndpoint {
                scope_identity: candidate.isolated_scope_identity,
                endpoint_identity: candidate.endpoint_identity,
            },
            implementation_evidence: ImplementationEvidence::AdmittedIsolatedEndpoint {
                endpoint_receipt_identity: candidate.endpoint_receipt_identity,
                isolated_manifest_receipt_identity: candidate
                    .isolated_manifest_receipt_identity
                    .clone(),
            },
            origin: ExecutableEntryOrigin::StaticSelection,
            execution_scope: parent_scope,
            containment: candidate.containment,
        };
        if self.root.known_entries.contains(&endpoint) {
            return Err("isolated provider endpoint is attached more than once".into());
        }
        self.root.known_entries.push(endpoint.clone());
        self.isolated.push(AdmittedIsolatedExecutableScope {
            endpoint,
            manifest_receipt_identity: candidate.isolated_manifest_receipt_identity,
            manifest: candidate.isolated_manifest,
        });
        self.isolated
            .sort_by_key(|admitted| match admitted.scope() {
                ExecutionScope::IsolatedProvider(identity) => identity,
                ExecutionScope::CallerAddressSpace => 0,
            });
        Ok(())
    }
}

fn validate_candidate(candidate: &mut IsolatedExecutableScopeCandidate) -> Result<(), String> {
    let provider_name = match &candidate.provider_identity {
        ProviderIdentity::NominalType(name) | ProviderIdentity::FreeExternalPlan(name) => name,
    };
    if provider_name.trim().is_empty() {
        return Err("isolated executable scope has no provider identity".into());
    }
    if candidate.provider_plan_report_identity == 0 {
        return Err(
            "isolated executable scope has the reserved zero provider-plan identity".into(),
        );
    }
    if candidate
        .provider_plan_digest
        .as_bytes()
        .iter()
        .all(|byte| *byte == 0)
    {
        return Err("isolated executable scope has a zero provider-plan digest".into());
    }
    if candidate.isolated_scope_identity == 0 {
        return Err("isolated executable scope has the reserved zero scope identity".into());
    }
    if candidate.endpoint_identity.trim().is_empty() {
        return Err("isolated executable scope has no endpoint identity".into());
    }
    if candidate.endpoint_receipt_identity.trim().is_empty() {
        return Err("isolated executable scope has no endpoint admission receipt".into());
    }
    if candidate
        .isolated_manifest_receipt_identity
        .trim()
        .is_empty()
    {
        return Err("isolated executable scope has no manifest admission receipt".into());
    }
    candidate.containment.sort_by(|left, right| {
        left.guarantee
            .cmp(&right.guarantee)
            .then_with(|| left.evidence_identity.cmp(&right.evidence_identity))
    });
    if candidate
        .containment
        .iter()
        .any(|evidence| evidence.evidence_identity.trim().is_empty())
    {
        return Err("isolated executable scope has empty containment evidence".into());
    }
    if candidate
        .containment
        .windows(2)
        .any(|pair| pair[0].guarantee == pair[1].guarantee)
    {
        return Err(
            "isolated executable scope repeats one containment guarantee; each axis needs one exact result"
                .into(),
        );
    }
    Ok(())
}

fn validate_manifest_scope(manifest: &ExecutableTcbManifest) -> Result<(), String> {
    let scope = manifest_scope(manifest);
    if matches!(scope, ExecutionScope::IsolatedProvider(0)) {
        return Err("executable manifest has the reserved zero isolated-scope identity".into());
    }
    if let Some(entry) = manifest
        .known_entries
        .iter()
        .find(|entry| entry.execution_scope != scope)
    {
        return Err(format!(
            "executable manifest entry {:?} belongs to scope {:?}, not manifest scope {:?}",
            entry.executable_identity, entry.execution_scope, scope
        ));
    }
    Ok(())
}

const fn manifest_scope(manifest: &ExecutableTcbManifest) -> ExecutionScope {
    match &manifest.completeness {
        ScopeCompleteness::Complete { scope, .. } | ScopeCompleteness::Incomplete { scope, .. } => {
            *scope
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ContainmentGuarantee, ExactExecutableTcbAllowance, ExecutableTcbProfile,
        IncompleteScopePolicy, SelectedProviderPlanFacts, evaluate_executable_tcb_profile,
    };

    fn isolated_manifest(scope_identity: u64) -> ExecutableTcbManifest {
        SelectedProviderPlanFacts::default()
            .with_execution_scope(ExecutionScope::IsolatedProvider(scope_identity))
            .expect("nonzero isolated scope")
            .executable_tcb_manifest()
    }

    fn candidate(scope_identity: u64) -> IsolatedExecutableScopeCandidate {
        IsolatedExecutableScopeCandidate {
            provider_identity: ProviderIdentity::NominalType("SandboxedCodec".into()),
            provider_plan_report_identity: 77,
            provider_plan_digest: ProviderPlan::default().identity_digest(),
            endpoint_identity: "endpoint:codec-v1".into(),
            endpoint_receipt_identity: "receipt:endpoint-v1".into(),
            isolated_manifest_receipt_identity: "receipt:manifest-v1".into(),
            isolated_scope_identity: scope_identity,
            containment: vec![ContainmentEvidence {
                guarantee: ContainmentGuarantee::MemoryIsolation,
                evidence_identity: "receipt:memory-isolation-v1".into(),
            }],
            isolated_manifest: isolated_manifest(scope_identity),
        }
    }

    #[test]
    fn caller_retains_endpoint_and_child_keeps_a_separate_manifest() {
        let root = SelectedProviderPlanFacts::default().executable_tcb_manifest();
        let mut manifests = ExecutableTcbManifestSet::new(root).expect("caller manifest");
        manifests
            .attach_isolated_scope(candidate(101))
            .expect("exact isolated scope");

        assert_eq!(manifests.root().known_entries.len(), 1);
        assert!(matches!(
            manifests.root().known_entries[0].executable_identity,
            ExecutableIdentity::IsolatedProviderEndpoint {
                scope_identity: 101,
                ..
            }
        ));
        assert_eq!(manifests.isolated().len(), 1);
        assert_eq!(
            manifests.isolated()[0].scope(),
            ExecutionScope::IsolatedProvider(101)
        );
    }

    #[test]
    fn child_incompleteness_does_not_change_parent_completeness() {
        let root = SelectedProviderPlanFacts::default().executable_tcb_manifest();
        let mut candidate = candidate(202);
        candidate.isolated_manifest.completeness = ScopeCompleteness::Incomplete {
            scope: ExecutionScope::IsolatedProvider(202),
            causes: Vec::new(),
            opaque_closure_evidence: Vec::new(),
            runtime_closure_evidence: Vec::new(),
        };
        let mut manifests = ExecutableTcbManifestSet::new(root).expect("caller manifest");
        manifests
            .attach_isolated_scope(candidate)
            .expect("incomplete child is separately reportable");

        assert!(matches!(
            manifests.root().completeness,
            ScopeCompleteness::Complete { .. }
        ));
        assert!(matches!(
            manifests.isolated()[0].manifest().completeness,
            ScopeCompleteness::Incomplete { .. }
        ));
    }

    #[test]
    fn attachment_rejects_scope_drift_and_duplicate_scope_identity() {
        let root = SelectedProviderPlanFacts::default().executable_tcb_manifest();
        let mut manifests = ExecutableTcbManifestSet::new(root).expect("caller manifest");
        let mut drifted = candidate(303);
        drifted.isolated_scope_identity = 304;
        assert!(
            manifests
                .attach_isolated_scope(drifted)
                .expect_err("scope drift")
                .contains("does not match admitted scope")
        );

        manifests
            .attach_isolated_scope(candidate(303))
            .expect("first exact child");
        assert!(
            manifests
                .attach_isolated_scope(candidate(303))
                .expect_err("duplicate child scope")
                .contains("attached more than once")
        );
    }

    #[test]
    fn profiles_evaluate_parent_endpoint_and_child_scope_independently() {
        let root = SelectedProviderPlanFacts::default().executable_tcb_manifest();
        let mut child = candidate(404);
        child.isolated_manifest.completeness = ScopeCompleteness::Incomplete {
            scope: ExecutionScope::IsolatedProvider(404),
            causes: Vec::new(),
            opaque_closure_evidence: Vec::new(),
            runtime_closure_evidence: Vec::new(),
        };
        let mut manifests = ExecutableTcbManifestSet::new(root).expect("caller manifest");
        manifests
            .attach_isolated_scope(child)
            .expect("separate child scope");
        let endpoint = &manifests.root().known_entries[0];
        let root_profile = ExecutableTcbProfile {
            name: "caller".into(),
            scope: ExecutionScope::CallerAddressSpace,
            allow_static_current_artifact_checked_bodies: true,
            exact_allowances: vec![ExactExecutableTcbAllowance {
                provider_identity: endpoint.provider_identity.clone(),
                provider_plan_report_identity: endpoint.provider_plan_report_identity,
                provider_plan_digest: endpoint.provider_plan_digest,
                selected_requirement: endpoint.selected_requirement.clone(),
                executable_identity: endpoint.executable_identity.clone(),
                implementation_evidence: endpoint.implementation_evidence.clone(),
                origin: endpoint.origin,
                execution_scope: endpoint.execution_scope,
                required_containment: vec![ContainmentGuarantee::MemoryIsolation],
            }],
            incomplete_scope: IncompleteScopePolicy::Reject,
        };
        evaluate_executable_tcb_profile(manifests.root(), &root_profile)
            .expect("child incompleteness does not contaminate caller profile");

        let child_profile = ExecutableTcbProfile {
            name: "isolated-child".into(),
            scope: ExecutionScope::IsolatedProvider(404),
            allow_static_current_artifact_checked_bodies: true,
            exact_allowances: Vec::new(),
            incomplete_scope: IncompleteScopePolicy::Reject,
        };
        assert!(
            evaluate_executable_tcb_profile(manifests.isolated()[0].manifest(), &child_profile)
                .is_err()
        );
    }
}
