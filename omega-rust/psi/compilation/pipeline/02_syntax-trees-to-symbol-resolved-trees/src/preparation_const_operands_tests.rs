/* Constant operands must select the declaration visible at their source site. */
use super::{GenericDataRequest, normalize_generic_data};
use std::sync::Arc;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{DataMember, Item};
use syntax_trees::types::{FixedArrayLength, TypeReferenceNode};

fn parse_files(units: &[(&str, &str)]) -> (SyntaxTrees, source::SourceMap) {
    let mut sources = source::SourceMap::default();
    let mut syntax = SyntaxTrees::default();
    for (path, text) in units {
        let path = std::path::PathBuf::from(path);
        let root = path.parent().unwrap().to_path_buf();
        let source = sources
            .add_checked_instance(
                path,
                *text,
                root,
                None,
                source::SourceOrigin::User,
                source::SourceResolutionStratum::Base,
                source::DependencyScope::Product,
            )
            .source_id;
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        let unit = tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap();
        syntax.extend_from(&unit);
    }
    (syntax, sources)
}

fn normalize_files(units: &[(&str, &str)]) -> Result<SyntaxTrees, Vec<diagnostics::Diagnostic>> {
    let (syntax, sources) = parse_files(units);
    normalize_generic_data(GenericDataRequest {
        syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
        retained_base: None,
        declaration_syntax: None,
        retained_declaration_syntax: None,
    })
}

fn instance_lengths(syntax: &SyntaxTrees) -> Vec<usize> {
    let mut lengths = Vec::new();
    for item in syntax.root_items() {
        let Item::Data(data) = item else { continue };
        if data.generic_instance.is_none() {
            continue;
        }
        for member in syntax.items.data_members(data.members) {
            let DataMember::Field(field) = member else {
                continue;
            };
            if let TypeReferenceNode::FixedArray {
                length: FixedArrayLength::Literal(length),
                ..
            } = syntax.type_references.type_reference(field.type_reference)
            {
                lengths.push(*length);
            }
        }
    }
    lengths.sort_unstable();
    lengths
}

#[test]
fn const_operands_in_two_packages_keep_their_own_values() {
    let first = "const LIMIT: u64 = 3;\n\
        data First<const N: u64> { bytes: [u8; N]; }\n\
        data FirstUse { value: First<LIMIT + 1>; }";
    let second = "const LIMIT: u64 = 9;\n\
        data Second<const N: u64> { bytes: [u8; N]; }\n\
        data SecondUse { value: Second<LIMIT + 1>; }";
    for units in [
        [("first/a.omg", first), ("second/b.omg", second)],
        [("second/b.omg", second), ("first/a.omg", first)],
    ] {
        let syntax = normalize_files(&units).unwrap();
        assert_eq!(instance_lengths(&syntax), [4, 10]);
    }
}

#[test]
fn const_operands_cannot_read_another_packages_private_constant() {
    let diagnostics = normalize_files(&[
        ("first/a.omg", "const LIMIT: u64 = 3;"),
        (
            "second/b.omg",
            "data Buffer<const N: u64> { bytes: [u8; N]; }\n\
            data Use { value: Buffer<LIMIT + 1>; }",
        ),
    ])
    .map(|_| ())
    .expect_err("a private constant is not visible from another package");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("LIMIT")),
        "{diagnostics:?}"
    );
}

#[test]
fn const_operands_read_derived_constants() {
    let syntax = normalize_files(&[(
        "app/main.omg",
        "const BASE: u64 = 3;\n\
        const LIMIT: u64 = BASE + 2;\n\
        data Buffer<const N: u64> { bytes: [u8; N]; }\n\
        data Use { value: Buffer<LIMIT + 1>; }",
    )])
    .unwrap();
    assert_eq!(instance_lengths(&syntax), [6]);
}

#[test]
fn const_operands_do_not_erase_a_constants_carrier() {
    let diagnostics = normalize_files(&[(
        "app/main.omg",
        "const LIMIT: u32 = 3;\n\
        data Buffer<const N: u64> { bytes: [u8; N]; }\n\
        data Use { value: Buffer<LIMIT + 1>; }",
    )])
    .map(|_| ())
    .expect_err("reading a typed constant does not silently widen it");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("carrier `u32`, expected `u64`")),
        "{diagnostics:?}"
    );
}

#[test]
fn const_operands_still_reject_overflow_and_unknown_names() {
    for (text, message) in [
        (
            "const LIMIT: u64 = 18446744073709551615;\n\
            data Buffer<const N: u64> { bytes: [u8; N]; }\n\
            data Use { value: Buffer<LIMIT + 1>; }",
            "addition exceeds",
        ),
        (
            "data Buffer<const N: u64> { bytes: [u8; N]; }\n\
            data Use { value: Buffer<MISSING + 1>; }",
            "MISSING",
        ),
    ] {
        let diagnostics = normalize_files(&[("app/main.omg", text)])
            .map(|_| ())
            .expect_err("invalid operand");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(message)),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn const_operands_without_a_source_map_still_use_header_selection() {
    let (syntax, _) = parse_files(&[(
        "app/main.omg",
        "const BASE: u64 = 3;\n\
        const LIMIT: u64 = BASE + 2;\n\
        data Buffer<const N: u64> { bytes: [u8; N]; }\n\
        data Use { value: Buffer<LIMIT + 1>; }",
    )]);
    let syntax = normalize_generic_data(GenericDataRequest::new(syntax)).unwrap();
    assert_eq!(instance_lengths(&syntax), [6]);
}

#[test]
fn const_operands_in_array_equations_keep_their_source() {
    let first = "const LIMIT: u64 = 3;\n\
        data First<Shape, const N: u64> requires Shape == [u8; N] { bytes: [u8; N]; }\n\
        data FirstUse { value: First<[u8; LIMIT]>; }";
    let second = "const BASE: u64 = 8; const LIMIT: u64 = BASE + 1;\n\
        data Second<Shape, const N: u64> requires Shape == [u8; N] { bytes: [u8; N]; }\n\
        data SecondUse { value: Second<[u8; LIMIT]>; }";
    let syntax = normalize_files(&[("first/a.omg", first), ("second/b.omg", second)]).unwrap();
    assert_eq!(instance_lengths(&syntax), [3, 9]);
}
