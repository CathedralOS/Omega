use super::*;
use std::path::Path;

#[test]
fn private_native_receipt_rejects_other_products_before_source_access() {
    for product in [
        RequestedCompileProduct::Check,
        RequestedCompileProduct::TerminalArtifact,
    ] {
        let request = CompileRequest::new(CompileOptions {
            root_path: "missing-checked-native-api-source.omg".into(),
            build_dir: None,
            target_name: None,
        })
        .with_requested_product(product);
        let diagnostics = driver::compile_native_with_checked_receipt(request)
            .expect_err("checked native API must reject non-native products");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            format!(
                "checked native compilation requires NativeArtifact production; received {product:?}"
            )
        );
    }
}

#[test]
fn private_native_receipt_rejoins_every_hosted_target() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .expect("compiler crate should have the repository above it");
    let root = repository.join("tests/omega/pass/optimizer/no_selection_empty_entry/main.omg");
    for target in [
        "linux_x86_64",
        "linux_arm64",
        "macos_arm64",
        "windows_x86_64",
    ] {
        let request = CompileRequest::new(CompileOptions {
            root_path: root.clone(),
            build_dir: Some(std::env::temp_dir().join(format!(
                "omega-private-native-receipt-{target}-{}",
                std::process::id()
            ))),
            target_name: Some(target.to_owned()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly);
        let compilation = driver::compile_native_with_checked_receipt(request)
            .unwrap_or_else(|diagnostics| panic!("{target}: {diagnostics:#?}"));
        assert_eq!(compilation.target_profile().target_name(), target);
        assert_eq!(
            compilation.checked().source_file_count(),
            compilation.report().source_file_count
        );
        assert_eq!(
            compilation.native_target(),
            compilation
                .report()
                .retained_native_artifact()
                .expect("paired report must retain its artifact")
                .target()
        );
        compilation
            .into_report()
            .into_retained_native_artifact()
            .expect("paired report must transfer its retained artifact")
            .validate()
            .expect("retained artifact must replay");
    }
}
