//! Select explicit control edges without replacing parallel source bindings with copies.
use super::{
    Builder, IntegerSign, LegalizedScalarFunction, LegalizedScalarInstructionKind,
    RegisterConstraintKey, ScalarType, SelectedBlockId, SelectedInstruction,
    SelectedInstructionKind, SelectedInstructionProvenance, ValueLocation, VirtualRegisterId,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::row;
use legalized_operations::{
    LegalizedScalarBlock, LegalizedScalarComparison as Comparison, LegalizedScalarReturnValue,
    LegalizedScalarSuccessor, LegalizedScalarTerminator,
};
use selected_instructions::{SelectedSuccessor, SelectedTerminator};
use selected_instructions::{SelectedValueBinding, SelectedValueTransport};

pub(super) fn build(
    function: usize,
    source: &LegalizedScalarFunction,
    block: &LegalizedScalarBlock,
    order: &[usize],
    builder: &mut Builder<'_>,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<SelectedTerminator, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::unsupported_shape(function);
    let constraints = builder.constraints;
    let keys = &constraints.keys;
    match &block.terminator {
        LegalizedScalarTerminator::ExternalCompletion { .. } => Err(invalid()),
        LegalizedScalarTerminator::ReturnCleanup { returned, calls } => {
            if calls.is_empty()
                || calls
                    .iter()
                    .any(|(owner, _)| owner.edge() != Some(returned.edge))
            {
                return Err(invalid());
            }
            if matches!(
                returned.value,
                LegalizedScalarReturnValue::Structural { .. }
                    | LegalizedScalarReturnValue::StructuralParameter { .. }
                    | LegalizedScalarReturnValue::SharedDescriptor { .. }
            ) {
                return super::aggregate_return::build(
                    source,
                    block,
                    returned,
                    builder,
                    calls,
                    environment,
                );
            }
            for (owner, call) in calls {
                super::cleanup_call::emit(*owner, call, returned.effect, builder, environment)?;
            }
            let mut normal = block.clone();
            normal.terminator = LegalizedScalarTerminator::Return(returned.clone());
            build(function, source, &normal, order, builder, environment)
        }
        LegalizedScalarTerminator::Crash {
            psi_edge,
            cause,
            site_guard,
            frontier_lower_bound,
            fuel,
            ..
        } => Ok(SelectedTerminator::Crash {
            instruction: terminal(
                builder,
                SelectedInstructionKind::Crash,
                keys.crash,
                &[],
                SelectedInstructionProvenance {
                    edges: vec![*psi_edge],
                    fuel: fuel.clone(),
                    ..Default::default()
                },
            )?,
            psi_edge: *psi_edge,
            cause: *cause,
            site_guard: site_guard.clone(),
            frontier_lower_bound: frontier_lower_bound.clone(),
        }),
        LegalizedScalarTerminator::StructuralCase { .. } => {
            super::structural_case::build(source, block, order, builder)
        }
        LegalizedScalarTerminator::Return(returned) => {
            let (kind, key, operands, values) = match returned.value {
                LegalizedScalarReturnValue::Structural { .. }
                | LegalizedScalarReturnValue::StructuralParameter { .. }
                | LegalizedScalarReturnValue::SharedDescriptor { .. } => {
                    return super::aggregate_return::build(
                        source,
                        block,
                        returned,
                        builder,
                        &[],
                        environment,
                    );
                }
                LegalizedScalarReturnValue::Unit => (
                    SelectedInstructionKind::ReturnUnit,
                    keys.return_unit,
                    Vec::new(),
                    Vec::new(),
                ),
                LegalizedScalarReturnValue::Value { value, scalar_type } => {
                    let (_, input, site, value_type) =
                        builder.resolve(value).ok_or_else(invalid)?;
                    if value_type != scalar_type {
                        return Err(invalid());
                    }
                    let result = source.call_plan.result.as_ref().ok_or_else(invalid)?;
                    let [
                        ValueLocation::Register {
                            register,
                            value_byte_offset: 0,
                            byte_size,
                        },
                    ] = result.locations.as_slice()
                    else {
                        return Err(invalid());
                    };
                    if crate::selection::scalar_call_abi::scalar_shape(value_type)
                        != Some(result.shape)
                        || *byte_size != result.shape.byte_size
                    {
                        return Err(invalid());
                    }
                    let keys_for_result = if matches!(scalar_type, ScalarType::IeeeFloat(_)) {
                        keys.return_float.as_slice()
                    } else {
                        std::slice::from_ref(&keys.return_i64)
                    };
                    let key = keys_for_result.iter().copied().find(|key| row(builder.catalog, *key).is_ok_and(|row| matches!(row.operands.as_slice(), [operand] if operand.fixed_view.is_some() && operand.fixed_view == environment.fixed_register_view(*register)))).ok_or_else(invalid)?;
                    let output = if let Some((kind, transfer_key)) =
                        crate::selection::scalar_call_abi::outgoing_float_transfer(value_type, keys)
                    {
                        let output = builder.register(value, site, value_type)?;
                        builder.registers[output.0 as usize].class =
                            row(builder.catalog, transfer_key)?
                                .operands
                                .get(1)
                                .ok_or_else(invalid)?
                                .class;
                        builder.emit(
                            kind,
                            transfer_key,
                            &[input, output],
                            SelectedInstructionProvenance {
                                values: vec![value],
                                ..Default::default()
                            },
                        )?;
                        output
                    } else {
                        builder.copy(input, value, site, value_type)?
                    };
                    (
                        SelectedInstructionKind::ReturnScalar,
                        key,
                        vec![output],
                        vec![value],
                    )
                }
            };
            let instruction = terminal(
                builder,
                kind,
                key,
                &operands,
                SelectedInstructionProvenance {
                    values,
                    edges: vec![returned.edge],
                    fuel: returned.fuel.clone(),
                    ..Default::default()
                },
            )?;
            Ok(SelectedTerminator::Return {
                instruction,
                psi_return_edge: returned.edge,
            })
        }
        LegalizedScalarTerminator::Jump {
            successor: next, ..
        } => {
            let successor = successor(source, order, builder, next)?;
            let instruction = terminal(
                builder,
                SelectedInstructionKind::Jump,
                keys.jump,
                &[],
                Default::default(),
            )?;
            Ok(SelectedTerminator::Jump {
                instruction,
                successor,
            })
        }
        LegalizedScalarTerminator::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            let mut base = *condition;
            let mut suffix_start = block.instructions.len();
            let mut inverted = false;
            let mut not_rows = Vec::new();
            while builder.resolve(base).is_none()
                && let Some(previous) = suffix_start
                    .checked_sub(1)
                    .and_then(|index| block.instructions.get(index))
            {
                let LegalizedScalarInstructionKind::BooleanNot { operand } = previous.kind else {
                    break;
                };
                if previous.result.as_ref().is_none_or(|result| {
                    result.value != base || result.scalar_type != ScalarType::Boolean
                }) {
                    return Err(invalid());
                }
                base = operand;
                inverted = !inverted;
                not_rows.push(previous);
                suffix_start -= 1;
            }
            not_rows.reverse();
            let comparison = suffix_start
                .checked_sub(1)
                .and_then(|index| block.instructions.get(index))
                .filter(|row| {
                    if builder.resolve(base).is_some() {
                        return false;
                    }
                    row.result
                        .as_ref()
                        .is_some_and(|result| result.value == base)
                });
            let (predicate, operand_type) = if let Some(row) = comparison
                && let LegalizedScalarInstructionKind::Compare {
                    predicate,
                    operand_type,
                    ..
                } = row.kind
            {
                (predicate, operand_type)
            } else {
                let (_, input, _, scalar_type) = builder.resolve(base).ok_or_else(invalid)?;
                if scalar_type != ScalarType::Boolean {
                    return Err(invalid());
                }
                builder.emit(
                    SelectedInstructionKind::CompareI64Zero,
                    keys.compare_i64_zero,
                    &[input],
                    SelectedInstructionProvenance {
                        values: vec![base],
                        ..Default::default()
                    },
                )?;
                // A Boolean register is true when nonzero, unlike Equal's zero predicate.
                inverted = !inverted;
                (Comparison::Equal, ScalarType::Boolean)
            };
            let branch_provenance = SelectedInstructionProvenance {
                operations: not_rows.iter().map(|row| row.operation).collect(),
                values: if not_rows.is_empty() {
                    vec![*condition]
                } else {
                    std::iter::once(base)
                        .chain(
                            not_rows
                                .iter()
                                .filter_map(|row| row.result.as_ref().map(|result| result.value)),
                        )
                        .collect()
                },
                fuel: not_rows
                    .iter()
                    .flat_map(|row| row.fuel.iter().copied())
                    .collect(),
                ..Default::default()
            };
            let when_true = successor(source, order, builder, when_true)?;
            let when_false = successor(source, order, builder, when_false)?;
            let (when_true, when_false) = if inverted {
                (when_false, when_true)
            } else {
                (when_true, when_false)
            };
            let signed = matches!(operand_type, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Signed);
            let kind = match predicate {
                Comparison::Equal => SelectedInstructionKind::ConditionalBranchNonZero,
                _ if signed => SelectedInstructionKind::ConditionalBranchI64LessThan,
                _ => SelectedInstructionKind::ConditionalBranchU64LessThan,
            };
            let instruction = terminal(
                builder,
                kind,
                keys.conditional_branch,
                &[],
                branch_provenance,
            )?;
            Ok(match predicate {
                Comparison::Equal => SelectedTerminator::ConditionalBranch {
                    instruction,
                    when_nonzero: when_false,
                    when_zero: when_true,
                },
                _ => {
                    let (when_less, when_not_less) = if predicate == Comparison::LessOrEqual {
                        (when_false, when_true)
                    } else {
                        (when_true, when_false)
                    };
                    if signed {
                        SelectedTerminator::ConditionalBranchI64LessThan {
                            instruction,
                            when_less,
                            when_not_less,
                        }
                    } else {
                        SelectedTerminator::ConditionalBranchU64LessThan {
                            instruction,
                            when_less,
                            when_not_less,
                        }
                    }
                }
            })
        }
    }
}

fn terminal(
    builder: &mut Builder<'_>,
    kind: SelectedInstructionKind,
    key: RegisterConstraintKey,
    operands: &[VirtualRegisterId],
    provenance: SelectedInstructionProvenance,
) -> Result<SelectedInstruction, SelectedInstructionError> {
    builder.emit(kind, key, operands, provenance)?;
    builder
        .instructions
        .last()
        .cloned()
        .ok_or(SelectedInstructionError::custody())
}

fn successor(
    source: &LegalizedScalarFunction,
    order: &[usize],
    builder: &mut Builder<'_>,
    next: &LegalizedScalarSuccessor,
) -> Result<SelectedSuccessor, SelectedInstructionError> {
    let preparation_start = builder.instructions.len();
    builder.block_body_end.get_or_insert(preparation_start);
    // Empty producers settle on the predecessor's terminator. Detached edge
    // preparation must not move their unconditional provenance onto one arm.
    let pending_provenance = std::mem::take(&mut builder.pending_provenance);
    let position = order
        .iter()
        .position(|index| source.blocks[*index].id == next.target)
        .ok_or(SelectedInstructionError::custody())?;
    let successor = SelectedSuccessor {
        structural_case: None,
        role: selected_instructions::SelectedSuccessorRole::Semantic,
        psi_edge: next.edge,
        block: SelectedBlockId(
            u32::try_from(position).map_err(|_| SelectedInstructionError::custody())?,
        ),
        source_target: next.target,
        structural_bindings: next
            .structural_bindings
            .iter()
            .map(|binding| {
                let semantic = &binding.semantic;
                // An address join receives its referent's address; the edge
                // bridge forms it from the lending root's storage.
                let projection = if let Some((_, parameter)) =
                    crate::selection::address_join_input::join(source, semantic.parameter)
                {
                    crate::selection::address_join_input::projection(
                        source,
                        next.target,
                        parameter,
                        &semantic.argument,
                        &binding.indices,
                        &builder.transport.pointers,
                        &builder.transport.local_slots,
                        &builder.transport.element_views,
                        &builder.transport.views,
                    )
                } else {
                    let parameter = source
                        .block(next.target)
                        .and_then(|block| block.structural_parameter(semantic.parameter))
                        .ok_or(SelectedInstructionError::custody())?;
                    crate::selection::aggregate_result_input::block_parameter_transport(
                        source,
                        next.target,
                        parameter,
                        &semantic.argument,
                        &binding.indices,
                        &builder.transport.pointers,
                        &builder.transport.local_slots,
                        &builder.transport.element_views,
                        &builder.transport.views,
                    )
                }
                .ok_or_else(|| {
                    SelectedInstructionError::custody_because(
                        "structural edge argument has no transport from its retained source storage",
                    )
                })?;
                let (transport, source_indices) =
                    project_block_storage(builder, next.edge, semantic.argument.place, projection)?;
                Ok(selected_instructions::SelectedStructuralBinding {
                    semantic: semantic.clone(),
                    transport,
                    source_indices,
                })
            })
            .collect::<Result<Vec<_>, SelectedInstructionError>>()?,
        bindings: next
            .bindings
            .iter()
            .map(|semantic| {
                let transport = if builder.required_values.contains(&semantic.parameter) {
                    let (_, argument, _, argument_type) = builder
                        .resolve(semantic.argument)
                        .ok_or(SelectedInstructionError::custody())?;
                    let (_, parameter, _, parameter_type) = builder
                        .resolve(semantic.parameter)
                        .ok_or(SelectedInstructionError::custody())?;
                    if argument_type != semantic.scalar_type
                        || parameter_type != semantic.scalar_type
                    {
                        return Err(SelectedInstructionError::custody());
                    }
                    SelectedValueTransport::Registers {
                        argument,
                        parameter,
                    }
                } else {
                    SelectedValueTransport::Unused
                };
                Ok(SelectedValueBinding {
                    semantic: *semantic,
                    transport,
                })
            })
            .collect::<Result<Vec<_>, SelectedInstructionError>>()?,
        fuel: next.fuel.clone(),
    };
    let preparation = builder.instructions[preparation_start..].to_vec();
    if !preparation.is_empty() {
        builder
            .edge_work_at(next.edge)?
            .preparation
            .extend(preparation);
    }
    builder.pending_provenance = pending_provenance;
    Ok(successor)
}

/// Descriptor loads only form an already-admitted loan's address. They do not
/// observe scalar backing or choose an origin from custody alternatives.
/// All bindings snapshot these addresses before edge homes are overwritten.
fn project_block_storage(
    builder: &mut Builder<'_>,
    edge: semantic_vocabulary::EdgeId,
    place: semantic_vocabulary::PlaceId,
    projection: crate::selection::aggregate_result_input::BlockParameterProjection,
) -> Result<
    (
        selected_instructions::SelectedStructuralTransport,
        Vec<legalized_operations::LegalizedRuntimeIndexOperand>,
    ),
    SelectedInstructionError,
> {
    use selected_instructions::{SelectedAddressBase, SelectedStructuralTransport};
    let mut transport = projection.transport;
    if transport == SelectedStructuralTransport::Unused
        || (projection.pointer_slots.is_empty()
            && projection.indices.is_empty()
            && projection.base_byte_offset.is_none())
    {
        return Ok((transport, Vec::new()));
    }
    let mut base = match transport {
        SelectedStructuralTransport::Address { base, .. }
        | SelectedStructuralTransport::ScalarReference { base, .. } => base,
        SelectedStructuralTransport::WholeValue { argument, .. } => {
            SelectedAddressBase::Register(argument)
        }
        _ => return Err(SelectedInstructionError::custody()),
    };
    let provenance = SelectedInstructionProvenance {
        edges: vec![edge],
        ..Default::default()
    };
    let mut pointer = match base {
        SelectedAddressBase::Register(pointer) => pointer,
        SelectedAddressBase::Local(slot) => {
            let pointer = super::structural::transport_register(builder, place, 0)?;
            builder.emit_local_frame_address(slot, pointer, provenance.clone())?;
            pointer
        }
    };
    if let Some(offset) = projection.base_byte_offset {
        let output = super::structural::transport_register(builder, place, 0)?;
        builder.emit_byte_view_address(pointer, offset, output, provenance.clone())?;
        pointer = output;
    }
    let (pointer, tail_start) = super::structural::runtime_address::cross_references_at_site(
        builder,
        provenance.clone(),
        selected_instructions::SelectedMemoryAccessOrigin::Edge(edge),
        place,
        pointer,
        &projection.indices,
        &projection.pointer_slots,
    )?;
    let source_indices = projection
        .indices
        .get(tail_start..)
        .ok_or(SelectedInstructionError::custody())?;
    let pointer = super::structural::runtime_address::scale_at_site(
        builder,
        provenance,
        place,
        0,
        pointer,
        source_indices,
    )?;
    base = SelectedAddressBase::Register(pointer);
    match &mut transport {
        SelectedStructuralTransport::Address {
            base: destination, ..
        }
        | SelectedStructuralTransport::ScalarReference {
            base: destination, ..
        } => *destination = base,
        SelectedStructuralTransport::WholeValue { argument, .. } => *argument = pointer,
        _ => return Err(SelectedInstructionError::custody()),
    }
    Ok((transport, source_indices.to_vec()))
}

pub(super) fn block_order(
    source: &LegalizedScalarFunction,
) -> Result<Vec<usize>, SelectedInstructionError> {
    crate::selection::block_order::derive(source)
}

pub(super) fn branch_suffix(
    source: &LegalizedScalarFunction,
    block: &LegalizedScalarBlock,
    index: usize,
) -> bool {
    if matches!(block.instructions.get(index).map(|row| &row.kind),
        Some(LegalizedScalarInstructionKind::Compare { operand_type, right_operand_type, .. })
            if operand_type != right_operand_type)
    {
        return false;
    }
    let Some(result) = block.instructions.get(index).and_then(|row| row.result) else {
        return false;
    };
    let mut value = result.value;
    for row in &block.instructions[index + 1..] {
        if !matches!(row.kind, LegalizedScalarInstructionKind::BooleanNot {operand} if operand == value)
            || row
                .result
                .is_none_or(|result| result.scalar_type != ScalarType::Boolean)
        {
            return false;
        }
        let Some(result) = row.result else {
            return false;
        };
        value = result.value;
    }

    for (position, row) in block.instructions.iter().enumerate().skip(index) {
        let Some(result) = row.result else {
            return false;
        };
        for owner in &source.blocks {
            for (consumer_index, consumer) in owner.instructions.iter().enumerate() {
                if owner.id == block.id && consumer_index == position + 1 {
                    continue;
                }
                if consumer.references_value(result.value) {
                    return false;
                }
            }
            if owner.id == block.id && position + 1 == block.instructions.len() {
                let legalized_operations::LegalizedScalarTerminator::Conditional {
                    condition,
                    when_true,
                    when_false,
                    ..
                } = &owner.terminator
                else {
                    return false;
                };
                if *condition != result.value
                    || [when_true, when_false]
                        .into_iter()
                        .flat_map(|edge| &edge.bindings)
                        .any(|binding| binding.argument == result.value)
                {
                    return false;
                }
            } else if owner.terminator.references_value(result.value) {
                return false;
            }
        }
    }
    matches!(block.terminator, LegalizedScalarTerminator::Conditional {condition,..}
        if condition == value)
}
