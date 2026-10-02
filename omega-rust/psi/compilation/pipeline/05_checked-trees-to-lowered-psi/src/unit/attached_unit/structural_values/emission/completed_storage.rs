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
    Ok(terminal_psi::StructuralOperationResult {
        place,
        structural_type: parameter.structural_type,
        multiplicity: parameter.multiplicity,
        qualifications: parameter.qualifications.clone(),
        projected_qualifications: parameter.projected_qualifications.clone(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    })
}

#[cfg(test)]
mod completed_result_tests {
    use super::*;
    use crate::terminal_identities::{block_id, place_id};
    use crate::terminal_identities::{operation_id, structural_domain_id, structural_type_id};
    use terminal_psi::{
        Operation, OperationKind, OperationResult, StructuralAccess, StructuralMultiplicity,
    };

    #[test]
    fn completed_storage_retains_exact_result_and_arrival_contracts() {
        let place = place_id(1);
        let structural_type = structural_type_id(1);
        let producer = operation_id(1);
        let block = block_id(1);
        let result = terminal_psi::StructuralOperationResult {
            place,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: vec![structural_domain_id(1)],
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        };
        let declaration = StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            },
        };
        let mut operations = OperationBuffer::new(0);
        assert!(completed_result(place, &declaration, &operations, block, &[]).is_err());
        operations.push(Operation {
            id: producer,
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Structural(result.clone()),
            kind: OperationKind::EstablishRecord { fields: Vec::new() },
        });
        assert_eq!(
            completed_result(place, &declaration, &operations, block, &[]).unwrap(),
            result
        );
        let mut wrong = declaration;
        wrong.kind = StructuralPlaceKind::OperationResult {
            producer: operation_id(2),
            structural_type,
        };
        assert!(completed_result(place, &wrong, &operations, block, &[]).is_err());
        wrong.kind = StructuralPlaceKind::OperationResult {
            producer,
            structural_type: structural_type_id(2),
        };
        assert!(completed_result(place, &wrong, &operations, block, &[]).is_err());
        let duplicate = operations[0].clone();
        operations.push(duplicate);
        assert!(completed_result(place, &declaration, &operations, block, &[]).is_err());
        operations.operations.clear();

        let declaration = StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::BlockParameter { block, position: 0 },
        };
        let parameter = StructuralParameterDeclaration {
            place,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: result.multiplicity,
            access: StructuralAccess::Owned,
            qualifications: result.qualifications.clone(),
            projected_qualifications: Vec::new(),
        };
        let mut parameters = vec![parameter];
        assert_eq!(
            completed_result(place, &declaration, &operations, block, &parameters).unwrap(),
            result
        );
        parameters[0].access = StructuralAccess::SharedBorrow;
        assert_eq!(
            completed_result(place, &declaration, &operations, block, &parameters).unwrap(),
            result
        );
        assert!(
            completed_result(place, &declaration, &operations, block_id(2), &parameters).is_err()
        );
        parameters[0].position = 1;
        assert!(completed_result(place, &declaration, &operations, block, &parameters).is_err());
        parameters[0].position = 0;
        parameters.push(parameters[0].clone());
        assert!(completed_result(place, &declaration, &operations, block, &parameters).is_err());
    }
}
