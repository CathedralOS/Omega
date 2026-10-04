//! Sequence the selected Psi lowering, optimization and publication owners.

use checked_trees::CheckedTrees;
use checked_trees_to_lowered_psi::{
    LoweringError, ProofSearchExhaustion, TerminalMachineSelection, lower_machine,
    select_terminal_machine,
};
use diagnostics::{Diagnostic, ExceededProvision};
use lowered_psi::LoweredPsi;
use lowered_psi_to_lowered_psi::{
    PsiOptimizationStageError, PsiOptimizationStageResult, PsiRewriteRefusalReason,
    run_psi_optimization,
};
use lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope;

use crate::custody::{ProducedTerminalArtifact, TerminalProductionCustody};
use crate::stage_timings::{TerminalProductionStage, TerminalProductionTimings};

pub use lowered_psi_to_terminal_psi::ProgramEntryTerminalReceiptError;
use lowered_psi_to_terminal_psi::{prepare_program_entry_receipt, rejoin_program_entry_receipt};

/// Exact borrowed source inputs and target-neutral selection for Terminal production.
pub struct TerminalProductionRequest<'a> {
    pub checked: &'a CheckedTrees,
    pub machine: TerminalMachineSelection<'a>,
    pub optimization_selections: optimization::PsiOptimizationSelections,
}

impl<'a> TerminalProductionRequest<'a> {
    /// Select the identity optimization phase by default.
    pub fn new(checked: &'a CheckedTrees, machine: TerminalMachineSelection<'a>) -> Self {
        Self {
            checked,
            machine,
            optimization_selections: optimization::PsiOptimizationSelections::default(),
        }
    }

    /// Produce the canonical Terminal artifact beside the custody the caller
    /// asked to retain.
    ///
    /// Every product shares one instrumented body: checked-input lowering,
    /// selected optimization, the `ProgramEntry` receipt legs when an entry
    /// identity is supplied, publication, and the checked boundary-operator
    /// scope. The source-signature digest remains opaque; later Omega
    /// settlement must independently compare it with the retained source
    /// signature.
    pub fn produce(
        self,
        custody: TerminalProductionCustody<'_>,
    ) -> Result<ProducedTerminalArtifact, TerminalArtifactProductionError> {
        let TerminalProductionCustody {
            entry_identity,
            retain_unoptimized,
            timings,
        } = custody;
        match self.produce_retained(entry_identity, retain_unoptimized, timings) {
            Ok(produced) => Ok(ProducedTerminalArtifact {
                artifact: produced.artifact,
                receipt: produced.receipt,
                unoptimized: produced.unoptimized,
                boundary_operator_scope: produced.boundary_operator_scope,
                source_call_occurrences: produced.source_call_occurrences,
                selected_ieee_float_comparison_occurrences: produced
                    .selected_ieee_float_comparison_occurrences,
                selected_integer_comparison_occurrences: produced
                    .selected_integer_comparison_occurrences,
                withheld_rewrites: produced.withheld_rewrites,
            }),
            Err(error) => Err(error),
        }
    }

    fn produce_retained(
        self,
        entry_identity: Option<[u8; 32]>,
        retain_unoptimized: bool,
        timings: &mut TerminalProductionTimings,
    ) -> Result<ProducedTerminalArtifact, TerminalArtifactProductionError> {
        let checked = self.checked;
        // The receipt names the selected checked machine, so the selection is
        // rejoined before lowering only when an entry receipt is requested.
        let entry_selection = match entry_identity {
            Some(_) => Some(
                timings
                    .record_result(TerminalProductionStage::MachineSelection, || {
                        select_terminal_machine(checked, self.machine)
                    })
                    .map_err(TerminalArtifactProductionError::Lowering)?,
            ),
            None => None,
        };
        let (unoptimized, optimized) = self.lower_and_optimize(retain_unoptimized, timings)?;
        let entry_receipt = match (entry_identity, entry_selection) {
            (Some(source_signature_identity), Some(selection)) => Some(
                timings
                    .record_result(TerminalProductionStage::EntryReceipt, || {
                        prepare_program_entry_receipt(
                            checked,
                            selection,
                            source_signature_identity,
                            optimized.lowered(),
                        )
                    })
                    .map_err(TerminalArtifactProductionError::EntryReceipt)?,
            ),
            _ => None,
        };
        // Publication consumes the stage result, so the pass decisions are read
        // off it first. They explain the published module rather than changing
        // it: a withheld rewrite leaves the artifact exactly as authored.
        let withheld_rewrites = withheld_rewrites(&optimized);
        let (artifact, lowered) = timings
            .record_result(TerminalProductionStage::Publication, || {
                publish_terminal_artifact(optimized, checked)
            })?;
        let receipt = match entry_receipt {
            Some(receipt) => {
                rejoin_program_entry_receipt(&receipt, &artifact)
                    .map_err(TerminalArtifactProductionError::EntryReceipt)?;
                Some(receipt)
            }
            None => None,
        };
        let boundary_operator_scope = timings
            .record_result(TerminalProductionStage::BoundaryOperatorScope, || {
                checked_boundary_operator_scope(checked, &artifact, &lowered)
            })
            .map_err(TerminalArtifactProductionError::Lowering)?;
        Ok(ProducedTerminalArtifact {
            artifact,
            receipt,
            unoptimized,
            boundary_operator_scope,
            source_call_occurrences: lowered.source_call_occurrences,
            selected_ieee_float_comparison_occurrences: lowered
                .selected_ieee_float_comparison_occurrences,
            selected_integer_comparison_occurrences: lowered
                .selected_integer_comparison_occurrences,
            withheld_rewrites,
        })
    }

    /// Lower once, retain the unoptimized module when asked, then run the
    /// selected optimization on the same lowering.
    fn lower_and_optimize(
        self,
        retain_unoptimized: bool,
        timings: &mut TerminalProductionTimings,
    ) -> Result<(Option<LoweredPsi>, PsiOptimizationStageResult), TerminalArtifactProductionError>
    {
        let lowered = timings
            .record_result(TerminalProductionStage::Lowering, || {
                lower_machine(self.checked, self.machine)
            })
            .map_err(TerminalArtifactProductionError::Lowering)?;
        let unoptimized = retain_unoptimized.then(|| lowered.clone());
        let optimized = timings
            .record_result(TerminalProductionStage::Optimization, || {
                run_psi_optimization(lowered, self.optimization_selections)
            })
            .map_err(TerminalArtifactProductionError::Optimization)?;
        Ok((unoptimized, optimized))
    }
}

/// Project stage 06's refusal records onto the shared optimization decision
/// vocabulary reports already speak.
///
/// A refusal is a skipped decision with a reason, which is the shape
/// [optimization reporting](../../../../../../wiki/spec/build/optimizations.md)
/// defines; the stage's own reasons name how the proposal failed, so each maps
/// onto the reason code that already means it. A changed proof question is the
/// independent validator rejecting the proposal; dangling proof context is an
/// incomplete proof for the proposed module.
fn withheld_rewrites(
    optimized: &PsiOptimizationStageResult,
) -> Vec<optimization_core::WithheldRewrite> {
    optimized
        .refusals()
        .iter()
        .map(|refusal| {
            let reason = match refusal.reason() {
                PsiRewriteRefusalReason::ChangedProofQuestion => {
                    optimization_core::OptimizationReasonCode::ValidationFailed
                }
                PsiRewriteRefusalReason::ProposalLeftProofContextDangling => {
                    optimization_core::OptimizationReasonCode::ProofIncomplete
                }
            };
            optimization_core::WithheldRewrite::new(refusal.optimization(), reason)
        })
        .collect()
}

/// Publication consumes exactly the validated optimization result; retained
/// source evidence is extracted only after canonical publication succeeds.
///
/// The checked mathematical signature — `Some` exactly when the program
/// carried `let`/`boundary let` mathematical declarations — is encoded onto
/// the artifact at this boundary, so a declaration-bearing program can never
/// publish an artifact that silently omits the checked signature the
/// receiver's kernel re-decides.
fn publish_terminal_artifact(
    optimized: PsiOptimizationStageResult,
    checked: &CheckedTrees,
) -> Result<(terminal_codec::CanonicalTerminalArtifact, LoweredPsi), TerminalArtifactProductionError>
{
    let artifact = lowered_psi_to_terminal_psi::finalize_terminal_artifact(
        &optimized,
        checked.facts.proof.mathematical_signature.as_ref(),
    )
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

impl TerminalArtifactProductionError {
    /// The refusal as the diagnostic compilation reports. A capacity
    /// exhaustion carries its exceeded provision — resource name, limit,
    /// declined demand and the unproved obligation as its internal-row
    /// coordinate — so the compiler-request outcome publishes `Incomplete`
    /// from that record instead of recognizing the message's wording.
    pub fn diagnostic(&self) -> Diagnostic {
        let message = self.diagnostic_message();
        match self.incomplete_capacity() {
            Some(exhaustion) => Diagnostic::incomplete(
                message,
                ExceededProvision {
                    resource: exhaustion.capacity.resource_name(),
                    limit: exhaustion.capacity.limit(),
                    requested: exhaustion.requested,
                    coordinate: exhaustion.obligation.get(),
                },
            ),
            None => Diagnostic::error(message),
        }
    }

    /// Report the production verdict without making a consumer classify a
    /// lowerer's private failures. Capacity exhaustion is incomplete evidence,
    /// not a refutation; an absent compiler route retains its own report.
    pub fn diagnostic_message(&self) -> String {
        if let Some(report) = self.unimplemented_report() {
            return report.to_owned();
        }
        if let Some(exhaustion) = self.incomplete_capacity() {
            return format!(
                "incomplete: terminal-artifact production stopped at the private \
                 `{}` proof-search capacity (provision {}) with obligation {} \
                 unproved; the module is not shown invalid",
                exhaustion.capacity.resource_name(),
                exhaustion.capacity.limit(),
                exhaustion.obligation,
            );
        }
        format!("terminal-artifact production failed: {self}")
    }

    /// The `unimplemented:` report of a refusal caused by a missing
    /// compiler route rather than by the program, when this is one.
    pub fn unimplemented_report(&self) -> Option<&str> {
        match self {
            Self::Lowering(error) => error.unimplemented_report(),
            _ => None,
        }
    }

    /// The named capacity, declined demand and unproved obligation when the
    /// refusal is a proof-search capacity exhaustion — the publication
    /// contract's `Incomplete` outcome rather than a defect of the program.
    pub fn incomplete_capacity(&self) -> Option<ProofSearchExhaustion> {
        match self {
            Self::Lowering(error) => error.incomplete_capacity(),
            _ => None,
        }
    }
}

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
