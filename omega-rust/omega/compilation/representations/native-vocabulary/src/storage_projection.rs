// Which storage a structural path selects: the endpoint type it reaches, a
// window over a collection, or a byte field of a carrier. Stage 01's semantic
// checker and instruction selection both ask this of structural declarations,
// so the question lives with the declarations' vocabulary.

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    ByteSequenceCarrier, ByteSequencePredicateSet, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
};
use terminal_semantics::borrowed_referent_matches;

#[cfg(test)]
mod tests {
    use super::{StructuralStorageProjection, structural_storage_projection};
    use semantic_vocabulary::{StructuralCaseId, StructuralFieldId, StructuralTypeId};
    use terminal_psi::{
        BindingRelevance, ByteSequenceCarrier, ByteSequencePredicateSet, StructuralAccess,
        StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
        StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    };

    const OWNER: ByteSequenceCarrier = ByteSequenceCarrier::BoundedOwned { capacity: 16 };
    const VIEW: ByteSequenceCarrier = ByteSequenceCarrier::BorrowedView {
        access: Some(StructuralAccess::SharedBorrow),
    };

    fn type_id(raw: u64) -> StructuralTypeId {
        StructuralTypeId::new(raw).unwrap()
    }

    fn utf8() -> ByteSequencePredicateSet {
        ByteSequencePredicateSet::from_bits(1).unwrap()
    }

    fn field(
        raw: u64,
        identity: terminal_psi::TerminalIdentityIndex,
        relevance: BindingRelevance,
        field_type: StructuralFieldType,
    ) -> StructuralFieldDeclaration {
        StructuralFieldDeclaration {
            inline_carrier: None,
            id: StructuralFieldId::new(raw).unwrap(),
            identity,
            relevance,
            field_type,
            storage: None,
        }
    }

    fn declaration(raw: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
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
        }
    }

    fn catalog() -> Vec<StructuralTypeDeclaration> {
        let qualified = |carrier| StructuralFieldType::PredicateByteSequence {
            carrier,
            predicates: utf8(),
        };
        vec![
            declaration(
                1,
                StructuralTypeShape::Record {
                    fields: vec![
                        field(1, idx("text"), BindingRelevance::Relevant, qualified(OWNER)),
                        field(2, idx("name"), BindingRelevance::Relevant, qualified(VIEW)),
                        field(
                            3,
                            idx("proof"),
                            BindingRelevance::Erased,
                            StructuralFieldType::ByteSequence(OWNER),
                        ),
                    ],
                    storage: terminal_psi::StructuralRecordStorage::Packed,
                },
            ),
            declaration(
                2,
                StructuralTypeShape::Sum {
                    cases: vec![
                        StructuralCaseDeclaration {
                            id: StructuralCaseId::new(1).unwrap(),
                            identity: idx("Empty"),
                            fields: Vec::new(),
                        },
                        StructuralCaseDeclaration {
                            id: StructuralCaseId::new(2).unwrap(),
                            identity: idx("Full"),
                            fields: vec![field(
                                4,
                                idx("bytes"),
                                BindingRelevance::Relevant,
                                StructuralFieldType::ByteSequence(OWNER),
                            )],
                        },
                    ],
                },
            ),
        ]
    }

    fn idx(name: &str) -> terminal_psi::TerminalIdentityIndex {
        let index = match name {
            "text" => 1,
            "name" => 2,
            "bytes" => 3,
            "proof" => 4,
            "Empty" => 5,
            "Full" => 6,
            _ => 0,
        };
        terminal_psi::TerminalIdentityIndex(index)
    }

    fn named(identity: terminal_psi::TerminalIdentityIndex) -> StructuralPathSegment {
        StructuralPathSegment::Field(identity)
    }

    #[test]
    fn byte_fields_without_a_declared_carrier_name_their_field_storage() {
        let types = catalog();
        let owner = structural_storage_projection(types.iter(), type_id(1), &[named(idx("text"))]);
        let view = structural_storage_projection(types.iter(), type_id(1), &[named(idx("name"))]);
        assert_eq!(
            owner,
            Some(StructuralStorageProjection::ByteField {
                carrier: OWNER,
                predicates: utf8(),
            })
        );
        assert_eq!(
            view,
            Some(StructuralStorageProjection::ByteField {
                carrier: VIEW,
                predicates: utf8(),
            })
        );
        let (owner, view) = (owner.unwrap(), view.unwrap());
        assert!(owner.matches(types.iter(), owner));
        assert!(!owner.matches(types.iter(), view));
        assert_eq!(
            structural_storage_projection(types.iter(), type_id(1), &[named(idx("proof"))]),
            None
        );
    }

    #[test]
    fn a_selected_payload_byte_field_names_its_case_storage() {
        let types = catalog();
        let full = StructuralPathSegment::Case(StructuralCaseId::new(2).unwrap());
        assert_eq!(
            structural_storage_projection(types.iter(), type_id(2), &[full, named(idx("bytes"))]),
            Some(StructuralStorageProjection::ByteField {
                carrier: OWNER,
                predicates: ByteSequencePredicateSet::EMPTY,
            })
        );
        let empty = StructuralPathSegment::Case(StructuralCaseId::new(1).unwrap());
        assert_eq!(
            structural_storage_projection(types.iter(), type_id(2), &[empty, named(idx("bytes"))]),
            None
        );
    }

    #[test]
    fn a_declared_owner_carrier_keeps_its_endpoint_spelling() {
        let mut types = catalog();
        types.push(declaration(3, StructuralTypeShape::ByteSequence(OWNER)));
        assert_eq!(
            structural_storage_projection(types.iter(), type_id(1), &[named(idx("text"))]),
            Some(StructuralStorageProjection::Endpoint(type_id(3)))
        );
        assert!(matches!(
            structural_storage_projection(types.iter(), type_id(1), &[named(idx("name"))]),
            Some(StructuralStorageProjection::ByteField { carrier: VIEW, .. })
        ));
    }
}
pub fn shared_view_declaration(declaration: &StructuralTypeDeclaration) -> bool {
    declaration.shape.is_borrowed_byte_view()
        || matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralStorageProjection {
    Endpoint(StructuralTypeId),
    CollectionWindow {
        backing: StructuralTypeId,
        offset: u64,
        length: u64,
    },
    ByteField {
        carrier: ByteSequenceCarrier,
        predicates: ByteSequencePredicateSet,
    },
}

impl StructuralStorageProjection {
    pub fn matches<'types>(
        self,
        declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        expected: Self,
    ) -> bool {
        match (self, expected) {
            (Self::Endpoint(actual), Self::Endpoint(expected)) => {
                borrowed_referent_matches(declarations, actual, expected)
            }
            (
                Self::CollectionWindow {
                    backing: actual,
                    offset,
                    length,
                },
                Self::CollectionWindow {
                    backing: expected,
                    offset: expected_offset,
                    length: expected_length,
                },
            ) => {
                offset == expected_offset
                    && length == expected_length
                    && borrowed_referent_matches(declarations, actual, expected)
            }
            (actual @ Self::ByteField { .. }, expected @ Self::ByteField { .. }) => {
                actual == expected
            }
            _ => false,
        }
    }
}

pub fn structural_storage_projection<'types>(
    declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralStorageProjection> {
    if matches!(path.last(), Some(StructuralPathSegment::FixedRange { .. })) {
        let window =
            terminal_semantics::fixed_collection_storage_window(declarations.clone(), root, path)?;
        Some(StructuralStorageProjection::CollectionWindow {
            backing: structural_projection_type(declarations, root, window.backing_path)?,
            offset: window.offset,
            length: window.length,
        })
    } else {
        structural_projection_type(declarations.clone(), root, path)
            .map(StructuralStorageProjection::Endpoint)
            .or_else(|| byte_field(declarations, root, path))
    }
}

pub fn structural_borrowed_storage_projection<'types>(
    declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    access: terminal_psi::StructuralAccess,
) -> Option<StructuralStorageProjection> {
    if access != terminal_psi::StructuralAccess::Owned
        && let Some(endpoint) = terminal_semantics::borrowed_primitive_projection_declaration(
            declarations.clone(),
            root,
            path,
            None,
        )
    {
        return Some(StructuralStorageProjection::Endpoint(endpoint));
    }
    structural_storage_projection(declarations, root, path)
}

fn byte_field<'types>(
    declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralStorageProjection> {
    let (StructuralPathSegment::Field(identity), container) = path.split_last()? else {
        return None;
    };
    let (container, case) = match container.split_last() {
        Some((StructuralPathSegment::Case(case), container)) => (container, Some(case)),
        _ => (container, None),
    };
    let owner = structural_projection_type(declarations.clone(), root, container)?;
    let shape = &declarations
        .into_iter()
        .find(|declaration| declaration.id == owner)?
        .shape;
    let fields = match case {
        Some(case) => shape
            .sum_cases()?
            .iter()
            .find(|candidate| candidate.id == *case)?
            .fields
            .as_slice(),
        None => shape.direct_fields()?,
    };
    let field = terminal_psi::live_field_by_identity(fields, *identity)?;
    Some(StructuralStorageProjection::ByteField {
        carrier: field.field_type.byte_sequence_carrier()?,
        predicates: field.field_type.byte_predicates(),
    })
}

pub fn structural_projection_type<'types>(
    declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    resolve_path(
        declarations.clone(),
        |identity| {
            declarations
                .clone()
                .find(|declaration| declaration.id == identity)
        },
        structural_type,
        path,
        crate::owner_loans::loaned_field_type,
    )
    .or_else(|| {
        terminal_semantics::primitive_projection_declaration(declarations, structural_type, path)
    })
}

pub fn resolve_path<'types>(
    declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    declaration: impl Fn(StructuralTypeId) -> Option<&'types StructuralTypeDeclaration>,
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    field_type: impl Fn(&terminal_psi::StructuralFieldType) -> Option<terminal_psi::StructuralFieldType>,
) -> Option<StructuralTypeId> {
    declaration(structural_type)?;
    let mut payload_fields = None;
    for (ordinal, segment) in path.iter().enumerate() {
        let declaration = declaration(structural_type)?;
        if let StructuralPathSegment::FixedRange { start, end } = segment {
            let terminal_psi::StructuralTypeShape::FixedArray {
                length: terminal_psi::ArrayExtent::Literal(length),
                ..
            } = declaration.shape
            else {
                return None;
            };
            if payload_fields.is_some()
                || start > end
                || *end > length
                || !matches!(
                    path.get(ordinal + 1),
                    Some(StructuralPathSegment::RepresentationView { .. })
                )
            {
                return None;
            }
            continue;
        }
        if let StructuralPathSegment::Case(case) = segment {
            if payload_fields.is_some() {
                return None;
            }
            let (terminal_psi::StructuralTypeShape::Sum { cases }
            | terminal_psi::StructuralTypeShape::Mixed { cases, .. }) = &declaration.shape
            else {
                return None;
            };
            payload_fields = Some(
                cases
                    .iter()
                    .find(|candidate| candidate.id == *case)?
                    .fields
                    .as_slice(),
            );
            continue;
        }
        if payload_fields.is_some() && !matches!(segment, StructuralPathSegment::Field(_)) {
            return None;
        }
        structural_type = match (segment, &declaration.shape) {
            (
                StructuralPathSegment::RepresentationView {
                    structural_type: view,
                    stored_scalar,
                },
                shape,
            ) => {
                if matches!(shape, terminal_psi::StructuralTypeShape::Reference { .. }) {
                    return None;
                }
                terminal_semantics::representation_view_declaration(
                    declarations.clone(),
                    *view,
                    *stored_scalar,
                )?
                .id
            }
            (StructuralPathSegment::Field(identity), shape) => {
                let fields = match payload_fields.take() {
                    Some(fields) => fields,
                    None => match shape {
                        terminal_psi::record_or_mixed_shape_pattern!(fields) => fields.as_slice(),
                        _ => return None,
                    },
                };
                let field = terminal_psi::live_field_by_identity(fields, *identity)?;
                match &field.field_type {
                    terminal_psi::StructuralFieldType::Structural(next) => *next,
                    leaf => field_type(leaf)?.leaf_declaration(declarations.clone())?.id,
                }
            }
            (
                StructuralPathSegment::FixedIndex(index),
                terminal_psi::StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                },
            ) if index < length => *element,
            (
                StructuralPathSegment::RuntimeIndex { .. },
                terminal_psi::StructuralTypeShape::FixedArray { element, .. },
            ) => *element,
            (
                StructuralPathSegment::FixedIndex(_),
                terminal_psi::StructuralTypeShape::ElementView { element, .. },
            ) => *element,
            (
                StructuralPathSegment::RuntimeIndex { .. },
                terminal_psi::StructuralTypeShape::ElementView { element, .. },
            ) => *element,
            _ => return None,
        };
    }
    payload_fields.is_none().then_some(structural_type)
}
