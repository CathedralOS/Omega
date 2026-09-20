//! Floating constant tables retain their declared format and representation.

use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

#[path = "../../../../../tests/native-differential/tests/common/native_function.rs"]
#[allow(dead_code)]
mod native_function;

struct Sources(std::path::PathBuf);

impl Sources {
    fn new(declarations: &str, body: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-constant-float-tables-{}-{}",
            std::process::id(),
            NEXT_PROJECT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("settings.omg"), declarations).unwrap();
        std::fs::write(root.join("main.omg"), body).unwrap();
        Self(root)
    }
}

impl Drop for Sources {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn floating_tables_execute_after_source_removal() {
    for carrier in ["f32", "f64"] {
        for (projection, negative_zero) in [
            ("COPIED[1].value", true),
            ("AGAIN[0].value", false),
            ("VALUES[1]", true),
            ("NESTED[1][0]", false),
        ] {
            let declarations = format!(
                "module settings;
             pub data Cell<T [copy]> [copy] {{ value: T; }}
             pub const TABLE: [Cell<{carrier}>; 2] = [
                 Cell {{ value: 1.5{carrier} }}, Cell {{ value: -0.0{carrier} }}
             ];
             pub const COPIED: [Cell<{carrier}>; 2] = TABLE;
             pub const AGAIN: [Cell<{carrier}>; 2] = COPIED;
             pub const VALUES: [{carrier}; 2] = [1.5{carrier}, -0.0{carrier}];
             pub const NESTED: [[{carrier}; 1]; 2] = [[-0.0{carrier}], [1.5{carrier}]];"
            );
            let body =
                format!("use settings; machine read() -> {carrier} {{ settings::{projection} }}");
            let sources = Sources::new(&declarations, &body);
            let path = sources.0.join("main.omg");
            let checked =
                compiler::compile_to_checked(compiler::CheckedCompileRequest::new(&path, None))
                    .expect("copied floating record table checks");
            let artifact = terminal_production::TerminalProductionRequest::new(&checked, "read")
                .produce_artifact()
                .expect("floating table read reaches Terminal");
            drop(checked);
            drop(sources);
            assert!(!path.exists());
            let artifact =
                terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
                    .unwrap();
            let (c_type, bits_type, expected) = if carrier == "f32" {
                (
                    "float",
                    "uint32_t",
                    if negative_zero {
                        "UINT32_C(0x80000000)"
                    } else {
                        "UINT32_C(0x3fc00000)"
                    },
                )
            } else {
                (
                    "double",
                    "uint64_t",
                    if negative_zero {
                        "UINT64_C(0x8000000000000000)"
                    } else {
                        "UINT64_C(0x3ff8000000000000)"
                    },
                )
            };
            publish_and_execute(
                &artifact,
                &format!(
                    "#include <stdint.h>\n#include <string.h>\nextern {c_type} omega_entry(void);\n\
             int main(void) {{ {c_type} value = omega_entry(); {bits_type} bits;\n\
             memcpy(&bits, &value, sizeof(bits)); return bits == {expected} ? 0 : 1; }}"
                ),
            );
        }
    }
}

fn publish_and_execute(artifact: &terminal_codec::CanonicalTerminalArtifact, driver: &str) {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let selections = optimization_core::OptimizationSelections::default();
        let optimized = native_realization::optimize_artifact_sections(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            native_realization::compiler_baseline_request_v1(&selections),
        )
        .unwrap();
        let physical = native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized, target, &[],
        ).unwrap();
        let fragments = machine_emission::stage_optimized_function_fragment_emission(
            physical.into_function_fragment_emission_source(),
        )
        .unwrap();
        let framed =
            machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
        let text = machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
        let source = std::sync::Arc::new(
            object_file::stage_optimized_relocation_free_object_container(text).unwrap(),
        );
        let object =
            image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
        image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
        let image = image_emission::emit_executable_image(&object, 0).unwrap();
        image_emission::validate_executable_image(&object, &image).unwrap();
        #[cfg(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64"),
        ))]
        if target == target::NativeTarget::host() {
            native_function::assert_c_text(
                &image.output().final_text_bytes,
                object.entry_function().text_offset,
                driver,
            );
        }
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    {
        let _ = driver;
        eprintln!("SKIP: floating table native execution requires a supported Linux or macOS host");
    }
}

#[test]
fn floating_aggregate_leaves_cannot_change_their_landed_carrier() {
    for initializer in ["1.5f64", "1u32"] {
        let source = format!(
            "module settings; pub data Cell<T [copy]> [copy] {{ value: T; }}
             pub const TABLE: [Cell<f32>; 1] = [Cell {{ value: {initializer} }}];"
        );
        let sources = Sources::new(&source, "use settings; machine read() -> u64 { 0 }");
        let error = compiler::compile_to_checked(compiler::CheckedCompileRequest::new(
            &sources.0.join("main.omg"),
            None,
        ))
        .err()
        .expect("unused table still owes exact floating carrier");
        assert!(
            error
                .iter()
                .any(|diagnostic| diagnostic.message.contains("floating carrier")),
            "{error:?}"
        );
    }
}

#[test]
fn floating_declaration_values_remain_ineligible_as_indices() {
    for (declaration, value) in [
        ("data Cell [copy] { value: f32; }", "Cell { value: 1.5f32 }"),
        (
            "data Cell [copy] { case Empty; case Value(value: f32); }",
            "Cell::Empty",
        ),
    ] {
        let source = format!(
            "{declaration} const VALUE: Cell = {value};
             data Key<const K: Cell> [copy] {{ value: u64; }}
             machine read(value: Key<VALUE>) -> u64 {{ value.value }}"
        );
        let sources = Sources::new("module settings;", &source);
        let error = compiler::compile_to_checked(compiler::CheckedCompileRequest::new(
            &sources.0.join("main.omg"),
            None,
        ))
        .err()
        .expect("floating fields cannot become static indices, even in an inactive case");
        assert!(
            error
                .iter()
                .any(|diagnostic| diagnostic.message.contains("const index")),
            "{error:?}"
        );
    }
}
