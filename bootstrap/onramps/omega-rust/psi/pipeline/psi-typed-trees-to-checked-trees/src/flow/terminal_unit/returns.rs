//! Structural Unit and scalar return analysis.

use super::*;

/// Build the exact checked carrier for `T in D -> T in D` whole-root
/// passthrough. Every wider ownership or control shape is omitted atomically.
pub(crate) fn build_checked_structural_return_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> CheckedStructuralReturnPlans {
    let mut shapes = ShapeCollector::new(program);
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| build_structural_return_machine(program, facts, &mut shapes, machine))
        .collect::<Vec<_>>();
    let retained = machines
        .iter()
        .flat_map(|plan| {
            std::iter::once(plan.attachment_type_identity.as_str())
                .chain(
                    plan.structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity.as_str()),
                )
                .chain(
                    plan.trivial_affine_locals
                        .iter()
                        .map(|local| local.type_identity.as_str()),
                )
                .chain(std::iter::once(plan.result.type_identity.as_str()))
        })
        .collect::<BTreeSet<_>>();
    let retained_domains = machines
        .iter()
        .flat_map(|plan| {
            plan.structural_parameters
                .iter()
                .flat_map(|parameter| &parameter.qualifications)
                .chain(&plan.result.qualifications)
                .map(|domain| domain.0)
        })
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    shapes
        .domains
        .retain(|domain| retained_domains.contains(&domain.domain.0));
    shapes.domains.sort_by_key(|domain| domain.domain.0);
    CheckedStructuralReturnPlans {
        structural_types: shapes.types.into_values().collect(),
        structural_domains: shapes.domains,
        machines,
    }
}

pub(super) fn build_structural_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<CheckedStructuralReturnMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let statements = program.statement_table.statements(state.statement_nodes);
    let (return_statement, local_statements) = statements.split_last()?;
    let StatementNode::Expression(return_expression) = return_statement else {
        return None;
    };
    if !local_statements
        .iter()
        .all(|statement| matches!(statement, StatementNode::LocalData(_)))
    {
        return None;
    }
    let return_expression = *return_expression;
    if !program.machine_contracts(machine).is_empty() {
        return None;
    }
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        structural_signature(program, shapes, machine, state, &binders)?;
    let trivial_affine_locals = local_statements
        .iter()
        .enumerate()
        .map(|(declaration_ordinal, statement)| {
            let StatementNode::LocalData(local) = statement else {
                unreachable!("the local prefix contains only local declarations")
            };
            let TypeReferenceNode::Named { .. } = program
                .type_reference_table
                .type_reference(local.type_reference)
            else {
                return None;
            };
            if local.is_mutable
                || !local.initial_value.is_valid()
                || crate::checks::type_multiplicity(program, local.type_reference)
                    != Multiplicity::Affine
                || !parameter_qualifications(program, shapes, local.type_reference, &binders)?
                    .is_empty()
                || type_graph_requires_nominal_drop(program, local.type_reference)
            {
                return None;
            }
            let ExpressionNode::StructLiteral(literal) =
                program.expression_table.expression(local.initial_value)
            else {
                return None;
            };
            if literal.case_name.is_some()
                || !program
                    .expression_table
                    .struct_fields(literal.fields)
                    .is_empty()
            {
                return None;
            }
            let local_events = facts
                .flow
                .ownership
                .permissions
                .iter()
                .filter(|(_, event)| {
                    event.machine_symbol == machine.symbol
                        && event.state_symbol == state.symbol
                        && event.root == psi_facts::PlaceRoot::Symbol(local.symbol)
                })
                .map(|(_, event)| event)
                .collect::<Vec<_>>();
            let [event] = local_events.as_slice() else {
                return None;
            };
            if event.source != PermissionEventSource::StateExit
                || event.kind != PermissionEventKind::AffineDrop
                || event.multiplicity != Multiplicity::Affine
                || event.access != PermissionAccess::Owned
                || event.claim_identity != PermissionClaimIdentity::Unknown
                || event.provenance != psi_language_semantics::PermissionProvenance::Unknown
                || event.obligation_live
                || !facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(event.segments)
                    .is_empty()
            {
                return None;
            }
            let type_identity = shapes.add_type(local.type_reference, &binders, &[])?;
            let shape = shapes.types.get(&type_identity)?;
            if !matches!(
                &shape.shape,
                CheckedUnitStructuralTypeShape::Record { fields } if fields.is_empty()
            ) {
                return None;
            }
            Some(CheckedTrivialAffineStructuralLocalPlan {
                declaration_ordinal: u32::try_from(declaration_ordinal).ok()?,
                type_identity,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let input = structural_parameters.first()?;
    if input.multiplicity != Multiplicity::Linear
        || input.is_self
        || structural_parameters
            .iter()
            .skip(1)
            .any(|discarded| discarded.multiplicity != Multiplicity::Affine || discarded.is_self)
    {
        return None;
    }
    let source_parameters = program.state_parameters(state);
    let source_parameter = source_parameters.get(input.position as usize)?;
    let ExpressionNode::Name(path) = program.expression_table.expression(return_expression) else {
        return None;
    };
    if path.symbol != source_parameter.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    let result_type_identity = shapes.add_type(state.return_type, &binders, &[])?;
    let result_qualifications =
        parameter_qualifications(program, shapes, state.return_type, &binders)?;
    if result_type_identity != input.type_identity
        || result_qualifications != input.qualifications
        || crate::checks::type_multiplicity(program, state.return_type) != Multiplicity::Linear
        || !state_contracts_are_exact_parameter_qualifications(
            program,
            state,
            source_parameter,
            &input.qualifications,
        )
    {
        return None;
    }
    let checked_entry_claims = entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        source_parameters,
    )?;
    let [entry_claim] = checked_entry_claims.as_slice() else {
        return None;
    };
    if entry_claim.parameter_index != 0
        || !entry_claim.path.is_empty()
        || entry_claim.carry != CarryPolicy::STRICT
    {
        return None;
    }
    let trivial_affine_discards = return_unit_affine_discards(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        source_parameters,
        &[],
        &trivial_affine_locals
            .iter()
            .filter_map(|plan| {
                local_statements
                    .get(plan.declaration_ordinal as usize)
                    .and_then(|statement| match statement {
                        StatementNode::LocalData(local) => Some(local.symbol),
                        _ => None,
                    })
            })
            .collect::<Vec<_>>(),
    );
    let expected_discards = (1..structural_parameters.len())
        .rev()
        .map(|position| u32::try_from(position).ok())
        .collect::<Option<Vec<_>>>()?;
    if trivial_affine_discards.as_deref() != Some(expected_discards.as_slice()) {
        return None;
    }
    let outcome_maps = facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .filter(|(_, map)| map.machine_symbol == machine.symbol && map.state_symbol == state.symbol)
        .map(|(_, map)| map)
        .collect::<Vec<_>>();
    let [outcome_map] = outcome_maps.as_slice() else {
        return None;
    };
    let [outcome] = facts
        .flow
        .ownership
        .claim_outcome_entries
        .span_or_empty(outcome_map.entries)
    else {
        return None;
    };
    let psi_checked_trees::FlowClaimOutcomeSource::Input {
        parameter_symbol,
        segments: input_segments,
    } = outcome.source
    else {
        return None;
    };
    if parameter_symbol != source_parameter.symbol
        || !facts
            .flow
            .ownership
            .segments
            .span_or_empty(input_segments)
            .is_empty()
        || !facts
            .flow
            .ownership
            .segments
            .span_or_empty(outcome.output_segments)
            .is_empty()
    {
        return None;
    }
    let reshuffles = facts
        .qualifications
        .content
        .identity_reshuffles
        .iter()
        .filter(|fact| fact.machine_symbol == machine.symbol && fact.state_symbol == state.symbol)
        .collect::<Vec<_>>();
    let [reshuffle] = reshuffles.as_slice() else {
        return None;
    };
    if reshuffle.claim_identity != entry_claim.claim_identity
        || reshuffle.input_parameter_symbol != source_parameter.symbol
        || !facts
            .flow
            .ownership
            .segments
            .span_or_empty(reshuffle.input_segments)
            .is_empty()
        || !facts
            .flow
            .ownership
            .segments
            .span_or_empty(reshuffle.output_segments)
            .is_empty()
    {
        return None;
    }
    Some(CheckedStructuralReturnMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        structural_parameters,
        returned_parameter_index: 0,
        result: CheckedStructuralResultPlan {
            type_identity: result_type_identity,
            multiplicity: Multiplicity::Linear,
            qualifications: result_qualifications,
        },
        trivial_affine_local_discard_ordinals: trivial_affine_locals
            .iter()
            .rev()
            .map(|local| local.declaration_ordinal)
            .collect(),
        trivial_affine_locals,
        entry_claim: entry_claim.clone(),
        trivial_affine_discards: expected_discards,
        transferred_claim: entry_claim.claim_identity,
    })
}

pub(super) fn state_contracts_are_exact_parameter_qualifications(
    program: &TypedTrees,
    state: &psi_typed_trees::state::State,
    parameter: &StateParameter,
    expected_domains: &[SemanticDomainId],
) -> bool {
    let mut actual_domains = Vec::new();
    for contract in program.state_contracts(state) {
        if contract.token_count != 0 || contract.kind != SignatureContractKind::Requires {
            return false;
        }
        let [ProofFact::Membership(membership)] = program.proof_facts.span_or_empty(contract.facts)
        else {
            return false;
        };
        let ExpressionNode::Name(path) = program.expression_table.expression(membership.value)
        else {
            return false;
        };
        if path.symbol != parameter.symbol
            || program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
        {
            return false;
        }
        let Some(domain) = program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == membership.domain_symbol)
        else {
            return false;
        };
        actual_domains.push(domain.semantic_id);
    }
    actual_domains.sort_by_key(|domain| domain.0);
    actual_domains == expected_domains
}

/// Compose the exact cleanup rows with source-independent structural
/// signatures and whole-parameter transfer maps for the first terminal
/// structural-control producer.
pub(crate) fn build_checked_structural_scalar_return_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
    unit_effects: &CheckedUnitEffectPlans,
    diagnostics: &mut Vec<Diagnostic>,
) -> CheckedStructuralScalarReturnPlans {
    let mut shapes = ShapeCollector::new(program);
    let trait_operator_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_trait_operator_scalar_return_machine(program, facts, &mut shapes, machine)
        })
        .collect::<Vec<_>>();
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_structural_scalar_return_machine(
                program,
                facts,
                unit_effects,
                &mut shapes,
                machine,
                diagnostics,
            )
        })
        .collect::<Vec<_>>();
    let retained = machines
        .iter()
        .flat_map(|machine| {
            std::iter::once(machine.attachment_type_identity.as_str()).chain(
                machine
                    .structural_parameters
                    .iter()
                    .map(|parameter| parameter.type_identity.as_str()),
            )
        })
        .chain(trait_operator_machines.iter().flat_map(|machine| {
            machine
                .attachment_type_identity
                .iter()
                .map(String::as_str)
                .chain(
                    machine
                        .structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity.as_str()),
                )
        }))
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    CheckedStructuralScalarReturnPlans {
        structural_types: shapes.types.into_values().collect(),
        machines,
        trait_operator_machines,
    }
}

fn build_trait_operator_scalar_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
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
                && application.fingerprint == candidate.conformance_application_fingerprint
        })?;
    if application.fingerprint == 0
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
                qualifications,
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
        conformance_application_fingerprint: candidate.conformance_application_fingerprint,
        requirement: candidate.trait_requirement_symbol,
        realization_machine: candidate.realization_machine_symbol,
        realization_state: candidate.realization_state_symbol,
        realization_return_expression,
        argument_source_positions,
    })
}

pub(crate) fn build_checked_boundary_scalar_return_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> CheckedBoundaryScalarReturnPlans {
    let mut shapes = ShapeCollector::new(program);
    let mut boundary_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode.is_boundary_declaration())
        .filter_map(|machine| build_boundary_machine(program, facts, &mut shapes, machine))
        .filter(|boundary| boundary.result_type.is_some())
        .collect::<Vec<_>>();
    boundary_machines.extend(
        build_static_boundary_requirements(program, facts, &mut shapes)
            .into_iter()
            .filter(|boundary| boundary.result_type.is_some()),
    );
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_boundary_scalar_return_machine(
                program,
                facts,
                &mut shapes,
                &boundary_machines,
                machine,
            )
        })
        .collect::<Vec<_>>();
    let retained = boundary_machines
        .iter()
        .flat_map(|boundary| {
            boundary
                .attachment_type_identity
                .iter()
                .map(String::as_str)
                .chain(
                    boundary
                        .structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity.as_str()),
                )
        })
        .chain(machines.iter().flat_map(|machine| {
            std::iter::once(machine.attachment_type_identity.as_str()).chain(
                machine
                    .structural_parameters
                    .iter()
                    .map(|parameter| parameter.type_identity.as_str()),
            )
        }))
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    CheckedBoundaryScalarReturnPlans {
        structural_types: shapes.types.into_values().collect(),
        structural_domains: {
            shapes.domains.sort_by_key(|domain| domain.domain.0);
            shapes.domains
        },
        boundary_machines,
        machines,
    }
}

pub(super) fn build_boundary_scalar_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
    machine: &psi_typed_trees::machine::Machine,
) -> Option<CheckedBoundaryScalarReturnMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let result_type = program.primitive_type_reference(state.return_type)?;
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        structural_signature(program, shapes, machine, state, &binders)?;
    if !checked_state_contracts_supported(program, machine, state, &structural_parameters)
        || machine_has_content_evidence(facts, machine.symbol, state.symbol)
    {
        return None;
    }
    let entry_claims = entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        program.state_parameters(state),
    )?;
    let [
        StatementNode::LocalData(local),
        StatementNode::Expression(_),
    ] = program.statement_table.statements(state.statement_nodes)
    else {
        return None;
    };
    if local.is_mutable
        || program.primitive_type_reference(local.type_reference) != Some(result_type)
        || !matches!(
            program.expression_table.expression(local.initial_value),
            ExpressionNode::Call(_)
        )
    {
        return None;
    }
    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;
    let [call] = facts.flow.control.calls.span_or_empty(state_flow.calls) else {
        return None;
    };
    if call.statement_index != 0 || call.call_ordinal != 0 {
        return None;
    }
    let boundary_call = build_call_operation(
        program,
        facts,
        machine,
        state,
        &structural_parameters,
        &entry_claims,
        call,
        false,
        Some(result_type),
    )?;
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        target_machine,
        structural_arguments,
        completion_receipts,
        ..
    } = &boundary_call
    else {
        return None;
    };
    if structural_arguments
        .iter()
        .any(|argument| !argument.path.is_empty())
        || !boundaries.iter().any(|boundary| {
            boundary.machine == *target_machine && boundary.result_type == Some(result_type)
        })
    {
        return None;
    }
    let expected_claims = entry_claims
        .iter()
        .map(|claim| claim.claim_identity)
        .collect::<Vec<_>>();
    let received_claims = completion_receipts
        .iter()
        .map(|receipt| receipt.claim_identity)
        .collect::<Vec<_>>();
    if expected_claims != received_claims {
        return None;
    }
    let return_statement_ordinal = 1;
    let return_expression = facts.values.scalar_expressions.expression_at(
        state.symbol,
        return_statement_ordinal,
        CheckedScalarExpressionRole::Return,
    )?;
    let returns_binding = match return_expression {
        CheckedScalarExpression::Local {
            position: 0,
            primitive_type,
        } => *primitive_type == result_type,
        CheckedScalarExpression::Boolean(expression) => {
            result_type == PrimitiveType::Bool
                && matches!(
                    expression.as_ref(),
                    psi_checked_trees::CheckedBooleanExpression::Local { position: 0 }
                )
        }
        _ => false,
    };
    if !returns_binding {
        return None;
    }
    Some(CheckedBoundaryScalarReturnMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        structural_parameters,
        entry_claims,
        boundary_call,
        result_type,
        return_statement_ordinal,
        contract_service_reach: facts.service_reaches.plan_for_machine(machine.symbol)?,
        service_reach: state_flow.service_reach.clone(),
    })
}

pub(super) fn build_structural_scalar_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    unit_effects: &CheckedUnitEffectPlans,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
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
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters, scalar_parameters) =
        structural_scalar_signature(program, shapes, machine, state, &binders)?;
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
        || structural_parameters.len() + scalar_parameters.len() != source_state_parameters.len()
        || authored_parameter_positions.len() != source_state_parameters.len()
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
                || parameter.multiplicity != Multiplicity::Affine
                || !parameter.qualifications.is_empty()
        })
    {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
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
    let [StatementNode::Expression(_)] = &statements[binding_count..] else {
        return None;
    };
    let return_statement_ordinal = u32::try_from(binding_count).ok()?;
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
                                psi_checked_trees::CheckedBooleanExpression::Local { position }
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
    let whole_discards = crate::flow::terminal_cleanup::checked_whole_affine_discard_parameters(
        program,
        facts,
        machine.symbol,
        state,
    )?;
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
                if cleanup_target.attachment_type_identity != checked_parameter.type_identity
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
                    cleanup_contract_fingerprint: cleanup_target.contract_fingerprint,
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
    Some(CheckedStructuralScalarReturnMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        structural_parameters,
        scalar_parameters,
        bindings,
        result_type,
        return_statement_ordinal,
        shared_boolean_convergence,
        caller_requirements,
        scalar_requirements,
        cleanup_actions,
    })
}

pub(super) fn is_bounded_scalar_nominal_cleanup_target(
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
    let CheckedUnitEffectOperationPlan::ReturnUnit {
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
            target_contract_fingerprint,
            service_reach,
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
            || !structural_arguments.is_empty()
            || !claim_transfers.is_empty()
        {
            return false;
        }
        helpers.push((*target_machine, *target_state, *target_contract_fingerprint));
    }

    helpers
        .into_iter()
        .all(|(helper_machine, helper_state, helper_fingerprint)| {
            let Some(helper) = unit_effects.for_machine(helper_machine) else {
                return false;
            };
            let helper_shape = unit_effects
                .structural_types
                .iter()
                .find(|shape| shape.identity == helper.attachment_type_identity);
            helper.machine != cleanup_machine
                && helper.state == helper_state
                && helper.contract_fingerprint == helper_fingerprint
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
                    [CheckedUnitEffectOperationPlan::ReturnUnit {
                        statement_index: 0,
                        trivial_affine_local_discard_ordinals,
                        trivial_affine_discards,
                    }] if trivial_affine_local_discard_ordinals.is_empty()
                        && trivial_affine_discards.is_empty()
                )
        })
}

pub(super) fn checked_boolean_contains_short_circuit(
    expression: &psi_checked_trees::CheckedBooleanExpression,
) -> bool {
    match expression {
        psi_checked_trees::CheckedBooleanExpression::And { .. }
        | psi_checked_trees::CheckedBooleanExpression::Or { .. } => true,
        psi_checked_trees::CheckedBooleanExpression::Not(operand) => {
            checked_boolean_contains_short_circuit(operand)
        }
        psi_checked_trees::CheckedBooleanExpression::Equal { left, right } => {
            checked_boolean_contains_short_circuit(left)
                || checked_boolean_contains_short_circuit(right)
        }
        psi_checked_trees::CheckedBooleanExpression::Constant(_)
        | psi_checked_trees::CheckedBooleanExpression::Parameter { .. }
        | psi_checked_trees::CheckedBooleanExpression::Local { .. }
        | psi_checked_trees::CheckedBooleanExpression::StructuralParameterField { .. }
        | psi_checked_trees::CheckedBooleanExpression::IntegerComparison { .. }
        | psi_checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
        | psi_checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
        | psi_checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | psi_checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => false,
    }
}

pub(super) fn is_structural_short_circuit_boolean_return(
    expression: &CheckedScalarExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    let CheckedScalarExpression::Boolean(expression) = expression else {
        return false;
    };
    checked_boolean_contains_short_circuit(expression)
        && is_structural_boolean_return_expression(expression, scalar_parameters, available_locals)
}

pub(super) fn checked_boolean_local_reference_count(
    expression: &psi_checked_trees::CheckedBooleanExpression,
    local: usize,
) -> usize {
    match expression {
        psi_checked_trees::CheckedBooleanExpression::Local { position } => {
            usize::from(*position == local)
        }
        psi_checked_trees::CheckedBooleanExpression::Not(operand) => {
            checked_boolean_local_reference_count(operand, local)
        }
        psi_checked_trees::CheckedBooleanExpression::Equal { left, right }
        | psi_checked_trees::CheckedBooleanExpression::And { left, right }
        | psi_checked_trees::CheckedBooleanExpression::Or { left, right } => {
            checked_boolean_local_reference_count(left, local)
                .saturating_add(checked_boolean_local_reference_count(right, local))
        }
        psi_checked_trees::CheckedBooleanExpression::Constant(_)
        | psi_checked_trees::CheckedBooleanExpression::Parameter { .. }
        | psi_checked_trees::CheckedBooleanExpression::StructuralParameterField { .. }
        | psi_checked_trees::CheckedBooleanExpression::IntegerComparison { .. }
        | psi_checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
        | psi_checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
        | psi_checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | psi_checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => 0,
    }
}

pub(super) fn is_structural_scalar_return_expression(
    expression: &CheckedScalarExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        CheckedScalarExpression::Boolean(expression) => {
            is_structural_boolean_return_expression(expression, scalar_parameters, available_locals)
        }
        expression => is_branch_free_structural_integer_expression(
            expression,
            scalar_parameters,
            available_locals,
        ),
    }
}

pub(super) fn is_structural_boolean_return_expression(
    expression: &psi_checked_trees::CheckedBooleanExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        psi_checked_trees::CheckedBooleanExpression::Constant(_) => true,
        psi_checked_trees::CheckedBooleanExpression::Not(operand) => {
            is_structural_boolean_return_expression(operand, scalar_parameters, available_locals)
        }
        psi_checked_trees::CheckedBooleanExpression::Equal { left, right }
        | psi_checked_trees::CheckedBooleanExpression::And { left, right }
        | psi_checked_trees::CheckedBooleanExpression::Or { left, right } => {
            is_structural_boolean_return_expression(left, scalar_parameters, available_locals)
                && is_structural_boolean_return_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        psi_checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        psi_checked_trees::CheckedBooleanExpression::Parameter { position } => {
            *position < scalar_parameters
        }
        psi_checked_trees::CheckedBooleanExpression::Local { position } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
        psi_checked_trees::CheckedBooleanExpression::StructuralParameterField { path, .. } => {
            path.len() == 1
        }
        psi_checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
        | psi_checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
        | psi_checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | psi_checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => false,
    }
}

pub(super) fn is_branch_free_structural_integer_expression(
    expression: &CheckedScalarExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        CheckedScalarExpression::IntegerLiteral { .. } => true,
        CheckedScalarExpression::IntegerBinary { left, right, .. } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
        | CheckedScalarExpression::IntegerWiden { operand, .. }
        | CheckedScalarExpression::IntegerExactCast { operand, .. } => {
            is_branch_free_structural_integer_expression(
                operand,
                scalar_parameters,
                available_locals,
            )
        }
        CheckedScalarExpression::Parameter { position, .. } => *position < scalar_parameters,
        CheckedScalarExpression::Local { position, .. } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
        CheckedScalarExpression::StructuralParameterField { .. }
        | CheckedScalarExpression::Boolean(_) => false,
    }
}

pub(super) fn is_branch_free_structural_scalar_expression(
    expression: &CheckedScalarExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        CheckedScalarExpression::Boolean(expression) => {
            is_branch_free_structural_boolean_expression(
                expression,
                scalar_parameters,
                available_locals,
            )
        }
        expression => is_branch_free_structural_integer_expression(
            expression,
            scalar_parameters,
            available_locals,
        ),
    }
}

pub(super) fn is_branch_free_structural_boolean_expression(
    expression: &psi_checked_trees::CheckedBooleanExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        psi_checked_trees::CheckedBooleanExpression::Constant(_) => true,
        psi_checked_trees::CheckedBooleanExpression::Not(operand) => {
            is_branch_free_structural_boolean_expression(
                operand,
                scalar_parameters,
                available_locals,
            )
        }
        psi_checked_trees::CheckedBooleanExpression::Equal { left, right } => {
            is_branch_free_structural_boolean_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_boolean_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        psi_checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        psi_checked_trees::CheckedBooleanExpression::Parameter { position } => {
            *position < scalar_parameters
        }
        psi_checked_trees::CheckedBooleanExpression::Local { position } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
        psi_checked_trees::CheckedBooleanExpression::StructuralParameterField { .. } => false,
        psi_checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
        | psi_checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
        | psi_checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | psi_checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => false,
        psi_checked_trees::CheckedBooleanExpression::And { .. }
        | psi_checked_trees::CheckedBooleanExpression::Or { .. } => false,
    }
}
