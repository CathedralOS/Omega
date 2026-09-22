use checked_trees::CheckedTrees;
use checked_trees_to_lowered_psi::{
    LoweringError, TerminalMachineSelection, lower_machine, select_terminal_machine,
};
use lowered_psi::{
    LoweredPsi, LoweredSelectedIeeeFloatComparisonOccurrence,
    LoweredSelectedIeeeFloatFmaOccurrence, LoweredSelectedIntegerComparisonOccurrence,
    LoweredSourceCallOccurrence,
};
use lowered_psi_to_lowered_psi::{
    PsiOptimizationStageError, PsiOptimizationStageResult, run_psi_optimization,
};
use lowered_psi_to_terminal_psi::{
    CheckedBoundaryOperatorApplicationScope, finalize_terminal_artifact,
};
use terminal_codec::terminal_psi_identity;
use terminal_psi::{CheckedProgramEntryTerminalReceipt, TerminalMachineResult};

use crate::stage_timings::{TerminalProductionStage, TerminalProductionTimings};

mod receiver_eligibility;
/// Canonical Terminal output coupled to its non-caller-authored checked D29
/// demand scope.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "checked Terminal production retains boundary-operator demand custody"]
pub struct ProducedTerminalArtifact {
    artifact: terminal_codec::CanonicalTerminalArtifact,
    boundary_operator_scope: CheckedBoundaryOperatorApplicationScope,
    selected_ieee_float_fma_occurrences: Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
    selected_ieee_float_comparison_occurrences: Vec<LoweredSelectedIeeeFloatComparisonOccurrence>,
    selected_integer_comparison_occurrences: Vec<LoweredSelectedIntegerComparisonOccurrence>,
}

impl ProducedTerminalArtifact {
    pub const fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }

    pub const fn boundary_operator_scope(&self) -> &CheckedBoundaryOperatorApplicationScope {
        &self.boundary_operator_scope
    }

    pub fn selected_ieee_float_fma_occurrences(&self) -> &[LoweredSelectedIeeeFloatFmaOccurrence] {
        &self.selected_ieee_float_fma_occurrences
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

    pub fn into_parts(
        self,
    ) -> (
        terminal_codec::CanonicalTerminalArtifact,
        CheckedBoundaryOperatorApplicationScope,
        Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
        Vec<LoweredSelectedIeeeFloatComparisonOccurrence>,
        Vec<LoweredSelectedIntegerComparisonOccurrence>,
    ) {
        (
            self.artifact,
            self.boundary_operator_scope,
            self.selected_ieee_float_fma_occurrences,
            self.selected_ieee_float_comparison_occurrences,
            self.selected_integer_comparison_occurrences,
        )
    }
}

/// Canonical Terminal artifact coupled to an opaque callback-use sidecar.
///
/// Psi does not interpret target-owned callback placement. This carrier only
/// makes the canonical producer's custody boundary explicit: the caller gives
/// the complete sidecar by value and receives the same value beside the
/// artifact. It grants no registration, invocation, address, or lifetime
/// authority.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "Terminal production must preserve callback-use custody"]
pub struct ProducedTerminalArtifactWithCallbackCustody<C> {
    artifact: terminal_codec::CanonicalTerminalArtifact,
    boundary_operator_scope: CheckedBoundaryOperatorApplicationScope,
    callback_custody: C,
    source_call_occurrences: Vec<LoweredSourceCallOccurrence>,
    selected_ieee_float_fma_occurrences: Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
    selected_ieee_float_comparison_occurrences: Vec<LoweredSelectedIeeeFloatComparisonOccurrence>,
    selected_integer_comparison_occurrences: Vec<LoweredSelectedIntegerComparisonOccurrence>,
}

impl<C> ProducedTerminalArtifactWithCallbackCustody<C> {
    pub const fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }

    pub const fn callback_custody(&self) -> &C {
        &self.callback_custody
    }

    pub const fn boundary_operator_scope(&self) -> &CheckedBoundaryOperatorApplicationScope {
        &self.boundary_operator_scope
    }

    pub fn source_call_occurrences(&self) -> &[LoweredSourceCallOccurrence] {
        &self.source_call_occurrences
    }

    pub fn selected_ieee_float_fma_occurrences(&self) -> &[LoweredSelectedIeeeFloatFmaOccurrence] {
        &self.selected_ieee_float_fma_occurrences
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

    pub fn into_parts(
        self,
    ) -> (
        terminal_codec::CanonicalTerminalArtifact,
        CheckedBoundaryOperatorApplicationScope,
        C,
        Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
        Vec<LoweredSelectedIeeeFloatComparisonOccurrence>,
        Vec<LoweredSelectedIntegerComparisonOccurrence>,
    ) {
        (
            self.artifact,
            self.boundary_operator_scope,
            self.callback_custody,
            self.selected_ieee_float_fma_occurrences,
            self.selected_ieee_float_comparison_occurrences,
            self.selected_integer_comparison_occurrences,
        )
    }

    #[allow(clippy::type_complexity)]
    pub fn into_parts_with_source_calls(
        self,
    ) -> (
        terminal_codec::CanonicalTerminalArtifact,
        CheckedBoundaryOperatorApplicationScope,
        C,
        Vec<LoweredSourceCallOccurrence>,
        Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
        Vec<LoweredSelectedIeeeFloatComparisonOccurrence>,
        Vec<LoweredSelectedIntegerComparisonOccurrence>,
    ) {
        (
            self.artifact,
            self.boundary_operator_scope,
            self.callback_custody,
            self.source_call_occurrences,
            self.selected_ieee_float_fma_occurrences,
            self.selected_ieee_float_comparison_occurrences,
            self.selected_integer_comparison_occurrences,
        )
    }
}

/// Canonical Terminal output coupled to the checked `ProgramEntry` receipt,
/// the checked D29 demand scope, and the caller's opaque callback-use custody.
///
/// The retained native route rejoins the checked entry receipt after the
/// checked frontend is gone, so the receipt must leave production beside the
/// artifact instead of remaining a direct-route-only custody object.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "ProgramEntry Terminal production retains entry and callback custody"]
pub struct ProducedProgramEntryTerminalArtifactWithCallbackCustody<C> {
    artifact: terminal_codec::CanonicalTerminalArtifact,
    receipt: CheckedProgramEntryTerminalReceipt,
    boundary_operator_scope: CheckedBoundaryOperatorApplicationScope,
    callback_custody: C,
    source_call_occurrences: Vec<LoweredSourceCallOccurrence>,
    selected_ieee_float_fma_occurrences: Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
    selected_ieee_float_comparison_occurrences: Vec<LoweredSelectedIeeeFloatComparisonOccurrence>,
    selected_integer_comparison_occurrences: Vec<LoweredSelectedIntegerComparisonOccurrence>,
}

impl<C> ProducedProgramEntryTerminalArtifactWithCallbackCustody<C> {
    pub const fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }

    pub const fn receipt(&self) -> &CheckedProgramEntryTerminalReceipt {
        &self.receipt
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

    pub fn selected_ieee_float_fma_occurrences(&self) -> &[LoweredSelectedIeeeFloatFmaOccurrence] {
        &self.selected_ieee_float_fma_occurrences
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

    #[allow(clippy::type_complexity)]
    pub fn into_parts_with_source_calls(
        self,
    ) -> (
        terminal_codec::CanonicalTerminalArtifact,
        CheckedProgramEntryTerminalReceipt,
        CheckedBoundaryOperatorApplicationScope,
        C,
        Vec<LoweredSourceCallOccurrence>,
        Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
        Vec<LoweredSelectedIeeeFloatComparisonOccurrence>,
        Vec<LoweredSelectedIntegerComparisonOccurrence>,
    ) {
        (
            self.artifact,
            self.receipt,
            self.boundary_operator_scope,
            self.callback_custody,
            self.source_call_occurrences,
            self.selected_ieee_float_fma_occurrences,
            self.selected_ieee_float_comparison_occurrences,
            self.selected_integer_comparison_occurrences,
        )
    }
}

/// Transactional rejection from callback-aware Terminal production.
///
/// The checked tree and selected machine are borrowed inputs. The only owned
/// input is the callback sidecar, so rejection returns it exactly for retry or
/// diagnosis instead of silently discarding it.
#[derive(Debug)]
#[must_use = "Terminal production rejection returns callback-use custody"]
pub struct CallbackCustodyTerminalArtifactProductionError<C> {
    error: TerminalArtifactProductionError,
    callback_custody: C,
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

/// Canonical Terminal artifact coupled to the checked-entry receipt produced
/// from the same lowering result.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "ProgramEntry Terminal production retains an entry-custody receipt"]
pub struct ProducedProgramEntryTerminalArtifact {
    artifact: terminal_codec::CanonicalTerminalArtifact,
    receipt: CheckedProgramEntryTerminalReceipt,
    boundary_operator_scope: CheckedBoundaryOperatorApplicationScope,
    selected_ieee_float_fma_occurrences: Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
    selected_ieee_float_comparison_occurrences: Vec<LoweredSelectedIeeeFloatComparisonOccurrence>,
    selected_integer_comparison_occurrences: Vec<LoweredSelectedIntegerComparisonOccurrence>,
}

impl ProducedProgramEntryTerminalArtifact {
    pub const fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }

    pub const fn receipt(&self) -> &CheckedProgramEntryTerminalReceipt {
        &self.receipt
    }

    pub const fn boundary_operator_scope(&self) -> &CheckedBoundaryOperatorApplicationScope {
        &self.boundary_operator_scope
    }

    pub fn selected_ieee_float_fma_occurrences(&self) -> &[LoweredSelectedIeeeFloatFmaOccurrence] {
        &self.selected_ieee_float_fma_occurrences
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

    pub fn into_parts(
        self,
    ) -> (
        terminal_codec::CanonicalTerminalArtifact,
        CheckedProgramEntryTerminalReceipt,
        CheckedBoundaryOperatorApplicationScope,
        Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
        Vec<LoweredSelectedIeeeFloatComparisonOccurrence>,
        Vec<LoweredSelectedIntegerComparisonOccurrence>,
    ) {
        (
            self.artifact,
            self.receipt,
            self.boundary_operator_scope,
            self.selected_ieee_float_fma_occurrences,
            self.selected_ieee_float_comparison_occurrences,
            self.selected_integer_comparison_occurrences,
        )
    }
}

/// Exact borrowed source inputs and target-neutral selection for Terminal production.
/// Output methods retain their distinct evidence and recovery contracts.
pub struct TerminalProductionRequest<'a> {
    pub checked: &'a CheckedTrees,
    pub machine: TerminalMachineSelection<'a>,
    pub optimization_selections: optimization::PsiOptimizationSelections,
}

impl<'a> TerminalProductionRequest<'a> {
    /// Select the identity optimization phase by default.
    pub fn new(checked: &'a CheckedTrees, machine_name: &'a str) -> Self {
        Self {
            checked,
            machine: TerminalMachineSelection::Name(machine_name),
            optimization_selections: optimization::PsiOptimizationSelections::default(),
        }
    }

    /// Select the exact checked machine the identity optimization phase applies to.
    pub fn for_machine_symbol(checked: &'a CheckedTrees, machine: symbols::SymbolHandle) -> Self {
        Self {
            checked,
            machine: TerminalMachineSelection::Symbol(machine),
            optimization_selections: optimization::PsiOptimizationSelections::default(),
        }
    }

    /// Publish only the canonical portable artifact.
    pub fn produce_artifact(
        self,
    ) -> Result<terminal_codec::CanonicalTerminalArtifact, TerminalArtifactProductionError> {
        self.produce_artifact_timed(&mut TerminalProductionTimings::default())
    }

    /// [`Self::produce_artifact`] recording each production leg into the
    /// Psi-owned timing carrier the caller merges into its timing report.
    pub fn produce_artifact_timed(
        self,
        timings: &mut TerminalProductionTimings,
    ) -> Result<terminal_codec::CanonicalTerminalArtifact, TerminalArtifactProductionError> {
        let optimized = self.lower_and_optimize_timed(timings)?;
        let (artifact, _) = timings.record_result(TerminalProductionStage::Publication, || {
            publish_terminal_artifact(optimized)
        })?;
        Ok(artifact)
    }

    /// Retain the checked D29 demand scope and selected floating-point occurrences.
    pub fn produce_checked_artifact(
        self,
    ) -> Result<ProducedTerminalArtifact, TerminalArtifactProductionError> {
        self.produce_checked_artifact_timed(&mut TerminalProductionTimings::default())
    }

    /// [`Self::produce_checked_artifact`] recording each production leg into
    /// the Psi-owned timing carrier the caller merges into its timing report.
    pub fn produce_checked_artifact_timed(
        self,
        timings: &mut TerminalProductionTimings,
    ) -> Result<ProducedTerminalArtifact, TerminalArtifactProductionError> {
        let (artifact, lowered, boundary_operator_scope) =
            self.produce_checked_parts_timed(timings)?;
        Ok(ProducedTerminalArtifact {
            artifact,
            boundary_operator_scope,
            selected_ieee_float_fma_occurrences: lowered.selected_ieee_float_fma_occurrences,
            selected_ieee_float_comparison_occurrences: lowered
                .selected_ieee_float_comparison_occurrences,
            selected_integer_comparison_occurrences: lowered
                .selected_integer_comparison_occurrences,
        })
    }

    /// Preserve the caller's opaque callback-use sidecar on success and rejection.
    ///
    /// Psi neither inspects the sidecar nor grants callback placement, registration,
    /// invocation, address, or lifetime authority.
    pub fn produce_with_callback_custody<C>(
        self,
        callback_custody: C,
    ) -> Result<
        ProducedTerminalArtifactWithCallbackCustody<C>,
        CallbackCustodyTerminalArtifactProductionError<C>,
    > {
        self.produce_with_callback_custody_timed(
            callback_custody,
            &mut TerminalProductionTimings::default(),
        )
    }

    /// [`Self::produce_with_callback_custody`] recording each production leg
    /// into the Psi-owned timing carrier the caller merges into its timing
    /// report.
    pub fn produce_with_callback_custody_timed<C>(
        self,
        callback_custody: C,
        timings: &mut TerminalProductionTimings,
    ) -> Result<
        ProducedTerminalArtifactWithCallbackCustody<C>,
        CallbackCustodyTerminalArtifactProductionError<C>,
    > {
        let (artifact, lowered, boundary_operator_scope) =
            match self.produce_checked_parts_timed(timings) {
                Ok(parts) => parts,
                Err(error) => {
                    return Err(CallbackCustodyTerminalArtifactProductionError {
                        error,
                        callback_custody,
                    });
                }
            };
        Ok(ProducedTerminalArtifactWithCallbackCustody {
            artifact,
            boundary_operator_scope,
            callback_custody,
            source_call_occurrences: lowered.source_call_occurrences,
            selected_ieee_float_fma_occurrences: lowered.selected_ieee_float_fma_occurrences,
            selected_ieee_float_comparison_occurrences: lowered
                .selected_ieee_float_comparison_occurrences,
            selected_integer_comparison_occurrences: lowered
                .selected_integer_comparison_occurrences,
        })
    }

    /// Retain the exact checked ProgramEntry-to-Terminal association.
    ///
    /// The source-signature digest remains opaque; later Omega settlement must
    /// independently compare it with the retained source signature.
    pub fn produce_program_entry(
        self,
        source_signature_identity: [u8; 32],
    ) -> Result<ProducedProgramEntryTerminalArtifact, TerminalArtifactProductionError> {
        self.produce_program_entry_timed(
            source_signature_identity,
            &mut TerminalProductionTimings::default(),
        )
    }

    /// [`Self::produce_program_entry`] recording each production leg into the
    /// Psi-owned timing carrier the caller merges into its timing report.
    pub fn produce_program_entry_timed(
        self,
        source_signature_identity: [u8; 32],
        timings: &mut TerminalProductionTimings,
    ) -> Result<ProducedProgramEntryTerminalArtifact, TerminalArtifactProductionError> {
        let (artifact, receipt, boundary_operator_scope, lowered) =
            self.produce_program_entry_parts_timed(source_signature_identity, timings)?;
        Ok(ProducedProgramEntryTerminalArtifact {
            boundary_operator_scope,
            artifact,
            receipt,
            selected_ieee_float_fma_occurrences: lowered.selected_ieee_float_fma_occurrences,
            selected_ieee_float_comparison_occurrences: lowered
                .selected_ieee_float_comparison_occurrences,
            selected_integer_comparison_occurrences: lowered
                .selected_integer_comparison_occurrences,
        })
    }

    /// Retain the checked ProgramEntry-to-Terminal association together with
    /// the caller's opaque callback-use sidecar on success and rejection.
    ///
    /// [`Self::produce_program_entry`] custody and
    /// [`Self::produce_with_callback_custody`]'s transactional sidecar in one
    /// product, so a retained Terminal artifact can carry the checked entry
    /// receipt its later native settlement will independently rejoin.
    pub fn produce_program_entry_with_callback_custody<C>(
        self,
        source_signature_identity: [u8; 32],
        callback_custody: C,
    ) -> Result<
        ProducedProgramEntryTerminalArtifactWithCallbackCustody<C>,
        CallbackCustodyTerminalArtifactProductionError<C>,
    > {
        self.produce_program_entry_with_callback_custody_timed(
            source_signature_identity,
            callback_custody,
            &mut TerminalProductionTimings::default(),
        )
    }

    /// [`Self::produce_program_entry_with_callback_custody`] recording each
    /// production leg into the Psi-owned timing carrier the caller merges into
    /// its timing report.
    pub fn produce_program_entry_with_callback_custody_timed<C>(
        self,
        source_signature_identity: [u8; 32],
        callback_custody: C,
        timings: &mut TerminalProductionTimings,
    ) -> Result<
        ProducedProgramEntryTerminalArtifactWithCallbackCustody<C>,
        CallbackCustodyTerminalArtifactProductionError<C>,
    > {
        let (artifact, receipt, boundary_operator_scope, lowered) =
            match self.produce_program_entry_parts_timed(source_signature_identity, timings) {
                Ok(parts) => parts,
                Err(error) => {
                    return Err(CallbackCustodyTerminalArtifactProductionError {
                        error,
                        callback_custody,
                    });
                }
            };
        Ok(ProducedProgramEntryTerminalArtifactWithCallbackCustody {
            artifact,
            receipt,
            boundary_operator_scope,
            callback_custody,
            source_call_occurrences: lowered.source_call_occurrences,
            selected_ieee_float_fma_occurrences: lowered.selected_ieee_float_fma_occurrences,
            selected_ieee_float_comparison_occurrences: lowered
                .selected_ieee_float_comparison_occurrences,
            selected_integer_comparison_occurrences: lowered
                .selected_integer_comparison_occurrences,
        })
    }

    #[allow(clippy::type_complexity)]
    fn produce_program_entry_parts_timed(
        self,
        source_signature_identity: [u8; 32],
        timings: &mut TerminalProductionTimings,
    ) -> Result<
        (
            terminal_codec::CanonicalTerminalArtifact,
            CheckedProgramEntryTerminalReceipt,
            CheckedBoundaryOperatorApplicationScope,
            LoweredPsi,
        ),
        TerminalArtifactProductionError,
    > {
        let selection = timings
            .record_result(TerminalProductionStage::MachineSelection, || {
                select_terminal_machine(self.checked, self.machine)
            })
            .map_err(TerminalArtifactProductionError::Lowering)?;
        let source_machine_name = selection.name.clone();
        let source_machine_symbol = selection.machine;
        let checked = self.checked;
        let optimized = self.lower_and_optimize_timed(timings)?;
        let optimized_lowered = optimized.lowered();
        timings.record_result(TerminalProductionStage::EntryReceipt, || {
            let entry_matches = optimized_lowered
                .semantic_module
                .machines
                .iter()
                .filter(|machine| machine.id == optimized_lowered.semantic_module.entry)
                .collect::<Vec<_>>();
            let [entry] = entry_matches.as_slice() else {
                return Err(TerminalArtifactProductionError::EntryReceipt(
                    ProgramEntryTerminalReceiptError::TerminalEntryMultiplicity(
                        entry_matches.len(),
                    ),
                ));
            };
            if entry.result != TerminalMachineResult::Unit {
                return Err(TerminalArtifactProductionError::EntryReceipt(
                    ProgramEntryTerminalReceiptError::NonUnitEntry,
                ));
            }
            Ok(())
        })?;
        let terminal_psi_identity = timings
            .record_result(TerminalProductionStage::TerminalIdentity, || {
                terminal_psi_identity(&optimized_lowered.semantic_module)
            })
            .map_err(ProgramEntryTerminalReceiptError::TerminalIdentity)
            .map_err(TerminalArtifactProductionError::EntryReceipt)?;
        let terminal_entry = optimized_lowered.semantic_module.entry;
        let receiver_eligibility = timings
            .record_result(TerminalProductionStage::ReceiverEligibility, || {
                Ok::<_, ()>(receiver_eligibility::derive(
                    checked,
                    selection,
                    &optimized_lowered.semantic_module,
                ))
            })
            .expect("receiver eligibility derivation is infallible");
        let (artifact, lowered) = timings
            .record_result(TerminalProductionStage::Publication, || {
                publish_terminal_artifact(optimized)
            })?;
        if artifact.manifest().semantic() != terminal_psi_identity {
            return Err(TerminalArtifactProductionError::EntryReceipt(
                ProgramEntryTerminalReceiptError::ArtifactSemanticIdentityMismatch,
            ));
        }
        let boundary_operator_scope = timings
            .record_result(TerminalProductionStage::BoundaryOperatorScope, || {
                checked_boundary_operator_scope(checked, &artifact, &lowered)
            })
            .map_err(TerminalArtifactProductionError::Lowering)?;
        Ok((
            artifact,
            CheckedProgramEntryTerminalReceipt::new(
                source_signature_identity,
                source_machine_name,
                source_machine_symbol,
                terminal_psi_identity,
                terminal_entry,
                receiver_eligibility,
            ),
            boundary_operator_scope,
            lowered,
        ))
    }

    fn lower_and_optimize_timed(
        self,
        timings: &mut TerminalProductionTimings,
    ) -> Result<PsiOptimizationStageResult, TerminalArtifactProductionError> {
        timings
            .record_result(TerminalProductionStage::LedgerCheck, || {
                crate::checked_ledger::verify(self.checked)
            })
            .map_err(TerminalArtifactProductionError::Lowering)?;
        let lowered = timings
            .record_result(TerminalProductionStage::Lowering, || {
                lower_machine(self.checked, self.machine)
            })
            .map_err(TerminalArtifactProductionError::Lowering)?;
        timings
            .record_result(TerminalProductionStage::Optimization, || {
                run_psi_optimization(lowered, self.optimization_selections)
            })
            .map_err(TerminalArtifactProductionError::Optimization)
    }

    fn produce_checked_parts_timed(
        self,
        timings: &mut TerminalProductionTimings,
    ) -> Result<
        (
            terminal_codec::CanonicalTerminalArtifact,
            LoweredPsi,
            CheckedBoundaryOperatorApplicationScope,
        ),
        TerminalArtifactProductionError,
    > {
        let checked = self.checked;
        let optimized = self.lower_and_optimize_timed(timings)?;
        let (artifact, lowered) = timings
            .record_result(TerminalProductionStage::Publication, || {
                publish_terminal_artifact(optimized)
            })?;
        let scope = timings
            .record_result(TerminalProductionStage::BoundaryOperatorScope, || {
                checked_boundary_operator_scope(checked, &artifact, &lowered)
            })
            .map_err(TerminalArtifactProductionError::Lowering)?;
        Ok((artifact, lowered, scope))
    }
}

/// Publication consumes exactly the validated optimization result; retained
/// source evidence is extracted only after canonical publication succeeds.
fn publish_terminal_artifact(
    optimized: PsiOptimizationStageResult,
) -> Result<(terminal_codec::CanonicalTerminalArtifact, LoweredPsi), TerminalArtifactProductionError>
{
    let artifact = finalize_terminal_artifact(&optimized)
        .map_err(TerminalArtifactProductionError::Artifact)?;
    Ok((artifact, optimized.into_lowered()))
}

#[derive(Debug)]
pub enum TerminalArtifactProductionError {
    Lowering(LoweringError),
    Optimization(PsiOptimizationStageError),
    Artifact(terminal_codec::CanonicalTerminalArtifactError),
    EntryReceipt(ProgramEntryTerminalReceiptError),
}

#[derive(Debug)]
pub enum ProgramEntryTerminalReceiptError {
    TerminalEntryMultiplicity(usize),
    NonUnitEntry,
    TerminalIdentity(terminal_codec::CodecError),
    ArtifactSemanticIdentityMismatch,
}

impl std::fmt::Display for ProgramEntryTerminalReceiptError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProgramEntryTerminalReceiptError {}

impl std::fmt::Display for TerminalArtifactProductionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalArtifactProductionError {}

fn checked_boundary_operator_scope(
    checked: &CheckedTrees,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    lowered: &LoweredPsi,
) -> Result<CheckedBoundaryOperatorApplicationScope, LoweringError> {
    lowered_psi_to_terminal_psi::checked_boundary_operator_scope(checked, artifact, lowered)
        .map_err(LoweringError::Unsupported)
}

#[cfg(test)]
mod tests {
    use checked_trees::CheckedTrees;

    use crate::{TerminalProductionRequest, TerminalProductionStage, TerminalProductionTimings};

    fn check_source(source: &str) -> CheckedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        typed_trees_to_checked_trees::lower_typed_trees(
            typed,
            &typed_trees_to_checked_trees::CheckingRequest::settled(),
        )
        .unwrap()
    }

    /// Production lowering runs the correspondence retention route: the batch
    /// is extracted from the checked program's typed trees and installed on
    /// the module the artifact publishes. Ordinary programs produce an empty
    /// batch, so the published module carries no quotient rows and still
    /// decodes to identical identity.
    #[test]
    fn production_installs_the_extracted_quotient_correspondence_batch() {
        let checked = check_source(
            "data Main { value: i32; } machine Main::run(&mut self) { self.value = 7; }",
        );
        let produced = TerminalProductionRequest::new(&checked, "Main::run")
            .produce_program_entry([7; 32])
            .unwrap();
        let module = terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
        assert!(module.quotient_correspondences.is_empty());
    }

    /// The non-entry producers share the instrumented body: an enabled
    /// carrier records the ledger/lowering/optimization/publication/
    /// boundary-scope ladder for the checked-artifact route, and the
    /// callback-custody variant returns the same ladder while retaining the
    /// caller's sidecar.
    #[test]
    fn non_entry_production_records_the_stage_ladder() {
        let checked = check_source(
            "data Main { value: i32; } machine Main::run(&mut self) { self.value = 7; }",
        );
        let mut timings = TerminalProductionTimings::enabled();
        let produced = TerminalProductionRequest::new(&checked, "Main::run")
            .produce_checked_artifact_timed(&mut timings)
            .unwrap();
        assert!(!produced.artifact().semantic_bytes().is_empty());
        let stages: Vec<TerminalProductionStage> =
            timings.rows().iter().map(|(stage, _)| *stage).collect();
        assert_eq!(
            stages,
            [
                TerminalProductionStage::LedgerCheck,
                TerminalProductionStage::Lowering,
                TerminalProductionStage::Optimization,
                TerminalProductionStage::Publication,
                TerminalProductionStage::BoundaryOperatorScope,
            ]
        );

        let mut callback_timings = TerminalProductionTimings::enabled();
        let produced = TerminalProductionRequest::new(&checked, "Main::run")
            .produce_with_callback_custody_timed(42u8, &mut callback_timings)
            .unwrap();
        assert_eq!(produced.callback_custody(), &42u8);
        let callback_stages: Vec<TerminalProductionStage> = callback_timings
            .rows()
            .iter()
            .map(|(stage, _)| *stage)
            .collect();
        assert_eq!(callback_stages, stages);
    }
}
