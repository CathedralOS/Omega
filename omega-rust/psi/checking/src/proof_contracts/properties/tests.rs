use super::*;

#[test]
fn shared_carriers_copy_without_copying_their_referents() {
    let program = crate::front_end::typed_program(
        "data Owned { value: u64; }
         data View [copy] { owned: &Owned; bytes: &[u8]; }
         data Nested [copy] { view: View; }
         machine entry() {}",
    );
    crate::validate_program(&program).expect("shared carriers compose under explicit copy");
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(&program, &mut diagnostics);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let view = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "View")
        .expect("View");
    for member in program.data_members(view) {
        let typed_trees::data::DataMember::Field(field) = member else {
            continue;
        };
        assert!(type_satisfies_declared_property(
            &program,
            &symbols,
            &[],
            field.type_reference,
            DeclaredPropertyRequirement::Copy,
        ));
        assert!(!type_satisfies_declared_property(
            &program,
            &symbols,
            &[],
            field.type_reference,
            DeclaredPropertyRequirement::Linear,
        ));
    }
}

#[test]
fn copy_composition_excludes_exclusive_references_and_affine_records() {
    for field_type in ["&mut u64", "&write u64", "Affine"] {
        let program = crate::front_end::typed_program(&format!(
            "data Affine {{ shared: &u64; }}
             data Invalid [copy] {{ field: {field_type}; }}
             machine entry() {{}}",
        ));
        let diagnostics = crate::validate_program(&program).expect_err(field_type);
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("data `Invalid` declares `[copy]` but field `field` is not `copy`")),
            "{field_type}: {diagnostics:?}"
        );
    }
}
