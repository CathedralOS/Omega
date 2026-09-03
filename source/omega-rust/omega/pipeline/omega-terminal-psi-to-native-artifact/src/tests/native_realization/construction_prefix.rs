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

const FOURTEENTH_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 15];
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
        values[13] = Empty {};
    }
"#;

const FIFTEENTH_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 16];
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
        values[13] = Empty {};
        values[14] = Empty {};
    }
"#;

const SIXTEENTH_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 17];
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
        values[13] = Empty {};
        values[14] = Empty {};
        values[15] = Empty {};
    }
"#;

const SEVENTEENTH_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 18];
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
        values[13] = Empty {};
        values[14] = Empty {};
        values[15] = Empty {};
        values[16] = Empty {};
    }
"#;

const EIGHTEENTH_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 19];
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
        values[13] = Empty {};
        values[14] = Empty {};
        values[15] = Empty {};
        values[16] = Empty {};
        values[17] = Empty {};
    }
"#;

const NINETEENTH_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 20];
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
        values[13] = Empty {};
        values[14] = Empty {};
        values[15] = Empty {};
        values[16] = Empty {};
        values[17] = Empty {};
        values[18] = Empty {};
    }
"#;

const TWENTIETH_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 21];
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
        values[13] = Empty {};
        values[14] = Empty {};
        values[15] = Empty {};
        values[16] = Empty {};
        values[17] = Empty {};
        values[18] = Empty {};
        values[19] = Empty {};
    }
"#;

const TWENTY_FIRST_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 22];
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
        values[13] = Empty {};
        values[14] = Empty {};
        values[15] = Empty {};
        values[16] = Empty {};
        values[17] = Empty {};
        values[18] = Empty {};
        values[19] = Empty {};
        values[20] = Empty {};
    }
"#;

const TWENTY_SECOND_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 23];
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
        values[13] = Empty {};
        values[14] = Empty {};
        values[15] = Empty {};
        values[16] = Empty {};
        values[17] = Empty {};
        values[18] = Empty {};
        values[19] = Empty {};
        values[20] = Empty {};
        values[21] = Empty {};
    }
"#;

const TWENTY_THIRD_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 24];
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
        values[13] = Empty {};
        values[14] = Empty {};
        values[15] = Empty {};
        values[16] = Empty {};
        values[17] = Empty {};
        values[18] = Empty {};
        values[19] = Empty {};
        values[20] = Empty {};
        values[21] = Empty {};
        values[22] = Empty {};
    }
"#;

const TWENTY_FOURTH_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 25];
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
        values[13] = Empty {};
        values[14] = Empty {};
        values[15] = Empty {};
        values[16] = Empty {};
        values[17] = Empty {};
        values[18] = Empty {};
        values[19] = Empty {};
        values[20] = Empty {};
        values[21] = Empty {};
        values[22] = Empty {};
        values[23] = Empty {};
    }
"#;

const TWENTY_FIFTH_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}
    machine Root::cleanup_prefix() {
        let mut values: [Empty; 26];
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
        values[13] = Empty {};
        values[14] = Empty {};
        values[15] = Empty {};
        values[16] = Empty {};
        values[17] = Empty {};
        values[18] = Empty {};
        values[19] = Empty {};
        values[20] = Empty {};
        values[21] = Empty {};
        values[22] = Empty {};
        values[23] = Empty {};
        values[24] = Empty {};
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
        (FOURTEENTH_CONSTRUCTION_PREFIX_SOURCE, 14_usize),
        (FIFTEENTH_CONSTRUCTION_PREFIX_SOURCE, 15_usize),
        (SIXTEENTH_CONSTRUCTION_PREFIX_SOURCE, 16_usize),
        (SEVENTEENTH_CONSTRUCTION_PREFIX_SOURCE, 17_usize),
        (EIGHTEENTH_CONSTRUCTION_PREFIX_SOURCE, 18_usize),
        (NINETEENTH_CONSTRUCTION_PREFIX_SOURCE, 19_usize),
        (TWENTIETH_CONSTRUCTION_PREFIX_SOURCE, 20_usize),
        (TWENTY_FIRST_CONSTRUCTION_PREFIX_SOURCE, 21_usize),
        (TWENTY_SECOND_CONSTRUCTION_PREFIX_SOURCE, 22_usize),
        (TWENTY_THIRD_CONSTRUCTION_PREFIX_SOURCE, 23_usize),
        (TWENTY_FOURTH_CONSTRUCTION_PREFIX_SOURCE, 24_usize),
        (TWENTY_FIFTH_CONSTRUCTION_PREFIX_SOURCE, 25_usize),
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

            if prefix_length == 25 {
                let mut fenced_successor = assigned.clone();
                let function = &mut fenced_successor.functions[0];
                let omega_assigned_target_operations::AssignedOperation::UnitBody(body) =
                    &mut function.operation
                else {
                    unreachable!()
                };
                let mut successor_operation = body.operations[prefix_length - 1].clone();
                let omega_assigned_target_operations::AssignedUnitOperation::EstablishTrivialAffineLocal {
                    psi_operation,
                    place,
                    ..
                } = &mut successor_operation
                else {
                    unreachable!()
                };
                *psi_operation = psi_core::OperationId::new(psi_operation.get() + 1)
                    .expect("successor operation");
                place.id = psi_core::PlaceId::new(place.id.get() + 1).expect("successor place");
                let psi_core::StructuralPlaceKind::TrivialAffineLocal {
                    declaration_ordinal,
                    construction: Some(construction),
                    ..
                } = &mut place.kind
                else {
                    unreachable!()
                };
                *declaration_ordinal = 25;
                construction.index = 25;
                let successor_operation_id = *psi_operation;
                let successor_place = place.id;
                body.operations.insert(prefix_length, successor_operation);
                let omega_assigned_target_operations::AssignedUnitOperation::Return {
                    cleanup_actions,
                    ..
                } = body.operations.last_mut().expect("Unit return")
                else {
                    unreachable!()
                };
                cleanup_actions.insert(
                    0,
                    psi_terminal::TerminalAffineCleanupAction::DiscardRoot(successor_place),
                );
                let psi_terminal::StructuralTypeShape::FixedArray { length, .. } = &mut body
                    .structural_types
                    .iter_mut()
                    .find(|declaration| declaration.id == root_structural_type)
                    .expect("construction root type")
                    .shape
                else {
                    unreachable!()
                };
                *length = 27;
                function.provenance.operations.push(successor_operation_id);
                assert!(omega_machine_emission::emit_machine_code(&fenced_successor).is_err());
            }

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

            if prefix_length == 25 {
                let mut fenced_successor = emitted.clone();
                let function = &mut fenced_successor.functions[0];
                let cleanup = function
                    .unit_affine_cleanup
                    .as_mut()
                    .expect("native function retains Unit cleanup custody");
                let mut successor_local = cleanup
                    .locals
                    .last()
                    .expect("construction-prefix local")
                    .clone();
                successor_local.0 = psi_core::OperationId::new(successor_local.0.get() + 1)
                    .expect("successor operation");
                successor_local.1.id = psi_core::PlaceId::new(successor_local.1.id.get() + 1)
                    .expect("successor place");
                let psi_core::StructuralPlaceKind::TrivialAffineLocal {
                    declaration_ordinal,
                    construction: Some(construction),
                    ..
                } = &mut successor_local.1.kind
                else {
                    unreachable!()
                };
                *declaration_ordinal = 25;
                construction.index = 25;
                let successor_operation = successor_local.0;
                let successor_place = successor_local.1.id;
                cleanup.locals.push(successor_local);
                cleanup.actions.insert(
                    0,
                    psi_terminal::TerminalAffineCleanupAction::DiscardRoot(successor_place),
                );
                let psi_terminal::StructuralTypeShape::FixedArray { length, .. } = &mut cleanup
                    .structural_types
                    .iter_mut()
                    .find(|declaration| declaration.id == root_structural_type)
                    .expect("construction root type")
                    .shape
                else {
                    unreachable!()
                };
                *length = 27;
                function.provenance.operations.push(successor_operation);
                let mut successor_attribution =
                    function.semantic_code_attribution[prefix_length - 1];
                successor_attribution.site =
                    omega_machine_code::SemanticCodeSite::Operation(successor_operation);
                successor_attribution.operation_ordinal = prefix_length;
                function
                    .semantic_code_attribution
                    .insert(prefix_length, successor_attribution);
                let return_attribution = function
                    .semantic_code_attribution
                    .last_mut()
                    .expect("Unit return attribution");
                assert!(matches!(
                    return_attribution.site,
                    omega_machine_code::SemanticCodeSite::Edge(_)
                ));
                return_attribution.operation_ordinal += 1;
                assert!(omega_image_emission::build_object_artifact(&fenced_successor).is_err());
            }

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
