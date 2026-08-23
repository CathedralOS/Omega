//! Receiver declaration and value-call target resolution.
//!
//! This module normalizes declared receiver type shells, replays every target
//! channel understood by lowering, and emits the fail-closed unresolved-call
//! diagnostic only after those channels have been exhausted.

use crate::symbols::TopLevelSymbols;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::TableCallExpression;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::StatementNode;
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// The declared TYPE NAME of a receiver that is a state param, state local,
/// whole-machine param, or machine-owned field -- walked through reference/
/// constraint shells to the Named/Generic/DynamicTrait head. `None` for
/// primitives, arrays, slices, and unknown names.
fn receiver_declared_type_name<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    state: &State,
    receiver: &str,
) -> Option<&'program str> {
    let handle = declared_receiver_type_reference(program, machine, state, receiver)?;
    named_type_reference_name(program, handle)
}

pub(crate) fn declared_receiver_type_reference(
    program: &TypedTrees,
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
                    let StatementNode::LocalData(local) = statement else {
                        return None;
                    };
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
            // State bindings are whole-machine scope: a param declared on any
            // state of this machine is readable everywhere in it.
            program.machine_states(machine).iter().find_map(|other| {
                program
                    .state_parameters(other)
                    .iter()
                    .find(|parameter| parameter.name.as_str() == receiver)
                    .map(|parameter| parameter.type_reference)
            })
        })
        .or_else(|| {
            let attached_data = machine.attached_data.as_ref()?;
            let definition = program
                .data_definitions()
                .iter()
                .find(|definition| &definition.name == attached_data)?;
            program.data_members(definition).iter().find_map(|member| {
                let psi_typed_trees::data::DataMember::Field(field) = member else {
                    return None;
                };
                (field.name.as_str() == receiver).then_some(field.type_reference)
            })
        })
}

pub(super) fn named_type_reference_name<'program>(
    program: &'program TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<&'program str> {
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
/// channels the LOWERING understands: a boundary-trait machine signature, a
/// machine's local state, or a machine attached to that data type.
fn type_name_resolves_value_call(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    type_name: &str,
    target: &str,
) -> bool {
    if let Some(trait_definition) = symbols.trait_definition(type_name)
        && program
            .trait_machine_signatures(trait_definition)
            .iter()
            .any(|signature| signature.name.as_str() == target)
    {
        return true;
    }
    if let Some(machine) = symbols.machine(type_name)
        && program
            .machine_states(machine)
            .iter()
            .any(|state| state.name.as_str() == target)
    {
        return true;
    }
    symbols
        .attached_machine_state(program, type_name, target)
        .is_some()
}

/// Decision layer for the value-call fall-through: everything the partial
/// bounds resolver above recognizes has already returned; anything the
/// LOWERING can still resolve is checked here (builtins, platform/trait
/// receivers, declared receiver types, type-name receivers). A target that
/// resolves through NONE of these names nothing anywhere -- it would silently
/// bind a ZII 0 at runtime, so it is a compile error.
#[allow(clippy::too_many_arguments)]
pub(super) fn report_unresolved_value_call(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: &State,
    symbols: &TopLevelSymbols<'_>,
    receiver_name: Option<&str>,
    receiver_type: Option<&str>,
    call: &TableCallExpression,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let target = call.target.as_str();
    // Named operators are callable requirements too. Use the same exact-symbol
    // or unique path+arity resolution as checked-flow analysis; a leaf spelling
    // alone is never enough to suppress this fail-closed fence.
    if psi_typed_trees::operator::resolve_named_expression_call(program, call).is_some() {
        return;
    }
    let Some(receiver) = receiver_name else {
        // Receiverless: the three machine channels missed; only the reserved
        // value builtins remain. `asm#port_in` is the value-position asm
        // intrinsic (`asm { in dest, port }` desugars to `dest =
        // asm#port_in(port)`); the name is unnameable from source.
        if matches!(
            target,
            "min" | "max" | "sqrt" | "asm#port_in" | "asm#pushfq" | "asm#rdmsr"
        ) || psi_language_core::inline_assembly::AsmControlRegister::from_read_intrinsic_name(
            target,
        )
        .is_some()
        {
            return;
        }
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}`: value call `{target}(..)` does not resolve to a \
             state of this machine, an attached sibling machine, or a free machine -- \
             it would silently bind 0 (ZII) at runtime. Check the name.",
            current_machine.name,
            current_state.name.as_str(),
        )));
        return;
    };

    // Collection/text view builtins: `arr.as_slice()` / `.as_mut_slice()`,
    // the text view `text.as_view()` (the borrow layer's own builtin list,
    // borrow/loans.rs), and the view byte accessor `view.bytes()`.
    if matches!(target, "as_slice" | "as_mut_slice" | "as_view" | "bytes") {
        return;
    }
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
         to any machine state, attached machine, platform state, or boundary-trait \
         method -- it would silently bind 0 (ZII) at runtime. Check the receiver and \
         method names.",
        current_machine.name,
        current_state.name.as_str(),
    )));
}
