//! A default has no authored named declaration. Its role and owner come only
//! from the admitted context; Product code and its same-spelled names stay inert.
#[path = "../../tests/support/build_sequence.rs"]
pub(crate) mod build_sequence;

use artifacts::compile_timings::counters;
use build_declarations::{BuildDeclarationKind, DependencyPurpose};
use compiler::{CheckedCompileRequest, CompilationSession};
use package_compilation::{PackageCompilationInputs, PackageSourceBinding};

const SOURCE: &str = "data Build { marker: u8; }\n\
    machine build(value: &mut Build) crashes Trap { crash Trap; }\n\
    machine main() {}\n";

fn binding(capture: &package_compilation::CapturedLocalSource, marker: u8) -> PackageSourceBinding {
    PackageSourceBinding::new(
        semantic_vocabulary::PackageKeyIdentity::from_digest([marker; 32]).unwrap(),
        "settled_default_build",
        capture.canonical_live_root().to_path_buf(),
    )
    .with_captured_source(capture.tree().clone())
    .unwrap()
}

fn execute(
    execution: Option<&mut build_settings::PreparedBuildExecution>,
) -> build_settings::ReturnedBuild {
    let execution = execution.expect("default source owes one real activation");
    let host = execution.host().expect("issue host once");
    let mut activation = host.start().expect("start default Build");
    while !activation
        .advance(1)
        .expect("resume default Build")
        .returned
    {}
    let returned = activation.complete().expect("normal default return");
    assert!(
        returned.declaration().is_none(),
        "default must not invent a named role"
    );
    returned
}

fn inherited(role: BuildDeclarationKind, marker: u8) {
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "default-build-owner",
        false,
    )
    .unwrap();
    std::fs::write(temporary.path().join("main.omg"), SOURCE).unwrap();
    let acquired = package_manager::resolution::source::AcquiredBuildSource::capture_local(
        temporary.path(),
        package_source::LocalSourceLimits::default(),
        package_source::ExternalSourceContext::derive(b"default-build-sdk"),
    )
    .unwrap();
    let capture = acquired.source_capture();
    let root = capture.canonical_live_root().join("main.omg");
    let source = binding(capture, marker);
    let inputs =
        PackageCompilationInputs::new(source.identity(), role, vec![source.clone()], Vec::new())
            .unwrap();
    let opposite = if role == BuildDeclarationKind::Application {
        BuildDeclarationKind::Package
    } else {
        BuildDeclarationKind::Application
    };
    let wrong_role =
        PackageCompilationInputs::new(source.identity(), opposite, vec![source], Vec::new())
            .unwrap();
    let host = target::TargetProfile::host_if_supported().unwrap();
    let mut session = CompilationSession::default();
    let before = counters::build_execution_count();
    let sources_before = counters::source_build_execution_count();
    let suppliers_before = counters::provider_defaults_execution_count();
    let mut invocation = session
        .begin_package_build(CheckedCompileRequest {
            package_inputs: Some(inputs.clone()),
            ..CheckedCompileRequest::new(&root, Some(host.target_name()))
        })
        .unwrap();
    let mut returned = execute(invocation.execution());
    assert!(
        returned
            .join_preacquired_sources(&wrong_role)
            .unwrap_err()
            .contains("differs")
    );
    returned.join_preacquired_sources(&inputs).unwrap();
    let mut settled = session
        .settle_package_build(invocation, Some(returned))
        .unwrap();
    assert!(
        !temporary.path().join("build.omg").exists(),
        "default source stays in memory"
    );
    std::fs::remove_file(&root).unwrap();
    for purpose in [DependencyPurpose::Build, DependencyPurpose::Product] {
        let selected = inputs.clone().with_compilation_purpose(purpose).unwrap();
        let product = session
            .prepare_package_product(&mut settled, host, selected, Default::default())
            .unwrap();
        session.check_package_build(product).unwrap();
    }
    build_sequence::print_execution_counts("default Build after both Product contexts");
    assert_eq!(
        counters::build_execution_count() - before,
        1,
        "one source Build must complete without Product replay"
    );
    assert_eq!(counters::source_build_execution_count() - sources_before, 1);
    assert_eq!(
        counters::provider_defaults_execution_count() - suppliers_before,
        0
    );
    println!(
        "default_role={role:?} source_executions=1 supplier_executions=0 retained_purposes=2 role_substitution=refused product_build_trap=not_executed"
    );
}

fn anonymous() {
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "default-build-anonymous",
        false,
    )
    .unwrap();
    let root = temporary.path().join("main.omg");
    std::fs::write(&root, SOURCE).unwrap();
    let acquired = package_manager::resolution::source::AcquiredBuildSource::capture_local(
        temporary.path(),
        package_source::LocalSourceLimits::default(),
        package_source::ExternalSourceContext::derive(b"default-build-anonymous-sdk"),
    )
    .unwrap();
    let source = binding(acquired.source_capture(), 87);
    let graft = PackageCompilationInputs::new(
        source.identity(),
        BuildDeclarationKind::Application,
        vec![source],
        Vec::new(),
    )
    .unwrap();
    let host = target::TargetProfile::host_if_supported().unwrap();
    let mut session = CompilationSession::default();
    let before = counters::build_execution_count();
    let sources_before = counters::source_build_execution_count();
    let suppliers_before = counters::provider_defaults_execution_count();
    let mut invocation = session
        .begin_source_compilation(CheckedCompileRequest::new(&root, Some(host.target_name())))
        .unwrap();
    let mut returned = execute(invocation.execution());
    assert!(
        returned
            .join_preacquired_sources(&graft)
            .unwrap_err()
            .contains("unbound default Build")
    );
    session
        .complete_source_check(invocation, Ok(Some(returned)))
        .result
        .unwrap();
    build_sequence::print_execution_counts("anonymous default Build completed");
    assert_eq!(counters::build_execution_count() - before, 1);
    assert_eq!(counters::source_build_execution_count() - sources_before, 1);
    assert_eq!(
        counters::provider_defaults_execution_count() - suppliers_before,
        0
    );
    assert!(!temporary.path().join("build.omg").exists());
    println!(
        "anonymous_default source_executions=1 supplier_executions=0 late_package_graft=refused product_build_trap=not_executed"
    );
}

pub fn run() {
    inherited(BuildDeclarationKind::Application, 85);
    inherited(BuildDeclarationKind::Package, 86);
    anonymous();
}
