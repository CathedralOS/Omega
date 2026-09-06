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
