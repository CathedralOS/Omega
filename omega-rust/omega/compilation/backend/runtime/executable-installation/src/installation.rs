/*
We represent the installation transition carriers (`InstallAuthority`,
`InstallationReceipt`, `InstalledCode`, `InstallationError`) and W^X
enforcement status (`WxEnforcement`).

Failed transitions return their linear inputs (`ValidatedPlacement`,
`InstallAuthority`, `InstallationReceipt`) inside `InstallationError` so
custody is never silently dropped.
*/

use crate::authority_digests::{
    ArtifactId, CodePlacementId, FinalValidationId, InstallationFactDigest, InstallationScopeId,
    InstalledCodeId,
};
use crate::placement::ValidatedPlacement;
use crate::placement::ValidatedPlacementEvidence;
use layout_plans::{EntryStubId, PlacementConstraints, RelocationTarget};
use target::Architecture;

#[derive(Debug, PartialEq, Eq)]
pub struct InstallAuthority {
    pub(crate) validated: ValidatedPlacementEvidence,
    pub(crate) required_facts: std::collections::BTreeSet<InstallationFactDigest>, // Provider-canonical claims required in addition to mandatory visibility and W^X gates.
}

impl InstallAuthority {
    pub fn from_admitted_provider(validated: &ValidatedPlacement) -> Self {
        Self {
            validated: ValidatedPlacementEvidence::from_validated(validated),
            required_facts: std::collections::BTreeSet::new(),
        }
    }

    pub fn with_required_facts(
        mut self,
        required_facts: impl IntoIterator<Item = InstallationFactDigest>,
    ) -> Self {
        self.required_facts = required_facts.into_iter().collect();
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WxEnforcement {
    HardwareEnforced,
    ConventionOnly,
    Unsupported,
}

#[derive(Debug, PartialEq, Eq)]
pub struct InstallationReceipt {
    pub(crate) installed: InstalledCodeId,
    pub(crate) validated: ValidatedPlacementEvidence,
    pub(crate) visibility_complete: bool,
    pub(crate) wx: WxEnforcement,
    pub(crate) established_facts: std::collections::BTreeSet<InstallationFactDigest>,
}

impl InstallationReceipt {
    pub fn from_provider(
        installed: InstalledCodeId,
        validated: &ValidatedPlacement,
        visibility_complete: bool,
        wx: WxEnforcement,
    ) -> Self {
        Self {
            installed,
            validated: ValidatedPlacementEvidence::from_validated(validated),
            visibility_complete,
            wx,
            established_facts: std::collections::BTreeSet::new(),
        }
    }

    pub fn with_established_facts(
        mut self,
        established_facts: impl IntoIterator<Item = InstallationFactDigest>,
    ) -> Self {
        self.established_facts = established_facts.into_iter().collect();
        self
    }
}

#[derive(Debug)]
pub struct InstallationError {
    pub(crate) validated: ValidatedPlacement,
    pub(crate) authority: InstallAuthority,
    pub(crate) receipt: InstallationReceipt,
    pub(crate) diagnostic: InstallationDiagnostic,
}

impl InstallationError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedPlacement, InstallAuthority, InstallationReceipt) {
        (self.validated, self.authority, self.receipt)
    }
}

#[derive(Debug)]
pub struct InstalledCode {
    pub(crate) identity: InstalledCodeId,
    pub(crate) validated: ValidatedPlacement,
    pub(crate) wx: WxEnforcement,
}

impl InstalledCode {
    pub fn artifact(&self) -> ArtifactId {
        self.validated.frozen.artifact.artifact.0.identity
    }

    pub const fn installation_scope(&self) -> InstallationScopeId {
        self.validated.frozen.placement.scope
    }

    pub const fn placement_constraints(&self) -> PlacementConstraints {
        self.validated.frozen.placement.constraints
    }

    pub fn architecture(&self) -> Architecture {
        self.validated.frozen.artifact.artifact.0.architecture
    }

    pub const fn placement(&self) -> CodePlacementId {
        self.validated.frozen.placement.placement
    }

    pub const fn validation(&self) -> FinalValidationId {
        self.validated.validation
    }

    pub const fn wx(&self) -> WxEnforcement {
        self.wx
    }

    pub fn selected_entry_target(
        &self,
        identity: EntryStubId,
    ) -> Result<RelocationTarget, InstallationDiagnostic> {
        self.validated
            .frozen
            .artifact
            .selected_entry_target(identity)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallationDiagnostic(pub String);

impl std::fmt::Display for InstallationDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}
