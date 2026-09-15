//! Optimizer module role: application leaf. Atomic exact edge fusion and custody rebinding.

use super::{
    AppliedStateArgumentSpecialization, PsiTransformationLedger, PsiTransformationRecord,
    StateArgumentSpecializationError, ValidatedStateArgumentSpecialization,
    VerifiedPsiOptimizationSession, rule_identity, validator_identity,
};
mod realize;

pub(super) use realize::{fused_node, realize};

pub(super) fn validated(
    session: VerifiedPsiOptimizationSession,
    validated: ValidatedStateArgumentSpecialization,
) -> Result<AppliedStateArgumentSpecialization, StateArgumentSpecializationError> {
    if validated.candidate.input != session.unit().identity {
        return Err(StateArgumentSpecializationError::StaleCandidateRevision {
            candidate: validated.candidate.input,
            current: session.unit().identity,
        });
    }
    if validated.output.identity != validated.candidate.output {
        return Err(StateArgumentSpecializationError::OutputIdentityMismatch {
            candidate: validated.candidate.output,
            reconstructed: validated.output.identity,
        });
    }
    let components = session.cycle_components().clone();
    let input = session.unit().identity;
    let terminal = session.unit().psi;
    let fuel_schedule = session.unit().fuel_schedule;
    let (verified_input, _) = session.into_parts();
    let next = VerifiedPsiOptimizationSession::from_transformed(verified_input, validated.output)
        .map_err(StateArgumentSpecializationError::TransformedValidation)?;
    // The fused edges run in component-free machines only, so the transformed
    // session must rederive the identical authenticated cycle roster.
    if *next.cycle_components() != components {
        return Err(StateArgumentSpecializationError::CandidateMismatch);
    }
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
    .map_err(StateArgumentSpecializationError::InvalidLedger)?;
    Ok(AppliedStateArgumentSpecialization {
        session: next,
        candidate: validated.candidate,
        ledger,
    })
}
