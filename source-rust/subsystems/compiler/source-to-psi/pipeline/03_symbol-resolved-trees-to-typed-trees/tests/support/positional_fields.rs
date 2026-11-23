//! One program for testing which field typing gives each place of a
//! positional pattern. `Pair` declares `zeta` first and `alpha` second, so
//! declaration order is neither alphabetical nor the order of the bindings'
//! names.
//!
//! `by_place` binds place 0 under the name `alpha`. The only thing that says
//! it reads `zeta` is the place: a compiler that looked the binding's name
//! up as a field would read the field `alpha` instead and return the other
//! value. `with_rest` ends in `..`, `in_arm` writes the pattern in a match
//! arm, and `by_name` is the named pattern the others must agree with.
#![allow(dead_code)]

#[path = "../../../../../../../../tests/support/frontend.rs"]
mod frontend;

use semantic_vocabulary::PackageKeyIdentity;
use source_compiler::representations::source_inputs::SourceMap;
use source_compiler::representations::source_state::TypedTrees;
use source_compiler::representations::source_state::expression::{
    ExpressionHandle, ExpressionNode, MatchFieldAction, MatchPattern, MemberSelection,
    TableMatchField,
};
use source_compiler::representations::source_state::statement::StatementNode;
use source_compiler::test_support::{resolution, typing};
use source_provenance::DependencyScope;
use symbols::SymbolHandle;

pub const MAIN: &str = r"data Pair [copy] { zeta: u8; alpha: u8; }
machine by_place(pair: Pair) -> u8 {
    let (alpha, _) = pair;
    alpha
}
machine with_rest(pair: Pair) -> u8 {
    let (first, ..) = pair;
    first
}
machine in_arm(pair: Pair) -> u8 {
    match pair { (_, second) -> second, }
}
machine by_name(pair: Pair) -> u8 {
    let Pair { zeta as _, alpha as named } = pair;
    named
}";

pub const MACHINES: [&str; 4] = ["by_place", "with_rest", "in_arm", "by_name"];

pub const REFERENCED: &str = include_str!(
    "../../../../tests/fixtures/pattern_field_identity/pass/borrowed_and_projected/main.omg"
);

pub const PACKAGE: [u8; 32] = [0x50; 32];

/// The source map and the typed trees of `text`, or typing's refusal as the
/// message and the source text it points at.
pub fn typed(text: &'static str) -> Result<(SourceMap, TypedTrees), (String, &'static str)> {
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
    let shared = std::sync::Arc::new(sources);
    request.sources = Some(shared.clone());
    let resolved = resolution::resolve(request).unwrap();
    match typing::lower_symbol_resolved_trees(&resolved) {
        Ok(typed) => Ok((std::sync::Arc::unwrap_or_clone(shared), typed)),
        Err(diagnostic) => {
            let span = diagnostic
                .source_span
                .unwrap_or_else(|| panic!("the refusal names no place: {diagnostic:?}"))
                .span;
            Err((diagnostic.message, &text[span.start..span.end]))
        }
    }
}

/// The symbol of the field `Pair` declares under `name`.
pub fn field(trees: &TypedTrees, name: &str) -> SymbolHandle {
    let pair = trees
        .data_definition_by_name_str("Pair")
        .expect("the program declares `Pair`");
    trees
        .data_members(pair)
        .iter()
        .filter_map(|member| member.as_field())
        .find(|field| field.name.as_str() == name)
        .unwrap_or_else(|| panic!("`Pair` declares no field `{name}`"))
        .symbol
}

/// One field read as the typed trees hold it: the expression, the source
/// text it stands for, the field's name on the member and the field symbol
/// typing selected. A positional read stands for the component or the use
/// that produced it. The read a named `let` pattern generates has no source
/// text of its own, and `written` is then empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldRead {
    pub expression: ExpressionHandle,
    pub written: &'static str,
    pub member: String,
    pub selected: SymbolHandle,
}

/// The one field read each machine of `MACHINES` returns, in that order:
/// the initializer of the local a `let` pattern binds, and the value of the
/// match arm. Typing may hold further copies of a read that nothing
/// executes; these are the ones the machines use.
pub fn field_reads(trees: &TypedTrees) -> Vec<FieldRead> {
    let read = |expression: ExpressionHandle| {
        let ExpressionNode::Member(member) = trees.expression_table.expression(expression) else {
            return None;
        };
        let span = trees.expression_table.source_span(expression).span;
        let MemberSelection::Field(selected) = member.selection else {
            panic!("typing left a member without its field: {member:?}")
        };
        Some(FieldRead {
            expression,
            written: &MAIN[span.start..span.end],
            member: member.member.as_str().to_owned(),
            selected,
        })
    };
    let arm = trees
        .expression_table
        .iter_expressions()
        .filter_map(|(_, node)| match node {
            ExpressionNode::Match(matched) => Some(matched),
            _ => None,
        })
        .flat_map(|matched| trees.expression_table.match_arms(matched.arms))
        .find_map(|arm| read(arm.value));
    MACHINES
        .iter()
        .map(|name| {
            let machine = trees.realized_machine_named(name).unwrap();
            trees
                .machine_states(machine)
                .iter()
                .flat_map(|state| state.statement_nodes.handles())
                .find_map(|handle| match trees.statement_table.statement(handle) {
                    StatementNode::LocalData(local)
                        if local.role == language_core::LocalBindingRole::Value =>
                    {
                        read(local.initial_value)
                    }
                    _ => None,
                })
                .or_else(|| arm.clone())
                .unwrap_or_else(|| panic!("`{name}` reads no field"))
        })
        .collect()
}

/// One pattern row as typing wrote it: the field's name, the field symbol
/// when the row carries one, and the name it binds.
pub type Row = (String, SymbolHandle, Option<String>);

fn rows(fields: &[TableMatchField]) -> Vec<Row> {
    fields
        .iter()
        .map(|row| {
            (
                row.member.as_str().to_owned(),
                row.field_symbol,
                match &row.action {
                    MatchFieldAction::Bind(binding) => Some(binding.as_str().to_owned()),
                    MatchFieldAction::Waive => None,
                    MatchFieldAction::Equal(_) => panic!("no pattern here tests a value"),
                },
            )
        })
        .collect()
}

/// The one `let` pattern in the machine `name`.
pub fn let_pattern_handle(
    trees: &TypedTrees,
    name: &str,
) -> source_compiler::representations::source_state::statement::StatementHandle {
    let machine = trees.realized_machine_named(name).unwrap();
    let mut markers = trees
        .machine_states(machine)
        .iter()
        .flat_map(|state| trees.body_statement_spans(state.statement_nodes))
        .flat_map(|span| span.handles())
        .filter(|&handle| {
            matches!(
                trees.statement_table.statement(handle),
                StatementNode::LocalData(local)
                    if local.role == language_core::LocalBindingRole::RecordPattern
            )
        });
    let marker = markers
        .next()
        .unwrap_or_else(|| panic!("`{name}` has no pattern"));
    assert!(markers.next().is_none(), "`{name}` has one pattern");
    marker
}

pub fn let_pattern<'trees>(
    trees: &'trees TypedTrees,
    name: &str,
) -> &'trees source_compiler::representations::source_state::statement::TableLocalData {
    trees
        .statement_table
        .statement(let_pattern_handle(trees, name))
        .as_local_data()
        .unwrap()
}

/// The roster of the one `let` pattern in the machine `name`.
pub fn let_roster(trees: &TypedTrees, name: &str) -> Vec<Row> {
    rows(
        trees
            .expression_table
            .match_fields(let_pattern(trees, name).pattern_fields),
    )
}

/// The roster and `has_rest` of the one destructuring match arm in `MAIN`.
pub fn arm_roster(trees: &TypedTrees) -> (Vec<Row>, bool) {
    let mut arms = trees
        .expression_table
        .iter_expressions()
        .filter_map(|(_, node)| match node {
            ExpressionNode::Match(matched) => Some(matched),
            _ => None,
        })
        .flat_map(|matched| trees.expression_table.match_arms(matched.arms))
        .filter_map(|arm| match &arm.pattern {
            MatchPattern::Destructure(pattern) => Some(pattern),
            _ => None,
        });
    let pattern = arms.next().expect("the program has a destructuring arm");
    assert!(arms.next().is_none(), "the program has one such arm");
    (
        rows(trees.expression_table.match_fields(pattern.fields)),
        pattern.has_rest,
    )
}
