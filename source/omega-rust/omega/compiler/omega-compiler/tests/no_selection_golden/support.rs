use omega_compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileRequest, RequestedCompileProduct,
    RetainedNativeArtifact,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) const HOSTED_NATIVE_TARGETS: [&str; 4] =
    ["linux_x64", "linux_arm64", "macos_arm64", "windows_x64"];

static BUILD_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .expect("compiler crate should have the repository above it")
        .to_path_buf()
}

pub(super) fn interpreter_canary() -> PathBuf {
    repo_root().join("tests/omega/pass/host/runtime_write_no_newline_exit")
}

pub(super) fn native_canary() -> PathBuf {
    repo_root().join("tests/omega/pass/optimizer/no_selection_empty_entry")
}

pub(super) fn fail_canary() -> PathBuf {
    repo_root().join("tests/omega/fail/optimizer/no_selection_wrong_arity")
}

pub(super) fn golden_for_target(target: &str) -> String {
    std::fs::read_to_string(
        repo_root()
            .join("tests/omega/golden/optimizer/no_selection")
            .join(format!("{target}.txt")),
    )
    .unwrap_or_else(|error| panic!("missing no-selection golden for {target}: {error}"))
    .trim()
    .to_owned()
}

fn temporary_build_dir(target: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-no-selection-golden-{target}-{}-{}",
        std::process::id(),
        BUILD_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

pub(super) fn compile_retained_native(target: &str) -> RetainedNativeArtifact {
    let build_dir = temporary_build_dir(target);
    let report = omega_compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: native_canary().join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: Some(target.to_owned()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .unwrap_or_else(|diagnostics| {
        panic!(
            "no-selection native compilation for {target} failed: {:#?}",
            diagnostic_snapshots(&diagnostics)
        )
    });
    assert!(!report.wrote_output());
    let artifact = report
        .into_retained_native_artifact()
        .expect("native request retains its product before publication");
    artifact
        .validate()
        .expect("the retained no-selection artifact must replay");
    let _ = std::fs::remove_dir_all(build_dir);
    artifact
}

pub(super) fn diagnostic_snapshots(diagnostics: &[psi_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            let span = diagnostic.source_span.map_or_else(
                || "none".to_owned(),
                |span| {
                    format!(
                        "source={};start={};end={}",
                        span.source_id.0, span.span.start, span.span.end
                    )
                },
            );
            format!("{:?}|{}|{span}", diagnostic.severity, diagnostic.message)
        })
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn retained_native_snapshot(target: &str, artifact: &RetainedNativeArtifact) -> String {
    let object = artifact.object();
    let image = artifact.image();
    let output = image.output();
    let inventory = &output.executable_regions;
    format!(
        concat!(
            "target={target};",
            "semantic={semantic_len}:{semantic_hash};proof={proof_len}:{proof_hash};",
            "object_text={object_text_len}:{object_text_hash};",
            "image={image_len}:{image_hash};final_text={final_text_len}:{final_text_hash};",
            "file={file_name};format={format};kind={kind:?};subsystem={subsystem:?};",
            "text={text};data={data};bss={bss};symbols={symbols};relocations={relocations};",
            "final_symbols={final_symbols};final_imports={final_imports};",
            "final_relocations={final_relocations};functions={functions};",
            "settlements={settlements};plans={plans};executions={executions};",
            "provider_closure={provider_closure};text_address={text_address};",
            "text_fingerprint={text_fingerprint};inventory={inventory_fingerprint};",
            "regions={regions};gaps={gaps}"
        ),
        target = target,
        semantic_len = artifact.semantic_bytes().len(),
        semantic_hash = sha256_hex(artifact.semantic_bytes()),
        proof_len = artifact.proof_bytes().len(),
        proof_hash = sha256_hex(artifact.proof_bytes()),
        object_text_len = object.text_bytes().len(),
        object_text_hash = sha256_hex(object.text_bytes()),
        image_len = output.bytes.len(),
        image_hash = sha256_hex(&output.bytes),
        final_text_len = output.final_text_bytes.len(),
        final_text_hash = sha256_hex(&output.final_text_bytes),
        file_name = output.file_name,
        format = output.format,
        kind = output.kind,
        subsystem = image.subsystem(),
        text = output.text_bytes,
        data = output.data_bytes,
        bss = output.bss_bytes,
        symbols = output.symbols,
        relocations = output.relocations,
        final_symbols = output.final_image_symbols,
        final_imports = output.final_image_imports,
        final_relocations = output.final_image_relocations,
        functions = image.functions().len(),
        settlements = image.boundary_settlements().len(),
        plans = artifact.selected_provider_plans().len(),
        executions = artifact.provider_executions().len(),
        provider_closure = artifact.selected_provider_closure_identity(),
        text_address = inventory.text_address,
        text_fingerprint = inventory.text_fingerprint,
        inventory_fingerprint = inventory.inventory_fingerprint,
        regions = inventory.regions.len(),
        gaps = inventory.unclassified_gaps.len(),
    )
}
