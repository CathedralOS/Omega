use super::check_program;

#[test]
fn accepts_folded_static_literal_join_in_persistent_borrow_storage() {
    let source = r#"
        data Main {
            stored: &[u8];
        }

        machine Main::store(&mut self) {
            self.stored = "program " + "static";
        }
    "#;

    check_program(source).expect("a folded literal join remains program-static storage");
}

#[test]
fn rejects_parameter_backed_view_call_result_in_persistent_storage() {
    let source = r#"
        data Main {
            stored: &[u8];
        }

        machine Main::forward(&self, text: &[u8]) -> &[u8] {
            text
        }

        machine Main::store(&mut self, text: &[u8]) {
            self.stored = self.forward(text);
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("the returned view still borrows the call parameter");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("assignment stores a borrow-carrying value in persistent field `stored`")),
        "expected the persistent-loan fence, got:\n{}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

// These aggregate snapshots deliberately copy shared loan carriers. Explicit
// copy permission does not establish static provenance: the paired negatives
// still require every borrowed leaf and index to retain its exact source.

#[test]
fn accepts_cross_state_static_runtime_index_forwarded_from_immutable_local() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            messages: [Message; 2];
            copy: Message;
            code: i32;
        }

        machine Main::touch_code(&mut self) {
            self.code = 7;
        }

        machine Main::store(&mut self) {
            let index: u64 [0..2] = 1;
            self.messages[index].body = "program static";
            self.touch_code();
            transition { _ -> copy_element(index) }

            state copy_element(&mut self, index: u64 [0..2]) {
                self.copy = self.messages[index];
            }
        }
    "#;

    check_program(source).expect(
        "an immutable local runtime index forwarded directly to a state parameter retains identity",
    );
}

#[test]
fn rejects_cross_state_static_runtime_index_through_mutable_local_alias() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            messages: [Message; 2];
            copy: Message;
        }

        machine Main::store(&mut self, source: u64 [0..2]) {
            let index: u64 [0..2] = source;
            let mut forwarded: u64 [0..2] = index;
            self.messages[index].body = "program static";
            transition { _ -> copy_element(forwarded) }

            state copy_element(&mut self, index: u64 [0..2]) {
                self.copy = self.messages[index];
            }
        }
    "#;

    let diagnostics = check_program(source)
        .expect_err("a mutable copy cannot establish stable runtime-index identity");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("assignment stores a borrow-carrying value in persistent field `copy`")),
        "expected the mutable-alias persistent fence, got:\n{}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn rejects_cross_state_static_runtime_index_rewritten_on_transition() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            messages: [Message; 2];
            copy: Message;
        }

        machine Main::store(&mut self, index: u64 [0..2]) {
            self.messages[index].body = "program static";
            transition { _ -> copy_element(0) }

            state copy_element(&mut self, index: u64 [0..2]) {
                self.copy = self.messages[index];
            }
        }
    "#;

    let diagnostics = check_program(source)
        .expect_err("rewriting an index does not preserve the established persistent leaf");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("assignment stores a borrow-carrying value in persistent field `copy`")),
        "expected the rewritten-index persistent fence, got:\n{}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn accepts_same_place_reassignment_from_static_persistent_storage() {
    let source = r#"
        data Main {
            stored: &[u8];
        }

        machine Main::store(&mut self) {
            self.stored = "program static";
            self.stored = self.stored;
        }
    "#;

    check_program(source)
        .expect("assignment reads established static provenance before replacing the same place");
}

#[test]
fn accepts_indexed_aggregate_copy_after_all_borrowed_leaves_become_static() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
            code: i32;
        }

        data Main {
            messages: [Message; 2];
            copy: Message;
        }

        machine Main::store(&mut self) {
            let index: u64 = 1;
            self.messages[index].body = "program static";
            self.messages[index].code = 7;
            self.copy = self.messages[index];
        }
    "#;

    check_program(source)
        .expect("an immutable indexed aggregate copy retains complete static leaf provenance");
}

#[test]
fn rejects_indexed_static_copy_through_mutable_index_binding() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            messages: [Message; 2];
            copy: Message;
        }

        machine Main::store(&mut self) {
            let mut index: u64 = 0;
            self.messages[index].body = "program static";
            index = 1;
            self.copy = self.messages[index];
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("a mutable index cannot identify one persistent source");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("assignment stores a borrow-carrying value in persistent field `copy`")),
        "expected the mutable-index provenance fence, got:\n{}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}
