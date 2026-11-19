fn build_fixture_product(
    root: &Path,
    profile: target::TargetProfile,
    product: orchestration::ProjectProduct,
    directory: PathBuf,
) -> compiler::CompileReport {
    use orchestration::{BuildRequest, BuildTargets};
    let outcomes = orchestration::build(BuildRequest {
        stage_validators: Default::default(),
        root_path: root.to_path_buf(),
        build_directory: Some(directory),
        targets: BuildTargets::Single(profile),
        product,
        timings: false,
        offline: true,
        verify: false,
        require_package_project: true,
        optimization_rollback: Default::default(),
        build_snapshot: Some(main_and_build_snapshot()),
    })
    .expect("admit the source-backed package request");
    let [outcome] =
        <[_; 1]>::try_from(outcomes).unwrap_or_else(|_| panic!("one requested fixture target"));
    assert_eq!(outcome.target, profile);
    outcome
        .result
        .unwrap_or_else(|error| panic!("source reaches its requested product: {error}"))
        .into_report()
        .expect("report of the requested product")
        .report
}

#[test]
fn historical_constant_closure_requires_exact_target_and_provider() {
    use orchestration::{NativeDestination, ProjectProduct};

    fn produce(root: &Path, target_name: &str, product: ProjectProduct) -> compiler::CompileReport {
        build_fixture_product(
            root,
            target::TargetProfile::from_canonical_target_name(target_name).unwrap(),
            product,
            std::env::temp_dir().join(format!(
                "omega-layout-closure-{}-{target_name}",
                std::process::id()
            )),
        )
    }

    let root = historical_constant_source();
    let workspace = platform_custody::temporary_directory::TemporaryDirectory::create(
        "historical-answer-closure",
        false,
    )
    .unwrap();
    // Compare two actual selections for one exact package. A second
    // package's identity could independently falsify the substitution.
    let project = copy_build_and_entry_source(&root, &workspace.path().join("project"));
    let retained = produce(&project, "linux_x86_64", ProjectProduct::Terminal)
        .into_retained_terminal_artifact()
        .expect("native proposal");
    // The executable route commits its provider choices to target code. The
    // original checked program and its Build/source companions keep one identity.
    let (mut session, build, _) = captured_package_build(
        &project,
        &workspace.path().join("checked"),
        target::TargetProfile::LinuxX64,
        Some(main_and_build_snapshot()),
    );
    let checked = session
        .compiler()
        .check_package_build(build)
        .expect("accepted scalar source");
    let publication = checked.terminal_publication();
    let source = publication
        .require_published(publication.optimizations())
        .unwrap();
    let original = source.artifact().view();
    let original_bytes = source.artifact().to_bytes();
    assert!(original.module().folded_constants.dependencies.is_empty());
    let source_demand = publication
        .prepare_provider_demand(
            Some("linux_x86_64"),
            &checked.build().config.provider_selections,
            &checked.build().config.native_bindings,
            checked.sources().package_identity().is_some(),
        )
        .expect("close the Linux source provider choices");
    let demand = portable_publication::ProductCodeSource {
        publication,
        sources: checked.sources(),
    }
    .publish_provider_demand(
        Some(target::TargetProfile::LinuxX64),
        source_demand,
        &mut source_compiler::UnmeasuredProduction,
    )
    .expect("source publishes the exact Linux scalar reach");
    let provider_demand::ProviderDemandInputs::Target {
        folded_entry,
        origins,
        ..
    } = demand.inputs()
    else {
        panic!("expected target-specific provider demand");
    };
    let installation = origins.installation().clone();
    let reopened = std::sync::Arc::new(
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&original_bytes)
            .expect("reopen the original published program"),
    );
    installation
        .clone()
        .attach(
            std::sync::Arc::clone(&reopened),
            target::TargetProfile::LinuxX64,
        )
        .expect("the installation still names the decoded program");
    assert_eq!(
        installation
            .clone()
            .attach(
                std::sync::Arc::clone(&reopened),
                target::TargetProfile::WindowsX64
            )
            .err(),
        Some("provider installation belongs to a different target"),
    );
    let reach = folded_entry.clone().unwrap();
    let producer = psi_authorization::HostProducerTrust::accept("package review fixture");
    let authorized = psi_authorization::authorize_terminal_artifact(
        std::sync::Arc::clone(source.artifact()),
        Some(&producer),
    )
    .expect("authorize the source publication");
    let admitted = native_inputs::admit(authorized, demand)
        .expect("admit the source publication before provider selection");
    let planner = provider_realization::select_published_providers(admitted)
        .expect("plan native providers from the published demand");
    let mut selected = planner.clone();
    selected
        .bind_folded_entry(
            source.artifact(),
            source.evidence().map(std::sync::Arc::as_ref),
        )
        .expect("normal target planner binds the exact reached providers");
    let folded = selected.effective_artifact(source.artifact()).unwrap();
    assert_eq!(
        folded.view().identity(),
        retained.artifact().view().identity()
    );
    assert_ne!(folded.view().identity(), original.identity());
    assert_eq!(
        installation
            .attach(
                std::sync::Arc::clone(folded),
                target::TargetProfile::LinuxX64
            )
            .err(),
        Some("provider installation belongs to a different Psi artifact"),
    );
    let mut without_dependencies = folded.semantic_module().clone();
    without_dependencies.folded_constants = Default::default();
    assert_eq!(&without_dependencies, original.module());
    assert_eq!(folded.debug_bytes(), source.artifact().debug_bytes());
    assert_eq!(source.artifact().to_bytes(), original_bytes);
    assert!(std::sync::Arc::ptr_eq(
        &source,
        &publication
            .require_published(publication.optimizations())
            .unwrap()
    ));
    let produced = checked
        .terminal_publication()
        .accepted()
        .unwrap()
        .produce(
            Default::default(),
            &mut source_compiler::UnmeasuredProduction,
        )
        .unwrap();
    assert!(
        produced
            .build_bindings()
            .calls_for(original.into())
            .is_some()
    );
    assert!(
        produced
            .build_bindings()
            .calls_for(folded.view().into())
            .is_none()
    );
    assert_eq!(
        checked.execution().realization().program(),
        original.identity()
    );
    let replayed = planner
        .with_folded_program(folded.view())
        .expect("receive the same exact dependency table")
        .into_result();
    assert_eq!(replayed.original_program(), original.identity());
    assert_eq!(replayed.realization().program(), folded.view().identity());
    products::PublishedNativeEntry::for_program(folded, &replayed)
        .expect("replayed entry retains its original nomination and its effective code");
    assert!(
        selected
            .bind_folded_entry(
                source.artifact(),
                source.evidence().map(std::sync::Arc::as_ref),
            )
            .is_err()
    );
    let proposal = retained
        .native_realization_proposal()
        .expect("native proposal");
    let decoded =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&retained.artifact().to_bytes())
            .expect("canonical Terminal roundtrip");
    assert_eq!(
        decoded.manifest().semantic(),
        retained.artifact().manifest().semantic()
    );
    let view = decoded.view();
    let plans = proposal.providers().selection.plans();
    assert_eq!(
        reach.bind(original, "linux_x86_64", plans).unwrap(),
        view.module().folded_constants
    );
    assert_eq!(
        reach.bind(original, "linux_arm64", plans).unwrap_err(),
        "folded scalar entry belongs to another program or deployment"
    );
    assert_eq!(
        reach.bind(view, "linux_x86_64", plans).unwrap_err(),
        "folded scalar entry belongs to another program or deployment"
    );
    assert!(
        selected
            .effective_artifact(retained.shared_artifact())
            .is_err(),
        "derived code must not reenter planning as its own source"
    );
    let preimages =
        target_provider_admission::matching_folded_constant_dependencies(view, "linux_x86_64", plans.iter())
            .expect("native admission rejoins exact real plans");
    assert!(
        !preimages.is_empty(),
        "folding must retain the selected extent provider"
    );
    let admit = |target, inputs: &[Vec<u8>]| {
        psi_provider_selections::admit_provider_installation(view, &[], &[])
            .expect("historical-only provider catalog")
            .admit_folded_constant_closure(view, target, inputs)
    };
    admit("linux_x86_64", &preimages).expect("exact historical closure");
    assert_eq!(
        admit("linux_arm64", &preimages).unwrap_err(),
        "folded constant closure belongs to another target"
    );
    assert_eq!(
        admit("linux_x86_64", &[]).unwrap_err(),
        "folded constant selected realization closure is incomplete or repeated"
    );
    let mut repeated = preimages.clone();
    repeated.push(preimages[0].clone());
    assert_eq!(
        admit("linux_x86_64", &repeated).unwrap_err(),
        "folded constant selected realization closure is incomplete or repeated"
    );
    let digests = view
        .module()
        .folded_constants
        .dependencies
        .iter()
        .filter(|dependency| dependency.target == "linux_x86_64")
        .map(|dependency| dependency.plan.to_vec())
        .collect::<Vec<_>>();
    assert_eq!(
        admit("linux_x86_64", &digests).unwrap_err(),
        "folded constant normalized plan input differs from its retained commitment"
    );

    let original_build = std::fs::read_to_string(root.with_file_name("build.omg"))
        .expect("authored Build selections");
    select_historical_linux_answer(&project, &original_build, Some("Same"));
    let alternate = produce(&project, "linux_x86_64", ProjectProduct::Terminal)
        .into_retained_terminal_artifact()
        .expect("alternate native proposal");
    assert!(
        planner
            .with_folded_program(alternate.artifact().view())
            .is_err()
    );
    let alternate_plans = alternate
        .native_realization_proposal()
        .unwrap()
        .providers()
        .selection
        .plans();
    let reason = target_provider_admission::matching_folded_constant_dependencies(
        view,
        "linux_x86_64",
        alternate_plans.iter(),
    )
    .unwrap_err();
    assert!(
        reason.contains("differs from the selected provider plan"),
        "{reason}"
    );
    let alternate_inputs = target_provider_admission::matching_folded_constant_dependencies(
        alternate.artifact().view(),
        "linux_x86_64",
        alternate_plans.iter(),
    )
    .expect("alternate provider's own real closure");
    assert_eq!(
        admit("linux_x86_64", &alternate_inputs).unwrap_err(),
        "folded constant normalized plan input differs from its retained commitment"
    );
    // This emits a Windows product but executes only its Terminal semantics on
    // this host. The frozen source's branch traps unless the derived value is
    // exactly 9 (the Windows provider extent 8 plus 1), rather than Linux's 5.
    let windows = produce(
        &root,
        "windows_x86_64",
        ProjectProduct::Native {
            destination: NativeDestination::Retained,
            private_entry_ram: Vec::new(),
            installation_assumptions: Vec::new(),
        },
    );
    let artifact = windows
        .retained_native_artifact()
        .expect("Windows native emission");
    let observation = terminal_runtime::execute(terminal_runtime::TerminalRuntimeRequest {
        entry: None,
        filesystem: Default::default(),
        native_services: Default::default(),
        native_provider: None,
        x86_flags: None,
        memory_ordering: None,
        provider_proposal: None,
        artifact: artifact.psi_artifact(),
        target: None,
        provisions: windows.runtime_provisions(),
        work_budget: 100_000,
        domain_exit: terminal_runtime::TerminalDomainExitDelegation::IsolatedContainingDomain,
        standard_input: None,
        clock: None,
    })
    .expect("Windows Terminal execution on this host");
    let complete = match &observation.status {
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit,
        ) => true,
        terminal_interpreter::TerminalExecutionStatus::ExternalCompletion(completion) => {
            completion.return_code == 0
        }
        _ => false,
    };
    assert!(complete, "{:?}", observation.status);
    println!("windows_terminal_exit=0 windows_derived_value=9 foreign_native_executed=false");

    println!(
        "canonical_roundtrip=ok exact_plan=admitted target_mismatch=rejected absent_plan=rejected repeated_plan=rejected digest_substitution=rejected equal_provider_substitution=rejected"
    );
}

#[test]
fn saved_program_uses_the_hosts_evaluated_native_binding() {
    let profile = target::TargetProfile::WindowsX64;
    let root = kept_fixture("run/providers/native_boundary_supply/main.omg");
    let workspace = platform_custody::temporary_directory::TemporaryDirectory::create(
        "saved-native-binding",
        false,
    )
    .unwrap();
    let saved = workspace.path().join("program.psi");
    let (host_packages, package_record, package_permissions, semantic_bindings, source_image) = {
        let producer = platform_custody::temporary_directory::TemporaryDirectory::create(
            "native-binding-producer",
            false,
        )
        .unwrap();
        let root = copy_build_and_entry_source(&root, &producer.path().join("project"));
        let source = build_fixture_product(
            &root,
            profile,
            orchestration::ProjectProduct::Native {
                destination: orchestration::NativeDestination::Retained,
                private_entry_ram: Vec::new(),
                installation_assumptions: Vec::new(),
            },
            producer.path().join("direct"),
        );
        let source_image = source
            .retained_native_artifact()
            .unwrap()
            .image()
            .output()
            .bytes
            .clone();
        let (mut session, build, _) = captured_package_build(
            &root,
            &producer.path().join("build"),
            profile,
            Some(main_and_build_snapshot()),
        );
        let host_packages = orchestration::installed_package_providers(profile, &build).unwrap();
        let checked = session.compiler().check_package_build(build).unwrap();
        let publication = checked.terminal_publication();
        let demand = publication
            .prepare_provider_demand(
                Some(profile.target_name()),
                &checked.build().config.provider_selections,
                &checked.build().config.native_bindings,
                false,
            )
            .unwrap();
        let demand = portable_publication::ProductCodeSource {
            publication,
            sources: checked.sources(),
        }
        .publish_provider_demand(
            Some(profile),
            demand,
            &mut source_compiler::UnmeasuredProduction,
        )
        .unwrap();
        let (provider_demand::ProviderDemandInputs::Target { origins, .. }, _) = demand.into_parts()
        else {
            panic!("the host selected a Windows deployment");
        };
        std::fs::write(&saved, origins.artifact().to_bytes()).unwrap();
        let inputs = origins.inputs();
        let bindings = inputs
            .bindings
            .iter()
            .cloned()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        (
            host_packages,
            record_storage::encode(&bindings, Default::default()).unwrap(),
            inputs.package_permissions.clone(),
            inputs.semantic_bindings.clone(),
            source_image,
        )
    };

    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&std::fs::read(&saved).unwrap())
            .unwrap();
    let producer = psi_authorization::HostProducerTrust::accept("fixture source compiler");
    let authorized = psi_authorization::authorize_terminal_artifact(
        std::sync::Arc::new(artifact),
        Some(&producer),
    )
    .unwrap();
    let empty = host_provider_inputs::TargetProviderInstallation::new(
        authorized.identity(),
        profile,
        Default::default(),
    );
    let receive = |installation| {
        orchestration::prepare_saved_program(
            authorized.clone(),
            &saved,
            profile,
            installation,
            orchestration::NativeDestination::Retained,
            &[],
        )
    };
    let missing = receive(empty).expect_err("a process ID needs a supplied binding");
    assert!(
        missing
            .into_diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("has no provider")
                && diagnostic.message.contains("HostProcessId"))
    );
    let bindings: Vec<evaluation_receipts::NativeBindingMaterialization> =
        record_storage::decode(&package_record, Default::default()).unwrap();
    let install = |bindings: Vec<evaluation_receipts::NativeBindingMaterialization>| {
        host_provider_inputs::TargetProviderInstallation::new(
            authorized.identity(),
            profile,
            host_provider_inputs::ProviderInstallationInputs {
                bindings: bindings.into_iter().map(Ok).collect(),
                package_permissions: package_permissions.clone(),
                semantic_bindings: semantic_bindings.clone(),
            },
        )
    };
    let mut wrong = bindings;
    wrong[0].service_declaration[0] ^= 1;
    let changed =
        receive(install(wrong)).expect_err("the same name cannot supply a changed service");
    assert!(changed.into_diagnostics().iter().any(|error| {
        error
            .message
            .contains("completed Build binding belongs to a different service contract")
    }));
    let report = receive(host_packages.clone().for_program(&authorized))
        .expect("admit the independently supplied root package process-ID binding");
    let image = report.retained_native_artifact().unwrap().image().output();
    assert!(image.bytes.windows(8).any(|bytes| bytes == b"_getpid\0"));
    assert!(
        image.bytes == source_image,
        "source and saved-file compilation produced different Windows executables"
    );
    let interpreted = orchestration::prepare_saved_program(
        authorized.clone(),
        &saved,
        profile,
        host_packages.for_program(&authorized),
        orchestration::NativeDestination::RuntimeAdmission,
        &[],
    )
    .expect("saved program reaches independent runtime admission");
    let runtime = interpreted
        .published_runtime_provisions()
        .expect("admitted runtime");
    assert_eq!(runtime.identity(), authorized.artifact().view().identity());
    assert_eq!(runtime.target_slot().owner, profile);
    assert_eq!(
        runtime.source_signature_identity(),
        report
            .published_runtime_provisions()
            .unwrap()
            .source_signature_identity()
    );
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        let exe = workspace.path().join("process-id.exe");
        std::fs::write(&exe, &image.bytes).unwrap();
        let result = std::process::Command::new(exe)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(0), "{result:?}");
    }
}
