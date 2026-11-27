//! Runs the files of one package from source text through the lexer, parser,
//! resolver and typer, then through the source driver's preparation, which
//! evaluates the constants and const arguments the request reaches and hands
//! the result to stage 04. The readers below state stage 04's output in terms
//! of the source: a constant by its path, a const argument by the text
//! written for it.
#![allow(dead_code)]

#[path = "../../../../../../../../tests/support/frontend.rs"]
mod frontend;

use application_identity::const_value::{DecodedCanonicalConstValue, decode_declaration_value};
use build_model::values::BuildTimeValue;
use semantic_vocabulary::PackageKeyIdentity;
use source_compiler::representations::source_inputs::SourceMap;
use source_compiler::representations::source_state::TypedTrees;
use source_compiler::representations::source_state::constant::ConstDeclaration;
use source_compiler::test_support::preparation::{
    PreparedTypedTrees, PreparedValue, TargetInputs, ValueInputs,
};
use source_compiler::test_support::{
    SourcePreparationRequest, TerminalSourceCompilation, prepare_source_for_test, resolution,
    typing,
};
use source_completion_accounting::{
    PureEvaluationRecord, PureTerminalEvaluationOutcome, SourceCompletionAccounting,
};
use source_provenance::DependencyScope;
use source_text::SourceSpan;
use symbols::SymbolHandle;

pub const PACKAGE: [u8; 32] = [0x59; 32];

/// What the request asks the two stages for.
#[derive(Debug, Clone, Copy)]
pub enum Demand<'a> {
    /// Every declaration of the package, whether or not anything uses it.
    Package,
    /// These machines and whatever they reach.
    Entries(&'a [&'a str]),
}

pub struct Prepared {
    /// Typing's output, before the source driver evaluated anything.
    pub typed: TypedTrees,
    /// Stage 04's output for the same trees.
    pub program: PreparedTypedTrees,
    /// The evaluations the source driver ran to prepare `program`.
    pub accounting: SourceCompletionAccounting,
    files: Vec<(String, String)>,
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

/// The typed trees of `files`, each `(path, text)` under one package root.
pub fn typed(files: &[(&str, &str)]) -> TypedTrees {
    typed_with_imports(files, &[])
}

fn typed_with_imports(files: &[(&str, &str)], imports: &[(&str, &str, &str)]) -> TypedTrees {
    let package = PackageKeyIdentity::from_digest(PACKAGE).unwrap();
    let mut sources = SourceMap::default();
    let mut syntax = None;
    for &(path, text) in files {
        let id = sources
            .add_checked_instance(
                format!("product/{path}").into(),
                text.to_owned(),
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
                frontend::parse(syntax, id, &tokens)
                    .unwrap_or_else(|error| panic!("{path} does not parse: {error:?}"));
            }
            None => {
                syntax = Some(
                    frontend::parse_syntax_trees_with_id(id, &tokens)
                        .unwrap_or_else(|error| panic!("{path} does not parse: {error:?}")),
                )
            }
        }
    }
    let mut request = resolution::ResolutionRequest::new(syntax.expect("at least one file"));
    request.top_level_bindings = resolution::fixture_file_bindings(&sources, imports);
    request.sources = Some(std::sync::Arc::new(sources));
    let resolved = resolution::resolve(request)
        .unwrap_or_else(|refusal| panic!("resolution refused: {refusal:?}"));
    typing::lower_symbol_resolved_trees(&resolved)
        .unwrap_or_else(|refusal| panic!("typing refused: {refusal:?}"))
}

/// Prepare `files` for `demand`, or return the messages of the refusal with
/// the source text each points at.
pub fn prepare(
    files: &[(&str, &str)],
    demand: Demand<'_>,
) -> Result<Prepared, Vec<(String, String)>> {
    prepare_with_imports(files, demand, &[])
}

pub fn prepare_with_imports(
    files: &[(&str, &str)],
    demand: Demand<'_>,
    imports: &[(&str, &str, &str)],
) -> Result<Prepared, Vec<(String, String)>> {
    let typed = typed_with_imports(files, imports);
    let entries: Vec<_> = match demand {
        Demand::Package => Vec::new(),
        Demand::Entries(names) => names
            .iter()
            .map(|name| {
                typed
                    .realized_machine_named(name)
                    .unwrap_or_else(|| panic!("the program declares no machine `{name}`"))
                    .symbol
            })
            .collect(),
    };
    let package = [Some(PackageKeyIdentity::from_digest(PACKAGE).unwrap())];
    let subjects: &[Option<PackageKeyIdentity>] = match demand {
        Demand::Package => &package,
        Demand::Entries(_) => &[],
    };
    let files: Vec<_> = files
        .iter()
        .map(|&(path, text)| (path.to_owned(), text.to_owned()))
        .collect();
    match prepare_source_for_test(SourcePreparationRequest {
        program: typed.clone(),
        accounting: Default::default(),
        admission: Default::default(),
        compilation: compilation(&entries, subjects),
    }) {
        Ok(frozen) => Ok(Prepared {
            typed,
            program: frozen.program,
            accounting: frozen.accounting,
            files,
        }),
        Err(failure) => Err(failure
            .diagnostics
            .into_iter()
            .map(|diagnostic| {
                let at = diagnostic
                    .source_span
                    .and_then(|span| {
                        files
                            .get(span.source_id.0)
                            .and_then(|file| file.1.get(span.span.start..span.span.end))
                    })
                    .unwrap_or_default()
                    .to_owned();
                (diagnostic.message, at)
            })
            .collect()),
    }
}

/// The inputs stage 04 recorded for one prepared value, in source terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inputs {
    /// The constant's path, or the text written for a const argument.
    pub value: String,
    /// The paths of the constants the value was computed from, sorted.
    pub constants: Vec<String>,
    pub targets: TargetInputs,
}

impl Prepared {
    pub fn path(&self, symbol: SymbolHandle) -> String {
        self.program.trees().symbols.display_path(symbol, "\\")
    }

    /// The source text a span covers.
    pub fn written(&self, span: SourceSpan) -> &str {
        &self.files[span.source_id.0].1[span.span.start..span.span.end]
    }

    /// The path of the file a span lies in, relative to the package root.
    pub fn file_at(&self, span: SourceSpan) -> &str {
        &self.files[span.source_id.0].0
    }

    /// The prepared declaration of the constant at `path`.
    pub fn constant(&self, path: &str) -> &ConstDeclaration {
        let trees = self.program.trees();
        trees
            .const_declarations()
            .iter()
            .find(|declaration| self.path(declaration.symbol) == path)
            .unwrap_or_else(|| panic!("the program declares no constant `{path}`"))
    }

    /// The value stage 04 froze on the constant at `path`, if it has one.
    pub fn value(&self, path: &str) -> Option<DecodedCanonicalConstValue> {
        declared_value(self.constant(path))
    }

    /// The constants stage 04 hands lowering, by path and sorted.
    pub fn constants(&self) -> Vec<String> {
        let mut paths: Vec<_> = self
            .program
            .constants()
            .map(|declaration| self.path(declaration.symbol))
            .collect();
        paths.sort();
        paths
    }

    fn inputs_of(&self, row: &ValueInputs) -> Inputs {
        let trees = self.program.trees();
        let mut constants: Vec<_> = row
            .constants
            .iter()
            .map(|&symbol| self.path(symbol))
            .collect();
        constants.sort();
        Inputs {
            value: match row.value {
                PreparedValue::Constant(symbol) => self.path(symbol),
                PreparedValue::StaticArgument { expression, .. } => self
                    .written(trees.expression_table.source_span(expression))
                    .to_owned(),
            },
            constants,
            targets: row.targets.clone(),
        }
    }

    /// Every value input stage 04 recorded, sorted by what it is the input of.
    pub fn inputs(&self) -> Vec<Inputs> {
        let mut rows: Vec<_> = self
            .program
            .values()
            .iter()
            .map(|row| self.inputs_of(row))
            .collect();
        rows.sort_by(|left, right| left.value.cmp(&right.value));
        rows
    }

    /// The inputs recorded for the constant at `path` or the const argument
    /// written as `value`.
    pub fn inputs_for(&self, value: &str) -> Inputs {
        let mut found = self.inputs().into_iter().filter(|row| row.value == value);
        let row = found
            .next()
            .unwrap_or_else(|| panic!("no prepared value is `{value}`"));
        assert!(
            found.next().is_none(),
            "`{value}` is prepared more than once"
        );
        row
    }

    /// The evaluations the source driver ran: who asked, and what came back.
    /// The requester is a constant's path, or the path of the declaration
    /// that holds a const argument. Sorted.
    pub fn evaluations(&self) -> Vec<(String, BuildTimeValue)> {
        let mut rows: Vec<_> = self
            .accounting
            .receipts()
            .iter()
            .map(|record| {
                let PureEvaluationRecord::Invocation(receipt) = record else {
                    panic!("expected a machine invocation, found {record:?}")
                };
                let PureTerminalEvaluationOutcome::Materialized { value, .. } = receipt.outcome()
                else {
                    panic!("the evaluation returned no value: {:?}", receipt.outcome())
                };
                (self.path(receipt.requester()), value.clone())
            })
            .collect();
        rows.sort_by(|left, right| left.0.cmp(&right.0));
        rows
    }
}

/// The value recorded on a constant's declaration, if it has been evaluated.
pub fn declared_value(declaration: &ConstDeclaration) -> Option<DecodedCanonicalConstValue> {
    let encoding = declaration.evaluated_value.as_ref()?.encoding()?;
    Some(
        decode_declaration_value(encoding)
            .unwrap_or_else(|| panic!("`{encoding}` is not a canonical constant value")),
    )
}

/// The integer a decoded constant holds.
pub fn integer(value: Option<DecodedCanonicalConstValue>) -> Option<i64> {
    match value? {
        DecodedCanonicalConstValue::Integer { value, .. } => value.to_i64(),
        other => panic!("expected an integer constant, found {other:?}"),
    }
}
