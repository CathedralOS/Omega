//! A client that reads each source product without linking the compiler stages.
pub mod obligation_report;
pub mod resource_report;
use omega_prepared::PreparedTrees;
use omega_resolved::{DeclarationId, NameRole, SymbolResolvedTrees};
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
    machine: &DeclarationId,
) -> bool {
    trees
        .uses()
        .any(|name| name.role == NameRole::Reference && name.target.declaration_candidates().contains(machine))
}

pub fn selected_nullary_call(
    trees: &TypedTrees<'_>,
    machine: &DeclarationId,
) -> bool {
    trees
        .calls
        .iter()
        .any(|call| &call.machine == machine && call.operands.is_empty())
}

pub fn frozen_integer(
    trees: &PreparedTrees<'_>,
    declaration: &DeclarationId,
) -> Option<u64> {
    trees.integer_constant(declaration)?.try_into().ok()
}
