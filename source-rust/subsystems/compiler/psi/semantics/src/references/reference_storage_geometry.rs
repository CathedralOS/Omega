use semantic_vocabulary::{
    AuthoredFieldStorage, ScalarApplicationId, ScalarType, StructuralTypeId,
    TargetCarrierLayout,
};
use terminal_psi::{
    LayoutApplicationId, StructuralFieldDeclaration, StructuralFieldType,
    StructuralPathSegment, StructuralRecordStorage, StructuralTypeDeclaration,
    StructuralTypeShape, TargetLayoutRealization,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceStorageProjection {
    pub byte_offset: u64,
    pub logical_scalar: ScalarType,
    pub stored_scalar: ScalarType,
}

pub enum ReferenceScalarStorage<'recipe> {
    Contiguous(ReferenceStorageProjection),
    Fragmented {
        byte_offset: u64,
        logical_scalar: ScalarType,
        fragments: &'recipe semantic_vocabulary::AuthoredBitFragments,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceStorageRegion {
    pub byte_offset: u64,
    pub byte_length: u64,
    pub scalar: Option<ReferenceStorageProjection>,
    pub array: Option<ReferenceArrayWindow>,
}

impl ReferenceStorageRegion {
    /// A base divisible by `base_alignment` and shifted by `byte_offset`
    /// retains the smaller of their powers of two. Offset zero keeps the
    /// entire base guarantee, including for the first field of a record.
    pub fn alignment_at(&self, base_alignment: u64) -> Option<u64> {
        base_alignment.is_power_of_two().then(|| {
            1u64 << base_alignment
                .trailing_zeros()
                .min(self.byte_offset.trailing_zeros())
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceArrayWindow {
    pub declaration: StructuralTypeId,
    pub byte_offset: u64,
    pub byte_length: u64,
}

fn align(offset: u64, alignment: u64) -> Option<u64> {
    offset
        .checked_add(alignment.checked_sub(1)?)
        .map(|value| value / alignment * alignment)
}

fn scalar_geometry(scalar: ScalarType) -> Option<(u64, u64)> {
    let width = u64::from(crate::reference_storage_byte_width(scalar)?);
    Some((width, width))
}

// Placement follows the existing native default, not the stricter
// byte-view codec query. Address placement remains target-owned.
fn primitive_scalar_geometry(
    scalar: ScalarType,
    carriers: Option<TargetCarrierLayout>,
) -> Result<(u64, u64), StorageGeometryError> {
    scalar_placement(
        scalar,
        carriers,
        layout_planning::standalone_integer_geometry,
    )
}

fn scalar_placement(
    scalar: ScalarType,
    carriers: Option<TargetCarrierLayout>,
    integer_geometry: fn(u16) -> Option<layout_plans::StorageGeometry>,
) -> Result<(u64, u64), StorageGeometryError> {
    match scalar {
        ScalarType::Integer(integer) if integer.is_address() => {
            let carriers = carriers.ok_or(StorageGeometryError::MissingCarrierLayout)?;
            carrier_geometry(carriers.pointer_byte_size, carriers.pointer_byte_alignment)
        }
        ScalarType::Integer(integer) => {
            let integer = semantic_vocabulary::ClosedIntegerType::try_from(integer)
                .map_err(|_| StorageGeometryError::UnsupportedScalar(scalar))?;
            let layout = integer_geometry(integer.bits())
                .ok_or(StorageGeometryError::UnsupportedScalar(scalar))?;
            Ok((layout.byte_size, layout.byte_alignment))
        }
        _ => scalar_geometry(scalar).ok_or(StorageGeometryError::UnsupportedScalar(scalar)),
    }
}

/// Physical record-field geometry before the layout policy places the field.
pub fn scalar_field_storage_geometry(
    scalar: ScalarType,
    carriers: Option<TargetCarrierLayout>,
) -> Result<(u64, u64), StorageGeometryError> {
    scalar_placement(scalar, carriers, layout_planning::field_integer_geometry)
}

fn field_geometry<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    owner: StructuralTypeId,
    field: &StructuralFieldDeclaration,
    storage: Option<&AuthoredFieldStorage>,
    active: &mut Vec<StructuralTypeId>,
    carriers: Option<TargetCarrierLayout>,
    records: &impl RecordStorageResolver,
    extents: &mut impl FnMut(ScalarApplicationId) -> Result<Option<u64>, StorageGeometryError>,
) -> Result<Option<(u64, u64)>, StorageGeometryError> {
    if let Some(scalar) = field.field_type.scalar_type() {
        return scalar_field_storage_geometry(
            storage
                .and_then(AuthoredFieldStorage::stored_scalar)
                .unwrap_or(scalar),
            carriers,
        )
        .map(Some);
    }
    if let Some(carrier) = field.field_type.byte_sequence_carrier() {
        if let Some(layout) = physical_carrier_geometry(records, owner, &field.field_type)? {
            return Ok(Some(layout));
        }
        return byte_sequence_geometry(carrier, carriers).map(Some);
    }
    let child = field
        .field_type
        .as_structural()
        .ok_or(StorageGeometryError::UnsupportedField)?;
    geometry(types, child, active, carriers, records, extents)
}

pub use terminal_psi::layouts::record_storage::{RecordStorageResolver, StorageGeometryError};

pub struct UnselectedRecordStorage;

impl RecordStorageResolver for UnselectedRecordStorage {
    fn realization(
        &self,
        record: StructuralTypeId,
        _application: LayoutApplicationId,
    ) -> Result<&TargetLayoutRealization, StorageGeometryError> {
        Err(StorageGeometryError::UnresolvedLayout(record))
    }
}

/// The checked representation width used by both source plan validation
/// and foreign storage recipes. Bounded nonnegative integers retain every
/// representable bit, independently of their byte-sized home carrier.
pub fn reference_field_representation_width<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    field: &StructuralFieldType,
) -> Option<u64> {
    if let StructuralFieldType::BoundedInteger(integer) = field {
        use semantic_vocabulary::IntegerValue;
        let maximum = match (integer.minimum(), integer.maximum()) {
            (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) if minimum >= 0 => {
                u128::try_from(maximum).ok()?
            }
            (IntegerValue::Unsigned(_), IntegerValue::Unsigned(maximum)) => maximum,
            _ => return Some(u64::from(integer.integer_type().bits()?)),
        };
        return Some(u64::from((u128::BITS - maximum.leading_zeros()).max(1)));
    }
    let scalar = field.scalar_type().or_else(|| {
        let identity = field.as_structural()?;
        let StructuralTypeShape::PrimitiveScalar(scalar) =
            crate::structural_declaration(types, identity)?.shape
        else {
            return None;
        };
        Some(scalar)
    })?;
    match scalar {
        // Code references keep their contract and custody; numeric fragments
        // cannot represent that association.
        ScalarType::Callable(_) => None,
        ScalarType::Boolean => Some(1),
        ScalarType::Integer(integer) => Some(u64::from(integer.bits()?)),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => Some(32),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => Some(64),
    }
}

/// Compute and check a retained storage recipe against its actual field.
/// The caller supplies child geometry from its own immutable target and
/// selected-layout context; foreign validation keeps its boundary policy.
pub fn authored_field_storage_extent<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    field: &terminal_psi::StructuralFieldDeclaration,
    storage: &AuthoredFieldStorage,
    record_size: u64,
    mut child_layout: impl FnMut(StructuralTypeId) -> Option<(u64, u64)>,
) -> Result<Option<(u64, u64)>, &'static str> {
    use semantic_vocabulary::AuthoredFieldStorage;

    let scalar = field
        .field_type
        .scalar_type()
        .or_else(|| field.field_type.atomic_scalar_type());
    let mut declared_layout = || {
        if let Some(scalar) = scalar {
            let width = u64::from(crate::reference_storage_byte_width(scalar)?);
            return Some((width, width));
        }
        let child = field.field_type.as_structural()?;
        child_layout(child)
    };
    let (offset, extent) = match storage {
        AuthoredFieldStorage::Fragmented(recipe) => {
            validate_storage_fragments(types.clone(), field, recipe, record_size)?;
            return Ok(None);
        }
        AuthoredFieldStorage::Scalar {
            byte_offset,
            stored_scalar: Some(stored),
        } => {
            let width = crate::reference_storage_byte_width(*stored)
                .ok_or("encoded field has no contiguous scalar footprint")?;
            (byte_offset, Some(u64::from(width)))
        }
        AuthoredFieldStorage::Scalar {
            byte_offset,
            stored_scalar: None,
        }
        | AuthoredFieldStorage::Direct { byte_offset } => {
            (byte_offset, declared_layout().map(|(size, _)| size))
        }
        AuthoredFieldStorage::Repeated {
            byte_offset,
            element_stride,
        } => {
            let Some(array) = field.field_type.as_structural() else {
                return Err("repeated field requires a fixed array");
            };
            let Some(StructuralTypeShape::FixedArray { element, length }) =
                crate::structural_declaration(types.clone(), array)
                    .map(|declaration| &declaration.shape)
            else {
                return Err("repeated field requires a fixed array");
            };
            if let Some((size, alignment)) = child_layout(*element)
                && (alignment == 0
                    || *element_stride < size
                    || *element_stride % alignment != 0
                    || *byte_offset % alignment != 0)
            {
                return Err("repeated field stride overlaps or misaligns its elements");
            }
            let length = length
                .literal()
                .ok_or("authored repeated storage requires a closed array extent")?;
            let extent = length
                .checked_mul(*element_stride)
                .ok_or("repeated field extent overflows")?;
            (byte_offset, Some(extent))
        }
    };
    if extent.is_some_and(|extent| {
        offset
            .checked_add(extent)
            .is_none_or(|end| end > record_size)
    }) {
        return Err("authored field extent exceeds its record storage");
    }
    Ok(extent.map(|extent| (*offset, extent)))
}

fn validate_storage_fragments<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    field: &terminal_psi::StructuralFieldDeclaration,
    recipe: &semantic_vocabulary::AuthoredBitFragments,
    record_size: u64,
) -> Result<(), &'static str> {
    let source_width = reference_field_representation_width(types, &field.field_type)
        .ok_or("fragmented field requires a known scalar representation width")?;
    if recipe.fragments().is_empty() {
        return Err("fragmented field requires a complete recipe");
    }
    let mut source_intervals = Vec::with_capacity(recipe.fragments().len());
    for fragment in recipe.fragments() {
        if fragment.container_width_bits == 0
            || !fragment.container_width_bits.is_multiple_of(8)
            || fragment.width == 0
            || fragment
                .container_byte_offset
                .checked_add(fragment.container_width_bits / 8)
                .is_none_or(|end| end > record_size)
            || fragment
                .destination_lsb
                .checked_add(fragment.width)
                .is_none_or(|end| end > fragment.container_width_bits)
        {
            return Err("fragmented field has an invalid transfer container or destination");
        }
        let end = fragment
            .source_lsb
            .checked_add(fragment.width)
            .ok_or("fragment source range overflows")?;
        if end > source_width {
            return Err("fragment source range exceeds its declaration representation");
        }
        source_intervals.push((fragment.source_lsb, end));
    }
    source_intervals.sort_unstable();
    let mut cursor = 0;
    for (start, end) in source_intervals {
        if start != cursor {
            return Err("fragment source ranges have a gap or overlap");
        }
        cursor = end;
    }
    if cursor != source_width {
        return Err("fragment source ranges do not tile the declaration representation");
    }
    Ok(())
}

/// Check occupied destination bits after all recipes are computed. Scratch
/// is proportional to those recipes, never to the declared byte extent.
pub fn validate_storage_destination_intervals(
    mut destinations: Vec<(u128, u128)>,
) -> Result<(), &'static str> {
    destinations.sort_unstable();
    if destinations.windows(2).any(|pair| pair[1].0 < pair[0].1) {
        return Err("authored field destination bits overlap");
    }
    Ok(())
}

/// Classify the representation source of one field. This describes
/// geometry only; an explicit child still needs the caller's exact
/// selected realization and supplies no storage or access permission.
pub fn schema_field_layout_source(
    module: &terminal_psi::TerminalModule,
    field: &terminal_psi::StructuralFieldType,
    records: &impl RecordStorageResolver,
) -> Result<terminal_psi::TargetLayoutFieldLayoutSource, StorageGeometryError> {
    use terminal_psi::{ByteSequenceCarrier, TargetLayoutFieldLayoutSource};
    if field.scalar_type().is_some()
        || matches!(
            field.byte_sequence_carrier(),
            Some(ByteSequenceCarrier::BoundedOwned { .. })
        )
    {
        return Ok(TargetLayoutFieldLayoutSource::Scalar);
    }
    let Some(mut identity) = field.as_structural() else {
        return Ok(TargetLayoutFieldLayoutSource::Default);
    };
    // Each step follows one array child. A chain longer than the
    // declaration roster repeats a type, without whole-module scratch.
    let mut remaining = module.types.structural_types.len();
    loop {
        let declaration = module
            .structural_type(identity)
            .ok_or(StorageGeometryError::MissingType(identity))?;
        if remaining == 0 {
            return Err(StorageGeometryError::RecursiveType(identity));
        }
        remaining -= 1;
        match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. }) => {
                return Ok(TargetLayoutFieldLayoutSource::Scalar);
            }
            StructuralTypeShape::FixedArray { element, length }
                if length.literal().is_some() =>
            {
                identity = *element;
            }
            StructuralTypeShape::Record {
                storage: StructuralRecordStorage::TargetLayout(application),
                ..
            } => {
                records.realization(identity, *application)?;
                return Ok(TargetLayoutFieldLayoutSource::Explicit);
            }
            _ => return Ok(TargetLayoutFieldLayoutSource::Default),
        }
    }
}

pub struct ReferenceRecordField<'storage> {
    pub declaration: &'storage StructuralFieldDeclaration,
    pub storage: Option<&'storage AuthoredFieldStorage>,
    pub byte_offset: Option<u64>,
    pub byte_size: u64,
    pub byte_alignment: u64,
    pub repeated: Option<ReferenceRepeatedField>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceRepeatedField {
    pub element_size: u64,
    pub element_count: u64,
    pub element_stride: u64,
}

pub struct ReferenceStorageGeometry<Types, Extents, Records = UnselectedRecordStorage> {
    types: Types,
    extents: Extents,
    carriers: Option<TargetCarrierLayout>,
    records: Records,
}

impl<'types, Types, Extents> ReferenceStorageGeometry<Types, Extents, UnselectedRecordStorage>
where
    Types: Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    Extents: FnMut(ScalarApplicationId) -> Result<u64, StorageGeometryError>,
{
    pub fn new(types: Types, extents: Extents) -> Self {
        Self {
            types,
            extents,
            carriers: None,
            records: UnselectedRecordStorage,
        }
    }
}

impl<'types, Types, Extents, Records> ReferenceStorageGeometry<Types, Extents, Records>
where
    Types: Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    Extents: FnMut(ScalarApplicationId) -> Result<u64, StorageGeometryError>,
    Records: RecordStorageResolver,
{
    pub fn with_record_storage<Selected: RecordStorageResolver>(
        self,
        records: Selected,
    ) -> ReferenceStorageGeometry<Types, Extents, Selected> {
        ReferenceStorageGeometry {
            types: self.types,
            extents: self.extents,
            carriers: self.carriers,
            records,
        }
    }

    pub fn with_target_carriers(mut self, carriers: Option<TargetCarrierLayout>) -> Self {
        self.carriers = carriers;
        self
    }

    pub fn dependencies(
        &self,
        root: StructuralTypeId,
    ) -> Result<Vec<ScalarApplicationId>, StorageGeometryError> {
        let mut dependencies = Vec::new();
        geometry(
            self.types.clone(),
            root,
            &mut Vec::new(),
            self.carriers,
            &self.records,
            &mut |application| {
                if !dependencies.contains(&application) {
                    dependencies.push(application);
                }
                Ok(None)
            },
        )?;
        Ok(dependencies)
    }

    pub fn layout(
        &mut self,
        root: StructuralTypeId,
    ) -> Result<(u64, u64), StorageGeometryError> {
        geometry(
            self.types.clone(),
            root,
            &mut Vec::new(),
            self.carriers,
            &self.records,
            &mut |application| (self.extents)(application).map(Some),
        )?
        .ok_or(StorageGeometryError::UnresolvedLayout(root))
    }

    pub fn sum_layout(
        &mut self,
        root: StructuralTypeId,
    ) -> Result<layout_plans::ConventionalSumGeometry, StorageGeometryError> {
        let declaration = crate::structural_declaration(self.types.clone(), root)
            .ok_or(StorageGeometryError::MissingType(root))?;
        let (fields, cases) = match &declaration.shape {
            StructuralTypeShape::Sum { cases } => (&[][..], cases),
            StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases),
            _ => return Err(StorageGeometryError::UnsupportedType(root)),
        };
        sum_layout(
            self.types.clone(),
            root,
            fields,
            cases,
            &mut vec![root],
            self.carriers,
            &self.records,
            &mut |application| (self.extents)(application).map(Some),
        )?
        .ok_or(StorageGeometryError::UnresolvedLayout(root))
    }

    pub fn region(
        &mut self,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<ReferenceStorageRegion> {
        storage_region(
            self.types.clone(),
            root,
            path,
            &self.records,
            self.carriers,
            &mut self.extents,
        )
    }

    /// Limit an observation to the geometry of its declared storage region.
    /// Following a pointer starts a new region. A slice's first element
    /// belongs to its backing allocation; the descriptor layout says nothing
    /// about that element's address. This limit alone proves no alignment.
    pub fn location_alignment_limit(
        &mut self,
        mut region: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<u64> {
        let mut start = 0;
        for (position, segment) in path.iter().enumerate() {
            if !matches!(
                segment,
                StructuralPathSegment::Referent
                    | StructuralPathSegment::FixedIndex(_)
                    | StructuralPathSegment::RuntimeIndex { .. }
            ) {
                continue;
            }
            let parent = crate::runtime_structural_path_tip(
                self.types.clone(),
                region,
                &path[start..position],
            )?;
            match (&parent.shape, segment) {
                (
                    StructuralTypeShape::Reference { referent, .. },
                    StructuralPathSegment::Referent,
                ) => region = *referent,
                (
                    StructuralTypeShape::ElementView { element, .. },
                    StructuralPathSegment::FixedIndex(_)
                    | StructuralPathSegment::RuntimeIndex { .. },
                ) => region = *element,
                (
                    shape,
                    StructuralPathSegment::FixedIndex(_)
                    | StructuralPathSegment::RuntimeIndex { .. },
                ) if shape.is_borrowed_byte_carrier() => return Some(1),
                (_, StructuralPathSegment::Referent) => return None,
                _ => continue,
            }
            start = position + 1;
        }
        let path = &path[start..];
        let (_, base) = self.layout(region).ok()?;
        let projected = self
            .region(region, path)
            .and_then(|region| region.alignment_at(base));
        let leaf = match crate::primitive_read_projection_type(self.types.clone(), region, path) {
            Some(scalar) => primitive_scalar_geometry(scalar, self.carriers).ok()?.1,
            None => {
                let leaf = crate::runtime_structural_path_tip(self.types.clone(), region, path)?;
                self.layout(leaf.id).ok()?.1
            }
        };
        Some(leaf.min(projected.unwrap_or(base)))
    }

    pub fn source_compatible(
        &mut self,
        source: StructuralTypeId,
        reference: StructuralTypeId,
        stored: Option<ScalarType>,
    ) -> bool {
        self.source_representation_compatible(source, reference, stored, false)
    }

    fn source_representation_compatible(
        &mut self,
        source: StructuralTypeId,
        reference: StructuralTypeId,
        stored: Option<ScalarType>,
        captured_geometry_matches: bool,
    ) -> bool {
        let Some(StructuralTypeShape::Reference { referent, .. }) =
            crate::structural_shape(self.types.clone(), reference)
        else {
            return false;
        };
        let Some(source_shape) = crate::structural_shape(self.types.clone(), source) else {
            return false;
        };
        let byte_endpoint = matches!(source_shape, StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
        if !integer.is_address() && integer.bits() == Some(8)
            && integer.sign() == semantic_vocabulary::IntegerSign::Unsigned);
        let Some(target) = crate::structural_shape(self.types.clone(), *referent) else {
            return false;
        };
        match (stored, target) {
            (Some(stored), StructuralTypeShape::PrimitiveScalar(logical)) => {
                let StructuralTypeShape::PrimitiveScalar(source) = source_shape else {
                    return false;
                };
                (byte_endpoint
                    || source == logical
                    || crate::scalar_reference_view_is_total(*source, *logical))
                    && crate::reference_storage_view_is_compatible(stored, *logical)
            }
            (None, terminal_psi::record_or_fixed_array_shape_pattern!()) => {
                use crate::references::reference_views::{
                    is_byte_scalar, plain_storage, storage_scalars_satisfy,
                };

                if !plain_storage(self.types.clone(), *referent, &mut Vec::new()) {
                    return false;
                }
                if byte_endpoint {
                    return true;
                }
                if !plain_storage(self.types.clone(), source, &mut Vec::new()) {
                    return false;
                }
                if let (Ok(source_layout), Ok(target_layout)) =
                    (self.layout(source), self.layout(*referent))
                    && (source_layout == target_layout
                        || (source_layout.0 == target_layout.0
                            && target_layout.1 != 0
                            && source_layout.1.is_multiple_of(target_layout.1)
                            && storage_scalars_satisfy(
                                self.types.clone(),
                                *referent,
                                &mut Vec::new(),
                                &is_byte_scalar,
                            )))
                {
                    return true;
                }
                // The captured address may be more aligned than its type.
                // Every byte must still belong to the source representation.
                captured_geometry_matches
                    && crate::representation_element_layout(self.types.clone(), source)
                        .is_some_and(|layout| layout.is_tightly_tiled())
            }
            _ => false,
        }
    }

    pub fn view_source_compatible(
        &mut self,
        source: StructuralTypeId,
        path: &[StructuralPathSegment],
        reference: StructuralTypeId,
        stored: Option<ScalarType>,
    ) -> bool {
        if matches!(path.last(), Some(StructuralPathSegment::FixedRange { .. })) {
            return self.region_source_compatible(source, path, reference, stored);
        }
        let Some(region) = self.region(source, path) else {
            return false;
        };
        let Some(StructuralTypeShape::Reference { referent, .. }) =
            crate::structural_shape(self.types.clone(), reference)
        else {
            return false;
        };
        let Ok((target_size, target_align)) = self.layout(*referent) else {
            return false;
        };
        let Ok((_, source_align)) = self.layout(source) else {
            return false;
        };
        let width = match stored {
            Some(stored) => {
                if region
                    .scalar
                    .is_none_or(|scalar| scalar.stored_scalar != stored)
                {
                    return false;
                }
                match crate::reference_storage_byte_width(stored) {
                    Some(width) => u64::from(width),
                    None => return false,
                }
            }
            None => target_size,
        };
        if region.byte_length != width
            || target_align == 0
            || !region.alignment_at(source_align).is_some_and(|alignment| alignment.is_multiple_of(target_align))
        {
            return false;
        }
        let tip = crate::runtime_structural_path_tip(self.types.clone(), source, path)
            .map(|row| row.id)
            .or_else(|| {
                crate::primitive_projection_declaration(self.types.clone(), source, path)
            });
        // The checked root and byte offset establish this address's
        // alignment even when the selected field has a lower minimum.
        tip.is_some_and(|tip| {
            self.source_representation_compatible(tip, reference, stored, true)
        })
    }

    pub fn region_source_compatible(
        &mut self,
        source: StructuralTypeId,
        path: &[StructuralPathSegment],
        reference: StructuralTypeId,
        stored: Option<ScalarType>,
    ) -> bool {
        if stored.is_some() {
            return false;
        }
        let Some(region) = self.region(source, path) else {
            return false;
        };
        let Some(array) = region.array else {
            return false;
        };
        let Some(StructuralTypeShape::Reference { referent, .. }) =
            crate::structural_shape(self.types.clone(), reference)
        else {
            return false;
        };
        if !crate::references::reference_views::plain_storage(
            self.types.clone(),
            array.declaration,
            &mut Vec::new(),
        ) || !crate::references::reference_views::plain_storage(
            self.types.clone(),
            *referent,
            &mut Vec::new(),
        ) {
            return false;
        }
        let (Ok((source_size, source_align)), Ok((target_size, target_align))) =
            (self.layout(source), self.layout(*referent))
        else {
            return false;
        };
        target_align != 0
            && region.alignment_at(source_align).is_some_and(|alignment| alignment.is_multiple_of(target_align))
            && region.byte_length == target_size
            && region
                .byte_offset
                .checked_add(region.byte_length)
                .is_some_and(|end| end <= source_size)
    }

    pub fn record_fields<'storage>(
        &'storage mut self,
        root: StructuralTypeId,
    ) -> Result<Vec<ReferenceRecordField<'storage>>, StorageGeometryError>
    where
        'types: 'storage,
    {
        let declaration = crate::structural_declaration(self.types.clone(), root)
            .ok_or(StorageGeometryError::MissingType(root))?;
        let StructuralTypeShape::Record { fields, storage } = &declaration.shape else {
            return Err(StorageGeometryError::UnsupportedType(root));
        };
        let selected = match storage {
            StructuralRecordStorage::TargetLayout(application) => {
                Some(self.records.realization(root, *application)?)
            }
            StructuralRecordStorage::Packed | StructuralRecordStorage::Authored(_) => None,
            _ => return Err(StorageGeometryError::UnsupportedType(root)),
        };
        let mut output = Vec::with_capacity(fields.len());
        let mut packed_offset = 0;
        for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
            let field_storage = match selected {
                Some(realization) => Some(
                    &realization
                        .fields
                        .iter()
                        .find(|row| row.field == field.id)
                        .ok_or(StorageGeometryError::MissingFieldStorage(root, field.id))?
                        .storage,
                ),
                None => field.storage.as_ref(),
            };
            let (byte_size, byte_alignment) = field_geometry(
                self.types.clone(),
                root,
                field,
                field_storage,
                &mut Vec::new(),
                self.carriers,
                &self.records,
                &mut |application| (self.extents)(application).map(Some),
            )?
            .ok_or(StorageGeometryError::UnresolvedLayout(root))?;
            let repeated = match field_storage {
                Some(AuthoredFieldStorage::Repeated { element_stride, .. }) => {
                    let array = field
                        .field_type
                        .as_structural()
                        .ok_or(StorageGeometryError::InvalidRecordStorage(root))?;
                    let StructuralTypeShape::FixedArray { element, length } =
                        &crate::structural_declaration(self.types.clone(), array)
                            .ok_or(StorageGeometryError::MissingType(array))?
                            .shape
                    else {
                        return Err(StorageGeometryError::InvalidRecordStorage(root));
                    };
                    let element_count = match length {
                        terminal_psi::ArrayExtent::Parameter { .. } => {
                            return Err(StorageGeometryError::UnresolvedLayout(array));
                        }
                        terminal_psi::ArrayExtent::Literal(count) => *count,
                        terminal_psi::ArrayExtent::Application(application) => {
                            (self.extents)(*application)?
                        }
                    };
                    let element_size = geometry(
                        self.types.clone(),
                        *element,
                        &mut Vec::new(),
                        self.carriers,
                        &self.records,
                        &mut |application| (self.extents)(application).map(Some),
                    )?
                    .ok_or(StorageGeometryError::UnresolvedLayout(*element))?
                    .0;
                    Some(ReferenceRepeatedField {
                        element_size,
                        element_count,
                        element_stride: *element_stride,
                    })
                }
                _ => None,
            };
            let byte_offset = match storage {
                StructuralRecordStorage::Packed => {
                    packed_offset = align(packed_offset, byte_alignment)
                        .ok_or(StorageGeometryError::Overflow)?;
                    let offset = packed_offset;
                    packed_offset = packed_offset
                        .checked_add(byte_size)
                        .ok_or(StorageGeometryError::Overflow)?;
                    Some(offset)
                }
                _ => field_storage
                    .ok_or(StorageGeometryError::MissingFieldStorage(root, field.id))?
                    .byte_offset(),
            };
            output.push(ReferenceRecordField {
                declaration: field,
                storage: field_storage,
                byte_offset,
                byte_size,
                byte_alignment,
                repeated,
            });
        }
        Ok(output)
    }
}

pub fn reference_storage_dependencies<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> Result<Vec<ScalarApplicationId>, StorageGeometryError> {
    ReferenceStorageGeometry::new(types, |application| {
        Err(StorageGeometryError::UnresolvedExtent(application))
    })
    .dependencies(root)
}

fn geometry<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    active: &mut Vec<StructuralTypeId>,
    carriers: Option<TargetCarrierLayout>,
    records: &impl RecordStorageResolver,
    extents: &mut impl FnMut(ScalarApplicationId) -> Result<Option<u64>, StorageGeometryError>,
) -> Result<Option<(u64, u64)>, StorageGeometryError> {
    if active.len() >= 128 {
        return Err(StorageGeometryError::NestingLimit);
    }
    if active.contains(&root) {
        return Err(StorageGeometryError::RecursiveType(root));
    }
    let shape = &crate::structural_declaration(types.clone(), root)
        .ok_or(StorageGeometryError::MissingType(root))?
        .shape;
    if matches!(
        shape,
        StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::ElementView { .. }
    ) && let Some(layout) =
        physical_carrier_geometry(records, root, &StructuralFieldType::Structural(root))?
    {
        return Ok(Some(layout));
    }
    active.push(root);
    let result = (|| match shape {
        StructuralTypeShape::Reference { .. } => {
            let carriers = carriers.ok_or(StorageGeometryError::MissingCarrierLayout)?;
            carrier_geometry(carriers.pointer_byte_size, carriers.pointer_byte_alignment)
                .map(Some)
        }
        StructuralTypeShape::ByteSequence(carrier) => {
            byte_sequence_geometry(*carrier, carriers).map(Some)
        }
        StructuralTypeShape::ElementView { .. } => {
            let carriers = carriers.ok_or(StorageGeometryError::MissingCarrierLayout)?;
            carrier_geometry(
                carriers.descriptor_byte_size,
                carriers.descriptor_byte_alignment,
            )
            .map(Some)
        }
        StructuralTypeShape::AtomicCell(cell) => scalar_geometry(cell.scalar_type())
            .map(Some)
            .ok_or(StorageGeometryError::UnsupportedScalar(cell.scalar_type())),
        StructuralTypeShape::PrimitiveScalar(scalar) => {
            primitive_scalar_geometry(*scalar, carriers).map(Some)
        }
        StructuralTypeShape::Record {
            storage: StructuralRecordStorage::TargetLayout(application),
            ..
        } => {
            let selected = records.realization(root, *application)?;
            Ok(Some((
                selected.storage.byte_size,
                selected.storage.byte_alignment,
            )))
        }
        StructuralTypeShape::Record {
            storage: StructuralRecordStorage::Authored(storage),
            ..
        } => Ok(Some((storage.byte_size, storage.byte_alignment))),
        StructuralTypeShape::Record {
            fields,
            storage: StructuralRecordStorage::Packed,
        } => {
            let mut size = Some(0);
            let mut alignment = 1;
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                let child = field_geometry(
                    types.clone(),
                    root,
                    field,
                    field.storage.as_ref(),
                    active,
                    carriers,
                    records,
                    extents,
                )?;
                let Some((child_size, child_alignment)) = child else {
                    size = None;
                    continue;
                };
                if let Some(current) = size {
                    size = Some(
                        align(current, child_alignment)
                            .and_then(|offset| offset.checked_add(child_size))
                            .ok_or(StorageGeometryError::Overflow)?,
                    );
                }
                alignment = alignment.max(child_alignment);
            }
            size.map(|size| {
                Ok((
                    align(size, alignment).ok_or(StorageGeometryError::Overflow)?,
                    alignment,
                ))
            })
            .transpose()
        }
        StructuralTypeShape::FixedArray { element, length } => {
            let length = match length {
                terminal_psi::ArrayExtent::Parameter { .. } => None,
                terminal_psi::ArrayExtent::Literal(length) => Some(*length),
                terminal_psi::ArrayExtent::Application(application) => extents(*application)?,
            };
            let element =
                geometry(types.clone(), *element, active, carriers, records, extents)?;
            let (Some(length), Some((size, alignment))) = (length, element) else {
                return Ok(None);
            };
            let size = align(size, alignment)
                .and_then(|stride| stride.checked_mul(length))
                .ok_or(StorageGeometryError::Overflow)?;
            Ok(Some((size, alignment)))
        }
        StructuralTypeShape::Sum { cases } => {
            sum_geometry(types, root, &[], cases, active, carriers, records, extents)
        }
        StructuralTypeShape::Mixed { fields, cases } => sum_geometry(
            types, root, fields, cases, active, carriers, records, extents,
        ),
        _ => Err(StorageGeometryError::UnsupportedType(root)),
    })();
    active.pop();
    result
}

fn physical_carrier_geometry(
    records: &impl RecordStorageResolver,
    owner: StructuralTypeId,
    carrier: &StructuralFieldType,
) -> Result<Option<(u64, u64)>, StorageGeometryError> {
    let Some((size, alignment)) = records.carrier_layout(owner, carrier)? else {
        return Ok(None);
    };
    // A native inline carrier can end before its trailing alignment (a
    // bounded byte value is one). The enclosing walk rounds placements;
    // requiring size to be an alignment multiple would invent padding.
    if size == 0 || !alignment.is_power_of_two() {
        return Err(StorageGeometryError::InvalidCarrierLayout);
    }
    Ok(Some((size, alignment)))
}

fn sum_geometry<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    owner: StructuralTypeId,
    common_fields: &[StructuralFieldDeclaration],
    cases: &[terminal_psi::StructuralCaseDeclaration],
    active: &mut Vec<StructuralTypeId>,
    carriers: Option<TargetCarrierLayout>,
    records: &impl RecordStorageResolver,
    extents: &mut impl FnMut(ScalarApplicationId) -> Result<Option<u64>, StorageGeometryError>,
) -> Result<Option<(u64, u64)>, StorageGeometryError> {
    Ok(sum_layout(
        types,
        owner,
        common_fields,
        cases,
        active,
        carriers,
        records,
        extents,
    )?
    .map(|geometry| (geometry.storage.byte_size, geometry.storage.byte_alignment)))
}

fn sum_layout<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    owner: StructuralTypeId,
    common_fields: &[StructuralFieldDeclaration],
    cases: &[terminal_psi::StructuralCaseDeclaration],
    active: &mut Vec<StructuralTypeId>,
    carriers: Option<TargetCarrierLayout>,
    records: &impl RecordStorageResolver,
    extents: &mut impl FnMut(ScalarApplicationId) -> Result<Option<u64>, StorageGeometryError>,
) -> Result<Option<layout_plans::ConventionalSumGeometry>, StorageGeometryError> {
    let mut fields = |fields: &[StructuralFieldDeclaration]| {
        fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .map(|field| {
                field_geometry(
                    types.clone(),
                    owner,
                    field,
                    field.storage.as_ref(),
                    active,
                    carriers,
                    records,
                    extents,
                )
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let common = fields(common_fields)?;
    let payloads = cases
        .iter()
        .map(|case| fields(&case.fields))
        .collect::<Result<Vec<_>, _>>()?;
    let Some(common) = common.into_iter().collect::<Option<Vec<_>>>() else {
        return Ok(None);
    };
    let Some(payloads) = payloads
        .into_iter()
        .map(|fields| fields.into_iter().collect::<Option<Vec<_>>>())
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };
    let carriers = carriers.ok_or(StorageGeometryError::MissingCarrierLayout)?;
    let tag = carrier_geometry(
        carriers.case_tag_byte_size,
        carriers.case_tag_byte_alignment,
    )?;
    let shape = |(byte_size, byte_alignment)| layout_plans::StorageGeometry {
        byte_size,
        byte_alignment,
    };
    let geometry = layout_plans::packing::conventional_sum_geometry(
        shape(tag),
        common.into_iter().map(shape),
        payloads
            .into_iter()
            .map(|fields| fields.into_iter().map(shape)),
    )
    .map_err(StorageGeometryError::AggregateLayout)?;
    Ok(Some(geometry))
}

fn carrier_geometry(size: u64, alignment: u64) -> Result<(u64, u64), StorageGeometryError> {
    if size == 0 || !alignment.is_power_of_two() || !size.is_multiple_of(alignment) {
        return Err(StorageGeometryError::InvalidCarrierLayout);
    }
    Ok((size, alignment))
}

fn byte_sequence_geometry(
    carrier: terminal_psi::ByteSequenceCarrier,
    carriers: Option<TargetCarrierLayout>,
) -> Result<(u64, u64), StorageGeometryError> {
    use terminal_psi::ByteSequenceCarrier;
    let carriers = carriers.ok_or(StorageGeometryError::MissingCarrierLayout)?;
    match carrier {
        ByteSequenceCarrier::BorrowedView { .. } => carrier_geometry(
            carriers.descriptor_byte_size,
            carriers.descriptor_byte_alignment,
        ),
        ByteSequenceCarrier::BoundedView { .. } => {
            carrier_geometry(carriers.pointer_byte_size, carriers.pointer_byte_alignment)
        }
        ByteSequenceCarrier::BoundedOwned { .. } => {
            Err(StorageGeometryError::UnsupportedByteSequence(carrier))
        }
    }
}

pub fn reference_storage_size<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> Option<u64> {
    reference_storage_layout(types, root).map(|(size, _)| size)
}

pub fn reference_storage_layout<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> Option<(u64, u64)> {
    ReferenceStorageGeometry::new(types, |application| {
        Err(StorageGeometryError::UnresolvedExtent(application))
    })
    .layout(root)
    .ok()
}

pub fn reference_storage_projection<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<ReferenceStorageProjection> {
    reference_storage_region(types, root, path)?.scalar
}

pub fn reference_storage_region<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<ReferenceStorageRegion> {
    crate::ReferenceStorageGeometry::new(types, |application| {
        Err(StorageGeometryError::UnresolvedExtent(application))
    })
    .region(root, path)
}

fn storage_region<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    records: &impl RecordStorageResolver,
    carriers: Option<TargetCarrierLayout>,
    extents: &mut impl FnMut(ScalarApplicationId) -> Result<u64, StorageGeometryError>,
) -> Option<ReferenceStorageRegion> {
    let projection = storage_path(types.clone(), root, path, records, carriers, extents)?;
    if projection.fragments.is_some() {
        return None;
    }
    if let Some(stride) = projection.repeated_stride {
        let StructuralTypeShape::FixedArray { element, .. } =
            crate::structural_shape(types.clone(), projection.carrier)?
        else {
            return None;
        };
        let (size, alignment) =
            selected_storage_layout(types.clone(), *element, records, carriers, extents)?;
        if stride != align(size, alignment)? {
            return None;
        }
    }
    let size = match (projection.window, projection.scalar) {
        (Some(window), _) => window.byte_length,
        (None, Some((_, stored))) => scalar_geometry(stored)?.0,
        (None, None) => {
            selected_storage_layout(types, projection.carrier, records, carriers, extents)?.0
        }
    };
    Some(ReferenceStorageRegion {
        byte_offset: projection.byte_offset,
        byte_length: size,
        scalar: projection.scalar.map(|(logical_scalar, stored_scalar)| {
            ReferenceStorageProjection {
                byte_offset: projection.byte_offset,
                logical_scalar,
                stored_scalar,
            }
        }),
        array: projection.window,
    })
}

fn selected_storage_layout<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    records: &impl RecordStorageResolver,
    carriers: Option<TargetCarrierLayout>,
    extents: &mut impl FnMut(ScalarApplicationId) -> Result<u64, StorageGeometryError>,
) -> Option<(u64, u64)> {
    geometry(
        types,
        root,
        &mut Vec::new(),
        carriers,
        records,
        &mut |application| extents(application).map(Some),
    )
    .ok()?
}

pub fn reference_scalar_storage<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<ReferenceScalarStorage<'types>> {
    let projection = storage_path(
        types,
        root,
        path,
        &UnselectedRecordStorage,
        None,
        &mut |application| Err(StorageGeometryError::UnresolvedExtent(application)),
    )?;
    let (logical_scalar, stored_scalar) = projection.scalar?;
    Some(match projection.fragments {
        Some(fragments) => ReferenceScalarStorage::Fragmented {
            byte_offset: projection.byte_offset,
            logical_scalar,
            fragments,
        },
        None => ReferenceScalarStorage::Contiguous(ReferenceStorageProjection {
            byte_offset: projection.byte_offset,
            logical_scalar,
            stored_scalar,
        }),
    })
}

struct StoragePathProjection<'recipe> {
    byte_offset: u64,
    carrier: StructuralTypeId,
    scalar: Option<(ScalarType, ScalarType)>,
    fragments: Option<&'recipe semantic_vocabulary::AuthoredBitFragments>,
    repeated_stride: Option<u64>,
    window: Option<ReferenceArrayWindow>,
}

fn storage_path<'types, 'storage>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    records: &'storage impl RecordStorageResolver,
    carriers: Option<TargetCarrierLayout>,
    extents: &mut impl FnMut(ScalarApplicationId) -> Result<u64, StorageGeometryError>,
) -> Option<StoragePathProjection<'storage>>
where
    'types: 'storage,
{
    let mut carrier = root;
    let mut byte_offset = 0_u64;
    let mut repeated_stride = None;
    let mut selected_storage = None;
    let mut selected_scalar = None;
    let mut fragments = None;
    let mut window: Option<ReferenceArrayWindow> = None;
    let mut segments = path.iter();
    while let Some(segment) = segments.next() {
        if let StructuralPathSegment::RepresentationView {
            structural_type,
            stored_scalar,
        } = segment
        {
            if fragments.is_some() {
                return None;
            }
            let source_size = if let Some(window) = window {
                window.byte_length
            } else {
                match selected_scalar {
                    Some(logical) => scalar_geometry(selected_storage.unwrap_or(logical))?.0,
                    None => match *crate::structural_shape(types.clone(), carrier)? {
                        StructuralTypeShape::PrimitiveScalar(logical) => {
                            scalar_geometry(selected_storage.unwrap_or(logical))?.0
                        }
                        _ => {
                            selected_storage_layout(
                                types.clone(),
                                carrier,
                                records,
                                carriers,
                                extents,
                            )?
                            .0
                        }
                    },
                }
            };
            let view = crate::representation_view_declaration(
                types.clone(),
                *structural_type,
                *stored_scalar,
            )?;
            let view_size = match stored_scalar {
                Some(scalar) => scalar_geometry(*scalar)?.0,
                None => {
                    selected_storage_layout(types.clone(), view.id, records, carriers, extents)?
                        .0
                }
            };
            if source_size != view_size {
                return None;
            }
            carrier = view.id;
            selected_storage = *stored_scalar;
            selected_scalar = None;
            repeated_stride = None;
            window = None;
            continue;
        }
        if window.is_some() {
            return None;
        }
        if selected_scalar.is_some() {
            return None;
        }
        let shape = &crate::structural_shape(types.clone(), carrier)?;
        match (shape, segment) {
            (
                StructuralTypeShape::Sum { .. } | StructuralTypeShape::Mixed { .. },
                StructuralPathSegment::Field(_) | StructuralPathSegment::Case(_),
            ) => {
                let (common, cases) = match shape {
                    StructuralTypeShape::Sum { cases } => (&[][..], cases),
                    StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases),
                    _ => return None,
                };
                let layout = sum_layout(
                    types.clone(),
                    carrier,
                    common,
                    cases,
                    &mut vec![carrier],
                    carriers,
                    records,
                    &mut |application| extents(application).map(Some),
                )
                .ok()??;
                let (field, offset) = match segment {
                    StructuralPathSegment::Field(identity) => {
                        let (ordinal, field) =
                            terminal_psi::live_field_entry(common, *identity)?;
                        (field, *layout.common_field_offsets.get(ordinal)?)
                    }
                    StructuralPathSegment::Case(case) => {
                        let (ordinal, selected) =
                            cases.iter().enumerate().find(|(_, row)| row.id == *case)?;
                        let StructuralPathSegment::Field(identity) = segments.next()? else {
                            return None;
                        };
                        let (field_ordinal, field) =
                            terminal_psi::live_field_entry(&selected.fields, *identity)?;
                        (
                            field,
                            *layout.case_field_offsets.get(ordinal)?.get(field_ordinal)?,
                        )
                    }
                    _ => return None,
                };
                let recipe = field.storage.as_ref();
                if matches!(recipe, Some(AuthoredFieldStorage::Fragmented(_))) {
                    return None;
                }
                byte_offset = byte_offset.checked_add(offset)?;
                selected_storage = recipe.and_then(AuthoredFieldStorage::stored_scalar);
                repeated_stride = recipe.and_then(AuthoredFieldStorage::element_stride);
                if let Some(logical_scalar) = field.field_type.scalar_type() {
                    selected_scalar = Some(logical_scalar);
                    continue;
                }
                carrier = field.field_type.as_structural()?;
            }
            (
                StructuralTypeShape::Record { fields, storage },
                StructuralPathSegment::Field(identity),
            ) => {
                let field = terminal_psi::live_field_by_identity(fields, *identity)?;
                let recipe = match storage {
                    StructuralRecordStorage::TargetLayout(application) => Some(
                        &records
                            .realization(carrier, *application)
                            .ok()?
                            .fields
                            .iter()
                            .find(|row| row.field == field.id)?
                            .storage,
                    ),
                    _ => field.storage.as_ref(),
                };
                let offset = match storage {
                    terminal_psi::StructuralRecordStorage::AccessView(_)
                    | terminal_psi::StructuralRecordStorage::Accessor(_) => return None,
                    StructuralRecordStorage::TargetLayout(_)
                    | StructuralRecordStorage::Authored(_) => match recipe? {
                        AuthoredFieldStorage::Fragmented(recipe) => {
                            fragments = Some(recipe);
                            0
                        }
                        storage => storage.byte_offset()?,
                    },
                    StructuralRecordStorage::Packed => {
                        let mut offset = 0;
                        for candidate in
                            fields.iter().filter(|field| !field.relevance.is_erased())
                        {
                            let (size, alignment) = field_geometry(
                                types.clone(),
                                carrier,
                                candidate,
                                candidate.storage.as_ref(),
                                &mut Vec::new(),
                                carriers,
                                records,
                                &mut |application| extents(application).map(Some),
                            )
                            .ok()??;
                            offset = align(offset, alignment)?;
                            if candidate.id == field.id {
                                break;
                            }
                            offset = offset.checked_add(size)?;
                        }
                        offset
                    }
                };
                byte_offset = byte_offset.checked_add(offset)?;
                selected_storage = recipe.and_then(AuthoredFieldStorage::stored_scalar);
                repeated_stride = recipe.and_then(AuthoredFieldStorage::element_stride);
                if let Some(logical_scalar) = field.field_type.scalar_type() {
                    selected_scalar = Some(logical_scalar);
                    continue;
                }
                let child = field.field_type.as_structural()?;
                carrier = child;
            }
            (
                StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                },
                StructuralPathSegment::FixedIndex(index),
            ) if index < length => {
                let (size, alignment) = selected_storage_layout(
                    types.clone(),
                    *element,
                    records,
                    carriers,
                    extents,
                )?;
                let stride = repeated_stride.take().unwrap_or(align(size, alignment)?);
                byte_offset = byte_offset.checked_add(index.checked_mul(stride)?)?;
                carrier = *element;
                selected_storage = None;
            }
            (
                StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                },
                StructuralPathSegment::FixedRange { start, end },
            ) if start <= end && end <= length => {
                let (size, alignment) = selected_storage_layout(
                    types.clone(),
                    *element,
                    records,
                    carriers,
                    extents,
                )?;
                let stride = repeated_stride.take().unwrap_or(align(size, alignment)?);
                if stride < size {
                    return None;
                }
                let array_offset = start.checked_mul(stride)?;
                byte_offset = byte_offset.checked_add(array_offset)?;
                let count = end.checked_sub(*start)?;
                let byte_length = if count == 0 {
                    0
                } else {
                    count
                        .checked_sub(1)?
                        .checked_mul(stride)?
                        .checked_add(size)?
                };
                window = Some(ReferenceArrayWindow {
                    declaration: carrier,
                    byte_offset: array_offset,
                    byte_length,
                });
                selected_storage = None;
            }
            _ => return None,
        }
    }
    let scalar = match selected_scalar {
        Some(logical) => Some((logical, selected_storage.unwrap_or(logical))),
        None => match *crate::structural_shape(types.clone(), carrier)? {
            StructuralTypeShape::PrimitiveScalar(logical) => {
                Some((logical, selected_storage.unwrap_or(logical)))
            }
            _ => None,
        },
    };
    Some(StoragePathProjection {
        byte_offset,
        carrier,
        scalar,
        fragments,
        repeated_stride,
        window,
    })
}

#[cfg(test)]
mod tests;
