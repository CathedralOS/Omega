//! Optimizer module role: validation leaf. Independent Terminal/current component identity replay.

use super::*;

pub(super) fn rederive_exact_components(
    module: &terminal_psi::TerminalModule,
    unit: &PsiOptimizationUnit,
) -> Result<OptimizerCycleComponentSnapshot, OptimizationUnitValidationError> {
    let terminal_psi = terminal_codec::terminal_psi_identity(module)
        .map_err(OptimizationUnitValidationError::ContextIdentity)?;
    let ranked = module
        .machines
        .iter()
        .filter_map(|machine| machine.ranked_scc.as_ref().map(|row| (machine, row)))
        .collect::<Vec<_>>();
    if ranked.is_empty() {
        return Ok(OptimizerCycleComponentSnapshot {
            terminal_psi,
            components: Vec::new(),
        });
    }
    let [(machine, ranked)] = ranked.as_slice() else {
        return Err(topology_mismatch(module.entry));
    };
    let terminal = topology::derive_components(&graph::terminal_graph(machine));
    let [terminal_component] = terminal.as_slice() else {
        return Err(topology_mismatch(machine.id));
    };
    let covered = ranked
        .covered_cyclic_edges
        .iter()
        .map(|edge| CycleComponentEdge {
            edge: edge.edge,
            source: edge.source,
            target: edge.target,
        })
        .collect::<BTreeSet<_>>();
    if !terminal_component.members.contains(&ranked.header)
        || !covered
            .iter()
            .all(|edge| terminal_component.id.internal_edges.contains(edge))
    {
        return Err(topology_mismatch(machine.id));
    }

    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine.id)
        .ok_or(OptimizationUnitValidationError::RankedCycleFunctionMissing(
            machine.id,
        ))?;
    let current = topology::derive_components(&graph::optimization_graph(function));
    let [current_component] = current.as_slice() else {
        return Err(topology_mismatch(machine.id));
    };
    if current_component.id != terminal_component.id
        || current_component.members != terminal_component.members
    {
        return Err(topology_mismatch(machine.id));
    }
    Ok(OptimizerCycleComponentSnapshot {
        terminal_psi,
        components: current,
    })
}

fn topology_mismatch(machine: MachineId) -> OptimizationUnitValidationError {
    OptimizationUnitValidationError::RankedCycleTopologyMismatch { machine }
}
