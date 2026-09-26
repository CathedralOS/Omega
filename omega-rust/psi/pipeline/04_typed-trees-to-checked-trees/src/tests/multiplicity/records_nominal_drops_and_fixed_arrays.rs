use crate::tests::front_end::{checked_program, checked_program_result};

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
fn fixed_array_state_result_maps_claims_by_literal_index() {
    let checked = checked_program(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
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
        machine Receipt::ack(self) {}
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
