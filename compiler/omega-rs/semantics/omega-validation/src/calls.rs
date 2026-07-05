use crate::expression_types::{
    argument_matches_type_reference_handle, cross_class_conflict, expression_type_name_handle,
};
use crate::locals::WritableRoots;
use crate::places::declared_place_type;
use crate::struct_literals::data_declares_field;
use crate::properties::{
    declared_property_names, referenced_type_parameter, type_satisfies_declared_property,
};
use crate::symbols::{MachineSymbols, TopLevelSymbols};
use crate::type_references::type_reference_label;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::name::Identifier;
use omega_typed_trees::signature::StateParameter;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{
    StatementNode, TableCall, TransitionGuardNode, TransitionTargetNode,
};
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate_call_node(
    program: &TypedTrees,
    call: &TableCall,
    current_machine: &omega_typed_trees::machine::Machine,
    state_name: &str,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_call_arguments_handles(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&State>,
    arguments: &[ExpressionHandle],
    target_name: &str,
    parameters: &[StateParameter],
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
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
        } else if let Some(parameter_primitive) =
            program.primitive_type_reference(parameter.type_reference)
            && let Some((value_class, target_class)) = cross_class_conflict(
                program,
                current_machine,
                current_state,
                *argument,
                parameter_primitive,
            )
        {
            // The shape gate blanket-accepts place/name arguments (`self.field`,
            // a local) against ANY primitive parameter, so a `bool` field passed
            // for an `i32` parameter slips through and the backend silently reads
            // it as garbage. Resolve the argument's scalar class and reject a
            // cross-class store, exactly as the assignment path does.
            diagnostics.push(Diagnostic::error(format!(
                "argument `{}` for state `{}` stores {} into a `{}` parameter, which holds {}",
                parameter.name,
                target_name,
                value_class.describe(),
                parameter_primitive.name(),
                target_class.describe(),
            )));
        }
    }

    let _ = (writable_roots, diagnostics);
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
/// call arguments.  Only the BOUND check (type-parameter property
/// satisfaction) is enforced here; full argument count/type checking for
/// value-position calls is a documented frontier -- see
/// `validate_call_arguments_handles`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_value_position_calls(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in program.statement_table.statements(state.statement_nodes) {
        match statement {
            StatementNode::Assignment(assignment) => {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    writable_roots,
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
                    *argument,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
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
                binary.right,
                diagnostics,
            );
        }
        ExpressionNode::Cast(cast) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
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
                *inner,
                diagnostics,
            );
        }
        ExpressionNode::Unary(unary) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
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
            ExpressionNode::Name(path) => {
                program.expression_table.name_path_members(path.members)
            }
            _ => &[],
        }
    };

    let arguments = program.expression_table.expression_handles(call.arguments);

    // Self-call or `self`-prefixed call: the callee is a state of the
    // current machine, an attached-data sibling machine, or a free machine.
    // Mirrors the same three-way fallback in `validate_call_node`.
    if receiver_members.is_empty()
        || matches!(receiver_members, [r] if r.as_str() == "self")
    {
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
    }

    let _ = writable_roots;
}
