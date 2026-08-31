//! Construction-prefix cleanup through machine, object, image, and installation custody.

use crate::tests::fixtures::checked_source::checked;

const CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 3];
        values[0] = Empty {};
        values[1] = Empty {};
    }
"#;

const WIDER_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 4];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
    }
"#;

const DEEPER_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 5];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
    }
"#;

const DEEPEST_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 6];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
    }
"#;

const SIXTH_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 7];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
    }
"#;

const SEVENTH_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 8];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
    }
"#;

const EIGHTH_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 9];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
    }
"#;

const NINTH_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 10];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
    }
"#;

const TENTH_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 11];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
        values[9] = Empty {};
    }
"#;

const ELEVENTH_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 12];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
        values[9] = Empty {};
        values[10] = Empty {};
    }
"#;

const TWELFTH_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 13];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
        values[9] = Empty {};
        values[10] = Empty {};
        values[11] = Empty {};
    }
"#;

const THIRTEENTH_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 14];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
        values[9] = Empty {};
        values[10] = Empty {};
        values[11] = Empty {};
        values[12] = Empty {};
    }
"#;

#[test]
fn construction_prefix_reaches_native_image_and_installation_custody() {
    for (source, prefix_length) in [
        (CONSTRUCTION_PREFIX_SOURCE, 2_usize),
        (WIDER_CONSTRUCTION_PREFIX_SOURCE, 3_usize),
        (DEEPER_CONSTRUCTION_PREFIX_SOURCE, 4_usize),
        (DEEPEST_CONSTRUCTION_PREFIX_SOURCE, 5_usize),
        (SIXTH_CONSTRUCTION_PREFIX_SOURCE, 6_usize),
        (SEVENTH_CONSTRUCTION_PREFIX_SOURCE, 7_usize),
        (EIGHTH_CONSTRUCTION_PREFIX_SOURCE, 8_usize),
        (NINTH_CONSTRUCTION_PREFIX_SOURCE, 9_usize),
        (TENTH_CONSTRUCTION_PREFIX_SOURCE, 10_usize),
        (ELEVENTH_CONSTRUCTION_PREFIX_SOURCE, 11_usize),
        (TWELFTH_CONSTRUCTION_PREFIX_SOURCE, 12_usize),
        (THIRTEENTH_CONSTRUCTION_PREFIX_SOURCE, 13_usize),
    ] {
        let checked = checked(source);
        let terminal = psi_checked_trees_to_terminal::produce_terminal_artifact(
            &checked,
            "Root::cleanup_prefix",
        )
        .expect("canonical construction-prefix artifact");
        let abstract_plan = omega_psi_to_abstract_operations::lower_artifact_sections(
            terminal.semantic_bytes(),
            terminal.proof_bytes(),
            &psi_proof_admission::AdmissionProfile::default(),
        )
        .expect("verified construction prefix enters Omega");

        for target in [
            omega_target::NativeTarget::linux_x64(),
            omega_target::NativeTarget::linux_arm64(),
        ] {
            let target_plan = omega_abstract_operations_to_target_operations::
            lower_to_target_operations_with_provider_executions(&abstract_plan, target, &[])
            .expect("construction prefix reaches target operations");
            let assigned = omega_target_operations_to_assigned_target_operations::assign_registers(
                &target_plan,
            )
            .expect("construction prefix has no ABI local assignment");
            let mut invalid_assigned = assigned.clone();
            let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
                &mut invalid_assigned.functions[0].operation
            else {
                unreachable!()
            };
            let omega_assigned_target_operations::AssignedUnitOperation::EstablishTrivialAffineLocal {
            place,
            ..
        } = &mut body.operations[0]
        else {
            unreachable!()
        };
            let psi_core::StructuralPlaceKind::TrivialAffineLocal {
                construction: Some(construction),
                ..
            } = &mut place.kind
            else {
                unreachable!()
            };
            let root_structural_type = construction.root_structural_type;
            construction.index = 1;
            assert!(omega_machine_emission::emit_machine_code(&invalid_assigned).is_err());
            let mut redirected_root = assigned.clone();
            let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
                &mut redirected_root.functions[0].operation
            else {
                unreachable!()
            };
            let omega_assigned_target_operations::AssignedUnitOperation::EstablishTrivialAffineLocal {
                place,
                ..
            } = &mut body.operations[prefix_length - 1]
            else {
                unreachable!()
            };
            let psi_core::StructuralPlaceKind::TrivialAffineLocal {
                structural_type,
                construction: Some(construction),
                ..
            } = &mut place.kind
            else {
                unreachable!()
            };
            construction.root_structural_type = *structural_type;
            assert!(omega_machine_emission::emit_machine_code(&redirected_root).is_err());
            let mut reordered_establishments = assigned.clone();
            let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
                &mut reordered_establishments.functions[0].operation
            else {
                unreachable!()
            };
            body.operations.swap(0, 1);
            assert!(omega_machine_emission::emit_machine_code(&reordered_establishments).is_err());
            let mut wrong_root_length = assigned.clone();
            let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
                &mut wrong_root_length.functions[0].operation
            else {
                unreachable!()
            };
            let psi_terminal::StructuralTypeShape::FixedArray { length, .. } = &mut body
                .structural_types
                .iter_mut()
                .find(|declaration| declaration.id == root_structural_type)
                .expect("construction root type")
                .shape
            else {
                unreachable!()
            };
            *length = u64::try_from(prefix_length).expect("bounded prefix length");
            assert!(omega_machine_emission::emit_machine_code(&wrong_root_length).is_err());
            let emitted = omega_machine_emission::emit_machine_code(&assigned)
                .expect("construction prefix reaches native cleanup emission");
            let function = &emitted.functions[0];
            let cleanup = function
                .unit_affine_cleanup
                .as_ref()
                .expect("native function retains Unit cleanup custody");
            assert_eq!(cleanup.locals.len(), prefix_length);
            assert!(cleanup.locals.iter().enumerate().all(
                |(index, (_, place, element_type))| matches!(
                    place.kind,
                    psi_core::StructuralPlaceKind::TrivialAffineLocal {
                        declaration_ordinal,
                        structural_type,
                        construction: Some(construction),
                    } if usize::try_from(declaration_ordinal) == Ok(index)
                        && structural_type == element_type.id
                        && usize::try_from(construction.index) == Ok(index)
                )
            ));
            assert_eq!(
                cleanup.actions,
                cleanup
                    .locals
                    .iter()
                    .rev()
                    .map(|(_, place, _)| {
                        psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place.id)
                    })
                    .collect::<Vec<_>>()
            );
            assert_eq!(function.semantic_code_attribution.len(), prefix_length + 1);

            let object = omega_image_emission::build_object_artifact(&emitted)
                .expect("object validation reconstructs construction cleanup");
            let image = omega_image_emission::emit_executable_image(&object, 0)
                .expect("image retains construction cleanup custody");
            omega_image_emission::validate_executable_image(&object, &image)
                .expect("image independently validates construction cleanup");
            let installation = omega_image_emission::build_installation_record(
                &image,
                psi_core::ProfileDecisionId::new(1).expect("profile decision"),
            )
            .expect("construction cleanup enters installation custody");
            let bytes = omega_image_emission::encode_installation_record(&installation)
                .expect("construction installation encodes");
            let decoded = omega_image_emission::decode_installation_record(&bytes)
                .expect("construction installation decodes");
            assert_eq!(
                decoded.functions()[0].unit_affine_cleanup,
                Some(cleanup.clone())
            );
            omega_image_emission::validate_installation_record(&decoded, &image)
                .expect("decoded installation binds construction image");

            let mut wrong_index = emitted.clone();
            let psi_core::StructuralPlaceKind::TrivialAffineLocal {
                construction: Some(construction),
                ..
            } = &mut wrong_index.functions[0]
                .unit_affine_cleanup
                .as_mut()
                .unwrap()
                .locals[0]
                .1
                .kind
            else {
                unreachable!()
            };
            construction.index = 1;
            assert!(omega_image_emission::build_object_artifact(&wrong_index).is_err());

            let mut redirected_root = emitted.clone();
            let (_, place, _) = &mut redirected_root.functions[0]
                .unit_affine_cleanup
                .as_mut()
                .unwrap()
                .locals[prefix_length - 1];
            let psi_core::StructuralPlaceKind::TrivialAffineLocal {
                structural_type,
                construction: Some(construction),
                ..
            } = &mut place.kind
            else {
                unreachable!()
            };
            construction.root_structural_type = *structural_type;
            assert!(omega_image_emission::build_object_artifact(&redirected_root).is_err());

            let mut wrong_root_length = emitted.clone();
            let root = wrong_root_length.functions[0]
                .unit_affine_cleanup
                .as_ref()
                .unwrap()
                .locals[0]
                .1
                .kind;
            let psi_core::StructuralPlaceKind::TrivialAffineLocal {
                construction: Some(construction),
                ..
            } = root
            else {
                unreachable!()
            };
            let cleanup = wrong_root_length.functions[0]
                .unit_affine_cleanup
                .as_mut()
                .unwrap();
            let psi_terminal::StructuralTypeShape::FixedArray { length, .. } = &mut cleanup
                .structural_types
                .iter_mut()
                .find(|declaration| declaration.id == construction.root_structural_type)
                .expect("construction root type")
                .shape
            else {
                unreachable!()
            };
            *length = u64::try_from(prefix_length).expect("bounded prefix length");
            assert!(omega_image_emission::build_object_artifact(&wrong_root_length).is_err());

            let mut reordered_cleanup = emitted.clone();
            reordered_cleanup.functions[0]
                .unit_affine_cleanup
                .as_mut()
                .unwrap()
                .actions
                .swap(0, 1);
            assert!(omega_image_emission::build_object_artifact(&reordered_cleanup).is_err());
        }
    }
}
