//! Zero initialization follows the value's instantiated storage, not every
//! payload mentioned by its type graph. A fixed array with zero elements has
//! no element value to initialize, even when that element type rejects zero.
//! The element declaration must still exist; schema well-formedness belongs to
//! the representation owner, independently of this zero-value judgment.

use semantic_vocabulary::{
    IntegerSign, IntegerType, ScalarType, StructuralCaseId, StructuralTypeId,
};
use terminal_psi::{
    StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityIndex,
};

/// Resource ceiling for expanded cells and pending work. Shape admission does
/// not use this ceiling: large valid types can need another materialization
/// strategy. Aggregate visits are bounded too, including empty element shapes.
/// A fixed byte array is one run leaf whatever its extent; a root with 65,536
/// other scalar array cells also needs room for its record fields and
/// traversal work. This execution budget is not a language validity limit;
/// 131,072 keeps materialization bounded without rejecting that ordinary root.
const MAX_ZEROED_LEAVES: usize = 131_072;

/// One zero-initialized leaf cell of a zero-constructible structural type,
/// addressed the way runtime storage keys it: a record/common or selected case field
/// under its parent path, a scalar array element at its full indexed path, or
/// a whole fixed byte array at the array's own path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZeroedStructuralLeaf {
    RecordField {
        path: Vec<StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        scalar_type: ScalarType,
    },
    /// A bounded byte field begins with an empty live range, and a shared
    /// view field with the empty view, which borrows nothing. Its scalar cell
    /// is the length metadata, not one of the unused capacity bytes or the
    /// view's never-dereferenced pointer.
    ByteSequenceField {
        path: Vec<StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        scalar_type: ScalarType,
    },
    /// A selected case's payload cell, distinct from common fields even when
    /// both declarations use the same field identity.
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
    /// A non-empty fixed array of unsigned 8-bit integers: `length`
    /// contiguous zero bytes at `path`. Byte storage is one backing per
    /// array, so the run is one leaf whatever its extent; a consumer that
    /// stores bytes individually expands it to the elements `length - 1`
    /// down to `0`, the order a per-element walk visits them.
    ByteArray {
        path: Vec<StructuralPathSegment>,
        length: u64,
    },
    /// The case-tag cell of a sum whose zeroed image selects its first
    /// declared case — the tag stores the case ordinal and all-zero bits
    /// name ordinal 0. `structural_type` retains the sum's own identity so
    /// the runtime registers the selected case beside the tag cell.
    SumSelection {
        path: Vec<StructuralPathSegment>,
        structural_type: StructuralTypeId,
        case: StructuralCaseId,
        scalar_type: ScalarType,
    },
}

/// An already established erased field occurrence. This identifies host
/// custody supplied independently of zero storage; it never creates a Binding
/// or determines an omitted proof term from its carrier's spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstablishedErasedStructuralField {
    pub field_path: Vec<TerminalIdentityIndex>,
    pub carrier_type_identity: TerminalIdentityIndex,
}

fn established_field_identity<'types>(
    declarations: &[&'types StructuralTypeDeclaration],
    field: &'types StructuralFieldDeclaration,
) -> Option<TerminalIdentityIndex> {
    match &field.field_type {
        StructuralFieldType::Erased { type_identity } if !type_identity.is_absent() => {
            Some(*type_identity)
        }
        StructuralFieldType::Structural(child) if !field.relevance.is_erased() => {
            let declaration = declarations
                .iter()
                .find(|declaration| declaration.id == *child)?;
            matches!(declaration.shape, StructuralTypeShape::BindingCarrier)
                .then_some(declaration.identity)
        }
        _ => None,
    }
}

impl ZeroedStructuralLeaf {
    /// The leaf's canonical static path below the established root.
    pub fn path(&self) -> &[StructuralPathSegment] {
        match self {
            ZeroedStructuralLeaf::RecordField { path, .. }
            | ZeroedStructuralLeaf::ByteSequenceField { path, .. }
            | ZeroedStructuralLeaf::CaseField { path, .. }
            | ZeroedStructuralLeaf::ArrayScalar { path, .. }
            | ZeroedStructuralLeaf::ByteArray { path, .. }
            | ZeroedStructuralLeaf::SumSelection { path, .. } => path,
        }
    }
}

/// Resolve whether a structural type's whole storage may begin zero: every
/// leaf a zero-valid scalar, sum case tag, or empty bounded byte field, and
/// every aggregate a record, fixed array, or admissible sum. A sum admits only when its first
/// declared case exists and every field of that case is a zero-valid scalar or
/// an aggregate that itself admits: the tag's all-zero bits select that case
/// (tag = case ordinal). Bounded integer fields must contain the carrier's
/// zero; an erased payload field has no zero spelling. Bounded byte fields and
/// shared view fields admit only when all declared byte predicates hold for
/// the empty sequence. Instantiated references, other borrowed views, whole
/// byte carriers and erased members have no admitted zero spelling through
/// this structural operation.
/// Mixed shapes admit common fields and their first case. The value traversal
/// rejects unknown declarations and cycles on instantiated paths. It neither
/// expands array elements nor applies the leaf enumeration resource ceiling.
pub fn zeroed_structural_shape<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> bool {
    let declarations: Vec<_> = types.collect();
    zeroed_declaration_graph(&declarations, root, false)
}

fn zeroed_declaration_graph(
    declarations: &[&StructuralTypeDeclaration],
    root: StructuralTypeId,
    allow_established_fields: bool,
) -> bool {
    // Declaration-indexed DFS marks distinguish a backedge from a shared
    // child reached by several fields. Array extents do not grow this walk.
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
            StructuralTypeShape::FixedArray { element, length } => {
                active[position] = true;
                pending.push((current, true));
                // Empty arrays retain their declared element schema but
                // instantiate no payload. Requiring its zero validity would
                // manufacture an element that neither consumer materializes.
                if *length != 0 {
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
                    terminal_psi::StructuralRecordStorage::PlacedView(_)
                    | terminal_psi::StructuralRecordStorage::PlacedAccessor(_),
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
            // The first case's payload is part of the zero value: a scalar
            // must accept zero and an aggregate must itself begin zero.
            for field in &first.fields {
                if field.relevance.is_erased() {
                    return false;
                }
                if let Some(member) = field.field_type.as_structural() {
                    pending.push((member, false));
                } else if zeroed_field_scalar(field).is_none() {
                    return false;
                }
            }
        }
        active[position] = true;
        pending.push((current, true));
        for field in fields {
            // This permissive shape walk only rejects cycles and invalid data
            // leaves. The materialization walk below independently requires
            // exact supplied custody for every such occurrence and consumes
            // each establishment once; no field is established here.
            if allow_established_fields && established_field_identity(declarations, field).is_some()
            {
                continue;
            }
            if field.relevance.is_erased() {
                return false;
            }
            if let Some(member) = field.field_type.as_structural() {
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

/// Zero storage holds an empty byte sequence: a bounded owner's live length is
/// zero, and a shared view field holds the empty shared view
/// (`wiki/spec/terminal-psi/structural_access.md`). Every declared byte
/// predicate must hold for that empty sequence. Mutable,
/// write-only and bounded views keep no zero spelling.
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

/// Enumerate every leaf a zeroed establishment of `root` must
/// materialize, or `None` when the shape is not zero-constructible or its
/// expansion exceeds the resource ceiling. Record fields key by their
/// parent path and field id; fixed-array elements key by the full indexed
/// path.
pub fn zeroed_structural_leaves<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> Option<Vec<ZeroedStructuralLeaf>> {
    zeroed_structural_leaves_with_established_erased_fields(types, root, &[])
}

/// Enumerate zero data around exact erased occurrences whose actual custody
/// the host has already established. Empty input is ordinary zeroing and
/// refuses every erased/Fused field. Paths are root-relative record/common
/// routes, matching admitted root establishment receipts; array/case service
/// establishment is not manufactured by this input vocabulary.
pub fn zeroed_structural_leaves_with_established_erased_fields<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    established_fields: &[EstablishedErasedStructuralField],
) -> Option<Vec<ZeroedStructuralLeaf>> {
    let declarations: Vec<&StructuralTypeDeclaration> = types.collect();
    if !zeroed_declaration_graph(&declarations, root, !established_fields.is_empty()) {
        return None;
    }
    let mut consumed = vec![false; established_fields.len()];
    let mut leaves = Vec::new();
    // (type id, path to its runtime place root)
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
                if let Some(carrier) = established_field_identity(&declarations, field) {
                    let mut field_path = path
                        .iter()
                        .map(|segment| match segment {
                            StructuralPathSegment::Field(identity) => Some(identity.clone()),
                            _ => None,
                        })
                        .collect::<Option<Vec<_>>>()?;
                    field_path.push(field.identity);
                    let mut matching =
                        established_fields
                            .iter()
                            .enumerate()
                            .filter(|(_, established)| {
                                established.field_path == field_path
                                    && established.carrier_type_identity == carrier
                            });
                    let (index, _) = matching.next()?;
                    if matching.next().is_some() || consumed[index] {
                        return None;
                    }
                    consumed[index] = true;
                    continue;
                }
                if let Some(member) = field.field_type.as_structural() {
                    let mut member_path = path.clone();
                    member_path.push(StructuralPathSegment::Field(field.identity));
                    pending.push((member, member_path));
                } else if zeroed_byte_field(field) {
                    leaves.push(ZeroedStructuralLeaf::ByteSequenceField {
                        path: path.clone(),
                        field: field.id,
                        scalar_type: ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 64).ok()?,
                        ),
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
            // No payload does not mean no validity: a selected binding must be
            // established by the entry/transfer route, never by all-zero bytes.
            StructuralTypeShape::BindingCarrier => return None,
            StructuralTypeShape::PrimitiveScalar(scalar_type) => {
                leaves.push(ZeroedStructuralLeaf::ArrayScalar {
                    path,
                    scalar_type: *scalar_type,
                });
            }
            StructuralTypeShape::FixedArray { element, length }
                if *length != 0
                    && declarations.iter().any(|declaration| {
                        declaration.id == *element
                            && matches!(declaration.shape,
                                StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                                    if integer.sign() == IntegerSign::Unsigned
                                        && integer.bits() == 8
                                        && !integer.is_address())
                    }) =>
            {
                leaves.push(ZeroedStructuralLeaf::ByteArray {
                    path,
                    length: *length,
                });
            }
            StructuralTypeShape::FixedArray { element, length } => {
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
                // Zeroed storage selects the first declared case — the tag
                // stores the case ordinal and zero names ordinal 0. Payload
                // leaves mint for that case only; later cases carry no cells.
                // An aggregate payload member continues below the case's own
                // `Case` path segment, the way a payload observation names it.
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

/// Replay predicate-only memberships on the active zero-filled fixed byte
/// arrays. This proves values, never routed authority or hidden live lengths.
pub fn zeroed_projected_byte_qualifications_hold<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'a terminal_psi::StructuralDomainDeclaration> + Clone,
    root: StructuralTypeId,
    qualifications: &[terminal_psi::StructuralPathQualification],
) -> bool {
    qualifications.iter().all(|row| {
        if row.path.is_empty() { return false; }
        for (index, segment) in row.path.iter().enumerate() {
            if let StructuralPathSegment::Case(selected) = segment {
                let Some(parent) = crate::runtime_structural_path_tip(types.clone(), root, &row.path[..index]) else { return false; };
                let cases = match &parent.shape {
                    StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => cases,
                    _ => return false,
                };
                if cases.first().map(|case| case.id) != Some(*selected) { return false; }
            }
        }
        let Some(carrier) = crate::runtime_structural_path_tip(types.clone(), root, &row.path) else { return false; };
        let StructuralTypeShape::FixedArray { element, length } = carrier.shape else { return false; };
        if !types.clone().any(|ty| ty.id == element && matches!(ty.shape,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                if integer.sign() == IntegerSign::Unsigned && integer.bits() == 8 && !integer.is_address())) {
            return false;
        }
        let Some(domain) = domains.clone().find(|domain| domain.id == row.domain) else { return false; };
        domain.carrier == carrier.id
            && domain.content_projection.is_none()
            && domain.establishment_routes.is_empty()
            && domain.byte_predicate.is_some_and(|predicate| match predicate {
                terminal_psi::ByteSequencePredicate::ValidUtf8 | terminal_psi::ByteSequencePredicate::AsciiOnly => true,
                terminal_psi::ByteSequencePredicate::NoNul => length == 0,
                terminal_psi::ByteSequencePredicate::NonEmpty => length != 0,
            })
    })
}
