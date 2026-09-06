//! Nested structural operands retain captured call coordinates and source scope.

use super::*;
use checked_trees::FlowFacts;

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
            let mut targets = program.machines().iter().filter_map(|owner| {
                let target = program.machine_states(owner).first()?;
                (target.symbol == call.target_symbol).then_some((owner, target))
            });
            let (owner, target) = targets.next()?;
            if targets.next().is_some()
                || owner.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                || !target.return_type.is_valid()
                || program
                    .primitive_type_reference(target.return_type)
                    .is_some()
                || matches!(
                    program
                        .type_reference_table
                        .type_reference(target.return_type),
                    TypeReferenceNode::Unit
                )
                || program.type_multiplicity(target.return_type)
                    != language_semantics::Multiplicity::Affine
                || !validation::has_plain_owned_contents(program, target.return_type)
                || !program.call_has_no_runtime_receiver(call, owner, target)
                || source.has_receiver != call.receiver.is_valid()
                || source.receiver_symbol
                    != if call.receiver.is_valid() {
                        owner.attached_data_symbol
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
