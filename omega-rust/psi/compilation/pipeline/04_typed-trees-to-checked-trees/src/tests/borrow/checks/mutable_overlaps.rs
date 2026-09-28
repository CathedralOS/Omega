use super::check_program;

/// Bodyless provider requirements create caller-side loans too, so declaration
/// validation must reject their ambiguous view sources before any call is
/// attributed.
#[test]
fn rejects_ambiguous_view_return_from_boundary_trait_signature() {
    let source = r#"
        boundary trait Storage {
            machine view(first: &u8, second: &u8) -> &u8;
        }

        data Main {}

        machine Main::main(&mut self) {}
    "#;

    let diagnostics = check_program(source)
        .expect_err("a bodyless view requirement with two ref inputs should be ambiguous");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("cannot infer which input the returned view borrows"),
        "expected the bodyless-signature elision rejection, got:\n{combined}"
    );
}
