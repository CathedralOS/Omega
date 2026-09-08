//! Preserve verifier-admitted natural components without inventing countdown evidence.
use super::*;

pub(super) fn rederive_components(
    module: &terminal_psi::TerminalModule,
    unit: &PsiOptimizationUnit,
    terminal_psi: terminal_psi::TerminalPsiIdentity,
) -> Result<OptimizerCycleComponentSnapshot, OptimizationUnitValidationError> {
    let mut components = Vec::new();
    for machine in &module.machines {
        let Some(ranking) = &machine.ranked_scc else {
            continue;
        };
        let invalid = || OptimizationUnitValidationError::RankedCycleTopologyMismatch {
            machine: machine.id,
        };
        let terminal_psi::TerminalRankedScc::Natural(_) = ranking else {
            // The legacy countdown entrance remains a separate, entry-only contract.
            return Err(invalid());
        };
        // VerifiedPsiOptimizationInput already authenticates every grouped
        // natural proof against this Terminal body. Reconstruct topology from
        // that body, independently of current IR, without a second rank solver.
        let terminal = topology::derive_components(&graph::terminal_graph(machine));
        let function = unit
            .functions
            .iter()
            .find(|function| function.machine == machine.id)
            .ok_or_else(invalid)?;
        let current = topology::derive_components(&graph::optimization_graph(function));
        if current != terminal {
            return Err(invalid());
        }
        components.extend(current);
    }
    components.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(OptimizerCycleComponentSnapshot {
        terminal_psi,
        components,
    })
}

pub(super) fn is_natural(module: &terminal_psi::TerminalModule, machine: MachineId) -> bool {
    module.machines.iter().any(|candidate| {
        candidate.id == machine
            && matches!(
                candidate.ranked_scc,
                Some(terminal_psi::TerminalRankedScc::Natural(_))
            )
    })
}
