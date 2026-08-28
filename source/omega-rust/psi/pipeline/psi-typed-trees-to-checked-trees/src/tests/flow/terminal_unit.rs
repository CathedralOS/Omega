use super::*;
use psi_checked_trees::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole,
};

mod calls;
mod cleanup;
mod returns;
mod shared_convergence;

use psi_checked_trees::{
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralFieldPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypeShape,
};
use psi_language_core::BindingRelevance;
use psi_language_semantics::Multiplicity;
use psi_typed_trees::types::PrimitiveType;

fn checked(source: &str) -> psi_checked_trees::CheckedTrees {
    let source = format!("boundary trait PortIo {{}}\n{source}");
    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn contextual_cleanup_diagnostics(source: &str) -> Vec<psi_diagnostics::Diagnostic> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed)
        .expect_err("contextual cleanup requirement-set mismatch must reject at its return edge")
}

fn machine_named(
    checked: &psi_checked_trees::CheckedTrees,
    name: &str,
) -> psi_symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| {
            machine.name.as_str() == name || machine.name.as_str().ends_with(&format!("::{name}"))
        })
        .unwrap_or_else(|| panic!("missing machine `{name}`"))
        .symbol
}

fn record_fields(
    shape: &psi_checked_trees::CheckedUnitStructuralTypePlan,
) -> &[CheckedUnitStructuralFieldPlan] {
    let CheckedUnitStructuralTypeShape::Record { fields } = &shape.shape else {
        panic!("expected record structural shape")
    };
    fields
}
