//! Source-proved fixed-extent writes retain ordinary native descriptor custody.

use super::*;

#[path = "mutable_writes/admission.rs"]
mod admission;

#[path = "mutable_writes/fixed_arrays.rs"]
mod fixed_arrays;

const FILL: &str = r#"
machine fill(out: &mut [u8], byte: u8) {
    transition { _ -> scan(out, 0, byte) }
    state scan(out: &mut [u8], position: u64, byte: u8) {
        transition position < out.len {
            true -> store(out, position, byte)
            false -> done()
        }
    }
    state store(out: &mut [u8], position: u64, byte: u8) {
        out[position] = byte;
        transition { _ -> scan(out, position + 1, byte) }
    }
    state done() {}
}
"#;

fn writer(relayed: bool) -> lowered_psi::LoweredPsi {
    let source = if relayed {
        format!(
            "{FILL}\n\
            machine relay(out: &mut [u8], byte: u8) {{ fill(out, byte); }}\n\
            machine enter(out: &mut [u8], byte: u8) {{ relay(out, 0); relay(out, byte); }}"
        )
    } else {
        FILL.to_owned()
    };
    lower_writer(&source, if relayed { "enter" } else { "fill" })
}

fn lower_writer(source: &str, entry: &str) -> lowered_psi::LoweredPsi {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, entry)
        .expect("source-authored mutable loop retains independently checked bounds");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .unwrap();
    assert!(lowered.semantic_module.boundary_machines.is_empty());
    assert!(
        lowered
            .semantic_module
            .machines
            .iter()
            .all(|machine| machine.ranked_scc.is_none())
    );
    lowered
}

fn publish(target: NativeTarget, relayed: bool) -> (image_emission::ExecutableImage, usize) {
    let lowered = writer(relayed);
    publish_lowered(target, &lowered)
}

fn publish_lowered(
    target: NativeTarget,
    lowered: &lowered_psi::LoweredPsi,
) -> (image_emission::ExecutableImage, usize) {
    let text =
        calls::stage_call_text_with_proof(target, &lowered.semantic_module, &lowered.proof_bundle);
    let source = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(text).unwrap(),
    );
    let object = image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
    image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
    assert_eq!(object.text_bytes(), source.source().text_section().bytes);
    let mut changed = object.clone();
    changed.text_bytes_mut_for_test()[object.entry_function().text_offset] ^= 1;
    assert!(image_emission::validate_function_fragment_object_artifact(&source, &changed).is_err());
    assert!(image_emission::emit_executable_image(&changed, 3).is_err());
    let mut missing_replay = object.clone();
    missing_replay.clear_fragment_replay_for_test();
    assert!(image_emission::emit_executable_image(&missing_replay, 3).is_err());
    let entry_offset = object.entry_function().text_offset;
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
    assert_eq!(
        image_emission::derive_stack_demand(&object, lowered.semantic_module.entry).unwrap(),
        image_emission::derive_installation_stack_demand(
            &decoded,
            &image,
            lowered.semantic_module.entry
        )
        .unwrap(),
    );
    (image, entry_offset)
}

#[test]
fn mutable_byte_writer_publishes_on_hosted_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        for relayed in [false, true] {
            let (image, _) = publish(target, relayed);
            assert!(!image.output().final_text_bytes.is_empty());
        }
    }
}

#[test]
fn mutable_byte_writer_executes_all_octets_and_preserves_descriptors_and_canaries() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64"),
    ))]
    for relayed in [false, true] {
        let (image, entry_offset) = publish(NativeTarget::host(), relayed);
        native_function::assert_c_text(
            &image.output().final_text_bytes,
            entry_offset,
            r#"
            #include <stdint.h>
            #include <stddef.h>
            #include <string.h>
            #include <unistd.h>
            struct ByteView { uint8_t *bytes; uint64_t length; };
            _Static_assert(sizeof(struct ByteView) == 16, "descriptor extent");
            _Static_assert(_Alignof(struct ByteView) == 8, "descriptor alignment");
            _Static_assert(offsetof(struct ByteView, length) == 8, "length offset");
            /* Native parameters place the scalar prefix before the descriptor pointer. */
            extern void omega_entry(uint8_t byte, const struct ByteView *out);
            int main(void) {
                alarm(10);
                const uint64_t lengths[] = {1, 3, 17};
                for (unsigned byte = 0; byte <= 255; ++byte) {
                    struct ByteView empty = {NULL, 0};
                    omega_entry((uint8_t)byte, &empty);
                    omega_entry((uint8_t)byte, &empty);
                    if (empty.bytes != NULL || empty.length != 0) return 1;
                    for (unsigned length_index = 0; length_index < 3; ++length_index) {
                        struct {
                            uint8_t before[8];
                            uint8_t bytes[17];
                            uint8_t after[8];
                        } storage;
                        memset(&storage, 0xa7, sizeof(storage));
                        for (unsigned position = 0; position < 17; ++position)
                            storage.bytes[position] = (uint8_t)(0x80 + position);
                        struct {
                            uint64_t before[2];
                            struct ByteView view;
                            uint64_t after[2];
                        } descriptor = {
                            {0x1122334455667788ULL, 0x8877665544332211ULL},
                            {storage.bytes, lengths[length_index]},
                            {0xfedcba9876543210ULL, 0x0123456789abcdefULL}
                        };
                        const struct ByteView saved = descriptor.view;
                        for (unsigned repetition = 0; repetition < 2; ++repetition) {
                            omega_entry((uint8_t)byte, &descriptor.view);
                            if (descriptor.view.bytes != saved.bytes ||
                                descriptor.view.length != saved.length) return 2;
                            if (descriptor.before[0] != 0x1122334455667788ULL ||
                                descriptor.before[1] != 0x8877665544332211ULL ||
                                descriptor.after[0] != 0xfedcba9876543210ULL ||
                                descriptor.after[1] != 0x0123456789abcdefULL) return 3;
                            for (unsigned position = 0; position < 8; ++position)
                                if (storage.before[position] != 0xa7 ||
                                    storage.after[position] != 0xa7) return 4;
                            for (unsigned position = 0; position < 17; ++position) {
                                uint8_t expected = position < saved.length
                                    ? (uint8_t)byte : (uint8_t)(0x80 + position);
                                if (storage.bytes[position] != expected) return 5;
                            }
                        }
                    }
                }
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
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    eprintln!(
        "SKIP mutable writer runtime: existing C/text harness supports Linux x64/AArch64 and macOS AArch64; cross-publication does not establish Windows execution"
    );
}
