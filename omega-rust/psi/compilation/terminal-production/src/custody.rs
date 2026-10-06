//! Owned artifact and source custody retained on production success.
//!
//! These are producer results beside the canonical artifact, not another Psi IR.

use lowered_psi::{
    LoweredPsi, LoweredSelectedIeeeFloatComparisonOccurrence,
    LoweredSelectedIntegerComparisonOccurrence, LoweredSourceCallOccurrence,
};
use lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope;
use terminal_psi::CheckedProgramEntryTerminalReceipt;

/// Canonical Terminal output beside the checked-source evidence the caller
/// asked production to retain: the checked D29 demand scope, the selected
/// floating-point and integer comparison occurrences, the source call
/// occurrences, and the checked `ProgramEntry` receipt when an entry identity
/// was supplied.
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
    pub(super) boundary_operator_scope: CheckedBoundaryOperatorApplicationScope,
    pub(super) source_call_occurrences: Vec<LoweredSourceCallOccurrence>,
    pub(super) selected_ieee_float_comparison_occurrences:
        Vec<LoweredSelectedIeeeFloatComparisonOccurrence>,
    pub(super) selected_integer_comparison_occurrences:
        Vec<LoweredSelectedIntegerComparisonOccurrence>,
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

    pub const fn boundary_operator_scope(&self) -> &CheckedBoundaryOperatorApplicationScope {
        &self.boundary_operator_scope
    }

    pub fn source_call_occurrences(&self) -> &[LoweredSourceCallOccurrence] {
        &self.source_call_occurrences
    }

    pub fn selected_ieee_float_comparison_occurrences(
        &self,
    ) -> &[LoweredSelectedIeeeFloatComparisonOccurrence] {
        &self.selected_ieee_float_comparison_occurrences
    }

    /// Selected integer comparison joins, the integer counterpart of the IEEE
    /// comparison roster. Omega rejoins each row to its exact selected
    /// provider before native realization may admit the operation.
    pub fn selected_integer_comparison_occurrences(
        &self,
    ) -> &[LoweredSelectedIntegerComparisonOccurrence] {
        &self.selected_integer_comparison_occurrences
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
        CheckedBoundaryOperatorApplicationScope,
        Vec<LoweredSourceCallOccurrence>,
        Vec<LoweredSelectedIeeeFloatComparisonOccurrence>,
        Vec<LoweredSelectedIntegerComparisonOccurrence>,
    ) {
        (
            self.artifact,
            self.receipt,
            self.unoptimized,
            self.boundary_operator_scope,
            self.source_call_occurrences,
            self.selected_ieee_float_comparison_occurrences,
            self.selected_integer_comparison_occurrences,
        )
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
