//! Boundary scalar return machines and their supported contracts.

use crate::execution::terminal_unit::cleanup::machine_has_content_evidence;
use crate::execution::terminal_unit::{
    CheckFacts, CheckedBoundaryMachinePlan, CheckedBoundaryScalarReturnMachinePlan,
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralParameterPlan, ExpectedCallValueResult, ExpressionNode, PrimitiveType,
    ProofFact, ShapeCollector, SignatureContractKind, StatementNode, TypedTrees,
    build_call_operation, checked_structural_signature_contract_supported, entry_claims,
    machine_binders, outer_calls, state_flow, structural_scalar_signature, structural_signature,
};

fn boundary_scalar_contracts_supported(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
) -> bool {
    let Some(checked_contract) = facts.contract_plans.for_machine(machine.symbol) else {
        return false;
    };
    let source_contracts = program.machine_contracts(machine);
    let mut requirements = checked_contract.closed_scalar_values.requires().iter();
    for contract in source_contracts {
        if contract.binding.is_some() {
            return false;
        }
        if matches!(contract.kind, SignatureContractKind::Crashes { .. }) {
            continue;
        }
        let structural = checked_structural_signature_contract_supported(
            program,
            machine,
            state,
            structural_parameters,
            contract,
        );
        if contract.kind == SignatureContractKind::Requires {
            let Some(requirement) = requirements.next() else {
                return false;
            };
            if structural {
                // The scalar contract keeps an explicit unsupported slot for
                // membership; the exact structural signature owns that clause.
                if requirement.is_some() {
                    return false;
                }
            } else if !matches!(
                program.proof_facts.span_or_empty(contract.facts),
                [ProofFact::Expression(_)]
            ) || requirement.is_none()
            {
                return false;
            }
        } else if !structural {
            return false;
        }
    }
    // Rejoin the entire implicit suffix through its existing source collector.
    // Unsupported, removed, added, or changed range rows cannot become omission.
    let expected_ranges =
        crate::values::lower_integer_parameter_range_requirements(program, machine);
    requirements.len() == expected_ranges.len()
        && requirements.zip(&expected_ranges).all(|(retained, expected)| {
            matches!((retained, expected),
                (Some(checked_trees::ClosedScalarContractValue::Predicate(retained)), Some(expected))
                    if retained == expected)
        })
        && program.state_contracts(state).iter().all(|contract| {
            contract.binding.is_none() && (matches!(contract.kind, SignatureContractKind::Crashes { .. })
                || checked_structural_signature_contract_supported(
                    program,
                    machine,
                    state,
                    structural_parameters,
                    contract,
                )
                || source_contracts
                    .iter()
                    .any(|source| source.kind == contract.kind && source.facts == contract.facts))
        })
}

pub(crate) fn build_boundary_scalar_return_machine(
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
    let carries_scalar_parameter = program.state_parameters(state).iter().any(|parameter| {
        !parameter.is_self
            && program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
    });
    let (attachment_type_identity, structural_parameters, scalar_parameters) =
        if carries_scalar_parameter {
            structural_scalar_signature(program, shapes, machine, state, &binders, false)?
        } else {
            let (attachment, structural) =
                structural_signature(program, shapes, machine, state, &binders, false)?;
            (attachment, structural, Vec::new())
        };
    if !boundary_scalar_contracts_supported(program, facts, machine, state, &structural_parameters)
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
        None,
        machine,
        state,
        &structural_parameters,
        &[],
        &entry_claims,
        call,
        false,
        Some(ExpectedCallValueResult::Scalar(result_type)),
        &[],
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
    let result_position = scalar_parameters.len();
    let returns_binding = match return_expression {
        CheckedScalarExpression::Local {
            position,
            primitive_type,
        } => *position == result_position && *primitive_type == result_type,
        CheckedScalarExpression::Boolean(expression) => {
            result_type == PrimitiveType::Bool
                && matches!(
                    expression.as_ref(),
                    checked_trees::CheckedBooleanExpression::Local { position }
                        if *position == result_position
                )
        }
        _ => false,
    };
    if !returns_binding {
        return None;
    }
    let erased_scalar_parameters =
        crate::execution::terminal_unit::types::erased_scalar_parameter_plans(program, state)?;
    Some(CheckedBoundaryScalarReturnMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        structural_parameters,
        scalar_parameters,
        erased_scalar_parameters,
        entry_claims,
        boundary_call,
        result_type,
        return_statement_ordinal,
        contract_service_reach: facts.service_reaches.plan_for_machine(machine.symbol)?,
        service_reach: state_flow.service_reach,
    })
}
