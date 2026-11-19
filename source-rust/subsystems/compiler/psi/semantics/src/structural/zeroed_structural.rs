use crate::exactly_one::exactly_one;
use semantic_vocabulary::{
    IntegerSign, IntegerType, ScalarType, StructuralCaseId, StructuralTypeId,
};
use terminal_psi::{
    StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityIndex,
};

const MAX_ZEROED_LEAVES: usize = 131_072;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZeroedStructuralLeaf {
    RecordField {
        path: Vec<StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        scalar_type: ScalarType,
    },
    // The zero value of a readable descriptor is its established empty
    // view. A typed descriptor and an inline byte field share this owner;
    // neither creates an element or borrows unknown storage.
    EmptyView {
        path: Vec<StructuralPathSegment>,
        field: Option<semantic_vocabulary::StructuralFieldId>,
        carrier: StructuralFieldType,
    },
    CaseField {
        path: Vec<StructuralPathSegment>,
        case: StructuralCaseId,
        field: semantic_vocabulary::StructuralFieldId,
        scalar_type: ScalarType,
    },
    ArrayScalar {
        path: Vec<StructuralPathSegment>,
        scalar_type: ScalarType,
    },
    ByteArray {
        path: Vec<StructuralPathSegment>,
        length: u64,
    },
    EmptyScalarArray {
        path: Vec<StructuralPathSegment>,
        structural_type: StructuralTypeId,
    },
    SumSelection {
        path: Vec<StructuralPathSegment>,
        structural_type: StructuralTypeId,
        case: StructuralCaseId,
        scalar_type: ScalarType,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstablishedErasedStructuralField {
    pub field_path: Vec<TerminalIdentityIndex>,
    pub carrier_type_identity: TerminalIdentityIndex,
}

fn established_field_identity(
    field: &StructuralFieldDeclaration,
) -> Option<TerminalIdentityIndex> {
    match &field.field_type {
        StructuralFieldType::Erased { type_identity } if !type_identity.is_absent() => {
            Some(*type_identity)
        }
        _ => None,
    }
}

impl ZeroedStructuralLeaf {
    pub fn path(&self) -> &[StructuralPathSegment] {
        match self {
            ZeroedStructuralLeaf::RecordField { path, .. }
            | ZeroedStructuralLeaf::EmptyView { path, .. }
            | ZeroedStructuralLeaf::CaseField { path, .. }
            | ZeroedStructuralLeaf::ArrayScalar { path, .. }
            | ZeroedStructuralLeaf::ByteArray { path, .. }
            | ZeroedStructuralLeaf::EmptyScalarArray { path, .. }
            | ZeroedStructuralLeaf::SumSelection { path, .. } => path,
        }
    }
}

pub fn zeroed_structural_shape<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> bool {
    zeroed_structural_fields_satisfy(types, root, |_| true)
}

pub fn zeroed_structural_fields_satisfy<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    accepts_field: impl Fn(&StructuralFieldDeclaration) -> bool,
) -> bool {
    zeroed_structural_types_and_fields_satisfy(types, root, |_| true, accepts_field)
}

/// Check declaration-level contracts as well as fields without expanding
/// a fixed array once per element or visiting inactive sum payloads.
pub fn zeroed_structural_types_and_fields_satisfy<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    accepts_type: impl Fn(&StructuralTypeDeclaration) -> bool,
    accepts_field: impl Fn(&StructuralFieldDeclaration) -> bool,
) -> bool {
    let declarations: Vec<_> = types.collect();
    zeroed_declaration_graph(
        &declarations,
        root,
        false,
        false,
        &accepts_type,
        &accepts_field,
    )
}

fn zeroed_declaration_graph(
    declarations: &[&StructuralTypeDeclaration],
    root: StructuralTypeId,
    allow_established_fields: bool,
    modulo_references: bool,
    accepts_type: &impl Fn(&StructuralTypeDeclaration) -> bool,
    accepts_field: &impl Fn(&StructuralFieldDeclaration) -> bool,
) -> bool {
    let mut active = vec![false; declarations.len()];
    let mut complete = vec![false; declarations.len()];
    let mut pending = vec![(root, false)];
    while let Some((current, exiting)) = pending.pop() {
        let Some(position) = declarations
            .iter()
            .position(|declaration| declaration.id == current)
        else {
            return false;
        };
        if exiting {
            active[position] = false;
            complete[position] = true;
            continue;
        }
        if complete[position] {
            continue;
        }
        if active[position] {
            return false;
        }
        if !accepts_type(declarations[position]) {
            return false;
        }
        if declarations[position]
            .scalar_restriction
            .as_ref()
            .is_some_and(|_| !crate::scalar_declaration_accepts_zero(declarations[position]))
        {
            return false;
        }
        let shape = &declarations[position].shape;
        let (fields, cases) = match shape {
            StructuralTypeShape::PrimitiveScalar(_) => (&[][..], None),
            StructuralTypeShape::AtomicCell(cell) => {
                if !cell.is_supported() || !crate::scalar_field_accepts_zero(&(*cell).into()) {
                    return false;
                }
                (&[][..], None)
            }
            StructuralTypeShape::ElementView {
                element,
                access: None | Some(terminal_psi::StructuralAccess::SharedBorrow),
            } => {
                // Empty has no member value whose default domain needs
                // establishment. The element declaration must still exist.
                if !declarations
                    .iter()
                    .any(|declaration| declaration.id == *element)
                {
                    return false;
                }
                (&[][..], None)
            }
            StructuralTypeShape::FixedArray { element, length } => {
                active[position] = true;
                pending.push((current, true));
                if length.literal() != Some(0) {
                    pending.push((*element, false));
                } else if !declarations
                    .iter()
                    .any(|declaration| declaration.id == *element)
                {
                    return false;
                }
                continue;
            }
            StructuralTypeShape::Record {
                storage:
                    terminal_psi::StructuralRecordStorage::AccessView(_)
                    | terminal_psi::StructuralRecordStorage::Accessor(_),
                ..
            } => return false,
            StructuralTypeShape::Record { fields, .. } => (fields.as_slice(), None),
            StructuralTypeShape::Sum { cases } => (&[][..], Some(cases)),
            StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), Some(cases)),
            _ => return false,
        };
        if let Some(cases) = cases {
            let Some(first) = cases.first() else {
                return false;
            };
            for field in &first.fields {
                if field.relevance.is_erased() || !accepts_field(field) {
                    return false;
                }
                if let Some(member) = field.field_type.as_structural() {
                    if modulo_references
                        && declarations.iter().any(|declaration| {
                            declaration.id == member
                                && matches!(
                                    declaration.shape,
                                    StructuralTypeShape::Reference { .. }
                                )
                        })
                    {
                        continue;
                    }
                    pending.push((member, false));
                } else if zeroed_field_scalar(field).is_none() {
                    return false;
                }
            }
        }
        active[position] = true;
        pending.push((current, true));
        for field in fields {
            if allow_established_fields && established_field_identity(field).is_some() {
                continue;
            }
            if field.relevance.is_erased() || !accepts_field(field) {
                return false;
            }
            if let Some(member) = field.field_type.as_structural() {
                if modulo_references
                    && declarations.iter().any(|declaration| {
                        declaration.id == member
                            && matches!(
                                declaration.shape,
                                StructuralTypeShape::Reference { .. }
                            )
                    })
                {
                    continue;
                }
                pending.push((member, false));
            } else if zeroed_field_scalar(field).is_none() && !zeroed_byte_field(field) {
                return false;
            }
        }
    }
    true
}

fn zeroed_field_scalar(field: &StructuralFieldDeclaration) -> Option<ScalarType> {
    if field.relevance.is_erased() {
        return None;
    }
    crate::scalar_field_accepts_zero(&field.field_type)
        .then(|| field.field_type.scalar_type())
        .flatten()
}

fn zeroed_byte_field(field: &StructuralFieldDeclaration) -> bool {
    !field.relevance.is_erased()
        && matches!(
            field.field_type.byte_sequence_carrier(),
            Some(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    | terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: None | Some(terminal_psi::StructuralAccess::SharedBorrow),
                    }
            )
        )
        && field.field_type.byte_predicates().holds_for(&[])
}

pub fn zeroed_structural_leaves<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> Option<Vec<ZeroedStructuralLeaf>> {
    zeroed_structural_leaves_with_established_erased_fields(types, root, &[])
}

pub fn zeroed_structural_leaves_with_established_erased_fields<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    established_fields: &[EstablishedErasedStructuralField],
) -> Option<Vec<ZeroedStructuralLeaf>> {
    zeroed_structural_leaves_impl(types, root, established_fields, false)
}

pub fn zeroed_structural_leaves_modulo_references<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> Option<Vec<ZeroedStructuralLeaf>> {
    zeroed_structural_leaves_impl(types, root, &[], true)
}

fn zeroed_structural_leaves_impl<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    established_fields: &[EstablishedErasedStructuralField],
    modulo_references: bool,
) -> Option<Vec<ZeroedStructuralLeaf>> {
    let declarations: Vec<&StructuralTypeDeclaration> = types.collect();
    if !zeroed_declaration_graph(
        &declarations,
        root,
        !established_fields.is_empty(),
        modulo_references,
        &|_| true,
        &|_| true,
    ) {
        return None;
    }
    let mut consumed = vec![false; established_fields.len()];
    let mut leaves = Vec::new();
    let mut pending = vec![(root, Vec::new())];
    let mut visited_cells = 0usize;
    while let Some((current, path)) = pending.pop() {
        let declaration = declarations
            .iter()
            .find(|declaration| declaration.id == current)?;
        if !matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_)) {
            visited_cells += 1;
            if visited_cells > MAX_ZEROED_LEAVES {
                return None;
            }
        }
        if let terminal_psi::record_or_mixed_shape_pattern!(fields) = &declaration.shape {
            for field in fields {
                if let Some(carrier) = established_field_identity(field) {
                    let mut field_path = path
                        .iter()
                        .map(|segment| match segment {
                            StructuralPathSegment::Field(identity) => Some(*identity),
                            _ => None,
                        })
                        .collect::<Option<Vec<_>>>()?;
                    field_path.push(field.identity);
                    let (index, _) = exactly_one(
                        established_fields.iter().enumerate(),
                        |(_, established)| {
                            established.field_path == field_path
                                && established.carrier_type_identity == carrier
                        },
                    )?;
                    if consumed[index] {
                        return None;
                    }
                    consumed[index] = true;
                    continue;
                }
                if let Some(member) = field.field_type.as_structural() {
                    if modulo_references
                        && declarations.iter().any(|declaration| {
                            declaration.id == member
                                && matches!(
                                    declaration.shape,
                                    StructuralTypeShape::Reference { .. }
                                )
                        })
                    {
                        continue;
                    }
                    let mut member_path = path.clone();
                    member_path.push(StructuralPathSegment::Field(field.identity));
                    pending.push((member, member_path));
                } else if zeroed_byte_field(field) {
                    leaves.push(ZeroedStructuralLeaf::EmptyView {
                        path: path.clone(),
                        field: Some(field.id),
                        carrier: field.field_type.clone(),
                    });
                } else {
                    leaves.push(ZeroedStructuralLeaf::RecordField {
                        path: path.clone(),
                        field: field.id,
                        scalar_type: zeroed_field_scalar(field)?,
                    });
                }
                if leaves.len().saturating_add(pending.len()) > MAX_ZEROED_LEAVES {
                    return None;
                }
            }
        }
        match &declaration.shape {
            StructuralTypeShape::AtomicCell(cell) => {
                leaves.push(ZeroedStructuralLeaf::ArrayScalar {
                    path,
                    scalar_type: cell.scalar_type(),
                });
            }
            StructuralTypeShape::ElementView { access: None | Some(terminal_psi::StructuralAccess::SharedBorrow), .. } => {
                leaves.push(ZeroedStructuralLeaf::EmptyView {
                    path,
                    field: None,
                    carrier: StructuralFieldType::Structural(current),
                });
            }
            StructuralTypeShape::PrimitiveScalar(scalar_type) => {
                leaves.push(ZeroedStructuralLeaf::ArrayScalar {
                    path,
                    scalar_type: *scalar_type,
                });
            }
            StructuralTypeShape::FixedArray { .. }
                if crate::scalar_array_leaf_shape(declarations.iter().copied(), current)
                    .is_some_and(|(_, count)| count == 0) =>
            {
                leaves.push(ZeroedStructuralLeaf::EmptyScalarArray {
                    path,
                    structural_type: current,
                });
            }
            StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } if *length != 0
                && declarations.iter().any(|declaration| {
                    declaration.id == *element
                        && matches!(declaration.shape,
                                StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                                    if integer.sign() == IntegerSign::Unsigned
                                        && integer.bits() == Some(8)
                                        && !integer.is_address())
                }) =>
            {
                leaves.push(ZeroedStructuralLeaf::ByteArray {
                    path,
                    length: *length,
                });
            }
            StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } => {
                let extent = usize::try_from(*length).ok()?;
                if leaves
                    .len()
                    .saturating_add(pending.len())
                    .saturating_add(extent)
                    > MAX_ZEROED_LEAVES
                {
                    return None;
                }
                for index in 0..extent {
                    let mut element_path = path.clone();
                    element_path.push(StructuralPathSegment::FixedIndex(index as u64));
                    pending.push((*element, element_path));
                }
            }
            StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                let first = cases.first()?;
                let tag_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).ok()?);
                leaves.push(ZeroedStructuralLeaf::SumSelection {
                    path: path.clone(),
                    structural_type: declaration.id,
                    case: first.id,
                    scalar_type: tag_type,
                });
                for field in &first.fields {
                    if let Some(member) = field.field_type.as_structural() {
                        let mut member_path = path.clone();
                        member_path.push(StructuralPathSegment::Case(first.id));
                        member_path.push(StructuralPathSegment::Field(field.identity));
                        pending.push((member, member_path));
                    } else {
                        leaves.push(ZeroedStructuralLeaf::CaseField {
                            path: path.clone(),
                            case: first.id,
                            field: field.id,
                            scalar_type: zeroed_field_scalar(field)?,
                        });
                    }
                    if leaves.len().saturating_add(pending.len()) > MAX_ZEROED_LEAVES {
                        return None;
                    }
                }
            }
            StructuralTypeShape::Record { .. } => {}
            _ => return None,
        }
        if leaves.len() + pending.len() > MAX_ZEROED_LEAVES {
            return None;
        }
    }
    consumed.iter().all(|consumed| *consumed).then_some(leaves)
}

/// Judge the actual default bytes of each selected carrier. A byte view
/// starts empty; a fixed array contains its literal count of zero octets.
pub(crate) fn zeroed_byte_qualifications_hold<'a, 'path>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'a terminal_psi::StructuralDomainDeclaration> + Clone,
    root: StructuralTypeId,
    mut qualifications: impl Iterator<
        Item = (
            &'path [StructuralPathSegment],
            semantic_vocabulary::StructuralDomainId,
        ),
    >,
) -> bool {
    qualifications.all(|(path, domain)| {
        let Some(domain) = domains.clone().find(|row| row.id == domain) else {
            return false;
        };
        // Default bytes cannot establish extra theories or access rules.
        if !domain.is_pure_predicate() {
            return false;
        }
        for (index, segment) in path.iter().enumerate() {
            if let StructuralPathSegment::Case(selected) = segment {
                let Some(parent) =
                    crate::runtime_structural_path_tip(types.clone(), root, &path[..index])
                else {
                    return false;
                };
                let cases = match &parent.shape {
                    StructuralTypeShape::Sum { cases }
                    | StructuralTypeShape::Mixed { cases, .. } => cases,
                    _ => return false,
                };
                if cases.first().map(|case| case.id) != Some(*selected) {
                    return false;
                }
            }
        }
        let Some(carrier) = crate::runtime_structural_path_tip(types.clone(), root, path)
        else {
            return false;
        };
        let Some(predicates) =
            crate::byte_qualification_predicates_in(domains.clone(), carrier.id, &[domain.id])
        else {
            return false;
        };
        match carrier.shape {
            StructuralTypeShape::ByteSequence(carrier)
                if carrier.is_owned_or_shared_byte_carrier() =>
            {
                predicates.holds_for(&[])
            }
            StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } => {
                if !types.clone().any(|ty| {
                    ty.id == element
                        && matches!(ty.shape,
                            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                                if integer.sign() == IntegerSign::Unsigned
                                    && integer.bits() == Some(8)
                                    && !integer.is_address())
                }) {
                    return false;
                }
                predicates.predicates().all(|predicate| match predicate {
                    terminal_psi::ByteSequencePredicate::ValidUtf8
                    | terminal_psi::ByteSequencePredicate::AsciiOnly => true,
                    terminal_psi::ByteSequencePredicate::NoNul => length == 0,
                    terminal_psi::ByteSequencePredicate::NonEmpty => length != 0,
                })
            }
            _ => false,
        }
    })
}

pub fn zeroed_projected_byte_qualifications_hold<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'a terminal_psi::StructuralDomainDeclaration> + Clone,
    root: StructuralTypeId,
    qualifications: &[terminal_psi::StructuralPathQualification],
) -> bool {
    qualifications.iter().all(|row| {
        if row.path.is_empty() {
            return false;
        }
        let Some(domain) = domains.clone().find(|domain| domain.id == row.domain) else {
            return false;
        };
        // Ordinary projected memberships are judged by the structural
        // membership owner; this query decides only byte contents.
        domain.byte_predicate.is_none()
            || zeroed_byte_qualifications_hold(
                types.clone(),
                std::iter::once(domain),
                root,
                std::iter::once((row.path.as_slice(), row.domain)),
            )
    })
}
