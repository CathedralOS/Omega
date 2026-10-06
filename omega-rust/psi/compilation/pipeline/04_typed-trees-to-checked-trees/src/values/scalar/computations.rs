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
use crate::values::scalar::expression_facts::is_integer;
use crate::values::scalar::expression_plans::ScalarLocal;
use crate::values::scalar::scalar_lowering::lower_return_expression;
use crate::values::scalar::semantic_casts;
use crate::values::scalar_expression_type;
use checked_trees::CheckedBooleanExpression;
use checked_trees::CheckedOperatorFacts;
use checked_trees::CheckedScalarExpression;
use checked_trees::CheckedScalarExpressionPlans;
use checked_trees::CheckedScalarExpressionRole;
use checked_trees::{
    CheckedScalarComputation, CheckedScalarComputationHandle, CheckedScalarComputationKind,
    CheckedScalarComputationPlans, CheckedScalarComputationRoot, FlowFacts, ProofFacts,
};
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::BinaryOperator;
use typed_trees::expression::ExpressionHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::expression::TableCastExpression;
use typed_trees::expression::UnaryOperator;
use typed_trees::signature::StateParameter;
use typed_trees::statement::StatementNode;
use typed_trees::statement::TransitionTargetNode;
use typed_trees::types::PrimitiveType;
use typed_trees::types::TypeReferenceNode;

mod boundary_calls;
mod byte_comparisons;
mod call_arguments;
mod cases;
mod dispatch;
mod enum_equality;
mod integers;
mod normal_return;
mod slice_view;
mod structural_fields;
mod structural_values;

pub(crate) fn build_checked_value_computation_plans(
    program: &TypedTrees,
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
                // An owned place is a structural value just like a
                // constructor operand, whether the authored expression names
                // the whole root or one of its projected children. Let the
                // same builder check its selected type and storage; local
                // copies and partial moves must not lose their operation
                // merely because they are the root of an initializer.
                //
                // What narrows this admission is the destination's owning
                // lane, not the shape of the place expression: a scalar
                // destination belongs to the primitive-local route, and a
                // whole returned reference to reference completion's affine
                // establishment and exact ingress map. Every other
                // destination is a structural carrier, and `structural_value`
                // remains the authority over which operation the read becomes
                // -- a copy, a borrow, or the projected selection transfer a
                // partial move out of a transparent record records. Reference
                // nodes remain valid inside record constructors. A
                // `let view: &T = &place` initializer is the same carrier a
                // selection arm admits; restricting that admission to
                // LocalData keeps a returned `&place` on its
                // reference-completion owner.
                if let Some((expression, expected)) = construction_destination
                    && (validation::is_scalar_case_value(program, expression, expected)
                        || (validation::reference_result_custody::parts(program, expected)
                            .is_none()
                            && program.primitive_type_reference(expected).is_none()
                            && matches!(
                                program.expression_table.expression(expression),
                                ExpressionNode::Name(_)
                                    | ExpressionNode::Member(_)
                                    | ExpressionNode::Indexed(_)
                            ))
                        || structural_values::is_record_value(program, expression, expected)
                        || structural_values::is_scalar_case_place_value(
                            program,
                            state.symbol,
                            statement_index,
                            expression,
                            expected,
                        )
                        || structural_values::is_structural_case_value(
                            program,
                            expression,
                            expected,
                        )
                        // A copyable `Unrestricted` leaf under a shared borrow
                        // is the same admission shape — the copy op observes
                        // where the case fan-out cannot reconstruct payloads.
                        || structural_values::needs_copied_place_root(
                            program,
                            state.symbol,
                            statement_index,
                            expression,
                            expected,
                        )
                        || (matches!(statement, StatementNode::LocalData(_))
                            && structural_values::is_shared_borrow_value(
                                program, expression, expected,
                            ))
                        // A shared or mutable slice local lends a collection the
                        // same way `&place` lends a record: its value is the
                        // lent place under the loan checked borrow admission
                        // already recorded, so it belongs to the same
                        // LocalData admission rather than to a producer family
                        // of its own.
                        || (matches!(statement, StatementNode::LocalData(_))
                            && structural_values::is_borrowed_slice_view_value(program, expected))
                        // Array construction owns the same child values in
                        // a local initializer and a returned expression. Each
                        // element keeps its value node and declared type so
                        // completion can retain its loan custody as well.
                        || structural_values::is_fixed_array_value(
                            program,
                            expression,
                            expected,
                        ))
                    && let Some(root) = builder.structural_value(expression, expected)
                {
                    builder.structural_values.roots.append(
                        checked_trees::CheckedStructuralValueRoot {
                            machine: machine.symbol,
                            state: state.symbol,
                            statement_ordinal,
                            expression,
                            type_reference: expected,
                            root,
                        },
                    );
                }
                // An uninitialized local of a zero-constructible
                // type mints its establishment here: the whole declared
                // shape begins zero and later element or field stores write
                // it. The root keys on the invalid initializer handle, which
                // the local plan arm matches by equality. Whole-local
                // rebinding permission does not govern member establishment.
                if let StatementNode::LocalData(local) = statement
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
                            expression: local.initial_value,
                            kind: checked_trees::CheckedStructuralValueKind::ZeroedStructural {},
                        },
                    );
                    builder.structural_values.roots.append(
                        checked_trees::CheckedStructuralValueRoot {
                            machine: machine.symbol,
                            state: state.symbol,
                            statement_ordinal,
                            expression: local.initial_value,
                            type_reference: local.type_reference,
                            root,
                        },
                    );
                }
                if let Some((expression, expected)) = construction_destination
                    && let Some(elements) = validation::scalar_array_elements(
                        program,
                        machine.symbol,
                        expression,
                        expected,
                    )
                {
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
                    let crate::semantic::calls::CallSite::Expression { call, .. } = site else {
                        continue;
                    };
                    let Ok(call_ordinal) = u32::try_from(call_ordinal) else {
                        continue;
                    };
                    builder.record_call_arguments(
                        statement_ordinal,
                        call_ordinal,
                        crate::flow::call_target_symbol(program, call),
                        program.expression_table.expression_handles(call.arguments),
                    );
                }
                if let StatementNode::LocalData(local) = statement {
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
                    if let ExpressionNode::Call(call) =
                        program.expression_table.expression(local.initial_value)
                    {
                        // Operands retain their own construction and evaluation
                        // regardless of which operation owns the call's result.
                        // Keep scalar whole-call ownership in argument_roots below;
                        // retaining operands does not establish a result operation.
                        builder.record_call_arguments(
                            statement_ordinal,
                            0,
                            crate::flow::call_target_symbol(program, call),
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
                        locals.push(ScalarLocal {
                            is_mutable: local.is_mutable,
                            symbol: local.symbol,
                            name: local.name.as_str().to_owned(),
                            primitive_type,
                            arithmetic_domain: program
                                .arithmetic_domain_for_type_reference(local.type_reference),
                            is_reference: false,
                        });
                    } else if let Some(binding) =
                        super::primitive_reference_read::local(program, local)
                    {
                        locals.push(binding);
                    }
                    continue;
                }
                if let StatementNode::Call(call) = statement {
                    builder.record_call_arguments(
                        statement_ordinal,
                        0,
                        call.target_symbol,
                        program.statement_table.expression_handles(call.arguments),
                    );
                    continue;
                }
                if let StatementNode::Expression(expression) = statement
                    && let ExpressionNode::Call(call) =
                        program.expression_table.expression(*expression)
                {
                    builder.record_call_arguments(
                        statement_ordinal,
                        0,
                        crate::flow::call_target_symbol(program, call),
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
                if let StatementNode::Assignment(assignment) = statement {
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
                    let written = crate::flow::expression_type_reference_in_state(
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
                        builder.structural_values.roots.append(
                            checked_trees::CheckedStructuralValueRoot {
                                machine: machine.symbol,
                                state: state.symbol,
                                statement_ordinal,
                                expression: assignment.value,
                                type_reference: expected,
                                root,
                            },
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
                            crate::flow::expression_type_reference_in_state(
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
                                program
                                    .state_parameters(state)
                                    .iter()
                                    .find(|parameter| parameter.symbol == name.symbol)
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
                            crate::flow::call_target_symbol(program, call),
                            program.expression_table.expression_handles(call.arguments),
                        );
                    }
                    continue;
                }
                if let StatementNode::Expression(expression) = statement
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
                let StatementNode::Transition(transition) = statement else {
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
                        let Some(crate::semantic::calls::CallSite::TransitionNamed {
                            arguments,
                            ..
                        }) = crate::semantic::calls::find_call_site(
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
                        if crate::semantic::calls::terminal_callable_target(
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
                        builder.structural_values.roots.append(
                            checked_trees::CheckedStructuralValueRoot {
                                machine: machine.symbol,
                                state: state.symbol,
                                statement_ordinal,
                                expression: *expression,
                                type_reference: state.return_type,
                                root,
                            },
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
                                    && crate::flow::canonical_place_from_expression_in_state(
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
                                builder.structural_values.roots.append(
                                    checked_trees::CheckedStructuralValueRoot {
                                        machine: machine.symbol,
                                        state: state.symbol,
                                        statement_ordinal,
                                        expression: *argument,
                                        type_reference: parameter.type_reference,
                                        root,
                                    },
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

struct Builder<'program, 'plans> {
    program: &'program TypedTrees,
    operators: &'program CheckedOperatorFacts,
    flow: &'program FlowFacts,
    borrow: &'program checked_trees::BorrowFacts,
    pure: &'program CheckedScalarExpressionPlans,
    exact_integer_casts: &'program [validation::ExactIntegerCastFact],
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    parameters: &'program [StateParameter],
    authored_parameters: &'program [StateParameter],
    parameter_types: &'program [PrimitiveType],
    locals: &'program [ScalarLocal],
    plans: &'plans mut CheckedScalarComputationPlans,
    structural_values: &'plans mut checked_trees::CheckedStructuralValuePlans,
}

impl Builder<'_, '_> {
    fn record_root(
        &mut self,
        pure: &CheckedScalarExpressionPlans,
        statement_ordinal: u32,
        role: CheckedScalarExpressionRole,
        expression: ExpressionHandle,
        expected_type: PrimitiveType,
    ) {
        if pure
            .expression_at(self.state, statement_ordinal, role)
            .is_some()
            || self.statement_owns_boundary_call(role, expression)
        {
            return;
        }
        // Structural value traversal and the ordinary call-operand roster can
        // reach the same authored operand. Retain its computation once; a
        // different source or type at this coordinate must remain a conflicting
        // root for independent custody checking to reject.
        if let Some(existing) = self.plans.root_at(self.state, statement_ordinal, role)
            && existing.machine == self.machine
            && self.plans.nodes.is_valid(existing.root)
            && self.plans.nodes.get(existing.root).authored_root == expression
            && self.plans.nodes.get(existing.root).primitive_type == expected_type
        {
            return;
        }
        if let Some(root) = self.expression(expression, expected_type) {
            self.plans.nodes.get_mut(root).authored_root = expression;
            self.plans.roots.append(CheckedScalarComputationRoot {
                machine: self.machine,
                state: self.state,
                statement_ordinal,
                role,
                root,
            });
        }
    }

    /// An authored conversion that cannot travel the pure lane still needs an
    /// exact node at its operand coordinate: the qualification arms above
    /// admit only representation-identical transfers, while custody positions
    /// such as call arguments read their evidence from this graph. The
    /// operand lowers at its own primitive; the node carries the cast's
    /// destination carrier and authored domain for consumers and the
    /// qualification-facts rejoin.
    fn authored_cast(
        &mut self,
        expression: ExpressionHandle,
        cast: &TableCastExpression,
        expected_type: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        if !semantic_casts::requires_custody(self.program, self.state, expression) {
            return None;
        }
        let operand = if let Some(source_primitive) =
            semantic_casts::result_type(self.program, self.state, cast.value)
                .and_then(|reference| self.program.primitive_type_reference(reference))
        {
            self.expression(cast.value, source_primitive)?
        } else {
            // The explicit cast supplies the carrier for an anonymous integer
            // expression. Preserve its exact landing and source occurrence;
            // the qualification remains an authored cast with its own proof.
            let target = self.program.primitive_type_reference(cast.target_type)?;
            let value = super::scalar_lowering::land_anonymous_scalar_expression(
                self.program,
                self.operators,
                cast.value,
                target,
            )?;
            let operand = self.insert(target, CheckedScalarComputationKind::Value(value));
            self.plans.nodes.get_mut(operand).value_source = cast.value;
            operand
        };
        let result_type = semantic_casts::result_type(self.program, self.state, expression)?;
        Some(self.insert(
            expected_type,
            CheckedScalarComputationKind::AuthoredCast {
                source_expression: expression,
                operand,
                result_type,
                domain: cast.domain,
            },
        ))
    }

    fn insert(
        &mut self,
        primitive_type: PrimitiveType,
        kind: CheckedScalarComputationKind,
    ) -> CheckedScalarComputationHandle {
        self.plans.nodes.append(CheckedScalarComputation {
            value_source: ExpressionHandle::invalid(),
            authored_root: ExpressionHandle::invalid(),
            primitive_type,
            kind,
        })
    }

    fn boolean(&mut self, value: bool) -> CheckedScalarComputationHandle {
        self.insert(
            PrimitiveType::Bool,
            CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(Box::new(
                CheckedBooleanExpression::Constant(value),
            ))),
        )
    }

    /// A same-carrier policy cast selects later arithmetic, not an operation
    /// on the saved payload. Forward every admitted operand through ordinary
    /// computation building: field reads, locals and calls must not acquire
    /// different cast semantics. The original call's checked policy remains
    /// attached to that call. Semantic domains and carrier conversions retain
    /// their separate proof and qualification custody.
    fn policy_cast_value(
        &mut self,
        expression: ExpressionHandle,
        expected_type: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        let operand = self.policy_cast_operand(expression)?;
        let ExpressionNode::Cast(cast) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        if self.program.primitive_type_reference(cast.target_type) != Some(expected_type) {
            return None;
        }
        self.expression(operand, expected_type)
    }

    fn policy_cast_operand(&self, expression: ExpressionHandle) -> Option<ExpressionHandle> {
        let ExpressionNode::Cast(cast) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        if cast.form.is_recast()
            || !cast.semantic_domain.is_empty()
            || !matches!(
                self.program
                    .type_reference_table
                    .type_reference(cast.target_type),
                TypeReferenceNode::Named { .. }
            )
        {
            return None;
        }
        let target = self.program.primitive_type_reference(cast.target_type)?;
        let operand_type = semantic_casts::result_type(self.program, self.state, cast.value)?;
        ((cast.domain != ArithmeticDomain::Exact
            || self
                .program
                .type_reference_table
                .arithmetic_domain(operand_type)
                != ArithmeticDomain::Exact)
            && !semantic_casts::has_declared_domains(self.program, operand_type)
            && self.program.primitive_type_reference(operand_type) == Some(target))
        .then_some(cast.value)
    }

    fn expression(
        &mut self,
        expression: ExpressionHandle,
        expected_type: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        if let Some(field) = self.structural_scalar_field(expression) {
            return (field.primitive_type == expected_type)
                .then(|| self.structural_field(expression, field))
                .flatten();
        }
        if let Some(operand) = self.policy_cast_value(expression, expected_type) {
            return Some(operand);
        }
        if let ExpressionNode::Cast(cast) =
            self.program.expression_table.expression(expression).clone()
            && cast.semantic_domain.is_empty()
            && let Some(source_type) =
                semantic_casts::result_type(self.program, self.state, cast.value)
            && semantic_casts::has_declared_domains(self.program, source_type)
        {
            if cast.form.is_recast()
                || cast.domain != ArithmeticDomain::Exact
                || !matches!(
                    self.program
                        .type_reference_table
                        .type_reference(cast.target_type),
                    TypeReferenceNode::Named { .. }
                )
                || !semantic_casts::has_only_vacuous_tags(self.program, source_type)
                || self.program.primitive_type_reference(source_type) != Some(expected_type)
                || self.program.primitive_type_reference(cast.target_type) != Some(expected_type)
            {
                return self.authored_cast(expression, &cast, expected_type);
            }
            let result_type = semantic_casts::result_type(self.program, self.state, expression)?;
            if semantic_casts::has_declared_domains(self.program, result_type) {
                return self.authored_cast(expression, &cast, expected_type);
            }
            let operand = self.expression(cast.value, expected_type)?;
            return Some(self.insert(
                expected_type,
                CheckedScalarComputationKind::Qualification {
                    source_expression: expression,
                    operand,
                    result_type,
                },
            ));
        }
        if let ExpressionNode::Cast(cast) =
            self.program.expression_table.expression(expression).clone()
            && !cast.semantic_domain.is_empty()
        {
            // Qualification changes semantic custody, not payload. It cannot
            // become a pure expression whose source marker erases the cast.
            // Final checked qualification facts are built after this graph;
            // share their declaration rule and let the consumer rejoin the
            // completed exact use fact before any publication.
            if cast.form.is_recast()
                || !matches!(
                    self.program
                        .type_reference_table
                        .type_reference(cast.target_type),
                    typed_trees::types::TypeReferenceNode::Named { .. }
                )
                || cast.domain != ArithmeticDomain::Exact
                || !cast.semantic_domain_id.is_valid()
                || !crate::facts::domain_is_vacuous(
                    self.program,
                    cast.semantic_domain_symbol,
                    &mut Vec::new(),
                )
                || self.program.primitive_type_reference(cast.target_type) != Some(expected_type)
                || self.program.primitive_type_reference(cast.result_type) != Some(expected_type)
            {
                return self.authored_cast(expression, &cast, expected_type);
            }
            let operand = self.expression(cast.value, expected_type)?;
            return Some(self.insert(
                expected_type,
                CheckedScalarComputationKind::Qualification {
                    source_expression: expression,
                    operand,
                    result_type: cast.result_type,
                },
            ));
        }
        if let ExpressionNode::Cast(cast) =
            self.program.expression_table.expression(expression).clone()
            && semantic_casts::requires_custody(self.program, self.state, expression)
        {
            return self.authored_cast(expression, &cast, expected_type);
        }
        if let ExpressionNode::Match(dispatch) =
            self.program.expression_table.expression(expression).clone()
        {
            return self.dispatch(expression, &dispatch, expected_type);
        }
        if expected_type == PrimitiveType::Bool
            && let Some(operator_use) = self.comparison_use(
                expression,
                checked_trees::CheckedOperatorOccurrence::Expression,
            )
            && let Some(primitive) = self.selected_comparison_primitive(operator_use)
        {
            let operands = self
                .operators
                .uses
                .get(operator_use)
                .operands(self.program)?;
            let left = self.expression(operands[0], primitive)?;
            let right = self.expression(operands[1], primitive)?;
            return Some(self.insert(
                PrimitiveType::Bool,
                CheckedScalarComputationKind::SelectedComparison {
                    operator_use,
                    left,
                    right,
                },
            ));
        }
        if expected_type == PrimitiveType::Bool
            && let Some(comparison) = self.byte_literal_comparison(expression)
        {
            return Some(comparison);
        }
        if expected_type == PrimitiveType::Bool
            && let Some(predicate) = self.byte_predicate(expression)
        {
            return Some(predicate);
        }
        if !semantic_casts::requires_custody(self.program, self.state, expression)
            && let Some(value) = lower_return_expression(
                self.program,
                self.operators,
                expression,
                self.parameters,
                self.authored_parameters,
                self.parameter_types,
                self.locals,
                expected_type,
                self.exact_integer_casts,
            )
        {
            let computation =
                self.insert(expected_type, CheckedScalarComputationKind::Value(value));
            self.plans.nodes.get_mut(computation).value_source = expression;
            return Some(computation);
        }
        if expected_type == PrimitiveType::Bool
            && let Some(equality) = self.enum_equality(expression)
        {
            return Some(equality);
        }
        if expected_type == PrimitiveType::Bool
            && let Some(membership) = self.case_membership(expression)
        {
            return Some(membership);
        }
        if is_integer(expected_type)
            && !matches!(
                self.program.expression_table.expression(expression),
                ExpressionNode::Call(_)
            )
        {
            let integer = self.integer_operand(expression, Some(expected_type))?;
            if scalar_expression_type(&integer.value)? != expected_type {
                return None;
            }
            return self.materialize_integer(integer);
        }
        match self.program.expression_table.expression(expression).clone() {
            ExpressionNode::Call(call) => {
                if let Some(selection) = self.integer_min_max(expression, &call, expected_type) {
                    return Some(selection);
                }
                if let Some(operator) = boundary_calls::boundary_operator(
                    self.program,
                    crate::flow::call_target_symbol(self.program, &call),
                ) {
                    return self.boundary_operator_call(expression, &call, operator, expected_type);
                }
                if !call.carries_only_positional_arguments() || !call.selects_only_nominal_route() {
                    return None;
                }
                let (target_machine, target_state) =
                    crate::semantic::calls::find_machine_by_entry_state(
                        self.program,
                        call.target_symbol,
                    )?;
                // A bodyless boundary declaration has no checked body for this
                // graph to embed. A whole-value statement call is the
                // statement's own result operation (`record_root` leaves it
                // there); a nested application instead becomes a boundary
                // node that invokes the exact occurrence between its operands
                // and its consumer. A `boundary machine` with an authored
                // body stays an ordinary callee — the same predicate the call
                // operation applies. A bodyless external leaf is the same
                // boundary edge through its exact satisfied requirement.
                if let Some((requirement, qualifier)) =
                    validation::exact_external_leaf_boundary_requirement(
                        self.program,
                        call.target_symbol,
                    )
                {
                    return self.external_boundary_call(
                        expression,
                        &call,
                        requirement,
                        qualifier,
                        expected_type,
                    );
                }
                if target_machine.supply_mode.is_boundary_declaration()
                    && !target_machine.body_is_present
                {
                    return self.boundary_machine_call(
                        expression,
                        &call,
                        target_machine,
                        target_state,
                        expected_type,
                    );
                }
                let has_runtime_receiver =
                    !self
                        .program
                        .call_has_no_runtime_receiver(&call, target_machine, target_state);
                if self
                    .program
                    .primitive_type_reference(target_state.return_type)?
                    != expected_type
                {
                    return None;
                }
                let target_parameters = self.program.state_parameters(target_state);
                if target_parameters.iter().any(|parameter| {
                    parameter.is_const
                        || (parameter.is_mutable
                            && self
                                .program
                                .primitive_type_reference(parameter.type_reference)
                                .is_some()
                            && crate::values::mutable_scalar_parameter_type(
                                self.program,
                                parameter,
                            )
                            .is_none())
                }) {
                    return None;
                }
                let (source_call, call_ordinal) =
                    self.call_ordinal(expression, call.target_symbol)?;
                let arguments = self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                    .to_vec();
                if has_runtime_receiver {
                    return None;
                }
                if arguments.len() != target_parameters.len() {
                    return None;
                }
                let (arguments, structural_arguments, temporary_cleanups) =
                    self.call_operands(source_call, &arguments, target_parameters)?;
                Some(self.insert(
                    expected_type,
                    CheckedScalarComputationKind::Call {
                        target_machine: target_machine.symbol,
                        target_state: target_state.symbol,
                        source_call,
                        call_ordinal,
                        arguments,
                        structural_arguments,
                        temporary_cleanups,
                    },
                ))
            }
            ExpressionNode::Binary(binary)
                if expected_type == PrimitiveType::Bool
                    && operator_is_builtin(self.operators, expression)
                    && matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
            {
                let condition = self.expression(binary.left, PrimitiveType::Bool)?;
                let evaluate_when = binary.operator == BinaryOperator::And;
                // A known skipped RHS has no FlowCallFact and must not need one.
                if let CheckedScalarComputationKind::Value(value) =
                    &self.plans.nodes.get(condition).kind
                    && let Some(facts::ScalarValue::Boolean(value)) =
                        crate::values::evaluate_checked_scalar(value, &mut |_| None)
                {
                    return if value == evaluate_when {
                        self.expression(binary.right, PrimitiveType::Bool)
                    } else {
                        Some(condition)
                    };
                }
                if normal_return::boolean_result(self.plans, condition) == Some(!evaluate_when) {
                    // Unlike replacing this with a constant, returning the
                    // original graph preserves every effect leading to its result.
                    return Some(condition);
                }
                let right = self.expression(binary.right, PrimitiveType::Bool)?;
                let skipped = self.boolean(!evaluate_when);
                let (when_true, when_false) = if evaluate_when {
                    (right, skipped)
                } else {
                    (skipped, right)
                };
                Some(self.insert(
                    PrimitiveType::Bool,
                    CheckedScalarComputationKind::Select {
                        source_expression: expression,
                        condition,
                        when_true,
                        when_false,
                    },
                ))
            }
            ExpressionNode::Unary(unary)
                if expected_type == PrimitiveType::Bool
                    && unary.operator == UnaryOperator::LogicalNot
                    && operator_is_builtin(self.operators, expression) =>
            {
                let operand = self.expression(unary.operand, PrimitiveType::Bool)?;
                let operands = self.plans.operands.insert_many([operand]);
                Some(self.insert(
                    PrimitiveType::Bool,
                    CheckedScalarComputationKind::Apply {
                        source_expression: expression,
                        expression: CheckedScalarExpression::Boolean(Box::new(
                            CheckedBooleanExpression::Not(Box::new(
                                CheckedBooleanExpression::Parameter { position: 0 },
                            )),
                        )),
                        operands,
                    },
                ))
            }
            ExpressionNode::Binary(binary)
                if expected_type == PrimitiveType::Bool
                    && matches!(
                        binary.operator,
                        BinaryOperator::Equal
                            | BinaryOperator::NotEqual
                            | BinaryOperator::Less
                            | BinaryOperator::LessOrEqual
                            | BinaryOperator::Greater
                            | BinaryOperator::GreaterOrEqual
                    )
                    && operator_is_builtin(self.operators, expression) =>
            {
                if let Some(comparison) = self.integer_comparison(expression, &binary) {
                    return Some(comparison);
                }
                if !matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) {
                    return None;
                }
                let left = self.expression(binary.left, PrimitiveType::Bool)?;
                let right = self.expression(binary.right, PrimitiveType::Bool)?;
                let operands = self.plans.operands.insert_many([left, right]);
                let mut template = CheckedBooleanExpression::Equal {
                    left: Box::new(CheckedBooleanExpression::Parameter { position: 0 }),
                    right: Box::new(CheckedBooleanExpression::Parameter { position: 1 }),
                };
                if binary.operator == BinaryOperator::NotEqual {
                    template = CheckedBooleanExpression::Not(Box::new(template));
                }
                Some(self.insert(
                    PrimitiveType::Bool,
                    CheckedScalarComputationKind::Apply {
                        source_expression: expression,
                        expression: CheckedScalarExpression::Boolean(Box::new(template)),
                        operands,
                    },
                ))
            }
            _ => None,
        }
    }

    fn call_ordinal(
        &self,
        expression: ExpressionHandle,
        target: SymbolHandle,
    ) -> Option<(arena::Handle<checked_trees::FlowCallFact>, u32)> {
        let state = self.flow.control.states.iter().find_map(|(_, state)| {
            (state.machine_symbol == self.machine && state.state_symbol == self.state)
                .then_some(state)
        })?;
        let mut matching = self
            .flow
            .control
            .calls
            .span_or_empty(state.calls)
            .iter()
            .filter(|call| {
                call.statement_index == self.statement_index
                    && call.target_symbol == target
                    && call.authored_expression == expression
            });
        let call = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        let handle = self
            .flow
            .control
            .calls
            .iter()
            .find_map(|(handle, candidate)| std::ptr::eq(candidate, call).then_some(handle))?;
        Some((handle, u32::try_from(call.call_ordinal).ok()?))
    }
}
