//! Pattern bindings do not turn borrowed affine payloads into owned snapshots.
use crate::tests::front_end::checked_program_result;

fn check_case_source(
    source: &str,
) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    checked_program_result(source)
}

#[test]
fn borrowed_indexed_affine_case_observation_preserves_array() {
    for receiver in ["&", "&mut"] {
        let source = format!(
            r#"
            data Kind {{ case Missing; case Other; }}
            data Holder {{ kinds: [Kind; 2]; }}
            machine Holder::observe({receiver} self, slot: u64 [0..=1]) -> i32 {{
                transition self.kinds[slot] {{
                    Kind::Missing -> found(slot)
                    _ -> other()
                }}
                state found({receiver} self, slot: u64 [0..=1]) -> i32 {{
                    transition self.kinds[slot] in Kind::Missing {{
                        true -> 1
                        false -> 2
                    }}
                }}
                state other({receiver} self) -> i32 {{ 0 }}
            }}
        "#
        );
        check_case_source(&source)
            .expect("case tests observe indexed affine values without extracting them");
    }
}

#[test]
fn indexed_observation_keeps_ordinary_and_argument_calls_before_scope_exit() {
    for body in [
        "let held: &Kind = &self.kinds[slot]; _ = self.reset(); transition held { Kind::Missing -> fill(slot) _ -> fill(slot) }",
        "transition self.kinds[slot] { Kind::Missing -> fill(self.reset()) _ -> fill(slot) }",
    ] {
        let source = format!(
            r#"
            data Kind {{ case Missing; case Other; }}
            data Holder {{ kinds: [Kind; 2]; }}
            machine Holder::reset(&mut self) -> u64 [0..=1] {{ self.kinds[0] = Kind::Other; 0 }}
            machine Holder::replace(&mut self, slot: u64 [0..=1]) {{
                {body}
                state fill(&mut self, slot: u64 [0..=1]) {{ self.kinds[slot] = Kind::Other; }}
            }}
        "#
        );
        let diagnostics =
            check_case_source(&source).expect_err("pre-exit calls retain active loans");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("still active")),
            "{diagnostics:#?}"
        );
    }
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

#[test]
fn borrowed_record_field_copy_constructor_preserves_receiver() {
    for receiver in ["&mut", "&"] {
        check_case_source(&record_field_constructor_source(receiver, "[copy]"))
            .expect("an explicitly copyable field can initialize an owned constructor payload");
    }
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
