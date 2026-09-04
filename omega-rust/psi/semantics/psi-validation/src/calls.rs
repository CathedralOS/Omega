use crate::arithmetic_domains::{self, ValueEnv};
use crate::expression_types::{
    argument_matches_type_reference_handle, expression_type_name_handle, report_cross_class_store,
    report_data_type_conflict,
};
use crate::locals::WritableRoots;
use crate::places::declared_place_type;
use crate::symbols::{MachineSymbols, TopLevelSymbols};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::signature::StateParameter;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{StatementNode, TableCall};
use psi_typed_trees::types::TypeReferenceHandle;

mod expression_scanning;
mod generic_bounds;
mod inline_assembly;
mod recursion;
mod result_use;
mod write_frames;

use expression_scanning::receiver_member_chain;
pub(crate) use expression_scanning::{
    declared_receiver_type_reference, report_local_receiver_value_call,
    report_nested_call_in_bound_value_call, validate_value_position_calls,
};
use generic_bounds::validate_machine_call_type_parameter_bounds;
pub(crate) use inline_assembly::validate_asm_value_destination;
use inline_assembly::{user_asm_contract, validate_asm_operand_constraint};
pub(crate) use recursion::{
    validate_proof_machine_recursion, validate_self_recursive_call_positions,
};
use result_use::validate_result_use;
use write_frames::machine_state_by_symbol;
pub use write_frames::{CallFrameResolver, frame_paths_overlap};
pub(crate) use write_frames::{
    boundary_trait_signature, conservative_call_written_paths, free_machine_entry_state,
    known_boundary_call_written_paths, known_call_written_paths, statement_value_expression_roots,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_call_node(
    program: &TypedTrees,
    call: &TableCall,
    current_machine: &psi_typed_trees::machine::Machine,
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

    // Asm intrinsic statements (`asm { hlt }`, `asm { out port, value }`)
    // desugar to calls on unnameable `asm#...` targets -- known-contract
    // instructions with FIXED shapes, validated here instead of against a
    // state signature. (`asm { in dest, port }` is an assignment whose value
    // is the `asm#port_in` call; the value-call path owns it.)
    if receiver_members.is_empty() && call.target.as_str().starts_with("asm#") {
        let control_write =
            psi_language_core::inline_assembly::AsmControlRegister::from_write_intrinsic_name(
                call.target.as_str(),
            );
        let (source_mnemonic, expected_arguments) = match control_write {
            Some(register) => (
                register
                    .write_mnemonic()
                    .expect("writable control-register intrinsic"),
                1,
            ),
            None => match call.target.as_str() {
                "asm#hlt" => ("hlt", 0),
                "asm#port_out" => ("out", 2),
                "asm#lfence" => ("lfence", 0),
                "asm#sfence" => ("sfence", 0),
                "asm#mfence" => ("mfence", 0),
                "asm#cli" => ("cli", 0),
                "asm#sti" => ("sti", 0),
                "asm#popfq" => ("popfq", 1),
                "asm#wrmsr" => ("wrmsr", 2),
                other => {
                    diagnostics.push(Diagnostic::error(format!(
                        "asm intrinsic `{other}` is not a statement form"
                    )));
                    return;
                }
            },
        };
        if arguments.len() != expected_arguments {
            diagnostics.push(Diagnostic::error(format!(
                "asm intrinsic `{}` takes {} operand(s), found {}",
                call.target,
                expected_arguments,
                arguments.len()
            )));
            return;
        }
        if control_write.is_some() || matches!(source_mnemonic, "out" | "popfq" | "wrmsr") {
            let contract = user_asm_contract(source_mnemonic);
            for (operand, constraint) in arguments.iter().zip(contract.operands.iter()) {
                validate_asm_operand_constraint(
                    program,
                    current_machine,
                    machine_symbols.state(state_name),
                    source_mnemonic,
                    *operand,
                    *constraint,
                    diagnostics,
                );
            }
        }
        return;
    }

    // `machine-self-call-cycle-ban` (settled 2026-07-13): a STATEMENT-position
    // call to the enclosing
    // machine's OWN ENTRY (`self.drip(n - 1);` as a trailing statement) is
    // tail recursion spelled as a call -- it lowered as a Nested-transition
    // loop and slipped the transition-arm fence. "Banned, if it reads as
    // recursion... go write this as states": repetition is a state
    // transition (`-> target(..)`), never a self-call statement.
    if matches!(receiver_members, [receiver] if receiver.as_str() == "self") {
        let machine_entry_name = current_machine
            .name
            .as_str()
            .rsplit("::")
            .next()
            .unwrap_or(current_machine.name.as_str());
        if call.target.as_str() == machine_entry_name {
            diagnostics.push(Diagnostic::error(format!(
                "`self.{}(..)` as a STATEMENT calls the enclosing machine's own entry -- tail recursion spelled as a call, which Omega does not support (machine call cycles are banned; stack size must be predictable). Write the repetition as states: transition to a sub-state or loop back with a bare `-> {}(..)` arm",
                call.target.as_str(),
                call.target.as_str(),
            )));
            return;
        }
    }

    if receiver_members.is_empty()
        || matches!(receiver_members, [receiver] if receiver.as_str() == "self")
    {
        if let Some(signature) =
            program.machine_parameter_signature_in(current_machine, call.target_symbol)
        {
            validate_result_use(
                program,
                call,
                signature.name.as_str(),
                signature.return_type,
                diagnostics,
            );
            validate_call_arguments_handles(
                program,
                current_machine,
                machine_symbols.state(state_name),
                value_env,
                arguments,
                signature.name.as_str(),
                program.state_signature_parameters(signature),
                None,
                writable_roots,
                diagnostics,
            );
            return;
        }

        // MP4 specializes `F(args)` to the selected concrete ENTRY symbol.
        // It remains receiverless because the whole callable parameter list
        // (including any explicit data argument) is already present.
        if let Some((callee_machine, state)) = machine_state_by_symbol(program, call.target_symbol)
            && callee_machine.symbol != current_machine.symbol
        {
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
                Some(state),
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
                Some(state),
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
            Some(state),
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
    let receiver_type = machine_symbols.callable_field_type(receiver);
    let receiver_type_reference = machine_symbols.state(state_name).and_then(|state| {
        declared_receiver_type_reference(program, current_machine, state, receiver)
    });

    if let Some(error) = receiver_type_reference.and_then(|type_reference| {
        crate::traits::dynamic_requirement_call_error(
            program,
            type_reference,
            call.target.as_str(),
            call.target_symbol,
        )
    }) {
        diagnostics.push(Diagnostic::error(error));
        return;
    }

    if let Some(type_reference) = receiver_type_reference {
        match crate::traits::generic_bound_requirement_call(
            program,
            current_machine,
            type_reference,
            call.target.as_str(),
        ) {
            Ok(Some(requirement)) => {
                let signature = requirement.signature;
                validate_result_use(
                    program,
                    call,
                    signature.name.as_str(),
                    signature.return_type,
                    diagnostics,
                );
                validate_generic_bound_argument_types(
                    program,
                    current_machine,
                    machine_symbols.state(state_name),
                    type_reference,
                    arguments,
                    &requirement,
                    diagnostics,
                );
                validate_call_arguments_handles(
                    program,
                    current_machine,
                    machine_symbols.state(state_name),
                    value_env,
                    arguments,
                    signature.name.as_str(),
                    program.state_signature_parameters(signature),
                    None,
                    writable_roots,
                    diagnostics,
                );
                return;
            }
            Ok(None) => {}
            Err(error) => {
                diagnostics.push(Diagnostic::error(error));
                return;
            }
        }
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
                Some(state),
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
            Some(state),
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
            None,
            writable_roots,
            diagnostics,
        );
        return;
    }

    let _ = diagnostics;
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

/// Whether an argument that is NOT spelled `&mut ...` still DELIVERS a
/// mutable reference: a bare name forwarding a `&mut` parameter, or a local
/// that is itself a `&mut` reference (declared `&mut T`, or bound to a
/// `&mut place` initializer). Everything else lends immutable access. Bindings
/// resolve at WHOLE-MACHINE scope (a
/// sub-state legitimately reads the entry state's params and locals), so
/// every state of the current machine is consulted.
fn argument_forwards_mutable_reference(
    program: &TypedTrees,
    current_machine: &Machine,
    argument: ExpressionHandle,
) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(argument) else {
        return false;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return false;
    };
    program.machine_states(current_machine).iter().any(|state| {
        if program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.is_mutable && parameter.name == *name)
        {
            return true;
        }
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(|statement| {
                let StatementNode::LocalData(local_data) = statement else {
                    return false;
                };
                if local_data.name != *name {
                    return false;
                }
                crate::locals::local_is_mutable_reference(program, local_data)
                    || (local_data.initial_value.is_valid()
                        && matches!(
                            program
                                .expression_table
                                .expression(local_data.initial_value),
                            ExpressionNode::Borrow(borrow)
                                if borrow.access
                                    == psi_language_semantics::ReferenceAccess::Mutable
                        ))
            })
    })
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
    callee_state: Option<&State>,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_call_arguments_handles_with_policy_retention(
        program,
        current_machine,
        current_state,
        value_env,
        arguments,
        target_name,
        parameters,
        callee_state,
        writable_roots,
        false,
        diagnostics,
    );
}

fn validate_generic_bound_argument_types(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&State>,
    receiver_type: TypeReferenceHandle,
    arguments: &[ExpressionHandle],
    requirement: &crate::traits::GenericBoundRequirement<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let parameters = program
        .state_signature_parameters(requirement.signature)
        .iter()
        .filter(|parameter| !parameter.is_self);
    for (argument, parameter) in arguments.iter().zip(parameters) {
        let Some(actual) = declared_place_type(program, current_machine, current_state, *argument)
        else {
            continue;
        };
        let required = crate::places::unwrapped_type_reference(program, parameter.type_reference)
            .unwrap_or(parameter.type_reference);
        let receiver = crate::places::unwrapped_type_reference(program, receiver_type)
            .unwrap_or(receiver_type);
        if !crate::traits::generic_bound_argument_matches(
            program,
            actual,
            required,
            receiver,
            requirement,
        ) {
            diagnostics.push(Diagnostic::error(format!(
                "argument `{}` for bounded trait requirement `{}::{}` does not match `{}` after applying the bound's generic arguments",
                parameter.name,
                requirement.trait_definition.name,
                requirement.signature.name,
                program.display_type_reference_with_constraints(parameter.type_reference),
            )));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_call_arguments_handles_with_policy_retention(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&State>,
    value_env: &ValueEnv,
    arguments: &[ExpressionHandle],
    target_name: &str,
    parameters: &[StateParameter],
    callee_state: Option<&State>,
    writable_roots: &WritableRoots<'_, '_>,
    retain_arithmetic_policy: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if report_argument_count_mismatch(target_name, parameters, arguments, diagnostics) {
        return;
    }

    let quotient_lift = callee_state.and_then(|state| {
        let argument_types = arguments
            .iter()
            .map(|argument| declared_place_type(program, current_machine, current_state, *argument))
            .collect::<Vec<_>>();
        crate::quotients::legacy_quotient_call_candidate(program, None, &argument_types, state)
    });
    if let Some(lift) = &quotient_lift {
        diagnostics.push(Diagnostic::error(format!(
            "cannot implicitly lift representative operation `{}` onto quotient `{}`; use `Quotient::lift<F, Respect>` or `Quotient::define<F, Respect>` with one exact named conformance",
            lift.operation.name, lift.quotient.name,
        )));
    }

    for (argument, parameter) in arguments
        .iter()
        .zip(parameters.iter().filter(|parameter| !parameter.is_self))
    {
        let expected_access = match program
            .type_reference_table
            .type_reference(parameter.type_reference)
        {
            psi_typed_trees::types::TypeReferenceNode::Reference { access, .. } => Some(*access),
            _ => None,
        };
        let supplied_access = match program.expression_table.expression(*argument) {
            ExpressionNode::Borrow(borrow) => Some(borrow.access),
            _ => None,
        };

        if expected_access == Some(psi_language_semantics::ReferenceAccess::WriteOnly)
            && supplied_access != Some(psi_language_semantics::ReferenceAccess::WriteOnly)
        {
            diagnostics.push(Diagnostic::error(format!(
                "argument `{}` for state `{}` requires explicit write-only attenuation; pass `&write ...` (a bare value or `&mut ...` does not establish the no-read contract)",
                parameter.name, target_name,
            )));
            continue;
        }
        if supplied_access == Some(psi_language_semantics::ReferenceAccess::WriteOnly)
            && expected_access != Some(psi_language_semantics::ReferenceAccess::WriteOnly)
        {
            diagnostics.push(Diagnostic::error(format!(
                "argument `{}` for state `{}` supplies `&write` to a parameter that may read; write-only authority cannot widen to shared or mutable access",
                parameter.name, target_name,
            )));
            continue;
        }

        let is_mutable = matches!(
            supplied_access,
            Some(
                psi_language_semantics::ReferenceAccess::Mutable
                    | psi_language_semantics::ReferenceAccess::WriteOnly
            )
        );

        if parameter.is_mutable && !is_mutable {
            // Not spelled `&mut ...`. The only legitimate remaining shape is
            // a FORWARD: a bare name that is itself already a `&mut`
            // reference (a `&mut` parameter passed onward, or a local bound
            // to a `&mut` borrow). Anything else lends IMMUTABLE access to a
            // parameter that may write through it -- the borrow-safety hole
            // this arm used to skip silently (the unenforced write segfaulted
            // natively).
            if !argument_forwards_mutable_reference(program, current_machine, *argument) {
                diagnostics.push(Diagnostic::error(format!(
                    "argument `{}` for state `{}` is declared `&mut` (`{}`), but the \
                     caller lends only immutable access -- pass `&mut ...` or forward a \
                     `&mut` binding",
                    parameter.name,
                    target_name,
                    program.display_type_reference_with_constraints(parameter.type_reference),
                )));
            }
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
        if quotient_lift.is_none() {
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
        }
        // Scalar-vs-data shape guard: `take_struct(5)` (a scalar for a struct param)
        // or `take_int(self.struct)` (a struct for a scalar param). Unlike the
        // array/scalar check below, this is SAFE at the argument position -- it fires
        // only on scalar-vs-DATA-type crossings, and `&buffer`/`addr`/text args
        // involve no data type on either side, so they never trigger it.
        crate::expression_types::report_scalar_data_shape_mismatch(
            program,
            current_machine,
            current_state,
            *argument,
            parameter.type_reference,
            &slot_context,
            "argument",
            diagnostics,
        );
        if retain_arithmetic_policy {
            crate::domain_weakening::validate_implicit_domain_weakening_retaining_arithmetic_policy(
                program,
                current_machine,
                current_state,
                *argument,
                parameter.type_reference,
                &slot_context,
                diagnostics,
            );
        } else {
            crate::domain_weakening::validate_implicit_domain_weakening(
                program,
                current_machine,
                current_state,
                *argument,
                parameter.type_reference,
                &slot_context,
                diagnostics,
            );
        }
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
    callee_machine: &Machine,
    callee_state: &State,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_value_call_argument_classes_with_receiver(
        program,
        current_machine,
        current_state,
        value_env,
        None,
        arguments,
        callee_machine,
        callee_state,
        diagnostics,
    );
}

#[allow(clippy::too_many_arguments)]
fn validate_value_call_argument_classes_with_receiver(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: &State,
    value_env: &ValueEnv,
    receiver_type: Option<TypeReferenceHandle>,
    arguments: &[ExpressionHandle],
    callee_machine: &Machine,
    callee_state: &State,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // (The void-callee-in-value-position check lives in report_void_value_callee:
    // it consults the resolved state's return type AND the callee machine's
    // transition VALUE arms, which is what keying off `callee_state.return_type`
    // alone could not do.)

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

    crate::contract_entailment::reject_refuted_value_call_requires(
        program,
        current_machine,
        current_state,
        callee_machine,
        callee_state,
        arguments,
        diagnostics,
    );

    let argument_types = arguments
        .iter()
        .map(|argument| {
            declared_place_type(program, current_machine, Some(current_state), *argument)
        })
        .collect::<Vec<_>>();
    let quotient_lift = crate::quotients::legacy_quotient_call_candidate(
        program,
        receiver_type,
        &argument_types,
        callee_state,
    );
    if let Some(lift) = &quotient_lift {
        diagnostics.push(Diagnostic::error(format!(
            "cannot implicitly lift representative operation `{}` onto quotient `{}`; use `Quotient::lift<F, Respect>` or `Quotient::define<F, Respect>` with one exact named conformance",
            lift.operation.name, lift.quotient.name,
        )));
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
            "argument `{}` for state `{}::{}`",
            parameter.name,
            callee_machine.name,
            callee_state.name.as_str()
        );
        if quotient_lift.is_none() {
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
        }
        // Scalar-vs-data shape guard -- safe at the argument position (see the twin
        // call in `validate_call_arguments_handles`): fires only on scalar-vs-DATA
        // crossings, which `&buffer`/`addr`/text args never are.
        crate::expression_types::report_scalar_data_shape_mismatch(
            program,
            current_machine,
            Some(current_state),
            *argument,
            parameter.type_reference,
            &slot_context,
            "argument",
            diagnostics,
        );
        crate::domain_weakening::validate_implicit_domain_weakening(
            program,
            current_machine,
            Some(current_state),
            *argument,
            parameter.type_reference,
            &slot_context,
            diagnostics,
        );
        // (No array/scalar shape check here -- see the note in
        // `validate_call_arguments_handles`: `&buffer`-into-`addr` and text/byte args
        // make the argument position a false-positive minefield.)
    }
}
