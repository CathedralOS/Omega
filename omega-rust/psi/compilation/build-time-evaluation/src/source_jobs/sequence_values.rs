//! Read complete sequence snapshots from checked source. Copying an Item keeps
//! its byte backing; affine sequences move intact through calls and borrows.

use crate::{
    BuildTimeValue, InvocationTarget, PreparedBuildMachineProgram, SourceCompletionAccounting,
};

const SOURCE: &str = r#"
data Item<'bytes> [copy] { number: u64; bytes: &'bytes [u8]; }
data Holder<'bytes> {
    tag: u64;
    rows: [Item<'bytes>];
    groups: [[Item<'bytes>]];
    empty: [Item<'bytes>];
    bytes: &'bytes [u8];
}
data Seen<'bytes> { holder: Holder<'bytes>; observed: u64; }
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
machine guarded_index(value: [Item]) -> u64 {
    transition value.len > 0 {
        true -> value[0].number
        _ -> 0
    }
}
"#;

fn target() -> InvocationTarget<'static> {
    // The existing explicit pure invocation fixture profile, not host inference
    // or native execution. No layout/provider context is supplied by this test.
    InvocationTarget {
        name: Some("linux_x86_64"),
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
    assert_eq!(
        error,
        "cannot prove operation safety in machine `forbidden_index` state `entry` return value. \
         (no proof found from the facts available here: the bounds do not decide the comparison)"
    );
    assert_eq!(
        accounting.receipts().len(),
        before,
        "no execution without a proved bounds judgment"
    );
}

#[test]
fn owned_source_sequence_reads_its_copyable_item_under_its_live_length_guard() {
    let prepared = prepared();
    let mut accounting = SourceCompletionAccounting::default();
    let made = entry(&prepared, "make", &[])
        .expect("same actual requester and source entry")
        .invoke_recorded(target(), &mut accounting)
        .expect("actual complete source value");
    for (name, number) in [("rows", 11), ("empty", 0)] {
        let result = entry(
            &prepared,
            "guarded_index",
            &[field(made.value(), name).clone()],
        )
        .expect("same checked owned sequence argument")
        .invoke_recorded(target(), &mut accounting)
        .expect("the bound is proved from this sequence's length");
        assert_eq!(result.value(), &BuildTimeValue::Int(number));
    }
    accounting
        .validate_charges()
        .expect("all actual invocations accounted");
}

const SHARED: &str = r#"
data Item<'bytes> [copy] { number: u64; bytes: &'bytes [u8]; }
machine echo<'items, 'bytes>(items: &'items [Item<'bytes>]) -> &'items [Item<'bytes>] { items }
machine consume<'items, 'bytes>(items: &'items [Item<'bytes>]) { }
"#;

/// A shared typed view is its descriptor and immutable backing, not a loan of
/// the descriptor's containing record. Exercise that actual argument branch.
#[test]
fn shared_typed_sequence_arguments_keep_complete_backing_and_refuse_mutable_fabrication() {
    let prepared = PreparedBuildMachineProgram::prepare(&super::tests::typed(SHARED), None)
        .expect("real shared typed-view source");
    let typed = prepared.typed();
    let echo = typed
        .realized_machine_named("echo")
        .expect("actual source echo");
    let Some([state]) = typed.machine_states(echo).as_array() else {
        panic!("exact entry state")
    };
    let Some([formal]) = typed.state_parameters(state).as_array() else {
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
    let mut foreign = item(11, b"first");
    let BuildTimeValue::Struct { type_name, .. } = &mut foreign else {
        unreachable!()
    };
    *type_name = "AnotherItem".into();
    let error = entry(&prepared, "echo", &[BuildTimeValue::Array(vec![foreign])])
        .err()
        .expect("host backing retains its nominal Item declaration");
    assert!(
        error.contains("record names another declaration"),
        "{error}"
    );
    let mutable = SHARED.replace("&'items [Item<'bytes>]", "&'items mut [Item<'bytes>]");
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
    let error = entry(&mutable, "echo", &[BuildTimeValue::Array(Vec::new())])
        .err()
        .expect("an empty immutable descriptor grants no exclusive loan");
    assert!(
        error.contains("mutable typed sequence requires its actual exclusive backing and loan"),
        "{error}"
    );
}

#[test]
fn shared_typed_sequence_inputs_retain_nested_sequence_descriptors() {
    let source = format!(
        "{SHARED}\nmachine echo_groups<'groups, 'bytes>(groups: &'groups [[Item<'bytes>]]) -> &'groups [[Item<'bytes>]] {{ groups }}\n"
    );
    let prepared = PreparedBuildMachineProgram::prepare(&super::tests::typed(&source), None)
        .expect("nested immutable view source");
    let value = BuildTimeValue::Array(vec![
        BuildTimeValue::Array(Vec::new()),
        BuildTimeValue::Array(Vec::new()),
    ]);
    let mut accounting = SourceCompletionAccounting::default();
    let result = entry(&prepared, "echo_groups", std::slice::from_ref(&value))
        .expect("ordinary nested view call")
        .invoke_recorded(target(), &mut accounting)
        .expect("both inner empty descriptors retain backing");
    assert_eq!(result.value(), &value);
    accounting
        .validate_charges()
        .expect("nested descriptors are accounted");
}

#[test]
fn shared_typed_sequence_input_boundary_refuses_incomplete_and_duplicate_contents() {
    use terminal_interpreter::{
        TerminalInstantiation, TerminalProgram, TerminalStorageContents, TerminalStructuralInputs,
        TerminalStructuralValue,
    };
    let prepared =
        PreparedBuildMachineProgram::prepare(&super::tests::typed(SHARED), None).unwrap();
    let value = BuildTimeValue::Array(vec![BuildTimeValue::Struct {
        type_name: "Item".into(),
        fields: vec![
            ("number".into(), BuildTimeValue::Int(11)),
            ("bytes".into(), BuildTimeValue::Text(b"first".to_vec())),
        ],
    }]);
    let entry = entry(&prepared, "echo", std::slice::from_ref(&value)).unwrap();
    let produced = entry.produce_invocation().unwrap();
    let program = TerminalProgram::borrow(produced.view());
    let parameters = &program.entry().unwrap().structural_parameters;
    let [parameter] = parameters else {
        panic!("actual host backing parameter")
    };
    let arguments = [TerminalStructuralValue::minted(
        1,
        parameter.structural_type,
        Vec::new(),
    )];
    let complete =
        crate::terminal_values::input_contents(&program, 0, parameter.structural_type, &value)
            .unwrap();
    program
        .instantiate(TerminalInstantiation {
            structural_inputs: TerminalStructuralInputs {
                arguments: &arguments,
                contents: std::slice::from_ref(&complete),
                ..Default::default()
            },
            ..Default::default()
        })
        .expect("complete immutable input boundary");
    for duplicate in [false, true] {
        let mut incomplete = complete.clone();
        let TerminalStorageContents::Elements { values, .. } = &mut incomplete.values[0].contents
        else {
            panic!("explicit sequence descriptor")
        };
        let mut leaves = values.to_vec();
        if duplicate {
            leaves.insert(0, leaves[0].clone());
        } else {
            leaves.pop();
        }
        *values = leaves.into();
        let rejected = program.instantiate(TerminalInstantiation {
            structural_inputs: TerminalStructuralInputs {
                arguments: &arguments,
                contents: std::slice::from_ref(&incomplete),
                ..Default::default()
            },
            ..Default::default()
        });
        assert!(
            rejected.is_err(),
            "host sequence contents must be complete and unique"
        );
    }
}

#[test]
fn shared_typed_sequence_inputs_in_owned_records_and_cases_keep_applied_lifetimes() {
    let source = format!(
        "{SHARED}\n\
         data Envelope<'bytes, 'items> {{ tag: u64; rows: &'items [Item<'bytes>]; }}\n\
         data Choice<'bytes, 'items> {{ case Rows(value: Envelope<'bytes, 'items>); }}\n\
         machine echo_record<'items, 'bytes>(value: Envelope<'bytes, 'items>) -> Envelope<'bytes, 'items> {{ value }}\n\
         machine echo_case<'items, 'bytes>(value: Choice<'bytes, 'items>) -> Choice<'bytes, 'items> {{ value }}\n"
    );
    let prepared = PreparedBuildMachineProgram::prepare(&super::tests::typed(&source), None)
        .expect("owned envelopes retain distinct enclosing and element regions");
    let mut accounting = SourceCompletionAccounting::default();
    for empty in [false, true] {
        let rows = if empty {
            Vec::new()
        } else {
            vec![BuildTimeValue::Struct {
                type_name: "Item".into(),
                fields: vec![
                    ("number".into(), BuildTimeValue::Int(29)),
                    ("bytes".into(), BuildTimeValue::Text(vec![0, 255, b'x'])),
                ],
            }]
        };
        let envelope = BuildTimeValue::Struct {
            type_name: "Envelope".into(),
            fields: vec![
                ("tag".into(), BuildTimeValue::Int(7)),
                ("rows".into(), BuildTimeValue::Array(rows)),
            ],
        };
        for case in [false, true] {
            let (name, value) = if case {
                (
                    "echo_case",
                    BuildTimeValue::Case {
                        variant: "Rows".into(),
                        payload: vec![("value".into(), envelope.clone())],
                    },
                )
            } else {
                ("echo_record", envelope.clone())
            };
            let returned = entry(&prepared, name, &[value])
                .expect("nested view receives the applied data lifetime tuple")
                .invoke_recorded(target(), &mut accounting)
                .expect("owned record/case transports actual host backing");
            let record = if case {
                let BuildTimeValue::Case { payload, .. } = returned.value() else {
                    panic!("the active case is retained")
                };
                assert_eq!(payload.len(), 1);
                &payload[0].1
            } else {
                returned.value()
            };
            assert_eq!(field(record, "tag"), &BuildTimeValue::Int(7));
            let rows = array(field(record, "rows"));
            assert_eq!(rows.len(), usize::from(!empty));
            if !empty {
                assert_eq!(field(&rows[0], "number"), &BuildTimeValue::Int(29));
                assert_eq!(
                    field(&rows[0], "bytes"),
                    &BuildTimeValue::Text(vec![0, 255, b'x'])
                );
            }
            let echoed = entry(&prepared, name, std::slice::from_ref(returned.value()))
                .expect("returned nominal identities rejoin the same formal")
                .invoke_recorded(target(), &mut accounting)
                .expect("a separate invocation retains all nested backing");
            assert_eq!(echoed.value(), returned.value());
        }
        let mutable = source.replace(
            "rows: &'items [Item<'bytes>]",
            "rows: &'items mut [Item<'bytes>]",
        );
        let mutable = PreparedBuildMachineProgram::prepare(&super::tests::typed(&mutable), None)
            .expect("retain the owned envelope's exclusive twin");
        let error = entry(&mutable, "echo_record", &[envelope])
            .err()
            .expect("immutable contents cannot create an exclusive nested view");
        assert!(
            error.contains("mutable typed sequence requires its actual exclusive backing and loan"),
            "{error}"
        );
    }
    accounting
        .validate_charges()
        .expect("every nested invocation is accounted");
}
