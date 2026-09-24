use super::checks::check_program;

fn literal_move_source(body: &str) -> String {
    format!(
        r#"
        data View {{ body: &mut u64; tag: u64; }}
        data Outer {{ inner: View; }}
        data Choice {{ case Selected(view: View); case Empty; }}
        data Main {{ value: u64; other: u64; }}
        machine write_outer(mut outer: Outer) {{ outer.inner.body = 1; }}
        machine write_array(mut views: [View; 2]) {{ views[0].body = 1; }}
        machine write_choice(mut choice: Choice) {{ choice.view.body = 1; }}
        machine Main::run(&mut self) {{ {body} }}
    "#
    )
}

#[test]
fn nested_literal_moves_retain_the_authorized_carrier_access_route() {
    for body in [
        "let local: View = View { body: &mut self.value }; write_outer(Outer { inner: local });",
        "let first: View = View { body: &mut self.value }; let second: View = View { body: &mut self.other }; write_array([first, second]);",
        "let local: View = View { body: &mut self.value }; write_choice(Choice::Selected { view: local });",
    ] {
        check_program(&literal_move_source(body))
            .expect("literal nesting preserves caller access route");
    }
}

#[test]
fn nested_literal_move_does_not_consume_an_owner_with_an_active_child_loan() {
    let source = r#"
        data Cell { value: u64; }
        data Owned { cell: Cell; }
        machine consume(owned: Owned) {}
        machine exercise() {
            let mut cell: Cell = Cell { value: 0 };
            let child: &mut u64 = &mut cell.value;
            consume(Owned { cell: cell });
            child = 1;
        }
    "#;
    let diagnostics =
        check_program(source).expect_err("constructor cannot hide borrowed owner transfer");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("receives an owned value while local borrow `child`")),
        "{diagnostics:?}"
    );
}

#[test]
fn nested_literal_duplicate_carrier_moves_remain_rejected() {
    let diagnostics = check_program(&literal_move_source(
        "let local: View = View { body: &mut self.value }; write_array([local, local]);",
    ))
    .expect_err("literal projection cannot erase duplicate ownership transfer");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("local")
                && (diagnostic.message.contains("move")
                    || diagnostic.message.contains("consum")
                    || diagnostic.message.contains("owned"))
        }),
        "{diagnostics:?}"
    );
}

#[test]
fn nested_literal_move_cannot_reborrow_the_captured_owner_in_a_sibling() {
    let diagnostics = check_program(&literal_move_source(
        "let local: View = View { body: &mut self.value }; write_array([local, View { body: &mut self.value }]);",
    )).expect_err("literal sibling still conflicts with active carrier loan");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("while local borrow `local`")),
        "{diagnostics:?}"
    );
}
