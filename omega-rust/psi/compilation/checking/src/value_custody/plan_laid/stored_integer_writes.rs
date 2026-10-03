//! Physical integer storage admission at the ordinary assignment boundary.
//!
//! Resolve the field through the shared place query, then join its declaration
//! identity to the checked layout. The origin of the receiver (parameter,
//! local, or recast view) does not select a different write rule. Analyze the
//! value before invalidating assignment facts; no Terminal certificate search
//! should be necessary to discover an unrepresentable source write.

use super::stored_integer_bounds;
use crate::proof_contracts::arithmetic_domains::Interval;
use crate::value_custody::places;
use checked_trees::values::CheckedStoredIntegerWrite;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::TableAssignment;

pub(crate) fn validate_stored_integer_write(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    assignment: &TableAssignment,
    interval: Interval,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedStoredIntegerWrite> {
    let field = places::declared_case_projection_field(program, machine, state, assignment.target)?;
    let (plan, field_index) = program.plan_laid_layouts.iter().find_map(|plan| {
        plan.field_symbols
            .iter()
            .position(|symbol| *symbol == field.symbol)
            .map(|field_index| (plan, field_index))
    })?;
    let integer = plan
        .integer_fields
        .iter()
        .find(|integer| integer.field_index == field_index)?;
    let (minimum, maximum) =
        stored_integer_bounds(u64::from(integer.stored_width_bits), integer.interpretation)?;
    // A total storage mapping needs no finite interval from the i64-backed
    // range engine (in particular for the full u64 carrier). Otherwise both
    // concrete bounds must be established; unknown is not permission to narrow.
    let fits = integer.write_is_total
        || (interval.low.is_some_and(|low| i128::from(low) >= minimum)
            && interval
                .high
                .is_some_and(|high| i128::from(high) <= maximum));
    if !fits {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} cannot prove assigned value fits integer storage for field `{}`: required {minimum}..={maximum}",
            field.name,
        )));
        return None;
    }
    Some(CheckedStoredIntegerWrite {
        target: assignment.target,
        value: assignment.value,
        field: field.symbol,
        minimum,
        maximum,
    })
}

#[cfg(test)]
mod tests {
    use super::stored_integer_bounds;
    use layout_plans::IntegerInterpretation::{Signed, Unsigned};

    #[test]
    fn stored_integer_bounds_preserve_signed_and_full_unsigned_carriers() {
        assert_eq!(stored_integer_bounds(8, Signed), Some((-128, 127)));
        assert_eq!(stored_integer_bounds(8, Unsigned), Some((0, 255)));
        assert_eq!(
            stored_integer_bounds(64, Signed),
            Some((i128::from(i64::MIN), i128::from(i64::MAX)))
        );
        assert_eq!(
            stored_integer_bounds(64, Unsigned),
            Some((0, i128::from(u64::MAX)))
        );
    }

    #[test]
    fn stored_integer_bounds_refuse_invalid_widths_before_shifting() {
        for width in [0, 65, u64::MAX] {
            for interpretation in [Signed, Unsigned] {
                assert_eq!(stored_integer_bounds(width, interpretation), None);
            }
        }
    }
}
