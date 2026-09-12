use super::super::*;

fn check_source(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed)
}

fn reject_source(source: &str) -> Vec<diagnostics::Diagnostic> {
    // Stop at source checking: absence of a Terminal executable plan cannot
    // establish rejection of an unauthorized receiver access.
    let diagnostics = match check_source(source) {
        Ok(_) => panic!("receiver access must fail source checking"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        !diagnostics.is_empty(),
        "rejection must report a diagnostic"
    );
    diagnostics
}

fn reject_assignment(source: &str, root: &str) {
    let diagnostics = reject_source(source);
    let expected =
        format!("assignment cannot write `{root}` because it is not mutable in this state");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(&expected)),
        "expected assignment writability diagnostic `{expected}`: {diagnostics:#?}"
    );
}

#[test]
fn shared_receiver_rejects_overlapping_exclusive_argument() {
    for caller in [
        "machine invoke(value: &mut Pair) -> u64 { value.inspect(&mut value) }",
        "machine invoke(input: u64) -> u64 {
            let value: Pair = Pair { left: input, right: 7 };
            value.inspect(&mut value)
        }",
    ] {
        let diagnostics = reject_source(&format!(
            "data Pair {{ left: u64; right: u64; }}
         machine Pair::inspect(&self, other: &mut Pair) -> u64 {{ self.right }}
         {caller}",
        ));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(
                    "receives shared receiver overlapping another argument in the same call"
                )),
            "exact receiver/argument incompatibility: {diagnostics:#?}"
        );
        assert!(
            !diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("LET-bound"))
        );
    }
}

#[test]
fn projected_receiver_and_live_slice_require_compatible_access() {
    for (receiver, accepted) in [("&self", true), ("&mut self", false)] {
        let source = format!(
            "data Reader {{ bytes: [u8; 4]; }}
             data Container {{ reader: Reader; }}
             machine Reader::observe({receiver}, bytes: &[u8]) -> u8 {{
                 transition bytes.len > 0 {{
                     true -> (bytes[0])
                     false -> 0
                 }}
             }}
             machine Container::check(&mut self) -> u8 {{
                 let view: &[u8] = self.reader.bytes.as_slice();
                 self.reader.observe(view)
             }}"
        );
        if accepted {
            check_source(&source).expect("shared receiver and shared slice may overlap");
        } else {
            let diagnostics = reject_source(&source);
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(
                        "receives mutable receiver while local borrow `view` is still active"
                    )),
                "an unused exclusive receiver still conflicts with its field's shared view: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn shared_self_direct_field_write_rejects() {
    reject_assignment(
        r#"
            data Pair { prefix: u8; value: u16; }

            machine Pair::replace(&self) {
                self.value = 17;
            }
        "#,
        "value",
    );
}

#[test]
fn shared_self_nested_field_write_rejects() {
    reject_assignment(
        r#"
            data Inner { value: u16; }
            data Outer { inner: Inner; }

            machine Outer::replace(&self) {
                self.inner.value = 17;
            }
        "#,
        "inner",
    );
}

#[test]
fn shared_self_literal_index_write_rejects() {
    reject_assignment(
        r#"
            data Inner { values: [u16; 2]; }
            data Outer { inner: Inner; }

            machine Outer::replace(&self) {
                self.inner.values[1] = 17;
            }
        "#,
        "inner",
    );
}

#[test]
fn mutable_self_direct_field_write_checks() {
    check_source(
        r#"
            data Pair { prefix: u8; value: u16; }

            machine Pair::replace(&mut self) {
                self.value = 17;
            }
        "#,
    )
    .expect("mutable self permits direct field stores");
}

#[test]
fn mutable_self_nested_field_write_checks() {
    check_source(
        r#"
            data Inner { value: u16; }
            data Outer { inner: Inner; }

            machine Outer::replace(&mut self) {
                self.inner.value = 17;
            }
        "#,
    )
    .expect("mutable self permits nested field stores");
}

#[test]
fn mutable_self_literal_index_write_checks() {
    check_source(
        r#"
            data Inner { values: [u16; 2]; }
            data Outer { inner: Inner; }

            machine Outer::replace(&mut self) {
                self.inner.values[1] = 17;
            }
        "#,
    )
    .expect("mutable self permits literal-index stores through nested fields");
}

#[test]
fn write_only_parameter_direct_field_write_checks() {
    check_source(
        r#"
            data Pair { prefix: u8; value: u16; }

            machine replace(destination: &write Pair) {
                destination.value = 17;
            }
        "#,
    )
    .expect("write-only parameter permits a non-observing primitive field store");
}

#[test]
fn write_only_parameter_nested_field_write_checks() {
    check_source(
        r#"
            data Inner { value: u16; }
            data Outer { inner: Inner; }

            machine replace(destination: &write Outer) {
                destination.inner.value = 17;
            }
        "#,
    )
    .expect("write-only parameter permits an invariant-free nested primitive field store");
}

#[test]
fn write_only_parameter_literal_index_write_checks() {
    check_source(
        r#"
            data Inner { values: [u16; 2]; }
            data Outer { inner: Inner; }

            machine replace(destination: &write Outer) {
                destination.inner.values[1] = 17;
            }
        "#,
    )
    .expect("write-only parameter permits an in-bounds literal primitive element store");
}

#[test]
fn write_only_parameter_field_read_rejects() {
    let diagnostics = reject_source(
        r#"
            data Pair { prefix: u8; value: u16; }

            machine observe(destination: &write Pair) {
                let prior: u16 = destination.value;
            }
        "#,
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("reads field `value` from write-only parameter `destination`")
                && diagnostic.message.contains("never grants observation")
        }),
        "expected write-only parameter observation diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn write_only_primitive_reads_remain_forbidden_below_scalar_operators() {
    for source in [
        "machine observe(value: &write u64, mask: u64) -> u64 { value ^ mask }",
        "machine observe(value: &write bool) -> bool { !value }",
    ] {
        let diagnostics = reject_source(source);
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("reads write-only parameter `value`")
                && diagnostic.message.contains("never observation")),
            "nested scalar read retains source access rejection: {diagnostics:#?}"
        );
    }
}

#[test]
fn write_only_parameter_literal_index_read_rejects() {
    let diagnostics = reject_source(
        r#"
            data Inner { values: [u16; 2]; }
            data Outer { inner: Inner; }

            machine copy(destination: &write Outer) {
                destination.inner.values[0] = destination.inner.values[1];
            }
        "#,
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("reads through index projection of write-only parameter `destination`")
                && diagnostic.message.contains("never observation")
        }),
        "expected write-only parameter index observation diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn shared_self_field_cannot_supply_mutable_call_argument() {
    let diagnostics = reject_source(
        r#"
            data Pair { prefix: u8; value: u16; }

            machine replace(value: &mut u16) { value = 17; }

            machine Pair::forward(&self) {
                replace(&mut self.value);
            }
        "#,
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("mutable argument `value`")
                && diagnostic.message.contains("is not writable in this state")
        }),
        "expected mutable argument writability diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn mutable_self_field_can_supply_mutable_call_argument() {
    check_source(
        r#"
            data Pair { prefix: u8; value: u16; }

            machine replace(value: &mut u16) { value = 17; }

            machine Pair::forward(&mut self) {
                replace(&mut self.value);
            }
        "#,
    )
    .expect("mutable self may lend a primitive field to a mutable call");
}

#[test]
fn shared_state_self_cannot_inherit_mutable_entry_authority() {
    reject_assignment(
        r#"
            data Pair { prefix: u8; value: u16; }

            machine Pair::replace(&mut self) {
                transition { _ -> store() }

                state store(&self) {
                    self.value = 17;
                }
            }
        "#,
        "value",
    );
}

#[test]
fn absent_state_self_cannot_inherit_mutable_entry_authority() {
    reject_assignment(
        r#"
            data Pair { prefix: u8; value: u16; }

            machine Pair::replace(&mut self) {
                transition { _ -> store() }

                state store() {
                    self.value = 17;
                }
            }
        "#,
        "value",
    );
}

#[test]
fn mutable_state_self_field_write_checks() {
    check_source(
        r#"
            data Pair { prefix: u8; value: u16; }

            machine Pair::replace(&mut self) {
                transition { _ -> store() }

                state store(&mut self) {
                    self.value = 17;
                }
            }
        "#,
    )
    .expect("the current state's explicit mutable self permits a field store");
}

#[test]
fn local_record_receiver_cannot_acquire_unauthorized_mutation() {
    for body in [
        "let cell: Cell = Cell { value: 17 }; cell.replace();",
        "let mut cell: Cell = Cell { value: 17 }; let view: &Cell = &cell; view.replace();",
    ] {
        let source = format!(
            "data Cell {{ value: u64; }}
             machine Cell::replace(&mut self) {{ self.value = 29; }}
             machine observe() {{ {body} }}"
        );
        reject_source(&source);
    }
}

#[test]
fn local_record_receiver_cannot_be_reused_after_owned_self_transfer() {
    let diagnostics = reject_source(
        "data Cell { value: u64; }
         machine Cell::consume(self) {}
         machine observe() {
             let cell: Cell = Cell { value: 17 };
             cell.consume();
             cell.consume();
         }",
    );
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("already transferred or consumed")),
        "owned receiver transfer must retire the original local: {diagnostics:#?}"
    );
}

#[test]
fn owned_record_child_cannot_be_transferred_twice() {
    let diagnostics = reject_source("data Inner { value: u64; } data Outer { first: Inner; second: Inner; }
        machine wrap() -> Outer { let child: Inner = Inner { value: 7 }; Outer { first: child, second: child } }");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("already transferred or consumed")),
        "{diagnostics:#?}"
    );
}
