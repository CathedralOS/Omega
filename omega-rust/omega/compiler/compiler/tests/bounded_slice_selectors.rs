//! Generic endpoint contracts survive source checking, portable replay and
//! native execution. The source is removed before either consumer runs.

use compiler::{CheckedCompileRequest, compile_to_checked};
use package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, PackageKeyIdentity};
use std::sync::atomic::{AtomicU64, Ordering};
use terminal_interpreter::{TerminalExecutionResult, TerminalScalarValue};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

#[path = "../../../../../tests/native-differential/tests/common/native_function.rs"]
#[allow(dead_code)]
mod native_function;

fn check_source(
    source: &str,
) -> Result<compiler::CheckedCompilation, Vec<diagnostics::Diagnostic>> {
    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "omega-bounded-slice-selectors-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir(&root).expect("isolated source directory");
    let main = root.join("main.omg");
    std::fs::write(&main, source).expect("generic endpoint source");
    let package = PackageKeyIdentity::from_digest([1; 32]).unwrap();
    let inputs = PackageCompilationInputs::new_package(
        package,
        vec![PackageSourceBinding::new(
            package,
            "bounded-endpoints",
            root.clone(),
        )],
        Vec::new(),
    )
    .unwrap();
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&main, Some("linux_x86_64"))
    });
    std::fs::remove_file(&main).unwrap();
    std::fs::remove_dir(&root).unwrap();
    assert!(!root.exists());
    checked
}

fn checked_source(source: &str) -> compiler::CheckedCompilation {
    check_source(source)
        .unwrap_or_else(|diagnostics| panic!("bounded endpoint source: {diagnostics:#?}"))
}

#[test]
fn bounded_generic_endpoints_execute_after_source_removal() {
    let checked = checked_source(
        r#"
        machine endpoint<const N: u64>() -> u64 [0..=3]
        requires N <= 3
        { N }
        machine main() -> u64 {
            let first: u64 = endpoint<2>();
            let second: u64 = endpoint<3>();
            transition first == 2 && second == 3 { true -> 7 false -> 0 }
        }
    "#,
    );
    assert_endpoint_executes(checked);
}

#[test]
fn typed_range_endpoint_drives_inferred_capacity_through_native_execution() {
    assert_endpoint_executes(checked_source(TYPED_RANGE_SOURCE));
}

#[test]
fn inferred_endpoint_type_drives_capacity_through_native_execution() {
    let source = TYPED_RANGE_SOURCE.replace("identity<u64>(7)", "identity(7u64)");
    assert_endpoint_executes(checked_source(&source));
}

#[test]
fn partially_explicit_endpoint_keeps_its_selected_type_through_native_execution() {
    let source = format!(
        "machine pick<T, Other>(value: T, ignored: Other) -> T {{ value }} {}",
        TYPED_RANGE_SOURCE.replace("identity<u64>(7)", "pick<u64[0..=7]>(7u64, true)")
    );
    assert_endpoint_executes(checked_source(&source));
}

#[test]
fn inferred_endpoint_arguments_cannot_conflict_or_override_an_explicit_type() {
    for (application, diagnostic) in [
        ("pick(7u64, 7u8)", "not specialized"),
        ("pick<u8>(7u64, 7u8)", "destination carrier"),
        ("pick<u64[0..=6]>(7u64, 7u64)", "outside declared range"),
    ] {
        let source = format!(
            "machine pick<T>(left: T, right: T) -> T {{ left }} {}",
            TYPED_RANGE_SOURCE.replace("identity<u64>(7)", application)
        );
        let errors = check_source(&source)
            .map(|_| ())
            .expect_err("inference must preserve binder identity and argument obligations");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains(diagnostic)),
            "{application}: {errors:?}"
        );
    }
}

#[test]
fn structural_type_endpoint_keeps_its_caller_context_through_native_execution() {
    let source = TYPED_RANGE_SOURCE.replace("identity<u64>(7)", "identity<u64[0..=7]>(7)");
    assert_endpoint_executes(checked_source(&source));
}

#[test]
fn computed_structural_type_bound_precedes_endpoint_specialization_and_execution() {
    let source = format!(
        "machine limit() -> u64 {{ 7 }} {}",
        TYPED_RANGE_SOURCE.replace("identity<u64>(7)", "identity<u64[0..=limit()]>(7)")
    );
    assert_endpoint_executes(checked_source(&source));
}

#[test]
fn transitive_computed_type_bound_precedes_helper_execution() {
    let source = format!(
        "machine limit() -> u64 {{ 7 }}
         machine wrapper() -> u64 {{ identity<u64[0..=limit()]>(7) }} {}",
        TYPED_RANGE_SOURCE.replace("identity<u64>(7)", "wrapper()")
    );
    assert_endpoint_executes(checked_source(&source));
}

#[test]
fn computed_structural_type_bound_rejects_an_out_of_range_argument() {
    let source = format!(
        "machine limit() -> u64 {{ 6 }} {}",
        TYPED_RANGE_SOURCE.replace("identity<u64>(7)", "identity<u64[0..=limit()]>(7)")
    );
    let errors = check_source(&source)
        .map(|_| ())
        .expect_err("the computed type range still constrains invocation arguments");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("outside declared range")),
        "{errors:?}"
    );
}

#[test]
fn unused_structural_type_argument_cannot_erase_an_invalid_computed_bound() {
    let source = format!(
        "machine limit() -> u64 {{ 7 }} machine ignored<T>() -> u64 {{ 7 }} {}",
        TYPED_RANGE_SOURCE.replace("identity<u64>(7)", "ignored<u64[0..=limit() / 0]>()")
    );
    let errors = check_source(&source)
        .map(|_| ())
        .expect_err("an unused type binder must retain its invalid range computation");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("range endpoint")),
        "{errors:?}"
    );
}

#[test]
fn structural_type_endpoint_rejects_its_argument_outside_the_selected_range() {
    let source = TYPED_RANGE_SOURCE.replace("identity<u64>(7)", "identity<u64[0..=6]>(7)");
    let errors = check_source(&source)
        .map(|_| ())
        .expect_err("closed structural type arguments retain their value obligations");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("outside declared range")),
        "{errors:?}"
    );
}

const TYPED_RANGE_SOURCE: &str = r#"
        machine identity<T>(value: T) -> T { value }
        machine capacity<const N: u64>(value: u64[0..=N]) -> u64 { N }
        machine main() -> u64 {
            let value: u64[0..=identity<u64>(7)] = 3;
            capacity(value)
        }
    "#;

#[test]
fn policy_endpoint_values_preserve_each_operation_through_native_execution() {
    for (policy, initial, operation, expected) in [
        ("Wrapping", "250", "+ 13", "7"),
        ("Saturating", "250", "+ 13", "255"),
    ] {
        let source = format!(
            "machine seed() -> u8 in {policy} {{ {initial} }}
             machine adjust(value: u8 in {policy}) -> u8 in {policy} {{ value {operation} }}
             machine exact(value: u8 in {policy}) -> u64 {{ value as u64 }}
             machine capacity<const N: u64>(value: u64[0..=N]) -> u64 {{ N }}
             machine main() -> u64 {{
                 let value: u64[0..=exact(adjust(seed()))] = 3;
                 let composed: u64[0..=exact(seed() {operation})] = 3;
                 let direct: u64[0..=seed() {operation}] = 3;
                 let joined: u64[0..=exact((match true {{ true -> seed(), false -> seed() }}) {operation})] = 3;
                 transition capacity(value) == {expected}
                     && capacity(composed) == {expected}
                     && capacity(direct) == {expected}
                     && capacity(joined) == {expected} {{ true -> 7 false -> 0 }}
             }}"
        );
        assert_endpoint_executes(checked_source(&source));
    }
}

#[test]
fn policy_endpoint_values_reject_implicit_policy_changes_and_invalid_landing() {
    for (argument, parameter) in [
        ("seed()", "u8"),
        ("seed()", "u8 in Saturating"),
        ("7u8", "u8 in Wrapping"),
        ("256", "u8 in Wrapping"),
        ("7 / 2", "u8 in Saturating"),
    ] {
        let source = format!(
            "machine seed() -> u8 in Wrapping {{ 7 }}
             machine exact(value: {parameter}) -> u64 {{ value as u64 }}
             machine main() -> u64 {{
                 let value: u64[0..=exact({argument})] = 3;
                 value
             }}"
        );
        assert!(
            check_source(&source).is_err(),
            "policy conversion admitted: {argument} -> {parameter}"
        );
    }
}

#[test]
fn data_field_computed_bound_drives_capacity_through_native_execution() {
    assert_endpoint_executes(checked_source(DATA_FIELD_RANGE_SOURCE));
}

#[test]
fn data_field_nested_computed_bound_keeps_its_static_obligations() {
    let source = format!(
        "machine limit() -> u64 {{ 7 }} {}",
        DATA_FIELD_RANGE_SOURCE.replace("identity<u64[0..=7]>", "identity<u64[0..=limit()]>")
    );
    assert_endpoint_executes(checked_source(&source));
}

#[test]
fn data_field_computed_bound_rejects_invalid_arguments_and_field_contents() {
    for (source, diagnostic) in [
        (
            DATA_FIELD_RANGE_SOURCE.replace("identity<u64[0..=7]>", "identity<u64[0..=6]>"),
            "outside declared range",
        ),
        (
            DATA_FIELD_RANGE_SOURCE.replace("length: 3", "length: 8"),
            "range",
        ),
    ] {
        let errors = check_source(&source)
            .map(|_| ())
            .expect_err("field-bound folding must retain argument and construction obligations");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains(diagnostic)),
            "{errors:?}"
        );
    }
}

const DATA_FIELD_RANGE_SOURCE: &str = r#"
        machine identity<T>(value: T) -> T { value }
        data Buffer { length: u64[0..=identity<u64[0..=7]>(7)]; }
        machine capacity<const N: u64>(value: u64[0..=N]) -> u64 { N }
        machine main() -> u64 {
            let buffer: Buffer = Buffer { length: 3 };
            capacity(buffer.length)
        }
        "#;

#[test]
fn typed_range_endpoint_rejects_value_beyond_computed_bound() {
    let source = TYPED_RANGE_SOURCE.replace("= 3;", "= 8;");
    let errors = check_source(&source)
        .map(|_| ())
        .expect_err("folded range must still constrain its stored value");
    assert!(
        errors.iter().any(|error| error.message.contains("range")),
        "{errors:#?}"
    );
}

fn assert_endpoint_executes(checked: compiler::CheckedCompilation) {
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("main"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("bounded endpoint calls publish Terminal")
    .into_artifact();
    drop(checked);
    assert_eq!(
        terminal_interpreter::interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .expect("source-free endpoint execution"),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            value: IntegerValue::Unsigned(7),
        }),
    );

    let selections = optimization_core::OptimizationSelections::new([]).unwrap();
    let optimized = native_realization::optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        native_realization::compiler_baseline_request_v1(&selections),
    )
    .unwrap();
    let physical =
        native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target::NativeTarget::host(),
            &[],
        )
        .unwrap();
    let fragments = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .unwrap();
    let framed = machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
    let text = machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
    let source = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(text).unwrap(),
    );
    let object = image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
    image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
    let image = image_emission::emit_direct_executable_image(&object, 3).unwrap();
    image_emission::validate_direct_executable_image(&object, &image).unwrap();
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    native_function::assert_c_text(
        &image.output().final_text_bytes,
        object.entry_function().text_offset,
        "#include <stdint.h>\nextern uint64_t omega_entry(void);\nint main(void) { return omega_entry() == 7 ? 0 : 1; }",
    );
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!("SKIP: native function execution requires a supported Linux or macOS host");
}

#[test]
fn inferred_slice_selectors_check_but_await_structural_control_plan() {
    let checked = checked_source(
        r#"
        data Main {}
        machine Main::endpoint<const N: u64>(&self, witness: &[u8; N]) -> u64 [0..=3]
        requires N <= 3
        { N }
        machine Main::window(&self, items: &[i32; 4]) -> u64 {
            let pair: [u8; 2] = [0, 0];
            let triple: [u8; 3] = [0, 0, 0];
            let pair_view: &[i32] = items[..self.endpoint(&pair)];
            let triple_view: &[i32] = items[..self.endpoint(&triple)];
            transition pair_view.len == 2 && triple_view.len == 3 { true -> 7 false -> 0 }
        }
        machine main() -> u64 {
            let selector: Main = Main {};
            let values: [i32; 4] = [11, 7, 6, 22];
            selector.window(&values)
        }
    "#,
    );
    // Keep the original slice customer visible beside its working scalar
    // dependency. Checked/interpreter success must not masquerade as native
    // slice support; replace these stops with execution when lowering lands.
    // Each machine stops at its own omission site: the endpoint's `&[u8; N]`
    // witness signature is not an admitted body, `window`'s slice-typed local
    // is a statement kind the sequence cannot bind, and `main` has no checked
    // scalar control plan at all.
    for (machine, stop) in [
        (
            "Main::endpoint",
            "`Main::endpoint` has no admitted body (local construction stopped at signature)",
        ),
        (
            "Main::window",
            "`Main::window` has no admitted body (local construction stopped at statement sequence: unsupported statement kind, statement 2)",
        ),
        (
            "main",
            "machine has no source-independent checked scalar control plan",
        ),
    ] {
        let error = terminal_production::TerminalProductionRequest::new(
            &checked,
            TerminalMachineSelection::Name(machine),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect_err("slice control-plan production remains unfinished")
        .into_parts()
        .0;
        assert!(
            format!("{error:?}").contains(stop),
            "{machine} stopped at a different omission: {error:?}",
        );
    }
}
