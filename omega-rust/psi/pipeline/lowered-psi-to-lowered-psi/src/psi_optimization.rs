//! Execute selected target-neutral passes and return their validated publication result.

use crate::optimization_error::PsiOptimizationStageError;

use crate::{
    control_flow_cleanup, copy_propagation, dead_scalar_elimination, global_value_numbering,
    proof_check_elision, sparse_conditional_constant_propagation,
};
use lowered_psi::LoweredPsi;
use optimization::{PRETERMINAL_PSI_PASS_CATALOG, PsiOptimization, PsiOptimizationSelections};
use terminal_codec::PsiOptimizationExecutionRecord;
use terminal_codec::{
    ProofBundleFingerprint, proof_bundle_fingerprint, terminal_psi_identity, validate_debug_map,
};
use terminal_psi::TerminalPsiIdentity;
use terminal_verifier::validate_module_for_optimization;

/// Execute the selected target-neutral optimization phase over the complete
/// unsealed Psi product.
///
/// The empty selection deliberately validates both sides of the identity
/// transformation. Selected passes execute in canonical order; the catalog is
/// exhaustive, so a future selection added without an implementation fails to
/// compile rather than being silently recorded as an executed identity — every
/// member's preterminal semantics must be stated here explicitly.
/// `StateSpecialization` rewrites the abstract-operations unit produced only
/// after this stage, so it executes in the post-terminal stage and is left out
/// of this stage's execution record.
pub fn run_psi_optimization(
    mut lowered: LoweredPsi,
    selections: PsiOptimizationSelections,
) -> Result<PsiOptimizationStageResult, PsiOptimizationStageError> {
    let (input_semantic, input_proof) = validate_carrier(&lowered)?;
    for selected in selections.as_slice() {
        match selected {
            PsiOptimization::ControlFlowCleanup => {
                lowered = control_flow_cleanup::cleanup(lowered)?;
            }
            PsiOptimization::SparseConditionalConstantPropagation => {
                lowered = sparse_conditional_constant_propagation::propagate(lowered)?;
            }
            PsiOptimization::CopyPropagation => {
                lowered = copy_propagation::propagate(lowered)?;
            }
            PsiOptimization::GlobalValueNumbering => {
                lowered = global_value_numbering::number(lowered)?;
            }
            PsiOptimization::DeadPureScalarElimination => {
                lowered = dead_scalar_elimination::eliminate(lowered)?;
            }
            PsiOptimization::ProofCheckElision => {
                lowered = proof_check_elision::elide(lowered)?;
            }
            PsiOptimization::StateSpecialization => {}
        }
    }

    let (output_semantic, output_proof) = validate_carrier(&lowered)?;
    let executed = PsiOptimizationSelections::new(
        selections
            .as_slice()
            .iter()
            .copied()
            .filter(|selected| PRETERMINAL_PSI_PASS_CATALOG.contains(selected)),
    )
    .expect("a filtered selection subset stays duplicate-free");
    let execution = PsiOptimizationExecutionRecord::new(
        executed,
        input_semantic,
        input_proof,
        output_semantic,
        output_proof,
    )
    .map_err(PsiOptimizationStageError::InvalidExecutionRecord)?;
    Ok(PsiOptimizationStageResult::new(
        lowered, selections, execution,
    ))
}

fn validate_carrier(
    lowered: &LoweredPsi,
) -> Result<(TerminalPsiIdentity, ProofBundleFingerprint), PsiOptimizationStageError> {
    validate_module_for_optimization(&lowered.semantic_module)
        .map_err(PsiOptimizationStageError::InvalidModule)?;
    let semantic = terminal_psi_identity(&lowered.semantic_module)
        .map_err(PsiOptimizationStageError::InvalidSemantic)?;
    let proof = proof_bundle_fingerprint(&lowered.proof_bundle)
        .map_err(PsiOptimizationStageError::InvalidProof)?;
    if let Some(debug_map) = lowered.debug_map.as_ref() {
        validate_debug_map(&lowered.semantic_module, debug_map)
            .map_err(PsiOptimizationStageError::InvalidDebugMap)?;
    }
    Ok((semantic, proof))
}

/// Validated output of the selected target-neutral Psi optimization phase.
///
/// Terminal publication accepts this type rather than an unvalidated lowering
/// result. Empty selection is an executed identity transformation. A selected
/// pass has no route until its rewrite and independent validator operate on
/// this complete carrier, including proof, debug, and source-custody sidecars.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "Terminal publication requires the validated Psi optimization result"]
pub struct PsiOptimizationStageResult {
    lowered: LoweredPsi,
    selections: PsiOptimizationSelections,
    execution: PsiOptimizationExecutionRecord,
}

impl PsiOptimizationStageResult {
    const fn new(
        lowered: LoweredPsi,
        selections: PsiOptimizationSelections,
        execution: PsiOptimizationExecutionRecord,
    ) -> Self {
        Self {
            lowered,
            selections,
            execution,
        }
    }

    pub const fn lowered(&self) -> &LoweredPsi {
        &self.lowered
    }

    pub const fn selections(&self) -> &PsiOptimizationSelections {
        &self.selections
    }

    pub const fn execution(&self) -> &PsiOptimizationExecutionRecord {
        &self.execution
    }

    pub fn into_lowered(self) -> LoweredPsi {
        self.lowered
    }
}
