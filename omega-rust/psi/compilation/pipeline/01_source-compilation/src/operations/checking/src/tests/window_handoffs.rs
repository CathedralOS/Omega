//! An invariant window opened on a loop block's formal, asked about at a
//! call in another block of the same loop. The block that broke the clause
//! passes its formal back to the loop head, which consumes the window there
//! (dependent_values.md, "Invariant windows"), so the call in the head sees
//! it closed. The head's arrival from the machine entry never passed the
//! opening write. The checker used to resolve the formal's storage at the
//! call, where the formal is out of scope, and refused every such loop with
//! "reference use without exact live backing correspondence".

use super::stored_references::run;

/// A loop whose `step` block breaks `a == b` by setting `a`, then runs
/// `restore` before passing its state back to the head. The head lends a
/// view of `row` to a call on every iteration. The entry traps unless the
/// loop ran three times and left `a` and `b` equal to 3.
fn walk(restore: &str) -> String {
    format!(
        r#"
data State requires self.a == self.b {{ row: [u8; 4]; y: u8; a: u8; b: u8; }}
machine look(bytes: &[u8]) -> u64 {{ bytes.len }}
machine walk(s: &mut State) {{
    transition {{ _ -> block head(s) }}
    machine head(s: &mut State) {{
        let length: u64 = look(&s.row[..]);
        transition s.y < 3 {{ true -> block step(s) _ -> done(s) }}
    }}
    machine step(s: &mut State) {{
        s.y = s.y + 1;
        s.a = s.y;
        {restore}
        transition {{ _ -> block head(s) }}
    }}
    machine done(s: &mut State) {{ }}
}}
machine entry() crashes Trap {{
    let mut s: State = State {{ row: [1, 2, 3, 4], y: 0, a: 0, b: 0 }};
    walk(&mut s);
    transition s.y == 3 && s.a == 3 && s.b == 3 {{ true -> ok() false -> bad() }}
    machine ok() {{ }}
    machine bad() {{ crash Trap; }}
}}
"#
    )
}

#[test]
fn a_window_restored_before_the_back_edge_is_closed_at_the_heads_call() {
    assert_eq!(
        run(&walk("s.b = s.y;")),
        Ok(terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        ))
    );
}

#[test]
fn a_window_left_open_at_the_back_edge_is_refused_there() {
    let refused = run(&walk("")).unwrap_err();
    assert!(
        refused.contains("DataDomain") && refused.contains("clause: 0"),
        "{refused}"
    );
}
