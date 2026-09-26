use super::check_program;

#[test]
fn rejects_persistent_borrow_storage_until_cross_state_loans_are_propagated() {
    let source = r#"
        data Main<'storage> {
            stored: &'storage mut i32;
        }

        machine Main::store(
            &mut self,
            source: &'storage mut i32
        ) {
            self.stored = source;
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("persistent borrow storage must fail closed");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("assignment stores a borrow-carrying value in persistent field `stored`"),
        "expected the persistent-loan fence, got:\n{combined}"
    );
}

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

#[test]
fn accepts_construction_seeded_persistent_field_source_in_persistent_storage() {
    // An attached-data field arrives holding the caller's 'storage-scoped
    // loan: reading it back supplies a persistent source for another
    // persistent store without any authored establishment.
    let source = r#"
        data Main<'storage> {
            view: &'storage mut [u8];
            alias: &'storage mut [u8];
        }

        machine Main::store(&mut self) {
            self.alias = self.view;
        }
    "#;

    check_program(source)
        .expect("a borrow read back from a persistent field is persistent-backed by construction");
}

#[test]
fn rejects_construction_seeded_source_after_a_may_write_frame() {
    // The construction seed is still invalidated normally: an opaque frame
    // that may write `view` retires its path even though the callee happened
    // to store program-static data — the caller cannot see that.
    let source = r#"
        data Main {
            view: &[u8];
            alias: &[u8];
        }

        machine Main::fill(&mut self) {
            self.view = "filled";
        }

        machine Main::store(&mut self) {
            self.fill();
            self.alias = self.view;
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("an opaque call frame drops the seeded persistent source");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("assignment stores a borrow-carrying value in persistent field `alias`")),
        "expected the persistent fence after a may-write frame, got:\n{}",
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
fn accepts_same_state_copy_from_static_persistent_storage() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
            code: i32;
        }

        data Main {
            first: Message;
            second: Message;
        }

        machine Main::store(&mut self) {
            self.first = Message { body: "program static", code: 7 };
            self.second = self.first;
        }
    "#;

    check_program(source).expect("the copy retains the established static provenance");
}

#[test]
fn accepts_cross_state_copy_from_static_persistent_storage() {
    let source = r#"
        data Main {
            first: &[u8];
            second: &[u8];
        }

        machine Main::store(&mut self) {
            self.first = "program static";
            transition { _ -> copy() }

            state copy(&mut self) {
                self.second = self.first;
            }
        }
    "#;

    check_program(source).expect("program-static persistent provenance crosses a graph-state edge");
}

#[test]
fn rejects_cross_state_static_provenance_missing_on_one_predecessor() {
    // Attached borrow fields enter construction-seeded: the caller populated
    // `first`, so `establish` does not need its own write to carry coverage
    // into `join`. `bypass` still loses it — the `scribble` frame may write
    // `first`, which retires the seeded path on that predecessor alone.
    let source = r#"
        data Main {
            first: &[u8];
            second: &[u8];
        }

        machine Main::scribble(&mut self) {
            self.first = "scribbled";
        }

        machine Main::store(&mut self, choose_static: bool) {
            transition choose_static {
                true -> establish()
                false -> bypass()
            }

            state establish(&mut self) {
                transition { _ -> join() }
            }

            state bypass(&mut self) {
                self.scribble();
                transition { _ -> join() }
            }

            state join(&mut self) {
                self.second = self.first;
            }
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("static provenance must hold on every predecessor");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("assignment stores a borrow-carrying value in persistent field `second`")),
        "expected the cross-state must-analysis fence, got:\n{}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn accepts_cross_state_static_aggregate_frontier_accumulation() {
    let source = r#"
        data Message [copy] {
            first: &[u8];
            second: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
        }

        machine Main::store(&mut self) {
            self.source.first = "first";
            transition { _ -> establish_second() }

            state establish_second(&mut self) {
                self.source.second = "second";
                transition { _ -> copy_complete() }
            }

            state copy_complete(&mut self) {
                self.copy = self.source;
            }
        }
    "#;

    check_program(source)
        .expect("each stable borrowed leaf crosses state edges into a complete frontier");
}

#[test]
fn accepts_cross_state_static_runtime_index_forwarded_through_state_parameter() {
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
            transition { _ -> copy_element(index) }

            state copy_element(&mut self, index: u64 [0..2]) {
                self.copy = self.messages[index];
            }
        }
    "#;

    check_program(source).expect(
        "an immutable runtime index forwarded unchanged to a state parameter retains identity",
    );
}

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
fn rejects_cross_state_static_runtime_index_from_mutable_local() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            messages: [Message; 2];
            copy: Message;
        }

        machine Main::store(&mut self) {
            let mut index: u64 [0..2] = 1;
            self.messages[index].body = "program static";
            transition { _ -> copy_element(index) }

            state copy_element(&mut self, index: u64 [0..2]) {
                self.copy = self.messages[index];
            }
        }
    "#;

    let diagnostics = check_program(source)
        .expect_err("a mutable local index cannot identify one persistent source across states");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("assignment stores a borrow-carrying value in persistent field `copy`")),
        "expected the mutable-index persistent fence, got:\n{}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
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
fn rejects_cross_state_static_runtime_index_through_computed_local_alias() {
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
            let forwarded: u64 [0..2] = index + 0;
            self.messages[index].body = "program static";
            transition { _ -> copy_element(forwarded) }

            state copy_element(&mut self, index: u64 [0..2]) {
                self.copy = self.messages[index];
            }
        }
    "#;

    let diagnostics = check_program(source)
        .expect_err("a computed copy needs equality proof beyond direct-name identity");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("assignment stores a borrow-carrying value in persistent field `copy`")),
        "expected the computed-alias persistent fence, got:\n{}",
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
fn accepts_static_persistent_copy_across_disjoint_call_frame() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: i32;
        }

        machine Main::touch_code(&mut self) {
            self.code = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("an exact disjoint call frame preserves static persistent provenance");
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
fn rejects_aggregate_copy_with_only_partial_static_leaf_coverage() {
    // Attached aggregates are construction-seeded leaf by leaf, so the fence
    // now needs coverage hidden another way: `retouch` may write `second`,
    // which retires exactly that leaf's seeded path. Re-establishing `first`
    // afterwards leaves `second` unproven — genuinely partial coverage.
    let source = r#"
        data Message [copy] {
            first: &[u8];
            second: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
        }

        machine Main::retouch(&mut self) {
            self.source.second = "rewritten";
        }

        machine Main::store(&mut self) {
            self.retouch();
            self.source.first = "program static";
            self.copy = self.source;
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("every borrowed source leaf needs static provenance");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("assignment stores a borrow-carrying value in persistent field `copy`")),
        "expected the incomplete aggregate-copy fence, got:\n{}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
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

/// An immutable local names the value its `let` bound, so naming the local and
/// writing the call directly must reach the same verdict. `pick` returns only
/// string literals, whose loans outlive every state, so the persistent store is
/// admitted either way.
#[test]
fn accepts_a_static_call_result_reaching_a_persistent_field_through_a_local() {
    let source = r#"
        data Main {
            out: &[u8];
        }

        machine Main::main(&mut self) {
            let picked: &[u8] = self.pick(true);
            self.out = picked;
        }

        machine Main::pick(&mut self, flag: bool) -> &[u8] {
            transition flag {
                true -> "Gate"
                false -> "Branch Room"
            }
        }
    "#;

    check_program(source)
        .expect("a local bound to a statically-sourced call carries that call's loans");
}

/// The control for the test above: only the multiplicity of the binding
/// differs. A `let mut` may be pointed at a state-local loan after this read,
/// so its current initializer does not describe what the field will hold and
/// the fence stays closed.
#[test]
fn rejects_a_mutable_local_reaching_a_persistent_field() {
    let source = r#"
        data Main {
            out: &[u8];
        }

        machine Main::main(&mut self) {
            let mut picked: &[u8] = self.pick(true);
            self.out = picked;
        }

        machine Main::pick(&mut self, flag: bool) -> &[u8] {
            transition flag {
                true -> "Gate"
                false -> "Branch Room"
            }
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("a mutable local may be repointed after this read");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("assignment stores a borrow-carrying value in persistent field `out`")),
        "expected the persistent-loan fence, got:\n{}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}
