//! Project the Nat checker's judgment without recognizing another rank syntax.

use checked_trees::{
    CheckedScalarMachineGraph, CheckedStructuralRankedArgumentPlan,
    CheckedStructuralRankedGuardPlan, CheckedStructuralRankedSccEdgePlan,
    CheckedStructuralRankedSccPlan,
};
use typed_trees::TypedTrees;

pub(super) fn plan(
    program: &TypedTrees,
    graph: &CheckedScalarMachineGraph,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Option<CheckedStructuralRankedSccPlan>> {
    let machine = crate::lookup::machine_by_symbol(program, graph.machine)?;
    if machine.termination_plan.implementation_witness.is_none() {
        return Some(None);
    }
    let Some(components) = crate::checks::termination::proven_nat_countdown_sccs_with_call_frames(
        program,
        machine,
        call_frames,
    ) else {
        // The Nat countdown is only one ranking order. A cyclic machine proven
        // under another order (`Slice::Length`, struct views) leaves its
        // judgment in the shared natural ranks; this lane publishes a ranked
        // SCC plan only for the countdown shape it can spell exactly.
        return if crate::checks::termination::proven_state_natural_ranks_with_call_frames(
            program,
            machine,
            call_frames,
        )
        .is_some_and(|ranks| !ranks.is_empty())
        {
            Some(None)
        } else {
            None
        };
    };
    // A machine-local scan cannot see a cycle spelled through sibling machine
    // successors; the fused row set is the SCC carrier in that shape.
    let components = if components.is_empty() && graph.states.len() > 1 {
        crate::checks::termination::proven_fused_nat_countdown_sccs_with_call_frames(
            program,
            &graph
                .states
                .iter()
                .map(|state| state.state)
                .collect::<Vec<_>>(),
            call_frames,
        )?
    } else {
        components
    };
    if components.is_empty() {
        return Some(None);
    }
    let [component] = components.as_slice() else {
        return None;
    };
    // The fused graph holds every state the SCC's edges can reach, so the
    // header is the row carrying the component's header state rather than the
    // graph's only row; each covered edge resolves its own source and target
    // rows inside the same fused list.
    let header = graph
        .states
        .iter()
        .find(|state| state.state == component.header_state)?;
    let rank_scalar_parameter_index = header.scalar_parameters.iter().position(|parameter| {
        parameter.source_position == component.header_rank_parameter_position
            && parameter.primitive_type == component.rank_primitive_type
    })?;
    let mut edges = Vec::new();
    for edge in &component.covered_cyclic_edges {
        let source = graph
            .states
            .iter()
            .find(|state| state.state == edge.source_state)?;
        let target = graph
            .states
            .iter()
            .find(|state| state.state == edge.target_state)?;
        let successor = super::successors::iter(&source.terminator).find(|successor| {
            successor.statement_ordinal == edge.statement_ordinal
                && successor.target == edge.target_state
                && !successor.is_continuation
        })?;
        let source_scalar_parameter_index =
            source.scalar_parameters.iter().position(|parameter| {
                parameter.source_position == edge.source_rank_parameter_position
                    && parameter.primitive_type == component.rank_primitive_type
            })?;
        let target_scalar_parameter_index =
            target.scalar_parameters.iter().position(|parameter| {
                parameter.source_position == edge.target_rank_parameter_position
                    && parameter.primitive_type == component.rank_primitive_type
            })?;
        // The edge names authored formal positions, while the successor's
        // `argument_count` counts authored actuals — an ambient borrowed `self`
        // owns a formal position but no actual, so bound the coordinate against
        // the target state's authored parameter count instead. The target may
        // live on a fused successor machine, so resolve its owner by state.
        let (_, target_state) =
            crate::semantic::calls::find_state_with_machine(program, edge.target_state)?;
        let target_parameters = program.state_parameters(target_state);
        if successor.argument_count as usize
            != target_parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count()
            || edge.target_rank_parameter_position as usize >= target_parameters.len()
        {
            return None;
        }
        edges.push(CheckedStructuralRankedSccEdgePlan {
            source_state: edge.source_state,
            target_state: edge.target_state,
            statement_ordinal: edge.statement_ordinal,
            guard: CheckedStructuralRankedGuardPlan::UnsignedParameterPositive {
                scalar_parameter_index: u32::try_from(source_scalar_parameter_index).ok()?,
                primitive_type: component.rank_primitive_type,
            },
            successor_argument: CheckedStructuralRankedArgumentPlan::UnsignedParameterMinusOne {
                argument_ordinal: edge.target_rank_parameter_position,
                source_scalar_parameter_index: u32::try_from(source_scalar_parameter_index).ok()?,
                target_scalar_parameter_index: u32::try_from(target_scalar_parameter_index).ok()?,
                primitive_type: component.rank_primitive_type,
            },
        });
    }
    Some(Some(CheckedStructuralRankedSccPlan {
        header_state: header.state,
        rank_scalar_parameter_index: u32::try_from(rank_scalar_parameter_index).ok()?,
        rank_primitive_type: component.rank_primitive_type,
        rank_lower_bound: component.rank_lower_bound,
        rank_upper_bound: component.rank_upper_bound,
        covered_cyclic_edges: edges,
    }))
}
