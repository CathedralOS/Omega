//! Source-to-machine custody for the bounded projected store/call lane.

use crate::tests::fixtures::checked_source::checked;

const SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> i32;
    }

    data Item [copy] {
        value: i32;
    }

    Primary: Item satisfies Measure {
        machine measure(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Main [copy] {
        item: Item;
    }

    machine Main::run(&mut self) {
        self.item.value = 17;
        let erased: &dyn Measure = &self.item as &dyn Item::Primary;
        let result: i32 = erased.measure();
    }
"#;

const WRITE_ONLY_SOURCE: &str = r#"
    data Pair { prefix: u8; target: u16; }
    data Inner { prefix: u8; value: u16; }
    data Outer { prefix: u8; inner: Inner; }
    data Sink {}

    machine Sink::direct(pair: &write Pair) {
        pair.target = 17;
    }

    machine Sink::nested(outer: &write Outer) {
        outer.inner.value = 19;
    }
"#;

#[test]
fn direct_dynamic_projected_store_and_call_reach_machine_custody() {
    let checked = checked(SOURCE);
    let terminal = psi_checked_trees_to_terminal::produce_terminal_artifact(&checked, "Main::run")
        .expect("direct dynamic source reaches canonical Terminal");
    let abstract_plan = omega_psi_to_abstract_operations::lower_artifact_sections(
        terminal.semantic_bytes(),
        terminal.proof_bytes(),
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("verified direct dynamic source reaches target-neutral Omega");

    for target in [
        omega_target::NativeTarget::linux_x64(),
        omega_target::NativeTarget::linux_arm64(),
    ] {
        let target_plan =
            omega_abstract_operations_to_target_operations::lower_to_target_operations(
                &abstract_plan,
                target,
            )
            .expect("projected store and call reach target operations");
        let assigned =
            omega_target_operations_to_assigned_target_operations::assign_registers(&target_plan)
                .expect("projected store and call retain physical assignment");
        let emitted = omega_machine_emission::emit_machine_code(&assigned)
            .expect("projected store and call reach machine emission");
        let caller = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("entry caller");
        let [store] = caller.unit_structural_scalar_field_stores.as_slice() else {
            panic!("one projected field store must survive machine emission")
        };
        assert_eq!(store.psi_operation.get(), 2);
        assert_eq!(store.field_byte_offset, 0);
        let parameter_home = caller
            .unit_parameter_homes
            .iter()
            .find(|home| home.place == store.destination.place)
            .expect("store destination has one exact staged parameter home");
        assert_eq!(store.parameter_home_byte_offset, parameter_home.byte_offset);
        assert_eq!(store.parameter_home_indirect, parameter_home.indirect);
        assert!(!store.bytes.is_empty());
        assert_eq!(
            &caller.bytes[store.code_offset..store.code_offset + store.byte_count],
            store.bytes
        );
        let [call] = caller.internal_unit_calls.as_slice() else {
            panic!("one projected structural scalar call must survive machine emission")
        };
        assert_eq!(
            call.result,
            Some(psi_core::ScalarType::Integer(
                psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32).unwrap()
            ))
        );
        assert_eq!(call.arguments.len(), 1);
        assert_eq!(call.arguments[0].path.len(), 1);
        assert_eq!(call.arguments[0].source_byte_offset, 0);
    }
}

#[test]
fn direct_and_nested_write_only_field_stores_reach_both_linux_targets() {
    let checked = checked(WRITE_ONLY_SOURCE);
    for (machine_name, path_len, field_byte_offset) in
        [("Sink::direct", 0_usize, 2_u32), ("Sink::nested", 1, 4)]
    {
        let terminal =
            psi_checked_trees_to_terminal::produce_terminal_artifact(&checked, machine_name)
                .expect("write-only field store reaches canonical Terminal");
        let abstract_plan = omega_psi_to_abstract_operations::lower_artifact_sections(
            terminal.semantic_bytes(),
            terminal.proof_bytes(),
            &psi_proof_admission::AdmissionProfile::default(),
        )
        .expect("verified write-only field store reaches target-neutral Omega");

        for target in [
            omega_target::NativeTarget::linux_x64(),
            omega_target::NativeTarget::linux_arm64(),
        ] {
            let target_plan =
                omega_abstract_operations_to_target_operations::lower_to_target_operations(
                    &abstract_plan,
                    target,
                )
                .expect("write-only field store reaches target custody");
            let assigned = omega_target_operations_to_assigned_target_operations::assign_registers(
                &target_plan,
            )
            .expect("write-only field store retains physical assignment");
            let emitted = omega_machine_emission::emit_machine_code(&assigned)
                .expect("write-only field store reaches machine emission");
            let function = emitted
                .functions
                .iter()
                .find(|function| function.machine == emitted.entry)
                .expect("entry function");
            let [store] = function.unit_structural_scalar_field_stores.as_slice() else {
                panic!("one write-only field store must survive machine emission")
            };
            assert!(!store.destination.is_self);
            assert_eq!(
                store.destination.access,
                psi_terminal::StructuralAccess::WriteOnlyBorrow
            );
            assert_eq!(store.path.len(), path_len);
            assert_eq!(store.field_byte_offset, field_byte_offset);
            let home = function
                .unit_parameter_homes
                .iter()
                .find(|home| home.place == store.destination.place)
                .expect("store destination home");
            assert_eq!(store.destination_placement, home.source);
            assert_eq!(store.parameter_home_byte_offset, home.byte_offset);
            assert_eq!(store.parameter_home_indirect, home.indirect);
            assert_eq!(
                &function.bytes[store.code_offset..store.code_offset + store.byte_count],
                store.bytes
            );

            let object = omega_image_emission::build_object_artifact(&emitted)
                .expect("object replay accepts the ordinary parameter store");
            let image = omega_image_emission::emit_executable_image(&object, 3)
                .expect("write-only field store reaches an executable image");
            let installation = omega_image_emission::build_installation_record(
                &image,
                psi_core::ProfileDecisionId::new(1).unwrap(),
            )
            .expect("installation retains the write-only field store");
            omega_image_emission::validate_installation_record(&installation, &image)
                .expect("installation independently replays the field store");

            let mut corrupted = emitted.clone();
            corrupted
                .functions
                .iter_mut()
                .find(|candidate| candidate.machine == function.machine)
                .expect("store-owning function")
                .unit_structural_scalar_field_stores[0]
                .bytes[0] ^= 1;
            assert_eq!(
                omega_image_emission::build_object_artifact(&corrupted),
                Err(
                    omega_image_emission::ObjectError::InvalidUnitStructuralScalarFieldStoreEvidence(
                        function.machine,
                    ),
                )
            );
        }
    }
}
