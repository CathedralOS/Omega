//! Checked mathematical declarations: the proof-surface records a top-level
//! `let`/`boundary let` elaborates into (PROOF-CONTRACT-MIGRATION).

use checked_trees::CheckedMathematicalDeclaration;

/// Build the checked mathematical-declaration surface for a typed program.
///
/// The parse/declarations leg records top-level `let`/`boundary let` items on
/// `SyntaxTreeRoots::mathematical_definitions` and resolution refuses them
/// explicitly, so no `TypedTrees` input reaches this stage yet and the list
/// stays empty. The leg that lands typed mathematical declarations fills this
/// builder — its output contract is [`CheckedMathematicalDeclaration`]:
/// universe binders in declaration order, the ordered ordinary telescope, the
/// statement's result, and either a transparent definition term or the
/// named-assumption absence a `boundary let` declares.
pub(crate) fn build_checked_mathematical_declarations(
    program: &typed_trees::TypedTrees,
) -> Vec<CheckedMathematicalDeclaration> {
    let _ = program;
    Vec::new()
}
