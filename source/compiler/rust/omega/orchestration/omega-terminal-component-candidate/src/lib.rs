#![forbid(unsafe_code)]

//! Authority-free handoff from Terminal native realization to component
//! deployment.
//!
//! The universal native artifact lives in `omega-terminal-native-artifact`.
//! This crate adds only component entry identity, the complete source-derived
//! selected provider-plan facts needed by deployment, and any build-bound
//! progress manifest.

pub use omega_terminal_native_artifact::{
    TerminalNativeArtifact, TerminalNativeArtifactParts, TerminalNativeProviderExecution,
    TerminalNativeSelectedProviderPlan,
};

pub type TerminalComponentProviderExecution = TerminalNativeProviderExecution;

#[derive(Debug)]
pub struct TerminalComponentCandidate {
    native_artifact: TerminalNativeArtifact,
    entry_machine: String,
    selected_provider_plans: omega_effects::SelectedProviderPlanFacts,
    component_progress: Option<omega_effects::ComponentProgressManifest>,
}

#[derive(Debug)]
pub struct TerminalComponentCandidateParts {
    pub native_artifact: TerminalNativeArtifact,
    pub entry_machine: String,
    pub selected_provider_plans: omega_effects::SelectedProviderPlanFacts,
    pub component_progress: Option<omega_effects::ComponentProgressManifest>,
}

impl TerminalComponentCandidate {
    /// Rejoin component policy to one already replayed native artifact.
    pub fn checked(parts: TerminalComponentCandidateParts) -> Result<Self, &'static str> {
        parts.native_artifact.validate()?;
        let mut projected = parts
            .selected_provider_plans
            .plans()
            .iter()
            .map(|plan| {
                TerminalNativeSelectedProviderPlan::new(
                    plan.identity_fingerprint(),
                    plan.rows
                        .iter()
                        .map(|row| row.requirement_identity.clone())
                        .collect(),
                )
            })
            .collect::<Vec<_>>();
        projected.sort_by_key(TerminalNativeSelectedProviderPlan::identity);
        if projected != parts.native_artifact.selected_provider_plans() {
            return Err(
                "component candidate selected provider facts disagree with its native artifact",
            );
        }
        if parts
            .component_progress
            .as_ref()
            .is_some_and(|manifest| manifest.pending().is_empty())
        {
            return Err("component candidate retained an empty progress manifest");
        }
        Ok(Self {
            native_artifact: parts.native_artifact,
            entry_machine: parts.entry_machine,
            selected_provider_plans: parts.selected_provider_plans,
            component_progress: parts.component_progress,
        })
    }

    pub const fn target(&self) -> omega_target::NativeTarget {
        self.native_artifact.target()
    }

    pub fn entry_machine(&self) -> &str {
        &self.entry_machine
    }

    pub fn semantic_bytes(&self) -> &[u8] {
        self.native_artifact.semantic_bytes()
    }

    pub fn proof_bytes(&self) -> &[u8] {
        self.native_artifact.proof_bytes()
    }

    pub const fn terminal_artifact(&self) -> &psi_terminal_codec::CanonicalTerminalArtifact {
        self.native_artifact.terminal_artifact()
    }

    pub const fn object(&self) -> &omega_terminal_image_emission::TerminalObjectArtifact {
        self.native_artifact.object()
    }

    pub const fn image(&self) -> &omega_terminal_image_emission::TerminalExecutableImage {
        self.native_artifact.image()
    }

    pub const fn selected_provider_plans(&self) -> &omega_effects::SelectedProviderPlanFacts {
        &self.selected_provider_plans
    }

    pub fn provider_executions(&self) -> &[TerminalNativeProviderExecution] {
        self.native_artifact.provider_executions()
    }

    pub const fn native_artifact(&self) -> &TerminalNativeArtifact {
        &self.native_artifact
    }

    pub const fn component_progress(&self) -> Option<&omega_effects::ComponentProgressManifest> {
        self.component_progress.as_ref()
    }

    pub fn into_parts(self) -> TerminalComponentCandidateParts {
        TerminalComponentCandidateParts {
            native_artifact: self.native_artifact,
            entry_machine: self.entry_machine,
            selected_provider_plans: self.selected_provider_plans,
            component_progress: self.component_progress,
        }
    }
}
