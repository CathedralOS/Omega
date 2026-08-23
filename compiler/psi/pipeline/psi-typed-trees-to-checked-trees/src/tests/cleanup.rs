use super::*;

fn diagnostics(source: &str) -> Vec<psi_diagnostics::Diagnostic> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect_err("invalid cleanup declaration must reject")
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
fn accepts_reserved_cleanup_shape() {
    let source = r#"
        data Wrapper { value: i32; }
        machine Wrapper::drop(&mut self) {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("reserved cleanup shape should check");
}

#[test]
fn accepts_exact_one_call_executable_cleanup_shape() {
    let source = r#"
        data Helper {}
        machine Helper::touch() {}
        data Wrapper { value: i32; }
        machine Wrapper::drop(&mut self) { Helper::touch(); }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("exact one-call cleanup shape should check");
}

#[test]
fn accepts_exact_two_call_executable_cleanup_shape() {
    let source = r#"
        data First {}
        machine First::touch() {}
        data Second {}
        machine Second::touch() {}
        data Wrapper { value: i32; }
        machine Wrapper::drop(&mut self) {
            First::touch();
            Second::touch();
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("exact two-call cleanup shape should check");
}

#[test]
fn accepts_exact_five_call_executable_cleanup_shape() {
    let source = r#"
        data First {}
        machine First::touch() {}
        data Second {}
        machine Second::touch() {}
        data Third {}
        machine Third::touch() {}
        data Fourth {}
        machine Fourth::touch() {}
        data Fifth {}
        machine Fifth::touch() {}
        data Wrapper { value: i32; }
        machine Wrapper::drop(&mut self) {
            First::touch();
            Second::touch();
            Third::touch();
            Fourth::touch();
            Fifth::touch();
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("exact five-call cleanup shape should check");
}

#[test]
fn executable_cleanup_rejects_repeated_nonempty_or_argumented_helpers() {
    rejects(
        r#"
            data Helper {}
            machine Helper::touch() {}
            data Wrapper { value: i32; }
            machine Wrapper::drop(&mut self) {
                Helper::touch();
                Helper::touch();
            }
        "#,
        "outside the executable cleanup slice",
    );
    rejects(
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
        "outside the executable cleanup slice",
    );
    rejects(
        r#"
            data Helper {}
            machine Helper::touch(value: u8) {}
            data Wrapper { value: i32; }
            machine Wrapper::drop(&mut self) { Helper::touch(1u8); }
        "#,
        "outside the executable cleanup slice",
    );
}

#[test]
fn accepts_cleanup_for_generic_attached_data() {
    let source = r#"
        data Wrapper<T> { value: T; }
        machine Wrapper::drop(&mut self) {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("attached-data generics are inherited through exact Self");
}

#[test]
fn rejects_owned_or_shared_cleanup_receiver() {
    for receiver in ["self", "&self"] {
        rejects(
            &format!("data Wrapper {{ value: i32; }} machine Wrapper::drop({receiver}) {{}}"),
            "must have exactly the receiver `&mut self`",
        );
    }
}

#[test]
fn rejects_cleanup_positional_parameters() {
    rejects(
        "data Wrapper { value: i32; } machine Wrapper::drop(&mut self, extra: i32) {}",
        "must have exactly the receiver `&mut self`",
    );
}

#[test]
fn rejects_cleanup_without_receiver() {
    rejects(
        "data Wrapper { value: i32; } machine Wrapper::drop() {}",
        "must have exactly the receiver `&mut self`",
    );
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
        data Wrapper { value: i32; }
        boundary machine Wrapper::drop(&mut self) terminates; ensures true;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("terminating bodyless cleanup should check");
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
