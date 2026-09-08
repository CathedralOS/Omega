//! Distinct incoming views bind one destination descriptor before real calls.
use super::*;
use semantic_vocabulary::{BlockId, EdgeId, ScalarType};
use terminal_psi::{Block, StructuralAccess, StructuralArgument, SuccessorEdge, Terminator};

fn view_transfer_module() -> TerminalModule {
    let mut module = fixtures::byte_view_read_call_module();
    module.machines[0] = fixtures::byte_view_length_module().machines.remove(0);
    let caller = &mut module.machines[1];
    caller.parameters[0].scalar_type = ScalarType::Boolean;
    let mut alternate = caller.structural_parameters[0].clone();
    alternate.place = PlaceId::new(112).unwrap();
    alternate.position = 1;
    caller.structural_parameters.push(alternate);
    let destination = PlaceId::new(122).unwrap();
    let join = BlockId::new(120).unwrap();
    caller.structural_places.extend([
        StructuralPlaceDeclaration {
            id: PlaceId::new(112).unwrap(),
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: destination,
            kind: StructuralPlaceKind::BlockParameter {
                block: join,
                position: 0,
            },
        },
    ]);
    let mut arrived = caller.blocks.remove(0);
    arrived.id = join;
    let mut parameter = caller.structural_parameters[0].clone();
    parameter.place = destination;
    arrived.structural_parameters.push(parameter);
    for operation in &mut arrived.operations {
        let OperationKind::CallStructuralScalar {
            arguments,
            structural_arguments,
            ..
        } = &mut operation.kind
        else {
            panic!("length helper call")
        };
        arguments.clear();
        structural_arguments[0].place = destination;
    }
    let successor = |edge, source| SuccessorEdge {
        edge: EdgeId::new(edge).unwrap(),
        target: join,
        arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: PlaceId::new(source).unwrap(),
            path: Vec::new(),
            access: StructuralAccess::SharedBorrow,
        }],
        trivial_affine_discards: Vec::new(),
    };
    caller.blocks = vec![
        Block {
            id: caller.entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Conditional {
                condition: caller.parameters[0].id,
                when_true: successor(113, 102),
                when_false: successor(114, 112),
            },
        },
        arrived,
    ];
    module
}

#[test]
fn byte_view_block_transfers_cross_lower_and_execute_selected_lengths() {
    let module = view_transfer_module();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let text = calls::stage_call_text_with_proof(target, &module, &ProofBundle::default());
        assert_eq!(text.text_section().resolved_internal_machine_calls.len(), 2);
        if target == NativeTarget::host() {
            let entry = text
                .text_section()
                .functions
                .iter()
                .find(|function| function.machine == module.entry)
                .unwrap();
            assert_lengths(
                &text.text_section().bytes,
                entry.section_offset.try_into().unwrap(),
            );
        }
        assert_mixed_result_publication_boundary(text);
    }
}

#[test]
fn byte_view_block_transfers_preserve_selected_pointer_and_bounds_through_calls() {
    let mut module = view_transfer_module();
    let original = fixtures::byte_view_read_call_module();
    module.machines[0] = original.machines[0].clone();
    let caller = &mut module.machines[1];
    let mut index = original.machines[1].parameters[0];
    index.id = semantic_vocabulary::ValueId::new(133).unwrap();
    caller.parameters.push(index);
    for operation in &mut caller.blocks[1].operations {
        let OperationKind::CallStructuralScalar { arguments, .. } = &mut operation.kind else {
            panic!("read helper call")
        };
        arguments.push(semantic_vocabulary::ValueId::new(133).unwrap());
    }
    let proof = fixtures::byte_view_read_proof(&module);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let text = calls::stage_call_text_with_proof(target, &module, &proof);
        assert_eq!(text.text_section().resolved_internal_machine_calls.len(), 2);
        if target == NativeTarget::host() {
            let entry = text
                .text_section()
                .functions
                .iter()
                .find(|function| function.machine == module.entry)
                .unwrap();
            assert_contents(
                &text.text_section().bytes,
                entry.section_offset.try_into().unwrap(),
            );
        }
        assert_mixed_result_publication_boundary(text);
    }
}

fn assert_mixed_result_publication_boundary(
    text: machine_emission::StagedOptimizedFixedFrameTextSection,
) {
    // Scalar-result mixed-ABI publication is a separate, still-closed family.
    // Effectful Unit block-view tests exercise the complete publication route.
    let source = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(text).unwrap(),
    );
    assert!(matches!(
        image_emission::build_function_fragment_object_artifact(source),
        Err(
            image_emission::FunctionFragmentObjectArtifactError::Unsupported(
                "shared function has unsupported ABI or boundary effects"
            )
        )
    ));
}

fn assert_contents(bytes: &[u8], entry_offset: usize) {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    native_function::assert_c_text(
        bytes,
        entry_offset,
        r#"
        #include <stdint.h>
        #include <stdbool.h>
        #include <stddef.h>
        #include <unistd.h>
        struct ByteView { const uint8_t *bytes; uint64_t length; };
        extern uint64_t omega_entry(bool first, uint64_t index, const struct ByteView *, const struct ByteView *);
        int main(void) {
            alarm(10);
            const uint8_t raw[] = {0xff, 0, 0x80, 0x41, 0};
            const uint8_t other[] = {0x17, 0xfe};
            struct ByteView first = {raw, sizeof(raw)}, second = {other, sizeof(other)};
            for (unsigned repetition = 0; repetition < 3; ++repetition) {
                for (uint64_t position = 0; position <= sizeof(raw); ++position) {
                    uint64_t expected_first = position < first.length ? first.bytes[position] : 256;
                    uint64_t expected_second = position < second.length ? second.bytes[position] : 256;
                    if (omega_entry(true, position, &first, &second) != expected_first) return 1;
                    if (omega_entry(false, position, &first, &second) != expected_second) return 2;
                }
                if (omega_entry(true, UINT64_MAX, &first, &second) != 256) return 3;
                second.bytes = NULL; second.length = 0;
                if (omega_entry(false, 0, &first, &second) != 256) return 4;
                if (omega_entry(true, 0, &first, &second) != raw[0]) return 5;
                second.bytes = other; second.length = sizeof(other);
                if (first.bytes != raw || first.length != sizeof(raw)) return 6;
            }
            return 0;
        }
    "#,
    );
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    {
        let _ = (bytes, entry_offset);
        eprintln!("SKIP: descriptor-transfer runtime requires supported Linux or macOS C host");
    }
}

fn assert_lengths(bytes: &[u8], entry_offset: usize) {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    native_function::assert_c_text(
        bytes,
        entry_offset,
        r#"
        #include <stdint.h>
        #include <stdbool.h>
        #include <stddef.h>
        struct ByteView { const uint8_t *bytes; uint64_t length; };
        extern uint64_t omega_entry(bool first, const struct ByteView *, const struct ByteView *);
        int main(void) {
            const uint8_t raw[] = {0xff, 0, 0x80, 0x41, 0};
            struct ByteView first = {raw, sizeof(raw)}, second = {raw + 1, 2};
            for (unsigned repetition = 0; repetition < 3; ++repetition) {
                if (omega_entry(true, &first, &second) != first.length) return 1;
                if (omega_entry(false, &first, &second) != second.length) return 2;
                second.bytes = NULL; second.length = 0;
                if (omega_entry(false, &first, &second) != 0) return 3;
                if (omega_entry(true, &first, &second) != sizeof(raw)) return 4;
                second.bytes = raw + 1; second.length = 2;
                if (first.bytes != raw || first.length != sizeof(raw)) return 5;
            }
            return 0;
        }
    "#,
    );
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    {
        let _ = (bytes, entry_offset);
        eprintln!("SKIP: descriptor-transfer runtime requires supported Linux or macOS C host");
    }
}
