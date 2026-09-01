use super::*;
use std::path::Path;

#[test]
fn native_product_stop_rejoins_every_hosted_target() {
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
        let report = driver::compile(request)
            .unwrap_or_else(|diagnostics| panic!("{target}: {diagnostics:#?}"));
        let profile = omega_target::TargetProfile::from_omega_target_name(Some(target))
            .expect("hosted target fixture must name a canonical target");
        assert_eq!(
            profile.native_target(),
            report
                .retained_native_artifact()
                .expect("paired report must retain its artifact")
                .target()
        );
        report
            .into_retained_native_artifact()
            .expect("paired report must transfer its retained artifact")
            .validate()
            .expect("retained artifact must replay");
    }
}
