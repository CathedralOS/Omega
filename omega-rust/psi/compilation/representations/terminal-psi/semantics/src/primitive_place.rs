//! Projections ending in a primitive structural referent.
//!
//! A primitive leaf is a whole `PrimitiveScalar` root, an
//! element reached through record fields holding structural children and
//! fixed-array elements, a plain IEEE record field, or a byte carrier's u8
//! element. Primitive stores and reads name it by a
//! `StructuralPathSegment` projection whose elements may be runtime-selected
//! (`primitive_projection_type`); a static projection also has one canonical
//! spelling (`canonical_primitive_path`) that facts are keyed by. A reference
//! crossing retains its carrier path and explicit `Referent` segment; it has no
//! owned canonical path. Geometry never grants the live permission needed to
//! follow that pointer. Consumers check custody separately from the leaf type.
//!
//! # Scalar field observations without a canonical storage identity
//!
//! A read may end at a scalar field below a runtime-selected element or a
//! stored-reference crossing. Its complete projection retains the selectors
//! and pointer boundaries, so it cannot mint an owned field alias. The verifier
//! separately authenticates bounds and live custody, then transports the exact
//! field's declared invariant onto the fresh read SSA value. Construction,
//! entry and replacement remain responsible for establishing that invariant.
//! A static integer/Boolean projection keeps its canonical field observation.
//! Stores keep `StructuralScalarFieldStore` and its declaration-owned receiving
//! obligation; read admission cannot authorize a raw restricted-field store.
//! Plain IEEE fields use primitive storage. Their exact format survives the
//! path; observing a refined IEEE field transports its predicate rather than
//! inventing a reflexive IEEE equality that would fail for NaN.

use crate::static_path::canonical_structural_path_tip;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerSign, IntegerType, ScalarType, StructuralTypeId,
};
use terminal_psi::{
    ByteSequenceCarrier, StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape, is_static_structural_path,
};

/// Reconstruct the primitive leaf from declaration identities and fixed bounds.
/// This grants no access or establishment authority for the root.
pub fn primitive_place_type<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<ScalarType> {
    // Primitive reads/stores retain their own operation grammar. Case
    // geometry for a borrowed argument does not authorize a direct payload read.
    if path
        .iter()
        .any(|segment| matches!(segment, CanonicalStructuralPathSegment::Case(_)))
    {
        return None;
    }
    if let Some((CanonicalStructuralPathSegment::Field(field), carrier)) = path.split_last() {
        let declaration = canonical_structural_path_tip(declarations.clone(), root, carrier)?;
        if let StructuralTypeShape::Record { fields } = &declaration.shape {
            let mut matching = fields.iter().filter(|candidate| candidate.id == *field);
            let field = matching.next()?;
            if matching.next().is_some() || field.relevance.is_erased() {
                return None;
            }
            if let Some(scalar) = plain_scalar_field(&field.field_type) {
                return Some(scalar);
            }
        }
    }
    match canonical_structural_path_tip(declarations, root, path)?.shape {
        StructuralTypeShape::PrimitiveScalar(scalar) => Some(scalar),
        _ => None,
    }
}

/// Resolve `path` from `root` to a fixed array whose element declaration is a
/// primitive scalar, returning that scalar type and the declared extent. The
/// backend's abstract indexed read and store name the array this way and carry
/// the runtime element as an operand; Terminal spells the same element as the
/// path's trailing `RuntimeIndex` segment.
pub fn fixed_array_place_shape<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<(ScalarType, u64)> {
    if path
        .iter()
        .any(|segment| matches!(segment, CanonicalStructuralPathSegment::Case(_)))
    {
        return None;
    }
    let declaration = canonical_structural_path_tip(declarations.clone(), root, path)?;
    let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
        return None;
    };
    let mut matching = declarations.filter(|declaration| declaration.id == element);
    let element_declaration = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    match element_declaration.shape {
        StructuralTypeShape::PrimitiveScalar(scalar) => Some((scalar, length)),
        _ => None,
    }
}

/// Resolve a primitive leaf projection that may select elements at run time.
/// Fields step into structural children or end at a plain IEEE scalar. A literal
/// index stays inside its extent; a runtime index selects an array element: its
/// bound is the obligation of the operation carrying the path. `Referent`
/// steps through a declared reference; a byte carrier's element is the u8 scalar.
/// `Case` names no direct primitive leaf; borrowed payload projection geometry
/// has a separate contract.
/// This grants no access authority.
pub fn primitive_projection_type<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<ScalarType> {
    if path
        .iter()
        .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
    {
        return None;
    }
    walk_primitive_projection(declarations, root, path, |_| {})?.primitive_scalar()
}

/// Rejoin a primitive storage endpoint to its unique declared carrier. Inline
/// byte elements have no declaration of their own; consumers retaining a
/// structural parameter identity use this same scalar endpoint judgment.
/// Missing or ambiguous carrier declarations grant no substitute identity.
pub fn primitive_projection_declaration<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    let scalar = primitive_projection_type(declarations.clone(), root, path)?;
    let mut matching = declarations
        .filter(|declaration| declaration.shape == StructuralTypeShape::PrimitiveScalar(scalar));
    let declaration = matching.next()?;
    matching.next().is_none().then_some(declaration.id)
}

/// Resolve the scalar a primitive *read* observes: every leaf
/// `primitive_projection_type` admits, plus a declared scalar field reached
/// through a runtime selection or stored reference. Declaration invariants
/// remain on that field and are independently transported to the observed SSA
/// value. Stores resolve through `primitive_projection_type` alone. This grants
/// no access authority.
pub fn primitive_read_projection_type<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<ScalarType> {
    if path
        .iter()
        .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
    {
        return None;
    }
    primitive_projection_type(declarations.clone(), root, path).or_else(|| {
        primitive_read_field(declarations, root, path)?
            .field_type
            .scalar_type()
    })
}

/// The exact scalar field observed by a noncanonical primitive read. The
/// complete carrier contains a runtime selector or reference crossing; its
/// final field identity is relevant and unique in its declared record. Bounds,
/// access and live permission remain independent operation obligations.
///
/// Restricted scalar declarations retain their complete predicate, including
/// IEEE refinements. Plain integer/Boolean fields keep their existing readable
/// carrier rule; plain IEEE fields already use primitive storage. This query
/// grants neither a canonical owned field identity nor primitive-store access.
pub fn primitive_read_field<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<&'a StructuralFieldDeclaration> {
    if path
        .iter()
        .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
    {
        return None;
    }
    let (StructuralPathSegment::Field(identity), carrier) = path.split_last()? else {
        return None;
    };
    // Static selectors can still cross a stored pointer. Such a read has no
    // canonical owned leaf, just as a runtime-selected observation does.
    if is_static_structural_path(carrier) && !carrier.contains(&StructuralPathSegment::Referent) {
        return None;
    }
    let ProjectionTip::Declared(record) =
        walk_primitive_projection(declarations, root, carrier, |_| {})?
    else {
        return None;
    };
    let StructuralTypeShape::Record { fields } = &record.shape else {
        return None;
    };
    let mut matching = fields.iter().filter(|field| field.identity == *identity);
    let field = matching.next()?;
    if matching.next().is_some()
        || field.relevance.is_erased()
        || fields
            .iter()
            .filter(|candidate| candidate.id == field.id)
            .count()
            != 1
    {
        return None;
    }
    match field.field_type {
        StructuralFieldType::Scalar(ScalarType::Boolean) => {}
        StructuralFieldType::Scalar(ScalarType::Integer(integer)) if !integer.is_address() => {}
        StructuralFieldType::BoundedInteger(_) | StructuralFieldType::RefinedScalar(_) => {
            field.field_type.scalar_type()?;
        }
        _ => return None,
    }
    Some(field)
}

/// The canonical spelling of a static primitive projection, the key facts
/// about the leaf are recorded under. `None` for a projection with a
/// runtime-selected element or a reference crossing: neither names an owned leaf.
pub fn canonical_primitive_path<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    if path
        .iter()
        .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
    {
        return None;
    }
    let mut canonical = Some(Vec::with_capacity(path.len()));
    let tip = walk_primitive_projection(declarations, root, path, |step| {
        match (step, &mut canonical) {
            (Some(segment), Some(canonical)) => canonical.push(segment),
            (None, canonical) => *canonical = None,
            (Some(_), None) => {}
        }
    })?;
    if matches!(tip, ProjectionTip::ByteElement) {
        return None;
    }
    tip.primitive_scalar()?;
    canonical
}

/// The canonical spelling of any static projection of fields holding
/// structural children and literal elements, with the declaration it
/// selects. `None` when a segment is runtime-selected, crosses a reference,
/// or names no place.
pub fn canonical_static_projection<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<(
    Vec<CanonicalStructuralPathSegment>,
    &'a StructuralTypeDeclaration,
)> {
    let mut canonical = Some(Vec::with_capacity(path.len()));
    let ProjectionTip::Declared(tip) =
        walk_primitive_projection(declarations, root, path, |step| {
            match (step, &mut canonical) {
                (Some(segment), Some(canonical)) => canonical.push(segment),
                (None, canonical) => *canonical = None,
                (Some(_), None) => {}
            }
        })?
    else {
        return None;
    };
    Some((canonical?, tip))
}

/// Resolve the declared structural tip of a runtime projection. A dynamic
/// selector determines an element type without identifying a canonical static
/// place. Bounds, live backing and access remain the consuming operation's
/// obligations; this query cannot turn a runtime selector into a field fact.
pub fn runtime_structural_path_tip<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<&'a StructuralTypeDeclaration> {
    match walk_primitive_projection(declarations, root, path, |_| {})? {
        ProjectionTip::Declared(tip) => Some(tip),
        ProjectionTip::ScalarField(_) | ProjectionTip::ByteElement => None,
    }
}

/// The tip of a primitive projection: a declared structural type, or a byte
/// carrier's element — the u8 scalar that rides no declared identity.
enum ProjectionTip<'a> {
    Declared(&'a StructuralTypeDeclaration),
    ScalarField(ScalarType),
    ByteElement,
}

// Bounded integers and qualified carriers retain their separate store proof
// contracts. A plain read/store must not erase those obligations merely
// because their representation is also a scalar.
fn plain_scalar_field(field: &StructuralFieldType) -> Option<ScalarType> {
    match field {
        StructuralFieldType::Scalar(scalar @ ScalarType::IeeeFloat(_)) => Some(*scalar),
        StructuralFieldType::IeeeFloat(format) => Some(ScalarType::IeeeFloat(*format)),
        _ => None,
    }
}

/// The declared backing type a byte-carrier field projects: the unique
/// declaration spelling that exact carrier. A field embeds its carrier
/// inline, so the backing it names can only be identified by the carrier
/// itself — the field's predicates and borrow access bind to the field,
/// never to the backing type.
pub fn byte_carrier_field_declaration<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    carrier: ByteSequenceCarrier,
) -> Option<&'a StructuralTypeDeclaration> {
    let mut matching = declarations.filter(|declaration| {
        matches!(
            declaration.shape,
            StructuralTypeShape::ByteSequence(declared)
                if same_byte_backing(carrier, declared)
        )
    });
    let declaration = matching.next()?;
    matching.next().is_none().then_some(declaration)
}

/// A field's byte carrier and a declaration's byte carrier name the same
/// backing when their kind and extent agree; the borrow access a field
/// binds belongs to the field, not to the backing type. Only the owned
/// carrier projects a declared backing at all: a view field's descriptor is
/// storage the record borrows, never a backing type the field names.
fn same_byte_backing(field: ByteSequenceCarrier, declared: ByteSequenceCarrier) -> bool {
    match (field, declared) {
        (
            ByteSequenceCarrier::BoundedOwned { capacity: field },
            ByteSequenceCarrier::BoundedOwned { capacity: declared },
        ) => field == declared,
        _ => false,
    }
}

impl ProjectionTip<'_> {
    fn primitive_scalar(&self) -> Option<ScalarType> {
        match self {
            Self::ScalarField(scalar) => Some(*scalar),
            Self::Declared(declaration) => match declaration.shape {
                StructuralTypeShape::PrimitiveScalar(scalar) => Some(scalar),
                _ => None,
            },
            Self::ByteElement => Some(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
            )),
        }
    }
}

/// One walk for both entry points. `step` receives each segment's canonical
/// spelling, or `None` for a runtime-selected element or reference crossing.
fn walk_primitive_projection<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    mut step: impl FnMut(Option<CanonicalStructuralPathSegment>),
) -> Option<ProjectionTip<'a>> {
    enum Position {
        Declared(StructuralTypeId),
        ScalarField(ScalarType),
        ByteElement,
    }
    let unique = |id: StructuralTypeId| {
        let mut matching = declarations
            .clone()
            .filter(move |declaration| declaration.id == id);
        let declaration = matching.next()?;
        matching.next().is_none().then_some(declaration)
    };
    let mut position = Position::Declared(root);
    let mut payload_fields = None;
    for segment in path {
        let Position::Declared(root) = position else {
            return None;
        };
        let declaration = unique(root)?;
        if let StructuralPathSegment::Case(case) = segment {
            if payload_fields.is_some() {
                return None;
            }
            let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                &declaration.shape
            else {
                return None;
            };
            let mut matching = cases.iter().filter(|candidate| candidate.id == *case);
            let selected = matching.next()?;
            if matching.next().is_some() {
                return None;
            }
            payload_fields = Some(selected.fields.as_slice());
            step(Some(CanonicalStructuralPathSegment::Case(*case)));
            continue;
        }
        if payload_fields.is_some() && !matches!(segment, StructuralPathSegment::Field(_)) {
            return None;
        }
        position = match (segment, &declaration.shape) {
            (StructuralPathSegment::Referent, StructuralTypeShape::Reference { referent, .. }) => {
                // Dereferencing observes the stored pointer, not an owned child
                // of its carrier. Never mint a canonical storage fact for it.
                step(None);
                Position::Declared(*referent)
            }
            (StructuralPathSegment::Field(identity), shape) => {
                let fields = match payload_fields.take() {
                    Some(fields) => fields,
                    None => match shape {
                        StructuralTypeShape::Record { fields }
                        | StructuralTypeShape::Mixed { fields, .. } => fields.as_slice(),
                        _ => return None,
                    },
                };
                let mut matching = fields.iter().filter(|field| field.identity == *identity);
                let field = matching.next()?;
                if matching.next().is_some()
                    || field.relevance.is_erased()
                    || fields
                        .iter()
                        .filter(|candidate| candidate.id == field.id)
                        .count()
                        != 1
                {
                    return None;
                }
                step(Some(CanonicalStructuralPathSegment::Field(field.id)));
                match field.field_type {
                    StructuralFieldType::Structural(child) => Position::Declared(child),
                    _ => match field.field_type.byte_sequence_carrier() {
                        Some(carrier) => Position::Declared(
                            byte_carrier_field_declaration(declarations.clone(), carrier)?.id,
                        ),
                        None => Position::ScalarField(plain_scalar_field(&field.field_type)?),
                    },
                }
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => {
                step(Some(CanonicalStructuralPathSegment::FixedIndex(*index)));
                Position::Declared(*element)
            }
            (
                StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::FixedArray { element, .. }
                | StructuralTypeShape::ElementView { element },
            ) => {
                step(None);
                Position::Declared(*element)
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::ByteSequence(carrier),
            ) => {
                let capacity = match carrier {
                    ByteSequenceCarrier::BoundedOwned { capacity }
                    | ByteSequenceCarrier::BoundedView { capacity, .. } => capacity,
                    // A literal coordinate on a dynamic view still needs the
                    // operation's runtime bound; its type proves no element exists.
                    ByteSequenceCarrier::BorrowedView { .. } => return None,
                };
                if index >= capacity {
                    return None;
                }
                step(Some(CanonicalStructuralPathSegment::FixedIndex(*index)));
                Position::ByteElement
            }
            (StructuralPathSegment::RuntimeIndex { .. }, StructuralTypeShape::ByteSequence(_)) => {
                step(None);
                Position::ByteElement
            }
            _ => return None,
        };
    }
    if payload_fields.is_some() {
        return None;
    }
    match position {
        Position::Declared(root) => unique(root).map(ProjectionTip::Declared),
        Position::ScalarField(scalar) => Some(ProjectionTip::ScalarField(scalar)),
        Position::ByteElement => Some(ProjectionTip::ByteElement),
    }
}
