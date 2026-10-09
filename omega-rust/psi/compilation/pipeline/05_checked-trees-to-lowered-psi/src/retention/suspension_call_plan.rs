//! Join checked suspension demands to the ordinary call emitter's namespaces.
//!
//! Source environments, scalar/structural operand lanes and machine-local
//! claims have different coordinates. The ephemeral call companion names
//! current homes and established identities; the callee's checked telescope
//! reindexes actual argument lanes. Neither join recreates storage, infers a
//! claim from a type, nor admits a loan without its checked carry obligations.
//! Runtime backing and parking remain selected-provider responsibilities.

use super::{
    CheckedTrees, LoweredSourceCallOccurrence, LoweringError, OperationId, OperationKind,
    TerminalModule, ValueDeclaration, ValueId, terminal_scalar_type, unsupported,
};
use crate::direct_calls::direct_call_parts;
use crate::terminal_identities::lookup_claim_id;
use terminal_psi::{TerminalSuspensionPlace, TerminalSuspensionValueType};
pub(crate) fn retain_suspension_call_plans(
    checked: &CheckedTrees,
    source_machines: &[symbols::SymbolHandle],
    source_calls: &[LoweredSourceCallOccurrence],
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    if !module.suspension_call_plans.is_empty() {
        return unsupported("suspension call plans must be produced exactly once");
    }

    let mut plans = Vec::new();
    for crossing in checked
        .facts
        .carry
        .suspension_crossings
        .iter()
        .filter(|crossing| source_machines.contains(&crossing.machine))
    {
        let mut occurrences = source_calls.iter().filter(|occurrence| {
            occurrence.source_state == crossing.state
                && occurrence.statement_index == crossing.statement_index
                && occurrence.call_ordinal == crossing.call_ordinal
        });
        let occurrence = occurrences.next().ok_or(LoweringError::Unsupported(
            "possibly-suspending checked call has no exact Terminal operation join",
        ))?;
        let terminal_operation =
            occurrence
                .terminal_site
                .operation()
                .ok_or(LoweringError::Unsupported(
                    "external completion cannot park a continuing activation",
                ))?;
        // Emission and checking name the same callee differently: scalar and
        // structural-result calls record the callee machine symbol while Unit
        // calls and `call.target_symbol` carry the resolved entry state.
        // Comparing owning-machine identities keeps the redirect check sound —
        // one coordinate maps to one authored callee — across both spellings.
        if occurrences.next().is_some()
            || owning_callable_symbol(checked, occurrence.source_target)
                != owning_callable_symbol(checked, crossing.target)
        {
            return unsupported("suspension crossing call join is duplicate or redirected");
        }
        let (machine_index, block_index, operation_index) = module
            .machines
            .iter()
            .enumerate()
            .flat_map(|(machine_index, machine)| {
                machine
                    .blocks
                    .iter()
                    .enumerate()
                    .flat_map(move |(block_index, block)| {
                        block.operations.iter().enumerate().map(
                            move |(operation_index, operation)| {
                                (machine_index, block_index, operation_index, operation)
                            },
                        )
                    })
            })
            .find(|(.., operation)| operation.id == terminal_operation)
            .map(|(machine_index, block_index, operation_index, _)| {
                (machine_index, block_index, operation_index)
            })
            .ok_or(LoweringError::Unsupported(
                "suspension crossing Terminal operation is absent",
            ))?;
        let operation =
            &module.machines[machine_index].blocks[block_index].operations[operation_index];
        let target = terminal_call_target(&operation.kind).ok_or(LoweringError::Unsupported(
            "suspension crossing Terminal operation is not an ordinary call",
        ))?;

        let mut live_values = Vec::with_capacity(crossing.live_values.len());
        for live in &crossing.live_values {
            let (place, value_type) = match live.origin {
                checked_trees::SuspensionCrossingValueOrigin::Parameter { position, .. }
                | checked_trees::SuspensionCrossingValueOrigin::Local {
                    environment_position: position,
                    ..
                } => source_binding(&occurrence.frontier_before_call, position)?,
                checked_trees::SuspensionCrossingValueOrigin::CallArgument { position } => {
                    call_argument_binding(
                        checked,
                        crossing.target,
                        &operation.kind,
                        module,
                        position,
                    )?
                }
            };
            if let TerminalSuspensionPlace::Scalar(value) = place {
                if !scalar_value_available_before(module, terminal_operation, value) {
                    return unsupported("suspension frontier value is unavailable at its call");
                }
                let source_type = checked
                    .primitive_type_reference(live.type_reference)
                    .ok_or(LoweringError::Unsupported(
                        "suspension scalar frontier has no checked scalar type",
                    ))?;
                if TerminalSuspensionValueType::Scalar(terminal_scalar_type(source_type)?)
                    != value_type
                {
                    return unsupported("suspension frontier scalar type join is inconsistent");
                }
            } else if let TerminalSuspensionPlace::Structural { place, .. } = &place
                && !module.machines[machine_index]
                    .structural_places
                    .iter()
                    .any(|declaration| declaration.id == *place)
            {
                return unsupported(
                    "suspension structural frontier has no caller storage declaration",
                );
            }
            let mut claims = live
                .claims
                .iter()
                .map(|identity| lookup_claim_id(&occurrence.frontier_before_call.claims, *identity))
                .collect::<Result<Vec<_>, _>>()?;
            claims.sort();
            live_values.push(terminal_psi::TerminalSuspensionLiveValue {
                place,
                value_type,
                storage: match live.storage {
                    checked_trees::SuspensionCrossingStorage::Parameter => {
                        terminal_psi::TerminalSuspensionStorage::Parameter
                    }
                    checked_trees::SuspensionCrossingStorage::Local => {
                        terminal_psi::TerminalSuspensionStorage::Local
                    }
                    checked_trees::SuspensionCrossingStorage::CallArgument => {
                        terminal_psi::TerminalSuspensionStorage::CallArgument
                    }
                },
                claim_count: u32::try_from(claims.len()).map_err(|_| {
                    LoweringError::Unsupported(
                        "suspension frontier claim count exceeds the Terminal vocabulary",
                    )
                })?,
                claims,
                effective: live.effective,
            });
        }
        live_values.sort_by(|left, right| {
            (&left.place, left.storage, left.value_type, left.effective).cmp(&(
                &right.place,
                right.storage,
                right.value_type,
                right.effective,
            ))
        });
        let crossing_id = checked_trees::canonical_suspension_crossing_id(&checked.typed, crossing)
            .ok_or(LoweringError::Unsupported(
                "suspension crossing identity cannot resolve its source symbols",
            ))?;
        // Bind the authoritative possibly-suspending demand onto the call
        // itself: the site and plan rows below are evidence for this marker,
        // not its source, so deleting the paired rows cannot erase the
        // required crossing.
        module.machines[machine_index].blocks[block_index].operations[operation_index]
            .suspension_crossing = Some(crossing_id);
        plans.push(terminal_psi::TerminalSuspensionCallPlan {
            operation: terminal_operation,
            crossing: crossing_id,
            target,
            effective: crossing.effective,
            live_value_count: u32::try_from(live_values.len()).map_err(|_| {
                LoweringError::Unsupported(
                    "suspension live-value count exceeds the Terminal vocabulary",
                )
            })?,
            live_values,
        });
    }
    plans.sort_by_key(|plan| (plan.operation, plan.crossing));
    module.suspension_call_plan_count = u32::try_from(plans.len()).map_err(|_| {
        LoweringError::Unsupported("suspension call plan count exceeds the Terminal vocabulary")
    })?;
    module.suspension_call_sites = plans
        .iter()
        .map(|plan| terminal_psi::TerminalSuspensionCallSite {
            operation: plan.operation,
            crossing: plan.crossing,
            target: plan.target,
            frontier_commitment: terminal_psi::suspension_frontier_commitment(plan),
        })
        .collect();
    module.suspension_call_plans = plans;
    Ok(())
}

/// Source positions and Terminal lanes are different namespaces. A structural
/// binding keeps the current home and path; an SSA binding keeps its value.
fn source_binding(
    bindings: &lowered_psi::LoweredSourceFrontierBindings,
    position: usize,
) -> Result<(TerminalSuspensionPlace, TerminalSuspensionValueType), LoweringError> {
    if let Some(binding) = bindings
        .scalars
        .iter()
        .find(|binding| binding.source_position == position)
    {
        return Ok((
            TerminalSuspensionPlace::Scalar(binding.value.id),
            TerminalSuspensionValueType::Scalar(binding.value.scalar_type),
        ));
    }
    let binding = bindings
        .structural
        .iter()
        .find(|binding| binding.source_position == position)
        .ok_or(LoweringError::Unsupported(
            "suspension frontier source position has no storage binding",
        ))?;
    Ok((
        TerminalSuspensionPlace::Structural {
            place: binding.argument.place,
            path: binding.argument.path.clone(),
        },
        TerminalSuspensionValueType::Structural(binding.structural_type),
    ))
}

/// Reindex one checked ordinary argument through the callee's explicit formal
/// telescope, excluding erased formals from both runtime lanes. Unit, scalar
/// and structural results do not change the argument namespace. The existing
/// emitted operand and callee formal supply the storage and type; this join
/// does not materialize another argument or infer linear claims from its type.
fn call_argument_binding(
    checked: &CheckedTrees,
    target: symbols::SymbolHandle,
    operation: &OperationKind,
    module: &TerminalModule,
    position: usize,
) -> Result<(TerminalSuspensionPlace, TerminalSuspensionValueType), LoweringError> {
    let parameters = validation::call_target_parameters(&checked.typed, target).ok_or(
        LoweringError::Unsupported("suspension call argument has no checked formal telescope"),
    )?;
    let parameter = parameters
        .get(position)
        .filter(|parameter| !parameter.relevance.is_erased())
        .ok_or(LoweringError::Unsupported(
            "suspension call argument has no runtime formal",
        ))?;
    let scalar = checked
        .primitive_type_reference(parameter.type_reference)
        .is_some();
    let lane_position = parameters[..position]
        .iter()
        .filter(|parameter| {
            !parameter.relevance.is_erased()
                && checked
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
                    == scalar
        })
        .count();
    let (scalars, structural, structural_parameters) =
        if let Some(call) = direct_call_parts(operation) {
            let callee = module
                .machines
                .iter()
                .find(|machine| machine.id == call.callee)
                .ok_or(LoweringError::Unsupported(
                    "suspension call argument has no emitted callee",
                ))?;
            (
                call.arguments,
                call.structural_arguments,
                callee.structural_parameters.as_slice(),
            )
        } else if let OperationKind::BoundaryCall {
            boundary,
            arguments,
            structural_arguments,
            ..
        } = operation
        {
            let callee = module
                .boundary_machines
                .iter()
                .find(|callee| callee.id == *boundary)
                .ok_or(LoweringError::Unsupported(
                    "suspension call argument has no emitted requirement",
                ))?;
            (
                arguments.as_slice(),
                structural_arguments.as_slice(),
                callee.structural_parameters.as_slice(),
            )
        } else {
            return unsupported("suspension call argument is not an ordinary call");
        };
    if scalar {
        let value = scalars
            .get(lane_position)
            .copied()
            .and_then(|value| scalar_declaration(module, value))
            .ok_or(LoweringError::Unsupported(
                "suspension call scalar argument has no emitted value",
            ))?;
        return Ok((
            TerminalSuspensionPlace::Scalar(value.id),
            TerminalSuspensionValueType::Scalar(value.scalar_type),
        ));
    }
    let argument = structural
        .get(lane_position)
        .ok_or(LoweringError::Unsupported(
            "suspension call structural argument has no emitted storage",
        ))?;
    let parameter = structural_parameters
        .get(lane_position)
        .ok_or(LoweringError::Unsupported(
            "suspension call structural argument has no emitted formal",
        ))?;
    Ok((
        TerminalSuspensionPlace::Structural {
            place: argument.place,
            path: argument.path.clone(),
        },
        TerminalSuspensionValueType::Structural(parameter.structural_type),
    ))
}

fn owning_callable_symbol(
    checked: &CheckedTrees,
    symbol: symbols::SymbolHandle,
) -> symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| {
            typed_trees::state::state_by_symbol(checked.machine_states(machine), symbol).is_some()
        })
        .map_or(symbol, |machine| machine.symbol)
}

fn terminal_call_target(
    kind: &OperationKind,
) -> Option<terminal_psi::TerminalSuspensionCallTarget> {
    if let Some(call) = direct_call_parts(kind) {
        return Some(terminal_psi::TerminalSuspensionCallTarget::Machine(
            call.callee,
        ));
    }
    match kind {
        OperationKind::BoundaryCall { boundary, .. } => Some(
            terminal_psi::TerminalSuspensionCallTarget::Boundary(*boundary),
        ),
        _ => None,
    }
}

fn scalar_declaration(module: &TerminalModule, value: ValueId) -> Option<ValueDeclaration> {
    module.machines.iter().find_map(|machine| {
        machine
            .parameters
            .iter()
            .chain(machine.block_parameters())
            .chain(
                machine
                    .operations()
                    .filter_map(|operation| operation.result.scalar_ref()),
            )
            .find(|declaration| declaration.id == value)
            .copied()
    })
}

/// The checked binding supplies source provenance; SSA visibility supplies the
/// emitted scope. A local can alias an ingress value or travel through a private
/// join parameter without manufacturing a new operation result at every call.
fn scalar_value_available_before(
    module: &TerminalModule,
    operation: OperationId,
    value: ValueId,
) -> bool {
    module.machines.iter().any(|machine| {
        machine.blocks.iter().any(|block| {
            block
                .operations
                .iter()
                .position(|candidate| candidate.id == operation)
                .is_some_and(|operation_index| {
                    machine
                        .parameters
                        .iter()
                        .chain(block.parameters.iter())
                        .chain(
                            block.operations[..operation_index]
                                .iter()
                                .filter_map(|candidate| candidate.result.scalar_ref()),
                        )
                        .any(|declaration| declaration.id == value)
                })
        })
    })
}
