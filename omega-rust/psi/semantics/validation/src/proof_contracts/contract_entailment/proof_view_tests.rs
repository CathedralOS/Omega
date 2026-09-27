//! Ordinary proof-library names do not introduce compiler-owned term forms.
use crate::proven_machine_contract_expressions;

#[test]
fn undeclared_proof_view_names_cannot_publish_transported_hypotheses() {
    for name in ["Bag", "Seq", "Range"] {
        let program = crate::front_end::typed_program(&format!(
            r#"
            machine theorem(items: &[u64], before: &[u64])
            requires {name}(items) == {name}(before)
            ensures {name}(items) == {name}(before)
            {{}}
            "#
        ));
        assert!(
            proven_machine_contract_expressions(&program, program.machines()[0].symbol).is_empty(),
            "{name} has no selected declaration to supply a proof term"
        );
    }
}
