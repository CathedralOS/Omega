//! Symbolic scalar reads through checked representation views.
//!
//! A view preserves a captured bit pattern, independently of the target's
//! numeric equality (notably NaNs and signed zero). This owner records that
//! interpretation and composes inverse views; recast admission still owns
//! geometry, validity, lifetime and access before any value is observed.

use super::StructuralTerm;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

pub(in crate::proof_contracts::contract_entailment) fn scalar_representation_term(
    program: &dyn crate::CheckingInput,
    cast: &typed_trees::expression::TableCastExpression,
    source: TypeReferenceHandle,
    value: StructuralTerm,
) -> Option<StructuralTerm> {
    if cast.form.recast_access() != Some(language_semantics::ReferenceAccess::Shared) {
        return None;
    }
    if !super::scalar_operand_is_observed(program, source, &value) {
        return None;
    }
    // The source borrow may enclose this recast, so its operand is the
    // selected place's owned carrier. An already borrowed operand must carry
    // its own authenticated shared observation.
    let source_carrier = match crate::reference_shell(program, source) {
        Some(shell) if shell.access == language_semantics::ReferenceAccess::Shared => shell.referee,
        Some(_) => return None,
        None => source,
    };
    let source = program.primitive_type_reference(source_carrier)?;
    // A recast stores the referent carrier separately from its loan access.
    // The prepared expression type owns the resulting descriptor shell.
    let target = program.primitive_type_reference(cast.target_type)?;
    let scalar = |primitive: PrimitiveType| {
        primitive.is_fixed_integer() || matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
    };
    if !scalar(source) || !scalar(target) || source.scalar_byte_size() != target.scalar_byte_size()
    {
        return None;
    }
    Some(normalize_scalar_representation(source, target, value))
}

pub(in crate::proof_contracts::contract_entailment) fn normalize_scalar_representation(
    source: PrimitiveType,
    target: PrimitiveType,
    value: StructuralTerm,
) -> StructuralTerm {
    if source == target {
        return value;
    }
    if let StructuralTerm::ScalarRepresentation {
        source: original,
        target: intermediate,
        value: original_value,
    } = value.semantic_value()
        && *intermediate == source
    {
        return normalize_scalar_representation(*original, target, *original_value.clone());
    }
    if let Some(value) = represented_literal(source, target, &value) {
        return value;
    }
    StructuralTerm::ScalarRepresentation {
        source,
        target,
        value: Box::new(value),
    }
}

fn represented_literal(
    source: PrimitiveType,
    target: PrimitiveType,
    value: &StructuralTerm,
) -> Option<StructuralTerm> {
    use numerics::bignum::BigInt;
    use numerics::literals::{FloatFormat, FloatLiteral};
    let width = u32::try_from(source.scalar_byte_size()?).ok()? * 8;
    if source.scalar_byte_size() != target.scalar_byte_size() || width > 64 || width == 0 {
        return None;
    }
    let bits = match value.semantic_value() {
        StructuralTerm::Integer(value) if source.is_fixed_integer() => {
            if source.is_signed_integer() {
                value.to_i64()? as u64
            } else {
                value.to_u64()?
            }
        }
        StructuralTerm::Float(value) => {
            let format = match source {
                PrimitiveType::F32 => FloatFormat::F32,
                PrimitiveType::F64 => FloatFormat::F64,
                _ => return None,
            };
            if value.landing().is_some_and(|landing| landing != format) {
                return None;
            }
            if let Some((presented_format, bits)) = value.presented_bits() {
                if presented_format != format {
                    return None;
                }
                bits
            } else {
                match format {
                    FloatFormat::F32 if !value.value_f32().is_nan() => {
                        u64::from(value.value_f32().to_bits())
                    }
                    FloatFormat::F64 if !value.value_f64().is_nan() => value.value_f64().to_bits(),
                    _ => return None,
                }
            }
        }
        _ => return None,
    } & (u64::MAX >> (64 - width));
    Some(match target {
        PrimitiveType::F32 => {
            StructuralTerm::Float(FloatLiteral::from_bits(FloatFormat::F32, bits)?)
        }
        PrimitiveType::F64 => {
            StructuralTerm::Float(FloatLiteral::from_bits(FloatFormat::F64, bits)?)
        }
        primitive if primitive.is_fixed_integer() => {
            StructuralTerm::Integer(if primitive.is_signed_integer() {
                let shift = 64 - width;
                BigInt::from_i64(((bits << shift) as i64) >> shift)
            } else {
                BigInt::from_u64(bits)
            })
        }
        _ => return None,
    })
}
