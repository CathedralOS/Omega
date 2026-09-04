//! Closed scalar observation reconstruction and equality.

use super::*;

pub(crate) fn observation_at(
    unit: &PsiOptimizationUnit,
    location: omega_optimization_unit::NodeLocation,
) -> Option<PsiNodeObservation> {
    reconstruct_psi_observation_model(unit)
        .nodes
        .into_iter()
        .find(|row| {
            row.machine == location.machine
                && row.block == location.block
                && row.node == location.node
        })
}

pub(crate) fn same_closed_scalar_observation(
    input: &PsiNodeObservation,
    output: &PsiNodeObservation,
) -> bool {
    input.machine == output.machine
        && input.block == output.block
        && input.node == output.node
        && input.definitions == output.definitions
        && input.effect == output.effect
        && input.ownership == output.ownership
        && input.provenance == output.provenance
        && input.fuel == output.fuel
        && input.crash == output.crash
        && input.suspension == output.suspension
        && input.events == output.events
}
