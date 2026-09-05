use omega_compiler::compile_to_checked;
use omega_representation_planning::{
    OpaqueRepresentationCopyDisposition, OpaqueRepresentationLifecycleDisposition,
};

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
    project_with_build(source, BUILD)
}

fn project_with_build(source: &str, build: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let ordinal = NEXT.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "omega-opaque-representation-lifecycle-{}-{ordinal}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create opaque representation fixture");
    fs::write(directory.join("main.omg"), source).expect("write fixture source");
    fs::write(directory.join("build.omg"), build).expect("write fixture build root");
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
    assert_eq!(
        selection.copy_disposition(),
        OpaqueRepresentationCopyDisposition::PlacementOnly
    );
    assert_eq!(
        selection.selected_application_commitment(),
        selection.rederived_selected_application_commitment()
    );
    assert!(
        checked
            .boundary_calling_plan_realizations()
            .iter()
            .all(|realization| realization
                .materialized_signature()
                .opaque_representation_uses()
                .is_empty()),
        "unused selection must not fabricate consumer demand"
    );
}

const COPY_BUILD: &str = r#"
machine build(builder: &mut Build) {
    builder.application("copyable-opaque-representation");
    builder.select_representation<CopyToken, CopyCarrierRepresentation>();
}
"#;

fn copy_source(carrier_declarations: &str) -> String {
    format!(
        r#"
use omega::language::core::representation;

boundary data CopyToken [copy];

{carrier_declarations}

CopyCarrierRepresentation:
    CopyCarrier satisfies OpaqueRepresentation<CopyToken>;

data Main {{}}
machine Main::main(&mut self) {{}}
"#,
    )
}

#[test]
fn copyable_opaque_consumes_a_recursive_inert_copy_receipt() {
    let path = project_with_build(
        &copy_source(
            r#"
data CopyLeaf [copy] { value: u64; }
data CopyPayload [copy] { case Empty; case Value(value: CopyLeaf); }
data CopyCarrier [copy] { payloads: [CopyPayload; 2]; }
"#,
        ),
        COPY_BUILD,
    );
    let checked = compile_to_checked(&path, None)
        .expect("a recursively copyable inert carrier must discharge opaque `[copy]`");
    let [selection] = checked.opaque_representation_selections() else {
        panic!("one exact copyable representation selection")
    };
    assert_eq!(
        selection.copy_disposition(),
        OpaqueRepresentationCopyDisposition::CheckedSemanticCopy
    );
    assert_eq!(
        selection.selected_application_commitment(),
        selection.rederived_selected_application_commitment()
    );
    assert_ne!(selection.selected_application_commitment(), [0; 32]);
    assert_ne!(
        selection.selected_application_commitment(),
        selection.application().commitment.as_bytes(),
        "the selected application must bind more than its conformance"
    );
    assert!(
        checked
            .boundary_calling_plan_realizations()
            .iter()
            .all(|realization| realization
                .materialized_signature()
                .opaque_representation_uses()
                .is_empty()),
        "an unused copy receipt must not fabricate consumer demand"
    );
    let _ = fs::remove_dir_all(path.parent().expect("temporary policy directory"));
}

#[test]
fn copyable_opaque_rejects_missing_or_noncopyable_carrier_receipts() {
    let missing = project_with_build(
        &copy_source("data CopyCarrier [copy] { value: u64; }"),
        "machine build(builder: &mut Build) { builder.application(\"missing-copy-receipt\"); }",
    );
    let rendered = compile_to_checked(&missing, None)
        .expect_err("copyable opaque data without a selection must remain unadmitted")
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("without an admitted property receipt"));

    for (name, carrier, expected) in [
        (
            "affine-root",
            "data CopyCarrier { value: u64; }",
            "every carrier declaration to be structurally `[copy]`",
        ),
        (
            "nested-affine",
            "data AffineLeaf { value: u64; }\ndata CopyCarrier [copy] { leaf: AffineLeaf; }",
            "every carrier declaration to be structurally `[copy]`",
        ),
    ] {
        let path = project_with_build(&copy_source(carrier), COPY_BUILD);
        let rendered = compile_to_checked(&path, None)
            .unwrap_err()
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            rendered.contains(expected),
            "{name}: unexpected diagnostics:\n{rendered}"
        );
        let _ = fs::remove_dir_all(path.parent().expect("temporary policy directory"));
    }
    let _ = fs::remove_dir_all(missing.parent().expect("temporary policy directory"));
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
