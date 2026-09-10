//! A checked suffix arrives by value before a real effectful Unit call.
use super::*;

fn block_unit_module() -> TerminalModule {
    let mut module = derived_unit_output_module();
    let caller = &mut module.machines[1];
    let block = BlockId::new(135).unwrap();
    let place = PlaceId::new(151).unwrap();
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::BlockParameter { block, position: 0 },
    });
    let mut parameter = caller.structural_parameters[0].clone();
    parameter.place = place;
    let operations = caller.blocks[1].operations.split_off(1);
    let continuation = std::mem::replace(
        &mut caller.blocks[1].terminator,
        Terminator::Jump {
            edge: EdgeId::new(150).unwrap(),
            target: block,
            arguments: Vec::new(),
            structural_arguments: vec![terminal_psi::StructuralArgument {
                place: PlaceId::new(130).unwrap(),
                path: Vec::new(),
                access: terminal_psi::StructuralAccess::SharedBorrow,
            }],
            residual_affine_discards: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    );
    let mut arrived = terminal_psi::Block {
        id: block,
        parameters: Vec::new(),
        structural_parameters: vec![parameter],
        operations,
        terminator: continuation,
    };
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut arrived.operations[1].kind
    else {
        panic!("effectful reader call")
    };
    structural_arguments[0].place = place;
    caller.blocks.insert(2, arrived);
    module
}

fn corrupt(call: &mut machine_code::InternalUnitCallRecord, mutation: &str, entry: BlockId) {
    let argument = &mut call.arguments[0];
    match mutation {
        "entry" => {
            argument.source =
                machine_code::InternalUnitStructuralArgumentSourceRecord::BlockParameter {
                    block: entry,
                    place: PlaceId::new(151).unwrap(),
                }
        }
        "block" => {
            argument.source =
                machine_code::InternalUnitStructuralArgumentSourceRecord::BlockParameter {
                    block: BlockId::new(130).unwrap(),
                    place: PlaceId::new(151).unwrap(),
                }
        }
        "place" => {
            argument.source =
                machine_code::InternalUnitStructuralArgumentSourceRecord::BlockParameter {
                    block: BlockId::new(135).unwrap(),
                    place: PlaceId::new(130).unwrap(),
                }
        }
        "producer" => {
            argument.source =
                machine_code::InternalUnitStructuralArgumentSourceRecord::EstablishedByteView {
                    psi_operation: OperationId::new(130).unwrap(),
                }
        }
        "offset" => {
            let machine_code::StructuralSourceLocation::Stack { byte_offset } =
                &mut argument.source_location
            else {
                panic!("block-local descriptor")
            };
            *byte_offset += 8;
        }
        "destination" => argument.destination.locations.clear(),
        "missing" => call.arguments.clear(),
        _ => panic!("unknown mutation"),
    }
}

#[test]
fn byte_view_block_unit_transfers_publish_exact_descriptor_custody() {
    let module = block_unit_module();
    let caller_entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap()
        .entry;
    assert_ne!(caller_entry, BlockId::new(135).unwrap());
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = std::sync::Arc::new(
            object_file::stage_optimized_relocation_free_object_container(
                stage_derived_unit_output(target, &module),
            )
            .unwrap(),
        );
        let object =
            image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
        image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
        let reader = MachineId::new(1).unwrap();
        let call = object
            .entry_function()
            .internal_unit_calls
            .iter()
            .find(|call| call.target == reader)
            .unwrap();
        assert_eq!(call.arguments.len(), 1);
        assert_eq!(call.arguments[0].place, PlaceId::new(151).unwrap());
        assert_eq!(
            call.arguments[0].source,
            machine_code::InternalUnitStructuralArgumentSourceRecord::BlockParameter {
                block: BlockId::new(135).unwrap(),
                place: PlaceId::new(151).unwrap(),
            }
        );
        let machine_code::StructuralSourceLocation::Stack { byte_offset } =
            call.arguments[0].source_location
        else {
            panic!("activation-local block descriptor")
        };
        assert_eq!(byte_offset % 8, 0);
        assert!(
            byte_offset
                .checked_add(16)
                .is_some_and(|end| end <= call.arguments[0].call_stack_bytes)
        );
        let image = image_emission::emit_executable_image(&object, 3).unwrap();
        image_emission::validate_executable_image(&object, &image).unwrap();
        let record = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
        )
        .unwrap();
        let decoded = image_emission::decode_installation_record(
            &image_emission::encode_installation_record(&record).unwrap(),
        )
        .unwrap();
        image_emission::validate_installation_record(&decoded, &image).unwrap();
        for mutation in [
            "entry",
            "block",
            "place",
            "producer",
            "offset",
            "destination",
            "missing",
        ] {
            let mut changed = object.clone();
            let call = changed
                .functions_mut_for_test()
                .iter_mut()
                .find(|function| function.machine == module.entry)
                .unwrap()
                .internal_unit_calls
                .iter_mut()
                .find(|call| call.target == reader)
                .unwrap();
            corrupt(call, mutation, caller_entry);
            assert!(
                image_emission::validate_function_fragment_object_artifact(&source, &changed)
                    .is_err(),
                "{mutation} on {target:?}"
            );
            assert!(
                image_emission::emit_executable_image(&changed, 3).is_err(),
                "{mutation} on {target:?}"
            );
            let mut changed = record.clone();
            let call = changed
                .internal_unit_calls_mut_for_test()
                .iter_mut()
                .find(|call| call.machine == module.entry && call.custody.target == reader)
                .unwrap();
            corrupt(&mut call.custody, mutation, caller_entry);
            assert!(
                image_emission::validate_installation_record(&changed, &image).is_err(),
                "{mutation} on {target:?}"
            );
        }
        if target == NativeTarget::host() {
            assert_output(
                &image.output().final_text_bytes,
                object.entry_function().text_offset,
            );
        }
    }
    #[cfg(not(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )))]
    eprintln!(
        "SKIP: block-view byte output runtime requires Linux; publication covered both Linux targets"
    );
}

fn assert_output(bytes: &[u8], entry_offset: usize) {
    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    native_function::assert_c_text(
        bytes,
        entry_offset,
        r#"
        #include <stdint.h>
        #include <stddef.h>
        #include <unistd.h>
        struct ByteView { const uint8_t *bytes; uint64_t length; };
        extern void omega_entry(uint64_t start, const struct ByteView *view);
        int main(void) {
            alarm(10);
            const uint8_t raw[] = {0xff, 0, 0x80}, other[] = {0x19, 0xfe};
            const struct ByteView cases[] = {{raw, 3}, {other, 2}, {NULL, 0}, {raw, 1}};
            uint8_t expected[256]; size_t count = 0;
            int channel[2]; if (pipe(channel)) return 1;
            int saved = dup(STDOUT_FILENO);
            if (saved < 0 || dup2(channel[1], STDOUT_FILENO) < 0) return 2;
            close(channel[1]);
            for (unsigned repeat = 0; repeat < 3; ++repeat)
                for (size_t sample = 0; sample < 4; ++sample) {
                    struct ByteView view = cases[sample];
                    for (uint64_t start = 0; start <= view.length + 1; ++start) {
                        omega_entry(start, &view);
                        if (start < view.length) expected[count++] = view.bytes[start];
                        expected[count++] = '!';
                        if (view.bytes != cases[sample].bytes || view.length != cases[sample].length) return 3;
                    }
                    omega_entry(UINT64_MAX, &view); expected[count++] = '!';
                }
            struct ByteView empty = {(const uint8_t *)(uintptr_t)UINT64_MAX, 1};
            omega_entry(1, &empty); expected[count++] = '!';
            if (dup2(saved, STDOUT_FILENO) < 0) return 4;
            close(saved);
            for (size_t index = 0; index < count; ++index) {
                uint8_t actual;
                if (read(channel[0], &actual, 1) != 1 || actual != expected[index]) return 5;
            }
            uint8_t extra; if (read(channel[0], &extra, 1) != 0) return 6;
            close(channel[0]); return 0;
        }
    "#,
    );
    #[cfg(not(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )))]
    let _ = (bytes, entry_offset);
}
