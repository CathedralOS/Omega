//! An authored Build returns its declaration and target selections to the project host.

#[path = "../../../../../tests/support/fixture_repository.rs"]
mod fixture_repository;

use build_declarations::BuildDeclaration;
use package_compilation::BuildExecutionInputs;
use package_manager::{
    operations::begin_local_project_preparation, resolution::source::ExecutedSourceResolution,
};
use project_build::planning::{PlanningRequest, PlanningSources, interpret, prepare};
use semantic_vocabulary::NativeBindingLocator;
use std::sync::OnceLock;
use target::TargetProfile;

#[test]
fn authored_build_returns_its_application_and_target_bindings() {
    configure_library();
    let fixture = fixture_repository::root()
        .join("tests/fixtures/run/providers/native_boundary_supply/main.omg");
    let project = begin_local_project_preparation(&fixture, &[TargetProfile::LinuxX64], true)
        .unwrap()
        .unwrap();
    let acquired = project.acquire_root().unwrap();
    let capture = acquired.source_capture();
    let root = acquired.entry_path();
    let evaluation = prepare(
        PlanningRequest {
            root: &root,
            sources: PlanningSources::Captured(capture),
            execution: BuildExecutionInputs::default(),
            artifact_only: false,
        },
        Default::default(),
        Default::default(),
    )
    .unwrap();

    let mut stage = interpret(evaluation, &mut ()).unwrap();
    let Some(BuildDeclaration::Application(application)) = stage.returned().declaration() else {
        panic!("the captured Build must return an application");
    };
    assert_eq!(application.name.as_str(), "native_boundary_supply");
    let observed = stage.observe_source_declaration().unwrap();
    assert_eq!(
        observed.source_capture().normalized().content_identity,
        capture.normalized().content_identity
    );
    assert!(!observed.is_settled());
    let declaration = acquired.declare_build(stage.returned()).unwrap();
    let subject = declaration.identity();
    let ExecutedSourceResolution::Complete(graph) = declaration.resolve().unwrap() else {
        panic!("the fixture declares no dependencies")
    };
    let inputs = graph
        .join_source_result(subject, &mut stage.source_result().unwrap())
        .unwrap();
    let product = stage.open_product(Some(&inputs)).unwrap();
    assert!(product.source().product_sources.is_none());
    assert!(
        product
            .source()
            .sources
            .selected_files()
            .any(|file| file.path == root)
    );
    assert_eq!(product.package_inputs().unwrap().root(), subject);
    let planned = product.product_stage(&[TargetProfile::LinuxX64]).unwrap();
    let program = planned
        .program(TargetProfile::LinuxX64, Vec::new())
        .unwrap();
    let compiler::Program::Source { source, target } = &program else {
        panic!("the named project must produce a source request");
    };
    assert_eq!(source.selections.subject, Some(subject));
    assert_eq!(source.selections.calling_targets, &["linux_x86_64"]);
    assert!(std::ptr::eq(
        source.build_accounting.unwrap(),
        &product.evaluated().source_completion_accounting,
    ));
    assert_eq!(
        target.settings.application.name.as_deref(),
        Some("native_boundary_supply")
    );
    assert_eq!(source.selections.policy.native_bindings.len(), 4);
    assert!(source.selections.native_bindings.evaluation.is_some());
    let computed = &product.evaluated().configuration;
    assert_eq!(computed.root_binding_requests.len(), 4);
    assert_eq!(computed.native_binding_requests.len(), 4);
    for (target, syscall) in [("linux_x86_64", 39), ("linux_arm64", 172)] {
        let binding = computed
            .native_binding_requests
            .iter()
            .find(|row| row.target.canonical_name() == target)
            .expect("Build issued the target's native binding");
        assert_eq!(
            binding.locator,
            NativeBindingLocator::Syscall { number: syscall }
        );
    }
    let compiler::Artifact::Native(output) = compiler::compile(compiler::CompileRequest::Native {
        program,
        entry_memory: &[],
    })
    .expect("the selected getpid binding must reach the native emitter") else {
        panic!("the Linux request must yield a native executable");
    };
    let artifact = output.retained_native_artifact().unwrap();
    assert!(artifact.image().output().bytes.starts_with(b"\x7fELF"));
    let module = artifact.psi_artifact().semantic_module();
    let [boundary] = module.boundary_machines.as_slice() else {
        panic!("getpid must be the only reached native requirement");
    };
    assert!(
        module
            .calling_abi_identity(boundary.id)
            .unwrap()
            .requirement_identity
            .contains("HostProcessId::get")
    );
}

fn configure_library() {
    static CONFIGURED: OnceLock<()> = OnceLock::new();
    CONFIGURED.get_or_init(|| {
        package_compilation::toolchain_library::configure(
            &fixture_repository::root().join("source/library"),
        )
        .unwrap()
    });
}

#[test]
fn dependent_continuation_runs_after_its_package_was_prepared() {
    use build_settings::host::requests::BuildDependencyPreparation;
    use build_settings::{CompileFailure, TerminalBuildActivation};
    use package_compilation::source_imports::ProductQueryDependency;
    use package_manager::resolution::source::{
        AcquiredBuildSource, AcquiredSourceSelection, BuildSourceAcquisitions,
    };
    use project_build::planning::PlanningServices;

    struct Helper<'a> {
        root: &'a AcquiredBuildSource,
        acquisitions: BuildSourceAcquisitions<'a>,
        prepared: usize,
    }

    impl PlanningServices for Helper<'_> {
        fn prepare_dependency(
            &mut self,
            activation: &TerminalBuildActivation,
            request: &BuildDependencyPreparation,
        ) -> Result<ProductQueryDependency, CompileFailure> {
            let selected = self
                .acquisitions
                .acquire_issued(
                    self.root,
                    activation.requests(),
                    request.purpose,
                    request.request_ordinal,
                )
                .unwrap();
            let AcquiredSourceSelection::Source(child) = selected else {
                panic!("the helper is a local package")
            };
            let entry = prepare(
                PlanningRequest {
                    root: &child.entry_path(),
                    sources: PlanningSources::Captured(child.source_capture()),
                    execution: BuildExecutionInputs {
                        dependency_activation: true,
                        ..Default::default()
                    },
                    artifact_only: false,
                },
                Default::default(),
                Default::default(),
            )?;
            let mut returned = interpret(entry, &mut ())?;
            let declared = child.declare_build(returned.returned()).unwrap();
            let subject = declared.identity();
            let ExecutedSourceResolution::Complete(graph) = declared.resolve().unwrap() else {
                panic!("the helper declares no dependencies")
            };
            let inputs = graph
                .join_source_result(subject, &mut returned.source_result()?)
                .unwrap();
            returned.open_product(Some(&inputs))?;
            self.prepared += 1;
            Ok(ProductQueryDependency {
                alias: "support".into(),
                inputs,
                generated: None,
            })
        }

        fn declared_source(
            &mut self,
            activation: &TerminalBuildActivation,
        ) -> Result<Option<package_compilation::PackageCompilationInputs>, CompileFailure> {
            Ok(Some(
                self.root
                    .declared_source_inputs(&activation.project_declaration().unwrap())
                    .unwrap(),
            ))
        }
    }

    configure_library();
    let path = fixture_repository::root().join(
        "tests/fixtures/fail/build/product_entry_query_foreign_helper_private_entry/main.omg",
    );
    let project = begin_local_project_preparation(&path, &[TargetProfile::LinuxX64], true)
        .unwrap()
        .unwrap();
    let mut acquisitions = project.source_acquisitions().unwrap();
    let root = acquisitions
        .retain_root(project.acquire_root().unwrap())
        .unwrap();
    let entry = prepare(
        PlanningRequest {
            root: &root.entry_path(),
            sources: PlanningSources::Captured(root.source_capture()),
            execution: BuildExecutionInputs::default(),
            artifact_only: false,
        },
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let mut services = Helper {
        root: &root,
        acquisitions,
        prepared: 0,
    };
    let refusal = interpret(entry, &mut services)
        .err()
        .expect("the second stage needs a Product query service");
    assert_eq!(services.prepared, 1);
    let diagnostics = refusal.into_diagnostics();
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("without a granted query service")),
        "{diagnostics:?}"
    );
}
