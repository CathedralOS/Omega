/*
    Nested data declarations (`wiki/spec/language/data_and_literals.md`,
    "Records, sums and nested declarations"). The parser hoists each nested
    declaration to a root item named `Owner::Name` and rewrites the owner's
    own spellings of that name, so a later stage reads ordinary declarations.
*/

use syntax_trees::SyntaxTrees;
use syntax_trees::item::{DataDefinition, DataMember, Item};
use syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};

fn data<'p>(parsed: &'p SyntaxTrees, name: &str) -> &'p DataDefinition {
    parsed
        .root_items()
        .find_map(|item| match item {
            Item::Data(data) if data.name.as_str(parsed) == name => Some(data),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no root data declaration `{name}`"))
}

fn root_data_names(parsed: &SyntaxTrees) -> Vec<&str> {
    parsed
        .root_items()
        .filter_map(|item| match item {
            Item::Data(data) => Some(data.name.as_str(parsed)),
            _ => None,
        })
        .collect()
}

/// The written type of each field, or of each payload field of each case.
fn member_types(parsed: &SyntaxTrees, data: &DataDefinition) -> Vec<(String, String)> {
    let named = |handle: TypeReferenceHandle| match parsed.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) => name.as_str(parsed).to_owned(),
        TypeReferenceNode::Generic { base_name, .. } => format!("{}<..>", base_name.as_str(parsed)),
        other => format!("{other:?}"),
    };
    let mut types = Vec::new();
    for member in parsed.items.data_members(data.members) {
        match member {
            DataMember::Field(field) => {
                types.push((
                    field.name.as_str(parsed).to_owned(),
                    named(field.type_reference),
                ));
            }
            DataMember::Variant(variant) => {
                for field in parsed.items.data_payload_fields(variant.payload) {
                    types.push((
                        format!(
                            "{}.{}",
                            variant.name.as_str(parsed),
                            field.name.as_str(parsed)
                        ),
                        named(field.type_reference),
                    ));
                }
            }
            DataMember::Retired(_) => {}
        }
    }
    types
}

fn pairs(rows: &[(&str, &str)]) -> Vec<(String, String)> {
    rows.iter()
        .map(|(name, ty)| ((*name).to_owned(), (*ty).to_owned()))
        .collect()
}

/// The spec's `Command` example: the owner's `op: Operation` names the nested
/// sum, which is a root declaration `Command::Operation` with its own cases.
#[test]
fn a_nested_declaration_is_a_root_declaration_under_its_owner_name() {
    let parsed = crate::tests::parse_source(
        "data Command {
            command_id: u64;
            op: Operation;

            data Operation {
                case GetUser(user_id: u64);
                case CountUsers;
            }
        }",
    );
    assert_eq!(root_data_names(&parsed), ["Command", "Command::Operation"]);
    let command = data(&parsed, "Command");
    assert_eq!(
        member_types(&parsed, command),
        pairs(&[("command_id", "u64"), ("op", "Command::Operation")])
    );
    let operation = data(&parsed, "Command::Operation");
    let cases: Vec<_> = parsed
        .items
        .data_members(operation.members)
        .iter()
        .map(|member| match member {
            DataMember::Variant(variant) => variant.name.as_str(&parsed),
            _ => panic!("the nested sum holds only its cases"),
        })
        .collect();
    assert_eq!(cases, ["GetUser", "CountUsers"]);
    // The nested name keeps the span of its own spelling.
    assert_eq!(
        operation.name.source_span().span.end - operation.name.source_span().span.start,
        "Operation".len()
    );
}

/// The innermost declaration of a name wins, a sibling is named through the
/// enclosing owner, and a name an inner owner qualified is not qualified
/// again even when its first segment spells an enclosing nested name.
#[test]
fn each_name_selects_its_innermost_nested_declaration() {
    let parsed = crate::tests::parse_source(
        "data A {
            b: B;
            x: X;
            data B {
                x: X;
                sibling: C;
                data X { v: u8; }
            }
            data C { v: u8; }
            data X { w: u16; }
            data A { v: u8; }
        }
        data Outside { a: A; x: A::X; }",
    );
    assert_eq!(
        root_data_names(&parsed),
        ["A", "A::B::X", "A::B", "A::C", "A::X", "A::A", "Outside"]
    );
    assert_eq!(
        member_types(&parsed, data(&parsed, "A")),
        pairs(&[("b", "A::B"), ("x", "A::X")])
    );
    assert_eq!(
        member_types(&parsed, data(&parsed, "A::B")),
        pairs(&[("x", "A::B::X"), ("sibling", "A::C")])
    );
    // Outside the owner, nothing is rewritten.
    assert_eq!(
        member_types(&parsed, data(&parsed, "Outside")),
        pairs(&[("a", "A"), ("x", "A::X")])
    );
}

/// `name: data Type { ... }` declares `Type` under the enclosing data, also
/// in a case payload: a case is not a scope.
#[test]
fn a_field_declared_with_its_type_names_the_type_under_the_enclosing_data() {
    let parsed = crate::tests::parse_source(
        "data Holder {
            payload: data Payload {
                case Small(value: u8);
                case Node(node: data Pair { left: u8; right: u8; });
            };
            count: u8;
        }",
    );
    assert_eq!(
        root_data_names(&parsed),
        ["Holder", "Holder::Payload::Pair", "Holder::Payload"]
    );
    assert_eq!(
        member_types(&parsed, data(&parsed, "Holder")),
        pairs(&[("payload", "Holder::Payload"), ("count", "u8")])
    );
    assert_eq!(
        member_types(&parsed, data(&parsed, "Holder::Payload")),
        pairs(&[
            ("Small.value", "u8"),
            ("Node.node", "Holder::Payload::Pair")
        ])
    );
}

/// A fieldless owner, a nested declaration with its own const parameter
/// applied at the owner's field, and a nested alias.
#[test]
fn a_nested_declaration_keeps_its_own_parameters() {
    let parsed = crate::tests::parse_source(
        "data Outer {
            slot: Inner<4>;
            data Inner<const N: u64> { values: [u8; N]; }
            data Header = Inner<2>;
        }
        data Empty { data Inner { v: u8; } }",
    );
    assert_eq!(
        member_types(&parsed, data(&parsed, "Outer")),
        pairs(&[("slot", "Outer::Inner<..>")])
    );
    assert_eq!(
        parsed
            .items
            .type_parameters(data(&parsed, "Outer::Inner").type_parameters)
            .len(),
        1
    );
    let header = parsed
        .root_items()
        .find_map(|item| match item {
            Item::TypeAlias(alias) => Some(alias),
            _ => None,
        })
        .expect("the nested alias is a root declaration");
    assert_eq!(header.name.as_str(&parsed), "Outer::Header");
    assert!(matches!(
        parsed.type_references.type_reference(header.expansion),
        TypeReferenceNode::Generic { base_name, .. } if base_name.as_str(&parsed) == "Outer::Inner"
    ));
    assert!(data(&parsed, "Empty").members.is_empty());
    assert_eq!(
        member_types(&parsed, data(&parsed, "Empty::Inner")),
        pairs(&[("v", "u8")])
    );
}

/// A nested declaration inherits its owner's visibility, recursively.
#[test]
fn a_nested_declaration_inherits_its_owners_visibility() {
    let parsed = crate::tests::parse_source("public data Command { op: Operation; data Operation { case Stop; data Detail { v: u8; } } }
        data Private { data Inner { v: u8; } }");
    for (name, public) in [
        ("Command", true),
        ("Command::Operation", true),
        ("Command::Operation::Detail", true),
        ("Private", false),
        ("Private::Inner", false),
    ] {
        assert_eq!(data(&parsed, name).is_public, public, "{name}");
    }
}

#[test]
fn nested_declarations_reject_their_own_visibility_parameters_inline_and_order() {
    for (source, expected) in [
        (
            "data Command { op: Operation; public data Operation { case Stop; } }",
            "takes no `public` of its own",
        ),
        (
            "data Command { data Operation { case Stop; } op: Operation; }",
            "declare nested data after the fields or cases",
        ),
        (
            "data Command { op: data Operation<T> { case Stop; }; }",
            "takes no parameters",
        ),
        (
            "data Command { op: data Operation = u8; }",
            "takes no parameters",
        ),
        // `Envelope<u32>::Body` and `Envelope<bool>::Body` are distinct
        // types; the hoisted `Envelope::Body` cannot tell them apart yet.
        (
            "data Envelope<T> { body: data Body { case Value(value: T); case Empty; }; }",
            "nested in an owner with static parameters",
        ),
        (
            "data Packet<'a> { data Payload<U> { value: @'a U; } }",
            "nested in an owner with static parameters",
        ),
    ] {
        let error = crate::tests::parse_error(source);
        assert!(
            error.message.contains(expected),
            "{source}: {}",
            error.message
        );
    }
}
