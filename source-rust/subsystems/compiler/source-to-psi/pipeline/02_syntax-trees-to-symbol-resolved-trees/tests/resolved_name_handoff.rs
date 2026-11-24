//! Name resolution must hand typing the declaration each written name selects.
//! Typing reads `TableNamePath::symbol` and never looks a name up again, so a
//! name bound to the wrong declaration becomes a program that reads the wrong
//! value or calls the wrong machine, and still type-checks when the two have
//! the same type.
//!
//! The spec rules are in `wiki/spec/language/machines.md` (nested machines
//! cannot capture runtime values from enclosing machines) and
//! `wiki/spec/language/constants.md`, "Evaluation".
//!
//! Each program is built so that the wrong choice is a different, valid
//! binding: a local and a package constant with the same name, and a local
//! with the same name as the value its initializer reads.

use crate::resolution::frontend_fixture::Lexer;
use crate::resolution::{ResolutionRequest, resolve};
use crate::source_resolved::SymbolResolvedTrees;
use crate::source_resolved::expression::ExpressionNode;
use symbols::SymbolHandle;

/// Resolve `files`, each `(path, text)` under the package directory `app`.
fn resolved(files: &[(&str, &str)]) -> SymbolResolvedTrees {
    let mut sources = crate::source_inputs::SourceMap::default();
    let mut syntax = syntax_trees::SyntaxTrees::default();
    for (path, text) in files {
        let source = sources
            .add_checked_instance(
                format!("app/{path}").into(),
                *text,
                "app".into(),
                None,
                source_admission::SourceOrigin::User,
                source_admission::SourceResolutionStratum::Base,
                source_provenance::DependencyScope::Product,
            )
            .source_id;
        let tokens = Lexer::new(text).tokenize().unwrap();
        let unit = crate::resolution::frontend_fixture::parse_syntax_trees_with_id(source, &tokens)
            .unwrap_or_else(|error| panic!("{path} does not parse: {}", error.message));
        syntax.extend_from(&unit);
    }
    resolve(ResolutionRequest {
        syntax,
        sources: Some(std::sync::Arc::new(sources)),
        top_level_bindings: Vec::new(),
        retained_base: None,
    })
    .unwrap_or_else(|refusal| panic!("resolution refused: {refusal:?}"))
}

/// What a name selects: its kind, its path, and where the selected
/// declaration is written as `(file, offset)`, or `None` when it selects
/// nothing.
type Selection = Option<(String, String, (usize, usize))>;

fn selection(trees: &SymbolResolvedTrees, symbol: SymbolHandle) -> Selection {
    symbol.is_valid().then(|| {
        let span = trees
            .symbols
            .symbol_source_span(symbol)
            .expect("a declared symbol has a source span");
        (
            format!("{:?}", trees.symbols.get(symbol).kind),
            trees.symbols.display_path(symbol, "\\"),
            (span.source_id.0, span.span.start),
        )
    })
}

/// The selection of every written name with this text, in source order of
/// `(file, offset)`.
fn uses(trees: &SymbolResolvedTrees, text: &str) -> Vec<((usize, usize), Selection)> {
    let table = &trees.tables.bodies.expressions;
    let mut found: Vec<_> = table
        .iter_expressions()
        .filter_map(|(handle, node)| {
            let ExpressionNode::Name(name) = node else {
                return None;
            };
            let members = table.name_path_members(name.members);
            let spelled: Vec<_> = members.iter().map(|member| member.as_str()).collect();
            (spelled.join("\\") == text).then(|| {
                let span = table.source_span(handle);
                (
                    (span.source_id.0, span.span.start),
                    selection(trees, name.symbol),
                )
            })
        })
        .filter(|((_, start), _)| *start != 0)
        .collect();
    found.sort_by_key(|(at, _)| *at);
    found
}

fn declared(kind: &str, path: &str, file: usize, offset: usize) -> Selection {
    Some((kind.to_owned(), path.to_owned(), (file, offset)))
}

#[test]
fn a_trait_value_argument_reads_the_providers_parameter_or_the_visible_constant() {
    use crate::source_resolved::types::TypeReference;

    let trees = resolved(&[(
        "main.omg",
        r#"
data K { }
const Tag: u64 = 29u64;
public boundary trait Reading<const N: u64> { machine total() -> u64 reaches none crashes none; }
Own<const K: u64> satisfies Reading<K> { machine total() -> u64 { 1u64 } }
Fixed satisfies Reading<Tag> { machine total() -> u64 { 2u64 } }
"#,
    )]);
    let global = trees
        .const_declarations
        .iter()
        .find(|row| trees.symbols.name(row.symbol) == "Tag")
        .unwrap()
        .symbol;
    assert!(global.is_valid());
    assert_eq!(trees.roots.conformances.len(), 2);
    for provider in trees.roots.conformances.iter() {
        let [TypeReference::Named { symbol, .. }] = trees.child_type_references(provider.arguments)
        else {
            panic!("expected a named value argument");
        };
        let expected = match trees.data_type_parameters(provider.type_parameters) {
            [formal] => {
                assert!(formal.symbol.is_valid() && formal.symbol != global);
                formal.symbol
            }
            [] => global,
            _ => panic!("unexpected parameters on provider"),
        };
        assert_eq!(*symbol, expected);
    }
}

/// `default_port` is nested in `connect`, which has a local also named
/// `port`. A nested machine cannot read its enclosing machine's runtime
/// values, so its `port` selects the package constant. The read stays a name
/// for that declaration, and later stages read the value it stores.
#[test]
fn a_constant_read_in_a_nested_machine_selects_the_package_constant() {
    let program = "const port: u32 = 1; machine connect() -> u32 { let port: u32 = 99; \
        default_port() machine default_port() -> u32 { port } }";
    let trees = resolved(&[("main.omg", program)]);
    let at = program.rfind("port }").unwrap();
    assert_eq!(
        uses(&trees, "port"),
        [((0, at), declared("Const", "port", 0, 6))]
    );
}

/// `let mutable total` introduces one local. The assignment, the read on
/// its right-hand side and the tail all select it, and `seed` selects the
/// parameter.
#[test]
fn every_use_after_a_let_selects_the_local_it_introduces() {
    let program =
        "machine step(seed: u8) -> u8 { let mutable total: u8 = seed; total = total + 1; total }";
    let trees = resolved(&[("main.omg", program)]);
    let local = declared("Local", "step\\entry\\total", 0, 43);
    assert_eq!(
        uses(&trees, "total"),
        [
            ((0, 61), local.clone()),
            ((0, 69), local.clone()),
            ((0, 80), local)
        ]
    );
    assert_eq!(
        uses(&trees, "seed"),
        [((0, 55), declared("Parameter", "step\\entry\\seed", 0, 13))]
    );
}

/// A local is in scope only after its initializer. In `let x = x;` the
/// initializer's `x` selects nothing (there is no other `x`), and the tail
/// selects the local. In the control, the initializer reads the parameter
/// and the tail reads the local.
#[test]
fn a_local_is_not_in_scope_in_its_own_initializer() {
    let program = "machine read_count(count: u32) -> u32 { let x = x; x }";
    let trees = resolved(&[("main.omg", program)]);
    assert_eq!(
        uses(&trees, "x"),
        [
            ((0, 48), None),
            ((0, 51), declared("Local", "read_count\\entry\\x", 0, 44)),
        ]
    );
    let control = "machine read_count(count: u32) -> u32 { let saved = count; saved }";
    let trees = resolved(&[("main.omg", control)]);
    assert_eq!(
        uses(&trees, "count"),
        [(
            (0, 52),
            declared("Parameter", "read_count\\entry\\count", 0, 19)
        )]
    );
    assert_eq!(
        uses(&trees, "saved"),
        [(
            (0, 59),
            declared("Local", "read_count\\entry\\saved", 0, 44)
        )]
    );
}

/// `use limits.omg;` binds the alias `limits` to that file, and
/// `|limits|::LIMIT` selects the constant it declares
/// (`wiki/spec/language/modules.md`, "Names and imports"). Name resolution
/// binds the file itself when no compiler loader supplied the alias, so a
/// program checked from its sources alone reads the same declaration the
/// Build would bind. In the twins only the import changes: under another
/// alias, or with no import at all, `|limits|::LIMIT` selects nothing, and
/// the read never falls back to the file's declaration by its name.
#[test]
fn a_file_alias_selects_the_declaration_of_the_file_it_imports() {
    let value = ("limits.omg", "public const LIMIT: u8 = 7;\n");
    let read = "machine read() -> u8 { |limits|::LIMIT }\n";
    let imported = format!("use limits.omg;\n{read}");
    let trees = resolved(&[("main.omg", &imported), value]);
    assert_eq!(
        uses(&trees, "|limits|::LIMIT"),
        [(
            (0, imported.rfind("|limits|").unwrap()),
            declared("Const", "LIMIT", 1, 13)
        )]
    );
    for twin in [format!("use limits.omg as other;\n{read}"), read.to_owned()] {
        let trees = resolved(&[("main.omg", &twin), value]);
        assert_eq!(
            uses(&trees, "|limits|::LIMIT"),
            [((0, twin.rfind("|limits|").unwrap()), None)],
            "{twin}"
        );
    }
}

#[test]
fn evidence_binder_children_belong_to_the_imported_trait() {
    let east = (
        "east/schema.omg",
        "use east/parent.omg as base; public trait Reading: |base|::Parent { machine east() -> u32; }",
    );
    let west = (
        "west/schema.omg",
        "use west/parent.omg as base; public trait Reading: |base|::Parent { machine west() -> u32; }",
    );
    for selected in ["east", "west"] {
        let other = if selected == "east" { "west" } else { "east" };
        let importing = format!(
            "use east/schema.omg as east; use west/schema.omg as west;
             machine apply<Reader satisfies |{selected}|::Reading>() -> u32 {{
                 Reader::{selected}() + Reader::{other}()
             }}"
        );
        for [first, second] in [[east, west], [west, east]] {
            let trees = resolved(&[
                ("main.omg", &importing),
                first,
                second,
                (
                    "east/parent.omg",
                    "public trait Parent { machine from_east() -> u32; }",
                ),
                (
                    "west/parent.omg",
                    "public trait Parent { machine from_west() -> u32; }",
                ),
            ]);
            let machine = trees
                .machines
                .iter()
                .find(|machine| machine.name.as_str() == "apply")
                .unwrap();
            let [bound] = machine.conformance_bounds.as_slice() else {
                panic!("one evidence binder")
            };
            let names = trees
                .symbols
                .child_handles(bound.binder)
                .into_iter()
                .flatten()
                .map(|child| trees.symbols.name(child))
                .collect::<Vec<_>>();
            assert_eq!(names, [selected, &format!("from_{selected}")]);
            let calls = trees
                .tables
                .bodies
                .expressions
                .iter_expressions()
                .filter_map(|(_, expr)| {
                    let ExpressionNode::Call(call) = expr else {
                        return None;
                    };
                    Some((call.target.as_str(), call.target_symbol))
                })
                .collect::<Vec<_>>();
            assert_eq!(calls.len(), 2);
            for (name, target) in calls {
                if name.ends_with(selected) {
                    assert!(target.is_valid());
                    assert_eq!(trees.symbols.get(target).parent, bound.binder);
                    assert_eq!(trees.symbols.name(target), selected);
                } else {
                    assert!(name.ends_with(other));
                    assert!(!target.is_valid(), "another trait's method was selected");
                }
            }
        }
    }
}
