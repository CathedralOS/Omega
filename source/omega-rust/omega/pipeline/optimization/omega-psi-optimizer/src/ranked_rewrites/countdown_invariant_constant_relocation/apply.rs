//! Optimizer module role: application leaf. Atomic exact node relocation and custody rebinding.

use super::*;

mod realize;

pub(super) use realize::{operation_location, realize};

pub(super) fn validated(
    session: VerifiedPsiOptimizationSession,
    validated: ValidatedCountdownInvariantConstantRelocation,
) -> Result<AppliedCountdownInvariantConstantRelocation, CountdownInvariantConstantRelocationError>
{
    if validated.candidate.input != session.unit().identity {
        return Err(
            CountdownInvariantConstantRelocationError::StaleCandidateRevision {
                candidate: validated.candidate.input,
                current: session.unit().identity,
            },
        );
    }
    if validated.output.identity != validated.candidate.output {
        return Err(
            CountdownInvariantConstantRelocationError::OutputIdentityMismatch {
                candidate: validated.candidate.output,
                reconstructed: validated.output.identity,
            },
        );
    }
    let input = session.unit().identity;
    let terminal = session.unit().psi;
    let fuel_schedule = session.unit().fuel_schedule;
    let (verified_input, _) = session.into_parts();
    let next = VerifiedPsiOptimizationSession::from_transformed(verified_input, validated.output)
        .map_err(CountdownInvariantConstantRelocationError::TransformedValidation)?;
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
    .map_err(CountdownInvariantConstantRelocationError::InvalidLedger)?;
    Ok(AppliedCountdownInvariantConstantRelocation {
        session: next,
        candidate: validated.candidate,
        ledger,
    })
}

pub(super) fn reconstruct_custody(
    session: &VerifiedPsiOptimizationSession,
) -> Result<(), CountdownInvariantConstantRelocationError> {
    session
        .counted_loop_analysis()
        .map_err(CountdownInvariantConstantRelocationError::CountedLoop)?;
    session
        .countdown_invariant_constant_analysis()
        .map_err(CountdownInvariantConstantRelocationError::InvariantConstant)?;
    session
        .countdown_invariant_constant_placement_analysis()
        .map_err(CountdownInvariantConstantRelocationError::ReconstructedPlacement)?;
    Ok(())
}
