//! Scopes admitted as isolated, and the manifest set that covers them.

use crate::provider_plan::ProviderPlanDigest;
use crate::{
    ContainmentEvidence, ExecutableTcbEntry, ExecutableTcbManifest, ExecutionScope,
    ProviderIdentity, ScopeCompleteness,
};

/// Deployment admission joining one caller-visible endpoint to the separate
/// executable manifest for the isolated provider scope behind it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IsolatedExecutableScopeCandidate {
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
pub(crate) struct AdmittedIsolatedExecutableScope {
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
pub(crate) struct ExecutableTcbManifestSet {
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
