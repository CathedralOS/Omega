//! Nested structural operands retain captured call coordinates and source scope.

use super::*;
use checked_trees::FlowFacts;

pub(crate) fn is_scalar_return_call(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> bool {
    let Some(primitive_type) = program.primitive_type_reference(state.return_type) else {
        return false;
    };
    if !matches!(program.statement_table.statements(state.statement_nodes).last(),
        Some(StatementNode::Expression(root)) if *root == expression)
    {
        return false;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    crate::flow::call_target_return_type(program, call.target_symbol).is_some_and(|return_type| {
        program.primitive_type_reference(return_type) == Some(primitive_type)
    })
}
pub(crate) fn retain_nested_structural_call_arguments(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    flow: &FlowFacts,
    plans: &mut CheckedScalarExpressionPlans,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let parameters = program
                .state_parameters(state)
                .iter()
                .filter(|parameter| {
                    program
                        .primitive_type_reference(parameter.type_reference)
                        .is_some()
                })
                .cloned()
                .collect::<Vec<_>>();
            let parameter_types = parameters
                .iter()
                .map(|parameter| program.primitive_type_reference(parameter.type_reference))
                .collect::<Option<Vec<_>>>()
                .expect("filtered scalar parameters retain primitive carriers");
            let mut locals = Vec::new();
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let Ok(statement_ordinal) = u32::try_from(statement_index) else {
                    continue;
                };
                for construction in
                    call_array_constructions(program, flow, machine, state, statement_index)
                {
                    let Some(array) = validation::scalar_array_elements(
                        program,
                        machine.symbol,
                        construction.expression,
                        construction.type_reference,
                    ) else {
                        continue;
                    };
                    for (element_index, (element, primitive_type)) in
                        array.elements.into_iter().enumerate()
                    {
                        let Ok(element_ordinal) = u32::try_from(element_index) else {
                            break;
                        };
                        let Some(value) = lower_return_expression(
                            program,
                            operators,
                            element,
                            &parameters,
                            program.state_parameters(state),
                            &parameter_types,
                            &locals,
                            primitive_type,
                            exact_integer_casts,
                        ) else {
                            continue;
                        };
                        let role = CheckedScalarExpressionRole::ArrayElement {
                            source: construction.source,
                            element_ordinal,
                        };
                        plans
                            .source_bindings
                            .append(CheckedScalarExpressionBindings {
                                destination: symbols::SymbolHandle::invalid(),
                                state: state.symbol,
                                statement_ordinal,
                                role,
                                expression: element,
                                symbols: plans.binding_symbols.insert_many(
                                    parameters.iter().map(|parameter| parameter.symbol).chain(
                                        locals
                                            .iter()
                                            .filter(|local| !local.is_mutable)
                                            .map(|local| local.symbol),
                                    ),
                                ),
                            });
                        plans.expressions.push(CheckedLocatedScalarExpression {
                            state: state.symbol,
                            statement_ordinal,
                            role,
                            expression: value,
                        });
                    }
                }
                for (call_ordinal, site) in
                    nested_structural_call_sites(program, flow, machine, state, statement_index)
                {
                    if let Some(arguments) = lower_boundary_call_arguments(
                        program,
                        operators,
                        state,
                        statement_ordinal,
                        call_ordinal,
                        &site,
                        &parameters,
                        program.state_parameters(state),
                        &parameter_types,
                        &locals,
                        exact_integer_casts,
                        true,
                    ) {
                        retain_call_arguments(
                            arguments,
                            &parameters,
                            &locals,
                            &mut plans.expressions,
                            &mut plans.source_bindings,
                            &mut plans.binding_symbols,
                        );
                    }
                }
                // Every operand of this statement sees the same declarations;
                // the enclosing initializer becomes available only afterward.
                if let StatementNode::LocalData(local) = statement
                    && local.initial_value.is_valid()
                    && let Some(primitive_type) =
                        program.primitive_type_reference(local.type_reference)
                {
                    locals.push(ScalarLocal {
                        is_mutable: local.is_mutable,
                        symbol: local.symbol,
                        name: local.name.as_str().to_owned(),
                        primitive_type,
                        arithmetic_domain: program
                            .arithmetic_domain_for_type_reference(local.type_reference),
                    });
                }
            }
        }
    }
}

/// Read captured occurrences directly. In particular, skipped syntax does not
/// acquire a flow call, and execution order never becomes occurrence identity.
pub(super) fn nested_structural_call_sites<'program>(
    program: &'program TypedTrees,
    flow: &FlowFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
) -> Vec<(usize, crate::CallSite<'program>)> {
    let mut states = flow.control.states.iter().filter_map(|(_, candidate)| {
        (candidate.machine_symbol == machine.symbol && candidate.state_symbol == state.symbol)
            .then_some(candidate)
    });
    let Some(source) = states.next() else {
        return Vec::new();
    };
    if states.next().is_some() {
        return Vec::new();
    }
    let Some(calls) = flow.control.calls.span(source.calls) else {
        return Vec::new();
    };
    calls
        .iter()
        .filter_map(|source| {
            if source.statement_index != statement_index
                || source.call_ordinal == 0
                || !program
                    .expression_table
                    .expression_is_valid(source.authored_expression)
                || calls.iter().any(|other| {
                    !std::ptr::eq(source, other)
                        && other.statement_index == statement_index
                        && (other.call_ordinal == source.call_ordinal
                            || other.authored_expression == source.authored_expression)
                })
            {
                return None;
            }
            let ExpressionNode::Call(call) = program
                .expression_table
                .expression(source.authored_expression)
            else {
                return None;
            };
            if call.target_symbol != source.target_symbol
                || !call.machine_arguments.is_empty()
                || !call.evidence_arguments.is_empty()
                || call.static_requirement_dispatch.is_some()
                || call.quotient_operation.is_some()
                || call.private_layout_operation.is_some()
            {
                return None;
            }
            nested_structural_call_return_type(program, machine.symbol, call)?;
            if source.has_receiver != call.receiver.is_valid()
                || source.receiver_symbol
                    != if call.receiver.is_valid() {
                        let ExpressionNode::Name(name) =
                            program.expression_table.expression(call.receiver)
                        else {
                            return None;
                        };
                        name.symbol
                    } else {
                        symbols::SymbolHandle::invalid()
                    }
            {
                return None;
            }
            Some((
                source.call_ordinal,
                crate::CallSite::Expression {
                    expression: source.authored_expression,
                    call,
                },
            ))
        })
        .collect()
}

/// One source eligibility judgment feeds pure/computed operands and structural
/// scheduling. A checked producer plan and its exact result shape are joined
/// later; this early selector creates no result or ownership evidence.
pub(crate) fn nested_structural_call_return_type(
    program: &TypedTrees,
    caller: symbols::SymbolHandle,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<TypeReferenceHandle> {
    if !call.target_symbol.is_valid()
        || !call.machine_arguments.is_empty()
        || !call.evidence_arguments.is_empty()
        || call.static_requirement_dispatch.is_some()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
    {
        return None;
    }
    let mut targets = program.machines().iter().filter_map(|owner| {
        let target = program.machine_states(owner).first()?;
        (target.symbol == call.target_symbol).then_some((owner, target))
    });
    let (return_type, ordinary) = if let Some((owner, target)) = targets.next() {
        if targets.next().is_some()
            || (owner.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                && !owner.supply_mode.is_boundary_declaration())
            || !program.call_has_no_runtime_receiver(call, owner, target)
        {
            return None;
        }
        (
            target.return_type,
            owner.supply_mode == language_semantics::MachineSupplyMode::CheckedBody,
        )
    } else {
        let selected = program.machine_parameter_signature(call.target_symbol);
        let requirement = match selected {
            Some((owner, signature)) if owner.symbol == caller => signature.symbol,
            Some(_) => return None,
            None => call.target_symbol,
        };
        let mut signatures = program
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
            .flat_map(|definition| {
                program
                    .trait_machine_signatures(definition)
                    .iter()
                    .filter(move |signature| signature.symbol == requirement)
                    .map(move |signature| (definition, signature))
            });
        let (definition, signature) = signatures.next()?;
        if signatures.next().is_some()
            || program
                .state_signature_parameters(signature)
                .iter()
                .any(|parameter| parameter.is_self)
            || if selected.is_some() {
                call.receiver.is_valid()
            } else {
                !program.expression_table.expression_is_valid(call.receiver)
                    || !matches!(program.expression_table.expression(call.receiver),
                        ExpressionNode::Name(name) if name.symbol == definition.symbol)
            }
        {
            return None;
        }
        (signature.return_type, false)
    };
    (return_type.is_valid()
        && program.primitive_type_reference(return_type).is_none()
        && !matches!(
            program.type_reference_table.type_reference(return_type),
            TypeReferenceNode::Unit
        )
        && ((program.type_multiplicity(return_type) == language_semantics::Multiplicity::Affine
            && validation::has_plain_owned_contents(program, return_type))
            || (ordinary
                && program.type_multiplicity(return_type)
                    == language_semantics::Multiplicity::Unrestricted
                && validation::is_closed_primitive_array_type(program, return_type))))
    .then_some(return_type)
}
