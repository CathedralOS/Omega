use super::{
    TypeReferenceHandle, TypeReferenceNode, TypedTrees, has_service_seam_contents,
    has_stable_observable_contents,
};
fn input(program: &TypedTrees) -> TypeReferenceHandle {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    program.state_parameters(&program.machine_states(machine)[0])[0].type_reference
}

#[test]
fn stable_observation_rejects_nested_mutation_authority_and_unknown_qualifiers() {
    for carrier in [
        "&mut [u8]",
        "&write [u8]",
        "Holder<Holder<Mutable>>",
        "&Holder<WriteOnly>",
    ] {
        let program = crate::front_end::typed_program(&format!(
            "data Holder<T> {{ value: T; }}
             data Mutable {{ value: &mut [u8]; }}
             data WriteOnly {{ value: &write u64; }}
             machine inspect(value: {carrier}) {{}}"
        ));
        assert!(
            !has_stable_observable_contents(&program, input(&program)),
            "{carrier}"
        );
    }

    // Unknown retained qualifications must not disappear through the scalar
    // classifier even though their old authored bracket surface is retired.
    for carrier in ["u64", "Holder<Scalar>", "&[u8]", "&Holder<Bytes>"] {
        for qualifier in ["atomic", "unknown"] {
            let mut program = crate::front_end::typed_program(&format!(
                "data Holder<T> {{ value: T; }}
                 data Scalar {{ value: u64; }} data Bytes {{ value: &[u8]; }}
                 machine inspect(value: {carrier}) {{}}"
            ));
            assert!(has_stable_observable_contents(&program, input(&program)));
            let primitives = program
                .type_reference_table
                .named_references()
                .filter_map(|(reference, _, name)| {
                    matches!(name, "u64" | "u8").then_some(reference)
                })
                .collect::<Vec<_>>();
            for reference in primitives {
                let base = program
                    .type_reference_table
                    .type_reference(reference)
                    .clone();
                let base_type = program.type_reference_table.insert(base);
                let constraints = program.type_reference_table.insert_constraints([
                    typed_trees::types::TypeConstraintNode::Named(qualifier.into()),
                ]);
                program.type_reference_table.substitute_node(
                    reference,
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    },
                );
            }
            assert!(
                !has_stable_observable_contents(&program, input(&program)),
                "{carrier}, {qualifier}"
            );
        }
    }
}

#[test]
fn service_seam_contents_keep_the_runtime_exclusions() {
    // An unresolvable erased member type still fails closed.
    let program = crate::front_end::typed_program(
        "data Sealed { payload: u64; proof [erased]: Missing; }
         machine inspect(value: Sealed) {}",
    );
    assert!(!has_service_seam_contents(&program, input(&program)));
    for carrier in [
        // Runtime contents are exactly stable-observation's: mutation
        // authority, linear carriers, and cleanup-hooked members still reject.
        "data Sealed { payload: &mut u64; proof [erased]: Evidence; }
         data Evidence { case Only; }
         machine inspect(value: Sealed) {}",
        "data Evidence { case Only; }
         data Token [linear] { value: u64; }
         data Sealed { payload: u64; token: Token; proof [erased]: Evidence; }
         machine inspect(value: Sealed) {}",
        "data Evidence { case Only; }
         data Sealed { payload: u64; proof [erased]: Evidence; }
         machine Sealed::drop(&mut self) {}
         machine inspect(value: Sealed) {}",
    ] {
        let program = crate::front_end::typed_program(carrier);
        assert!(
            !has_service_seam_contents(&program, input(&program)),
            "{carrier}"
        );
    }
    // An erased member with runtime-bearing contents still stays behind: a
    // seam-marshalable record may carry erased members whose own contents
    // would not survive observation, because they never cross.
    let program = crate::front_end::typed_program(
        "data Loaned { holder: &u64; }
         data Evidence { case Only; }
         data Sealed { payload: u64; proof [erased]: Loaned; tag: Evidence; }
         machine inspect(value: Sealed) {}",
    );
    assert!(has_service_seam_contents(&program, input(&program)));
}
