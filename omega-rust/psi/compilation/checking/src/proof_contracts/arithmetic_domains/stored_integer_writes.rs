//! Concrete stores preserve the semantic integer while fitting its layout width.

use super::integer_ranges::known_u64_value;
use super::{Interval, ValueEnvironment};
use crate::value_custody::{expression_types, places};
use checked_trees::CheckedStoredIntegerWrite;
use diagnostics::Diagnostic;
use semantic_vocabulary::{IntegerSign, IntegerType};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::PrimitiveType;

#[cfg(test)]
mod tests;

/// Discharge against the pre-store environment. The returned width belongs to
/// this exact destination/value occurrence; it is not a changed semantic type.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_stored_integer_write(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    destination: ExpressionHandle,
    value: ExpressionHandle,
    interval: Interval,
    primitive: Option<PrimitiveType>,
    environment: &ValueEnvironment,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedStoredIntegerWrite> {
    let state = state?;
    let ExpressionNode::Member(member) = program.expression_table.expression(destination) else {
        return None;
    };
    let receiver = expression_types::expression_result_type_reference(
        program,
        machine,
        state,
        member.receiver,
    )?;
    let carrier =
        crate::reference_shell(program, receiver).map_or(receiver, |reference| reference.referee);
    let data = places::data_definition_for_type(program, carrier)?;
    let plan = program
        .plan_laid_layouts
        .iter()
        .find(|plan| plan.data_symbol == data.symbol)?;
    if plan.integer_fields.is_empty() {
        return None;
    }
    let field = places::exact_data_member_field(
        program,
        data,
        member.member_symbol,
        member.member.as_str(),
        member.case_variant.as_ref().map(|variant| variant.as_str()),
    );
    if let Some(field) = field {
        let index = plan
            .field_symbols
            .iter()
            .position(|symbol| *symbol == field.symbol);
        if index.is_some_and(|index| {
            !plan
                .integer_fields
                .iter()
                .any(|integer| integer.field_index == index)
        }) {
            return None;
        }
    }
    let projection = field
        .and_then(|field| crate::recast_scalar_projection(program, carrier, &[], field.symbol));
    let Some(projection) = projection else {
        diagnostics.push(Diagnostic::error(format!(
            "IntegerAt store in {owner} has no exact stored field geometry"
        )));
        return None;
    };
    let stored = projection.stored_integer?;
    let fits = if let Some(value) = known_u64_value(program, environment, value) {
        fits_bounds(stored, i128::from(value), i128::from(value))
    } else {
        interval_fits(stored, interval, primitive)
    };
    if !fits {
        diagnostics.push(Diagnostic::error(format!(
            "IntegerAt store in {owner} requires a value proven to fit {}-bit {} storage",
            stored.bits(),
            match stored.sign() {
                IntegerSign::Signed => "signed",
                IntegerSign::Unsigned => "unsigned",
            },
        )));
        return None;
    }
    Some(CheckedStoredIntegerWrite {
        machine: machine.symbol,
        state: state.symbol,
        destination,
        value,
        field: field?.symbol,
        stored_type: stored,
    })
}

fn interval_fits(
    stored: IntegerType,
    interval: Interval,
    primitive: Option<PrimitiveType>,
) -> bool {
    // A typed value stays within its semantic carrier even when the interval
    // abstraction has an unbounded endpoint (notably the upper half of u64).
    // Physical storage never supplies this missing semantic bound.
    let carrier = primitive
        .filter(|primitive| primitive.accepts_integer_literal())
        .and_then(|primitive| {
            let bits = primitive.scalar_byte_size()?.checked_mul(8)?;
            Some(if primitive.is_signed_integer() {
                (-(1_i128 << (bits - 1)), (1_i128 << (bits - 1)) - 1)
            } else {
                (0, (1_i128 << bits) - 1)
            })
        });
    let low = match (interval.low().map(i128::from), carrier) {
        (Some(low), Some((minimum, _))) => Some(low.max(minimum)),
        (low, None) => low,
        (None, Some((minimum, _))) => Some(minimum),
    };
    let high = match (interval.high().map(i128::from), carrier) {
        (Some(high), Some((_, maximum))) => Some(high.min(maximum)),
        (high, None) => high,
        (None, Some((_, maximum))) => Some(maximum),
    };
    low.zip(high)
        .is_some_and(|(low, high)| low <= high && fits_bounds(stored, low, high))
}

fn fits_bounds(stored: IntegerType, low: i128, high: i128) -> bool {
    let bits = stored.bits();
    // IntegerAt currently admits whole-byte widths through 64 bits. Reject an
    // unsupported geometry instead of overflowing the bound calculation.
    if bits == 0 || bits > 64 || bits % 8 != 0 {
        return false;
    }
    let (minimum, maximum) = match stored.sign() {
        IntegerSign::Signed => (-(1_i128 << (bits - 1)), (1_i128 << (bits - 1)) - 1),
        IntegerSign::Unsigned => (0, (1_i128 << bits) - 1),
    };
    low >= minimum && high <= maximum
}
