//! The corpus chooses products from executed Build observations, never syntax.
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::CompilationProduct;
use fixture_package_inputs::{discover_fixture, produce_fixture, repo_root, review_fixture};
use target::TargetProfile;

// The phase counter is process-wide; ordinary libtest may run these together.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn reviewed_fixture_discovers_an_empty_dangerous_service_roster() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let root = repo_root()
        .join("tests/fixtures/pass/facts/compound_index_guard_through_call_result_exit/main.omg");
    let discovered = discover_fixture(&root, Default::default()).expect("kept index-guard Build");
    let mut prepared = review_fixture(
        discovered,
        &[TargetProfile::LinuxX64],
        CompilationProduct::Terminal(Default::default()),
    )
    .expect("the checked program needs no dangerous service");
    assert!(
        prepared.package_inputs.is_some(),
        "the test must reach package review"
    );
    let output = produce_fixture(&mut prepared, Some("linux_x86_64"))
        .expect("the reviewed program still publishes its original checks");
    assert!(
        output
            .into_report()
            .expect("Terminal report")
            .into_retained_terminal_artifact()
            .is_some()
    );
}

#[test]
fn fixture_accepts_the_entry_schema_from_its_checked_supplier() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let workspace = repo_root().join("build").join(format!(
        "fixture-entry-schema-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&workspace).unwrap();
    let standard_library = fixture_package_inputs::bundled_standard_library_root();
    let root = workspace.join("main.omg");
    std::fs::write(&root, "machine main() {}\n").unwrap();
    std::fs::write(
        root.with_file_name("build.omg"),
        format!(
            r#"machine build(builder: &mut Build) {{
    Build::application(builder, "fixture_entry_schema");
    let supplier: DependencyRegistration = Build::dependency(builder, dependency::from_source(Source::Path {{ location: {standard_library:?} }}));
    Build::bind(builder, macos_arm64::ProgramEntry, main);
}}"#
        ),
    )
    .unwrap();
    let discovered =
        discover_fixture(&root, Default::default()).expect("complete the fixture Build");
    let profile = TargetProfile::MacosArm64;
    let mut prepared = review_fixture(
        discovered,
        &[profile],
        CompilationProduct::Terminal(Default::default()),
    )
    .expect("admit the checked standard-library entry schema");
    let inputs = prepared.package_inputs.as_ref().unwrap();
    let standard_library = standard_library.canonicalize().unwrap();
    let (supplier, _) = inputs
        .packages()
        .find(|(_, root)| *root == standard_library)
        .expect("exact standard-library package");
    let role = package_compilation::AcceptedSemanticBindingRole::MacosArm64ProgramEntry;
    let accepted = inputs.accepted_semantic_binding(role).unwrap();
    assert_eq!(accepted.package(), supplier);
    assert_eq!(accepted.declaration_path(), "MacosApplication");
    let published = produce_fixture(&mut prepared, Some(profile.target_name()))
        .expect("carry the accepted binding into Product completion");
    assert!(
        published
            .into_report()
            .unwrap()
            .into_retained_terminal_artifact()
            .is_some()
    );
    drop(prepared);
    std::fs::remove_dir_all(workspace).unwrap();
}

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

#[test]
fn target_audit_rejoins_compiled_foreign_operands() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let root = repo_root().join("tests/fixtures/run/providers/native_boundary_supply/main.omg");
    let report = orchestration::compile_sources(
        &mut compiler::CompilationSession::default(),
        compiler::CompileRequest::new(compiler::CompileOptions {
            root_path: root,
            build_dir: None,
            target_name: Some("macos_arm64".to_owned()),
        })
        .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|errors| panic!("process-ID native production: {errors:?}"));
    let artifact = report
        .retained_native_artifact()
        .expect("retain the emitted direct native artifact");
    assert_eq!(
        artifact.object().foreign_calls().len(),
        1,
        "the kept source reaches its native process-ID binding"
    );
    image_emission::verify_retained_target_operations(artifact.object())
        .unwrap_or_else(|error| panic!("process-ID target audit: {error:?}"));
}

#[test]
fn target_audit_checks_the_retained_indirect_result_register() {
    use calling_conventions::MachineRegister::{Aarch64X, X86Rcx, X86Rdi, X86Rdx};
    use calling_conventions::{IndirectPointerLocation, ValueLocation, ValueShape};
    use target::TargetProfile::{LinuxArm64, LinuxX64, MacosArm64, WindowsX64};
    use target_operations::{NativeCallOrigin, TargetCallResult, TargetUnitOperation};
    use target_operations_to_selected_instructions::{
        TargetOperationAuditError, verify_target_operation_plan_with_closed_module,
    };

    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let root =
        repo_root().join("samples/cli/facts/nominal_record_representation/call_result/main.omg");
    let cases = [
        (LinuxX64, X86Rdi, X86Rdx),
        (LinuxArm64, Aarch64X(8), Aarch64X(9)),
        (MacosArm64, Aarch64X(8), Aarch64X(9)),
        (WindowsX64, X86Rcx, X86Rdx),
    ];
    let request = compiler::CompileRequest::for_targets(
        root.clone(),
        root.with_file_name("build"),
        cases.iter().map(|&(profile, _, _)| Some(profile)).collect(),
    )
    .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact);
    let outcomes =
        orchestration::compile_sources(&mut compiler::CompilationSession::default(), request)
            .unwrap_or_else(|errors| panic!("aggregate return matrix: {errors:?}"));
    assert_eq!(outcomes.outcomes().len(), cases.len());
    for (profile, hidden_register, wrong_register) in cases {
        let outcome = outcomes
            .outcomes()
            .iter()
            .find(|outcome| outcome.target_profile() == Some(profile))
            .expect("requested target has a compilation outcome");
        let report = outcome
            .report()
            .unwrap_or_else(|| panic!("{profile:?} aggregate return: {:?}", outcome.diagnostics()));
        let object = report
            .retained_native_artifact()
            .expect("emitted native artifact")
            .object();
        let target = object
            .retained_fragment_source()
            .expect("object retains the optimized target producer")
            .source()
            .source()
            .source()
            .source()
            .optimized_target();
        let optimized = target.optimized();
        let verify = |plan| {
            verify_target_operation_plan_with_closed_module(
                plan,
                optimized.plan(),
                optimized.unit(),
                target.boundary_origins(),
                target.lowering_facts().installed_interrupts(),
                target.lowering_facts().closed_module(),
            )
        };
        verify(target.target_operations())
            .unwrap_or_else(|error| panic!("{profile:?} aggregate target audit: {error:?}"));
        let mut mismatched = target.target_operations().clone();
        let mut results = mismatched.functions.iter_mut().flat_map(|function| {
            let caller = function.machine;
            function
                .graph
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .filter_map(move |operation| {
                    let TargetUnitOperation::Call {
                        origin: NativeCallOrigin::Authored,
                        result: TargetCallResult::Structural { .. },
                        call_plan,
                        ..
                    } = &mut operation.kind
                    else {
                        return None;
                    };
                    let placement = call_plan.result.as_mut()?;
                    (placement.shape == ValueShape::integer(24, 8)).then_some((caller, placement))
                })
        });
        let (caller, placement) = results
            .next()
            .expect("compiled call returns the three-word record");
        assert!(results.next().is_none());
        assert_eq!(
            placement.locations.as_slice(),
            [ValueLocation::indirect(
                IndirectPointerLocation::Register(hidden_register),
                None,
                24,
                8,
            )],
        );
        // Keep the original aggregate geometry and change only the caller's
        // hidden register. The audit must catch disagreement with the callee.
        placement.locations[0] = ValueLocation::indirect(
            IndirectPointerLocation::Register(wrong_register),
            None,
            24,
            8,
        );
        let error = verify(&mismatched).expect_err(
            "a different hidden result register must fail independent target validation",
        );
        assert!(
            matches!(error, TargetOperationAuditError::SourceCustodyMismatch {
                machine: Some(machine), ..
            } if machine == caller),
            "{profile:?}: {error:?}"
        );
    }
}
