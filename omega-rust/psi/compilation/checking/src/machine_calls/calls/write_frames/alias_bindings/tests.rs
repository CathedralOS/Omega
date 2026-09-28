use super::{assignment_copies_primitive_referent, assignment_replaces_untracked_reference};
use typed_trees::statement::StatementNode;

#[test]
fn primitive_copy_classification_does_not_hide_reference_replacements() {
    for source in [
        "machine observe(input: &mut u8, output: &mut u8) { output = &mut input; }",
        "machine select(input: &u8) -> &u8 { input } machine observe(input: &u8, output: &mut u8) { output = select(input); }",
        "machine observe(input: &mut u8, output: &mut u8) { let mut local: &mut u8 = &mut output; local = &mut input; }",
        "data Record { value: u8; } machine observe(input: &Record, output: &mut Record) { output = input; }",
    ] {
        let program = crate::front_end::typed_program(source);
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
