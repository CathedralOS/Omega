//! Exact authored mixed arguments on scalar state edges.

use arena::Arena;
use checked_trees::{
    CheckedScalarBranchDestination, CheckedScalarMachineGraph, CheckedScalarStateGraph,
    CheckedScalarStateTerminator, CheckedScalarSuccessor, CheckedStructuralAccess,
    CheckedStructuralControlTransferPlan, CheckedStructuralControlTransferSourcePlan,
    CheckedStructuralScalarArgumentPlan, CheckedStructuralScalarArgumentSourcePlan,
};
use language_semantics::{Multiplicity, PermissionEventSource};
use typed_trees::{
    TypedTrees,
    expression::ExpressionNode,
    state::State,
    statement::{StatementNode, TransitionExit, TransitionTargetNode},
};

struct SuccessorArguments {
    structural: Vec<CheckedStructuralControlTransferPlan>,
    scalar: Vec<CheckedStructuralScalarArgumentPlan>,
}

pub(super) fn iter(
    terminator: &CheckedScalarStateTerminator,
) -> impl Iterator<Item = &CheckedScalarSuccessor> {
    fn branch(destination: &CheckedScalarBranchDestination) -> Option<&CheckedScalarSuccessor> {
        match destination {
            CheckedScalarBranchDestination::Jump(successor) => Some(successor),
            _ => None,
        }
    }
    match terminator {
        CheckedScalarStateTerminator::Jump(successor) => [Some(successor), None],
        CheckedScalarStateTerminator::Conditional {
            when_true,
            when_false,
            ..
        } => [branch(when_true), branch(when_false)],
        _ => [None, None],
    }
    .into_iter()
    .flatten()
}

fn iter_mut(
    terminator: &mut CheckedScalarStateTerminator,
) -> impl Iterator<Item = &mut CheckedScalarSuccessor> {
    fn branch(
        destination: &mut CheckedScalarBranchDestination,
    ) -> Option<&mut CheckedScalarSuccessor> {
        match destination {
            CheckedScalarBranchDestination::Jump(successor) => Some(successor),
            _ => None,
        }
    }
    match terminator {
        CheckedScalarStateTerminator::Jump(successor) => [Some(successor), None],
        CheckedScalarStateTerminator::Conditional {
            when_true,
            when_false,
            ..
        } => [branch(when_true), branch(when_false)],
        _ => [None, None],
    }
    .into_iter()
    .flatten()
}

pub(super) fn retain(
    program: &TypedTrees,
    graph: &mut CheckedScalarMachineGraph,
    structural: &mut Arena<CheckedStructuralControlTransferPlan>,
    scalar: &mut Arena<CheckedStructuralScalarArgumentPlan>,
) -> Option<()> {
    // Resolve all edges before mutating their spans. Working rows are private;
    // only the completed argument partition enters the durable arenas.
    let graph_view: &CheckedScalarMachineGraph = graph;
    let rows = graph_view
        .states
        .iter()
        .flat_map(|source| {
            iter(&source.terminator)
                .map(move |successor| arguments(program, graph_view, source, successor))
        })
        .collect::<Option<Vec<_>>>()?;
    for (successor, rows) in graph
        .states
        .iter_mut()
        .flat_map(|source| iter_mut(&mut source.terminator))
        .zip(rows)
    {
        successor.structural_transfers = structural.insert_many(rows.structural);
        successor.scalar_arguments = scalar.insert_many(rows.scalar);
    }
    Some(())
}

pub(super) fn validate(
    program: &TypedTrees,
    graph: &CheckedScalarMachineGraph,
    structural: &Arena<CheckedStructuralControlTransferPlan>,
    scalar: &Arena<CheckedStructuralScalarArgumentPlan>,
) -> Option<()> {
    for source in &graph.states {
        for successor in iter(&source.terminator) {
            let expected = arguments(program, graph, source, successor)?;
            if structural.span(successor.structural_transfers)? != expected.structural
                || scalar.span(successor.scalar_arguments)? != expected.scalar
            {
                return None;
            }
        }
    }
    Some(())
}

fn arguments(
    program: &TypedTrees,
    graph: &CheckedScalarMachineGraph,
    source: &CheckedScalarStateGraph,
    successor: &CheckedScalarSuccessor,
) -> Option<SuccessorArguments> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == graph.machine)?;
    let states = program.machine_states(machine);
    let source_state = states.iter().find(|state| state.symbol == source.state)?;
    let target = graph
        .states
        .iter()
        .find(|state| state.state == successor.target)?;
    let target_state = states.iter().find(|state| state.symbol == target.state)?;
    let source_parameters = program.state_parameters(source_state);
    let target_parameters = program.state_parameters(target_state);
    if !source.structural_parameters.is_empty() || !target.structural_parameters.is_empty() {
        if states.len() != 1 || source.state != target.state {
            return None;
        }
        let (structural, scalar, _) =
            super::super::terminal_unit::structural_scalar_graph_signature(program, source_state)?;
        if source.structural_parameters != structural || source.scalar_parameters != scalar {
            return None;
        }
    }
    let StatementNode::Transition(transition) = program
        .statement_table
        .statements(source_state.statement_nodes)
        .get(successor.statement_ordinal as usize)?
    else {
        return None;
    };
    let destination = if successor.is_continuation {
        transition.continuation
    } else {
        transition.target
    };
    if transition.exit != TransitionExit::Ordinary
        || !program
            .statement_table
            .transition_target_is_valid(destination)
    {
        return None;
    }
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(destination)
    else {
        return None;
    };
    let arguments = program.statement_table.expression_handles(*arguments);
    let target_index = crate::checks::termination::named_transition_target_state_index(
        program,
        machine,
        path.symbol,
    )?;
    if states.get(target_index)?.symbol != target.state
        || arguments.len() != successor.argument_count as usize
        || arguments.len() != target_parameters.len()
        || target_parameters.len()
            != target.scalar_parameters.len() + target.structural_parameters.len()
    {
        return None;
    }
    let mut rows = SuccessorArguments {
        structural: Vec::new(),
        scalar: Vec::new(),
    };
    let mut transferred_affine = Vec::new();
    for (argument_position, (actual, formal)) in arguments.iter().zip(target_parameters).enumerate()
    {
        let argument_ordinal = u32::try_from(argument_position).ok()?;
        if let Some(primitive_type) = program.primitive_type_reference(formal.type_reference) {
            let target_scalar_parameter_index = u32::try_from(rows.scalar.len()).ok()?;
            let retained = target.scalar_parameters.get(rows.scalar.len())?;
            if retained.source_position != argument_ordinal
                || retained.primitive_type != primitive_type
            {
                return None;
            }
            // Even a direct parameter is read through its exact expression
            // role, preserving current mutable values and computed operands.
            rows.scalar.push(CheckedStructuralScalarArgumentPlan {
                argument_ordinal,
                source: CheckedStructuralScalarArgumentSourcePlan::Expression,
                target_scalar_parameter_index,
                primitive_type,
            });
            continue;
        }
        let target_parameter_index = u32::try_from(rows.structural.len()).ok()?;
        let target_parameter = target.structural_parameters.get(rows.structural.len())?;
        let ExpressionNode::Name(name) = program.expression_table.expression(*actual) else {
            return None;
        };
        let [spelling] = program.expression_table.name_path_members(name.members) else {
            return None;
        };
        let (source_position, parameter) =
            source_parameters
                .iter()
                .enumerate()
                .find(|(_, parameter)| {
                    name.symbol.is_valid()
                        && name.head_symbol == name.symbol
                        && parameter.symbol == name.symbol
                        && parameter.name == *spelling
                        && !parameter.is_mutable
                        && !parameter.is_const
                        && !parameter.is_self
                })?;
        let source_index = source
            .structural_parameters
            .iter()
            .position(|parameter| parameter.position as usize == source_position)?;
        let source_parameter = &source.structural_parameters[source_index];
        if target_parameter.position != argument_ordinal
            || source_parameter.type_identity != target_parameter.type_identity
            || source_parameter.access != target_parameter.access
            || source_parameter.multiplicity != target_parameter.multiplicity
            || !source_parameter.qualifications.is_empty()
            || !target_parameter.qualifications.is_empty()
            || source_parameter.fused_service_erasure.is_some()
            || target_parameter.fused_service_erasure.is_some()
            || program.normalized_type_identity(parameter.type_reference)
                != program.normalized_type_identity(formal.type_reference)
        {
            return None;
        }
        if source_parameter.access == CheckedStructuralAccess::Owned
            && source_parameter.multiplicity == Multiplicity::Affine
        {
            if transferred_affine.contains(&source_index) {
                return None;
            }
            transferred_affine.push(source_index);
        }
        rows.structural.push(CheckedStructuralControlTransferPlan {
            source: CheckedStructuralControlTransferSourcePlan::Parameter {
                index: u32::try_from(source_index).ok()?,
            },
            target_parameter_index,
        });
    }
    Some(rows)
}

/// Named state transfers share the existing call-occurrence permission ledger.
pub(super) fn owned_transfers(
    program: &TypedTrees,
    machine_symbol: symbols::SymbolHandle,
    state: &State,
    source: &CheckedScalarStateGraph,
    structural: &Arena<CheckedStructuralControlTransferPlan>,
) -> Option<Vec<(PermissionEventSource, facts::PlaceRoot)>> {
    let mut transfers = Vec::new();
    for successor in iter(&source.terminator) {
        let permission_source =
            transition_permission_source(program, machine_symbol, state, successor)?;
        for transfer in structural.span(successor.structural_transfers)? {
            let CheckedStructuralControlTransferSourcePlan::Parameter { index } = transfer.source
            else {
                return None;
            };
            let parameter = source.structural_parameters.get(index as usize)?;
            if parameter.access != CheckedStructuralAccess::Owned
                || parameter.multiplicity != Multiplicity::Affine
            {
                continue;
            }
            let parameter = program
                .state_parameters(state)
                .get(parameter.position as usize)?;
            transfers.push((
                permission_source,
                facts::PlaceRoot::Symbol(parameter.symbol),
            ));
        }
    }
    Some(transfers)
}

fn transition_permission_source(
    program: &TypedTrees,
    machine_symbol: symbols::SymbolHandle,
    state: &State,
    successor: &CheckedScalarSuccessor,
) -> Option<PermissionEventSource> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    let statement_index = successor.statement_ordinal as usize;
    let StatementNode::Transition(transition) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)?
    else {
        return None;
    };
    let destination = if successor.is_continuation {
        transition.continuation
    } else {
        transition.target
    };
    let mut call_ordinal = 0usize;
    while let Some(site) = crate::find_call_site(
        program,
        machine_symbol,
        state.symbol,
        statement_index,
        call_ordinal,
    ) {
        if let crate::CallSite::TransitionNamed { path, .. } = site
            && crate::semantic_calls::transition_call_target(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
            ) == Some(destination)
        {
            return Some(PermissionEventSource::Call {
                statement_index,
                call_ordinal,
                // The ledger retains the authored target, not the normalized
                // entry-state identity used by the executable successor.
                target_symbol: path.symbol,
            });
        }
        call_ordinal = call_ordinal.checked_add(1)?;
    }
    None
}
