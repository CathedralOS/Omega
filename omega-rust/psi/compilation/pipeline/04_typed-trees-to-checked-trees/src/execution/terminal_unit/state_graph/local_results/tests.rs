use super::*;

#[test]
fn qualified_byte_local_disposition_rejoins_canonical_storage_identity() {
    let program = crate::tests::front_end::typed_program(
        r#"
        domain [u8; 16]::Utf8(bytes) requires valid_utf8(bytes);
        machine text() -> [u8; 16] in Utf8 { "Call" }
        machine observe() -> bool {
            let constructed: [u8; 16] in Utf8 = "Go" + "!";
            let returned: [u8; 16] in Utf8 = text();
            true
        }
        "#,
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "observe")
        .expect("observer");
    let state = &program.machine_states(machine)[0];
    let statements = program.statement_table.statements(state.statement_nodes);
    for ordinal in 0..2 {
        let StatementNode::LocalData(local) = &statements[ordinal] else {
            panic!("the authored byte local");
        };
        let identity = crate::execution::terminal_unit::types::byte_sequence_type_identity(
            &program,
            local.type_reference,
            &[],
            &[],
        )
        .expect("canonical byte carrier");
        assert_ne!(
            identity,
            program
                .normalized_type_identity(local.type_reference)
                .as_str(),
            "qualification identity is not the emitted storage identity"
        );
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index: ordinal as u32,
            binding_ordinal: ordinal as u32,
            type_identity: identity,
            multiplicity: Multiplicity::Unrestricted,
        };
        assert!(
            permits_disposal(&program, state, &result, &[], &[]),
            "a copyable owner needs no affine disposal receipt"
        );

        let mut wrong_identity = result.clone();
        wrong_identity.type_identity.push_str("different-capacity");
        assert!(!permits_disposal(
            &program,
            state,
            &wrong_identity,
            &[],
            &[]
        ));
        let mut wrong_multiplicity = result.clone();
        wrong_multiplicity.multiplicity = Multiplicity::Affine;
        assert!(!permits_disposal(
            &program,
            state,
            &wrong_multiplicity,
            &[],
            &[]
        ));
        let mut absent_local = result;
        absent_local.statement_index = u32::MAX;
        assert!(!permits_disposal(&program, state, &absent_local, &[], &[]));
    }
}
