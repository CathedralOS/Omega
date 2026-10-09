//! Owned artifact and source custody retained on production success.
//!
//! These are producer results beside the canonical artifact, not another Psi IR.

use lowered_psi::LoweredPsi;
use terminal_psi::CheckedProgramEntryTerminalReceipt;

/// Canonical Terminal output beside the checked-source evidence the caller
/// asked production to retain: the unoptimized module and the checked
/// `ProgramEntry` receipt when an entry identity was supplied.
///
/// The retained native route rejoins the checked entry receipt after the
/// checked frontend is gone, so the receipt leaves production beside the
/// artifact instead of remaining a direct-route-only custody object.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "Terminal production retains the artifact and checked-source custody"]
pub struct ProducedTerminalArtifact {
    pub(super) artifact: terminal_codec::CanonicalTerminalArtifact,
    pub(super) receipt: Option<CheckedProgramEntryTerminalReceipt>,
    pub(super) unoptimized: Option<LoweredPsi>,
}

impl ProducedTerminalArtifact {
    pub const fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }

    /// The checked `ProgramEntry` receipt, present exactly when the request
    /// carried an entry identity.
    pub const fn receipt(&self) -> Option<&CheckedProgramEntryTerminalReceipt> {
        self.receipt.as_ref()
    }

    /// The lowered module before selected optimization, present exactly when
    /// the custody asked to retain it: the composition an admission check
    /// that must not see optimization reads, without lowering again.
    pub const fn unoptimized(&self) -> Option<&LoweredPsi> {
        self.unoptimized.as_ref()
    }

    /// Keep only the canonical portable artifact.
    pub fn into_artifact(self) -> terminal_codec::CanonicalTerminalArtifact {
        self.artifact
    }

    #[allow(clippy::type_complexity)]
    pub fn into_parts(
        self,
    ) -> (
        terminal_codec::CanonicalTerminalArtifact,
        Option<CheckedProgramEntryTerminalReceipt>,
        Option<LoweredPsi>,
    ) {
        (self.artifact, self.receipt, self.unoptimized)
    }
}

/// What production retains beside the canonical portable artifact.
/// The entry identity is opaque checked source-signature custody; it does not
/// select a target or install host services.
#[derive(Debug, Clone, Copy, Default)]
pub struct TerminalProductionCustody {
    pub entry_identity: Option<[u8; 32]>,
    pub retain_unoptimized: bool,
}
