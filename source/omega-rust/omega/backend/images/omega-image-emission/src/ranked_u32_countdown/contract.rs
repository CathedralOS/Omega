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
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralMultiplicity,
    TerminalAffineCleanupAction, TerminalMachine, TerminalRankedGuard,
    TerminalRankedSuccessorArgument, Terminator,
};

use crate::{ObjectArtifact, ObjectError, ObjectFunction};

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
    replay_verifier_custody(record).ok_or_else(invalid)?;

    let graph = record.custody.graph;
    let component = &record.custody.ranked_scc;
    let replay = &record.custody.semantic_replay;
    let [replay_machine] = replay.machines.as_slice() else {
        return Err(invalid());
    };
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
        || psi_terminal_codec::terminal_psi_identity(replay).ok() != Some(plan.psi)
        || replay.entry != function.machine
        || replay_machine.id != function.machine
        || replay_machine.attachment != function.attachment
        || replay_machine.ranked_scc.as_ref() != Some(component)
        || replay.structural_types != record.structural_types
        || !replay_ranked_graph_matches(replay_machine, record)
    {
        return Err(invalid());
    }
    replay_calling_and_structural_contract(plan.target, record).ok_or_else(invalid)?;
    replay_structural_frontier(record).ok_or_else(invalid)?;
    Ok(())
}

fn replay_verifier_custody(record: &RankedU32CountdownMachineCodeRecord) -> Option<()> {
    let custody = &record.custody;
    let proof = psi_terminal_codec::decode_proof_bundle(&custody.proof_replay).ok()?;
    let profile = psi_proof_admission::AdmissionProfile::default();
    let native = psi_terminal_verifier::verify_module_for_native_ranked_countdown(
        &custody.semantic_replay,
        &proof,
        &profile,
    )
    .ok()?;
    let fixed = psi_terminal_verifier::verify_module_for_fixed_fuel(
        &custody.semantic_replay,
        &proof,
        &profile,
    )
    .ok()?;
    let derived = psi_terminal_fixed_fuel::derive_ranked_countdown_entry_fuel(
        &fixed,
        custody.semantic_replay.entry,
    )
    .ok()?;
    if derived.terminal_psi() != custody.fixed_fuel.terminal_psi()
        || derived.schedule() != custody.fixed_fuel.schedule()
        || derived.entry() != custody.fixed_fuel.entry()
        || derived.relevant_preconditions() != custody.fixed_fuel.relevant_preconditions()
        || derived.ceiling_units() != custody.fixed_fuel.ceiling_units()
    {
        return None;
    }

    let projected = &custody.structural_frontiers;
    let verified = native.structural_frontiers().machine(projected.machine)?;
    let verified_header = verified.block_entry(projected.header)?;
    let verified_backedge = verified.edge_exit(projected.backedge)?;
    if !frontier_matches(&projected.header_entry, verified_header)
        || !frontier_matches(&projected.backedge_exit, verified_backedge)
    {
        return None;
    }
    Some(())
}

fn frontier_matches(
    projected: &omega_abstract_operations::RankedStructuralOwnershipFrontier,
    verified: &psi_terminal_verifier::VerifiedStructuralOwnershipFrontier,
) -> bool {
    projected.claims().len() == verified.claims().len()
        && projected
            .claims()
            .iter()
            .zip(verified.claims())
            .all(|(left, right)| {
                left.claim == right.claim
                    && left.input == right.input
                    && left.path == right.path
                    && left.multiplicity == right.multiplicity
            })
        && projected.owned_places().len() == verified.owned_places().len()
        && projected
            .owned_places()
            .iter()
            .zip(verified.owned_places())
            .all(|(left, right)| {
                left.place == right.place && left.multiplicity == right.multiplicity
            })
        && projected.partial_custody().len() == verified.partial_custody().len()
        && projected
            .partial_custody()
            .iter()
            .zip(verified.partial_custody())
            .all(|(left, right)| left.place == right.place && left.moved_paths == right.moved_paths)
}

pub(super) fn replay_ranked_countdown_object_contract(
    artifact: &ObjectArtifact,
    function: &ObjectFunction,
    record: &RankedU32CountdownMachineCodeRecord,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidRankedCountdown(function.machine);
    if artifact.functions().len() != 1
        || artifact.entry() != function.machine
        || record.custody.fixed_fuel.terminal_psi() != artifact.psi()
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
        || !ranked_object_body_is_exclusive(artifact, function)
    {
        return Err(invalid());
    }
    replay_verifier_custody(record).ok_or_else(invalid)?;

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

    replay_calling_and_structural_contract(artifact.target(), record).ok_or_else(invalid)?;
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
    let replay = &record.custody.semantic_replay;
    let [replay_machine] = replay.machines.as_slice() else {
        return None;
    };
    let [replay_structural] = replay_machine.structural_parameters.as_slice() else {
        return None;
    };
    let referent_shape = crate::structural_condition_layout::replay_structural_value_shape(
        replay_structural.structural_type,
        &replay.structural_types,
    )?;
    let affine_owned = !replay_structural.is_self
        && replay_structural.multiplicity == StructuralMultiplicity::Affine
        && replay_structural.access == StructuralAccess::Owned;
    let persistent_receiver =
        replay_structural.is_self && replay_structural.access == StructuralAccess::MutableBorrow;
    let expected_structural_shape = if persistent_receiver {
        ValueShape::integer(
            u16::try_from(target.pointer_size).ok()?,
            u16::try_from(target.pointer_alignment).ok()?,
        )
    } else {
        referent_shape
    };
    let expected_rank = ValueShape::integer(4, 4);
    if rank.shape != expected_rank
        || rank.locations.as_slice()
            != [ValueLocation::Register {
                register: expected_rank_home,
                value_byte_offset: 0,
                byte_size: 4,
            }]
        || structural_parameter.place != replay_structural.place
        || structural_parameter.structural_type != replay_structural.structural_type
        || structural_parameter.multiplicity != replay_structural.multiplicity
        || structural_parameter.access != replay_structural.access
        || (!affine_owned && !persistent_receiver)
        || structural_parameter.shape != expected_structural_shape
        || structural != &structural_parameter.placement
        || record
            .structural_types
            .iter()
            .filter(|declaration| declaration.id == structural_parameter.structural_type)
            .count()
            != 1
        || (affine_owned
            && record.cleanup_actions.as_slice()
                != [TerminalAffineCleanupAction::DiscardRoot(
                    structural_parameter.place,
                )])
        || (persistent_receiver && !record.cleanup_actions.is_empty())
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
    let [replay_structural] = record
        .custody
        .semantic_replay
        .machines
        .first()?
        .structural_parameters
        .as_slice()
    else {
        return None;
    };
    let header = record
        .custody
        .structural_frontiers
        .block_entry(component.header)?;
    let backedge = record
        .custody
        .structural_frontiers
        .edge_exit(covered.edge)?;
    let affine_owned = !replay_structural.is_self
        && replay_structural.multiplicity == StructuralMultiplicity::Affine
        && replay_structural.access == StructuralAccess::Owned;
    let persistent_receiver =
        replay_structural.is_self && replay_structural.access == StructuralAccess::MutableBorrow;
    let affine_frontier = matches!(header.owned_places(), [owned]
        if owned.place == structural.place
            && owned.multiplicity == StructuralMultiplicity::Affine);
    let receiver_frontier = header.owned_places().is_empty();
    (header == backedge
        && ((affine_owned && affine_frontier) || (persistent_receiver && receiver_frontier))
        && header.claims().is_empty()
        && header.partial_custody().is_empty()
        && structural.place == replay_structural.place
        && structural.structural_type == replay_structural.structural_type
        && structural.multiplicity == replay_structural.multiplicity
        && structural.access == replay_structural.access)
        .then_some(())
}

fn replay_ranked_graph_matches(
    machine: &TerminalMachine,
    record: &RankedU32CountdownMachineCodeRecord,
) -> bool {
    let graph = record.custody.graph;
    let Some(ranked) = machine.ranked_scc.as_ref() else {
        return false;
    };
    let [covered] = ranked.covered_cyclic_edges.as_slice() else {
        return false;
    };
    let block = |id| machine.blocks.iter().find(|block| block.id == id);
    let Some(entry) = block(machine.entry) else {
        return false;
    };
    let Terminator::Jump {
        edge: preheader_edge,
        target: preheader_target,
        arguments: preheader_arguments,
        ..
    } = &entry.terminator
    else {
        return false;
    };
    let Some(header) = block(ranked.header) else {
        return false;
    };
    let Some(rank_index) = header
        .parameters
        .iter()
        .position(|parameter| parameter.id == ranked.rank_parameter)
    else {
        return false;
    };
    if *preheader_target != ranked.header || preheader_arguments.len() != header.parameters.len() {
        return false;
    }
    let Some(&initial_value) = preheader_arguments.get(rank_index) else {
        return false;
    };
    let [zero, compare] = header.operations.as_slice() else {
        return false;
    };
    if !matches!(
        zero.kind,
        OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(0)
        }
    ) {
        return false;
    }
    let Some(zero_value) = scalar_result(zero) else {
        return false;
    };
    if !matches!(
        compare.kind,
        OperationKind::IntegerLessThan { left, right }
            if left == zero_value && right == ranked.rank_parameter
    ) {
        return false;
    }
    let Some(condition) = scalar_result(compare) else {
        return false;
    };
    let Terminator::Conditional {
        condition: terminator_condition,
        when_true,
        when_false,
    } = &header.terminator
    else {
        return false;
    };
    let TerminalRankedGuard::UnsignedParameterPositive {
        block: guard_block,
        edge: guard_edge,
        condition: guard_condition,
        parameter: guard_parameter,
    } = covered.guard;
    if *terminator_condition != condition
        || guard_block != ranked.header
        || guard_edge != when_true.edge
        || when_true.target != covered.source
        || guard_condition != condition
        || guard_parameter != ranked.rank_parameter
    {
        return false;
    }
    let Some(decrement) = block(covered.source) else {
        return false;
    };
    let [one, subtract] = decrement.operations.as_slice() else {
        return false;
    };
    if !matches!(
        one.kind,
        OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(1)
        }
    ) {
        return false;
    }
    let Some(one_value) = scalar_result(one) else {
        return false;
    };
    let OperationKind::ExactIntegerSubtract {
        left,
        right,
        obligation,
    } = subtract.kind
    else {
        return false;
    };
    if left != ranked.rank_parameter || right != one_value {
        return false;
    }
    let Some(subtract_value) = scalar_result(subtract) else {
        return false;
    };
    let TerminalRankedSuccessorArgument::UnsignedParameterMinusOne {
        argument_index,
        argument,
        source_parameter,
        target_parameter,
    } = covered.successor_argument;
    if argument != subtract_value
        || source_parameter != ranked.rank_parameter
        || target_parameter != ranked.rank_parameter
    {
        return false;
    }
    let Terminator::Jump {
        edge: backedge,
        target: backedge_target,
        arguments: backedge_arguments,
        ..
    } = &decrement.terminator
    else {
        return false;
    };
    let Ok(argument_index) = usize::try_from(argument_index) else {
        return false;
    };
    if *backedge != covered.edge
        || *backedge_target != covered.target
        || covered.target != ranked.header
        || backedge_arguments.get(argument_index) != Some(&subtract_value)
    {
        return false;
    }
    let Some(done) = block(when_false.target) else {
        return false;
    };
    let Terminator::ReturnUnit {
        edge: return_edge,
        trivial_affine_discards,
    } = &done.terminator
    else {
        return false;
    };
    let [structural] = machine.structural_parameters.as_slice() else {
        return false;
    };
    let exact_cleanup =
        if structural.is_self && structural.access == StructuralAccess::MutableBorrow {
            trivial_affine_discards.is_empty()
        } else {
            trivial_affine_discards.as_slice() == [structural.place]
        };
    done.operations.is_empty()
        && exact_cleanup
        && graph.entry == machine.entry
        && graph.preheader_edge == *preheader_edge
        && graph.initial_value == initial_value
        && graph.zero_operation == zero.id
        && graph.zero_value == zero_value
        && graph.compare_operation == compare.id
        && graph.false_exit_edge == when_false.edge
        && graph.done_block == done.id
        && graph.one_operation == one.id
        && graph.one_value == one_value
        && graph.subtract_operation == subtract.id
        && graph.subtract_obligation == obligation
        && graph.return_edge == *return_edge
}

fn scalar_result(operation: &Operation) -> Option<psi_core::ValueId> {
    let OperationResult::Scalar(result) = operation.result else {
        return None;
    };
    Some(result.id)
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
