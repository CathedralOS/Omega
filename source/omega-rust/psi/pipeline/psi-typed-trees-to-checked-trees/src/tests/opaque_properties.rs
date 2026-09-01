use super::*;

const COPY_OPAQUE: &str = r#"
boundary data Token [copy];
data Main {}
machine Main::main(&mut self) {}
"#;

fn typed(source: &str) -> psi_typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

fn data_symbol(program: &psi_typed_trees::TypedTrees, name: &str) -> SymbolHandle {
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == name)
        .unwrap_or_else(|| panic!("missing data `{name}`"))
        .symbol
}

fn rendered(diagnostics: Vec<psi_diagnostics::Diagnostic>) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn ordinary_lowering_rejects_copyable_opaque_without_receipt() {
    let diagnostics = crate::lower_typed_trees(typed(COPY_OPAQUE))
        .expect_err("ordinary Psi lowering must not mint an opaque copy receipt");
    assert!(rendered(diagnostics).contains("without an admitted property receipt"));
}

#[test]
fn exact_opaque_copy_receipt_is_consumed_once() {
    let program = typed(COPY_OPAQUE);
    let receipt = psi_validation::OpaqueDataPropertyReceipt::copy(data_symbol(&program, "Token"));
    crate::lower_typed_trees_with_selected_generic_operator_providers(program, &[], &[receipt])
        .expect("the exact orchestration receipt should close opaque copy validation");
}

#[test]
fn duplicate_and_wrong_declaration_receipts_reject() {
    let duplicate = typed(COPY_OPAQUE);
    let receipt = psi_validation::OpaqueDataPropertyReceipt::copy(data_symbol(&duplicate, "Token"));
    let diagnostics = crate::lower_typed_trees_with_selected_generic_operator_providers(
        duplicate,
        &[],
        &[receipt, receipt],
    )
    .expect_err("duplicate opaque property receipts must reject");
    assert!(rendered(diagnostics).contains("repeat one exact declaration"));

    let wrong = typed(COPY_OPAQUE);
    let receipt = psi_validation::OpaqueDataPropertyReceipt::copy(data_symbol(&wrong, "Main"));
    let diagnostics =
        crate::lower_typed_trees_with_selected_generic_operator_providers(wrong, &[], &[receipt])
            .expect_err("a transparent declaration cannot receive an opaque property receipt");
    assert!(rendered(diagnostics).contains("targets non-opaque declaration"));
}

#[test]
fn copy_receipt_rejects_an_opaque_that_does_not_claim_copy() {
    let program = typed(
        r#"
boundary data Token;
data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let receipt = psi_validation::OpaqueDataPropertyReceipt::copy(data_symbol(&program, "Token"));
    let diagnostics =
        crate::lower_typed_trees_with_selected_generic_operator_providers(program, &[], &[receipt])
            .expect_err("a copy receipt must match the declaration's exact property claim");
    assert!(rendered(diagnostics).contains("does not claim `[copy]`"));
}
