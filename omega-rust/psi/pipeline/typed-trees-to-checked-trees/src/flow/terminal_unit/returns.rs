//! Structural Unit and scalar return analysis.

use super::*;

mod selected_operator;
use selected_operator::build_selected_operator_structural_scalar_return_machine;

/// Build the exact checked carriers for `T in D -> T in D` whole-root
/// passthrough and the separate zero-input payload-less sum-case constructor.
/// Every wider ownership or control shape is omitted atomically.
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
    let payloadless_case_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_payloadless_case_return_machine(program, facts, &mut shapes, machine)
        })
        .collect::<Vec<_>>();
    let claim_free_affine_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_claim_free_affine_structural_return_machine(program, facts, &mut shapes, machine)
        })
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
        .chain(payloadless_case_machines.iter().flat_map(|plan| {
            [
                plan.attachment_type_identity.as_str(),
                plan.result.type_identity.as_str(),
            ]
        }))
        .chain(claim_free_affine_machines.iter().flat_map(|plan| {
            [
                plan.attachment_type_identity.as_str(),
                plan.structural_parameter.type_identity.as_str(),
                plan.result.type_identity.as_str(),
            ]
        }))
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
        claim_free_affine_machines,
        payloadless_case_machines,
    }
}

fn build_claim_free_affine_structural_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Option<CheckedClaimFreeAffineStructuralReturnMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let [StatementNode::Expression(return_expression)] =
        program.statement_table.statements(state.statement_nodes)
    else {
        return None;
    };
    if !program.machine_contracts(machine).is_empty()
        || !program.state_contracts(state).is_empty()
        || machine_has_content_evidence(facts, machine.symbol, state.symbol)
    {
        return None;
    }
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters, scalar_parameters) =
        structural_scalar_signature(program, shapes, machine, state, &binders, false)?;
    let [structural_parameter] = structural_parameters.as_slice() else {
        return None;
    };
    if structural_parameter.is_self
        || structural_parameter.multiplicity != Multiplicity::Affine
        || structural_parameter.access != CheckedStructuralAccess::Owned
        || !structural_parameter.qualifications.is_empty()
        || structural_parameter.fused_service_erasure.is_some()
        || scalar_parameters.is_empty()
        || scalar_parameters.iter().any(|parameter| {
            !matches!(
                parameter.primitive_type,
                PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
            )
        })
    {
        return None;
    }
    let source_parameters = program.state_parameters(state);
    let source_parameter = source_parameters.get(structural_parameter.position as usize)?;
    let ExpressionNode::Name(path) = program.expression_table.expression(*return_expression) else {
        return None;
    };
    if path.symbol != source_parameter.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
        || crate::checks::type_multiplicity(program, state.return_type) != Multiplicity::Affine
    {
        return None;
    }
    let result_type_identity = shapes.add_type(state.return_type, &binders, &[])?;
    if result_type_identity != structural_parameter.type_identity
        || !parameter_qualifications(program, shapes, state.return_type, &binders)?.is_empty()
        || !matches!(
            &shapes.types.get(&result_type_identity)?.shape,
            CheckedUnitStructuralTypeShape::Record { fields }
                if matches!(
                    fields.as_slice(),
                    [field]
                        if matches!(
                            &field.field_type,
                            CheckedUnitStructuralFieldType::Scalar(
                                PrimitiveType::I64 | PrimitiveType::U64
                            )
                        )
                )
        )
    {
        return None;
    }
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let checked_entry_claims = entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        source_parameters,
    )?;
    if !facts
        .flow
        .control
        .calls
        .span_or_empty(flow.calls)
        .is_empty()
        || !service_reach_is_empty(facts, flow.service_reach)
        || !checked_entry_claims.is_empty()
    {
        return None;
    }
    Some(CheckedClaimFreeAffineStructuralReturnMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        structural_parameter: structural_parameter.clone(),
        scalar_parameters,
        result: CheckedStructuralResultPlan {
            type_identity: result_type_identity,
            multiplicity: Multiplicity::Affine,
            qualifications: Vec::new(),
        },
        return_statement_ordinal: 0,
    })
}

fn build_payloadless_case_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Option<CheckedPayloadlessCaseReturnMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let [StatementNode::Expression(return_expression)] =
        program.statement_table.statements(state.statement_nodes)
    else {
        return None;
    };
    if !machine.lifetime_parameters.is_empty()
        || !program.machine_type_parameters(machine).is_empty()
        || !program.machine_owned_data(machine).is_empty()
        || !program.machine_trait_conformances(machine).is_empty()
        || !machine.conformance_bounds.is_empty()
        || !program.machine_invokes(machine).is_empty()
        || machine.suspends
        || machine.blocks
        || !program.machine_contracts(machine).iter().all(|contract| {
            matches!(
                contract.kind,
                SignatureContractKind::EnsuresForResultCase { .. }
            )
        })
        || !program.state_contracts(state).is_empty()
        || !program.state_parameters(state).is_empty()
    {
        return None;
    }
    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;
    if !facts
        .flow
        .control
        .calls
        .span_or_empty(state_flow.calls)
        .is_empty()
        || !service_reach_is_empty(facts, state_flow.service_reach)
        || !service_reach_plan_is_empty(
            facts,
            facts.service_reaches.plan_for_machine(machine.symbol)?,
        )
    {
        return None;
    }

    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        structural_signature(program, shapes, machine, state, &binders, false)?;
    if !structural_parameters.is_empty() {
        return None;
    }
    let TypeReferenceNode::Named {
        symbol: result_data_symbol,
        ..
    } = program
        .type_reference_table
        .type_reference(state.return_type)
    else {
        return None;
    };
    if crate::checks::type_multiplicity(program, state.return_type) != Multiplicity::Unrestricted {
        return None;
    }
    let result_qualifications =
        parameter_qualifications(program, shapes, state.return_type, &binders)?;
    if !result_qualifications.is_empty() {
        return None;
    }
    let returned_case_symbol = match program.expression_table.expression(*return_expression) {
        // The canonical payload-less constructor is represented as its exact
        // two-symbol nominal path (`Sum::Case`), not as an empty record.
        ExpressionNode::Name(path)
            if path.head_symbol == *result_data_symbol
                && path.symbol.is_valid()
                && program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    == 2 =>
        {
            path.symbol
        }
        // Retain the equivalent explicit empty-brace case form if the parser
        // preserves it as a structural literal.
        ExpressionNode::StructLiteral(literal)
            if literal.type_symbol == *result_data_symbol
                && literal.case_name.is_some()
                && program
                    .expression_table
                    .struct_fields(literal.fields)
                    .is_empty() =>
        {
            literal.case_symbol?
        }
        _ => return None,
    };
    let result_data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *result_data_symbol)?;
    let result_members = program.data_members(result_data);
    if result_members.len() < 2
        || result_members.iter().any(|member| {
            !matches!(
                member,
                DataMember::Variant(variant)
                    if program.data_payload_fields(variant).is_empty()
            )
        })
    {
        return None;
    }
    let returned_case_identity = result_members.iter().find_map(|member| {
        let DataMember::Variant(variant) = member else {
            return None;
        };
        (variant.symbol == returned_case_symbol).then(|| {
            variant
                .identity
                .map(|identity| format!("#{identity}"))
                .unwrap_or_else(|| variant.name.as_str().to_owned())
        })
    })?;
    let result_type_identity = shapes.add_type(state.return_type, &binders, &[])?;
    let CheckedUnitStructuralTypeShape::Sum { cases } =
        &shapes.types.get(&result_type_identity)?.shape
    else {
        return None;
    };
    if cases.len() != result_members.len()
        || !cases.iter().all(|case| case.fields.is_empty())
        || !cases
            .iter()
            .any(|case| case.identity == returned_case_identity)
    {
        return None;
    }

    Some(CheckedPayloadlessCaseReturnMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        result: CheckedStructuralResultPlan {
            type_identity: result_type_identity,
            multiplicity: Multiplicity::Unrestricted,
            qualifications: result_qualifications,
        },
        returned_case_identity,
    })
}

/// Build the bounded internal structural-result call slice. The caller has
/// one linear whole-root input, performs one final direct call to an already
/// admitted structural-return machine, and returns that result immediately.
/// Bodyless calls, projections, staged locals, and wider result maps remain
/// deliberately outside this carrier.
pub(crate) fn build_checked_structural_call_return_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
    structural_returns: &CheckedStructuralReturnPlans,
) -> CheckedStructuralCallReturnPlans {
    let mut shapes = ShapeCollector::new(program);
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_structural_call_return_machine(
                program,
                facts,
                structural_returns,
                &mut shapes,
                machine,
            )
        })
        .collect::<Vec<_>>();
    let payloadless_guarded_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_payloadless_guarded_call_return_machine(
                program,
                facts,
                structural_returns,
                &mut shapes,
                machine,
            )
        })
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
                .chain(std::iter::once(plan.result.type_identity.as_str()))
        })
        .chain(payloadless_guarded_machines.iter().flat_map(|plan| {
            [
                plan.attachment_type_identity.as_str(),
                plan.result.type_identity.as_str(),
            ]
        }))
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
    CheckedStructuralCallReturnPlans {
        structural_types: shapes.types.into_values().collect(),
        structural_domains: shapes.domains,
        machines,
        payloadless_guarded_machines,
    }
}

fn build_payloadless_guarded_call_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    structural_returns: &CheckedStructuralReturnPlans,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Option<CheckedPayloadlessGuardedCallReturnMachinePlan> {
    let (state, tail_state) = match program.machine_states(machine) {
        [state] => (state, None),
        [state, tail] => (state, Some(tail)),
        _ => return None,
    };
    if !machine.lifetime_parameters.is_empty()
        || !program.machine_type_parameters(machine).is_empty()
        || !program.machine_owned_data(machine).is_empty()
        || !program.machine_trait_conformances(machine).is_empty()
        || !machine.conformance_bounds.is_empty()
        || machine.suspends
        || machine.blocks
        || !program.machine_contracts(machine).is_empty()
        || !program.state_contracts(state).is_empty()
        || !program.state_parameters(state).is_empty()
    {
        return None;
    }
    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;
    if !service_reach_is_empty(facts, state_flow.service_reach)
        || !service_reach_plan_is_empty(
            facts,
            facts.service_reaches.plan_for_machine(machine.symbol)?,
        )
    {
        return None;
    }

    let flow_calls = facts.flow.control.calls.span_or_empty(state_flow.calls);
    let flow_call = flow_calls.iter().find(|call| call.statement_index == 0)?;
    if flow_call.statement_index != 0
        || flow_call.call_ordinal != 0
        || !flow_call.has_receiver
        || !flow_call.accesses.is_empty()
        || !flow_call.requires.is_empty()
        || !flow_call.ensures.is_empty()
        || !service_reach_is_empty(facts, flow_call.service_reach)
        || flow_call.suspension != language_semantics::SuspensionSummary::default()
        || flow_call.blocking != language_semantics::BlockingSummary::default()
        || flow_call.operational_acknowledgement
            != language_semantics::CallOperationalAcknowledgement::default()
    {
        return None;
    }

    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(saved) = statements.first()? else {
        return None;
    };
    if saved.is_mutable {
        return None;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(saved.initial_value)
    else {
        return None;
    };
    if !call.machine_arguments.is_empty()
        || !program
            .expression_table
            .expression_handles(call.arguments)
            .is_empty()
        || !call.evidence_arguments.is_empty()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
    {
        return None;
    }
    if flow_call.target_symbol != call.target_symbol {
        return None;
    }
    let call_site = crate::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        flow_call.statement_index,
        flow_call.call_ordinal,
    )?;
    let crate::CallSite::Expression { expression, .. } = call_site else {
        return None;
    };
    if expression != saved.initial_value {
        return None;
    }
    let (target_machine, target_state) = program.machines().iter().find_map(|target_machine| {
        program
            .machine_states(target_machine)
            .iter()
            .find(|target_state| target_state.symbol == call.target_symbol)
            .map(|target_state| (target_machine, target_state))
    })?;
    let target_plan = structural_returns.payloadless_case_for_machine(target_machine.symbol)?;
    if target_plan.state != target_state.symbol
        || flow_call.receiver_symbol != target_machine.attached_data_symbol
    {
        return None;
    }
    let ExpressionNode::Name(receiver) = program.expression_table.expression(call.receiver) else {
        return None;
    };
    let receiver_symbol = std::iter::once(receiver.head_symbol)
        .chain(
            program
                .expression_table
                .name_path_member_symbols(receiver.member_symbols)
                .iter()
                .copied(),
        )
        .chain(std::iter::once(receiver.symbol))
        .find(|symbol| symbol.is_valid())?;
    if receiver_symbol != target_machine.attached_data_symbol
        || !matches!(
            program.expression_table.name_path_members(receiver.members),
            [_]
        )
    {
        return None;
    }

    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        structural_signature(program, shapes, machine, state, &binders, false)?;
    if !structural_parameters.is_empty()
        || attachment_type_identity != target_plan.attachment_type_identity
    {
        return None;
    }
    let TypeReferenceNode::Named {
        symbol: result_data_symbol,
        ..
    } = program
        .type_reference_table
        .type_reference(state.return_type)
    else {
        return None;
    };
    if crate::checks::type_multiplicity(program, state.return_type) != Multiplicity::Unrestricted {
        return None;
    }
    let result_qualifications =
        parameter_qualifications(program, shapes, state.return_type, &binders)?;
    if !result_qualifications.is_empty() {
        return None;
    }
    let result_type_identity = shapes.add_type(state.return_type, &binders, &[])?;
    if result_type_identity != target_plan.result.type_identity {
        return None;
    }
    let result_data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *result_data_symbol)?;
    let result_cases = program
        .data_members(result_data)
        .iter()
        .filter_map(|member| {
            let DataMember::Variant(variant) = member else {
                return None;
            };
            program
                .data_payload_fields(variant)
                .is_empty()
                .then_some(variant.symbol)
        })
        .collect::<Vec<_>>();
    if result_cases.len() < 2 || result_cases.len() != program.data_members(result_data).len() {
        return None;
    }

    let expression_is_saved = |expression, statement_index| {
        crate::flow::place::contextual_canonical_place_from_expression(
            program,
            state.symbol,
            statement_index,
            expression,
        )
        .is_some_and(|place| {
            place.root == facts::PlaceRoot::Symbol(saved.symbol) && place.segments.is_empty()
        })
    };
    if let Some(tail_state) = tail_state {
        let [parameter] = program.state_parameters(tail_state) else {
            return None;
        };
        let contracts = program.state_contracts(tail_state);
        let [StatementNode::Expression(returned)] = program
            .statement_table
            .statements(tail_state.statement_nodes)
        else {
            return None;
        };
        let tail_flow = super::types::state_flow(facts, machine.symbol, tail_state.symbol)?;
        let returned_parameter = crate::flow::place::contextual_canonical_place_from_expression(
            program,
            tail_state.symbol,
            0,
            *returned,
        )
        .is_some_and(|place| {
            place.root == facts::PlaceRoot::Symbol(parameter.symbol) && place.segments.is_empty()
        });
        if parameter.is_const
            || parameter.is_mutable
            || parameter.is_self
            || !(1..=15).contains(&contracts.len())
            || contracts.iter().any(|contract| {
                !matches!(contract.kind, SignatureContractKind::Requires)
                    || contract.binding.is_none()
            })
            || program.normalized_type_identity(tail_state.return_type)
                != program.normalized_type_identity(state.return_type)
            || !returned_parameter
            || !facts
                .flow
                .control
                .calls
                .span_or_empty(tail_flow.calls)
                .is_empty()
            || !service_reach_is_empty(facts, tail_flow.service_reach)
        {
            return None;
        }
    }
    let destructures = statements
        .iter()
        .skip(1)
        .take_while(|statement| matches!(statement, StatementNode::LocalData(_)))
        .collect::<Vec<_>>();
    if destructures.len() != result_cases.len()
        || destructures.iter().any(|statement| {
            !matches!(statement, StatementNode::LocalData(local)
                if !local.is_mutable
                    && local.name.as_str().starts_with("__arm_destructure#V="))
        })
    {
        return None;
    }
    let transitions = &statements[1 + destructures.len()..];
    if transitions.len() != result_cases.len() {
        return None;
    }
    let mut covered_cases = Vec::new();
    let mut covered_arms = Vec::new();
    let mut tail_arm = None;
    for (offset, statement) in transitions.iter().enumerate() {
        let statement_index = 1 + destructures.len() + offset;
        let StatementNode::Transition(transition) = statement else {
            return None;
        };
        if transition.exit != TransitionExit::Ordinary || transition.continuation.is_valid() {
            return None;
        }
        let TransitionGuardNode::When(guard) = transition.guard else {
            return None;
        };
        let (subject, result_case) = crate::proof::exact_outcome_case_test(program, guard)?;
        if !expression_is_saved(subject, statement_index)
            || !result_cases.contains(&result_case)
            || covered_cases.contains(&result_case)
        {
            return None;
        }
        covered_cases.push(result_case);
        covered_arms.push((statement_index, result_case));
        match program.statement_table.transition_target(transition.target) {
            TransitionTargetNode::Value(value) if expression_is_saved(*value, statement_index) => {}
            TransitionTargetNode::Named {
                path,
                arguments,
                evidence_arguments,
                ..
            } if tail_state.is_some_and(|tail| path.symbol == tail.symbol)
                && tail_arm.is_none()
                && matches!(
                    program.statement_table.expression_handles(*arguments),
                    [argument] if expression_is_saved(*argument, statement_index)
                )
                && (1..=15).contains(&evidence_arguments.len()) =>
            {
                tail_arm = Some(statement_index);
            }
            _ => return None,
        }
    }
    if covered_cases.len() != result_cases.len()
        || result_cases
            .iter()
            .any(|result_case| !covered_cases.contains(result_case))
    {
        return None;
    }

    let tied_arms = facts
        .proof
        .outcome_specific_arms
        .iter()
        .filter(|(_, arm)| {
            arm.caller_machine_symbol == machine.symbol
                && arm.caller_state_symbol == state.symbol
                && arm.result_call_statement_index == 0
        })
        .collect::<Vec<_>>();
    if tied_arms.iter().any(|(_, arm)| {
        arm.result_data != *result_data_symbol
            || !result_cases.contains(&arm.result_case)
            || !expression_is_saved(arm.result_expression, arm.statement_index)
            || !covered_arms.contains(&(arm.statement_index, arm.result_case))
    }) {
        return None;
    }
    let matching_arms = tied_arms;
    if matching_arms.is_empty()
        || result_cases.iter().any(|result_case| {
            matching_arms
                .iter()
                .filter(|(_, arm)| arm.result_case == *result_case)
                .count()
                > 1
        })
    {
        return None;
    }
    let selected = matching_arms.iter().flat_map(|(_, arm)| {
        arm.rows.iter().filter_map(move |row| {
            row.selected_term.map(|selected_term| {
                (
                    arm.statement_index,
                    row.guarantee,
                    selected_term,
                    &row.validity,
                    row.instantiated_proposition.as_ref(),
                )
            })
        })
    });
    let mut selected_evidence = selected
        .map(
            |(arm_statement_index, guarantee, selected_term, validity, proposition)| {
                if !expression_is_saved(validity.result_occurrence, arm_statement_index)
                    || validity.referenced_occurrences.len() > 1
                    || validity
                        .evidence_interface_scope
                        .as_ref()
                        .is_none_or(|scope| {
                            !scope.reference_regions.is_empty()
                                || scope.retained_occurrences.len() > 1
                                || scope.retained_occurrences != validity.referenced_occurrences
                        })
                    || (!validity.referenced_occurrences.is_empty()
                        && proposition.is_none_or(|proposition| proposition.arguments.len() != 1))
                {
                    return None;
                }
                Some(CheckedPayloadlessGuardedCallEvidencePlan {
                    arm_statement_index: u32::try_from(arm_statement_index).ok()?,
                    guarantee,
                    selected_term,
                    substitutes_result: !validity.referenced_occurrences.is_empty(),
                    tail_use: None,
                })
            },
        )
        .collect::<Option<Vec<_>>>()?;
    selected_evidence.sort_by_key(|selection| {
        (
            selection.guarantee.arena_index(),
            selection.guarantee.generation(),
        )
    });
    match (tail_state, tail_arm) {
        (None, None) => {
            if flow_calls.len() != 1 {
                return None;
            }
        }
        (Some(tail_state), Some(tail_statement_index)) => {
            if flow_calls.len() != 2 || !(1..=15).contains(&selected_evidence.len()) {
                return None;
            }
            if selected_evidence
                .windows(2)
                .any(|pair| pair[0].selected_term == pair[1].selected_term)
            {
                return None;
            }
            let tail_flow_call = flow_calls.iter().find(|call| {
                call.statement_index == tail_statement_index
                    && call.target_symbol == tail_state.symbol
            })?;
            let tail_contract_call = crate::flow::common::proof_contract_call(
                &facts.proof,
                machine.symbol,
                state.symbol,
                tail_statement_index,
                tail_flow_call.call_ordinal,
            )?;
            let requirements = facts
                .proof
                .contract_fact_refs
                .span_or_empty(tail_contract_call.requires);
            let arguments = facts
                .proof
                .contract_evidence_arguments
                .span_or_empty(tail_contract_call.evidence_arguments);
            if requirements.len() != selected_evidence.len()
                || arguments.len() != selected_evidence.len()
            {
                return None;
            }
            if tail_flow_call.has_receiver
                || tail_contract_call.target_state_symbol != tail_state.symbol
            {
                return None;
            }
            for (input_position, ((selection, requirement), argument)) in selected_evidence
                .iter_mut()
                .zip(requirements)
                .zip(arguments)
                .enumerate()
            {
                let requirement = facts.proof.contract_facts.get(requirement.fact);
                if selection.arm_statement_index != u32::try_from(tail_statement_index).ok()?
                    || argument.source != selection.selected_term
                    || argument.lane_position != input_position
                    || requirement.evidence_term != Some(argument.parameter)
                {
                    return None;
                }
                selection.tail_use = Some(CheckedPayloadlessGuardedCallEvidenceUsePlan {
                    target_state: tail_state.symbol,
                    input_position: u32::try_from(input_position).ok()?,
                    parameter: argument.parameter,
                });
            }
        }
        _ => return None,
    }

    Some(CheckedPayloadlessGuardedCallReturnMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        result: CheckedStructuralResultPlan {
            type_identity: result_type_identity,
            multiplicity: Multiplicity::Unrestricted,
            qualifications: Vec::new(),
        },
        call: CheckedUnitCallCoordinate {
            statement_index: 0,
            call_ordinal: 0,
        },
        target_machine: target_machine.symbol,
        target_state: target_state.symbol,
        selected_evidence,
    })
}

fn build_structural_call_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    structural_returns: &CheckedStructuralReturnPlans,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Option<CheckedStructuralCallReturnMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let [StatementNode::Expression(return_expression)] =
        program.statement_table.statements(state.statement_nodes)
    else {
        return None;
    };
    let ExpressionNode::Call(call_expression) =
        program.expression_table.expression(*return_expression)
    else {
        return None;
    };
    if !program.machine_contracts(machine).is_empty() {
        return None;
    }
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        structural_signature(program, shapes, machine, state, &binders, false)?;
    let [input] = structural_parameters.as_slice() else {
        return None;
    };
    let source_parameters = program.state_parameters(state);
    let [source_parameter] = source_parameters else {
        return None;
    };
    if input.position != 0
        || input.multiplicity != Multiplicity::Linear
        || input.is_self
        || parameter_root_symbol(machine.symbol, source_parameter) != source_parameter.symbol
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
    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;
    let [call] = facts.flow.control.calls.span_or_empty(state_flow.calls) else {
        return None;
    };
    if call.statement_index != 0
        || call.call_ordinal != 0
        || call.target_symbol != call_expression.target_symbol
    {
        return None;
    }
    let call_site = crate::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let crate::CallSite::Expression { expression, .. } = &call_site else {
        return None;
    };
    if *expression != *return_expression {
        return None;
    }
    let target_state = crate::find_state(program, call.target_symbol)?;
    let target_machine = program.machines().iter().find(|candidate| {
        candidate.supply_mode == MachineSupplyMode::CheckedBody
            && program
                .machine_states(candidate)
                .iter()
                .any(|candidate_state| candidate_state.symbol == target_state.symbol)
    })?;
    let target = structural_returns.for_machine(target_machine.symbol)?;
    let [target_parameter] = target.structural_parameters.as_slice() else {
        return None;
    };
    if target.state != target_state.symbol
        || target.returned_parameter_index != 0
        || !target.trivial_affine_locals.is_empty()
        || !target.trivial_affine_local_discard_ordinals.is_empty()
        || !target.trivial_affine_discards.is_empty()
        || target_parameter.type_identity != input.type_identity
        || target_parameter.qualifications != input.qualifications
        || target.result.type_identity != result_type_identity
        || target.result.qualifications != result_qualifications
        || target.result.multiplicity != Multiplicity::Linear
    {
        return None;
    }
    let structural_arguments = structural_call_arguments(
        program,
        facts,
        call,
        machine,
        state,
        &structural_parameters,
        &[],
        &[],
        target_machine,
        target_state,
        &call_site,
        call.receiver_symbol,
        call.statement_index,
        false,
        false,
    )?;
    let [argument] = structural_arguments.as_slice() else {
        return None;
    };
    if argument.source_parameter_index() != Some(0)
        || !argument.path.is_empty()
        || argument.type_identity != input.type_identity
        || argument.access != CheckedStructuralAccess::Owned
        || argument.byte_sequence_literal().is_some()
    {
        return None;
    }
    let claim_transfers = call_claim_transfers(
        facts,
        machine.symbol,
        state.symbol,
        call,
        &structural_parameters,
        &checked_entry_claims,
        &structural_arguments,
        PermissionEventKind::Transfer,
    )?;
    let [claim_transfer] = claim_transfers.as_slice() else {
        return None;
    };
    if claim_transfer.argument_index != 0
        || claim_transfer.claim_identity != entry_claim.claim_identity
    {
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
    let checked_trees::FlowClaimOutcomeSource::Input {
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
    let target_contract = facts.contract_plans.for_machine(target_machine.symbol)?;
    Some(CheckedStructuralCallReturnMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        structural_parameters,
        result: CheckedStructuralResultPlan {
            type_identity: result_type_identity,
            multiplicity: Multiplicity::Linear,
            qualifications: result_qualifications,
        },
        entry_claim: entry_claim.clone(),
        call: CheckedStructuralCallPlan {
            coordinate: CheckedUnitCallCoordinate {
                statement_index: 0,
                call_ordinal: 0,
            },
            target_machine: target_machine.symbol,
            target_state: target_state.symbol,
            target_contract_report_fingerprint: target_contract.report_fingerprint,
            service_reach: call.service_reach,
            structural_arguments,
            claim_transfers,
            callee_returned_claim: target.transferred_claim,
        },
        returned_claim: entry_claim.claim_identity,
    })
}

pub(super) fn build_structural_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
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
        structural_signature(program, shapes, machine, state, &binders, false)?;
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
                        && event.root == facts::PlaceRoot::Symbol(local.symbol)
                })
                .map(|(_, event)| event)
                .collect::<Vec<_>>();
            let [establishment, settlement] = local_events.as_slice() else {
                return None;
            };
            let establishment_source = PermissionEventSource::Statement {
                statement_index: declaration_ordinal,
            };
            let establishment_provenance = language_semantics::PermissionProvenance::Established {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                source: establishment_source,
            };
            if establishment.source != establishment_source
                || establishment.kind != PermissionEventKind::Establish
                || establishment.multiplicity != Multiplicity::Affine
                || establishment.access != PermissionAccess::Owned
                || establishment.claim_identity != PermissionClaimIdentity::Unknown
                || establishment.provenance != establishment_provenance
                || establishment.obligation_live
                || !facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(establishment.segments)
                    .is_empty()
                || settlement.source != PermissionEventSource::StateExit
                || settlement.kind != PermissionEventKind::AffineDrop
                || settlement.multiplicity != Multiplicity::Affine
                || settlement.access != PermissionAccess::Owned
                || settlement.claim_identity != PermissionClaimIdentity::Unknown
                || settlement.provenance != establishment_provenance
                || settlement.obligation_live
                || !facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(settlement.segments)
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
                construction: None,
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
    let checked_trees::FlowClaimOutcomeSource::Input {
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
    state: &typed_trees::state::State,
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
    selected_operator_applications: &[crate::SelectedOperatorApplication],
    diagnostics: &mut Vec<Diagnostic>,
) -> CheckedStructuralScalarReturnPlans {
    let mut shapes = ShapeCollector::new(program);
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
    let selected_operator_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_selected_operator_structural_scalar_return_machine(
                program,
                facts,
                &mut shapes,
                machine,
                &machines,
                selected_operator_applications,
            )
        })
        .collect::<Vec<_>>();
    let trait_operator_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_trait_operator_scalar_return_machine(program, facts, &mut shapes, machine)
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
        .chain(selected_operator_machines.iter().flat_map(|machine| {
            machine
                .structural_parameters
                .iter()
                .map(|parameter| parameter.type_identity.as_str())
        }))
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    CheckedStructuralScalarReturnPlans {
        structural_types: shapes.types.into_values().collect(),
        machines,
        selected_operator_machines,
        trait_operator_machines,
    }
}

fn build_trait_operator_scalar_return_machine(
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
        .filter(|boundary| boundary.result.scalar().is_some())
        .collect::<Vec<_>>();
    boundary_machines.extend(
        build_static_boundary_requirements(program, facts, &mut shapes)
            .into_iter()
            .filter(|boundary| boundary.result.scalar().is_some()),
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
    machine: &typed_trees::machine::Machine,
) -> Option<CheckedBoundaryScalarReturnMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let result_type = program.primitive_type_reference(state.return_type)?;
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        structural_signature(program, shapes, machine, state, &binders, false)?;
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
    let source_calls = facts.flow.control.calls.span(state_flow.calls)?;
    let outer_calls = outer_calls(program, facts, machine.symbol, state, source_calls)?;
    let [call] = outer_calls.as_slice() else {
        return None;
    };
    if call.statement_index != 0
        || call.call_ordinal != 0
        || call.authored_expression != local.initial_value
        || !program
            .expression_table
            .expression_is_valid(local.initial_value)
    {
        return None;
    }
    let boundary_call = build_call_operation(
        program,
        facts,
        machine,
        state,
        &structural_parameters,
        &[],
        &[],
        &entry_claims,
        call,
        false,
        Some(ExpectedCallValueResult::Scalar(result_type)),
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
            boundary.machine == *target_machine && boundary.result.scalar() == Some(result_type)
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
                    checked_trees::CheckedBooleanExpression::Local { position: 0 }
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
        service_reach: state_flow.service_reach,
    })
}

pub(super) fn build_structural_scalar_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    unit_effects: &CheckedUnitEffectPlans,
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
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters, scalar_parameters) =
        structural_scalar_signature(program, shapes, machine, state, &binders, false)?;
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
            target_contract_report_fingerprint,
            service_reach,
            scalar_arguments,
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
    expression: &checked_trees::CheckedBooleanExpression,
) -> bool {
    match expression {
        checked_trees::CheckedBooleanExpression::StorageRead { .. } => false,
        checked_trees::CheckedBooleanExpression::And { .. }
        | checked_trees::CheckedBooleanExpression::Or { .. } => true,
        checked_trees::CheckedBooleanExpression::Not(operand) => {
            checked_boolean_contains_short_circuit(operand)
        }
        checked_trees::CheckedBooleanExpression::Equal { left, right } => {
            checked_boolean_contains_short_circuit(left)
                || checked_boolean_contains_short_circuit(right)
        }
        checked_trees::CheckedBooleanExpression::Constant(_)
        | checked_trees::CheckedBooleanExpression::Parameter { .. }
        | checked_trees::CheckedBooleanExpression::Local { .. }
        | checked_trees::CheckedBooleanExpression::StructuralParameterField { .. }
        | checked_trees::CheckedBooleanExpression::IntegerComparison { .. }
        | checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
        | checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
        | checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => false,
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
    expression: &checked_trees::CheckedBooleanExpression,
    local: usize,
) -> usize {
    match expression {
        checked_trees::CheckedBooleanExpression::StorageRead { .. } => 0,
        checked_trees::CheckedBooleanExpression::Local { position } => {
            usize::from(*position == local)
        }
        checked_trees::CheckedBooleanExpression::Not(operand) => {
            checked_boolean_local_reference_count(operand, local)
        }
        checked_trees::CheckedBooleanExpression::Equal { left, right }
        | checked_trees::CheckedBooleanExpression::And { left, right }
        | checked_trees::CheckedBooleanExpression::Or { left, right } => {
            checked_boolean_local_reference_count(left, local)
                .saturating_add(checked_boolean_local_reference_count(right, local))
        }
        checked_trees::CheckedBooleanExpression::Constant(_)
        | checked_trees::CheckedBooleanExpression::Parameter { .. }
        | checked_trees::CheckedBooleanExpression::StructuralParameterField { .. }
        | checked_trees::CheckedBooleanExpression::IntegerComparison { .. }
        | checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
        | checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
        | checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => 0,
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
    expression: &checked_trees::CheckedBooleanExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        checked_trees::CheckedBooleanExpression::StorageRead { .. } => false,
        checked_trees::CheckedBooleanExpression::Constant(_) => true,
        checked_trees::CheckedBooleanExpression::Not(operand) => {
            is_structural_boolean_return_expression(operand, scalar_parameters, available_locals)
        }
        checked_trees::CheckedBooleanExpression::Equal { left, right }
        | checked_trees::CheckedBooleanExpression::And { left, right }
        | checked_trees::CheckedBooleanExpression::Or { left, right } => {
            is_structural_boolean_return_expression(left, scalar_parameters, available_locals)
                && is_structural_boolean_return_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        checked_trees::CheckedBooleanExpression::Parameter { position } => {
            *position < scalar_parameters
        }
        checked_trees::CheckedBooleanExpression::Local { position } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
        checked_trees::CheckedBooleanExpression::StructuralParameterField { path, .. } => {
            path.len() == 1
        }
        checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
        | checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
        | checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => false,
    }
}

pub(super) fn is_branch_free_structural_integer_expression(
    expression: &CheckedScalarExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        CheckedScalarExpression::StorageRead { .. } => false,
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
        CheckedScalarExpression::IeeeFloatLiteral { .. }
        | CheckedScalarExpression::StructuralParameterField { .. }
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
    expression: &checked_trees::CheckedBooleanExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        checked_trees::CheckedBooleanExpression::StorageRead { .. } => false,
        checked_trees::CheckedBooleanExpression::Constant(_) => true,
        checked_trees::CheckedBooleanExpression::Not(operand) => {
            is_branch_free_structural_boolean_expression(
                operand,
                scalar_parameters,
                available_locals,
            )
        }
        checked_trees::CheckedBooleanExpression::Equal { left, right } => {
            is_branch_free_structural_boolean_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_boolean_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        checked_trees::CheckedBooleanExpression::Parameter { position } => {
            *position < scalar_parameters
        }
        checked_trees::CheckedBooleanExpression::Local { position } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
        checked_trees::CheckedBooleanExpression::StructuralParameterField { .. } => false,
        checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
        | checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
        | checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => false,
        checked_trees::CheckedBooleanExpression::And { .. }
        | checked_trees::CheckedBooleanExpression::Or { .. } => false,
    }
}
