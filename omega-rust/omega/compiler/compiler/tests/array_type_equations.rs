//! Fixed-array equations recover a complete data application before checking,
//! then specialize its attached machine into independently executable Terminal.

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
            "omega-array-type-equations-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).expect("create isolated source fixture");
        fs::write(root.join("main.omg"), source).expect("write array-equation source");
        Self { root }
    }

    fn check(&self) -> Result<CheckedCompilation, Vec<Diagnostic>> {
        let package = PackageKeyIdentity::from_digest([1; 32]).expect("fixture package identity");
        let inputs = PackageCompilationInputs::new_package(
            package,
            vec![PackageSourceBinding::new(
                package,
                "array-equations",
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

fn assert_capacity_executes_without_source(source: &str, expected: u64) {
    let fixture = SourceFixture::new(source);
    let checked = fixture.check().unwrap_or_else(|diagnostics| {
        panic!(
            "array-equation source must check; fixture retained at {}:\n{}",
            fixture.root.display(),
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        )
    });
    let capacities = checked
        .typed
        .machines()
        .iter()
        .filter(|machine| checked.typed.machine_type_parameters(machine).is_empty())
        .map(|machine| checked.symbols.display_path(machine.symbol, "::"))
        .filter(|name| name.ends_with("::capacity"))
        .collect::<Vec<_>>();
    let [capacity] = capacities.as_slice() else {
        panic!(
            "inferred and explicit tuples must select one concrete attached capacity machine: {capacities:?}"
        );
    };
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, capacity)
        .produce_artifact()
        .expect("recovered const argument reaches an attached machine's Terminal product");
    drop(checked);
    let source_root = fixture.root.clone();
    drop(fixture);
    assert!(
        !source_root.exists(),
        "source fixture must be removed before execution"
    );
    assert_eq!(
        interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .expect("independently decoded attached machine executes without source"),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 carrier"),
            value: IntegerValue::Unsigned(u128::from(expected)),
        }),
    );
}

#[test]
fn inferred_array_element_and_extent_reach_checked_identity_and_terminal_method() {
    for count in [4, 7] {
        assert_capacity_executes_without_source(
            &format!(
                r#"
data Buffer<Backing, Element, const Count: u64>
where Backing == [Element; Count]
{{
    storage: Backing;
}}
machine Buffer::capacity<const Count: u64>() -> u64 {{ Count }}

machine preserve(value: Buffer<[u8; {count}]>) -> Buffer<[u8; {count}], u8, {count}> {{ value }}
"#,
            ),
            count,
        );
    }
}

#[test]
fn reverse_array_construction_reaches_checked_identity_and_terminal_method() {
    assert_capacity_executes_without_source(
        r#"
data Buffer<Element, const Count: u64, Backing>
where Backing == [Element; Count]
{
    storage: Backing;
}
machine Buffer::capacity<const Count: u64>() -> u64 { Count }

machine preserve(value: Buffer<u8, 4>) -> Buffer<u8, 4, [u8; 4]> { value }
"#,
        4,
    );
}

#[test]
fn explicit_array_element_or_extent_conflict_rejects_before_terminal() {
    for (arguments, expected) in [
        ("[u8; 4], u16, 4", "conflicting element types"),
        ("[u8; 4], u8, 5", "explicit argument is 5"),
    ] {
        let fixture = SourceFixture::new(&format!(
            r#"
data Buffer<Backing, Element, const Count: u64>
where Backing == [Element; Count]
{{
    storage: Backing;
}}
machine Buffer::capacity<const Count: u64>() -> u64 {{ Count }}
machine preserve(value: Buffer<{arguments}>) -> Buffer<{arguments}> {{ value }}
"#,
        ));
        let diagnostics = fixture
            .check()
            .map(|_| ())
            .expect_err("an explicit argument cannot be overwritten by structural inference");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "expected {expected:?} for {arguments}, received {diagnostics:?}"
        );
    }
}

#[test]
fn array_equations_do_not_discard_other_instantiation_or_runtime_facts() {
    for (source, expected) in [
        (
            r#"
            data Buffer<Backing, Element, const Count: u64>
            where Backing == [Element; Count], Count > 8
            { storage: Backing; }
            machine preserve(value: Buffer<[u8; 4]>) -> Buffer<[u8; 4]> { value }
        "#,
            "is false",
        ),
        (
            r#"
            data Buffer<Backing, const Count: u64>
            where Count == [u8; 4]
            { storage: Backing; }
        "#,
            "mixes type and value kinds",
        ),
        (
            r#"
            data Buffer where [u8; 4] == [u8; 4] { value: u64; }
        "#,
            "not a runtime value",
        ),
    ] {
        let fixture = SourceFixture::new(source);
        let diagnostics = fixture
            .check()
            .map(|_| ())
            .expect_err("undischarged fact rejects");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "expected {expected:?}, received {diagnostics:?}"
        );
    }
}

#[test]
fn array_equations_on_unspecialized_containers_cannot_lose_their_obligation() {
    let fixture = SourceFixture::new(
        r#"
        data Buffer<Backing, Element, const Count: u64>
        where Backing == [Element; Count]
        { storage: Backing; }
        machine Buffer::extra<Other>() -> u64 { 0 }
        machine preserve(value: Buffer<[u8; 4], u8, 5>) -> Buffer<[u8; 4], u8, 5> { value }
    "#,
    );
    let diagnostics = fixture
        .check()
        .map(|_| ())
        .expect_err("an unspecialized container must keep its type equation");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("retains unsolved type equations")),
        "{diagnostics:?}"
    );
}
