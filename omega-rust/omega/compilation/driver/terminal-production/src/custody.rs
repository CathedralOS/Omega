//! Owned custody retained on production success and returned on rejection.
//!
//! These are driver results beside the canonical artifact, not another Psi IR.

use lowered_psi::{
    LoweredPsi, LoweredSelectedIeeeFloatComparisonOccurrence,
    LoweredSelectedIntegerComparisonOccurrence, LoweredSourceCallOccurrence,
};
use lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope;
use terminal_psi::CheckedProgramEntryTerminalReceipt;

use crate::stage_timings::TerminalProductionTimings;
use crate::terminal_production::TerminalArtifactProductionError;

/// Canonical Terminal output beside the checked-source evidence the caller
/// asked production to retain: the checked D29 demand scope, the selected
/// floating-point and integer comparison occurrences, the source call
/// occurrences, the checked `ProgramEntry` receipt when an entry identity was
/// supplied, and the caller's opaque callback-use custody.
///
/// The retained native route rejoins the checked entry receipt after the
/// checked frontend is gone, so the receipt leaves production beside the
/// artifact instead of remaining a direct-route-only custody object. Psi
/// neither inspects the callback sidecar nor grants callback placement,
/// registration, invocation, address, or lifetime authority.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "Terminal production retains checked-source and callback custody"]
pub struct ProducedTerminalArtifact<C> {
    pub(super) artifact: terminal_codec::CanonicalTerminalArtifact,
    pub(super) receipt: Option<CheckedProgramEntryTerminalReceipt>,
    pub(super) unoptimized: Option<LoweredPsi>,
    pub(super) boundary_operator_scope: CheckedBoundaryOperatorApplicationScope,
    pub(super) callback_custody: C,
    pub(super) source_call_occurrences: Vec<LoweredSourceCallOccurrence>,
    pub(super) selected_ieee_float_comparison_occurrences:
        Vec<LoweredSelectedIeeeFloatComparisonOccurrence>,
    pub(super) selected_integer_comparison_occurrences:
        Vec<LoweredSelectedIntegerComparisonOccurrence>,
    /// Rewrites a selected pass proposed and then withheld. Empty when every
    /// pass either rewrote or found nothing; the published artifact is the
    /// same either way.
    pub(super) withheld_rewrites: Vec<optimization_core::WithheldRewrite>,
}

impl<C> ProducedTerminalArtifact<C> {
    pub const fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }

    /// The rewrites selected passes proposed and withheld, with the reason
    /// each validator gave. A pass that found nothing to rewrite records
    /// nothing here, so an empty slice and a withheld proposal stay
    /// distinguishable in the report.
    pub fn withheld_rewrites(&self) -> &[optimization_core::WithheldRewrite] {
        &self.withheld_rewrites
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

    pub const fn callback_custody(&self) -> &C {
        &self.callback_custody
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
        C,
        Vec<LoweredSourceCallOccurrence>,
        Vec<LoweredSelectedIeeeFloatComparisonOccurrence>,
        Vec<LoweredSelectedIntegerComparisonOccurrence>,
    ) {
        (
            self.artifact,
            self.receipt,
            self.unoptimized,
            self.boundary_operator_scope,
            self.callback_custody,
            self.source_call_occurrences,
            self.selected_ieee_float_comparison_occurrences,
            self.selected_integer_comparison_occurrences,
        )
    }
}

/// Transactional rejection from Terminal production.
///
/// The checked tree and selected machine are borrowed inputs. The only owned
/// input is the callback sidecar, so rejection returns it exactly for retry or
/// diagnosis instead of silently discarding it.
#[derive(Debug)]
#[must_use = "Terminal production rejection returns callback-use custody"]
pub struct CallbackCustodyTerminalArtifactProductionError<C> {
    pub(super) error: TerminalArtifactProductionError,
    pub(super) callback_custody: C,
}

impl<C> CallbackCustodyTerminalArtifactProductionError<C> {
    pub const fn error(&self) -> &TerminalArtifactProductionError {
        &self.error
    }

    pub const fn callback_custody(&self) -> &C {
        &self.callback_custody
    }

    pub fn into_parts(self) -> (TerminalArtifactProductionError, C) {
        (self.error, self.callback_custody)
    }
}

impl<C> std::fmt::Display for CallbackCustodyTerminalArtifactProductionError<C> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(formatter)
    }
}

impl<C: std::fmt::Debug> std::error::Error for CallbackCustodyTerminalArtifactProductionError<C> {}

/// What one production run keeps in custody beside the artifact.
///
/// `entry_identity` is the checked source-signature identity a
/// `ProgramEntry` receipt retains; `None` publishes the machine as an
/// ordinary Terminal product with no entry receipt. `callback_custody` is the
/// caller's opaque callback-use sidecar, returned by value on success and on
/// rejection. `timings` records each production leg; a
/// [`TerminalProductionTimings::default`] carrier runs the same body
/// unmeasured.
pub struct TerminalProductionCustody<'t, C> {
    pub entry_identity: Option<[u8; 32]>,
    pub callback_custody: C,
    /// Retain the lowered module as it was before selected optimization,
    /// beside the artifact. An admission that must judge the unoptimized
    /// composition (the authored behavior exclusions) reads it from the
    /// product instead of lowering the entry a second time.
    pub retain_unoptimized: bool,
    pub timings: &'t mut TerminalProductionTimings,
}

impl<'t> TerminalProductionCustody<'t, ()> {
    /// No entry receipt and no callback sidecar: only the artifact and its
    /// checked-scope evidence are retained.
    pub fn artifact_only(timings: &'t mut TerminalProductionTimings) -> Self {
        Self {
            entry_identity: None,
            callback_custody: (),
            retain_unoptimized: false,
            timings,
        }
    }
}
