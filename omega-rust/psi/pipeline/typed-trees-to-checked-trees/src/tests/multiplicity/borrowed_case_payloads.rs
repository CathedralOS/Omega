//! Pattern bindings do not turn borrowed affine payloads into owned snapshots.
use super::*;

fn check_case_source(
    source: &str,
) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize case custody");
    let syntax = parse_syntax_trees(&tokens).expect("parse case custody");
    let resolved = lower_syntax_trees(&syntax).expect("resolve case custody");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type case custody");
    lower_typed_trees(typed)
}

fn rejects_borrowed_transfer(source: &str) {
    let diagnostics = check_case_source(source).expect_err("borrowed affine transfer must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot transfer a non-copy case payload out of borrowed storage")),
        "{diagnostics:#?}"
    );
}

const TYPES: &str = "
    data Kind { case Missing; case Other; }
    data Outcome { case Error(kind: Kind); case Ok; }
    data Holder { outcome: Outcome; }
";

#[test]
fn borrowed_affine_case_payload_cannot_become_owned_state_parameter() {
    let source = format!(
        "{TYPES}\n{}",
        r#"
        machine Holder::run(&mut self) -> i32 {
            transition self.outcome {
                Outcome::Error { kind } -> failed(kind)
                _ -> done()
            }
            state failed(&mut self, kind: Kind) -> i32 {
                transition kind { Kind::Missing -> done() _ -> done() }
            }
            state done(&mut self) -> i32 { 0 }
        }
    "#
    );
    rejects_borrowed_transfer(&source);
}

#[test]
fn borrowed_affine_case_payload_cannot_feed_two_owned_parameters() {
    let source = format!(
        "{TYPES}\n{}",
        r#"
        machine Holder::run(&mut self) -> i32 {
            transition self.outcome {
                Outcome::Error { kind } -> failed(kind, kind)
                _ -> done()
            }
            state failed(&mut self, first: Kind, second: Kind) -> i32 { 0 }
            state done(&mut self) -> i32 { 0 }
        }
    "#
    );
    rejects_borrowed_transfer(&source);
}

#[test]
fn borrowed_affine_case_extraction_cannot_leave_whole_parent_readable() {
    let source = format!(
        "{TYPES}\n{}",
        r#"
        machine inspect(value: &Outcome) -> i32 { 0 }
        machine Holder::run(&mut self) -> i32 {
            transition self.outcome {
                Outcome::Error { kind } -> failed(kind)
                _ -> done()
            }
            state failed(&mut self, kind: Kind) -> i32 { inspect(&self.outcome) }
            state done(&mut self) -> i32 { 0 }
        }
    "#
    );
    rejects_borrowed_transfer(&source);
}

#[test]
fn borrowed_copy_case_payload_can_feed_owned_state_parameter() {
    let types = TYPES.replace("data Kind {", "data Kind [copy] {");
    let source = format!(
        "{types}\n{}",
        r#"
        machine Holder::run(&mut self) -> i32 {
            transition self.outcome {
                Outcome::Error { kind } -> failed(kind)
                _ -> done()
            }
            state failed(&mut self, kind: Kind) -> i32 {
                transition kind { Kind::Missing -> done() _ -> done() }
            }
            state done(&mut self) -> i32 { 0 }
        }
    "#
    );
    check_case_source(&source).expect("copy payload observation retains borrowed parent");
}

#[test]
fn borrowed_affine_sum_tag_observation_does_not_extract_payload() {
    let source = format!(
        "{TYPES}\n{}",
        r#"
        machine Holder::run(&mut self) -> i32 {
            transition self.outcome { Outcome::Ok -> done() _ -> done() }
            state done(&mut self) -> i32 { 0 }
        }
    "#
    );
    check_case_source(&source).expect("tag observation does not move affine payload");
}

#[test]
fn owned_affine_case_payload_can_transfer_to_state_parameter() {
    let source = format!(
        "{TYPES}\n{}",
        r#"
        machine run(outcome: Outcome) -> i32 {
            transition outcome {
                Outcome::Error { kind } -> failed(kind)
                _ -> done()
            }
            state failed(kind: Kind) -> i32 {
                transition kind { Kind::Missing -> done() _ -> done() }
            }
            state done() -> i32 { 0 }
        }
    "#
    );
    check_case_source(&source).expect("owned case extraction transfers its payload");
}

#[test]
fn comparison_still_consumes_nested_call_owned_arguments() {
    let source = format!(
        "{TYPES}\n{}",
        r#"
        machine consume(kind: Kind) -> bool { true }
        machine Holder::run(&mut self) -> bool {
            transition self.outcome {
                Outcome::Error { kind } -> (consume(kind) == true)
                _ -> false
            }
        }
    "#
    );
    rejects_borrowed_transfer(&source);
}
