//! Independent object-boundary replay for the exact ranked `u32` countdown.

use omega_calling_conventions::{
    evaluate_call_plan, CallSignature, CallingPolicy, ValueLocation, ValueShape,
};
use omega_machine_code::{
    MachineCodeFunction, NativeFuelAttribution, NativeFuelSite, RankedU32CountdownMachineCodeRecord,
};
use omega_target::NativeTarget;
use omega_target_operations::{MachineRegister, TerminalPsiProvenance};
use psi_core::{IntegerSign, IntegerType, IntegerValue};
use psi_terminal::{
    StructuralAccess, StructuralMultiplicity, TerminalAffineCleanupAction, TerminalPsiIdentity,
    TerminalRankedGuard, TerminalRankedSuccessorArgument,
};
use psi_terminal_fuel::TerminalFuelSchedule;

use crate::ObjectError;

#[derive(Clone, Copy)]
struct RankedLayout {
    preheader_branch_offset: usize,
    preheader_branch_byte_count: usize,
    header_offset: usize,
    compare_offset: usize,
    compare_byte_count: usize,
    positive_path_offset: usize,
    decrement_offset: usize,
    decrement_byte_count: usize,
    backward_branch_offset: usize,
    backward_branch_byte_count: usize,
    exit_offset: usize,
    return_offset: usize,
    return_byte_count: usize,
}

const X86_64_LAYOUT: RankedLayout = RankedLayout {
    preheader_branch_offset: omega_isa_x86_64::X86_64_RANKED_U32_PREHEADER_BRANCH_OFFSET,
    preheader_branch_byte_count: omega_isa_x86_64::X86_64_RANKED_U32_PREHEADER_BRANCH_BYTE_COUNT,
    header_offset: omega_isa_x86_64::X86_64_RANKED_U32_HEADER_OFFSET,
    compare_offset: omega_isa_x86_64::X86_64_RANKED_U32_COMPARE_OFFSET,
    compare_byte_count: omega_isa_x86_64::X86_64_RANKED_U32_COMPARE_BYTE_COUNT,
    positive_path_offset: omega_isa_x86_64::X86_64_RANKED_U32_POSITIVE_PATH_OFFSET,
    decrement_offset: omega_isa_x86_64::X86_64_RANKED_U32_DECREMENT_OFFSET,
    decrement_byte_count: omega_isa_x86_64::X86_64_RANKED_U32_DECREMENT_BYTE_COUNT,
    backward_branch_offset: omega_isa_x86_64::X86_64_RANKED_U32_BACKWARD_BRANCH_OFFSET,
    backward_branch_byte_count: omega_isa_x86_64::X86_64_RANKED_U32_BACKWARD_BRANCH_BYTE_COUNT,
    exit_offset: omega_isa_x86_64::X86_64_RANKED_U32_EXIT_OFFSET,
    return_offset: omega_isa_x86_64::X86_64_RANKED_U32_RETURN_OFFSET,
    return_byte_count: omega_isa_x86_64::X86_64_RANKED_U32_RETURN_BYTE_COUNT,
};

const AARCH64_LAYOUT: RankedLayout = RankedLayout {
    preheader_branch_offset: omega_isa_aarch64::AARCH64_RANKED_U32_PREHEADER_BRANCH_OFFSET,
    preheader_branch_byte_count: omega_isa_aarch64::AARCH64_RANKED_U32_PREHEADER_BRANCH_BYTE_COUNT,
    header_offset: omega_isa_aarch64::AARCH64_RANKED_U32_HEADER_OFFSET,
    compare_offset: omega_isa_aarch64::AARCH64_RANKED_U32_COMPARE_OFFSET,
    compare_byte_count: omega_isa_aarch64::AARCH64_RANKED_U32_COMPARE_BYTE_COUNT,
    positive_path_offset: omega_isa_aarch64::AARCH64_RANKED_U32_POSITIVE_PATH_OFFSET,
    decrement_offset: omega_isa_aarch64::AARCH64_RANKED_U32_DECREMENT_OFFSET,
    decrement_byte_count: omega_isa_aarch64::AARCH64_RANKED_U32_DECREMENT_BYTE_COUNT,
    backward_branch_offset: omega_isa_aarch64::AARCH64_RANKED_U32_BACKWARD_BRANCH_OFFSET,
    backward_branch_byte_count: omega_isa_aarch64::AARCH64_RANKED_U32_BACKWARD_BRANCH_BYTE_COUNT,
    exit_offset: omega_isa_aarch64::AARCH64_RANKED_U32_EXIT_OFFSET,
    return_offset: omega_isa_aarch64::AARCH64_RANKED_U32_RETURN_OFFSET,
    return_byte_count: omega_isa_aarch64::AARCH64_RANKED_U32_RETURN_BYTE_COUNT,
};

pub(crate) fn validate_ranked_countdown(
    function: &MachineCodeFunction,
    psi: TerminalPsiIdentity,
    target: NativeTarget,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidRankedCountdownEvidence(function.machine);
    let record = function.ranked_u32_countdown.as_ref().ok_or_else(invalid)?;
    let (expected_bytes, layout, rank_home) = if target == NativeTarget::linux_x64() {
        (
            omega_isa_x86_64::encode_ranked_u32_countdown_in_edi().to_vec(),
            X86_64_LAYOUT,
            MachineRegister::X86Rdi,
        )
    } else if target == NativeTarget::linux_arm64() {
        (
            omega_isa_aarch64::encode_ranked_u32_countdown_in_w0().to_vec(),
            AARCH64_LAYOUT,
            MachineRegister::Aarch64X(0),
        )
    } else {
        return Err(invalid());
    };
    if function.bytes != expected_bytes {
        return Err(ObjectError::RankedCountdownBytesMismatch(function.machine));
    }
    if function.unit_stack.is_some()
        || !function.unit_parameter_homes.is_empty()
        || !function.unit_parameters.is_empty()
        || function.scalar_stack.is_some()
        || !function.internal_calls.is_empty()
        || !function.internal_unit_calls.is_empty()
        || function.unit_affine_cleanup.is_some()
        || function.scalar_affine_cleanup.is_some()
        || !function.scalar_control_affine_cleanups.is_empty()
        || !function.scalar_structural_parameters.is_empty()
        || !function.scalar_structural_parameter_homes.is_empty()
        || !function.port_effects.is_empty()
        || !function.boundary_settlements.is_empty()
        || function.structural_return.is_some()
    {
        return Err(ObjectError::RankedCountdownEvidenceConflict(
            function.machine,
        ));
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
    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32 is valid");
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
    if record.custody.fixed_fuel.terminal_psi() != psi
        || record.custody.fixed_fuel.entry() != function.machine
        || record.custody.fixed_fuel.schedule() != TerminalFuelSchedule::CURRENT.identity()
        || record.custody.fixed_fuel.ceiling_units() != 5 + 6 * u64::from(u32::MAX)
        || !record
            .custody
            .fixed_fuel
            .relevant_preconditions()
            .is_empty()
        || function.provenance != expected_provenance
        || component.rank_type != u32_type
        || component.lower_bound != IntegerValue::Unsigned(0)
        || component.upper_bound != IntegerValue::Unsigned(u128::from(u32::MAX))
        || covered.target != component.header
        || guard_block != component.header
        || guard_parameter != component.rank_parameter
        || source_parameter != component.rank_parameter
        || target_parameter != component.rank_parameter
        || argument_index != 0
        || record.call_plan.parameters.len() != 2
        || record.structural_parameters.len() != 1
    {
        return Err(invalid());
    }

    let rank_placement = &record.call_plan.parameters[0];
    if rank_placement.shape != ValueShape::integer(4, 4)
        || rank_placement.locations.as_slice()
            != [ValueLocation::Register {
                register: rank_home,
                value_byte_offset: 0,
                byte_size: 4,
            }]
    {
        return Err(invalid());
    }
    let structural = &record.structural_parameters[0];
    let expected_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![ValueShape::integer(4, 4), structural.shape],
            result: None,
        },
    )
    .map_err(|_| invalid())?;
    if structural.multiplicity != StructuralMultiplicity::Affine
        || structural.access != StructuralAccess::Owned
        || record.call_plan != expected_call_plan
        || record.call_plan.parameters[1] != structural.placement
        || !record
            .structural_types
            .iter()
            .any(|declaration| declaration.id == structural.structural_type)
        || record.cleanup_actions.as_slice()
            != [TerminalAffineCleanupAction::DiscardRoot(structural.place)]
    {
        return Err(invalid());
    }
    let header_frontier = record
        .custody
        .structural_frontiers
        .block_entry(component.header)
        .ok_or_else(invalid)?;
    let backedge_frontier = record
        .custody
        .structural_frontiers
        .edge_exit(covered.edge)
        .ok_or_else(invalid)?;
    let [owned] = header_frontier.owned_places() else {
        return Err(invalid());
    };
    if header_frontier != backedge_frontier
        || owned.place != structural.place
        || owned.multiplicity != StructuralMultiplicity::Affine
        || !header_frontier.claims().is_empty()
        || !header_frontier.partial_custody().is_empty()
        || graph.entry == component.header
        || graph.done_block == component.header
    {
        return Err(invalid());
    }

    if function.fuel_attribution != expected_fuel(record, layout) {
        return Err(invalid());
    }
    Ok(())
}

fn expected_fuel(
    record: &RankedU32CountdownMachineCodeRecord,
    layout: RankedLayout,
) -> Vec<NativeFuelAttribution> {
    let graph = record.custody.graph;
    let covered = &record.custody.ranked_scc.covered_cyclic_edges[0];
    let TerminalRankedGuard::UnsignedParameterPositive {
        edge: guard_edge, ..
    } = covered.guard;
    let schedule = record.custody.fixed_fuel.schedule();
    [
        (
            NativeFuelSite::Edge(graph.preheader_edge),
            layout.preheader_branch_offset,
            layout.preheader_branch_byte_count,
        ),
        (
            NativeFuelSite::Operation(graph.zero_operation),
            layout.header_offset,
            0,
        ),
        (
            NativeFuelSite::Operation(graph.compare_operation),
            layout.compare_offset,
            layout.compare_byte_count,
        ),
        (
            NativeFuelSite::Edge(guard_edge),
            layout.positive_path_offset,
            0,
        ),
        (
            NativeFuelSite::Operation(graph.one_operation),
            layout.positive_path_offset,
            0,
        ),
        (
            NativeFuelSite::Operation(graph.subtract_operation),
            layout.decrement_offset,
            layout.decrement_byte_count,
        ),
        (
            NativeFuelSite::Edge(covered.edge),
            layout.backward_branch_offset,
            layout.backward_branch_byte_count,
        ),
        (
            NativeFuelSite::Edge(graph.false_exit_edge),
            layout.exit_offset,
            0,
        ),
        (
            NativeFuelSite::Edge(graph.return_edge),
            layout.return_offset,
            layout.return_byte_count,
        ),
    ]
    .into_iter()
    .enumerate()
    .map(
        |(operation_ordinal, (site, code_offset, byte_count))| NativeFuelAttribution {
            schedule,
            site,
            units: 1,
            operation_ordinal,
            code_offset,
            byte_count,
        },
    )
    .collect()
}
