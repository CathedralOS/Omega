//! Machine equations select one exact tuple before publishing executable Terminal.

use compiler::{CheckedCompilation, CheckedCompileRequest, compile_to_checked};
use diagnostics::Diagnostic;
use package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, PackageKeyIdentity};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};

#[path = "../../../../../tests/native-differential/tests/common/native_function.rs"]
#[allow(dead_code)]
mod native_function;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct SourceFixture {
    root: PathBuf,
}

impl SourceFixture {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-machine-type-equations-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).expect("create isolated source fixture");
        fs::write(root.join("main.omg"), source).expect("write machine-equation source");
        Self { root }
    }

    fn check(&self) -> Result<CheckedCompilation, Vec<Diagnostic>> {
        let package = PackageKeyIdentity::from_digest([1; 32]).expect("fixture package identity");
        let inputs = PackageCompilationInputs::new_package(
            package,
            vec![PackageSourceBinding::new(
                package,
                "machine-equations",
                self.root.clone(),
            )],
            Vec::new(),
        )
        .expect("one isolated source package");
        compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(inputs),
            ..CheckedCompileRequest::new(&self.root.join("main.omg"), Some("linux_x86_64"))
        })
    }
}

impl Drop for SourceFixture {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn assert_recovered_executes_without_source(
    source: &str,
    expected: u64,
) -> terminal_codec::CanonicalTerminalArtifact {
    let fixture = SourceFixture::new(source);
    let checked = fixture.check().unwrap_or_else(|diagnostics| {
        panic!(
            "machine equation source must check at {}: {diagnostics:?}",
            fixture.root.display()
        )
    });
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "recovered")
        .produce_artifact()
        .expect("recovered constant reaches Terminal publication");
    drop(checked);
    let source_root = fixture.root.clone();
    drop(fixture);
    assert!(!source_root.exists());
    assert_eq!(
        interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .expect("independently decoded machine executes without source"),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 carrier"),
            value: IntegerValue::Unsigned(u128::from(expected)),
        }),
    );
    artifact
}

#[test]
fn machine_range_equation_recovers_omitted_capacity_through_terminal() {
    let _ = assert_recovered_executes_without_source(
        include_str!("../../../../../tests/omega/pass/generics/machine_type_equations/main.omg"),
        256,
    );
}

#[test]
fn machine_range_equations_check_complete_tuples_and_canonical_ranges() {
    for arguments in ["u64[0..=256]", "u64[0..257], 256", "u64[0..=256], 256"] {
        let _ = assert_recovered_executes_without_source(
            &format!(
                r#"
            machine capacity<Length, const Capacity: u64>() -> u64
            where Length == u64[0..=Capacity]
            {{ Capacity }}
            machine recovered() -> u64 {{ capacity<{arguments}>() }}
        "#
            ),
            256,
        );
    }
}

#[test]
fn machine_array_equation_recovers_element_and_extent_through_terminal() {
    let _ = assert_recovered_executes_without_source(
        r#"
        machine capacity<Backing, Element, const Count: u64>() -> u64
        where Backing == [Element; Count]
        { Count }
        machine recovered() -> u64 { capacity<[u8; 7]>() }
    "#,
        7,
    );
}

#[test]
fn static_attached_equations_discharge_closed_tuples_during_checking() {
    let source = r#"
        data Buffer {}
        machine Buffer::capacity<Backing, Element, const Count: u64>() -> u64
        where Backing == [Element; Count]
        { Count }
        machine recovered() -> u64 { Buffer::capacity<[u8; 7]>() }
        "#;
    SourceFixture::new(source)
        .check()
        .expect("the complete attached tuple checks");
    assert_equation_rejects(
        &source.replace("capacity<[u8; 7]>()", "capacity<[u8; 7], u8, 8>()"),
        "explicit argument is 8",
    );
}

#[test]
fn receiver_machine_equation_updates_the_original_record() {
    let artifact = assert_recovered_executes_without_source(RECEIVER_EQUATION, 7);
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let selections = optimization_core::OptimizationSelections::new([]).unwrap();
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
        let image = image_emission::emit_executable_image(&object, 3).unwrap();
        image_emission::validate_executable_image(&object, &image).unwrap();
        let record = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
        )
        .unwrap();
        let encoded = image_emission::encode_installation_record(&record).unwrap();
        let decoded = image_emission::decode_installation_record(&encoded).unwrap();
        image_emission::validate_installation_record(&decoded, &image).unwrap();
        if target == target::NativeTarget::host() {
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
            eprintln!(
                "SKIP: attached-equation native execution requires a supported Linux or macOS host"
            );
        }
    }
}

const RECEIVER_EQUATION: &str = r#"
        data Buffer { capacity: u64; }
        machine Buffer::resize<Backing, Element, const Count: u64>(&mut self)
        where Backing == [Element; Count]
        { self.capacity = Count; }
        machine recovered() -> u64 {
            let mut buffer: Buffer = Buffer { capacity: 0 };
            buffer.resize<[u8; 7]>();
            buffer.capacity
        }
        "#;

#[test]
fn attached_machine_equations_reject_conflicting_or_missing_arguments() {
    assert_equation_rejects(
        &RECEIVER_EQUATION.replace("resize<[u8; 7]>()", "resize<[u8; 7], u8, 8>()"),
        "explicit argument is 8",
    );
    assert_equation_rejects(
        &RECEIVER_EQUATION.replace("resize<[u8; 7]>()", "resize<[u8; 7], u16, 7>()"),
        "equation",
    );
    assert_equation_rejects(
        &RECEIVER_EQUATION.replace("resize<[u8; 7]>()", "resize()"),
        "supply",
    );
}

#[test]
fn same_named_receiver_equations_keep_their_selected_owner() {
    let source = format!(
        "data Other {{ capacity: u64; }}
         machine Other::resize<Backing, Element, const Count: u64>(&mut self)
         where Backing == [Element; Count], Element == u16
         {{ self.capacity = Count; }}
         {}",
        RECEIVER_EQUATION.replace(
            "buffer.resize<[u8; 7]>();",
            "let mut other: Other = Other { capacity: 0 };
             buffer.resize<[u8; 7]>();
             other.resize<[u16; 9]>();"
        )
    );
    let _ = assert_recovered_executes_without_source(&source, 7);
    assert_equation_rejects(
        &source.replace("other.resize<[u16; 9]>()", "other.resize<[u8; 9]>()"),
        "equation",
    );
}

#[test]
fn attached_equations_do_not_turn_open_or_noncall_selections_into_closed_calls() {
    let declaration = "data Buffer {} data Element {}
        machine Buffer::capacity<Backing, Element, const Count: u64>() -> u64
        where Backing == [Element; Count] { Count }";
    assert_equation_rejects(
        &format!(
            "{declaration} machine forward<Element>() -> u64 {{ Buffer::capacity<[Element; 7]>() }} machine recovered() -> u64 {{ 0 }}"
        ),
        "open caller binder",
    );
    assert_equation_rejects(
        &format!("{declaration} machine recovered() -> u64 {{ Buffer::capacity }}"),
        "noncall selection",
    );
    assert_equation_rejects(
        &format!(
            "module scope; {declaration} machine recovered() -> u64 {{ scope::Buffer::capacity }}"
        ),
        "noncall selection",
    );
}

#[test]
fn implicit_cleanup_cannot_discard_attached_equations() {
    assert_equation_rejects(
        "data Buffer {}
         machine Buffer::drop<Type>(&mut self) where Type == u8 {}
         machine recovered() -> u64 { let buffer: Buffer = Buffer {}; 0 }",
        "may not declare method-local lifetime or type parameters",
    );
}

#[test]
fn receiver_names_do_not_exempt_nested_calls_from_equation_discharge() {
    let source = RECEIVER_EQUATION
        .replace(
            "machine recovered()",
            "machine Buffer::outer<Backing, Element, const Count: u64>(&mut self)
         where Backing == [Element; Count]
         { self.resize<[u8; 7], u8, 8>(); }
         machine recovered()",
        )
        .replace("buffer.resize<[u8; 7]>()", "buffer.outer<[u8; 7]>()");
    assert_equation_rejects(&source, "explicit argument is 8");
}

#[test]
fn result_receiver_cannot_bypass_equation_discharge() {
    assert_equation_rejects(
        "data Buffer {}
         machine Buffer::capacity<Type>(&self) -> u64 where Type == u8 { 7 }
         machine create() -> Buffer { Buffer {} }
         machine recovered() -> u64 { create().capacity<u16>() }",
        "equation",
    );
}

#[test]
fn open_receiver_cannot_bypass_equation_discharge() {
    assert_equation_rejects(
        "data Buffer {}
         machine Buffer::capacity<Type>(&self) -> u64 where Type == u8 { 7 }
         machine forward<Receiver>(value: &Receiver) -> u64 { value.capacity<u16>() }
         machine recovered() -> u64 { let buffer: Buffer = Buffer {}; forward<Buffer>(&buffer) }",
        "generic callee did not resolve",
    );
}

fn assert_equation_rejects(source: &str, expected: &str) {
    let fixture = SourceFixture::new(source);
    let diagnostics = match fixture.check() {
        Ok(_) => panic!("an undischarged machine equation must reject:\n{source}"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected)),
        "expected {expected:?}, received {diagnostics:?}"
    );
}

#[test]
fn explicit_machine_tuple_cannot_override_exact_equation() {
    assert_equation_rejects(
        r#"
        machine capacity<Length, const Capacity: u64>() -> u64
        where Length == u64[0..=Capacity]
        { Capacity }
        machine recovered() -> u64 { capacity<u64[0..=256], 512>() }
    "#,
        "explicit argument is 512",
    );
}

#[test]
fn machine_equation_rejects_type_value_kind_mixture() {
    assert_equation_rejects(
        r#"
        machine unused<Length, const Capacity: u64>() -> u64
        where Length == Capacity
        { 0 }
        machine recovered() -> u64 { 0 }
        "#,
        "mixes type and value kinds",
    );
    assert_equation_rejects(
        r#"
        machine capacity<Length, const Capacity: u64>() -> u64
        where Length == Capacity
        { Capacity }
        machine recovered() -> u64 { capacity<u64, 256>() }
    "#,
        "mixes type and value kinds",
    );
}

#[test]
fn forwarding_cannot_discard_an_unsolved_machine_equation() {
    assert_equation_rejects(
        r#"
        machine capacity<Length, const Capacity: u64>() -> u64
        where Length == u64[0..=Capacity]
        { Capacity }
        machine forward<Length, const Count: u64>() -> u64 {
            capacity<Length, Count>()
        }
        machine recovered() -> u64 { forward<u64[0..=256], 512>() }
    "#,
        "equation",
    );
}

#[test]
fn machine_equation_does_not_guess_an_unconstrained_capacity() {
    assert_equation_rejects(
        r#"
        machine capacity<Length, const Capacity: u64>() -> u64
        where Length == u64[0..=Capacity]
        { Capacity }
        machine recovered() -> u64 { capacity() }
    "#,
        "supply",
    );
}

#[test]
fn machine_equation_constructs_an_omitted_type_from_closed_const() {
    let _ = assert_recovered_executes_without_source(
        r#"
        machine capacity<const Capacity: u64, Length>() -> u64
        where Length == u64[0..=Capacity]
        { Capacity }
        machine recovered() -> u64 { capacity<256>() }
    "#,
        256,
    );
}

#[test]
fn machine_equations_preserve_named_conformance_obligations() {
    let source = r#"
        trait Marker {}
        data Item {}
        data Unmarked {}
        Primary: Item satisfies Marker {}
        machine capacity<Length, Element, const Capacity: u64>() -> u64
        where Length == u64[0..=Capacity], Element satisfies Item::Primary
        { Capacity }
        machine recovered() -> u64 { capacity<u64[0..257], Item>() }
    "#;
    let _ = assert_recovered_executes_without_source(source, 256);
    assert_equation_rejects(
        &source.replace(
            "capacity<u64[0..257], Item>()",
            "capacity<u64[0..257], Unmarked>()",
        ),
        "conformance",
    );
}

#[test]
fn explicit_structural_type_cannot_bind_a_const_parameter() {
    assert_equation_rejects(
        r#"
        machine constant<const Value: u64, Type>() -> u64 { Value }
        machine recovered() -> u64 { constant<u64[0..=256], 256>() }
        "#,
        "argument",
    );
    assert_equation_rejects(
        r#"
        machine constant<const Value: u64>() -> u64 { Value }
        machine recovered() -> u64 { constant<u64[0..=256]>() }
    "#,
        "argument",
    );
}

#[test]
fn unused_static_type_argument_still_checks_its_lifetime_scope() {
    assert_equation_rejects(
        r#"
        data Holder<'item> { value: &'item u8; }
        machine constant<Type>() -> u64 { 256 }
        machine recovered() -> u64 { constant<[Holder<'missing>; 1]>() }
    "#,
        "lifetime",
    );
}

#[test]
fn range_probe_cannot_execute_a_wrapper_with_an_unsolved_machine_equation() {
    assert_equation_rejects(
        r#"
        machine capacity<Length, const Capacity: u64>() -> u64
        where Length == u64[0..=Capacity]
        { Capacity }
        machine endpoint() -> u64 { capacity<u64[0..=256], 512>() }
        data Box<Length> { value: Length; }
        machine preserve(value: Box<u64[0..=endpoint()]>) -> Box<u64[0..=endpoint()]> {
            value
        }
        machine recovered() -> u64 { 0 }
    "#,
        "equation",
    );
}
