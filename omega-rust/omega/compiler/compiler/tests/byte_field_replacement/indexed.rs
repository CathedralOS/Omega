//! Source-authored runtime indexed writes retain the field's live extent.

use super::{AdmissionProfile, NativeTarget, publish};

fn source(nested: bool, maximum: u64) -> String {
    let (field, selected) = if nested {
        ("payload: Payload;", "payload.out")
    } else {
        ("out: [u8;3] in Utf8;", "out")
    };
    format!(
        r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Payload {{ out: [u8;3] in Utf8; sibling: u64; }}
        data Record {{ before: u64; {field} after: u64; }}
        machine Record::replace(&mut self, position: u64 [0..={maximum}], byte: u8 [0..=127]) {{
            self.{selected} = "XXX";
            self.{selected}[position] = byte;
        }}
    "#
    )
}

fn typed_source(source: &str) -> typed_trees::TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

pub(super) fn indexed_replacement(nested: bool) -> lowered_psi::LoweredPsi {
    let typed = typed_source(&source(nested, 2));
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("authored index range and ASCII byte establish indexed replacement");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Record::replace")
        .expect("source-authored indexed replacement reaches Terminal");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("indexed replacement independently verifies live-length evidence");
    assert!(
        lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                &operation.kind,
                terminal_psi::OperationKind::StructuralByteSequenceFieldByteStore { path, .. }
                if path.len() == usize::from(nested)
            ))
    );
    lowered
}

#[test]
fn source_indexed_byte_field_replacement_publishes_native() {
    for nested in [false, true] {
        let lowered = indexed_replacement(nested);
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let (image, _) = publish(&lowered, target);
            assert!(!image.output().final_text_bytes.is_empty());
        }
    }
}

fn cyclic_source(nested: bool) -> String {
    let (field, selected) = if nested {
        ("payload: Payload;", "payload.out")
    } else {
        ("out: [u8;3] in Utf8;", "out")
    };
    format!(
        r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Payload {{ out: [u8;3] in Utf8; sibling: u64; }}
        data Record {{ before: u64; {field} after: u64; want_position: u64 [0..=2]; want_byte: u8 [0..=127]; turns: u64 in Wrapping; }}
        machine Record::rewrite(&mut self, position: u64 [0..=2], byte: u8 [0..=127]) {{
            self.{selected} = "old";
            self.want_position = position;
            self.want_byte = byte;
            self.turns = 0;
            transition {{ _ -> write() }}
            state write(&mut self) {{
                self.{selected}[self.want_position] = self.want_byte;
                transition self.turns < 2 {{ true -> again() _ -> done() }}
            }}
            state again(&mut self) {{
                self.turns = self.turns + 1;
                transition {{ _ -> write() }}
            }}
            state done(&mut self) {{ self.after = self.before; }}
        }}
    "#
    )
}

pub(super) fn cyclic_indexed_replacement(nested: bool) -> lowered_psi::LoweredPsi {
    let typed = typed_source(&cyclic_source(nested));
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("authored cyclic index range and ASCII byte establish indexed replacement");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Record::rewrite")
        .expect("cyclic indexed replacement reaches Terminal");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("cyclic indexed replacement independently verifies live-length evidence");
    assert!(
        lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                &operation.kind,
                terminal_psi::OperationKind::StructuralByteSequenceFieldByteStore { path, .. }
                if path.len() == usize::from(nested)
            ))
    );
    lowered
}

#[test]
fn cyclic_indexed_byte_field_replacement_publishes_native() {
    for nested in [false, true] {
        let lowered = cyclic_indexed_replacement(nested);
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let (image, _) = publish(&lowered, target);
            assert!(!image.output().final_text_bytes.is_empty());
        }
    }
}

#[test]
fn cyclic_indexed_store_updates_original_backing_without_changing_extent() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    for nested in [false, true] {
        let (image, entry_offset) =
            publish(&cyclic_indexed_replacement(nested), NativeTarget::host());
        let (field, selected, sibling) = if nested {
            (
                "struct Payload payload;",
                "payload.out",
                "if (record.payload.sibling != saved.payload.sibling) return 4;",
            )
        } else {
            ("struct Text out;", "out", "")
        };
        super::native_function::assert_c_text(
            &image.output().final_text_bytes,
            entry_offset,
            &format!(
                r#"
            #include <stdint.h>
            #include <string.h>
            #include <unistd.h>
            struct Text {{ uint64_t length; uint8_t bytes[3]; }};
            struct Payload {{ struct Text out; uint64_t sibling; }};
            struct Record {{ uint64_t before; {field} uint64_t after; uint64_t want_position; uint8_t want_byte; uint64_t turns; }};
            /* Omega's mixed ABI puts scalar parameters before structural parameters. */
            extern void omega_entry(uint64_t position, uint8_t byte, struct Record *destination);
            int main(void) {{
                alarm(10);
                struct Record record;
                memset(&record, 0xa7, sizeof(record));
                record.{selected}.length = 3;
                memcpy(record.{selected}.bytes, "old", 3);
                struct Record saved;
                memcpy(&saved, &record, sizeof(record));
                for (unsigned trial = 0; trial < 3; ++trial) {{
                    uint64_t position = trial;
                    uint8_t byte = (uint8_t)(65 + trial);
                    struct Record expected;
                    memcpy(&expected, &saved, sizeof(expected));
                    expected.{selected}.bytes[position] = byte;
                    expected.want_position = position;
                    expected.want_byte = byte;
                    expected.after = expected.before;
                    expected.turns = 2;
                    omega_entry(position, byte, &record);
                    if (record.{selected}.length != 3) return 1;
                    for (unsigned offset = 0; offset < 3; ++offset) {{
                        if (record.{selected}.bytes[offset] != expected.{selected}.bytes[offset]) return 2;
                    }}
                    if (record.before != expected.before) return 3;
                    {sibling}
                    if (record.after != expected.after) return 5;
                    if (record.turns != 2) return 6;
                }}
                return 0;
            }}
        "#
            ),
        );
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    panic!("native cyclic indexed-store execution requires a hosted target");
}

#[test]
fn indexed_store_rejects_index_at_live_length() {
    for nested in [false, true] {
        let typed = typed_source(&source(nested, 3));
        let result = typed_trees_to_checked_trees::lower_typed_trees(
            typed,
            &typed_trees_to_checked_trees::CheckingRequest::settled(),
        );
        if let Ok(checked) = result {
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&checked, "Record::replace").is_err(),
                "index range includes live length and must not publish"
            );
        }
    }
}

#[test]
fn source_indexed_store_updates_original_backing_without_changing_extent() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    for nested in [false, true] {
        let (image, entry_offset) = publish(&indexed_replacement(nested), NativeTarget::host());
        let (field, selected, sibling) = if nested {
            (
                "struct Payload payload;",
                "payload.out",
                "if (record.payload.sibling != saved.payload.sibling) return 4;",
            )
        } else {
            ("struct Text out;", "out", "")
        };
        super::native_function::assert_c_text(
            &image.output().final_text_bytes,
            entry_offset,
            &format!(
                r#"
            #include <stdint.h>
            #include <string.h>
            #include <unistd.h>
            struct Text {{ uint64_t length; uint8_t bytes[3]; }};
            struct Payload {{ struct Text out; uint64_t sibling; }};
            struct Record {{ uint64_t before; {field} uint64_t after; }};
            /* Omega's mixed ABI puts scalar parameters before structural parameters. */
            extern void omega_entry(uint64_t position, uint8_t byte, struct Record *destination);
            int main(void) {{
                alarm(10);
                const uint8_t values[] = {{0, 65, 127}};
                struct Record record;
                memset(&record, 0xa7, sizeof(record));
                record.{selected}.length = 3;
                memcpy(record.{selected}.bytes, "old", 3);
                struct Record saved;
                memcpy(&saved, &record, sizeof(record));
                for (uint64_t position = 0; position < 3; ++position) {{
                    for (unsigned trial = 0; trial < sizeof(values); ++trial) {{
                        struct Record expected;
                        memcpy(&expected, &saved, sizeof(expected));
                        memcpy(expected.{selected}.bytes, "XXX", 3);
                        expected.{selected}.bytes[position] = values[trial];
                        omega_entry(position, values[trial], &record);
                        if (record.{selected}.length != 3) return 1;
                        for (unsigned offset = 0; offset < 3; ++offset) {{
                            if (record.{selected}.bytes[offset] != (offset == position ? values[trial] : 'X')) return 2;
                        }}
                        if (record.before != saved.before || record.after != saved.after) return 3;
                        {sibling}
                        if (memcmp(&record, &expected, sizeof(record))) return 5;
                    }}
                }}
                return 0;
            }}
        "#
            ),
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
        "SKIP indexed-store runtime: requires Linux x64/AArch64 or macOS AArch64; four-target publication is separate"
    );
}
