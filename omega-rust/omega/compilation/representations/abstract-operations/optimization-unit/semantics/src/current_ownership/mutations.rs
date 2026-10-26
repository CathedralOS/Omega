use crate::current_ownership::CurrentOwnership;
use crate::{OperationSite, OptimizationUnitValidationError};
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, PlaceId};
use terminal_psi::StructuralMultiplicity;

pub(super) fn insert_owned_result(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: u32,
    frontier: &mut CurrentOwnership,
    place: PlaceId,
    multiplicity: StructuralMultiplicity,
) -> Result<(), OptimizationUnitValidationError> {
    // Copyable payloads do not add disposal obligations. Their producer and
    // availability were checked before replay; result claims are replayed by
    // the caller independently of this owned-place insertion.
    if multiplicity == StructuralMultiplicity::Unrestricted {
        return Ok(());
    }
    if frontier.owned_places.insert(place, multiplicity).is_some() {
        return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive(
            OperationSite::new(function.machine, block, node),
            place,
        ));
    }
    // A new establishment replaces drained scalar storage only after the
    // operation's pin replay has established that no prior loan survives.
    frontier.partial_custody_paths.remove(&place);
    Ok(())
}

pub(super) fn reject_live_linear_claim(
    function: &PsiOptimizationFunction,
    block: BlockId,
    frontier: &CurrentOwnership,
) -> Result<(), OptimizationUnitValidationError> {
    if let Some(claim) = frontier.claims.iter().find_map(|(claim, live)| {
        (live.multiplicity == Some(StructuralMultiplicity::Linear)).then_some(*claim)
    }) {
        return Err(
            OptimizationUnitValidationError::CurrentLinearClaimAtReturn {
                machine: function.machine,
                block,
                claim,
            },
        );
    }
    Ok(())
}
