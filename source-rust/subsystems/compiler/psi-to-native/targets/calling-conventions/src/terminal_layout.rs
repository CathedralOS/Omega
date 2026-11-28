/*
We compute native storage geometry from canonical Terminal declarations.
`StructuralLayoutResolver` resolves demanded roots once within a caller-owned
query context; `StructuralLayouts` materializes the same computation for stages
that need the complete table. ABI classification and artifact checking use this
owner, not independent approximations of authored storage or array stride.
Only realized shapes are shared across roots so a cycle or unclosed extent
reports the first error reached from its queried root.
*/

use crate::ValueShape;
use arena::{IdTable, NonDenseIdentities};
use semantic_vocabulary::{
    ClosedIntegerType, ClosedScalarType, IeeeFloatFormat, ScalarApplicationId, ScalarType,
    StructuralTypeId,
};
use std::cell::RefCell;
use std::collections::BTreeSet;
use terminal_psi::{StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalLayoutError {
    RecursiveStructuralType(StructuralTypeId),
    UnknownStructuralType(StructuralTypeId),
    UnclosedTargetLayout(StructuralTypeId),
    UnclosedOpaqueRepresentation(StructuralTypeId),
    UnclosedArrayExtent(StructuralTypeId),
    UnclosedOwnedSequence(StructuralTypeId),
    StructuralTypeTooLarge(StructuralTypeId),
    EmptyStructuralType(StructuralTypeId),
    UnsupportedStructuralSum(StructuralTypeId),
    InvalidStorageGeometry(StructuralTypeId),
    ReentrantLayoutQuery(StructuralTypeId),
    UnclosedScalarType(ScalarType),
}

/// Borrows a declaration catalog together with its invocation-owned native
/// layout queries. Consumers retain this context across related projections.
pub struct NativeGeometry<'a> {
    declarations: &'a [StructuralTypeDeclaration],
    pub(super) resolver: StructuralLayoutResolver<'a>,
}

impl<'a> NativeGeometry<'a> {
    pub fn new(declarations: &'a [StructuralTypeDeclaration]) -> Result<Self, NonDenseIdentities> {
        Self::with_storage_inputs(declarations, StorageLayoutInputs::default())
    }

    pub fn with_storage_inputs(
        declarations: &'a [StructuralTypeDeclaration],
        inputs: StorageLayoutInputs<'a>,
    ) -> Result<Self, NonDenseIdentities> {
        Ok(Self {
            declarations,
            resolver: StructuralLayoutResolver::with_storage_inputs(declarations, inputs)?,
        })
    }

    pub fn owned_sequence(
        &self,
        root: StructuralTypeId,
    ) -> Result<OwnedSequenceLayout, TerminalLayoutError> {
        let StructuralTypeShape::ElementView {
            element,
            access: None,
        } = self.declaration(root)?.shape
        else {
            return Err(TerminalLayoutError::InvalidStorageGeometry(root));
        };
        owned_sequence_layout(
            root,
            self.shape(element)?,
            sequence_capacity(self.resolver.inputs.owned_sequences, root)?,
        )
    }

    pub fn shape(&self, root: StructuralTypeId) -> Result<crate::ValueShape, TerminalLayoutError> {
        self.resolver.shape(root)
    }

    pub fn formal_storage_shape(
        &self,
        root: StructuralTypeId,
        access: terminal_psi::StructuralAccess,
    ) -> Result<ValueShape, TerminalLayoutError> {
        formal_storage_shape(&self.resolver.declarations, root, access, || {
            self.shape(root)
        })
    }

    pub fn storage_inputs(&self) -> StorageLayoutInputs<'a> {
        self.resolver.inputs
    }

    /// The public field must retain a codec for a scalar witness. A direct
    /// aggregate reference occupies its first word and leaves that codec zero.
    pub fn package_reference_field(
        &self,
        public: &StructuralFieldType,
        actual: &StructuralFieldType,
    ) -> Option<bool> {
        let StructuralTypeShape::Reference {
            referent, access, ..
        } = self.declaration(public.as_structural()?).ok()?.shape
        else {
            return None;
        };
        if !self.storage_inputs().is_package_witness(referent) {
            return None;
        }
        Some(
            actual
                .as_structural()
                .and_then(|id| self.declaration(id).ok())
                .is_some_and(|declaration| {
                    matches!(declaration.shape,
                StructuralTypeShape::Reference { access: supplied, .. } if supplied == access)
                }),
        )
    }

    pub fn owned_sequence_bounds(&self) -> &'a [(StructuralTypeId, u64)] {
        self.resolver.inputs.owned_sequences
    }

    pub fn declaration(
        &self,
        root: StructuralTypeId,
    ) -> Result<&'a StructuralTypeDeclaration, TerminalLayoutError> {
        self.resolver.declaration(root)
    }

    pub fn field_shape(
        &self,
        field: &StructuralFieldType,
        owner: StructuralTypeId,
    ) -> Result<crate::ValueShape, TerminalLayoutError> {
        self.resolver.field_shape(field, owner)
    }

    pub fn sum_layout(
        &self,
        root: StructuralTypeId,
    ) -> Result<crate::ConventionalSumLayout, TerminalLayoutError> {
        self.resolver.sum_layout(root)
    }

    pub fn field_placement(
        &self,
        field: &terminal_psi::StructuralFieldDeclaration,
        owner: StructuralTypeId,
        conventional_offset: u32,
    ) -> Result<crate::terminal_layout::NativeFieldPlacement, TerminalLayoutError> {
        self.resolver
            .field_placement(field, owner, conventional_offset)
    }

    pub fn field_storage_shape(
        &self,
        field: &terminal_psi::StructuralFieldDeclaration,
        owner: StructuralTypeId,
    ) -> Result<crate::ValueShape, TerminalLayoutError> {
        self.resolver.field_storage_shape(field, owner)
    }
}

impl std::ops::Deref for NativeGeometry<'_> {
    type Target = [StructuralTypeDeclaration];

    fn deref(&self) -> &Self::Target {
        self.declarations
    }
}

pub use structural_storage::structural_type_catalog::StorageLayoutInputs;

type Declarations<'a> = IdTable<StructuralTypeId, &'a StructuralTypeDeclaration>;
type KnownShapes = IdTable<StructuralTypeId, Option<ValueShape>>;

/// Recomputes native geometry from declarations, resolving only demanded roots.
/// Retain one resolver across related queries; it never consumes a producer's
/// claimed layout result as evidence.
pub struct StructuralLayoutResolver<'a> {
    declarations: Declarations<'a>,
    inputs: StorageLayoutInputs<'a>,
    scratch: RefCell<LayoutScratch>,
}

struct LayoutScratch {
    known: KnownShapes,
    active: IdTable<StructuralTypeId, bool>,
}

impl<'a> StructuralLayoutResolver<'a> {
    pub fn new(
        rows: impl IntoIterator<Item = &'a StructuralTypeDeclaration>,
    ) -> Result<Self, NonDenseIdentities> {
        Self::with_storage_inputs(rows, StorageLayoutInputs::default())
    }

    pub fn with_storage_inputs(
        rows: impl IntoIterator<Item = &'a StructuralTypeDeclaration>,
        inputs: StorageLayoutInputs<'a>,
    ) -> Result<Self, NonDenseIdentities> {
        let declarations = IdTable::from_rows(rows.into_iter().map(|row| (row.id, row)))?;
        let mut known = declarations.map(|_, _| None);
        for home in inputs.execution_owners {
            if let Some(declaration) = declarations.get(home.ty())
                && matches!(declaration.shape, StructuralTypeShape::BorrowingOpaque { lifetime_count } if lifetime_count.get() == 1)
                && declaration.owns_linear_claim
                && declaration.multiplicity == terminal_psi::StructuralMultiplicity::Linear
                && let Some(slot) = known.get_mut(home.ty())
            {
                *slot = Some(ValueShape::integer(home.bytes(), home.alignment()));
            }
        }
        let active = declarations.map(|_, _| false);
        Ok(Self {
            declarations,
            inputs,
            scratch: RefCell::new(LayoutScratch { known, active }),
        })
    }

    pub fn declaration(
        &self,
        structural_type: StructuralTypeId,
    ) -> Result<&'a StructuralTypeDeclaration, TerminalLayoutError> {
        self.declarations
            .get(structural_type)
            .copied()
            .ok_or(TerminalLayoutError::UnknownStructuralType(structural_type))
    }

    pub fn shape(
        &self,
        structural_type: StructuralTypeId,
    ) -> Result<ValueShape, TerminalLayoutError> {
        let mut scratch = self
            .scratch
            .try_borrow_mut()
            .map_err(|_| TerminalLayoutError::ReentrantLayoutQuery(structural_type))?;
        let LayoutScratch { known, active } = &mut *scratch;
        resolve_structural_shape(
            structural_type,
            &self.declarations,
            self.inputs,
            known,
            active,
            &mut reject_unclosed_extent,
        )?
        .ok_or(TerminalLayoutError::UnclosedArrayExtent(structural_type))
    }

    pub fn field_shape(
        &self,
        field: &StructuralFieldType,
        owner: StructuralTypeId,
    ) -> Result<ValueShape, TerminalLayoutError> {
        match field {
            StructuralFieldType::Structural(nested) => self.shape(*nested),
            _ => scalar_field_shape(field, owner),
        }
    }

    /// Locates one contiguous field using its authored recipe when present.
    /// Fragmented fields need their transfer recipe, not a fabricated addressable
    /// scalar. Repeated fields retain their explicit stride for element traversal.
    pub fn field_placement(
        &self,
        field: &terminal_psi::StructuralFieldDeclaration,
        owner: StructuralTypeId,
        conventional_offset: u32,
    ) -> Result<NativeFieldPlacement, TerminalLayoutError> {
        let invalid = || TerminalLayoutError::InvalidStorageGeometry(owner);
        let (shape, byte_offset, element_stride) = match &field.storage {
            Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(_)) => return Err(invalid()),
            Some(storage) => {
                let byte_offset = storage
                    .byte_offset()
                    .and_then(|offset| u32::try_from(offset).ok())
                    .ok_or_else(invalid)?;
                let element_stride = storage
                    .element_stride()
                    .map(u32::try_from)
                    .transpose()
                    .map_err(|_| invalid())?;
                let shape = self.field_storage_shape(field, owner)?;
                (shape, byte_offset, element_stride)
            }
            None => {
                let shape = self.field_shape(&field.field_type, owner)?;
                let byte_offset =
                    checked_align_up_u32(conventional_offset, u32::from(shape.alignment))
                        .ok_or_else(invalid)?;
                (shape, byte_offset, None)
            }
        };
        let owner_shape = self.shape(owner)?;
        if byte_offset
            .checked_add(shape.byte_size)
            .is_none_or(|end| end > owner_shape.byte_size)
        {
            return Err(invalid());
        }
        Ok(NativeFieldPlacement {
            shape,
            byte_offset,
            element_stride,
        })
    }

    /// Computes a field's physical footprint without discarding an encoded
    /// scalar width or an authored array stride. Its location is checked by
    /// `field_placement` when the caller projects it within an owner.
    pub fn field_storage_shape(
        &self,
        field: &terminal_psi::StructuralFieldDeclaration,
        owner: StructuralTypeId,
    ) -> Result<ValueShape, TerminalLayoutError> {
        let invalid = || TerminalLayoutError::InvalidStorageGeometry(owner);
        let Some(storage) = &field.storage else {
            return self.field_shape(&field.field_type, owner);
        };
        if matches!(
            storage,
            semantic_vocabulary::AuthoredFieldStorage::Fragmented(_)
        ) {
            return Err(invalid());
        }
        if let Some(scalar) = storage.stored_scalar() {
            return Ok(ValueShape {
                alignment: 1,
                ..scalar_shape(scalar)?
            });
        }
        let Some(stride) = storage.element_stride() else {
            return self.field_shape(&field.field_type, owner);
        };
        let stride = u32::try_from(stride).map_err(|_| invalid())?;
        let nested = field.field_type.as_structural().ok_or_else(invalid)?;
        let StructuralTypeShape::FixedArray { element, length } = self.declaration(nested)?.shape
        else {
            return Err(invalid());
        };
        let length = length.literal().ok_or_else(invalid)?;
        let element = self.shape(element)?;
        if stride < element.byte_size || stride % u32::from(element.alignment) != 0 {
            return Err(invalid());
        }
        let bytes = u64::from(stride)
            .checked_mul(length)
            .and_then(|bytes| u32::try_from(bytes).ok())
            .ok_or_else(invalid)?;
        Ok(ValueShape::integer(
            bytes,
            if length == 0 { 1 } else { element.alignment },
        ))
    }

    pub fn sum_layout(
        &self,
        structural_type: StructuralTypeId,
    ) -> Result<crate::ConventionalSumLayout, TerminalLayoutError> {
        let declaration = self.declaration(structural_type)?;
        let (common, cases) = match &declaration.shape {
            StructuralTypeShape::Sum { cases } => (&[][..], cases),
            StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases),
            _ => {
                return Err(TerminalLayoutError::UnsupportedStructuralSum(
                    structural_type,
                ));
            }
        };
        conventional_sum_layout_from_parts(structural_type, common, cases, &mut |field| {
            self.field_shape(field, structural_type).map(Some)
        })?
        .ok_or(TerminalLayoutError::UnclosedArrayExtent(structural_type))
    }

    fn materialize(self) -> StructuralLayouts<'a> {
        let Self {
            declarations,
            inputs,
            scratch,
        } = self;
        let LayoutScratch {
            mut known,
            mut active,
        } = scratch.into_inner();
        let shapes = declarations.map(|structural_type, _| {
            resolve_structural_shape(
                structural_type,
                &declarations,
                inputs,
                &mut known,
                &mut active,
                &mut reject_unclosed_extent,
            )?
            .ok_or(TerminalLayoutError::UnclosedArrayExtent(structural_type))
        });
        StructuralLayouts {
            declarations,
            shapes,
            inputs,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NativeFieldPlacement {
    pub shape: ValueShape,
    pub byte_offset: u32,
    pub element_stride: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct StructuralLayouts<'a> {
    declarations: Declarations<'a>,
    shapes: IdTable<StructuralTypeId, Result<ValueShape, TerminalLayoutError>>,
    inputs: StorageLayoutInputs<'a>,
}

impl<'a> StructuralLayouts<'a> {
    pub fn new(
        rows: impl IntoIterator<Item = &'a StructuralTypeDeclaration>,
    ) -> Result<Self, NonDenseIdentities> {
        Ok(StructuralLayoutResolver::new(rows)?.materialize())
    }

    pub fn with_storage_inputs(
        rows: impl IntoIterator<Item = &'a StructuralTypeDeclaration>,
        inputs: StorageLayoutInputs<'a>,
    ) -> Result<Self, NonDenseIdentities> {
        Ok(StructuralLayoutResolver::with_storage_inputs(rows, inputs)?.materialize())
    }

    pub fn owned_sequence(
        &self,
        root: StructuralTypeId,
    ) -> Result<OwnedSequenceLayout, TerminalLayoutError> {
        let StructuralTypeShape::ElementView {
            element,
            access: None,
        } = self
            .declarations
            .get(root)
            .ok_or(TerminalLayoutError::UnknownStructuralType(root))?
            .shape
        else {
            return Err(TerminalLayoutError::InvalidStorageGeometry(root));
        };
        owned_sequence_layout(
            root,
            self.shape(element)?,
            sequence_capacity(self.inputs.owned_sequences, root)?,
        )
    }

    pub fn shape(
        &self,
        structural_type: StructuralTypeId,
    ) -> Result<ValueShape, TerminalLayoutError> {
        self.shapes.get(structural_type).copied().unwrap_or(Err(
            TerminalLayoutError::UnknownStructuralType(structural_type),
        ))
    }

    pub fn formal_storage_shape(
        &self,
        root: StructuralTypeId,
        access: terminal_psi::StructuralAccess,
    ) -> Result<ValueShape, TerminalLayoutError> {
        formal_storage_shape(&self.declarations, root, access, || self.shape(root))
    }

    pub fn field_shape(
        &self,
        field: &StructuralFieldType,
        owner: StructuralTypeId,
    ) -> Result<ValueShape, TerminalLayoutError> {
        match field {
            StructuralFieldType::Structural(nested) => self.shape(*nested),
            _ => scalar_field_shape(field, owner),
        }
    }

    pub fn sum_layout(
        &self,
        structural_type: StructuralTypeId,
    ) -> Result<crate::ConventionalSumLayout, TerminalLayoutError> {
        let declaration = self
            .declarations
            .get(structural_type)
            .ok_or(TerminalLayoutError::UnknownStructuralType(structural_type))?;
        let (common_fields, cases) = match &declaration.shape {
            StructuralTypeShape::Sum { cases } => (&[][..], cases),
            StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases),
            _ => {
                return Err(TerminalLayoutError::UnsupportedStructuralSum(
                    structural_type,
                ));
            }
        };
        conventional_sum_layout_from_parts(structural_type, common_fields, cases, &mut |field| {
            self.field_shape(field, structural_type).map(Some)
        })?
        .ok_or(TerminalLayoutError::UnclosedArrayExtent(structural_type))
    }

    pub fn scalar_storage_type(&self, structural_type: StructuralTypeId) -> Option<ScalarType> {
        scalar_storage_type(structural_type, &self.declarations)
    }

    pub fn required_scalar_extents(
        &self,
        roots: impl IntoIterator<Item = StructuralTypeId>,
    ) -> Result<BTreeSet<ScalarApplicationId>, TerminalLayoutError> {
        let mut known = self.shapes.map(|_, shape| shape.ok());
        let mut active = self.declarations.map(|_, _| false);
        let mut required = BTreeSet::new();
        for root in roots {
            resolve_structural_shape(
                root,
                &self.declarations,
                self.inputs,
                &mut known,
                &mut active,
                &mut |_, application| {
                    required.insert(application);
                    Ok(None)
                },
            )?;
        }
        Ok(required)
    }
}

impl<'a> std::ops::Deref for StructuralLayouts<'a> {
    type Target = Declarations<'a>;

    fn deref(&self) -> &Self::Target {
        &self.declarations
    }
}

/// Owned sequences are relocatable values: a live count followed by bounded
/// inline elements. Borrowed views retain their pointer/count presentation.
/// Moving an owned value therefore never copies a pointer to its former frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnedSequenceLayout {
    pub capacity: u64,
    pub elements_offset: u32,
    pub stride: u32,
    pub shape: ValueShape,
}

fn formal_storage_shape(
    declarations: &Declarations<'_>,
    root: StructuralTypeId,
    access: terminal_psi::StructuralAccess,
    stored: impl FnOnce() -> Result<ValueShape, TerminalLayoutError>,
) -> Result<ValueShape, TerminalLayoutError> {
    let declaration = declarations
        .get(root)
        .ok_or(TerminalLayoutError::UnknownStructuralType(root))?;
    if let Some(pointer) = opaque_borrow_transport(&declaration.shape, access) {
        return Ok(pointer);
    }
    if let Some(element) = borrowed_slice_element(&declaration.shape, access) {
        declarations
            .get(element)
            .ok_or(TerminalLayoutError::UnknownStructuralType(element))?;
        Ok(ValueShape::integer(16, 8))
    } else {
        stored()
    }
}

/// The declaration of `[T]` can occur both as owned inline storage and as a
/// borrowed formal transporting a pointer and a count. Only the occurrence
/// supplies the permission that distinguishes their native representations.
pub fn borrowed_slice_element(
    shape: &StructuralTypeShape,
    access: terminal_psi::StructuralAccess,
) -> Option<StructuralTypeId> {
    match shape {
        StructuralTypeShape::ElementView {
            element,
            access: None,
        } if access != terminal_psi::StructuralAccess::Owned => Some(*element),
        _ => None,
    }
}

/// The first subscript of a borrowed typed slice is relative to the pointer
/// in descriptor word zero. Further projections use the element's own storage.
pub struct BorrowedElement<'path> {
    pub remaining: &'path [terminal_psi::StructuralPathSegment],
    pub fixed_offset: u32,
    pub selector: Option<(
        semantic_vocabulary::ValueId,
        semantic_vocabulary::ObligationId,
        u32,
    )>,
}

pub fn borrowed_element_index<'path>(
    shape: ValueShape,
    path: &'path [terminal_psi::StructuralPathSegment],
) -> Option<BorrowedElement<'path>> {
    let (index, remaining) = path.split_first()?;
    let stride = checked_align_up_u32(shape.byte_size, u32::from(shape.alignment))?;
    let (fixed_offset, selector) = match index {
        terminal_psi::StructuralPathSegment::FixedIndex(index) => (
            u32::try_from(index.checked_mul(u64::from(stride))?).ok()?,
            None,
        ),
        terminal_psi::StructuralPathSegment::RuntimeIndex { index, obligation } => {
            (0, Some((*index, *obligation, stride)))
        }
        _ => return None,
    };
    Some(BorrowedElement {
        remaining,
        fixed_offset,
        selector,
    })
}

pub fn element_view_length_offset(shape: &StructuralTypeShape) -> Option<u32> {
    element_view_length_at(shape, None)
}

pub fn element_view_length_at(
    shape: &StructuralTypeShape,
    formal_access: Option<terminal_psi::StructuralAccess>,
) -> Option<u32> {
    if formal_access
        .and_then(|access| borrowed_slice_element(shape, access))
        .is_some()
    {
        return Some(8);
    }
    match shape {
        StructuralTypeShape::ElementView { access: None, .. } => Some(0),
        StructuralTypeShape::ElementView {
            access: Some(_), ..
        } => Some(8),
        _ => None,
    }
}

/// Borrowing a boundary-opaque value transports the address alone. The shape
/// describes the pointer word and supplies no size or alignment for its pointee.
pub fn opaque_borrow_transport(
    shape: &StructuralTypeShape,
    access: terminal_psi::StructuralAccess,
) -> Option<ValueShape> {
    (access != terminal_psi::StructuralAccess::Owned
        && matches!(shape, StructuralTypeShape::Opaque | StructuralTypeShape::BorrowingOpaque { .. }))
    .then_some(ValueShape::integer(8, 8))
}

/// Psi's owned sequence is a logical descriptor, but its native storage is an
/// aggregate carrying the elements. Only actual view descriptors use the
/// descriptor-specific native return and forwarding paths.
pub fn is_native_view_descriptor(shape: &StructuralTypeShape) -> bool {
    shape.is_view_descriptor()
        && !matches!(shape, StructuralTypeShape::ElementView { access: None, .. })
}

/// A descriptor copy must distinguish a borrowed formal from inline owned storage,
/// even when both occurrences have the same logical element sequence type.
pub fn is_native_view_descriptor_at(
    shape: &StructuralTypeShape,
    formal_access: Option<terminal_psi::StructuralAccess>,
) -> bool {
    is_native_view_descriptor(shape)
        || formal_access.is_some_and(|access| borrowed_slice_element(shape, access).is_some())
}

fn sequence_capacity(
    bounds: &[(StructuralTypeId, u64)],
    owner: StructuralTypeId,
) -> Result<u64, TerminalLayoutError> {
    let at = bounds
        .binary_search_by_key(&owner, |(identity, _)| *identity)
        .map_err(|_| TerminalLayoutError::UnclosedOwnedSequence(owner))?;
    if at > 0 && bounds[at - 1].0 == owner || bounds.get(at + 1).is_some_and(|next| next.0 == owner)
    {
        return Err(TerminalLayoutError::InvalidStorageGeometry(owner));
    }
    Ok(bounds[at].1)
}

fn owned_sequence_layout(
    owner: StructuralTypeId,
    element: ValueShape,
    capacity: u64,
) -> Result<OwnedSequenceLayout, TerminalLayoutError> {
    let invalid = || TerminalLayoutError::StructuralTypeTooLarge(owner);
    let alignment = element.alignment.max(8);
    let elements_offset =
        checked_align_up_u32(8, u32::from(element.alignment)).ok_or_else(invalid)?;
    let stride = checked_align_up_u32(element.byte_size, u32::from(element.alignment))
        .ok_or_else(invalid)?;
    let byte_size = u64::from(stride)
        .checked_mul(capacity)
        .and_then(|bytes| bytes.checked_add(u64::from(elements_offset)))
        .and_then(|bytes| u32::try_from(bytes).ok())
        .and_then(|bytes| checked_align_up_u32(bytes, u32::from(alignment)))
        .ok_or_else(invalid)?;
    Ok(OwnedSequenceLayout {
        capacity,
        elements_offset,
        stride,
        shape: ValueShape::integer(byte_size, alignment),
    })
}

fn reject_unclosed_extent(
    owner: StructuralTypeId,
    _: ScalarApplicationId,
) -> Result<Option<u64>, TerminalLayoutError> {
    Err(TerminalLayoutError::UnclosedArrayExtent(owner))
}

fn resolve_structural_shape(
    structural_type: StructuralTypeId,
    declarations: &Declarations<'_>,
    inputs: StorageLayoutInputs<'_>,
    known: &mut KnownShapes,
    active: &mut IdTable<StructuralTypeId, bool>,
    extent: &mut impl FnMut(
        StructuralTypeId,
        ScalarApplicationId,
    ) -> Result<Option<u64>, TerminalLayoutError>,
) -> Result<Option<ValueShape>, TerminalLayoutError> {
    if let Some(Some(shape)) = known.get(structural_type) {
        return Ok(Some(*shape));
    }
    let visiting = active
        .get_mut(structural_type)
        .ok_or(TerminalLayoutError::UnknownStructuralType(structural_type))?;
    if *visiting {
        return Err(TerminalLayoutError::RecursiveStructuralType(
            structural_type,
        ));
    }
    *visiting = true;
    let result = (|| {
        let declaration = declarations
            .get(structural_type)
            .copied()
            .ok_or(TerminalLayoutError::UnknownStructuralType(structural_type))?;
        match &declaration.shape {
            StructuralTypeShape::TypeParameter => {
                Err(TerminalLayoutError::UnclosedTargetLayout(structural_type))
            }
            StructuralTypeShape::Opaque | StructuralTypeShape::BorrowingOpaque { .. } => Err(
                TerminalLayoutError::UnclosedOpaqueRepresentation(structural_type),
            ),
            StructuralTypeShape::Reference { referent, .. } => {
                declarations
                    .get(*referent)
                    .ok_or(TerminalLayoutError::UnknownStructuralType(*referent))?;
                Ok(Some(ValueShape::integer(
                    if inputs.is_package_witness(*referent)
                        || scalar_storage_type(*referent, declarations).is_some()
                    {
                        runtime_abi::ScalarReferenceLayout::NATIVE.byte_size
                    } else {
                        8
                    },
                    8,
                )))
            }
            StructuralTypeShape::PrimitiveScalar(scalar) => scalar_shape(*scalar).map(Some),
            StructuralTypeShape::AtomicCell(cell) => scalar_shape(cell.scalar_type()).map(Some),
            StructuralTypeShape::ByteSequence(carrier) => {
                byte_sequence_shape(*carrier, structural_type).map(Some)
            }
            StructuralTypeShape::ElementView {
                element,
                access: None,
            } => {
                let capacity = sequence_capacity(inputs.owned_sequences, structural_type)?;
                let Some(element) = resolve_structural_shape(
                    *element,
                    declarations,
                    inputs,
                    known,
                    active,
                    extent,
                )?
                else {
                    return Ok(None);
                };
                owned_sequence_layout(structural_type, element, capacity)
                    .map(|layout| Some(layout.shape))
            }
            StructuralTypeShape::ElementView {
                element,
                access: Some(_),
            } => {
                declarations
                    .get(*element)
                    .ok_or(TerminalLayoutError::UnknownStructuralType(*element))?;
                Ok(Some(ValueShape::integer(16, 8)))
            }
            StructuralTypeShape::Record { fields, storage } => {
                if matches!(
                    storage,
                    terminal_psi::StructuralRecordStorage::TargetLayout(_)
                ) {
                    return Err(TerminalLayoutError::UnclosedTargetLayout(structural_type));
                }
                if let terminal_psi::StructuralRecordStorage::Authored(storage)
                | terminal_psi::StructuralRecordStorage::AccessView(storage)
                | terminal_psi::StructuralRecordStorage::Accessor(storage) = storage
                {
                    if !storage.byte_alignment.is_power_of_two() {
                        return Err(TerminalLayoutError::InvalidStorageGeometry(structural_type));
                    }
                    return Ok(Some(ValueShape::integer(
                        u32::try_from(storage.byte_size).map_err(|_| {
                            TerminalLayoutError::StructuralTypeTooLarge(structural_type)
                        })?,
                        u16::try_from(storage.byte_alignment).map_err(|_| {
                            TerminalLayoutError::StructuralTypeTooLarge(structural_type)
                        })?,
                    )));
                }
                let mut byte_size = 0_u32;
                let mut alignment = 1_u16;
                let mut complete = true;
                for field in fields {
                    if field.relevance.is_erased()
                        || matches!(field.field_type, StructuralFieldType::Erased { .. })
                    {
                        continue;
                    }
                    let field_shape = resolve_structural_field_shape(
                        &field.field_type,
                        structural_type,
                        declarations,
                        inputs,
                        known,
                        active,
                        extent,
                    )?;
                    let Some(field_shape) = field_shape else {
                        complete = false;
                        continue;
                    };
                    if !complete {
                        continue;
                    }
                    alignment = alignment.max(field_shape.alignment);
                    byte_size =
                        checked_align_up_u32(byte_size, u32::from(field_shape.alignment))
                            .ok_or(TerminalLayoutError::StructuralTypeTooLarge(structural_type))?;
                    byte_size = byte_size
                        .checked_add(field_shape.byte_size)
                        .ok_or(TerminalLayoutError::StructuralTypeTooLarge(structural_type))?;
                }
                if !complete {
                    return Ok(None);
                }
                byte_size = checked_align_up_u32(byte_size, u32::from(alignment))
                    .ok_or(TerminalLayoutError::StructuralTypeTooLarge(structural_type))?;
                Ok(Some(ValueShape::integer(byte_size, alignment)))
            }
            StructuralTypeShape::FixedArray { element, length } => {
                let length = match length {
                    terminal_psi::ArrayExtent::Parameter { .. } => {
                        return Err(TerminalLayoutError::UnclosedArrayExtent(structural_type));
                    }
                    terminal_psi::ArrayExtent::Literal(length) => Some(*length),
                    terminal_psi::ArrayExtent::Application(application) => {
                        extent(structural_type, *application)?
                    }
                };
                if empty_primitive_array(structural_type, declarations) {
                    return Ok(Some(ValueShape::integer(0, 1)));
                }
                let element = resolve_structural_shape(
                    *element,
                    declarations,
                    inputs,
                    known,
                    active,
                    extent,
                )?;
                let (Some(length), Some(element)) = (length, element) else {
                    return Ok(None);
                };
                let stride = checked_align_up_u32(element.byte_size, u32::from(element.alignment))
                    .ok_or(TerminalLayoutError::StructuralTypeTooLarge(structural_type))?;
                let byte_size = u64::from(stride)
                    .checked_mul(length)
                    .and_then(|size| u32::try_from(size).ok())
                    .ok_or(TerminalLayoutError::StructuralTypeTooLarge(structural_type))?;
                Ok(Some(ValueShape::integer(
                    byte_size,
                    if length == 0 { 1 } else { element.alignment },
                )))
            }
            StructuralTypeShape::Sum { cases } => {
                conventional_sum_layout_from_parts(structural_type, &[], cases, &mut |field| {
                    resolve_structural_field_shape(
                        field,
                        structural_type,
                        declarations,
                        inputs,
                        known,
                        active,
                        extent,
                    )
                })
                .map(|layout| layout.map(|layout| layout.shape))
            }
            StructuralTypeShape::Mixed { fields, cases } => {
                conventional_sum_layout_from_parts(structural_type, fields, cases, &mut |field| {
                    resolve_structural_field_shape(
                        field,
                        structural_type,
                        declarations,
                        inputs,
                        known,
                        active,
                        extent,
                    )
                })
                .map(|layout| layout.map(|layout| layout.shape))
            }
        }
    })();
    if let Some(visiting) = active.get_mut(structural_type) {
        *visiting = false;
    }
    let shape = result?;
    if let Some(shape) = shape
        && let Some(known) = known.get_mut(structural_type)
    {
        *known = Some(shape);
    }
    Ok(shape)
}

fn conventional_sum_layout_from_parts(
    structural_type: StructuralTypeId,
    common_fields: &[terminal_psi::StructuralFieldDeclaration],
    cases: &[terminal_psi::StructuralCaseDeclaration],
    field_shape: &mut dyn FnMut(
        &StructuralFieldType,
    ) -> Result<Option<ValueShape>, TerminalLayoutError>,
) -> Result<Option<crate::ConventionalSumLayout>, TerminalLayoutError> {
    if cases.is_empty() {
        return Err(TerminalLayoutError::EmptyStructuralType(structural_type));
    }
    let common = common_fields
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .map(|field| field_shape(&field.field_type))
        .collect::<Result<Vec<_>, _>>()?;
    let payloads = cases
        .iter()
        .map(|case| {
            case.fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .map(|field| field_shape(&field.field_type))
                .collect::<Result<Vec<_>, _>>()
        })
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
    crate::evaluate_conventional_sum_layout(&common, &payloads)
        .map(Some)
        .map_err(|_| TerminalLayoutError::StructuralTypeTooLarge(structural_type))
}

fn resolve_structural_field_shape(
    field: &StructuralFieldType,
    owner: StructuralTypeId,
    declarations: &Declarations<'_>,
    inputs: StorageLayoutInputs<'_>,
    known: &mut KnownShapes,
    active: &mut IdTable<StructuralTypeId, bool>,
    extent: &mut impl FnMut(
        StructuralTypeId,
        ScalarApplicationId,
    ) -> Result<Option<u64>, TerminalLayoutError>,
) -> Result<Option<ValueShape>, TerminalLayoutError> {
    if let StructuralFieldType::Structural(nested) = field {
        return resolve_structural_shape(*nested, declarations, inputs, known, active, extent);
    }
    scalar_field_shape(field, owner).map(Some)
}

fn scalar_storage_type(
    structural_type: StructuralTypeId,
    declarations: &Declarations<'_>,
) -> Option<ScalarType> {
    let StructuralTypeShape::PrimitiveScalar(scalar) = declarations.get(structural_type)?.shape
    else {
        return None;
    };
    runtime_abi::ScalarStorageCodec::from_scalar(scalar).map(|_| scalar)
}

fn scalar_field_shape(
    field: &StructuralFieldType,
    owner: StructuralTypeId,
) -> Result<ValueShape, TerminalLayoutError> {
    if matches!(
        field,
        StructuralFieldType::Atomic(_) | StructuralFieldType::AtomicBoundedInteger(_)
    ) && field.atomic_scalar_type().is_none()
    {
        return Err(TerminalLayoutError::UnknownStructuralType(owner));
    }
    match field {
        StructuralFieldType::RefinedScalar(refinement) => {
            scalar_field_shape(&StructuralFieldType::Scalar(refinement.carrier()), owner)
        }
        StructuralFieldType::Scalar(scalar @ ScalarType::Callable(_)) => scalar_shape(*scalar),
        StructuralFieldType::Atomic(ScalarType::Callable(_)) => {
            Err(TerminalLayoutError::UnknownStructuralType(owner))
        }
        StructuralFieldType::Scalar(ScalarType::Boolean)
        | StructuralFieldType::Atomic(ScalarType::Boolean) => Ok(ValueShape::integer(1, 1)),
        StructuralFieldType::Scalar(ScalarType::Integer(integer))
        | StructuralFieldType::Atomic(ScalarType::Integer(integer)) => {
            Ok(ValueShape::field_integer(closed_integer(*integer)?))
        }
        StructuralFieldType::BoundedInteger(bounds)
        | StructuralFieldType::AtomicBoundedInteger(bounds) => Ok(ValueShape::field_integer(
            closed_integer(bounds.integer_type())?,
        )),
        StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32))
        | StructuralFieldType::Atomic(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32))
        | StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32) => Ok(ValueShape::float(4)),
        StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64))
        | StructuralFieldType::Atomic(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64))
        | StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64) => Ok(ValueShape::float(8)),
        StructuralFieldType::ByteSequence(carrier)
        | StructuralFieldType::PredicateByteSequence { carrier, .. } => {
            byte_sequence_shape(*carrier, owner)
        }
        StructuralFieldType::Structural(_) | StructuralFieldType::Erased { .. } => {
            Err(TerminalLayoutError::UnknownStructuralType(owner))
        }
    }
}

pub fn byte_sequence_shape(
    carrier: terminal_psi::ByteSequenceCarrier,
    structural_type: StructuralTypeId,
) -> Result<ValueShape, TerminalLayoutError> {
    let byte_size = match carrier {
        terminal_psi::ByteSequenceCarrier::BorrowedView { .. } => 16_u64,
        terminal_psi::ByteSequenceCarrier::BoundedView { .. } => 8_u64,
        terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity } => capacity
            .checked_add(8)
            .ok_or(TerminalLayoutError::StructuralTypeTooLarge(structural_type))?,
    };
    Ok(ValueShape::integer(
        u32::try_from(byte_size)
            .map_err(|_| TerminalLayoutError::StructuralTypeTooLarge(structural_type))?,
        8,
    ))
}

pub fn checked_align_up_u32(value: u32, alignment: u32) -> Option<u32> {
    if alignment == 0 {
        return None;
    }
    let remainder = value % alignment;
    if remainder == 0 {
        Some(value)
    } else {
        value.checked_add(alignment - remainder)
    }
}

fn closed_integer(
    integer: semantic_vocabulary::IntegerType,
) -> Result<ClosedIntegerType, TerminalLayoutError> {
    integer
        .try_into()
        .map_err(|_| TerminalLayoutError::UnclosedScalarType(ScalarType::Integer(integer)))
}

/// Foreign raw declarations cross one explicit geometry boundary. Native
/// arithmetic below this point only receives concrete scalar descriptors.
pub fn scalar_shape(scalar: ScalarType) -> Result<ValueShape, TerminalLayoutError> {
    let closed = ClosedScalarType::try_from(scalar)
        .map_err(|_| TerminalLayoutError::UnclosedScalarType(scalar))?;
    Ok(match closed {
        ClosedScalarType::Callable(_) => ValueShape::integer(8, 8),
        ClosedScalarType::Boolean => ValueShape::integer(1, 1),
        ClosedScalarType::Integer(integer) => ValueShape::standalone_integer(integer),
        ClosedScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => ValueShape::float(4),
        ClosedScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => ValueShape::float(8),
    })
}

fn empty_primitive_array(mut current: StructuralTypeId, declarations: &Declarations<'_>) -> bool {
    let mut empty = false;
    for _ in 0..declarations.len() {
        match declarations
            .get(current)
            .map(|declaration| &declaration.shape)
        {
            Some(StructuralTypeShape::FixedArray { element, length }) => {
                let Some(length) = length.literal() else {
                    return false;
                };
                empty |= length == 0;
                current = *element;
            }
            Some(StructuralTypeShape::PrimitiveScalar(scalar)) => {
                return empty && runtime_abi::ScalarStorageCodec::from_scalar(*scalar).is_some();
            }
            _ => return false,
        }
    }
    false
}

#[cfg(test)]
mod width_closure_tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, IntegerType, MachineId, ValueId, WidthParameter};

    #[test]
    fn raw_scalar_and_field_geometry_refuse_a_symbolic_width() {
        let scalar = ScalarType::Integer(IntegerType::parameterized(
            IntegerSign::Unsigned,
            WidthParameter {
                owner: MachineId::new(1).unwrap(),
                value: ValueId::new(1).unwrap(),
            },
        ));
        assert_eq!(
            scalar_shape(scalar),
            Err(TerminalLayoutError::UnclosedScalarType(scalar))
        );
        assert_eq!(
            scalar_field_shape(
                &StructuralFieldType::Scalar(scalar),
                StructuralTypeId::new(1).unwrap()
            ),
            Err(TerminalLayoutError::UnclosedScalarType(scalar)),
        );
    }

    #[test]
    fn closed_geometry_retains_non_byte_width_and_field_alignment() {
        let u13 = IntegerType::new(IntegerSign::Unsigned, 13).unwrap();
        assert_eq!(
            scalar_shape(ScalarType::Integer(u13)).unwrap(),
            ValueShape::integer(2, 2)
        );
        let u24 = IntegerType::new(IntegerSign::Unsigned, 24).unwrap();
        assert_eq!(
            scalar_field_shape(
                &StructuralFieldType::Scalar(ScalarType::Integer(u24)),
                StructuralTypeId::new(1).unwrap()
            )
            .unwrap(),
            ValueShape::integer(3, 4),
        );
    }
}

#[cfg(test)]
mod storage_layout_tests {
    use super::*;
    use terminal_psi::{StructuralAccess, TerminalIdentityIndex};

    fn declaration(id: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            id: StructuralTypeId::new(id).unwrap(),
            identity: TerminalIdentityIndex(id as u32),
            shape,
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            scalar_restriction: None,
        }
    }

    #[test]
    fn execution_owner_has_one_word_only_for_its_checked_borrowing_declaration() {
        let mut owner = declaration(
            1,
            StructuralTypeShape::BorrowingOpaque {
                lifetime_count: std::num::NonZeroU32::MIN,
            },
        );
        owner.owns_linear_claim = true;
        owner.multiplicity = terminal_psi::StructuralMultiplicity::Linear;
        let mut other = owner.clone();
        other.id = StructuralTypeId::new(2).unwrap();
        let mut plain = owner.clone();
        plain.id = StructuralTypeId::new(3).unwrap();
        plain.shape = StructuralTypeShape::Opaque;
        let rows = [owner, other, plain];
        let target = target::NativeTarget::linux_x64();
        let homes = [0, 2].map(|index| {
            runtime_abi::ExecutionOwnerHome::for_target(rows[index].id, target).unwrap()
        });
        let storage = StorageLayoutInputs {
            execution_owners: &homes,
            ..StorageLayoutInputs::default()
        };
        let geometry = NativeGeometry::with_storage_inputs(&rows, storage).unwrap();
        let layouts = StructuralLayouts::with_storage_inputs(rows.iter(), storage).unwrap();
        assert!(
            layouts
                .required_scalar_extents([rows[0].id])
                .unwrap()
                .is_empty()
        );
        assert!(layouts.required_scalar_extents([rows[1].id]).is_err());
        assert_eq!(geometry.shape(rows[0].id), Ok(ValueShape::integer(8, 8)));
        for foreign in &rows[1..] {
            assert_eq!(
                geometry.shape(foreign.id),
                Err(TerminalLayoutError::UnclosedOpaqueRepresentation(
                    foreign.id
                )),
            );
        }
        assert_eq!(
            NativeGeometry::new(&rows).unwrap().shape(rows[0].id),
            Err(TerminalLayoutError::UnclosedOpaqueRepresentation(
                rows[0].id
            )),
        );
    }

    fn declarations() -> Vec<StructuralTypeDeclaration> {
        let element = StructuralTypeId::new(1).unwrap();
        vec![
            declaration(
                1,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        32,
                    )
                    .unwrap(),
                )),
            ),
            declaration(
                2,
                StructuralTypeShape::ElementView {
                    element,
                    access: None,
                },
            ),
            declaration(
                3,
                StructuralTypeShape::ElementView {
                    element,
                    access: Some(StructuralAccess::SharedBorrow),
                },
            ),
            declaration(
                4,
                StructuralTypeShape::FixedArray {
                    element: StructuralTypeId::new(2).unwrap(),
                    length: terminal_psi::ArrayExtent::Literal(2),
                },
            ),
        ]
    }

    #[test]
    fn only_package_references_reserve_a_scalar_codec_for_an_unknown_referent() {
        let id = |n| StructuralTypeId::new(n).unwrap();
        let reference = |referent| StructuralTypeShape::Reference {
            referent,
            access: StructuralAccess::MutableBorrow,
        };
        let rows = [
            declaration(1, StructuralTypeShape::Opaque),
            declaration(2, reference(id(1))),
            declaration(3, StructuralTypeShape::Opaque),
            declaration(4, reference(id(3))),
            declaration(5, StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)),
            declaration(6, reference(id(5))),
        ];
        let witnesses = [id(1)];
        let inputs = StorageLayoutInputs {
            package_witnesses: &witnesses,
            ..StorageLayoutInputs::default()
        };
        let geometry = NativeGeometry::with_storage_inputs(&rows, inputs).unwrap();
        assert_eq!(geometry.shape(id(2)).unwrap(), ValueShape::integer(16, 8));
        assert_eq!(geometry.shape(id(4)).unwrap(), ValueShape::integer(8, 8));
        assert_eq!(geometry.shape(id(6)).unwrap(), ValueShape::integer(16, 8));
        let public = StructuralFieldType::Structural(id(2));
        assert_eq!(
            geometry.package_reference_field(&public, &StructuralFieldType::Structural(id(4))),
            Some(true)
        );
        assert_eq!(
            geometry.package_reference_field(&public, &StructuralFieldType::Structural(id(6))),
            Some(true)
        );
        assert_eq!(
            geometry.package_reference_field(
                &public,
                &StructuralFieldType::Scalar(ScalarType::Boolean)
            ),
            Some(false)
        );
        assert!(
            geometry.shape(id(1)).is_err(),
            "a uniform borrow grants no layout for its referent"
        );
    }

    #[test]
    fn owned_sequence_storage_moves_inline_while_borrowed_views_keep_descriptors() {
        let rows = declarations();
        let owned = rows[1].id;
        let bounds = [(owned, 3)];
        let geometry = NativeGeometry::with_storage_inputs(
            &rows,
            StorageLayoutInputs {
                owned_sequences: &bounds,
                ..Default::default()
            },
        )
        .unwrap();
        for access in [None, Some(terminal_psi::StructuralAccess::Owned)] {
            assert!(!is_native_view_descriptor_at(&rows[1].shape, access));
        }
        let storage = geometry.owned_sequence(owned).unwrap();
        assert_eq!(
            (storage.elements_offset, storage.stride, storage.capacity),
            (8, 4, 3)
        );
        assert_eq!(geometry.shape(owned).unwrap(), ValueShape::integer(24, 8));
        assert_eq!(
            geometry.shape(rows[2].id).unwrap(),
            ValueShape::integer(16, 8)
        );
        assert_eq!(
            geometry.shape(rows[3].id).unwrap(),
            ValueShape::integer(48, 8)
        );
        assert_eq!(element_view_length_offset(&rows[1].shape), Some(0));
        assert_eq!(element_view_length_offset(&rows[2].shape), Some(8));
        let layouts = StructuralLayouts::with_storage_inputs(
            rows.iter(),
            StorageLayoutInputs {
                owned_sequences: &bounds,
                ..Default::default()
            },
        )
        .unwrap();
        for access in [
            terminal_psi::StructuralAccess::SharedBorrow,
            terminal_psi::StructuralAccess::MutableBorrow,
        ] {
            assert!(is_native_view_descriptor_at(&rows[1].shape, Some(access)));
            assert_eq!(
                geometry.formal_storage_shape(owned, access),
                layouts.formal_storage_shape(owned, access)
            );
            assert_eq!(
                element_view_length_at(&rows[1].shape, Some(access)),
                Some(8)
            );
            assert_eq!(
                layouts.formal_storage_shape(owned, access).unwrap(),
                ValueShape::integer(16, 8)
            );
        }
        assert_eq!(
            layouts
                .formal_storage_shape(owned, terminal_psi::StructuralAccess::Owned)
                .unwrap(),
            storage.shape
        );
    }

    #[test]
    fn an_owned_sequence_needs_its_own_finite_unambiguous_storage_bound() {
        let rows = declarations();
        let owned = rows[1].id;
        assert_eq!(
            NativeGeometry::new(&rows).unwrap().shape(owned),
            Err(TerminalLayoutError::UnclosedOwnedSequence(owned))
        );
        let wrong_owner = [(rows[2].id, 3)];
        assert_eq!(
            NativeGeometry::with_storage_inputs(
                &rows,
                StorageLayoutInputs {
                    owned_sequences: &wrong_owner,
                    ..Default::default()
                }
            )
            .unwrap()
            .shape(owned),
            Err(TerminalLayoutError::UnclosedOwnedSequence(owned))
        );
        let duplicate = [(owned, 1), (owned, 3)];
        assert_eq!(
            NativeGeometry::with_storage_inputs(
                &rows,
                StorageLayoutInputs {
                    owned_sequences: &duplicate,
                    ..Default::default()
                }
            )
            .unwrap()
            .shape(owned),
            Err(TerminalLayoutError::InvalidStorageGeometry(owned))
        );
        let overflow = [(owned, u64::MAX)];
        assert_eq!(
            NativeGeometry::with_storage_inputs(
                &rows,
                StorageLayoutInputs {
                    owned_sequences: &overflow,
                    ..Default::default()
                }
            )
            .unwrap()
            .shape(owned),
            Err(TerminalLayoutError::StructuralTypeTooLarge(owned))
        );
        let empty = [(owned, 0)];
        assert_eq!(
            NativeGeometry::with_storage_inputs(
                &rows,
                StorageLayoutInputs {
                    owned_sequences: &empty,
                    ..Default::default()
                }
            )
            .unwrap()
            .shape(owned)
            .unwrap(),
            ValueShape::integer(8, 8)
        );
    }
}
