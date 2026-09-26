use super::SymbolHandle;
use crate::tests::front_end::typed_program;
const COPY_OPAQUE: &str = r#"
boundary data Token [copy];
data Main {}
machine Main::main(&mut self) {}
"#;

fn data_symbol(program: &typed_trees::TypedTrees, name: &str) -> SymbolHandle {
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == name)
        .unwrap_or_else(|| panic!("missing data `{name}`"))
        .symbol
}

fn rendered(diagnostics: Vec<diagnostics::Diagnostic>) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn duplicate_and_wrong_declaration_receipts_reject() {
    let duplicate = typed_program(COPY_OPAQUE);
    let receipt = validation::OpaqueDataPropertyReceipt::copy(data_symbol(&duplicate, "Token"));
    let diagnostics = crate::lower_typed_trees(
        duplicate,
        &crate::CheckingRequest::settled().with_opaque_property_receipts(&[receipt, receipt]),
    )
    .expect_err("duplicate opaque property receipts must reject");
    assert!(rendered(diagnostics).contains("repeat one exact declaration"));

    let wrong = typed_program(COPY_OPAQUE);
    let receipt = validation::OpaqueDataPropertyReceipt::copy(data_symbol(&wrong, "Main"));
    let diagnostics = crate::lower_typed_trees(
        wrong,
        &crate::CheckingRequest::settled().with_opaque_property_receipts(&[receipt]),
    )
    .expect_err("a transparent declaration cannot receive an opaque property receipt");
    assert!(rendered(diagnostics).contains("targets non-opaque declaration"));
}

#[test]
fn copy_receipt_rejects_an_opaque_that_does_not_claim_copy() {
    let program = typed_program(
        r#"
boundary data Token;
data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let receipt = validation::OpaqueDataPropertyReceipt::copy(data_symbol(&program, "Token"));
    let diagnostics = crate::lower_typed_trees(
        program,
        &crate::CheckingRequest::settled().with_opaque_property_receipts(&[receipt]),
    )
    .expect_err("a copy receipt must match the declaration's exact property claim");
    assert!(rendered(diagnostics).contains("does not claim `[copy]`"));
}
