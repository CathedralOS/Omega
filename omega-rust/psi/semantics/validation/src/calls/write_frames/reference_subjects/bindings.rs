//! Exact reference transport checks exposure independently of may-write frames.
//! An earlier operand can expose a slot even when its helper frame is empty.

use super::super::caller_aliases::{CallerWriteSite, caller_statement_at_site};
use super::super::*;

/// A checked local carrier can capture a reference binding without exposing
/// it to another machine. Every call operand inside that initializer still
/// needs its own exposure fence, independently of the call's write frame.
pub(in crate::calls::write_frames) fn calls_expose_bindings(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    aliases: &[(String, FramePlaceOrigin)],
) -> bool {
    caller_aliases::expression_any(program, expression, |expression| {
        let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
            return false;
        };
        program
            .expression_table
            .expression_handles(call.arguments)
            .iter()
            .copied()
            .chain(call.receiver.is_valid().then_some(call.receiver))
            .any(|operand| {
                local_aliases::expression_reborrows_stable_alias_binding(
                    program, operand, parameters, aliases,
                )
            })
    })
}

pub(in crate::calls::write_frames) fn are_stable_at_site(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    site: CallerWriteSite<'_>,
) -> Option<()> {
    let (state, before, _) = caller_statement_at_site(program, machine, site)?;
    let prefix = walk_state_write_prefix(
        program,
        machine,
        state,
        symbols,
        &mut FrameInference::default(),
        &mut Vec::new(),
        Some(StateWriteQuery::ReferenceBefore(before)),
    )?;
    let reference_binding_exposed = |expression| {
        local_aliases::expression_reborrows_stable_alias_binding(
            program,
            expression,
            program.state_parameters(state),
            &prefix.aliases,
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
            ) || reference_binding_exposed(expression)
        }
        CallerWriteSite::Statement(_) => return None,
    };
    (!exposed).then_some(())
}
