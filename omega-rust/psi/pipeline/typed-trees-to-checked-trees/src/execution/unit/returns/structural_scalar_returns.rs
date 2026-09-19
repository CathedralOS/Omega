//! Structural and trait-operator scalar return machines.

use crate::execution::terminal_unit::cleanup::{
    checked_requires_expressions, machine_has_content_evidence,
    nominal_cleanup_boolean_requirements, nominal_cleanup_missing_requirement,
    nominal_scalar_caller_requirements, scalar_nominal_cleanup_missing_requirement_diagnostic,
    service_reach_is_empty, service_reach_plan_is_empty,
};
use crate::execution::terminal_unit::primitive_effects;
use crate::execution::terminal_unit::returns::scalar_return_expressions::{
    checked_boolean_contains_short_circuit, checked_boolean_local_reference_count,
    is_branch_free_structural_boolean_expression, is_branch_free_structural_scalar_expression,
    is_structural_boolean_return_expression, is_structural_scalar_return_expression,
    is_structural_short_circuit_boolean_return,
};
use crate::execution::terminal_unit::{
    BTreeSet, CheckFacts, CheckedScalarBinding, CheckedScalarBindingValue, CheckedScalarExpression,
    CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedStructuralScalarReturnCleanupAction, CheckedStructuralScalarReturnMachinePlan,
    CheckedTraitOperatorScalarReturnMachinePlan, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, CheckedUnitEffectPlans, CheckedUnitNominalAffineCleanupPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralTypeShape, Diagnostic, ExpressionNode,
    MachineSupplyMode, Multiplicity, PermissionAccess, PermissionEventKind, PermissionEventSource,
    PrimitiveType, ShapeCollector, StatementNode, SymbolHandle, TypeReferenceNode, TypedTrees,
    checked_shared_boolean_convergence, free_structural_scalar_signature, is_reference,
    machine_binders, parameter_qualifications, projected_parameter_qualifications,
    shared_convergence, state_flow, structural_scalar_signature, type_graph_requires_nominal_drop,
};

pub(crate) fn build_trait_operator_scalar_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Option<CheckedTraitOperatorScalarReturnMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    if !program.machine_contracts(machine).is_empty()
        || !program.state_contracts(state).is_empty()
        || machine_has_content_evidence(facts, machine.symbol, state.symbol)
    {
        return None;
    }
    let [StatementNode::Expression(expression)] =
        program.statement_table.statements(state.statement_nodes)
    else {
        return None;
    };
    let expression = *expression;
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    let candidate = facts
        .operators
        .selected_trait_candidate_in_machine(expression, machine.symbol)?;
    let specialization = program
        .machine_specializations
        .iter()
        .find(|specialization| specialization.instance == machine.symbol)?;
    let application = specialization
        .conformance_applications
        .iter()
        .find(|application| {
            application.declaration == candidate.conformance_symbol
                && application.report_fingerprint
                    == candidate.conformance_application_report_fingerprint
                && application.commitment == candidate.conformance_application_commitment
        })?;
    if application.report_fingerprint == 0
        || !application.rows.iter().any(|row| {
            row.requirement == candidate.trait_requirement_symbol
                && row.realization_machine == candidate.realization_machine_symbol
                && row.realization_state == candidate.realization_state_symbol
        })
    {
        return None;
    }
    let realization_machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == candidate.realization_machine_symbol)?;
    let [realization_state] = program.machine_states(realization_machine) else {
        return None;
    };
    if realization_state.symbol != candidate.realization_state_symbol {
        return None;
    }
    let [StatementNode::Expression(realization_expression)] = program
        .statement_table
        .statements(realization_state.statement_nodes)
    else {
        return None;
    };
    let realization_return_expression = CheckedScalarExpression::Boolean(Box::new(
        crate::values::lower_machine_parameter_boolean_expression(
            program,
            &facts.operators,
            realization_machine,
            *realization_expression,
            &[],
        )?,
    ));

    let binders = machine_binders(program, machine);
    let attachment_type_identity = machine
        .attached_data
        .as_ref()
        .and_then(|attached_name| {
            program
                .data_definitions()
                .iter()
                .find(|data| data.name == *attached_name)
        })
        .and_then(|attached| shapes.add_attached_data(attached, &binders));
    if machine.attached_data.is_some() != attachment_type_identity.is_some() {
        return None;
    }
    let source_parameters = program.state_parameters(state);
    let structural_parameters = source_parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| {
            if parameter.is_const
                || parameter.is_mutable
                || is_reference(program, parameter.type_reference)
                || program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
            {
                return None;
            }
            let type_identity = if parameter.is_self {
                attachment_type_identity.clone()?
            } else {
                shapes.add_type(parameter.type_reference, &binders, &[])?
            };
            let multiplicity = crate::checks::type_multiplicity(program, parameter.type_reference);
            let qualifications =
                parameter_qualifications(program, shapes, parameter.type_reference, &binders)?;
            if multiplicity != Multiplicity::Affine || !qualifications.is_empty() {
                return None;
            }
            Some(CheckedUnitStructuralParameterPlan {
                position: u32::try_from(position).ok()?,
                is_self: parameter.is_self,
                type_identity,
                multiplicity,
                access: CheckedStructuralAccess::Owned,
                qualifications,
                projected_qualifications: projected_parameter_qualifications(
                    program,
                    shapes,
                    parameter.type_reference,
                    &binders,
                )?,
                fused_service_erasure: None,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    if structural_parameters.is_empty() {
        return None;
    }
    let argument_source_positions = [binary.left, binary.right]
        .iter()
        .map(|operand| {
            let ExpressionNode::Name(path) = program.expression_table.expression(*operand) else {
                return None;
            };
            if program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
            {
                return None;
            }
            source_parameters
                .iter()
                .position(|parameter| parameter.symbol == path.symbol)
                .and_then(|position| u32::try_from(position).ok())
        })
        .collect::<Option<Vec<_>>>()?;
    if argument_source_positions.len() != structural_parameters.len()
        || argument_source_positions
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != structural_parameters.len()
    {
        return None;
    }
    Some(CheckedTraitOperatorScalarReturnMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        structural_parameters,
        result_type: program.primitive_type_reference(state.return_type)?,
        return_statement_ordinal: 0,
        conformance: candidate.conformance_symbol,
        conformance_application_report_fingerprint: candidate
            .conformance_application_report_fingerprint,
        conformance_application_commitment: candidate.conformance_application_commitment,
        requirement: candidate.trait_requirement_symbol,
        realization_machine: candidate.realization_machine_symbol,
        realization_state: candidate.realization_state_symbol,
        realization_return_expression,
        argument_source_positions,
    })
}

pub(crate) fn build_structural_scalar_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    unit_effects: Option<&CheckedUnitEffectPlans>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedStructuralScalarReturnMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    if facts.flow.ownership.permissions.iter().any(|(_, event)| {
        event.machine_symbol == machine.symbol
            && event.state_symbol == state.symbol
            && event.source == PermissionEventSource::StateEntry
            && event.kind == PermissionEventKind::Establish
            && event.access == PermissionAccess::Owned
    }) {
        return None;
    }
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    if !facts
        .service_reaches
        .rows
        .services(flow.service_reach.direct)
        .is_empty()
        || !facts
            .service_reaches
            .rows
            .services(flow.service_reach.transitive)
            .is_empty()
    {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let has_primitive_effect = matches!(
        statements,
        [StatementNode::Assignment(_), StatementNode::Expression(_)]
    );
    let primitive_reference_body = has_primitive_effect
        || (matches!(statements, [StatementNode::Expression(_)])
            && primitive_effects::has_plain_primitive_borrows(program, state));
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters, scalar_parameters) =
        if machine.attached_data.is_none() && primitive_reference_body {
            let (structural, scalar) =
                free_structural_scalar_signature(program, shapes, state, &binders)?;
            (None, structural, scalar)
        } else {
            let (attachment, structural, scalar) =
                structural_scalar_signature(program, shapes, machine, state, &binders, false)?;
            (Some(attachment), structural, scalar)
        };
    let effects = if primitive_reference_body {
        primitive_effects::build(
            program,
            facts,
            shapes,
            machine,
            state,
            &structural_parameters,
            &scalar_parameters,
        )?
    } else {
        Vec::new()
    };
    let source_state_parameters = program.state_parameters(state);
    let authored_parameter_positions = structural_parameters
        .iter()
        .map(|parameter| parameter.position)
        .chain(
            scalar_parameters
                .iter()
                .map(|parameter| parameter.source_position),
        )
        .collect::<BTreeSet<_>>();
    if structural_parameters.is_empty()
        || structural_parameters.len() + scalar_parameters.len()
            != crate::execution::terminal_unit::abi_parameter_count(source_state_parameters)
        || authored_parameter_positions.len()
            != crate::execution::terminal_unit::abi_parameter_count(source_state_parameters)
        || authored_parameter_positions
            .iter()
            .copied()
            .enumerate()
            .any(|(position, authored)| u32::try_from(position).ok() != Some(authored))
        || scalar_parameters
            .windows(2)
            .any(|pair| pair[0].source_position >= pair[1].source_position)
        || structural_parameters.iter().any(|parameter| {
            parameter.is_self
                || (!primitive_reference_body && parameter.multiplicity != Multiplicity::Affine)
                || !parameter.qualifications.is_empty()
        })
    {
        return None;
    }
    let binding_count = statements
        .iter()
        .take_while(|statement| matches!(statement, StatementNode::LocalData(_)))
        .count();
    let bindings = statements[..binding_count]
        .iter()
        .enumerate()
        .map(|(statement_index, statement)| {
            let StatementNode::LocalData(local) = statement else {
                unreachable!("binding prefix contains only local data")
            };
            if local.is_mutable || !local.initial_value.is_valid() {
                return None;
            }
            let statement_ordinal = u32::try_from(statement_index).ok()?;
            let binding_ordinal = statement_ordinal;
            let primitive_type = program.primitive_type_reference(local.type_reference)?;
            let expression = facts.values.scalar_expressions.expression_at(
                state.symbol,
                statement_ordinal,
                CheckedScalarExpressionRole::LocalInitializer { binding_ordinal },
            )?;
            let branch_free = is_branch_free_structural_scalar_expression(
                expression,
                scalar_parameters.len(),
                statement_index,
            );
            let short_circuit_boolean = primitive_type == PrimitiveType::Bool
                && matches!(expression, CheckedScalarExpression::Boolean(expression)
                if checked_boolean_contains_short_circuit(expression)
                    && is_structural_boolean_return_expression(
                        expression,
                        scalar_parameters.len(),
                        statement_index,
                    ));
            (branch_free || short_circuit_boolean).then_some((
                CheckedScalarBinding {
                    destination: checked_trees::CheckedScalarBindingDestination::Immutable,
                    statement_ordinal,
                    primitive_type,
                    value: CheckedScalarBindingValue::Expression,
                },
                branch_free,
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    let bindings_are_branch_free = bindings.iter().all(|(_, branch_free)| *branch_free);
    let binding_branch_free = bindings
        .iter()
        .map(|(_, branch_free)| *branch_free)
        .collect::<Vec<_>>();
    let bindings = bindings
        .into_iter()
        .map(|(binding, _)| binding)
        .collect::<Vec<_>>();
    let return_position = binding_count.checked_add(effects.len())?;
    let [StatementNode::Expression(_)] = &statements[return_position..] else {
        return None;
    };
    let return_statement_ordinal = u32::try_from(return_position).ok()?;
    let result_type = program.primitive_type_reference(state.return_type)?;
    let return_expression = facts.values.scalar_expressions.expression_at(
        state.symbol,
        return_statement_ordinal,
        CheckedScalarExpressionRole::Return,
    )?;
    let return_is_branch_free = is_branch_free_structural_scalar_expression(
        return_expression,
        scalar_parameters.len(),
        binding_count,
    );
    let return_is_short_circuit_boolean = is_structural_short_circuit_boolean_return(
        return_expression,
        scalar_parameters.len(),
        binding_count,
    );
    let final_binding_is_source_distributed_short_circuit_return = binding_count > 0
        && binding_branch_free[..binding_count - 1]
            .iter()
            .all(|branch_free| *branch_free)
        && !binding_branch_free[binding_count - 1]
        && bindings[binding_count - 1].primitive_type == PrimitiveType::Bool
        && facts
            .values
            .scalar_expressions
            .expression_at(
                state.symbol,
                u32::try_from(binding_count - 1).ok()?,
                CheckedScalarExpressionRole::LocalInitializer {
                    binding_ordinal: u32::try_from(binding_count - 1).ok()?,
                },
            )
            .is_some_and(|expression| {
                is_structural_short_circuit_boolean_return(
                    expression,
                    scalar_parameters.len(),
                    binding_count - 1,
                )
            })
        && matches!(
            return_expression,
            CheckedScalarExpression::Boolean(expression)
                if is_branch_free_structural_boolean_expression(
                    expression,
                    scalar_parameters.len(),
                    binding_count,
                ) && checked_boolean_local_reference_count(
                    expression,
                    scalar_parameters.len() + binding_count - 1,
                ) > 0
        );
    let final_short_circuit_continuation_chain_is_source_distributed = binding_count >= 2
        && binding_branch_free
            .iter()
            .position(|branch_free| !*branch_free)
            .is_some_and(|short_circuit_index| {
                if short_circuit_index + 1 >= binding_count
                    || !binding_branch_free[..short_circuit_index]
                        .iter()
                        .all(|branch_free| *branch_free)
                    || !bindings[short_circuit_index..]
                        .iter()
                        .all(|binding| binding.primitive_type == PrimitiveType::Bool)
                {
                    return false;
                }
                let Ok(short_circuit_ordinal) = u32::try_from(short_circuit_index) else {
                    return false;
                };
                let short_circuit_is_supported = facts
                    .values
                    .scalar_expressions
                    .expression_at(
                        state.symbol,
                        short_circuit_ordinal,
                        CheckedScalarExpressionRole::LocalInitializer {
                            binding_ordinal: short_circuit_ordinal,
                        },
                    )
                    .is_some_and(|expression| {
                        is_structural_short_circuit_boolean_return(
                            expression,
                            scalar_parameters.len(),
                            short_circuit_index,
                        )
                    });
                short_circuit_is_supported
                    && (short_circuit_index + 1..binding_count).all(|continuation_index| {
                        let Ok(binding_ordinal) = u32::try_from(continuation_index) else {
                            return false;
                        };
                        facts
                            .values
                            .scalar_expressions
                            .expression_at(
                                state.symbol,
                                binding_ordinal,
                                CheckedScalarExpressionRole::LocalInitializer { binding_ordinal },
                            )
                            .is_some_and(|expression| {
                                matches!(
                                    expression,
                                    CheckedScalarExpression::Boolean(boolean)
                                        if (is_branch_free_structural_boolean_expression(
                                            boolean,
                                            scalar_parameters.len(),
                                            continuation_index,
                                        ) || is_structural_short_circuit_boolean_return(
                                            expression,
                                            scalar_parameters.len(),
                                            continuation_index,
                                        )) && checked_boolean_local_reference_count(
                                            boolean,
                                            scalar_parameters.len() + continuation_index - 1,
                                        ) > 0
                                )
                            })
                    })
                    && matches!(
                        return_expression,
                        CheckedScalarExpression::Boolean(expression)
                            if matches!(expression.as_ref(),
                                checked_trees::CheckedBooleanExpression::Local { position }
                                    if *position
                                        == scalar_parameters.len() + binding_count - 1)
                    )
            });
    if !is_structural_scalar_return_expression(
        return_expression,
        scalar_parameters.len(),
        binding_count,
    ) {
        return None;
    }
    let whole_discards =
        crate::execution::terminal_cleanup::checked_whole_affine_discard_parameters(
            program,
            facts,
            machine.symbol,
            state,
        )?;
    if primitive_reference_body && !whole_discards.is_empty() {
        return None;
    }
    let has_nominal_cleanup = whole_discards.iter().any(|(_, position)| {
        source_state_parameters
            .get(*position as usize)
            .is_some_and(|parameter| {
                type_graph_requires_nominal_drop(program, parameter.type_reference)
            })
    });
    if has_nominal_cleanup
        && (structural_parameters.len() != whole_discards.len()
            || !(bindings_are_branch_free
                && (return_is_branch_free || return_is_short_circuit_boolean)
                || final_binding_is_source_distributed_short_circuit_return
                || final_short_circuit_continuation_chain_is_source_distributed))
    {
        return None;
    }
    let (caller_requirements, scalar_requirements) = if has_nominal_cleanup {
        nominal_scalar_caller_requirements(
            program,
            facts,
            machine,
            state,
            source_state_parameters,
            &scalar_parameters,
        )?
    } else {
        let checked_contracts =
            checked_requires_expressions(program, facts, machine.symbol, state.symbol)?;
        if !checked_contracts.is_empty() {
            return None;
        }
        (Vec::new(), Vec::new())
    };
    let cleanup_actions = whole_discards
        .iter()
        .map(|(_, position)| {
            let source_parameter = source_state_parameters.get(*position as usize)?;
            let checked_parameter = structural_parameters
                .iter()
                .find(|parameter| parameter.position == *position)?;
            if has_nominal_cleanup
                && (source_parameter.is_self
                    || source_parameter.is_const
                    || source_parameter.is_mutable
                    || checked_parameter.is_self
                    || checked_parameter.multiplicity != Multiplicity::Affine
                    || !checked_parameter.qualifications.is_empty())
            {
                return None;
            }
            if !type_graph_requires_nominal_drop(program, source_parameter.type_reference) {
                return Some(CheckedStructuralScalarReturnCleanupAction::DiscardRoot(
                    *position,
                ));
            }
            let nominal_cleanup = (|| {
                let TypeReferenceNode::Named {
                    symbol: parameter_data_symbol,
                    ..
                } = program
                    .type_reference_table
                    .type_reference(source_parameter.type_reference)
                else {
                    return None;
                };
                let parameter_data = program
                    .data_definitions()
                    .iter()
                    .find(|data| data.symbol == *parameter_data_symbol)?;
                let cleanup_machines = program
                    .machines()
                    .iter()
                    .filter(|candidate| {
                        candidate.supply_mode == MachineSupplyMode::CheckedBody
                            && candidate.name.as_str().ends_with("::drop")
                            && candidate
                                .attached_data
                                .as_ref()
                                .is_some_and(|attached| attached == &parameter_data.name)
                    })
                    .collect::<Vec<_>>();
                let [cleanup_machine] = cleanup_machines.as_slice() else {
                    return None;
                };
                let [cleanup_state] = program.machine_states(cleanup_machine) else {
                    return None;
                };
                let [cleanup_receiver] = program.state_parameters(cleanup_state) else {
                    return None;
                };
                let unit_effects = unit_effects?;
                let cleanup_target = unit_effects.for_machine(cleanup_machine.symbol)?;
                let cleanup_requirements = nominal_cleanup_boolean_requirements(
                    program,
                    facts,
                    cleanup_machine,
                    cleanup_state,
                    cleanup_receiver,
                )?;
                if let Some(missing) = nominal_cleanup_missing_requirement(
                    checked_parameter.position,
                    &caller_requirements,
                    &cleanup_requirements,
                ) {
                    diagnostics.push(scalar_nominal_cleanup_missing_requirement_diagnostic(
                        program,
                        machine,
                        state,
                        return_statement_ordinal,
                        source_parameter,
                        cleanup_machine,
                        missing,
                    ));
                    return None;
                }
                if cleanup_target.attachment_type_identity.as_deref()
                    != Some(checked_parameter.type_identity.as_str())
                    || !is_bounded_scalar_nominal_cleanup_target(
                        facts,
                        unit_effects,
                        cleanup_machine.symbol,
                        cleanup_target,
                    )
                {
                    return None;
                }
                Some(CheckedUnitNominalAffineCleanupPlan {
                    source_parameter_index: checked_parameter.position,
                    type_identity: checked_parameter.type_identity.clone(),
                    cleanup_machine: cleanup_machine.symbol,
                    cleanup_state: cleanup_target.state,
                    cleanup_contract_report_fingerprint: cleanup_target.contract_report_fingerprint,
                    requirements: cleanup_requirements,
                })
            })()?;
            Some(CheckedStructuralScalarReturnCleanupAction::InvokeNominal(
                nominal_cleanup,
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    let shared_boolean_convergence = has_nominal_cleanup
        .then(|| {
            checked_shared_boolean_convergence(
                facts,
                state.symbol,
                &bindings,
                return_expression,
                scalar_parameters.len(),
                &cleanup_actions,
            )
        })
        .flatten();
    // The source-distributed fallback cannot realize mixed member/integer
    // predicates excluded by the shared Boolean convergence plan.
    if has_nominal_cleanup
        && shared_boolean_convergence.is_none()
        && bindings
            .iter()
            .filter_map(|binding| {
                facts.values.scalar_expressions.expression_at(
                    state.symbol,
                    binding.statement_ordinal,
                    CheckedScalarExpressionRole::LocalInitializer {
                        binding_ordinal: binding.statement_ordinal,
                    },
                )
            })
            .chain(std::iter::once(return_expression))
            .any(|expression| {
                shared_convergence::shared_boolean_has_member_and_integer_inputs(
                    expression,
                    scalar_parameters.len(),
                )
            })
    {
        return None;
    }
    let erased_scalar_parameters =
        crate::execution::terminal_unit::types::erased_scalar_parameter_plans(program, state)?;
    Some(CheckedStructuralScalarReturnMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        structural_parameters,
        scalar_parameters,
        erased_scalar_parameters,
        bindings,
        effects,
        result_type,
        return_statement_ordinal,
        shared_boolean_convergence,
        caller_requirements,
        scalar_requirements,
        cleanup_actions,
    })
}

pub(crate) fn is_bounded_scalar_nominal_cleanup_target(
    facts: &CheckFacts,
    unit_effects: &CheckedUnitEffectPlans,
    cleanup_machine: SymbolHandle,
    cleanup_target: &CheckedUnitEffectMachinePlan,
) -> bool {
    if !cleanup_target.structural_parameters.is_empty()
        || !cleanup_target.trivial_affine_locals.is_empty()
        || !cleanup_target.entry_claims.is_empty()
        || !cleanup_target.body_qualifications.is_empty()
        || !service_reach_is_empty(facts, cleanup_target.service_reach)
        || !service_reach_plan_is_empty(facts, cleanup_target.contract_service_reach)
    {
        return false;
    }
    let Some((cleanup_return, cleanup_calls)) = cleanup_target.operations.split_last() else {
        return false;
    };
    let CheckedUnitEffectOperationPlan::Complete {
        statement_index,
        trivial_affine_local_discard_ordinals,
        trivial_affine_discards,
    } = cleanup_return
    else {
        return false;
    };
    if usize::try_from(*statement_index).ok() != Some(cleanup_calls.len())
        || !trivial_affine_local_discard_ordinals.is_empty()
        || !trivial_affine_discards.is_empty()
    {
        return false;
    }

    let mut helpers = Vec::with_capacity(cleanup_calls.len());
    for (statement_index, operation) in cleanup_calls.iter().enumerate() {
        let CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            service_reach,
            scalar_arguments,
            erased_scalar_arguments,
            structural_arguments,
            claim_transfers,
        } = operation
        else {
            return false;
        };
        if usize::try_from(coordinate.statement_index).ok() != Some(statement_index)
            || coordinate.call_ordinal != 0
            || *target_machine == cleanup_machine
            || helpers
                .iter()
                .any(|(helper, _, _)| helper == target_machine)
            || !service_reach_is_empty(facts, *service_reach)
            || !scalar_arguments.is_empty()
            || !erased_scalar_arguments.is_empty()
            || !structural_arguments.is_empty()
            || !claim_transfers.is_empty()
        {
            return false;
        }
        helpers.push((
            *target_machine,
            *target_state,
            *target_contract_report_fingerprint,
        ));
    }

    helpers
        .into_iter()
        .all(|(helper_machine, helper_state, helper_fingerprint)| {
            let Some(helper) = unit_effects.for_machine(helper_machine) else {
                return false;
            };
            let helper_shape = unit_effects.structural_types.iter().find(|shape| {
                helper.attachment_type_identity.as_deref() == Some(shape.identity.as_str())
            });
            helper.machine != cleanup_machine
                && helper.state == helper_state
                && helper.contract_report_fingerprint == helper_fingerprint
                && matches!(
                    helper_shape.map(|shape| &shape.shape),
                    Some(CheckedUnitStructuralTypeShape::Record { fields }) if fields.is_empty()
                )
                && helper.structural_parameters.is_empty()
                && helper.trivial_affine_locals.is_empty()
                && helper.entry_claims.is_empty()
                && helper.body_qualifications.is_empty()
                && service_reach_is_empty(facts, helper.service_reach)
                && service_reach_plan_is_empty(facts, helper.contract_service_reach)
                && matches!(
                    helper.operations.as_slice(),
                    [CheckedUnitEffectOperationPlan::Complete {
                        statement_index: 0,
                        trivial_affine_local_discard_ordinals,
                        trivial_affine_discards,
                    }] if trivial_affine_local_discard_ordinals.is_empty()
                        && trivial_affine_discards.is_empty()
                )
        })
}
