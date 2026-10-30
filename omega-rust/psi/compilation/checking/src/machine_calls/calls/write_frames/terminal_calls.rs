//! A terminal machine invocation contributes call writes, not an internal edge.
//!
//! Named transition syntax carries both forms. Use the declaration owner to
//! classify them, then compose the callee's complete summary with actual
//! argument origins. Operand effects are collected by the state walker before
//! this call; a terminal spelling neither hides them nor grants an empty frame.
//!
//! Specialization has already selected the executable target of a static
//! callable. Its retained public binder still owns source contract admission;
//! that provenance marker does not make the selected body's ordinary write
//! and result transport unavailable. All terminal invocations use the same
//! resolved-target join as ordinary calls.
//!
//! A tail call to the calling machine itself is not a terminal invocation.
//! Runtime recursion lowers to a backedge into the entry state, and its frame
//! is the same entry-state summary the call would have asked for, so it would
//! depend on itself and degrade to opaque. It is a named edge to the entry
//! state instead (`transition_topology::named_transition_target_state`), and
//! the cycle equations substitute its actuals like any other edge.

use super::caller_aliases::{CallOriginContext, CallerWriteSite};
use super::{FrameInference, Machine, MachineSymbols, SymbolHandle, TopLevelSymbols};
use typed_trees::statement::{TransitionTargetHandle, TransitionTargetNode};

pub(super) fn target(
    program: &dyn crate::CheckingInput,
    caller: &Machine,
    transition: TransitionTargetHandle,
) -> SymbolHandle {
    if !transition.is_valid() {
        return SymbolHandle::invalid();
    }
    let TransitionTargetNode::Named { path, .. } =
        program.statement_table.transition_target(transition)
    else {
        return SymbolHandle::invalid();
    };
    if path.symbol == caller.symbol {
        return SymbolHandle::invalid();
    }
    crate::declarations::transitions::named_transition_call_symbol(
        program,
        caller.symbol,
        path.symbol,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn written_paths(
    program: &dyn crate::CheckingInput,
    caller: &Machine,
    transition: TransitionTargetHandle,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    origins: &CallOriginContext<'_>,
) -> Option<Vec<String>> {
    // Retain the exact callable state: spelling-based lookup can accidentally
    // substitute an internal destination with a different write contract.
    let callee_symbol = target(program, caller, transition);
    if !callee_symbol.is_valid() {
        return None;
    }
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(transition)
    else {
        return None;
    };
    let (name, receiver) = program
        .statement_table
        .name_path_members(path.members)
        .split_last()?;
    let receiver = receiver
        .iter()
        .map(|member| member.as_str().to_owned())
        .collect::<Vec<_>>();
    let arguments = program.statement_table.expression_handles(*arguments);
    let site = CallerWriteSite::Transition(transition);
    let argument_types = super::call_targets::call_argument_types(
        program,
        caller,
        callee_symbol,
        name.as_str(),
        &receiver,
        site,
        symbols,
    );
    let argument_origins = origins.argument_origins(
        program,
        caller,
        machine_symbols,
        symbols,
        inference,
        arguments,
        &argument_types,
    )?;
    let paths = if super::machine_state_by_symbol(program, callee_symbol).is_some() {
        super::known_call_written_paths_for_parts(
            program,
            callee_symbol,
            name.as_str(),
            &receiver,
            &[],
            None,
            arguments,
            caller,
            machine_symbols,
            symbols,
            inference,
            Some(&argument_origins),
        )
    } else {
        // Signature calls retain the same origin restrictions as ordinary
        // statement calls. A selected requirement never falls back to a
        // same-spelled machine body, and its qualifier grants no storage.
        if arguments.iter().any(|argument| {
            super::assignment_targets::expression_is_effectful_indexed_place(program, *argument)
        }) {
            return None;
        }
        super::boundary_calls::known_boundary_call_written_paths_for_parts(
            program,
            caller,
            machine_symbols,
            symbols,
            &receiver,
            name.as_str(),
            site,
            arguments,
            inference,
        )
        .or_else(|| {
            super::boundary_calls::known_requirement_call_written_paths_for_parts(
                program,
                caller,
                machine_symbols,
                symbols,
                &receiver,
                name.as_str(),
                site,
                arguments,
                inference,
            )
        })
    }?;
    Some(origins.close_paths(program, paths))
}
