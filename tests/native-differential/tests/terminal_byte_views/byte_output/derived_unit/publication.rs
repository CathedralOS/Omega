//! Derived descriptor custody survives ordinary object and image publication.
use super::*;

fn container(
    target: NativeTarget,
) -> std::sync::Arc<object_file::StagedOptimizedRelocationFreeObjectContainer> {
    std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(stage_derived_unit_output(
            target,
            &derived_unit_output_module(),
        ))
        .unwrap(),
    )
}

#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(target_os = "macos", target_arch = "aarch64")
))]
pub(super) fn published_image(target: NativeTarget) -> (image_emission::ExecutableImage, usize) {
    let source = container(target);
    let object = image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
    image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
    let entry_offset = object.entry_function().text_offset;
    let image = image_emission::emit_executable_image(&object, 3).unwrap();
    image_emission::validate_executable_image(&object, &image).unwrap();
    let record = image_emission::build_installation_record(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .unwrap();
    let encoded = image_emission::encode_installation_record(&record).unwrap();
    let decoded = image_emission::decode_installation_record(&encoded).unwrap();
    image_emission::validate_installation_record(&decoded, &image).unwrap();
    (image, entry_offset)
}

#[test]
fn derived_view_unit_output_publishes_objects_images_and_installation() {
    let module = derived_unit_output_module();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let source = std::sync::Arc::new(
            object_file::stage_optimized_relocation_free_object_container(
                stage_derived_unit_output(target, &module),
            )
            .unwrap(),
        );
        let object = image_emission::build_function_fragment_object_artifact(source.clone())
            .unwrap_or_else(|error| panic!("derived Unit object on {target:?}: {error}"));
        image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
        assert_eq!(object.text_bytes(), source.source().text_section().bytes);
        let call = object
            .entry_function()
            .internal_unit_calls
            .iter()
            .find(|call| call.target == MachineId::new(1).unwrap())
            .unwrap();
        assert_eq!(call.arguments.len(), 1);
        let argument = &call.arguments[0];
        assert_eq!(argument.place, PlaceId::new(130).unwrap());
        assert_eq!(
            argument.source,
            machine_code::InternalUnitStructuralArgumentSourceRecord::EstablishedByteView {
                psi_operation: OperationId::new(130).unwrap(),
            }
        );
        assert_eq!(
            argument.access,
            terminal_psi::StructuralAccess::SharedBorrow
        );
        assert!(matches!(
            argument.source_location,
            machine_code::StructuralSourceLocation::Stack { .. }
        ));
        let image = image_emission::emit_executable_image(&object, 3).unwrap();
        image_emission::validate_executable_image(&object, &image).unwrap();
        let record = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
        )
        .unwrap();
        let encoded = image_emission::encode_installation_record(&record).unwrap();
        let decoded = image_emission::decode_installation_record(&encoded).unwrap();
        image_emission::validate_installation_record(&decoded, &image).unwrap();
        assert_eq!(
            image_emission::derive_stack_demand(&object, module.entry).unwrap(),
            image_emission::derive_installation_stack_demand(&decoded, &image, module.entry)
                .unwrap(),
        );
    }
}

fn corrupt_descriptor(call: &mut machine_code::InternalUnitCallRecord, mutation: &str) {
    if mutation == "missing" {
        call.arguments.clear();
        return;
    }
    let argument = &mut call.arguments[0];
    match mutation {
        "producer" => {
            argument.source =
                machine_code::InternalUnitStructuralArgumentSourceRecord::EstablishedByteView {
                    psi_operation: OperationId::new(120).unwrap(),
                }
        }
        "place" => argument.place = PlaceId::new(102).unwrap(),
        "offset" => {
            let machine_code::StructuralSourceLocation::Stack { byte_offset } =
                &mut argument.source_location
            else {
                panic!("local descriptor")
            };
            *byte_offset += 8;
        }
        "access" => argument.access = terminal_psi::StructuralAccess::Owned,
        "shape" => argument.shape.byte_size += 8,
        "destination" => argument.destination.locations.clear(),
        "call_bytes" => argument.bytes[0] ^= 1,
        "span" => argument.code_offset += 1,
        _ => panic!("unknown descriptor mutation {mutation}"),
    }
}

#[test]
fn derived_view_unit_output_publication_rejects_descriptor_substitution() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let source = container(target);
        let object =
            image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
        image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
        let image = image_emission::emit_executable_image(&object, 3).unwrap();
        let record = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
        )
        .unwrap();
        for mutation in [
            "producer",
            "place",
            "offset",
            "access",
            "shape",
            "destination",
            "call_bytes",
            "span",
            "missing",
        ] {
            let mut changed = object.clone();
            let call = changed
                .functions_mut_for_test()
                .iter_mut()
                .find(|function| function.machine == MachineId::new(100).unwrap())
                .unwrap()
                .internal_unit_calls
                .iter_mut()
                .find(|call| call.target == MachineId::new(1).unwrap())
                .unwrap();
            corrupt_descriptor(call, mutation);
            assert!(
                image_emission::validate_function_fragment_object_artifact(&source, &changed)
                    .is_err(),
                "{mutation} on {target:?}"
            );
            assert!(
                image_emission::emit_executable_image(&changed, 3).is_err(),
                "{mutation} on {target:?}"
            );

            let mut changed_record = record.clone();
            let call = changed_record
                .internal_unit_calls_mut_for_test()
                .iter_mut()
                .find(|call| {
                    call.machine == MachineId::new(100).unwrap()
                        && call.custody.target == MachineId::new(1).unwrap()
                })
                .unwrap();
            corrupt_descriptor(&mut call.custody, mutation);
            assert!(
                image_emission::validate_installation_record(&changed_record, &image).is_err(),
                "installed {mutation} on {target:?}"
            );
            if mutation == "producer" {
                // A valid identity encoding is not evidence that the cited
                // operation produced this descriptor.
                let encoded = image_emission::encode_installation_record(&changed_record).unwrap();
                let decoded = image_emission::decode_installation_record(&encoded).unwrap();
                assert!(image_emission::validate_installation_record(&decoded, &image).is_err());
            }
        }
        let mut missing_replay = object.clone();
        missing_replay.clear_fragment_replay_for_test();
        image_emission::validate_function_fragment_object_artifact(&source, &missing_replay)
            .unwrap();
        assert!(image_emission::emit_executable_image(&missing_replay, 3).is_err());
        let mut corrupt_text = object.clone();
        corrupt_text.text_bytes_mut_for_test()[object.entry_function().text_offset] ^= 1;
        assert!(
            image_emission::validate_function_fragment_object_artifact(&source, &corrupt_text)
                .is_err()
        );
        assert!(image_emission::emit_executable_image(&corrupt_text, 3).is_err());
    }
}
