//! Exact float coercions preserve the operand's denotation at the checked use.
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
//! Integer destinations additionally require every enclosed source value to
//! be integral and representable; named truncation is a weaker operation.
//! This applies under every arithmetic policy: the suffix changes future
//! operations, not the source payload or the coercion's proof obligation.
//! The cast binder retains the authored target on its derived call so this
//! check shares the ordinary walk's guard and write lifetime, not a second
//! initializer or guard replay before binding.
//! Integer sources use the shared integer analysis. Every integer in the
//! contiguous precision band is exact; outside it only a proved singleton
//! can establish representability here. Compare its mathematical integer with
//! the exact rounded meaning, not an already-rounded host float.

use diagnostics::Diagnostic;
use numerics::float_semantics::{FloatFormat, FloatMeaning, FloatSemantics};
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

/// Discharge the authored coercion at an unchanged cast or its derived call.
/// The target is an obligation, never a premise about the operand.
pub(crate) fn validate_exact_float_coercion(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    operand: typed_trees::expression::ExpressionHandle,
    target: TypeReferenceHandle,
    environment: &crate::proof_contracts::arithmetic_domains::ValueEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let source = super::expression_result_type_reference(program, machine, state, operand)
        .and_then(|reference| program.primitive_type_reference(reference));
    let Some(source) = source else {
        return true;
    };
    let Some(target) = program.primitive_type_reference(target) else {
        return true;
    };
    if numerics::float_semantics_catalog::IntegerCarrier::from_spelling(source.name()).is_some()
        && matches!(target, PrimitiveType::F32 | PrimitiveType::F64)
    {
        let format = if target == PrimitiveType::F32 {
            FloatFormat::BINARY32
        } else {
            FloatFormat::BINARY64
        };
        let proven = crate::proof_contracts::arithmetic_domains::integer_source_bounds(
            program,
            machine,
            state,
            operand,
            source,
            environment,
            diagnostics,
        )
        .is_some_and(|(minimum, maximum)| {
            let exact_limit = 1i128 << format.precision;
            if minimum >= -exact_limit && maximum <= exact_limit {
                true
            } else if minimum == maximum {
                let integer = numerics::bignum::BigInt::from_i128(minimum);
                FloatSemantics::from_integer(format, &integer)
                    == FloatMeaning::FiniteNonZero(numerics::bignum::BigRational::from_integer(
                        integer,
                    ))
            } else {
                false
            }
        });
        if !proven {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` cannot prove integer-to-`{}` cast operand `{}` \
                 preserves its denotation; prove exact format representability or use a named conversion to round",
                machine.name, state.name, target.name(), program.expression_table.display_name(operand),
            )).with_source_span(program.expression_table.source_span(operand)));
        }
        return proven;
    }
    if !matches!(source, PrimitiveType::F32 | PrimitiveType::F64) {
        return true;
    }
    if source == target || (source == PrimitiveType::F32 && target == PrimitiveType::F64) {
        return true;
    }
    if !target.accepts_integer_literal() && target != PrimitiveType::F32 {
        return true;
    }
    let bounds = crate::proof_contracts::arithmetic_domains::float_source_bounds(
        program,
        machine,
        Some(state),
        environment,
        operand,
    );
    let proven = bounds.is_some_and(|bounds| {
        if target.accepts_integer_literal() {
            bounds.is_exact_integer(target)
        } else if bounds.minimum == bounds.maximum {
            let meaning = FloatMeaning::from_f64(bounds.minimum);
            FloatSemantics::convert(FloatFormat::BINARY32, &meaning) == meaning
        } else {
            false
        }
    });
    if proven {
        return true;
    }
    diagnostics.push(
        Diagnostic::error(format!(
            "machine `{}` state `{}` cannot prove Exact float-to-`{}` cast operand `{}` \
             preserves its denotation; prove an integral representable value for an integer \
             target, or exact format representability; use a named conversion to round or truncate",
            machine.name,
            state.name,
            target.name(),
            program.expression_table.display_name(operand),
        ))
        .with_source_span(program.expression_table.source_span(operand)),
    );
    false
}
