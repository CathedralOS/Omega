//! Whole-value installation preserves vacancy and intact disposal as distinct
//! contracts. Geometry alone cannot erase a nominal obligation: replacement
//! requires the exact value declaration's unrestricted multiplicity, no claims,
//! and storage without retained permission resources. Bounds, qualifications,
//! absence of overlapping debt and active loan authority remain separate.
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralStoreDisposition, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
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
            && unrestricted_store_value(types, identity, multiplicity, claims)
    })
}

pub fn unrestricted_store_value<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    identity: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
    has_claims: bool,
) -> bool {
    multiplicity == StructuralMultiplicity::Unrestricted
        && !has_claims
        && resource_free_storage(types, identity, &mut Vec::new())
}

fn resource_free_storage<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    identity: StructuralTypeId,
    visiting: &mut Vec<StructuralTypeId>,
) -> bool {
    if visiting.contains(&identity) {
        return false;
    }
    let mut matching = types
        .clone()
        .filter(|declaration| declaration.id == identity);
    let Some(declaration) = matching.next() else {
        return false;
    };
    if matching.next().is_some() {
        return false;
    }
    visiting.push(identity);
    let valid = match &declaration.shape {
        StructuralTypeShape::PrimitiveScalar(_) => true,
        StructuralTypeShape::ByteSequence(carrier) => {
            matches!(carrier, ByteSequenceCarrier::BoundedOwned { .. })
        }
        StructuralTypeShape::Record { fields, .. } => {
            fields_are_resource_free(types.clone(), fields, visiting)
        }
        StructuralTypeShape::FixedArray { element, .. } => {
            resource_free_storage(types.clone(), *element, visiting)
        }
        StructuralTypeShape::Sum { cases } => cases
            .iter()
            .all(|case| fields_are_resource_free(types.clone(), &case.fields, visiting)),
        StructuralTypeShape::Mixed { fields, cases } => {
            fields_are_resource_free(types.clone(), fields, visiting)
                && cases
                    .iter()
                    .all(|case| fields_are_resource_free(types.clone(), &case.fields, visiting))
        }
        StructuralTypeShape::BindingCarrier
        | StructuralTypeShape::Reference { .. }
        | StructuralTypeShape::ElementView { .. } => false,
    };
    visiting.pop();
    valid
}

fn fields_are_resource_free<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    fields: &[StructuralFieldDeclaration],
    visiting: &mut Vec<StructuralTypeId>,
) -> bool {
    fields
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .all(|field| match &field.field_type {
            StructuralFieldType::Scalar(_)
            | StructuralFieldType::BoundedInteger(_)
            | StructuralFieldType::RefinedScalar(_)
            | StructuralFieldType::IeeeFloat(_) => true,
            StructuralFieldType::Structural(identity) => {
                resource_free_storage(types.clone(), *identity, visiting)
            }
            StructuralFieldType::ByteSequence(carrier)
            | StructuralFieldType::PredicateByteSequence { carrier, .. } => {
                matches!(carrier, ByteSequenceCarrier::BoundedOwned { .. })
            }
            StructuralFieldType::Atomic(_) | StructuralFieldType::Erased { .. } => false,
        })
}
