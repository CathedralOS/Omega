//! Equal values hide a wrong declaration choice. Follow one initializer through the public stage products.
use omega_lexer::{SourceSpan, Token, TokenKind, lex};
use omega_parser::{IndexedSyntax, NodeKind, parse_file};
use omega_preparation::{PreparedTrees, PreparedValueOrigin, prepare_source_package_diagnosed};
use omega_resolver::{NameRole, ParsedSource, StageSource, Symbol};
use omega_typer::{NodeSemantics, SourcePackage, TypedTrees, resolve_source_package, type_source_package};

const STAGES: [&str; 5] = ["L0", "L1", "L2", "L3", "L4"];

fn report_enabled() -> bool {
    std::env::var_os("OMEGA_LAYER_REPORT").is_some()
}

fn observe(
    stage: &str,
    selected: &str,
    actual: &str,
) {
    if report_enabled() {
        println!("OMEGA_CONSTANT_IDENTITY\t{stage}\t{selected}\t{actual}");
    }
    assert_eq!(actual, selected, "{stage} selected the wrong constant");
}

/// Visit each product while its source and arena are live. A later failure must preserve
/// the observations already made at earlier layers.
fn visit_products(selected: &str, mut visit: impl FnMut(usize, &dyn Fn() -> String)) {
    let prefix = "const FIRST:u8=7u8; const OTHER:u8=7u8; const COPY:u8=";
    let source = format!("{prefix}{selected};");
    let span = SourceSpan::new(prefix.len(), prefix.len() + selected.len());
    let tokens = lex(source.as_bytes()).unwrap();
    visit(0, &|| read_tokens(&source, span, &tokens));

    let syntax = parse_file(source.as_bytes(), &tokens).unwrap();
    let indexed = syntax.index(&source, &tokens).unwrap();
    visit(1, &|| read_syntax(span, &indexed));

    let package = resolve_source_package(&[StageSource {
        package: "app",
        source: ParsedSource { logical_path: "main.omg", text: &source, tokens: &tokens, tree: &syntax },
    }]).unwrap();
    visit(2, &|| read_resolution(span, &package));

    let typed = type_source_package(&package).unwrap();
    visit(3, &|| read_typing(&typed));
    let prepared = prepare_source_package_diagnosed(&package).unwrap();
    visit(4, &|| read_preparation(prepared.typed(), &prepared));
}

fn read_tokens(source: &str, span: SourceSpan, tokens: &[Token]) -> String {
    let token = tokens.iter().find(|token| token.get_span() == span).unwrap();
    assert_eq!(token.get_kind(), TokenKind::Word);
    std::str::from_utf8(span.slice(source.as_bytes()).unwrap()).unwrap().to_owned()
}

fn read_syntax(span: SourceSpan, indexed: &IndexedSyntax) -> String {
    let expressions: Vec<_> = indexed
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::PathExpression)
        .collect();
    let [expression] = expressions.as_slice() else {
        panic!("expected one initializer reference")
    };
    let leaves = &indexed.tokens[expression.first_token..expression.end_token];
    let [leaf] = leaves else { panic!("expected one name at the initializer") };
    assert_eq!(leaf.span, span);
    leaf.text.to_owned()
}

fn read_resolution(
    span: SourceSpan,
    package: &SourcePackage,
) -> String {
    let mut references = package
        .uses()
        .filter(|usage| usage.span == span && usage.role == NameRole::Reference);
    let Symbol::Declaration(decl) = &references.next().unwrap().target else {
        panic!("expected a constant declaration")
    };
    assert!(references.next().is_none());
    assert_eq!(decl.package, "app");
    decl.path.clone()
}

fn read_typing(typed: &TypedTrees) -> String {
    let alias = typed
        .constants
        .iter()
        .find(|c| c.declaration.path == "COPY")
        .unwrap();
    let tree = typed.trees().next().unwrap();
    let mut constants = tree
        .semantics_at(alias.initializer.node.node)
        .filter_map(|record| match record {
            NodeSemantics::Constant(id) => Some(id),
            _ => None,
        });
    let constant = constants.next().unwrap();
    assert!(constants.next().is_none());
    assert_eq!(constant.package, "app");
    constant.path.clone()
}

/// `typed` supplies the COPY declaration and must come from the same twin as `prepared`.
fn read_preparation(
    typed: &TypedTrees,
    prepared: &PreparedTrees,
) -> String {
    let alias = typed
        .constants
        .iter()
        .find(|c| c.declaration.path == "COPY")
        .unwrap();
    assert_eq!(prepared.integer_constant(&alias.declaration), Some(&7u8.into()));
    let inputs = prepared
        .dependencies()
        .inputs(&PreparedValueOrigin::Constant(alias.declaration.clone()))
        .unwrap();
    let [dependency] = inputs.constants.as_slice() else {
        panic!("expected exactly one input of COPY")
    };
    assert_eq!(dependency.package, "app");
    dependency.path.clone()
}

fn follow_initializer(selected: &str) {
    visit_products(selected, |index, read| observe(STAGES[index], selected, &read()));
}

#[test]
fn alias_selects_first_equal_valued_constant() {
    follow_initializer("FIRST");
}

#[test]
fn alias_selects_other_equal_valued_constant() {
    follow_initializer("OTHER");
}

/// Each stage reader, handed the OTHER twin's real product while FIRST is still expected,
/// must report OTHER; every earlier stage keeps reading FIRST's unmodified products.
#[test]
fn each_stage_reader_distinguishes_substituted_other_product() {
    for (substituted, stage) in STAGES.into_iter().enumerate() {
        let mut actual = Vec::new();
        visit_products("FIRST", |index, read| {
            if index < substituted {
                actual.push((index, read()));
            }
        });
        visit_products("OTHER", |index, read| {
            if index == substituted {
                actual.push((index, read()));
            }
        });
        let expected: Vec<_> = (0..=substituted)
            .map(|index| (index, if index == substituted { "OTHER" } else { "FIRST" }.to_owned()))
            .collect();
        assert_eq!(actual, expected, "substituted {stage} product");
        if report_enabled() {
            println!("OMEGA_IDENTITY_CONTROL\t{stage}\tFIRST\tOTHER");
        }
    }
}
