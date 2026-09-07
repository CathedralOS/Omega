//! Exact machine-code emission for the first admitted ranked countdown.

use assigned_target_operations::{AssignedFunction, AssignedRankedU32Countdown};
use calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValueShape, evaluate_call_plan,
};
use machine_code::{
    MachineCodeFunction, RankedU32CountdownMachineCodeRecord, SemanticCodeAttribution,
    SemanticCodeSite,
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use target::NativeTarget;
use target_operations::MachineRegister;
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, TerminalAffineCleanupAction, TerminalPsiIdentity,
    TerminalRankedGuard, TerminalRankedSuccessorArgument,
};

use crate::EmissionError;

#[derive(Debug, Clone, Copy)]
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
    preheader_branch_offset: isa_x86_64::X86_64_RANKED_U32_PREHEADER_BRANCH_OFFSET,
    preheader_branch_byte_count: isa_x86_64::X86_64_RANKED_U32_PREHEADER_BRANCH_BYTE_COUNT,
    header_offset: isa_x86_64::X86_64_RANKED_U32_HEADER_OFFSET,
    compare_offset: isa_x86_64::X86_64_RANKED_U32_COMPARE_OFFSET,
    compare_byte_count: isa_x86_64::X86_64_RANKED_U32_COMPARE_BYTE_COUNT,
    positive_path_offset: isa_x86_64::X86_64_RANKED_U32_POSITIVE_PATH_OFFSET,
    decrement_offset: isa_x86_64::X86_64_RANKED_U32_DECREMENT_OFFSET,
    decrement_byte_count: isa_x86_64::X86_64_RANKED_U32_DECREMENT_BYTE_COUNT,
    backward_branch_offset: isa_x86_64::X86_64_RANKED_U32_BACKWARD_BRANCH_OFFSET,
    backward_branch_byte_count: isa_x86_64::X86_64_RANKED_U32_BACKWARD_BRANCH_BYTE_COUNT,
    exit_offset: isa_x86_64::X86_64_RANKED_U32_EXIT_OFFSET,
    return_offset: isa_x86_64::X86_64_RANKED_U32_RETURN_OFFSET,
    return_byte_count: isa_x86_64::X86_64_RANKED_U32_RETURN_BYTE_COUNT,
};

const AARCH64_LAYOUT: RankedLayout = RankedLayout {
    preheader_branch_offset: isa_aarch64::AARCH64_RANKED_U32_PREHEADER_BRANCH_OFFSET,
    preheader_branch_byte_count: isa_aarch64::AARCH64_RANKED_U32_PREHEADER_BRANCH_BYTE_COUNT,
    header_offset: isa_aarch64::AARCH64_RANKED_U32_HEADER_OFFSET,
    compare_offset: isa_aarch64::AARCH64_RANKED_U32_COMPARE_OFFSET,
    compare_byte_count: isa_aarch64::AARCH64_RANKED_U32_COMPARE_BYTE_COUNT,
    positive_path_offset: isa_aarch64::AARCH64_RANKED_U32_POSITIVE_PATH_OFFSET,
    decrement_offset: isa_aarch64::AARCH64_RANKED_U32_DECREMENT_OFFSET,
    decrement_byte_count: isa_aarch64::AARCH64_RANKED_U32_DECREMENT_BYTE_COUNT,
    backward_branch_offset: isa_aarch64::AARCH64_RANKED_U32_BACKWARD_BRANCH_OFFSET,
    backward_branch_byte_count: isa_aarch64::AARCH64_RANKED_U32_BACKWARD_BRANCH_BYTE_COUNT,
    exit_offset: isa_aarch64::AARCH64_RANKED_U32_EXIT_OFFSET,
    return_offset: isa_aarch64::AARCH64_RANKED_U32_RETURN_OFFSET,
    return_byte_count: isa_aarch64::AARCH64_RANKED_U32_RETURN_BYTE_COUNT,
};

pub(super) fn emit(
    function: &AssignedFunction,
    countdown: &AssignedRankedU32Countdown,
    psi: TerminalPsiIdentity,
    target: NativeTarget,
) -> Result<MachineCodeFunction, EmissionError> {
    validate(function, countdown, psi, target)?;

    let (bytes, record) = if target == NativeTarget::linux_x64() {
        let bytes = isa_x86_64::encode_ranked_u32_countdown_in_edi().to_vec();
        let record = record(countdown);
        (bytes, record)
    } else if target == NativeTarget::linux_arm64() {
        let bytes = isa_aarch64::encode_ranked_u32_countdown_in_w0().to_vec();
        let record = record(countdown);
        (bytes, record)
    } else {
        return Err(EmissionError::InvalidRankedCountdown(function.machine));
    };
    let layout = if target == NativeTarget::linux_x64() {
        X86_64_LAYOUT
    } else {
        AARCH64_LAYOUT
    };
    let semantic_code_attribution = semantic_code_attribution(&record.custody, layout);

    Ok(MachineCodeFunction {
        machine: function.machine,
        attachment: function.attachment,
        fixed_integer_scalar_abi: function.fixed_integer_scalar_abi.clone(),
        mixed_structural_scalar_abi: function.mixed_structural_scalar_abi.clone(),
        structural_call_scalar_return: None,
        unit_scalar_abi: None,
        provenance: function.provenance.clone(),
        bytes,
        x86_scalar_fma: Vec::new(),
        x86_scalar_fma_occurrences: Vec::new(),
        x86_floating_control: None,
        unit_stack: None,
        unit_parameter_homes: Vec::new(),
        unit_parameters: Vec::new(),
        scalar_stack: None,
        internal_calls: Vec::new(),
        foreign_calls: Vec::new(),
        internal_unit_calls: Vec::new(),
        internal_unit_scalar_calls: Vec::new(),
        installed_provider_unit_scalar_calls: Vec::new(),
        dynamic_calls: Vec::new(),
        stored_dynamic_calls: Vec::new(),
        dynamic_parameter_calls: Vec::new(),
        forwarded_dynamic_parameter_calls: Vec::new(),
        forwarded_dynamic_descriptor_calls: Vec::new(),
        unit_scalar_homes: Vec::new(),
        unit_integer_constants: Vec::new(),
        unit_affine_scalar_records: Vec::new(),
        unit_structural_scalar_field_stores: Vec::new(),
        unit_write_only_primitive_stores: Vec::new(),
        scalar_structural_scalar_field_stores: Vec::new(),
        unit_affine_cleanup: None,
        scalar_affine_cleanup: None,
        scalar_control_affine_cleanups: Vec::new(),
        scalar_structural_parameters: Vec::new(),
        scalar_structural_parameter_homes: Vec::new(),
        ranked_u32_countdown: Some(record),
        semantic_code_attribution,
        port_effects: Vec::new(),
        boundary_settlements: Vec::new(),
        structural_return: None,
    })
}

fn record(countdown: &AssignedRankedU32Countdown) -> RankedU32CountdownMachineCodeRecord {
    RankedU32CountdownMachineCodeRecord {
        custody: countdown.custody.clone(),
        call_plan: countdown.call_plan.clone(),
        structural_types: countdown.structural_types.clone(),
        structural_parameters: countdown.structural_parameters.clone(),
        cleanup_actions: countdown.cleanup_actions.clone(),
    }
}

fn semantic_code_attribution(
    custody: &target_operations::RankedU32CountdownCustody,
    layout: RankedLayout,
) -> Vec<SemanticCodeAttribution> {
    let graph = custody.graph;
    let component = &custody.ranked_scc;
    let covered = &component.covered_cyclic_edges[0];
    let TerminalRankedGuard::UnsignedParameterPositive {
        edge: guard_edge, ..
    } = covered.guard;
    [
        (
            SemanticCodeSite::Edge(graph.preheader_edge),
            layout.preheader_branch_offset,
            layout.preheader_branch_byte_count,
        ),
        (
            SemanticCodeSite::Operation(graph.zero_operation),
            layout.header_offset,
            0,
        ),
        (
            SemanticCodeSite::Operation(graph.compare_operation),
            layout.compare_offset,
            layout.compare_byte_count,
        ),
        (
            SemanticCodeSite::Edge(guard_edge),
            layout.positive_path_offset,
            0,
        ),
        (
            SemanticCodeSite::Operation(graph.one_operation),
            layout.positive_path_offset,
            0,
        ),
        (
            SemanticCodeSite::Operation(graph.subtract_operation),
            layout.decrement_offset,
            layout.decrement_byte_count,
        ),
        (
            SemanticCodeSite::Edge(covered.edge),
            layout.backward_branch_offset,
            layout.backward_branch_byte_count,
        ),
        (
            SemanticCodeSite::Edge(graph.false_exit_edge),
            layout.exit_offset,
            0,
        ),
        (
            SemanticCodeSite::Edge(graph.return_edge),
            layout.return_offset,
            layout.return_byte_count,
        ),
    ]
    .into_iter()
    .enumerate()
    .map(
        |(operation_ordinal, (site, code_offset, byte_count))| SemanticCodeAttribution {
            site,
            operation_ordinal,
            code_offset,
            byte_count,
        },
    )
    .collect()
}

fn validate(
    function: &AssignedFunction,
    countdown: &AssignedRankedU32Countdown,
    psi: TerminalPsiIdentity,
    target: NativeTarget,
) -> Result<(), EmissionError> {
    let invalid = || EmissionError::InvalidRankedCountdown(function.machine);
    if target != NativeTarget::linux_x64() && target != NativeTarget::linux_arm64() {
        return Err(invalid());
    }
    let graph = countdown.custody.graph;
    let component = &countdown.custody.ranked_scc;
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
    let expected_rank_home = if target == NativeTarget::linux_x64() {
        MachineRegister::X86Rdi
    } else {
        MachineRegister::Aarch64X(0)
    };
    let expected_provenance = target_operations::TerminalPsiProvenance {
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
    if countdown.custody.fixed_fuel.terminal_psi() != psi
        || countdown.custody.fixed_fuel.entry() != function.machine
        || countdown.custody.fixed_fuel.schedule()
            != terminal_fuel::TerminalFuelSchedule::CURRENT.identity()
        || countdown.custody.fixed_fuel.ceiling_units() != 5 + 6 * u64::from(u32::MAX)
        || !countdown
            .custody
            .fixed_fuel
            .relevant_preconditions()
            .is_empty()
        || function.provenance != expected_provenance
        || countdown.rank_home != expected_rank_home
        || component.rank_type != u32_type
        || component.lower_bound != IntegerValue::Unsigned(0)
        || component.upper_bound != IntegerValue::Unsigned(u128::from(u32::MAX))
        || covered.target != component.header
        || guard_block != component.header
        || guard_parameter != component.rank_parameter
        || source_parameter != component.rank_parameter
        || target_parameter != component.rank_parameter
        || argument_index != 0
        || countdown.call_plan.parameters.len() != 2
        || countdown.structural_parameters.len() != 1
    {
        return Err(invalid());
    }
    let rank_placement = &countdown.call_plan.parameters[0];
    if rank_placement.shape != ValueShape::integer(4, 4)
        || rank_placement.locations.as_slice()
            != [ValueLocation::Register {
                register: expected_rank_home,
                value_byte_offset: 0,
                byte_size: 4,
            }]
    {
        return Err(invalid());
    }
    let structural = &countdown.structural_parameters[0];
    let [replay_machine] = countdown.custody.semantic_replay.machines.as_slice() else {
        return Err(invalid());
    };
    let [replay_structural] = replay_machine.structural_parameters.as_slice() else {
        return Err(invalid());
    };
    let affine_owned = !replay_structural.is_self
        && replay_structural.multiplicity == StructuralMultiplicity::Affine
        && replay_structural.access == StructuralAccess::Owned;
    let persistent_receiver =
        replay_structural.is_self && replay_structural.access == StructuralAccess::MutableBorrow;
    let expected_structural_shape = if persistent_receiver {
        let declarations = &countdown.custody.semantic_replay.structural_types;
        if countdown.structural_types != *declarations {
            return Err(invalid());
        }
        let referent = crate::unit::replay_finite_material_shape(
            declarations,
            replay_structural.structural_type,
        )
        .ok_or_else(invalid)?;
        ValueShape::borrowed_reference(referent.byte_size, referent.alignment)
    } else {
        structural.shape
    };
    let expected_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![ValueShape::integer(4, 4), expected_structural_shape],
            result: None,
        },
    )
    .map_err(|_| invalid())?;
    if structural.place != replay_structural.place
        || structural.structural_type != replay_structural.structural_type
        || structural.multiplicity != replay_structural.multiplicity
        || structural.access != replay_structural.access
        || (!affine_owned && !persistent_receiver)
        || structural.shape != expected_structural_shape
        || countdown.call_plan != expected_call_plan
        || countdown.call_plan.parameters[1] != structural.placement
        || !countdown
            .structural_types
            .iter()
            .any(|declaration| declaration.id == structural.structural_type)
        || (affine_owned
            && countdown.cleanup_actions.as_slice()
                != [TerminalAffineCleanupAction::DiscardRoot(structural.place)])
        || (persistent_receiver && !countdown.cleanup_actions.is_empty())
    {
        return Err(invalid());
    }
    let header_frontier = countdown
        .custody
        .structural_frontiers
        .block_entry(component.header)
        .ok_or_else(invalid)?;
    let backedge_frontier = countdown
        .custody
        .structural_frontiers
        .edge_exit(covered.edge)
        .ok_or_else(invalid)?;
    let affine_frontier = matches!(header_frontier.owned_places(), [owned]
        if owned.place == structural.place
            && owned.multiplicity == StructuralMultiplicity::Affine);
    let receiver_frontier = header_frontier.owned_places().is_empty();
    if header_frontier != backedge_frontier
        || (affine_owned && !affine_frontier)
        || (persistent_receiver && !receiver_frontier)
        || !header_frontier.claims().is_empty()
        || !header_frontier.partial_custody().is_empty()
        || graph.entry == component.header
        || graph.done_block == component.header
    {
        return Err(invalid());
    }
    Ok(())
}
