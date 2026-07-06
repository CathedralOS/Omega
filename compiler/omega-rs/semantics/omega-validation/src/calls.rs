use crate::arithmetic_domains::{self, ValueEnv};
use crate::expression_types::{
    argument_matches_type_reference_handle, expression_type_name_handle, report_cross_class_store,
    report_data_type_conflict,
};
use crate::locals::WritableRoots;
use crate::places::declared_place_type;
use crate::properties::{
    declared_property_names, referenced_type_parameter, type_satisfies_declared_property,
};
use crate::struct_literals::data_declares_field;
use crate::symbols::{MachineSymbols, TopLevelSymbols};
use crate::type_references::type_reference_label;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::DataMember;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::name::Identifier;
use omega_typed_trees::signature::StateParameter;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{
    StatementNode, TableCall, TransitionGuardNode, TransitionTargetNode,
};
use omega_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_call_node(
    program: &TypedTrees,
    call: &TableCall,
    current_machine: &omega_typed_trees::machine::Machine,
    state_name: &str,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    value_env: &ValueEnv,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let receiver_members = program.statement_table.name_path_members(call.receiver);
    let arguments = program.statement_table.expression_handles(call.arguments);

    // `Schema::encode(...)` / `Schema::decode(...)`: the wire
    // module owns the synthesized encoder/decoder calls' diagnostics
    // (chapter 20, wire stage 2).
    if crate::wire::validate_wire_schema_call(
        program,
        call,
        current_machine,
        machine_symbols.state(state_name),
        diagnostics,
    ) {
        return;
    }

    if receiver_members.is_empty()
        || matches!(receiver_members, [receiver] if receiver.as_str() == "self")
    {
        if let Some(state) = machine_symbols.state(&call.target) {
            validate_result_use(
                program,
                call,
                state.name.as_str(),
                state.return_type,
                diagnostics,
            );
            validate_call_arguments_handles(
                program,
                current_machine,
                machine_symbols.state(state_name),
                value_env,
                arguments,
                state.name.as_str(),
                program.state_parameters(state),
                writable_roots,
                diagnostics,
            );
            validate_machine_call_type_parameter_bounds(
                program,
                symbols,
                current_machine,
                state,
                state.name.as_str(),
                arguments,
                current_machine,
                machine_symbols.state(state_name),
                diagnostics,
            );
            return;
        }

        let attached_state = current_machine
            .attached_data
            .as_ref()
            .and_then(|attached_data| {
                symbols.attached_machine_state(
                    program,
                    attached_data.as_str(),
                    call.target.as_str(),
                )
            });
        // A receiverless call can also target a FREE top-level machine
        // (`machine compute(item: &Item) -> i32`, called as `compute(item)`);
        // its implicit entry state carries the parameters and return type.
        let Some((callee_machine, state)) = attached_state
            .or_else(|| free_machine_entry_state(program, symbols, call.target.as_str()))
        else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has no local state `{}`",
                current_machine.name, call.target
            )));
            return;
        };

        // Diagnostics name the call as spelled (`compute`), not the free
        // machine's generated entry-state name (`entry`).
        validate_result_use(
            program,
            call,
            call.target.as_str(),
            state.return_type,
            diagnostics,
        );
        validate_call_arguments_handles(
            program,
            current_machine,
            machine_symbols.state(state_name),
            value_env,
            arguments,
            call.target.as_str(),
            program.state_parameters(state),
            writable_roots,
            diagnostics,
        );
        validate_machine_call_type_parameter_bounds(
            program,
            symbols,
            callee_machine,
            state,
            call.target.as_str(),
            arguments,
            current_machine,
            machine_symbols.state(state_name),
            diagnostics,
        );
        return;
    }

    let receiver = receiver_members
        .last()
        .map(|member| member.as_str())
        .unwrap_or_default();
    let receiver_type = machine_symbols.contained_type(receiver);

    if let Some(platform) = receiver_type.and_then(|type_name| symbols.platform(type_name)) {
        let Some(state_signature) = program
            .platform_state_signatures(platform)
            .iter()
            .find(|state| state.name == call.target)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "platform `{}` has no state `{}`",
                platform.name, call.target
            )));
            return;
        };

        validate_result_use(
            program,
            call,
            &state_signature.name,
            state_signature.return_type,
            diagnostics,
        );
        validate_call_arguments_handles(
            program,
            current_machine,
            machine_symbols.state(state_name),
            value_env,
            arguments,
            &state_signature.name,
            program.state_signature_parameters(state_signature),
            writable_roots,
            diagnostics,
        );
        return;
    }

    if let Some(machine) = receiver_type
        .and_then(|type_name| symbols.machine(type_name))
        .or_else(|| symbols.machine(receiver))
    {
        if let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.name == call.target)
        {
            validate_result_use(program, call, &state.name, state.return_type, diagnostics);
            validate_call_arguments_handles(
                program,
                current_machine,
                machine_symbols.state(state_name),
                value_env,
                arguments,
                &state.name,
                program.state_parameters(state),
                writable_roots,
                diagnostics,
            );
            validate_machine_call_type_parameter_bounds(
                program,
                symbols,
                machine,
                state,
                state.name.as_str(),
                arguments,
                current_machine,
                machine_symbols.state(state_name),
                diagnostics,
            );
            return;
        };

        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` has no state `{}`",
            machine.name, call.target
        )));
        return;
    }

    if let Some((callee_machine, state)) = receiver_type.and_then(|type_name| {
        symbols.attached_machine_state(program, type_name, call.target.as_str())
    }) {
        validate_result_use(program, call, &state.name, state.return_type, diagnostics);
        validate_call_arguments_handles(
            program,
            current_machine,
            machine_symbols.state(state_name),
            value_env,
            arguments,
            &state.name,
            program.state_parameters(state),
            writable_roots,
            diagnostics,
        );
        validate_machine_call_type_parameter_bounds(
            program,
            symbols,
            callee_machine,
            state,
            state.name.as_str(),
            arguments,
            current_machine,
            machine_symbols.state(state_name),
            diagnostics,
        );
        return;
    }

    // Boundary/trait receivers (e.g. `self.console.exit_process(0)`) resolve to a
    // trait machine signature. Strict result use plus argument validation apply
    // here -- a boundary is still a typed call, and a cross-class argument
    // (`exit_process(self.bool_field)`) would otherwise reach the host encoder as
    // a raw byte and be read as garbage with no frontend error.
    if let Some(signature) = receiver_type
        .and_then(|type_name| symbols.trait_definition(type_name))
        .and_then(|trait_definition| {
            program
                .trait_machine_signatures(trait_definition)
                .iter()
                .find(|signature| signature.name == call.target)
        })
    {
        validate_result_use(
            program,
            call,
            &signature.name,
            signature.return_type,
            diagnostics,
        );
        validate_call_arguments_handles(
            program,
            current_machine,
            machine_symbols.state(state_name),
            value_env,
            arguments,
            &signature.name,
            program.state_signature_parameters(signature),
            writable_roots,
            diagnostics,
        );
        return;
    }

    let _ = diagnostics;
}

/// The FREE top-level machine named `target` and its entry state (`machine
/// compute(item: &Item) -> i32 { ... }`), or None. The parser names a free
/// machine's implicit entry state `entry`; explicit entry states matching the
/// call target name win first.
fn free_machine_entry_state<'program>(
    program: &'program TypedTrees,
    symbols: &TopLevelSymbols<'program>,
    target: &str,
) -> Option<(&'program Machine, &'program State)> {
    let machine = symbols.machine(target)?;
    if machine.attached_data.is_some() {
        return None;
    }

    let states = program.machine_states(machine);
    states
        .iter()
        .find(|state| state.name.as_str() == target)
        .or_else(|| states.iter().find(|state| state.name.as_str() == "entry"))
        .or_else(|| states.first())
        .map(|state| (machine, state))
}

/// FROZEN DECISION 13 residue -- machine-call monomorphization arguments.
/// A bracket bound on a callee type parameter (`machine copy_it<T [copy]>`)
/// must hold for the concrete type the call instantiates `T` with. There is
/// no explicit type-argument list at call sites today: instantiation is
/// positional inference, so each non-self parameter whose declared type names
/// a bounded callee type parameter (`x: &T`, `x: T`, `[T; N]`, constrained
/// forms) pins `T` to the matching argument's declared place type, and that
/// concrete type must satisfy every bound via the same structural check the
/// data-instantiation path uses (`type_satisfies_declared_property`). An
/// in-scope bounded parameter of the CALLER counts as carrying its bound, so
/// a generic caller may forward its own `U [copy]`.
///
/// FRONTIER (stands down silently, like the wire argument checks): arguments
/// the declared-place scope cannot type (call results, indexed elements,
/// literals, nested member chains), parameters whose type buries `T` inside a
/// generic (`Box<T>`) or slice (`&[T]`).
///
/// Both STATEMENT-position calls (via `validate_call_node`) and VALUE-position
/// calls (via `validate_value_position_calls` + `scan_expression_calls`) now
/// reach this function.
#[allow(clippy::too_many_arguments)]
fn validate_machine_call_type_parameter_bounds(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    callee_machine: &Machine,
    callee_state: &State,
    target_name: &str,
    arguments: &[ExpressionHandle],
    current_machine: &Machine,
    current_state: Option<&State>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let type_parameters = program.machine_type_parameters(callee_machine);
    if type_parameters.is_empty() {
        return;
    }

    let caller_type_parameters = program.machine_type_parameters(current_machine);

    for (argument, parameter) in arguments.iter().zip(
        program
            .state_parameters(callee_state)
            .iter()
            .filter(|parameter| !parameter.is_self),
    ) {
        let Some(type_parameter) =
            referenced_type_parameter(program, type_parameters, parameter.type_reference)
        else {
            continue;
        };
        let bound_names = declared_property_names(&type_parameter.bounds);
        if bound_names.is_empty() {
            continue;
        }
        let Some(argument_type) =
            declared_place_type(program, current_machine, current_state, *argument)
        else {
            continue;
        };
        for property in &bound_names {
            if type_satisfies_declared_property(
                program,
                symbols,
                caller_type_parameters,
                argument_type,
                property,
            ) {
                continue;
            }
            diagnostics.push(Diagnostic::error(format!(
                "type parameter `{} [{}]` of machine `{target_name}` was instantiated with `{}`, which does not declare `[{property}]`",
                type_parameter.name,
                bound_names.join(", "),
                type_reference_label(program, argument_type)
            )));
        }
    }
}

/// FROZEN DECISION 9 -- STRICT RESULT USE: a statement-position call whose callee
/// returns a non-unit value must not silently drop that value. Intentional
/// discards are spelled `_ = call();` (which sets `discards_result`). "Non-unit"
/// means the resolved callee declares a return type (`-> T`) that is not `()`.
fn validate_result_use(
    program: &TypedTrees,
    call: &TableCall,
    target_name: &str,
    return_type: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if call.discards_result || !return_type.is_valid() {
        return;
    }

    if matches!(
        program.type_reference_table.type_reference(return_type),
        TypeReferenceNode::Unit
    ) {
        return;
    }

    diagnostics.push(Diagnostic::error(format!(
        "call to `{target_name}` discards its non-unit `{}` result; consume the value or discard it explicitly with `_ = {target_name}(...);`",
        program.display_type_reference_with_constraints(return_type)
    )));
}

/// Reports the "state `X` expects N argument(s), got M" error when `arguments`
/// does not match the callee's callable (non-`self`) parameter count, returning
/// `true` on a mismatch so callers skip the per-argument checks (which zip the
/// two and would misalign). SINGLE SOURCE OF TRUTH for call arity across the
/// statement-position (`validate_call_arguments_handles`) and value-position
/// (`validate_value_call_argument_classes`) paths.
pub(crate) fn report_argument_count_mismatch(
    target_name: &str,
    parameters: &[StateParameter],
    arguments: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let callable_parameter_count = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();

    if arguments.len() != callable_parameter_count {
        diagnostics.push(Diagnostic::error(format!(
            "state `{}` expects {} argument(s), got {}",
            target_name,
            callable_parameter_count,
            arguments.len()
        )));
        return true;
    }
    false
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_call_arguments_handles(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&State>,
    value_env: &ValueEnv,
    arguments: &[ExpressionHandle],
    target_name: &str,
    parameters: &[StateParameter],
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if report_argument_count_mismatch(target_name, parameters, arguments, diagnostics) {
        return;
    }

    for (argument, parameter) in arguments
        .iter()
        .zip(parameters.iter().filter(|parameter| !parameter.is_self))
    {
        let is_mutable = matches!(
            program.expression_table.expression(*argument),
            ExpressionNode::Mutable(_)
        );

        if parameter.is_mutable && !is_mutable {
            continue;
        }

        if !parameter.is_mutable && is_mutable {
            continue;
        }

        let expected_type =
            program.display_type_reference_with_constraints(parameter.type_reference);

        if !argument_matches_type_reference_handle(program, *argument, parameter.type_reference) {
            diagnostics.push(Diagnostic::error(format!(
                "argument `{}` for state `{}` expects `{}`, got `{}`",
                parameter.name,
                target_name,
                expected_type,
                expression_type_name_handle(program, *argument)
            )));
        } else if !report_cross_class_argument(
            program,
            current_machine,
            current_state,
            *argument,
            parameter,
            target_name,
            diagnostics,
        ) {
            // The shape gate blanket-accepts place/name arguments (`self.field`,
            // a local) against ANY primitive parameter, so a `bool` field passed
            // for an `i32` parameter slips through and the backend silently reads
            // it as garbage. Resolve the argument's scalar class and reject a
            // cross-class store, exactly as the assignment path does. Only args
            // that PASSED the shape gate reach here, so cross-class LITERALS (which
            // the shape gate already rejects above) are not double-reported. When
            // the classes DO agree (a same-class numeric arg), check the narrowing
            // obligation -- `take_i8(self.i64_field)` would silently truncate.
            report_narrowing_argument(
                program,
                current_machine,
                current_state,
                value_env,
                *argument,
                parameter,
                target_name,
                diagnostics,
            );
        }
        // An array-literal argument (`sink([300, ..])`) is checked element-wise
        // against the parameter's `[T; N]` element type -- the scalar guards above
        // no-op on a non-primitive (array) parameter.
        if let Some(state) = current_state {
            crate::struct_literals::validate_array_literal_elements(
                program,
                current_machine,
                state,
                *argument,
                parameter.type_reference,
                diagnostics,
            );
        }
        // Nominal guard: the shape gate blanket-accepts a place/name argument
        // against ANY `Named` parameter, so `take_foo(&self.bar)` (a `&Bar` for a
        // `&Foo` parameter) is silently accepted and reads the wrong storage.
        // Reject when both parameter and argument resolve to concrete data types
        // that differ (every non-data form is skipped, so no false positive on
        // trait/generic parameters or computed arguments).
        let slot_context = format!("argument `{}` for state `{target_name}`", parameter.name);
        report_data_type_conflict(
            program,
            current_machine,
            current_state,
            *argument,
            parameter.type_reference,
            &slot_context,
            "argument",
            diagnostics,
        );
        // NOTE: an array/scalar SHAPE check does NOT belong at the argument position
        // -- `&self.msg` (address-of a `[u8; N]` buffer) passed to an `addr`/pointer
        // param is a valid array-value-into-scalar-target flow, and boundary/host
        // text params (`addr`, byte slices) accept text/byte values freely. The
        // reference/`addr` and text representations make args a false-positive
        // minefield; a wrong-count/type arg is already caught here + at the backend.
    }

    let _ = (writable_roots, diagnostics);
}

/// Reject a single ARGUMENT whose scalar class conflicts with its `parameter`'s
/// primitive type -- a `bool`/text value passed where a numeric parameter is
/// expected (or vice versa), which the backend would otherwise read as garbage.
/// Shared by the statement/transition path (`validate_call_arguments_handles`)
/// and the value-position path (`validate_value_call_argument_classes`). Returns
/// `true` if it reported. A non-primitive parameter (a data reference, a struct)
/// or an unresolvable argument class yields `false` -- no report.
fn report_cross_class_argument(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&State>,
    argument: ExpressionHandle,
    parameter: &StateParameter,
    target_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let Some(parameter_primitive) = program.primitive_type_reference(parameter.type_reference)
    else {
        return false;
    };
    let slot_context = format!("argument `{}` for state `{target_name}`", parameter.name);
    report_cross_class_store(
        program,
        Some(current_machine),
        current_state,
        argument,
        parameter_primitive,
        &slot_context,
        "parameter",
        diagnostics,
    )
}

/// Reject a single numeric ARGUMENT that NARROWS into its `parameter` -- a wider
/// value (`self.big: i64 = 300`) passed where a narrower integer parameter is
/// expected (`x: i8`), which the backend would otherwise silently truncate
/// (300 -> 44). Decision-17's narrowing proof obligation, applied at the call
/// boundary exactly as `check_narrowing_assignment` applies it at the assignment
/// boundary. Honors dominating guards via the flow-sensitive `value_env`, so a
/// guarded-in-range argument is not flagged. The argument's OWN arithmetic is
/// analyzed into a THROWAWAY buffer, so only the narrowing check contributes a
/// diagnostic here (an arg's exact-overflow obligation is not this gate's job).
fn report_narrowing_argument(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&State>,
    value_env: &ValueEnv,
    argument: ExpressionHandle,
    parameter: &StateParameter,
    target_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(parameter_primitive) = program.primitive_type_reference(parameter.type_reference)
    else {
        return;
    };
    let owner = format!(
        "machine `{}` state `{target_name}` argument `{}`",
        current_machine.name, parameter.name,
    );
    arithmetic_domains::check_value_narrowing(
        program,
        current_machine,
        current_state,
        argument,
        parameter_primitive,
        value_env,
        &owner,
        diagnostics,
    );
}

/// Reject cross-class scalar ARGUMENTS at a VALUE-position call site
/// (`let r = self.f(self.bool_field)`). The value-position path validates only
/// type-parameter bounds, so the same cross-class hole the statement/transition
/// paths had applies here. Unlike `validate_call_arguments_handles` there is no
/// shape gate ahead of this, so it also covers literal arguments.
fn validate_value_call_argument_classes(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: &State,
    value_env: &ValueEnv,
    arguments: &[ExpressionHandle],
    callee_state: &State,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // NOTE: a void-callee-in-value-position check (`let r: i32 = self.act()` for a
    // unit `act`, which silently binds a ZII 0) does NOT belong here -- the resolved
    // `callee_state.return_type` is EMPTY for terminating/transition machine shapes
    // (a `-> usize` after a `terminates` block, or a value returned via a transition
    // arm), so keying off it false-positives on machines that DO return a value
    // (verified: it flagged `weaken -> usize` and `table_size() -> usize`). That
    // check needs reliable per-call return-type resolution (the complete value-call
    // resolver), the same frontier as the nonexistent-value-call gap in TASKS.

    // Arity: value-position calls (`let r = self.pick(1)`) reach only this path,
    // never `validate_call_arguments_handles`, so without this a wrong argument
    // count compiled silently (a missing arg then read its ZII default). Safe here
    // because this function runs only on a RESOLVED callee -- the resolver's blind
    // spots fall through earlier without reaching it.
    if report_argument_count_mismatch(
        callee_state.name.as_str(),
        program.state_parameters(callee_state),
        arguments,
        diagnostics,
    ) {
        return;
    }

    for (argument, parameter) in arguments.iter().zip(
        program
            .state_parameters(callee_state)
            .iter()
            .filter(|parameter| !parameter.is_self),
    ) {
        // Class check first; narrowing only when the classes agree (a same-class
        // numeric arg), so a cross-class arg is not double-reported. Mirrors the
        // statement/transition path in `validate_call_arguments_handles`.
        if !report_cross_class_argument(
            program,
            current_machine,
            Some(current_state),
            *argument,
            parameter,
            callee_state.name.as_str(),
            diagnostics,
        ) {
            report_narrowing_argument(
                program,
                current_machine,
                Some(current_state),
                value_env,
                *argument,
                parameter,
                callee_state.name.as_str(),
                diagnostics,
            );
        }
        // Nominal guard (value-position complement): `let r = self.take_foo(&self.bar)`
        // with a `&Foo` parameter is silently accepted -- the same wrong-data-type
        // hole the statement/transition path has.
        let slot_context = format!(
            "argument `{}` for state `{}`",
            parameter.name,
            callee_state.name.as_str()
        );
        report_data_type_conflict(
            program,
            current_machine,
            Some(current_state),
            *argument,
            parameter.type_reference,
            &slot_context,
            "argument",
            diagnostics,
        );
        // (No array/scalar shape check here -- see the note in
        // `validate_call_arguments_handles`: `&buffer`-into-`addr` and text/byte args
        // make the argument position a false-positive minefield.)
    }
}

/// FROZEN DECISION 13 residue (value-position complement of `validate_call_node`).
///
/// Walk every expression in every statement of `state` and enforce
/// machine-call type-parameter bounds for VALUE-position calls
/// (`let r = self.pick(&self.h)`).  These never reach `validate_call_node`
/// because they appear as `ExpressionNode::Call` inside expression trees,
/// not as top-level `StatementNode::Call` nodes.
///
/// Scope: covers all expression positions that feed into statements
/// (LocalData initializers, assignment values/targets, guard expressions,
/// transition arguments, terminal expressions) and recurses into nested
/// call arguments.  Enforces the type-parameter BOUND check plus, for a
/// RESOLVED callee, argument arity (`report_argument_count_mismatch`) and the
/// per-argument class/narrowing/nominal checks (`validate_value_call_argument_classes`).
/// The remaining frontier is the UNRESOLVED callee: a value call whose target
/// no branch resolves falls through silently (the nonexistent-value-call gap),
/// which needs the complete value-call target resolver.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_value_position_calls(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement: &StatementNode,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    value_env: &ValueEnv,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        StatementNode::Assignment(assignment) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                assignment.value,
                diagnostics,
            );
            // target is a place (Name/Member/Indexed), no calls to validate
        }
        StatementNode::Call(call) => {
            // Statement-position call arguments may themselves be value calls.
            for argument in program.statement_table.expression_handles(call.arguments) {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    value_env,
                    *argument,
                    diagnostics,
                );
            }
        }
        StatementNode::Expression(expression) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                *expression,
                diagnostics,
            );
        }
        StatementNode::LocalData(local_data) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                local_data.initial_value,
                diagnostics,
            );
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    value_env,
                    guard,
                    diagnostics,
                );
            }
            for target_handle in [transition.target, transition.continuation] {
                if !target_handle.is_valid() {
                    continue;
                }
                let target = program.statement_table.transition_target(target_handle);
                match target {
                    TransitionTargetNode::Named { arguments, .. } => {
                        for argument in program.statement_table.expression_handles(*arguments) {
                            scan_expression_calls(
                                program,
                                machine,
                                state,
                                machine_symbols,
                                symbols,
                                writable_roots,
                                value_env,
                                *argument,
                                diagnostics,
                            );
                        }
                    }
                    TransitionTargetNode::Value(expression) => {
                        scan_expression_calls(
                            program,
                            machine,
                            state,
                            machine_symbols,
                            symbols,
                            writable_roots,
                            value_env,
                            *expression,
                            diagnostics,
                        );
                    }
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}

/// Is `name` a legal single-segment BARE name in the body of `machine`?
///
/// A bare `Name` resolves to a field (implicit `self`), a top-level symbol (type
/// / machine / platform / trait), an enum case constant (`Red`), or a local/
/// parameter. The binding scope for locals and parameters is the WHOLE machine,
/// not one state: a sub-state legitimately reads a parameter or `let` declared on
/// the machine's entry (or an ancestor) state (`state nonpos` reading the entry
/// state's `n`). We therefore scan every state's parameters and `LocalData`.
///
/// The allow-list is deliberately GENEROUS -- scanning ALL states over-approximates
/// the true lexical scope, so an out-of-scope-but-declared name is accepted (an
/// UNDER-rejection, never a false rejection of a real name). The sole goal is to
/// catch a name that exists NOWHERE (a typo reading as 0/garbage).
fn is_known_bare_name(
    program: &TypedTrees,
    machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    name: &str,
) -> bool {
    // Field of the receiver data (bare `fld` == `self.fld`), owned data, or a
    // contained object.
    if machine_symbols.has_member(name)
        || machine_symbols.has_owned_data(name)
        || machine_symbols.contained_type(name).is_some()
    {
        return true;
    }
    // Top-level symbol: a type, machine, platform, or trait spelled bare.
    if symbols.has_type(name)
        || symbols.machine(name).is_some()
        || symbols.platform(name).is_some()
        || symbols.trait_definition(name).is_some()
    {
        return true;
    }
    // Enum case constant used bare (`let s: Signal = Red`).
    for definition in program.data_definitions() {
        for member in program.data_members(definition) {
            if let DataMember::Variant(variant) = member
                && variant.name.as_str() == name
            {
                return true;
            }
        }
    }
    // Parameter or local declared on ANY state of this machine (whole-machine
    // scope -- see the doc comment).
    for other in program.machine_states(machine) {
        for parameter in program.state_parameters(other) {
            if parameter.name.as_str() == name {
                return true;
            }
        }
        for statement in program.statement_table.statements(other.statement_nodes) {
            if let StatementNode::LocalData(local) = statement
                && local.name.as_str() == name
            {
                return true;
            }
        }
    }
    false
}

/// Whether `operand`'s type is a float (`f32`/`f64`): a float literal, or a place
/// whose declared type resolves to a float primitive. Looks through a `Mutable`
/// wrapper. Used to reject bitwise/shift/modulo on floats.
fn expression_is_float_typed(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    operand: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(operand) {
        ExpressionNode::Float(_) => true,
        ExpressionNode::Mutable(inner) => {
            expression_is_float_typed(program, machine, state, *inner)
        }
        _ => crate::places::declared_place_type(program, machine, Some(state), operand)
            .and_then(|type_reference| program.primitive_type_reference(type_reference))
            .is_some_and(|primitive| matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)),
    }
}

/// Recursively scan `expression` for `ExpressionNode::Call` nodes and
/// validate machine-call type-parameter bounds for each one found.
#[allow(clippy::too_many_arguments)]
fn scan_expression_calls(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    value_env: &ValueEnv,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    // Unknown-field READ: a direct `self.<field>` read of a nonexistent field (a typo)
    // gets a clear error instead of silently passing type-check. Mirrors the
    // assignment-target write check (places.rs): scoped to a direct `self.<field>`
    // against the machine's top-level data fields, versioned data excluded. Nested
    // `self.a.b` (checked at `a` when the recursion reaches the receiver) and non-self
    // members are left alone. The recursion continues afterward.
    if let Some(field_name) = crate::places::direct_self_field_member(program, expression)
        && let Some(data) = crate::places::machine_attached_data(program, machine)
        && !data_declares_field(program, data, field_name)
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` reads `self.{field_name}`, but data `{}` has no field \
             `{field_name}` (check the spelling of the field name)",
            machine.name.as_str(),
            state.name.as_str(),
            data.name.as_str()
        )));
    }
    // Member / index access on a PRIMITIVE-scalar receiver: a number or bool has no
    // fields, no `.len`, and is not indexable, so `x.field` / `x.len` / `x[0]` on an
    // `i32` local silently read a ZII 0. Reject when the receiver's declared type
    // resolves to a numeric/bool primitive; an UNRESOLVED receiver (or a struct /
    // array / slice / String receiver) is left alone -- struct-field lookup and
    // String indexing are separate. String is excluded because text carries a
    // `.len`/byte view.
    let primitive_access = match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => Some((
            member.receiver,
            format!("member `{}`", member.member.as_str()),
        )),
        ExpressionNode::Indexed(indexed) => Some((indexed.collection, "an index".to_owned())),
        _ => None,
    };
    if let Some((receiver, access)) = primitive_access
        && let Some(receiver_type) =
            crate::places::declared_place_type(program, machine, Some(state), receiver)
        && let Some(primitive) = program.primitive_type_reference(receiver_type)
        && primitive != PrimitiveType::String
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` accesses {access} of a `{}` value, but a primitive scalar \
             has no members or elements",
            machine.name.as_str(),
            state.name.as_str(),
            primitive.name(),
        )));
    }
    // Unknown BARE NAME: a SINGLE-segment name (`undeclared_var`, not `self.x` or
    // `Type::Case`) that resolves to nothing is otherwise silently accepted and reads
    // as 0/garbage. Reject it when it is none of the legal bare-name forms. The scope
    // is the whole MACHINE (a sub-state may read the entry state's params/locals), and
    // `true`/`false` are single-segment `Name` nodes here, so they are skipped. The
    // allow-list is deliberately GENEROUS -- an unrecognised valid form only
    // UNDER-rejects (misses a typo), never falsely rejects a real name.
    if let ExpressionNode::Name(path) = program.expression_table.expression(expression)
        && let [only] = program.expression_table.name_path_members(path.members)
    {
        let name = only.as_str();
        if name != "self"
            && name != "true"
            && name != "false"
            && !is_known_bare_name(program, machine, machine_symbols, symbols, name)
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` uses `{name}`, which is not a declared local, \
                 parameter, field, or type (check the spelling)",
                machine.name.as_str(),
                state.name.as_str(),
            )));
        }
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => {
            let call = call.clone();
            validate_expression_call_bounds(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                &call,
                diagnostics,
            );
            // Recurse into the receiver and arguments (nested calls).
            if call.receiver.is_valid() {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    value_env,
                    call.receiver,
                    diagnostics,
                );
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    value_env,
                    *argument,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            let (left, right, operator) = (binary.left, binary.right, binary.operator);
            crate::expression_types::report_cross_class_binary_operands(
                program,
                machine,
                Some(state),
                left,
                right,
                diagnostics,
            );
            // Non-`+` arithmetic / shift / bitwise on TEXT operands (`s - t`, `s * s`)
            // is meaningless -- text supports only `+` (concat), `==`, `!=`.
            crate::expression_types::report_invalid_text_operator(
                program,
                machine,
                Some(state),
                operator,
                left,
                right,
                diagnostics,
            );
            // Ordering / arithmetic / bitwise on an array operand (`xs < ys`) is
            // meaningless -- arrays cannot carry domain operators, so it otherwise
            // lowers to a garbage byte op.
            crate::expression_types::report_array_operator_operands(
                program,
                machine,
                Some(state),
                operator,
                left,
                right,
                diagnostics,
            );
            // Arithmetic / ordering on a STRUCT with no declared operator (`P + P`
            // for a plain `data P`) likewise lowers to a garbage byte op; a domain
            // operator (`Quantity + Quantity`) stays valid.
            crate::expression_types::report_undeclared_struct_operator(
                program,
                machine,
                Some(state),
                operator,
                left,
                diagnostics,
            );
            // Bitwise / shift / modulo are not defined for FLOAT operands: the
            // interpreter rejects them ("float modulo/shift/bitwise not supported")
            // and the backend cannot encode them, but `--check` passed silently.
            // Reject at check with a clear message (the set matches the interpreter's
            // exactly). If float bit-ops are ever added, update the interpreter and
            // this together.
            use omega_typed_trees::expression::BinaryOperator;
            if matches!(
                operator,
                BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseXor
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::Modulo
            ) && (expression_is_float_typed(program, machine, state, left)
                || expression_is_float_typed(program, machine, state, right))
            {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}` applies `{operator:?}` to a float operand, but \
                     bitwise, shift, and modulo operators are defined for integers only",
                    machine.name.as_str(),
                    state.name.as_str(),
                )));
            }
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                binary.left,
                diagnostics,
            );
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                binary.right,
                diagnostics,
            );
        }
        ExpressionNode::Cast(cast) => {
            // An `as` target must be a scalar primitive. A cast to an unknown type
            // (`x as Bogus`) or a non-scalar type (`x as Foo`, a data type) otherwise
            // compiles SILENTLY as identity -- the target resolves to no primitive, so
            // the conversion is a no-op and the value passes through with the wrong type.
            if let Some(target) = program
                .expression_table
                .name_path_members(cast.target_type)
                .last()
                && PrimitiveType::from_name(target.as_str()).is_none()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}` casts with `as {target}`, but `{target}` is not \
                     a scalar type; `as` converts between scalar types only",
                    machine.name.as_str(),
                    state.name.as_str(),
                )));
            }
            // `<number> as bool` reinterprets bits into a non-{0,1} bool silently;
            // there is no number->bool `as` conversion (write `n != 0`).
            if program
                .expression_table
                .name_path_members(cast.target_type)
                .last()
                .and_then(|target| PrimitiveType::from_name(target.as_str()))
                == Some(PrimitiveType::Bool)
            {
                crate::expression_types::report_number_to_bool_cast(
                    program,
                    machine,
                    Some(state),
                    cast.value,
                    diagnostics,
                );
            }
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                cast.value,
                diagnostics,
            );
        }
        ExpressionNode::Indexed(indexed) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                indexed.collection,
                diagnostics,
            );
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                indexed.index,
                diagnostics,
            );
        }
        ExpressionNode::Member(member) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                member.receiver,
                diagnostics,
            );
        }
        ExpressionNode::Mutable(inner) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                *inner,
                diagnostics,
            );
        }
        ExpressionNode::Unary(unary) => {
            // Logical `!` is bool-only (bitwise-not is `~`); reject `!<non-bool>`.
            crate::expression_types::report_non_bool_logical_not(
                program,
                machine,
                Some(state),
                unary.operand,
                diagnostics,
            );
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                unary.operand,
                diagnostics,
            );
        }
        ExpressionNode::ArrayLiteral(elements) => {
            let elements = *elements;
            for element in program.expression_table.expression_handles(elements) {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    value_env,
                    *element,
                    diagnostics,
                );
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            let fields = literal.fields;
            for field in program.expression_table.struct_fields(fields) {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    value_env,
                    field.value,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Range(range) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                range.start,
                diagnostics,
            );
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                range.end,
                diagnostics,
            );
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

/// Enforce machine-call type-parameter bounds for a single VALUE-position
/// `ExpressionNode::Call`.  The receiver name path is extracted from the
/// receiver expression (must be a `Name` node with identifier segments).
/// Other receiver shapes (member chains, indexed, etc.) are beyond this
/// scope and stand down silently, consistent with the statement-path's
/// handling of unrecognised receivers.
/// A VALUE-position call to a GENERIC machine is not lowered natively yet: the
/// monomorphized result slot is never materialized, so the call silently yields
/// zero (#40). Reject it cleanly until machine monomorphization lands.
/// Statement-position calls to generic machines (which lower and run when the
/// body touches only concrete storage) are NOT affected -- this fence is only
/// reached from expression positions.
fn fence_generic_value_callee(
    program: &TypedTrees,
    callee_machine: &Machine,
    target: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if program.machine_type_parameters(callee_machine).is_empty() {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "a value call to the generic machine `{target}` is not supported natively yet (the \
         monomorphized result is never materialized): wrap it in a concrete machine, or use a \
         statement call",
    )));
}

#[allow(clippy::too_many_arguments)]
fn validate_expression_call_bounds(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: &State,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    value_env: &ValueEnv,
    call: &TableCallExpression,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Extract receiver name-path members from the receiver expression.
    // A self-call has no receiver (`call.receiver` is invalid) or an
    // explicit `self` Name; an external-receiver call has a Name whose
    // members name the receiver object.
    let receiver_members: &[Identifier] = if !call.receiver.is_valid() {
        &[]
    } else {
        match program.expression_table.expression(call.receiver) {
            ExpressionNode::Name(path) => program.expression_table.name_path_members(path.members),
            _ => &[],
        }
    };

    let arguments = program.expression_table.expression_handles(call.arguments);

    // Self-call or `self`-prefixed call: the callee is a state of the
    // current machine, an attached-data sibling machine, or a free machine.
    // Mirrors the same three-way fallback in `validate_call_node`.
    if receiver_members.is_empty() || matches!(receiver_members, [r] if r.as_str() == "self") {
        if let Some(callee_state) = machine_symbols.state(call.target.as_str()) {
            validate_machine_call_type_parameter_bounds(
                program,
                symbols,
                current_machine,
                callee_state,
                callee_state.name.as_str(),
                arguments,
                current_machine,
                Some(current_state),
                diagnostics,
            );
            validate_value_call_argument_classes(
                program,
                current_machine,
                current_state,
                value_env,
                arguments,
                callee_state,
                diagnostics,
            );
            return;
        }

        // A self-call can also target a SIBLING machine that shares the same
        // attached data (`machine Main::pick<T [copy]>` called from
        // `machine Main::main`). The statement-position path uses
        // `symbols.attached_machine_state(program, attached_data, call.target)`.
        let attached_state = current_machine
            .attached_data
            .as_ref()
            .and_then(|attached_data| {
                symbols.attached_machine_state(
                    program,
                    attached_data.as_str(),
                    call.target.as_str(),
                )
            });

        if let Some((callee_machine, callee_state)) = attached_state {
            fence_generic_value_callee(program, callee_machine, call.target.as_str(), diagnostics);
            validate_machine_call_type_parameter_bounds(
                program,
                symbols,
                callee_machine,
                callee_state,
                call.target.as_str(),
                arguments,
                current_machine,
                Some(current_state),
                diagnostics,
            );
            validate_value_call_argument_classes(
                program,
                current_machine,
                current_state,
                value_env,
                arguments,
                callee_state,
                diagnostics,
            );
            return;
        }

        // Free machine call (`compute(item)` -- no `self.`, no receiver).
        if let Some((callee_machine, callee_state)) =
            free_machine_entry_state(program, symbols, call.target.as_str())
        {
            fence_generic_value_callee(program, callee_machine, call.target.as_str(), diagnostics);
            validate_machine_call_type_parameter_bounds(
                program,
                symbols,
                callee_machine,
                callee_state,
                call.target.as_str(),
                arguments,
                current_machine,
                Some(current_state),
                diagnostics,
            );
            validate_value_call_argument_classes(
                program,
                current_machine,
                current_state,
                value_env,
                arguments,
                callee_state,
                diagnostics,
            );
        }
        return;
    }

    let receiver_name = receiver_members
        .last()
        .map(|m| m.as_str())
        .unwrap_or_default();
    let receiver_type = machine_symbols.contained_type(receiver_name);

    // External machine receiver.
    if let Some(callee_machine) = receiver_type
        .and_then(|type_name| symbols.machine(type_name))
        .or_else(|| symbols.machine(receiver_name))
    {
        if let Some(callee_state) = program
            .machine_states(callee_machine)
            .iter()
            .find(|s| s.name == call.target)
        {
            fence_generic_value_callee(program, callee_machine, call.target.as_str(), diagnostics);
            validate_machine_call_type_parameter_bounds(
                program,
                symbols,
                callee_machine,
                callee_state,
                callee_state.name.as_str(),
                arguments,
                current_machine,
                Some(current_state),
                diagnostics,
            );
            validate_value_call_argument_classes(
                program,
                current_machine,
                current_state,
                value_env,
                arguments,
                callee_state,
                diagnostics,
            );
        }
        return;
    }

    // Attached-data machine receiver.
    if let Some((callee_machine, callee_state)) = receiver_type.and_then(|type_name| {
        symbols.attached_machine_state(program, type_name, call.target.as_str())
    }) {
        fence_generic_value_callee(program, callee_machine, call.target.as_str(), diagnostics);
        validate_machine_call_type_parameter_bounds(
            program,
            symbols,
            callee_machine,
            callee_state,
            callee_state.name.as_str(),
            arguments,
            current_machine,
            Some(current_state),
            diagnostics,
        );
        validate_value_call_argument_classes(
            program,
            current_machine,
            current_state,
            value_env,
            arguments,
            callee_state,
            diagnostics,
        );
    }

    let _ = writable_roots;
}
