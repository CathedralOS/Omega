use crate::tests::front_end::{checked_program, checked_program_result};

#[test]
fn nested_linear_record_extraction_stays_conservative_without_field_algebra() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        boundary requirement Receipt::ack(self);
        data Pair [linear] {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let pair: Pair = Pair { left: left, right: right };
            let extracted: Receipt = pair.left;
            Receipt::ack(extracted);
            0
        }
    "#;
    let diagnostics = checked_program_result(source)
        .expect_err("partial linear-record extraction needs per-field resource accounting");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("linear value `pair` reaches scope exit")
    }));
}

#[test]
fn nominal_drop_rejects_direct_partial_move() {
    let source = r#"
        data Leaf { value: i32; }
        data Wrapper { leaf: Leaf; }
        machine Wrapper::drop(&mut self) {}
        data Main {}
        machine Main::run() {
            let wrapper: Wrapper = Wrapper { leaf: Leaf { value: 1 } };
            let extracted: Leaf = wrapper.leaf;
        }
    "#;
    let diagnostics = checked_program_result(source)
        .expect_err("a nominal drop machine requires its whole valid receiver");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot partially move a value of `Wrapper`")
    }));
}

#[test]
fn nominal_drop_rejects_move_below_nested_prefix() {
    let source = r#"
        data Leaf { value: i32; }
        data Wrapper { leaf: Leaf; }
        machine Wrapper::drop(&mut self) {}
        data Outer { wrapper: Wrapper; }
        data Main {}
        machine Main::run() {
            let outer: Outer = Outer {
                wrapper: Wrapper { leaf: Leaf { value: 1 } },
            };
            let extracted: Leaf = outer.wrapper.leaf;
        }
    "#;
    let diagnostics = checked_program_result(source)
        .expect_err("every proper nominal-drop prefix retains whole-value entitlement");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot partially move a value of `Wrapper`")
    }));
}

#[test]
fn nominal_drop_rejects_move_below_generic_prefix() {
    let source = r#"
        data Leaf { value: i32; }
        data Wrapper { leaf: Leaf; }
        machine Wrapper::drop(&mut self) {}
        data Box<T> { value: T; }
        data Main {}
        machine Main::run() {
            let boxed: Box<Wrapper> = Box {
                value: Wrapper { leaf: Leaf { value: 1 } },
            };
            let extracted: Leaf = boxed.value.leaf;
        }
    "#;
    let diagnostics = checked_program_result(source)
        .expect_err("generic substitution preserves a nested nominal drop entitlement");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot partially move a value of `Wrapper`")
    }));
}

#[test]
fn nominal_drop_does_not_authorize_borrowed_self_extraction() {
    let source = r#"
        data Leaf { value: i32; }
        data Wrapper { leaf: Leaf; }
        machine Wrapper::drop(&mut self) {}
        machine Wrapper::take(&mut self) {
            let extracted: Leaf = self.leaf;
        }
    "#;
    let diagnostics = checked_program_result(source)
        .expect_err("a nominal drop hook does not grant ownership of borrowed contents");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot transfer a non-copy value out of borrowed storage")
    }));
}

#[test]
fn nominal_drop_allows_explicit_consuming_decomposition() {
    checked_program(
        r#"
        data Leaf { value: i32; }
        data Wrapper { leaf: Leaf; }
        machine Wrapper::drop(&mut self) {}
        machine Wrapper::into_leaf(self) -> Leaf { self.leaf }
        "#,
    );
}

#[test]
fn transparent_record_rejects_duplicate_field_move() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        boundary requirement Receipt::ack(self);
        data Pair {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let pair: Pair = Pair { left: left, right: right };
            let first: Receipt = pair.left;
            let duplicate: Receipt = pair.left;
            let remaining: Receipt = pair.right;
            Receipt::ack(first);
            Receipt::ack(duplicate);
            Receipt::ack(remaining);
            0
        }
    "#;
    let diagnostics =
        checked_program_result(source).expect_err("one field claim cannot move twice");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("linear value `pair.left` was already transferred")
    }));
}

#[test]
fn fixed_array_rejects_duplicate_literal_index_move() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        boundary requirement Receipt::ack(self);
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let receipts: [Receipt; 2] = [left, right];
            let first: Receipt = receipts[0];
            let duplicate: Receipt = receipts[0];
            Receipt::ack(first);
            Receipt::ack(duplicate);
            let second: Receipt = receipts[1];
            Receipt::ack(second);
            0
        }
    "#;
    let diagnostics =
        checked_program_result(source).expect_err("the same fixed element cannot move twice");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("receipts[0]")
                && diagnostic
                    .message
                    .contains("already transferred or consumed")
        }),
        "expected a duplicate fixed-index move diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn fixed_array_state_result_maps_claims_by_literal_index() {
    let checked = checked_program(
        r#"
        data Receipt [linear] { code: i32; }
        boundary requirement Receipt::ack(self);
        data Main {}
        machine Main::pack(left: Receipt, right: Receipt) -> [Receipt; 2] {
            [left, right]
        }
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let receipts: [Receipt; 2] = Main::pack(left, right);
            let first: Receipt = receipts[0];
            let second: Receipt = receipts[1];
            Receipt::ack(first);
            Receipt::ack(second);
            0
        }
        "#,
    );

    let pack_symbol = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .find(|state| state.name.as_str() == "pack")
        .map(|state| state.symbol)
        .expect("pack state");
    let map = checked
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .map(|(_, map)| map)
        .find(|map| map.state_symbol == pack_symbol)
        .expect("fixed-array outcome map");
    let entries = checked
        .facts
        .flow
        .ownership
        .claim_outcome_entries
        .span_or_empty(map.entries);
    assert_eq!(entries.len(), 2);
    for (expected, entry) in entries.iter().enumerate() {
        assert_eq!(
            checked
                .facts
                .flow
                .ownership
                .segments
                .span_or_empty(entry.output_segments),
            [facts::PlaceSegment::FixedIndex { index: expected }]
        );
    }
}

#[test]
fn transparent_record_sibling_assignment_transfers_the_source_claim() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        boundary requirement Receipt::ack(self);
        data Pair {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let pair: Pair = Pair { left: left, right: right };
            let old_left: Receipt = pair.left;
            Receipt::ack(old_left);
            pair.left = pair.right;
            let forwarded: Receipt = pair.left;
            Receipt::ack(forwarded);
            let duplicate: Receipt = pair.right;
            Receipt::ack(duplicate);
            0
        }
    "#;
    let diagnostics = checked_program_result(source)
        .expect_err("assigning from a sibling must transfer its claim");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("linear value `pair.right` was already transferred")
    }));
}
