use crate::exactly_one::exactly_one;
use crate::structural::static_path::canonical_structural_path_tip;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerSign, IntegerType, ObligationId, Proposition,
    ScalarType, StructuralTypeId,
};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

/// Classify the actual memory operand, including a separately retained
/// field selector. Knowing its scalar carrier supplies no access or bounds.
pub(crate) fn primitive_operation_projection_type<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    projection: terminal_psi::OperationProjection<'_>,
) -> Option<ScalarType> {
    let Some(field) = projection.field else {
        return primitive_projection_type(declarations, root, projection.path);
    };
    let field =
        crate::projected_field_declaration(declarations.clone(), root, projection.path, field)?;
    let mut path = Vec::with_capacity(projection.path.len() + 1);
    path.extend_from_slice(projection.path);
    path.push(StructuralPathSegment::Field(field.identity));
    primitive_projection_type(declarations, root, &path)
}

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

pub fn primitive_projection_declaration<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    match walk_primitive_projection(
        declarations.clone(),
        root,
        path,
        plain_scalar_field,
        |_| {},
    )? {
        ProjectionTip::Declared(declaration) => {
            return matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
                .then_some(declaration.id);
        }
        ProjectionTip::ScalarField(_) => {
            return runtime_structural_path_tip(declarations, root, path)
                .filter(|declaration| {
                    matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
                })
                .map(|declaration| declaration.id);
        }
        ProjectionTip::ByteElement => {}
    }
    let scalar = primitive_projection_type(declarations.clone(), root, path)?;
    let mut matching = declarations.filter(|declaration| {
        declaration.shape == StructuralTypeShape::PrimitiveScalar(scalar)
            && declaration.scalar_restriction.is_none()
    });
    let declaration = matching.next()?;
    matching.next().is_none().then_some(declaration.id)
}

pub fn primitive_projection_restriction<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<&'a StructuralFieldType> {
    if let Some(field) = primitive_read_field(declarations.clone(), root, path)
        && matches!(
            field.field_type,
            StructuralFieldType::Scalar(_)
                | StructuralFieldType::IeeeFloat(_)
                | StructuralFieldType::BoundedInteger(_)
                | StructuralFieldType::RefinedScalar(_)
        )
    {
        return crate::scalar_field_requires_evidence(&field.field_type)
            .then_some(&field.field_type);
    }
    runtime_structural_path_tip(declarations, root, path)?
        .scalar_restriction
        .as_ref()
}

pub fn borrowed_primitive_projection_declaration<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    expected: Option<StructuralTypeId>,
) -> Option<StructuralTypeId> {
    let expected = match expected {
        Some(expected) => Some(crate::structural_declaration(
            declarations.clone(),
            expected,
        )?),
        None => None,
    };
    let tip = walk_primitive_projection(
        declarations.clone(),
        root,
        path,
        |field| {
            if expected
                .is_some_and(|expected| expected.scalar_restriction.as_ref() == Some(field))
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
    if let Some(field) = crate::primitive_read_field(declarations.clone(), root, path) {
        let carrier = match expected {
            Some(expected) => expected,
            None => crate::structural_declaration(declarations.clone(), field.inline_carrier?)?,
        };
        return field
            .field_type
            .borrowed_presentation_type(StructuralAccess::SharedBorrow, Some(carrier))?
            .declares_leaf(carrier)
            .then_some(carrier.id);
    }
    if let Some(expected) = expected {
        return (expected.shape == StructuralTypeShape::PrimitiveScalar(scalar)
            && expected.scalar_restriction.is_none())
        .then_some(expected.id);
    }
    let mut matching = declarations.filter(|declaration| {
        declaration.shape == StructuralTypeShape::PrimitiveScalar(scalar)
            && declaration.scalar_restriction.is_none()
    });
    let declaration = matching.next()?;
    matching.next().is_none().then_some(declaration.id)
}

pub fn primitive_read_projection_type<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<ScalarType> {
    walk_primitive_projection(
        declarations,
        root,
        path,
        primitive_read_scalar_field,
        |_| {},
    )
    .and_then(|tip| tip.primitive_scalar())
}

pub(crate) fn primitive_read_field<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<&'a StructuralFieldDeclaration> {
    let (StructuralPathSegment::Field(_), _) = path.split_last()? else {
        return None;
    };
    let field = crate::runtime_structural_field(declarations, root, path)?;
    integer_or_boolean_field(&field.field_type)?;
    Some(field)
}

fn integer_or_boolean_field(field: &StructuralFieldType) -> Option<ScalarType> {
    match field {
        StructuralFieldType::Scalar(ScalarType::Boolean | ScalarType::Integer(_))
        | StructuralFieldType::BoundedInteger(_)
        | StructuralFieldType::RefinedScalar(_) => field.scalar_type(),
        _ => None,
    }
}

// Path identity and read typing must recognize the same primitive leaves.
// Atomic and byte storage use their own operations rather than this reader.
fn primitive_read_scalar_field(field: &StructuralFieldType) -> Option<ScalarType> {
    match field {
        StructuralFieldType::Atomic(_) | StructuralFieldType::AtomicBoundedInteger(_) => None,
        _ => field.scalar_type(),
    }
}

/// A contract reads the declared scalar at an exact canonical path. Inline
/// scalar fields need no separately materialized structural declaration;
/// their field declaration already owns the carrier and qualification.
pub fn canonical_primitive_read_type<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<ScalarType> {
    if let Some((CanonicalStructuralPathSegment::Field(field), prefix)) = path.split_last() {
        let field =
            crate::observed_field_declaration(declarations.clone(), root, prefix, *field)?;
        if let StructuralFieldType::Structural(child) = field.field_type {
            let declaration = canonical_structural_path_tip(declarations, child, &[])?;
            match declaration.shape {
                StructuralTypeShape::PrimitiveScalar(scalar) => Some(scalar),
                _ => None,
            }
        } else {
            primitive_read_scalar_field(&field.field_type)
        }
    } else {
        let declaration = canonical_structural_path_tip(declarations, root, path)?;
        match declaration.shape {
            StructuralTypeShape::PrimitiveScalar(scalar) => Some(scalar),
            _ => None,
        }
    }
}

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

pub fn canonical_primitive_read_path<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    let mut canonical = Some(Vec::with_capacity(path.len()));
    let tip = walk_primitive_projection(
        declarations,
        root,
        path,
        primitive_read_scalar_field,
        |step| match (step, &mut canonical) {
            (Some(segment), Some(canonical)) => canonical.push(segment),
            (None, canonical) => *canonical = None,
            (Some(_), None) => {}
        },
    )?;
    if matches!(tip, ProjectionTip::ByteElement) {
        return None;
    }
    tip.primitive_scalar()?;
    canonical
}

/// The region of inline storage a projection names, as the canonical
/// path every analysis of that storage compares: what is read, what is
/// written, what is moved.
///
/// A static projection of declared storage has its own path, and so
/// does a primitive scalar read. A byte field is the third case: its
/// descriptor is inline storage of its record, so its region is the
/// carrier's path and the field. The static projection stops there
/// because a view's bytes lie outside the record; the descriptor does
/// not. A payload field's carrier is the sum, and its region lies under
/// the case, as a scalar payload leaf's does.
pub fn canonical_inline_region<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    canonical_static_projection(declarations.clone(), root, path)
        .map(|(path, _)| path)
        .or_else(|| canonical_primitive_read_path(declarations.clone(), root, path))
        .or_else(|| {
            let (StructuralPathSegment::Field(_), carrier) = path.split_last()? else {
                return None;
            };
            let field = crate::runtime_structural_field(declarations.clone(), root, path)?;
            field.field_type.byte_sequence_carrier()?;
            let (carrier, case) = match carrier.split_last() {
                Some((StructuralPathSegment::Case(case), sum)) => (sum, Some(*case)),
                _ => (carrier, None),
            };
            let (mut canonical, _) = canonical_static_projection(declarations, root, carrier)?;
            canonical.extend(case.map(CanonicalStructuralPathSegment::Case));
            canonical.push(CanonicalStructuralPathSegment::Field(field.id));
            Some(canonical)
        })
}

pub fn canonical_static_projection<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<(
    Vec<CanonicalStructuralPathSegment>,
    &'a StructuralTypeDeclaration,
)> {
    let mut canonical = Some(Vec::with_capacity(path.len()));
    let ProjectionTip::Declared(tip) = walk_primitive_projection(
        declarations,
        root,
        path,
        plain_scalar_field,
        |step| match (step, &mut canonical) {
            (Some(segment), Some(canonical)) => canonical.push(segment),
            (None, canonical) => *canonical = None,
            (Some(_), None) => {}
        },
    )?
    else {
        return None;
    };
    Some((canonical?, tip))
}

/// The read projections of a path that may select an element at run time:
/// each static step keeps its canonical segment, and each run-time index
/// becomes an `Index` over the selector's value. `rows[row]` yields
/// `[Index(row)]` where `canonical_static_projection` yields nothing. A
/// path with any other step that has no canonical form has none here either.
pub fn canonical_read_projection<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    value_type: impl Fn(semantic_vocabulary::ValueId) -> Option<ScalarType>,
) -> Option<(
    Vec<semantic_vocabulary::ScalarReadProjection>,
    &'a StructuralTypeDeclaration,
)> {
    use semantic_vocabulary::{ScalarReadProjection, ScalarTerm};
    let mut steps = Vec::with_capacity(path.len());
    let ProjectionTip::Declared(tip) =
        walk_primitive_projection(declarations, root, path, plain_scalar_field, |step| {
            steps.push(step)
        })?
    else {
        return None;
    };
    if steps.len() != path.len() {
        return None;
    }
    let projection = steps
        .into_iter()
        .zip(path)
        .map(|(step, segment)| match (step, segment) {
            (Some(segment), _) => Some(ScalarReadProjection::Static(segment)),
            (None, StructuralPathSegment::RuntimeIndex { index, .. }) => {
                Some(ScalarReadProjection::Index(Box::new(ScalarTerm::value(
                    *index,
                    value_type(*index)?,
                ))))
            }
            (None, _) => None,
        })
        .collect::<Option<Vec<_>>>()?;
    Some((projection, tip))
}

pub enum Position<Scalar = ScalarType> {
    Declared(StructuralTypeId),
    ScalarField(Scalar),
    ByteElement,
}

/// A scalar field is a stored leaf even without a synthetic structural
/// carrier row. Use the same primitive reader grammar as static paths;
/// callers still distinguish byte elements and enforce their own access.
pub fn runtime_read_projection_tip<'a>(
    declarations: impl StructuralDeclarations<'a>,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<Position> {
    match walk_primitive_projection(
        declarations,
        root,
        path,
        primitive_read_scalar_field,
        |_| {},
    )? {
        ProjectionTip::Declared(declaration) => Some(Position::Declared(declaration.id)),
        ProjectionTip::ScalarField(Some(scalar)) => Some(Position::ScalarField(scalar)),
        ProjectionTip::ScalarField(None) => None,
        ProjectionTip::ByteElement => Some(Position::ByteElement),
    }
}

pub fn runtime_structural_path_tip<'a>(
    declarations: impl StructuralDeclarations<'a>,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<&'a StructuralTypeDeclaration> {
    match walk_primitive_projection(
        declarations.clone(),
        root,
        path,
        plain_scalar_field,
        |_| {},
    )? {
        ProjectionTip::Declared(tip) => Some(tip),
        ProjectionTip::ScalarField(_) => {
            let field = crate::runtime_structural_field(declarations.clone(), root, path)?;
            declarations.inline_carrier(field)
        }
        ProjectionTip::ByteElement => None,
    }
}

/// Where a projection walk reads declarations. Each answer equals the one a
/// scan of the module's whole declaration list gives when it accepts exactly
/// one row with the identity, so an index changes the cost of a walk and
/// never its result. Any declaration iterator is such a scan; a hot caller
/// passes the module's `StructuralTypeIndex` instead.
pub trait StructuralDeclarations<'a>: Clone {
    /// The one declaration with this identity; none when it is absent or
    /// declared twice.
    fn unique(&self, id: StructuralTypeId) -> Option<&'a StructuralTypeDeclaration>;
    /// `representation_views::declaration` over the same list.
    fn representation_view(
        &self,
        view: StructuralTypeId,
        stored: Option<ScalarType>,
    ) -> Option<&'a StructuralTypeDeclaration>;
    /// `inline_carrier_declaration` over the same list.
    fn inline_carrier(
        &self,
        field: &StructuralFieldDeclaration,
    ) -> Option<&'a StructuralTypeDeclaration>;
    /// `StructuralFieldType::leaf_declaration` over the same list.
    fn leaf_declaration(
        &self,
        field_type: &StructuralFieldType,
    ) -> Option<&'a StructuralTypeDeclaration>;
}

impl<'a, I> StructuralDeclarations<'a> for I
where
    I: Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
{
    fn unique(&self, id: StructuralTypeId) -> Option<&'a StructuralTypeDeclaration> {
        exactly_one(self.clone(), |row| row.id == id)
    }

    fn representation_view(
        &self,
        view: StructuralTypeId,
        stored: Option<ScalarType>,
    ) -> Option<&'a StructuralTypeDeclaration> {
        terminal_psi::types::representation_views::declaration(self.clone(), view, stored)
    }

    fn inline_carrier(
        &self,
        field: &StructuralFieldDeclaration,
    ) -> Option<&'a StructuralTypeDeclaration> {
        crate::inline_carrier_declaration(self.clone(), field)
    }

    fn leaf_declaration(
        &self,
        field_type: &StructuralFieldType,
    ) -> Option<&'a StructuralTypeDeclaration> {
        field_type.leaf_declaration(self.clone())
    }
}

/// A caller's own identity lookup, with view and inline-carrier rows still
/// read from its declaration list.
#[derive(Clone)]
struct LookupWithList<I, F> {
    declarations: I,
    declaration: F,
}

impl<'a, I, F> StructuralDeclarations<'a> for LookupWithList<I, F>
where
    I: Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    F: Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration> + Clone,
{
    fn unique(&self, id: StructuralTypeId) -> Option<&'a StructuralTypeDeclaration> {
        (self.declaration)(id)
    }

    fn representation_view(
        &self,
        view: StructuralTypeId,
        stored: Option<ScalarType>,
    ) -> Option<&'a StructuralTypeDeclaration> {
        self.declarations.representation_view(view, stored)
    }

    fn inline_carrier(
        &self,
        field: &StructuralFieldDeclaration,
    ) -> Option<&'a StructuralTypeDeclaration> {
        self.declarations.inline_carrier(field)
    }

    fn leaf_declaration(
        &self,
        field_type: &StructuralFieldType,
    ) -> Option<&'a StructuralTypeDeclaration> {
        self.declarations.leaf_declaration(field_type)
    }
}

/// The storage carrier of an inline field when its address is lent. An
/// owned byte carrier keeps its storage type rather than a copied view.
pub fn borrowed_storage_field_type(field: &StructuralFieldType) -> Option<StructuralFieldType> {
    match field.byte_sequence_carrier() {
        Some(carrier @ ByteSequenceCarrier::BoundedOwned { .. }) => {
            Some(StructuralFieldType::ByteSequence(carrier))
        }
        _ => field.canonical_leaf_type(),
    }
}

/// Resolve the actual type selected by a storage path, including scalar
/// leaves without a separate inline declaration. This supplies neither
/// bounds evidence nor permission to access the selected storage.
pub fn borrowed_projection_type<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    expected: Option<StructuralTypeId>,
) -> Option<StructuralTypeId> {
    if let Some(selected) = runtime_structural_path_tip(declarations.clone(), root, path) {
        return Some(selected.id);
    }
    if matches!(path.last(), Some(StructuralPathSegment::Field(_))) {
        let field = crate::runtime_structural_field(declarations.clone(), root, path)?;
        // A field can spell its carrier without an inline type ID. An
        // explicit but invalid ID cannot be replaced by another row.
        if field.relevance.is_erased() || field.inline_carrier.is_some() {
            return None;
        }
        let carrier = borrowed_storage_field_type(&field.field_type)?
            .leaf_declaration(declarations.clone())?;
        return crate::structural_declaration(declarations, carrier.id).map(|row| row.id);
    }
    let scalar =
        borrowed_primitive_projection_declaration(declarations.clone(), root, path, expected)?;
    crate::structural_declaration(declarations, scalar).map(|row| row.id)
}

pub fn runtime_structural_copy_type<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    expected: Option<StructuralTypeId>,
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
        let expected =
            crate::structural_declaration(declarations, expected.or(field.inline_carrier)?)?;
        return field
            .field_type
            .leaf_copy_type(Some(expected))?
            .declares_leaf(expected)
            .then_some(expected.id);
    }
    runtime_structural_path_tip(declarations, root, path).map(|declaration| declaration.id)
}

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
                !crate::descriptors::descriptor_results::is_descriptor(
                    declarations.clone(),
                    current,
                )
            })
        },
        None,
    )
    .is_some()
}

/// Returns the exact declared descriptor stored at an inline path.
/// The storage of a borrowed view belongs to its carrier. Its elements do not.
pub fn inline_descriptor_type<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<&'a StructuralTypeDeclaration> {
    if !projection_stays_in_inline_storage(declarations.clone(), root, path) {
        return None;
    }
    runtime_structural_path_tip(declarations, root, path)
        .filter(|declaration| declaration.shape.is_borrowed_descriptor())
}

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
                let terminal_psi::StructuralTypeShape::FixedArray {
                    length: terminal_psi::ArrayExtent::Literal(length),
                    ..
                } = current.shape
                else {
                    return None;
                };
                place_count = place_count.checked_mul(usize::try_from(length).ok()?)?;
            }
        }
        Some(Self { path, place_count })
    }

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
            let terminal_psi::StructuralTypeShape::FixedArray {
                length: terminal_psi::ArrayExtent::Literal(length),
                ..
            } = current.shape
            else {
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

enum ProjectionTip<'a> {
    Declared(&'a StructuralTypeDeclaration),
    ScalarField(Option<ScalarType>),
    ByteElement,
}

fn plain_scalar_field(field: &StructuralFieldType) -> Option<ScalarType> {
    match field {
        StructuralFieldType::Scalar(scalar @ ScalarType::IeeeFloat(_)) => Some(*scalar),
        StructuralFieldType::IeeeFloat(format) => Some(ScalarType::IeeeFloat(*format)),
        _ => None,
    }
}

pub(crate) fn byte_carrier_geometry_matches(
    field: ByteSequenceCarrier,
    declared: ByteSequenceCarrier,
) -> bool {
    match (field, declared) {
        (
            ByteSequenceCarrier::BoundedOwned { capacity: field },
            ByteSequenceCarrier::BoundedOwned { capacity: declared },
        ) => field == declared,
        (
            ByteSequenceCarrier::BorrowedView { access: field },
            ByteSequenceCarrier::BorrowedView { access: declared },
        ) => declared.is_none() || declared == field,
        (
            ByteSequenceCarrier::BoundedView {
                capacity: field,
                access: field_access,
            },
            ByteSequenceCarrier::BoundedView {
                capacity: declared,
                access: declared_access,
            },
        ) => {
            field == declared && (declared_access.is_none() || declared_access == field_access)
        }
        _ => false,
    }
}

impl ProjectionTip<'_> {
    fn primitive_scalar(&self) -> Option<ScalarType> {
        match self {
            Self::ScalarField(scalar) => *scalar,
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

fn walk_primitive_projection<'a>(
    declarations: impl StructuralDeclarations<'a>,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    scalar_field: impl Fn(&StructuralFieldType) -> Option<ScalarType>,
    step: impl FnMut(Option<CanonicalStructuralPathSegment>),
) -> Option<ProjectionTip<'a>> {
    walk_primitive_projection_with_policy(
        declarations,
        root,
        path,
        scalar_field,
        step,
        |_, _| true,
        None,
    )
}

pub(crate) fn projection_storage_dependencies<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    dependencies: &mut std::collections::BTreeSet<StructuralTypeId>,
    include_root: bool,
) -> Option<Option<StructuralTypeId>> {
    let backing = match path.split_last() {
        Some((StructuralPathSegment::FixedRange { .. }, backing)) => backing,
        _ => path,
    };
    let tip = walk_primitive_projection_with_policy(
        declarations,
        root,
        backing,
        plain_scalar_field,
        |_| {},
        |_, _| true,
        Some((include_root, dependencies)),
    )?;
    Some(match tip {
        ProjectionTip::Declared(declaration) => Some(declaration.id),
        ProjectionTip::ScalarField(_) | ProjectionTip::ByteElement => None,
    })
}

type TypeDependencies = std::collections::BTreeSet<StructuralTypeId>;

fn walk_primitive_projection_with_policy<'a>(
    declarations: impl StructuralDeclarations<'a>,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    scalar_field: impl Fn(&StructuralFieldType) -> Option<ScalarType>,
    mut step: impl FnMut(Option<CanonicalStructuralPathSegment>),
    mut may_step: impl FnMut(Option<StructuralTypeId>, &StructuralPathSegment) -> bool,
    mut dependencies: Option<(bool, &mut TypeDependencies)>,
) -> Option<ProjectionTip<'a>> {
    let mut position = Position::Declared(root);
    let mut payload_fields = None;
    for (ordinal, segment) in path.iter().enumerate() {
        let declared = match position {
            Position::Declared(root) => Some(root),
            _ => None,
        };
        if let (Some((include_root, dependencies)), Some(declared)) =
            (dependencies.as_mut(), declared)
            && (*include_root || ordinal > 0)
        {
            dependencies.insert(declared);
        }
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
                    if matches!(declarations.unique(root)?.shape,
                        StructuralTypeShape::Opaque | StructuralTypeShape::Reference { .. }))
            {
                return None;
            }
            let view = declarations.representation_view(*structural_type, *stored_scalar)?;
            step(None);
            position = Position::Declared(view.id);
            continue;
        }
        let Position::Declared(root) = position else {
            return None;
        };
        let declaration = declarations.unique(root)?;
        if let StructuralPathSegment::FixedRange { start, end } = segment {
            let StructuralTypeShape::FixedArray {
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
            step(None);
            continue;
        }
        if let StructuralPathSegment::Case(case) = segment {
            if payload_fields.is_some() {
                return None;
            }
            let cases = declaration.shape.sum_cases()?;
            let selected = exactly_one(cases.iter(), |candidate| candidate.id == *case)?;
            payload_fields = Some(selected.fields.as_slice());
            step(Some(CanonicalStructuralPathSegment::Case(*case)));
            continue;
        }
        if payload_fields.is_some() && !matches!(segment, StructuralPathSegment::Field(_)) {
            return None;
        }
        position = match (segment, &declaration.shape) {
            (
                StructuralPathSegment::Referent,
                StructuralTypeShape::Reference { referent, .. },
            ) => {
                step(None);
                Position::Declared(*referent)
            }
            (StructuralPathSegment::Field(identity), shape) => {
                let fields = match payload_fields.take() {
                    Some(fields) => fields,
                    None => match shape {
                        terminal_psi::record_or_mixed_shape_pattern!(fields) => {
                            fields.as_slice()
                        }
                        _ => return None,
                    },
                };
                let field = terminal_psi::live_field(fields, *identity)?;
                step(Some(CanonicalStructuralPathSegment::Field(field.id)));
                match field.field_type {
                    StructuralFieldType::Structural(child) => Position::Declared(child),
                    _ if field.field_type.atomic_cell_type().is_some()
                        && field.inline_carrier.is_some() =>
                    {
                        Position::Declared(declarations.inline_carrier(field)?.id)
                    }
                    _ => match field.field_type.byte_sequence_carrier() {
                        Some(carrier) => {
                            if !matches!(carrier, ByteSequenceCarrier::BoundedOwned { .. }) {
                                step(None);
                            }
                            Position::Declared(declarations.inline_carrier(field)?.id)
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
            ) if !matches!(
                length,
                terminal_psi::ArrayExtent::Parameter { .. }
                    | terminal_psi::ArrayExtent::Application(_)
            ) && length.literal().is_none_or(|length| *index < length) =>
            {
                step(
                    length
                        .literal()
                        .map(|_| CanonicalStructuralPathSegment::FixedIndex(*index)),
                );
                Position::Declared(*element)
            }
            (
                StructuralPathSegment::FixedIndex(_),
                StructuralTypeShape::ElementView { element, .. },
            ) => {
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
                    ByteSequenceCarrier::BorrowedView { .. } => None,
                };
                if capacity.is_some_and(|capacity| index >= capacity) {
                    return None;
                }
                step(capacity.map(|_| CanonicalStructuralPathSegment::FixedIndex(*index)));
                Position::ByteElement
            }
            (
                StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::ByteSequence(_),
            ) => {
                step(None);
                Position::ByteElement
            }
            _ => return None,
        };
    }
    if payload_fields.is_some() && dependencies.is_none() {
        return None;
    }
    match position {
        Position::Declared(root) => {
            let declaration = declarations.unique(root)?;
            if let Some((include_root, dependencies)) = dependencies
                && (include_root || !path.is_empty())
            {
                dependencies.insert(root);
            }
            Some(ProjectionTip::Declared(declaration))
        }
        Position::ScalarField((_, scalar)) => Some(ProjectionTip::ScalarField(scalar)),
        Position::ByteElement => Some(ProjectionTip::ByteElement),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeIndexGoalError {
    InvalidPath,
    OpenExtent,
    MissingExtent,
    MissingSelector,
    UnsupportedSelector,
}

/// Find a live extent immediately before a projection. Only adjacent length
/// observations may intervene; no mutation can change the descriptor between
/// this measurement and the access whose bound it establishes.
pub fn preceding_collection_length(
    operations: &[terminal_psi::Operation],
    root: semantic_vocabulary::PlaceId,
    path: &[StructuralPathSegment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
) -> Option<semantic_vocabulary::ValueId> {
    for operation in operations.iter().rev() {
        let matches = match &operation.kind {
            terminal_psi::OperationKind::ElementViewLength { source }
            | terminal_psi::OperationKind::ByteSequenceLength { source } => {
                source.place == root
                    && crate::structural_paths_select_same_location(&source.path, path)
                    && source.access == StructuralAccess::SharedBorrow
            }
            terminal_psi::OperationKind::StructuralByteSequenceFieldLength {
                source,
                path: observed,
                field: selected,
            } => {
                *source == root
                    && Some(*selected) == field
                    && path.split_last().is_some_and(|(step, prefix)| {
                        matches!(step, StructuralPathSegment::Field(_))
                            && crate::structural_paths_select_same_location(observed, prefix)
                    })
            }
            _ => return None,
        };
        if matches {
            let result = operation.result.scalar_ref()?;
            let count = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).ok()?);
            return (result.scalar_type == count).then_some(result.id);
        }
    }
    None
}

/// Formation of a collection selection, in its mathematical index domain.
pub fn scalar_index_bound(
    index: semantic_vocabulary::ScalarTerm,
    length: semantic_vocabulary::ScalarTerm,
) -> Option<Proposition> {
    use semantic_vocabulary::{IntegerMathTerm, IntegerValue, ScalarTerm};
    let integer = index
        .scalar_type()
        .as_integer()
        .filter(|integer| !integer.is_address())?;
    let upper = if index.scalar_type() == length.scalar_type() {
        Proposition::LessThan(index.clone(), length)
    } else {
        Proposition::IntegerMathLessThan(
            IntegerMathTerm::embed(index.clone())?,
            IntegerMathTerm::embed(length)?,
        )
    };
    Some(if integer.sign() == IntegerSign::Signed {
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(
                ScalarTerm::integer(integer, IntegerValue::Signed(0)).ok()?,
                index,
            ),
            upper,
        ])
    } else {
        upper
    })
}

/// Use the visited goals only when the indexed prefix is valid.
pub fn visit_runtime_index_goals<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    declaration: impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    values: &impl crate::ValueTypes,
    application_type: impl Fn(semantic_vocabulary::ScalarApplicationId) -> Option<ScalarType>,
    mut measured_extent: impl FnMut(&[StructuralPathSegment]) -> Option<semantic_vocabulary::ValueId>,
    mut visit: impl FnMut(ObligationId, Proposition),
) -> Result<(), RuntimeIndexGoalError> {
    let Some(last_index) = path.iter().rposition(|step| step.runtime_index().is_some()) else {
        return Ok(());
    };
    let mut error = RuntimeIndexGoalError::InvalidPath;
    let mut position = 0;
    let walked = walk_primitive_projection_with_policy(
        LookupWithList {
            declarations,
            declaration: &declaration,
        },
        root,
        &path[..=last_index],
        plain_scalar_field,
        |_| {},
        |owner, step| {
            let prefix = &path[..position];
            position += 1;
            let Some((index, obligation)) = step.runtime_index() else {
                return true;
            };
            let Some(array) = owner.and_then(&declaration) else {
                return false;
            };
            let Some(carrier) = values.value_type(index) else {
                error = RuntimeIndexGoalError::MissingSelector;
                return false;
            };
            let goal = match array.shape {
                StructuralTypeShape::FixedArray {
                    length: terminal_psi::ArrayExtent::Literal(extent),
                    ..
                } => super::static_path::runtime_index_bound(index, carrier, extent),
                StructuralTypeShape::FixedArray {
                    length: terminal_psi::ArrayExtent::Parameter { value: length, .. },
                    ..
                } => {
                    let Some(length_type) = values.value_type(length) else {
                        error = RuntimeIndexGoalError::MissingExtent;
                        return false;
                    };
                    scalar_index_bound(
                        semantic_vocabulary::ScalarTerm::value(index, carrier),
                        semantic_vocabulary::ScalarTerm::value(length, length_type),
                    )
                }
                StructuralTypeShape::FixedArray {
                    length: terminal_psi::ArrayExtent::Application(application),
                    ..
                } => {
                    let Some(length_type) = application_type(application) else {
                        error = RuntimeIndexGoalError::MissingExtent;
                        return false;
                    };
                    scalar_index_bound(
                        semantic_vocabulary::ScalarTerm::value(index, carrier),
                        semantic_vocabulary::ScalarTerm::Application {
                            application,
                            scalar_type: length_type,
                        },
                    )
                }
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                    capacity,
                    ..
                }) => super::static_path::runtime_index_bound(index, carrier, capacity),
                StructuralTypeShape::ElementView { .. }
                | StructuralTypeShape::ByteSequence(
                    ByteSequenceCarrier::BorrowedView { .. }
                    | ByteSequenceCarrier::BoundedOwned { .. },
                ) => {
                    let count = ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
                    );
                    let Some(length) = measured_extent(prefix) else {
                        error = RuntimeIndexGoalError::MissingExtent;
                        return false;
                    };
                    Some(length)
                        .filter(|length| {
                            carrier == count && values.value_type(*length) == Some(count)
                        })
                        .map(|length| {
                            Proposition::LessThan(
                                semantic_vocabulary::ScalarTerm::value(index, count),
                                semantic_vocabulary::ScalarTerm::value(length, count),
                            )
                        })
                }
                _ => {
                    error = RuntimeIndexGoalError::OpenExtent;
                    return false;
                }
            };
            let Some(goal) = goal else {
                error = RuntimeIndexGoalError::UnsupportedSelector;
                return false;
            };
            visit(obligation, goal);
            true
        },
        None,
    );
    walked.ok_or(error).map(|_| ())
}

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
        None,
    )? {
        ProjectionTip::Declared(tip) => Some(Some(tip.id)),
        ProjectionTip::ScalarField(_) | ProjectionTip::ByteElement => Some(None),
    }
}

#[cfg(test)]
mod declaration_lookup_tests {
    use super::*;
    use crate::descriptor_lifetimes::StructuralTypeIndex;
    use semantic_vocabulary::StructuralFieldId;
    use terminal_psi::{BindingRelevance, StructuralRecordStorage, TerminalIdentityIndex};

    fn declaration(raw: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            requires: Vec::new(),
            id: StructuralTypeId::new(raw).unwrap(),
            identity: TerminalIdentityIndex(raw as u32),
            owns_linear_claim: false,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            no_code_affine_cleanup: true,
            descriptor_cleanup: true,
            nominal_cleanup: None,
            quotient_carrier: None,
            scalar_restriction: None,
            shape,
        }
    }

    fn field(
        raw: u64,
        field_type: StructuralFieldType,
        inline_carrier: Option<u64>,
    ) -> StructuralFieldDeclaration {
        StructuralFieldDeclaration {
            declaration_identity: Default::default(),
            id: StructuralFieldId::new(raw).unwrap(),
            identity: TerminalIdentityIndex(raw as u32),
            relevance: BindingRelevance::Relevant,
            storage: None,
            field_type,
            inline_carrier: inline_carrier.map(id),
        }
    }

    fn id(raw: u64) -> StructuralTypeId {
        StructuralTypeId::new(raw).unwrap()
    }

    fn byte() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap())
    }

    /// A record whose fields reach an inline scalar carrier, a fixed array,
    /// a scalar without a carrier, a type declared twice, an absent type and
    /// carriers that do not fit, plus representation views.
    fn declarations() -> Vec<StructuralTypeDeclaration> {
        let record = StructuralTypeShape::Record {
            storage: StructuralRecordStorage::Packed,
            fields: vec![
                field(1, StructuralFieldType::Scalar(ScalarType::Boolean), Some(2)),
                field(2, StructuralFieldType::Structural(id(3)), None),
                field(3, StructuralFieldType::Scalar(byte()), None),
                field(4, StructuralFieldType::Structural(id(5)), None),
                field(5, StructuralFieldType::Structural(id(42)), None),
                field(6, StructuralFieldType::Scalar(byte()), Some(2)),
                field(7, StructuralFieldType::Scalar(ScalarType::Boolean), Some(5)),
            ],
        };
        vec![
            declaration(1, record),
            declaration(2, StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)),
            declaration(
                3,
                StructuralTypeShape::FixedArray {
                    element: id(2),
                    length: terminal_psi::ArrayExtent::Literal(4),
                },
            ),
            declaration(4, StructuralTypeShape::PrimitiveScalar(byte())),
            declaration(5, StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)),
            declaration(5, StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)),
        ]
    }

    fn field_step(raw: u32) -> StructuralPathSegment {
        StructuralPathSegment::Field(TerminalIdentityIndex(raw))
    }

    fn view(raw: u64, stored_scalar: Option<ScalarType>) -> StructuralPathSegment {
        StructuralPathSegment::RepresentationView {
            structural_type: id(raw),
            stored_scalar,
        }
    }

    fn paths() -> Vec<Vec<StructuralPathSegment>> {
        vec![
            vec![],
            vec![field_step(1)],
            vec![field_step(2)],
            vec![field_step(2), StructuralPathSegment::FixedIndex(1)],
            vec![field_step(2), StructuralPathSegment::FixedIndex(9)],
            vec![field_step(3)],
            vec![field_step(4)],
            vec![field_step(5)],
            vec![field_step(6)],
            vec![field_step(7)],
            vec![field_step(99)],
            vec![view(3, None), StructuralPathSegment::FixedIndex(0)],
            vec![view(5, None)],
            vec![view(2, Some(ScalarType::Boolean))],
        ]
    }

    fn read_tip(position: Option<Position>) -> Option<String> {
        position.map(|position| match position {
            Position::Declared(id) => format!("declared {}", id.get()),
            Position::ScalarField(scalar) => format!("scalar {scalar:?}"),
            Position::ByteElement => "byte".to_owned(),
        })
    }

    #[test]
    fn the_type_index_walks_every_path_as_the_declaration_list_does() {
        let types = declarations();
        let index = StructuralTypeIndex::new(&types);
        for root in [1, 3, 5, 42].map(id) {
            for path in paths() {
                assert_eq!(
                    runtime_structural_path_tip(types.iter(), root, &path).map(std::ptr::from_ref),
                    runtime_structural_path_tip(&index, root, &path).map(std::ptr::from_ref),
                    "structural tip of {root:?} {path:?}"
                );
                assert_eq!(
                    read_tip(runtime_read_projection_tip(types.iter(), root, &path)),
                    read_tip(runtime_read_projection_tip(&index, root, &path)),
                    "read tip of {root:?} {path:?}"
                );
                assert_eq!(
                    crate::resolved_structural_path(types.iter(), root, &path),
                    crate::resolved_structural_path(&index, root, &path),
                    "resolved path of {root:?} {path:?}"
                );
                assert_eq!(
                    crate::runtime_structural_field(types.iter(), root, &path)
                        .map(std::ptr::from_ref),
                    crate::runtime_structural_field(&index, root, &path).map(std::ptr::from_ref),
                    "field of {root:?} {path:?}"
                );
            }
        }
    }

    // The comparison above means something only if the fixture reaches each
    // lookup, both where it resolves and where it refuses.
    #[test]
    fn the_fixture_reaches_carriers_views_duplicates_and_absences() {
        let types = declarations();
        let index = StructuralTypeIndex::new(&types);
        let tip = |path: &[StructuralPathSegment]| {
            runtime_structural_path_tip(&index, id(1), path).map(|declaration| declaration.id)
        };
        assert_eq!(tip(&[]), Some(id(1)));
        assert_eq!(
            tip(&[field_step(1)]),
            Some(id(2)),
            "an inline scalar carrier"
        );
        assert_eq!(
            tip(&[field_step(2), StructuralPathSegment::FixedIndex(1)]),
            Some(id(2))
        );
        assert_eq!(
            tip(&[field_step(2), StructuralPathSegment::FixedIndex(9)]),
            None
        );
        assert_eq!(
            tip(&[field_step(3)]),
            None,
            "a scalar field without a carrier"
        );
        assert_eq!(tip(&[field_step(4)]), None, "a type declared twice");
        assert_eq!(tip(&[field_step(5)]), None, "an absent type");
        assert_eq!(tip(&[field_step(6)]), None, "a carrier of another scalar");
        assert_eq!(tip(&[field_step(7)]), None, "a carrier declared twice");
        assert_eq!(
            tip(&[view(3, None), StructuralPathSegment::FixedIndex(0)]),
            Some(id(2)),
            "a representation view"
        );
        assert_eq!(tip(&[view(5, None)]), None, "a view declared twice");
        assert_eq!(
            read_tip(runtime_read_projection_tip(&index, id(1), &[field_step(3)])),
            Some(format!("scalar {:?}", byte()))
        );
        assert_eq!(runtime_structural_path_tip(&index, id(5), &[]), None);
    }
}
