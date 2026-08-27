#![forbid(unsafe_code)]

//! Authority-free handoff from terminal compilation to component deployment.
//!
//! This crate owns only the non-visible compiler candidate and its consuming
//! decomposition. A candidate carries checked identities and artifact bytes,
//! but grants no installation, provider-occurrence, progress-establishment,
//! installed-code, filesystem, or publication authority. Keeping this carrier
//! below both `omega-compiler` and `omega-component-deployment` lets the
//! compiler hand it to the deployment owner without a dependency cycle.

use omega_terminal_installation_evidence::TerminalProviderExecutionEvidence;

/// One exact admitted provider execution selected for a staged terminal
/// component. This is an owned identity projection, not a provider occurrence
/// or an installation receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalComponentProviderExecution {
    requirement_identity: String,
    provider_plan: u64,
    provider_execution_identity: u64,
    provider_execution_fingerprint: u64,
    normalized_root_identity: u64,
    boundary_contract_fingerprint: u64,
}

impl TerminalComponentProviderExecution {
    /// Copy the authority-free identity projection from admitted provider
    /// execution evidence. The evidence itself remains with its owner.
    pub fn from_evidence(evidence: &dyn TerminalProviderExecutionEvidence) -> Self {
        Self {
            requirement_identity: evidence.requirement_identity().to_owned(),
            provider_plan: evidence.provider_plan(),
            provider_execution_identity: evidence.provider_execution_identity(),
            provider_execution_fingerprint: evidence.provider_execution_fingerprint(),
            normalized_root_identity: evidence.normalized_root_identity(),
            boundary_contract_fingerprint: evidence.boundary_contract_fingerprint(),
        }
    }
}

impl TerminalProviderExecutionEvidence for TerminalComponentProviderExecution {
    fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    fn provider_plan(&self) -> u64 {
        self.provider_plan
    }

    fn provider_execution_identity(&self) -> u64 {
        self.provider_execution_identity
    }

    fn provider_execution_fingerprint(&self) -> u64 {
        self.provider_execution_fingerprint
    }

    fn normalized_root_identity(&self) -> u64 {
        self.normalized_root_identity
    }

    fn boundary_contract_fingerprint(&self) -> u64 {
        self.boundary_contract_fingerprint
    }
}

/// A source-independent, non-visible terminal component candidate.
///
/// The candidate retains everything compilation can honestly establish. It
/// contains no output path, visibility receipt, installed-code claim, provider
/// occurrence, or progress-establishment receipt; those belong to deployment.
#[derive(Debug)]
pub struct TerminalComponentCandidate {
    target: omega_target::NativeTarget,
    entry_machine: String,
    terminal_artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    object: omega_terminal_image_emission::TerminalObjectArtifact,
    image: omega_terminal_image_emission::TerminalExecutableImage,
    selected_provider_plans: omega_effects::SelectedProviderPlanFacts,
    provider_executions: Vec<TerminalComponentProviderExecution>,
    component_progress: Option<omega_effects::ComponentProgressManifest>,
}

/// Complete owned compiler output transferred to deployment.
///
/// These parts grant no installation or publication authority. Deployment
/// must still bind them to real provider occurrences and one exact
/// installed-code occurrence.
#[derive(Debug)]
pub struct TerminalComponentCandidateParts {
    pub target: omega_target::NativeTarget,
    pub entry_machine: String,
    pub terminal_artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    pub object: omega_terminal_image_emission::TerminalObjectArtifact,
    pub image: omega_terminal_image_emission::TerminalExecutableImage,
    pub selected_provider_plans: omega_effects::SelectedProviderPlanFacts,
    pub provider_executions: Vec<TerminalComponentProviderExecution>,
    pub component_progress: Option<omega_effects::ComponentProgressManifest>,
}

impl TerminalComponentCandidate {
    /// Seal one complete authority-free staging result into its consuming
    /// handoff carrier. Semantic and artifact validation remains the staging
    /// producer's responsibility; deployment independently replays the joins
    /// before any registry claim or publication.
    pub fn from_parts(parts: TerminalComponentCandidateParts) -> Self {
        Self {
            target: parts.target,
            entry_machine: parts.entry_machine,
            terminal_artifact: parts.terminal_artifact,
            object: parts.object,
            image: parts.image,
            selected_provider_plans: parts.selected_provider_plans,
            provider_executions: parts.provider_executions,
            component_progress: parts.component_progress,
        }
    }

    pub const fn target(&self) -> omega_target::NativeTarget {
        self.target
    }

    pub fn entry_machine(&self) -> &str {
        &self.entry_machine
    }

    pub fn semantic_bytes(&self) -> &[u8] {
        self.terminal_artifact.semantic_bytes()
    }

    pub fn proof_bytes(&self) -> &[u8] {
        self.terminal_artifact.proof_bytes()
    }

    pub const fn terminal_artifact(&self) -> &psi_terminal_codec::CanonicalTerminalArtifact {
        &self.terminal_artifact
    }

    pub const fn object(&self) -> &omega_terminal_image_emission::TerminalObjectArtifact {
        &self.object
    }

    pub const fn image(&self) -> &omega_terminal_image_emission::TerminalExecutableImage {
        &self.image
    }

    pub const fn selected_provider_plans(&self) -> &omega_effects::SelectedProviderPlanFacts {
        &self.selected_provider_plans
    }

    pub fn provider_executions(&self) -> &[TerminalComponentProviderExecution] {
        &self.provider_executions
    }

    pub const fn component_progress(&self) -> Option<&omega_effects::ComponentProgressManifest> {
        self.component_progress.as_ref()
    }

    /// Transfer the complete non-visible compiler candidate into deployment.
    pub fn into_parts(self) -> TerminalComponentCandidateParts {
        TerminalComponentCandidateParts {
            target: self.target,
            entry_machine: self.entry_machine,
            terminal_artifact: self.terminal_artifact,
            object: self.object,
            image: self.image,
            selected_provider_plans: self.selected_provider_plans,
            provider_executions: self.provider_executions,
            component_progress: self.component_progress,
        }
    }
}
