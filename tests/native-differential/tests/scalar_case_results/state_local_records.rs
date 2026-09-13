//! State-local record storage ends after its getter observation, on either exit.
use super::{NativeTarget, produce_source, publish};

const MUTATION_SOURCE: &str = r#"
        data Region { value: u64; }
        data Filter { region: Region; }
        machine Filter::new(value: u64) -> Filter { Filter { region: Region { value: value } } }
        machine Region::get(&self) -> u64 { self.value }
        machine Region::set(&mut self, value: u64) { self.value = value; }
        machine Filter::get(&self) -> u64 { self.region.get() }
        machine Filter::set(&mut self, value: u64) { self.region.set(value); }
        data Main {}
        machine Main::main(input: u64, out: &mut [u8]) {
            let mut local: Filter = Filter::new(input);
            let before: u64 = local.get();
            local.set(input ^ 255);
            let after: u64 = local.get();
            transition before == input && after == (input ^ 255) {
                true -> writable(out, 11)
                false -> writable(out, 255)
            }
            state writable(out: &mut [u8], marker: u8) {
                transition out.len > 0 { true -> store(out, marker) false -> done() }
            }
            state store(out: &mut [u8], marker: u8) { out[0] = marker; }
            state done() {}
        }
    "#;

#[test]
fn state_local_mutation_preserves_nested_storage_and_earlier_snapshot() {
    assert_mutation_observations(MUTATION_SOURCE);
}

#[test]
fn state_local_mutation_sequences_constructor_operand_calls_after_scalar_locals() {
    let source = MUTATION_SOURCE.replace(
        "let mut local: Filter = Filter::new(input);",
        "let seed: u64 = identity(input);\n\
         let mut local: Filter = Filter::new(identity(seed));",
    );
    assert_mutation_observations(&format!(
        "machine identity(value: u64) -> u64 {{ value }}\n{source}"
    ));
}

#[test]
fn state_local_mutation_short_circuit_guard_publishes_canonical_terminal() {
    let source = MUTATION_SOURCE.replace(
        "before == input && after == (input ^ 255)",
        "before != 0 && input / before == 1 && after == (input ^ 255)",
    );
    let _artifact = produce_source("Main::main", &source);
}

#[test]
fn state_local_mutation_short_circuits_guard_before_selected_cleanup() {
    let source = MUTATION_SOURCE.replace(
        "before == input && after == (input ^ 255)",
        "before != 0 && input / before == 1 && after == (input ^ 255)",
    );
    let artifact = produce_source("Main::main", &source);
    super::membership::execute(
        &artifact,
        r#"
        #include <stdint.h>
        struct byte_view { uint8_t *bytes; uint64_t length; };
        extern void omega_entry(uint64_t input, struct byte_view *out);
        int main(void) {
            const uint64_t inputs[] = {0, 7, UINT64_MAX};
            for (unsigned index = 0; index < sizeof(inputs) / sizeof(inputs[0]); ++index) {
                uint8_t byte = 99;
                struct byte_view output = {&byte, 1};
                omega_entry(inputs[index], &output);
                if (byte != (inputs[index] ? 11 : 255)) return 1;
                if (output.bytes != &byte || output.length != 1) return 2;
            }
            return 0;
        }
    "#,
    );
}

fn assert_mutation_observations(source: &str) {
    let artifact = produce_source("Main::main", source);
    super::membership::execute(
        &artifact,
        r#"
        #include <stdint.h>
        struct byte_view { uint8_t *bytes; uint64_t length; };
        extern void omega_entry(uint64_t input, struct byte_view *out);
        int main(void) {
            const uint64_t inputs[] = {0, 255, UINT64_C(0x8123456789abcdef), UINT64_MAX};
            for (unsigned repeat = 0; repeat < 3; ++repeat) {
                for (unsigned index = 0; index < sizeof(inputs) / sizeof(inputs[0]); ++index) {
                    for (uint64_t length = 0; length < 2; ++length) {
                        uint8_t bytes[] = {99, 99, 99};
                        struct byte_view output = {bytes + 1, length};
                        omega_entry(inputs[index], &output);
                        if (bytes[0] != 99 || bytes[2] != 99) return 1;
                        if (bytes[1] != (length ? 11 : 99)) return 2;
                        if (output.bytes != bytes + 1 || output.length != length) return 3;
                    }
                }
            }
            return 0;
        }
    "#,
    );
}

#[test]
fn state_local_mutation_rejects_immutable_receiver_and_overlapping_shared_read() {
    let immutable = MUTATION_SOURCE.replace("let mut local: Filter", "let local: Filter");
    let overlapping = "data Region { value: u64; }
        data Filter { region: Region; }
        machine Filter::new(value: u64) -> Filter { Filter { region: Region { value: value } } }
        machine Region::inspect(&self, other: &mut u64) -> u64 { self.value }
        machine observe(input: u64) -> u64 {
            let mut local: Filter = Filter::new(input);
            local.region.inspect(&mut local.region.value)
        }";
    for (source, expected) in [
        (
            immutable.as_str(),
            "requires a mutable receiver, but its source is not writable in this state",
        ),
        (
            overlapping,
            "receives shared receiver overlapping another argument in the same call",
        ),
    ] {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokenize");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
        let resolved =
            syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
        let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type");
        let diagnostics = match typed_trees_to_checked_trees::lower_typed_trees(typed) {
            Ok(_) => panic!("incompatible local receiver access must fail source checking"),
            Err(diagnostics) => diagnostics,
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "expected {expected}: {diagnostics:#?}"
        );
    }
}

const SOURCE: &str = r#"
    data Region { value: u64; }
    data Filter { region: Region; }
    machine Filter::new(value: u64) -> Filter {
        Filter { region: Region { value: value } }
    }
    machine Region::get(&self) -> u64 { self.value }
    machine Filter::get(&self) -> u64 { self.region.get() }
    data Main {}
    machine Main::main(input: u64, out: &mut [u8]) {
        let local: Filter = Filter::new(input);
        let observed: u64 = local.get();
        transition observed == 7 {
            true -> selected(out, observed, input)
            false -> other(out, observed, input)
        }
        state selected(out: &mut [u8], observed: u64, expected: u64) {
            transition observed == expected {
                true -> writable(out, 11)
                false -> writable(out, 255)
            }
        }
        state other(out: &mut [u8], observed: u64, expected: u64) {
            transition observed == expected {
                true -> writable(out, 22)
                false -> writable(out, 255)
            }
        }
        state writable(out: &mut [u8], marker: u8) {
            transition out.len > 0 { true -> store(out, marker) false -> done() }
        }
        state store(out: &mut [u8], marker: u8) { out[0] = marker; }
        state done() {}
    }
"#;

#[test]
fn state_local_nested_record_getter_survives_conditional_disposal() {
    let artifact = produce_source("Main::main", SOURCE);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let locals = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| operation.result.structural())
        .collect::<Vec<_>>();
    let [local] = locals.as_slice() else {
        panic!("one state-local Filter result");
    };
    let disposals = entry
        .blocks
        .iter()
        .map(|block| match &block.terminator {
            terminal_psi::Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => when_true
                .trivial_affine_discards
                .iter()
                .chain(&when_false.trivial_affine_discards)
                .filter(|place| **place == local.place)
                .count(),
            terminal_psi::Terminator::Jump {
                trivial_affine_discards,
                ..
            } => trivial_affine_discards
                .iter()
                .filter(|place| **place == local.place)
                .count(),
            _ => 0,
        })
        .sum::<usize>();
    assert_eq!(
        disposals, 2,
        "both selected routes dispose the local after reading it"
    );

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (image, offset) = publish(&artifact, target);
        #[cfg(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        ))]
        if target == NativeTarget::host() {
            super::native_function::assert_c_text(
                &image.output().final_text_bytes,
                offset,
                r#"
                #include <stdint.h>
                #include <stddef.h>
                #include <string.h>
                struct byte_view { uint8_t *bytes; uint64_t length; };
                extern void omega_entry(uint64_t input, struct byte_view *out);
                int main(void) {
                    const uint64_t inputs[] = {0, 7, UINT64_C(0x123456789abcdef0), UINT64_MAX};
                    for (unsigned repeat = 0; repeat < 3; ++repeat) {
                        for (size_t index = 0; index < sizeof(inputs) / sizeof(inputs[0]); ++index) {
                            for (uint64_t length = 0; length < 4; ++length) {
                                uint8_t bytes[6];
                                memset(bytes, 0xa7, sizeof(bytes));
                                struct byte_view out = {bytes + 1, length};
                                omega_entry(inputs[index], &out);
                                if (out.bytes != bytes + 1 || out.length != length) return 1;
                                for (size_t position = 0; position < sizeof(bytes); ++position) {
                                    uint8_t expected = length && position == 1
                                        ? (inputs[index] == 7 ? 11 : 22) : 0xa7;
                                    if (bytes[position] != expected) return 2;
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
            all(target_os = "macos", target_arch = "aarch64")
        )))]
        {
            let _ = (image, offset);
            eprintln!(
                "SKIP: state-local record runtime requires a matching Linux or macOS ARM64 host; cross-target publication was checked"
            );
        }
    }
}
