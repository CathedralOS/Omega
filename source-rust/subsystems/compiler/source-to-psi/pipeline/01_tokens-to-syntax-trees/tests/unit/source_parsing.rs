use crate::frontend_fixture::Lexer;
use crate::{parse_file, parse_into as parse};
use source_files::{DependencyScope, LoadedSource, SourceOrigin};
use source_text::SourceId;
use std::sync::Arc;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::Item;

fn lexed_file(source_id: SourceId, path: &str, text: &str) -> tokens::LexedSource {
    let mut loaded = LoadedSource {
        source_id,
        path: path.into(),
        source: Arc::from(text),
        origin: Some(SourceOrigin::User),
        scope: DependencyScope::Build,
    };
    source_lexer::lex_file(&mut loaded, &mut tokens::TokenScratch::default()).unwrap()
}

#[test]
fn appending_sources_preserves_root_handles_and_source_identity() {
    let mut trees = SyntaxTrees::new(SourceId(11));
    let first_tokens = Lexer::new("data First {} /* trailing trivia */")
        .tokenize()
        .unwrap();
    let first = parse(&mut trees, SourceId(11), &first_tokens)
        .unwrap()
        .items;
    let original = trees.root_item(first[0]).clone();
    let mut lexed = lexed_file(SourceId(22), "second.omg", "data Second {} data Third {}");
    let backing = lexed.tokens.source_owner().unwrap().clone();
    let mut page = SyntaxTrees::new_with_generation(SourceId(22), 2);
    let appended = parse_file(&mut page, &mut lexed).unwrap();
    trees.adopt_page(page);
    assert_eq!(appended.source_id, SourceId(22));
    assert_eq!(appended.path, std::path::PathBuf::from("second.omg"));
    assert_eq!(appended.origin, Some(SourceOrigin::User));
    assert_eq!(appended.scope, DependencyScope::Build);
    assert!(Arc::ptr_eq(&appended.source, &backing));
    let second = &appended.root_items;

    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 2);
    assert_eq!(trees.root_item_handles(), [first[0], second[0], second[1]]);
    assert!(syntax_trees::SyntaxEquivalent::same_syntax(
        trees.root_item(first[0]),
        &original,
        &trees.text,
        &trees.text
    ));
    for (handle, expected_source) in [(first[0], SourceId(11)), (second[0], SourceId(22))] {
        let Item::Data(data) = trees.root_item(handle) else {
            panic!("data root")
        };
        assert_eq!(data.name.source_span().source_id, expected_source);
    }
    let trivia = Lexer::new(" // no declarations\n ").tokenize().unwrap();
    assert!(
        parse(&mut trees, SourceId(33), &trivia)
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(trees.root_item_count(), 3);
}

#[test]
fn parser_retains_one_worker_page_identity_through_nested_handles() {
    const PAGE_IDENTITY: u32 = 7;
    let source_id = SourceId(44);
    let tokens =
        Lexer::new("machine read(value: u64) -> u64 { let selected: u64 = value; selected }")
            .tokenize()
            .unwrap();
    let mut trees = SyntaxTrees::new_with_generation(source_id, PAGE_IDENTITY);
    let roots = parse(&mut trees, source_id, &tokens).unwrap().items;

    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].generation(), PAGE_IDENTITY);
    assert!(
        trees
            .expressions
            .iter_expressions()
            .all(|(handle, _)| handle.generation() == PAGE_IDENTITY)
    );

    let Item::Machine(machine) = trees.root_item(roots[0]) else {
        panic!("machine root")
    };
    assert_eq!(machine.states.start().generation(), PAGE_IDENTITY);
    for state in trees.items.state_handles(machine.states) {
        assert_eq!(state.generation(), PAGE_IDENTITY);
        let state = trees.items.state(*state);
        assert_eq!(state.parameters.start().generation(), PAGE_IDENTITY);
        assert_eq!(state.statements.start().generation(), PAGE_IDENTITY);
        for parameter in trees.items.state_parameters(state.parameters) {
            assert_eq!(parameter.generation(), PAGE_IDENTITY);
            assert_eq!(
                trees
                    .items
                    .state_parameter(*parameter)
                    .type_reference
                    .generation(),
                PAGE_IDENTITY
            );
        }
        assert!(
            trees
                .items
                .statements(state.statements)
                .iter()
                .all(|statement| statement.generation() == PAGE_IDENTITY)
        );
    }
}

#[test]
fn parse_file_error_reports_source_location_without_affecting_adopted_trees() {
    let mut trees = SyntaxTrees::new(SourceId(11));
    let tokens = Lexer::new("data Retained {} ;").tokenize().unwrap();
    let error = parse(&mut trees, SourceId(22), &tokens).unwrap_err();
    assert_eq!(error.source_span.source_id, SourceId(22));
    assert_eq!(error.source_span.span.start, 17);
    assert_eq!(trees.root_item_count(), 1);
    let Item::Data(data) = trees.root_items().next().unwrap() else {
        panic!("data root")
    };
    assert_eq!(data.name.as_str(&trees), "Retained");

    let mut page = SyntaxTrees::new_with_generation(SourceId(33), 2);
    let mut failing = lexed_file(SourceId(33), "appended.omg", "data Another {}\n  ;");
    let error = parse_file(&mut page, &mut failing)
        .unwrap_err()
        .into_diagnostic();
    assert!(error.message.starts_with("appended.omg:2:3: "));
    assert_eq!(trees.root_item_count(), 1);
    let Item::Data(data) = trees.root_items().next().unwrap() else {
        panic!("existing syntax remains intact");
    };
    assert_eq!(data.name.as_str(&trees), "Retained");
    assert_eq!(data.name.source_span().source_id, SourceId(22));
}

#[test]
fn import_bounds_preserve_interleaved_root_ordinals() {
    let cases = [
        ("", 0..0, vec![]),
        ("data Only {}", 0..0, vec![]),
        ("use alpha.omg; data Tail {}", 0..1, vec![0]),
        ("data Head {} use alpha.omg;", 1..2, vec![1]),
        (
            "data Head {} use alpha.omg; let pi(): |alpha|::Real = term; data Middle {} use beta.omg; data Tail {}",
            1..4,
            vec![1, 3],
        ),
    ];
    for (text, expected_range, expected_ordinals) in cases {
        let mut trees = SyntaxTrees::default();
        let mut lexed = lexed_file(SourceId(27), "imports.omg", text);
        let source = parse_file(&mut trees, &mut lexed).unwrap();
        assert_eq!(source.import_root_range, expected_range);
        let actual = source.root_items[source.import_root_range.clone()]
            .iter()
            .enumerate()
            .filter_map(|(offset, root)| {
                matches!(trees.root_item(*root), Item::Use(_))
                    .then_some(source.import_root_range.start + offset)
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected_ordinals);
    }
}

#[test]
fn syntax_names_remain_valid_after_all_token_and_source_rows_are_released() {
    let mut lexed = lexed_file(SourceId(55), "retained.omg", "data Retained {}");
    let mut syntax = SyntaxTrees::default();
    let parsed = parse_file(&mut syntax, &mut lexed).unwrap();
    let root = parsed.root_items[0];
    drop(lexed);
    drop(parsed);
    let Item::Data(data) = syntax.root_item(root) else {
        panic!("data root")
    };
    assert_eq!(data.name.as_str(&syntax), "Retained");
    assert_eq!(data.name.source_span().source_id, SourceId(55));
    let retained_name = data.name.clone();
    let retained_text = syntax.text.clone();
    drop(syntax);
    assert_eq!(retained_name.as_str(&retained_text), "Retained");
}

#[test]
fn float_spellings_share_forest_custody_and_survive_cross_forest_copy() {
    use syntax_trees::expression::ExpressionNode;
    let source: Arc<str> = Arc::from(
        "machine numbers() { let first: f32 = 1.25f32; let second: f64 = -2.5e-2f64; first }",
    );
    let tokens = crate::frontend_fixture::lex_retained(source.clone()).unwrap();
    let syntax = crate::parse_syntax_trees(&tokens).unwrap();
    drop(tokens);
    assert_eq!(
        Arc::strong_count(&source),
        2,
        "one forest owner, no literal owners"
    );
    let original = syntax
        .expressions
        .iter_expressions()
        .filter_map(|(handle, node)| matches!(node, ExpressionNode::Float(_)).then_some(handle))
        .collect::<Vec<_>>();
    assert_eq!(
        original.len(),
        3,
        "positive, unsigned operand, folded negative"
    );
    let expected = original
        .iter()
        .map(|handle| {
            (
                syntax.display_expression_name(*handle),
                syntax.expressions.source_span(*handle),
            )
        })
        .collect::<Vec<_>>();
    assert!(
        expected
            .iter()
            .any(|(spelling, _)| spelling == "-2.5e-2f64")
    );
    let mut copied = SyntaxTrees::default();
    let handles = original
        .iter()
        .map(|handle| copied.copy_expression_from(&syntax, *handle))
        .collect::<Vec<_>>();
    assert_eq!(
        Arc::strong_count(&source),
        3,
        "copies retain one page, not one owner per literal"
    );
    drop(syntax);
    assert_eq!(Arc::strong_count(&source), 2);
    drop(source);
    for (handle, (spelling, span)) in handles.into_iter().zip(expected) {
        assert_eq!(copied.display_expression_name(handle), spelling);
        assert_eq!(copied.expressions.source_span(handle), span);
    }
}

#[test]
fn a_file_used_for_a_product_entry_can_also_be_used_as_build_code() {
    use syntax_trees::item::UseItem;
    let text = r#"
use main.omg as app;
use helper.omg as helper;
use slot.omg;
use provider.omg;
use local.omg;
machine build(builder: @mutable Build) {
    Build::bind(builder, |helper|::Entry, |app|::main);
    Build::select_provider::<|slot|::Reading, |provider|::Selected>(builder);
    Helpers::select_provider::<|local|::Reader>(builder);
    |helper|::configure(builder);
}"#;
    let mut trees = SyntaxTrees::new(SourceId(1));
    let tokens = Lexer::new(text).tokenize().unwrap();
    let roots = parse(&mut trees, SourceId(1), &tokens).unwrap();
    let imports = roots
        .items
        .iter()
        .filter_map(|root| match trees.root_item(*root) {
            Item::Use(UseItem {
                alias, qualifiers, ..
            }) => Some((
                alias.as_str(&trees),
                !qualifiers.is_empty()
                    && qualifiers
                        .iter()
                        .all(|usage| usage.role == syntax_trees::item::FileQualifierRole::Product),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        imports,
        [
            ("app", true),
            ("helper", false),
            ("slot", true),
            ("provider", true),
            ("local", false),
        ]
    );
}

#[test]
fn a_description_uses_the_file_selected_by_the_query_alias() {
    use syntax_trees::item::{FileQualifierRole::*, UseItem};
    let text = "use {omega}/language/core/reflect/reflect.omg as describe;
        use fake.omg as reflect;
        use {package}/types.omg as types;
        machine run() {
            _ = |describe|::field(|types|::Table::item);
            _ = |describe|::parameter(|types|::Reader::run::size);
            _ = |reflect|::field(|types|::Table::item);
            _ = |describe|::field(|types|::compute());
        }";
    let mut trees = SyntaxTrees::new(SourceId(1));
    let tokens = Lexer::new(text).tokenize().unwrap();
    let roots = parse(&mut trees, SourceId(1), &tokens).unwrap();
    let Item::Use(UseItem { qualifiers, .. }) = trees.root_item(roots.items[2]) else {
        panic!("expected the types file import");
    };
    assert_eq!(
        qualifiers
            .iter()
            .map(|usage| usage.role)
            .collect::<Vec<_>>(),
        [Description, Description, Code, Code]
    );
}

#[test]
fn file_imports_keep_their_roots_filenames_and_local_aliases() {
    use syntax_trees::item::UseItem;
    let text = "use math/vector.omg; use {geometry}/math/value-cell.omg as chosen;";
    let mut trees = SyntaxTrees::new(SourceId(1));
    let tokens = Lexer::new(text).tokenize().unwrap();
    let roots = parse(&mut trees, SourceId(1), &tokens).unwrap();
    let targets = roots
        .items
        .iter()
        .map(|item| {
            let Item::Use(UseItem {
                root,
                components,
                alias,
                ..
            }) = trees.root_item(*item)
            else {
                panic!("expected a file import");
            };
            (
                root.as_ref().map(|root| root.as_str(&trees)),
                trees
                    .items
                    .identifier_path_members(*components)
                    .iter()
                    .map(|name| name.as_str(&trees))
                    .collect::<Vec<_>>(),
                alias.as_str(&trees),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        targets,
        [
            (None, vec!["math", "vector"], "vector"),
            (Some("geometry"), vec!["math", "value-cell"], "chosen"),
        ]
    );
}

#[test]
fn invalid_file_paths_and_aliases_are_refused_at_the_import() {
    let parse_error = |text: &str| {
        let tokens = Lexer::new(text).tokenize().unwrap();
        parse(&mut SyntaxTrees::new(SourceId(1)), SourceId(1), &tokens).unwrap_err()
    };
    for text in ["use missing/u8.omg;", "use missing/value.omg as u8;"] {
        assert_eq!(
            parse_error(text).message,
            "file alias `u8` names a builtin type"
        );
    }
    let duplicate = parse_error("use audio/text.omg;\nuse {dep}/text.omg;");
    assert!(
        duplicate
            .message
            .contains("duplicate file import alias `text`, first imported at line 1, column 11"),
        "{}",
        duplicate.message
    );
    assert_eq!(duplicate.source_span.span.start, 30);
    for text in [
        "use dir/value-cell.omg;",
        "use dir/value.omg as value-cell;",
        "use /a.omg;",
        "use ./a.omg;",
        "use ../a.omg;",
        "use a/../b.omg;",
        "use helpers;",
        r"use net\wire\Packet;",
        "use net::wire::Packet;",
        "use dir/a;",
        "use dir/a.txt;",
        "use {dep}//x.omg;",
    ] {
        parse_error(text);
    }
}
#[test]
fn a_backslash_cannot_select_an_imported_type_or_machine() {
    for text in [
        r"use net/wire.omg; data Record { field: wire\Packet; }",
        r"use net/wire.omg; machine caller() { wire\send::<u8>(); }",
    ] {
        crate::tests::parse_error(text);
    }
}

#[test]
fn root_operand_file_roles_follow_the_declared_build_record() {
    use syntax_trees::item::UseItem;
    for (header, declaration, product_only) in [
        (
            "",
            "Build satisfies Writer { Writer::bind = write; }",
            false,
        ),
        ("<const Build: Writer>", "", false),
        ("<Build satisfies Writer>", "", false),
        ("<const Build: u64>", "", true),
        ("", "", true),
    ] {
        let text = format!(
            r#"
            use values.omg;
            machine configure{header}(builder: @mutable Build) {{
                Build::bind(builder, |values|::slot, |values|::entry);
            }}
            trait Writer {{ machine bind(a: u64, b: u64, c: u64); }}
            {declaration}
        "#
        );
        let parsed = crate::tests::parse_source(&text);
        let import = parsed.root_items().find_map(|item| match item {
            Item::Use(UseItem { qualifiers, .. }) => Some(
                !qualifiers.is_empty()
                    && qualifiers
                        .iter()
                        .all(|usage| usage.role == syntax_trees::item::FileQualifierRole::Product),
            ),
            _ => None,
        });
        assert_eq!(import, Some(product_only), "{text}");
    }
}
