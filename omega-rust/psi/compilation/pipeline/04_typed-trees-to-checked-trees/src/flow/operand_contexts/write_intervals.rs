//! Content-write prefixes restart at each statement. Comparing two prefixes
//! without their statement coordinates can overlook a replacement in between,
//! or mistake a pre-read write for a later mutation. One directed interval
//! joins the producer's remaining writes, complete intervening statements and
//! the consumer's completed prefix. Actual observations and ordinary operands
//! share this chronology; their owners supply the storage-overlap judgment.

use arena::HandleSpan;
use checked_trees::{FlowControlFacts, FlowOperandFact, FlowOperandReferent, FlowStatementFact};
use symbols::SymbolHandle;

#[derive(Clone, Copy)]
pub(in crate::flow) struct ContentObservationPoint {
    pub state: SymbolHandle,
    pub statement: usize,
    pub writes: HandleSpan<FlowOperandReferent>,
    pub unknown_write: bool,
}

impl ContentObservationPoint {
    pub(super) fn from_operand(operand: &FlowOperandFact) -> Self {
        Self {
            state: operand.state_symbol,
            statement: operand.statement_index,
            writes: operand.entry_origin_writes,
            unknown_write: operand.entry_origin_unknown_write,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::flow) fn contents_observation_survives(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    statements: HandleSpan<FlowStatementFact>,
    first: ContentObservationPoint,
    second: ContentObservationPoint,
    place: &validation::CanonicalPlace,
    write_changes: super::SourceWriteRelation,
) -> bool {
    contents_writes_between(program, control, statements, first, second).is_some_and(|writes| {
        writes.into_iter().all(|(_, write)| {
            place.root != write.root
                || control
                    .operand_referent_segments
                    .span(write.segments)
                    .is_some_and(|segments| !write_changes(program, place, segments))
        })
    })
}

pub(super) fn contents_writes_between<'facts>(
    program: &dyn validation::CheckingInput,
    control: &'facts FlowControlFacts,
    statements: HandleSpan<FlowStatementFact>,
    first: ContentObservationPoint,
    second: ContentObservationPoint,
) -> Option<Vec<(usize, &'facts FlowOperandReferent)>> {
    if first.state != second.state || first.unknown_write || second.unknown_write {
        return None;
    }
    let mut writes = Vec::new();
    if first.statement == second.statement {
        writes.extend(
            super::write_prefix_extension(control, first.writes, second.writes)?
                .iter()
                .map(|write| (first.statement, write)),
        );
        return Some(writes);
    }
    let state = program.state_by_symbol(first.state)?;
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let start = order.position(first.statement)?;
    let end = order.position(second.statement)?;
    if start >= end {
        return None;
    }
    let statements = control.statements.span(statements)?;
    for (position, index) in order.indices()[start..end].iter().enumerate() {
        let mut matching = statements
            .iter()
            .filter(|row| row.statement_index == *index);
        let statement = matching.next()?;
        if matching.next().is_some() || statement.completed_content_unknown_write {
            return None;
        }
        let remaining = if position == 0 {
            super::write_prefix_extension(
                control,
                first.writes,
                statement.completed_content_writes,
            )?
        } else {
            control
                .operand_origin_writes
                .span(statement.completed_content_writes)?
        };
        writes.extend(remaining.iter().map(|write| (*index, write)));
    }
    writes.extend(
        control
            .operand_origin_writes
            .span(second.writes)?
            .iter()
            .map(|write| (second.statement, write)),
    );
    Some(writes)
}
