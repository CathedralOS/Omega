//! Shared portable output and independent application judgments.
//!
//! These are producer results beside the canonical artifact, not another Psi IR.

use terminal_psi::CheckedProgramEntryTerminalReceipt;

/// Canonical Terminal output beside the checked-source evidence the caller
/// asked production to retain, in request order. Each application carries its
/// own refusal or optional checked `ProgramEntry` receipt. Production success
/// alone does not mean an application was admitted. Earlier representations
/// never escape.
///
/// The retained native route rejoins the checked entry receipt after the
/// checked frontend is gone, so the receipt leaves production beside the
/// artifact instead of remaining a direct-route-only custody object.
/// Designated Build operands likewise travel as source-issued companion context,
/// bound to the artifact and exact emitted operations, never as debug payload.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "Terminal production retains independent application judgments"]
pub struct ProducedTerminalArtifact {
    pub(super) artifact: terminal_codec::CanonicalTerminalArtifact,
    pub(super) build_bindings: crate::TerminalBuildBindings,
    pub(super) applications:
        Vec<Result<Option<CheckedProgramEntryTerminalReceipt>, Vec<diagnostics::Diagnostic>>>,
}

impl ProducedTerminalArtifact {
    pub const fn build_bindings(&self) -> &crate::TerminalBuildBindings {
        &self.build_bindings
    }
    pub const fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }

    /// Keep only the canonical portable artifact. This discards application
    /// judgments and Build binding context, and confers no authority to execute
    /// or publish a native product.
    pub fn into_artifact(self) -> terminal_codec::CanonicalTerminalArtifact {
        self.artifact
    }

    /// Extract the portable artifact and application judgments, discarding Build
    /// binding context. Build consumers borrow that context before extraction.
    pub fn into_parts(
        self,
    ) -> (
        terminal_codec::CanonicalTerminalArtifact,
        Vec<Result<Option<CheckedProgramEntryTerminalReceipt>, Vec<diagnostics::Diagnostic>>>,
    ) {
        (self.artifact, self.applications)
    }
}

/// What production retains beside the canonical portable artifact.
/// The entry identity is opaque checked source-signature custody; it does not
/// select a target or install host services.
#[derive(Clone, Copy, Default)]
pub struct TerminalApplicationCustody<'a> {
    pub entry_identity: Option<[u8; 32]>,
    /// Exact checked source member whose signature the companion retains.
    /// This does not select or rewrite the portable semantic root.
    pub entry_machine: Option<crate::TerminalMachineSelection<'a>>,
    /// Additional checked roots governed by this application's policy, such as
    /// its selected callbacks. These mint no ProgramEntry receipt.
    pub admission_roots: &'a [symbols::SymbolHandle],
    pub admission: Option<&'a dyn crate::TerminalCompositionAdmission>,
}

/// Independent consumers of one checked execution closure. Admission failure
/// belongs to that application, not to another consumer of the same program.
/// An empty slice requests only the portable artifact.
#[derive(Clone, Copy, Default)]
pub struct TerminalProductionCustody<'a> {
    pub applications: &'a [TerminalApplicationCustody<'a>],
}
