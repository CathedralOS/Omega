/*
An owned element sequence such as `[Ticket { number: 11u32 }]` is immutable and keeps its exact
element roster wherever it goes: a local, a returned value or a by-value argument. A frame slot
cannot back it, because the creating machine's frame ends at return while the sequence lives on.
When every element is a record of constant scalars, the roster's bytes are known at compile time,
so native code places them in the image's read-only literal pool and the sequence's stored-length
descriptor (pointer, element count) points there. That backing lives as long as the program, so
moving the descriptor never outlives its storage.

Target lowering, legalization and image emission each derive the roster from the same abstract
construction through this one function, so the pool bytes always match the checked elements.
Anything else (a runtime element value, a nested structural field, authored field storage)
returns `None`, and native lowering refuses the construction.
*/

use abstract_operations::AbstractOperation;
use calling_conventions::ValueShape;
use calling_conventions::terminal_layout::{
    NativeGeometry, StructuralLayouts, TerminalLayoutError, checked_align_up_u32,
};
use semantic_vocabulary::{IntegerValue, ScalarType, StructuralTypeId, ValueId};
use terminal_psi::{
    RecordFieldValue, StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape,
};

/// The native layout queries the roster encoding needs. Target lowering holds a
/// `StructuralLayouts` and later stages a `NativeGeometry`; both answer from the same
/// declarations.
pub trait RosterGeometry {
    fn declaration(&self, structural_type: StructuralTypeId) -> Option<&StructuralTypeDeclaration>;
    fn shape(&self, structural_type: StructuralTypeId) -> Result<ValueShape, TerminalLayoutError>;
    fn field_shape(
        &self,
        field: &StructuralFieldType,
        owner: StructuralTypeId,
    ) -> Result<ValueShape, TerminalLayoutError>;
}

impl RosterGeometry for StructuralLayouts<'_> {
    fn declaration(&self, structural_type: StructuralTypeId) -> Option<&StructuralTypeDeclaration> {
        self.get(structural_type).copied()
    }
    fn shape(&self, structural_type: StructuralTypeId) -> Result<ValueShape, TerminalLayoutError> {
        StructuralLayouts::shape(self, structural_type)
    }
    fn field_shape(
        &self,
        field: &StructuralFieldType,
        owner: StructuralTypeId,
    ) -> Result<ValueShape, TerminalLayoutError> {
        StructuralLayouts::field_shape(self, field, owner)
    }
}

impl RosterGeometry for NativeGeometry<'_> {
    fn declaration(&self, structural_type: StructuralTypeId) -> Option<&StructuralTypeDeclaration> {
        NativeGeometry::declaration(self, structural_type).ok()
    }
    fn shape(&self, structural_type: StructuralTypeId) -> Result<ValueShape, TerminalLayoutError> {
        NativeGeometry::shape(self, structural_type)
    }
    fn field_shape(
        &self,
        field: &StructuralFieldType,
        owner: StructuralTypeId,
    ) -> Result<ValueShape, TerminalLayoutError> {
        NativeGeometry::field_shape(self, field, owner)
    }
}

/// The read-only bytes of one constant owned element sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstantElementRoster {
    pub element: StructuralTypeId,
    pub count: u64,
    /// Element size rounded up to its alignment, as element views index it.
    pub stride: u32,
    pub alignment: u16,
    pub octets: Vec<u8>,
}

/// Encodes `operation` when it builds an owned element sequence from records of constant
/// scalars, all established among `function`'s operations.
pub fn constant_element_roster<'a>(
    function: impl Iterator<Item = &'a AbstractOperation> + Clone,
    operation: &AbstractOperation,
    layouts: &(impl RosterGeometry + ?Sized),
) -> Option<ConstantElementRoster> {
    let AbstractOperation::EstablishStructuralArray {
        result, elements, ..
    } = operation
    else {
        return None;
    };
    let StructuralTypeShape::ElementView {
        element,
        access: None,
    } = layouts.declaration(result.structural_type)?.shape
    else {
        return None;
    };
    if !result.claims.is_empty() || !result.qualification_establishments.is_empty() {
        return None;
    }
    let shape = layouts.shape(element).ok()?;
    let stride = checked_align_up_u32(shape.byte_size, u32::from(shape.alignment))?;
    let StructuralTypeShape::Record {
        fields: declarations,
        storage: terminal_psi::StructuralRecordStorage::Packed,
    } = &layouts.declaration(element)?.shape
    else {
        return None;
    };
    let mut octets = Vec::with_capacity(usize::try_from(stride).ok()? * elements.len());
    for argument in elements {
        if argument.access != terminal_psi::StructuralAccess::Owned || !argument.path.is_empty() {
            return None;
        }
        let mut producers = function.clone().filter_map(|operation| {
            operation
                .structural_result()
                .filter(|(_, produced)| produced.place == argument.place)
                .map(|_| operation)
        });
        let (
            Some(AbstractOperation::EstablishRecord {
                result: record,
                fields,
                ..
            }),
            None,
        ) = (producers.next(), producers.next())
        else {
            return None;
        };
        if record.structural_type != element || !record.claims.is_empty() {
            return None;
        }
        let start = octets.len();
        octets.resize(start + usize::try_from(stride).ok()?, 0);
        let mut offset = 0_u32;
        let mut encoded = 0_usize;
        for declaration in declarations {
            if declaration.relevance.is_erased()
                || matches!(declaration.field_type, StructuralFieldType::Erased { .. })
            {
                continue;
            }
            let scalar_type = match declaration.field_type {
                StructuralFieldType::Scalar(
                    scalar @ (ScalarType::Boolean | ScalarType::Integer(_)),
                ) if declaration.storage.is_none() => scalar,
                _ => return None,
            };
            let field_shape = layouts.field_shape(&declaration.field_type, element).ok()?;
            offset = checked_align_up_u32(offset, u32::from(field_shape.alignment))?;
            let mut initializers = fields
                .iter()
                .filter(|initializer| initializer.field == declaration.id);
            let (
                Some(terminal_psi::RecordFieldInitializer {
                    value: RecordFieldValue::Scalar { value, .. },
                    ..
                }),
                None,
            ) = (initializers.next(), initializers.next())
            else {
                return None;
            };
            let width = usize::try_from(field_shape.byte_size).ok()?;
            let bits = encode(
                constant_value(function.clone(), *value, scalar_type)?,
                width,
            )?;
            let at = start + usize::try_from(offset).ok()?;
            octets
                .get_mut(at..at + width)?
                .copy_from_slice(&bits.to_le_bytes()[..width]);
            offset = offset.checked_add(field_shape.byte_size)?;
            encoded += 1;
        }
        if encoded != fields.len() || offset > stride {
            return None;
        }
    }
    Some(ConstantElementRoster {
        element,
        count: u64::try_from(elements.len()).ok()?,
        stride,
        alignment: shape.alignment,
        octets,
    })
}

/// A scalar constant as the function defines it, or `None` when the value is computed at run
/// time or its type differs from the field's.
fn constant_value<'a>(
    function: impl Iterator<Item = &'a AbstractOperation>,
    value: ValueId,
    expected: ScalarType,
) -> Option<IntegerValue> {
    let mut producers = function.filter_map(|operation| match operation {
        AbstractOperation::IntegerConstant {
            result,
            scalar_type,
            value: constant,
            ..
        } if *result == value => Some((*scalar_type == expected).then_some(*constant)),
        AbstractOperation::BooleanConstant {
            result,
            value: constant,
            ..
        } if *result == value => Some(
            (expected == ScalarType::Boolean)
                .then_some(IntegerValue::Unsigned(u128::from(*constant))),
        ),
        _ => None,
    });
    match (producers.next(), producers.next()) {
        (Some(value), None) => value,
        _ => None,
    }
}

/// The little-endian two's-complement bits of `value` in `width` bytes, refusing a value
/// that does not fit. Every hosted target is little-endian.
fn encode(value: IntegerValue, width: usize) -> Option<u128> {
    if width == 0 || width > 16 {
        return None;
    }
    let bits = u32::try_from(width * 8).ok()?;
    let mask = u128::MAX >> (128 - bits);
    match value {
        IntegerValue::Unsigned(unsigned) => (unsigned & !mask == 0).then_some(unsigned),
        IntegerValue::Signed(signed) => {
            let low = i128::MIN >> (128 - bits);
            let high = i128::MAX >> (128 - bits);
            (low..=high)
                .contains(&signed)
                .then_some((signed as u128) & mask)
        }
    }
}
