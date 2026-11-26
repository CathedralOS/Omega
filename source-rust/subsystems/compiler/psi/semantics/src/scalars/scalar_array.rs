use semantic_vocabulary::{ScalarType, StructuralTypeId};
use terminal_psi::{StructuralTypeDeclaration, StructuralTypeShape};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PackedArrayRole {
    Input,
    Result,
}

pub fn packed_integer_array_type<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    operation: semantic_vocabulary::PackedIntegerBinaryOperation,
) -> Option<semantic_vocabulary::IntegerType> {
    use semantic_vocabulary::{IntegerSign, PackedIntegerBinaryOperation::*};
    let (sign, bits, lanes) = match operation {
        WrappingAddU8x16 => (IntegerSign::Unsigned, 8, 16),
        WrappingAddI16x8 | WrappingSubI16x8 | MinI16x8 | CmpGtI16x8 | BitAndI16x8
        | BitAndNotI16x8 => (IntegerSign::Signed, 16, 8),
        ShuffleI16x8 { lanes } if lanes.iter().all(|&index| index < 16) => {
            (IntegerSign::Signed, 16, 8)
        }
        ShuffleI16x8 { .. } => return None,
    };
    packed_array_lane_type(types, root, sign, bits, lanes)
}

pub fn packed_integer_unary_array_type<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    operation: semantic_vocabulary::PackedIntegerUnaryOperation,
    role: PackedArrayRole,
) -> Option<semantic_vocabulary::IntegerType> {
    use semantic_vocabulary::{IntegerSign, PackedIntegerUnaryOperation::*};
    let (sign, bits, lanes) = match (operation, role) {
        (ShiftRightArithmeticI16x8 { count: 0..=15 } | AbsI16x8, _)
        | (LoadI16x4ZeroExtend | WidenU8x8ToI16x8 | WidenU8x4ToI16x8, PackedArrayRole::Result)
        | (NarrowI16x8ToU8x8Saturating | NarrowI16x8ToU8x4Saturating, PackedArrayRole::Input) => {
            (IntegerSign::Signed, 16, 8)
        }
        (LoadI16x4ZeroExtend, PackedArrayRole::Input) => (IntegerSign::Signed, 16, 4),
        (WidenU8x8ToI16x8, PackedArrayRole::Input)
        | (NarrowI16x8ToU8x8Saturating, PackedArrayRole::Result) => (IntegerSign::Unsigned, 8, 8),
        (WidenU8x4ToI16x8, PackedArrayRole::Input)
        | (NarrowI16x8ToU8x4Saturating, PackedArrayRole::Result) => (IntegerSign::Unsigned, 8, 4),
        (ShiftRightArithmeticI16x8 { .. }, _) => return None,
    };
    packed_array_lane_type(types, root, sign, bits, lanes)
}

/// Packed arithmetic constructs every bit in an unrestricted, one-dimensional array.
fn packed_array_lane_type<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    sign: semantic_vocabulary::IntegerSign,
    bits: u16,
    expected_lanes: u64,
) -> Option<semantic_vocabulary::IntegerType> {
    let array = crate::structural_declaration(types.clone(), root)?;
    let StructuralTypeShape::FixedArray {
        element,
        length: terminal_psi::ArrayExtent::Literal(length),
    } = array.shape
    else {
        return None;
    };
    if length != expected_lanes {
        return None;
    }
    let leaf = crate::structural_declaration(types, element)?;
    let StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)) = leaf.shape else {
        return None;
    };
    let plain = [array, leaf].iter().all(|ty| {
        ty.is_copyable()
            && !ty.owns_linear_claim
            && ty.scalar_restriction.is_none()
            && ty.requires.is_empty()
            && ty.quotient_carrier.is_none()
            && ty.nominal_cleanup.is_none()
    });
    (plain && integer.sign() == sign && integer.bits() == Some(bits)).then_some(integer)
}

pub fn scalar_array_leaf_path<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    mut ordinal: u64,
) -> Option<Vec<terminal_psi::StructuralPathSegment>> {
    let (_, count) = scalar_array_leaf_declaration(types.clone(), root)?;
    if ordinal >= count {
        return None;
    }
    let mut dimensions = Vec::new();
    let mut current = root;
    loop {
        match *crate::structural_shape(types.clone(), current)? {
            StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } => {
                dimensions.push(length);
                current = element;
            }
            StructuralTypeShape::PrimitiveScalar(_) => break,
            _ => return None,
        }
    }
    let mut path = Vec::with_capacity(dimensions.len());
    for length in dimensions.into_iter().rev() {
        path.push(terminal_psi::StructuralPathSegment::FixedIndex(
            ordinal.checked_rem(length)?,
        ));
        ordinal = ordinal.checked_div(length)?;
    }
    path.reverse();
    Some(path)
}

pub fn scalar_array_leaf_shape<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> Option<(ScalarType, u64)> {
    scalar_array_leaf_declaration(types, root).map(|(declaration, count)| {
        let StructuralTypeShape::PrimitiveScalar(scalar) = declaration.shape else {
            unreachable!()
        };
        (scalar, count)
    })
}

pub fn scalar_array_leaf_declaration<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> Option<(&'types StructuralTypeDeclaration, u64)> {
    let mut current = root;
    let mut leaf_count = Some(1_u64);
    let mut empty = false;
    let mut array = false;
    for _ in 0..types.clone().count() {
        let declaration = crate::structural_declaration(types.clone(), current)?;
        match declaration.shape {
            StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } => {
                array = true;
                empty |= length == 0;
                leaf_count = leaf_count.and_then(|count| count.checked_mul(length));
                current = element;
            }
            StructuralTypeShape::PrimitiveScalar(_) if array => {
                return Some((declaration, if empty { 0 } else { leaf_count? }));
            }
            _ => return None,
        }
    }
    None
}
