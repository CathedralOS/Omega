//! Cross-target provider authority on the public source and saved-Psi paths.
#[path = "../../../../../tests/support/fixture_repository.rs"]
mod fixture_repository;

use compiler::{Artifact, CompileRequest, NativeTarget, Program, TargetProviderInstallation};
use host_provider_inputs::ProviderInstallationInputs;
use package_compilation::{
    BuildExecutionInputs, BuildSourceCaptureObligation, BuildSourceCaptureRequest,
};
use package_manager::{
    operations::begin_local_project_preparation, resolution::source::ExecutedSourceResolution,
};
use project_build::planning::{self, PlannedProduct, PlanningRequest, PlanningSources};
use psi_authorization::{HostProducerTrust, authorize_terminal_artifact};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use target::TargetProfile;
use terminal_codec::CanonicalTerminalArtifact;

fn open_project(path: &Path, targets: &[TargetProfile]) -> PlannedProduct {
    static LIBRARY: OnceLock<()> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        package_compilation::toolchain_library::configure(
            &fixture_repository::root().join("source/library"),
        )
        .unwrap()
    });
    let project = begin_local_project_preparation(path, targets, true)
        .unwrap()
        .unwrap();
    let selected = BuildSourceCaptureRequest::new(["main.omg", "build.omg"].map(|name| {
        (
            name.as_bytes().to_vec(),
            BuildSourceCaptureObligation::Required,
        )
    }))
    .unwrap();
    let acquired = project.acquire_selected_root(&selected).unwrap();
    let prepared = planning::prepare(
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
    let mut returned = planning::interpret(prepared, &mut ()).unwrap();
    let declaration = acquired.declare_build(returned.returned()).unwrap();
    let subject = declaration.identity();
    let ExecutedSourceResolution::Complete(graph) = declaration.resolve().unwrap() else {
        panic!("fixture Build unexpectedly requested a dependency");
    };
    let inputs = graph
        .join_source_result(subject, &mut returned.source_result().unwrap())
        .unwrap();
    returned.open_product(Some(&inputs)).unwrap()
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
        .join("tests/fixtures/run/calls/scalar_contract_route_exit/main.omg");
    let project = open_project(&fixture, &targets);
    let stage = project.product_stage(&targets).unwrap();
    let Artifact::Psi(published) = compiler::compile(CompileRequest::Psi(stage.source())).unwrap()
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
fn saved_windows_image_requires_the_exact_getpid_binding_and_entry() {
    let profile = TargetProfile::WindowsX64;
    let path = fixture_repository::root()
        .join("tests/fixtures/run/providers/native_boundary_supply/main.omg");
    let product = open_project(&path, &[profile]);
    let stage = product.product_stage(&[profile]).unwrap();
    let Artifact::Native(direct) = compiler::compile(CompileRequest::Native {
        program: stage.program(profile, vec![]).unwrap(),
        entry_memory: &[],
    })
    .unwrap() else {
        panic!("expected source image")
    };
    let publication =
        source_compiler::compile_for_target(stage.source(), profile.target_name()).unwrap();
    let (published, providers, exclusions) = publication.into_parts();
    let reopened = Arc::new(CanonicalTerminalArtifact::from_bytes(&published.to_bytes()).unwrap());
    let trust = HostProducerTrust::accept("source-produced Windows fixture");
    let authorized = authorize_terminal_artifact(Arc::clone(&reopened), Some(&trust)).unwrap();
    let source_compiler::provider_supply::SourceProviderDemand::Target { bindings, .. } =
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
        let refusal = compiler::compile(CompileRequest::Native {
            program: saved(install(inputs)),
            entry_memory: &[],
        })
        .err()
        .expect("foreign import requires its exact checked declaration");
        assert!(
            refusal
                .iter()
                .any(|diagnostic| diagnostic.message.contains(reason)),
            "{refusal:?}"
        );
    }
    let installation = install(bindings);
    let Artifact::Native(received) = compiler::compile(CompileRequest::Native {
        program: saved(installation.clone()),
        entry_memory: &[],
    })
    .unwrap() else {
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
    let Artifact::Runtime(runtime) = compiler::compile(CompileRequest::Runtime {
        program: saved(installation),
        entry_memory: &[],
    })
    .unwrap() else {
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
    let source = source_compiler::compile(stage.source()).unwrap_or_else(|reasons| {
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
    let Artifact::Runtime(folded) = compiler::compile(CompileRequest::Runtime {
        program: stage.program(profile, vec![]).unwrap(),
        entry_memory: &[],
    })
    .unwrap() else {
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
    let plans = original
        .native_realization_proposal()
        .unwrap()
        .providers()
        .selection
        .plans();
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
    let other_plans = other
        .native_realization_proposal()
        .unwrap()
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
