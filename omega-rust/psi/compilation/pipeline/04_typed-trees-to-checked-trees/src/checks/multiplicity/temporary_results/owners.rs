//! The anonymous owners one statement's calls consume. A producer completes
//! before its consumer: a construction establishes its value, and a call
//! result is established by its call's completion. One source evaluation
//! inventory supplies named/anonymous permission production and the exact loan
//! continuation; executable admission consumes that published roster.
use super::CheckFacts;
use crate::checks::multiplicity::linear_obligations::type_reference_is_reference;
use crate::checks::type_carries_linear_obligation;
use language_semantics::PermissionEventSource;
use typed_trees::expression::ExpressionNode;

/// How the consuming call uses the anonymous owner.
pub(crate) enum TemporaryConsumption {
    /// An evaluated loan retains the whole producing owner, even when its
    /// operand selects a child; the owner dies after the call.
    SharedBorrow {
        borrowed_path: Vec<facts::PlaceSegment>,
    },
    /// The call receives the completed owner whole, with every claim its
    /// producer established or its operands moved into it.
    OwnedTransfer,
}

pub(crate) struct TemporaryOwner {
    pub(crate) expression: typed_trees::expression::ExpressionHandle,
    pub(crate) type_reference: typed_trees::types::TypeReferenceHandle,
    pub(crate) consumption: TemporaryConsumption,
    pub(crate) evaluation: checked_trees::FlowPermissionEvaluation,
    pub(crate) producer: PermissionEventSource,
    pub(crate) consumer: PermissionEventSource,
}

/// The claim-bearing type of a by-value operand an anonymous producer
/// completes. Its claims are established at the producer, not borrowed from
/// another owner. A conversion or selection that conserves an existing owner
/// (a qualification cast, a dispatch result) is not such a producer and keeps
/// its source custody; claim-free operands keep the affine temporary rules.
pub(crate) fn owned_temporary_type(
    program: &dyn ::validation::CheckingInput,
    state_symbol: symbols::SymbolHandle,
    statement_index: usize,
    parameter: &typed_trees::signature::StateParameter,
    argument: typed_trees::expression::ExpressionHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    if parameter.is_const
        || parameter.relevance.is_erased()
        || type_reference_is_reference(program, parameter.type_reference)
        || !matches!(
            program.expression_table.expression(argument),
            ExpressionNode::StructLiteral(_)
                | ExpressionNode::ArrayLiteral(_)
                | ExpressionNode::Call(_)
        )
    {
        return None;
    }
    let place = validation::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        argument,
    )?;
    if place.root != facts::PlaceRoot::Expression(argument) || !place.segments.is_empty() {
        return None;
    }
    let type_reference =
        validation::canonical_place_type_reference(program, state_symbol, statement_index, &place)?;
    type_carries_linear_obligation(program, type_reference).then_some(type_reference)
}

pub(crate) fn temporary_owners(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    calls: &[checked_trees::FlowCallFact],
) -> Vec<TemporaryOwner> {
    let completed = program
        .statement_table
        .indexed_statements(state.statement_nodes)
        .map(|(statement_index, statement)| {
            crate::checks::ownership_occurrences::statement_steps(
                program,
                machine,
                state,
                statement_index,
                statement,
                &[],
                &facts.operators,
            )
            .into_iter()
            .filter_map(|occurrence| match occurrence.step {
                crate::checks::ownership_occurrences::StatementStep::Complete(expression) => {
                    Some(expression)
                }
                _ => None,
            })
            .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut owners = Vec::new();
    for consumer in calls {
        if facts.flow.control.is_retired(state.symbol, consumer)
            || (consumer.authored_expression.is_valid()
                && !completed
                    .get(consumer.statement_index)
                    .is_some_and(|expressions| expressions.contains(&consumer.authored_expression)))
        {
            continue;
        }
        let Some(site) = validation::find_call_site(
            program,
            machine.symbol,
            state.symbol,
            consumer.statement_index,
            consumer.call_ordinal,
        ) else {
            continue;
        };
        let Some(parameters) = validation::call_target_parameters(program, consumer.target_symbol)
        else {
            continue;
        };
        let arguments = validation::call_site_argument_expressions(program, &site);
        if parameters.len() != arguments.len() {
            continue;
        }
        for (parameter, argument) in parameters.iter().zip(arguments) {
            let (expression, type_reference, consumption) = if let Some(type_reference) =
                owned_temporary_type(
                    program,
                    state.symbol,
                    consumer.statement_index,
                    parameter,
                    *argument,
                ) {
                (
                    *argument,
                    type_reference,
                    TemporaryConsumption::OwnedTransfer,
                )
            } else {
                if parameter.is_const || parameter.is_mutable || parameter.relevance.is_erased() {
                    continue;
                }
                let Some((borrowed, _)) = crate::semantic::calls::shared_argument_referent(
                    program,
                    *argument,
                    parameter.type_reference,
                ) else {
                    continue;
                };
                let Some(place) = validation::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    consumer.statement_index,
                    borrowed,
                ) else {
                    continue;
                };
                let Some(expression) = place.root.as_expression() else {
                    continue;
                };
                let root = validation::CanonicalPlace {
                    root: place.root,
                    segments: Vec::new(),
                };
                let Some(type_reference) = validation::canonical_place_type_reference(
                    program,
                    state.symbol,
                    consumer.statement_index,
                    &root,
                ) else {
                    continue;
                };
                (
                    expression,
                    type_reference,
                    TemporaryConsumption::SharedBorrow {
                        borrowed_path: place.segments,
                    },
                )
            };
            let evaluation = crate::flow::call_permission_evaluation(
                program,
                machine,
                state,
                consumer.statement_index,
                consumer.call_ordinal,
            );
            if evaluation == checked_trees::FlowPermissionEvaluation::Unknown {
                continue;
            }
            let producer = if let Some(_) = program.expression_table.call_node(expression) {
                let mut producers = calls.iter().filter(|producer| {
                    producer.statement_index == consumer.statement_index
                        && producer.authored_expression == expression
                });
                let Some(producer) = producers.next() else {
                    continue;
                };
                if producers.next().is_some()
                    || crate::flow::call_permission_evaluation(
                        program,
                        machine,
                        state,
                        producer.statement_index,
                        producer.call_ordinal,
                    ) != evaluation
                {
                    continue;
                }
                PermissionEventSource::Call {
                    statement_index: producer.statement_index,
                    call_ordinal: producer.call_ordinal,
                    target_symbol: producer.target_symbol,
                }
            } else {
                PermissionEventSource::Statement {
                    statement_index: consumer.statement_index,
                }
            };
            owners.push(TemporaryOwner {
                expression,
                type_reference,
                consumption,
                evaluation,
                producer,
                consumer: PermissionEventSource::Call {
                    statement_index: consumer.statement_index,
                    call_ordinal: consumer.call_ordinal,
                    target_symbol: consumer.target_symbol,
                },
            });
        }
    }
    owners
}
