//! One program for testing how an array extent that names a constant crosses
//! resolution, typing and the source driver's evaluation into stage 04, and
//! what stage 05 does with it.
//!
//! Two modules each declare a public constant named `N`, with different
//! values. `choose` takes an array of `|east|::N` bytes and passes it to `pair`,
//! which requires two. `other` takes an array of `|west|::N` bytes and passes it
//! to `triple`, which requires three. `plain` writes its extent as the
//! literal `2` and names no constant.
//!
//! Only the qualified path tells the two constants apart, so a compiler that
//! picked a constant by its last name would give one machine the other's
//! extent. With `|east|::N` at 2 and `|west|::N` at 3 the program is valid; any
//! other pairing passes an array of the wrong length.
#![allow(dead_code)]

#[path = "../../../../../../../../tests/support/frontend.rs"]
mod frontend;

use application_identity::const_value::{DecodedCanonicalConstValue, decode_declaration_value};
use semantic_vocabulary::PackageKeyIdentity;
use source_compiler::representations::source_inputs::SourceMap;
use source_compiler::representations::source_state::TypedTrees;
use source_compiler::representations::source_state::constant::ConstDeclaration;
use source_compiler::representations::source_state::constant_recipes::DeclaredArrayExtent;
use source_compiler::representations::source_state::types::{
    FixedArrayLength, TypeReferenceHandle, TypeReferenceNode,
};
use source_compiler::test_support::preparation::PreparedTypedTrees;
use source_compiler::test_support::{
    SourcePreparationRequest, TerminalSourceCompilation, prepare_source_for_test, resolution,
    typing,
};
use source_completion_accounting::SourceCompletionAccounting;
use source_provenance::DependencyScope;
use symbols::SymbolHandle;

pub const MAIN: &str = r"use east/value.omg as east;
use west/value.omg as west;
machine pair(values: @[u8; 2]) -> u8 { values[0u64] }
machine triple(values: @[u8; 3]) -> u8 { values[0u64] }
machine choose(values: @[u8; |east|::N]) -> u8 { pair(values) }
machine other(values: @[u8; |west|::N]) -> u8 { triple(values) }
machine plain(values: @[u8; 2]) -> u8 { pair(values) }";

/// The machines that hold an extent under test, in source order.
pub const MACHINES: [&str; 3] = ["choose", "other", "plain"];

/// The argument each machine passes on, as the lowered-form refusal reports
/// it: the `values` inside `pair(values)` or `triple(values)`.
pub fn passed_argument(machine: &str) -> std::ops::Range<usize> {
    let declaration = MAIN.find(&format!("machine {machine}(")).unwrap();
    let call = declaration + MAIN[declaration..].find("(values) }").unwrap();
    call + 1..call + 1 + "values".len()
}

/// How each module writes its constant. Resolution records the value of a
/// literal on its declaration. A call has no value until the
/// source driver runs it, which happens after typing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Initializer {
    Literal,
    Call,
}

pub const INITIALIZERS: [Initializer; 2] = [Initializer::Literal, Initializer::Call];

impl Initializer {
    fn module(self, value: u64) -> String {
        match self {
            Self::Literal => format!("public const N: u64 = {value}u64;"),
            Self::Call => format!(
                "public const N: u64 = extent();
                machine extent() -> u64 crashes none {{ {value}u64 }}"
            ),
        }
    }
}

pub struct Prepared {
    /// Typing's output, before the source driver evaluated any constant.
    pub typed: TypedTrees,
    /// Stage 04's output for the same trees.
    pub program: PreparedTypedTrees,
    /// The evaluations the source driver ran to prepare `program`.
    pub accounting: SourceCompletionAccounting,
}

/// A request for `entries` alone. It names no package subject, so the two
/// stages read a constant only when one of the entries reaches it.
pub fn compilation(entries: &[SymbolHandle]) -> TerminalSourceCompilation<'_> {
    TerminalSourceCompilation {
        declaration_metadata: &[],
        entries,
        subjects: &[],
        purpose: DependencyScope::Product,
        selected: &[],
        bindings: &[],
        calling: None,
        providers: None,
        supplies: &[],
    }
}

pub fn machine(trees: &TypedTrees, name: &str) -> SymbolHandle {
    trees.realized_machine_named(name).unwrap().symbol
}

pub fn machines(trees: &TypedTrees, names: &[&str]) -> Vec<SymbolHandle> {
    names.iter().map(|name| machine(trees, name)).collect()
}

/// Run the program through the lexer, parser, resolver and typer, then
/// through the source driver's preparation for `entries`, which evaluates the
/// constants those machines reach and hands the result to stage 04.
pub fn prepare(east: u64, west: u64, initializer: Initializer, entries: &[&str]) -> Prepared {
    let package = PackageKeyIdentity::from_digest([0x58; 32]).unwrap();
    let eastern = initializer.module(east);
    let western = initializer.module(west);
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
    request.top_level_bindings = resolution::fixture_file_bindings(
        &sources,
        &[
            ("product/main.omg", "product/east/value.omg", "east"),
            ("product/main.omg", "product/west/value.omg", "west"),
        ],
    );
    request.sources = Some(std::sync::Arc::new(sources));
    let resolved = resolution::resolve(request).unwrap();
    let typed = typing::lower_symbol_resolved_trees(&resolved).unwrap();
    let entries = machines(&typed, entries);
    let frozen = prepare_source_for_test(SourcePreparationRequest {
        program: typed.clone(),
        accounting: Default::default(),
        admission: Default::default(),
        compilation: compilation(&entries),
    })
    .unwrap_or_else(|failure| panic!("preparation refused: {:?}", failure.diagnostics));
    Prepared {
        typed,
        program: frozen.program,
        accounting: frozen.accounting,
    }
}

/// One array type in `main.omg` whose extent names a constant, as the typed
/// trees hold it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtentUse {
    pub array: TypeReferenceHandle,
    /// The path the author wrote as the extent.
    pub written: &'static str,
    /// The declaration typing recorded for that path.
    pub selected: SymbolHandle,
    /// How the array type states its extent.
    pub form: FixedArrayLength,
    /// The length a reader of the array type gets, if it has one yet.
    pub length: Option<u64>,
}

/// Every array type whose extent names a constant, in source order. An array
/// with a literal extent, such as `[u8; 2]`, records no constant and is not
/// among them.
pub fn extent_uses(trees: &TypedTrees) -> Vec<ExtentUse> {
    let types = &trees.type_reference_table;
    let mut uses: Vec<_> = types
        .type_reference_rows()
        .filter_map(|(array, node)| {
            let TypeReferenceNode::FixedArray { length, .. } = node else {
                return None;
            };
            let [origin] = types.constant_origins(array).unwrap() else {
                return None;
            };
            let span = origin.reference.span;
            Some(ExtentUse {
                array,
                written: &MAIN[span.start..span.end],
                selected: origin.declaration,
                form: length.clone(),
                length: trees
                    .declared_array_extent(array)
                    .and_then(DeclaredArrayExtent::literal),
            })
        })
        .collect();
    uses.sort_by_key(|extent| MAIN.find(extent.written));
    uses
}

/// The lengths of the arrays that name no constant, sorted.
pub fn literal_extents(trees: &TypedTrees) -> Vec<usize> {
    let types = &trees.type_reference_table;
    let mut lengths: Vec<_> = types
        .type_reference_rows()
        .filter_map(|(array, node)| match node {
            TypeReferenceNode::FixedArray {
                length: FixedArrayLength::Literal(length),
                ..
            } if types.constant_origins(array).unwrap().is_empty() => Some(*length),
            _ => None,
        })
        .collect();
    lengths.sort_unstable();
    lengths
}

pub fn path(trees: &TypedTrees, symbol: SymbolHandle) -> String {
    let span = trees
        .symbols
        .symbol_source_span(symbol)
        .expect("constant declaration span");
    let sources = trees.symbols.source_map().unwrap();
    let file = sources.file_at(span).unwrap();
    format!(
        "{}:{}",
        file.logical_filename().unwrap(),
        trees.symbols.name(symbol)
    )
}

/// The value recorded on a constant's declaration, if it has been evaluated.
pub fn declared_value(declaration: &ConstDeclaration) -> Option<u64> {
    let encoding = declaration.evaluated_value.as_ref()?.encoding()?;
    match decode_declaration_value(encoding) {
        Some(DecodedCanonicalConstValue::Integer { value, .. }) => value.to_u64(),
        other => panic!("expected an integer constant, found {other:?}"),
    }
}
