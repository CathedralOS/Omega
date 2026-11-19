use crate::exactly_one::exactly_one;
use semantic_vocabulary::{OperationId, PlaceId, StructuralTypeId};
use terminal_psi::StructuralRecordStorage;
use terminal_psi::{
    ByteSequenceCarrier, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralStoreDisposition, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine,
};

pub fn structural_store_value_admitted<'types>(
    machine: &TerminalMachine,
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    disposition: StructuralStoreDisposition,
    value: PlaceId,
) -> bool {
    if disposition == StructuralStoreDisposition::Vacant {
        return true;
    }
    let declaration = machine
        .structural_parameters
        .iter()
        .chain(machine.block_structural_parameters())
        .find(|parameter| parameter.place == value)
        .map(|parameter| (parameter.structural_type, parameter.multiplicity, false))
        .or_else(|| {
            machine.operations().find_map(|operation| {
                operation
                    .result
                    .structural()
                    .filter(|result| result.place == value)
                    .map(|result| {
                        (
                            result.structural_type,
                            result.multiplicity,
                            !result.claims.is_empty(),
                        )
                    })
            })
        });
    declaration.is_some_and(|(identity, multiplicity, claims)| {
        !machine
            .entry_claims
            .iter()
            .chain(&machine.block_entry_claims)
            .any(|claim| claim.input == value)
            && !machine
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == value)
            && discardable_store_value(types, identity, multiplicity, claims)
    })
}

/// A producer has reconstructed receiving storage before it publishes
/// claim transfers. Check the same incoming-value rule as the receiver,
/// joining only the actual replacement inputs to their declarations.
pub fn validate_structural_store_values(
    machine: &TerminalMachine,
    types: &[StructuralTypeDeclaration],
    stores: &[(OperationId, StructuralStoreDisposition)],
) -> Result<(), StructuralStoreValueError> {
    let replacements = machine
        .operations()
        .filter_map(|operation| {
            let terminal_psi::OperationKind::StoreStructuralField { value, .. } =
                &operation.kind
            else {
                return None;
            };
            let position = stores
                .binary_search_by_key(&operation.id, |(id, _)| *id)
                .ok()?;
            (stores[position].1 == StructuralStoreDisposition::ReplaceDiscardable)
                .then_some((operation.id, value.place))
        })
        .collect::<Vec<_>>();
    if replacements.is_empty() {
        return Ok(());
    }
    let mut values = replacements
        .iter()
        .map(|(_, place)| (*place, None, false))
        .collect::<Vec<_>>();
    values.sort_unstable_by_key(|(place, _, _)| *place);
    values.dedup_by_key(|(place, _, _)| *place);
    // This subset is bounded by the actual store roster, not the largest
    // supplied place identity. Source declarations are traversed once.
    let mut declare = |place, declaration| {
        if let Ok(position) = values.binary_search_by_key(&place, |(place, _, _)| *place)
            && values[position].1.replace(declaration).is_some()
        {
            return Err(StructuralStoreValueError::SourceDeclaration(place));
        }
        Ok(())
    };
    for parameter in machine
        .structural_parameters
        .iter()
        .chain(machine.block_structural_parameters())
    {
        declare(
            parameter.place,
            (parameter.structural_type, parameter.multiplicity, false),
        )?;
    }
    for result in machine
        .operations()
        .filter_map(|operation| operation.result.structural())
    {
        declare(
            result.place,
            (
                result.structural_type,
                result.multiplicity,
                !result.claims.is_empty(),
            ),
        )?;
    }
    for place in machine
        .entry_claims
        .iter()
        .chain(&machine.block_entry_claims)
        .map(|claim| claim.input)
        .chain(
            machine
                .content_entry_claims
                .iter()
                .map(|claim| claim.input.root),
        )
    {
        if let Ok(position) = values.binary_search_by_key(&place, |(place, _, _)| *place) {
            values[position].2 = true;
        }
    }
    for (operation, place) in replacements {
        let position = values
            .binary_search_by_key(&place, |(place, _, _)| *place)
            .map_err(|_| StructuralStoreValueError::SourceDeclaration(place))?;
        let (_, declaration, entry_claims) = values[position];
        let (identity, multiplicity, result_claims) =
            declaration.ok_or(StructuralStoreValueError::SourceDeclaration(place))?;
        if !discardable_store_value(
            types.iter(),
            identity,
            multiplicity,
            entry_claims || result_claims,
        ) {
            return Err(StructuralStoreValueError::CannotDiscard {
                operation,
                multiplicity,
            });
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralStoreValueError {
    SourceDeclaration(PlaceId),
    CannotDiscard {
        operation: OperationId,
        multiplicity: StructuralMultiplicity,
    },
}

pub fn discardable_store_value<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    identity: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
    has_claims: bool,
) -> bool {
    let descriptor_cleanup = types
        .clone()
        .any(|declaration| declaration.id == identity && declaration.descriptor_cleanup);
    (multiplicity == StructuralMultiplicity::Unrestricted
        || multiplicity == StructuralMultiplicity::Affine
            && types.clone().any(|declaration| {
                declaration.id == identity
                    && (declaration.no_code_affine_cleanup || descriptor_cleanup)
            }))
        && !has_claims
        && disposable_storage(types, identity, descriptor_cleanup, &mut Vec::new())
}

fn disposable_storage<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    identity: StructuralTypeId,
    descriptor_cleanup: bool,
    visiting: &mut Vec<StructuralTypeId>,
) -> bool {
    if visiting.contains(&identity) {
        return false;
    }
    let Some(declaration) =
        exactly_one(types.clone(), |declaration| declaration.id == identity)
    else {
        return false;
    };
    visiting.push(identity);
    let valid = match &declaration.shape {
        StructuralTypeShape::Opaque => declaration.no_code_affine_cleanup,
        StructuralTypeShape::TypeParameter => false,
        StructuralTypeShape::PrimitiveScalar(_) => true,
        StructuralTypeShape::AtomicCell(_) => true,
        StructuralTypeShape::ByteSequence(carrier) => {
            descriptor_cleanup || matches!(carrier, ByteSequenceCarrier::BoundedOwned { .. })
        }
        StructuralTypeShape::Record {
            storage:
                StructuralRecordStorage::AccessView(_) | StructuralRecordStorage::Accessor(_),
            ..
        } => false,
        StructuralTypeShape::Record { fields, .. } => {
            fields_are_disposable(types.clone(), fields, descriptor_cleanup, visiting)
        }
        StructuralTypeShape::FixedArray { element, .. } => {
            disposable_storage(types.clone(), *element, descriptor_cleanup, visiting)
        }
        StructuralTypeShape::Sum { cases } => cases.iter().all(|case| {
            fields_are_disposable(types.clone(), &case.fields, descriptor_cleanup, visiting)
        }),
        StructuralTypeShape::Mixed { fields, cases } => {
            fields_are_disposable(types.clone(), fields, descriptor_cleanup, visiting)
                && cases.iter().all(|case| {
                    fields_are_disposable(
                        types.clone(),
                        &case.fields,
                        descriptor_cleanup,
                        visiting,
                    )
                })
        }
        StructuralTypeShape::Reference { access, .. } => {
            descriptor_cleanup && *access != terminal_psi::StructuralAccess::Owned
        }
        StructuralTypeShape::ElementView { .. } => descriptor_cleanup,
    };
    visiting.pop();
    valid
}

fn fields_are_disposable<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    fields: &[StructuralFieldDeclaration],
    descriptor_cleanup: bool,
    visiting: &mut Vec<StructuralTypeId>,
) -> bool {
    fields
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .all(|field| match &field.field_type {
            StructuralFieldType::Scalar(_)
            | StructuralFieldType::BoundedInteger(_)
            | StructuralFieldType::RefinedScalar(_)
            | StructuralFieldType::IeeeFloat(_)
            | StructuralFieldType::Atomic(_)
            | StructuralFieldType::AtomicBoundedInteger(_) => true,
            StructuralFieldType::Structural(identity) => {
                disposable_storage(types.clone(), *identity, descriptor_cleanup, visiting)
            }
            StructuralFieldType::ByteSequence(carrier)
            | StructuralFieldType::PredicateByteSequence { carrier, .. } => {
                descriptor_cleanup
                    || matches!(carrier, ByteSequenceCarrier::BoundedOwned { .. })
            }
            StructuralFieldType::Erased { .. } => false,
        })
}
