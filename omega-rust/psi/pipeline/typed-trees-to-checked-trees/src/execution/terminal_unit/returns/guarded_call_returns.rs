//! Payloadless guarded call return machines.

use crate::execution::terminal_unit::cleanup::{
    service_reach_is_empty, service_reach_plan_is_empty,
};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedPayloadlessGuardedCallEvidencePlan,
    CheckedPayloadlessGuardedCallEvidenceUsePlan, CheckedPayloadlessGuardedCallReturnMachinePlan,
    CheckedStructuralResultPlan, CheckedStructuralReturnPlans, CheckedUnitCallCoordinate,
    DataMember, ExpressionNode, Multiplicity, ShapeCollector, SignatureContractKind, StatementNode,
    TransitionExit, TransitionGuardNode, TransitionTargetNode, TypeReferenceNode, TypedTrees,
    machine_binders, parameter_qualifications, state_flow, structural_signature,
};

pub(crate) fn build_payloadless_guarded_call_return_machine(
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
    if !call.carries_only_positional_arguments()
        || !program
            .expression_table
            .expression_handles(call.arguments)
            .is_empty()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
    {
        return None;
    }
    if flow_call.target_symbol != call.target_symbol {
        return None;
    }
    let call_site = crate::semantic_calls::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        flow_call.statement_index,
        flow_call.call_ordinal,
    )?;
    let crate::semantic_calls::CallSite::Expression { expression, .. } = call_site else {
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
        crate::flow::contextual_canonical_place_from_expression(
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
        let tail_flow = crate::execution::terminal_unit::types::state_flow(
            facts,
            machine.symbol,
            tail_state.symbol,
        )?;
        let returned_parameter = crate::flow::contextual_canonical_place_from_expression(
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
            let tail_contract_call = crate::flow::proof_contract_call(
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
            projected_qualifications: Vec::new(),
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
