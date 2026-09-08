//! Preserve verified Natural and unranked components under exact source custody.
use super::*;

pub(super) fn rederive_components(
    module: &terminal_psi::TerminalModule,
    unit: &PsiOptimizationUnit,
    terminal_psi: terminal_psi::TerminalPsiIdentity,
) -> Result<OptimizerCycleComponentSnapshot, OptimizationUnitValidationError> {
    let mut components = Vec::new();
    for machine in &module.machines {
        let invalid = || OptimizationUnitValidationError::RankedCycleTopologyMismatch {
            machine: machine.id,
        };
        if !matches!(
            machine.ranked_scc,
            None | Some(terminal_psi::TerminalRankedScc::Natural(_))
        ) {
            // The legacy countdown entrance remains a separate, entry-only contract.
            return Err(invalid());
        }
        // VerifiedPsiOptimizationInput authenticates this Terminal body and its
        // safety evidence, including grouped Natural evidence when present.
        // Reconstruct topology independently of current IR; unranked components
        // acquire no termination or work certificate.
        let terminal = topology::derive_components(&graph::terminal_graph(machine));
        if terminal.is_empty() {
            // Acyclic source machines retain their existing pruning rules.
            // A new current cycle there receives no admitted-machine authority.
            continue;
        }
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
