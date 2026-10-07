//! Preserve verified Natural and unranked components under exact source custody.

use super::{
    OptimizationUnitValidationError, OptimizerCycleComponentSnapshot, PsiOptimizationUnit, graph,
    topology,
};
pub(super) fn rederive_components(
    module: &terminal_psi::TerminalModule,
    seed_functions: &[abstract_operations::AbstractFunction],
    unit: &PsiOptimizationUnit,
    terminal_psi: terminal_psi::TerminalPsiIdentity,
) -> Result<OptimizerCycleComponentSnapshot, OptimizationUnitValidationError> {
    let mut components = Vec::new();
    // The admitted seed is the immutable invocation scope. Canonical Terminal
    // may retain other cyclic bodies, but a later revision must not silently
    // discard a cyclic body that this invocation admitted into its seed.
    for seed in seed_functions {
        let machine = module
            .machines
            .iter()
            .find(|machine| machine.id == seed.machine)
            .ok_or(
                OptimizationUnitValidationError::RankedCycleTopologyMismatch {
                    machine: seed.machine,
                },
            )?;
        let invalid = || OptimizationUnitValidationError::RankedCycleTopologyMismatch {
            machine: machine.id,
        };
        // VerifiedPsiOptimizationInput authenticates this Terminal body and its
        // safety evidence, including grouped Natural evidence when present.
        // The verifier's canonical cyclic-component surface owns Terminal-side
        // member identity — the topology every retained Natural row was
        // validated against — while the current optimizer body is still
        // reconstructed independently below and must equal it exactly.
        // Unranked components acquire no termination or work certificate.
        let terminal = topology::derive_terminal_components(machine)?;
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
