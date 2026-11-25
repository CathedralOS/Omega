use super::{DescriptorLocation, DescriptorPathSegment};
use crate::exactly_one::exactly_one;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, PlaceId, ScalarType, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    ByteSequenceCarrier, StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
};

#[derive(Clone, Copy)]
enum TypeSlot {
    Empty,
    Unique(usize),
    /// Keeps the first matching index so `find` matches `Iterator::find` while
    /// `unique` still rejects duplicate IDs.
    Duplicate(usize),
}

impl TypeSlot {
    fn record(&mut self, index: usize) {
        *self = match *self {
            Self::Empty => Self::Unique(index),
            Self::Unique(first) | Self::Duplicate(first) => Self::Duplicate(first),
        };
    }

    fn first(self) -> Option<usize> {
        match self {
            Self::Empty => None,
            Self::Unique(index) | Self::Duplicate(index) => Some(index),
        }
    }

    fn unique(self) -> Option<usize> {
        match self {
            Self::Unique(index) => Some(index),
            Self::Empty | Self::Duplicate(_) => None,
        }
    }
}

/// Pre-indexed view over a module's structural type declarations so descriptor
/// replay resolves types, inline carriers, leaf carriers and representation
/// views in O(1) instead of scanning the full declaration slice on every step.
pub(crate) struct StructuralTypeIndex<'a> {
    types: &'a [StructuralTypeDeclaration],
    dense: Option<Vec<TypeSlot>>,
    sparse: Vec<(StructuralTypeId, TypeSlot)>,
    /// Indices of declarations whose shape can match `StructuralFieldType::declares_leaf`.
    leaf_indices: Vec<usize>,
}

impl<'a> StructuralTypeIndex<'a> {
    pub(crate) fn new(types: &'a [StructuralTypeDeclaration]) -> Self {
        let mut max_id = 0u64;
        let mut leaf_indices = Vec::new();
        for (index, declaration) in types.iter().enumerate() {
            max_id = max_id.max(declaration.id.get());
            if matches!(
                declaration.shape,
                StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::ByteSequence(_)
            ) {
                leaf_indices.push(index);
            }
        }
        let dense_limit = types.len().saturating_mul(4).saturating_add(64) as u64;
        if max_id <= dense_limit {
            let mut dense = vec![TypeSlot::Empty; (max_id as usize) + 1];
            for (index, declaration) in types.iter().enumerate() {
                dense[declaration.id.get() as usize].record(index);
            }
            Self {
                types,
                dense: Some(dense),
                sparse: Vec::new(),
                leaf_indices,
            }
        } else {
            let mut entries: Vec<(StructuralTypeId, usize)> = types
                .iter()
                .enumerate()
                .map(|(index, declaration)| (declaration.id, index))
                .collect();
            entries.sort_by_key(|(id, _)| *id);
            let mut sparse: Vec<(StructuralTypeId, TypeSlot)> = Vec::with_capacity(entries.len());
            for (id, index) in entries {
                if let Some((last_id, slot)) = sparse.last_mut()
                    && *last_id == id
                {
                    slot.record(index);
                } else {
                    sparse.push((id, TypeSlot::Unique(index)));
                }
            }
            Self {
                types,
                dense: None,
                sparse,
                leaf_indices,
            }
        }
    }

    fn slot(&self, id: StructuralTypeId) -> TypeSlot {
        if let Some(dense) = &self.dense {
            dense
                .get(id.get() as usize)
                .copied()
                .unwrap_or(TypeSlot::Empty)
        } else {
            self.sparse
                .binary_search_by_key(&id, |(entry_id, _)| *entry_id)
                .map_or(TypeSlot::Empty, |pos| self.sparse[pos].1)
        }
    }

    pub(crate) fn find(&self, id: StructuralTypeId) -> Option<&'a StructuralTypeDeclaration> {
        self.slot(id).first().map(|index| &self.types[index])
    }

    pub(crate) fn unique(&self, id: StructuralTypeId) -> Option<&'a StructuralTypeDeclaration> {
        self.slot(id).unique().map(|index| &self.types[index])
    }

    pub(crate) fn descriptor_location(
        &self,
        root: PlaceId,
        root_type: StructuralTypeId,
        path: &[StructuralPathSegment],
        final_field: Option<StructuralFieldId>,
    ) -> Option<DescriptorLocation> {
        descriptor_location_with(self, root, root_type, path, final_field)
    }

    pub(crate) fn descriptor_location_type(
        &self,
        root_type: StructuralTypeId,
        path: &[DescriptorPathSegment],
    ) -> Option<StructuralTypeId> {
        resolved_tip_with(self, root_type, path, |_, _, _| None)?
    }

    pub(crate) fn resolved_path(
        &self,
        root_type: StructuralTypeId,
        path: &[DescriptorPathSegment],
        constructor_element: impl FnMut(
            &[DescriptorPathSegment],
            StructuralTypeId,
            u64,
        ) -> Option<StructuralTypeId>,
    ) -> Option<(Vec<StructuralPathSegment>, Option<StructuralTypeId>)> {
        resolved_path_with(self, root_type, path, constructor_element)
    }

    pub(crate) fn resolved_tip(
        &self,
        root_type: StructuralTypeId,
        path: &[DescriptorPathSegment],
        constructor_element: impl FnMut(
            &[DescriptorPathSegment],
            StructuralTypeId,
            u64,
        ) -> Option<StructuralTypeId>,
    ) -> Option<Option<StructuralTypeId>> {
        resolved_tip_with(self, root_type, path, constructor_element)
    }
}

trait TypeLookup<'a> {
    fn unique_declaration(&self, id: StructuralTypeId) -> Option<&'a StructuralTypeDeclaration>;
    fn inline_carrier(
        &self,
        field: &StructuralFieldDeclaration,
    ) -> Option<&'a StructuralTypeDeclaration>;
    fn leaf_declaration(
        &self,
        field_type: &StructuralFieldType,
    ) -> Option<&'a StructuralTypeDeclaration>;
    fn representation_view(
        &self,
        view: StructuralTypeId,
        stored: Option<ScalarType>,
    ) -> Option<&'a StructuralTypeDeclaration>;
}

impl<'a> TypeLookup<'a> for &'a [StructuralTypeDeclaration] {
    fn unique_declaration(&self, id: StructuralTypeId) -> Option<&'a StructuralTypeDeclaration> {
        declaration(self, id)
    }

    fn inline_carrier(
        &self,
        field: &StructuralFieldDeclaration,
    ) -> Option<&'a StructuralTypeDeclaration> {
        crate::inline_carrier_declaration(self.iter(), field)
    }

    fn leaf_declaration(
        &self,
        field_type: &StructuralFieldType,
    ) -> Option<&'a StructuralTypeDeclaration> {
        field_type.leaf_declaration(self.iter())
    }

    fn representation_view(
        &self,
        view: StructuralTypeId,
        stored: Option<ScalarType>,
    ) -> Option<&'a StructuralTypeDeclaration> {
        crate::representation_view_declaration(self.iter(), view, stored)
    }
}

impl<'a> TypeLookup<'a> for StructuralTypeIndex<'a> {
    fn unique_declaration(&self, id: StructuralTypeId) -> Option<&'a StructuralTypeDeclaration> {
        self.unique(id)
    }

    fn inline_carrier(
        &self,
        field: &StructuralFieldDeclaration,
    ) -> Option<&'a StructuralTypeDeclaration> {
        if field.relevance.is_erased() {
            return None;
        }
        let carrier = self.unique(field.inline_carrier?)?;
        crate::inline_carrier_declaration(std::iter::once(carrier), field)
    }

    fn leaf_declaration(
        &self,
        field_type: &StructuralFieldType,
    ) -> Option<&'a StructuralTypeDeclaration> {
        if let StructuralFieldType::Structural(id) = field_type {
            return self.unique(*id);
        }
        field_type.leaf_declaration(self.leaf_indices.iter().map(|&index| &self.types[index]))
    }

    fn representation_view(
        &self,
        view: StructuralTypeId,
        stored: Option<ScalarType>,
    ) -> Option<&'a StructuralTypeDeclaration> {
        let declaration = self.unique(view)?;
        crate::representation_view_declaration(std::iter::once(declaration), view, stored)
    }
}

pub fn canonical_descriptor_location(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    root_type: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<DescriptorLocation> {
    let path = path
        .iter()
        .map(|segment| match segment {
            CanonicalStructuralPathSegment::Field(field) => {
                DescriptorPathSegment::Field(*field)
            }
            CanonicalStructuralPathSegment::FixedIndex(index) => {
                DescriptorPathSegment::FixedIndex(*index)
            }
            CanonicalStructuralPathSegment::Case(case) => {
                DescriptorPathSegment::Case(*case)
            }
            CanonicalStructuralPathSegment::Referent => DescriptorPathSegment::Referent,
        })
        .collect::<Vec<_>>();
    resolved_path(types, root_type, &path, |_, _, _| None)?;
    Some(DescriptorLocation { root, path })
}

pub fn descriptor_location(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    root_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    final_field: Option<StructuralFieldId>,
) -> Option<DescriptorLocation> {
    descriptor_location_with(&types, root, root_type, path, final_field)
}

fn descriptor_location_with<'a, L: TypeLookup<'a>>(
    types: &L,
    root: PlaceId,
    root_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    final_field: Option<StructuralFieldId>,
) -> Option<DescriptorLocation> {
    let mut result = DescriptorLocation::whole(root);
    let mut tip = Some(root_type);
    let mut case_fields = None;
    for segment in path {
        let declaration = types.unique_declaration(tip?)?;
        let canonical = match segment {
            StructuralPathSegment::RepresentationView {
                structural_type,
                stored_scalar,
            } if case_fields.is_none() => {
                tip = Some(
                    types
                        .representation_view(*structural_type, *stored_scalar)?
                        .id,
                );
                DescriptorPathSegment::RepresentationView {
                    structural_type: *structural_type,
                    stored_scalar: *stored_scalar,
                }
            }
            StructuralPathSegment::Referent if case_fields.is_none() => {
                let StructuralTypeShape::Reference { referent, .. } = declaration.shape
                else {
                    return None;
                };
                tip = Some(referent);
                DescriptorPathSegment::Referent
            }
            StructuralPathSegment::FixedIndex(index) if case_fields.is_none() => {
                tip = fixed_element(&declaration.shape, *index)?;
                DescriptorPathSegment::FixedIndex(*index)
            }
            StructuralPathSegment::Case(id) if case_fields.is_none() => {
                let cases = declaration.shape.sum_cases()?;
                let case = exactly_one(cases.iter(), |case| case.id == *id)?;
                case_fields = Some(case.fields.as_slice());
                DescriptorPathSegment::Case(*id)
            }
            StructuralPathSegment::Field(name) => {
                let fields = match case_fields.take() {
                    Some(fields) => fields,
                    None => fields(declaration)?,
                };
                let mut matching = fields.iter().filter(|field| {
                    field.identity == *name && !field.relevance.is_erased()
                });
                let field = matching.next()?;
                if matching.next().is_some()
                    || fields.iter().filter(|other| other.id == field.id).count() != 1
                {
                    return None;
                }
                tip = match field.field_type {
                    StructuralFieldType::Structural(id) => Some(id),
                    _ => None,
                };
                DescriptorPathSegment::Field(field.id)
            }
            _ => return None,
        };
        result.path.push(canonical);
    }
    if let Some(id) = final_field {
        let declaration = types.unique_declaration(tip?)?;
        let fields = match case_fields.take() {
            Some(fields) => fields,
            None => fields(declaration)?,
        };
        let mut matching = fields
            .iter()
            .filter(|field| field.id == id && !field.relevance.is_erased());
        let field = matching.next()?;
        if matching.next().is_some()
            || fields
                .iter()
                .filter(|other| other.identity == field.identity)
                .count()
                != 1
        {
            return None;
        }
        result.path.push(DescriptorPathSegment::Field(id));
    }
    case_fields.is_none().then_some(result)
}

pub fn fixed_runtime_index_alternatives(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    root_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<Vec<DescriptorLocation>> {
    let index_at = path.iter().position(|segment| {
        matches!(segment, StructuralPathSegment::RuntimeIndex { .. })
    })?;
    let suffix = &path[index_at + 1..];
    if suffix
        .iter()
        .any(|segment| matches!(segment, StructuralPathSegment::RuntimeIndex { .. }))
    {
        return None;
    }
    fixed_element_paths(types, root, root_type, &path[..index_at], suffix, None)?
        .map(|path| descriptor_location(types, root, root_type, &path, None))
        .collect()
}

/// One member is selected at run time. A caller may check each member for
/// overlap or availability, but cannot use the set as a single coordinate.
pub(crate) fn fixed_element_paths<'path>(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    root_type: StructuralTypeId,
    prefix: &'path [StructuralPathSegment],
    suffix: &'path [StructuralPathSegment],
    interval: Option<std::ops::Range<u64>>,
) -> Option<impl Iterator<Item = Vec<StructuralPathSegment>> + use<'path>> {
    let prefix_location = descriptor_location(types, root, root_type, prefix, None)?;
    let prefix_type = descriptor_location_type(types, root_type, &prefix_location.path)?;
    let length = indexed_carrier_length(types, prefix_type)?;
    let interval = interval.unwrap_or(0..length);
    if interval.is_empty() || interval.end > length || interval.end - interval.start > 4096 {
        return None;
    }
    let crosses_reference = matches!(
        declaration(types, prefix_type)?.shape,
        StructuralTypeShape::Reference { .. }
    );
    Some(interval.map(move |index| {
        let mut path = prefix.to_vec();
        if crosses_reference {
            path.push(StructuralPathSegment::Referent);
        }
        path.push(StructuralPathSegment::FixedIndex(index));
        path.extend_from_slice(suffix);
        path
    }))
}

fn indexed_carrier_length(
    types: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
) -> Option<u64> {
    let shape = &declaration(types, structural_type)?.shape;
    match shape {
        StructuralTypeShape::FixedArray { length, .. } => length.literal(),
        StructuralTypeShape::ByteSequence(
            ByteSequenceCarrier::BoundedView { capacity, .. }
            | ByteSequenceCarrier::BoundedOwned { capacity },
        ) => Some(*capacity),
        StructuralTypeShape::Reference { referent, .. } => {
            indexed_carrier_length(types, *referent)
        }
        _ => None,
    }
}

/// Indexing a view borrows through that descriptor, even when the
/// selected element is an ordinary scalar. This identifies permission,
/// not the address or extent of the payload.
pub fn view_element_descriptor_prefix(
    types: &[StructuralTypeDeclaration],
    root_type: StructuralTypeId,
    path: &[DescriptorPathSegment],
) -> Result<Option<usize>, &'static str> {
    let mut tip = Some(root_type);
    let mut case_fields = None;
    let mut element = None;
    for (position, segment) in path.iter().enumerate() {
        let owner = declaration(types, tip.ok_or("a loan path crosses a scalar leaf")?)
            .ok_or("a loan path has no exact structural declaration")?;
        if case_fields.is_none()
            && matches!(
                segment,
                DescriptorPathSegment::FixedIndex(_)
                    | DescriptorPathSegment::RuntimeIndex { .. }
            )
            && owner.shape.is_view_descriptor()
            && element.replace(position).is_some()
        {
            return Err("an inner view needs its own captured descriptor");
        }
        tip = match segment {
            DescriptorPathSegment::Field(id) => {
                let fields = match case_fields.take() {
                    Some(fields) => fields,
                    None => fields(owner).ok_or("a loan field has no record owner")?,
                };
                let field = exactly_one(fields.iter(), |field| {
                    field.id == *id && !field.relevance.is_erased()
                })
                .ok_or("a loan field has no exact declaration")?;
                match field.field_type {
                    StructuralFieldType::Structural(id) => Some(id),
                    _ => crate::inline_carrier_declaration(types.iter(), field)
                        .map(|declaration| declaration.id),
                }
            }
            DescriptorPathSegment::Case(id) if case_fields.is_none() => {
                let cases = owner
                    .shape
                    .sum_cases()
                    .ok_or("a loan case has no sum owner")?;
                let case = exactly_one(cases.iter(), |case| case.id == *id)
                    .ok_or("a loan case has no exact declaration")?;
                case_fields = Some(case.fields.as_slice());
                Some(owner.id)
            }
            DescriptorPathSegment::Referent if case_fields.is_none() => {
                if element.is_some() {
                    return Err("reference backing beneath an unresolved element selector");
                }
                let StructuralTypeShape::Reference { referent, .. } = owner.shape else {
                    return Err("a loan referent has no reference descriptor");
                };
                Some(referent)
            }
            DescriptorPathSegment::FixedIndex(_)
            | DescriptorPathSegment::RuntimeIndex { .. }
                if case_fields.is_none() =>
            {
                match owner.shape {
                    StructuralTypeShape::FixedArray { element, .. }
                    | StructuralTypeShape::ElementView { element, .. } => Some(element),
                    StructuralTypeShape::ByteSequence(_) => None,
                    _ => return Err("a loan index has no array or view owner"),
                }
            }
            DescriptorPathSegment::RepresentationView {
                structural_type,
                stored_scalar,
            } if case_fields.is_none() => Some(
                crate::representation_view_declaration(
                    types.iter(),
                    *structural_type,
                    *stored_scalar,
                )
                .ok_or("a loan presentation has no exact declaration")?
                .id,
            ),
            // Extent observations name the descriptor's storage, not
            // its separately borrowed bytes.
            DescriptorPathSegment::Extent => return Ok(element),
            _ => return Err("a loan path has no exact typed projection"),
        };
    }
    Ok(element)
}

pub fn descriptor_location_type(
    types: &[StructuralTypeDeclaration],
    root_type: StructuralTypeId,
    path: &[DescriptorPathSegment],
) -> Option<StructuralTypeId> {
    resolved_tip_with(&types, root_type, path, |_, _, _| None)?
}
pub fn descriptor_path(
    types: &[StructuralTypeDeclaration],
    root_type: StructuralTypeId,
    path: &[DescriptorPathSegment],
) -> Option<Vec<StructuralPathSegment>> {
    Some(resolved_path_with(&types, root_type, path, |_, _, _| None)?.0)
}
pub(crate) fn resolved_path(
    types: &[StructuralTypeDeclaration],
    root_type: StructuralTypeId,
    path: &[DescriptorPathSegment],
    constructor_element: impl FnMut(
        &[DescriptorPathSegment],
        StructuralTypeId,
        u64,
    ) -> Option<StructuralTypeId>,
) -> Option<(Vec<StructuralPathSegment>, Option<StructuralTypeId>)> {
    resolved_path_with(&types, root_type, path, constructor_element)
}

fn resolved_path_with<'a, L: TypeLookup<'a>>(
    types: &L,
    root_type: StructuralTypeId,
    path: &[DescriptorPathSegment],
    mut constructor_element: impl FnMut(
        &[DescriptorPathSegment],
        StructuralTypeId,
        u64,
    ) -> Option<StructuralTypeId>,
) -> Option<(Vec<StructuralPathSegment>, Option<StructuralTypeId>)> {
    let mut result = Vec::with_capacity(path.len());
    let mut tip = Some(root_type);
    let mut case_fields = None;
    for segment in path {
        let declaration = types.unique_declaration(tip?)?;
        result.push(match segment {
            DescriptorPathSegment::RepresentationView {
                structural_type,
                stored_scalar,
            } if case_fields.is_none() => {
                tip = Some(
                    types
                        .representation_view(*structural_type, *stored_scalar)?
                        .id,
                );
                StructuralPathSegment::RepresentationView {
                    structural_type: *structural_type,
                    stored_scalar: *stored_scalar,
                }
            }
            DescriptorPathSegment::Referent if case_fields.is_none() => {
                let StructuralTypeShape::Reference { referent, .. } = declaration.shape
                else {
                    return None;
                };
                tip = Some(referent);
                StructuralPathSegment::Referent
            }
            DescriptorPathSegment::FixedIndex(index) if case_fields.is_none() => {
                tip = match declaration.shape {
                    StructuralTypeShape::ElementView {
                        access: None | Some(terminal_psi::StructuralAccess::SharedBorrow),
                        ..
                    } => {
                        let prefix = &path[..result.len()];
                        Some(constructor_element(prefix, declaration.id, *index)?)
                    }
                    _ => fixed_element(&declaration.shape, *index)?,
                };
                StructuralPathSegment::FixedIndex(*index)
            }
            DescriptorPathSegment::Case(id) if case_fields.is_none() => {
                let cases = declaration.shape.sum_cases()?;
                let case = exactly_one(cases.iter(), |case| case.id == *id)?;
                case_fields = Some(case.fields.as_slice());
                StructuralPathSegment::Case(*id)
            }
            DescriptorPathSegment::Field(id) => {
                let fields = match case_fields.take() {
                    Some(fields) => fields,
                    None => fields(declaration)?,
                };
                let field = exactly_one(fields.iter(), |field| {
                    field.id == *id && !field.relevance.is_erased()
                })?;
                // A leaf field that states its carrier names its
                // exact declaration. Reading it back from the leaf
                // shape finds nothing as soon as two declarations
                // share that shape, as the byte views of two regions
                // do, and a path ending at such a field would then
                // have no type at all.
                tip = match field.field_type {
                    StructuralFieldType::Structural(id) => Some(id),
                    _ => types
                        .inline_carrier(field)
                        .or_else(|| {
                            field
                                .field_type
                                .canonical_leaf_type()
                                .and_then(|leaf| types.leaf_declaration(&leaf))
                        })
                        .map(|declaration| declaration.id),
                };
                StructuralPathSegment::Field(field.identity)
            }
            _ => return None,
        });
    }
    case_fields.is_none().then_some((result, tip))
}

fn resolved_tip_with<'a, L: TypeLookup<'a>>(
    types: &L,
    root_type: StructuralTypeId,
    path: &[DescriptorPathSegment],
    mut constructor_element: impl FnMut(
        &[DescriptorPathSegment],
        StructuralTypeId,
        u64,
    ) -> Option<StructuralTypeId>,
) -> Option<Option<StructuralTypeId>> {
    let mut tip = Some(root_type);
    let mut case_fields = None;
    for (index, segment) in path.iter().enumerate() {
        let declaration = types.unique_declaration(tip?)?;
        match segment {
            DescriptorPathSegment::RepresentationView {
                structural_type,
                stored_scalar,
            } if case_fields.is_none() => {
                tip = Some(
                    types
                        .representation_view(*structural_type, *stored_scalar)?
                        .id,
                );
            }
            DescriptorPathSegment::Referent if case_fields.is_none() => {
                let StructuralTypeShape::Reference { referent, .. } = declaration.shape else {
                    return None;
                };
                tip = Some(referent);
            }
            DescriptorPathSegment::FixedIndex(ordinal) if case_fields.is_none() => {
                tip = match declaration.shape {
                    StructuralTypeShape::ElementView {
                        access: None | Some(terminal_psi::StructuralAccess::SharedBorrow),
                        ..
                    } => {
                        let prefix = &path[..index];
                        Some(constructor_element(prefix, declaration.id, *ordinal)?)
                    }
                    _ => fixed_element(&declaration.shape, *ordinal)?,
                };
            }
            DescriptorPathSegment::Case(id) if case_fields.is_none() => {
                let cases = declaration.shape.sum_cases()?;
                let case = exactly_one(cases.iter(), |case| case.id == *id)?;
                case_fields = Some(case.fields.as_slice());
            }
            DescriptorPathSegment::Field(id) => {
                let fields = match case_fields.take() {
                    Some(fields) => fields,
                    None => fields(declaration)?,
                };
                let field = exactly_one(fields.iter(), |field| {
                    field.id == *id && !field.relevance.is_erased()
                })?;
                tip = match field.field_type {
                    StructuralFieldType::Structural(id) => Some(id),
                    _ => types
                        .inline_carrier(field)
                        .or_else(|| {
                            field
                                .field_type
                                .canonical_leaf_type()
                                .and_then(|leaf| types.leaf_declaration(&leaf))
                        })
                        .map(|declaration| declaration.id),
                };
            }
            _ => return None,
        }
    }
    case_fields.is_none().then_some(tip)
}

fn declaration(
    types: &[StructuralTypeDeclaration],
    id: StructuralTypeId,
) -> Option<&StructuralTypeDeclaration> {
    let mut matching = types.iter().filter(|declaration| declaration.id == id);
    let declaration = matching.next()?;
    matching.next().is_none().then_some(declaration)
}
fn fields(
    declaration: &StructuralTypeDeclaration,
) -> Option<&[terminal_psi::StructuralFieldDeclaration]> {
    match &declaration.shape {
        terminal_psi::record_or_mixed_shape_pattern!(fields) => Some(fields),
        _ => None,
    }
}

fn fixed_element(
    shape: &StructuralTypeShape,
    index: u64,
) -> Option<Option<StructuralTypeId>> {
    match shape {
        StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(length),
        } if index < *length => Some(Some(*element)),
        StructuralTypeShape::ByteSequence(
            ByteSequenceCarrier::BoundedView { capacity, .. }
            | ByteSequenceCarrier::BoundedOwned { capacity },
        ) if index < *capacity => Some(None),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::ScalarType;
    use terminal_psi::{StructuralMultiplicity, TerminalIdentityIndex};

    fn scalar_decl(id: u64, scalar: ScalarType) -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(id).unwrap(),
            identity: TerminalIdentityIndex::ABSENT,
            declaration_identity: TerminalIdentityIndex::ABSENT,
            shape: StructuralTypeShape::PrimitiveScalar(scalar),
            multiplicity: StructuralMultiplicity::Unrestricted,
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            quotient_carrier: None,
            scalar_restriction: None,
            requires: Vec::new(),
        }
    }

    #[test]
    fn type_index_matches_linear_lookup_for_dense_sparse_and_duplicate_ids() {
        let dense = [
            scalar_decl(1, ScalarType::Boolean),
            scalar_decl(
                2,
                ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32),
            ),
            scalar_decl(
                2,
                ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64),
            ),
        ];
        let index = StructuralTypeIndex::new(&dense);
        assert!(index.dense.is_some());
        let id1 = StructuralTypeId::new(1).unwrap();
        let id2 = StructuralTypeId::new(2).unwrap();
        let id3 = StructuralTypeId::new(3).unwrap();
        assert_eq!(
            index.find(id1),
            terminal_psi::find_structural_type(&dense, id1)
        );
        assert_eq!(index.unique(id1), declaration(&dense, id1));
        assert_eq!(
            index.find(id2),
            terminal_psi::find_structural_type(&dense, id2)
        );
        assert_eq!(index.unique(id2), None);
        assert_eq!(index.find(id3), None);
        assert_eq!(index.unique(id3), None);

        let sparse = [
            scalar_decl(1, ScalarType::Boolean),
            scalar_decl(
                1_000_000,
                ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32),
            ),
            scalar_decl(
                1_000_000,
                ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64),
            ),
        ];
        let sparse_index = StructuralTypeIndex::new(&sparse);
        assert!(sparse_index.dense.is_none());
        let big = StructuralTypeId::new(1_000_000).unwrap();
        assert_eq!(
            sparse_index.find(id1),
            terminal_psi::find_structural_type(&sparse, id1)
        );
        assert_eq!(sparse_index.unique(id1), declaration(&sparse, id1));
        assert_eq!(
            sparse_index.find(big),
            terminal_psi::find_structural_type(&sparse, big)
        );
        assert_eq!(sparse_index.unique(big), None);
        assert_eq!(sparse_index.find(id3), None);
    }
}
