//! Mutation reachable through a shared structural observation.
//!
//! Dedicated atomic leaves retain mutation authority independently of their
//! scalar layout. Ordinary shared contents stay stable; a borrowed descriptor
//! may reach mutable backing outside its containing slot. This judgment reads
//! only the Terminal type catalog and grants no operation or loan authority.

use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralFieldType, StructuralTypeDeclaration,
    StructuralTypeShape,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SharedMutation {
    None,
    Inline,
    /// Borrowed backing or an unresolved declaration cannot be bounded by the
    /// selected inline slot. Consumers must preserve their alias fence.
    Unbounded,
}

/// Classify the contents selected by an exact canonical observation path.
/// Empty paths observe the whole type. Missing or ambiguous selections fail
/// closed; erased fields never contribute executable mutation authority.
pub fn shared_mutation<'types>(
    declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> SharedMutation {
    if let Some((CanonicalStructuralPathSegment::Field(field), prefix)) = path.split_last() {
        return crate::observed_field_declaration(declarations.clone(), root, prefix, *field)
            .map(|field| field_mutation(declarations, &field.field_type, &mut Vec::new()))
            .unwrap_or(SharedMutation::Unbounded);
    }
    let Some(selected) = crate::canonical_structural_path_tip(declarations.clone(), root, path)
    else {
        return SharedMutation::Unbounded;
    };
    type_mutation(declarations, selected.id, &mut Vec::new())
}

fn type_mutation<'types>(
    declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    identity: StructuralTypeId,
    active: &mut Vec<StructuralTypeId>,
) -> SharedMutation {
    // Reference cycles alone add no write authority. Every other branch of
    // the declaration still contributes its own atomic or exclusive leaves.
    if active.contains(&identity) {
        return SharedMutation::None;
    }
    let mut matching = declarations
        .clone()
        .filter(|candidate| candidate.id == identity);
    let Some(declaration) = matching.next() else {
        return SharedMutation::Unbounded;
    };
    if matching.next().is_some() {
        return SharedMutation::Unbounded;
    }
    active.push(identity);
    let result = match &declaration.shape {
        StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::FusedServiceBinding => {
            SharedMutation::None
        }
        StructuralTypeShape::ByteSequence(carrier) => byte_mutation(*carrier),
        StructuralTypeShape::Reference { referent, access } => {
            if *access != StructuralAccess::SharedBorrow
                || type_mutation(declarations.clone(), *referent, active) != SharedMutation::None
            {
                SharedMutation::Unbounded
            } else {
                SharedMutation::None
            }
        }
        StructuralTypeShape::ElementView { element } => {
            if type_mutation(declarations.clone(), *element, active) == SharedMutation::None {
                SharedMutation::None
            } else {
                SharedMutation::Unbounded
            }
        }
        StructuralTypeShape::FixedArray { element, length } => {
            if *length == 0 {
                SharedMutation::None
            } else {
                type_mutation(declarations.clone(), *element, active)
            }
        }
        StructuralTypeShape::Record { fields } => {
            fields_mutation(declarations.clone(), fields.iter(), active)
        }
        StructuralTypeShape::Sum { cases } => fields_mutation(
            declarations.clone(),
            cases.iter().flat_map(|case| &case.fields),
            active,
        ),
        StructuralTypeShape::Mixed { fields, cases } => fields_mutation(
            declarations.clone(),
            fields
                .iter()
                .chain(cases.iter().flat_map(|case| &case.fields)),
            active,
        ),
    };
    active.pop();
    result
}

fn fields_mutation<'types, 'fields>(
    declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    fields: impl Iterator<Item = &'fields terminal_psi::StructuralFieldDeclaration>,
    active: &mut Vec<StructuralTypeId>,
) -> SharedMutation {
    fields
        .filter(|field| !field.relevance.is_erased())
        .map(|field| field_mutation(declarations.clone(), &field.field_type, active))
        .max()
        .unwrap_or(SharedMutation::None)
}

fn field_mutation<'types>(
    declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    field: &StructuralFieldType,
    active: &mut Vec<StructuralTypeId>,
) -> SharedMutation {
    match field {
        StructuralFieldType::Atomic(_) => SharedMutation::Inline,
        StructuralFieldType::Structural(identity) => type_mutation(declarations, *identity, active),
        StructuralFieldType::ByteSequence(carrier)
        | StructuralFieldType::PredicateByteSequence { carrier, .. } => byte_mutation(*carrier),
        StructuralFieldType::Scalar(_)
        | StructuralFieldType::BoundedInteger(_)
        | StructuralFieldType::RefinedScalar(_)
        | StructuralFieldType::IeeeFloat(_)
        | StructuralFieldType::Erased { .. } => SharedMutation::None,
    }
}

fn byte_mutation(carrier: ByteSequenceCarrier) -> SharedMutation {
    match carrier {
        ByteSequenceCarrier::BorrowedView {
            access: Some(access),
        }
        | ByteSequenceCarrier::BoundedView {
            access: Some(access),
            ..
        } if access != StructuralAccess::SharedBorrow => SharedMutation::Unbounded,
        _ => SharedMutation::None,
    }
}

#[cfg(test)]
mod tests;
