//! Validated function-text execution, not native artifact or provider publication.
use std::path::Path;

use native_realization::{compiler_baseline_request_v1, optimize_artifact_sections};
use optimization_core::OptimizationSelections;
use proof_admission::AdmissionProfile;
use target::NativeTarget;
use terminal_psi::OperationKind;

#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(target_os = "macos", target_arch = "aarch64")
))]
#[path = "common/native_function.rs"]
#[allow(dead_code)]
mod native_function;

fn checked(path: &Path, target_name: &str) -> compiler::CheckedCompilation {
    compiler::compile_to_checked(path, Some(target_name)).unwrap_or_else(|diagnostics| {
        panic!("checked selected comparison for {target_name}: {diagnostics:#?}")
    })
}

fn text(
    checked: &compiler::CheckedCompilation,
    entry: &str,
    target: NativeTarget,
    expected_comparisons: usize,
) -> (Vec<u8>, usize) {
    let lowered = checked_trees_to_lowered_psi::lower_machine(checked, entry).unwrap();
    compiler::validate_lowered_ieee_float_comparison_custody(checked, &lowered)
        .expect("source comparison joins exact selected provider and operands");
    let produced =
        terminal_production::produce_terminal_artifact_with_checked_boundary_operator_scope(
            checked, entry,
        )
        .expect("canonical Terminal with retained exact checked scope");
    assert_eq!(
        produced.selected_ieee_float_comparison_occurrences().len(),
        expected_comparisons
    );
    assert_eq!(
        produced.selected_ieee_float_comparison_occurrences(),
        lowered.selected_ieee_float_comparison_occurrences
    );
    let artifact = produced.artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert_eq!(module, lowered.semantic_module);
    let mut authored_calls = module
        .machines
        .iter()
        .flat_map(|machine| {
            machine.blocks.iter().flat_map(move |block| {
                block
                    .operations
                    .iter()
                    .filter_map(move |operation| match operation.kind {
                        OperationKind::Call { callee, .. }
                        | OperationKind::CallStructuralScalar { callee, .. } => {
                            Some((machine.id, operation.id, callee))
                        }
                        _ => None,
                    })
            })
        })
        .collect::<Vec<_>>();
    let selections = OptimizationSelections::new([]).unwrap();
    let optimized = optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .unwrap_or_else(|error| panic!("admit {entry} {target:?}: {error:#?}"));
    let post_terminal = optimized.selections().project_post_terminal();
    let target_operations =
        abstract_operations_to_target_operations::lower_optimized_to_target_operations(
            optimized, target,
        )
        .unwrap_or_else(|error| panic!("ordinary target graph {entry} {target:?}: {error:#?}"));
    assert!(
        target_operations
            .target_operations()
            .functions
            .iter()
            .all(|function| !function.graph.blocks.is_empty())
    );
    let physical = native_realization::stage_optimized_verified_physical_pipeline(
        target_operations,
        post_terminal.selections(),
    )
    .unwrap_or_else(|error| panic!("physical graph {entry} {target:?}: {error:#?}"));
    let fragments = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .unwrap();
    let framed = machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
    let text = machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
    machine_emission::validate_optimized_fixed_frame_text_section(&text)
        .expect("independent full function-text replay");
    let mut calls = text
        .text_section()
        .resolved_internal_machine_calls
        .iter()
        .map(|call| (call.caller, call.operation, call.callee))
        .collect::<Vec<_>>();
    // Physical block placement need not follow the authored block roster.
    // Compare exact occurrence identities; graph replay and runtime results
    // independently check which arms execute and in what control-flow order.
    calls.sort_unstable();
    authored_calls.sort_unstable();
    assert_eq!(
        calls, authored_calls,
        "reached-arm call roster retains original occurrences"
    );
    assert_eq!(text.text_section().functions.len(), module.machines.len());
    (
        text.text_section().bytes.clone(),
        usize::try_from(text.text_section().semantic_entry_offset).unwrap(),
    )
}

fn execute_host(bytes: &[u8], offset: usize, native: NativeTarget, driver: &str) {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    if native == NativeTarget::host() {
        native_function::assert_c_text(bytes, offset, driver);
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    {
        let _ = (bytes, offset, native, driver);
        eprintln!(
            "SKIP: C execution requires Linux x86-64/AArch64 or macOS AArch64; four-target text replay remains enabled"
        );
    }
}

fn targets() -> [(NativeTarget, &'static str); 4] {
    [
        (NativeTarget::linux_x64(), "linux_x86_64"),
        (NativeTarget::linux_arm64(), "linux_arm64"),
        (NativeTarget::macos_arm64(), "macos_arm64"),
        (NativeTarget::windows_x64(), "windows_x86_64"),
    ]
}

#[test]
fn selected_float_match_call_graph_cross_replays_and_executes_on_host() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../omega/pass/expressions/match_float_patterns/main.omg");
    for (native, name) in targets() {
        let checked = checked(&source, name);
        let (bytes, offset) = text(&checked, "choose", native, 2);
        execute_host(
            &bytes,
            offset,
            native,
            include_str!("ieee_comparisons/choose.c"),
        );
    }
}

#[test]
fn six_ieee_relations_both_formats_cross_replay_and_match_host_ieee() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ieee_comparisons/relations.omg");
    for (native, name) in targets() {
        let checked = checked(&source, name);
        for (relation, operator) in [
            ("equal", "=="),
            ("not_equal", "!="),
            ("less", "<"),
            ("less_or_equal", "<="),
            ("greater", ">"),
            ("greater_or_equal", ">="),
        ] {
            for (bits, scalar, integer) in [(32, "float", "uint32_t"), (64, "double", "uint64_t")] {
                let entry = format!("{relation}{bits}");
                let (bytes, offset) = text(&checked, &entry, native, 1);
                let driver = include_str!("ieee_comparisons/relations.c")
                    .replace("FLOAT_TYPE", scalar)
                    .replace("BITS_TYPE", integer)
                    .replace("FORMAT_BITS", &bits.to_string())
                    .replace("COMPARE_OPERATOR", operator);
                execute_host(&bytes, offset, native, &driver);
            }
        }
    }
}

#[test]
fn ten_float_arguments_cross_replay_and_return_stack_argument_bits_on_host() {
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ieee_comparisons/stack_arguments.omg");
    for (native, name) in targets() {
        let checked = checked(&source, name);
        for (bits, scalar, integer) in [(32, "float", "uint32_t"), (64, "double", "uint64_t")] {
            let (bytes, offset) = text(&checked, &format!("forward{bits}"), native, 0);
            let driver = include_str!("ieee_comparisons/stack_arguments.c")
                .replace("FLOAT_TYPE", scalar)
                .replace("BITS_TYPE", integer)
                .replace("FORMAT_BITS", &bits.to_string());
            execute_host(&bytes, offset, native, &driver);
        }
    }
}
