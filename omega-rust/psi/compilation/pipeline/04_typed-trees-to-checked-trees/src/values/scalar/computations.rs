//! Checked execution plans for call-bearing scalar writes, guards, arguments, and returns.
//!
//! A known normal-return result is not an effect-free computation. Short-circuit
//! selection may omit an unreachable RHS, but must retain the evaluated left
//! graph, including calls whose result becomes known only at a later selection.
//! Keep exact source occurrences on retained applications and selections so
//! folding an enclosing guard does not change their operand custody.
//! One builder owns both plan arenas: structural members may compute scalar
//! operands, and scalar observations may first establish a structural value.
use crate::values::operator_is_builtin;
use crate::values::scalar::expression_plans::ScalarLocal;
use crate::values::scalar::semantic_casts;
use checked_trees::CheckedBooleanExpression;
use checked_trees::CheckedOperatorFacts;
use checked_trees::CheckedScalarExpression;
use checked_trees::CheckedScalarExpressionPlans;
use checked_trees::CheckedScalarExpressionRole;
use checked_trees::{
    CheckedScalarComputationHandle, CheckedScalarComputationKind, CheckedScalarComputationPlans,
    FlowFacts, ProofFacts,
};
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;
use typed_trees::expression::BinaryOperator;
use typed_trees::expression::ExpressionHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::expression::UnaryOperator;
use typed_trees::statement::StatementNode;
use typed_trees::statement::TransitionTargetNode;
use typed_trees::types::PrimitiveType;
use typed_trees::types::TypeReferenceNode;

mod boundary_calls;
mod byte_comparisons;
mod call_arguments;
mod cases;
mod dispatch;
mod dynamic_calls;
mod enum_equality;
mod integers;
mod normal_return;
mod slice_view;
mod structural_fields;
mod structural_values;

use builder::Builder;
mod builder;

pub(crate) fn build_checked_value_computation_plans(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    flow: &FlowFacts,
    borrow: &checked_trees::BorrowFacts,
    proof: &ProofFacts,
    pure: &CheckedScalarExpressionPlans,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> (
    CheckedScalarComputationPlans,
    checked_trees::CheckedStructuralValuePlans,
) {
    let mut plans = CheckedScalarComputationPlans::default();
    let mut structural_values = checked_trees::CheckedStructuralValuePlans::default();
    for machine in program.machines() {
        // Existing named-output emission joins statement binding positions.
        // It must not silently reinterpret computation-local call positions.
        if proof.proof_output_calls.iter().any(|(_, call)| {
            call.caller_machine_symbol == machine.symbol && call.runtime_call.is_some()
        }) {
            continue;
        }
        let states = program.machine_states(machine);
        for state in states {
            let scalar_parameters = program
                .state_parameters(state)
                .iter()
                .filter(|parameter| {
                    crate::values::scalar::occupies_scalar_position(program, parameter)
                })
                .cloned()
                .collect::<Vec<_>>();
            let parameters = scalar_parameters.as_slice();
            let Some(parameter_types) = parameters
                .iter()
                .map(|parameter| program.primitive_type_reference(parameter.type_reference))
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            let mut locals = Vec::new();
            let captured =
                crate::values::scalar::array_constructions::unique_state_flow(flow, machine, state);
            let statements = program.statement_table.statements(state.statement_nodes);
            for (statement_index, statement) in statements.iter().enumerate() {
                let Ok(statement_ordinal) = u32::try_from(statement_index) else {
                    continue;
                };
                let mut builder = Builder {
                    program,
                    operators,
                    flow,
                    borrow,
                    pure,
                    exact_integer_casts,
                    machine: machine.symbol,
                    state: state.symbol,
                    statement_index,
                    parameters,
                    authored_parameters: program.state_parameters(state),
                    parameter_types: &parameter_types,
                    locals: &locals,
                    plans: &mut plans,
                    structural_values: &mut structural_values,
                    integer_occurrences: Vec::new(),
                };
                // Endpoints are ordinary operands. Capture every occurrence
                // before constructing consumers, so a call result and a pure
                // bound supply the same range operation at its authored site.
                for (expression, site) in super::subslice_endpoints::sites(
                    program,
                    machine,
                    state,
                    statement_index,
                    statement,
                )
                .into_iter()
                .rev()
                {
                    for (endpoint, role, ()) in super::subslice_endpoints::subslice_endpoints(
                        program,
                        operators,
                        expression,
                        site,
                        |_| Some(()),
                    ) {
                        builder.record_root(
                            pure,
                            statement_ordinal,
                            role,
                            endpoint,
                            PrimitiveType::U64,
                        );
                    }
                }
                let construction_destination = match statement {
                    StatementNode::LocalData(local) => {
                        Some((local.initial_value, local.type_reference))
                    }
                    StatementNode::Expression(expression) => Some((*expression, state.return_type)),
                    _ => None,
                };
                // A closed primitive array whose elements the scalar route
                // enumerates owns one scalar root per element.
                let scalar_elements =
                    construction_destination.and_then(|(expression, expected)| {
                        validation::scalar_array_elements(
                            program,
                            machine.symbol,
                            expression,
                            expected,
                        )
                    });
                // Destination type selects the value owner, not the authored
                // source shape. The structural builder owns construction,
                // projections and reference descriptors equally in a local
                // initializer and a return; primitive destinations stay with
                // the scalar route unless they carry a checked case value.
                // Each construction has one owner: element calls claimed by
                // scalar element roots cannot also be structural operands.
                if let Some((expression, expected)) = construction_destination
                    && scalar_elements.is_none()
                    && (validation::is_scalar_case_value(program, expression, expected)
                        || program.primitive_type_reference(expected).is_none())
                    && let Some(root) = builder.structural_value(expression, expected)
                {
                    builder.push_structural_root(statement_ordinal, expression, expected, root);
                }
                // An uninitialized local of a zero-constructible
                // type mints its establishment here: the whole declared
                // shape begins zero and later element or field stores write
                // it. The root keys on the invalid initializer handle, which
                // the local plan arm matches by equality. Whole-local
                // rebinding permission does not govern member establishment.
                if let Some(local) = statement.as_local_data()
                    && program
                        .primitive_type_reference(local.type_reference)
                        .is_none()
                    && !program
                        .expression_table
                        .expression_is_valid(local.initial_value)
                    && validation::zeroed_structural_parts(program, local.type_reference)
                {
                    let root = builder.structural_values.nodes.append(
                        checked_trees::CheckedStructuralValue {
                            machine: machine.symbol,
                            state: state.symbol,
                            statement_index: statement_ordinal,
                            expression: local.initial_value,
                            type_reference: local.type_reference,
                            qualification_decisions: Default::default(),
                            kind: checked_trees::CheckedStructuralValueKind::ZeroedStructural {},
                        },
                    );
                    builder.push_structural_root(
                        statement_ordinal,
                        local.initial_value,
                        local.type_reference,
                        root,
                    );
                }
                if let Some(elements) = scalar_elements {
                    for (element_index, (element, primitive_type)) in
                        elements.elements.into_iter().enumerate()
                    {
                        let Ok(element_ordinal) = u32::try_from(element_index) else {
                            break;
                        };
                        builder.record_root(
                            pure,
                            statement_ordinal,
                            CheckedScalarExpressionRole::ArrayElement {
                                source: checked_trees::CheckedArrayConstructionSource::Statement,
                                element_ordinal,
                            },
                            element,
                            primitive_type,
                        );
                    }
                }
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
                    builder.retain_structural_argument(
                        construction.expression,
                        construction.type_reference,
                    );
                }
                for (call_ordinal, site) in captured
                    .map(|captured| {
                        super::call_arguments::nested_structural_call_sites(
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
                    let validation::CallSite::Expression { call, .. } = site else {
                        continue;
                    };
                    let Ok(call_ordinal) = u32::try_from(call_ordinal) else {
                        continue;
                    };
                    builder.record_call_arguments(
                        statement_ordinal,
                        call_ordinal,
                        validation::call_target_symbol(program, call),
                        program.expression_table.expression_handles(call.arguments),
                    );
                }
                if let Some(local) = statement.as_local_data() {
                    let structural_destination = program
                        .primitive_type_reference(local.type_reference)
                        .is_none();
                    // A scalar boundary application tree (`min(a * b, 2.5)`,
                    // including a named operator) is also a result operation
                    // whose nested applications are computed operands.
                    let argument_roots = (!local.is_mutable || structural_destination)
                        && (validation::result_initializer_call_is_supported(
                            program,
                            machine,
                            local.initial_value,
                        ) || validation::nested_boundary_applications_compose(
                            program,
                            local.initial_value,
                        ));
                    if let Some(call) = program.expression_table.call_node(local.initial_value) {
                        // Operands retain their own construction and evaluation
                        // regardless of which operation owns the call's result.
                        // Keep scalar whole-call ownership in argument_roots below;
                        // retaining operands does not establish a result operation.
                        builder.record_call_arguments(
                            statement_ordinal,
                            0,
                            validation::call_target_symbol(program, call),
                            program.expression_table.expression_handles(call.arguments),
                        );
                    }
                    if let Some(primitive_type) =
                        program.primitive_type_reference(local.type_reference)
                    {
                        let binding_ordinal = u32::try_from(
                            locals
                                .iter()
                                .filter(|local: &&ScalarLocal| local.is_value_binding())
                                .count(),
                        );
                        if let Ok(binding_ordinal) = binding_ordinal {
                            let role = if local.is_mutable {
                                CheckedScalarExpressionRole::StorageInitializer
                            } else {
                                CheckedScalarExpressionRole::LocalInitializer { binding_ordinal }
                            };
                            // A result operation owns its outer call; otherwise the
                            // ordinary computation must retain it even when all
                            // arguments are pure. Purity of operands cannot route
                            // a call away from state-local operation sequencing.
                            if !argument_roots {
                                builder.record_root(
                                    pure,
                                    statement_ordinal,
                                    role,
                                    local.initial_value,
                                    primitive_type,
                                );
                            }
                        }
                        // The initializer may read earlier locals, never its own binding.
                        locals.push(ScalarLocal::from_local_data(program, local, primitive_type));
                    } else if let Some(binding) =
                        super::primitive_reference_read::local(program, local)
                    {
                        locals.push(binding);
                    }
                    continue;
                }
                if let Some(call) = statement.as_call() {
                    builder.record_call_arguments(
                        statement_ordinal,
                        0,
                        call.target_symbol,
                        program.statement_table.expression_handles(call.arguments),
                    );
                    continue;
                }
                if let Some(expression) = statement.as_expression()
                    && let ExpressionNode::Call(call) =
                        program.expression_table.expression(*expression)
                {
                    builder.record_call_arguments(
                        statement_ordinal,
                        0,
                        validation::call_target_symbol(program, call),
                        program.expression_table.expression_handles(call.arguments),
                    );
                    if validation::unit_statement_call_is_supported(
                        program,
                        machine,
                        state,
                        *expression,
                    ) {
                        continue;
                    }
                }
                if let Some(assignment) = statement.as_assignment() {
                    // A constructed assignment value is the same structural
                    // constructor a `let` initializer roots. Each scalar field
                    // initializer of a record keeps its own `RecordField`
                    // computation coordinate for the decomposed store route;
                    // a case literal, or a copy of an `Unrestricted` place
                    // (`self.copy = self.items[0]` over a `[copy]` record), is
                    // established whole and replaces its field through the
                    // statement sequence's window pair. An array literal of
                    // structural elements registers each element as its own
                    // value node, as a `let` initializer does; the sequence
                    // establishes the elements and replaces the array one
                    // element window at a time.
                    // Keep the destination's complete declared type. Carrier
                    // classifiers may inspect its underlying shape, but value
                    // custody must still distinguish arithmetic policies and
                    // reference shells when comparing the source and result.
                    // Assignment through a reference writes its referent, so
                    // `referent = Sum::Case` through `referent: &mut Sum`
                    // roots a value of the referee, which the sequence then
                    // stores through the loan's whole-referent window.
                    let written = validation::expression_type_reference_in_state(
                        program,
                        state.symbol,
                        statement_index,
                        assignment.target,
                    )
                    .map(|destination| validation::assignment_value_type(program, destination));
                    if let Some(expected) = written
                        && (structural_values::is_record_value(program, assignment.value, expected)
                            || validation::is_scalar_case_value(
                                program,
                                assignment.value,
                                expected,
                            )
                            || structural_values::is_structural_case_value(
                                program,
                                assignment.value,
                                expected,
                            )
                            || structural_values::needs_copied_place_root(
                                program,
                                state.symbol,
                                statement_index,
                                assignment.value,
                                expected,
                            )
                            || structural_values::is_fixed_array_value(
                                program,
                                assignment.value,
                                expected,
                            ))
                        && let Some(root) = builder.structural_value(assignment.value, expected)
                    {
                        builder.push_structural_root(
                            statement_ordinal,
                            assignment.value,
                            expected,
                            root,
                        );
                    }
                    if matches!(
                        program.expression_table.expression(assignment.target),
                        ExpressionNode::Member(_)
                    ) && let Some(primitive_type) = validation::declared_place_type_raw(
                        program,
                        machine,
                        Some(state),
                        assignment.target,
                    )
                    .and_then(|reference| program.primitive_type_reference(reference))
                    {
                        builder.record_root(
                            pure,
                            statement_ordinal,
                            CheckedScalarExpressionRole::AssignmentValue,
                            assignment.value,
                            primitive_type,
                        );
                    }
                    // An indexed store evaluates each selector of its
                    // target before its value. The pure plan already keeps
                    // the same `AssignmentIndex { depth }`/`AssignmentValue`
                    // coordinates for the operands it can carry; a range
                    // index is a subslice endpoint pair, not a selector.
                    for (depth, index) in
                        validation::assignment_target_selectors(program, assignment.target)
                            .into_iter()
                            .enumerate()
                    {
                        let Ok(depth) = u32::try_from(depth) else {
                            break;
                        };
                        builder.record_root(
                            pure,
                            statement_ordinal,
                            CheckedScalarExpressionRole::AssignmentIndex { depth },
                            index,
                            PrimitiveType::U64,
                        );
                    }
                    if matches!(
                        program.expression_table.expression(assignment.target),
                        ExpressionNode::Indexed(_)
                    ) && let Some(primitive_type) =
                            validation::expression_type_reference_in_state(
                                program,
                                state.symbol,
                                statement_index,
                                assignment.target,
                            )
                            .and_then(|reference| {
                                crate::values::scalar::expression_plans::assignment_target_primitive_type(
                                    program,
                                    reference,
                                )
                            })
                        {
                            builder.record_root(
                                pure,
                                statement_ordinal,
                                CheckedScalarExpressionRole::AssignmentValue,
                                assignment.value,
                                primitive_type,
                            );
                        }
                    if let ExpressionNode::Name(name) =
                        program.expression_table.expression(assignment.target)
                        && name.symbol.is_valid()
                        && name.head_symbol == name.symbol
                        && let Some(primitive_type) = locals
                            .iter()
                            .find(|local| local.symbol == name.symbol && local.is_mutable)
                            .map(|local| local.primitive_type)
                            .or_else(|| {
                                // Storage destinations use the authored parameter roster;
                                // dense scalar inputs deliberately exclude reference parameters.
                                program.state_parameter_by_symbol(state, name.symbol)
                                    .filter(|parameter| {
                                        (parameter.is_mutable)
                                            && !parameter.is_const
                                    })
                                    .and_then(|parameter| {
                                        crate::values::scalar::expression_plans::assignment_target_primitive_type(
                                            program,
                                            parameter.type_reference,
                                        )
                                    })
                            })
                    {
                        // The completed RHS replaces storage only after evaluation.
                        // Reads here retain the existing destination symbol; assignments
                        // neither append an immutable local nor change its namespace.
                        builder.record_root(
                            pure,
                            statement_ordinal,
                            CheckedScalarExpressionRole::AssignmentValue,
                            assignment.value,
                            primitive_type,
                        );
                    }
                    if let ExpressionNode::Call(call) =
                        program.expression_table.expression(assignment.value)
                        && builder
                            .plans
                            .root_at(
                                state.symbol,
                                statement_ordinal,
                                CheckedScalarExpressionRole::AssignmentValue,
                            )
                            .is_none()
                    {
                        // A whole RHS computation already owns its nested calls.
                        // Otherwise the statement call owns the result store and
                        // needs the same argument roots as an initializer, including
                        // copied records that cannot live in scalar computations.
                        builder.record_call_arguments(
                            statement_ordinal,
                            0,
                            validation::call_target_symbol(program, call),
                            program.expression_table.expression_handles(call.arguments),
                        );
                    }
                    continue;
                }
                if let Some(expression) = statement.as_expression()
                    && statement_index + 1 == statements.len()
                    && let Some(result_type) = program.primitive_type_reference(state.return_type)
                {
                    builder.record_root(
                        pure,
                        statement_ordinal,
                        CheckedScalarExpressionRole::Return,
                        *expression,
                        result_type,
                    );
                }
                let Some(transition) = statement.as_transition() else {
                    continue;
                };
                // Named machine invocations use the same argument roles as
                // expression calls. Their captured preorder coordinate is
                // independent of guard/operand evaluation order; internal
                // state transfers keep the transition roles below instead.
                if let Some(captured) = captured {
                    for call in flow
                        .control
                        .calls
                        .span_or_empty(captured.calls)
                        .iter()
                        .filter(|call| call.statement_index == statement_index)
                    {
                        let Some(validation::CallSite::TransitionNamed { arguments, .. }) =
                            validation::find_call_site(
                                program,
                                machine.symbol,
                                state.symbol,
                                statement_index,
                                call.call_ordinal,
                            )
                        else {
                            continue;
                        };
                        // The exact callable contract owns argument roles for
                        // bodies and requirements alike. Current-activation
                        // state edges keep the transition roles below.
                        if validation::terminal_callable_target(
                            program,
                            machine,
                            call.target_symbol,
                        )
                        .is_none()
                        {
                            continue;
                        }
                        let Ok(call_ordinal) = u32::try_from(call.call_ordinal) else {
                            continue;
                        };
                        builder.record_call_arguments(
                            statement_ordinal,
                            call_ordinal,
                            call.target_symbol,
                            program.statement_table.expression_handles(arguments),
                        );
                    }
                }
                if let typed_trees::statement::TransitionGuardNode::When(expression) =
                    transition.guard
                {
                    builder.record_root(
                        pure,
                        statement_ordinal,
                        CheckedScalarExpressionRole::Guard,
                        expression,
                        PrimitiveType::Bool,
                    );
                }
                for (target, continuation) in
                    [(transition.target, false), (transition.continuation, true)]
                {
                    if !target.is_valid() {
                        continue;
                    }
                    if transition.exit == typed_trees::statement::TransitionExit::Ordinary
                        && let TransitionTargetNode::Value(expression) =
                            program.statement_table.transition_target(target)
                        && program
                            .primitive_type_reference(state.return_type)
                            .is_none()
                        && let Some(root) = builder.structural_value(*expression, state.return_type)
                    {
                        builder.push_structural_root(
                            statement_ordinal,
                            *expression,
                            state.return_type,
                            root,
                        );
                    }
                    if transition.exit == typed_trees::statement::TransitionExit::Ordinary
                        && let TransitionTargetNode::Value(expression) =
                            program.statement_table.transition_target(target)
                        && let Some(result_type) =
                            program.primitive_type_reference(state.return_type)
                    {
                        let role = if continuation {
                            CheckedScalarExpressionRole::ContinuationReturn
                        } else {
                            CheckedScalarExpressionRole::Return
                        };
                        builder.record_root(
                            pure,
                            statement_ordinal,
                            role,
                            *expression,
                            result_type,
                        );
                    }
                    let TransitionTargetNode::Named {
                        path,
                        arguments,
                        evidence_arguments,
                        ..
                    } = program.statement_table.transition_target(target)
                    else {
                        continue;
                    };
                    if !evidence_arguments.is_empty() {
                        continue;
                    }
                    let Some(target_state) =
                        crate::checks::termination::named_transition_target_state_index(
                            program,
                            machine,
                            path.symbol,
                        )
                        .and_then(|target_index| states.get(target_index))
                    else {
                        continue;
                    };
                    // Authored actuals exclude the implicit receiver. Each
                    // argument keeps its full target parameter position so the
                    // recorded role joins the pure expression plan, the saved
                    // jump-argument capture, and every downstream consumer on
                    // the same coordinate.
                    let target_parameters =
                        program.state_parameters(target_state).iter().enumerate();
                    let arguments = program.statement_table.expression_handles(*arguments);
                    for (argument, (target_position, parameter)) in
                        arguments.iter().zip(target_parameters)
                    {
                        let Ok(argument_ordinal) = u32::try_from(target_position) else {
                            continue;
                        };
                        let role = if continuation {
                            CheckedScalarExpressionRole::TransitionContinuationArgument {
                                argument_ordinal,
                            }
                        } else {
                            CheckedScalarExpressionRole::TransitionArgument { argument_ordinal }
                        };
                        let Some(expected_type) =
                            program.primitive_type_reference(parameter.type_reference)
                        else {
                            // A composite argument needing fresh storage — a
                            // literal, indexed copy, or call result — is
                            // established only on its selected edge. Indexed
                            // copies retain selector evaluation in the value
                            // plan; field-only copies and whole-root transfers
                            // retain their existing direct custody plans.
                            // Retain its structural value so the successor
                            // plan can name it as an established source; the
                            // builder admits a call only when its declared
                            // result is exactly the parameter's type.
                            if !continuation
                                && transition.exit
                                    == typed_trees::statement::TransitionExit::Ordinary
                                && (structural_values::is_record_value(
                                    program,
                                    *argument,
                                    parameter.type_reference,
                                ) || structural_values::is_borrowed_slice_view_value(
                                    program,
                                    parameter.type_reference,
                                ) || structural_values::is_structural_case_value(
                                    program,
                                    *argument,
                                    parameter.type_reference,
                                ) || (!crate::execution::terminal_unit::types::is_reference(
                                    program,
                                    parameter.type_reference,
                                ) && structural_values::needs_copied_place_root(
                                    program,
                                    state.symbol,
                                    statement_index,
                                    *argument,
                                    parameter.type_reference,
                                )
                                    && validation::canonical_place_from_expression_in_state(
                                        program,
                                        state.symbol,
                                        statement_index,
                                        *argument,
                                    )
                                    .is_some_and(|place| {
                                        place.segments.iter().any(|segment| {
                                            matches!(
                                                segment,
                                                facts::PlaceSegment::Index { .. }
                                                    | facts::PlaceSegment::FixedIndex { .. }
                                            )
                                        })
                                    }))
                                    || matches!(
                                        program.expression_table.expression(*argument),
                                        ExpressionNode::ArrayLiteral(_) | ExpressionNode::Call(_)
                                    ))
                                && builder
                                    .structural_values
                                    .root_for_expression(state.symbol, statement_ordinal, *argument)
                                    .is_none()
                                && let Some(root) =
                                    builder.structural_value(*argument, parameter.type_reference)
                            {
                                builder.push_structural_root(
                                    statement_ordinal,
                                    *argument,
                                    parameter.type_reference,
                                    root,
                                );
                            }
                            continue;
                        };
                        builder.record_root(
                            pure,
                            statement_ordinal,
                            role,
                            *argument,
                            expected_type,
                        );
                    }
                }
            }
        }
    }
    (plans, structural_values)
}
