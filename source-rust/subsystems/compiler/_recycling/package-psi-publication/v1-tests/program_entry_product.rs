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
        r#"machine build(builder: @mut Build) {
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

            let (signature, application) =
                compiler::prepare_program_entry(&checked.clone().into_published().unwrap())
                    .unwrap();
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
                program_entry_contracts::SelectedProgramEntrySignature::from_module_entry(
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
            let schema = source_compiler::entry_contracts::selected_program_entry_boundary(
                catalog,
                &package_compilation::source_imports::source_target(
                    signature.target_slot().owner,
                    None,
                ),
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
            let fresh = std::sync::Arc::new(
                source_compiler::SourcePublication::judge(
                    std::sync::Arc::clone(accepted.source()),
                    executable_roster.clone(),
                    checked.terminal_publication().optimizations().clone(),
                )
                .expect("all original scope is judged with the requested descriptors"),
            );
            let receive = |settled: &products::SelectedProviderResult| {
                products::CheckedCompilation::from_settled_execution(
                    products::CheckedCompilationInputs {
                        settled: settled.clone(),
                        source_inspection: std::sync::Arc::new(
                            products::SelectedProviderSource::new(
                                std::sync::Arc::clone(checked.source_inspection().inventory()),
                                std::sync::Arc::clone(settled.shared_plans()),
                            ),
                        ),
                        build: std::sync::Arc::new(checked.build().clone()),
                        build_admission: checked.build_admission().clone(),
                    },
                    std::sync::Arc::new(checked.sources().clone()),
                    Default::default(),
                    std::sync::Arc::clone(&fresh),
                )
            };
            let rechecked = receive(checked.execution())
                .expect("a fresh judgment of the same program can receive its selected providers");
            rechecked.admit_trust().unwrap();
            assert!(fresh.published().is_none());

            let independently_checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            )
            .result
            .unwrap_or_else(|failure| {
                panic!(
                    "independent program owner: {:?}",
                    failure.into_diagnostics()
                )
            });
            assert_eq!(
                independently_checked.execution().original_program(),
                checked.execution().original_program()
            );
            assert!(!std::sync::Arc::ptr_eq(
                independently_checked.source_inspection().program(),
                checked.source_inspection().program(),
            ));
            assert_eq!(
                receive(independently_checked.execution()).unwrap_err()[0].message,
                "selected providers belong to a different accepted program"
            );

            let unrelated = workspace.join("different-program");
            std::fs::create_dir_all(&unrelated).unwrap();
            std::fs::copy(workspace.join("build.omg"), unrelated.join("build.omg")).unwrap();
            let other_main = unrelated.join("main.omg");
            std::fs::write(&other_main, "machine main() {}\n").unwrap();
            let other_checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&other_main, Some("linux_x86_64")),
            )
            .result
            .unwrap_or_else(|failure| panic!("second program: {:?}", failure.into_diagnostics()));
            assert_eq!(
                other_checked.execution().target_profile(),
                checked.execution().target_profile()
            );
            assert_ne!(
                other_checked.execution().original_program(),
                checked.execution().original_program()
            );
            // Both programs use the same Build entry and target. Publish the
            // other checked source with its own optimization and program identities.
            let other_publication = other_checked.terminal_publication();
            let other_program = portable_publication::ProductCodeSource {
                publication: other_publication,
                sources: other_checked.sources(),
            }
            .publish(
                other_publication.optimizations(),
                &mut source_compiler::UnmeasuredProduction,
            )
            .expect("publish the independently checked program");
            let refusal = products::PublishedNativeEntry::for_program(
                other_program.artifact(),
                checked.execution(),
            )
            .err()
            .expect("a different program cannot borrow this target selection");
            assert!(diagnostics::join_messages(&refusal).contains("different accepted program"));
            let mismatch = receive(other_checked.execution()).unwrap_err();
            assert_eq!(
                mismatch[0].message,
                "selected providers belong to a different accepted program"
            );
            assert!(rechecked.published_artifact().is_none());
            let ready = portable_publication::ProductCodeSource {
                publication: &fresh,
                sources: checked.sources(),
            }
            .publish(
                fresh.optimizations(),
                &mut source_compiler::UnmeasuredProduction,
            )
            .expect("publish only after checking the fresh source");
            assert!(std::sync::Arc::ptr_eq(
                rechecked.published_artifact().unwrap(),
                ready.artifact()
            ));
            let executable = fresh.accepted().unwrap();
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
                    row.projections_in_context(
                        typed,
                        source_provenance::DependencyScope::Product,
                    )
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
                source_compiler::judge_source_roster(executable_roster, std::sync::Arc::clone(accepted.source()))
                    .is_err()
            );
        })
        .unwrap()
        .join()
        .unwrap();
    });
    std::fs::remove_dir_all(workspace).unwrap();
}

#[path = "../../../../tests/support/frontend.rs"]
mod frontend_fixture;

// The unchanged fixture imports GNU libc's versioned abs through the GNU loader.
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
