use crate::tests::front_end::{checked_program, checked_program_result};

fn diagnostics(source: &str) -> Vec<diagnostics::Diagnostic> {
    checked_program_result(source).expect_err("invalid cleanup declaration must reject")
}

fn rejects(source: &str, expected: &str) {
    let diagnostics = diagnostics(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected)),
        "expected diagnostic containing `{expected}`, got {diagnostics:?}"
    );
}

#[test]
fn rejects_authored_method_selection_of_reserved_cleanup() {
    rejects(
        r#"
            data Resource { value: i32; }
            machine Resource::drop(&mut self) {}
            machine misuse(resource: &mut Resource) {
                resource.drop();
            }
        "#,
        "reserved cleanup machine `Resource::drop` is compiler-selected",
    );
}

#[test]
fn accepts_repeated_nonempty_or_argumented_hook_bodies() {
    for source in [
        r#"
            data Helper {}
            machine Helper::touch() {}
            data Wrapper { value: i32; }
            machine Wrapper::drop(&mut self) {
                Helper::touch();
                Helper::touch();
            }
        "#,
        r#"
            data Leaf {}
            machine Leaf::finish() {}
            data First {}
            machine First::touch() {}
            data Second {}
            machine Second::touch() { Leaf::finish(); }
            data Wrapper { value: i32; }
            machine Wrapper::drop(&mut self) {
                First::touch();
                Second::touch();
            }
        "#,
        r#"
            data Helper {}
            machine Helper::touch(value: u8) {}
            data Wrapper { value: i32; }
            machine Wrapper::drop(&mut self) { Helper::touch(1u8); }
        "#,
    ] {
        checked_program(source);
    }
}

#[test]
fn rejects_non_mutable_cleanup_receiver() {
    for receiver in ["self", "&self", "&write self"] {
        rejects(
            &format!("data Wrapper {{ value: i32; }} machine Wrapper::drop({receiver}) {{}}"),
            "must have exactly the receiver `&mut self`",
        );
    }
}

#[test]
fn rejects_method_local_cleanup_generic() {
    rejects(
        "data Wrapper { value: i32; } machine Wrapper::drop<T>(&mut self) {}",
        "may not declare method-local lifetime or type parameters",
    );
}

#[test]
fn rejects_cleanup_result() {
    rejects(
        "data Wrapper { value: i32; } machine Wrapper::drop(&mut self) -> i32 { 0 }",
        "must return Unit",
    );
}

#[test]
fn bodyless_cleanup_requires_published_termination() {
    rejects(
        "data Wrapper { value: i32; } boundary machine Wrapper::drop(&mut self) ensures true;",
        "must publish `terminates;`",
    );

    let source = r#"
        pub data Wrapper { value: i32; }
        boundary machine Wrapper::drop(&mut self) terminates; ensures true;
    "#;
    checked_program(source);
}

#[test]
fn rejects_suspending_or_blocking_cleanup() {
    for behavior in ["suspends;", "blocks;"] {
        rejects(
            &format!(
                "data Wrapper {{ value: i32; }} machine Wrapper::drop(&mut self) {behavior} {{}}"
            ),
            "must be non-suspending and nonblocking",
        );
    }
}

#[test]
fn rejects_cleanup_crash_contract() {
    for cause in ["Trap", "Abort"] {
        rejects(
            &format!(
                "data Wrapper {{ value: i32; }} machine Wrapper::drop(&mut self) crashes {cause} {{}}"
            ),
            "may not declare a crash outcome",
        );
    }
}
