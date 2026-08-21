//! Value-position expression-call scanning and bound validation.
//!
//! The parent call validator owns shared argument/type rules. This child
//! owns recursive expression traversal, value-callee resolution, and the
//! diagnostics emitted at those scan sites.

use super::{
    free_machine_entry_state, machine_state_by_symbol, user_asm_contract,
    validate_asm_operand_constraint, validate_call_arguments_handles,
    validate_call_arguments_handles_with_policy_retention, validate_generic_bound_argument_types,
    validate_machine_call_type_parameter_bounds, validate_value_call_argument_classes,
};
use crate::arithmetic_domains::{self, ValueEnv};
use crate::locals::WritableRoots;
use crate::symbols::{MachineSymbols, TopLevelSymbols};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::name::Identifier;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{StatementNode, TransitionTargetNode};
use psi_typed_trees::types::{PrimitiveType, TypeReferenceNode};

mod target_resolution;
mod traversal;

pub(crate) use target_resolution::declared_receiver_type_reference;
use target_resolution::{named_type_reference_name, report_unresolved_value_call};
pub(crate) use traversal::validate_value_position_calls;

/// Enforce machine-call type-parameter bounds for a single VALUE-position
/// `ExpressionNode::Call`.  The receiver name path is extracted from the
/// receiver expression (must be a `Name` node with identifier segments).
/// Other receiver shapes (member chains, indexed, etc.) are beyond this
/// scope and stand down silently, consistent with the statement-path's
/// handling of unrecognised receivers.
/// A VALUE-position call from emitted concrete code may not retain a GENERIC
/// callee: its result slot has no concrete layout. Uninstantiated generic
/// templates are different—they are checked modularly but never emitted, and
/// their symbolic calls are resolved by fixed-point specialization once a
/// concrete outer call selects them.
fn fence_generic_value_callee(
    program: &TypedTrees,
    caller_machine: &Machine,
    callee_machine: &Machine,
    target: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // An uninstantiated generic template is checked modularly but is never
    // emitted. Its symbolic value calls become concrete when an outer call
    // specializes the template. Keep the fence for concrete callers whose
    // selected callee somehow remains generic: that is still an incomplete
    // lowering and must fail loudly.
    if !program.machine_type_parameters(caller_machine).is_empty()
        || program.machine_type_parameters(callee_machine).is_empty()
    {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "a value call to generic machine `{target}` cannot derive a complete \
         type/const/machine/conformance specialization tuple from its argument and result types; add a \
         concrete destination annotation or provide concrete argument type evidence",
    )));
}

/// The receiver's spelled member chain, root -> leaf (`["self", "p",
/// "second"]` for `self.p.second.stored()`). `None` for non-place receivers
/// (calls, literals). Mirrors the state-call plan's `append_receiver_path`
/// walk at the typed layer.
pub(super) fn receiver_member_chain(
    program: &TypedTrees,
    receiver: psi_typed_trees::expression::ExpressionHandle,
) -> Option<Vec<String>> {
    if !receiver.is_valid() {
        return None;
    }
    match program.expression_table.expression(receiver) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            (!members.is_empty()).then(|| {
                members
                    .iter()
                    .map(|member| member.as_str().to_string())
                    .collect()
            })
        }
        ExpressionNode::Member(member) => {
            let mut chain = receiver_member_chain(program, member.receiver)?;
            chain.push(member.member.as_str().to_string());
            Some(chain)
        }
        ExpressionNode::Mutable(inner) => receiver_member_chain(program, *inner),
        _ => None,
    }
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
    if (call.target.as_str() == "asm#pushfq"
        || psi_language_core::inline_assembly::AsmControlRegister::from_read_intrinsic_name(
            call.target.as_str(),
        )
        .is_some())
        && !call.receiver.is_valid()
    {
        let arguments = program.expression_table.expression_handles(call.arguments);
        if !arguments.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "asm intrinsic `{}` takes 0 operands, found {}",
                call.target,
                arguments.len()
            )));
        }
        return;
    }

    if matches!(call.target.as_str(), "asm#port_in" | "asm#rdmsr") && !call.receiver.is_valid() {
        let (intrinsic, instruction, operand_index) = if call.target.as_str() == "asm#port_in" {
            ("asm#port_in", "in", 1)
        } else {
            ("asm#rdmsr", "rdmsr", 1)
        };
        let arguments = program.expression_table.expression_handles(call.arguments);
        if arguments.len() != 1 {
            diagnostics.push(Diagnostic::error(format!(
                "asm intrinsic `{intrinsic}` takes 1 operand, found {}",
                arguments.len()
            )));
            return;
        }
        let contract = user_asm_contract(instruction);
        validate_asm_operand_constraint(
            program,
            current_machine,
            Some(current_state),
            instruction,
            arguments[0],
            contract.operands[operand_index],
            diagnostics,
        );
        return;
    }

    if let Some(operator) = psi_typed_trees::operator::resolve_named_expression_call(program, call)
    {
        let explicit_arguments = program.expression_table.expression_handles(call.arguments);
        let parameters = program.operator_parameters(operator);
        let has_self_parameter = parameters.iter().any(|parameter| parameter.is_self);
        let mut arguments = Vec::with_capacity(explicit_arguments.len() + 1);
        if call.receiver.is_valid()
            && !has_self_parameter
            && parameters.len() == explicit_arguments.len() + 1
        {
            arguments.push(call.receiver);
        }
        arguments.extend_from_slice(explicit_arguments);
        let retains_arithmetic_policy = matches!(
            (
                program
                    .operator_path_members(operator.name)
                    .first()
                    .map(|name| name.as_str()),
                program.primitive_type_reference(operator.return_type),
            ),
            (Some("F32"), Some(PrimitiveType::F32)) | (Some("F64"), Some(PrimitiveType::F64))
        );
        validate_call_arguments_handles_with_policy_retention(
            program,
            current_machine,
            Some(current_state),
            value_env,
            &arguments,
            call.target.as_str(),
            parameters,
            None,
            writable_roots,
            retains_arithmetic_policy,
            diagnostics,
        );
        validate_named_float_to_integer_call(
            program,
            current_machine,
            current_state,
            value_env,
            operator,
            explicit_arguments,
            diagnostics,
        );
        return;
    }

    // Resolve the receiver: is this a self-call, and if not, the name of the
    // receiver object (field/local). A self-call has no receiver (`call.receiver`
    // invalid) or an explicit `self`. A `Name`-path receiver names the object via
    // its last member; a `Member` receiver (`self.host.method(..)`, where
    // `self.host` is a member access, NOT a name path) names it via that member —
    // WITHOUT this, a member receiver fell through as an empty path and the call
    // was misrouted into the self-call branch (resolving to a same-named sibling
    // state instead of the field's boundary/machine type).
    let (call_is_self, external_receiver_name): (bool, Option<&str>) = if !call.receiver.is_valid()
    {
        (true, None)
    } else {
        match program.expression_table.expression(call.receiver) {
            ExpressionNode::Name(path) => {
                let members = program.expression_table.name_path_members(path.members);
                if members.is_empty() || matches!(members, [r] if r.as_str() == "self") {
                    (true, None)
                } else {
                    (false, members.last().map(Identifier::as_str))
                }
            }
            ExpressionNode::Member(member) => (false, Some(member.member.as_str())),
            _ => (true, None),
        }
    };

    let arguments = program.expression_table.expression_handles(call.arguments);

    // Self-call or `self`-prefixed call: the callee is a state of the
    // current machine, an attached-data sibling machine, or a free machine.
    // Mirrors the same three-way fallback in `validate_call_node`.
    if call_is_self {
        if let Some(signature) =
            program.machine_parameter_signature_in(current_machine, call.target_symbol)
        {
            if !signature.return_type.is_valid() {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}`: machine parameter `{}` does not return a value but is used in a VALUE position",
                    current_machine.name,
                    current_state.name,
                    signature.name,
                )));
            }
            validate_call_arguments_handles(
                program,
                current_machine,
                Some(current_state),
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

        if let Some((callee_machine, callee_state)) =
            machine_state_by_symbol(program, call.target_symbol)
            && callee_machine.symbol != current_machine.symbol
        {
            report_void_value_callee(
                program,
                callee_machine,
                current_machine,
                current_state,
                callee_state,
                call.target.as_str(),
                diagnostics,
            );
            fence_generic_value_callee(
                program,
                current_machine,
                callee_machine,
                call.target.as_str(),
                diagnostics,
            );
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
                callee_machine,
                callee_state,
                diagnostics,
            );
            return;
        }

        if let Some(callee_state) = machine_symbols.state(call.target.as_str()) {
            report_void_value_callee(
                program,
                current_machine,
                current_machine,
                current_state,
                callee_state,
                call.target.as_str(),
                diagnostics,
            );
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
                current_machine,
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
            report_void_value_callee(
                program,
                callee_machine,
                current_machine,
                current_state,
                callee_state,
                call.target.as_str(),
                diagnostics,
            );
            fence_generic_value_callee(
                program,
                current_machine,
                callee_machine,
                call.target.as_str(),
                diagnostics,
            );
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
                callee_machine,
                callee_state,
                diagnostics,
            );
            return;
        }

        // Free machine call (`compute(item)` -- no `self.`, no receiver).
        if let Some((callee_machine, callee_state)) =
            free_machine_entry_state(program, symbols, call.target.as_str())
        {
            report_void_value_callee(
                program,
                callee_machine,
                current_machine,
                current_state,
                callee_state,
                call.target.as_str(),
                diagnostics,
            );
            fence_generic_value_callee(
                program,
                current_machine,
                callee_machine,
                call.target.as_str(),
                diagnostics,
            );
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
                callee_machine,
                callee_state,
                diagnostics,
            );
            return;
        }
        report_unresolved_value_call(
            program,
            current_machine,
            current_state,
            symbols,
            None,
            None,
            call,
            diagnostics,
        );
        return;
    }

    let receiver_name = external_receiver_name.unwrap_or_default();
    // Direct field/local receivers resolve by bare name. A NESTED self-rooted
    // VALUE-position member chain (`self.p.a.get()`) resolves by walking the
    // chain's declared field types to the leaf type (receiver-place staircase,
    // rung 3). The full arc is now sound: symbol resolution stamps the nested
    // symbols (rung 2b) so the state-call plan records the call; the backend
    // storage walk descends plain-DATA intermediates (rung 2a/D1) so the
    // callee's `self` base resolves; and the emission-planning
    // contained-receiver blocker rejects an ambiguous nested receiver (a
    // same-type sibling that the by-type walk would misresolve) loudly instead
    // of binding 0. STATEMENT-position nested calls are validated separately
    // (`validate_call_node`) and remain unsupported -- see TASKS D2.
    let receiver_type_reference = crate::places::declared_place_type(
        program,
        current_machine,
        Some(current_state),
        call.receiver,
    );
    let receiver_type = machine_symbols
        .callable_field_type(receiver_name)
        .or_else(|| {
            let chain = receiver_member_chain(program, call.receiver)?;
            if chain.len() < 3 || chain.first().map(String::as_str) != Some("self") {
                return None;
            }
            crate::places::nested_receiver_type_name(
                program,
                current_machine,
                Some(current_state),
                &chain,
            )
        })
        .or_else(|| {
            receiver_type_reference
                .and_then(|type_reference| named_type_reference_name(program, type_reference))
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
                if !signature.return_type.is_valid()
                    || matches!(
                        program
                            .type_reference_table
                            .type_reference(signature.return_type),
                        TypeReferenceNode::Unit
                    )
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "trait requirement `{}::{}` does not return a value but is used in a VALUE position",
                        requirement.trait_definition.name,
                        signature.name,
                    )));
                }
                validate_generic_bound_argument_types(
                    program,
                    current_machine,
                    Some(current_state),
                    type_reference,
                    arguments,
                    &requirement,
                    diagnostics,
                );
                validate_call_arguments_handles(
                    program,
                    current_machine,
                    Some(current_state),
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

    if let Some(receiver_type_reference) = receiver_type_reference
        && let Some(candidate) = crate::quotients::legacy_attached_quotient_call_candidate(
            program,
            receiver_type_reference,
            call.target.as_str(),
        )
    {
        diagnostics.push(Diagnostic::error(format!(
            "cannot implicitly lift attached representative operation `{}` onto quotient `{}`; use `Quotient::lift<F, Respect>` or `Quotient::define<F, Respect>` with one exact named conformance",
            candidate.operation.name, candidate.quotient.name,
        )));
        return;
    }

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
            report_void_value_callee(
                program,
                callee_machine,
                current_machine,
                current_state,
                callee_state,
                call.target.as_str(),
                diagnostics,
            );
            fence_generic_value_callee(
                program,
                current_machine,
                callee_machine,
                call.target.as_str(),
                diagnostics,
            );
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
                callee_machine,
                callee_state,
                diagnostics,
            );
            return;
        }
        report_unresolved_value_call(
            program,
            current_machine,
            current_state,
            symbols,
            Some(receiver_name),
            receiver_type,
            call,
            diagnostics,
        );
        return;
    }

    // Attached-data machine receiver.
    if let Some((callee_machine, callee_state)) = receiver_type.and_then(|type_name| {
        symbols.attached_machine_state(program, type_name, call.target.as_str())
    }) {
        report_void_value_callee(
            program,
            callee_machine,
            current_machine,
            current_state,
            callee_state,
            call.target.as_str(),
            diagnostics,
        );
        fence_generic_value_callee(
            program,
            current_machine,
            callee_machine,
            call.target.as_str(),
            diagnostics,
        );
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
            callee_machine,
            callee_state,
            diagnostics,
        );
        let _ = writable_roots;
        return;
    }
    report_unresolved_value_call(
        program,
        current_machine,
        current_state,
        symbols,
        Some(receiver_name),
        receiver_type,
        call,
        diagnostics,
    );

    let _ = writable_roots;
}

fn validate_named_float_to_integer_call(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: &State,
    value_env: &ValueEnv,
    operator: &psi_typed_trees::operator::OperatorDefinition,
    arguments: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let path = program.operator_path_members(operator.name);
    let [namespace, requirement] = path else {
        return;
    };
    if !matches!(requirement.as_str(), "from_f32" | "from_f64")
        || program
            .type_reference_table
            .arithmetic_domain(operator.return_type)
            != psi_numerics::arithmetic::ArithmeticDomain::Exact
    {
        return;
    }
    let target = match namespace.as_str() {
        "I8" => PrimitiveType::I8,
        "I16" => PrimitiveType::I16,
        "I32" => PrimitiveType::I32,
        "I64" => PrimitiveType::I64,
        "U8" => PrimitiveType::U8,
        "U16" => PrimitiveType::U16,
        "U32" => PrimitiveType::U32,
        "U64" => PrimitiveType::U64,
        _ => return,
    };
    let [value] = arguments else {
        return;
    };
    if arithmetic_domains::float_source_proves_int_cast(
        program,
        current_machine,
        Some(current_state),
        value_env,
        *value,
        target,
    ) {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` cannot prove unqualified `{}::{}` operand `{}` is finite and truncates into `{}`; constrain it with a declared range or dominating non-NaN/range guard, or select result type `{} in Trapping` or `{} in Saturating`",
        current_machine.name,
        current_state.name,
        namespace,
        requirement,
        program.expression_table.display_name(*value),
        target.name(),
        target.name(),
        target.name(),
    )));
}

/// A value call on a LET-BOUND LOCAL receiver (`let p: Pair = ..; p.total()`)
/// reads ZII natively: receiver resolution reaches machine FIELDS and state
/// PARAMETERS only, so the callee's `self.field` reads bind to nothing and
/// the result silently zeroes when the caller is itself an inlined value
/// callee (Main-state spellings hit the emission backstop instead). Fence it
/// loudly until local receiver resolution lands (TASKS.md "local-receiver
/// value calls"). Field receivers, `self`, and state-parameter receivers are
/// the supported (canaried) forms.
pub(crate) fn report_local_receiver_value_call(
    program: &TypedTrees,
    machine: &Machine,
    state_name: &str,
    value: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !value.is_valid() {
        return;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(value) else {
        return;
    };
    if !call.receiver.is_valid() {
        return;
    }
    // BUILTIN view/operand methods (`view.bytes()`, `arr.as_slice()`, min/max
    // shapes) compose on locals through the operand machinery -- the same
    // exemption list as the nested-argument fence above.
    if matches!(
        call.target.as_str(),
        "min" | "max" | "sqrt" | "as_slice" | "as_mut_slice" | "as_view" | "bytes"
    ) {
        return;
    }
    // Only a BARE single-member NAME receiver can be a local; `self.x` and
    // deeper member paths route through the supported field machinery.
    let ExpressionNode::Name(path) = program.expression_table.expression(call.receiver) else {
        return;
    };
    let members = program.expression_table.name_path_members(path.members);
    let [receiver_name] = members else {
        return;
    };
    let receiver = receiver_name.as_str();
    if receiver == "self" {
        return;
    }
    // A state PARAMETER (whole-machine scope) is a supported receiver.
    let is_parameter = program.machine_states(machine).iter().any(|state| {
        program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.name.as_str() == receiver)
    });
    if is_parameter {
        return;
    }
    // A machine FIELD read as a bare name cannot happen (fields spell
    // `self.x`), but keep the check total: owned-data names pass through.
    let is_field = program
        .machine_owned_data(machine)
        .iter()
        .any(|owned| owned.name.as_str() == receiver);
    if is_field {
        return;
    }
    // A LOCAL-DATA binding anywhere in the machine (bindings are
    // whole-machine scope).
    let local = program.machine_states(machine).iter().find_map(|state| {
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| {
                let StatementNode::LocalData(local) = statement else {
                    return None;
                };
                (local.name.as_str() == receiver).then_some(local)
            })
    });
    let Some(local) = local else {
        return;
    };
    // A checked local dynamic coercion is not an ordinary local receiver:
    // closed-row lowering devirtualizes the call onto the coercion's retained
    // source place. Invalid, missing, or ambiguous conformance selection is
    // rejected independently by `collect_dynamic_conformance_selections`.
    if type_reference_contains_dynamic_trait(program, local.type_reference)
        && matches!(
            program.expression_table.expression(local.initial_value),
            ExpressionNode::Cast(cast)
                if type_reference_contains_dynamic_trait(program, cast.target_type)
        )
    {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{state_name}`: value call `{}.{}(..)` uses a LET-bound          local as its receiver, which reads ZII (zeroes) natively -- receiver          resolution reaches machine fields and state parameters only. Store the          value in a data field (`self.{} = {}; self.{}.{}(..)`) or pass it as a          state parameter.",
        machine.name,
        receiver,
        call.target.as_str(),
        receiver,
        receiver,
        receiver,
        call.target.as_str(),
    )));
}

fn type_reference_contains_dynamic_trait(
    program: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        psi_typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            type_reference_contains_dynamic_trait(program, *referee)
        }
        psi_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_contains_dynamic_trait(program, *base_type)
        }
        psi_typed_trees::types::TypeReferenceNode::DynamicTrait { .. } => true,
        _ => false,
    }
}

/// A LET/ASSIGNMENT-bound value call whose ARGUMENT nests another machine call
/// (`let out = self.double(self.inc(3))`) reads a garbage inner result: the
/// inner callee's frame locals cannot materialize inside the outer call's
/// argument context in the VALUE sink (some consumer shapes fence loudly with
/// "needs stack/local storage lowering", but the guard-consumer shape slipped
/// through and natively bound 0). STATEMENT-call arguments take a different
/// materialization path and legitimately nest (the dungeon's
/// `self.append_exit(.., self.direction_command(self.opposite(d)), ..)`), so
/// this check runs ONLY on the value expression of a local/assignment.
pub(crate) fn report_nested_call_in_bound_value_call(
    program: &TypedTrees,
    machine: &Machine,
    state_name: &str,
    value: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !value.is_valid() {
        return;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(value) else {
        return;
    };
    // A BUILTIN outer call (`let v = max(self.range(..), floor)`) composes:
    // builtin arguments materialize as operands through the call-result-local
    // machinery (canaried). Only a MACHINE outer call's argument context is
    // broken.
    if matches!(
        call.target.as_str(),
        "min" | "max" | "sqrt" | "as_slice" | "as_mut_slice" | "as_view" | "bytes"
    ) {
        return;
    }
    for argument in program.expression_table.expression_handles(call.arguments) {
        if let Some(inner) = first_non_builtin_call(program, *argument) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{state_name}`: a value-call argument cannot itself \
                 be a machine call yet (`{inner}(..)` nested in `{}(..)` would read a \
                 garbage result) -- bind the inner call to a local first, then pass \
                 the local.",
                machine.name,
                call.target.as_str(),
            )));
            return;
        }
    }
}

/// The first NON-BUILTIN machine call nested anywhere inside `expression`
/// (its target name, for the diagnostic), or None. Reserved value builtins
/// (`min`/`max`/`sqrt`) and the view builtins (`as_slice`/`as_mut_slice`/
/// `as_view`/`bytes`) compose in arguments and are exempt.
fn first_non_builtin_call(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<Identifier> {
    if !expression.is_valid() {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => first_non_builtin_call(program, atomic.value),
        ExpressionNode::Call(call) => {
            if !matches!(
                call.target.as_str(),
                "min" | "max" | "sqrt" | "as_slice" | "as_mut_slice" | "as_view" | "bytes"
            ) {
                return Some(call.target.clone());
            }
            program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .find_map(|argument| first_non_builtin_call(program, *argument))
        }
        ExpressionNode::Binary(binary) => first_non_builtin_call(program, binary.left)
            .or_else(|| first_non_builtin_call(program, binary.right)),
        ExpressionNode::Unary(unary) => first_non_builtin_call(program, unary.operand),
        ExpressionNode::Cast(cast) => first_non_builtin_call(program, cast.value),
        ExpressionNode::Mutable(inner) => first_non_builtin_call(program, *inner),
        ExpressionNode::Indexed(indexed) => first_non_builtin_call(program, indexed.collection)
            .or_else(|| first_non_builtin_call(program, indexed.index)),
        ExpressionNode::Member(member) => first_non_builtin_call(program, member.receiver),
        _ => None,
    }
}

/// A VOID callee in VALUE position used to compile and silently bind 0 (ZII)
/// -- and native/interp DIVERGED on the bound value. "Void" means: no declared
/// return type on the resolved state (the parser now lands `-> T` written
/// after the machine clauses too; it used to be silently dropped) AND no state
/// of the callee machine produces a value through a transition VALUE arm --
/// undeclared-return value machines (`transition r > 0 { true -> self.f(r-1)
/// false -> 0 }`, the termination-canary surface) stay callable.
fn report_void_value_callee(
    program: &TypedTrees,
    callee_machine: &Machine,
    current_machine: &Machine,
    current_state: &State,
    callee_state: &State,
    callee_display: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if callee_state.return_type.is_valid() {
        return;
    }
    let produces_value = program.machine_states(callee_machine).iter().any(|state| {
        state.return_type.is_valid()
            || program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    let StatementNode::Transition(transition) = statement else {
                        return false;
                    };
                    [transition.target, transition.continuation]
                        .iter()
                        .any(|handle| {
                            handle.is_valid()
                                && matches!(
                                    program.statement_table.transition_target(*handle),
                                    TransitionTargetNode::Value(_)
                                )
                        })
                })
    });
    if produces_value {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}`: `{callee_display}(..)` does not return a value but is \
         used in a VALUE position -- it would silently bind 0 (ZII) at runtime. Declare \
         a return type on the callee (`-> T`) or call it as a statement.",
        current_machine.name,
        current_state.name.as_str(),
    )));
}
