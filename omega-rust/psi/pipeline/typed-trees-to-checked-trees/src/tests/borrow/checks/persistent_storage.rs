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
fn accepts_program_static_literal_in_persistent_borrow_storage() {
    let source = r#"
        data Main {
            stored: &[u8];
        }

        machine Main::store(&mut self) {
            self.stored = "program static";
        }
    "#;

    check_program(source).expect("a literal view needs no state-local source loan");
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
fn accepts_nested_program_static_literal_in_persistent_aggregate_storage() {
    let source = r#"
        data Message {
            body: &[u8];
            code: i32;
        }

        data Main {
            stored: Message;
        }

        machine Main::store(&mut self) {
            self.stored = Message { body: "program static", code: 7 };
        }
    "#;

    check_program(source).expect("only the aggregate's borrow-carrying field needs classification");
}

#[test]
fn accepts_static_view_call_result_in_persistent_storage() {
    let source = r#"
        data Main {
            stored: &[u8];
        }

        machine Main::pick(&self, first: bool) -> &[u8] {
            transition first {
                true -> "first"
                false -> "second"
            }
        }

        machine Main::store(&mut self) {
            self.stored = self.pick(true);
        }
    "#;

    check_program(source).expect("every value exit of pick is program-static storage");
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
    let source = r#"
        data Main {
            first: &[u8];
            second: &[u8];
        }

        machine Main::store(&mut self, choose_static: bool) {
            transition choose_static {
                true -> establish()
                false -> bypass()
            }

            state establish(&mut self) {
                self.first = "program static";
                transition { _ -> join() }
            }

            state bypass(&mut self) {
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
fn accepts_cross_state_static_fixed_index_copy() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            messages: [Message; 2];
            copy: Message;
        }

        machine Main::store(&mut self) {
            self.messages[1].body = "program static";
            transition { _ -> copy_element() }

            state copy_element(&mut self) {
                self.copy = self.messages[1];
            }
        }
    "#;

    check_program(source)
        .expect("a literal fixed-index path retains static provenance across states");
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
fn accepts_cross_state_static_runtime_index_through_immutable_local_alias() {
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
            let forwarded: u64 [0..2] = index;
            self.messages[index].body = "program static";
            transition { _ -> copy_element(forwarded) }

            state copy_element(&mut self, index: u64 [0..2]) {
                self.copy = self.messages[index];
            }
        }
    "#;

    check_program(source).expect(
        "a direct immutable local-copy alias retains the runtime index identity across states",
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
fn accepts_cross_state_static_leaf_across_disjoint_scalar_mutation() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
            code: i32;
        }

        data Main {
            source: Message;
            copy: Message;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            transition { _ -> mutate_scalar() }

            state mutate_scalar(&mut self) {
                self.source.code = 7;
                transition { _ -> copy_complete() }
            }

            state copy_complete(&mut self) {
                self.copy = self.source;
            }
        }
    "#;

    check_program(source)
        .expect("a disjoint scalar mutation does not invalidate the static borrowed leaf");
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
fn accepts_static_persistent_copy_across_disjoint_cyclic_alias_frame() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: i32;
        }

        machine identity(value: &mut i32) -> &mut i32 {
            value
        }

        machine Main::touch_code_cycle(&mut self) {
            let alias: &mut i32 = &mut self.code;
            transition { _ -> cycle(alias) }

            state cycle(&mut self, value: &mut i32) {
                value = 7;
                transition { _ -> cycle(identity(value)) }
            }
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code_cycle();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("a transparent helper preserves the exact cyclic alias permutation");
}

#[test]
fn accepts_static_persistent_copy_across_attached_transparent_result_frame() {
    // The shared receiver is a call operand like any other: since 12c70b7e72 a
    // `&self` receiver overlapping an exclusive `&mut self.code` argument in
    // the same call is rejected as interference. The attached helper therefore
    // lives on a disjoint field so its transparent result still comes from the
    // explicit argument alone.
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Forwarder {
            marker: u8;
        }

        data Main {
            source: Message;
            copy: Message;
            forwarder: Forwarder;
            code: i32;
        }

        machine Forwarder::forward_alias(&self, value: &mut i32) -> &mut i32 {
            value
        }

        machine Main::touch_code(&mut self) {
            let alias: &mut i32 = self.forwarder.forward_alias(&mut self.code);
            alias = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("an attached transparent result preserves its explicit argument's disjoint frame");
}

#[test]
fn accepts_static_persistent_copy_across_local_alias_helper_result_frame() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: i32;
        }

        machine identity(value: &mut i32) -> &mut i32 {
            value
        }

        machine forward_alias(value: &mut i32) -> &mut i32 {
            let first: &mut i32 = identity(value);
            let second: &mut i32 = &mut first;
            second
        }

        machine write(value: &mut i32) {
            value = 7;
        }

        machine Main::touch_code(&mut self) {
            write(forward_alias(&mut self.code));
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("a transparent helper result supplied directly to a statement call stays exact");
}

#[test]
fn accepts_static_persistent_copy_across_local_index_helper_result_frame() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: [u64; 2];
        }

        machine return_local_index(cells: &mut [u64; 2]) -> &mut u64 {
            let index: u64 = 0;
            &mut cells[index]
        }

        machine Main::touch_code(&mut self) {
            let alias: &mut u64 = return_local_index(&mut self.code);
            alias = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("an effect-free local-index helper result preserves its collection frame");
}

#[test]
fn accepts_static_persistent_copy_across_local_index_alias_frame() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: [u64; 2];
        }

        machine Main::touch_code(&mut self) {
            let index: u64 = 0;
            let alias: &mut u64 = &mut self.code[index];
            alias = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source).expect("an effect-free local-index alias preserves its collection frame");
}

#[test]
fn accepts_static_persistent_copy_across_mutable_slice_view_frame() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: [u64; 2];
        }

        machine return_slice(cells: &mut [u64; 2]) -> &mut [u64] {
            let view: &mut [u64] = cells.as_mut_slice();
            view
        }

        machine Main::touch_code(&mut self) {
            let view: &mut [u64] = return_slice(&mut self.code);
            transition view.len > 0 {
                true -> write(view)
                false -> {}
            }

            state write(&mut self, view: &mut [u64]) {
                view[0] = 7;
            }
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("a mutable slice view preserves its backing array's disjoint frame");
}

#[test]
fn accepts_static_persistent_copy_across_mutable_slice_statement_argument_frame() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: [u64; 2];
        }

        machine write_slice(view: &mut [u64]) {
            transition view.len > 0 {
                true -> write(view)
                false -> {}
            }

            state write(view: &mut [u64]) {
                view[0] = 7;
            }
        }

        machine Main::touch_code(&mut self) {
            write_slice(self.code.as_mut_slice());
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("a direct mutable-slice statement argument preserves its backing array frame");
}

#[test]
fn accepts_static_persistent_copy_after_discarded_slice_view_expression() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: u64;
            cells: [u64; 2];
        }

        machine return_after_slice_length<'value, 'cells>(
            value: &'value mut u64,
            cells: &'cells mut [u64; 2]
        ) -> &'value mut u64 {
            cells.as_mut_slice().len;
            value
        }

        machine Main::touch_code(&mut self) {
            let alias: &mut u64 =
                return_after_slice_length(&mut self.code, &mut self.cells);
            alias = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("a discarded slice-view length read does not obscure a disjoint exact frame");
}

#[test]
fn accepts_static_persistent_copy_after_discarded_shared_slice_view_expression() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: u64;
            cells: [u64; 2];
        }

        machine return_after_shared_slice_length<'value, 'cells>(
            value: &'value mut u64,
            cells: &'cells [u64; 2]
        ) -> &'value mut u64 {
            cells.as_slice().len;
            value
        }

        machine Main::touch_code(&mut self) {
            let alias: &mut u64 =
                return_after_shared_slice_length(&mut self.code, self.cells);
            alias = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("a discarded shared-slice length read does not obscure a disjoint exact frame");
}

#[test]
fn accepts_static_persistent_copy_across_recast_local_frame() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: u64;
        }

        machine recast_write_then_return(value: &mut u64) -> &mut u64 {
            let view: &mut f64 = &mut value as &mut f64;
            view = 3.0;
            value
        }

        machine Main::touch_code(&mut self) {
            let alias: &mut u64 = recast_write_then_return(&mut self.code);
            alias = 4;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("a recast write does not obscure a helper's exact returned parameter origin");
}

#[test]
fn accepts_static_persistent_copy_across_value_write_helper_result_frame() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: i32;
        }

        machine identity(value: &mut i32) -> &mut i32 {
            value
        }

        machine write_then_return(value: &mut i32) -> &mut i32 {
            identity(value) = 7;
            value
        }

        machine Main::touch_code(&mut self) {
            let alias: &mut i32 = write_then_return(&mut self.code);
            alias = 8;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("a transparent call-produced assignment target preserves the helper result origin");
}

#[test]
fn accepts_static_persistent_copy_across_isolated_scratch_helper_frame() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: i32;
        }

        machine return_with_scratch(value: &mut i32) -> &mut i32 {
            let mut scratch: [i32; 2] = [0, 1];
            scratch[0] = 2;
            value
        }

        machine Main::touch_code(&mut self) {
            let alias: &mut i32 = return_with_scratch(&mut self.code);
            alias = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source).expect(
        "a reference-free scratch local cannot alter a helper's exact returned-alias origin",
    );
}

#[test]
fn accepts_static_persistent_copy_across_rebound_helper_result_frame() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            source: Message;
            spare: Message;
            copy: Message;
        }

        machine identity_message<'source>(
            value: &'source mut Message
        ) -> &'source mut Message {
            value
        }

        machine choose_second<'first, 'second>(
            first: &'first mut Message,
            second: &'second mut Message
        ) -> &'second mut Message {
            let mut selected: &mut Message = &mut first;
            selected = identity_message(second);
            selected
        }

        machine Main::touch_spare(&mut self) {
            let selected: &mut Message =
                choose_second(&mut self.source, &mut self.spare);
            selected.body = "spare static";
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_spare();
            self.copy = self.source;
        }
    "#;

    check_program(source).expect(
        "a transparent call-produced rebind selects only the replacement argument's write frame",
    );
}

#[test]
fn accepts_static_persistent_copy_across_pure_expression_helper_frame() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: i32;
        }

        machine return_after_read(value: &mut i32) -> &mut i32 {
            value == value;
            value
        }

        machine Main::touch_code(&mut self) {
            let alias: &mut i32 = return_after_read(&mut self.code);
            alias = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("an effect-free discarded expression cannot alter the returned alias origin");
}

#[test]
fn accepts_static_persistent_copy_across_receiver_result_frame() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: i32;
        }

        machine Main::code_alias(&mut self) -> &mut i32 {
            &mut self.code
        }

        machine Main::touch_code(&mut self) {
            let alias: &mut i32 = self.code_alias();
            alias = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("an exact receiver-rooted result preserves its disjoint caller frame");
}

#[test]
fn accepts_static_persistent_copy_across_isolated_record_local_frame() {
    let source = r#"
        data Cell {
            value: i32;
        }

        data Message [copy] {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
        }

        machine Main::touch_local_record(&mut self) {
            let local: [Cell; 2] = [Cell { value: 0 }, Cell { value: 1 }];
            let alias: &mut i32 = &mut local[0].value;
            alias = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_local_record();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("a primitive-only record local contributes no caller-visible write frame");
}

#[test]
fn accepts_static_persistent_copy_across_stable_rebound_alias_frame() {
    let source = r#"
        data Message [copy] {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            first: i32;
            second: i32;
        }

        machine Main::touch_second(&mut self) {
            let alias: &mut i32 = &mut self.first;
            alias = &mut self.second;
            alias = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_second();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("a direct stable alias replacement publishes its replacement origin's frame");
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

/// The same replacement rule applies to a direct reference local: its old
/// source is released and its new source remains borrowed through later use.
#[test]
fn accepts_precise_reference_local_reassignment() {
    let source = r#"
        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise<'source>(
            first: &'source mut i32,
            second: &'source mut i32
        ) {
            let mut selected: &'source mut i32 = first;
            selected = second;
            write(first);
            write(selected);
        }
    "#;

    check_program(source)
        .expect("reference replacement should release the old source and retain the new source");
}
