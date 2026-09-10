//! Array actuals use the captured call occurrence and authored formal position.
//! Discovery does not schedule evaluation or create a synthetic call. Pure and
//! computed leaves share this roster after flow capture, while statement-owned
//! arrays retain their existing pre-flow value production.

use super::*;
use checked_trees::{CheckedArrayConstructionSource, FlowFacts};

#[derive(Clone, Copy)]
pub(crate) struct CallArrayConstruction {
    pub source: CheckedArrayConstructionSource,
    pub expression: ExpressionHandle,
    pub type_reference: TypeReferenceHandle,
}

pub(crate) fn call_array_constructions(
    program: &TypedTrees,
    flow: &FlowFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
) -> Vec<CallArrayConstruction> {
    let mut states = flow.control.states.iter().filter_map(|(_, candidate)| {
        (candidate.machine_symbol == machine.symbol && candidate.state_symbol == state.symbol)
            .then_some(candidate)
    });
    let Some(captured) = states.next() else {
        return Vec::new();
    };
    if states.next().is_some() {
        return Vec::new();
    }
    let Some(calls) = flow.control.calls.span(captured.calls) else {
        return Vec::new();
    };
    let mut output = Vec::new();
    for call in calls
        .iter()
        .filter(|call| call.statement_index == statement_index)
    {
        if calls
            .iter()
            .filter(|other| {
                other.statement_index == statement_index && other.call_ordinal == call.call_ordinal
            })
            .count()
            != 1
        {
            continue;
        }
        let Some(site) = crate::find_call_site(
            program,
            machine.symbol,
            state.symbol,
            statement_index,
            call.call_ordinal,
        ) else {
            continue;
        };
        let target_symbol = match &site {
            crate::CallSite::Expression { call, .. } => call.target_symbol,
            crate::CallSite::Statement(call) => call.target_symbol,
            crate::CallSite::TransitionNamed { .. } => continue,
        };
        if target_symbol != call.target_symbol {
            continue;
        }
        let Some(target) = crate::find_state(program, target_symbol) else {
            continue;
        };
        // A scalar computation owns its nested structural operands. Giving
        // those leaves independent statement roots would schedule them twice
        // and could move construction out of a selective branch.
        let direct_root = match program
            .statement_table
            .statements(state.statement_nodes)
            .get(statement_index)
        {
            Some(StatementNode::Call(_)) => matches!(site, crate::CallSite::Statement(_)),
            Some(StatementNode::LocalData(local)) => {
                call.authored_expression == local.initial_value
            }
            Some(StatementNode::Expression(expression)) => call.authored_expression == *expression,
            _ => false,
        };
        if !direct_root
            && program
                .primitive_type_reference(target.return_type)
                .is_some()
        {
            continue;
        }
        let mut owners = program.machines().iter().filter(|owner| {
            program
                .machine_states(owner)
                .iter()
                .any(|state| state.symbol == target.symbol)
        });
        let Some(owner) = owners.next() else {
            continue;
        };
        if owners.next().is_some()
            || owner.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        {
            continue;
        }
        let parameters = program.state_parameters(target);
        let arguments = crate::call_site_argument_expressions(program, &site);
        let explicit_self = arguments.len()
            > parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count();
        let explicit_parameters = parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| !parameter.is_self || explicit_self);
        if explicit_parameters.clone().count() != arguments.len() {
            continue;
        }
        for ((position, parameter), expression) in explicit_parameters.zip(arguments) {
            if parameter.is_self
                || parameter.is_const
                || parameter.is_mutable
                || !validation::is_closed_primitive_array_type(program, parameter.type_reference)
                || validation::scalar_array_elements(
                    program,
                    machine.symbol,
                    *expression,
                    parameter.type_reference,
                )
                .is_none()
            {
                continue;
            }
            let (Ok(call_ordinal), Ok(parameter_position)) =
                (u32::try_from(call.call_ordinal), u32::try_from(position))
            else {
                continue;
            };
            output.push(CallArrayConstruction {
                source: CheckedArrayConstructionSource::CallArgument {
                    call_ordinal,
                    parameter_position,
                },
                expression: *expression,
                type_reference: parameter.type_reference,
            });
        }
    }
    output
}
