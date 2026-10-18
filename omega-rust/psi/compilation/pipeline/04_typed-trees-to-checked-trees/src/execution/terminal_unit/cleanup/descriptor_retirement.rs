//! Intrinsic descriptor lifetime at an admitted consumer's normal completion.
//!
//! Calls and scalar observations consume the same fresh permission carrier.
//! Logical copyability does not keep its physical lifetime open until return.
//! This selects disposal of that carrier only, never its referent or a nominal
//! backing owner's hook. Ordinary source loan checking still owns availability.

pub(crate) fn descriptor_retirement(
    program: &dyn validation::CheckingInput,
    source: checked_trees::CheckedUnitStructuralArgumentSourcePlan,
    reference: typed_trees::types::TypeReferenceHandle,
) -> Option<Vec<checked_trees::CheckedNormalCleanupAction>> {
    validation::reference_result_custody::stored_loan_parts(program, reference)?;
    match validation::reference_result_custody::result_multiplicity(program, reference) {
        language_semantics::Multiplicity::Unrestricted => Some(Vec::new()),
        language_semantics::Multiplicity::Affine => {
            Some(vec![checked_trees::CheckedNormalCleanupAction::Discard(
                checked_trees::CheckedUnitPartialAffineDiscardPlan {
                    source,
                    path: Vec::new(),
                    type_identity: validation::structural_type_identity(
                        program,
                        reference,
                        &[],
                        &[],
                    ),
                    case_conditions: Default::default(),
                },
            )])
        }
        // A normal observation cannot discharge a linear obligation.
        language_semantics::Multiplicity::Linear => None,
    }
}
