//! Exclusive machine-code and object carrier envelopes.

use omega_machine_code::{
    MachineCodeFunction, MachineCodePlan, RankedU32CountdownMachineCodeRecord,
};

use crate::{ObjectArtifact, ObjectFunction};

pub(super) fn validate_machine_code_carrier(
    plan: &MachineCodePlan,
    function: &MachineCodeFunction,
    record: &RankedU32CountdownMachineCodeRecord,
) -> Option<()> {
    (plan.functions.len() == 1
        && plan.entry == function.machine
        && custody_identifies_carrier(plan.psi, function.machine, function.attachment, record)
        && ranked_body_is_exclusive(function))
    .then_some(())
}

pub(super) fn validate_object_carrier(
    artifact: &ObjectArtifact,
    function: &ObjectFunction,
    record: &RankedU32CountdownMachineCodeRecord,
) -> Option<()> {
    (artifact.functions().len() == 1
        && artifact.entry() == function.machine
        && custody_identifies_carrier(
            artifact.psi(),
            function.machine,
            function.attachment,
            record,
        )
        && ranked_object_body_is_exclusive(artifact, function))
    .then_some(())
}

fn custody_identifies_carrier(
    psi: psi_terminal::TerminalPsiIdentity,
    machine: psi_core::MachineId,
    attachment: Option<psi_core::StructuralTypeId>,
    record: &RankedU32CountdownMachineCodeRecord,
) -> bool {
    record.custody.fixed_fuel.terminal_psi() == psi
        && record.custody.fixed_fuel.entry() == machine
        && record.custody.structural_frontiers.machine == machine
        && record.custody.fixed_fuel.schedule()
            == psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity()
        && record.custody.fixed_fuel.ceiling_units() == 5 + 6 * u64::from(u32::MAX)
        && record
            .custody
            .fixed_fuel
            .relevant_preconditions()
            .is_empty()
        && attachment.is_some()
        && record
            .structural_types
            .iter()
            .filter(|declaration| Some(declaration.id) == attachment)
            .count()
            == 1
}

fn ranked_body_is_exclusive(function: &MachineCodeFunction) -> bool {
    function.unit_stack.is_none()
        && function.scalar_stack.is_none()
        && function.internal_calls.is_empty()
        && function.internal_unit_calls.is_empty()
        && function.unit_parameters.is_empty()
        && function.unit_parameter_homes.is_empty()
        && function.unit_affine_cleanup.is_none()
        && function.scalar_affine_cleanup.is_none()
        && function.scalar_control_affine_cleanups.is_empty()
        && function.scalar_structural_parameters.is_empty()
        && function.scalar_structural_parameter_homes.is_empty()
        && function.port_effects.is_empty()
        && function.boundary_settlements.is_empty()
        && function.structural_return.is_none()
}

fn ranked_object_body_is_exclusive(artifact: &ObjectArtifact, function: &ObjectFunction) -> bool {
    function.unit_stack.is_none()
        && function.scalar_stack.is_none()
        && function.unit_call_stacks.is_empty()
        && function.scalar_call_stacks.is_empty()
        && function.internal_unit_calls.is_empty()
        && function.unit_parameters.is_empty()
        && function.unit_parameter_homes.is_empty()
        && function.unit_affine_cleanup.is_none()
        && function.scalar_affine_cleanup.is_none()
        && function.scalar_control_affine_cleanups.is_empty()
        && function.scalar_structural_parameters.is_empty()
        && function.scalar_structural_parameter_homes.is_empty()
        && function.structural_return.is_none()
        && artifact.port_effects().is_empty()
        && artifact.boundary_settlements().is_empty()
}
