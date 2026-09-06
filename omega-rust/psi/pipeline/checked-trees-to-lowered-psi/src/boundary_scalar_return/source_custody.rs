//! Authored result binding and return custody for the boundary-return route.

use super::*;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;

pub(super) fn validate(
    checked: &CheckedTrees,
    plan: &CheckedBoundaryScalarReturnMachinePlan,
) -> Result<(), LoweringError> {
    let program = &checked.typed;
    let (machine, state) = crate::scalar_source_custody::authored_state(checked, plan.state)?;
    let mut attachments = program
        .data_definitions()
        .iter()
        .filter(|data| machine.attached_data.as_ref() == Some(&data.name));
    let attachment = attachments.next().ok_or(LoweringError::Unsupported(
        "boundary scalar return has no authored attachment",
    ))?;
    if attachments.next().is_some() || !program.data_type_parameters(attachment).is_empty() {
        return unsupported("boundary scalar return attachment is ambiguous or generic");
    }
    // Match ShapeCollector's normalized named identity from the exact authored
    // declaration. The attached-data spelling only locates that declaration;
    // the emitted identity must retain its resolved symbol path.
    let mut attachment_identity = String::from("named(name(");
    for character in program
        .symbols
        .display_path(attachment.symbol, "::")
        .chars()
    {
        if matches!(character, '\\' | '(' | ')' | ',') {
            attachment_identity.push('\\');
        }
        attachment_identity.push(character);
    }
    attachment_identity.push_str("))");
    if attachment_identity != plan.attachment_type_identity {
        return unsupported("boundary scalar return attachment disagrees with its authored owner");
    }
    let mut source_flows = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, flow)| flow)
        .filter(|flow| flow.machine_symbol == plan.machine && flow.state_symbol == plan.state);
    let source_flow = source_flows.next().ok_or(LoweringError::Unsupported(
        "boundary scalar return has no checked source flow",
    ))?;
    if source_flows.next().is_some()
        || source_flow.service_reach != plan.service_reach
        || checked.facts.service_reaches.plan_for_machine(plan.machine)
            != Some(plan.contract_service_reach)
    {
        return unsupported("boundary scalar return disagrees with its source service reach");
    }
    let content = &checked.facts.qualifications.content;
    if content
        .identity_reshuffles
        .iter()
        .any(|fact| fact.machine_symbol == plan.machine && fact.state_symbol == plan.state)
        || content
            .partition_compositions
            .iter()
            .any(|fact| fact.machine_symbol == plan.machine && fact.state_symbol == plan.state)
    {
        return unsupported("boundary scalar return cannot erase source content evidence");
    }
    let expected_scalar_parameters =
        crate::attached_unit::checked_scalar_source_parameters(checked, state)?;
    let source_structural_parameters = program
        .state_parameters(state)
        .iter()
        .enumerate()
        .filter(|(_, source)| {
            program
                .primitive_type_reference(source.type_reference)
                .is_none()
        })
        .collect::<Vec<_>>();
    // Scalar requirements use the existing checked predicate package; structural
    // memberships stay with the exact structural signature. Neither route may
    // erase authored requirements or guarantees during shared closure assembly.
    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || plan.scalar_parameters != expected_scalar_parameters
        || source_structural_parameters.len() != plan.structural_parameters.len()
        || source_structural_parameters
            .iter()
            .zip(&plan.structural_parameters)
            .any(|((position, source), retained)| {
                source.is_const
                    || source.is_self != retained.is_self
                    || usize::try_from(retained.position).ok() != Some(*position)
            })
        || program
            .state_contracts(state)
            .iter()
            .chain(program.machine_contracts(machine))
            .any(|contract| {
                contract.binding.is_some()
                    || program
                        .proof_facts
                        .span_or_empty(contract.facts)
                        .iter()
                        .any(|fact| match (&contract.kind, fact) {
                            (
                                checked_trees::signature::SignatureContractKind::Requires,
                                checked_trees::domain::ProofFact::Membership(_),
                            ) => memberships::validate(checked, plan, fact).is_err(),
                            (
                                checked_trees::signature::SignatureContractKind::Requires,
                                checked_trees::domain::ProofFact::Expression(_),
                            ) => !program.machine_contracts(machine).contains(contract),
                            (
                                checked_trees::signature::SignatureContractKind::Ensures,
                                checked_trees::domain::ProofFact::Expression(expression),
                            ) => {
                                !program.expression_table.expression_is_valid(*expression)
                                    || !matches!(
                                        program.expression_table.expression(*expression),
                                        ExpressionNode::Boolean(true)
                                    )
                            }
                            (
                                checked_trees::signature::SignatureContractKind::Crashes { .. },
                                _,
                            ) => false,
                            _ => true,
                        })
            })
    {
        return unsupported(
            "boundary scalar return source exceeds its structural signature and contract slice",
        );
    }
    checked_requirements(checked, plan)?;
    let [
        StatementNode::LocalData(local),
        StatementNode::Expression(returned),
    ] = program.statement_table.statements(state.statement_nodes)
    else {
        return unsupported("boundary scalar return lost its authored initializer and return");
    };
    if machine.symbol != plan.machine
        || program.machine_states(machine).len() != 1
        || local.is_mutable
        || !local.symbol.is_valid()
        || program.primitive_type_reference(local.type_reference) != Some(plan.result_type)
        || program.primitive_type_reference(state.return_type) != Some(plan.result_type)
        || plan.return_statement_ordinal != 1
        || !program
            .expression_table
            .expression_is_valid(local.initial_value)
        || !program.expression_table.expression_is_valid(*returned)
    {
        return unsupported(
            "boundary scalar result disagrees with its authored binding or carrier",
        );
    }
    let ExpressionNode::Name(name) = program.expression_table.expression(*returned) else {
        return unsupported("boundary scalar return is not its established result local");
    };
    if name.symbol != local.symbol
        || name.head_symbol != local.symbol
        || program
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return unsupported("boundary scalar return names another source value");
    }
    let (binding, expression) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(
            plan.state,
            plan.return_statement_ordinal,
            CheckedScalarExpressionRole::Return,
        )
        .ok_or(LoweringError::Unsupported(
            "boundary scalar return has no unique source-bound value",
        ))?;
    crate::scalar_source_custody::validate_pure(
        checked,
        binding,
        terminal_scalar_type(plan.result_type)?,
    )?;
    let result_position = plan.scalar_parameters.len();
    let returns_local = match expression {
        CheckedScalarExpression::Local {
            position,
            primitive_type,
        } => *position == result_position && *primitive_type == plan.result_type,
        CheckedScalarExpression::Boolean(expression) => {
            plan.result_type == PrimitiveType::Bool
                && matches!(
                    expression.as_ref(),
                    checked_trees::CheckedBooleanExpression::Local { position }
                        if *position == result_position
                )
        }
        _ => false,
    };
    if !returns_local {
        return unsupported("boundary scalar return plan does not read its established result");
    }
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        coordinate,
        scalar_arguments,
        ..
    } = &plan.boundary_call
    else {
        return unsupported("boundary scalar return has no boundary operation");
    };
    if scalar_arguments.iter().any(|argument| {
        matches!(
            argument,
            checked_trees::CheckedCallScalarArgument::Computation(_)
        )
    }) {
        crate::call_source_custody::initializers::validate(
            checked,
            plan.machine,
            plan.state,
            *coordinate,
        )?;
    }
    Ok(())
}
