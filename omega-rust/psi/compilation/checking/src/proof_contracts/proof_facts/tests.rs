//! A transparent formula's proof does not establish its arguments' qualifications.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

fn qualified_proposition(actual_type: &str) -> TypedTrees {
    crate::front_end::typed_program_from_texts(&[&format!(
        r#"
        domain [u8]::Never(bytes) requires false {{}}
        proposition admitted(bytes: &[u8] in Never, marker: u64) = marker == marker;
        machine expose(bytes: {actual_type})
        ensures admitted(bytes, 0u64)
        {{}}
        "#,
    )])
}

fn formation_diagnostics(program: &TypedTrees) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for machine in program.machines() {
        crate::declarations::state_signatures::validate_machine_contracts(
            program,
            machine,
            &mut diagnostics,
        );
    }
    diagnostics
}

#[test]
fn qualified_proposition_rejects_unqualified_reference_despite_transparent_equality() {
    let program = qualified_proposition("&[u8]");
    let diagnostics = formation_diagnostics(&program);
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("proposition `admitted` argument 1 does not match parameter `bytes`")),
        "a reflexive formula cannot establish its reference argument in Never: {diagnostics:?}"
    );

    // This separate judgment accepts the equality itself. Formation must reject
    // before its transparency can make the missing qualification disappear.
    let mut entailment = Vec::new();
    crate::proof_contracts::proposition_entailment::validate_proposition_entailment(
        &program,
        &mut entailment,
    );
    assert!(entailment.is_empty(), "{entailment:?}");
}

#[test]
fn qualified_proposition_accepts_reference_with_exact_declared_qualification() {
    // A declaration may assume an uninhabited input; constructing that input is
    // still impossible. The application does not invent its qualification.
    let program = qualified_proposition("&[u8] in Never");
    let diagnostics = formation_diagnostics(&program);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
