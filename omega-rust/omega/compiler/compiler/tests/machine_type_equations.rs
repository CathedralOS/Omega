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

fn assert_recovered_executes_without_source(source: &str, expected: u64) {
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
}

#[test]
fn machine_range_equation_recovers_omitted_capacity_through_terminal() {
    assert_recovered_executes_without_source(
        include_str!("../../../../../tests/omega/pass/generics/machine_type_equations/main.omg"),
        256,
    );
}

#[test]
fn machine_range_equations_check_complete_tuples_and_canonical_ranges() {
    for arguments in ["u64[0..=256]", "u64[0..257], 256", "u64[0..=256], 256"] {
        assert_recovered_executes_without_source(
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
    assert_recovered_executes_without_source(
        r#"
        machine capacity<Backing, Element, const Count: u64>() -> u64
        where Backing == [Element; Count]
        { Count }
        machine recovered() -> u64 { capacity<[u8; 7]>() }
    "#,
        7,
    );
}

fn assert_equation_rejects(source: &str, expected: &str) {
    let fixture = SourceFixture::new(source);
    let diagnostics = fixture
        .check()
        .map(|_| ())
        .expect_err("an undischarged machine equation must reject");
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
    assert_recovered_executes_without_source(
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
    assert_recovered_executes_without_source(source, 256);
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
