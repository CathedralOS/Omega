//! Dynamic receiver places and stored receivers.
//!
//! The place rule itself lives on `TypedTrees`
//! (`dynamic_descriptor_place`/`dynamic_dispatch_subject_place`) beside the
//! subject-position rule it depends on, so Psi's later stages rejoin a
//! dispatched place by asking the same question rather than reading a call's
//! receiver symbol, which names the trait on a qualified spelling.

use crate::execution::terminal_unit::{CheckFacts, SymbolHandle, TypedTrees};

pub(crate) use typed_trees::dynamic_traits::DynamicDescriptorPlace as DynamicReceiverPlace;

/// The place one dynamic dispatch reads its descriptor from: the operand at
/// the dispatched requirement's subject position.
///
/// A requirement names its subject as an ordinary parameter, so the authored
/// call supplies the descriptor as that position's argument
/// (`Shape::code(erased)`). The qualifier a trait-qualified call carries names
/// the trait, not storage, so the subject is located by POSITION in the
/// requirement -- the same rule `dynamic_subject_position` states for the
/// surface itself. Only the subject operand is supported: a requirement with
/// further parameters supplies no descriptor place here.
pub(crate) fn dispatch_subject_place(
    program: &TypedTrees,
    call_site: &crate::semantic::calls::CallSite<'_>,
    target_symbol: SymbolHandle,
) -> Option<DynamicReceiverPlace> {
    program.dynamic_dispatch_subject_place(
        target_symbol,
        crate::semantic::calls::call_site_argument_expressions(program, call_site),
    )
}

pub(crate) fn stored_dynamic_receiver<'facts>(
    program: &TypedTrees,
    facts: &'facts CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    call_site: &crate::semantic::calls::CallSite<'_>,
    target_symbol: SymbolHandle,
) -> Option<&'facts checked_trees::DynamicDescriptorStorageFact> {
    let place = dispatch_subject_place(program, call_site, target_symbol)?;
    facts.dynamic_conformances.stored_receiver(
        machine,
        state,
        place.root,
        &place.path,
        statement_index,
    )
}
