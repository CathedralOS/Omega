//! Whole-value installation preserves vacancy and intact disposal as distinct
//! contracts. Geometry alone cannot erase a nominal obligation: replacement
//! requires unrestricted multiplicity or source-retained no-code affine
//! disposal authority, no claims, and resource-free storage. A separate
//! checked authority permits reference-descriptor retirement; receiving custody
//! still proves the exact resident roster and child closure at that store. Bounds, qualifications,
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
            && discardable_store_value(types, identity, multiplicity, claims)
    })
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
            descriptor_cleanup || matches!(carrier, ByteSequenceCarrier::BoundedOwned { .. })
        }
        StructuralTypeShape::Record {
            storage:
                terminal_psi::StructuralRecordStorage::PlacedView(_)
                | terminal_psi::StructuralRecordStorage::PlacedAccessor(_),
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
                    fields_are_disposable(types.clone(), &case.fields, descriptor_cleanup, visiting)
                })
        }
        StructuralTypeShape::Reference { access, .. } => {
            descriptor_cleanup && *access != terminal_psi::StructuralAccess::Owned
        }
        StructuralTypeShape::ElementView { .. } => descriptor_cleanup,
        StructuralTypeShape::BindingCarrier => false,
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
            // This judge consumes a complete owned, claim-free value into
            // exclusive storage. Retiring its atomic cells is not a shared
            // resident observation or permission to copy a borrowed cell.
            | StructuralFieldType::Atomic(_) => true,
            StructuralFieldType::Structural(identity) => {
                disposable_storage(types.clone(), *identity, descriptor_cleanup, visiting)
            }
            StructuralFieldType::ByteSequence(carrier)
            | StructuralFieldType::PredicateByteSequence { carrier, .. } => {
                descriptor_cleanup || matches!(carrier, ByteSequenceCarrier::BoundedOwned { .. })
            }
            StructuralFieldType::Erased { .. } => false,
        })
}
