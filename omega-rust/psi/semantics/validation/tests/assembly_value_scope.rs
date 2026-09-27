use typed_trees::TypedTrees;
use validation::validate_program;

fn assert_scope_rejected(program: &TypedTrees) {
    let diagnostics =
        validate_program(program).expect_err("assembly assertion capture must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("asm assertion value")),
        "{diagnostics:?}"
    );
}

#[test]
fn assembly_assertions_reject_entry_parameters_and_locals() {
    for kind in ["requires", "ensures"] {
        for (parameters, setup, expression) in [
            ("hidden: u64", "", "hidden == hidden"),
            ("", "let hidden: u64 = 7;", "hidden == hidden"),
            ("hidden: Packet", "", "hidden.value == hidden.value"),
            ("hidden: [u64; 2]", "", "hidden[0] == hidden[0]"),
        ] {
            let source = format!(
                "data Packet {{ value: u64; }} machine run({parameters}) {{ {setup} transition {{ _ -> next() }} state next() {{ asm where {kind} ({expression}) {{ lfence }} }} }}"
            );
            assert_scope_rejected(&crate::front_end::typed_program(&source));
        }
    }
}
