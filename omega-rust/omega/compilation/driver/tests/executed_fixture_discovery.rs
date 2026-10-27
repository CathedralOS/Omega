//! The corpus chooses products from executed Build observations, never syntax.
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::CompilationProduct;
use fixture_package_inputs::{discover_fixture, produce_fixture, repo_root, review_fixture};
use target::TargetProfile;

// The phase counter is process-wide; ordinary libtest may run these together.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn native_fixture_admission_does_not_upgrade_a_package_check() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let root = repo_root()
        .join("tests/omega/pass/build/explicit_provider_readiness_exit/backing/main.omg");
    let discovered = discover_fixture(&root, Default::default()).expect("backing source");
    let product = CompilationProduct::Native(compilation_products::NativeProductRequest {
        private_entry_ram: Vec::new(),
        terminal_authority_policy:
            terminal_psi_to_abstract_operations::provider_admission::current_terminal_authority_policy(),
        terminal_authority_permission_policy: None,
        optimization_rollback: Default::default(),
    });
    let diagnostics = review_fixture(discovered, &[TargetProfile::MacosArm64], product)
        .err()
        .expect("a package is not a native application root");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("native production requires application root")),
        "{diagnostics:?}"
    );
}

#[test]
fn executed_bindings_select_targets_and_retained_legs_keep_separate_staging() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let root = repo_root().join("tests/omega/pass/build/explicit_provider_readiness_exit/main.omg");
    let first = discover_fixture(&root, Default::default()).expect("first Build");
    assert_eq!(
        first.targets,
        [
            "windows_x86_64",
            "linux_x86_64",
            "macos_arm64",
            "linux_arm64"
        ],
        "the untaken UEFI binding is not an executed target"
    );
    let second = discover_fixture(&root, Default::default()).expect("independent Build");
    let builds = artifacts::compile_timings::counters::build_execution_count();
    for discovered in [first, second] {
        let mut prepared = review_fixture(
            discovered,
            &[TargetProfile::MacosArm64],
            CompilationProduct::Terminal(Default::default()),
        )
        .expect("admit retained Product");
        let output = produce_fixture(&mut prepared, Some("macos_arm64")).expect("complete Product");
        let report = output.into_report().expect("Terminal report");
        assert!(report.into_retained_terminal_artifact().is_some());
    }
    assert_eq!(
        artifacts::compile_timings::counters::build_execution_count(),
        builds,
        "Product review and completion must not replay either settled Build",
    );
}

#[test]
fn completed_outputs_keep_exact_bytes_after_package_staging_closes() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let repository = repo_root();
    let root =
        repository.join("tests/omega/pass/build/acquired_generator_completed_output_set/main.omg");
    let discovered = discover_fixture(&root, Default::default()).expect("generator Build");
    let mut prepared = review_fixture(
        discovered,
        &[TargetProfile::MacosArm64],
        CompilationProduct::Terminal(Default::default()),
    )
    .expect("generator Product");
    let output = produce_fixture(&mut prepared, Some("macos_arm64")).expect("completed generator");
    let report = output.into_report().expect("generator report");
    let directory = fixture_package_inputs::sealed_fixture_staging::fresh_staging_build_dir(
        &repository,
        root.parent().unwrap(),
    )
    .join("publication");
    compilation_products::publication::publish_compilation(report, &directory)
        .expect("publication owns its bytes after package completion");
    let sets = std::fs::read_dir(directory.join("completed"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(sets.len(), 1);
    assert_eq!(
        std::fs::read(sets[0].join("files/companion.txt")).unwrap(),
        b"omega\n"
    );
    assert_eq!(
        std::fs::read(sets[0].join("files/generated.omg")).unwrap(),
        b"public const GENERATED: u32 = 73;\n",
    );
}
