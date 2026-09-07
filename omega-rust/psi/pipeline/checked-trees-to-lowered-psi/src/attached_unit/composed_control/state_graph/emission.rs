//! Emit every authored state using shared catalogs and simultaneous scalar edges.

use super::*;

pub(in crate::attached_unit::composed_control) fn emit(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    admitted: AdmittedGraph<'_>,
    terminal_machine: MachineId,
    parameters: Vec<StructuralParameterDeclaration>,
    scalar_parameters: Vec<ValueDeclaration>,
    catalogs: &mut catalogs::ComposedCatalogs,
) -> Result<(TerminalMachine, Vec<LoweredSourceCallOccurrence>), LoweringError> {
    let mut state_ids = Vec::new();
    let mut state_values = vec![scalar_parameters.clone()];
    for (position, state) in plan.states.iter().enumerate() {
        state_ids.push(block_id(allocate_dense(&mut catalogs.next_block)?));
        if position != 0 {
            state_values.push(
                state
                    .scalar_parameters
                    .iter()
                    .map(|parameter| {
                        Ok(ValueDeclaration {
                            id: value_id(allocate_dense(&mut catalogs.next_value)?),
                            scalar_type: terminal_scalar_type(parameter.primitive_type)?,
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?,
            );
        }
    }
    let mut blocks = Vec::new();
    let mut occurrences = Vec::new();
    for (position, state) in plan.states.iter().enumerate() {
        let state_parameters = admitted.view_roots[position]
            .iter()
            .zip(&state.structural_parameters)
            .map(|(root, source)| {
                let mut parameter = parameters[*root].clone();
                parameter.position = source.position;
                parameter
            })
            .collect::<Vec<_>>();
        let mut operations = OperationBuffer::new(catalogs.next_operation - 1);
        let mut evaluation = crate::attached_unit::argument_evaluation::Evaluation {
            structural_parameters: state
                .structural_parameters
                .iter()
                .zip(&state_parameters)
                .map(|(source, parameter)| (source.position, parameter.clone()))
                .collect(),
            entry: state_ids[position],
            current: state_ids[position],
            parameters: if position == 0 {
                Vec::new()
            } else {
                state_values[position].clone()
            },
            operation_start: 0,
            blocks: Vec::new(),
        };
        let mut values = state_values[position].clone();
        let mut next_value = catalogs.next_value;
        let mut next_block = catalogs.next_block;
        let mut next_edge = catalogs.next_edge;
        super::super::emission::emit_call_operations(
            checked,
            plan.machine,
            state,
            catalogs,
            &state_parameters,
            &[],
            &mut evaluation,
            &mut values,
            &mut next_value,
            &mut next_block,
            &mut next_edge,
            &mut operations,
        )?;
        let mut successor =
            |edge: &CheckedStructuralControlSuccessorPlan| -> Result<SuccessorEdge, LoweringError> {
                let target = plan
                    .states
                    .iter()
                    .position(|state| state.state == edge.target_state)
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph target disappeared during emission",
                    ))?;
                Ok(SuccessorEdge {
                    edge: edge_id(allocate_dense(&mut next_edge)?),
                    target: state_ids[target],
                    arguments: edge
                        .scalar_arguments
                        .iter()
                        .map(|transfer| {
                            values
                                .get(transfer.source_scalar_parameter_index as usize)
                                .map(|value| value.id)
                                .ok_or(LoweringError::Unsupported(
                                    "Unit graph scalar successor value missing",
                                ))
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                    trivial_affine_discards: Vec::new(),
                })
            };
        let terminator = match &state.terminator {
            CheckedComposedUnitControlTerminatorPlan::ReturnUnit => Terminator::ReturnUnit {
                edge: edge_id(allocate_dense(&mut next_edge)?),
                trivial_affine_discards: Vec::new(),
            },
            CheckedComposedUnitControlTerminatorPlan::Jump { successor: edge } => {
                let edge = successor(edge)?;
                Terminator::Jump {
                    edge: edge.edge,
                    target: edge.target,
                    arguments: edge.arguments,
                    trivial_affine_discards: Vec::new(),
                    residual_affine_discards: Vec::new(),
                }
            }
            CheckedComposedUnitControlTerminatorPlan::Conditional {
                guard,
                when_true,
                when_false,
            } => {
                let expression = lower_checked_scalar_expression(guard)?;
                let condition =
                    emit_direct_expression(&expression, &values, &mut next_value, &mut operations);
                Terminator::Conditional {
                    condition,
                    when_true: successor(when_true)?,
                    when_false: successor(when_false)?,
                }
            }
            _ => return unsupported("Unit graph terminator escaped admission"),
        };
        evaluation.blocks.push(Block {
            id: evaluation.current,
            parameters: evaluation.parameters,
            operations: operations[evaluation.operation_start..].to_vec(),
            terminator,
        });
        blocks.extend(evaluation.blocks);
        occurrences.extend(operations.source_calls);
        catalogs.next_value = next_value;
        catalogs.next_block = next_block;
        catalogs.next_edge = next_edge;
        catalogs.next_operation = operations.next_identity;
    }
    blocks.sort_by_key(|block| block.id);
    let attachment = plan
        .attachment_type_identity
        .as_ref()
        .map(|identity| lookup_type_id(&catalogs.type_ids, identity))
        .transpose()?;
    let mut machine = TerminalMachine {
        id: terminal_machine,
        attachment,
        parameters: scalar_parameters,
        structural_places: parameters
            .iter()
            .map(|parameter| StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::Parameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                },
            })
            .collect(),
        structural_parameters: parameters,
        entry_claims: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        published_service_ceiling: lower_installation_machine_service_ceiling(
            checked,
            plan.machine,
            plan.contract_service_reach,
            plan.service_reach,
            &catalogs.service_ids,
        )?,
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: state_ids[0],
        blocks,
        contract: MachineContract {
            id: contract_id(terminal_machine.get()),
            requires: Vec::new(),
            ensures: Vec::new(),
            crash_routes: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    machine.contract.crash_routes =
        lower_checked_crash_route_buckets(&catalogs.root_crash_routes, &machine.parameters)?;
    Ok((machine, occurrences))
}
