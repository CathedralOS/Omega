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
use semantic_vocabulary::{
    IdTable, IeeeFloatFormat, NonDenseIdentities, ScalarApplicationId, ScalarType, StructuralTypeId,
};
use std::cell::RefCell;
use std::collections::BTreeSet;
use terminal_psi::{StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalLayoutError {
    RecursiveStructuralType(StructuralTypeId),
    UnknownStructuralType(StructuralTypeId),
    UnclosedTargetLayout(StructuralTypeId),
    UnclosedArrayExtent(StructuralTypeId),
    StructuralTypeTooLarge(StructuralTypeId),
    EmptyStructuralType(StructuralTypeId),
    UnsupportedStructuralSum(StructuralTypeId),
    InvalidStorageGeometry(StructuralTypeId),
    ReentrantLayoutQuery(StructuralTypeId),
}

/// Borrows a declaration catalog together with its invocation-owned native
/// layout queries. Consumers retain this context across related projections.
pub struct NativeGeometry<'a> {
    declarations: &'a [StructuralTypeDeclaration],
    pub(super) resolver: StructuralLayoutResolver<'a>,
}

impl<'a> NativeGeometry<'a> {
    pub fn new(declarations: &'a [StructuralTypeDeclaration]) -> Result<Self, NonDenseIdentities> {
        Ok(Self {
            declarations,
            resolver: StructuralLayoutResolver::new(declarations)?,
        })
    }

    pub fn shape(&self, root: StructuralTypeId) -> Result<crate::ValueShape, TerminalLayoutError> {
        self.resolver.shape(root)
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

type Declarations<'a> = IdTable<StructuralTypeId, &'a StructuralTypeDeclaration>;
type KnownShapes = IdTable<StructuralTypeId, Option<ValueShape>>;

/// Recomputes native geometry from declarations, resolving only demanded roots.
/// Retain one resolver across related queries; it never consumes a producer's
/// claimed layout result as evidence.
pub struct StructuralLayoutResolver<'a> {
    declarations: Declarations<'a>,
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
        let declarations = IdTable::from_rows(rows.into_iter().map(|row| (row.id, row)))?;
        let known = declarations.map(|_, _| None);
        let active = declarations.map(|_, _| false);
        Ok(Self {
            declarations,
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
                ..scalar_shape(scalar)
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
                &mut known,
                &mut active,
                &mut reject_unclosed_extent,
            )?
            .ok_or(TerminalLayoutError::UnclosedArrayExtent(structural_type))
        });
        StructuralLayouts {
            declarations,
            shapes,
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
}

impl<'a> StructuralLayouts<'a> {
    pub fn new(
        rows: impl IntoIterator<Item = &'a StructuralTypeDeclaration>,
    ) -> Result<Self, NonDenseIdentities> {
        Ok(StructuralLayoutResolver::new(rows)?.materialize())
    }

    pub fn shape(
        &self,
        structural_type: StructuralTypeId,
    ) -> Result<ValueShape, TerminalLayoutError> {
        self.shapes.get(structural_type).copied().unwrap_or(Err(
            TerminalLayoutError::UnknownStructuralType(structural_type),
        ))
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

fn reject_unclosed_extent(
    owner: StructuralTypeId,
    _: ScalarApplicationId,
) -> Result<Option<u64>, TerminalLayoutError> {
    Err(TerminalLayoutError::UnclosedArrayExtent(owner))
}

fn resolve_structural_shape(
    structural_type: StructuralTypeId,
    declarations: &Declarations<'_>,
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
            StructuralTypeShape::Reference { referent, .. } => {
                declarations
                    .get(*referent)
                    .ok_or(TerminalLayoutError::UnknownStructuralType(*referent))?;
                Ok(Some(ValueShape::integer(
                    if scalar_storage_type(*referent, declarations).is_some() {
                        runtime_abi::ScalarReferenceLayout::NATIVE.byte_size
                    } else {
                        8
                    },
                    8,
                )))
            }
            StructuralTypeShape::PrimitiveScalar(scalar) => Ok(Some(scalar_shape(*scalar))),
            StructuralTypeShape::AtomicCell(cell) => Ok(Some(scalar_shape(cell.scalar_type()))),
            StructuralTypeShape::ByteSequence(carrier) => {
                byte_sequence_shape(*carrier, structural_type).map(Some)
            }
            StructuralTypeShape::ElementView { element, .. } => {
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
                    terminal_psi::ArrayExtent::Literal(length) => Some(*length),
                    terminal_psi::ArrayExtent::Application(application) => {
                        extent(structural_type, *application)?
                    }
                };
                if empty_primitive_array(structural_type, declarations) {
                    return Ok(Some(ValueShape::integer(0, 1)));
                }
                let element =
                    resolve_structural_shape(*element, declarations, known, active, extent)?;
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
    known: &mut KnownShapes,
    active: &mut IdTable<StructuralTypeId, bool>,
    extent: &mut impl FnMut(
        StructuralTypeId,
        ScalarApplicationId,
    ) -> Result<Option<u64>, TerminalLayoutError>,
) -> Result<Option<ValueShape>, TerminalLayoutError> {
    if let StructuralFieldType::Structural(nested) = field {
        return resolve_structural_shape(*nested, declarations, known, active, extent);
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
        StructuralFieldType::Scalar(scalar @ ScalarType::Callable(_)) => Ok(scalar_shape(*scalar)),
        StructuralFieldType::Atomic(ScalarType::Callable(_)) => {
            Err(TerminalLayoutError::UnknownStructuralType(owner))
        }
        StructuralFieldType::Scalar(ScalarType::Boolean)
        | StructuralFieldType::Atomic(ScalarType::Boolean) => Ok(ValueShape::integer(1, 1)),
        StructuralFieldType::Scalar(ScalarType::Integer(integer))
        | StructuralFieldType::Atomic(ScalarType::Integer(integer)) => {
            Ok(ValueShape::field_integer(*integer))
        }
        StructuralFieldType::BoundedInteger(bounds)
        | StructuralFieldType::AtomicBoundedInteger(bounds) => {
            Ok(ValueShape::field_integer(bounds.integer_type()))
        }
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

pub fn scalar_shape(scalar: ScalarType) -> ValueShape {
    match scalar {
        ScalarType::Callable(_) => ValueShape::integer(8, 8),
        ScalarType::Boolean => ValueShape::integer(1, 1),
        ScalarType::Integer(integer) => ValueShape::standalone_integer(integer),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => ValueShape::float(4),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => ValueShape::float(8),
    }
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
