//! The byte writer receives an explicit value-preserving u8-to-i32 conversion.
use super::*;

fn widened_byte_output_module() -> TerminalModule {
    let mut module = byte_output_module();
    let machine = &mut module.machines[0];
    let destination_type = machine.parameters[0].scalar_type;
    machine.parameters[0].scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let widened = ValueId::new(6).unwrap();
    let OperationKind::BoundaryCall { arguments, .. } = &mut machine.blocks[0].operations[0].kind
    else {
        panic!("byte-output boundary")
    };
    arguments[0] = widened;
    machine.blocks[0].operations.insert(
        0,
        Operation {
            id: OperationId::new(6).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                id: widened,
                scalar_type: destination_type,
            }),
            kind: OperationKind::IntegerWiden {
                operand: machine.parameters[0].id,
            },
        },
    );
    module
}

fn widening_return_module() -> TerminalModule {
    let mut module = widened_byte_output_module();
    module.boundary_machines.clear();
    let machine = &mut module.machines[0];
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: ValueId::new(9).unwrap(),
        scalar_type,
    });
    machine.blocks[0].operations.truncate(1);
    let OperationResult::Scalar(result) = &mut machine.blocks[0].operations[0].result else {
        panic!("widened scalar")
    };
    result.scalar_type = scalar_type;
    machine.blocks[0].terminator = Terminator::Return {
        edge: EdgeId::new(8).unwrap(),
        value: ValueId::new(6).unwrap(),
        cleanup_actions: Vec::new(),
    };
    module
}

#[test]
fn widened_byte_output_executes_all_256_bytes_on_linux() {
    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        let text = stage_byte_output_module(NativeTarget::host(), &widened_byte_output_module());
        let placed = text.text_section();
        native_function::assert_c_text(
            &placed.bytes,
            placed.functions[0].section_offset.try_into().unwrap(),
            r#"
            #include <stdint.h>
            #include <unistd.h>
            extern void omega_entry(uint8_t byte);
            int main(void) {
                alarm(10);
                int channel[2];
                if (pipe(channel)) return 1;
                int saved = dup(STDOUT_FILENO);
                if (saved < 0 || dup2(channel[1], STDOUT_FILENO) < 0) return 2;
                close(channel[1]);
                for (unsigned repetition = 0; repetition < 2; ++repetition)
                    for (unsigned byte = 0; byte < 256; ++byte) omega_entry((uint8_t)byte);
                if (dup2(saved, STDOUT_FILENO) < 0) return 3;
                close(saved);
                for (unsigned repetition = 0; repetition < 2; ++repetition)
                    for (unsigned byte = 0; byte < 256; ++byte) {
                        uint8_t actual;
                        if (read(channel[0], &actual, 1) != 1 || actual != byte) return 4;
                    }
                uint8_t extra;
                if (read(channel[0], &extra, 1) != 0) return 5;
                close(channel[0]);
                return 0;
            }
        "#,
        );
    }
    #[cfg(not(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )))]
    eprintln!(
        "SKIP: byte-output builtin executes only on Linux; macOS/Windows providers are not substituted"
    );
}

#[test]
fn widening_return_cross_lowers_on_all_hosted_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let text = calls::stage_call_text_with_proof(
            target,
            &widening_return_module(),
            &ProofBundle::default(),
        );
        assert!(!text.text_section().bytes.is_empty());
    }
}

#[test]
fn widening_return_executes_all_bytes_and_normalizes_upper_input_bits() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    {
        let text = calls::stage_call_text_with_proof(
            NativeTarget::host(),
            &widening_return_module(),
            &ProofBundle::default(),
        );
        let placed = text.text_section();
        native_function::assert_c_text(
            &placed.bytes,
            placed.functions[0].section_offset.try_into().unwrap(),
            r#"
            #include <stdint.h>
            #include <unistd.h>
            /* Raw ABI probe: the generated parameter is u8 in the low byte;
               its register's remaining bits are not part of the source value. */
            extern uint64_t omega_entry(uint64_t raw_byte_carrier);
            int main(void) {
                alarm(10);
                const uint64_t upper[] = { 0, UINT64_C(0xffffffffffffff00), UINT64_C(0xa580ff1700000000) };
                for (unsigned pattern = 0; pattern < 3; ++pattern)
                    for (uint64_t byte = 0; byte < 256; ++byte)
                        if (omega_entry(upper[pattern] | byte) != byte) return 1;
                return 0;
            }
        "#,
        );
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!(
        "SKIP: callable C runtime requires supported Linux or macOS host; cross-lowering remains separate"
    );
}

#[test]
fn widened_byte_output_cross_lowers_and_publishes_on_linux_targets() {
    let module = widened_byte_output_module();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let text = stage_byte_output_module(target, &module);
        assert!(!text.text_section().bytes.is_empty());
        let source = std::sync::Arc::new(
            object_file::stage_optimized_relocation_free_object_container(text).unwrap(),
        );
        let object =
            image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
        image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
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
        for mutation in 0..3 {
            let mut changed = object.clone();
            let argument = &mut changed.boundary_settlements_mut_for_test()[0]
                .settlement
                .runtime_scalar_arguments[0];
            let machine_code::InternalUnitScalarArgumentSourceRecord::SelectedBoundary {
                source_value,
                scalar_type,
                ..
            } = &mut argument.source
            else {
                panic!("selected widened source")
            };
            match mutation {
                0 => *source_value = ValueId::new(5).unwrap(),
                1 => {
                    *scalar_type =
                        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap())
                }
                2 => argument.placement.shape = calling_conventions::ValueShape::integer(1, 1),
                _ => unreachable!(),
            }
            assert!(
                image_emission::validate_function_fragment_object_artifact(&source, &changed)
                    .is_err(),
                "widened input substitution {mutation}"
            );
            assert!(image_emission::emit_executable_image(&changed, 3).is_err());
        }
    }
}
