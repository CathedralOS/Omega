//! Read-only call-target, formal-type, and result-shape queries for write-frame
//! analysis.
//!
//! This leaf selects a free machine's source entry state and resolves exact
//! state symbols, sharing boundary signature selection with the frame owner.
//! It also classifies the one concrete discarded-result shape
//! that cannot redirect a returned-place relation. It does not infer write
//! frames.

use super::boundary_calls::boundary_trait_signature_for_parts;
use crate::declarations::symbols::TopLevelSymbols;
use symbols::SymbolHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

/// Formal types supply context only for newly admitted computed values. An
/// unresolved target leaves that context absent, without inventing a type or
/// changing admission of existing pure places and direct-call trees.
pub(super) fn call_argument_types(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    target_symbol: SymbolHandle,
    target_name: &str,
    receiver: &[String],
    site: super::caller_aliases::CallerWriteSite<'_>,
    symbols: &TopLevelSymbols<'_>,
) -> Vec<TypeReferenceHandle> {
    // A dynamic slot has already instantiated its complete operand roster,
    // including the independent selector. The public declaration omits that
    // lane, so reading it here would shift every actual's type by one position.
    if let Some(slot) = program.dynamic_slot_by_symbol(target_symbol) {
        return slot
            .parameters
            .iter()
            .map(|parameter| parameter.type_reference)
            .collect();
    }
    let Some((machine, state)) = machine_state_by_symbol(program, target_symbol).or_else(|| {
        receiver
            .is_empty()
            .then(|| free_machine_entry_state(program, symbols, target_name))
            .flatten()
    }) else {
        return boundary_trait_signature_for_parts(
            program,
            current_machine,
            receiver,
            target_name,
            site,
        )
        .map(|signature| signature_parameter_types(program, signature))
        .or_else(|| {
            // A resolved requirement signature supplies the same formal
            // context, pairing every formal with its explicit argument.
            super::boundary_calls::requirement_signature_for_site(
                program,
                current_machine,
                receiver,
                target_name,
                site,
            )
            .map(|signature| signature_parameter_types(program, signature))
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
        .map(|parameter| parameter.type_reference)
        .collect()
}

/// The FREE top-level machine named `target` and its entry state (`machine
/// compute(item: &Item) -> i32 { ... }`), or None. The parser names a free
/// machine's implicit entry state `entry`; explicit entry states matching the
/// call target name win first.
pub(crate) fn free_machine_entry_state<'program>(
    program: &'program dyn crate::CheckingInput,
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

pub(crate) fn machine_state_by_symbol(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<(&Machine, &State)> {
    if !symbol.is_valid() {
        return None;
    }
    let machine_symbol = program.symbols.get(symbol).parent;
    let machine = program.machine_by_symbol(machine_symbol)?;
    program
        .state_in_machine(machine, symbol)
        .map(|state| (machine, state))
}

/// A signature's formal types in declaration order — the argument context
/// either signature lane supplies.
fn signature_parameter_types(
    program: &dyn crate::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
) -> Vec<TypeReferenceHandle> {
    program
        .state_signature_parameters(signature)
        .iter()
        .map(|parameter| parameter.type_reference)
        .collect()
}
