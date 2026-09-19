//! Forwarded helper chains retain descriptor custody separately from scalar
//! execution. A helper's call result is one binding in its authored sequence,
//! not necessarily its returned value. Ordinary scalar evaluation and exit
//! replay therefore own the surrounding calculations and returning branches.

use crate::unit::dynamic_composed_unit::applications::{
    exact_machine_service_summary, validate_empty_contract, validate_empty_service_summary,
};
use crate::unit::dynamic_composed_unit::dynamic_lanes::{
    ForwardedHelperIds, LoweredDynamicRealization,
};
use crate::unit::dynamic_composed_unit::source_lowering::dynamic_parameter_interface;
use crate::unit::dynamic_composed_unit::store_operations::empty_terminal_contract;
use crate::unit::{
    CheckedTrees, LoweredSourceCallOccurrence, LoweringError, allocate_dense, block_id, edge_id,
    lower_installation_machine_service_ceiling, machine_id, operation_id, terminal_scalar_type,
    unsupported, value_id,
};
use checked_trees::CheckedDynamicScalarCallPlan;
use terminal_psi::{
    Block, ClosedConformanceApplication, ClosedConformanceRow, Operation, OperationKind,
    OperationResult, TerminalDynamicDescriptorArgument, TerminalDynamicDescriptorSource,
    TerminalDynamicDispatchCatalog, TerminalMachine, TerminalMachineResult, Terminator,
    ValueDeclaration,
};

pub(crate) fn forwarded_helper_chain_ids(
    plan: &CheckedDynamicScalarCallPlan,
    realizations: &[LoweredDynamicRealization],
    next_block: &mut u64,
    next_operation: &mut u64,
    next_value: &mut u64,
    next_edge: &mut u64,
) -> Result<Vec<ForwardedHelperIds>, LoweringError> {
    if !matches!(
        plan.origin,
        checked_trees::CheckedDynamicScalarCallOrigin::Forwarded { .. }
    ) {
        if !plan.forwarding_transfers.is_empty() || !plan.forwarding_helpers.is_empty() {
            return unsupported("local dynamic call retained forwarding transfers");
        }
        return Ok(Vec::new());
    }
    let first_machine = realizations
        .iter()
        .map(|realization| realization.machine.get())
        .max()
        .ok_or(LoweringError::Unsupported(
            "forwarded dynamic dispatch has no realization machine",
        ))?
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "forwarded dynamic helper machine identity overflowed",
        ))?;
    (0..=plan.forwarding_transfers.len())
        .map(|ordinal| {
            let ordinal = u64::try_from(ordinal).map_err(|_| {
                LoweringError::Unsupported("forwarded dynamic helper count exceeds u64")
            })?;
            Ok(ForwardedHelperIds {
                machine: machine_id(first_machine.checked_add(ordinal).ok_or(
                    LoweringError::Unsupported(
                        "forwarded dynamic helper machine identity overflowed",
                    ),
                )?),
                block: block_id(allocate_dense(next_block)?),
                operation: operation_id(allocate_dense(next_operation)?),
                operation_value: value_id(allocate_dense(next_value)?),
                result_value: value_id(allocate_dense(next_value)?),
                edge: edge_id(allocate_dense(next_edge)?),
            })
        })
        .collect()
}

pub(crate) fn extend_parameter_forwarding_catalog(
    catalog: &mut TerminalDynamicDispatchCatalog,
    helpers: &[ForwardedHelperIds],
) -> Result<(), LoweringError> {
    let [template] = catalog.parameters.as_slice() else {
        return unsupported("multi-hop dynamic forwarding lost its first parameter interface");
    };
    let template = template.clone();
    let [dispatch] = catalog.parameter_dispatches.as_mut_slice() else {
        return unsupported("multi-hop dynamic forwarding lost its final parameter dispatch");
    };
    for helper in &helpers[1..] {
        let mut parameter = template.clone();
        parameter.owner = helper.machine;
        catalog.parameters.push(parameter);
    }
    for pair in helpers.windows(2) {
        catalog.arguments.push(TerminalDynamicDescriptorArgument {
            owner: pair[0].machine,
            operation: pair[0].operation,
            parameter_ordinal: 0,
            source: TerminalDynamicDescriptorSource::Parameter { ordinal: 0 },
        });
    }
    let final_helper = helpers.last().ok_or(LoweringError::Unsupported(
        "multi-hop dynamic forwarding has no final helper",
    ))?;
    dispatch.owner = final_helper.machine;
    dispatch.operation = final_helper.operation;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn materialize_forwarded_helper_for_source(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
    application: &ClosedConformanceApplication,
    selected_row: &ClosedConformanceRow,
    ids: ForwardedHelperIds,
    source_machine: symbols::SymbolHandle,
    next_helper: Option<semantic_vocabulary::MachineId>,
    body: &checked_trees::CheckedDynamicScalarHelperPlan,
    next_block: &mut u64,
    next_operation: &mut u64,
    next_value: &mut u64,
    next_edge: &mut u64,
    source_calls: &mut [LoweredSourceCallOccurrence],
) -> Result<TerminalMachine, LoweringError> {
    let checked_contract = checked
        .facts
        .contract_plans
        .for_machine(source_machine)
        .ok_or(LoweringError::Unsupported(
            "forwarded dynamic helper has no checked contract",
        ))?;
    validate_empty_contract(
        checked,
        source_machine,
        checked_contract.report_fingerprint,
        checked_contract.commitment,
    )?;
    let service_summary = exact_machine_service_summary(checked, source_machine)?;
    validate_empty_service_summary(checked, service_summary)?;
    let service_contract = checked
        .facts
        .service_reaches
        .plan_for_machine(source_machine)
        .ok_or(LoweringError::Unsupported(
            "forwarded dynamic helper has no checked service contract",
        ))?;
    let published_service_ceiling = lower_installation_machine_service_ceiling(
        checked,
        source_machine,
        service_contract,
        service_summary,
        &[],
    )?;
    let (_, requirement_slot) = dynamic_parameter_interface(application, selected_row)?;
    let scalar_type = terminal_scalar_type(plan.result.primitive_type)?;
    let blocks = materialize_helper_body(
        checked,
        body,
        ids,
        source_machine,
        next_helper,
        requirement_slot,
        scalar_type,
        next_block,
        next_operation,
        next_value,
        next_edge,
        source_calls,
    )?;
    Ok(TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: ids.machine,
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: ids.result_value,
            scalar_type,
        }),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling,
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: ids.block,
        blocks,
        contract: empty_terminal_contract(ids.machine.get()),
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn materialize_forwarded_helper_chain(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
    application: &ClosedConformanceApplication,
    selected_row: &ClosedConformanceRow,
    helpers: &[ForwardedHelperIds],
    next_block: &mut u64,
    next_operation: &mut u64,
    next_value: &mut u64,
    next_edge: &mut u64,
    source_calls: &mut [LoweredSourceCallOccurrence],
) -> Result<Vec<TerminalMachine>, LoweringError> {
    if helpers.is_empty() {
        return Ok(Vec::new());
    }
    let checked_trees::CheckedDynamicScalarCallOrigin::Forwarded {
        machine: final_source_machine,
        ..
    } = plan.origin
    else {
        return unsupported("forwarded helper chain requires a forwarded checked origin");
    };
    if plan.forwarding_transfers.len() + 1 != helpers.len()
        || plan.forwarding_helpers.len() != helpers.len()
    {
        return unsupported("forwarded helper chain length drifted from checked custody");
    }
    helpers
        .iter()
        .enumerate()
        .map(|(index, ids)| {
            let source_machine = plan
                .forwarding_transfers
                .get(index)
                .map(|transfer| transfer.caller_machine)
                .unwrap_or(final_source_machine);
            let next_helper = helpers.get(index + 1).map(|next| next.machine);
            let body = &plan.forwarding_helpers[index];
            let (state, coordinate) = if let Some(transfer) = plan.forwarding_transfers.get(index) {
                (transfer.caller_state, transfer.coordinate)
            } else if let checked_trees::CheckedDynamicScalarCallOrigin::Forwarded {
                state,
                coordinate,
                ..
            } = plan.origin
            {
                (state, coordinate)
            } else {
                return unsupported("forwarded helper has no source call");
            };
            if body.machine != source_machine
                || body.state != state
                || body.call_result.statement_index != coordinate.statement_index
                || body.call_result.primitive_type != plan.result.primitive_type
            {
                return unsupported("forwarded helper body changed its source call");
            }
            materialize_forwarded_helper_for_source(
                checked,
                plan,
                application,
                selected_row,
                *ids,
                source_machine,
                next_helper,
                body,
                next_block,
                next_operation,
                next_value,
                next_edge,
                source_calls,
            )
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn materialize_helper_body(
    checked: &CheckedTrees,
    body: &checked_trees::CheckedDynamicScalarHelperPlan,
    ids: ForwardedHelperIds,
    source_machine: symbols::SymbolHandle,
    next_helper: Option<semantic_vocabulary::MachineId>,
    requirement_slot: u32,
    scalar_type: semantic_vocabulary::ScalarType,
    next_block: &mut u64,
    next_operation: &mut u64,
    next_value: &mut u64,
    next_edge: &mut u64,
    source_calls: &mut [LoweredSourceCallOccurrence],
) -> Result<Vec<Block>, LoweringError> {
    use crate::emission::operation_emission::buffer::OperationBuffer;
    use crate::emission::operation_emission::calls::CallEmissionContext;
    use crate::unit::attached_unit::argument_evaluation::Evaluation;
    use checked_trees::CheckedScalarExpressionRole;
    use checked_trees::statement::StatementNode;

    let prefix = crate::unit::attached_unit::validate_scalar_control_tail(
        checked,
        source_machine,
        body.state,
        &body.scalar_control,
    )?;
    if body.scalar_locals.len().checked_add(1) != Some(prefix)
        || body.call_result.statement_index as usize >= prefix
        || terminal_scalar_type(body.scalar_control.primitive_type)? != scalar_type
    {
        return unsupported("forwarded helper omitted or changed its scalar body");
    }
    let (_, state) =
        crate::expression_preparation::source_custody::authored_state(checked, body.state)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    let mut reserved_entry = ids.block.get();
    let mut evaluation = Evaluation::new(&mut reserved_entry)?;
    let mut operations = OperationBuffer::new(next_operation.checked_sub(1).ok_or(
        LoweringError::Unsupported("forwarded helper operation identities are absent"),
    )?);
    let mut values = Vec::new();
    let mut locals = body.scalar_locals.iter();
    let mut calls = CallEmissionContext {
        machine_ids: &[],
        requirement_counts: &[],
        next_obligation_identity: 1,
        obligation_limit: 1,
    };
    for (ordinal, statement) in statements.iter().take(prefix).enumerate() {
        let StatementNode::LocalData(local) = statement else {
            return unsupported("forwarded helper lost an ordered scalar binding");
        };
        if local.is_mutable
            || !local.symbol.is_valid()
            || !checked
                .expression_table
                .expression_is_valid(local.initial_value)
        {
            return unsupported("forwarded helper cannot erase storage or an absent initializer");
        }
        let (binding, value) = if ordinal == body.call_result.statement_index as usize {
            (&body.call_result, None)
        } else {
            let (binding, value) = locals.next().ok_or(LoweringError::Unsupported(
                "forwarded helper omitted a scalar initializer",
            ))?;
            (binding, Some(value))
        };
        if binding.statement_index as usize != ordinal
            || binding.binding_ordinal as usize != values.len()
            || checked.primitive_type_reference(local.type_reference)
                != Some(binding.primitive_type)
        {
            return unsupported("forwarded helper binding order or type drifted");
        }
        let value = if let Some(value) = value {
            evaluation.source_value(
                checked,
                source_machine,
                body.state,
                binding.statement_index,
                CheckedScalarExpressionRole::LocalInitializer {
                    binding_ordinal: binding.binding_ordinal,
                },
                &checked_trees::CheckedCallScalarArgument::Pure(value.clone()),
                values.len(),
                &mut values,
                next_value,
                next_block,
                next_edge,
                &mut operations,
                &mut calls,
            )?
        } else {
            let occurrence = source_calls
                .iter_mut()
                .find(|call| call.terminal_operation == ids.operation)
                .ok_or(LoweringError::Unsupported(
                    "forwarded helper lost its source call occurrence",
                ))?;
            let checked_trees::expression::ExpressionNode::Call(call) =
                checked.expression_table.expression(local.initial_value)
            else {
                return unsupported("forwarded helper substituted its call initializer");
            };
            if occurrence.source_state != body.state
                || occurrence.statement_index != ordinal
                || occurrence.call_ordinal != 0
                || occurrence.source_target != call.target_symbol
            {
                return unsupported("forwarded helper call coordinate drifted");
            }
            occurrence.source_values_before_call = values.clone();
            let result = ValueDeclaration {
                qualifications: Default::default(),
                id: ids.operation_value,
                scalar_type,
            };
            operations.push(Operation {
                static_reach_binding: None,
                id: ids.operation,
                result: OperationResult::Scalar(result),
                kind: match next_helper {
                    Some(callee) => OperationKind::CallStructuralScalar {
                        callee,
                        arguments: Vec::new(),
                        erased_arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                    None => OperationKind::CallDynamicParameterScalar {
                        parameter_ordinal: 0,
                        requirement_slot,
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                },
            });
            result
        };
        if value.scalar_type != terminal_scalar_type(binding.primitive_type)? {
            return unsupported("forwarded helper changed a scalar initializer type");
        }
        values.push(value);
    }
    let result = evaluation.scalar_control_result(
        checked,
        source_machine,
        body.state,
        &body.scalar_control,
        &mut values,
        next_value,
        next_block,
        next_edge,
        &mut operations,
        &mut calls,
    )?;
    // Pure expressions may expand to branches, but must not invent additional
    // calls, selected providers or proof obligations outside this helper plan.
    if !operations.source_calls.is_empty()
        || !operations.selected_ieee_float_fmas.is_empty()
        || !operations.selected_ieee_float_comparisons.is_empty()
        || !operations.selected_integer_comparisons.is_empty()
    {
        return unsupported("forwarded helper introduced an unretained scalar call");
    }
    evaluation.blocks.push(Block {
        id: evaluation.current,
        parameters: evaluation.parameters,
        structural_parameters: evaluation.block_structural_parameters,
        erased_scalar_formals: Vec::new(),
        operations: operations[evaluation.operation_start..].to_vec(),
        terminator: Terminator::Return {
            edge: ids.edge,
            value: result.id,
            cleanup_actions: Vec::new(),
        },
    });
    *next_operation = operations.next_identity;
    evaluation.blocks.sort_by_key(|block| block.id);
    Ok(evaluation.blocks)
}

pub(crate) fn dynamic_source_call_occurrences_for_chain(
    plan: &CheckedDynamicScalarCallPlan,
    caller_operation: semantic_vocabulary::OperationId,
    helpers: &[ForwardedHelperIds],
) -> Result<Vec<LoweredSourceCallOccurrence>, LoweringError> {
    if helpers.len() <= 1 {
        return dynamic_source_call_occurrences(plan, caller_operation, helpers.first().copied());
    }
    let checked_trees::CheckedDynamicScalarCallOrigin::Forwarded {
        state: final_state,
        coordinate: final_coordinate,
        ..
    } = plan.origin
    else {
        return unsupported("forwarded source-call chain requires a forwarded checked origin");
    };
    let first_state = plan
        .forwarding_transfers
        .first()
        .ok_or(LoweringError::Unsupported(
            "multi-hop source-call chain lost its first transfer",
        ))?
        .caller_state;
    let mut occurrences = vec![LoweredSourceCallOccurrence {
        source_site: None,
        source_state: plan.caller_state,
        statement_index: usize::try_from(plan.coordinate.statement_index).map_err(|_| {
            LoweringError::Unsupported("direct dynamic statement coordinate exceeds usize")
        })?,
        call_ordinal: usize::try_from(plan.coordinate.call_ordinal)
            .map_err(|_| LoweringError::Unsupported("direct dynamic call ordinal exceeds usize"))?,
        terminal_operation: caller_operation,
        source_target: first_state,
        source_values_before_call: Vec::new(),
    }];
    for (transfer, helper) in plan.forwarding_transfers.iter().zip(helpers) {
        occurrences.push(LoweredSourceCallOccurrence {
            source_site: None,
            source_state: transfer.caller_state,
            statement_index: usize::try_from(transfer.coordinate.statement_index).map_err(
                |_| {
                    LoweringError::Unsupported(
                        "parameter forwarding statement coordinate exceeds usize",
                    )
                },
            )?,
            call_ordinal: usize::try_from(transfer.coordinate.call_ordinal).map_err(|_| {
                LoweringError::Unsupported("parameter forwarding call ordinal exceeds usize")
            })?,
            terminal_operation: helper.operation,
            source_target: transfer.target_state,
            source_values_before_call: Vec::new(),
        });
    }
    let final_helper = helpers.last().ok_or(LoweringError::Unsupported(
        "multi-hop source-call chain has no final helper",
    ))?;
    occurrences.push(LoweredSourceCallOccurrence {
        source_site: None,
        source_state: final_state,
        statement_index: usize::try_from(final_coordinate.statement_index).map_err(|_| {
            LoweringError::Unsupported("forwarded dynamic statement coordinate exceeds usize")
        })?,
        call_ordinal: usize::try_from(final_coordinate.call_ordinal).map_err(|_| {
            LoweringError::Unsupported("forwarded dynamic call ordinal exceeds usize")
        })?,
        terminal_operation: final_helper.operation,
        source_target: plan.requirement,
        source_values_before_call: Vec::new(),
    });
    Ok(occurrences)
}

fn dynamic_source_call_occurrences(
    plan: &CheckedDynamicScalarCallPlan,
    caller_operation: semantic_vocabulary::OperationId,
    forwarded_helper: Option<ForwardedHelperIds>,
) -> Result<Vec<LoweredSourceCallOccurrence>, LoweringError> {
    let statement_index = usize::try_from(plan.coordinate.statement_index).map_err(|_| {
        LoweringError::Unsupported("direct dynamic statement coordinate exceeds usize")
    })?;
    let call_ordinal = usize::try_from(plan.coordinate.call_ordinal)
        .map_err(|_| LoweringError::Unsupported("direct dynamic call ordinal exceeds usize"))?;
    let mut occurrences = vec![LoweredSourceCallOccurrence {
        source_site: None,
        source_state: plan.caller_state,
        statement_index,
        call_ordinal,
        terminal_operation: caller_operation,
        source_target: match plan.origin {
            checked_trees::CheckedDynamicScalarCallOrigin::Local => plan.requirement,
            checked_trees::CheckedDynamicScalarCallOrigin::Forwarded { state, .. } => state,
        },
        source_values_before_call: Vec::new(),
    }];
    if let (
        Some(helper),
        checked_trees::CheckedDynamicScalarCallOrigin::Forwarded {
            state, coordinate, ..
        },
    ) = (forwarded_helper, plan.origin)
    {
        occurrences.push(LoweredSourceCallOccurrence {
            source_site: None,
            source_state: state,
            statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                LoweringError::Unsupported("forwarded dynamic statement coordinate exceeds usize")
            })?,
            call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                LoweringError::Unsupported("forwarded dynamic call ordinal exceeds usize")
            })?,
            terminal_operation: helper.operation,
            source_target: plan.requirement,
            source_values_before_call: Vec::new(),
        });
    }
    Ok(occurrences)
}
