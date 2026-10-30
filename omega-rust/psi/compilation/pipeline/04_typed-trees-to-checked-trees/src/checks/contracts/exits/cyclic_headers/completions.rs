//! Prove a completion property from all of its actual result producers.
//!
//! A terminal invocation forwards its result unchanged. Following these edges
//! can cross machine boundaries or revisit a cycle without citing any callee's
//! postcondition. Every finite normal completion must reach a value producer;
//! each producer must independently establish the requested property. Unknown
//! dispatch, missing flow and an unproved producer refuse the whole closure.
//!
//! A result-only property has no invocation inputs to substitute at a foreign
//! entry. Input-dependent properties stay within the activation whose admitted
//! header invariants retain those input snapshots. This traversal does not grant
//! recursive citation, termination, argument admission or reference authority.

use super::{
    AdmittedGuarantee, Header, exit_return_expression, fixed_integer, is_result_reference,
};
use arena::Handle;
use checked_trees::{CheckFacts, CheckedScalarExpression, FlowExitFact};
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;
use typed_trees::{
    domain::ProofFact,
    expression::{ExpressionHandle, ExpressionNode},
    machine::Machine,
    state::State,
    statement::{StatementNode, TransitionExit, TransitionTargetNode},
};
use validation::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticValue,
};

pub(super) fn proves(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    owner: SymbolHandle,
    contract: Handle<ProofFact>,
    admitted: &[AdmittedGuarantee],
) -> bool {
    let Some(machine) = program.machine(owner) else {
        return false;
    };
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    let Some(guarantee) = program.proof_facts.get(contract).as_expression() else {
        return false;
    };
    let origins = crate::flow::activation_parameter_origins(program, machine);
    let origins = program
        .machine_states(machine)
        .iter()
        .map(|state| crate::flow::state_parameter_origins(program, machine, state, &origins))
        .collect::<Vec<_>>();
    let Some(header) = Header::new(program, facts, machine, entry, &origins) else {
        return false;
    };
    let mut expressions = Vec::new();
    validation::expression_traversal::collect_expression_tree(
        program,
        *guarantee,
        &mut expressions,
    );
    let result_only = header.proposition_is_admitted(*guarantee)
        && expressions.iter().all(|expression| {
            !matches!(
                program.expression_table.expression(*expression),
                ExpressionNode::Name(_)
            ) || is_result_reference(program, machine, *expression)
        });
    // This answer is cached for every exit carrying this contract. Include
    // every body of the owner, even one unreachable from its entry; a proved
    // entry closure must not discharge a separately checked invalid body.
    let mut pending = program
        .machine_states(machine)
        .iter()
        .map(|state| (owner, state.symbol))
        .collect::<Vec<_>>();
    let mut visited = Vec::new();
    let mut observed_return = false;
    while let Some((machine_symbol, state_symbol)) = pending.pop() {
        if visited.contains(&(machine_symbol, state_symbol)) {
            continue;
        }
        visited.push((machine_symbol, state_symbol));
        let Some((current_machine, state)) = program.state_flow_site(machine_symbol, state_symbol)
        else {
            return false;
        };
        let Some(state_flow) = Header::state_flow(facts, current_machine, state) else {
            return false;
        };
        let mut outgoing_found = false;
        for (_, outgoing) in facts.flow.control.outgoing.iter().filter(|(_, outgoing)| {
            outgoing.machine_symbol == machine_symbol && outgoing.state_symbol == state_symbol
        }) {
            outgoing_found = true;
            if let Some(StatementNode::Transition(transition)) = program
                .statement_table
                .statement_at(state, outgoing.statement_index)
                && matches!(transition.exit, TransitionExit::Crash(_))
                && outgoing.transition_target == transition.target
            {
                continue;
            }
            // Internal activation edges and ordinary terminal invocations share
            // this completion traversal, but retain their original call semantics.
            if outgoing.transition_target.is_valid()
                && let TransitionTargetNode::Named { path, .. } = program
                    .statement_table
                    .transition_target(outgoing.transition_target)
                && let Some(destination) =
                    validation::activation_target_state_index(program, current_machine, path.symbol)
            {
                pending.push((
                    machine_symbol,
                    program.machine_states(current_machine)[destination].symbol,
                ));
                continue;
            }
            let mut exits = facts
                .flow
                .control
                .exits
                .span_or_empty(state_flow.exits)
                .iter()
                .filter(|exit| {
                    exit.statement_index == outgoing.statement_index
                        && exit.transition_target == outgoing.transition_target
                        && exit.returned_value == outgoing.returned_value
                });
            let Some(exit) = exits.next() else {
                return false;
            };
            if exits.next().is_some() {
                return false;
            }
            if let Some(call) = super::super::super::return_values::exit_return_call_in_state(
                program, facts, state_flow, exit,
            ) {
                let Some(callee) = program.machine_holding_state(call.target_symbol) else {
                    return false;
                };
                let Some(callee_entry) = program.machine_states(callee).first() else {
                    return false;
                };
                if call.target_symbol != callee_entry.symbol
                    || (!result_only && callee.symbol != owner)
                    || fixed_integer(program, entry.return_type).is_none()
                    || fixed_integer(program, entry.return_type)
                        != fixed_integer(program, callee_entry.return_type)
                {
                    return false;
                }
                pending.push((callee.symbol, callee_entry.symbol));
                continue;
            }
            if machine_symbol == owner
                && admitted.contains(&AdmittedGuarantee {
                    state_symbol,
                    statement_index: exit.statement_index,
                    transition_target: exit.transition_target,
                    contract,
                })
            {
                observed_return = true;
                continue;
            }
            if !result_only
                || !proves_value(
                    program,
                    facts,
                    owner,
                    *guarantee,
                    current_machine,
                    state,
                    exit,
                )
            {
                return false;
            }
            observed_return = true;
        }
        if !outgoing_found {
            return false;
        }
    }
    observed_return
}

fn proves_value(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    owner: SymbolHandle,
    guarantee: ExpressionHandle,
    machine: &Machine,
    state: &State,
    exit: &FlowExitFact,
) -> bool {
    let origins = crate::flow::activation_parameter_origins(program, machine);
    let origins = program
        .machine_states(machine)
        .iter()
        .map(|state| crate::flow::state_parameter_origins(program, machine, state, &origins))
        .collect::<Vec<_>>();
    let Some(header) = Header::new(program, facts, machine, state, &origins) else {
        return false;
    };
    let returned = exit_return_expression(program, exit);
    let bindings = header.caller_atoms(state);
    let Some(value) = return_value(&header, state, exit, returned, &bindings) else {
        return false;
    };
    let mut hypotheses = header.requirements(header.formal_roster());
    if state.symbol == header.entry.symbol {
        hypotheses.extend(header.requirements(bindings.clone()));
    }
    for context in facts
        .flow
        .contexts
        .semantic_context_refs
        .span_or_empty(exit.entry_semantic_contexts)
    {
        for fact in facts
            .semantic
            .context_view(facts.semantic.contexts.get(context.context))
            .facts()
        {
            for (expression, holds) in header.arm_facts(fact) {
                hypotheses.push(header.hypothesis(expression, bindings.clone(), holds));
            }
        }
    }
    let goal = ScopedArithmeticExpression {
        owner,
        expression: guarantee,
        bindings: vec![ScopedArithmeticBinding {
            binder: ScopedArithmeticBinder::Result,
            value,
        }],
    };
    header.proves(&hypotheses, &goal)
}

/// Exact terms retain their symbolic relationships. Other selected arithmetic
/// contributes only independently established bounds, never an integer rewrite
/// of residue operations. Both routes read immutable scalar parameters only:
/// no replayed local initializer, borrowed storage or imported call guarantee
/// can justify a cyclic completion.
fn return_value(
    header: &Header<'_, '_>,
    state: &State,
    exit: &FlowExitFact,
    returned: ExpressionHandle,
    bindings: &[ScopedArithmeticBinding],
) -> Option<ScopedArithmeticValue> {
    if header
        .term_reference(returned)
        .is_some_and(|term| term.domain == ArithmeticDomain::Exact)
    {
        return Some(ScopedArithmeticValue::Term(ScopedArithmeticExpression {
            owner: header.machine.symbol,
            expression: returned,
            bindings: bindings.to_vec(),
        }));
    }
    let (primitive, _) = fixed_integer(header.program, state.return_type)?;
    let selected = crate::values::lower_unit_scalar_argument_for_places(
        header.program,
        &header.facts.operators,
        state,
        exit.statement_index,
        returned,
        primitive,
        |place, primitive_type| {
            let facts::PlaceRoot::Symbol(symbol) = place.root else {
                return None;
            };
            (place.segments.is_empty()
                && header
                    .roster
                    .iter()
                    .any(|parameter| parameter.symbol == symbol))
            .then_some(CheckedScalarExpression::StorageRead {
                symbol,
                primitive_type,
            })
        },
    )?;
    let mut values = crate::values::LivePlaceIntervals {
        program: header.program,
        semantic: &header.facts.semantic,
        contexts: &[],
        parameters: header.program.state_parameters(state),
        symbols: &[],
        state: state.symbol,
    };
    let bounds = validation::selected_scalar_interval(&selected, &mut values)?;
    Some(ScopedArithmeticValue::BoundedInteger {
        value: Box::new(ScopedArithmeticValue::Atom {
            identity: format!("completion:{:?}:{returned:?}", state.symbol),
            unsigned: super::unsigned(primitive),
        }),
        minimum: Some(bounds.minimum),
        maximum: Some(bounds.maximum),
    })
}
