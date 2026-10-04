//! Preserve producer-authored custody when publishing a completed source value.

use super::{
    BlockId, LoweringError, OperationBuffer, PlaceId, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralPlaceKind, unsupported,
};

/// Registration retains the completed storage contract, rather than creating
/// an unqualified approximation of the value. Source replay and each emitted
/// operation own establishment; a continuation owns its arrival declaration.
pub(super) fn completed_result(
    place: PlaceId,
    declaration: &StructuralPlaceDeclaration,
    operations: &OperationBuffer,
    block: BlockId,
    parameters: &[StructuralParameterDeclaration],
    arrival_claims: &[terminal_psi::StructuralResultClaimBinding],
) -> Result<terminal_psi::StructuralOperationResult, LoweringError> {
    if declaration.id != place {
        return unsupported("completed value changed its declared place");
    }
    let mut producers = operations.iter().filter_map(|operation| {
        operation
            .result
            .structural()
            .filter(|result| result.place == place)
            .map(|result| (operation.id, result))
    });
    if let Some((producer, result)) = producers.next() {
        if producers.next().is_some()
            || parameters.iter().any(|parameter| parameter.place == place)
            || declaration.kind
                != (StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type: result.structural_type,
                })
        {
            return unsupported("completed value does not have one exact producer");
        }
        return Ok(result.clone());
    }
    let StructuralPlaceDeclaration {
        kind:
            StructuralPlaceKind::BlockParameter {
                block: declared_block,
                position,
            },
        ..
    } = declaration
    else {
        return unsupported("completed value has no producer or arrival declaration");
    };
    let mut arrivals = parameters
        .iter()
        .filter(|parameter| parameter.place == place);
    let parameter = arrivals.next().ok_or(LoweringError::Unsupported(
        "completed value has no current arrival",
    ))?;
    if arrivals.next().is_some()
        || *declared_block != block
        || parameter.position != *position
        || parameter.is_self
        || parameters
            .iter()
            .filter(|parameter| parameter.position == *position)
            .count()
            != 1
    {
        return unsupported("completed value changed its exact arrival");
    }
    if arrival_claims.windows(2).any(|pair| pair[0] >= pair[1])
        || (parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            && arrival_claims.is_empty())
    {
        return unsupported("completed linear arrival lost its exact claim frontier");
    }
    Ok(terminal_psi::StructuralOperationResult {
        place,
        structural_type: parameter.structural_type,
        multiplicity: parameter.multiplicity,
        qualifications: parameter.qualifications.clone(),
        projected_qualifications: parameter.projected_qualifications.clone(),
        qualification_establishments: Vec::new(),
        claims: arrival_claims.to_vec(),
    })
}
