use super::*;

pub(super) fn retain_suspension_call_plans(
    checked: &CheckedTrees,
    source_machines: &[psi_symbols::SymbolHandle],
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
        if crossing.receiver.is_some() {
            return unsupported(
                "receiver-bearing suspension frontier lacks an exact Terminal receiver place join",
            );
        }
        let mut occurrences = source_calls.iter().filter(|occurrence| {
            occurrence.source_state == crossing.state
                && occurrence.statement_index == crossing.statement_index
                && occurrence.call_ordinal == crossing.call_ordinal
        });
        let occurrence = occurrences.next().ok_or(LoweringError::Unsupported(
            "possibly-suspending checked call has no exact Terminal operation join",
        ))?;
        if occurrences.next().is_some() || occurrence.source_target != crossing.target {
            return unsupported("suspension crossing call join is duplicate or redirected");
        }
        let operation = module
            .machines
            .iter()
            .flat_map(|machine| machine.blocks.iter())
            .flat_map(|block| block.operations.iter())
            .find(|operation| operation.id == occurrence.terminal_operation)
            .ok_or(LoweringError::Unsupported(
                "suspension crossing Terminal operation is absent",
            ))?;
        let call_arguments = match &operation.kind {
            OperationKind::Call { arguments, .. } => arguments.as_slice(),
            _ => {
                return unsupported(
                    "suspension frontier lowering supports receiver-free scalar calls only",
                );
            }
        };
        let target = terminal_call_target(&operation.kind).ok_or(LoweringError::Unsupported(
            "suspension crossing Terminal operation is not an ordinary call",
        ))?;

        let mut live_values = Vec::with_capacity(crossing.live_values.len());
        for live in &crossing.live_values {
            if !source_origin_is_exact(checked, crossing.state, live.origin) {
                return unsupported("suspension frontier source value origin is inexact");
            }
            if !live.claims.is_empty() {
                return unsupported(
                    "suspension frontier has live claims without an exact Terminal ClaimId join",
                );
            }
            let value = match live.origin {
                psi_checked_trees::SuspensionCrossingValueOrigin::Parameter {
                    position, ..
                } => occurrence
                    .source_values_before_call
                    .get(position)
                    .copied()
                    .ok_or(LoweringError::Unsupported(
                        "suspension frontier scalar environment position is unavailable",
                    ))?,
                psi_checked_trees::SuspensionCrossingValueOrigin::Local {
                    environment_position,
                    ..
                } => {
                    let value = occurrence
                        .source_values_before_call
                        .get(environment_position)
                        .copied()
                        .ok_or(LoweringError::Unsupported(
                            "suspension frontier local environment position is unavailable",
                        ))?;
                    if !scalar_operation_result_before(
                        module,
                        occurrence.terminal_operation,
                        value.id,
                    ) {
                        return unsupported(
                            "threaded local suspension frontier lacks exact source-storage provenance",
                        );
                    }
                    value
                }
                psi_checked_trees::SuspensionCrossingValueOrigin::CallArgument { position } => {
                    let id =
                        call_arguments
                            .get(position)
                            .copied()
                            .ok_or(LoweringError::Unsupported(
                                "suspension frontier call argument position is unavailable",
                            ))?;
                    scalar_declaration(module, id).ok_or(LoweringError::Unsupported(
                        "suspension frontier call argument value is undeclared",
                    ))?
                }
                psi_checked_trees::SuspensionCrossingValueOrigin::Persistent { .. } => {
                    return unsupported(
                        "persistent suspension frontier lacks an exact Terminal PlaceId join",
                    );
                }
            };
            let source_type = checked
                .primitive_type_reference(live.type_reference)
                .ok_or(LoweringError::Unsupported(
                    "suspension frontier structural type lacks an exact Terminal type join",
                ))?;
            if terminal_scalar_type(source_type)? != value.scalar_type {
                return unsupported("suspension frontier scalar type join is inconsistent");
            }
            live_values.push(psi_terminal::TerminalSuspensionLiveValue {
                place: psi_terminal::TerminalSuspensionPlace::Scalar(value.id),
                value_type: psi_terminal::TerminalSuspensionValueType::Scalar(value.scalar_type),
                storage: match live.storage {
                    psi_checked_trees::SuspensionCrossingStorage::Persistent => {
                        psi_terminal::TerminalSuspensionStorage::Persistent
                    }
                    psi_checked_trees::SuspensionCrossingStorage::Parameter => {
                        psi_terminal::TerminalSuspensionStorage::Parameter
                    }
                    psi_checked_trees::SuspensionCrossingStorage::Local => {
                        psi_terminal::TerminalSuspensionStorage::Local
                    }
                    psi_checked_trees::SuspensionCrossingStorage::CallArgument => {
                        psi_terminal::TerminalSuspensionStorage::CallArgument
                    }
                },
                claim_count: 0,
                claims: Vec::new(),
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
        plans.push(psi_terminal::TerminalSuspensionCallPlan {
            operation: occurrence.terminal_operation,
            crossing: psi_checked_trees::canonical_suspension_crossing_id(&checked.typed, crossing)
                .ok_or(LoweringError::Unsupported(
                    "suspension crossing identity cannot resolve its source symbols",
                ))?,
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
        .map(|plan| psi_terminal::TerminalSuspensionCallSite {
            operation: plan.operation,
            crossing: plan.crossing,
            target: plan.target,
            frontier_commitment: psi_terminal::suspension_frontier_commitment(plan),
        })
        .collect();
    module.suspension_call_plans = plans;
    Ok(())
}

fn source_origin_is_exact(
    checked: &CheckedTrees,
    state_symbol: psi_symbols::SymbolHandle,
    origin: psi_checked_trees::SuspensionCrossingValueOrigin,
) -> bool {
    let Some(state) = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .find(|state| state.symbol == state_symbol)
    else {
        return false;
    };
    match origin {
        psi_checked_trees::SuspensionCrossingValueOrigin::Persistent { symbol } => checked
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.is_self && parameter.symbol == symbol),
        psi_checked_trees::SuspensionCrossingValueOrigin::Parameter { symbol, position } => checked
            .state_parameters(state)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .nth(position)
            .is_some_and(|parameter| parameter.symbol == symbol),
        psi_checked_trees::SuspensionCrossingValueOrigin::Local {
            symbol,
            statement_index,
            environment_position,
        } => {
            let statements = checked.statement_table.statements(state.statement_nodes);
            let Some(psi_checked_trees::statement::StatementNode::LocalData(local)) =
                statements.get(statement_index)
            else {
                return false;
            };
            let preceding_locals = statements[..statement_index]
                .iter()
                .filter(|statement| {
                    matches!(
                        statement,
                        psi_checked_trees::statement::StatementNode::LocalData(_)
                    )
                })
                .count();
            let parameter_count = checked
                .state_parameters(state)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count();
            local.symbol == symbol
                && environment_position == parameter_count.saturating_add(preceding_locals)
        }
        psi_checked_trees::SuspensionCrossingValueOrigin::CallArgument { .. } => true,
    }
}

fn terminal_call_target(
    kind: &OperationKind,
) -> Option<psi_terminal::TerminalSuspensionCallTarget> {
    match kind {
        OperationKind::Call { callee, .. }
        | OperationKind::CallUnit { callee, .. }
        | OperationKind::CallStructuralScalar { callee, .. }
        | OperationKind::CallStructural { callee, .. }
        | OperationKind::CallStructuralWithScalarArguments { callee, .. } => {
            Some(psi_terminal::TerminalSuspensionCallTarget::Machine(*callee))
        }
        OperationKind::BoundaryCall { boundary, .. } => Some(
            psi_terminal::TerminalSuspensionCallTarget::Boundary(*boundary),
        ),
        OperationKind::CallDynamicScalar {
            descriptor_ordinal, ..
        }
        | OperationKind::CallDynamicUnit {
            descriptor_ordinal, ..
        } => Some(
            psi_terminal::TerminalSuspensionCallTarget::DynamicDescriptor {
                ordinal: *descriptor_ordinal,
            },
        ),
        OperationKind::CallDynamicParameterScalar {
            parameter_ordinal,
            requirement_slot,
            ..
        }
        | OperationKind::CallDynamicParameterUnit {
            parameter_ordinal,
            requirement_slot,
            ..
        } => Some(
            psi_terminal::TerminalSuspensionCallTarget::DynamicParameter {
                parameter_ordinal: *parameter_ordinal,
                requirement_slot: *requirement_slot,
            },
        ),
        _ => None,
    }
}

fn scalar_declaration(module: &TerminalModule, value: ValueId) -> Option<ValueDeclaration> {
    module.machines.iter().find_map(|machine| {
        machine
            .parameters
            .iter()
            .chain(
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| block.parameters.iter()),
            )
            .chain(machine.blocks.iter().flat_map(|block| {
                block
                    .operations
                    .iter()
                    .filter_map(|operation| operation.result.scalar_ref())
            }))
            .find(|declaration| declaration.id == value)
            .copied()
    })
}

fn scalar_operation_result_before(
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
                    block.operations[..operation_index]
                        .iter()
                        .filter_map(|candidate| candidate.result.scalar_ref())
                        .any(|result| result.id == value)
                })
        })
    })
}
