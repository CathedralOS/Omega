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
            .contains("cannot transfer a non-copy value out of borrowed storage")),
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

fn record_field_constructor_source(receiver: &str, multiplicity: &str) -> String {
    format!(
        r#"
        data Kind {multiplicity} {{ case Missing; case Other; }}
        data Outcome {{ case Error(kind: Kind); case Ok; }}
        data Holder {{ kind: Kind; }}
        machine Holder::wrap({receiver} self) -> Outcome {{
            Outcome::Error {{ kind: self.kind }}
        }}
    "#
    )
}

fn rejects_borrowed_record_field_constructor(receiver: &str) {
    rejects_borrowed_transfer(&record_field_constructor_source(receiver, ""));
}

#[test]
fn borrowed_record_field_mutable_constructor_cannot_move_affine_value() {
    rejects_borrowed_record_field_constructor("&mut");
}

#[test]
fn borrowed_record_field_shared_constructor_cannot_move_affine_value() {
    rejects_borrowed_record_field_constructor("&");
}

#[test]
fn borrowed_record_field_copy_constructor_preserves_receiver() {
    for receiver in ["&mut", "&"] {
        check_case_source(&record_field_constructor_source(receiver, "[copy]"))
            .expect("an explicitly copyable field can initialize an owned constructor payload");
    }
}

#[test]
fn borrowed_record_field_affine_tag_observation_preserves_receiver() {
    for receiver in ["&mut", "&"] {
        let source = format!(
            r#"
            data Kind {{ case Missing; case Other; }}
            data Holder {{ kind: Kind; }}
            machine Holder::observe({receiver} self) -> i32 {{
                transition self.kind {{ Kind::Missing -> found() _ -> other() }}
                state found({receiver} self) -> i32 {{ 1 }}
                state other({receiver} self) -> i32 {{ 0 }}
            }}
        "#
        );
        check_case_source(&source).expect("observing a borrowed field's tag does not move it");
    }
}

#[test]
fn owned_record_field_constructor_transfers_affine_value() {
    let source = record_field_constructor_source("", "");
    check_case_source(&source).expect("owned receiver permits extracting its affine field");
}

#[test]
fn owned_wrapper_cannot_transfer_affine_field_from_borrowed_referent() {
    for reference in ["&", "&mut"] {
        let source = format!(
            r#"
            data Kind {{ case Missing; case Other; }}
            data Holder {{ kind: Kind; }}
            data Wrapper {{ holder: {reference} Holder; }}
            data Outcome {{ case Error(kind: Kind); case Ok; }}
            machine wrap(wrapper: Wrapper) -> Outcome {{
                Outcome::Error {{ kind: wrapper.holder.kind }}
            }}
        "#
        );
        rejects_borrowed_transfer(&source);
    }
}

#[test]
fn owned_wrapper_transfers_reference_carrier_without_taking_referent() {
    let source = r#"
        data Kind { case Missing; case Other; }
        data Holder { kind: Kind; }
        data Wrapper { holder: &mut Holder; }
        machine inspect(holder: &mut Holder) -> i32 {
            transition holder.kind { Kind::Missing -> found() _ -> other() }
            state found() -> i32 { 1 }
            state other() -> i32 { 0 }
        }
        machine forward(wrapper: Wrapper) -> i32 { inspect(wrapper.holder) }
    "#;
    check_case_source(source).expect("moving an owned reference carrier preserves its borrow");
}

#[test]
fn constructor_borrow_formation_does_not_move_affine_referent() {
    let source = r#"
        data Cell { value: u64; }
        data View { body: &mut Cell; }
        data Outer { inner: View; }
        data Main { cell: Cell; }
        machine write(mut outer: Outer) { outer.inner.body.value = 1; }
        machine Main::run(&mut self) {
            let local: View = View { body: &mut self.cell };
            write(Outer { inner: local });
        }
    "#;
    check_case_source(source).expect("constructing and moving a borrow does not move its referent");
}

#[test]
fn computed_borrow_target_still_consumes_nested_owned_call_argument() {
    let source = r#"
        data Token {}
        data Cell { value: u64; }
        data View { body: &mut Cell; }
        data Main { cells: [Cell; 2]; }
        machine index(token: Token) -> u64 [0..=0] { 0 }
        machine consume(token: Token) {}
        machine Main::run(&mut self, token: Token) {
            let view: View = View { body: &mut self.cells[index(token)] };
            consume(token);
        }
    "#;
    let diagnostics = check_case_source(source)
        .expect_err("computing a borrow target still moves its owned call argument");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("token")
                && (diagnostic.message.contains("move") || diagnostic.message.contains("consum"))),
        "{diagnostics:#?}"
    );
}
