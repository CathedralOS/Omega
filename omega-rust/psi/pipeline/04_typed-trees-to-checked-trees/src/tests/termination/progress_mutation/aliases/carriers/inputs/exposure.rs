use crate::tests::front_end::{checked_program_result, typed_program};

use super::fixture_source;
use crate::CheckingRequest;
use crate::lower_typed_trees;

fn reject_exposed_input(access: &str, preceding: &str, operand: &str, extra: &str) {
    let source = fixture_source(
        access,
        &format!(
            "let borrowed: &{access}Context = carrier.context;
             {preceding}
             transition {{ _ -> waiting({operand}, borrowed) }}
             state waiting(ignored: u64, selected: &Context) -> u64
             requires selected.scheduler in WeakFair
             {{ wait_context(selected) }}"
        ),
        extra,
    );
    let Err(diagnostics) = checked_program_result(&source) else {
        panic!("exposure cannot preserve an input reference's requires evidence");
    };
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove requires contract for call waiting")),
        "{preceding} / {operand}: {diagnostics:#?}"
    );
}

#[test]
fn earlier_operand_empty_mutable_ancestor_method_rejects_requires() {
    for access in ["", "mut "] {
        reject_exposed_input(
            access,
            "",
            "carrier.touch()",
            "machine Carrier::touch(&mut self) -> u64 { 0 }",
        );
    }
}

#[test]
fn a_referent_method_does_not_replace_a_saved_reference_binding() {
    for (preceding, operand) in [
        ("_ = carrier.context.increment_counter();", "0"),
        ("", "carrier.context.increment_counter()"),
    ] {
        let source = fixture_source(
            "mut ",
            &format!(
                "let borrowed: &mut Context = carrier.context;
                 {preceding}
                 transition {{ _ -> waiting({operand}, borrowed) }}
                 state waiting(ignored: u64, selected: &Context) -> u64
                 requires selected.scheduler in WeakFair
                 {{ wait_context(selected) }}"
            ),
            "machine Context::increment_counter(&mut self) terminates; -> u64 {
                 self.counter = 1; 0
             }",
        );
        let mut program = typed_program(&source);
        crate::lookup::resolve_projected_receiver_calls(&mut program).unwrap();
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "replace")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let statements = program.statement_table.statements(state.statement_nodes);
        let typed_trees::statement::StatementNode::LocalData(borrowed) = &statements[0] else {
            panic!("reference local")
        };
        let resolver = validation::CallFrameResolver::new(&program).unwrap();
        let (root, segments) = resolver
            .local_reference_origin_before_statement(
                machine,
                statements.last().unwrap(),
                borrowed.symbol,
            )
            .expect("referent writes do not replace the reference slot");
        assert_eq!(root, program.state_parameters(state)[0].symbol);
        let [facts::PlaceSegment::Field { symbol }] = segments.as_slice() else {
            panic!("input reference field: {segments:?}")
        };
        assert_eq!(
            program.symbols.display_path(*symbol, "::"),
            "Carrier::context"
        );
        // Reference identity is not permission to mutate through the parent
        // while the saved exclusive loan remains live.
        let Err(diagnostics) = lower_typed_trees(program, &CheckingRequest::settled()) else {
            panic!("{preceding} / {operand}: a live exclusive loan must reject mutation");
        };
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("while local borrow `borrowed` is still active")
            }),
            "{diagnostics:#?}"
        );
    }
}
