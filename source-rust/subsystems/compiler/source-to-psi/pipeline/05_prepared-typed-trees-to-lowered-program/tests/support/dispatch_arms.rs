//! Programs for testing what each stage hands the next about dispatch arms:
//! `MAIN` and `ESCAPE` for block-valued arms, `GUARDED` for a `match`
//! without a subject.
//!
//! `MAIN`: `pick` binds a block local and returns it:
//! only the block's own statement says the arm's value is 41. `scoped` builds
//! a record inside its block and reads it there, so the record's storage
//! must end where the arm joins its sibling. `escape` returns a loan of such
//! a record out of its block, which outlives the storage it names.
//!
//! The rules: `wiki/spec/language/patterns.md`, "a comma separates each arm
//! from the next, including after a block-valued arm", and
//! `wiki/spec/language/expressions.md`, "A block introduces local scope and
//! cleanup edges, not a new machine declaration".
#![allow(dead_code)]

#[path = "../../../../../../../../tests/support/frontend.rs"]
mod frontend;

use semantic_vocabulary::PackageKeyIdentity;
use source_compiler::representations::source_inputs::SourceMap;
use source_compiler::representations::source_state::TypedTrees;
use source_compiler::representations::source_state::expression::{
    ExpressionHandle, ExpressionNode, TableBlockExpression, TableMatchExpression,
};
use source_compiler::representations::source_state::statement::StatementNode;
use source_compiler::test_support::lowering::{self, Elaboration};
use source_compiler::test_support::preparation::{self, PreparedTypedTrees};
use source_compiler::test_support::{
    SourcePreparationRequest, TerminalSourceCompilation, prepare_source_for_test, resolution,
    typing,
};
use source_provenance::DependencyScope;
use symbols::SymbolHandle;
use terminal_check_tables::compilation::{PreparedCompilation, TerminalCompilation};
use terminal_psi::TerminalMachine;

pub const MAIN: &str = r"data Counter { value: u32; }
machine read(counter: @Counter) -> u32 { counter.value }
machine pick(code: u32) -> u32 {
    match code { 0 -> { let port: u32 = 41; port }, _ -> 7, }
}
machine scoped(code: u32) -> u32 {
    let seen: u32 = match code { 0 -> { let made: Counter = Counter { value: 3 }; read(@made) }, _ -> 4, };
    seen
}";

/// `escape` is refused, so it is a package of its own: every declaration
/// of a package is checked with it.
pub const ESCAPE: &str = r"data Counter { value: u32; }
machine read(counter: @Counter) -> u32 { counter.value }
machine escape(code: u32, other: @Counter) -> u32 {
    let view: @Counter = match code { 0 -> { let made: Counter = Counter { value: 3 }; @made }, _ -> other, };
    read(view)
}";

/// Matches without a subject: each arm's condition reads the parameters, and
/// `_` closes the ladder (`wiki/spec/language/patterns.md`, "Bare names in
/// dispatch arms" and "Coverage and domain tests"). In `gap`, only the
/// condition `a > b` proves that `a - b` cannot go below zero.
pub const GUARDED: &str = r"machine larger(a: u32, b: u32) -> u32 {
    match { a > b -> a, _ -> b }
}
machine gap(a: u32, b: u32) -> u32 {
    match { a > b -> a - b, _ -> 0 }
}";

/// Block-valued arms holding `crash` and `transition`:
/// `guarded_crash` crashes only when `code == 0`, so `crashes Trap code == 0`
/// covers it; `early_exit` builds an owned record in the enclosing machine and
/// another in the block arm, then leaves the enclosing machine via `transition`.
pub const CONTROL_FLOW: &str = r"data Counter { value: u32; }
machine read(counter: @Counter) -> u32 { counter.value }
machine guarded_crash(code: u32) -> u32
crashes Trap code == 0
{
    let seen: u32 = match code {
        0 -> { let made: Counter = Counter { value: 3 }; crash Trap; },
        _ -> 7,
    };
    seen
}
machine early_exit(code: u32, inner: u32) -> u32 {
    let outer: Counter = Counter { value: 10 };
    let seen: u32 = match code {
        0 -> {
            let made: Counter = Counter { value: inner };
            transition made.value {
                0 -> 41,
                _ -> 42,
            }
        },
        _ -> 7,
    };
    read(@outer) + seen
}";

pub const UNCOVERED_CRASH: &str = r"data Counter { value: u32; }
machine uncovered_crash(code: u32) -> u32
crashes none
{
    let seen: u32 = match code {
        0 -> { let made: Counter = Counter { value: 3 }; crash Trap; },
        _ -> 7,
    };
    seen
}";

pub const WRONG_GUARD_CRASH: &str = r"data Counter { value: u32; }
machine wrong_guard_crash(code: u32) -> u32
crashes Trap code == 1
{
    let seen: u32 = match code {
        0 -> { let made: Counter = Counter { value: 3 }; crash Trap; },
        _ -> 7,
    };
    seen
}";

pub const TRAILING_AFTER_CRASH: &str = r"machine trailing(code: u32) -> u32
crashes Trap code == 0
{
    match code {
        0 -> { crash Trap; 5 },
        _ -> 7,
    }
}";

const PACKAGE: [u8; 32] = [0x42; 32];

/// The typed trees of `MAIN`, after the lexer, parser, resolver and typer.
pub fn typed() -> TypedTrees {
    typed_source(MAIN)
}

/// The typed trees of `text`.
pub fn typed_source(text: &'static str) -> TypedTrees {
    let package = PackageKeyIdentity::from_digest(PACKAGE).unwrap();
    let mut sources = SourceMap::default();
    let id = sources
        .add_checked_instance(
            "product/main.omg".into(),
            text,
            "product".into(),
            Some(package),
            source_admission::SourceOrigin::User,
            source_admission::SourceResolutionStratum::Base,
            DependencyScope::Product,
        )
        .source_id;
    let tokens = frontend::Lexer::new(text).tokenize().unwrap();
    let syntax = frontend::parse_syntax_trees_with_id(id, &tokens).unwrap();
    let mut request = resolution::ResolutionRequest::new(syntax);
    request.sources = Some(std::sync::Arc::new(sources));
    let resolved = resolution::resolve(request).unwrap();
    typing::lower_symbol_resolved_trees(&resolved).unwrap()
}

/// The block whose statements bind the local `name`, and its handle.
pub fn block_binding(trees: &TypedTrees, name: &str) -> (ExpressionHandle, TableBlockExpression) {
    let mut found = trees
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, node)| match node {
            ExpressionNode::Block(block) => Some((handle, *block)),
            _ => None,
        })
        .filter(|(_, block)| {
            trees
                .statement_table
                .statements(block.statements)
                .iter()
                .filter_map(StatementNode::as_local_data)
                .any(|local| local.name.as_str() == name)
        });
    let block = found
        .next()
        .unwrap_or_else(|| panic!("typing kept no block binding `{name}`"));
    assert!(found.next().is_none(), "one block binds `{name}`");
    block
}

/// The matches without a subject, in source order, with their handles.
pub fn subjectless_matches(trees: &TypedTrees) -> Vec<(ExpressionHandle, TableMatchExpression)> {
    trees
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, node)| match node {
            ExpressionNode::Match(matched)
                if !trees.expression_table.expression_is_valid(matched.subject) =>
            {
                Some((handle, *matched))
            }
            _ => None,
        })
        .collect()
}

pub struct Prepared {
    pub program: PreparedTypedTrees,
    entries: Vec<SymbolHandle>,
    subjects: [Option<PackageKeyIdentity>; 1],
}

impl Prepared {
    pub fn compilation(&self) -> TerminalSourceCompilation<'_> {
        TerminalSourceCompilation {
            declaration_metadata: &[],
            entries: &self.entries,
            subjects: &self.subjects,
            purpose: DependencyScope::Product,
            selected: &[],
            bindings: &[],
            calling: None,
            providers: None,
            supplies: &[],
        }
    }

    /// Stage 05's output for `program`, under this request.
    pub fn lower_program(&self, program: &PreparedTypedTrees) -> PreparedCompilation {
        let lowered = lowering::prepare_compilation(
            program,
            self.compilation(),
            terminal_codec::TerminalPublication::Portable,
        )
        .unwrap();
        let TerminalCompilation::Prepared(lowered) = lowered.program else {
            panic!("the program binds no callable shape that could fail to form")
        };
        lowered
    }

    /// Stage 05's output for the unchanged program, which must lower.
    pub fn lower(&self) -> PreparedCompilation {
        let lowered = self.lower_program(&self.program);
        assert!(lowered.closure.is_ok(), "{:?}", lowered.closure);
        lowered
    }

    /// Stage 04's product for the same trees after `change`.
    pub fn prepared_with(&self, change: impl FnOnce(&mut TypedTrees)) -> PreparedTypedTrees {
        let values = self.program.values().to_vec();
        let mut trees = self.program.clone().into_inner();
        change(&mut trees);
        preparation::prepare_typed_trees(trees, &values, &[]).unwrap()
    }
}

/// `MAIN` through stage 04, with `entries` as the request's entries.
pub fn prepare(entries: &[&str]) -> Prepared {
    prepare_source(MAIN, entries)
}

/// `text` through stage 04, with `entries` as the request's entries.
pub fn prepare_source(text: &'static str, entries: &[&str]) -> Prepared {
    let typed = typed_source(text);
    let entries: Vec<_> = entries
        .iter()
        .map(|name| typed.realized_machine_named(name).unwrap().symbol)
        .collect();
    let subjects = [Some(PackageKeyIdentity::from_digest(PACKAGE).unwrap())];
    let frozen = prepare_source_for_test(SourcePreparationRequest {
        program: typed,
        accounting: Default::default(),
        admission: Default::default(),
        compilation: TerminalSourceCompilation {
            declaration_metadata: &[],
            entries: &entries,
            subjects: &subjects,
            purpose: DependencyScope::Product,
            selected: &[],
            bindings: &[],
            calling: None,
            providers: None,
            supplies: &[],
        },
    })
    .unwrap_or_else(|failure| panic!("preparation refused: {:?}", failure.diagnostics));
    Prepared {
        program: frozen.program,
        entries,
        subjects,
    }
}

/// The lowered machine the source names `name`.
pub fn machine<'a>(
    prepared: &Prepared,
    lowered: &'a Elaboration,
    name: &str,
) -> &'a TerminalMachine {
    let symbols = &prepared.program.trees().symbols;
    lowered
        .module
        .machines
        .iter()
        .find(|machine| symbols.name(*lowered.machines.get(machine.id).unwrap()) == name)
        .unwrap_or_else(|| panic!("stage 05 lowered no machine named `{name}`"))
}
