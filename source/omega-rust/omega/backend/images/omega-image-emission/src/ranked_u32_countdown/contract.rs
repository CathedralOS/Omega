use omega_calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValueShape, evaluate_call_plan,
};
use omega_machine_code::{
    MachineCodeFunction, MachineCodePlan, RankedU32CountdownMachineCodeRecord,
};
use omega_target::NativeTarget;
use omega_target_operations::{MachineRegister, TerminalPsiProvenance};
use psi_core::{IntegerSign, IntegerType, IntegerValue};
use psi_terminal::{
    StructuralAccess, StructuralMultiplicity, TerminalAffineCleanupAction, TerminalRankedGuard,
    TerminalRankedSuccessorArgument,
};

use crate::ObjectError;

pub(super) fn replay_ranked_countdown_contract(
    plan: &MachineCodePlan,
    function: &MachineCodeFunction,
    record: &RankedU32CountdownMachineCodeRecord,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidRankedCountdown(function.machine);
    if plan.functions.len() != 1
        || plan.entry != function.machine
        || record.custody.fixed_fuel.terminal_psi() != plan.psi
        || record.custody.fixed_fuel.entry() != function.machine
        || record.custody.structural_frontiers.machine != function.machine
        || record.custody.fixed_fuel.schedule()
            != psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity()
        || record.custody.fixed_fuel.ceiling_units() != 5 + 6 * u64::from(u32::MAX)
        || !record
            .custody
            .fixed_fuel
            .relevant_preconditions()
            .is_empty()
        || function.attachment.is_none()
        || record
            .structural_types
            .iter()
            .filter(|declaration| Some(declaration.id) == function.attachment)
            .count()
            != 1
        || !ranked_body_is_exclusive(function)
    {
        return Err(invalid());
    }

    let graph = record.custody.graph;
    let component = &record.custody.ranked_scc;
    let [covered] = component.covered_cyclic_edges.as_slice() else {
        return Err(invalid());
    };
    let TerminalRankedGuard::UnsignedParameterPositive {
        block: guard_block,
        edge: guard_edge,
        parameter: guard_parameter,
        ..
    } = covered.guard;
    let TerminalRankedSuccessorArgument::UnsignedParameterMinusOne {
        argument_index,
        source_parameter,
        target_parameter,
        ..
    } = covered.successor_argument;
    let expected_provenance = TerminalPsiProvenance {
        operations: vec![
            graph.zero_operation,
            graph.compare_operation,
            graph.one_operation,
            graph.subtract_operation,
        ],
        edges: vec![
            graph.preheader_edge,
            guard_edge,
            graph.false_exit_edge,
            covered.edge,
            graph.return_edge,
        ],
    };
    if component.rank_type != IntegerType::new(IntegerSign::Unsigned, 32).expect("u32 is valid")
        || component.lower_bound != IntegerValue::Unsigned(0)
        || component.upper_bound != IntegerValue::Unsigned(u128::from(u32::MAX))
        || covered.target != component.header
        || guard_block != component.header
        || guard_parameter != component.rank_parameter
        || source_parameter != component.rank_parameter
        || target_parameter != component.rank_parameter
        || argument_index != 0
        || graph.entry == component.header
        || graph.done_block == component.header
        || function.provenance != expected_provenance
    {
        return Err(invalid());
    }

    replay_calling_and_structural_contract(plan.target, record).ok_or_else(invalid)?;
    replay_structural_frontier(record).ok_or_else(invalid)?;
    Ok(())
}

fn replay_calling_and_structural_contract(
    target: NativeTarget,
    record: &RankedU32CountdownMachineCodeRecord,
) -> Option<()> {
    let expected_rank_home = if target == NativeTarget::linux_x64() {
        MachineRegister::X86Rdi
    } else if target == NativeTarget::linux_arm64() {
        MachineRegister::Aarch64X(0)
    } else {
        return None;
    };
    let [rank, structural] = record.call_plan.parameters.as_slice() else {
        return None;
    };
    let [structural_parameter] = record.structural_parameters.as_slice() else {
        return None;
    };
    let expected_rank = ValueShape::integer(4, 4);
    if rank.shape != expected_rank
        || rank.locations.as_slice()
            != [ValueLocation::Register {
                register: expected_rank_home,
                value_byte_offset: 0,
                byte_size: 4,
            }]
        || structural_parameter.multiplicity != StructuralMultiplicity::Affine
        || structural_parameter.access != StructuralAccess::Owned
        || structural != &structural_parameter.placement
        || record
            .structural_types
            .iter()
            .filter(|declaration| declaration.id == structural_parameter.structural_type)
            .count()
            != 1
        || record.cleanup_actions.as_slice()
            != [TerminalAffineCleanupAction::DiscardRoot(
                structural_parameter.place,
            )]
    {
        return None;
    }
    let expected = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![expected_rank, structural_parameter.shape],
            result: None,
        },
    )
    .ok()?;
    (record.call_plan == expected).then_some(())
}

fn replay_structural_frontier(record: &RankedU32CountdownMachineCodeRecord) -> Option<()> {
    let component = &record.custody.ranked_scc;
    let covered = &component.covered_cyclic_edges[0];
    let structural = &record.structural_parameters[0];
    let header = record
        .custody
        .structural_frontiers
        .block_entry(component.header)?;
    let backedge = record
        .custody
        .structural_frontiers
        .edge_exit(covered.edge)?;
    let [owned] = header.owned_places() else {
        return None;
    };
    (header == backedge
        && owned.place == structural.place
        && owned.multiplicity == StructuralMultiplicity::Affine
        && header.claims().is_empty()
        && header.partial_custody().is_empty())
    .then_some(())
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
