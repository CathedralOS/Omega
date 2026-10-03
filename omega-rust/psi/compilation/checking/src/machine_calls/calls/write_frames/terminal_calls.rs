//! A terminal machine invocation contributes call writes, not an internal edge.
//!
//! Named transition syntax carries both forms. Use the declaration owner to
//! classify them, then compose the callee's complete summary with actual
//! argument origins. Operand effects are collected by the state walker before
//! this call; a terminal spelling neither hides them nor grants an empty frame.

use super::caller_aliases::{CallOriginContext, CallerWriteSite};
use super::{FrameInference, Machine, MachineSymbols, SymbolHandle, TopLevelSymbols, TypedTrees};
use typed_trees::statement::{TransitionTargetHandle, TransitionTargetNode};

pub(super) fn target(
    program: &TypedTrees,
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
    crate::declarations::transitions::named_transition_call_symbol(
        program,
        caller.symbol,
        path.symbol,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn written_paths(
    program: &TypedTrees,
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
        path,
        arguments,
        static_machine_parameter,
        ..
    } = program.statement_table.transition_target(transition)
    else {
        return None;
    };
    if static_machine_parameter.is_valid() {
        return None;
    }
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
        super::known_call_written_paths_for_parts_with_origins(
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
                None,
                site,
                arguments,
                inference,
            )
        })
    }?;
    Some(origins.close_paths(paths))
}

#[cfg(test)]
mod tests {
    fn frame(source: &str) -> facts::NormalizedWriteFrame {
        let program = crate::front_end::typed_program(source);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str().ends_with("inspect"))
            .expect("caller");
        crate::CallFrameResolver::new(&program)
            .expect("resolved declarations")
            .inferred_state_write_frame(machine, &program.machine_states(machine)[0])
    }

    #[test]
    fn terminal_call_keeps_receiver_argument_and_operand_writes() {
        let summary = frame(
            r#"
            data Counter { before: i32; operand: i32; after: i32; }
            machine bump(value: &mut i32) -> i32 { value = 1; 1 }
            machine Counter::finish(&mut self, output: &mut i32, value: i32) {
                self.after = value;
                output = value;
            }
            machine Counter::inspect(&mut self, output: &mut i32) {
                self.before = 1;
                transition { _ -> self.finish(output, bump(&mut self.operand)) }
            }
        "#,
        );
        assert_eq!(
            summary.complete_paths(),
            Some(
                [
                    "$P0".to_owned(),
                    "self.after".to_owned(),
                    "self.before".to_owned(),
                    "self.operand".to_owned(),
                ]
                .as_slice()
            )
        );
    }

    #[test]
    fn internal_cycle_keeps_terminal_call_writes_outside_its_edges() {
        let summary = frame(
            r#"
            data Counter { before: i32; after: i32; }
            machine Counter::finish(&mut self) { self.after = 2; }
            machine Counter::inspect(&mut self, again: bool) {
                transition { _ -> cycle(again) }
                state cycle(&mut self, again: bool) {
                    self.before = 1;
                    transition again {
                        true -> cycle(false)
                        false -> self.finish()
                    }
                }
            }
        "#,
        );
        assert_eq!(
            summary.complete_paths(),
            Some(["self.after".to_owned(), "self.before".to_owned(),].as_slice())
        );
    }

    #[test]
    fn terminal_calls_substitute_mutable_actuals_instead_of_erasing_callee_writes() {
        for (actual, expected) in [("left", "$P0"), ("right", "$P1")] {
            let summary = frame(&format!(
                r#"
                machine fill(output: &mut i32) {{ output = 7; }}
                machine inspect(left: &mut i32, right: &mut i32) {{
                    transition {{ _ -> fill({actual}) }}
                }}
            "#
            ));
            assert_eq!(
                summary.complete_paths(),
                Some([expected.to_owned()].as_slice())
            );
        }
    }
}
