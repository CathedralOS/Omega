use typed_trees::{TypedTrees, data::DataMember, data_applications::{prepare, settle_const_argument}, types::{FixedArrayLength, TypeReferenceNode}};

#[test]
fn a_computed_extent_rebuilds_dependent_payloads_at_their_original_handles() {
    let text = r#"
data Buffer<T, const N: u64> { slots: [T; N]; }
data Envelope<T> { case Value(payload: T); }
data Holder { value: Envelope<Buffer<u8, capacity()>>; }
machine capacity() -> u64 { 4 }
"#;
    let resolved = crate::resolve_fixture(text);
    let mut program = crate::lower_symbol_resolved_trees(&resolved).unwrap();
    let holder = program
        .data_definitions()
        .iter()
        .find(|row| row.name.as_str() == "Holder")
        .unwrap();
    let DataMember::Field(field) = &program.data_members(holder)[0] else {
        panic!("Holder.value")
    };
    let envelope = field.type_reference;
    let arguments = |program: &TypedTrees, reference| {
        let TypeReferenceNode::Generic { arguments, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            panic!("generic source type");
        };
        *arguments
    };
    let buffer = program
        .type_reference_table
        .type_reference_handles(arguments(&program, envelope))[0];
    let parameters = program
        .type_reference_table
        .type_reference_handles(arguments(&program, buffer));
    let (byte, count) = (parameters[0], parameters[1]);
    let extent = |program: &TypedTrees, buffer| {
        let view = program.data_application_definition(buffer).unwrap();
        let DataMember::Field(slots) = &program.data_members(view)[0] else {
            panic!("Buffer.slots")
        };
        slots.type_reference
    };
    let original_extent = extent(&program, buffer);
    assert!(
        matches!(program.type_reference_table.type_reference(original_extent),
        TypeReferenceNode::FixedArray { element_type, length: FixedArrayLength::Expression(arg) }
        if *element_type == byte && *arg == count)
    );
    assert!(!program.data_application_is_closed(envelope));
    assert_eq!(
        settle_const_argument(&mut program, count, "-1"),
        Err("an applied array length is not a closed nonnegative integer".into())
    );
    assert!(matches!(
        program.type_reference_table.type_reference(count),
        TypeReferenceNode::ConstExpression(_)
    ));
    assert_eq!(extent(&program, buffer), original_extent);

    settle_const_argument(&mut program, count, "4").unwrap();
    assert_eq!(
        program
            .type_reference_table
            .anonymous_numeric_literal_value(count),
        Some(4)
    );
    assert_eq!(
        program
            .closed_data_definition(buffer)
            .unwrap()
            .generic_instance,
        Some(buffer)
    );
    let view = program.closed_data_definition(envelope).unwrap();
    assert_eq!(view.generic_instance, Some(envelope));
    let DataMember::Variant(value) = &program.data_members(view)[0] else {
        panic!("Envelope.Value")
    };
    let nested = program.data_payload_fields(value)[0].type_reference;
    for reference in [
        original_extent,
        extent(&program, buffer),
        extent(&program, nested),
    ] {
        assert!(
            matches!(program.type_reference_table.type_reference(reference),
            TypeReferenceNode::FixedArray { element_type, length: FixedArrayLength::Literal(4) }
            if *element_type == byte)
        );
    }
    let sizes = |program: &TypedTrees| {
        (
            program.type_reference_table.type_reference_count(),
            program.data_members.len(),
            program.data_payload_fields.len(),
        )
    };
    let before = sizes(&program);
    program.discard_open_data_application(buffer);
    assert!(program.data_application_is_closed(buffer));
    assert_eq!(prepare(&mut program, buffer), Ok(true));
    assert_eq!(prepare(&mut program, envelope), Ok(true));
    assert_eq!(sizes(&program), before);
}

#[test]
fn a_named_layout_closes_only_its_own_data_application() {
    let declarations = r#"
data Word { bits: u64; }
data Other { other: u8; }
trait Layout<T> { machine measure(value: T) -> u64; }
WordLayout satisfies Layout<Word> { machine measure(value: Word) -> u64 { value.bits } }
OtherLayout satisfies Layout<Other> { machine measure(value: Other) -> u64 { 0u64 } }
"#;
    for parameter in ["Policy satisfies Layout<T>", "const Policy: Layout<T>"] {
        let source = format!("{declarations}data Storage<T, {parameter}> {{ value: T; }}");
        for (holder, closed) in [
            (
                "data Holder { storage: Storage<Word, WordLayout>; }".to_owned(),
                true,
            ),
            (
                format!("data Holder<T, {parameter}> {{ field: Storage<T, Policy>; }}"),
                false,
            ),
        ] {
            let resolved = crate::resolve_fixture(&format!("{source}\n{holder}"));
            let program = crate::lower_symbol_resolved_trees(&resolved).unwrap();
            let holder = program
                .data_definitions()
                .iter()
                .find(|d| d.name.as_str() == "Holder")
                .unwrap();
            let DataMember::Field(field) = &program.data_members(holder)[0] else {
                panic!("Holder.storage");
            };
            assert_eq!(
                program.data_application_is_closed(field.type_reference),
                closed,
                "{parameter}"
            );
        }
        let resolved = crate::resolve_fixture(&format!(
            "{source}\ndata Holder {{ storage: Storage<Word, OtherLayout>; }}"
        ));
        let error = crate::lower_symbol_resolved_trees(&resolved).expect_err("wrong layout");
        assert!(
            error.message.contains("OtherLayout")
                && error.message.contains("Layout")
                && error.message.contains("Storage"),
            "{}",
            error.message
        );
    }
}
