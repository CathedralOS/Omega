//! Rejoin the complete authored successor roster before partitioning its namespaces.

use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};
use checked_trees::{
    CheckedScalarStateGraph, CheckedScalarSuccessor, CheckedStructuralAccess,
    CheckedStructuralControlTransferSourcePlan, CheckedStructuralScalarArgumentSourcePlan,
    CheckedTrees,
};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};
use symbols::SymbolHandle;

use super::{authored_state, parameter_storage};
use crate::{LoweringError, unsupported};

#[cfg(test)]
mod tests;

pub(super) fn validate(
    checked: &CheckedTrees,
    source_state: SymbolHandle,
    successor: &CheckedScalarSuccessor,
) -> Result<(), LoweringError> {
    let (machine, state) = authored_state(checked, source_state)?;
    let Some(StatementNode::Transition(transition)) = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(successor.statement_ordinal as usize)
    else {
        return unsupported("scalar successor has no authored transition");
    };
    let destination = if successor.is_continuation {
        transition.continuation
    } else {
        transition.target
    };
    if transition.exit != TransitionExit::Ordinary
        || !checked
            .statement_table
            .transition_target_is_valid(destination)
    {
        return unsupported("scalar successor has no live ordinary transition target");
    }
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = checked.statement_table.transition_target(destination)
    else {
        return unsupported("scalar successor is not an authored state transfer");
    };
    let (target_machine, target_state) = authored_state(checked, successor.target)?;
    let actuals = checked.statement_table.expression_handles(*arguments);
    let formals = checked.state_parameters(target_state);
    if normalize_machine_state_target(checked, machine, path.symbol)? != successor.target
        || target_machine.symbol != machine.symbol
        || usize::try_from(arguments.count()).ok() != Some(actuals.len())
        || actuals.len() != successor.argument_count as usize
        || actuals.len() != formals.len()
    {
        return unsupported("scalar successor target disagrees with its authored state transfer");
    }

    let plans = &checked.facts.flow.terminal_scalar_graphs;
    let mut graphs = plans
        .machines
        .iter()
        .filter(|graph| graph.machine == machine.symbol);
    let graph = graphs.next().ok_or(LoweringError::Unsupported(
        "scalar successor has no source machine graph",
    ))?;
    if graphs.next().is_some() {
        return unsupported("scalar successor has ambiguous source machine graphs");
    }
    let source = graph_state(&graph.states, source_state)?;
    let target = graph_state(&graph.states, successor.target)?;
    // Reuse the signature's independent typed-source checks, including the
    // numeric-constraint plain-owned classifier and nominal/catalog custody.
    // Matching two retained signatures cannot establish these facts.
    parameter_storage(checked, machine.symbol, source)?;
    if source_state != successor.target {
        parameter_storage(checked, machine.symbol, target)?;
    }
    let structural = plans
        .structural_transfers
        .span(successor.structural_transfers)
        .ok_or(LoweringError::Unsupported(
            "scalar successor has an invalid structural transfer span",
        ))?;
    let scalar = plans
        .scalar_arguments
        .span(successor.scalar_arguments)
        .ok_or(LoweringError::Unsupported(
            "scalar successor has an invalid scalar argument span",
        ))?;
    if structural.len() != target.structural_parameters.len()
        || scalar.len() != target.scalar_parameters.len()
    {
        return unsupported("scalar successor rows do not fill its complete authored signature");
    }

    let source_parameters = checked.state_parameters(state);
    let mut scalar_position = 0;
    let mut structural_position = 0;
    let mut affine_sources = Vec::new();
    for (argument_position, (actual, formal)) in actuals.iter().zip(formals).enumerate() {
        if !checked.expression_table.expression_is_valid(*actual) {
            return unsupported("scalar successor has no live authored argument");
        }
        if let Some(primitive) = checked.primitive_type_reference(formal.type_reference) {
            let row = &scalar[scalar_position];
            if row.argument_ordinal as usize != argument_position
                || row.target_scalar_parameter_index as usize != scalar_position
                || row.primitive_type != primitive
                || row.source != CheckedStructuralScalarArgumentSourcePlan::Expression
            {
                return unsupported(
                    "scalar successor scalar row changed its authored slot or type",
                );
            }
            // Scalar graphs retain even direct parameter reads as expressions:
            // the existing expression/computation owner rejoins this exact role,
            // including current mutable values and selected operands.
            scalar_position += 1;
            continue;
        }
        let row = &structural[structural_position];
        let CheckedStructuralControlTransferSourcePlan::Parameter {
            index: source_index,
        } = row.source
        else {
            return unsupported("scalar successor requires a whole structural parameter");
        };
        let source_position = source_parameter_position(checked, source_parameters, *actual)?;
        let source_parameter = &source_parameters[source_position];
        let expected_source_index = source_parameters[..source_position]
            .iter()
            .filter(|parameter| {
                checked
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
            })
            .count();
        let retained_source = source
            .structural_parameters
            .get(source_index as usize)
            .ok_or(LoweringError::Unsupported(
                "scalar successor structural source is absent",
            ))?;
        let retained_target = &target.structural_parameters[structural_position];
        if checked
            .primitive_type_reference(source_parameter.type_reference)
            .is_some()
            || source_index as usize != expected_source_index
            || retained_source.position as usize != source_position
            || row.target_parameter_index as usize != structural_position
            || retained_target.position as usize != argument_position
            || retained_source.type_identity != retained_target.type_identity
            || retained_source.access != retained_target.access
            || retained_source.multiplicity != retained_target.multiplicity
            || retained_source.qualifications != retained_target.qualifications
            || checked.normalized_type_identity(source_parameter.type_reference)
                != checked.normalized_type_identity(formal.type_reference)
        {
            return unsupported(
                "scalar successor structural row changed its source, slot, or type",
            );
        }
        if retained_source.access == CheckedStructuralAccess::Owned
            && retained_source.multiplicity == Multiplicity::Affine
        {
            if affine_sources.contains(&source_index) {
                return unsupported("scalar successor transfers an affine parameter twice");
            }
            affine_sources.push(source_index);
            validate_affine_permission(
                checked,
                machine.symbol,
                source_state,
                transition_permission_source(checked, machine.symbol, state, successor)?,
                source_parameter.symbol,
            )?;
        }
        structural_position += 1;
    }
    Ok(())
}

fn graph_state(
    states: &[CheckedScalarStateGraph],
    symbol: SymbolHandle,
) -> Result<&CheckedScalarStateGraph, LoweringError> {
    let mut matches = states.iter().filter(|state| state.state == symbol);
    let state = matches.next().ok_or(LoweringError::Unsupported(
        "scalar successor state is absent from its machine graph",
    ))?;
    if matches.next().is_some() {
        return unsupported("scalar successor state has ambiguous graph ownership");
    }
    Ok(state)
}

fn source_parameter_position(
    checked: &CheckedTrees,
    parameters: &[checked_trees::signature::StateParameter],
    expression: ExpressionHandle,
) -> Result<usize, LoweringError> {
    let ExpressionNode::Name(name) = checked.expression_table.expression(expression) else {
        return unsupported("scalar successor structural actual is not a whole parameter name");
    };
    let [spelling] = checked.expression_table.name_path_members(name.members) else {
        return unsupported("scalar successor structural actual is a projected or foreign name");
    };
    if !name.symbol.is_valid() || name.head_symbol != name.symbol || name.members.count() != 1 {
        return unsupported("scalar successor structural actual lost its exact resolved symbol");
    }
    parameters
        .iter()
        .position(|parameter| parameter.symbol == name.symbol && parameter.name == *spelling)
        .ok_or(LoweringError::Unsupported(
            "scalar successor structural actual does not name its source parameter",
        ))
}

fn validate_affine_permission(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    permission_source: PermissionEventSource,
    parameter: SymbolHandle,
) -> Result<(), LoweringError> {
    let ownership = &checked.facts.flow.ownership;
    // Named transitions retain the authored call target and preorder ordinal,
    // not the normalized executable entry-state target.
    let mut events = ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.machine_symbol == machine
                && event.state_symbol == state
                && event.root == facts::PlaceRoot::Symbol(parameter)
                && event.access == PermissionAccess::Owned
                && event.source == permission_source
        });
    let event = events.next().ok_or(LoweringError::Unsupported(
        "scalar successor lost its exact affine transition permission",
    ))?;
    if events.next().is_some()
        || event.kind != PermissionEventKind::Transfer
        || event.multiplicity != Multiplicity::Affine
        || event.claim_identity != PermissionClaimIdentity::Unknown
        || event.provenance != PermissionProvenance::Unknown
        || event.obligation_live
        || ownership
            .segments
            .span(event.segments)
            .is_none_or(|segments| !segments.is_empty())
    {
        return unsupported("scalar successor changed its whole affine transition permission");
    }
    Ok(())
}

pub(crate) fn normalize_machine_state_target(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    target: SymbolHandle,
) -> Result<SymbolHandle, LoweringError> {
    let states = checked.machine_states(machine);
    if target.is_valid() && states.iter().any(|state| state.symbol == target) {
        return Ok(target);
    }
    if target.is_valid() && target == machine.symbol {
        // This extension owns single-state scalar loops. The sole state is
        // independently attached to the exact machine handle, not its spelling.
        if let [entry] = states {
            return Ok(entry.symbol);
        }
    }
    unsupported("scalar successor has no exact machine/state target normalization")
}

fn transition_permission_source(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: &checked_trees::state::State,
    successor: &CheckedScalarSuccessor,
) -> Result<PermissionEventSource, LoweringError> {
    let statement_index = successor.statement_ordinal as usize;
    let Some(StatementNode::Transition(transition)) = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)
    else {
        return unsupported("scalar successor permission has no source transition");
    };
    let control = &checked.facts.flow.control;
    let mut states = control
        .states
        .iter()
        .map(|(_, state)| state)
        .filter(|candidate| {
            candidate.machine_symbol == machine && candidate.state_symbol == state.symbol
        });
    let retained = states.next().ok_or(LoweringError::Unsupported(
        "scalar successor has no captured call owner",
    ))?;
    if states.next().is_some() {
        return unsupported("scalar successor has ambiguous captured call ownership");
    }
    let mut roots = control
        .calls
        .span(retained.calls)
        .ok_or(LoweringError::Unsupported(
            "scalar successor has an invalid captured call span",
        ))?
        .iter()
        .filter(|call| {
            call.statement_index == statement_index && !call.authored_expression.is_valid()
        })
        .collect::<Vec<_>>();
    roots.sort_by_key(|call| call.call_ordinal);
    if roots
        .windows(2)
        .any(|pair| pair[0].call_ordinal == pair[1].call_ordinal)
    {
        return unsupported("scalar successor has duplicate captured call coordinates");
    }
    let mut roots = roots.into_iter();
    let mut selected = None;
    for (target, is_continuation) in [(transition.target, false), (transition.continuation, true)] {
        if !target.is_valid() {
            continue;
        }
        let TransitionTargetNode::Named { path, .. } =
            checked.statement_table.transition_target(target)
        else {
            continue;
        };
        let call = roots.next().ok_or(LoweringError::Unsupported(
            "scalar successor lost its captured named call",
        ))?;
        if call.target_symbol != path.symbol
            || call.has_receiver
            || call.receiver_symbol != path.head_symbol
        {
            return unsupported("scalar successor captured call differs from its source target");
        }
        if is_continuation == successor.is_continuation {
            selected = Some(PermissionEventSource::Call {
                statement_index,
                call_ordinal: call.call_ordinal,
                target_symbol: path.symbol,
            });
        }
    }
    if roots.next().is_some() {
        return unsupported("scalar successor has an unauthored named call occurrence");
    }
    selected.ok_or(LoweringError::Unsupported(
        "scalar successor lost its selected call coordinate",
    ))
}
