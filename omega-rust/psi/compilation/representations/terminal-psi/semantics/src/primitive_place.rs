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
//! Selected payload reads use `primitive_read_projection_type` and retain the
//! case namespace on the original backing. Storage/reference formation keeps
//! the stricter primitive projection grammar; geometry grants no case evidence.
//!
//! # Scalar field observations without a canonical storage identity
//!
//! A read may end at a restricted static field, a runtime-selected element or a
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
    StructuralTypeDeclaration, StructuralTypeShape,
};

/// Reconstruct the primitive leaf from declaration identities and fixed bounds.
/// This grants no access or establishment authority for the root.
pub fn primitive_place_type<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<ScalarType> {
    // This canonical storage query does not grant selected-case read authority.
    // Observations use the read projection query and separate live case checks.
    if path
        .iter()
        .any(|segment| matches!(segment, CanonicalStructuralPathSegment::Case(_)))
    {
        return None;
    }
    if let Some((CanonicalStructuralPathSegment::Field(field), carrier)) = path.split_last() {
        let declaration = canonical_structural_path_tip(declarations.clone(), root, carrier)?;
        if let StructuralTypeShape::Record { fields, .. } = &declaration.shape {
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
/// index stays inside a declared extent. A borrowed descriptor's live extent
/// is an operation obligation even for literal indices, just as for runtime
/// indices. Such selections never mint a canonical owned path. `Referent`
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
    walk_primitive_projection(declarations, root, path, plain_scalar_field, |_| {})?
        .primitive_scalar()
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
    if let Some(declaration) = runtime_structural_path_tip(declarations.clone(), root, path)
        && matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
    {
        return Some(declaration.id);
    }
    let scalar = primitive_projection_type(declarations.clone(), root, path)?;
    let mut matching = declarations.filter(|declaration| {
        declaration.shape == StructuralTypeShape::PrimitiveScalar(scalar)
            && declaration.scalar_restriction.is_none()
    });
    let declaration = matching.next()?;
    matching.next().is_none().then_some(declaration.id)
}

/// The exact declared scalar restriction at a primitive projection endpoint.
/// Bounds and access remain separate; a matching physical carrier elsewhere
/// in the module cannot supply this declaration's predicate.
pub fn primitive_projection_restriction<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<&'a StructuralFieldType> {
    runtime_structural_path_tip(declarations, root, path)?
        .scalar_restriction
        .as_ref()
}

/// The declared receiving predicate transported to this fresh read value.
/// Runtime selection and reference crossings change storage geometry, never
/// the selected declaration's predicate. Equality with current storage remains
/// a separate observation and cannot invalidate this completed snapshot.
pub fn primitive_read_requirement<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    result: semantic_vocabulary::ValueId,
) -> Option<semantic_vocabulary::Proposition> {
    let restriction =
        primitive_projection_restriction(declarations.clone(), root, path).or_else(|| {
            primitive_read_field(declarations, root, path).map(|field| &field.field_type)
        })?;
    crate::scalar_field_requirement(restriction, result)
}

/// The unique primitive carrier presented through a loan of original
/// storage. Restricted fields keep their declaration-owned predicate and exact
/// path; presenting that storage does not manufacture an owned leaf type.
/// Psi checks the declared receiving predicate again at writable handback,
/// using the selected callee's normal-return guarantees, not arrival facts.
/// Bounds, selected cases, reference access and loan custody remain separate
/// obligations. Atomic fields grant no such endpoint.
pub fn borrowed_primitive_projection_declaration<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    expected: Option<StructuralTypeId>,
) -> Option<StructuralTypeId> {
    // A whole declared scalar keeps its exact type. An inline field presents
    // its observation carrier instead: the existence of a bounded declaration
    // elsewhere must not change a plain shared call's contract.
    let expected = expected.and_then(|expected| {
        declarations
            .clone()
            .find(|declaration| declaration.id == expected)
    });
    let tip = walk_primitive_projection(
        declarations.clone(),
        root,
        path,
        |field| {
            if expected.is_some_and(|expected| expected.scalar_restriction.as_ref() == Some(field))
            {
                return field.scalar_type();
            }
            match field.shared_observation_shape()? {
                StructuralTypeShape::PrimitiveScalar(scalar) => Some(scalar),
                _ => None,
            }
        },
        |_| {},
    )?;
    if let ProjectionTip::Declared(declaration) = tip
        && matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
    {
        return Some(declaration.id);
    }
    let scalar = tip.primitive_scalar()?;
    if let Some(expected) = expected
        && expected.scalar_restriction.is_some()
        && crate::primitive_read_field(declarations.clone(), root, path)
            .is_some_and(|field| expected.scalar_restriction.as_ref() == Some(&field.field_type))
    {
        return Some(expected.id);
    }
    let mut matching = declarations.filter(|declaration| {
        declaration.shape == StructuralTypeShape::PrimitiveScalar(scalar)
            && declaration.scalar_restriction.is_none()
    });
    let declaration = matching.next()?;
    matching.next().is_none().then_some(declaration.id)
}

/// Resolve the scalar a primitive *read* observes: every leaf
/// `primitive_projection_type` admits, plus a declared scalar field reached
/// through a static path, runtime selection or stored reference. Declaration
/// invariants remain on that field and are independently transported to the observed SSA
/// value. Selected cases retain their declared payload namespace on the same
/// read path: observation does not copy or transfer the containing owner.
/// Stores resolve through `primitive_projection_type` alone. This query grants
/// neither active-case evidence nor access authority.
pub fn primitive_read_projection_type<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<ScalarType> {
    walk_primitive_projection(declarations.clone(), root, path, plain_scalar_field, |_| {})
        .and_then(|tip| tip.primitive_scalar())
        .or_else(|| {
            primitive_read_field(declarations, root, path)?
                .field_type
                .scalar_type()
        })
}

/// The exact scalar field observed by a primitive read. Its final field
/// identity is relevant and unique in its declared record. Static paths,
/// runtime selectors and reference crossings preserve that declaration. Bounds,
/// access and live permission remain independent operation obligations.
///
/// Restricted scalar declarations retain their complete predicate, including
/// IEEE refinements. Plain integer/Boolean fields retain their declared carrier,
/// including inert address data; plain IEEE fields already use primitive storage. This query
/// grants neither a canonical owned field identity nor primitive-store access.
pub fn primitive_read_field<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<&'a StructuralFieldDeclaration> {
    let (StructuralPathSegment::Field(_), _) = path.split_last()? else {
        return None;
    };
    // A restricted field is readable even when it is not writable primitive
    // storage. Keep its predicate at the declared field and on the observed
    // snapshot; this query does not create a canonical owned storage leaf.
    // A final Case selects a field namespace on the carrier; it does not
    // establish a standalone record. Preserve that namespace through the same
    // declaration query used by explicit field observations.
    let field = crate::runtime_structural_field(declarations, root, path)?;
    match field.field_type {
        StructuralFieldType::Scalar(ScalarType::Boolean) => {}
        StructuralFieldType::Scalar(ScalarType::Integer(_)) => {}
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
    canonical_primitive_read_path(declarations, root, path)
}

/// Resolve fixed read coordinates, including selected payload namespaces, to
/// declaration-local storage identities. This is observation geometry, not a
/// store/reference-formation judgment or proof that a case is currently active.
/// Execution checks the discriminator before using the selected backing.
pub fn canonical_primitive_read_path<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    let mut canonical = Some(Vec::with_capacity(path.len()));
    let tip =
        walk_primitive_projection(declarations, root, path, plain_scalar_field, |step| match (
            step,
            &mut canonical,
        ) {
            (Some(segment), Some(canonical)) => canonical.push(segment),
            (None, canonical) => *canonical = None,
            (Some(_), None) => {}
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
        walk_primitive_projection(declarations, root, path, plain_scalar_field, |step| match (
            step,
            &mut canonical,
        ) {
            (Some(segment), Some(canonical)) => canonical.push(segment),
            (None, canonical) => *canonical = None,
            (Some(_), None) => {}
        })?
    else {
        return None;
    };
    Some((canonical?, tip))
}

/// Where a primitive projection walk lands.
pub enum Position<Scalar = ScalarType> {
    Declared(StructuralTypeId),
    ScalarField(Scalar),
    ByteElement,
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
    match walk_primitive_projection(declarations.clone(), root, path, plain_scalar_field, |_| {})? {
        ProjectionTip::Declared(tip) => Some(tip),
        ProjectionTip::ScalarField(field, _) => {
            field.canonical_leaf_type()?.leaf_declaration(declarations)
        }
        ProjectionTip::ByteElement => None,
    }
}

/// The value type selected by a leaf copy, using the shared projection grammar.
/// Copying an inline byte field materializes its descriptor or owned snapshot;
/// it is not reference formation. Readability across reference boundaries is
/// required, but active-case evidence, bounds and live custody remain with the
/// checked operation. Resolving a declared payload never proves it is active.
pub fn runtime_structural_copy_type<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for (position, segment) in path.iter().enumerate() {
        if matches!(segment, StructuralPathSegment::Referent) {
            let parent =
                runtime_structural_path_tip(declarations.clone(), root, &path[..position])?;
            if !matches!(parent.shape, StructuralTypeShape::Reference { access, .. }
                if access != terminal_psi::StructuralAccess::WriteOnlyBorrow)
            {
                return None;
            }
        }
    }
    if matches!(path.last(), Some(StructuralPathSegment::Field(_))) {
        let field = crate::runtime_structural_field(declarations.clone(), root, path)?;
        if let StructuralFieldType::Structural(child) = field.field_type {
            return Some(child);
        }
        // The endpoint is a materialized value, not a geometry-only view of
        // its storage. In particular, retain a stored byte descriptor's access.
        let shape = field.field_type.leaf_copy_shape()?;
        return declarations
            .clone()
            .find(|declaration| declaration.shape == shape)
            .map(|declaration| declaration.id);
    }
    runtime_structural_path_tip(declarations, root, path).map(|declaration| declaration.id)
}

/// Whether a typed projection stays inside its root's inline storage. A
/// descriptor field itself is inline; selecting its referent or an element
/// leaves that storage. Runtime selection within an owned array does not.
/// This says nothing about ownership of the root or authority over its bytes.
pub fn projection_stays_in_inline_storage<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> bool {
    walk_primitive_projection_with_policy(
        declarations.clone(),
        root,
        path,
        plain_scalar_field,
        |_| {},
        |current, segment| {
            if matches!(segment, StructuralPathSegment::Referent) {
                return false;
            }
            !matches!(
                segment,
                StructuralPathSegment::FixedIndex(_)
                    | StructuralPathSegment::RuntimeIndex { .. }
                    | StructuralPathSegment::RepresentationView { .. }
            ) || current.is_none_or(|current| {
                !crate::descriptor_results::is_descriptor(declarations.clone(), current)
            })
        },
    )
    .is_some()
}

/// A finite inline storage selection, independent of which descriptors are
/// resident. Keep declared coverage separate from residence: inactive case
/// payloads may have no cells, but missing unconditional cells are not coverage.
pub struct StructuralStorageSelection<'path> {
    path: &'path [StructuralPathSegment],
    pub place_count: usize,
}

impl<'path> StructuralStorageSelection<'path> {
    pub fn new<'types>(
        declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &'path [StructuralPathSegment],
    ) -> Option<Self> {
        runtime_structural_path_tip(declarations.clone(), root, path)?;
        let mut place_count = 1usize;
        for (position, segment) in path.iter().enumerate() {
            if matches!(segment, StructuralPathSegment::RuntimeIndex { .. }) {
                let current =
                    runtime_structural_path_tip(declarations.clone(), root, &path[..position])?;
                let terminal_psi::StructuralTypeShape::FixedArray { length, .. } = current.shape
                else {
                    return None;
                };
                place_count = place_count.checked_mul(usize::try_from(length).ok()?)?;
            }
        }
        Some(Self { path, place_count })
    }

    /// Match actual storage without expanding every possible array slot.
    /// The consuming type query checks each resident coordinate's bounds;
    /// this selection does not authenticate custody or a case discriminator.
    pub fn relative_path<'stored>(
        &self,
        stored: &'stored [StructuralPathSegment],
    ) -> Option<&'stored [StructuralPathSegment]> {
        let prefix = stored.get(..self.path.len())?;
        prefix
            .iter()
            .zip(self.path)
            .all(|(resident, selected)| {
                resident == selected
                    || matches!(
                        (selected, resident),
                        (
                            StructuralPathSegment::RuntimeIndex { .. },
                            StructuralPathSegment::FixedIndex(_)
                        )
                    )
            })
            .then(|| &stored[self.path.len()..])
    }
}

/// Enumerate the stored descriptor paths required by a runtime projection.
/// Declared fixed extents determine coverage; missing live custody rows cannot
/// shrink it. The caller's finite roster bounds expansion before allocation.
/// These paths check custody only. They never select the emitted address.
pub fn stored_descriptor_paths<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    maximum: usize,
) -> Option<Vec<Vec<StructuralPathSegment>>> {
    if maximum == 0 {
        return None;
    }
    runtime_structural_path_tip(declarations.clone(), root, path)?;
    let mut candidates = vec![Vec::new()];
    for (position, segment) in path.iter().enumerate() {
        if matches!(segment, StructuralPathSegment::RuntimeIndex { .. }) {
            let current =
                runtime_structural_path_tip(declarations.clone(), root, &path[..position])?;
            let terminal_psi::StructuralTypeShape::FixedArray { length, .. } = current.shape else {
                return None;
            };
            let length = usize::try_from(length).ok()?;
            let count = candidates.len().checked_mul(length)?;
            if count == 0 || count > maximum {
                return None;
            }
            let mut expanded = Vec::with_capacity(count);
            for candidate in candidates {
                for index in 0..length {
                    let mut selected = candidate.clone();
                    selected.push(StructuralPathSegment::FixedIndex(
                        u64::try_from(index).ok()?,
                    ));
                    expanded.push(selected);
                }
            }
            candidates = expanded;
        } else {
            for candidate in &mut candidates {
                candidate.push(segment.clone());
            }
        }
    }
    Some(candidates)
}

/// The tip of a primitive projection: a declared structural type, or a byte
/// carrier's element — the u8 scalar that rides no declared identity.
enum ProjectionTip<'a> {
    Declared(&'a StructuralTypeDeclaration),
    ScalarField(&'a StructuralFieldType, Option<ScalarType>),
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

/// The unique declaration describing an inline byte carrier. For a view this
/// identifies descriptor geometry, not owned backing. Predicates and access
/// stay attached to the original field; the path walker cannot canonicalize
/// that borrowed crossing into owned storage.
pub fn byte_carrier_field_declaration<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    carrier: ByteSequenceCarrier,
) -> Option<&'a StructuralTypeDeclaration> {
    let mut matching = declarations.filter(|declaration| {
        matches!(
            declaration.shape,
            StructuralTypeShape::ByteSequence(declared)
                if byte_carrier_geometry_matches(carrier, declared)
        )
    });
    let declaration = matching.next()?;
    matching.next().is_none().then_some(declaration)
}

/// Kind and extent identify carrier geometry; a field's access remains on
/// the field rather than becoming part of the standalone declaration.
pub fn byte_carrier_geometry_matches(
    field: ByteSequenceCarrier,
    declared: ByteSequenceCarrier,
) -> bool {
    match (field, declared) {
        (
            ByteSequenceCarrier::BoundedOwned { capacity: field },
            ByteSequenceCarrier::BoundedOwned { capacity: declared },
        ) => field == declared,
        (
            ByteSequenceCarrier::BorrowedView { .. },
            ByteSequenceCarrier::BorrowedView { access: None },
        ) => true,
        (
            ByteSequenceCarrier::BoundedView {
                capacity: field, ..
            },
            ByteSequenceCarrier::BoundedView {
                capacity: declared,
                access: None,
            },
        ) => field == declared,
        _ => false,
    }
}

impl ProjectionTip<'_> {
    fn primitive_scalar(&self) -> Option<ScalarType> {
        match self {
            Self::ScalarField(_, scalar) => *scalar,
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
    scalar_field: impl Fn(&StructuralFieldType) -> Option<ScalarType>,
    step: impl FnMut(Option<CanonicalStructuralPathSegment>),
) -> Option<ProjectionTip<'a>> {
    walk_primitive_projection_with_policy(declarations, root, path, scalar_field, step, |_, _| true)
}

/// The shared projection grammar, with a nominal-boundary policy before each
/// step. Policy cannot authorize malformed geometry or manufacture a leaf.
fn walk_primitive_projection_with_policy<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    scalar_field: impl Fn(&StructuralFieldType) -> Option<ScalarType>,
    mut step: impl FnMut(Option<CanonicalStructuralPathSegment>),
    mut may_step: impl FnMut(Option<StructuralTypeId>, &StructuralPathSegment) -> bool,
) -> Option<ProjectionTip<'a>> {
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
        let declared = match position {
            Position::Declared(root) => Some(root),
            _ => None,
        };
        if !may_step(declared, segment) {
            return None;
        }

        if let StructuralPathSegment::RepresentationView {
            structural_type,
            stored_scalar,
        } = segment
        {
            if payload_fields.is_some()
                || matches!(position, Position::Declared(root)
                    if matches!(unique(root)?.shape, StructuralTypeShape::Reference { .. }))
            {
                return None;
            }
            let view = crate::reference_views::representation_view_declaration(
                declarations.clone(),
                *structural_type,
                *stored_scalar,
            )?;
            // Interpretation never mints an owned canonical fact, even when
            // later fields have fixed declaration-local identities.
            step(None);
            position = Position::Declared(view.id);
            continue;
        }
        let Position::Declared(root) = position else {
            return None;
        };
        let declaration = unique(root)?;
        if let StructuralPathSegment::Case(case) = segment {
            if payload_fields.is_some() {
                return None;
            }
            let cases = declaration.shape.sum_cases()?;
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
                        terminal_psi::record_or_mixed_shape_pattern!(fields) => fields.as_slice(),
                        _ => return None,
                    },
                };
                let field = terminal_psi::live_field(fields, *identity)?;
                step(Some(CanonicalStructuralPathSegment::Field(field.id)));
                match field.field_type {
                    StructuralFieldType::Structural(child) => Position::Declared(child),
                    _ => match field.field_type.byte_sequence_carrier() {
                        Some(carrier) => {
                            if !matches!(carrier, ByteSequenceCarrier::BoundedOwned { .. }) {
                                // The descriptor belongs to the record; its
                                // backing does not. Runtime geometry may cross
                                // it, but no owned canonical path may do so.
                                step(None);
                            }
                            Position::Declared(
                                byte_carrier_field_declaration(declarations.clone(), carrier)?.id,
                            )
                        }
                        None => Position::ScalarField((
                            &field.field_type,
                            scalar_field(&field.field_type),
                        )),
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
                StructuralPathSegment::FixedIndex(_),
                StructuralTypeShape::ElementView { element, .. },
            ) => {
                // A literal index still crosses descriptor-backed storage;
                // its runtime bound is not an owned canonical field fact.
                step(None);
                Position::Declared(*element)
            }
            (
                StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::FixedArray { element, .. }
                | StructuralTypeShape::ElementView { element, .. },
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
                    | ByteSequenceCarrier::BoundedView { capacity, .. } => Some(capacity),
                    // A literal coordinate on a dynamic view still needs the
                    // operation's runtime bound. This query identifies its
                    // carrier, not existence or owned canonical storage.
                    ByteSequenceCarrier::BorrowedView { .. } => None,
                };
                if capacity.is_some_and(|capacity| index >= capacity) {
                    return None;
                }
                step(capacity.map(|_| CanonicalStructuralPathSegment::FixedIndex(*index)));
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
        Position::ScalarField((field, scalar)) => Some(ProjectionTip::ScalarField(field, scalar)),
        Position::ByteElement => Some(ProjectionTip::ByteElement),
    }
}

/// Resolve a projection while retaining nominal opacity at every boundary.
/// An inline scalar/byte leaf has no structural declaration of its own.
pub(crate) fn projection_tip_with_policy<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    may_step: impl FnMut(Option<StructuralTypeId>, &StructuralPathSegment) -> bool,
) -> Option<Option<StructuralTypeId>> {
    match walk_primitive_projection_with_policy(
        declarations,
        root,
        path,
        plain_scalar_field,
        |_| {},
        may_step,
    )? {
        ProjectionTip::Declared(tip) => Some(Some(tip.id)),
        ProjectionTip::ScalarField(_, _) | ProjectionTip::ByteElement => Some(None),
    }
}
