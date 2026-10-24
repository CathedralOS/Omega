//! Install authorities, receipts, installed code, its registry authority
//! and installation diagnostics.

use crate::authority_digests::{
    ArtifactId, CodePlacementId, FinalValidationId, InstallationFactDigest, InstallationScopeId,
    InstalledCodeId,
};
use crate::placement::ValidatedPlacement;
use crate::placement::ValidatedPlacementEvidence;
use layout_plans::{EntryStubId, PlacementConstraints, RelocationTarget};
use target::Architecture;

/// One-shot authority to install one exact validated placement. Required
/// facts are open provider vocabulary — the receiver names in advance the
/// provider-canonical claims the contracted write-to-execute operation must
/// establish, such as its exact cache-order and instruction-fetch completion
/// facts — while instruction-fetch visibility and the W^X transition remain
/// mandatory lifecycle gates. An authority demanding no facts admits any
/// receipt that satisfies the lifecycle gates.
#[derive(Debug, PartialEq, Eq)]
pub struct InstallAuthority {
    pub(crate) validated: ValidatedPlacementEvidence,
    pub(crate) required_facts: std::collections::BTreeSet<InstallationFactDigest>,
}

impl InstallAuthority {
    pub fn from_admitted_provider(validated: &ValidatedPlacement) -> Self {
        Self {
            validated: ValidatedPlacementEvidence::from_validated(validated),
            required_facts: std::collections::BTreeSet::new(),
        }
    }

    /// Names the provider-canonical facts the installation receipt must
    /// establish, the same contract retirement authorities already impose.
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

/// Provider result of the one contracted write-to-execute operation. Beyond
/// the mandatory visibility and W^X claims it reports the provider-canonical
/// facts the operation established; the install gate requires the authority's
/// demanded set to appear here rather than recording assertions unchallenged.
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

    /// Records the provider-canonical facts the operation established.
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

/// Linear installed claim. It exposes identity/reporting only; callable entry
/// references are derived by the separate CFI/entry-reference gate.
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

    /// Exact installation scope carried by this admitted placement.
    pub const fn installation_scope(&self) -> InstallationScopeId {
        self.validated.frozen.placement.scope
    }

    /// Exact placement constraints retained by this installed occurrence.
    /// The projection is descriptive evidence — normalized range, alignment,
    /// phase, regime, and scope identities — and grants no placement or
    /// execute authority.
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

    /// Returns a sealed target only for an entry admitted with this installed
    /// artifact. The numeric address stays private to writer execution.
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
