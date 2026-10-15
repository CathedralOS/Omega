//! Join the bounds check with the byte classes the assignment transfer retained.
//! A write's Terminal obligation contains both; a bounds verdict alone cannot
//! authorize overwriting an ASCII byte with 255. Read only the completed
//! statement's live contexts, never a historical fact or a later restoration.
//! Finite reference choices require that judgment for every captured backing,
//! independently of whether one candidate already had the desired byte class.
use checked_trees::CheckFacts;
use facts::{FactOrigin, FactPayload};

use typed_trees::statement::StatementNode;

pub(super) fn record(program: &dyn ::validation::CheckingInput, facts: &mut CheckFacts) {
    let mut proven = Vec::new();
    for (_, state_flow) in facts.flow.control.states.iter() {
        let Some(state) = program.state_by_symbol(state_flow.state_symbol) else {
            continue;
        };
        let statements = program.statement_table.statements(state.statement_nodes);
        for statement_flow in facts
            .flow
            .control
            .statements
            .span_or_empty(state_flow.statements)
        {
            let Some(StatementNode::Assignment(assignment)) =
                statements.get(statement_flow.statement_index)
            else {
                continue;
            };
            if !facts.scalar_index_is_proven(assignment.target) {
                continue;
            }
            let Some(indexed) = program.expression_table.indexed_node(assignment.target) else {
                continue;
            };
            let Some(carrier) = validation::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement_flow.statement_index,
                indexed.collection,
            ) else {
                continue;
            };
            let Some(reference) = validation::canonical_place_type_reference(
                program,
                state.symbol,
                statement_flow.statement_index,
                &carrier,
            ) else {
                continue;
            };
            if validation::byte_sequence_carrier(program, reference, &[]).is_none()
                && !matches!(program.type_reference_table.type_reference(
                    program.type_reference_table.peeled_type_reference(reference)),
                    typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. }
                        if program.primitive_type_reference(*element_type) == Some(typed_trees::types::PrimitiveType::U8))
            {
                continue;
            }
            // The destination operand captured every possible referent before
            // RHS evaluation. Prove preservation on that complete roster,
            // never on one convenient candidate or a replayed alias spelling.
            let store_target = statement_flow.store_target;
            if !facts.flow.control.operands.is_valid(store_target) {
                continue;
            }
            let operand = facts.flow.control.operands.get(store_target);
            let Some(referents) = facts.flow.control.operand_referents.span(operand.referents)
            else {
                continue;
            };
            if referents.is_empty() {
                continue;
            }
            let carriers = referents
                .iter()
                .map(|referent| {
                    let segments = facts
                        .flow
                        .control
                        .operand_referent_segments
                        .span(referent.segments)?;
                    let (
                        facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. },
                        parent,
                    ) = segments.split_last()?
                    else {
                        return None;
                    };
                    Some(validation::CanonicalPlace {
                        root: referent.root,
                        segments: parent.to_vec(),
                    })
                })
                .collect::<Option<Vec<_>>>();
            let Some(carriers) = carriers else {
                continue;
            };
            let declared_domains =
                validation::field_domains::domain_constraint_symbols(program, reference);
            let preserved = carriers.iter().all(|carrier| {
                let Some(candidate_type) = validation::canonical_place_type_reference(
                    program,
                    state.symbol,
                    statement_flow.statement_index,
                    carrier,
                ) else {
                    return false;
                };
                let mut domains = declared_domains.clone();
                domains.extend(validation::field_domains::domain_constraint_symbols(
                    program,
                    candidate_type,
                ));
                domains.into_iter().all(|domain| {
                    let Some(required) =
                        typed_trees::byte_predicates::domain_byte_predicate(program, domain)
                    else {
                        return false;
                    };
                    facts
                        .flow
                        .contexts
                        .semantic_context_refs
                        .span_or_empty(statement_flow.exit_semantic_contexts)
                        .iter()
                        .flat_map(|reference| {
                            facts
                                .semantic
                                .context_view(facts.semantic.contexts.get(reference.context))
                                .facts()
                        })
                        .any(|fact| {
                            let Some(place) = fact.place.as_place() else {
                                return false;
                            };
                            let FactPayload::BytePredicate { predicate } = fact.payload else {
                                return false;
                            };
                            fact.origin == FactOrigin::StatementTransfer
                                && fact.point
                                    == facts::ProgramPoint::Statement {
                                        machine_symbol: state_flow.machine_symbol,
                                        state_symbol: state.symbol,
                                        statement_index: statement_flow.statement_index,
                                    }
                                && predicate.implies(required)
                                && crate::flow::canonical_place_from_semantic_place(
                                    program,
                                    &facts.semantic,
                                    facts.semantic.places.get(place),
                                )
                                .as_ref()
                                    == Some(carrier)
                        })
                })
            });
            if preserved {
                proven.push(assignment.target);
            }
        }
    }
    proven.sort_by_key(|expression| checked_trees::operators::handle_position(*expression));
    proven.dedup();
    facts.proven_byte_writes = proven;
}

/// Key each proven `[]` occurrence of an assignment target by the
/// `AssignmentIndex` depth plans name it with, so lowering decides a runtime
/// element's bound from that coordinate alone.
pub(super) fn record_assignment_indexes(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
) {
    let mut proven = Vec::new();
    for (_, state_flow) in facts.flow.control.states.iter() {
        let Some(state) = program.state_by_symbol(state_flow.state_symbol) else {
            continue;
        };
        let statements = program.statement_table.statements(state.statement_nodes);
        for statement_flow in facts
            .flow
            .control
            .statements
            .span_or_empty(state_flow.statements)
        {
            let Some(StatementNode::Assignment(assignment)) =
                statements.get(statement_flow.statement_index)
            else {
                continue;
            };
            let Ok(statement_index) = u32::try_from(statement_flow.statement_index) else {
                continue;
            };
            for (depth, occurrence) in
                validation::assignment_target_indexes(&program.expression_table, assignment.target)
                    .into_iter()
                    .enumerate()
            {
                if let Ok(depth) = u32::try_from(depth)
                    && facts.scalar_index_is_proven(occurrence)
                {
                    proven.push(checked_trees::ProvenAssignmentIndex {
                        state: state.symbol,
                        statement_index,
                        depth,
                        coordinate: facts.scalar_index_coordinates.iter().find_map(
                            |(expression, coordinate)| {
                                (*expression == occurrence).then_some(*coordinate)
                            },
                        ),
                    });
                }
            }
        }
    }
    proven.sort_by_key(checked_trees::ProvenAssignmentIndex::key);
    proven.dedup();
    facts.proven_assignment_indexes = proven;
}
