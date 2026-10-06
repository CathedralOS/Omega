//! Product reporting beside Psi-owned artifact and source custody.
//!
//! These are driver results beside the canonical artifact, not another Psi IR.

use lowered_psi::{
    LoweredPsi, LoweredSelectedIeeeFloatComparisonOccurrence,
    LoweredSelectedIntegerComparisonOccurrence, LoweredSourceCallOccurrence,
};
use lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope;
use terminal_psi::CheckedProgramEntryTerminalReceipt;

use artifacts::compile_timings::CompileTimings;

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
    pub(super) production: psi_terminal_production::ProducedTerminalArtifact,
}

impl ProducedTerminalArtifact {
    pub const fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        self.production.artifact()
    }

    /// The checked `ProgramEntry` receipt, present exactly when the request
    /// carried an entry identity.
    pub const fn receipt(&self) -> Option<&CheckedProgramEntryTerminalReceipt> {
        self.production.receipt()
    }

    /// The lowered module before selected optimization, present exactly when
    /// the custody asked to retain it: the composition an admission check
    /// that must not see optimization reads, without lowering again.
    pub const fn unoptimized(&self) -> Option<&LoweredPsi> {
        self.production.unoptimized()
    }

    pub const fn boundary_operator_scope(&self) -> &CheckedBoundaryOperatorApplicationScope {
        self.production.boundary_operator_scope()
    }

    pub fn source_call_occurrences(&self) -> &[LoweredSourceCallOccurrence] {
        self.production.source_call_occurrences()
    }

    pub fn selected_ieee_float_comparison_occurrences(
        &self,
    ) -> &[LoweredSelectedIeeeFloatComparisonOccurrence] {
        self.production.selected_ieee_float_comparison_occurrences()
    }

    /// Selected integer comparison joins, the integer counterpart of the IEEE
    /// comparison roster. Omega rejoins each row to its exact selected
    /// provider before native realization may admit the operation.
    pub fn selected_integer_comparison_occurrences(
        &self,
    ) -> &[LoweredSelectedIntegerComparisonOccurrence] {
        self.production.selected_integer_comparison_occurrences()
    }

    /// Keep only the canonical portable artifact.
    pub fn into_artifact(self) -> terminal_codec::CanonicalTerminalArtifact {
        self.production.into_artifact()
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
        self.production.into_parts()
    }
}

/// What one production run keeps in custody beside the artifact.
///
/// `entry_identity` is the checked source-signature identity a
/// `ProgramEntry` receipt retains; `None` publishes the machine as an
/// ordinary Terminal product with no entry receipt. `timings` records each
/// production leg into the request's own ladder; a disabled `CompileTimings`
/// runs the same body unmeasured.
pub struct TerminalProductionCustody<'t> {
    pub entry_identity: Option<[u8; 32]>,
    /// Retain the lowered module as it was before selected optimization,
    /// beside the artifact. An admission that must judge the unoptimized
    /// composition (the authored behavior exclusions) reads it from the
    /// product instead of lowering the entry a second time.
    pub retain_unoptimized: bool,
    pub timings: &'t mut CompileTimings,
}

impl<'t> TerminalProductionCustody<'t> {
    /// No entry receipt: only the artifact and its checked-scope evidence are
    /// retained.
    pub fn artifact_only(timings: &'t mut CompileTimings) -> Self {
        Self {
            entry_identity: None,
            retain_unoptimized: false,
            timings,
        }
    }
}
