//! Reference formation keeps its original storage and states its observation type.
//!
//! Exact referent identity remains the aggregate contract. A shared primitive
//! reference may instead expose a total representation view: every source bit
//! pattern must be valid at the target. This is not source authority, lifetime,
//! qualification evidence or permission to write through the viewed carrier.

use semantic_vocabulary::{ScalarType, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralTypeDeclaration, StructuralTypeShape};

use crate::scalar_representation_view::{integer_bits, integer_from_bits};
use crate::{ScalarRepresentationViewPair, ScalarViewValue};

/// Whether every representation of `source` is a valid observation at `target`.
/// Address carriers never acquire integer representation authority by width.
/// Integer-to-Boolean needs a landing proof and is not a total shared view.
/// Nonidentity integer views use the existing at-most-64-bit storage bit map.
pub fn scalar_reference_view_is_total(source: ScalarType, target: ScalarType) -> bool {
    if source == target {
        return true;
    }
    if let (ScalarType::Integer(source), ScalarType::Integer(target)) = (source, target) {
        return !source.is_address()
            && !target.is_address()
            && source.bits() == target.bits()
            && source.bits() <= 64;
    }
    ScalarRepresentationViewPair::classify(source, target)
        .is_some_and(|pair| !pair.requires_landing())
}

/// Observe a validated source scalar without modifying its backing value.
/// The bit map is shared with scalar representation operations; integer
/// sign changes use that owner's exact fixed-width bit interpretation.
pub fn scalar_reference_view_value(
    source: ScalarType,
    target: ScalarType,
    value: ScalarViewValue,
) -> Option<ScalarViewValue> {
    if !scalar_reference_view_is_total(source, target) {
        return None;
    }
    if source == target {
        return match (source, value) {
            (ScalarType::Boolean, ScalarViewValue::Boolean(_)) => Some(value),
            (ScalarType::Integer(integer), ScalarViewValue::Integer(value))
                if integer.admits(value) =>
            {
                Some(ScalarViewValue::Integer(value))
            }
            (ScalarType::IeeeFloat(format), ScalarViewValue::IeeeFloat(value))
                if value.format() == format =>
            {
                Some(ScalarViewValue::IeeeFloat(value))
            }
            _ => None,
        };
    }
    if let (
        ScalarType::Integer(source),
        ScalarType::Integer(target),
        ScalarViewValue::Integer(value),
    ) = (source, target, value)
    {
        return Some(ScalarViewValue::Integer(integer_from_bits(
            target,
            integer_bits(source, value)?,
        )));
    }
    ScalarRepresentationViewPair::classify(source, target)?.view(value)
}

/// Reconstruct one reference's source-to-observation representation contract.
/// `reference` names the result carrier, not its referent. Callers independently
/// validate source access, exact occurrence, qualifications and live backing.
pub fn reference_source_compatible<'types>(
    mut types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    source: StructuralTypeId,
    reference: StructuralTypeId,
) -> bool {
    let Some(StructuralTypeShape::Reference { referent, access }) = types
        .clone()
        .find(|declaration| declaration.id == reference)
        .map(|declaration| &declaration.shape)
    else {
        return false;
    };
    let Some(source_shape) = types
        .clone()
        .find(|declaration| declaration.id == source)
        .map(|declaration| &declaration.shape)
    else {
        return false;
    };
    if source == *referent {
        return true;
    }
    if *access != StructuralAccess::SharedBorrow {
        return false;
    }
    let Some(StructuralTypeShape::PrimitiveScalar(target)) = types
        .find(|declaration| declaration.id == *referent)
        .map(|declaration| &declaration.shape)
    else {
        return false;
    };
    matches!(source_shape, StructuralTypeShape::PrimitiveScalar(source)
        if scalar_reference_view_is_total(*source, *target))
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IeeeFloatFormat, IntegerSign, IntegerType};

    fn unsigned(bits: u16) -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap())
    }

    #[test]
    fn shared_reference_views_observe_bits_without_numeric_conversion() {
        use semantic_vocabulary::{IeeeFloatValue, IntegerValue};
        let f32_type = ScalarType::IeeeFloat(IeeeFloatFormat::Binary32);
        let payload = IeeeFloatValue::Binary32(0x7fc0_1234);
        let bits = ScalarViewValue::Integer(IntegerValue::Unsigned(0x7fc0_1234));
        assert_eq!(
            scalar_reference_view_value(
                f32_type,
                unsigned(32),
                ScalarViewValue::IeeeFloat(payload)
            ),
            Some(bits)
        );
        assert_eq!(
            scalar_reference_view_value(unsigned(32), f32_type, bits),
            Some(ScalarViewValue::IeeeFloat(payload))
        );
        let signed = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 8).unwrap());
        assert_eq!(
            scalar_reference_view_value(
                signed,
                unsigned(8),
                ScalarViewValue::Integer(IntegerValue::Signed(-1))
            ),
            Some(ScalarViewValue::Integer(IntegerValue::Unsigned(255)))
        );
        assert_eq!(
            scalar_reference_view_value(
                unsigned(8),
                ScalarType::Boolean,
                ScalarViewValue::Integer(IntegerValue::Unsigned(1))
            ),
            None
        );
        assert_eq!(
            scalar_reference_view_value(
                unsigned(8),
                signed,
                ScalarViewValue::Integer(IntegerValue::Unsigned(256))
            ),
            None
        );
    }

    #[test]
    fn shared_reference_views_require_total_exact_carriers() {
        let f32_type = ScalarType::IeeeFloat(IeeeFloatFormat::Binary32);
        let f64_type = ScalarType::IeeeFloat(IeeeFloatFormat::Binary64);
        assert!(scalar_reference_view_is_total(f32_type, unsigned(32)));
        assert!(scalar_reference_view_is_total(unsigned(64), f64_type));
        assert!(scalar_reference_view_is_total(
            ScalarType::Boolean,
            unsigned(8)
        ));
        assert!(!scalar_reference_view_is_total(
            unsigned(8),
            ScalarType::Boolean
        ));
        assert!(!scalar_reference_view_is_total(f32_type, unsigned(64)));
        assert!(!scalar_reference_view_is_total(f32_type, f64_type));
        assert!(!scalar_reference_view_is_total(
            ScalarType::Integer(IntegerType::address(64).unwrap()),
            unsigned(64),
        ));
        let id = |raw| StructuralTypeId::new(raw).unwrap();
        let mut types = vec![
            StructuralTypeDeclaration {
                id: id(1),
                identity: "f32".into(),
                shape: StructuralTypeShape::PrimitiveScalar(f32_type),
            },
            StructuralTypeDeclaration {
                id: id(2),
                identity: "u32".into(),
                shape: StructuralTypeShape::PrimitiveScalar(unsigned(32)),
            },
            StructuralTypeDeclaration {
                id: id(3),
                identity: "view".into(),
                shape: StructuralTypeShape::Reference {
                    referent: id(2),
                    access: StructuralAccess::SharedBorrow,
                },
            },
        ];
        assert!(reference_source_compatible(types.iter(), id(1), id(3)));
        assert!(!reference_source_compatible(types.iter(), id(4), id(3)));
        assert!(!reference_source_compatible(types.iter(), id(1), id(2)));
        let StructuralTypeShape::Reference { access, .. } = &mut types[2].shape else {
            unreachable!()
        };
        *access = StructuralAccess::MutableBorrow;
        assert!(!reference_source_compatible(types.iter(), id(1), id(3)));
        assert!(reference_source_compatible(types.iter(), id(2), id(3)));
    }
}
