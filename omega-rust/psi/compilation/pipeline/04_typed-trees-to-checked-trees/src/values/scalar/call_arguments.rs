//! Nested structural operands retain captured call coordinates and source scope.
use crate::values::scalar::call_lowering::lower_call_arguments;
use crate::values::scalar::call_lowering::retain_call_arguments;
use crate::values::scalar::expression_plans::ScalarLocal;
use crate::values::scalar::scalar_lowering::lower_return_expression;
use checked_trees::CheckedLocatedScalarExpression;
use checked_trees::CheckedOperatorFacts;
use checked_trees::CheckedScalarExpressionBindings;
use checked_trees::CheckedScalarExpressionPlans;
use checked_trees::CheckedScalarExpressionRole;
use checked_trees::FlowFacts;
use typed_trees::expression::ExpressionHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;
use typed_trees::statement::TransitionGuardNode;
use typed_trees::types::TypeReferenceHandle;

pub(crate) fn is_scalar_return_call(
    program: &dyn ::validation::CheckingInput,
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
    let Some(call) = program.expression_table.call_node(expression) else {
        return false;
    };
    let target = validation::call_target_symbol(program, call);
    validation::call_target_return_type(program, target).is_some_and(|return_type| {
        program.primitive_type_reference(return_type) == Some(primitive_type)
    })
}
pub(crate) fn retain_nested_structural_call_arguments(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    flow: &FlowFacts,
    plans: &mut CheckedScalarExpressionPlans,
    proof_terms: &mut Vec<checked_trees::CheckedLocatedProofTerm>,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let parameters = program
                .state_parameters(state)
                .iter()
                .filter(|parameter| {
                    crate::values::scalar::occupies_scalar_position(program, parameter)
                })
                .cloned()
                .collect::<Vec<_>>();
            let parameter_types = parameters
                .iter()
                .map(|parameter| program.primitive_type_reference(parameter.type_reference))
                .collect::<Option<Vec<_>>>()
                .expect("filtered scalar parameters retain primitive carriers");
            let mut locals = Vec::new();
            let captured =
                crate::values::scalar::array_constructions::unique_state_flow(flow, machine, state);
            for (statement_index, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                let Ok(statement_ordinal) = u32::try_from(statement_index) else {
                    continue;
                };
                for construction in captured
                    .map(|captured| {
                        crate::values::scalar::array_constructions::call_array_constructions_in(
                            program,
                            flow,
                            captured,
                            machine,
                            state,
                            statement_index,
                        )
                    })
                    .unwrap_or_default()
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
                                            .filter(|local| local.is_value_binding())
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
                for (call_ordinal, site) in captured
                    .map(|captured| {
                        nested_structural_call_sites(
                            program,
                            flow,
                            captured,
                            machine,
                            statement,
                            statement_index,
                        )
                    })
                    .unwrap_or_default()
                {
                    if let Some(arguments) = lower_call_arguments(
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
                    ) {
                        retain_call_arguments(
                            arguments,
                            &parameters,
                            &locals,
                            &mut plans.expressions,
                            proof_terms,
                            &mut plans.source_bindings,
                            &mut plans.binding_symbols,
                        );
                    }
                }
                // Every operand of this statement sees the same declarations;
                // the enclosing initializer becomes available only afterward.
                if let Some(local) = statement.as_local_data() {
                    if let Some(primitive_type) =
                        program.primitive_type_reference(local.type_reference)
                    {
                        locals.push(ScalarLocal::from_local_data(program, local, primitive_type));
                    } else if let Some(binding) =
                        super::primitive_reference_read::local(program, local)
                    {
                        locals.push(binding);
                    }
                }
            }
        }
    }
}

/// Read captured occurrences directly. In particular, skipped syntax does not
/// acquire a flow call, and execution order never becomes occurrence identity.
/// `source` is the state's unique flow fact, which the caller resolves once
/// per state.
pub(crate) fn nested_structural_call_sites<'program>(
    program: &'program dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    source: &checked_trees::FlowStateFact,
    machine: &typed_trees::machine::Machine,
    statement: &StatementNode,
    statement_index: usize,
) -> Vec<(usize, validation::CallSite<'program>)> {
    let Some(calls) = flow.control.calls.span(source.calls) else {
        return Vec::new();
    };
    // A transition's guarded call is the statement's first call site yet its
    // arguments belong to the guard computation, not to the statement kind's
    // own root roster: no planner mints its operand rows elsewhere, and its
    // scalar result keeps it out of the structural-return admission below.
    let transition_guard = match statement {
        StatementNode::Transition(transition) => match transition.guard {
            TransitionGuardNode::When(expression) => Some(expression),
            TransitionGuardNode::Always => None,
        },
        _ => None,
    };
    let state = program.state_in_machine(machine, source.state_symbol);
    calls
        .iter()
        .filter_map(|source| {
            let is_transition_guard = transition_guard == Some(source.authored_expression);
            // The first computed case subject also owns ordinary argument
            // roots. Its surrounding case test is not the call expression;
            // later guard and payload copies only observe the saved result.
            let is_saved_subject = source.call_ordinal == 0
                && state.is_some_and(|state| {
                    validation::saved_dispatch_subject(
                        program,
                        state,
                        statement_index,
                        source.authored_expression,
                    ) == Some((statement_index, source.authored_expression))
                });
            if source.statement_index != statement_index
                || (source.call_ordinal == 0 && !is_transition_guard && !is_saved_subject)
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
            if validation::call_target_symbol(program, call) != source.target_symbol
                || !call.carries_only_positional_arguments()
                || !call.selects_only_nominal_route()
            {
                return None;
            }
            if !is_transition_guard {
                nested_structural_call_return_type(program, machine.symbol, call)?;
            }
            if source.has_receiver != call.receiver.is_valid()
                || source.receiver_symbol
                    != if call.receiver.is_valid() {
                        let name = program.expression_table.name_path_node(call.receiver)?;
                        name.symbol
                    } else {
                        symbols::SymbolHandle::invalid()
                    }
            {
                return None;
            }
            Some((
                source.call_ordinal,
                validation::CallSite::Expression {
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
/// Borrow-free owned storage can carry nominal cleanup: its consuming
/// continuation must select the hook, rather than exclude the producer here.
pub(crate) fn nested_structural_call_return_type(
    program: &dyn ::validation::CheckingInput,
    caller: symbols::SymbolHandle,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<TypeReferenceHandle> {
    if !call.target_symbol.is_valid()
        || !call.carries_only_positional_arguments()
        || !call.selects_only_nominal_route()
    {
        return None;
    }
    let mut targets = program.machines().iter().filter_map(|owner| {
        let target = program.machine_states(owner).first()?;
        (target.symbol == call.target_symbol).then_some((owner, target))
    });
    let return_type = if let Some(slot) = program.dynamic_slot_by_symbol(call.target_symbol) {
        // The instantiated requirement owns this completed value's signature;
        // runtime selection never picks a representative implementation here.
        // Returned loans still need a public slot-relative origin contract.
        if !validation::has_contents(
            program,
            slot.return_type,
            &[],
            validation::ContentsRequirement::BorrowFree,
        ) {
            return None;
        }
        slot.return_type
    } else if let Some((owner, target)) = targets.next() {
        if targets.next().is_some()
            || !(owner.supplies_checked_body() || owner.has_bodyless_signature())
            || !program.call_has_no_runtime_receiver(call, owner, target)
        {
            return None;
        }
        target.return_type
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
        let (_, signature) = signatures.next()?;
        if signatures.next().is_some() {
            return None;
        }
        // The exact requirement owns the result type for every operand
        // spelling. Its receiver retains the explicit Binding authority after
        // source normalization, not a trait-name expression. Rechecking that
        // old shape excluded nested boundary results even though the ordinary
        // call planner already joins the selected receiver and its custody.
        // Type discovery grants neither provider selection nor call authority.
        signature.return_type
    };
    // A completed operand uses the same carrier judgment as a named result.
    // Body ownership selects execution and admission later, not whether the
    // signature's descriptor can participate in an enclosing expression.
    crate::execution::terminal_unit::control::checked_structural_result_type(
        program,
        &mut crate::execution::terminal_unit::types::ShapeCollector::new(program),
        return_type,
        &[],
    )?;
    Some(return_type)
}
