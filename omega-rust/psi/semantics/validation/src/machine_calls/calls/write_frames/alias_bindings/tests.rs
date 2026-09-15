use super::{
    TypedTrees, assignment_copies_primitive_referent, assignment_replaces_untracked_reference,
    state_reference_parameter_binding_is_stable,
};
use typed_trees::statement::StatementNode;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("symbols");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("types")
}

#[test]
fn primitive_parameter_copies_keep_exact_write_frames_and_reference_bindings() {
    for scalar in ["u8", "u64", "bool", "f32"] {
        for source_access in ["", "mut "] {
            for destination_access in ["mut", "write"] {
                let program = typed(&format!(
                    "machine observe(input: &{source_access}{scalar}, output: &{destination_access} {scalar}) {{ output = input; }}"
                ));
                let machine = &program.machines()[0];
                let state = &program.machine_states(machine)[0];
                let resolver = crate::CallFrameResolver::new(&program).expect("frame resolver");
                assert_eq!(
                    resolver
                        .inferred_state_write_frame(machine, state)
                        .complete_paths(),
                    Some(["$P1".to_owned()].as_slice()),
                    "{source_access}{scalar} -> {destination_access}",
                );
                let destination = program.state_parameters(state)[1].symbol;
                assert!(state_reference_parameter_binding_is_stable(
                    &program,
                    machine,
                    state,
                    destination
                ));
            }
        }
    }
}

#[test]
fn primitive_copy_classification_does_not_hide_reference_replacements() {
    for source in [
        "machine observe(input: &mut u8, output: &mut u8) { output = &mut input; }",
        "machine select(input: &u8) -> &u8 { input } machine observe(input: &u8, output: &mut u8) { output = select(input); }",
        "machine observe(input: &mut u8, output: &mut u8) { let mut local: &mut u8 = &mut output; local = &mut input; }",
        "data Record { value: u8; } machine observe(input: &Record, output: &mut Record) { output = input; }",
    ] {
        let program = typed(source);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "observe")
            .expect("observe");
        let state = &program.machine_states(machine)[0];
        let assignment = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| {
                if let StatementNode::Assignment(assignment) = statement {
                    Some(assignment)
                } else {
                    None
                }
            })
            .expect("assignment");
        assert!(
            !assignment_copies_primitive_referent(&program, machine, state, assignment),
            "{source}"
        );
        assert!(
            assignment_replaces_untracked_reference(&program, machine, state, assignment, &[]),
            "{source}"
        );
    }
}
