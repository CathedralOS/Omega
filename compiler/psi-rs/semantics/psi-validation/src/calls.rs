use crate::arithmetic_domains::{self, ValueEnv};
use crate::expression_types::{
    argument_matches_type_reference_handle, expression_type_name_handle, report_cross_class_store,
    report_data_type_conflict,
};
use crate::locals::WritableRoots;
use crate::places::declared_place_type;
use crate::properties::{
    declared_property_requirements, referenced_type_parameter, type_satisfies_declared_property,
};
use crate::symbols::{MachineSymbols, TopLevelSymbols};
use crate::type_references::type_reference_label;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::signature::StateParameter;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{StatementNode, TableCall};
use psi_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

mod expression_scanning;
mod recursion;
mod write_frames;

use expression_scanning::receiver_member_chain;
pub(crate) use expression_scanning::{
    declared_receiver_type_reference, report_local_receiver_value_call,
    report_nested_call_in_bound_value_call, validate_value_position_calls,
};
pub(crate) use recursion::{
    validate_proof_machine_recursion, validate_self_recursive_call_positions,
};
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

fn user_asm_contract(mnemonic: &str) -> psi_language_core::inline_assembly::AsmInstructionContract {
    let Some(psi_language_core::inline_assembly::AsmCatalogEntry::Contract(contract)) =
        psi_language_core::inline_assembly::asm_catalog_entry(mnemonic)
    else {
        panic!("accepted asm intrinsic `{mnemonic}` is absent from the shared catalog");
    };
    assert_eq!(
        contract.availability,
        psi_language_core::inline_assembly::AsmInstructionAvailability::UserChecked,
        "source asm intrinsic `{mnemonic}` must be user-checked"
    );
    contract
}

fn validate_asm_operand_constraint(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    instruction: &str,
    operand: ExpressionHandle,
    constraint: psi_language_core::inline_assembly::AsmOperandConstraint,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let ExpressionNode::Integer(literal) = program.expression_table.expression(operand) {
        if let Some(maximum) = constraint.maximum_literal()
            && literal.value_u64().is_some_and(|value| value <= maximum)
        {
            return;
        }
        diagnostics.push(Diagnostic::error(format!(
            "asm instruction `{instruction}` operand `{}` requires target register `{}` \
             constraint `{}`{}; integer literal `{}` is outside that operand class",
            constraint.role,
            constraint.target_register,
            constraint.expected_type_name(),
            constraint
                .maximum_literal()
                .map(|maximum| format!(" or a literal in 0..={maximum}"))
                .unwrap_or_default(),
            literal.text(),
        )));
        return;
    }

    let actual = if constraint.requires_place() {
        crate::places::declared_place_type(program, machine, state, operand)
            .and_then(|type_reference| program.primitive_type_reference(type_reference))
    } else {
        asm_operand_primitive_type(program, machine, state, operand)
    };
    let expected = PrimitiveType::from_name(constraint.expected_type_name())
        .expect("asm operand constraint must name a primitive type");
    if actual == Some(expected) {
        return;
    }

    let actual = actual
        .map(|primitive| format!("`{}`", primitive.name()))
        .unwrap_or_else(|| expression_type_name_handle(program, operand).to_owned());
    let place_requirement = constraint
        .requires_writable_place()
        .then_some(" writable place")
        .unwrap_or("");
    diagnostics.push(Diagnostic::error(format!(
        "asm instruction `{instruction}` operand `{}` requires an exact `{}`{place_requirement} \
         for target register `{}`, found {actual}",
        constraint.role,
        constraint.expected_type_name(),
        constraint.target_register,
    )));
}

fn asm_operand_primitive_type(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    operand: ExpressionHandle,
) -> Option<PrimitiveType> {
    match program.expression_table.expression(operand) {
        ExpressionNode::Mutable(inner) => {
            asm_operand_primitive_type(program, machine, state, *inner)
        }
        ExpressionNode::Cast(cast) => program.primitive_type_reference(cast.target_type),
        _ => crate::places::declared_place_type(program, machine, state, operand)
            .and_then(|type_reference| program.primitive_type_reference(type_reference)),
    }
}

pub(crate) fn validate_asm_value_destination(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    assignment: &psi_typed_trees::statement::TableAssignment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ExpressionNode::Call(call) = program.expression_table.expression(assignment.value) else {
        return;
    };
    let instruction =
        match psi_language_core::inline_assembly::AsmControlRegister::from_read_intrinsic_name(
            call.target.as_str(),
        ) {
            Some(register) => register.read_mnemonic(),
            None => match call.target.as_str() {
                "asm#port_in" => "in",
                "asm#pushfq" => "pushfq",
                "asm#rdmsr" => "rdmsr",
                _ => return,
            },
        };
    let contract = user_asm_contract(instruction);
    validate_asm_operand_constraint(
        program,
        machine,
        state,
        instruction,
        assignment.target,
        contract.operands[0],
        diagnostics,
    );
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
    // A claim-free bodyless boundary declaration is a SYMBOL for contracts,
    // not an executable provider.  It has neither checked code nor a `via`
    // realization, so allowing an ordinary body call would turn "introduces
    // no fact" into a hidden runtime implementation hole.  Contract
    // expressions are not body call sites and remain free to name the symbol.
    let compiler_placed_accessor = callee_machine
        .attached_data
        .as_ref()
        .is_some_and(|attached| attached.as_str().starts_with("PlacedField<"));
    if callee_machine.supply_mode == psi_language_semantics::MachineSupplyMode::Boundary
        && !compiler_placed_accessor
        && program
            .statement_table
            .statements(callee_state.statement_nodes)
            .is_empty()
    {
        diagnostics.push(Diagnostic::error(format!(
            "bodyless boundary symbol `{target_name}` has no executable realization; use it only in contracts, or satisfy a boundary requirement via an admitted provider"
        )));
    }

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
        let bounds = declared_property_requirements(&type_parameter.bounds);
        if bounds.is_empty() {
            continue;
        }
        let bound_labels = bounds.iter().map(ToString::to_string).collect::<Vec<_>>();
        let Some(argument_type) =
            declared_place_type(program, current_machine, current_state, *argument)
        else {
            continue;
        };
        for property in bounds {
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
                "type parameter `{} [{}]` of machine `{target_name}` was instantiated with `{}`, which does not satisfy `[{property}]`",
                type_parameter.name,
                bound_labels.join(", "),
                type_reference_label(program, argument_type)
            )));
        }
    }
}

/// FROZEN DECISION 9 -- STRICT RESULT USE: a statement-position call whose callee
/// returns a non-unit value must not silently drop that value. Intentional
/// discards are spelled `_ = call();` (which sets `discards_result`). "Non-unit"
/// means the resolved callee declares a return type (`-> T`) that is not `()`.
///
/// PROOF-MACHINE callees are exempt (owner, 2026-07-12): a bare statement
/// call to a proof machine is a CITATION (ch10 "Citing Proofs") -- the
/// lemma is invoked for its ensures and erases at codegen, so there is no
/// runtime result to drop. The exemption is a property of the callee's
/// (computed) classification, visible at its declaration -- never of the
/// call site's context.
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

    if call.receiver.is_empty() {
        let classification = psi_typed_trees::proof_only::classify(program);
        let is_citation = program
            .machines()
            .iter()
            .find(|candidate| {
                candidate.attached_data.is_none() && candidate.name.as_str() == call.target.as_str()
            })
            .is_some_and(|callee| classification.is_proof_machine(program, callee));
        if is_citation {
            return;
        }
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

/// Whether an argument that is NOT spelled `&mut ...` still DELIVERS a
/// mutable reference: a bare name forwarding a `&mut` parameter, or a local
/// that is itself a `&mut` reference (declared `&mut T`, or bound to a
/// `&mut place` initializer). Everything else lends immutable access -- a
/// shared `&` vanishes at parse time, so a bare place expression IS the
/// immutable-lend spelling. Bindings resolve at WHOLE-MACHINE scope (a
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
                            ExpressionNode::Mutable(_)
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
        let is_mutable = matches!(
            program.expression_table.expression(*argument),
            ExpressionNode::Mutable(_)
        );

        if parameter.is_mutable && !is_mutable {
            // Not spelled `&mut ...`. The only legitimate remaining shape is
            // a FORWARD: a bare name that is itself already a `&mut`
            // reference (a `&mut` parameter passed onward, or a local bound
            // to a `&mut` borrow). Anything else lends IMMUTABLE access to a
            // parameter that may write through it -- the borrow-safety hole
            // this arm used to skip silently (a shared `&` vanishes at parse
            // time, so a bare place expression IS the immutable-lend
            // spelling; the unenforced write segfaulted natively).
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
