//! Current relational formulas bind the storage actually observed by Flow.
//! Cached scalar algebra carries source occurrences only. A completed structural
//! read supplies its address and content epoch; neither a later selector value
//! nor an equal-looking source path can replace that observation.

use super::super::FlowBuildContext;
use arena::HandleSpan;
use checked_trees::{CheckedScalarExpression, FlowOperandReferent};
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;
use validation::CanonicalPlace;

pub(in crate::flow) struct ScalarObservation<'flow> {
    pub place: CanonicalPlace,
    pub reference: TypeReferenceHandle,
    pub read: Option<&'flow checked_trees::FlowValueObservationFact>,
}

/// Resolve the producer's actual selected geometry once. A zero observation
/// belongs to declared/formal algebra and never enters a saved-copy namespace.
/// A nonzero but unavailable occurrence is unknown, not a declaration fallback.
pub(in crate::flow) fn scalar_observation_at<'flow>(
    program: &dyn validation::CheckingInput,
    build: &'flow FlowBuildContext<'_>,
    state: SymbolHandle,
    statement_index: usize,
    operand: &CheckedScalarExpression,
) -> Option<ScalarObservation<'flow>> {
    let (place, reference, read) = match operand {
        CheckedScalarExpression::StructuralParameterIndexedRead { observation, .. }
        | CheckedScalarExpression::StructuralParameterField { observation, .. }
            if observation.is_valid() =>
        {
            if !program.expression_table.expression_is_valid(*observation) {
                return None;
            }
            let statements = super::super::reference_spans::appended_span_since(
                &build.control.statements,
                build.state_statements_start,
            );
            let mut matching = build
                .control
                .statements
                .span(statements)?
                .iter()
                .filter(|statement| statement.statement_index == statement_index)
                .flat_map(|statement| {
                    build
                        .control
                        .value_observations
                        .span_or_empty(statement.value_observations)
                })
                .filter(|read| read.expression == *observation);
            let read = matching.next()?;
            if matching.next().is_some() || !build.control.operands.is_valid(read.receiver) {
                return None;
            }
            let receiver = build.control.operands.get(read.receiver);
            if receiver.state_symbol != state || receiver.statement_index != statement_index {
                return None;
            }
            let [subject] = build.control.operand_referents.span(read.places)? else {
                // A union of possible addresses is not one scalar observation.
                return None;
            };
            let source = CanonicalPlace {
                root: subject.root,
                segments: build
                    .control
                    .operand_referent_segments
                    .span(subject.segments)?
                    .to_vec(),
            };
            let reference = validation::expression_type_reference_in_state(
                program,
                state,
                statement_index,
                *observation,
            )?;
            let reference = validation::field_domains::readable_type_reference(program, reference)?;
            (source, reference, Some(read))
        }
        _ => {
            let (place, reference) = crate::facts::boolean_predicates::declared_observation(
                program,
                state,
                statement_index,
                operand,
            )?;
            (place, reference, None)
        }
    };
    Some(ScalarObservation {
        place,
        reference,
        read,
    })
}

#[allow(clippy::too_many_arguments)]
pub(in crate::flow) fn current_scalar_observation(
    program: &dyn validation::CheckingInput,
    build: &FlowBuildContext<'_>,
    state: SymbolHandle,
    statement_index: usize,
    writes: HandleSpan<FlowOperandReferent>,
    unknown_write: bool,
    operand: &CheckedScalarExpression,
) -> Option<(CanonicalPlace, TypeReferenceHandle)> {
    let ScalarObservation {
        place,
        reference,
        read,
    } = scalar_observation_at(program, build, state, statement_index, operand)?;
    if let Some(read) = read {
        let machine = program.machine_holding_state(state)?;
        let state_definition = program.state_in_machine(machine, state)?;
        // Captured fixed coordinates and immutable selector identities denote
        // exact storage. An unresolved mutable coordinate cannot establish a
        // current value by being rendered the same way at two evaluations.
        if !super::super::stable_case_subject(
            program,
            machine,
            state_definition,
            statement_index,
            &place,
        ) || !super::super::operand_contexts::observation_prefixes_agree(
            program,
            &build.control,
            read.origin_writes,
            read.origin_unknown_write,
            writes,
            unknown_write,
            &place,
        ) {
            return None;
        }
    }
    Some((place, reference))
}
