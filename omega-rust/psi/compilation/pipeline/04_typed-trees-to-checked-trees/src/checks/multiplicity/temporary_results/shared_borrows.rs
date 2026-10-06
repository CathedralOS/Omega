//! Shared access to an anonymous owner ends before that owner's call cleanup.
//! Every contained linear obligation needs an authorized disposition on normal
//! completion, including obligations outside a borrowed projection. One source
//! evaluation inventory supplies named/anonymous permission production and the
//! exact loan continuation; executable admission consumes that published roster.
use super::{
    CheckFacts, FlowPermissionEventFact, HandleSpan, Multiplicity, PermissionAccess,
    PermissionClaimIdentity, PermissionEventKind, PermissionProvenance,
};
use crate::checks::type_carries_linear_obligation;
use crate::checks::type_multiplicity;
use diagnostics::Diagnostic;
use language_semantics::PermissionEventSource;
use typed_trees::expression::ExpressionNode;

/// An evaluated loan retains the whole producing owner, even when its operand
/// selects a child. The source evaluation inventory is shared with permission
/// production so skipped calls cannot establish anonymous custody.
pub(crate) struct SharedTemporaryOwner {
    pub(crate) expression: typed_trees::expression::ExpressionHandle,
    pub(crate) type_reference: typed_trees::types::TypeReferenceHandle,
    pub(crate) borrowed_path: Vec<facts::PlaceSegment>,
    pub(crate) evaluation: checked_trees::FlowPermissionEvaluation,
    pub(crate) producer: PermissionEventSource,
    pub(crate) consumer: PermissionEventSource,
}

pub(crate) fn shared_temporary_owners(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    calls: &[checked_trees::FlowCallFact],
) -> Vec<SharedTemporaryOwner> {
    let completed = program
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
        let Some(site) = crate::semantic::calls::find_call_site(
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
        let arguments = crate::semantic::calls::call_site_argument_expressions(program, &site);
        if parameters.len() != arguments.len() {
            continue;
        }
        for (parameter, argument) in parameters.iter().zip(arguments) {
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
            let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                consumer.statement_index,
                borrowed,
            ) else {
                continue;
            };
            let facts::PlaceRoot::Expression(expression) = place.root else {
                continue;
            };
            let root = crate::flow::CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            };
            let Some(type_reference) = crate::flow::canonical_place_type_reference(
                program,
                state.symbol,
                consumer.statement_index,
                &root,
            ) else {
                continue;
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
            let producer =
                if let ExpressionNode::Call(_) = program.expression_table.expression(expression) {
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
            owners.push(SharedTemporaryOwner {
                expression,
                type_reference,
                borrowed_path: place.segments,
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

pub(in crate::checks::multiplicity) fn append_shared_borrow(
    program: &typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: usize,
    evaluation: checked_trees::FlowPermissionEvaluation,
    owners: &[SharedTemporaryOwner],
    places: &mut [crate::checks::multiplicity::linear_obligations::LinearPlace],
    permissions: &mut Vec<FlowPermissionEventFact>,
    diagnostics: &mut Vec<Diagnostic>,
    dispositions: &mut checked_trees::ContextualCleanupFacts,
) {
    for owner in owners.iter().filter(|owner| {
        matches!(owner.consumer, PermissionEventSource::Call { statement_index: index, .. } if index == statement_index)
            && owner.evaluation == evaluation
    }) {
        let root = facts::PlaceRoot::Expression(owner.expression);
        let carries_claims = type_carries_linear_obligation(program, owner.type_reference);
        if carries_claims && !crate::flow::whole_cleanup_is_eligible(program, owner.type_reference) {
            let PermissionEventSource::Call { target_symbol, .. } = owner.consumer else { continue; };
            diagnostics.push(Diagnostic::error(format!(
                "cannot automatically dispose borrowed temporary `{}` after call `{}` at statement {}: every contained linear obligation needs its owner-authorized terminal disposition",
                program.normalized_type_identity(owner.type_reference),
                program.symbols.name(target_symbol), statement_index,
            )));
            continue;
        }
        if type_multiplicity(program, owner.type_reference) == Multiplicity::Unrestricted {
            continue;
        }
        if carries_claims {
            // The death owns the complete value. Its selected hook plan, not
            // the borrowed callee, authorizes disposal of every remaining claim.
            dispositions.dispositions.append(checked_trees::ContextualCleanupDisposition {
                machine, state, source: owner.consumer,
                exit: checked_trees::FlowClaimJoinExit::default(),
                root, disposed_type: owner.type_reference,
                path: HandleSpan::empty(), exclusions: HandleSpan::empty(),
            });
        }
        permissions.push(FlowPermissionEventFact {
            evaluation, machine_symbol: machine, state_symbol: state, source: owner.consumer,
            kind: PermissionEventKind::Establish,
            multiplicity: Multiplicity::Unrestricted, access: PermissionAccess::Shared,
            claim_identity: PermissionClaimIdentity::Unknown,
            provenance: PermissionProvenance::Established { machine_symbol: machine, state_symbol: state, source: owner.producer },
            root, segments: facts.flow.ownership.segments.insert_many(owner.borrowed_path.iter().copied()),
            obligation_live: false,
        });
        for place in places.iter_mut().filter(|place| place.root == root && place.live) {
            permissions.push(FlowPermissionEventFact {
                evaluation, machine_symbol: machine, state_symbol: state, source: owner.consumer,
                kind: if place.multiplicity == Multiplicity::Linear { PermissionEventKind::Consume } else { PermissionEventKind::AffineDrop },
                multiplicity: place.multiplicity, access: PermissionAccess::Owned,
                claim_identity: place.claim_identity.unwrap_or(PermissionClaimIdentity::Unknown),
                provenance: place.provenance.unwrap_or(PermissionProvenance::Unknown),
                root, segments: facts.flow.ownership.segments.insert_many(place.path.iter().copied()),
                obligation_live: false,
            });
            place.live = false;
        }
    }
}
