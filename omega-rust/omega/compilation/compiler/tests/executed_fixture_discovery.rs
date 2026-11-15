//! The corpus chooses products from executed Build observations, never syntax.
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::CompilationProduct;
use fixture_package_inputs::{discover_fixture, produce_fixture, repo_root, review_fixture};
use target::TargetProfile;

// The phase counter is process-wide; ordinary libtest may run these together.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn native_project_build_publishes_the_reached_boundary_calling_application() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let root = repo_root().join("tests/fixtures/run/providers/native_boundary_supply/main.omg");
    let profile = TargetProfile::host();
    // The ordinary source-checking API captures the source, executes its
    // local Build and completes the accepted publication in one session.
    let checked = orchestration::check_source(
        &mut compiler::CompilationSession::default(),
        compiler::CheckedCompileRequest::new(&root, Some(profile.target_name())),
    )
    .result
    .unwrap_or_else(|failure| {
        panic!(
            "kept native binding project: {:?}",
            failure.into_diagnostics()
        )
    });
    assert_eq!(
        checked.build().config.native_bindings.len(),
        4,
        "the real Build retains all four authored target bindings"
    );
    assert_eq!(
        checked.execution().native_bindings().rows.len(),
        1,
        "the selected target retains its own evaluated binding"
    );
    let published = checked
        .terminal_publication()
        .published()
        .expect("source checking publishes the accepted Terminal product");
    let module = published.artifact().semantic_module();
    let [boundary] = module.boundary_machines.as_slice() else {
        panic!("the kept project reaches exactly its process-ID requirement")
    };
    let identity = module
        .calling_abi_identity(boundary.id)
        .expect("the reached native boundary keeps its calling association");
    let callable = boundary.declaration.as_ref().unwrap().source.callable;
    assert_eq!(
        Some(identity.requirement_identity.as_str()),
        module.identities.text(callable)
    );
    let [association] = module.calling_signatures.as_slice() else {
        panic!("the reached requirement has one exact calling association")
    };
    assert_eq!(association.boundary, boundary.id);
    let declaration = module
        .calling_abi_signature(*association.application.as_ref().unwrap())
        .unwrap();
    assert_eq!(declaration.identity(&module.identities).unwrap(), identity);
}

#[test]
fn native_fixture_admission_does_not_upgrade_a_package_check() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let root = repo_root()
        .join("tests/fixtures/pass/build/explicit_provider_readiness_exit/backing/main.omg");
    let discovered = discover_fixture(&root, Default::default()).expect("backing source");
    let product = CompilationProduct::Native(products::NativeProductRequest {
        installation_assumptions: Vec::new(),
        retain_native_inputs: false,
        image: Default::default(),
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
    let root =
        repo_root().join("tests/fixtures/pass/build/explicit_provider_readiness_exit/main.omg");
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
    let root = repository
        .join("tests/fixtures/pass/build/acquired_generator_completed_output_set/main.omg");
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
    products::publication::publish_compilation(report, &directory)
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
