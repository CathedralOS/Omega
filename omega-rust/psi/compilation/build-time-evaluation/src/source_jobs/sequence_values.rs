//! Read complete sequence snapshots from checked source. Copying an Item keeps
//! its byte backing; affine sequences move intact through calls and borrows.

use crate::{
    BuildTimeValue, InvocationTarget, PreparedBuildMachineProgram, SourceCompletionAccounting,
};

const SOURCE: &str = r#"
data Item [copy] { number: u64; bytes: &[u8]; }
data Holder { tag: u64; rows: [Item]; groups: [[Item]]; empty: [Item]; bytes: &[u8]; }
data Seen { holder: Holder; observed: u64; }
machine make() -> Holder {
    let first: Item = Item { number: 11, bytes: "first" };
    Holder {
        tag: 7,
        rows: [first, Item { number: 29, bytes: "second" }],
        groups: [[first], []], empty: [], bytes: "carrier"
    }
}
machine transport(value: Holder) -> Holder { value }
machine inspect(value: &Holder) -> u64 { value.tag }
machine borrow_roundtrip(value: Holder) -> Seen {
    let observed: u64 = inspect(&value);
    Seen { holder: value, observed: observed }
}
machine zero() -> Holder { let value: Holder; value }
machine forbidden_index(value: [Item]) -> u64 { value[0].number }
"#;

fn target() -> InvocationTarget<'static> {
    // The existing explicit pure invocation fixture profile, not host inference
    // or native execution. No layout/provider context is supplied by this test.
    InvocationTarget {
        name: "linux_x86_64",
        layout_target: None,
        carrier_layout: None,
        address_model: None,
    }
}

fn field<'a>(value: &'a BuildTimeValue, name: &str) -> &'a BuildTimeValue {
    let BuildTimeValue::Struct { fields, .. } = value else {
        panic!("complete returned record")
    };
    let mut matching = fields.iter().filter(|(field, _)| field == name);
    let value = &matching.next().expect("declared field").1;
    assert!(matching.next().is_none());
    value
}

fn array(value: &BuildTimeValue) -> &[BuildTimeValue] {
    let BuildTimeValue::Array(values) = value else {
        panic!("complete actual sequence")
    };
    values
}

fn prepared() -> PreparedBuildMachineProgram {
    PreparedBuildMachineProgram::prepare(&super::tests::typed(SOURCE), None)
        .expect("retain actual source")
}

fn entry(
    prepared: &PreparedBuildMachineProgram,
    name: &str,
    arguments: &[BuildTimeValue],
) -> Result<super::PreparedBuildMachineEntry, String> {
    let symbol = prepared
        .typed()
        .realized_machine_named(name)
        .expect("actual source declaration")
        .symbol;
    prepared.entry(symbol, symbol, arguments)
}

#[test]
fn source_sequences_keep_all_records_nested_backing_and_borrowed_transport() {
    let prepared = prepared();
    let mut accounting = SourceCompletionAccounting::default();
    let made = entry(&prepared, "make", &[])
        .expect("same actual requester and source entry")
        .invoke_recorded(target(), &mut accounting)
        .expect("actual sequence construction");
    let holder = made.value();
    let rows = array(field(holder, "rows"));
    assert_eq!(rows.len(), 2);
    assert_eq!(field(&rows[0], "number"), &BuildTimeValue::Int(11));
    assert_eq!(
        field(&rows[0], "bytes"),
        &BuildTimeValue::Text(b"first".to_vec())
    );
    assert_eq!(field(&rows[1], "number"), &BuildTimeValue::Int(29));
    assert_eq!(
        field(&rows[1], "bytes"),
        &BuildTimeValue::Text(b"second".to_vec())
    );
    let groups = array(field(holder, "groups"));
    assert_eq!(groups.len(), 2);
    assert_eq!(
        array(&groups[0]),
        &rows[..1],
        "the copied Item keeps its complete contents"
    );
    assert!(array(&groups[1]).is_empty());
    assert!(array(field(holder, "empty")).is_empty());
    assert_eq!(
        field(holder, "bytes"),
        &BuildTimeValue::Text(b"carrier".to_vec())
    );
    for _ in 0..2 {
        // Each invocation receives a fresh owned argument graph. This copies
        // the host snapshot, never granting Omega copy permission to [Item].
        let moved = entry(&prepared, "transport", std::slice::from_ref(holder))
            .expect("same checked transport actual")
            .invoke_recorded(target(), &mut accounting)
            .expect("whole affine sequence transport");
        assert_eq!(moved.value(), holder);
        let borrowed = entry(
            &prepared,
            "borrow_roundtrip",
            std::slice::from_ref(moved.value()),
        )
        .expect("same checked borrowed actual")
        .invoke_recorded(target(), &mut accounting)
        .expect("whole carrier survives its actual shared loan");
        assert_eq!(field(borrowed.value(), "observed"), &BuildTimeValue::Int(7));
        assert_eq!(field(borrowed.value(), "holder"), holder);
    }
    accounting
        .validate_charges()
        .expect("same invocation charges and complete results");
}

#[test]
fn source_sequence_zero_is_empty_backing_and_preserves_byte_zero() {
    let prepared = prepared();
    let mut accounting = SourceCompletionAccounting::default();
    let zero = entry(&prepared, "zero", &[])
        .expect("same actual requester and source entry")
        .invoke_recorded(target(), &mut accounting)
        .expect("actual zero descriptors");
    assert_eq!(field(zero.value(), "tag"), &BuildTimeValue::Int(0));
    for name in ["rows", "groups", "empty"] {
        assert!(array(field(zero.value(), name)).is_empty());
    }
    assert_eq!(
        field(zero.value(), "bytes"),
        &BuildTimeValue::Text(Vec::new())
    );
    let returned = entry(&prepared, "transport", std::slice::from_ref(zero.value()))
        .expect("same checked empty actual")
        .invoke_recorded(target(), &mut accounting)
        .expect("empty still has an established descriptor after a whole move");
    assert_eq!(returned.value(), zero.value());
    accounting
        .validate_charges()
        .expect("actual empty backing accounting");
}

#[test]
fn source_sequence_inputs_keep_element_identity_and_literal_indices_need_live_bounds() {
    let prepared = prepared();
    let mut accounting = SourceCompletionAccounting::default();
    let made = entry(&prepared, "make", &[])
        .expect("same actual requester and source entry")
        .invoke_recorded(target(), &mut accounting)
        .expect("actual complete source value");
    let mut wrong = made.value().clone();
    let BuildTimeValue::Struct { fields, .. } = &mut wrong else {
        unreachable!()
    };
    let (_, BuildTimeValue::Array(rows)) =
        fields.iter_mut().find(|(name, _)| name == "rows").unwrap()
    else {
        unreachable!()
    };
    let BuildTimeValue::Struct { type_name, .. } = &mut rows[0] else {
        unreachable!()
    };
    *type_name = "SameShapeButAnotherDeclaration".into();
    let before = accounting.receipts().len();
    let error = entry(&prepared, "transport", &[wrong])
        .and_then(|entry| entry.invoke_recorded(target(), &mut accounting))
        .expect_err("an element's shape does not supply its declaration identity");
    assert!(
        error.contains("record names another declaration"),
        "{error}"
    );
    assert_eq!(
        accounting.receipts().len(),
        before,
        "no execution for a malformed element"
    );
    let error = entry(
        &prepared,
        "forbidden_index",
        &[field(made.value(), "rows").clone()],
    )
    .and_then(|entry| entry.invoke_recorded(target(), &mut accounting))
    .expect_err("a literal index still needs the same view's live length and bounds judgment");
    assert!(
        error.contains("sequence index needs its current length and bounds judgment"),
        "{error}"
    );
}

/// A shared typed view is its descriptor and immutable backing, not a loan of
/// the descriptor's containing record. Exercise that actual argument branch.
#[test]
fn shared_typed_sequence_arguments_keep_complete_backing_and_refuse_mutable_fabrication() {
    const SHARED: &str = r#"
data Item [copy] { number: u64; bytes: &[u8]; }
machine echo(items: &[Item]) -> &[Item] { items }
machine consume(items: &[Item]) { }
"#;
    let prepared = PreparedBuildMachineProgram::prepare(&super::tests::typed(SHARED), None)
        .expect("real shared typed-view source");
    let typed = prepared.typed();
    let echo = typed
        .realized_machine_named("echo")
        .expect("actual source echo");
    let [state] = typed.machine_states(echo) else {
        panic!("exact entry state")
    };
    let [formal] = typed.state_parameters(state) else {
        panic!("exact shared formal")
    };
    let typed_trees::types::TypeReferenceNode::Reference { referee, .. } = typed
        .type_reference_table
        .type_reference(formal.type_reference)
    else {
        panic!("actual shared view reference")
    };
    let typed_trees::types::TypeReferenceNode::Slice { element_type } =
        typed.type_reference_table.type_reference(*referee)
    else {
        panic!("actual sequence element declaration")
    };
    let element_identity = typed
        .type_identity_text(typed.normalized_type_identity(*element_type))
        .expect("canonical Item identity from the actual formal");
    let item = |number, bytes: &[u8]| BuildTimeValue::Struct {
        type_name: element_identity.clone(),
        fields: vec![
            ("number".into(), BuildTimeValue::Int(number)),
            ("bytes".into(), BuildTimeValue::Text(bytes.to_vec())),
        ],
    };
    let elements = BuildTimeValue::Array(vec![item(11, b"first"), item(29, b"second")]);
    let mut accounting = SourceCompletionAccounting::default();
    for value in [elements, BuildTimeValue::Array(Vec::new())] {
        let returned = entry(&prepared, "echo", std::slice::from_ref(&value))
            .expect("checked shared typed-view actual")
            .invoke_recorded(target(), &mut accounting)
            .expect("complete shared sequence result");
        assert_eq!(returned.value(), &value);
        entry(&prepared, "consume", std::slice::from_ref(returned.value()))
            .expect("returned backing is materialized in a separate checked caller")
            .invoke_recorded(target(), &mut accounting)
            .expect("shared typed-view consumer");
    }
    accounting
        .validate_charges()
        .expect("all actual invocations accounted");
    let mutable = SHARED.replace("&[Item]", "&mut [Item]");
    let mutable = PreparedBuildMachineProgram::prepare(&super::tests::typed(&mutable), None)
        .expect("retain the exact mutable twin for checking");
    let error = entry(
        &mutable,
        "echo",
        &[BuildTimeValue::Array(vec![
            item(11, b"first"),
            item(29, b"second"),
        ])],
    )
    .err()
    .expect("immutable host values cannot supply exclusive typed backing");
    assert!(
        error.contains("mutable typed sequence requires its actual exclusive backing and loan"),
        "{error}"
    );
}
