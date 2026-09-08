//! Source-produced projected borrows preserve their contracts through Omega admission.
//! Native callers observe stores through the original projected referent.

use target::NativeTarget;

#[path = "common/native_function.rs"]
#[allow(dead_code)]
mod native_function;

#[path = "terminal_psi_indexed_receivers/stack_pointers.rs"]
mod stack_pointers;

#[path = "terminal_psi_indexed_receivers/primitive_stores.rs"]
mod primitive_stores;

fn artifact(source: &str) -> terminal_codec::CanonicalTerminalArtifact {
    artifact_for(source, "forward")
}

fn artifact_for(source: &str, entry: &str) -> terminal_codec::CanonicalTerminalArtifact {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    terminal_production::produce_terminal_artifact(&checked, entry).unwrap()
}

fn optimize(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
) -> abstract_operations_to_abstract_operations::ValidatedOptimizedAbstractPlan {
    let selections = optimization_core::OptimizationSelections::new([]).unwrap();
    let optimized = native_realization::optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        native_realization::compiler_baseline_request_v1(&selections),
    )
    .expect("source-produced receiver contract survives independent Omega admission");
    assert_eq!(optimized.plan(), optimized.verified_input().plan());
    assert_eq!(
        optimized.validation().initial_unit(),
        optimized.validation().final_unit()
    );
    assert!(optimized.commits().is_empty());
    optimized
}

const INDEXED_ALIAS: &str = "data Record [copy] { value: u16; }
    machine Record::replace(&write self) { self.value = 17; }
    machine forward(records: &write [Record; 2]) {
        let held: &write [Record; 2] = &write records;
        held[1].replace();
    }";

#[test]
fn source_projected_write_borrows_preserve_optimizer_contracts() {
    for (borrow, referent, receiver) in [
        ("write", "[Record; 2]", "root[1]"),
        ("mut", "[Record; 2]", "root[1]"),
        ("write", "[[[Record; 2]; 2]; 2]", "root[1][0][1]"),
        ("mut", "[[[Record; 2]; 2]; 2]", "root[1][0][1]"),
        ("write", "Container", "root.records[1]"),
        ("mut", "Container", "root.records[1]"),
        ("write", "[Container; 2]", "root[1].records[0]"),
        ("mut", "[Container; 2]", "root[1].records[0]"),
        ("mut", "Container", "root.record"),
    ] {
        let source = format!(
            "data Record [copy] {{ value: u16; }}
             data Container [copy] {{ record: Record; records: [Record; 2]; }}
             machine Record::replace(&write self) {{ self.value = 17; }}
             machine forward(root: &{borrow} {referent}) {{ {receiver}.replace(); }}"
        );
        let _optimized = optimize(&artifact(&source));
    }
    let _optimized = optimize(&artifact(INDEXED_ALIAS));
}

fn native_text(
    source: &str,
    target: NativeTarget,
) -> machine_emission::StagedOptimizedFixedFrameTextSection {
    native_text_for(source, target, "forward")
}

fn native_text_for(
    source: &str,
    target: NativeTarget,
    entry: &str,
) -> machine_emission::StagedOptimizedFixedFrameTextSection {
    let artifact = artifact_for(source, entry);
    let physical =
        native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimize(&artifact),
            target,
            &[],
        )
        .unwrap_or_else(|error| panic!("{target:?}: {error:?}\n{source}"));
    let fragments = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .unwrap();
    let framed = machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
    let placed = machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
    machine_emission::validate_optimized_fixed_frame_text_section(&placed).unwrap();
    placed
}

#[test]
fn indexed_write_only_alias_cross_lowers_with_its_receiver_call() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let placed = native_text(INDEXED_ALIAS, target);
        assert_eq!(placed.text_section().functions.len(), 2);
        assert_eq!(
            placed.text_section().resolved_internal_machine_calls.len(),
            1
        );
    }
}

#[test]
fn indexed_write_only_alias_updates_caller_storage_without_neighbor_writes() {
    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    {
        let placed = native_text(INDEXED_ALIAS, NativeTarget::host());
        let module =
            terminal_codec::decode_module(artifact(INDEXED_ALIAS).semantic_bytes()).unwrap();
        let text = placed.text_section();
        let entry = text
            .functions
            .iter()
            .find(|function| function.machine == module.entry)
            .unwrap();
        native_function::assert_c_text(
            &text.bytes,
            entry.section_offset.try_into().unwrap(),
            r#"
            #include <stdint.h>
            typedef struct { uint16_t value; } Record;
            extern void omega_entry(Record *records);
            int main(void) {
                struct { uint16_t before; Record records[2]; uint16_t after; } frame =
                    { 0x1357, {{0x2468}, {0xffff}}, 0xabcd };
                omega_entry(frame.records);
                if (frame.before != 0x1357 || frame.records[0].value != 0x2468 ||
                    frame.records[1].value != 17 || frame.after != 0xabcd) return 1;
                frame.records[1].value = 65530;
                omega_entry(frame.records);
                return frame.before != 0x1357 || frame.records[0].value != 0x2468 ||
                    frame.records[1].value != 17 || frame.after != 0xabcd;
            }
        "#,
        );
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!("SKIP: caller-byte execution requires a supported Linux or macOS native host");
}

#[test]
fn projected_receiver_stores_preserve_exact_scalar_width_and_padding() {
    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    for (scalar, native_scalar, literal) in [
        ("u8", "uint8_t", "17"),
        ("u16", "uint16_t", "17"),
        ("u32", "uint32_t", "17"),
        ("u64", "uint64_t", "17"),
        ("bool", "_Bool", "true"),
    ] {
        for access in ["write", "mut"] {
            let source = format!(
                "data Record [copy] {{ before: u8; value: {scalar}; after: u8; }}
                machine Record::replace(&write self) {{ self.value = {literal}; }}
                machine forward(records: &{access} [Record; 2]) {{ records[1].replace(); }}"
            );
            let placed = native_text(&source, NativeTarget::host());
            let module = terminal_codec::decode_module(artifact(&source).semantic_bytes()).unwrap();
            let text = placed.text_section();
            let entry = text
                .functions
                .iter()
                .find(|function| function.machine == module.entry)
                .unwrap();
            let expected = if scalar == "bool" { 1 } else { 17 };
            let driver = format!(
                r#"
                #include <stdint.h>
                #include <string.h>
                typedef struct {{ uint8_t before; {native_scalar} value; uint8_t after; }} Record;
                extern void omega_entry(Record *records);
                int main(void) {{
                    struct {{ uint64_t before; Record records[2]; uint64_t after; }} frame;
                    memset(&frame, 0xa5, sizeof frame);
                    frame.records[0].value = 0;
                    frame.records[1].value = 0;
                    unsigned char expected[sizeof frame];
                    frame.records[1].value = {expected};
                    memcpy(expected, &frame, sizeof frame);
                    frame.records[1].value = 0;
                    omega_entry(frame.records);
                    return memcmp(expected, &frame, sizeof frame) != 0;
                }}
            "#
            );
            native_function::assert_c_text(
                &text.bytes,
                entry.section_offset.try_into().unwrap(),
                &driver,
            );
        }
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!(
        "SKIP: scalar-width caller observations require a supported Linux or macOS native host"
    );
}

#[test]
fn interleaved_and_repeated_receiver_calls_preserve_the_root_pointer() {
    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    for (referent, body, expected_words, calls) in [
        (
            "[[Record; 2]; 2]",
            "root[1][0].replace();",
            "1,2,3,17,5,6,7,8",
            1,
        ),
        (
            "[Container; 2]",
            "root[1].records[0].replace();",
            "1,2,3,4,5,17,7,8",
            1,
        ),
        (
            "[Record; 2]",
            "let held: &write [Record; 2] = &write root; held[1].replace(); held[0].replace(); held[1].replace();",
            "1,17,17,4,5,6,7,8",
            3,
        ),
    ] {
        let source = format!(
            "data Record [copy] {{ value: u16; }}
            data Container [copy] {{ record: Record; records: [Record; 2]; }}
            machine Record::replace(&write self) {{ self.value = 17; }}
            machine forward(root: &write {referent}) {{ {body} }}"
        );
        let placed = native_text(&source, NativeTarget::host());
        let module = terminal_codec::decode_module(artifact(&source).semantic_bytes()).unwrap();
        let text = placed.text_section();
        assert_eq!(text.resolved_internal_machine_calls.len(), calls);
        let entry = text
            .functions
            .iter()
            .find(|function| function.machine == module.entry)
            .unwrap();
        native_function::assert_c_text(
            &text.bytes,
            entry.section_offset.try_into().unwrap(),
            &format!(
                r#"
            #include <stdint.h>
            #include <string.h>
            extern void omega_entry(uint16_t *root);
            int main(void) {{
                uint16_t words[8] = {{1,2,3,4,5,6,7,8}};
                const uint16_t expected[8] = {{{expected_words}}};
                omega_entry(words + 1);
                return memcmp(words, expected, sizeof words) != 0;
            }}
        "#
            ),
        );
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!(
        "SKIP: root-pointer caller observations require a supported Linux or macOS native host"
    );
}

#[test]
fn field_stores_use_runtime_scalar_inputs() {
    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    for (scalar, native_scalar) in [
        ("u8", "uint8_t"),
        ("u16", "uint16_t"),
        ("u32", "uint32_t"),
        ("u64", "uint64_t"),
        ("bool", "_Bool"),
    ] {
        let source = format!(
            "data Record [copy] {{ before: u8; value: {scalar}; after: u8; }}
            machine Record::forward(&write self, value: {scalar}) {{ self.value = value; }}"
        );
        let placed = native_text_for(&source, NativeTarget::host(), "Record::forward");
        let module = terminal_codec::decode_module(
            artifact_for(&source, "Record::forward").semantic_bytes(),
        )
        .unwrap();
        let text = placed.text_section();
        let entry = text
            .functions
            .iter()
            .find(|function| function.machine == module.entry)
            .unwrap();
        native_function::assert_c_text(
            &text.bytes,
            entry.section_offset.try_into().unwrap(),
            &format!(
                r#"
            #include <stdint.h>
            #include <string.h>
            typedef struct {{ uint8_t before; {native_scalar} value; uint8_t after; }} Record;
            extern void omega_entry({native_scalar} value, Record *root);
            int main(void) {{
                Record root;
                memset(&root, 0xa5, sizeof root);
                root.value = 0;
                for (unsigned value = 0; value < 4; ++value) {{
                    root.value = value;
                    unsigned char expected[sizeof root];
                    memcpy(expected, &root, sizeof root);
                    root.value = !value;
                    omega_entry(value, &root);
                    if (memcmp(expected, &root, sizeof root)) return 1;
                }}
                return 0;
            }}
        "#
            ),
        );
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!("SKIP: runtime-input stores require a supported Linux or macOS native host");
}
