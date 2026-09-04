//! Read-only call-target, formal-type, and result-shape queries for write-frame
//! analysis.
//!
//! This leaf selects a free machine's source entry state and resolves exact
//! state symbols, sharing boundary signature selection with the frame owner.
//! It also classifies the one concrete discarded-result shape
//! that cannot redirect a returned-place relation. It does not infer write
//! frames.

use super::boundary_calls::boundary_trait_signature_for_parts;
use crate::symbols::{MachineSymbols, TopLevelSymbols};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::TableCall;
use psi_typed_trees::types::TypeReferenceHandle;

/// Formal types supply context only for newly admitted computed values. An
/// unresolved target leaves that context absent, without inventing a type or
/// changing admission of existing pure places and direct-call trees.
pub(super) fn call_argument_types(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
    target_name: &str,
    receiver: &[String],
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
) -> Vec<TypeReferenceHandle> {
    let Some((machine, state)) = machine_state_by_symbol(program, target_symbol).or_else(|| {
        receiver
            .is_empty()
            .then(|| free_machine_entry_state(program, symbols, target_name))
            .flatten()
    }) else {
        return boundary_trait_signature_for_parts(
            program,
            machine_symbols,
            symbols,
            receiver,
            target_name,
        )
        .map(|signature| {
            program
                .state_signature_parameters(signature)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .map(|parameter| parameter.type_reference)
                .collect()
        })
        .unwrap_or_default();
    };
    if receiver.is_empty() == machine.attached_data.is_some()
        || !program.machine_type_parameters(machine).is_empty()
    {
        return Vec::new();
    }
    program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .map(|parameter| parameter.type_reference)
        .collect()
}

/// The FREE top-level machine named `target` and its entry state (`machine
/// compute(item: &Item) -> i32 { ... }`), or None. The parser names a free
/// machine's implicit entry state `entry`; explicit entry states matching the
/// call target name win first.
pub(crate) fn free_machine_entry_state<'program>(
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

pub(in crate::calls) fn machine_state_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<(&Machine, &State)> {
    if !symbol.is_valid() {
        return None;
    }
    let machine_symbol = program.symbols.get(symbol).parent;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == symbol)
        .map(|state| (machine, state))
}

/// An explicitly discarded result cannot redirect a returned-place relation
/// only when the resolved internal callee is an ordinary nongeneric body and
/// its declared result is a concrete primitive. The parent complete-frame
/// check remains responsible for proving every side write. Boundary, generic,
/// reference-bearing, aggregate, and unresolved calls fail closed.
pub(super) fn discarded_primitive_internal_call_is_relationally_neutral(
    program: &TypedTrees,
    call: &TableCall,
    symbols: &TopLevelSymbols<'_>,
) -> bool {
    let Some((callee_machine, callee_state)) = machine_state_by_symbol(program, call.target_symbol)
        .or_else(|| {
            call.receiver
                .is_empty()
                .then(|| free_machine_entry_state(program, symbols, call.target.as_str()))
                .flatten()
        })
    else {
        return false;
    };
    call.receiver.is_empty() != callee_machine.attached_data.is_some()
        && callee_machine.supply_mode == psi_language_semantics::MachineSupplyMode::CheckedBody
        && callee_machine.lifetime_parameters.is_empty()
        && program.machine_type_parameters(callee_machine).is_empty()
        && call.machine_arguments.is_empty()
        && program
            .primitive_type_reference(callee_state.return_type)
            .is_some()
}
