/*
We classify borrowed block-parameter joins whose runtime carrier is the
referent's address, and resolve the referent type and access ceiling along
projected paths.

Shared and exclusive joins admit primitive scalars, service binding leaves,
bounded byte owners, and aggregate storage through projections without copying
the referent, whereas canonical byte and element views keep their `{base, extent}`
descriptor carriers. When resolving a loan across reference crossings
(`PathSegment::Referent`), enclosing shared access attenuates descendants to
shared access, and reading a stored pointer through write-only storage is
refused.
*/

use crate::structural_type_catalog::unique_structural_type;
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    StructuralAccess, StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment as PathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

pub fn is_address_join<'a>(
    parameter: &StructuralParameterDeclaration,
    lookup: impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration> + Copy,
) -> bool {
    matches!(
        parameter.access,
        StructuralAccess::SharedBorrow
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow
    ) && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && lookup(parameter.structural_type).is_some_and(|declaration| {
            declaration.id == parameter.structural_type
                && !declaration.shape.is_canonical_view_descriptor()
        })
}

pub fn is_address_join_in(
    parameter: &StructuralParameterDeclaration,
    types: &[StructuralTypeDeclaration],
) -> bool {
    is_address_join(parameter, |identity| {
        unique_structural_type(types.iter(), identity)
    })
}

pub fn is_static_projection(path: &[PathSegment]) -> bool {
    path.iter().all(|segment| {
        matches!(
            segment,
            PathSegment::Field(_) | PathSegment::Case(_) | PathSegment::FixedIndex(_)
        )
    })
}

pub fn is_address_projection(path: &[PathSegment]) -> bool {
    path.iter().all(|segment| {
        matches!(
            segment,
            PathSegment::Field(_)
                | PathSegment::Case(_)
                | PathSegment::FixedIndex(_)
                | PathSegment::Referent
                | PathSegment::RuntimeIndex { .. }
        )
    })
}

pub fn loan_access_permitted<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[PathSegment],
    source: StructuralAccess,
    destination: StructuralAccess,
) -> bool {
    use StructuralAccess::{MutableBorrow, Owned, SharedBorrow, WriteOnlyBorrow};
    if !is_address_projection(path)
        || borrowed_referent_type(declarations.clone(), root, path, destination).is_none()
    {
        return false;
    }
    let mut ceiling = source;
    for (position, segment) in path.iter().enumerate() {
        if *segment != PathSegment::Referent {
            continue;
        }
        if ceiling == WriteOnlyBorrow {
            return false;
        }
        let Some(reference) = referent_type(declarations.clone(), root, &path[..position])
            .and_then(|identity| unique_structural_type(declarations.clone(), identity))
        else {
            return false;
        };
        let StructuralTypeShape::Reference { access, .. } = reference.shape else {
            return false;
        };
        ceiling = match ceiling {
            SharedBorrow => SharedBorrow,
            Owned | MutableBorrow => access,
            WriteOnlyBorrow => return false,
        };
    }
    matches!(
        (ceiling, destination),
        (
            Owned | MutableBorrow,
            SharedBorrow | MutableBorrow | WriteOnlyBorrow
        ) | (SharedBorrow, SharedBorrow)
            | (WriteOnlyBorrow, WriteOnlyBorrow)
    )
}

pub fn borrowed_referent_type<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[PathSegment],
    access: StructuralAccess,
) -> Option<StructuralTypeId> {
    if !is_address_projection(path) || access == StructuralAccess::Owned {
        return None;
    }
    for (position, segment) in path.iter().enumerate() {
        if *segment != PathSegment::Referent {
            continue;
        }
        let reference = referent_type(declarations.clone(), root, &path[..position])?;
        let declaration = unique_structural_type(declarations.clone(), reference)?;
        let StructuralTypeShape::Reference {
            access: permission, ..
        } = declaration.shape
        else {
            return None;
        };
        if !(permission == access
            || permission == StructuralAccess::MutableBorrow
                && matches!(
                    access,
                    StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
                ))
            || permission == StructuralAccess::WriteOnlyBorrow && position + 1 != path.len()
        {
            return None;
        }
    }
    referent_type(declarations, root, path)
}

pub fn referent_type<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[PathSegment],
) -> Option<StructuralTypeId> {
    let unique = |matches: &mut dyn Iterator<Item = &'a StructuralTypeDeclaration>| match (
        matches.next(),
        matches.next(),
    ) {
        (Some(declaration), None) => Some(declaration),
        _ => None,
    };
    let mut current = unique(
        &mut declarations
            .clone()
            .filter(|declaration| declaration.id == root),
    )?;
    let mut payload_fields = None;
    for segment in path {
        if let PathSegment::Case(identity) = segment {
            if payload_fields.is_some() {
                return None;
            }
            let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                &current.shape
            else {
                return None;
            };
            let mut matching = cases.iter().filter(|case| case.id == *identity);
            let selected = matching.next()?;
            if matching.next().is_some() {
                return None;
            }
            payload_fields = Some(selected.fields.as_slice());
            continue;
        }
        if payload_fields.is_some() && !matches!(segment, PathSegment::Field(_)) {
            return None;
        }
        let next = match (segment, &current.shape) {
            (PathSegment::Referent, StructuralTypeShape::Reference { referent, .. }) => unique(
                &mut declarations
                    .clone()
                    .filter(|declaration| declaration.id == *referent),
            )?,
            (PathSegment::Field(identity), shape) => {
                let fields = match payload_fields.take() {
                    Some(fields) => fields,
                    None => match shape {
                        terminal_psi::record_or_mixed_shape_pattern!(fields) => fields.as_slice(),
                        _ => return None,
                    },
                };
                let mut matching = fields.iter().filter(|field| field.identity == *identity);
                let field = match (matching.next(), matching.next()) {
                    (Some(field), None)
                        if !field.relevance.is_erased()
                            && fields
                                .iter()
                                .filter(|candidate| candidate.id == field.id)
                                .count()
                                == 1 =>
                    {
                        field
                    }
                    _ => return None,
                };
                match &field.field_type {
                    StructuralFieldType::Structural(child) => unique(
                        &mut declarations
                            .clone()
                            .filter(|declaration| declaration.id == *child),
                    )?,
                    leaf => super::owner_loans::loaned_field_type(leaf)?
                        .leaf_declaration(declarations.clone())?,
                }
            }
            (
                PathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                },
            ) if index < length => unique(
                &mut declarations
                    .clone()
                    .filter(|declaration| declaration.id == *element),
            )?,
            (
                PathSegment::FixedIndex(_) | PathSegment::RuntimeIndex { .. },
                StructuralTypeShape::ElementView { element, .. },
            ) => unique(
                &mut declarations
                    .clone()
                    .filter(|declaration| declaration.id == *element),
            )?,
            (PathSegment::RuntimeIndex { .. }, StructuralTypeShape::FixedArray { element, .. }) => {
                unique(
                    &mut declarations
                        .clone()
                        .filter(|declaration| declaration.id == *element),
                )?
            }
            _ => return None,
        };
        current = next;
    }
    payload_fields.is_none().then_some(current.id)
}

#[cfg(test)]
mod tests {
    use super::borrowed_referent_type;
    use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
    use terminal_psi::{
        BindingRelevance, ByteSequenceCarrier, ByteSequencePredicateSet, StructuralAccess,
        StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape,
    };

    const OWNER: ByteSequenceCarrier = ByteSequenceCarrier::BoundedOwned { capacity: 32 };

    fn type_id(raw: u64) -> StructuralTypeId {
        StructuralTypeId::new(raw).unwrap()
    }

    fn idx(name: &str) -> terminal_psi::TerminalIdentityIndex {
        terminal_psi::TerminalIdentityIndex(match name {
            "line" => 1,
            "name" => 2,
            _ => 0,
        })
    }

    fn field(
        raw: u64,
        identity: terminal_psi::TerminalIdentityIndex,
        field_type: StructuralFieldType,
    ) -> StructuralFieldDeclaration {
        StructuralFieldDeclaration {
            inline_carrier: None,
            id: StructuralFieldId::new(raw).unwrap(),
            identity,
            relevance: BindingRelevance::Relevant,
            field_type,
            storage: None,
        }
    }

    fn catalog() -> Vec<StructuralTypeDeclaration> {
        let declaration = |raw, shape| StructuralTypeDeclaration {
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: type_id(raw),
            identity: terminal_psi::TerminalIdentityIndex(10 + raw as u32),
            shape,
        };
        vec![
            declaration(
                1,
                StructuralTypeShape::Record {
                    fields: vec![
                        field(
                            1,
                            idx("line"),
                            StructuralFieldType::PredicateByteSequence {
                                carrier: OWNER,
                                predicates: ByteSequencePredicateSet::from_bits(1).unwrap(),
                            },
                        ),
                        field(
                            2,
                            idx("name"),
                            StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
                                access: Some(StructuralAccess::SharedBorrow),
                            }),
                        ),
                    ],
                    storage: terminal_psi::StructuralRecordStorage::Packed,
                },
            ),
            declaration(2, StructuralTypeShape::ByteSequence(OWNER)),
            declaration(
                3,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: None,
                }),
            ),
        ]
    }

    #[test]
    fn an_owner_field_loan_names_the_owner_carrier() {
        let types = catalog();
        let line = [StructuralPathSegment::Field(idx("line"))];
        for access in [
            StructuralAccess::MutableBorrow,
            StructuralAccess::SharedBorrow,
        ] {
            assert_eq!(
                borrowed_referent_type(types.iter(), type_id(1), &line, access),
                Some(type_id(2))
            );
        }
        assert_eq!(
            borrowed_referent_type(types.iter(), type_id(1), &line, StructuralAccess::Owned),
            None
        );
    }

    #[test]
    fn a_stored_view_field_is_a_descriptor_not_a_referent() {
        let types = catalog();
        let name = [StructuralPathSegment::Field(idx("name"))];
        assert_eq!(
            borrowed_referent_type(
                types.iter(),
                type_id(1),
                &name,
                StructuralAccess::SharedBorrow
            ),
            None
        );
    }
}
