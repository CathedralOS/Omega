//! A client that reads each source product without linking the compiler stages.
use omega_prepared::PreparedTrees;
use omega_resolved::{NameRole, SymbolResolvedTrees};
use omega_syntax::SyntaxTree;
use omega_tokens::{Punctuation, Token, TokenKind};
use omega_typed::TypedTrees;

pub fn assignment_spelling<'a>(
    source: &'a str,
    tokens: &[Token],
    tree: &SyntaxTree,
) -> Option<&'a str> {
    tree.index(source, tokens)?
        .tokens
        .into_iter()
        .find(|leaf| leaf.kind == TokenKind::Punctuation(Punctuation::Equal))
        .map(|leaf| leaf.text)
}

pub fn resolves_reference_to(
    trees: &SymbolResolvedTrees<'_>,
    machine_name: &str,
) -> bool {
    trees.uses().any(|name| {
        name.role == NameRole::Reference
            && name
                .target
                .declaration_candidates()
                .iter()
                .any(|id| id.path == machine_name)
    })
}

pub fn selected_nullary_call(
    trees: &TypedTrees<'_>,
    machine_name: &str,
) -> bool {
    trees
        .calls
        .iter()
        .any(|call| call.machine.path == machine_name && call.operands.is_empty())
}

pub fn frozen_integer(
    trees: &PreparedTrees<'_>,
    name: &str,
) -> Option<u64> {
    let declaration = trees
        .typed()
        .resolved()
        .declarations
        .iter()
        .find(|id| id.path == name)?;
    trees.integer_constant(declaration)?.try_into().ok()
}
