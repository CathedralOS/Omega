//! Shared access to an anonymous owner ends before that owner's call cleanup.
//! Every contained linear obligation needs an authorized disposition on normal
//! completion, including obligations outside a borrowed projection. Executable
//! event coverage is narrower than source legality: omitting an unsupported
//! claim-bearing event roster must not silently authorize abandoning its owner.
use super::{
    CheckFacts, FlowPermissionEventFact, HandleSpan, Multiplicity, PermissionAccess,
    PermissionClaimIdentity, PermissionEventKind, PermissionProvenance,
};
use crate::checks::type_carries_linear_obligation;
use crate::checks::type_multiplicity;
use diagnostics::Diagnostic;
use language_semantics::PermissionEventSource;
use typed_trees::expression::ExpressionNode;

pub(in crate::checks::multiplicity) fn append_shared_borrow(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    permissions: &mut Vec<FlowPermissionEventFact>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((_, flow)) = facts.flow.control.states.iter().find(|(_, flow)| {
        flow.machine_symbol == machine.symbol && flow.state_symbol == state.symbol
    }) else {
        return;
    };
    let Some(calls) = facts.flow.control.calls.span(flow.calls) else {
        return;
    };
    // The call table retains syntax on skipped paths. Use the shared source
    // evaluation inventory, not another Boolean/match recognizer, to decide
    // which expressions can establish a temporary in this statement.
    let completed_expressions = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
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
    // Temporary ownership follows each borrowed operand's producer and normal
    // consumer completion. Neither statement spelling nor sibling call count
    // determines whether that owner exists.
    for consumer in calls {
        if facts.flow.control.is_retired(state.symbol, consumer)
            || (consumer.authored_expression.is_valid()
                && !completed_expressions
                    .get(consumer.statement_index)
                    .is_some_and(|completed| completed.contains(&consumer.authored_expression)))
        {
            continue;
        }
        let Some(source) = crate::semantic::calls::find_call_site(
            program,
            machine.symbol,
            state.symbol,
            consumer.statement_index,
            consumer.call_ordinal,
        ) else {
            continue;
        };
        let Some(parameters) =
            crate::semantic::calls::call_target_parameters(program, consumer.target_symbol)
        else {
            continue;
        };
        let arguments = crate::semantic::calls::call_site_argument_expressions(program, &source);
        let parameters = parameters.iter();
        if parameters.clone().count() != arguments.len() {
            continue;
        }
        for (parameter, argument) in parameters.zip(arguments) {
            let Some(events) = events(
                program,
                machine,
                state,
                calls,
                consumer,
                parameter,
                *argument,
                diagnostics,
            ) else {
                continue;
            };
            if !permissions.iter().any(|event| {
                event.machine_symbol == machine.symbol
                    && event.state_symbol == state.symbol
                    && event.root == events[0].root
            }) {
                permissions.extend(events);
            }
        }
    }
}

fn events(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    calls: &[checked_trees::FlowCallFact],
    consumer: &checked_trees::FlowCallFact,
    parameter: &typed_trees::signature::StateParameter,
    argument: typed_trees::expression::ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<[FlowPermissionEventFact; 3]> {
    // Only runtime shared operands establish an anonymous owner here. Erased
    // and compile-time arguments have no runtime cleanup continuation.
    if parameter.is_const || parameter.is_mutable || parameter.relevance.is_erased() {
        return None;
    }
    let (expression, referee) = crate::semantic::calls::shared_argument_referent(
        program,
        argument,
        parameter.type_reference,
    )?;
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        consumer.statement_index,
        expression,
    )?;
    if let facts::PlaceRoot::Expression(owner) = place.root {
        let root = crate::flow::CanonicalPlace {
            root: place.root,
            segments: Vec::new(),
        };
        let owner_type = crate::flow::canonical_place_type_reference(
            program,
            state.symbol,
            consumer.statement_index,
            &root,
        )?;
        if type_carries_linear_obligation(program, owner_type)
            && !crate::flow::whole_cleanup_is_eligible(program, owner_type)
        {
            diagnostics.push(Diagnostic::error(format!(
                "cannot automatically dispose borrowed temporary `{}` after call `{}` at statement {}: every contained linear obligation needs its owner-authorized terminal disposition",
                program.normalized_type_identity(owner_type),
                program.symbols.name(consumer.target_symbol),
                consumer.statement_index,
            )));
            return None;
        }
        // The loan can select a child, but it never makes that child the
        // temporary's owner. Claim-free whole-owner events below retain their
        // existing coverage; projected and claim-bearing production remains
        // an explicit executable-plan dependency after this legality check.
        if owner != expression {
            return None;
        }
    }
    if place.root != facts::PlaceRoot::Expression(expression) || !place.segments.is_empty() {
        return None;
    }
    let evaluation = crate::flow::call_permission_evaluation(
        program,
        machine,
        state,
        consumer.statement_index,
        consumer.call_ordinal,
    );
    if evaluation == checked_trees::FlowPermissionEvaluation::Unknown {
        return None;
    }
    // Calls retain their invocation coordinate; other expression-owned values
    // are established in this statement's evaluation. The expression root
    // distinguishes sibling constructions without inventing producer calls.
    let (producer_source, produced_type) =
        if let ExpressionNode::Call(authored) = program.expression_table.expression(expression) {
            let mut producers = calls.iter().filter(|producer| {
                producer.statement_index == consumer.statement_index
                    && producer.authored_expression == expression
            });
            let producer = producers.next()?;
            if producers.next().is_some()
                || producer.call_ordinal <= consumer.call_ordinal
                || crate::flow::call_target_symbol(program, authored) != producer.target_symbol
                || crate::flow::call_permission_evaluation(
                    program,
                    machine,
                    state,
                    producer.statement_index,
                    producer.call_ordinal,
                ) != evaluation
            {
                return None;
            }
            (
                PermissionEventSource::Call {
                    statement_index: producer.statement_index,
                    call_ordinal: producer.call_ordinal,
                    target_symbol: producer.target_symbol,
                },
                crate::flow::call_target_return_type(program, producer.target_symbol)?,
            )
        } else {
            if !validation::checked_argument_matches_type_reference(program, expression, referee) {
                return None;
            }
            (
                PermissionEventSource::Statement {
                    statement_index: consumer.statement_index,
                },
                referee,
            )
        };
    if program.normalized_type_identity(referee) != program.normalized_type_identity(produced_type)
        || type_multiplicity(program, produced_type) != Multiplicity::Affine
        || type_carries_linear_obligation(program, produced_type)
        || !validation::has_borrow_free_contents(program, produced_type)
        || !crate::flow::whole_cleanup_is_eligible(program, produced_type)
    {
        return None;
    }
    let consumer_source = PermissionEventSource::Call {
        statement_index: consumer.statement_index,
        call_ordinal: consumer.call_ordinal,
        target_symbol: consumer.target_symbol,
    };
    let establishment = FlowPermissionEventFact {
        evaluation,
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        source: producer_source,
        kind: PermissionEventKind::Establish,
        multiplicity: Multiplicity::Affine,
        access: PermissionAccess::Owned,
        claim_identity: PermissionClaimIdentity::Unknown,
        provenance: PermissionProvenance::Established {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            source: producer_source,
        },
        root: facts::PlaceRoot::Expression(expression),
        segments: HandleSpan::empty(),
        obligation_live: false,
    };
    Some([
        establishment.clone(),
        FlowPermissionEventFact {
            source: consumer_source,
            multiplicity: Multiplicity::Unrestricted,
            access: PermissionAccess::Shared,
            ..establishment.clone()
        },
        FlowPermissionEventFact {
            source: consumer_source,
            kind: PermissionEventKind::AffineDrop,
            ..establishment
        },
    ])
}
