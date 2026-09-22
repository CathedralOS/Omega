//! Optimizer module role: application leaf. Atomic exact observation folding and custody retention.

use super::{
    AppliedFieldValueSpecialization, FieldValueSpecializationError, PsiTransformationLedger,
    PsiTransformationRecord, ValidatedFieldValueSpecialization, VerifiedPsiOptimizationSession,
    rule_identity, validator_identity,
};
mod realize;
mod substitution;

pub(super) use realize::{realize, transform_function};

pub(super) fn validated(
    session: VerifiedPsiOptimizationSession,
    validated: ValidatedFieldValueSpecialization,
) -> Result<AppliedFieldValueSpecialization, FieldValueSpecializationError> {
    if validated.candidate.input != session.unit().identity {
        return Err(FieldValueSpecializationError::StaleCandidateRevision {
            candidate: validated.candidate.input,
            current: session.unit().identity,
        });
    }
    if validated.output.identity != validated.candidate.output {
        return Err(FieldValueSpecializationError::OutputIdentityMismatch {
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
        .map_err(FieldValueSpecializationError::TransformedValidation)?;
    // The folded nodes run in component-free machines only, so the transformed
    // session must rederive the identical authenticated cycle roster.
    if *next.cycle_components() != components {
        return Err(FieldValueSpecializationError::CandidateMismatch);
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
    .map_err(FieldValueSpecializationError::InvalidLedger)?;
    Ok(AppliedFieldValueSpecialization {
        session: next,
        candidate: validated.candidate,
        ledger,
    })
}
