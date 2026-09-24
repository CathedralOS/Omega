use crate::tests::front_end::{checked_program, checked_program_result};

#[test]
fn nested_linear_record_extraction_stays_conservative_without_field_algebra() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
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
fn transparent_record_frontier_preserves_independent_field_origins() {
    let checked = checked_program(
        r#"
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
            let forwarded: Pair = pair;
            let extracted_left: Receipt = forwarded.left;
            let extracted_right: Receipt = forwarded.right;
            Receipt::ack(extracted_left);
            Receipt::ack(extracted_right);
            0
        }
        "#,
    );

    use language_semantics::{PermissionAccess, PermissionEventKind};
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| event.access == PermissionAccess::Owned)
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 16);

    let pair_symbol = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .flat_map(|state| checked.statement_table.statements(state.statement_nodes))
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == "pair" =>
            {
                Some(local.symbol)
            }
            _ => None,
        })
        .expect("pair local");
    let pair_establishments = events
        .iter()
        .copied()
        .filter(|event| {
            event.kind == PermissionEventKind::Establish
                && event.root == facts::PlaceRoot::Symbol(pair_symbol)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        pair_establishments.len(),
        2,
        "transparent records establish one frontier entry per contained linear field"
    );
    assert!(pair_establishments.iter().all(|event| {
        checked
            .facts
            .flow
            .ownership
            .segments
            .span_or_empty(event.segments)
            .len()
            == 1
    }));
    assert_ne!(
        pair_establishments[0].provenance, pair_establishments[1].provenance,
        "constructing an aggregate must not collapse independent field lineages"
    );
    assert_ne!(
        pair_establishments[0].claim_identity, pair_establishments[1].claim_identity,
        "constructing an aggregate must not collapse independent field claims"
    );
    for establishment in pair_establishments {
        assert_eq!(
            events
                .iter()
                .filter(|event| event.provenance == establishment.provenance)
                .count(),
            8,
            "each source claim must map through its field and extracted local independently"
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.claim_identity == establishment.claim_identity)
                .count(),
            8,
            "each source claim identity must survive aggregate and local transfers"
        );
    }
}

#[test]
fn transparent_record_entry_claims_share_lineage_but_have_distinct_identities() {
    let checked = checked_program(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Pair {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::run() -> i32 { 0 }
        machine consume(pair: Pair) {
            let left: Receipt = pair.left;
            let right: Receipt = pair.right;
            Receipt::ack(left);
            Receipt::ack(right);
        }
        "#,
    );

    use language_semantics::{PermissionClaimIdentity, PermissionEventKind, PermissionEventSource};
    let entries = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.source == PermissionEventSource::StateEntry
                && event.kind == PermissionEventKind::Establish
        })
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].provenance, entries[1].provenance);
    assert_ne!(entries[0].claim_identity, entries[1].claim_identity);
    assert!(
        entries
            .iter()
            .all(|event| event.claim_identity != PermissionClaimIdentity::Unknown)
    );
}

#[test]
fn transparent_record_partial_move_leaves_sibling_obligation_live() {
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
            let extracted: Receipt = pair.left;
            Receipt::ack(extracted);
            0
        }
    "#;
    let diagnostics =
        checked_program_result(source).expect_err("the untouched sibling remains an obligation");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("linear value `pair.right` reaches scope exit")
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
fn nominal_drop_allows_whole_value_move() {
    checked_program(
        r#"
        data Leaf { value: i32; }
        data Wrapper { leaf: Leaf; }
        machine Wrapper::drop(&mut self) {}
        data Main {}
        machine Main::run() {
            let wrapper: Wrapper = Wrapper { leaf: Leaf { value: 1 } };
            let moved: Wrapper = wrapper;
        }
        "#,
    );
}

#[test]
fn nominal_drop_allows_moving_nested_value_whole() {
    checked_program(
        r#"
        data Leaf { value: i32; }
        data Wrapper { leaf: Leaf; }
        machine Wrapper::drop(&mut self) {}
        data Outer { wrapper: Wrapper; }
        data Main {}
        machine Main::run() {
            let outer: Outer = Outer {
                wrapper: Wrapper { leaf: Leaf { value: 1 } },
            };
            let moved: Wrapper = outer.wrapper;
        }
        "#,
    );
}

#[test]
fn nominal_drop_allows_copying_primitive_field() {
    checked_program(
        r#"
        data Wrapper { value: i32; }
        machine Wrapper::drop(&mut self) {}
        data Main {}
        machine Main::run() {
            let wrapper: Wrapper = Wrapper { value: 1 };
            let copied: i32 = wrapper.value;
        }
        "#,
    );
}

#[test]
fn nominal_drop_allows_owned_production_into_self_field() {
    checked_program(
        r#"
        data Leaf { value: i32; }
        data LeafFactory {}
        boundary operator LeafFactory::create() -> Leaf;
        data Wrapper { leaf: Leaf; }
        machine Wrapper::drop(&mut self) {}
        machine Wrapper::replace(&mut self) {
            self.leaf = LeafFactory::create();
        }
        "#,
    );
}

#[test]
fn transparent_affine_record_allows_partial_move() {
    checked_program(
        r#"
        data Leaf { value: i32; }
        data Pair { left: Leaf; right: Leaf; }
        data Main {}
        machine Main::run() {
            let pair: Pair = Pair {
                left: Leaf { value: 1 },
                right: Leaf { value: 2 },
            };
            let extracted: Leaf = pair.left;
        }
        "#,
    );
}

#[test]
fn transparent_record_rejects_duplicate_field_move() {
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
fn fixed_array_partial_move_leaves_sibling_obligation_live() {
    let checked = checked_program(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let receipts: [Receipt; 2] = [left, right];
            let first: Receipt = receipts[0];
            Receipt::ack(first);
            let second: Receipt = receipts[1];
            Receipt::ack(second);
            0
        }
        "#,
    );

    use language_semantics::{PermissionAccess, PermissionEventKind};
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| event.access == PermissionAccess::Owned)
        .collect::<Vec<_>>();
    let receipt_establishments = events
        .iter()
        .copied()
        .filter(|event| {
            event.kind == PermissionEventKind::Establish
                && event.obligation_live
                && matches!(
                    checked
                        .facts
                        .flow
                        .ownership
                        .segments
                        .span_or_empty(event.segments),
                    [facts::PlaceSegment::FixedIndex { .. }]
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(receipt_establishments.len(), 2);
    let indices = receipt_establishments
        .iter()
        .map(|event| {
            let [facts::PlaceSegment::FixedIndex { index }] = checked
                .facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments)
            else {
                unreachable!("fixed array claim path")
            };
            *index
        })
        .collect::<Vec<_>>();
    assert_eq!(indices, [0, 1]);
    assert_ne!(
        receipt_establishments[0].claim_identity,
        receipt_establishments[1].claim_identity
    );
}

#[test]
fn fixed_array_rejects_duplicate_literal_index_move() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
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

#[test]
fn nested_generic_transparent_record_retains_the_concrete_claim_path() {
    let checked = checked_program(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Box<T> { value: T; }
        data Envelope<T> { boxed: Box<T>; }
        data Main {}
        machine Main::run() -> i32 {
            let issued: Receipt = Receipt { code: 7 };
            let boxed: Box<Receipt> = Box { value: issued };
            let envelope: Envelope<Receipt> = Envelope { boxed: boxed };
            let extracted: Receipt = envelope.boxed.value;
            Receipt::ack(extracted);
            0
        }
        "#,
    );

    let envelope_symbol = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .flat_map(|state| checked.statement_table.statements(state.statement_nodes))
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == "envelope" =>
            {
                Some(local.symbol)
            }
            _ => None,
        })
        .expect("envelope local");
    let establishment = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .find(|event| {
            event.kind == language_semantics::PermissionEventKind::Establish
                && event.root == facts::PlaceRoot::Symbol(envelope_symbol)
        })
        .expect("nested generic frontier establishment");
    assert_eq!(
        checked
            .facts
            .flow
            .ownership
            .segments
            .span_or_empty(establishment.segments)
            .len(),
        2,
        "the concrete claim must remain nested under both generic record fields"
    );
}
