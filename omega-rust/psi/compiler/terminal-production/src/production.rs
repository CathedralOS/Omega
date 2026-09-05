use checked_trees::CheckedTrees;
use checked_trees_to_lowered_psi::{LoweringError, lower_machine, select_terminal_machine};
use lowered_psi::{LoweredPsi, LoweredSelectedIeeeFloatFmaOccurrence, LoweredSourceCallOccurrence};
use lowered_psi_to_lowered_psi::{PsiOptimizationStageError, run_psi_optimization};
use lowered_psi_to_terminal_psi::{
    CheckedBoundaryOperatorApplicationScope, finalize_terminal_artifact,
};
use semantic_vocabulary::MachineId;
use terminal_codec::terminal_psi_identity;
use terminal_psi::TerminalMachineResult;
/// Canonical Terminal output coupled to its non-caller-authored checked D29
/// demand scope.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "checked Terminal production retains boundary-operator demand custody"]
pub struct ProducedTerminalArtifact {
    artifact: terminal_codec::CanonicalTerminalArtifact,
    boundary_operator_scope: CheckedBoundaryOperatorApplicationScope,
    selected_ieee_float_fma_occurrences: Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
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

    pub fn into_parts(
        self,
    ) -> (
        terminal_codec::CanonicalTerminalArtifact,
        CheckedBoundaryOperatorApplicationScope,
        Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
    ) {
        (
            self.artifact,
            self.boundary_operator_scope,
            self.selected_ieee_float_fma_occurrences,
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

    pub fn into_parts(
        self,
    ) -> (
        terminal_codec::CanonicalTerminalArtifact,
        CheckedBoundaryOperatorApplicationScope,
        C,
        Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
    ) {
        (
            self.artifact,
            self.boundary_operator_scope,
            self.callback_custody,
            self.selected_ieee_float_fma_occurrences,
        )
    }

    pub fn into_parts_with_source_calls(
        self,
    ) -> (
        terminal_codec::CanonicalTerminalArtifact,
        CheckedBoundaryOperatorApplicationScope,
        C,
        Vec<LoweredSourceCallOccurrence>,
        Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
    ) {
        (
            self.artifact,
            self.boundary_operator_scope,
            self.callback_custody,
            self.source_call_occurrences,
            self.selected_ieee_float_fma_occurrences,
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

/// Durable source-to-Terminal join for one checked `ProgramEntry`.
///
/// The source-signature identity is computed by the build-owned declaration
/// checker and supplied here as opaque digest bytes. The remaining fields are
/// reconstructed by this producer from the exact checked machine and the
/// canonical Terminal module. This receipt owns no target, calling convention,
/// runtime roots, image, installation, or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedProgramEntryTerminalReceipt {
    source_signature_identity: [u8; 32],
    source_machine_name: String,
    terminal_psi_identity: terminal_psi::TerminalPsiIdentity,
    terminal_entry: MachineId,
}

impl CheckedProgramEntryTerminalReceipt {
    pub const fn source_signature_identity(&self) -> [u8; 32] {
        self.source_signature_identity
    }

    pub fn source_machine_name(&self) -> &str {
        &self.source_machine_name
    }

    pub const fn terminal_psi_identity(&self) -> terminal_psi::TerminalPsiIdentity {
        self.terminal_psi_identity
    }

    pub const fn terminal_entry(&self) -> MachineId {
        self.terminal_entry
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

    pub fn into_parts(
        self,
    ) -> (
        terminal_codec::CanonicalTerminalArtifact,
        CheckedProgramEntryTerminalReceipt,
        CheckedBoundaryOperatorApplicationScope,
        Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
    ) {
        (
            self.artifact,
            self.receipt,
            self.boundary_operator_scope,
            self.selected_ieee_float_fma_occurrences,
        )
    }
}

/// Lower one checked source product, execute the identity Psi optimization
/// phase, and then cross the explicit Terminal publication boundary.
pub fn produce_terminal_artifact(
    checked: &CheckedTrees,
    machine_name: &str,
) -> Result<terminal_codec::CanonicalTerminalArtifact, TerminalArtifactProductionError> {
    produce_terminal_artifact_with_optimizations(
        checked,
        machine_name,
        optimization::PsiOptimizationSelections::default(),
    )
}

/// Lower one checked source product through an exact selected Psi phase and
/// cross the explicit Terminal publication boundary.
pub fn produce_terminal_artifact_with_optimizations(
    checked: &CheckedTrees,
    machine_name: &str,
    selections: optimization::PsiOptimizationSelections,
) -> Result<terminal_codec::CanonicalTerminalArtifact, TerminalArtifactProductionError> {
    let lowered =
        lower_machine(checked, machine_name).map_err(TerminalArtifactProductionError::Lowering)?;
    let optimized = run_psi_optimization(lowered, selections)
        .map_err(TerminalArtifactProductionError::Optimization)?;
    finalize_terminal_artifact(&optimized).map_err(TerminalArtifactProductionError::Artifact)
}

/// Produce canonical Terminal semantics while preserving the exact checked
/// D29 demand scope needed by compiler-owned native evidence derivation.
pub fn produce_terminal_artifact_with_checked_boundary_operator_scope(
    checked: &CheckedTrees,
    machine_name: &str,
) -> Result<ProducedTerminalArtifact, TerminalArtifactProductionError> {
    produce_terminal_artifact_with_checked_boundary_operator_scope_and_optimizations(
        checked,
        machine_name,
        optimization::PsiOptimizationSelections::default(),
    )
}

/// Produce canonical Terminal semantics from an exact selected Psi phase while
/// preserving the checked D29 demand scope.
pub fn produce_terminal_artifact_with_checked_boundary_operator_scope_and_optimizations(
    checked: &CheckedTrees,
    machine_name: &str,
    selections: optimization::PsiOptimizationSelections,
) -> Result<ProducedTerminalArtifact, TerminalArtifactProductionError> {
    let lowered =
        lower_machine(checked, machine_name).map_err(TerminalArtifactProductionError::Lowering)?;
    let optimized = run_psi_optimization(lowered, selections)
        .map_err(TerminalArtifactProductionError::Optimization)?;
    let artifact = finalize_terminal_artifact(&optimized)
        .map_err(TerminalArtifactProductionError::Artifact)?;
    let lowered = optimized.into_lowered();
    let boundary_operator_scope = checked_boundary_operator_scope(checked, &artifact, &lowered)
        .map_err(TerminalArtifactProductionError::Lowering)?;
    Ok(ProducedTerminalArtifact {
        artifact,
        boundary_operator_scope,
        selected_ieee_float_fma_occurrences: lowered.selected_ieee_float_fma_occurrences,
    })
}

/// Produce the canonical source-free artifact without losing the caller's
/// exact callback-use sidecar.
///
/// The sidecar remains opaque because callback placement is target-owned Omega
/// evidence rather than Terminal-Psi vocabulary. Success and rejection both
/// return it by value in its original order.
pub fn produce_terminal_artifact_with_callback_custody<C>(
    checked: &CheckedTrees,
    machine_name: &str,
    callback_custody: C,
) -> Result<
    ProducedTerminalArtifactWithCallbackCustody<C>,
    CallbackCustodyTerminalArtifactProductionError<C>,
> {
    produce_terminal_artifact_with_callback_custody_and_optimizations(
        checked,
        machine_name,
        callback_custody,
        optimization::PsiOptimizationSelections::default(),
    )
}

/// Produce a selected canonical Terminal artifact without losing the caller's
/// exact callback-use sidecar.
pub fn produce_terminal_artifact_with_callback_custody_and_optimizations<C>(
    checked: &CheckedTrees,
    machine_name: &str,
    callback_custody: C,
    selections: optimization::PsiOptimizationSelections,
) -> Result<
    ProducedTerminalArtifactWithCallbackCustody<C>,
    CallbackCustodyTerminalArtifactProductionError<C>,
> {
    let lowered = match lower_machine(checked, machine_name) {
        Ok(lowered) => lowered,
        Err(error) => {
            return Err(CallbackCustodyTerminalArtifactProductionError {
                error: TerminalArtifactProductionError::Lowering(error),
                callback_custody,
            });
        }
    };
    let optimized = match run_psi_optimization(lowered, selections) {
        Ok(optimized) => optimized,
        Err(error) => {
            return Err(CallbackCustodyTerminalArtifactProductionError {
                error: TerminalArtifactProductionError::Optimization(error),
                callback_custody,
            });
        }
    };
    let artifact = match finalize_terminal_artifact(&optimized) {
        Ok(artifact) => artifact,
        Err(error) => {
            return Err(CallbackCustodyTerminalArtifactProductionError {
                error: TerminalArtifactProductionError::Artifact(error),
                callback_custody,
            });
        }
    };
    let lowered = optimized.into_lowered();
    let boundary_operator_scope =
        match checked_boundary_operator_scope(checked, &artifact, &lowered) {
            Ok(scope) => scope,
            Err(error) => {
                return Err(CallbackCustodyTerminalArtifactProductionError {
                    error: TerminalArtifactProductionError::Lowering(error),
                    callback_custody,
                });
            }
        };
    Ok(ProducedTerminalArtifactWithCallbackCustody {
        boundary_operator_scope,
        artifact,
        callback_custody,
        source_call_occurrences: lowered.source_call_occurrences,
        selected_ieee_float_fma_occurrences: lowered.selected_ieee_float_fma_occurrences,
    })
}

/// Produce a canonical Terminal artifact while retaining the exact checked
/// `ProgramEntry` to Terminal-entry association.
///
/// `source_signature_identity` is an opaque domain-separated identity
/// computed while the complete typed `ProgramEntry` declaration is still
/// available. This stage does not interpret or recreate it. A later Omega
/// settlement must independently compare it with the retained source
/// signature before granting native custody.
pub fn produce_program_entry_terminal_artifact(
    checked: &CheckedTrees,
    machine_name: &str,
    source_signature_identity: [u8; 32],
) -> Result<ProducedProgramEntryTerminalArtifact, TerminalArtifactProductionError> {
    produce_program_entry_terminal_artifact_with_optimizations(
        checked,
        machine_name,
        source_signature_identity,
        optimization::PsiOptimizationSelections::default(),
    )
}

/// Receipt-coupled `ProgramEntry` production with explicit target-neutral
/// optimization selections.
pub fn produce_program_entry_terminal_artifact_with_optimizations(
    checked: &CheckedTrees,
    machine_name: &str,
    source_signature_identity: [u8; 32],
    optimization_selections: optimization::PsiOptimizationSelections,
) -> Result<ProducedProgramEntryTerminalArtifact, TerminalArtifactProductionError> {
    let selection = select_terminal_machine(checked, machine_name)
        .map_err(TerminalArtifactProductionError::Lowering)?;
    let source_machine_name = selection.name.clone();
    let lowered =
        lower_machine(checked, machine_name).map_err(TerminalArtifactProductionError::Lowering)?;
    let optimized = run_psi_optimization(lowered, optimization_selections)
        .map_err(TerminalArtifactProductionError::Optimization)?;
    let optimized_lowered = optimized.lowered();
    let entry_matches = optimized_lowered
        .semantic_module
        .machines
        .iter()
        .filter(|machine| machine.id == optimized_lowered.semantic_module.entry)
        .collect::<Vec<_>>();
    let [entry] = entry_matches.as_slice() else {
        return Err(TerminalArtifactProductionError::EntryReceipt(
            ProgramEntryTerminalReceiptError::TerminalEntryMultiplicity(entry_matches.len()),
        ));
    };
    if entry.result != TerminalMachineResult::Unit {
        return Err(TerminalArtifactProductionError::EntryReceipt(
            ProgramEntryTerminalReceiptError::NonUnitEntry,
        ));
    }
    let terminal_psi_identity = terminal_psi_identity(&optimized_lowered.semantic_module)
        .map_err(ProgramEntryTerminalReceiptError::TerminalIdentity)
        .map_err(TerminalArtifactProductionError::EntryReceipt)?;
    let terminal_entry = optimized_lowered.semantic_module.entry;
    let artifact = finalize_terminal_artifact(&optimized)
        .map_err(TerminalArtifactProductionError::Artifact)?;
    let lowered = optimized.into_lowered();
    if artifact.manifest().semantic() != terminal_psi_identity {
        return Err(TerminalArtifactProductionError::EntryReceipt(
            ProgramEntryTerminalReceiptError::ArtifactSemanticIdentityMismatch,
        ));
    }
    let boundary_operator_scope = checked_boundary_operator_scope(checked, &artifact, &lowered)
        .map_err(TerminalArtifactProductionError::Lowering)?;
    Ok(ProducedProgramEntryTerminalArtifact {
        boundary_operator_scope,
        artifact,
        receipt: CheckedProgramEntryTerminalReceipt {
            source_signature_identity,
            source_machine_name,
            terminal_psi_identity,
            terminal_entry,
        },
        selected_ieee_float_fma_occurrences: lowered.selected_ieee_float_fma_occurrences,
    })
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
