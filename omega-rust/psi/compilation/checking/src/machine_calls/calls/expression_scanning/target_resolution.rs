//! Receiver declaration and value-call target resolution.
//!
//! This module normalizes declared receiver type shells, replays every target
//! channel understood by lowering, and emits the fail-closed unresolved-call
//! diagnostic only after those channels have been exhausted.

use crate::declarations::symbols::TopLevelSymbols;
use diagnostics::Diagnostic;
use typed_trees::expression::TableCallExpression;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// The declared TYPE NAME of a receiver that is a state param, state local,
/// or machine-owned field -- walked through reference/
/// constraint shells to the Named/Generic/DynamicTrait head. `None` for
/// primitives, arrays, slices, and unknown names.
fn receiver_declared_type_name<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    receiver: &str,
) -> Option<&'program str> {
    let handle = declared_receiver_type_reference(program, machine, state, receiver)?;
    named_type_reference_name(program, handle)
}

pub(crate) fn declared_receiver_type_reference(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    receiver: &str,
) -> Option<TypeReferenceHandle> {
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == receiver)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| {
                    let local = statement.as_local_data()?;
                    (local.name.as_str() == receiver).then_some(local.type_reference)
                })
        })
        .or_else(|| {
            program
                .machine_owned_data(machine)
                .iter()
                .find(|owned| owned.name.as_str() == receiver)
                .map(|owned| owned.type_reference)
        })
        .or_else(|| {
            let definition = program.attached_data_definition(machine)?;
            program.data_members(definition).iter().find_map(|member| {
                let field = member.as_field()?;
                (field.name.as_str() == receiver).then_some(field.type_reference)
            })
        })
}

pub(super) fn named_type_reference_name(
    program: &dyn crate::CheckingInput,
    handle: TypeReferenceHandle,
) -> Option<&str> {
    if let Some(requirement) =
        typed_trees::service::service_requirement_declaration(program, handle)
    {
        return program
            .trait_definition_by_symbol(requirement)
            .map(|definition| definition.name.as_str());
    }
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            named_type_reference_name(program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            named_type_reference_name(program, *base_type)
        }
        TypeReferenceNode::Named { name, .. }
        | TypeReferenceNode::Generic {
            base_name: name, ..
        }
        | TypeReferenceNode::DynamicTrait { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

/// True when `type_name` resolves the value-call target through any of the
/// remaining channels the lowering understands: a boundary-trait machine
/// signature or a machine's entry. Attached calls use their selected entry
/// and exact receiver type before this unresolved-call diagnostic.
fn type_name_resolves_value_call(
    program: &dyn crate::CheckingInput,
    symbols: &TopLevelSymbols<'_>,
    type_name: &str,
    target: &str,
) -> bool {
    symbols
        .trait_definition(type_name)
        .is_some_and(|trait_definition| {
            program
                .trait_machine_signatures(trait_definition)
                .iter()
                .any(|signature| signature.name.as_str() == target)
        })
        || symbols.machine(type_name).is_some_and(|machine| {
            program
                .machine_states(machine)
                .first()
                .is_some_and(|state| state.name.as_str() == target)
        })
}

/// Decision layer for the value-call fall-through: everything the partial
/// bounds resolver above recognizes has already returned; anything the
/// LOWERING can still resolve is checked here (builtins, platform/trait
/// receivers, declared receiver types, type-name receivers). A target that
/// resolves through NONE of these names nothing anywhere -- it would silently
/// bind a ZII 0 at runtime, so it is a compile error.
#[allow(clippy::too_many_arguments)]
pub(super) fn report_unresolved_value_call(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    current_state: &State,
    symbols: &TopLevelSymbols<'_>,
    receiver_name: Option<&str>,
    receiver_type: Option<&str>,
    call: &TableCallExpression,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // A retained `size_of`/`align_of` query is a compiler-owned `u64` value
    // whose answer is the selected target's layout; each target's closure
    // folds it, so the shared judgment treats it as an opaque value.
    if call.layout_query.is_some() {
        return;
    }
    let target = call.target.as_str();
    // Named operators are callable requirements too. Use the same exact-symbol
    // or unique path+arity resolution as checked-flow analysis; a leaf spelling
    // alone is never enough to suppress this fail-closed fence.
    if typed_trees::operator::resolve_named_expression_call(program, call).is_some() {
        return;
    }
    let Some(receiver) = receiver_name else {
        // Receiverless: the three machine channels missed; only the reserved
        // value builtins remain: the composing scalar computations, the
        // value-position asm intrinsics (`asm { in dest, port }` desugars to
        // `dest = asm#port_in(port)`; the names are unnameable from source)
        // and the register reads their catalogs own.
        if program
            .symbols
            .builtin_function_for_symbol(call.target_symbol)
            .is_some_and(|builtin| builtin.is_value_position_asm_intrinsic())
            || language_core::inline_assembly::AsmControlRegister::from_read_intrinsic_name(target)
                .is_some()
            || language_core::inline_assembly::AsmSystemRegister::from_read_intrinsic_name(target)
                .is_some()
        {
            return;
        }
        // The primitive byte predicates (`valid_utf8`, `no_nul`,
        // `ascii_only`, `non_empty`) are the reserved unary boolean probes
        // over a byte-carrier operand; a domain's `requires` clause names
        // them and checked flow mints their result as per-byte-class facts
        // at the operand's place.
        if language_semantics::byte_predicates::ByteSequencePredicate::from_name(target).is_some() {
            if call.arguments.len() == 1 {
                return;
            }
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}`: byte-predicate call `{target}(..)` takes exactly \
                 one byte-carrier operand.",
                current_machine.name,
                current_state.name.as_str(),
            )));
            return;
        }
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}`: value call `{target}(..)` does not resolve to a \
             callable machine entry or requirement. Internal states are transition \
             destinations, not ordinary callees. Check the name.",
            current_machine.name,
            current_state.name.as_str(),
        )));
        return;
    };

    // Wire-schema synthesized codecs (`Schema::encode(..)` / `::decode(..)`)
    // are not user machines; a data-definition receiver resolves them.
    if matches!(target, "encode" | "decode")
        && program
            .data_definitions()
            .iter()
            .any(|definition| definition.name.as_str() == receiver)
    {
        return;
    }

    let declared_type =
        receiver_declared_type_name(program, current_machine, current_state, receiver);
    let resolves = receiver_type
        .is_some_and(|type_name| type_name_resolves_value_call(program, symbols, type_name, target))
        || declared_type
            .is_some_and(|type_name| type_name_resolves_value_call(program, symbols, type_name, target))
        // The receiver may BE a type name (`Real.from(..)`, `Worker.run(..)`).
        || type_name_resolves_value_call(program, symbols, receiver, target);
    if resolves {
        return;
    }

    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}`: value call `{receiver}.{target}(..)` does not resolve \
         to a callable machine entry or requirement. Check the receiver and method names.",
        current_machine.name,
        current_state.name.as_str(),
    )));
}
