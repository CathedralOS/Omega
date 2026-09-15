//! Scratch probe for WRITE-ONLY-BORROW: early loan closure / restored-parent
//! uses through ordinary reference preparation/receiving replay.
//! Not part of the wave's claimed fixture paths; remove or fold into the
//! owning suite once the slice lands.

use target::NativeTarget;

#[path = "common/native_function.rs"]
#[allow(dead_code)]
mod native_function;

use native_function::assert_c_text;

fn artifact_for(source: &str, entry: &str) -> terminal_codec::CanonicalTerminalArtifact {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    terminal_production::TerminalProductionRequest::new(&checked, entry)
        .produce_artifact()
        .unwrap()
}

fn optimize(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
) -> abstract_operations_to_abstract_operations::ValidatedOptimizedAbstractPlan {
    let selections = optimization_core::OptimizationSelections::new([]).unwrap();
    native_realization::optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        native_realization::compiler_baseline_request_v1(&selections),
    )
    .expect("source-produced restored-parent contract survives Omega admission")
}

fn published_text(source: &str, target: NativeTarget) -> (Vec<u8>, usize) {
    published_text_for(source, "forward", target)
}

fn native_text_for(
    source: &str,
    entry: &str,
    target: NativeTarget,
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

fn published_text_for(source: &str, entry: &str, target: NativeTarget) -> (Vec<u8>, usize) {
    let placed = native_text_for(source, entry, target);
    let entry = placed.text_section().semantic_entry;
    let container = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(placed).unwrap(),
    );
    let object = image_emission::build_function_fragment_object_artifact(container.clone())
        .unwrap_or_else(|error| panic!("{target:?}: object publication: {error:?}\n{source}"));
    image_emission::validate_function_fragment_object_artifact(&container, &object).unwrap();
    let entry_offset = object
        .functions()
        .iter()
        .find(|function| function.machine == entry)
        .unwrap()
        .text_offset;
    let image = image_emission::emit_executable_image(&object, 3).unwrap();
    image_emission::validate_executable_image(&object, &image).unwrap();
    let installed = image_emission::build_installation_record(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .unwrap();
    let bytes = image_emission::encode_installation_record(&installed).unwrap();
    let decoded = image_emission::decode_installation_record(&bytes).unwrap();
    image_emission::validate_installation_record(&decoded, &image).unwrap();
    (image.output().final_text_bytes.clone(), entry_offset)
}

/// Write-only child store is its last use; the restored exclusive parent is
/// mutated by the exact next receiver-free call.
const RESTORED_EXCLUSIVE: &str = "machine mutate(value: &mut i32) { value = 2; }
    machine forward(root: &mut i32) {
        let parent: &mut i32 = &mut root;
        let child: &write i32 = &write parent;
        child = 1;
        mutate(parent);
    }";

/// Projected parent: write-only child of records[0] closes early; the restored
/// parent element is mutated by the next call.
const RESTORED_PROJECTED: &str = "machine mutate(value: &mut i32) { value = 2; }
    machine forward(records: &mut [i32; 2]) {
        let parent: &mut i32 = &mut records[0];
        let child: &write i32 = &write parent;
        child = 1;
        mutate(parent);
    }";

/// Sole shared child freezes the parent until its last use; the restored
/// parent is then mutated by the next call.
const RESTORED_SHARED: &str = "machine observe(value: &i32) -> i32 { value }
    machine mutate(value: &mut i32) { value = 1; }
    machine forward(root: &mut i32) {
        let parent: &mut i32 = &mut root;
        let child: &i32 = &parent;
        let _ = observe(child);
        mutate(parent);
    }";

#[test]
fn restored_exclusive_parent_publishes() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let _ = published_text(RESTORED_EXCLUSIVE, target);
    }
}

#[test]
fn restored_projected_parent_publishes() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let _ = published_text(RESTORED_PROJECTED, target);
    }
}

#[test]
fn restored_shared_parent_publishes() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let _ = published_text(RESTORED_SHARED, target);
    }
}

/// Attached projected borrow: the callee mutates the scalar leaf reached
/// through `self.value` in the caller.
const ATTACHED_SCALAR_LEAF: &str = "data Main { value: i32; }
    machine mutate(value: &mut i32) { value = 2; }
    machine Main::main(&mut self) { mutate(&mut self.value); }";

/// Attached projected write-only borrow through the same leaf.
const ATTACHED_WRITE_ONLY_LEAF: &str = "data Main { value: i32; }
    machine fill(out: &write i32) { out = 2; }
    machine Main::main(&mut self) { fill(&write self.value); }";

#[test]
fn restored_exclusive_parent_executes_on_host() {
    let (bytes, entry) = published_text(RESTORED_EXCLUSIVE, NativeTarget::host());
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    assert_c_text(
        &bytes,
        entry,
        r#"
        #include <stdint.h>
        #include <string.h>
        extern void omega_entry(int32_t *root);
        int main(void) {
            struct { uint64_t before; int32_t root; uint64_t after; } frame;
            memset(&frame, 0xa5, sizeof frame);
            frame.root = -7;
            unsigned char expected[sizeof frame];
            memcpy(expected, &frame, sizeof frame);
            frame.root = 2;
            omega_entry(&frame.root);
            return memcmp(expected, &frame, sizeof frame) != 0;
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
        let _ = (bytes, entry);
        eprintln!("SKIP: restored-parent execution requires a supported host");
    }
}

#[test]
fn attached_scalar_leaf_executes_on_host() {
    let (bytes, entry) =
        published_text_for(ATTACHED_SCALAR_LEAF, "Main::main", NativeTarget::host());
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    assert_c_text(
        &bytes,
        entry,
        r#"
        #include <stdint.h>
        #include <string.h>
        extern void omega_entry(int32_t *self_value);
        int main(void) {
            struct { uint64_t before; int32_t value; uint64_t after; } frame;
            memset(&frame, 0xa5, sizeof frame);
            frame.value = -7;
            unsigned char expected[sizeof frame];
            memcpy(expected, &frame, sizeof frame);
            expected[8] = 2;
            expected[9] = 0;
            expected[10] = 0;
            expected[11] = 0;
            omega_entry(&frame.value);
            return memcmp(expected, &frame, sizeof frame) != 0;
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
        let _ = (bytes, entry);
        eprintln!("SKIP: attached scalar-leaf execution requires a supported host");
    }
}

#[test]
fn attached_write_only_leaf_executes_on_host() {
    let (bytes, entry) =
        published_text_for(ATTACHED_WRITE_ONLY_LEAF, "Main::main", NativeTarget::host());
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    assert_c_text(
        &bytes,
        entry,
        r#"
        #include <stdint.h>
        #include <string.h>
        extern void omega_entry(int32_t *self_value);
        int main(void) {
            struct { uint64_t before; int32_t value; uint64_t after; } frame;
            memset(&frame, 0xa5, sizeof frame);
            frame.value = -7;
            unsigned char expected[sizeof frame];
            memcpy(expected, &frame, sizeof frame);
            expected[8] = 2;
            expected[9] = 0;
            expected[10] = 0;
            expected[11] = 0;
            omega_entry(&frame.value);
            return memcmp(expected, &frame, sizeof frame) != 0;
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
        let _ = (bytes, entry);
        eprintln!("SKIP: attached write-only leaf execution requires a supported host");
    }
}

#[test]
fn bisect_machine_lowering() {
    for (entry, source) in [
        ("mutate", "machine mutate(value: &mut i32) { value = 2; }"),
        (
            "replace",
            "machine replace(destination: &write i32, value: i32) { destination = value; }",
        ),
        (
            "forward",
            "machine mutate(value: &mut i32) { value = 2; }
            machine forward(root: &mut i32) { mutate(root); }",
        ),
        (
            "forward",
            "machine mutate(value: &mut i32) { value = 2; }
            machine forward(root: &mut i32) { mutate(&mut root); }",
        ),
        (
            "forward",
            "machine mutate(value: &mut i32) { value = 2; }
            machine forward(root: &mut i32) {
                let parent: &mut i32 = &mut root;
                mutate(parent);
            }",
        ),
        (
            "forward",
            "machine mutate(value: &mut i32) { value = 2; }
            machine forward(root: &mut i32) {
                let parent: &mut i32 = &mut root;
                let child: &write i32 = &write parent;
                child = 1;
                mutate(parent);
            }",
        ),
        (
            "forward",
            "machine observe(value: &i32) -> i32 { value }
            machine mutate(value: &mut i32) { value = 1; }
            machine forward(root: &mut i32) {
                let parent: &mut i32 = &mut root;
                let child: &i32 = &parent;
                let _ = observe(child);
                mutate(parent);
            }",
        ),
    ] {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
        let result =
            terminal_production::TerminalProductionRequest::new(&checked, entry).produce_artifact();
        eprintln!("=== {entry} ===\n{source}\n-> {result:?}\n");
    }
}

#[test]
fn bisect_attached_restored_parent() {
    for (entry, source) in [
        (
            "Main::exercise",
            "data Cell { value: i32; }
            data Main { cell: Cell; }
            machine mutate(value: &mut Cell) { value = Cell { value: 2 }; }
            machine Main::exercise(&mut self) {
                let parent: &mut Cell = &mut self.cell;
                let child: &write Cell = &write parent;
                child.value = 1;
                mutate(parent);
            }",
        ),
        (
            "Main::exercise",
            "data Main { value: i32; }
            machine observe(value: &i32) {}
            machine mutate(value: &mut i32) { value = 1; }
            machine Main::exercise(&mut self) {
                let parent: &mut i32 = &mut self.value;
                let child: &i32 = &parent;
                observe(child);
                mutate(parent);
            }",
        ),
        (
            "Main::exercise",
            "data Main { value: i32; }
            machine mutate(value: &mut i32) { value = 1; }
            machine Main::exercise(&mut self) {
                let parent: &mut i32 = &mut self.value;
                mutate(parent);
            }",
        ),
    ] {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
        let result =
            terminal_production::TerminalProductionRequest::new(&checked, entry).produce_artifact();
        match &result {
            Ok(_) => eprintln!("=== {entry} ===\n{source}\n-> Ok\n"),
            Err(error) => eprintln!("=== {entry} ===\n{source}\n-> {error:?}\n"),
        }
    }
}

#[test]
fn bisect_attached_via_main() {
    for (entry, source) in [
        (
            "Main::main",
            "data Main { value: i32; }
            machine mutate(value: &mut i32) { value = 1; }
            machine Main::exercise(&mut self) {
                let parent: &mut i32 = &mut self.value;
                mutate(parent);
            }
            machine Main::main(&mut self) { self.exercise(); }",
        ),
        (
            "Main::main",
            "data Cell { value: i32; }
            data Main { cell: Cell; }
            machine mutate(value: &mut Cell) { value = Cell { value: 2 }; }
            machine Main::exercise(&mut self) {
                let parent: &mut Cell = &mut self.cell;
                let child: &write Cell = &write parent;
                child.value = 1;
                mutate(parent);
            }
            machine Main::main(&mut self) { self.exercise(); }",
        ),
        (
            "Main::main",
            "data Main { value: i32; }
            machine observe(value: &i32) {}
            machine mutate(value: &mut i32) { value = 1; }
            machine Main::exercise(&mut self) {
                let parent: &mut i32 = &mut self.value;
                let child: &i32 = &parent;
                observe(child);
                mutate(parent);
            }
            machine Main::main(&mut self) { self.exercise(); }",
        ),
    ] {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
        let result =
            terminal_production::TerminalProductionRequest::new(&checked, entry).produce_artifact();
        match &result {
            Ok(_) => eprintln!("=== {entry} ===\n{source}\n-> Ok\n"),
            Err(error) => eprintln!("=== {entry} ===\n{source}\n-> {error:?}\n"),
        }
    }
}

#[test]
fn bisect_attached_calls_free() {
    for (entry, source) in [
        (
            "Main::main",
            "data Main { value: i32; }
            machine mutate(value: &mut i32) { value = 1; }
            machine Main::main(&mut self) { mutate(&mut self.value); }",
        ),
        (
            "Main::main",
            "data Main { value: i32; }
            machine Main::exercise(&mut self) { self.value = 1; }
            machine Main::main(&mut self) { self.exercise(); }",
        ),
        (
            "Main::main",
            "data Main { value: i32; }
            machine mutate(value: &mut i32) { value = 1; }
            machine Main::exercise(&mut self) { mutate(&mut self.value); }
            machine Main::main(&mut self) { self.exercise(); }",
        ),
        (
            "Main::main",
            "data Main { value: i32; }
            machine mutate(value: &mut i32) { value = 1; }
            machine Main::exercise(&mut self) {
                let child: &write i32 = &write self.value;
                child = 1;
                mutate(&mut self.value);
            }
            machine Main::main(&mut self) { self.exercise(); }",
        ),
    ] {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
        let result =
            terminal_production::TerminalProductionRequest::new(&checked, entry).produce_artifact();
        match &result {
            Ok(_) => eprintln!("=== {entry} ===\n{source}\n-> Ok\n"),
            Err(error) => eprintln!("=== {entry} ===\n{source}\n-> {error:?}\n"),
        }
    }
}

#[test]
fn bisect_write_only_scalar_leaf() {
    for (entry, source) in [
        ("Main::main", "data Main { value: i32; }
            machine fill(out: &write i32) { out = 1; }
            machine Main::main(&mut self) { fill(&write self.value); }"),
        ("Main::main", "data Main { value: i32; }
            machine fill(out: &write i32) { out = 1; }
            machine Main::exercise(&mut self) { fill(&write self.value); }
            machine Main::main(&mut self) { self.exercise(); }"),
        ("Main::main", "data Main { value: i32; }
            machine Main::main(&mut self) { let child: &write i32 = &write self.value; child = 1; }"),
        ("forward", "machine fill(out: &write i32) { out = 1; }
            machine forward(root: &write i32) { fill(&write root); }"),
        ("forward", "machine fill(out: &write i32) { out = 1; }
            machine forward(root: &write i32) { fill(root); }"),
    ] {
        let tokens = source_files_to_tokens::Lexer::new(source).tokenize().unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
        let result = terminal_production::TerminalProductionRequest::new(&checked, entry)
            .produce_artifact();
        match &result {
            Ok(_) => eprintln!("=== {entry} ===\n{source}\n-> Ok\n"),
            Err(error) => eprintln!("=== {entry} ===\n{source}\n-> {error:?}\n"),
        }
    }
}
