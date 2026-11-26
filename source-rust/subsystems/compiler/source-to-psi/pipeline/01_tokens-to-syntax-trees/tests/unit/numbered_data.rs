use syntax_trees::item::{DataMember, Item};

#[test]
fn numbered_declarations_share_ordinary_data_and_source_identity() {
    for declaration in [
        "data Message { #7 value: u32; retired #8; }",
        "data Message<T> { #7 value: T; retired #8; }",
        "data Message [copy] { #7 value: u32; retired #8; }",
        "data Message requires value <= 10 { #7 value: u32; retired #8; }",
        "data Message<'scope> { #7 value: @'scope u32; retired #8; }",
        "data Message { #7 value: u32; #9 case Ready; }",
    ] {
        let parsed = crate::tests::parse_source(declaration);
        let Some(Item::Data(definition)) = parsed.root_items().next() else {
            panic!("numbering must not select a different declaration representation");
        };
        let DataMember::Field(field) = &parsed.items.data_members(definition.members)[0] else {
            panic!("ordinary first field");
        };
        assert_eq!(field.identity, Some(7));
        assert_eq!(field.name.as_str(&parsed), "value");
        let span = field.name.source_span().span;
        assert_eq!(&declaration[span.start..span.end], "value");
    }
}

#[test]
fn nested_version_blocks_reject_regardless_of_leading_member() {
    for members in [
        "version v1 { #1 value: u32; } #1 value: u32;",
        "#1 value: u32; version v1 { #1 value: u32; }",
        "retired #2; version v1 { #1 value: u32; } #1 value: u32;",
    ] {
        let declaration = format!("data Message {{ {members} }}");
        let error = crate::tests::parse_error(&declaration);
        assert!(error.message.contains("data `version` blocks are retired"));
    }
}

#[test]
fn retired_is_a_field_name_unless_followed_by_an_identity() {
    for declaration in [
        "data Record { retired: u32; }",
        "data Sum { case Held(retired: u32); }",
        "data Record { #1 retired: u32; retired #2; }",
        "data Sum { #1 case Held(#1 retired: u32, retired #2); }",
    ] {
        let parsed = crate::tests::parse_source(declaration);
        let data = crate::tests::root_data(&parsed, "data declaration");
        let field = match &parsed.items.data_members(data.members)[0] {
            DataMember::Field(field) => field,
            DataMember::Variant(variant) => &parsed.items.data_payload_fields(variant.payload)[0],
            _ => panic!("the field must retain its ordinary declaration"),
        };
        assert_eq!(field.name.as_str(&parsed), "retired");
    }
}

#[test]
fn numbered_data_rejects_duplicate_and_retired_identity_reuse() {
    for members in [
        "#7 left: u32; #7 right: u32;",
        "#7 value: u32; retired #7;",
        "#7 value: u32; retired #8; retired #8;",
    ] {
        let declaration = format!("data Message {{ {members} }}");
        let error = crate::tests::parse_error(&declaration);
        assert!(error.message.contains("declared more than once"));
    }
}
