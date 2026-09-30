//! Uses of an owner after its custody was transferred or consumed
//! (wiki/spec/language/ownership.md, paths and control flow): every projection
//! rooted at a consumed owned place — field reads, nested fields, borrows,
//! method receivers, projected moves and field stores — rejects, while reads
//! before the transfer and after a whole re-establishing store still check.

use crate::tests::front_end::checked_program_result;

const GUARD: &str = "
    data Inner { tag: i32; }
    data Guard { handle: i32; inner: Inner; }
    machine consume(value: Guard) { }
    machine inspect(value: &Guard) -> i32 { value.handle }
    machine Guard::peek(&self) -> i32 { self.handle }
    data Main {}
";

fn rejects_consumed_use(body: &str, spelling: &str) {
    let source = format!("{GUARD} machine Main::main() -> i32 {{ {body} }}");
    let diagnostics = checked_program_result(&source)
        .err()
        .unwrap_or_else(|| panic!("`{body}` must reject a use of a consumed owner"));
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(&format!("`{spelling}`"))
                && diagnostic
                    .message
                    .contains("after `g` was transferred or consumed at statement 1")
        }),
        "{body}: {diagnostics:#?}"
    );
}

#[test]
fn every_projection_through_a_consumed_owner_rejects() {
    for (body, spelling) in [
        (
            "let g: Guard = Guard { handle: 7, inner: Inner { tag: 1 } }; consume(g); g.handle",
            "g.handle",
        ),
        (
            "let g: Guard = Guard { handle: 7, inner: Inner { tag: 1 } }; consume(g); g.inner.tag",
            "g.inner.tag",
        ),
        (
            "let g: Guard = Guard { handle: 7, inner: Inner { tag: 1 } }; consume(g); inspect(&g)",
            "g",
        ),
        (
            "let g: Guard = Guard { handle: 7, inner: Inner { tag: 1 } }; consume(g); g.peek()",
            "g",
        ),
        (
            "let g: Guard = Guard { handle: 7, inner: Inner { tag: 1 } }; consume(g);
             let moved: Inner = g.inner; moved.tag",
            "g.inner",
        ),
        (
            "let mut g: Guard = Guard { handle: 7, inner: Inner { tag: 1 } }; consume(g);
             g.handle = 3; 0",
            "g.handle",
        ),
    ] {
        rejects_consumed_use(body, spelling);
    }
}

#[test]
fn a_consumed_by_value_parameter_rejects_later_reads() {
    let source = format!(
        "{GUARD} machine Main::main() -> i32 {{ run(Guard {{ handle: 7, inner: Inner {{ tag: 1 }} }}) }}
         machine run(g: Guard) -> i32 {{ consume(g); g.handle }}"
    );
    let diagnostics =
        checked_program_result(&source).expect_err("a consumed parameter holds no value");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("after `g` was transferred or consumed at statement 0")),
        "{diagnostics:#?}"
    );
}

#[test]
fn reads_before_the_transfer_and_after_reassignment_check() {
    checked_program_result(&format!(
        "{GUARD} machine Main::main() -> i32 {{
             let mut g: Guard = Guard {{ handle: 7, inner: Inner {{ tag: 1 }} }};
             let snapshot: i32 = g.handle;
             let observed: i32 = inspect(&g);
             consume(g);
             g = Guard {{ handle: snapshot, inner: Inner {{ tag: observed }} }};
             let restored: i32 = g.inner.tag;
             consume(g);
             restored
         }}"
    ))
    .expect("the owner holds a value at every read");
}

#[test]
fn a_second_transfer_keeps_its_established_diagnostic() {
    let diagnostics = checked_program_result(&format!(
        "{GUARD} machine Main::main() -> i32 {{
             let g: Guard = Guard {{ handle: 7, inner: Inner {{ tag: 1 }} }};
             consume(g);
             consume(g);
             0
         }}"
    ))
    .expect_err("a repeated transfer rejects");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("affine value `g` was already transferred or consumed")),
        "{diagnostics:#?}"
    );
}

#[test]
fn a_transition_argument_reads_its_own_operand_after_consumption() {
    // Transition arguments live in the statement table's handle arena; the
    // replay must observe `g.handle` here, not an unrelated expression.
    let diagnostics = checked_program_result(&format!(
        "{GUARD} machine Main::main() -> i32 {{ run() }}
         machine report(value: i32) -> i32 {{ value }}
         machine run() -> i32 {{
             let g: Guard = Guard {{ handle: 7, inner: Inner {{ tag: 1 }} }};
             consume(g);
             transition {{ _ -> report(g.handle) }}
         }}"
    ))
    .expect_err("a transition argument reads through the consumed owner");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`g.handle`")
                && diagnostic
                    .message
                    .contains("after `g` was transferred or consumed at statement 1")
        }),
        "{diagnostics:#?}"
    );
}

#[test]
fn a_move_on_one_transition_arm_leaves_the_next_arm_owner_intact() {
    // Each arm is its own guarded exit statement; a missed arm falls through
    // to the next one with `g` still held.
    checked_program_result(&format!(
        "{GUARD} machine Main::main() -> i32 {{ run(true) }}
         machine finish(value: Guard) -> i32 {{ value.handle }}
         machine run(flag: bool) -> i32 {{
             let g: Guard = Guard {{ handle: 7, inner: Inner {{ tag: 1 }} }};
             transition flag {{ true -> finish(g) _ -> finish(g) }}
         }}"
    ))
    .expect("only the taken edge transfers `g`; the guard-miss path still holds it");
}
