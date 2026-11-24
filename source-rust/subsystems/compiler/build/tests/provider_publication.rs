//! Cross-target provider authority on the public source and saved-Psi paths.
#[path = "provider_publication/conformances.rs"]
mod conformances;
#[path = "provider_publication/fixed_image.rs"]
mod fixed_image;
#[path = "../../../../../tests/support/fixture_repository.rs"]
mod fixture_repository;
#[path = "provider_publication/provider_grants.rs"]
mod provider_grants;

use compiler::{Artifact, CompileRequest, NativeTarget, Program, TargetProviderInstallation};
use host_provider_inputs::ProviderInstallationInputs;
use package_compilation::{
    BuildExecutionInputs, BuildSourceCaptureObligation, BuildSourceCaptureRequest,
};
use package_manager::{
    operations::begin_local_project_preparation, resolution::source::ExecutedSourceResolution,
};
use project_build::{PlannedProduct, PlanningRequest, PlanningSources};
use psi_authorization::{HostProducerTrust, authorize_terminal_artifact};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use target::TargetProfile;
use terminal_codec::CanonicalTerminalArtifact;

fn open_project(path: &Path, targets: &[TargetProfile]) -> PlannedProduct {
    let (returned, inputs) = run_project_build(path, targets);
    returned.open_product(Some(&inputs)).unwrap()
}

fn run_project_build(
    path: &Path,
    targets: &[TargetProfile],
) -> (
    project_build::ReturnedPlanningStage,
    package_compilation::PackageCompilationInputs,
) {
    run_project_build_with_sources(path, targets, &["main.omg", "build.omg"])
}

fn run_project_build_with_sources(
    path: &Path,
    targets: &[TargetProfile],
    files: &[&str],
) -> (
    project_build::ReturnedPlanningStage,
    package_compilation::PackageCompilationInputs,
) {
    static LIBRARY: OnceLock<()> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        package_compilation::toolchain_library::configure(
            &fixture_repository::root().join("source/library"),
        )
        .unwrap()
    });
    let project = begin_local_project_preparation(path, targets, true).unwrap();
    let selected = BuildSourceCaptureRequest::new(files.iter().map(|name| {
        (
            name.as_bytes().to_vec(),
            BuildSourceCaptureObligation::Required,
        )
    }))
    .unwrap();
    let acquired = project.acquire_selected_root(&selected).unwrap();
    let prepared = project_build::prepare(
        PlanningRequest {
            root: &acquired.entry_path(),
            sources: PlanningSources::Captured(acquired.source_capture()),
            execution: BuildExecutionInputs::default(),
            artifact_only: false,
        },
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let mut returned = project_build::interpret(prepared, &mut ()).unwrap();
    let declaration = acquired.declare_build(returned.returned()).unwrap();
    let subject = declaration.identity();
    let ExecutedSourceResolution::Complete(graph) = declaration.resolve().unwrap() else {
        panic!("fixture Build unexpectedly requested a dependency");
    };
    let inputs = graph
        .join_source_result(subject, &mut returned.source_result().unwrap())
        .unwrap();
    (returned, inputs)
}

#[test]
fn a_mixed_build_keeps_a_firmware_provider_selection_scoped_to_uefi() {
    let temporary =
        platform_custody::temporary_directory::TemporaryDirectory::create("mixed-provider", false)
            .unwrap();
    let root = temporary.path().join("main.omg");
    std::fs::write(
        &root,
        r"use omega\language\core\Extent;
boundary trait Sensor { machine read() -> u32 reaches none crashes none; }
FirmwareSensor satisfies Sensor { machine read() -> u32 { 17u32 } }
boundary trait Counter { machine value() -> u64 reaches none crashes none; }
LocalCounter satisfies Counter { machine value() -> u64 { 42u64 } }
boundary trait Clock { machine tick() -> u64 reaches none crashes none; }
FixedClock satisfies Clock { machine tick() -> u64 { 0u64 } }
machine hosted() {}
machine firmware(image: Extent in Granted, initial_storage: Extent in Granted)
    crashes Abort { crash Abort; }",
    )
    .unwrap();
    std::fs::write(
        temporary.path().join("build.omg"),
        r#"machine build(builder: @mutable Build) {
    Build::application(builder, "mixed");
    Build::select_provider::<Counter, LocalCounter>(builder);
    let mode: CompositionMode = fused_mode();
    Build::select_provider::<Clock, FixedClock>(builder, mode);
    Build::select_provider::<Sensor, FirmwareSensor>(builder, uefi_x86_64);
    Build::bind(builder, uefi_x86_64\ProgramEntry, firmware);
    Build::bind(builder, linux_x86_64\ProgramEntry, hosted);
}
machine fused_mode() -> CompositionMode { CompositionMode::Fused }"#,
    )
    .unwrap();
    let targets = [TargetProfile::UefiX64, TargetProfile::LinuxX64];
    let project = open_project(&root, &targets);
    let returned = &project.evaluated().configuration;
    assert_eq!(returned.root_binding_requests.len(), 2);
    use semantic_vocabulary::source_selections::{CompositionMode, ProductTarget};
    let policies: Vec<_> = returned
        .product_selection_requests
        .iter()
        .map(|selection| (selection.target, selection.composition))
        .collect();
    assert_eq!(
        policies,
        [
            (None, Some(CompositionMode::Fused)),
            (None, Some(CompositionMode::Fused)),
            (
                Some(ProductTarget::UefiX86_64),
                Some(CompositionMode::Fused)
            ),
        ]
    );
}

#[test]
fn one_portable_program_carries_the_four_selected_entry_contracts() {
    let targets = [
        TargetProfile::LinuxX64,
        TargetProfile::LinuxArm64,
        TargetProfile::MacosArm64,
        TargetProfile::WindowsX64,
    ];
    let fixture = fixture_repository::root()
        .join("tests/fixtures/calls/run/scalar_contract_route_exit/main.omg");
    let project = open_project(&fixture, &targets);
    let stage = project.product_stage(&targets).unwrap();
    let Artifact::Psi(published) =
        compiler::compile(CompileRequest::Psi(stage.source()), Default::default())
            .unwrap()
            .artifact
    else {
        panic!("expected portable program");
    };
    let module = published.semantic_module();
    let evidence = published.published_evidence().unwrap();
    assert_eq!(evidence.module, published.manifest().semantic());
    assert!(!evidence.seals.is_empty());
    let mut expected = std::collections::BTreeSet::new();
    for profile in targets {
        let slot = profile.program_entry_slot();
        let contract = module
            .program_entry_contract(profile.target_name())
            .unwrap();
        let schema = &module
            .service_schema(contract.schema_declaration, contract.schema_application)
            .unwrap()
            .surface;
        for requirement in [
            slot.semantic_arrival_requirement,
            slot.physical_arrival_requirement.unwrap(),
        ] {
            let (owner, name) = requirement.rsplit_once("::").unwrap();
            let mut matches = schema
                .methods
                .iter()
                .filter(|method| method.requirement_owner == owner && method.name == name);
            let method = matches.next().unwrap();
            assert!(matches.next().is_none(), "ambiguous {requirement}");
            expected.insert(schema.identity_text(method.requirement_identity).to_owned());
        }
    }
    let actual: std::collections::BTreeSet<_> = module
        .interfaces.calling_abi_signatures
        .iter()
        .map(|abi| {
            assert!(abi.storage.is_ok(), "{:?}", abi.storage);
            abi.identity(&module.identities)
                .unwrap()
                .requirement_identity
        })
        .collect();
    assert_eq!(expected.len(), 5);
    assert_eq!(module.interfaces.calling_abi_signatures.len(), expected.len());
    assert_eq!(actual, expected);
    let linear = module
        .types.structural_types
        .iter()
        .find(|row| row.owns_linear_claim)
        .unwrap();
    assert_eq!(
        linear.multiplicity,
        terminal_psi::StructuralMultiplicity::Linear
    );
    assert!(!linear.is_copyable() && !linear.no_code_affine_cleanup && !linear.descriptor_cleanup);
}

#[test]
fn private_entry_requires_the_target_of_its_parent() {
    use provider_realization::native_execution::{BoundNativeCallable, prepare_program};
    let project = platform_custody::temporary_directory::TemporaryDirectory::create(
        "native-entry-target",
        false,
    )
    .unwrap();
    let main = project.path().join("main.omg");
    std::fs::write(&main, "machine main() {}").unwrap();
    std::fs::write(
        project.path().join("build.omg"),
        r#"machine build(builder: @mutable Build) {
        Build::application(builder, "entry_target");
        Build::bind(builder, linux_x86_64\ProgramEntry, main);
        Build::bind(builder, linux_arm64\ProgramEntry, main);
    }"#,
    )
    .unwrap();
    let targets = [TargetProfile::LinuxX64, TargetProfile::LinuxArm64];
    let project = open_project(&main, &targets);
    let stage = project.product_stage(&targets).unwrap();
    let [x64, arm64] = targets.map(|target| {
        let Artifact::Runtime(product) = compiler::compile(
            CompileRequest::Runtime {
                program: stage.program(target, vec![]).unwrap(),
                entry_memory: &[],
            },
            Default::default(),
        )
        .unwrap()
        .artifact
        else {
            panic!("expected runtime product")
        };
        product.into_retained_terminal_artifact().unwrap()
    });
    assert_eq!(
        x64.artifact().view().identity(),
        arm64.artifact().view().identity()
    );
    let parent = prepare_program(x64.native_realization_proposal(), x64.authorization()).unwrap();
    // The same machines can be published with different evidence. Only the original
    // publication is authorized for this proposal and its private entries.
    let module = x64.artifact().semantic_module();
    let debug = x64
        .artifact()
        .debug_bytes()
        .is_none()
        .then(|| terminal_codec::TerminalDebugMap {
            semantic: x64.artifact().view().identity(),
            files: vec![],
            sites: vec![],
        });
    let republished = CanonicalTerminalArtifact::from_parts(
        module.clone(),
        terminal_codec::build_identity_optimization_execution_record(module).unwrap(),
        debug.as_ref(),
    )
    .unwrap();
    let producer = psi_authorization::HostProducerTrust::accept("entry test");
    let republished = psi_authorization::authorize_terminal_artifact(
        std::sync::Arc::new(republished),
        Some(&producer),
    )
    .unwrap();
    assert_eq!(
        republished.artifact().view().identity(),
        x64.artifact().view().identity()
    );
    assert_ne!(republished.identity(), x64.authorization().identity());
    let proposal = x64.native_realization_proposal();
    let providers = proposal.providers();
    let mut entry = proposal.program_entry().unwrap().clone();
    assert_eq!(
        entry
            .bind_physical_adapter(
                republished.artifact(),
                &providers.selection,
                &providers.checked_adapters,
            )
            .unwrap_err(),
        "physical adapter belongs to another canonical artifact",
    );
    let errors = prepare_program(x64.native_realization_proposal(), &republished)
        .err()
        .expect("the proposal must use its original publication");
    assert_eq!(
        errors[0].message,
        "native proposal belongs to another canonical artifact"
    );
    let errors = BoundNativeCallable::prepare(
        x64.native_realization_proposal(),
        &parent,
        republished.artifact(),
        x64.native_realization_proposal().root_callable(),
    )
    .err()
    .expect("the private entry must use the publication authorized by its parent");
    assert_eq!(
        errors[0].message,
        x64.authorization()
            .require_artifact(republished.artifact())
            .unwrap_err()
            .to_string()
    );
    let errors = BoundNativeCallable::prepare(
        arm64.native_realization_proposal(),
        &parent,
        x64.artifact(),
        x64.native_realization_proposal().root_callable(),
    )
    .err()
    .expect("the x64 parent cannot supply layouts for an Arm64 entry");
    assert_eq!(
        errors[0].message,
        "native execution belongs to another target"
    );
}

#[test]
fn saved_windows_image_requires_the_exact_getpid_binding_and_entry() {
    let profile = TargetProfile::WindowsX64;
    let path = fixture_repository::root()
        .join("tests/fixtures/providers/run/native_boundary_supply/main.omg");
    let product = open_project(&path, &[profile]);
    let stage = product.product_stage(&[profile]).unwrap();
    let Artifact::Native(direct) = compiler::compile(
        CompileRequest::Native {
            program: stage.program(profile, vec![]).unwrap(),
            entry_memory: &[],
        },
        Default::default(),
    )
    .unwrap()
    .artifact
    else {
        panic!("expected source image")
    };
    let publication =
        source_compiler::compile_for_target(stage.source(), profile.target_name(), None).unwrap();
    let (published, providers, exclusions) = publication.into_parts();
    let reopened = Arc::new(CanonicalTerminalArtifact::from_bytes(&published.to_bytes()).unwrap());
    let trust = HostProducerTrust::accept("source-produced Windows fixture");
    let authorized = authorize_terminal_artifact(Arc::clone(&reopened), Some(&trust)).unwrap();
    let source_compiler::publication::SourceProviderDemand::Target { bindings, .. } =
        providers.receive(&reopened, profile.target_name()).unwrap()
    else {
        panic!("source did not issue a Windows deployment");
    };
    let Program::Source { target, .. } = stage.program(profile, vec![]).unwrap() else {
        panic!("expected source target")
    };
    let install = |bindings| {
        TargetProviderInstallation::new(
            authorized.identity(),
            profile,
            ProviderInstallationInputs {
                bindings,
                package_permissions: target.package_permissions.clone(),
                semantic_bindings: Arc::clone(&target.semantic_bindings),
            },
        )
    };
    let saved = |installation| Program::Psi {
        program: authorized.clone(),
        target: NativeTarget {
            profile,
            installation,
            application: target.settings.application.clone(),
            optimizations: target.settings.optimizations,
            x86_scalar_fma_provider: target.settings.x86_scalar_fma_provider,
            requested_boundaries: Vec::new(),
            behavior_exclusions: &exclusions,
        },
    };
    let mut changed = bindings.clone();
    changed[0].as_mut().unwrap().service_declaration[0] ^= 1;
    for (inputs, reason) in [
        (changed, "different service contract"),
        (Vec::new(), "has no provider"),
    ] {
        let refusal = compiler::compile(
            CompileRequest::Native {
                program: saved(install(inputs)),
                entry_memory: &[],
            },
            Default::default(),
        )
        .err()
        .expect("foreign import requires its exact checked declaration")
        .into_diagnostics();
        assert!(
            refusal
                .iter()
                .any(|diagnostic| diagnostic.message.contains(reason)),
            "{refusal:?}"
        );
    }
    let installation = install(bindings);
    let Artifact::Native(received) = compiler::compile(
        CompileRequest::Native {
            program: saved(installation.clone()),
            entry_memory: &[],
        },
        Default::default(),
    )
    .unwrap()
    .artifact
    else {
        panic!("expected saved native image")
    };
    let image = &received
        .retained_native_artifact()
        .unwrap()
        .image()
        .output()
        .bytes;
    assert!(image.windows(8).any(|bytes| bytes == b"_getpid\0"));
    assert_eq!(
        image,
        &direct
            .retained_native_artifact()
            .unwrap()
            .image()
            .output()
            .bytes
    );
    let Artifact::Runtime(runtime) = compiler::compile(
        CompileRequest::Runtime {
            program: saved(installation),
            entry_memory: &[],
        },
        Default::default(),
    )
    .unwrap()
    .artifact
    else {
        panic!("expected runtime admission")
    };
    let runtime = runtime.runtime_provisions().unwrap();
    assert_eq!(runtime.identity(), reopened.view().identity());
    assert_eq!(runtime.target_slot().owner, profile);
    assert_eq!(
        runtime.source_signature_identity(),
        received
            .runtime_provisions()
            .unwrap()
            .source_signature_identity()
    );
    let module = reopened.semantic_module();
    let contract = module
        .program_entry_contract(profile.target_name())
        .unwrap();
    let entry = module.machine(contract.entry).unwrap();
    assert_eq!(entry.id, runtime.entry());
    let declared = &entry.declaration.as_ref().unwrap().machine;
    let identity = module.identities.text(declared.identity()).unwrap();
    let root = terminal_psi::PreparedNominalRoot::new(identity, "diagnostic alias", None).unwrap();
    let receipt = terminal_codec::entry_receipt::prepare_program_entry_receipt(
        &root,
        runtime.source_signature_identity(),
        &reopened,
    )
    .unwrap();
    assert_eq!(receipt.terminal_entry(), runtime.entry());
    assert_eq!(receipt.terminal_psi_identity(), runtime.identity());
    let wrong_owner =
        terminal_psi::PreparedNominalRoot::new("other-package::main", "main", None).unwrap();
    assert!(
        terminal_codec::entry_receipt::prepare_program_entry_receipt(
            &wrong_owner,
            runtime.source_signature_identity(),
            &reopened,
        )
        .is_err()
    );
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        let directory = platform_custody::temporary_directory::TemporaryDirectory::create(
            "windows-getpid",
            false,
        )
        .unwrap();
        let file = directory.path().join("getpid.exe");
        std::fs::write(&file, image).unwrap();
        let status = std::process::Command::new(file)
            .stdin(std::process::Stdio::null())
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(0));
    }
}

fn folded_product(path: &Path, profile: TargetProfile) -> product_publication::CompletedProduct {
    let product = open_project(path, &[profile]);
    let stage = product.product_stage(&[profile]).unwrap();
    let source = source_compiler::compile(stage.source(), None).unwrap_or_else(|reasons| {
        panic!(
            "portable publication of {} for {}: {reasons:?}",
            path.display(),
            profile.target_name()
        )
    });
    let original_bytes = source.to_bytes();
    assert!(
        source
            .semantic_module()
            .scalars.folded_constants
            .dependencies
            .is_empty()
    );
    let Artifact::Runtime(folded) = compiler::compile(
        CompileRequest::Runtime {
            program: stage.program(profile, vec![]).unwrap(),
            entry_memory: &[],
        },
        Default::default(),
    )
    .unwrap()
    .artifact
    else {
        panic!("expected folded product")
    };
    let artifact = folded.artifact().unwrap();
    assert_ne!(artifact.view().identity(), source.view().identity());
    let mut without_fold = artifact.semantic_module().clone();
    without_fold.scalars.folded_constants = Default::default();
    assert_eq!(&without_fold, source.semantic_module());
    assert_eq!(artifact.debug_bytes(), source.debug_bytes());
    assert_eq!(source.to_bytes(), original_bytes);
    folded
}

#[test]
fn folded_layout_reopens_only_with_its_selected_target_and_plan_preimages() {
    let sample =
        fixture_repository::root().join("samples/cli/facts/target_layout_derived_constant");
    let temporary =
        platform_custody::temporary_directory::TemporaryDirectory::create("provider-fold", false)
            .unwrap();
    let root = temporary.path().join("main.omg");
    let build = temporary.path().join("build.omg");
    std::fs::copy(sample.join("main.omg"), &root).unwrap();
    let original_build = std::fs::read_to_string(sample.join("build.omg")).unwrap();
    std::fs::write(&build, &original_build).unwrap();
    let linux = TargetProfile::LinuxX64;
    let original = folded_product(&root, linux)
        .into_retained_terminal_artifact()
        .unwrap();
    let reopened = CanonicalTerminalArtifact::from_bytes(&original.artifact().to_bytes()).unwrap();
    let view = reopened.view();
    let proposal = original.native_realization_proposal();
    let execution = provider_realization::native_execution::prepare_program(
        proposal,
        original.authorization(),
    )
    .unwrap();
    let root_reach = execution.scope().reach();
    assert!(!root_reach.scalar_application_demand().is_empty());
    // The empty transition arm reads none of the size-dependent constants in main.
    let done = view
        .module()
        .machines
        .iter()
        .find(|machine| {
            root_reach.contains_machine(machine.id)
                && machine.operations().next().is_none()
                && machine.blocks.iter().all(|block| {
                    matches!(block.terminator, terminal_psi::Terminator::ReturnUnit { .. })
                })
        })
        .expect("the native program reaches its empty done arm");
    let callable = provider_realization::native_execution::BoundNativeCallable::prepare(
        proposal,
        &execution,
        original.artifact(),
        terminal_psi::TargetMachineCallable::Machine(done.id),
    )
    .unwrap();
    let (_, _, _, _, scope, _) = callable.into_parts();
    assert!(std::ptr::eq(scope.reach().layouts(), root_reach.layouts()));
    assert!(scope.reach().scalar_application_demand().is_empty());

    let plans = proposal.providers().selection.plans();
    let preimages = target_provider_admission::matching_folded_constant_dependencies(
        view,
        linux.target_name(),
        plans.iter(),
    )
    .unwrap();
    assert!(!preimages.is_empty());
    let admit = |target, inputs: &[Vec<u8>]| {
        psi_provider_selections::admit_provider_installation(view, &[], &[])
            .unwrap()
            .admit_folded_constant_closure(view, target, inputs)
    };
    admit(linux.target_name(), &preimages).unwrap();
    assert_eq!(
        admit("linux_arm64", &preimages).unwrap_err(),
        "folded constant closure belongs to another target"
    );
    let mut repeated = preimages.clone();
    repeated.push(preimages[0].clone());
    for incomplete in [&[][..], repeated.as_slice()] {
        assert_eq!(
            admit(linux.target_name(), incomplete).unwrap_err(),
            "folded constant selected realization closure is incomplete or repeated"
        );
    }
    let digests: Vec<_> = view
        .module()
        .scalars.folded_constants
        .dependencies
        .iter()
        .filter(|dependency| dependency.target == linux.target_name())
        .map(|dependency| dependency.plan.to_vec())
        .collect();
    assert_eq!(
        admit(linux.target_name(), &digests).unwrap_err(),
        "folded constant normalized plan input differs from its retained commitment"
    );

    // Small and Same both return four. Swapping the chosen implementation
    // must be observable even when the resulting language-level value agrees.
    let selection = "Build::select_provider::<Answer, Small>(builder, linux_x86_64);";
    assert_eq!(original_build.matches(selection).count(), 1);
    std::fs::write(
        &build,
        original_build.replace(selection, &selection.replace("Small", "Same")),
    )
    .unwrap();
    let other = folded_product(&root, linux)
        .into_retained_terminal_artifact()
        .unwrap();
    let mismatched = provider_realization::native_execution::prepare_program(
        proposal,
        other.authorization(),
    )
    .err()
    .expect("the original proposal cannot prepare a different folded program");
    assert_eq!(mismatched.len(), 1);
    assert_eq!(
        mismatched[0].message,
        "native proposal belongs to another canonical artifact"
    );
    let other_plans = other
        .native_realization_proposal()
        .providers()
        .selection
        .plans();
    let reason = target_provider_admission::matching_folded_constant_dependencies(
        view,
        linux.target_name(),
        other_plans.iter(),
    )
    .unwrap_err();
    assert!(
        reason.contains("differs from the selected provider plan"),
        "{reason}"
    );
    let other_inputs = target_provider_admission::matching_folded_constant_dependencies(
        other.artifact().view(),
        linux.target_name(),
        other_plans.iter(),
    )
    .unwrap();
    assert_eq!(
        admit(linux.target_name(), &other_inputs).unwrap_err(),
        "folded constant normalized plan input differs from its retained commitment"
    );

    let windows = folded_product(&sample.join("main.omg"), TargetProfile::WindowsX64);
    let observation = terminal_runtime::execute(terminal_runtime::TerminalRuntimeRequest {
        entry: None,
        filesystem: Default::default(),
        native_services: Default::default(),
        native_provider: None,
        x86_flags: None,
        memory_ordering: None,
        provider_proposal: None,
        artifact: windows.artifact().unwrap(),
        target: Some("windows_x86_64"),
        provisions: windows
            .runtime_provisions()
            .map(|runtime| runtime.execution()),
        work_budget: 100_000,
        domain_exit: terminal_runtime::TerminalDomainExitDelegation::IsolatedContainingDomain,
        standard_input: None,
        clock: None,
    })
    .unwrap();
    use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};
    let success = match &observation.status {
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => true,
        TerminalExecutionStatus::ExternalCompletion(completion) => completion.return_code == 0,
        _ => false,
    };
    assert!(success, "{:?}", observation.status);
}

#[test]
fn root_and_private_callable_enforce_the_selected_process_exit() {
    use artifact_lowering::{
        RetainedNativeProviderInputs, admit_prepared_native_realization, lower_native_callable,
        test_support::{lower_native_execution, prepare_native_providers},
    };
    use provider_realization::native_execution::{BoundNativeCallable, prepare_program};

    let workspace =
        platform_custody::temporary_directory::TemporaryDirectory::create("exit-scope", false)
            .unwrap();
    let source = workspace.path().join("main.omg");
    std::fs::write(
        workspace.path().join("build.omg"),
        r#"machine build(builder: @mutable Build) {
    Build::application(builder, "exit_scope");
    Build::bind(builder, linux_x86_64\ProgramEntry, main);
}"#,
    )
    .unwrap();
    let profile = TargetProfile::LinuxX64;
    let publish = |literal: &str| {
        std::fs::write(
            &source,
            format!(
                r"use omega\language\core\Never;
use omega\language\core\ProcessExit;
use omega\language\core\ProcessExitPrimitive;
machine exit_helper(code: i32) reaches ProcessExit crashes none terminates {{ ProcessExitPrimitive(code); }}
machine passthrough(value: i32) -> i32 {{ value }}
machine main() reaches ProcessExit crashes none terminates {{ exit_helper(passthrough({literal})); }}"
            ),
        )
        .unwrap();
        let product = open_project(&source, &[profile]);
        let stage = product.product_stage(&[profile]).unwrap();
        let Artifact::Runtime(product) = compiler::compile(
            CompileRequest::Runtime {
                program: stage.program(profile, vec![]).unwrap(),
                entry_memory: &[],
            },
            Default::default(),
        )
        .unwrap()
        .artifact
        else {
            panic!("expected runtime admission")
        };
        product.into_retained_terminal_artifact().unwrap()
    };
    let retained = publish("1i32");
    let artifact = retained.artifact();
    let proposal = retained.native_realization_proposal();
    let execution = prepare_program(proposal, retained.authorization()).unwrap();
    let module = artifact.semantic_module();
    let selected = execution
        .compiler_builtins()
        .iter()
        .find(|row| {
            row.identity()
                == Some(
                    provider_realization::CompilerIntrinsicExecutionIdentity::HostedExitProcessI32,
                )
        })
        .expect("the process exit requirement retains its selected builtin");
    let boundary = selected.boundary();
    assert_eq!(selected.artifact(), artifact.view().identity());
    let helper = |name: &str| {
        let machine = module
            .machines
            .iter()
            .find_map(|machine| {
                let identity = machine.declaration.as_ref()?.machine.identity();
                module
                    .identities
                    .text(identity)?
                    .contains(name)
                    .then_some(machine.id)
            })
            .expect(name);
        assert!(execution.scope().reach().contains_machine(machine));
        assert_ne!(execution.scope().reach().entry(), machine);
        terminal_psi::TargetMachineCallable::Machine(machine)
    };
    let exit = helper("exit_helper");
    let passthrough = helper("passthrough");

    let selection = execution.selection();
    assert!(selected.belongs_to(selection));
    let index = selection
        .plans()
        .iter()
        .position(|plan| std::ptr::eq(plan, selected.plan()))
        .expect("the builtin retains the exact selected plan");
    let mut changed = selection.recovery_candidate();
    changed.plans[index].provider_declaration = Some("OtherExitDeclaration".into());
    let changed = changed.validate().unwrap();
    let other = &changed.plans()[index];
    assert_eq!(other.identity_digest(), selected.plan().identity_digest());
    assert_ne!(
        other.provider_declaration,
        selected.plan().provider_declaration
    );
    assert!(!selected.belongs_to(&changed));

    let permissions = provider_admission::terminal_authority_permissions::admit_package_terminal_authority_permissions(
        proposal.package_terminal_authority_permissions().to_vec(),
        proposal.package_terminal_authority_permission_identities().clone(),
        "test package permissions",
    ).unwrap();
    let providers = prepare_native_providers(
        proposal,
        RetainedNativeProviderInputs {
            execution: &execution,
            optimization_selections: proposal.post_terminal_optimizations().selections(),
            accepted_package_permissions: &permissions,
        },
    )
    .unwrap();
    let settlements = providers.program_settlements();
    let policy = provider_admission::current_terminal_authority_policy();
    let base = providers.admission_request(&settlements, &policy, &None, None, &[]);
    let second = publish("2i32");
    let second_proposal = second.native_realization_proposal();
    let second_execution = prepare_program(second_proposal, second.authorization()).unwrap();
    assert_ne!(selected.artifact(), second.artifact().view().identity());
    assert!(selected.belongs_to(second_execution.selection()));
    assert!(
        second_execution
            .compiler_builtins()
            .iter()
            .any(|row| row.boundary() == boundary && row.requirement() == selected.requirement())
    );
    let second_input =
        lower_native_execution(second_proposal, &second_execution, &[], None).unwrap();
    let errors = admit_prepared_native_realization(second_input, second.artifact(), &base)
        .err()
        .expect("the same local process exit boundary cannot be replayed across Psi artifacts");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("different Psi artifact")),
        "{errors:?}"
    );
    let mut other_owner = base;
    other_owner.providers.selected_provider_plans = &changed;
    let root = lower_native_execution(proposal, &execution, &[], None).unwrap();
    let errors = admit_prepared_native_realization(root, artifact, &other_owner)
        .err()
        .expect("the selected process exit cannot be admitted for another owner");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("different provider selection")),
        "{errors:?}"
    );
    let missing = Some(provider_admission::terminal_authority_permissions::admit_package_terminal_authority_permissions(vec![], Default::default(), "no process-exit permission").unwrap());
    let mut root_context = None;
    for (entry, needs_exit) in [(None, true), (Some(exit), true), (Some(passthrough), false)] {
        for restricted in [false, true] {
            let mut request = base;
            if restricted {
                request.providers.terminal_authority_permission_policy = &missing;
            }
            let coverage;
            let input = if let Some(entry) = entry {
                let callable =
                    BoundNativeCallable::prepare(proposal, &execution, artifact, entry).unwrap();
                let (input, callable_coverage) = lower_native_callable(callable, None).unwrap();
                coverage = callable_coverage;
                request.entry = entry;
                request.providers.program_provider_context = Some(root_context.as_ref().unwrap());
                request.boundary_application_coverage = Some(&coverage);
                input
            } else {
                lower_native_execution(proposal, &execution, &[], None).unwrap()
            };
            let result = admit_prepared_native_realization(input, artifact, &request);
            if needs_exit && restricted {
                let errors = result
                    .err()
                    .expect("a process-exit call needs receiving permission");
                assert!(
                    errors
                        .iter()
                        .any(|error| error.message.contains("permission policy has no exact row")),
                    "{errors:?}"
                );
            } else {
                let admitted = result.unwrap();
                let settled_exit = admitted.providers.settlements.iter().any(|row| {
                    row.boundary == boundary
                        && matches!(
                            row.execution,
                            provider_admission::AdmittedBoundaryExecution::CompilerBuiltin(_)
                        )
                });
                assert_eq!(settled_exit, needs_exit);
                if entry.is_none() {
                    root_context = Some(Arc::clone(admitted.providers.program_provider_context()));
                }
            }
        }
    }
}

#[test]
fn fresh_review_records_the_build_that_declared_the_package() {
    use review_evidence::record::{PackagePolicyCallableRole, PackageReviewNominalOwner};

    let temp = platform_custody::temporary_directory::TemporaryDirectory::create(
        "fresh-build-identity",
        false,
    )
    .unwrap();
    let root = temp.path();
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: @mutable Build) { Build::application(builder, "review_example"); Build::bind(builder, linux_x86_64\ProgramEntry, main); }"#,
    )
    .unwrap();
    std::fs::write(root.join("main.omg"), "machine main() {}").unwrap();

    let target = TargetProfile::LinuxX64;
    let (returned, inputs) = run_project_build(&root.join("build.omg"), &[target]);
    let owner = inputs.root();
    let settled = returned.settle_dependency(inputs.clone()).unwrap();
    let product = settled.open_product(inputs).unwrap();
    let checked = product.product_stage(&[target]).unwrap().review_policy(target).unwrap();
    let mut builds = checked.policy.callables.callables().iter()
        .filter(|callable| callable.role == PackagePolicyCallableRole::Build);
    let build = builds.next().expect("the executed Build is part of the review");
    assert!(builds.next().is_none());
    assert_eq!(
        build.identity.owner,
        PackageReviewNominalOwner::Package(owner)
    );
    assert_eq!(build.identity.path, "build.omg::build");
}

#[test]
fn public_service_ceilings_are_reviewed_without_an_installed_provider_or_grant() {
    let target = TargetProfile::LinuxX64;
    for (name, service, critical) in [
        ("StandardInput", r"use omega\language\core\StandardInput;", true),
        ("StandardOutput", r"use omega\language\core\StandardOutput;", true),
        ("StandardOutput", "public boundary trait StandardOutput {}", false),
    ] {
        let project = platform_custody::temporary_directory::TemporaryDirectory::create(
            "review-callable-service-ceiling",
            false,
        )
        .unwrap();
        let root = project.path();
        std::fs::write(
            root.join("build.omg"),
            r#"machine build(builder: @mutable Build) { Build::package(builder, "review_ceiling"); }"#,
        )
        .unwrap();
        std::fs::write(root.join("main.omg"), format!("{service}\npublic machine inspect_value(value: u32) -> u32 reaches {name} crashes none {{ value }}\n")).unwrap();
        let (returned, inputs) = run_project_build(&root.join("build.omg"), &[target]);
        let settled = returned.settle_dependency(inputs.clone()).unwrap();
        let product = settled.open_product(inputs).unwrap();
        let checked = product
            .product_stage(&[target])
            .unwrap()
            .review_policy(target)
            .unwrap();
        let callable = checked
            .policy
            .callables
            .callables
            .iter()
            .find(|row| row.identity.path == "main.omg::inspect_value")
            .unwrap();
        let reach = callable.declared_service_reach.as_deref().unwrap();
        assert_eq!(reach.len(), 1);
        assert_eq!(
            reach[0].path(),
            if critical {
                name
            } else {
                "main.omg::StandardOutput"
            }
        );
        assert_eq!(
            checked.policy.dangerous_capabilities.len(),
            usize::from(critical)
        );
        assert_eq!(checked.policy.slack_uses.len(), usize::from(critical));
        assert!(checked.policy.selected_providers.plans.is_empty());
        assert!(checked.policy.terminal_permissions.services.is_empty());
        assert!(checked.semantic_binding_candidates.is_empty());
    }
}

#[test]
fn review_nominates_the_referenced_host_declaration() {
    use provider_acceptance::AcceptedSemanticBindingRole;

    let temp = platform_custody::temporary_directory::TemporaryDirectory::create(
        "review-host-by-file",
        false,
    )
    .unwrap();
    let root = temp.path();
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: @mutable Build) { Build::package(builder, "host_by_file"); }"#,
    )
    .unwrap();
    std::fs::create_dir(root.join("other")).unwrap();
    std::fs::write(
        root.join("other/host.omg"),
        "public boundary trait TimeHost {}",
    )
    .unwrap();
    let target = TargetProfile::LinuxX64;
    for (reference, expected_file, nominates_host) in [
        ("TimeHost", "main.omg::TimeHost", true),
        ("|other|::TimeHost", "other/host.omg::TimeHost", false),
    ] {
        std::fs::write(
            root.join("main.omg"),
            format!(
                "use other/host.omg as other; public boundary trait TimeHost {{}} \
                public machine probe() reaches {reference} crashes none {{}}"
            ),
        )
        .unwrap();
        let (returned, inputs) = run_project_build_with_sources(
            &root.join("build.omg"),
            &[target],
            &["build.omg", "main.omg", "other/host.omg"],
        );
        let owner = inputs.root();
        let product = returned
            .settle_dependency(inputs.clone())
            .unwrap()
            .open_product(inputs)
            .unwrap();
        let checked = product
            .product_stage(&[target])
            .unwrap()
            .review_policy(target)
            .unwrap();
        let probe = checked
            .policy
            .callables
            .callables()
            .iter()
            .find(|row| row.identity.path == "main.omg::probe")
            .unwrap();
        let [reached] = probe.declared_service_reach.as_deref().unwrap() else {
            panic!("probe names one service")
        };
        assert_eq!(reached.path(), expected_file);
        let candidates = &checked.semantic_binding_candidates;
        if nominates_host {
            let [host] = candidates.as_slice() else {
                panic!("expected one host nomination")
            };
            assert_eq!(
                host.binding().role(),
                AcceptedSemanticBindingRole::TimeHostService
            );
            assert_eq!(host.binding().package(), owner);
            assert_eq!(host.binding().declaration_path(), "TimeHost");
        } else {
            assert!(candidates.is_empty());
        }
    }
}

#[test]
fn reviewing_a_selected_hosted_exit_uses_its_product_declaration() {
    use provider_acceptance::AcceptedSemanticBindingRole;
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "review-hosted-exit-product",
        false,
    )
    .unwrap();
    let root = temporary.path();
    std::fs::write(
        root.join("build.omg"),
        r#"use omega\language\core\ProcessExit;
        machine build(builder: @mutable Build) {
            Build::package(builder, "exit_provider");
            Build::select_provider::<ProcessExit, LeafExit>(builder, linux_x86_64);
        }"#,
    )
    .unwrap();
    std::fs::write(
        root.join("main.omg"),
        r#"use omega\language\core\Never;
        use omega\language\core\ProcessExit;
        use omega\language\core\ProcessExitPrimitive;
        public LeafExit satisfies ProcessExit {
            machine exit_process(code: i32) -> Never reaches ProcessExit crashes none terminates {
                ProcessExitPrimitive(code);
            }
        }
        public machine finish(code: i32) -> Never reaches ProcessExit crashes none terminates {
            ProcessExit::exit_process(code);
        }"#,
    )
    .unwrap();
    let target = TargetProfile::LinuxX64;
    let (returned, inputs) = run_project_build(&root.join("build.omg"), &[target]);
    let product = returned
        .settle_dependency(inputs.clone())
        .unwrap()
        .open_product(inputs)
        .unwrap();
    let checked = product
        .product_stage(&[target])
        .unwrap()
        .review_policy(target)
        .unwrap();
    let [exit] = checked.semantic_binding_candidates.as_slice() else {
        panic!("expected the selected process-exit nomination");
    };
    assert_eq!(
        exit.binding().role(),
        AcceptedSemanticBindingRole::ProcessExitExitProcessI32
    );
    assert_eq!(exit.binding().terminal_authority_permissions().len(), 1);
    assert!(checked.policy.terminal_permissions.services.is_empty());
}

#[test]
fn review_records_an_explicit_empty_grant_and_the_unpermitted_sibling() {
    use provider_acceptance::{
        AcceptedSemanticBinding, AcceptedSemanticBindingRole, ServiceTerminalAuthorityPermission,
        accepted_service_schema_digest,
    };
    use provider_plans::ServiceSchema;
    use semantic_vocabulary::TerminalAuthorityDisposition;

    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "review-receiver-policy",
        false,
    )
    .unwrap();
    let root = temporary.path();
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: @mutable Build) { Build::package(builder, "receiver_review"); }"#,
    )
    .unwrap();
    std::fs::write(
        root.join("main.omg"),
        r#"
public boundary trait TimeHost {
    machine ticks() -> u64 reaches none crashes none;
    machine frequency() -> u64 reaches TimeHost crashes none;
}
"#,
    )
    .unwrap();
    let target = TargetProfile::LinuxX64;
    let (returned, inputs) = run_project_build(&root.join("build.omg"), &[target]);
    let settled = returned.settle_dependency(inputs.clone()).unwrap();
    let product = settled.open_product(inputs.clone()).unwrap();
    let stage = product.product_stage(&[target]).unwrap();
    let (declarations, _) = source_compiler::review_package(
        stage.source(),
        target.target_name(),
        &mut Default::default(),
    )
    .unwrap();
    let checked = declarations.accepted().unwrap();
    let source = checked.source();
    let service = source
        .traits()
        .iter()
        .find(|decl| {
            decl.name.as_str() == "TimeHost"
                && source.symbols.symbol_package_identity(decl.symbol) == Some(inputs.root())
        })
        .unwrap();
    let declaration = source.symbols.display_path(service.symbol, "::");
    let schema = ServiceSchema::from_declaration(checked.slot_schema(service.symbol).unwrap());
    let role = AcceptedSemanticBindingRole::TimeHostService;
    let digest = accepted_service_schema_digest(role, &schema);
    let binding =
        AcceptedSemanticBinding::new_service(role, inputs.root(), declaration, digest).unwrap();
    let ticks = schema
        .declaration
        .methods
        .iter()
        .find(|row| row.name == "ticks")
        .unwrap();
    let review = |requirement: &str| {
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let grant = ServiceTerminalAuthorityPermission::new(
            digest,
            identities.intern(requirement),
            TerminalAuthorityDisposition::from_classes([]),
        );
        let accepted = binding
            .clone()
            .with_terminal_authority_permissions(vec![grant], &identities.build())
            .unwrap();
        let bound = inputs
            .clone()
            .with_accepted_semantic_bindings(vec![accepted])
            .unwrap();
        let product = settled.open_product(bound).unwrap();
        product
            .product_stage(&[target])
            .unwrap()
            .review_policy(target)
    };
    let checked = review(schema.identity_text(ticks.requirement_identity)).unwrap();
    let [service] = checked.policy.terminal_permissions.services.as_slice() else {
        panic!("expected one receiver permission service")
    };
    assert_eq!(
        service
            .methods
            .iter()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>(),
        schema
            .declaration
            .methods
            .iter()
            .map(|method| method.name.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(service.methods.len(), 2);
    for method in &service.methods {
        match method.name.as_str() {
            "ticks" => assert!(method.authority.service_reach.is_empty()),
            "frequency" => assert_eq!(
                method.authority.service_reach.as_slice(),
                std::slice::from_ref(&service.service),
            ),
            unexpected => panic!("unexpected permission method {unexpected}"),
        }
    }
    let frequency = &service
        .methods
        .iter()
        .find(|method| method.name == "frequency")
        .unwrap()
        .requirement;
    assert!(
        checked
            .policy
            .callables
            .callables()
            .iter()
            .any(|callable| callable.identity() == frequency)
    );
    assert!(service.service.application.is_none());
    let host = checked
        .policy
        .public_api
        .traits
        .iter()
        .find(|item| item.identity == service.service.declaration)
        .unwrap();
    assert!(
        host.requirements
            .iter()
            .any(|requirement| &requirement.identity == frequency)
    );
    let method = service
        .methods
        .iter()
        .find(|method| method.name == "ticks")
        .unwrap();
    let [grant] = service.permissions.as_slice() else {
        panic!("only the supplied permission belongs to the receiver")
    };
    assert_eq!(grant.requirement, method.requirement);
    assert!(grant.permitted.classes().is_empty());
    assert!(review("unrelated::ticks").is_err());
}

#[test]
fn public_method_contracts_keep_their_declaration_binders() {
    use review_evidence::record::{
        PackagePolicyBaseline, PackagePolicyTypeParameterKind, PackageReviewContractBinaryOperator,
        PackageReviewContractExpression as Expression, PackageReviewContractFact,
        PackageReviewContractKind,
    };
    let directory = platform_custody::temporary_directory::TemporaryDirectory::create(
        "public-method-binders",
        false,
    )
    .unwrap();
    let build = directory.path().join("build.omg");
    std::fs::write(
        &build,
        r#"machine build(builder: @mutable Build) { Build::package(builder, "receiver"); }"#,
    )
    .unwrap();
    std::fs::write(
        directory.path().join("main.omg"),
        r#"
public boundary trait Receiver {
    machine send<const Count: u64>(text: @[u8; Count]) -> u64
    requires Count > 0 reaches none crashes none;
}
public machine deliver<'input>(text: @'input [u8; 22]) -> u64
reaches none invokes Receiver { Receiver::send(text) }

public boundary trait Framed<Frame> {
    machine send<const Count: u64>(text: @[u8; Count]) -> u64
    requires Count > 0 requires text[Count - 1] == 0 reaches none crashes none;
}
"#,
    )
    .unwrap();

    let target = TargetProfile::LinuxX64;
    let (returned, inputs) = run_project_build(&build, &[target]);
    let settled = returned.settle_dependency(inputs.clone()).unwrap();
    let product = settled.open_product(inputs).unwrap();
    let reviewed = product
        .product_stage(&[target])
        .unwrap()
        .review_policy(target)
        .unwrap();
    let bytes = reviewed.policy.canonical_bytes().unwrap();
    let policy = PackagePolicyBaseline::recover_canonical(&bytes, Default::default()).unwrap();
    policy
        .validate_package_membership(|owner| owner == policy.package, Default::default())
        .unwrap();
    for (path, count_binder) in [("main.omg::Receiver", 0), ("main.omg::Framed", 1)] {
        let owner = policy
            .public_api
            .traits
            .iter()
            .find(|row| row.identity.path == path)
            .unwrap();
        let [send] = owner.requirements.as_slice() else {
            panic!("expected one method on {path}")
        };
        let callable = policy
            .callables
            .callables()
            .iter()
            .find(|row| row.identity == send.identity)
            .unwrap();
        let expected_parameters = owner
            .type_parameters
            .iter()
            .chain(&send.type_parameters)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(callable.type_parameters, expected_parameters);
        assert_eq!(callable.contracts, send.contracts);
        let [count] = send.type_parameters.as_slice() else {
            panic!("expected the method's Count binder")
        };
        assert!(
            matches!(&count.kind, PackagePolicyTypeParameterKind::Const(carrier) if carrier.canonical().contains("u64"))
        );
        let [text] = send.parameters.as_slice() else {
            panic!("expected the borrowed array")
        };
        assert!(
            text.type_identity
                .canonical()
                .contains(&format!("const-parameter($T{count_binder})")),
            "array extent on {path} uses the wrong binder: {text:?}"
        );
        assert!(
            send.contracts
                .iter()
                .all(|contract| contract.kind == PackageReviewContractKind::Requires)
        );
        assert!(send.contracts.iter().any(|contract| matches!(
            &contract.fact, PackageReviewContractFact::Expression(Expression::Binary {
                operator: PackageReviewContractBinaryOperator::Greater, left, right, ..
            }) if **left == Expression::GenericBinder(count_binder) && **right == Expression::Integer("0".into())
        )));
        if path == "main.omg::Framed" {
            use review_evidence::record::PackageReviewContractOperatorMeaning::Builtin;
            let last_index = Expression::Binary {
                meaning: Builtin,
                operator: PackageReviewContractBinaryOperator::Subtract,
                left: Box::new(Expression::GenericBinder(count_binder)),
                right: Box::new(Expression::Integer("1".into())),
            };
            let final_byte = Expression::Indexed {
                meaning: Builtin,
                collection: Box::new(Expression::Parameter(0)),
                index: Box::new(last_index),
            };
            assert!(send.contracts.iter().any(|contract| matches!(
                &contract.fact, PackageReviewContractFact::Expression(Expression::Binary {
                    meaning: Builtin, operator: PackageReviewContractBinaryOperator::Equal, left, right,
                }) if **left == final_byte && **right == Expression::Integer("0".into())
            )));
        }
    }
}

#[test]
fn review_retains_public_alias_names_with_their_normalized_expansions() {
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "review-public-aliases",
        false,
    )
    .unwrap();
    let root = temporary.path();
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: @mutable Build) { Build::package(builder, "public_aliases"); }"#,
    )
    .unwrap();
    std::fs::write(
        root.join("main.omg"),
        r#"
use other.omg;
public data KeyHash<K> = machine(key: @K) -> u64;
public data Renamed<T> = machine(key: @T) -> u64;
public data Word = u32;
public data SameWord = Word;
public data WordPair = [Word; 2];
data LocalAlias = i64;
"#,
    )
    .unwrap();
    std::fs::write(root.join("other.omg"), "public data Word = u32;").unwrap();
    let target = TargetProfile::LinuxX64;
    let (returned, inputs) = run_project_build_with_sources(
        &root.join("build.omg"),
        &[target],
        &["build.omg", "main.omg", "other.omg"],
    );
    let settled = returned.settle_dependency(inputs.clone()).unwrap();
    let product = settled.open_product(inputs).unwrap();
    let policy = product
        .product_stage(&[target])
        .unwrap()
        .review_policy(target)
        .unwrap()
        .policy;
    let aliases = policy.public_api.type_aliases.as_deref().unwrap();
    let [hash, renamed, same, word, pair, sibling] = aliases else {
        panic!("expected all six public aliases")
    };
    assert_eq!(
        (
            &*same.identity.path,
            &*word.identity.path,
            &*pair.identity.path
        ),
        ("main.omg::SameWord", "main.omg::Word", "main.omg::WordPair")
    );
    assert_eq!(sibling.identity.path, "other.omg::Word");
    assert_eq!(sibling.expansion, word.expansion);
    assert_eq!(hash.identity.path, "main.omg::KeyHash");
    assert_eq!(renamed.identity.path, "main.omg::Renamed");
    assert_eq!(hash.type_parameters.len(), 1);
    assert_eq!(hash.expansion, renamed.expansion);
    assert_eq!(same.expansion, word.expansion);
    assert_ne!(pair.expansion, word.expansion);
    assert!(policy.public_data().is_empty());
}

#[test]
fn selected_provider_review_preserves_qualified_scalar_parameters() {
    for provider in ["Marked", "Other"] {
        check_qualified_parameter("i32", "self >= 0", "self <= 0", provider, false);
    }
}

#[test]
fn selected_provider_review_preserves_qualified_structural_parameters() {
    check_qualified_parameter(
        "[u8; 3]",
        "self[0] == 0u8",
        "self[0] != 0u8",
        "Marked",
        true,
    );
}

#[test]
fn selected_provider_review_preserves_borrowed_domains_and_authored_postconditions() {
    use review_evidence::record::{PackageReviewContractFact, PackageReviewContractKind};

    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "review-borrowed-path",
        false,
    )
    .unwrap();
    let root = temporary.path();
    let build = root.join("build.omg");
    std::fs::write(
        &build,
        r#"machine build(builder: @mutable Build) {
        Build::package(builder, "reading");
        Build::select_provider::<Reading, Provider>(builder, linux_x86_64);
    }"#,
    )
    .unwrap();
    std::fs::write(
        root.join("main.omg"),
        r#"
use {omega}/language/core/byte_observation/byte_observation.omg as bytes;
public domain [u8]::Path requires |bytes|::no_nul(@self[..]);
public boundary trait Reading {
    machine read(path: @[u8] in Path) -> i32 reaches none crashes none;
    machine one(path: @[u8] in Path) -> i32 ensures result == 1i32 reaches none crashes none;
}
Provider satisfies Reading {
    machine read(path: @[u8] in Path) -> i32 { 0i32 }
    machine one(path: @[u8] in Path) -> i32 { 1i32 }
}
public machine run<'view>(path: @'view [u8] in Path) -> i32 reaches Reading {
    Reading::read(path)
}
"#,
    )
    .unwrap();
    let target = TargetProfile::LinuxX64;
    let (returned, inputs) = run_project_build(&build, &[target]);
    let product = returned
        .settle_dependency(inputs.clone())
        .unwrap()
        .open_product(inputs)
        .unwrap();
    let review = product
        .product_stage(&[target])
        .unwrap()
        .review_policy(target)
        .unwrap();
    let policy = review.policy;
    policy
        .validate_package_membership(|key| key == policy.package, Default::default())
        .unwrap();
    let [plan] = policy.selected_providers.plans.as_slice() else {
        panic!("expected Reading")
    };
    assert_eq!(plan.methods.len(), 2);
    for method in &plan.methods {
        let declaration = policy
            .public_api
            .traits
            .iter()
            .flat_map(|row| &row.requirements)
            .find(|row| row.identity == method.requirement)
            .unwrap();
        let callable = policy
            .callables
            .callables
            .iter()
            .find(|row| row.identity == method.requirement)
            .unwrap();
        assert_eq!(declaration.contracts, callable.contracts);
        assert_eq!(method.signature.parameters, declaration.parameters);
        assert_eq!(declaration.parameter_qualification_preconditions.len(), 1);
        assert!(declaration.returned_borrow_origins.is_empty());
        let mut clauses = declaration.contracts.iter();
        let entry = clauses
            .find(|clause| clause.kind == PackageReviewContractKind::Requires)
            .unwrap();
        assert!(
            matches!(&entry.fact, PackageReviewContractFact::Membership { domain, .. }
            if domain.path().ends_with("::Path"))
        );
        let authored = method.name == "one";
        assert_eq!(declaration.contracts.len(), 1 + usize::from(authored));
        if authored {
            assert!(
                declaration
                    .contracts
                    .iter()
                    .any(|clause| clause.kind == PackageReviewContractKind::Ensures
                        && matches!(clause.fact, PackageReviewContractFact::Expression(_)))
            );
        }
    }
}

fn check_qualified_parameter(
    carrier: &str,
    predicate: &str,
    other: &str,
    provider_domain: &str,
    structural: bool,
) {
    use build_evaluation_result::CompileFailure;
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "review-qualified-provider",
        false,
    )
    .unwrap();
    let root = temporary.path();
    let build = root.join("build.omg");
    std::fs::write(
        &build,
        r#"machine build(builder: @mutable Build) {
            Build::package(builder, "reading");
            Build::select_provider::<Reading, Provider>(builder, linux_x86_64);
        }"#,
    )
    .unwrap();
    std::fs::write(
        root.join("main.omg"),
        format!(
            r#"
public domain {carrier}::Marked requires {predicate};
public domain {carrier}::Other requires {other};
public boundary trait Reading {{
    machine read(plain: i32, value: {carrier} in Marked) -> i32 reaches none crashes none;
}}
public machine run(plain: i32, value: {carrier} in Marked) -> i32 reaches Reading {{
    Reading::read(plain, value)
}}
Provider satisfies Reading {{
    machine read(plain: i32, value: {carrier} in {provider_domain}) -> i32 {{ 1i32 }}
}}"#
        ),
    )
    .unwrap();
    let target = TargetProfile::LinuxX64;
    let (returned, inputs) = run_project_build(&build, &[target]);
    let product = returned
        .settle_dependency(inputs.clone())
        .unwrap()
        .open_product(inputs)
        .unwrap();
    let stage = product.product_stage(&[target]).unwrap();
    let reviewed = stage.review_policy(target);
    if provider_domain == "Other" {
        let Err(
            CompileFailure::Diagnostics(diagnostics)
            | CompileFailure::SourceCompletion { diagnostics, .. },
        ) = reviewed
        else {
            panic!("the provider with a different domain must be refused");
        };
        assert!(
            diagnostics.iter().any(|error| error
                .message
                .contains("member signature differs from its requirement")),
            "{diagnostics:?}"
        );
        return;
    }
    let policy = reviewed.unwrap().policy;
    policy
        .validate_package_membership(|key| key == policy.package, Default::default())
        .unwrap();
    let [plan] = policy.selected_providers.plans.as_slice() else {
        panic!("expected the selected Reading provider");
    };
    let [method] = plan.methods.as_slice() else {
        panic!("expected Reading::read");
    };
    let declaration = policy
        .public_api
        .traits
        .iter()
        .flat_map(|item| &item.requirements)
        .find(|item| item.identity == method.requirement)
        .unwrap();
    assert_eq!(method.signature.parameters, declaration.parameters);
    let [plain, qualified] = method.signature.parameters.as_slice() else {
        panic!("expected two formals");
    };
    assert_ne!(plain.type_identity, qualified.type_identity);
    assert!(qualified.type_identity.canonical.contains("Marked"));
    let [contract] = declaration.contracts.as_slice() else {
        panic!("expected the input membership contract");
    };
    assert_eq!(
        contract.kind,
        review_evidence::record::PackageReviewContractKind::Requires
    );
    let review_evidence::record::PackageReviewContractFact::Membership {
        value: review_evidence::record::PackageReviewContractExpression::Parameter(1),
        domain,
        ..
    } = &contract.fact
    else {
        panic!("expected membership on the second parameter");
    };
    assert!(domain.path().ends_with("::Marked"));
    if structural {
        assert_eq!(
            declaration.structural_parameter_positions.as_deref(),
            Some(&[1][..])
        );
        let [qualification] = declaration.parameter_qualification_preconditions.as_slice() else {
            panic!("expected one structural qualification");
        };
        assert_eq!(qualification.structural_index(), 0);
        assert_eq!(qualification.domain(), domain);

        use review_evidence::record::{
            PackageReviewContractBinaryOperator, PackageReviewContractExpression as Expression,
            PackageReviewContractFact, PackageReviewContractOperatorMeaning,
        };
        let marked = policy
            .public_api
            .domains
            .iter()
            .find(|row| &row.identity == domain)
            .unwrap();
        let [PackageReviewContractFact::Expression(Expression::Binary {
            operator: PackageReviewContractBinaryOperator::Equal,
            left,
            ..
        })] = marked.predicate_facts.as_slice() else {
            panic!("expected the Marked domain's equality predicate");
        };
        let Expression::Indexed { meaning, collection, index } = left.as_ref() else {
            panic!("expected the domain predicate to retain self[0]");
        };
        assert_eq!(*meaning, PackageReviewContractOperatorMeaning::Builtin);
        assert_eq!(collection.as_ref(), &Expression::DomainSubject);
        assert_eq!(index.as_ref(), &Expression::Integer("0".to_owned()));
    } else {
        assert!(declaration.parameter_qualification_preconditions.is_empty());
    }
}

#[test]
fn selected_provider_review_preserves_declared_crash_and_suspend_ceilings() {
    use review_evidence::record::{
        PackagePolicyCrashGuard, PackagePolicyCrashRoute, PackageReviewCrashCause,
    };
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "review-scalar-lending",
        false,
    )
    .unwrap();
    let source = fixture_repository::root()
        .join("samples/cli/support/fact_dependencies/scalar_lending_producer");
    for file in ["build.omg", "main.omg"] {
        std::fs::copy(source.join(file), temporary.path().join(file)).unwrap();
    }
    let target = TargetProfile::LinuxX64;
    let (returned, inputs) = run_project_build(&temporary.path().join("build.omg"), &[target]);
    let settled = returned.settle_dependency(inputs.clone()).unwrap();
    let product = settled.open_product(inputs).unwrap();
    let reviewed = product
        .product_stage(&[target])
        .unwrap()
        .review_policy(target)
        .unwrap();
    let policy = &reviewed.policy;
    policy
        .validate_package_membership(|key| key == policy.package, Default::default())
        .expect("elided reference regions must belong to the reviewed requirement");
    for (service, causes, suspends) in [
        (
            "main.omg::Lending",
            vec![PackagePolicyCrashRoute {
                cause: PackageReviewCrashCause::Trap,
                alternative_guards: vec![PackagePolicyCrashGuard::Truth],
            }],
            true,
        ),
        ("main.omg::Observation", vec![], false),
    ] {
        let declared = policy
            .public_api
            .traits
            .iter()
            .find(|item| item.identity.path == service)
            .unwrap();
        let [requirement] = declared.requirements.as_slice() else {
            panic!("one requirement in {service}");
        };
        assert_eq!(
            requirement.published_crash.as_deref(),
            Some(causes.as_slice())
        );
        let callable = policy
            .callables
            .callables()
            .iter()
            .find(|item| item.identity() == &requirement.identity)
            .unwrap();
        assert_eq!(callable.checked_crash.published, causes);
        assert_eq!(callable.declared_may_suspend, Some(suspends));
        let selected = policy
            .selected_providers
            .plans
            .iter()
            .find(|plan| plan.schema_declaration == declared.identity)
            .unwrap();
        let [method] = selected.methods.as_slice() else {
            panic!("one selected method in {service}");
        };
        assert_eq!(method.requirement, requirement.identity);
        assert_eq!(method.may_suspend, suspends);
    }
    let mut narrowed = policy.clone();
    let lending = narrowed
        .public_api
        .traits
        .iter_mut()
        .find(|service| service.identity.path == "main.omg::Lending")
        .unwrap();
    lending.requirements[0].lifetime_parameter_count = 0;
    assert_eq!(
        narrowed
            .validate_package_membership(|key| key == policy.package, Default::default())
            .unwrap_err()
            .error(),
        review_evidence::encoding::PackagePolicyMembershipError::MalformedIdentity
    );
}

#[test]
fn review_and_deployment_publish_the_same_entryless_provider() {
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "review-entryless-provider",
        false,
    )
    .unwrap();
    let root = temporary.path();
    let path = root.join("build.omg");
    std::fs::write(
        &path,
        r#"machine build(builder: @mutable Build) { Build::package(builder, "numbers"); }"#,
    )
    .unwrap();
    std::fs::write(
        root.join("main.omg"),
        r#"
public boundary trait Reading {
    machine value(input: u64) -> u64 reaches none crashes none;
}
public ConstantReading satisfies Reading {
    machine value(input: u64) -> u64 { input }
}
"#,
    )
    .unwrap();
    let target = TargetProfile::LinuxX64;
    let (returned, inputs) = run_project_build(&path, &[target]);
    let settled = returned.settle_dependency(inputs.clone()).unwrap();
    let product = settled.open_product(inputs).unwrap();
    let stage = product.product_stage(&[target]).unwrap();
    let deployed = source_compiler::compile_for_target(stage.source(), target.target_name(), None)
        .unwrap()
        .into_parts()
        .0;
    let (declarations, reviewed) = source_compiler::review_package(
        stage.source(),
        target.target_name(),
        &mut Default::default(),
    )
    .unwrap();
    assert_eq!(declarations.published_program(), deployed.view().identity());
    let reviewed = reviewed.into_parts().0;
    assert_eq!(reviewed.view().identity(), deployed.view().identity());
    let candidates = &reviewed.semantic_module().interfaces.provider_candidates;
    assert_eq!(candidates.len(), 1);
}
