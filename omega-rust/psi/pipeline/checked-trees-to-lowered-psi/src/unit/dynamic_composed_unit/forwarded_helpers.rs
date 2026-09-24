//! Forwarded helper chains retain descriptor custody separately from the
//! caller. Each helper replaces one checked machine whose call forwards the
//! descriptor parameter to the next helper, or, for the last one, dispatches
//! through it. The chain, its descriptor catalog and its source calls are
//! lowered once for either result; a lane supplies only the helper's body.
//! A scalar helper's call result is one binding in its authored sequence, not
//! necessarily its returned value, so ordinary scalar evaluation and exit
//! replay own the surrounding calculations and returning branches. A Unit
//! helper's body is its one forwarding call.

use crate::unit::dynamic_composed_unit::applications::{
    exact_machine_service_summary, validate_empty_contract, validate_empty_service_summary,
};
use crate::unit::dynamic_composed_unit::dynamic_lanes::{
    DynamicCall, DynamicCallView, ForwardedHelperIds, ForwardedHelperValues,
    LoweredDynamicRealization,
};
use crate::unit::dynamic_composed_unit::source_lowering::{
    dynamic_parameter_interface, machine_call,
};
use crate::unit::dynamic_composed_unit::store_operations::empty_terminal_contract;
use crate::unit::{
    CheckedTrees, LoweredSourceCallOccurrence, LoweringError, allocate_dense, block_id, edge_id,
    lower_installation_machine_service_ceiling, machine_id, operation_id, terminal_scalar_type,
    unsupported, value_id,
};
use checked_trees::CheckedUnitCallCoordinate;
use semantic_vocabulary::ScalarType;
use symbols::SymbolHandle;
use terminal_psi::{
    Block, ClosedConformanceApplication, ClosedConformanceRow, Operation, OperationKind,
    OperationResult, TerminalDynamicDescriptorArgument, TerminalDynamicDescriptorSource,
    TerminalDynamicDispatchCatalog, TerminalMachine, TerminalMachineResult, Terminator,
    ValueDeclaration,
};

/// One forwarded helper as the chain materializes it.
pub(crate) struct ForwardedHelperSite {
    /// The helper's position in the chain, outermost first.
    pub(crate) index: usize,
    pub(crate) ids: ForwardedHelperIds,
    /// The checked machine and state this helper replaces, and the
    /// coordinate of its call.
    pub(crate) source_machine: SymbolHandle,
    pub(crate) source_state: SymbolHandle,
    pub(crate) source_coordinate: CheckedUnitCallCoordinate,
    /// The next helper this one calls; `None` for the dispatching helper.
    pub(crate) next_helper: Option<semantic_vocabulary::MachineId>,
    pub(crate) requirement_slot: u32,
    /// The helper's scalar result type, or `None` for a Unit helper.
    pub(crate) result_type: Option<ScalarType>,
}

/// Identities for the forwarded helper chain, numbered after the realization
/// machines. A scalar helper also binds its call's result and its own.
pub(crate) fn forwarded_helper_chain_ids<Call: DynamicCall>(
    call: &Call,
    realizations: &[LoweredDynamicRealization],
    next_block: &mut u64,
    next_operation: &mut u64,
    next_value: &mut u64,
    next_edge: &mut u64,
) -> Result<Vec<ForwardedHelperIds>, LoweringError> {
    let plan = call.view();
    if plan.forwarded.is_none() {
        if !plan.forwarding_transfers.is_empty()
            || call.helper_body_count().is_some_and(|count| count != 0)
        {
            return unsupported("local dynamic call retained forwarding transfers");
        }
        return Ok(Vec::new());
    }
    let returns_scalar = call.result().is_some();
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
                scalar_values: if returns_scalar {
                    Some(ForwardedHelperValues {
                        call: value_id(allocate_dense(next_value)?),
                        result: value_id(allocate_dense(next_value)?),
                    })
                } else {
                    None
                },
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

/// The forwarded helper machines: each validates the checked machine it
/// replaces as contract- and service-free, publishes that machine's service
/// ceiling and returns its lane's result from the lane's body.
#[allow(clippy::too_many_arguments)]
pub(crate) fn materialize_forwarded_helper_chain<Call: DynamicCall>(
    checked: &CheckedTrees,
    call: &Call,
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
    let plan = call.view();
    let Some(origin) = plan.forwarded else {
        return unsupported("forwarded helper chain requires a forwarded checked origin");
    };
    if plan.forwarding_transfers.len() + 1 != helpers.len()
        || call
            .helper_body_count()
            .is_some_and(|count| count != helpers.len())
    {
        return unsupported("forwarded helper chain length drifted from checked custody");
    }
    let (_, requirement_slot) = dynamic_parameter_interface(application, selected_row)?;
    let result_type = call.result_type()?;
    helpers
        .iter()
        .enumerate()
        .map(|(index, ids)| {
            let (source_machine, source_state, source_coordinate) =
                match plan.forwarding_transfers.get(index) {
                    Some(transfer) => (
                        transfer.caller_machine,
                        transfer.caller_state,
                        transfer.coordinate,
                    ),
                    None => (origin.machine, origin.state, origin.coordinate),
                };
            let site = ForwardedHelperSite {
                index,
                ids: *ids,
                source_machine,
                source_state,
                source_coordinate,
                next_helper: helpers.get(index + 1).map(|next| next.machine),
                requirement_slot,
                result_type,
            };
            let published_service_ceiling = forwarded_helper_service_ceiling(checked, &site)?;
            let result = match (result_type, ids.scalar_values) {
                (Some(scalar_type), Some(values)) => {
                    TerminalMachineResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: values.result,
                        scalar_type,
                    })
                }
                (None, None) => TerminalMachineResult::Unit,
                _ => return unsupported("forwarded helper identities disagree with its result"),
            };
            let blocks = call.materialize_helper_body(
                checked,
                &site,
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
                result,
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
        })
        .collect()
}

/// The replaced machine must retain an empty checked contract and an empty
/// service summary; its installation ceiling becomes the helper's.
fn forwarded_helper_service_ceiling(
    checked: &CheckedTrees,
    site: &ForwardedHelperSite,
) -> Result<Vec<semantic_vocabulary::ServiceId>, LoweringError> {
    let checked_contract = checked
        .facts
        .contract_plans
        .for_machine(site.source_machine)
        .ok_or(LoweringError::Unsupported(
            "forwarded dynamic helper has no checked contract",
        ))?;
    validate_empty_contract(
        checked,
        site.source_machine,
        checked_contract.report_fingerprint,
        checked_contract.commitment,
    )?;
    let service_summary = exact_machine_service_summary(checked, site.source_machine)?;
    validate_empty_service_summary(checked, service_summary)?;
    let service_contract = checked
        .facts
        .service_reaches
        .plan_for_machine(site.source_machine)
        .ok_or(LoweringError::Unsupported(
            "forwarded dynamic helper has no checked service contract",
        ))?;
    lower_installation_machine_service_ceiling(
        checked,
        site.source_machine,
        service_contract,
        service_summary,
        &[],
    )
}

/// The helper's own call: into the next helper, or through its descriptor
/// parameter when it dispatches.
fn forwarded_helper_call(site: &ForwardedHelperSite) -> OperationKind {
    let returns_scalar = site.result_type.is_some();
    match site.next_helper {
        Some(callee) => machine_call(callee, Vec::new(), returns_scalar),
        None if returns_scalar => OperationKind::CallDynamicParameterScalar {
            parameter_ordinal: 0,
            requirement_slot: site.requirement_slot,
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
        None => OperationKind::CallDynamicParameterUnit {
            parameter_ordinal: 0,
            requirement_slot: site.requirement_slot,
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }
}

/// A Unit helper's one block: its forwarding call, then its return.
pub(crate) fn materialize_unit_helper_body(site: &ForwardedHelperSite) -> Block {
    Block {
        structural_parameters: Vec::new(),
        id: site.ids.block,
        parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        operations: vec![Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: site.ids.operation,
            result: OperationResult::Unit,
            kind: forwarded_helper_call(site),
        }],
        terminator: Terminator::ReturnUnit {
            edge: site.ids.edge,
            trivial_affine_discards: Vec::new(),
        },
    }
}

/// A scalar helper's blocks: its ordered pure locals with its call's result
/// at the checked position, then its checked scalar control.
#[allow(clippy::too_many_arguments)]
pub(crate) fn materialize_scalar_helper_body(
    checked: &CheckedTrees,
    body: &checked_trees::CheckedDynamicScalarHelperPlan,
    site: &ForwardedHelperSite,
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

    let ids = site.ids;
    let source_machine = site.source_machine;
    let values_ids = ids.scalar_values.ok_or(LoweringError::Unsupported(
        "forwarded scalar helper has no result values",
    ))?;
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
                id: values_ids.call,
                scalar_type,
            };
            operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: ids.operation,
                result: OperationResult::Scalar(result),
                kind: forwarded_helper_call(site),
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
        erased_proof_formals: Vec::new(),
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

/// The source-call occurrences of one dispatch: each caller's call into the
/// first forwarded helper (or into the requirement when the call is local),
/// each forwarding transfer's call, then the final helper's dispatch. Every
/// forwarded caller shares the first caller's forwarding chain.
pub(crate) fn dynamic_source_call_occurrences(
    callers: &[(&DynamicCallView<'_>, semantic_vocabulary::OperationId)],
    helpers: &[ForwardedHelperIds],
) -> Result<Vec<LoweredSourceCallOccurrence>, LoweringError> {
    let [(chain, _), ..] = callers else {
        return unsupported("dynamic source-call chain has no caller");
    };
    let mut occurrences = callers
        .iter()
        .map(|(caller, operation)| {
            let target = match caller.forwarded {
                Some(origin) => caller
                    .forwarding_transfers
                    .first()
                    .map_or(origin.state, |transfer| transfer.caller_state),
                None => caller.requirement,
            };
            source_call_occurrence(caller.caller_state, caller.coordinate, *operation, target)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let Some(origin) = chain.forwarded else {
        return Ok(occurrences);
    };
    for (transfer, helper) in chain.forwarding_transfers.iter().zip(helpers) {
        occurrences.push(source_call_occurrence(
            transfer.caller_state,
            transfer.coordinate,
            helper.operation,
            transfer.target_state,
        )?);
    }
    let final_helper = helpers.last().ok_or(LoweringError::Unsupported(
        "forwarded source-call chain has no final helper",
    ))?;
    occurrences.push(source_call_occurrence(
        origin.state,
        origin.coordinate,
        final_helper.operation,
        chain.requirement,
    )?);
    Ok(occurrences)
}

fn source_call_occurrence(
    state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    operation: semantic_vocabulary::OperationId,
    target: SymbolHandle,
) -> Result<LoweredSourceCallOccurrence, LoweringError> {
    Ok(LoweredSourceCallOccurrence {
        source_site: None,
        source_state: state,
        statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
            LoweringError::Unsupported("dynamic source-call statement coordinate exceeds usize")
        })?,
        call_ordinal: usize::try_from(coordinate.call_ordinal)
            .map_err(|_| LoweringError::Unsupported("dynamic source-call ordinal exceeds usize"))?,
        terminal_operation: operation,
        source_target: target,
        source_values_before_call: Vec::new(),
    })
}
