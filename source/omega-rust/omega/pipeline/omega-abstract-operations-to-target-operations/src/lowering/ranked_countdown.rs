//! Closed legalization for the first admitted native-ranked control slice.

use super::scalar::scalar_shape;
use super::shared::*;
use super::structural_layout::structural_shape;

pub(super) fn lower(
    ranked: &RankedNativeAbstractOperationPlan,
    target: NativeTarget,
) -> Result<TargetOperationPlan, LoweringError> {
    let plan = &ranked.plan;
    let custody = &ranked.countdown;
    let invalid = || LoweringError::InvalidRankedCountdown(plan.entry);

    if target != NativeTarget::linux_x64() && target != NativeTarget::linux_arm64() {
        return Err(invalid());
    }

    let [function] = plan.functions.as_slice() else {
        return Err(invalid());
    };
    if function.machine != plan.entry
        || custody.fixed_fuel.terminal_psi() != plan.psi
        || custody.fixed_fuel.entry() != plan.entry
        || custody.structural_frontiers.machine != function.machine
        || !plan.boundary_machines.is_empty()
        || !plan.provider_candidates.is_empty()
        || function.result != AbstractFunctionResult::Unit
        || function.parameters.len() != 1
        || function.structural_parameters.len() != 1
        || function.entry != custody.graph.entry
        || function.block_entries.len() != 4
    {
        return Err(invalid());
    }

    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32 is valid");
    let initial = function.parameters[0];
    let structural_parameter = &function.structural_parameters[0];
    let [replay_machine] = custody.semantic_replay.machines.as_slice() else {
        return Err(invalid());
    };
    let [replay_structural_parameter] = replay_machine.structural_parameters.as_slice() else {
        return Err(invalid());
    };
    let affine_owned = !structural_parameter.is_self
        && structural_parameter.multiplicity == StructuralMultiplicity::Affine
        && structural_parameter.access == StructuralAccess::Owned;
    let persistent_receiver = structural_parameter.is_self
        && structural_parameter.access == StructuralAccess::MutableBorrow;
    let component = &custody.ranked_scc;
    let [covered] = component.covered_cyclic_edges.as_slice() else {
        return Err(invalid());
    };
    let TerminalRankedGuard::UnsignedParameterPositive {
        block: guard_block,
        edge: guard_edge,
        condition,
        parameter: guard_parameter,
    } = covered.guard;
    let TerminalRankedSuccessorArgument::UnsignedParameterMinusOne {
        argument_index,
        argument: next_rank,
        source_parameter,
        target_parameter,
    } = covered.successor_argument;
    if initial.value != custody.graph.initial_value
        || initial.scalar_type != ScalarType::Integer(u32_type)
        || component.rank_type != u32_type
        || component.lower_bound != IntegerValue::Unsigned(0)
        || component.upper_bound != IntegerValue::Unsigned(u128::from(u32::MAX))
        || structural_parameter.position != 0
        || structural_parameter != replay_structural_parameter
        || (!affine_owned && !persistent_receiver)
        || !structural_parameter.qualifications.is_empty()
        || covered.target != component.header
        || guard_block != component.header
        || guard_parameter != component.rank_parameter
        || source_parameter != component.rank_parameter
        || target_parameter != component.rank_parameter
        || argument_index != 0
    {
        return Err(invalid());
    }

    let preheader_entry = block_entry(function, custody.graph.entry).ok_or_else(invalid)?;
    let header_entry = block_entry(function, component.header).ok_or_else(invalid)?;
    let decrement_entry = block_entry(function, covered.source).ok_or_else(invalid)?;
    let done_entry = block_entry(function, custody.graph.done_block).ok_or_else(invalid)?;
    let [header_parameter] = header_entry.parameters.as_slice() else {
        return Err(invalid());
    };
    if !preheader_entry.parameters.is_empty()
        || header_parameter.value != component.rank_parameter
        || header_parameter.scalar_type != ScalarType::Integer(u32_type)
        || !decrement_entry.parameters.is_empty()
        || !done_entry.parameters.is_empty()
    {
        return Err(invalid());
    }

    let preheader = block_operations(function, custody.graph.entry).ok_or_else(invalid)?;
    let header = block_operations(function, component.header).ok_or_else(invalid)?;
    let decrement = block_operations(function, covered.source).ok_or_else(invalid)?;
    let done = block_operations(function, custody.graph.done_block).ok_or_else(invalid)?;

    let [
        AbstractOperation::Jump {
            psi_edge: preheader_edge,
            target: preheader_target,
            bindings: preheader_bindings,
            trivial_affine_discards: preheader_discards,
        },
    ] = preheader
    else {
        return Err(invalid());
    };
    let [preheader_binding] = preheader_bindings.as_slice() else {
        return Err(invalid());
    };
    if *preheader_edge != custody.graph.preheader_edge
        || *preheader_target != component.header
        || preheader_binding.parameter != component.rank_parameter
        || preheader_binding.argument != initial.value
        || preheader_binding.scalar_type != ScalarType::Integer(u32_type)
        || !preheader_discards.is_empty()
    {
        return Err(invalid());
    }

    let [
        AbstractOperation::IntegerConstant {
            psi_operation: zero_operation,
            result: zero_value,
            scalar_type: zero_type,
            value: zero,
        },
        AbstractOperation::IntegerLessThan {
            psi_operation: compare_operation,
            result: compare_value,
            left: compare_left,
            right: compare_right,
        },
        AbstractOperation::Conditional {
            condition: branch_condition,
            when_true,
            when_false,
        },
    ] = header
    else {
        return Err(invalid());
    };
    if *zero_operation != custody.graph.zero_operation
        || *zero_value != custody.graph.zero_value
        || *zero_type != ScalarType::Integer(u32_type)
        || *zero != IntegerValue::Unsigned(0)
        || *compare_operation != custody.graph.compare_operation
        || *compare_value != condition
        || *compare_left != *zero_value
        || *compare_right != component.rank_parameter
        || *branch_condition != condition
        || when_true.psi_edge != guard_edge
        || when_true.target != covered.source
        || !when_true.bindings.is_empty()
        || !when_true.trivial_affine_discards.is_empty()
        || when_false.psi_edge != custody.graph.false_exit_edge
        || when_false.target != custody.graph.done_block
        || !when_false.bindings.is_empty()
        || !when_false.trivial_affine_discards.is_empty()
    {
        return Err(invalid());
    }

    let [
        AbstractOperation::IntegerConstant {
            psi_operation: one_operation,
            result: one_value,
            scalar_type: one_type,
            value: one,
        },
        AbstractOperation::ExactIntegerSubtract {
            psi_operation: subtract_operation,
            obligation: subtract_obligation,
            result: subtract_value,
            scalar_type: subtract_type,
            left: subtract_left,
            right: subtract_right,
        },
        AbstractOperation::Jump {
            psi_edge: backedge,
            target: backedge_target,
            bindings: backedge_bindings,
            trivial_affine_discards: backedge_discards,
        },
    ] = decrement
    else {
        return Err(invalid());
    };
    let [backedge_binding] = backedge_bindings.as_slice() else {
        return Err(invalid());
    };
    if *one_operation != custody.graph.one_operation
        || *one_value != custody.graph.one_value
        || *one_type != ScalarType::Integer(u32_type)
        || *one != IntegerValue::Unsigned(1)
        || *subtract_operation != custody.graph.subtract_operation
        || *subtract_obligation != custody.graph.subtract_obligation
        || *subtract_value != next_rank
        || *subtract_type != u32_type
        || *subtract_left != component.rank_parameter
        || *subtract_right != *one_value
        || *backedge != covered.edge
        || *backedge_target != component.header
        || backedge_binding.parameter != component.rank_parameter
        || backedge_binding.argument != next_rank
        || backedge_binding.scalar_type != ScalarType::Integer(u32_type)
        || !backedge_discards.is_empty()
    {
        return Err(invalid());
    }

    let [
        AbstractOperation::ReturnUnit {
            psi_edge: return_edge,
            cleanup_actions,
        },
    ] = done
    else {
        return Err(invalid());
    };
    if *return_edge != custody.graph.return_edge {
        return Err(invalid());
    }
    if (affine_owned
        && cleanup_actions.as_slice()
            != [TerminalAffineCleanupAction::DiscardRoot(
                structural_parameter.place,
            )])
        || (persistent_receiver && !cleanup_actions.is_empty())
    {
        return Err(invalid());
    }

    let header_frontier = custody
        .structural_frontiers
        .block_entry(component.header)
        .ok_or_else(invalid)?;
    let backedge_frontier = custody
        .structural_frontiers
        .edge_exit(covered.edge)
        .ok_or_else(invalid)?;
    if header_frontier != backedge_frontier {
        return Err(invalid());
    }
    let affine_frontier = matches!(header_frontier.owned_places(), [owned]
        if owned.place == structural_parameter.place
            && owned.multiplicity == StructuralMultiplicity::Affine);
    let receiver_frontier = header_frontier.owned_places().is_empty();
    if ((affine_owned && !affine_frontier) || (persistent_receiver && !receiver_frontier))
        || !header_frontier.claims().is_empty()
        || !header_frontier.partial_custody().is_empty()
    {
        return Err(invalid());
    }

    let structural_types = plan
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let pointer_shape = ValueShape::integer(
        u16::try_from(target.pointer_size).map_err(|_| invalid())?,
        u16::try_from(target.pointer_alignment).map_err(|_| invalid())?,
    );
    let structural_shapes = function
        .structural_parameters
        .iter()
        .map(|parameter| {
            let referent = structural_shape(
                parameter.structural_type,
                &structural_types,
                &mut shape_cache,
                &mut active,
            )?;
            Ok::<ValueShape, LoweringError>(
                if parameter.access == StructuralAccess::MutableBorrow {
                    pointer_shape
                } else {
                    referent
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let signature = CallSignature {
        parameters: std::iter::once(scalar_shape(initial.value, initial.scalar_type, true)?)
            .chain(structural_shapes.iter().copied())
            .collect(),
        result: None,
    };
    let call_plan = evaluate_call_plan(CallingPolicy::native_for_target(target), &signature)
        .map_err(LoweringError::AbiPlan)?;
    if call_plan.parameters.len() != 1 + function.structural_parameters.len() {
        return Err(LoweringError::AbiParameterCountMismatch {
            expected: 1 + function.structural_parameters.len(),
            actual: call_plan.parameters.len(),
        });
    }
    let structural_parameters = function
        .structural_parameters
        .iter()
        .zip(structural_shapes)
        .zip(&call_plan.parameters[1..])
        .map(
            |((parameter, shape), placement)| TargetStructuralParameter {
                place: parameter.place,
                structural_type: parameter.structural_type,
                multiplicity: parameter.multiplicity,
                access: parameter.access,
                shape,
                placement: placement.clone(),
            },
        )
        .collect::<Vec<_>>();

    Ok(TargetOperationPlan {
        psi: plan.psi,
        target,
        entry: plan.entry,
        functions: vec![TargetFunction {
            machine: function.machine,
            attachment: function.attachment,
            fixed_integer_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: vec![
                    *zero_operation,
                    *compare_operation,
                    *one_operation,
                    *subtract_operation,
                ],
                edges: vec![
                    *preheader_edge,
                    guard_edge,
                    when_false.psi_edge,
                    *backedge,
                    *return_edge,
                ],
            },
            operation: TargetOperation::RankedU32Countdown(TargetRankedU32Countdown {
                custody: custody.clone(),
                call_plan,
                structural_types: plan.structural_types.clone(),
                structural_parameters,
                cleanup_actions: cleanup_actions.clone(),
            }),
        }],
    })
}

fn block_operations(function: &AbstractFunction, block: BlockId) -> Option<&[AbstractOperation]> {
    let (index, entry) = function
        .block_entries
        .iter()
        .enumerate()
        .find(|(_, entry)| entry.block == block)?;
    let end = function
        .block_entries
        .get(index + 1)
        .map_or(function.operations.len(), |next| next.operation_offset);
    function.operations.get(entry.operation_offset..end)
}

fn block_entry(
    function: &AbstractFunction,
    block: BlockId,
) -> Option<&omega_abstract_operations::AbstractBlockEntry> {
    function
        .block_entries
        .iter()
        .find(|entry| entry.block == block)
}
