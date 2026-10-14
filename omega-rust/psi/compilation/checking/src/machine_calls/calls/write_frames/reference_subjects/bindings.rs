//! Exact reference transport checks exposure independently of may-write frames.
//! An earlier operand can expose a slot even when its helper frame is empty.
//! A named reference's explicit reborrow observes its established referent,
//! not the binding slot. Use the write summary's origin join here too; tracking
//! shared contents must not turn a proven address into an opaque one. Frozen
//! carrier-leaf replacement and loan-access validity remain separate checks.

use super::super::caller_aliases::{CallerWriteSite, caller_statement_at_site};
use super::super::{
    ExpressionHandle, FramePlaceOrigin, Machine, StateParameter, TopLevelSymbols, caller_aliases,
    local_aliases, stored_origins,
};
use crate::machine_calls::calls::write_frames::state_write_walk::{
    CollectedStatementPrefix, collected_prefix_at,
};
use std::sync::Mutex;
use symbols::SymbolHandle;
use symbols::SymbolKeyMap as HashMap;

/// A checked local carrier can capture a reference binding without exposing
/// it to another machine. Every call operand inside that initializer still
/// needs its own exposure fence, independently of the call's write frame.
pub(in crate::machine_calls::calls::write_frames) fn calls_expose_bindings(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    divergent_aliases: &[(String, Vec<FramePlaceOrigin>)],
) -> bool {
    caller_aliases::expression_any(program, expression, |expression| {
        let Some(call) = program.expression_table.call_node(expression) else {
            return false;
        };
        program
            .expression_table
            .expression_handles(call.arguments)
            .iter()
            .copied()
            .chain(call.receiver.is_valid().then_some(call.receiver))
            .any(|operand| {
                local_aliases::expression_reborrows_unresolved_reference_binding(
                    program,
                    machine,
                    operand,
                    parameters,
                    isolated_local_roots,
                    aliases,
                    divergent_aliases,
                )
            })
    })
}

pub(in crate::machine_calls::calls::write_frames) fn are_stable_at_site(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    site: CallerWriteSite<'_>,
    collections: &Mutex<
        HashMap<(SymbolHandle, SymbolHandle), Option<Vec<Option<CollectedStatementPrefix>>>>,
    >,
) -> Option<()> {
    let (state, _, index) = caller_statement_at_site(program, machine, symbols, site)?;
    let prefix = collected_prefix_at(collections, program, machine, state, symbols, index, true)?;
    let reference_binding_exposed = |expression| {
        local_aliases::expression_reborrows_unresolved_reference_binding(
            program,
            machine,
            expression,
            program.state_parameters(state),
            &[],
            &prefix.aliases,
            &prefix.divergent,
        )
    };
    let exposed = match site {
        CallerWriteSite::Call(call) => {
            stored_origins::call_exposes_frozen_binding(
                program,
                machine,
                state,
                call,
                &prefix.stored,
                &prefix.aliases,
                symbols,
                &mut super::super::FrameInference::for_state(state.symbol),
                false,
            ) || program
                .statement_table
                .expression_handles(call.arguments)
                .iter()
                .any(|expression| reference_binding_exposed(*expression))
        }
        CallerWriteSite::Expression(expression) => {
            stored_origins::expression_exposes_frozen_binding(
                program,
                machine,
                state,
                expression,
                &prefix.stored,
                &prefix.aliases,
                symbols,
                &mut super::super::FrameInference::for_state(state.symbol),
            ) || reference_binding_exposed(expression)
        }
        CallerWriteSite::Statement(_) | CallerWriteSite::Transition(_) => return None,
    };
    (!exposed).then_some(())
}
