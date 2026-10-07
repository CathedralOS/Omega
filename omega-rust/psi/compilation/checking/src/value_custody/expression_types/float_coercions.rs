//! Exact format coercions preserve the operand's denotation before elaboration.
//!
//! A destination-owned format conversion is total and may round, so selecting
//! it cannot discharge an authored `as`. Widening preserves every binary32
//! value; narrowing needs evidence about the source value, independently of
//! the receiving type. Declaration-backed result types provide the same facts
//! for parameters, locals, fields and selected calls. A singleton enclosure
//! proves narrowing only when conversion preserves that exact IEEE meaning.
//! Merely checking two representable endpoints would admit lossy values between
//! them. Non-singleton binary64 ranges need stronger representability evidence
//! than the range type alone supplies and remain unproved here.

use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use numerics::float_semantics::{FloatFormat, FloatMeaning, FloatSemantics};
use semantic_vocabulary::IeeeFloatValue;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionNode, TableCastExpression};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

/// Discharge format-changing Exact casts while the authored operand still
/// exists. The binder calls this before replacing a cast by its requirement
/// call; ordinary expression validation uses it for casts that remain casts.
pub(crate) fn validate_exact_float_format_cast(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    cast: &TableCastExpression,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if cast.form.is_recast() || cast.domain != ArithmeticDomain::Exact {
        return true;
    }
    let Some(source_reference) =
        super::expression_result_type_reference(program, machine, state, cast.value)
    else {
        return true;
    };
    if program.primitive_type_reference(source_reference) != Some(PrimitiveType::F64)
        || program.primitive_type_reference(cast.target_type) != Some(PrimitiveType::F32)
    {
        // Same-carrier casts do not convert; binary32 -> binary64 is total.
        // Integer conversion conditions remain with their own proof owner.
        return true;
    }
    let source_value = match program.expression_table.expression(cast.value) {
        ExpressionNode::Float(literal) => Some(literal.landed_f64()),
        _ => singleton_float_value(program, source_reference),
    };
    if source_value.is_some_and(|value| {
        let meaning = FloatMeaning::from_f64(value);
        FloatSemantics::convert(FloatFormat::BINARY32, &meaning) == meaning
    }) {
        return true;
    }
    diagnostics.push(
        Diagnostic::error(format!(
            "machine `{}` state `{}` cannot prove Exact float-to-`f32` cast operand `{}` \
             preserves its denotation; prove exact representability or use the named \
             rounded conversion `F32::from_f64`",
            machine.name,
            state.name,
            program.expression_table.display_name(cast.value),
        ))
        .with_source_span(program.expression_table.source_span(cast.value)),
    );
    false
}

fn singleton_float_value(program: &TypedTrees, mut reference: TypeReferenceHandle) -> Option<f64> {
    let primitive = program.primitive_type_reference(reference)?;
    let mut low = f64::NEG_INFINITY;
    let mut high = f64::INFINITY;
    while reference.is_valid() {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint in program.type_reference_table.constraints(*constraints) {
                    if let TypeConstraintNode::Range {
                        minimum, maximum, ..
                    } = constraint
                    {
                        low = low.max(endpoint_value(crate::closed_float_range_endpoint(
                            program, *minimum, primitive,
                        )?));
                        high = high.min(endpoint_value(crate::closed_float_range_endpoint(
                            program, *maximum, primitive,
                        )?));
                    }
                }
                reference = *base_type;
            }
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            _ => break,
        }
    }
    // Ordered finite endpoints exclude NaN. The enclosure may contain either
    // signed zero; both signs independently survive an exact format conversion.
    (low.is_finite() && low == high).then_some(low)
}

fn endpoint_value(value: IeeeFloatValue) -> f64 {
    match value {
        IeeeFloatValue::Binary32(bits) => f64::from(f32::from_bits(bits)),
        IeeeFloatValue::Binary64(bits) => f64::from_bits(bits),
    }
}
