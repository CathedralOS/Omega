use omega_compiler::compile_to_checked;
use omega_representation_planning::OpaqueRepresentationLifecycleDisposition;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const BUILD: &str = r#"
machine build(builder: &mut Build) {
    builder.application("opaque-representation-lifecycle");
    builder.select_representation<
        InterruptAcknowledgement,
        CarrierRepresentation
    >();
}
"#;

fn project(source: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let ordinal = NEXT.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "omega-opaque-representation-lifecycle-{}-{ordinal}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create opaque representation fixture");
    fs::write(directory.join("main.omg"), source).expect("write fixture source");
    fs::write(directory.join("build.omg"), BUILD).expect("write fixture build root");
    directory.join("main.omg")
}

fn source(carrier_declarations: &str) -> String {
    format!(
        r#"
use omega::language::core::interrupt;

{carrier_declarations}

CarrierRepresentation:
    Carrier satisfies OpaqueRepresentation<InterruptAcknowledgement>;

data Main {{}}
machine Main::main(&mut self) {{}}
"#
    )
}

fn rejection(carrier_declarations: &str) -> String {
    compile_to_checked(&project(&source(carrier_declarations)), None)
        .expect_err("invalid unused representation selection must reject")
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn valid_unused_selection_retains_explicit_inert_lifecycle() {
    let checked = compile_to_checked(
        &project(&source(
            r#"
data Leaf { value: u64; }
data Carrier { leaf: Leaf; bytes: [u8; 4]; }
"#,
        )),
        None,
    )
    .expect("closed cleanup-free carrier must be admitted");
    let [selection] = checked.opaque_representation_selections() else {
        panic!("one exact representation selection")
    };
    assert_eq!(
        selection.lifecycle(),
        OpaqueRepresentationLifecycleDisposition::Inert
    );
    assert!(
        checked
            .boundary_calling_plan_realizations()
            .iter()
            .all(|realization| realization
                .materialized_signature
                .opaque_representation_uses()
                .is_empty()),
        "unused selection must not fabricate consumer demand"
    );
}

#[test]
fn carrier_closure_rejects_direct_and_nested_nominal_cleanup() {
    for (name, declarations) in [
        (
            "direct",
            r#"
data Carrier { value: u64; }
machine Carrier::drop(&mut self) {}
"#,
        ),
        (
            "nested",
            r#"
data Resource { value: u64; }
machine Resource::drop(&mut self) {}
data Carrier { resource: Resource; }
"#,
        ),
    ] {
        let rendered = rejection(declarations);
        assert!(
            rendered.contains("independently invoked nominal cleanup"),
            "{name}: unexpected diagnostics:\n{rendered}"
        );
    }
}

#[test]
fn carrier_closure_rejects_linear_debt_in_arrays_and_inactive_sum_payloads() {
    for (name, declarations) in [
        (
            "array",
            r#"
data Receipt [linear] { value: u64; }
data Carrier { receipts: [Receipt; 2]; }
"#,
        ),
        (
            "sum payload",
            r#"
data Receipt [linear] { value: u64; }
data Payload { case Empty; case Pending(receipt: Receipt); }
data Carrier { payload: Payload; }
"#,
        ),
    ] {
        let rendered = rejection(declarations);
        assert!(
            rendered.contains("live linear ownership debt"),
            "{name}: unexpected diagnostics:\n{rendered}"
        );
    }
}

#[test]
fn carrier_closure_rejects_nested_boundary_opaque_and_external_storage() {
    for (name, declarations, expected) in [
        (
            "nested opaque",
            r#"
boundary data Foreign [linear];
data Carrier { foreign: Foreign; }
"#,
            "boundary-opaque data",
        ),
        (
            "borrowed storage",
            r#"
data Carrier { bytes: &[u8]; }
"#,
            "borrowed or external storage",
        ),
    ] {
        let rendered = rejection(declarations);
        assert!(
            rendered.contains(expected),
            "{name}: unexpected diagnostics:\n{rendered}"
        );
    }
}
