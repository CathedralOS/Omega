//! One program for testing what stage 04 hands stage 05 about a constant used
//! as a match pattern. Two modules each declare a public Boolean constant
//! named `ON`, with opposite values. `choose` tests `east\ON`, `other` tests
//! `west\ON`, and `plain` tests the literal `true` and names no constant.
//!
//! Only the qualified path tells the two constants apart, so a compiler that
//! picked a constant by its last name would give one machine the other's
//! value and return the wrong arm.
#![allow(dead_code)]

#[path = "../../../../../../../../tests/support/frontend.rs"]
mod frontend;

use application_identity::const_value::{DecodedCanonicalConstValue, decode_declaration_value};
use semantic_vocabulary::PackageKeyIdentity;
use source_compiler::representations::source_inputs::SourceMap;
use source_compiler::representations::source_state::TypedTrees;
use source_compiler::representations::source_state::constant::ConstDeclaration;
use source_compiler::representations::source_state::expression::{
    ExpressionHandle, ExpressionNode,
};
use source_compiler::test_support::preparation::PreparedTypedTrees;
use source_compiler::test_support::{
    SourcePreparationRequest, TerminalSourceCompilation, prepare_source_for_test, resolution,
    typing,
};
use source_provenance::DependencyScope;
use symbols::SymbolHandle;

pub const MAIN: &str = r"machine choose(x: bool) -> u8 {
    match x { east\ON -> 17u8, _ -> 23u8, }
}
machine other(x: bool) -> u8 {
    match x { west\ON -> 17u8, _ -> 23u8, }
}
machine plain(x: bool) -> u8 {
    match x { true -> 17u8, _ -> 23u8, }
}";

pub const MACHINES: [&str; 3] = ["choose", "other", "plain"];

/// How each module writes its constant. A literal is settled when names are
/// resolved; a call is run by the source driver before stage 04. Both must
/// reach stage 05 in the same form.
#[derive(Debug, Clone, Copy)]
pub enum Initializer {
    Literal,
    Call,
}

pub const INITIALIZERS: [Initializer; 2] = [Initializer::Literal, Initializer::Call];

impl Initializer {
    fn module(self, value: bool) -> String {
        match self {
            Self::Literal => format!("public const ON: bool = {value};"),
            Self::Call => format!(
                "public const ON: bool = flag();
                machine flag() -> bool crashes none {{ {value} }}"
            ),
        }
    }
}

pub struct Prepared {
    pub program: PreparedTypedTrees,
    entries: Vec<SymbolHandle>,
    subjects: [Option<PackageKeyIdentity>; 1],
}

impl Prepared {
    /// The request both stages receive: the three machines as entries and
    /// every declaration of the package as a subject, so stage 05 also reads
    /// both constant declarations.
    pub fn compilation(&self) -> TerminalSourceCompilation<'_> {
        compilation(&self.entries, &self.subjects)
    }
}

fn compilation<'a>(
    entries: &'a [SymbolHandle],
    subjects: &'a [Option<PackageKeyIdentity>],
) -> TerminalSourceCompilation<'a> {
    TerminalSourceCompilation {
        declaration_metadata: &[],
        entries,
        subjects,
        purpose: DependencyScope::Product,
        selected: &[],
        bindings: &[],
        calling: None,
        providers: None,
        supplies: &[],
    }
}

/// Run the program through the lexer, parser, resolver, typer and the source
/// driver's constant evaluation, then stage 04. `east` is the value of
/// `east\ON`; `west\ON` gets the opposite.
pub fn prepare(east: bool, initializer: Initializer) -> Prepared {
    let package = PackageKeyIdentity::from_digest([0x45; 32]).unwrap();
    let eastern = initializer.module(east);
    let western = initializer.module(!east);
    let mut sources = SourceMap::default();
    let mut syntax = None;
    for (path, text) in [
        ("product/main.omg", MAIN),
        ("product/east/value.omg", eastern.as_str()),
        ("product/west/value.omg", western.as_str()),
    ] {
        let id = sources
            .add_checked_instance(
                path.into(),
                text,
                "product".into(),
                Some(package),
                source_admission::SourceOrigin::User,
                source_admission::SourceResolutionStratum::Base,
                DependencyScope::Product,
            )
            .source_id;
        let tokens = frontend::Lexer::new(text).tokenize().unwrap();
        match &mut syntax {
            Some(syntax) => {
                frontend::parse(syntax, id, &tokens).unwrap();
            }
            None => syntax = Some(frontend::parse_syntax_trees_with_id(id, &tokens).unwrap()),
        }
    }
    let mut request = resolution::ResolutionRequest::new(syntax.unwrap());
    request.sources = Some(std::sync::Arc::new(sources));
    let resolved = resolution::resolve(request).unwrap();
    let typed = typing::lower_symbol_resolved_trees(&resolved).unwrap();
    let entries: Vec<_> = MACHINES
        .iter()
        .map(|name| typed.realized_machine_named(name).unwrap().symbol)
        .collect();
    let subjects = [Some(package)];
    let frozen = prepare_source_for_test(SourcePreparationRequest {
        program: typed,
        accounting: Default::default(),
        admission: Default::default(),
        compilation: compilation(&entries, &subjects),
    })
    .unwrap_or_else(|failure| panic!("preparation refused: {:?}", failure.diagnostics));
    Prepared {
        program: frozen.program,
        entries,
        subjects,
    }
}

/// One place in `main.omg` where a pattern names a constant, as the typed
/// trees hold it: the expression, the path the author wrote there, the
/// declaration selected for it and the value written at the use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstantUse {
    pub expression: ExpressionHandle,
    pub written: &'static str,
    pub selected: SymbolHandle,
    pub value: bool,
}

/// Every expression that carries a constant selection, in source order.
pub fn constant_uses(trees: &TypedTrees) -> Vec<ConstantUse> {
    let mut uses: Vec<_> = trees
        .expression_table
        .iter_expressions()
        .filter_map(|(expression, node)| {
            let selected = trees
                .expression_const_declaration(expression)
                .unwrap()?
                .symbol;
            let span = trees.expression_table.source_span(expression).span;
            let ExpressionNode::Boolean(value) = node else {
                panic!("a Boolean constant use must arrive as its value, found {node:?}")
            };
            Some(ConstantUse {
                expression,
                written: &MAIN[span.start..span.end],
                selected,
                value: *value,
            })
        })
        .collect();
    uses.sort_by_key(|constant| MAIN.find(constant.written));
    uses
}

/// The value stage 04 froze on a constant's declaration.
pub fn declared_value(declaration: &ConstDeclaration) -> bool {
    let encoding = declaration
        .evaluated_value
        .as_ref()
        .and_then(|value| value.encoding())
        .expect("a prepared constant has an evaluated value");
    match decode_declaration_value(encoding) {
        Some(DecodedCanonicalConstValue::Boolean(value)) => value,
        other => panic!("expected a Boolean constant, found {other:?}"),
    }
}
