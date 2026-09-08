//! Provider-free ordered edge values remain observable on macOS as well as Linux.
use super::*;

fn ordered_scalar_module() -> TerminalModule {
    let mut module = fixtures::byte_view_length_module();
    module.structural_types.clear();
    let machine = &mut module.machines[0];
    machine.structural_parameters.clear();
    machine.structural_places.clear();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let value = |identity| ValueId::new(identity).unwrap();
    let declaration = |identity| ValueDeclaration {
        id: value(identity),
        scalar_type,
    };
    machine.parameters = vec![
        declaration(10),
        declaration(11),
        ValueDeclaration {
            id: value(12),
            scalar_type: ScalarType::Boolean,
        },
        ValueDeclaration {
            id: value(13),
            scalar_type: ScalarType::Boolean,
        },
    ];
    let successor = |edge, arguments| terminal_psi::SuccessorEdge {
        edge: EdgeId::new(edge).unwrap(),
        target: BlockId::new(20).unwrap(),
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks[0].operations.clear();
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: value(12),
        when_true: successor(17, vec![value(10), value(11)]),
        when_false: successor(18, vec![value(11), value(10)]),
    };
    machine.blocks.push(terminal_psi::Block {
        id: BlockId::new(20).unwrap(),
        parameters: vec![declaration(21), declaration(22)],
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::Conditional {
            condition: value(13),
            when_true: terminal_psi::SuccessorEdge {
                target: BlockId::new(30).unwrap(),
                ..successor(23, Vec::new())
            },
            when_false: terminal_psi::SuccessorEdge {
                target: BlockId::new(40).unwrap(),
                ..successor(24, Vec::new())
            },
        },
    });
    for (identity, returned) in [(30, 21), (40, 22)] {
        machine.blocks.push(terminal_psi::Block {
            id: BlockId::new(identity).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Return {
                edge: EdgeId::new(identity).unwrap(),
                value: value(returned),
                cleanup_actions: Vec::new(),
            },
        });
    }
    module
}

#[test]
fn scalar_edge_copies_publish_and_execute_ordered_values_without_provider() {
    let module = ordered_scalar_module();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let text = calls::stage_call_text_with_proof(target, &module, &ProofBundle::default());
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
        image_emission::validate_installation_record(&record, &image).unwrap();
        if target == NativeTarget::host() {
            assert_ordered_values(
                &image.output().final_text_bytes,
                object.entry_function().text_offset,
            );
        }
    }
}

fn assert_ordered_values(bytes: &[u8], entry_offset: usize) {
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
        extern uint64_t omega_entry(uint64_t first, uint64_t second, bool forward, bool return_first);
        int main(void) {
            const uint64_t values[] = {0, 1, 2, 255, 256, UINT64_C(0x8000000000000000), UINT64_MAX};
            for (unsigned repetition = 0; repetition < 3; ++repetition)
                for (unsigned left = 0; left < 7; ++left)
                    for (unsigned right = 0; right < 7; ++right)
                        for (unsigned forward = 0; forward < 2; ++forward)
                          for (unsigned return_first = 0; return_first < 2; ++return_first) {
                            uint64_t first = values[left], second = values[right];
                            uint64_t expected = (forward == return_first) ? first : second;
                            if (omega_entry(first, second, forward != 0, return_first != 0) != expected) return 1;
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
        eprintln!("SKIP: pure scalar C execution requires supported Linux or macOS host");
    }
}
