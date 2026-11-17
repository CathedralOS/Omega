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
                "machine build(builder: &mut Build) {{ Build::{operation}(builder, \"entryless\"); }}\n"
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
                        .program_entry_contracts
                        .is_empty()
                );
                let refusal = compiler::prepare_program_entry(&checked)
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
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add_checked_instance(
            "source/main.omg".into(),
            source_text,
            "source".into(),
            Some(semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32]).unwrap()),
            source::SourceOrigin::User,
            source::SourceResolutionStratum::Base,
            source::DependencyScope::Product,
        )
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(source_text)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
    let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
    request.sources = Some(std::sync::Arc::new(sources));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let declaration = |name: &str| {
        typed
            .traits()
            .iter()
            .find(|row| row.name.as_str() == name)
            .unwrap()
    };
    let scope = |name: &str| {
        typed_trees::trait_definition::AppliedTraitScope::new(
            &typed,
            declaration(name),
            typed_trees::trait_definition::TraitApplicationArguments {
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

#[test]
fn selected_entry_receipt_keeps_the_accepted_artifact_and_rejects_other_owners() {
    let repository = fixture_package_inputs::repo_root();
    let workspace = repository.join("build").join(format!(
        "omega-selected-entry-receipt-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&workspace);
    std::fs::create_dir_all(&workspace).unwrap();
    let main = workspace.join("main.omg");
    std::fs::copy(
        repository.join("tests/fixtures/run/calls/scalar_contract_route_exit/main.omg"),
        &main,
    )
    .unwrap();
    // The receipt belongs to Build's selected entry. Keep the scalar program
    // unchanged and select its entry explicitly for each hosted target.
    std::fs::write(
        workspace.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    Build::application(builder, "selected_entry_receipt");
    Build::bind(builder, windows_x86_64::ProgramEntry, main);
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
    Build::bind(builder, linux_arm64::ProgramEntry, main);
    Build::bind(builder, macos_arm64::ProgramEntry, main);
}
"#,
    )
    .unwrap();
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            )
            .result
            .unwrap_or_else(|failure| {
                panic!("default scalar check: {:?}", failure.into_diagnostics())
            });
            let accepted = checked.terminal_publication().accepted().unwrap();
            let program = checked.terminal_publication().published().unwrap();
            let module = program.artifact().semantic_module();
            assert_eq!(module, accepted.module());
            assert!(std::sync::Arc::ptr_eq(
                accepted.source(),
                checked.source_inspection().program(),
            ));
            let evidence = program.evidence().expect("actual discharged evidence");
            assert!(!evidence.seals.is_empty());
            assert_eq!(evidence.module, program.artifact().manifest().semantic());

            let (signature, application) = compiler::prepare_program_entry(&checked).unwrap();
            let receipt = application.receipt.unwrap().expect("actual entry receipt");
            assert_eq!(
                receipt.source_signature_identity(),
                signature.identity().bytes()
            );
            assert_eq!(
                receipt.terminal_psi_identity(),
                program.artifact().manifest().semantic()
            );
            assert_eq!(application.roots.len(), 1);
            assert_eq!(
                application.roots[0].as_ref().unwrap().machine,
                receipt.terminal_entry()
            );
            assert_eq!(
                signature.target_slot().owner,
                target::TargetProfile::from_canonical_target_name("linux_x86_64").unwrap(),
            );
            let root = terminal_psi::PreparedNominalRoot::new(
                signature.declaration(),
                "diagnostic name is not authority",
                None,
            )
            .unwrap();
            assert_eq!(root.resolve(module).unwrap().id, receipt.terminal_entry());
            let wrong_owner =
                terminal_psi::PreparedNominalRoot::new("other-package::main", "main", None)
                    .unwrap();
            assert!(
                terminal_codec::entry_receipt::prepare_program_entry_receipt(
                    &wrong_owner,
                    signature.identity().bytes(),
                    program.artifact(),
                )
                .is_err()
            );

            // A portable body with the same entry ID is insufficient when its
            // declaration or arrival signature is absent.
            let mut malformed = module.clone();
            malformed
                .machines
                .iter_mut()
                .find(|machine| machine.id == receipt.terminal_entry())
                .unwrap()
                .declaration = None;
            assert!(
                program_entry_plan::SelectedProgramEntrySignature::from_module_entry(
                    signature.target_slot(),
                    &malformed,
                    receipt.terminal_entry(),
                    "main",
                )
                .unwrap_err()
                .contains("source declaration")
            );

            // Entry metadata belongs to the initial shared source request.
            // Rejudging that same roster must preserve its accepted artifact;
            // no target may patch metadata into an existing publication.
            let catalog = checked
                .source_inspection()
                .inventory()
                .services_for(accepted.source().as_ref())
                .expect("the accepted source's retained Product service catalog");
            let schema = package_compilation::selected_program_entry_boundary(
                catalog,
                signature.target_slot(),
                None,
            )
            .expect("the selected target's authenticated source schema");
            assert!(
                checked
                    .terminal_publication()
                    .with_declaration_metadata(&[schema])
                    .unwrap_err()[0]
                    .message
                    .contains("before Terminal publication")
            );
            let mut executable_roster = checked.terminal_publication().roster().unwrap().clone();
            assert!(executable_roster.declaration_metadata.contains(&schema));
            let executable = executable_roster
                .judge(std::sync::Arc::clone(accepted.source()))
                .expect("all original scope is judged with the requested descriptors");
            assert!(std::sync::Arc::ptr_eq(
                accepted.source(),
                executable.source()
            ));
            assert_eq!(
                accepted.boundary_requirements_declared().unwrap(),
                executable.boundary_requirements_declared().unwrap()
            );
            // The semantic arrival is shared; each of the four bound targets
            // contributes its own physical arrival before source judgment.
            assert_eq!(module.calling_abi_signatures.len(), 5);
            let typed = accepted.source();
            let catalog = typed.boundary_signatures();
            let mut expected_abis = [
                ("ProgramStorageEntry", "source/library/core/extent.omg"),
                (
                    "LinuxArm64PhysicalEntry",
                    "source/library/std/targets/linux_arm64/entry.omg",
                ),
                (
                    "LinuxX86_64PhysicalEntry",
                    "source/library/std/targets/linux_x86_64/entry.omg",
                ),
                (
                    "MacosArm64PhysicalEntry",
                    "source/library/std/targets/macos_arm64/entry.omg",
                ),
                (
                    "WindowsProcessEntry",
                    "source/library/std/targets/windows_x86_64/entry.omg",
                ),
            ]
            .map(|(expected_owner, expected_source)| {
                let path = repository.join(expected_source);
                let mut rows = catalog.abi_signatures().iter().filter(|row| {
                    row.projections_in_context(typed, source::DependencyScope::Product)
                        .any(|projection| {
                            typed
                                .trait_machine_signature(projection.source)
                                .is_some_and(|(owner, signature)| {
                                    owner.name.as_str() == expected_owner
                                        && signature.name.as_str() == "enter"
                                })
                                && typed
                                    .symbols
                                    .symbol_provenance_source_span(projection.source)
                                    .and_then(|span| typed.symbols.source_file(span))
                                    .is_some_and(|source| source.path == path)
                        })
                });
                let row = rows
                    .next()
                    .expect("the fixed source owns its exact arrival ABI");
                assert!(
                    rows.next().is_none(),
                    "one Product ABI per fixed source arrival: {expected_owner}"
                );
                semantic_vocabulary::CallingAbiIdentity {
                    requirement_identity: row.requirement_identity.clone(),
                    declaring_application: row.declaring_application.clone(),
                    operands: row.operands.clone(),
                }
            });
            expected_abis.sort_by_key(|abi| abi.id());
            let mut actual_abis = module
                .calling_abi_signatures
                .iter()
                .map(|row| row.identity(&module.identities).unwrap())
                .collect::<Vec<_>>();
            actual_abis.sort_by_key(|abi| abi.id());
            assert_eq!(actual_abis, expected_abis);
            let native_code = executable
                .produce(
                    Default::default(),
                    &mut source_compiler::UnmeasuredProduction,
                )
                .unwrap()
                .into_shared_portable()
                .unwrap();
            let native_module = native_code.artifact().semantic_module();
            assert_eq!(native_module, executable.module());
            assert_eq!(
                native_module.calling_abi_signatures,
                module.calling_abi_signatures
            );
            assert!(
                native_module
                    .calling_abi_signatures
                    .iter()
                    .all(|row| row.storage.is_ok()),
                "the real address and linear storage descriptors must materialize: {:?}",
                native_module.calling_abi_signatures
            );
            let linear = native_module
                .structural_types
                .iter()
                .find(|row| row.owns_linear_claim)
                .expect("the semantic Extent descriptor retains its linear custody");
            assert_eq!(
                linear.multiplicity,
                terminal_psi::StructuralMultiplicity::Linear
            );
            assert!(
                !linear.is_copyable()
                    && !linear.no_code_affine_cleanup
                    && !linear.descriptor_cleanup
            );
            assert_eq!(
                native_code.artifact().manifest().semantic(),
                program.artifact().manifest().semantic()
            );
            assert_eq!(
                native_code.evidence().unwrap().module,
                native_code.artifact().manifest().semantic()
            );
            assert_eq!(program.artifact().semantic_module(), accepted.module());
            executable_roster
                .declaration_metadata
                .push(symbols::SymbolHandle::invalid());
            assert!(
                executable_roster
                    .judge(std::sync::Arc::clone(accepted.source()))
                    .is_err()
            );
        })
        .unwrap()
        .join()
        .unwrap();
    });
    std::fs::remove_dir_all(workspace).unwrap();
}
