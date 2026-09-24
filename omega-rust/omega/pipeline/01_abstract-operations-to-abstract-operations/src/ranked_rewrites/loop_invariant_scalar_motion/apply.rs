//! Optimizer module role: application leaf. Atomic exact node relocation and custody rebinding.

use super::{
    AppliedLoopInvariantScalarMotion, LoopInvariantScalarMotionError, PsiTransformationLedger,
    PsiTransformationRecord, ValidatedLoopInvariantScalarMotion, VerifiedPsiOptimizationSession,
    rule_identity, validator_identity,
};
mod realize;

pub(super) use realize::{operation_location, realize};

pub(super) fn validated(
    session: VerifiedPsiOptimizationSession,
    validated: ValidatedLoopInvariantScalarMotion,
) -> Result<AppliedLoopInvariantScalarMotion, LoopInvariantScalarMotionError> {
    if validated.candidate.input != session.unit().identity {
        return Err(LoopInvariantScalarMotionError::StaleCandidateRevision {
            candidate: validated.candidate.input,
            current: session.unit().identity,
        });
    }
    if validated.output.identity != validated.candidate.output {
        return Err(LoopInvariantScalarMotionError::OutputIdentityMismatch {
            candidate: validated.candidate.output,
            reconstructed: validated.output.identity,
        });
    }
    let input = session.unit().identity;
    let terminal = session.unit().psi;
    let fuel_schedule = session.unit().fuel_schedule;
    let (verified_input, _) = session.into_parts();
    let next = VerifiedPsiOptimizationSession::from_transformed(verified_input, validated.output)
        .map_err(LoopInvariantScalarMotionError::TransformedValidation)?;
    reconstruct_custody(&next)?;
    let record = PsiTransformationRecord {
        rule: rule_identity(),
        candidate: validated.candidate.identity,
        validator: validator_identity(),
        input,
        output: next.unit().identity,
        pruned_machines: Vec::new(),
        provenance: validated.provenance,
    };
    let ledger = PsiTransformationLedger::new(
        terminal,
        fuel_schedule,
        input,
        next.unit().identity,
        vec![record],
    )
    .map_err(LoopInvariantScalarMotionError::InvalidLedger)?;
    Ok(AppliedLoopInvariantScalarMotion {
        session: next,
        candidate: validated.candidate,
        ledger,
    })
}

/// Reconstruct every loop-carried custody the transformed unit still owns.
/// Counted-loop, invariant-constant, and placement custody reconstruct for
/// each surviving certificate-bearing component; uncertified Natural and
/// unranked components keep their component and freeze custody through
/// `from_transformed` alone.
pub(super) fn reconstruct_custody(
    session: &VerifiedPsiOptimizationSession,
) -> Result<(), LoopInvariantScalarMotionError> {
    if !session.ranking_certificates().certificates().is_empty() {
        session
            .counted_loop_analysis()
            .map_err(LoopInvariantScalarMotionError::CountedLoop)?;
        session
            .countdown_invariant_constant_analysis()
            .map_err(LoopInvariantScalarMotionError::InvariantConstant)?;
        session
            .countdown_invariant_constant_placement_analysis()
            .map_err(LoopInvariantScalarMotionError::ReconstructedPlacement)?;
    }
    Ok(())
}
