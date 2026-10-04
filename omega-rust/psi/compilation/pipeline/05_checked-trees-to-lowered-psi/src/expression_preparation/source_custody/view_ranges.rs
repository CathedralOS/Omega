//! Computed view ranges retain exact array backing or a view and its borrow row.
//! Range replay validates operator meaning and endpoint custody without running
//! either endpoint. Descriptor production belongs to ordered call evaluation.
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedTrees;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};

pub(crate) fn captured_range(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
    expression: ExpressionHandle,
) -> Result<checked_trees::CapturedPlace, LoweringError> {
    let operation = validation::subslice_expression(&checked.typed, expression).ok_or(
        LoweringError::Unsupported("computed subslice lost its readable range expression"),
    )?;
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(operation) else {
        return unsupported("computed subslice lost its indexed source");
    };
    let ExpressionNode::Range(range) = checked.expression_table.expression(indexed.index) else {
        return unsupported("computed subslice lost its range");
    };
    let mut captured =
        crate::emission::call_source_custody::projected_receivers::observation_place(
            checked,
            machine,
            state,
            statement as usize,
            indexed.collection,
        )?;
    // The borrow ledger canonicalizes closed constant bounds as a fixed
    // half-open range. An omitted end still observes the runtime view length,
    // so it retains the authored index identity rather than a guessed extent.
    let fixed_bounds = (|| {
        let start = if range.start.is_valid() {
            usize::try_from(
                checked
                    .expression_table
                    .constant_integer_value(range.start)?,
            )
            .ok()?
        } else {
            0
        };
        if !range.end.is_valid() {
            return None;
        }
        let mut end =
            usize::try_from(checked.expression_table.constant_integer_value(range.end)?).ok()?;
        if range.end_inclusive {
            end = end.checked_add(1)?;
        }
        Some((start, end))
    })();
    let segment = match fixed_bounds {
        Some((start, end)) => facts::PlaceSegment::FixedRange { start, end },
        None => facts::PlaceSegment::Index {
            expression: indexed.index,
        },
    };
    captured.segments.push(segment);
    Ok(captured)
}
