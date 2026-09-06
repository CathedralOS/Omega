use super::*;
use checked_trees::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole,
};

mod boundary_result_operands;
mod calls;
mod cleanup;
mod composed_call_arguments;
mod composed_claims;
mod composed_internal_calls;
mod composed_nested_control;
mod composed_prefixed_control;
mod composed_transitive_internal_calls;
mod free_scalar_parameters;
mod nested_boundary_results;
mod receiver_stores;
mod returns;
mod scalar_boundary_targets;
mod scalar_sequences;
mod shared_convergence;
mod tail_calls;

use checked_trees::{
    CheckedBoundaryMachineResultPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralFieldPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypeShape,
};
use language_core::BindingRelevance;
use language_semantics::Multiplicity;
use typed_trees::types::PrimitiveType;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let source = format!("boundary trait PortIo {{}}\n{source}");
    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn contextual_cleanup_diagnostics(source: &str) -> Vec<diagnostics::Diagnostic> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed)
        .expect_err("contextual cleanup requirement-set mismatch must reject at its return edge")
}

fn machine_named(checked: &checked_trees::CheckedTrees, name: &str) -> symbols::SymbolHandle {
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
    shape: &checked_trees::CheckedUnitStructuralTypePlan,
) -> &[CheckedUnitStructuralFieldPlan] {
    let CheckedUnitStructuralTypeShape::Record { fields } = &shape.shape else {
        panic!("expected record structural shape")
    };
    fields
}
