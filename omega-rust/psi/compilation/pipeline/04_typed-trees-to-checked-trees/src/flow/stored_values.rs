//! Associate resident storage with its exact completed producer. This is a
//! value association, not permission to copy, retain a loan, or grant custody.
//! Consumers select the write relation they need: descriptor identity survives
//! descendant writes, while correspondence of contents does not. Missing rows
//! and unknown writes never fall back to a historical initializer.

use arena::{Handle, HandleSpan};
use checked_trees::{FlowControlFacts, FlowOperandFact, FlowOperandReferent, FlowStatementFact};
use validation::CanonicalPlace;

pub(crate) enum StoredOperand {
    Unavailable,
    Value {
        capture: Handle<FlowOperandFact>,
        reference: typed_trees::types::TypeReferenceHandle,
        projection: Vec<facts::PlaceSegment>,
    },
}

#[derive(Clone, Copy)]
pub(crate) enum StoredObservation {
    Binding,
    Contents,
}

pub(crate) fn before_statement(
    program: &dyn validation::CheckingInput,
    semantic: &facts::FactPlan,
    control: &FlowControlFacts,
    state: &typed_trees::state::State,
    statements: HandleSpan<FlowStatementFact>,
    before: usize,
    writes: HandleSpan<FlowOperandReferent>,
    unknown_write: bool,
    source: &CanonicalPlace,
    observation: StoredObservation,
) -> Option<StoredOperand> {
    let stable = match observation {
        StoredObservation::Binding => super::operand_contexts::binding_origin_is_stable,
        StoredObservation::Contents => super::operand_contexts::entry_origin_is_stable,
    };
    if !stable(program, control, writes, unknown_write, source) {
        return Some(StoredOperand::Unavailable);
    }
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let end = order.position(before).or_else(|| {
        (before
            == program
                .statement_table
                .statements(state.statement_nodes)
                .len())
        .then_some(order.indices().len())
    })?;
    for &statement_index in order.indices()[..end].iter().rev() {
        let Some(statement) = control
            .statements
            .span_or_empty(statements)
            .iter()
            .find(|statement| statement.statement_index == statement_index)
        else {
            return Some(StoredOperand::Unavailable);
        };
        if semantic.places.is_valid(statement.stored_destination)
            && control.operands.is_valid(statement.stored_value)
        {
            let Some(destination) = super::canonical_place_from_semantic_place(
                program,
                semantic,
                semantic.places.get(statement.stored_destination),
            ) else {
                return Some(StoredOperand::Unavailable);
            };
            if destination.root == source.root
                && destination.segments.len() <= source.segments.len()
                && destination
                    .segments
                    .iter()
                    .zip(&source.segments)
                    .all(|(left, right)| {
                        super::canonical_place_segments_equal(program, *left, *right)
                    })
            {
                let stored = control.operands.get(statement.stored_value);
                return Some(StoredOperand::Value {
                    capture: statement.stored_value,
                    reference: stored.type_reference,
                    projection: source.segments[destination.segments.len()..].to_vec(),
                });
            }
        }
        let (writes, unknown) = match observation {
            StoredObservation::Binding => (
                statement.completed_writes,
                statement.completed_unknown_write,
            ),
            StoredObservation::Contents => (
                statement.completed_content_writes,
                statement.completed_content_unknown_write,
            ),
        };
        if !stable(program, control, writes, unknown, source) {
            return Some(StoredOperand::Unavailable);
        }
    }
    None
}
