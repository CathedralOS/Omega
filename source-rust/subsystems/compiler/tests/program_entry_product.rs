//! The selected Build entry joins the actual accepted publication, rather than
//! selecting an ambient `main` or publishing a second judgment.

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

#[test]
fn declaration_only_checking_preserves_role_and_requires_an_entry_for_execution() {
    for (operation, role) in [
        ("package", build_declarations::BuildDeclarationKind::Package),
        (
            "application",
            build_declarations::BuildDeclarationKind::Application,
        ),
    ] {
        let root = fixture_package_inputs::repo_root()
            .join("build")
            .join(format!(
                "omega-declaration-only-{}-{operation}",
                std::process::id()
            ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let main = root.join("main.omg");
        std::fs::write(&main, "public data Item { value: u64; }\n").unwrap();
        std::fs::write(
            root.join("build.omg"),
            format!(
                "machine build(builder: @mut Build) {{ Build::{operation}(builder, \"entryless\"); }}\n"
            ),
        )
        .unwrap();
        std::thread::scope(|scope| {
            compiler::checked::spawn_compile_worker(scope, || {
                let mut prepared =
                    fixture_package_inputs::reviewed_repository_fixture_package_inputs(
                        &main,
                        Some("macos_arm64"),
                    )?;
                let checked = fixture_package_inputs::check_fixture(
                    &mut prepared,
                    "macos_arm64",
                    Default::default(),
                )?;
                assert_eq!(
                    checked.build().project_declaration.as_ref().unwrap().kind(),
                    role
                );
                assert_eq!(
                    checked.build_admission().application.is_some(),
                    role == build_declarations::BuildDeclarationKind::Application
                );
                assert!(checked.build().config.root_bindings.is_empty());
                assert!(
                    checked
                        .terminal_publication()
                        .accepted()
                        .unwrap()
                        .module()
                        .interfaces.program_entry_contracts
                        .is_empty()
                );
                let refusal =
                    compiler::prepare_program_entry(&checked.clone().into_published().unwrap())
                        .expect_err("execution still needs an actual checked entry");
                assert_eq!(refusal.len(), 1);
                assert_eq!(
                    refusal[0].message,
                    "target `macos_arm64` has no unique checked entry contract"
                );
                drop(checked);
                drop(prepared);
                Ok::<(), Vec<diagnostics::Diagnostic>>(())
            })
            .unwrap()
            .join()
            .unwrap()
        })
        .unwrap_or_else(|diagnostics| {
            panic!("{operation} source checking needs no executable entry: {diagnostics:?}")
        });
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn trait_implementation_nomination_uses_the_exact_binder_and_nominal_bound() {
    let source_text = "trait A {}\ntrait B {}\nGood satisfies A {}\nWrong satisfies B {}\ntrait Holder<Policy satisfies A> {}\ntrait OtherHolder<Policy satisfies B> {}\nboundary trait Chosen: Holder<Good> {}\nboundary trait Other: OtherHolder<Wrong> {}\n";
    let mut sources = source_compiler::source_inputs::SourceMap::default();
    let source_id = sources
        .add_checked_instance(
            "source/main.omg".into(),
            source_text,
            "source".into(),
            Some(semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32]).unwrap()),
            source_admission::SourceOrigin::User,
            source_admission::SourceResolutionStratum::Base,
            source_provenance::DependencyScope::Product,
        )
        .source_id;
    let tokens = crate::frontend_fixture::Lexer::new(source_text)
        .tokenize()
        .unwrap();
    let syntax = crate::frontend_fixture::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
    let mut request = source_compiler::resolution::ResolutionRequest::new(syntax);
    request.sources = Some(std::sync::Arc::new(sources));
    let resolved = source_compiler::resolution::resolve(request).unwrap();
    let typed = source_compiler::typing::lower_symbol_resolved_trees(&resolved).unwrap();
    let declaration = |name: &str| {
        typed
            .traits()
            .iter()
            .find(|row| row.name.as_str() == name)
            .unwrap()
    };
    let scope = |name: &str| {
        source_compiler::source_state::trait_definition::AppliedTraitScope::new(
            &typed,
            declaration(name),
            source_compiler::source_state::trait_definition::TraitApplicationArguments {
                lexical_owner: declaration(name).symbol,
                enclosing_data_applications: &[],
                arguments: &[],
                lifetimes: &[],
            },
        )
        .unwrap()
    };
    let chosen = scope("Chosen")
        .declaring_scope(&typed, declaration("Holder").symbol)
        .unwrap();
    let other = scope("Other")
        .declaring_scope(&typed, declaration("OtherHolder").symbol)
        .unwrap();
    let parameter = typed.trait_type_parameters(declaration("Holder"))[0].symbol;
    let foreign_parameter = typed.trait_type_parameters(declaration("OtherHolder"))[0].symbol;
    let selected = chosen
        .named_implementation_argument(&typed, parameter)
        .unwrap();
    assert_eq!(
        selected.symbol,
        typed
            .conformances()
            .iter()
            .find(|row| row
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "Good"))
            .unwrap()
            .symbol
    );
    assert_eq!(selected.trait_symbol, declaration("A").symbol);
    assert!(
        chosen
            .named_implementation_argument(&typed, foreign_parameter)
            .unwrap_err()
            .contains("not declared"),
        "same-spelled Policy from another owner cannot bind this application"
    );
    let mut wrong = chosen.environment().scope(&typed).unwrap();
    let reference = other.parameter_argument(&typed, foreign_parameter);
    wrong
        .substitutions
        .iter_mut()
        .find(|(owner, _)| *owner == parameter)
        .unwrap()
        .1 = reference;
    assert!(
        wrong
            .named_implementation_argument(&typed, parameter)
            .unwrap_err()
            .contains("another nominal trait")
    );
    let mut open = chosen.environment().scope(&typed).unwrap();
    open.substitutions.retain(|(owner, _)| *owner != parameter);
    assert!(
        open.named_implementation_argument(&typed, parameter)
            .unwrap_err()
            .contains("not closed")
    );
}

#[path = "../../../../tests/support/frontend.rs"]
mod frontend_fixture;

// The unchanged fixture imports GNU libc's versioned abs through the GNU loader.
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
#[test]
fn dynamic_native_publication_and_receiver_replay_need_no_producer_recheck_report() {
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let root = fixture_package_inputs::repo_root().join(
                "source-rust/subsystems/compiler/tests/fixtures/build/pass/native_row_outranks_scoped_implementations_exit/main.omg",
            );
            let discovered = fixture_package_inputs::discover_fixture(&root, Default::default())
                .unwrap_or_else(|errors| panic!("dynamic fixture discovery: {errors:#?}"));
            let product = compiler::CompilationProduct::Native(products::NativeProductRequest {
                installation_assumptions: Vec::new(),
                retain_native_inputs: false,
                retain_native_stage_inputs: false,
                image: Default::default(),
                private_entry_ram: Vec::new(),
                terminal_authority_policy: provider_admission::current_terminal_authority_policy(),
                terminal_authority_permission_policy: None,
                optimization_rollback: Default::default(),
            });
            let mut prepared = fixture_package_inputs::review_fixture(
                discovered,
                &[target::TargetProfile::LinuxX64],
                product,
            )
            .unwrap_or_else(|errors| panic!("dynamic fixture review: {errors:#?}"));
            let output = fixture_package_inputs::produce_fixture(&mut prepared, Some("linux_x86_64"))
                .unwrap_or_else(|errors| panic!("dynamic fixture production: {errors:#?}"));
            let report = orchestration::complete_product(output)
                .and_then(compiler::CompilationOutput::into_report)
                .expect("complete the original versioned libc import");
            let artifact = report
                .retained_dynamic_elf_artifact()
                .expect("the selected abs import must produce a dynamic ELF artifact");
            let output = artifact.image().output();
            assert!(output.final_image_imports > 0);
            assert!(output.compiler_text_validation.is_none());
            assert!(output.compiler_function_validation.is_none());
            artifact
                .validate()
                .expect("the receiver completely replays fresh dynamic bytes without a report");
            let bytes = output.bytes.clone();
            let directory = platform_custody::temporary_directory::TemporaryDirectory::create(
                "dynamic-image-publication",
                false,
            )
            .unwrap();
            let published = products::publication::publish_compilation(report, directory.path())
                .expect("ordinary dynamic publication requires no legacy replay report");
            let path = published.checked_native_executable_path().unwrap();
            assert_eq!(std::fs::read(path).unwrap(), bytes);
            let execution = std::process::Command::new(path)
                .stdin(std::process::Stdio::null())
                .output()
                .expect("run the original dynamically linked program");
            assert_eq!(
                execution.status.code(),
                Some(0),
                "stdout={:?}; stderr={:?}",
                String::from_utf8_lossy(&execution.stdout),
                String::from_utf8_lossy(&execution.stderr),
            );
        })
        .unwrap()
        .join()
        .unwrap();
    });
}
