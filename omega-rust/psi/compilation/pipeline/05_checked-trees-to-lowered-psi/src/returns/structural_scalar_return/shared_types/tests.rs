use super::super::{
    lower_structural_scalar_return_machine, lower_structural_scalar_return_machine_in_namespace,
};
use terminal_psi::{
    BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralTypeDeclaration,
    StructuralTypeShape,
};

fn check_shared_catalog(source: &str) {
    let checked = crate::front_end::checked_program(source);
    let [plan] = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines
        .as_slice()
    else {
        panic!("one structural scalar helper");
    };
    let standalone = lower_structural_scalar_return_machine(&checked, plan)
        .expect("the helper lowers without the enclosing attachment catalog");
    let mut shared = standalone.semantic_module.structural_types.clone();
    let unrelated = semantic_vocabulary::StructuralTypeId::new(100).unwrap();
    shared.push(StructuralTypeDeclaration {
        id: unrelated,
        identity: "enclosing provider attachment".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: semantic_vocabulary::StructuralFieldId::new(100).unwrap(),
                identity: "console".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Erased {
                    type_identity: "Binding<Console>".into(),
                },
            }],
        },
    });
    // This attachment belongs to the enclosing machine, not the helper. It is
    // valid only with its owner, so copying it into a helper-only module fails
    // independent validation even though the helper never references it.
    let lowered = lower_structural_scalar_return_machine_in_namespace(
        &checked,
        plan,
        semantic_vocabulary::MachineId::new(2).unwrap(),
        super::super::TERMINAL_MACHINE_IDENTITY_STRIDE,
        Some(&shared),
    )
    .expect("only the helper's checked type closure belongs to its temporary module");
    assert_eq!(
        lowered.semantic_module.structural_types, standalone.semantic_module.structural_types,
        "shared IDs and transitive referent shapes survive unchanged"
    );
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent proof verification");

    for mutation in ["missing", "duplicate", "shape"] {
        let mut changed = shared.clone();
        match mutation {
            "missing" => {
                changed.remove(0);
            }
            "duplicate" => changed.push(changed[0].clone()),
            "shape" => {
                changed[0].shape =
                    StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Boolean);
            }
            _ => unreachable!(),
        }
        assert!(
            lower_structural_scalar_return_machine_in_namespace(
                &checked,
                plan,
                semantic_vocabulary::MachineId::new(2).unwrap(),
                super::super::TERMINAL_MACHINE_IDENTITY_STRIDE,
                Some(&changed),
            )
            .is_err(),
            "shared dependency mutation must reject: {mutation}"
        );
    }
}

#[test]
fn borrowed_scalar_helper_excludes_the_callers_provider_attachment() {
    check_shared_catalog("machine inspect(value: &mut i32) -> i32 { 70 }");
}

#[test]
fn scalar_helper_keeps_nested_referents_without_unrelated_attachments() {
    check_shared_catalog(
        "data Root {} data Inner { count: u64; } data Outer { inner: Inner; }\n\
         machine Root::inspect(value: Outer) -> i32 { 70 }",
    );
}
