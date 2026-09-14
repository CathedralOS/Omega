//! Ordered statement checks and flow-sensitive value updates.

use crate::calls::validate_call_node;
use crate::expression_types::ExpressionTypeOwner;
use crate::expression_types::validate_expression_type_handle;
use crate::locals::WritableRoots;
use crate::places::validate_assignment_target_handle;
use crate::symbols::MachineSymbols;
use crate::transitions::validate_transition_target_node;
use crate::type_references::TypeReferenceOwner;
use crate::type_references::validate_type_reference_handle_with_type_parameters;
use crate::{
    ExactIntegerCastFact, TopLevelSymbols, ValidatedBoundaryOperatorApplication,
    arithmetic_domains, calls, domain_weakening, expression_types, placed_views, places,
    proof_facts, struct_literals, transitions,
};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::statement::StatementNode;
use typed_trees::statement::TransitionTargetNode;

pub(super) fn is_exact_executable_drop_body(
    program: &TypedTrees,
    cleanup: &typed_trees::machine::Machine,
) -> bool {
    let [cleanup_state] = program.machine_states(cleanup) else {
        return false;
    };
    let statements = program
        .statement_table
        .statements(cleanup_state.statement_nodes);
    if statements.is_empty() {
        return false;
    }

    let mut helper_symbols = Vec::with_capacity(statements.len());
    for statement in statements {
        let StatementNode::Call(call) = statement else {
            return false;
        };
        if !call.machine_arguments.is_empty()
            || !program
                .statement_table
                .expression_handles(call.arguments)
                .is_empty()
            || call.discards_result
        {
            return false;
        }

        let helpers = program
            .machines()
            .iter()
            .filter_map(|machine| {
                let [state] = program.machine_states(machine) else {
                    return None;
                };
                (state.symbol == call.target_symbol).then_some((machine, state))
            })
            .collect::<Vec<_>>();
        let [(helper, helper_state)] = helpers.as_slice() else {
            return false;
        };
        let Some(helper_attachment) = helper.attached_data.as_ref() else {
            return false;
        };
        let helper_data = program
            .data_definitions()
            .iter()
            .filter(|data| &data.name == helper_attachment)
            .collect::<Vec<_>>();
        let [helper_data] = helper_data.as_slice() else {
            return false;
        };

        if helper.symbol == cleanup.symbol
            || helper_symbols.contains(&helper.symbol)
            || helper.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
            || !helper.lifetime_parameters.is_empty()
            || !program.machine_type_parameters(helper).is_empty()
            || !program.machine_owned_data(helper).is_empty()
            || !program.machine_trait_conformances(helper).is_empty()
            || !helper.conformance_bounds.is_empty()
            || !program.machine_invokes(helper).is_empty()
            || helper.suspends
            || helper.blocks
            || !program.machine_contracts(helper).is_empty()
            || !program.state_parameters(helper_state).is_empty()
            || !program.state_contracts(helper_state).is_empty()
            || !matches!(
                program
                    .type_reference_table
                    .type_reference(helper_state.return_type),
                typed_trees::types::TypeReferenceNode::Unit
            )
            || !program
                .statement_table
                .statements(helper_state.statement_nodes)
                .is_empty()
            || !helper_data.lifetime_parameters.is_empty()
            || !program.data_type_parameters(helper_data).is_empty()
            || !program.data_members(helper_data).is_empty()
        {
            return false;
        }
        helper_symbols.push(helper.symbol);
    }
    true
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_state_statement_node(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state_name: &str,
    current_state: Option<&typed_trees::state::State>,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    statement_handle: typed_trees::statement::StatementHandle,
    statement: &StatementNode,
    value_env: &mut arithmetic_domains::ValueEnv,
    transition_values: &transitions::TransitionValueEnvironments,
    direct_written: Option<Vec<String>>,
    exact_integer_casts: &mut Vec<ExactIntegerCastFact>,
    boundary_operator_applications: &mut Vec<ValidatedBoundaryOperatorApplication>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(state) = current_state {
        placed_views::validate_statement(program, machine, state, statement, diagnostics);
    }
    match statement {
        StatementNode::RootBinding(_) => {}
        StatementNode::AssemblyFact(fact) => {
            let state = current_state;
            if let Some(state) = state {
                crate::locals::StateValueScope {
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    prior_statements: writable_roots.statements,
                    context: "asm assertion",
                }
                .expression(fact.expression, diagnostics);
            }
            if !proof_facts::is_boolean_asm_fact_expression(
                program,
                machine,
                state,
                fact.expression,
            ) {
                let kind = match fact.kind {
                    typed_trees::statement::AssemblyFactKind::Requires => "requires",
                    typed_trees::statement::AssemblyFactKind::Ensures => "ensures",
                };
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{state_name}` asm `{kind}` fact `{}` is not boolean-shaped",
                    machine.name,
                    program.expression_table.display_name(fact.expression),
                )));
            }
        }
        StatementNode::Assignment(assignment) => {
            let state = current_state;
            if !state.is_some_and(|state| {
                placed_views::assignment_is_placed_atomic_operation(
                    program, machine, state, assignment,
                )
            }) {
                validate_assignment_target_handle(
                    program,
                    assignment.target,
                    writable_roots,
                    diagnostics,
                    machine,
                    state,
                    state_name,
                );
            }
            calls::validate_asm_value_destination(program, machine, state, assignment, diagnostics);
            let assignment_target_type =
                places::declared_place_type(program, machine, state, assignment.target)
                    // An indexed target (`self.xs[i] = ..`) has no member path, so
                    // `declared_place_type` returns None -- fall back to the array/slice
                    // ELEMENT type so the store checks below see the real slot type.
                    .or_else(|| {
                        places::declared_indexed_projection_type(
                            program,
                            machine,
                            current_state,
                            assignment.target,
                        )
                    });
            // Weakening is about the target's STATIC qualification, so retain
            // the Constrained shell that the older representation/class checks
            // intentionally unwrap above.
            let assignment_target_type_raw =
                places::declared_place_type_raw(program, machine, state, assignment.target)
                    .or_else(|| {
                        places::declared_indexed_projection_type_raw(
                            program,
                            machine,
                            current_state,
                            assignment.target,
                        )
                    });
            let assignment_target_primitive =
                assignment_target_type.and_then(|handle| program.primitive_type_reference(handle));
            if let Some(state) = current_state {
                struct_literals::validate_array_window_elements(
                    program,
                    machine,
                    state,
                    assignment.target,
                    assignment.value,
                    diagnostics,
                );
            }
            // An array-literal RHS into a `[T; N]` target: check each element's
            // class + narrowing against T. The scalar guards below skip a non-
            // primitive (array) target, so this is the element-level complement.
            if let Some(current_state) = current_state
                && let Some(target_type) = assignment_target_type
            {
                struct_literals::validate_array_literal_elements(
                    program,
                    machine,
                    current_state,
                    assignment.value,
                    target_type,
                    diagnostics,
                );
            }
            let owner = format!("machine `{}` state `{state_name}` assignment", machine.name);
            // Cross-class scalar guard: `self.i32 = true` (a bool into a numeric
            // field) is a soundness hole -- the backend silently stores `1` --
            // whether the bool arrives as a literal or through a `self.bool_field`
            // place. Reject the unambiguous cross-class cases before value-range
            // analysis (which assumes a class-compatible RHS).
            if let Some(target_primitive) = assignment_target_primitive {
                expression_types::report_cross_class_store(
                    program,
                    Some(machine),
                    current_state,
                    assignment.value,
                    target_primitive,
                    &owner,
                    "place",
                    diagnostics,
                );
            }
            // Nominal guard: `self.foo = self.bar` (a `Bar` value into a `Foo` place)
            // is silently accepted -- the wrong-data-type complement of the scalar
            // guard above.
            if let Some(target_type) = assignment_target_type {
                expression_types::report_data_type_conflict(
                    program,
                    machine,
                    current_state,
                    assignment.value,
                    target_type,
                    &owner,
                    "place",
                    diagnostics,
                );
                // Shape guard: `self.scalar = self.xs` (array into a scalar place) --
                // caught by the backend today (a crude `NeedsMachineOwnedWrite`); this
                // gives a clear frontend message and covers the mirror.
                expression_types::report_array_scalar_shape_mismatch(
                    program,
                    machine,
                    current_state,
                    assignment.value,
                    target_type,
                    &owner,
                    "place",
                    diagnostics,
                );
                // Scalar-vs-data shape guard: `self.struct_field = 5` / `self.scalar
                // = self.struct` (a scalar into a struct slot or the mirror), between
                // the scalar-class and nominal gates.
                expression_types::report_scalar_data_shape_mismatch(
                    program,
                    machine,
                    current_state,
                    assignment.value,
                    target_type,
                    &owner,
                    "place",
                    diagnostics,
                );
            }
            if let Some(target_type) = assignment_target_type_raw {
                domain_weakening::validate_implicit_domain_weakening(
                    program,
                    machine,
                    current_state,
                    assignment.value,
                    target_type,
                    &owner,
                    diagnostics,
                );
            }
            calls::report_nested_call_in_local_assignment(
                program,
                machine,
                current_state,
                state_name,
                assignment,
                diagnostics,
            );
            let before = diagnostics.len();
            let assignment_target_domain = assignment_target_type
                .map(|handle| program.arithmetic_domain_for_type_reference(handle))
                .unwrap_or(numerics::arithmetic::ArithmeticDomain::Exact);
            let (interval, source_primitive) = assignment_target_type_raw
                .and_then(|destination| {
                    arithmetic_domains::validate_anonymous_integer_range(
                        program,
                        places::assignment_value_type(program, destination),
                        assignment.value,
                        &owner,
                        diagnostics,
                    )
                })
                .unwrap_or_else(|| {
                    arithmetic_domains::validate_value_range(
                        program,
                        machine,
                        current_state,
                        assignment.value,
                        value_env,
                        assignment_target_primitive,
                        assignment_target_domain,
                        &owner,
                        diagnostics,
                    )
                });
            // Only a CLEANLY-analyzed RHS reaches the narrowing check -- an RHS that
            // already erred (its own overflow, a type error) is not re-flagged.
            if diagnostics.len() == before {
                arithmetic_domains::check_narrowing_assignment(
                    assignment_target_primitive,
                    interval,
                    source_primitive,
                    &owner,
                    diagnostics,
                );
                // Containment for non-literal stores into ranged places (see
                // the local arm's twin note; literal stores refuse through
                // the proof plan).
                if let Some(handle) = assignment_target_type_raw {
                    arithmetic_domains::enforce_symbolic_range(
                        program,
                        machine,
                        current_state,
                        handle,
                        assignment.value,
                        &owner,
                        diagnostics,
                    );
                }
                if let Some(handle) = assignment_target_type {
                    arithmetic_domains::check_range_containment(
                        program,
                        handle,
                        interval,
                        &owner,
                        diagnostics,
                    );
                }
            }
            // Analyze the RHS against pre-store facts, then invalidate every
            // spelling of the storage before recording the new target value.
            arithmetic_domains::collect_exact_integer_cast_facts(
                program,
                machine,
                current_state,
                assignment.value,
                value_env,
                exact_integer_casts,
            );
            if let Some(written) = direct_written {
                value_env.invalidate_assignment_paths(
                    program,
                    machine,
                    current_state,
                    assignment.target,
                    &written,
                );
            } else {
                value_env.clear();
            }
            arithmetic_domains::record_assignment(
                value_env,
                arithmetic_domains::place_path(program, assignment.target),
                interval,
                places::declared_place_type_raw(program, machine, current_state, assignment.target)
                    .and_then(|handle| {
                        arithmetic_domains::enforced_declared_range(program, handle)
                    }),
            );
            if diagnostics.len() == before {
                arithmetic_domains::record_unsigned_literal_assignment(
                    program,
                    value_env,
                    arithmetic_domains::place_path(program, assignment.target),
                    assignment_target_primitive,
                    assignment.value,
                );
                arithmetic_domains::record_float_literal_assignment(
                    program,
                    value_env,
                    arithmetic_domains::place_path(program, assignment.target),
                    assignment_target_primitive,
                    assignment.value,
                );
            }
        }
        StatementNode::Call(call) => {
            if let Some(state) = current_state {
                crate::operators::validate_named_statement_operator_application(
                    program,
                    symbols,
                    machine,
                    state,
                    statement_handle,
                    call,
                    boundary_operator_applications,
                    diagnostics,
                );
            }
            validate_call_node(
                program,
                call,
                machine,
                state_name,
                current_state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                diagnostics,
            );
            // R5 frame seed: a resolved acyclic INTERNAL call preserves facts
            // outside its conservatively instantiated may-write set. Unknown,
            // unsummarized, and overlapping implementations remain
            // conservative. Authored `stores` clauses are retired; exactness
            // grows through inferred implementation summaries.
            if let Some(written) = direct_written {
                value_env.invalidate_written_paths(&written);
            } else {
                value_env.clear();
            }
            // R4 witness mint: a BOUNDARY callee's `ensures` re-seeds the
            // `&mut` out-arguments' places (the boundary model's citable
            // fact) -- `fw.get_size(&mut self.n)` with `ensures size <= 8`
            // leaves `self.n` in [type_low, 8].
            if let Some(signature) = crate::calls::boundary_trait_signature(
                program,
                machine,
                machine_symbols,
                symbols,
                call,
            ) {
                arithmetic_domains::seed_out_param_ensures(
                    program,
                    machine,
                    current_state,
                    call,
                    signature,
                    value_env,
                );
            }
        }
        StatementNode::Expression(expression) => {
            let Some(state) = current_state else {
                return;
            };

            if calls::unit_statement_call_is_supported(program, machine, state, *expression) {
                // Value-position call validation already checked the callee and
                // its operands. Retain cast obligations inside those operands;
                // Unit itself has no scalar return range or landing.
                arithmetic_domains::collect_exact_integer_cast_facts(
                    program,
                    machine,
                    Some(state),
                    *expression,
                    value_env,
                    exact_integer_casts,
                );
                return;
            }
            if !state.return_type.is_valid() {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{state_name}` has a terminal expression but no return type",
                    machine.name
                )));
                return;
            }

            let shape_before = diagnostics.len();
            struct_literals::validate_array_literal_elements(
                program,
                machine,
                state,
                *expression,
                state.return_type,
                diagnostics,
            );
            validate_expression_type_handle(
                program,
                *expression,
                state.return_type,
                diagnostics,
                ExpressionTypeOwner::StateTerminalExpression {
                    machine: machine.name.as_str(),
                    state: state_name,
                },
            );
            let owner = format!(
                "machine `{}` state `{state_name}` terminal expression",
                machine.name
            );
            let return_primitive = program.primitive_type_reference(state.return_type);
            // The shape gate above rejects a cross-class LITERAL terminal return
            // (`-> i32 { true }`) but blanket-accepts place/name values, so a bool
            // FIELD returned from an `-> i32` machine slips. Add the class check for
            // those -- only when the shape gate did not already error, so a literal
            // is not double-reported.
            if diagnostics.len() == shape_before {
                let slot_context = format!("machine `{}` state `{state_name}`", machine.name);
                if let Some(target_primitive) = return_primitive {
                    expression_types::report_cross_class_store(
                        program,
                        Some(machine),
                        Some(state),
                        *expression,
                        target_primitive,
                        &slot_context,
                        "return value",
                        diagnostics,
                    );
                }
                // Nominal guard: `-> Foo { self.bar }` returns a `Bar` as a `Foo`.
                expression_types::report_data_type_conflict(
                    program,
                    machine,
                    Some(state),
                    *expression,
                    state.return_type,
                    &slot_context,
                    "return value",
                    diagnostics,
                );
                // Shape guard: `-> i32 { self.xs }` returns an array as a scalar.
                expression_types::report_array_scalar_shape_mismatch(
                    program,
                    machine,
                    Some(state),
                    *expression,
                    state.return_type,
                    &slot_context,
                    "return value",
                    diagnostics,
                );
                expression_types::report_scalar_data_shape_mismatch(
                    program,
                    machine,
                    Some(state),
                    *expression,
                    state.return_type,
                    &slot_context,
                    "return value",
                    diagnostics,
                );
                domain_weakening::validate_implicit_domain_weakening(
                    program,
                    machine,
                    Some(state),
                    *expression,
                    state.return_type,
                    &slot_context,
                    diagnostics,
                );
            }
            let before = diagnostics.len();
            let (return_interval, source_primitive) =
                arithmetic_domains::validate_anonymous_integer_range(
                    program,
                    state.return_type,
                    *expression,
                    &owner,
                    diagnostics,
                )
                .unwrap_or_else(|| {
                    arithmetic_domains::validate_value_range(
                        program,
                        machine,
                        Some(state),
                        *expression,
                        value_env,
                        return_primitive,
                        program.arithmetic_domain_for_type_reference(state.return_type),
                        &owner,
                        diagnostics,
                    )
                });
            // A cleanly-analyzed return value that cannot fit the declared return
            // type is a silent narrowing (`-> i8 { 300 }`), same as a store.
            if diagnostics.len() == before {
                arithmetic_domains::check_narrowing_assignment(
                    return_primitive,
                    return_interval,
                    source_primitive,
                    &owner,
                    diagnostics,
                );
            }
            // S4: enforce a declared return `[a..=b]` so call-site narrowing
            // that trusts it stays sound (the interval is already computed above).
            arithmetic_domains::enforce_symbolic_range(
                program,
                machine,
                Some(state),
                state.return_type,
                *expression,
                &owner,
                diagnostics,
            );
            arithmetic_domains::enforce_declared_return_range(
                program,
                state.return_type,
                return_interval,
                &owner,
                diagnostics,
            );
            arithmetic_domains::collect_exact_integer_cast_facts(
                program,
                machine,
                Some(state),
                *expression,
                value_env,
                exact_integer_casts,
            );
        }
        StatementNode::LocalData(local_data) => {
            let mut type_parameters = program.machine_type_parameters(machine).to_vec();
            let mut lifetime_parameters = machine.lifetime_parameters.clone();
            if let Some(attached_data) = &machine.attached_data
                && let Some(definition) = program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.name == *attached_data)
            {
                for parameter in program.data_type_parameters(definition) {
                    if !type_parameters
                        .iter()
                        .any(|existing| existing.symbol == parameter.symbol)
                    {
                        type_parameters.push(parameter.clone());
                    }
                }
                for parameter in &definition.lifetime_parameters {
                    if !lifetime_parameters
                        .iter()
                        .any(|existing| existing == parameter)
                    {
                        lifetime_parameters.push(parameter.clone());
                    }
                }
            }
            validate_type_reference_handle_with_type_parameters(
                program,
                local_data.type_reference,
                symbols,
                diagnostics,
                TypeReferenceOwner::StateLocalData {
                    machine: machine.name.as_str(),
                    state: state_name,
                    local: local_data.name.as_str(),
                    generic_depth: 0,
                },
                &type_parameters,
                &lifetime_parameters,
            );
            let local_target_primitive =
                program.primitive_type_reference(local_data.type_reference);
            // An array-literal initializer (`let a: [T; N] = [300, ..]`) is checked
            // element-wise against T.
            if let Some(current_state) = current_state {
                struct_literals::validate_array_literal_elements(
                    program,
                    machine,
                    current_state,
                    local_data.initial_value,
                    local_data.type_reference,
                    diagnostics,
                );
            }
            let owner = format!(
                "machine `{}` state `{state_name}` local `{}`",
                machine.name,
                local_data.name.as_str()
            );
            // Unit is not storage for an initializer's value. Generated
            // bindings can retain an inference sentinel, but an authored
            // annotation must never silently discard a produced value.
            if local_data.initial_value.is_valid()
                && local_data.type_reference.is_valid()
                && !local_data.type_is_inferred
                && matches!(
                    program
                        .type_reference_table
                        .type_reference(local_data.type_reference),
                    typed_trees::types::TypeReferenceNode::Unit
                )
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} has an explicit Unit type and cannot bind an initializer value; use the value's type or explicitly discard the result"
                )));
            }
            // Cross-class guard: `let x: i32 = true` stores a bool into a numeric
            // local -- a silent miscompile, same as the assignment / arg / field
            // positions. Only an INITIALIZED `let` has a value to class-check (a
            // bare `let x: bool;` filled later by an `&mut` out-param has an invalid
            // initializer). (Narrowing on the initializer is checked below.)
            if local_data.initial_value.is_valid()
                && let Some(target_primitive) = local_target_primitive
            {
                expression_types::report_cross_class_store(
                    program,
                    Some(machine),
                    current_state,
                    local_data.initial_value,
                    target_primitive,
                    &owner,
                    "local",
                    diagnostics,
                );
            }
            // Nominal guard: `let f: Foo = self.bar` binds a `Bar` value to a `Foo`
            // local -- wrong data type. Only an initialized `let` has a value.
            if local_data.initial_value.is_valid() {
                expression_types::report_data_type_conflict(
                    program,
                    machine,
                    current_state,
                    local_data.initial_value,
                    local_data.type_reference,
                    &owner,
                    "local",
                    diagnostics,
                );
                // Shape guard: `let y: i32 = self.xs` (array -> scalar) or
                // `let xs: [i32; 3] = 5` (scalar -> array) otherwise bind a
                // wrong-shaped value silently (the array case read a ZII 0).
                expression_types::report_array_scalar_shape_mismatch(
                    program,
                    machine,
                    current_state,
                    local_data.initial_value,
                    local_data.type_reference,
                    &owner,
                    "local",
                    diagnostics,
                );
                expression_types::report_scalar_data_shape_mismatch(
                    program,
                    machine,
                    current_state,
                    local_data.initial_value,
                    local_data.type_reference,
                    &owner,
                    "local",
                    diagnostics,
                );
                domain_weakening::validate_implicit_domain_weakening(
                    program,
                    machine,
                    current_state,
                    local_data.initial_value,
                    local_data.type_reference,
                    &owner,
                    diagnostics,
                );
            }
            calls::report_nested_call_in_local_initializer(
                program,
                machine,
                state_name,
                local_data.initial_value,
                diagnostics,
            );
            let before = diagnostics.len();
            let (interval, source_primitive) =
                arithmetic_domains::validate_anonymous_integer_range(
                    program,
                    local_data.type_reference,
                    local_data.initial_value,
                    &owner,
                    diagnostics,
                )
                .unwrap_or_else(|| {
                    arithmetic_domains::validate_value_range(
                        program,
                        machine,
                        current_state,
                        local_data.initial_value,
                        value_env,
                        local_target_primitive,
                        program.arithmetic_domain_for_type_reference(local_data.type_reference),
                        &owner,
                        diagnostics,
                    )
                });
            // A cleanly-analyzed initializer whose value cannot fit the declared
            // local type is a silent narrowing (`let x: i8 = 300`).
            if local_data.initial_value.is_valid() && diagnostics.len() == before {
                arithmetic_domains::check_narrowing_assignment(
                    local_target_primitive,
                    interval,
                    source_primitive,
                    &owner,
                    diagnostics,
                );
                // Containment against a declared Exact `[a..=b]`: literal
                // initializers refuse through the proof plan, but a
                // NON-LITERAL initializer's interval was never checked --
                // `let idx: u32 [0..=11] = <expr provably up to 12>` stored
                // unproven and the index prover then TRUSTED the range (a
                // confirmed native OOB read, found landing the R3 product
                // rule). The enforced range only exists under Exact shells,
                // so non-Exact declarations are untouched (the
                // `range-constraints-require-exact-domain` gate
                // already rejects range+domain combinations).
                // A reference binding stores the reference, not a fresh value
                // into the referee. Its referee facts are checked by ordinary
                // borrow compatibility and, for a stated recast, by the
                // bidirectional representation judgment. Treating the borrow
                // expression as a numeric store into the referee incorrectly
                // rejects every range-refined reference initializer.
                if !matches!(
                    program
                        .type_reference_table
                        .type_reference(local_data.type_reference),
                    typed_trees::types::TypeReferenceNode::Reference { .. }
                ) {
                    arithmetic_domains::enforce_symbolic_range(
                        program,
                        machine,
                        current_state,
                        local_data.type_reference,
                        local_data.initial_value,
                        &owner,
                        diagnostics,
                    );
                    arithmetic_domains::check_range_containment(
                        program,
                        local_data.type_reference,
                        interval,
                        &owner,
                        diagnostics,
                    );
                }
            }
            if local_data.initial_value.is_valid() {
                arithmetic_domains::collect_exact_integer_cast_facts(
                    program,
                    machine,
                    current_state,
                    local_data.initial_value,
                    value_env,
                    exact_integer_casts,
                );
                arithmetic_domains::record_assignment(
                    value_env,
                    Some(local_data.name.as_str().to_owned()),
                    interval,
                    local_data
                        .type_reference
                        .is_valid()
                        .then(|| {
                            arithmetic_domains::enforced_declared_range(
                                program,
                                local_data.type_reference,
                            )
                        })
                        .flatten(),
                );
                if diagnostics.len() == before {
                    arithmetic_domains::record_unsigned_literal_assignment(
                        program,
                        value_env,
                        Some(local_data.name.as_str().to_owned()),
                        program.primitive_type_reference(local_data.type_reference),
                        local_data.initial_value,
                    );
                    arithmetic_domains::record_float_literal_assignment(
                        program,
                        value_env,
                        Some(local_data.name.as_str().to_owned()),
                        program.primitive_type_reference(local_data.type_reference),
                        local_data.initial_value,
                    );
                }
            }
        }
        StatementNode::Transition(transition) => {
            if let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard {
                arithmetic_domains::collect_exact_integer_cast_facts(
                    program,
                    machine,
                    current_state,
                    guard,
                    value_env,
                    exact_integer_casts,
                );
            }
            validate_transition_target_node(
                program,
                machine,
                current_state,
                value_env,
                transition_values.for_target(transition.target),
                transition.target,
                machine_symbols,
                symbols,
                writable_roots,
                diagnostics,
            );

            if transition.continuation.is_valid() {
                validate_transition_target_node(
                    program,
                    machine,
                    current_state,
                    value_env,
                    transition_values.for_target(transition.continuation),
                    transition.continuation,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    diagnostics,
                );
            }

            // Return and argument expressions use their collected target-local
            // premises: the primary arm assumes the guard, the continuation
            // assumes its complement, and each crosses only its own effects.
            // Keep the ordinary guard environment for targets without values.
            let narrowed = arithmetic_domains::guard_narrowed_env(
                program,
                machine,
                current_state,
                &transition.guard,
                value_env,
            );

            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target) {
                    TransitionTargetNode::Named { arguments, .. } => {
                        for (argument_index, argument) in program
                            .statement_table
                            .expression_handles(*arguments)
                            .iter()
                            .enumerate()
                        {
                            let argument_env = transition_values
                                .for_target(target)
                                .get(argument_index)
                                .unwrap_or(&narrowed);
                            arithmetic_domains::collect_exact_integer_cast_facts(
                                program,
                                machine,
                                current_state,
                                *argument,
                                argument_env,
                                exact_integer_casts,
                            );
                        }
                    }
                    TransitionTargetNode::Value(expression) => {
                        let return_env = transition_values
                            .for_target(target)
                            .first()
                            .unwrap_or(&narrowed);
                        arithmetic_domains::collect_exact_integer_cast_facts(
                            program,
                            machine,
                            current_state,
                            *expression,
                            return_env,
                            exact_integer_casts,
                        );
                    }
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }

            // Fall-through complement: an exit-if-true transition (valid
            // target, no `_` arm) leaves the guard REFUTED for every later
            // statement in this state -- the MR2 terminal-tail shape's
            // `n - 1` after `transition n == 0 { true -> exit }`.
            if transition.target.is_valid() && !transition.continuation.is_valid() {
                *value_env = arithmetic_domains::fall_through_narrowed_env(
                    program,
                    machine,
                    current_state,
                    &transition.guard,
                    value_env,
                );
            }

            // A transition VALUE target (`_ -> (expr)`) is a return value. When the
            // state's return type declares a `[a..=b]`, enforce the value is provably
            // within it (so call-site narrowing that trusts the range is sound); the
            // exact-overflow + narrowing obligations apply too. Gated inside
            // `validate_return_value_range`, so plain returns are unaffected.
            if let Some(state) = current_state
                && state.return_type.is_valid()
            {
                for target in [transition.target, transition.continuation] {
                    if !target.is_valid() {
                        continue;
                    }
                    if let TransitionTargetNode::Value(return_expression) =
                        program.statement_table.transition_target(target)
                    {
                        struct_literals::validate_array_literal_elements(
                            program,
                            machine,
                            state,
                            *return_expression,
                            state.return_type,
                            diagnostics,
                        );
                        let return_env = transition_values
                            .for_target(target)
                            .first()
                            .unwrap_or(&narrowed);
                        // Cross-class guard: `_ -> (true)` returning a bool from an
                        // i32-returning machine is a silent miscompile. The terminal
                        // `{ true }` return form is caught by the general shape gate;
                        // the transition-VALUE form was not. Class complement of the
                        // range/overflow/narrowing check below.
                        if let Some(return_primitive) =
                            program.primitive_type_reference(state.return_type)
                        {
                            expression_types::report_cross_class_store(
                                program,
                                Some(machine),
                                Some(state),
                                *return_expression,
                                return_primitive,
                                &format!("machine `{}` state `{state_name}`", machine.name),
                                "return value",
                                diagnostics,
                            );
                        }
                        // Nominal guard: `-> Foo { transition { _ -> (self.bar) } }`
                        // returns a `Bar` as a `Foo`.
                        expression_types::report_data_type_conflict(
                            program,
                            machine,
                            Some(state),
                            *return_expression,
                            state.return_type,
                            &format!("machine `{}` state `{state_name}`", machine.name),
                            "return value",
                            diagnostics,
                        );
                        // Shape guard: an array returned as a scalar (or vice versa).
                        expression_types::report_array_scalar_shape_mismatch(
                            program,
                            machine,
                            Some(state),
                            *return_expression,
                            state.return_type,
                            &format!("machine `{}` state `{state_name}`", machine.name),
                            "return value",
                            diagnostics,
                        );
                        expression_types::report_scalar_data_shape_mismatch(
                            program,
                            machine,
                            Some(state),
                            *return_expression,
                            state.return_type,
                            &format!("machine `{}` state `{state_name}`", machine.name),
                            "return value",
                            diagnostics,
                        );
                        domain_weakening::validate_implicit_domain_weakening(
                            program,
                            machine,
                            Some(state),
                            *return_expression,
                            state.return_type,
                            &format!("machine `{}` state `{state_name}`", machine.name),
                            diagnostics,
                        );
                        arithmetic_domains::validate_return_value_range(
                            program,
                            machine,
                            state,
                            *return_expression,
                            return_env,
                            &format!(
                                "machine `{}` state `{state_name}` return value",
                                machine.name
                            ),
                            diagnostics,
                        );
                    }
                }
            }

            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                if let TransitionTargetNode::Named { arguments, .. } =
                    program.statement_table.transition_target(target)
                {
                    for (argument_index, argument) in program
                        .statement_table
                        .expression_handles(*arguments)
                        .iter()
                        .enumerate()
                    {
                        let argument_env = transition_values
                            .for_target(target)
                            .get(argument_index)
                            .unwrap_or(&narrowed);
                        arithmetic_domains::validate_arithmetic_domains(
                            program,
                            machine,
                            current_state,
                            *argument,
                            argument_env,
                            None,
                            numerics::arithmetic::ArithmeticDomain::Exact,
                            &format!(
                                "machine `{}` state `{state_name}` transition argument",
                                machine.name
                            ),
                            diagnostics,
                        );
                    }
                }
            }
        }
    }
}
