// V1 integration cases from compiler/tests/package_review.rs.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod native_entry_controls;

#[test]
fn saved_native_inputs_roundtrip_the_checked_product_on_every_host() {
    use std::sync::Arc;
    let source = kept_fixture("run/calls/scalar_contract_route_exit/main.omg");
    let discovered = fixture_package_inputs::discover_fixture(
        &source,
        package_compilation::BuildExecutionInputs {
            snapshot: fixture_package_inputs::fixture_build_snapshot(&source).unwrap(),
            ..Default::default()
        },
    )
    .expect("discover the unchanged scalar-contract program");
    // Preparing Windows inputs exercises the stored product on every host;
    // this control neither emits nor executes a platform image.
    let profile = target::TargetProfile::WindowsX64;
    let product = compiler::CompilationProduct::Native(products::NativeProductRequest {
        retain_native_inputs: true,
        retain_native_stage_inputs: false,
        installation_assumptions: Vec::new(),
        image: Default::default(),
        private_entry_ram: Vec::new(),
        terminal_authority_policy:
            provider_admission::current_terminal_authority_policy(),
        terminal_authority_permission_policy: None,
        optimization_rollback: Default::default(),
    });
    let mut prepared = fixture_package_inputs::review_fixture(discovered, &[profile], product)
        .expect("admit the scalar-contract native product");
    assert_eq!(prepared.plans.len(), 1);
    let (actual_profile, request) = prepared.plans.pop().unwrap();
    assert_eq!(actual_profile, profile);
    let build = orchestration::prepare_package(
        prepared.session.compiler(),
        request.occurrences,
        &mut prepared.completed,
    )
    .unwrap_or_else(|error| {
        panic!(
            "prepare scalar-contract source: {:?}",
            error.failure.into_diagnostics()
        )
    });
    let mut outcomes = prepared
        .session
        .compiler()
        .complete_package_products(vec![(
            (),
            compiler::PreparedPackageProduct {
                build,
                product: request.product,
                permissions: request.terminal_permissions,
            },
        )])
        .expect("produce checked native inputs");
    assert_eq!(outcomes.len(), 1);
    let ((), output) = outcomes.pop().unwrap();
    let compiler::CompilationOutput::Native(pending) = output.expect("prepare native application")
    else {
        panic!("expected the pending native application");
    };
    // Saved inputs must recover after their source session has been released.
    drop(prepared);
    let original = &pending.terminal;
    assert!(
        original
            .evidence()
            .is_some_and(|evidence| !evidence.seals.is_empty()),
        "the unchanged fixture must produce actual operation evidence"
    );
    let canonical = Arc::new(
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&original.artifact().to_bytes())
            .unwrap(),
    );
    let authorization = original.authorization().clone();
    authorization.require_artifact(&canonical).unwrap();
    let (recovered, permissions, assumptions) = products::recover_native_derivation_inputs(
        authorization,
        pending
            .retained_native_inputs
            .as_ref()
            .expect("requested saved inputs"),
    )
    .expect("recover the exact saved pair");
    let mut differences = Vec::new();
    if recovered.evidence() != original.evidence() {
        differences.push(format!(
            "operation evidence changed: original {} seals, recovered {:?}",
            original.evidence().unwrap().seals.len(),
            recovered.evidence().map(|evidence| evidence.seals.len())
        ));
    }
    let original_identities = original
        .native_realization_proposal()
        .unwrap()
        .providers()
        .selection
        .identities()
        .entries();
    let recovered_identities = recovered
        .native_realization_proposal()
        .unwrap()
        .providers()
        .selection
        .identities()
        .entries();
    if original_identities != recovered_identities {
        let first = original_identities
            .iter()
            .zip(recovered_identities)
            .position(|(left, right)| left != right);
        differences.push(format!("provider identity order changed at {first:?}: original {} entries, recovered {} entries", original_identities.len(), recovered_identities.len()));
    }
    assert!(
        recovered == *original,
        "saved native inputs changed the checked product: {}",
        differences.join("; ")
    );
    assert_eq!(permissions, pending.accepted_package_permissions);
    assert!(assumptions.is_empty());

    // The saved companion is data: it cannot attach another module's seals
    // or an intact seal whose obligation is not owned by the authorized body.
    let evidence = original.evidence().unwrap();
    let original_blob = terminal_codec::encode_operation_evidence(evidence).unwrap();
    let saved = pending.retained_native_inputs.as_ref().unwrap();
    assert!(saved.ends_with(&original_blob));
    let replace_evidence = |changed: &terminal_codec::EvidenceSeals| {
        let blob = terminal_codec::encode_operation_evidence(changed).unwrap();
        // The mutations below replace fixed-width identity coordinates only.
        assert_eq!(blob.len(), original_blob.len());
        let mut forged = saved.clone();
        let start = forged.len() - blob.len();
        forged[start..].copy_from_slice(&blob);
        forged
    };
    let refuse = |bytes: &[u8], expected: &str| {
        let errors =
            products::recover_native_derivation_inputs(original.authorization().clone(), bytes)
                .expect_err("saved evidence cannot supply attachment authority");
        assert!(
            errors.iter().any(|error| error.message.contains(expected)),
            "{errors:?}"
        );
    };
    let mut foreign = evidence.as_ref().clone();
    foreign.module.program_fingerprint = terminal_psi::SemanticFingerprint::from_bytes([9; 32]);
    refuse(
        &replace_evidence(&foreign),
        "operation evidence belongs to another canonical artifact",
    );
    let mut unowned = evidence.as_ref().clone();
    let missing = semantic_vocabulary::MachineId::new(u64::MAX).unwrap();
    assert!(
        original
            .artifact()
            .semantic_module()
            .machine(missing)
            .is_none()
    );
    unowned.seals[0].machine = missing;
    unowned.seals[0].fingerprint = unowned.seals[0].recompute().unwrap();
    refuse(&replace_evidence(&unowned), "UnownedSeal");
    let mut damaged = saved.clone();
    *damaged.last_mut().unwrap() ^= 1;
    refuse(&damaged, "operation evidence seal fingerprint mismatch");
}

// The foreign contract keeps its authored method identity, while its closed
// outer and local arguments control exactly which call instances it can supply.
// Restrict an existing working registrar to the callback it actually receives.
// Its program, C provider, layout and exit oracle remain the original control.
#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn closed_native_binding_reloads_and_executes_the_descriptor_callback() {
    use std::sync::Arc;
    let root = fixture_package_inputs::repo_root();
    let fixture = kept_fixture("pass/layouts/private_callback_inherited_policy_exit");
    let scratch = root.join("build").join(format!(
        "omega-closed-native-callback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&scratch).unwrap();
    for entry in std::fs::read_dir(&fixture).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            std::fs::copy(entry.path(), scratch.join(entry.file_name())).unwrap();
        }
    }
    assert_eq!(
        std::fs::read(scratch.join("main.omg")).unwrap(),
        std::fs::read(fixture.join("main.omg")).unwrap()
    );
    let original = std::fs::read_to_string(scratch.join("build.omg")).unwrap();
    assert_eq!(
        original.matches("Build::bind<Registrar::register>").count(),
        1
    );
    let build = original
        .replace(
            "Build::bind<Registrar::register>",
            "Build::bind<Registrar::register<TimerHandler::call>>",
        )
        .replace(
            "location: \"../../../../../source/library/std\"",
            &format!("location: {:?}", root.join("source/library/std")),
        );
    std::fs::write(scratch.join("build.omg"), build).unwrap();
    let provider = scratch.join("provider");
    let prepared_provider = std::process::Command::new("python3")
        .arg(fixture.join("fixture/prepare.py"))
        .arg("--output")
        .arg(&provider)
        .output()
        .expect("prepare the existing authored C provider");
    assert!(
        prepared_provider.status.success(),
        "C provider: {:?}",
        prepared_provider
    );
    eprintln!(
        "C_PROVIDER_PREPARATION {}",
        String::from_utf8_lossy(&prepared_provider.stdout)
    );
    let source = scratch.join("main.omg");
    let discovered = fixture_package_inputs::discover_fixture(
        &source,
        package_compilation::BuildExecutionInputs {
            snapshot: fixture_package_inputs::fixture_build_snapshot(&source).unwrap(),
            ..Default::default()
        },
    )
    .expect("discover the exact closed registrar binding");
    let product = compiler::CompilationProduct::Native(products::NativeProductRequest {
        installation_assumptions: Vec::new(),
        retain_native_inputs: true,
        retain_native_stage_inputs: false,
        image: Default::default(),
        private_entry_ram: Vec::new(),
        terminal_authority_policy:
            provider_admission::current_terminal_authority_policy(),
        terminal_authority_permission_policy: None,
        optimization_rollback: Default::default(),
    });
    let mut prepared = fixture_package_inputs::review_fixture(
        discovered,
        &[target::TargetProfile::MacosArm64],
        product,
    )
    .expect("admit the closed native product without weakening its callback contract");
    // Stop at the ordinary package producer's pending native result so saved
    // inputs can be recovered before native completion consumes them.
    assert_eq!(prepared.plans.len(), 1);
    let (profile, request) = prepared.plans.pop().unwrap();
    assert_eq!(profile, target::TargetProfile::MacosArm64);
    let root_occurrence = request.occurrences.len() - 1;
    let build = orchestration::prepare_package(
        prepared.session.compiler(),
        request.occurrences,
        &mut prepared.completed,
    )
    .unwrap_or_else(|error| {
        panic!(
            "prepare the admitted package's exact source closure at occurrence {}: {:?}",
            error.occurrence,
            error.failure.into_diagnostics()
        )
    });
    let mut outcomes = prepared
        .session
        .compiler()
        .complete_package_products(vec![(
            (),
            compiler::PreparedPackageProduct {
                build,
                product: request.product,
                permissions: request.terminal_permissions,
            },
        )])
        .expect("produce the native product and saved inputs");
    assert_eq!(outcomes.len(), 1);
    let ((), output) = outcomes.pop().unwrap();
    let mut output = output.expect("produce the admitted native application");
    let compiler::CompilationOutput::Native(pending) = &output else {
        panic!("expected native application work");
    };
    let bytes = pending
        .retained_native_inputs
        .as_ref()
        .expect("save the exact native derivation inputs");
    let artifact = Arc::new(
        terminal_codec::CanonicalTerminalArtifact::from_bytes(
            &pending.terminal.artifact().to_bytes(),
        )
        .unwrap(),
    );
    let (recovered, recovered_permissions, installation_assumptions) =
        products::recover_native_derivation_inputs(
            {
                let authorization = pending.terminal.authorization().clone();
                authorization.require_artifact(&artifact).unwrap();
                authorization
            },
            bytes,
        )
        .expect("reload the closed binding from its canonical artifact and saved inputs");
    assert_eq!(recovered, pending.terminal);
    assert_eq!(recovered_permissions, pending.accepted_package_permissions);
    assert!(installation_assumptions.is_empty());
    // Keep the already retained coordinates with a failure so a missing choice
    // can be distinguished from a different closed call without another probe.
    let proposal = recovered.native_realization_proposal().unwrap();
    let module = recovered.artifact().semantic_module();
    let mut binding_context = format!(
        "target={} selected_plans={} direct_leaves={} callbacks={}\n",
        proposal.target_profile().target_name(),
        proposal.providers().selection.plans().len(),
        proposal.providers().direct_boundary_leaves.len(),
        module.callback_applications.len(),
    );
    for callback in &module.callback_applications {
        let operation = module
            .machine(callback.registrar_machine)
            .expect("retained callback registrar machine")
            .operations()
            .find(|operation| operation.id == callback.registrar)
            .expect("retained callback registrar operation");
        let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = operation.kind else {
            panic!("retained registrar must be a boundary call");
        };
        let declaration = module.boundary_machine(boundary).unwrap();
        let abi = module.calling_abi_identity(boundary).unwrap();
        binding_context.push_str(&format!(
            "registrar={:?} boundary={boundary:?} owner={:?} required={:?} abi={abi:?}\n",
            callback.registrar,
            module.identities.text(declaration.owner.identity()),
            module.identities.text(declaration.identity),
        ));
        for plan in proposal.providers().selection.plans() {
            for row in plan.rows.iter().filter(|row| {
                plan.identities.text(row.declared_requirement_identity) == abi.requirement_identity
            }) {
                binding_context.push_str(&format!(
                    "selected target={} root={:?} selected={:?} declared={:?} abi={:?}\n",
                    plan.target,
                    plan.schema.root_application_identity(),
                    plan.identities.text(row.requirement_identity),
                    plan.identities.text(row.declared_requirement_identity),
                    row.calling_signature,
                ));
            }
        }
    }
    for target in proposal.target_provider_proposals() {
        match &target.selection {
            Ok(selection) => {
                binding_context.push_str(&format!(
                    "publication target={} plans={}\n",
                    target.target,
                    selection.plans.len()
                ));
                for selected in &selection.plans {
                    for row in &selected.rows {
                        if row.calling_signature.is_some() {
                            binding_context.push_str(&format!(
                                "publication selected={:?} declared={:?} abi={:?}\n",
                                selected.identities.text(row.requirement_identity),
                                selected.identities.text(row.declared_requirement_identity),
                                row.calling_signature,
                            ));
                        }
                    }
                }
            }
            Err(errors) => binding_context.push_str(&format!(
                "publication target={} refused={errors:?}\n",
                target.target
            )),
        }
    }
    let context_path = scratch.join("native-binding-context.txt");
    std::fs::write(&context_path, &binding_context).unwrap();
    let source_selection = proposal.providers().selection.clone();
    // Std can retain unrelated direct imports. Keep these source-free records
    // so the final reached registrar can still be checked for an extra stub.
    let direct_boundary_leaves = proposal.providers().direct_boundary_leaves.clone();
    // Native completion now has only recovered semantic inputs, not a source
    // compiler that could supply a missing declaration or calling decision.
    drop(std::mem::take(prepared.session.compiler()));
    drop(std::mem::take(&mut prepared.completed));
    let compiler::CompilationOutput::Native(pending) = &mut output else {
        unreachable!()
    };
    pending.terminal = recovered;
    pending.accepted_package_permissions = recovered_permissions;
    let completed =
        orchestration::complete_product(output).map_err(|error| compiler::PackageCompileFailure {
            occurrence: root_occurrence,
            failure: error.into_diagnostics().into(),
        });
    let mut outcomes = prepared
        .completion
        .take()
        .expect("the original package admission must still close its outcome")
        .complete(vec![completed], prepared.session.resources());
    assert_eq!(outcomes.len(), 1);
    let outcome = outcomes.pop().unwrap();
    assert_eq!(outcome.target, profile);
    let report = outcome
        .result
        .unwrap_or_else(|error| {
            panic!(
                "complete recovered native work through ordinary package reconciliation: {error:?}\nretained native binding coordinates: {}\n{binding_context}",
                context_path.display()
            )
        })
        .into_report()
        .expect("complete actual native lowering after saved-input recovery");
    drop(prepared);
    // The emitted call must use the exact saved native row. Its checked scope
    // and completed image also show that this callback was actually reached.
    let artifact = report.retained_native_artifact().unwrap();
    let module = artifact.psi_artifact().semantic_module();
    let scope = artifact
        .object()
        .native_execution_scope(
            artifact.psi_artifact().view().identity(),
            *artifact.selected_provider_closure_digest().as_bytes(),
        )
        .expect("rejoin the actual image's artifact, target, entry and provider choices")
        .expect("the checked native image must retain its execution scope");
    assert!(
        !scope.callbacks().is_empty(),
        "the existing descriptor callback must actually be selected for execution"
    );
    for occurrence in scope.callbacks() {
        let callback = module
            .callback_application(occurrence.application_index)
            .expect("the executed callback must retain its exact application");
        assert_eq!(callback.registrar, occurrence.terminal_operation);
        assert!(scope.contains_machine(callback.registrar_machine));
        let operation = module
            .machine(callback.registrar_machine)
            .expect("retained callback registrar machine")
            .operations()
            .find(|operation| operation.id == callback.registrar)
            .expect("actual emitted registrar operation");
        let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = operation.kind else {
            panic!("registrar must retain its exact boundary call");
        };
        assert!(scope.contains_boundary(boundary));
        let declaration = module
            .boundary_machines
            .iter()
            .find(|declaration| declaration.id == boundary)
            .unwrap();
        let requirement = module.identities.text(declaration.identity).unwrap();
        let source = &declaration
            .declaration
            .as_ref()
            .expect("the executed registrar retains its checked declaration")
            .source;
        let semantic = module
            .identities
            .text(source.semantic_declaration)
            .expect("the registrar has its canonical semantic declaration");
        let callable = module
            .identities
            .text(source.callable)
            .expect("the registrar has its complete checked callable");
        assert!(
            direct_boundary_leaves.iter().all(|leaf| {
                !leaf
                    .plan
                    .rows
                    .iter()
                    .any(|row| leaf.plan.identities.text(row.requirement_identity) == requirement)
                    && !(leaf.declaration_identity == semantic
                        && leaf.callable_identity == callable)
            }),
            "the reached bare native registrar must not acquire a forwarding-machine leaf"
        );
        let (plan, row) = source_selection
            .selected_requirement_row(requirement)
            .expect("the reached closed registrar must retain its exact native binding");
        assert_ne!(row.requirement_identity, row.declared_requirement_identity);
        let selected = artifact
            .selected_provider_plans()
            .iter()
            .filter(|plan| {
                plan.requirement_identities()
                    .iter()
                    .any(|id| id == requirement)
            })
            .collect::<Vec<_>>();
        let [selected] = selected.as_slice() else {
            panic!(
                "executed registrar {requirement} must select exactly one completed plan; got {}",
                selected.len()
            );
        };
        assert_eq!(selected.report_identity(), plan.report_fingerprint());
        assert_eq!(
            selected.plan_digest().as_bytes(),
            plan.identity_digest().as_bytes()
        );
        let abi = module.calling_abi_identity(boundary).unwrap();
        assert_eq!(row.calling_signature, Some(abi.id()));
        assert_eq!(
            plan.identities.text(row.declared_requirement_identity),
            abi.requirement_identity
        );
        let matching = plan
            .schema
            .calling_applications
            .iter()
            .enumerate()
            .filter(|(_, application)| application.abi == abi)
            .collect::<Vec<_>>();
        let [(calling_index, original)] = matching.as_slice() else {
            panic!("the exact ABI must retain exactly one original checked boundary contract");
        };
        assert_eq!(plan.schema.calling_application(&abi), Some(&original.plan));
        assert_eq!(
            plan.schema.identity_text(original.requirement),
            abi.requirement_identity
        );
        assert!(!original.plan.commitment.is_zero());
        assert_ne!(original.plan.report_fingerprint, 0);
        let wrong = module
            .calling_abi_signatures
            .iter()
            .find(|declaration| {
                declaration.id != abi.id()
                    && module.identities.text(declaration.requirement_identity)
                        != Some(abi.requirement_identity.as_str())
            })
            .expect("another actual declaration ABI must provide a substitution falsifier")
            .identity(&module.identities)
            .unwrap();
        assert!(plan.schema.calling_application(&wrong).is_none());
        let mut changed = plan.schema.clone();
        // The original requirement coordinate must reject another real
        // declaration's ABI, independently of its supplied calling policy.
        changed.calling_applications[*calling_index].abi = wrong;
        assert_eq!(
            changed.validate_calling_applications().unwrap_err(),
            "calling evidence ABI key differs from its requirement coordinate"
        );
    }
    native_entry_controls::inspect(&report, &scratch)
        .expect("preserve the original native entry readback falsifiers");
    let published =
        products::publication::publish_compilation(report, &scratch.join("native-output"))
            .expect("publish the checked native executable");
    let observed = std::process::Command::new(published.checked_native_executable_path().unwrap())
        .env("DYLD_LIBRARY_PATH", &provider)
        .output()
        .expect("run the unchanged descriptor callback program");
    assert_eq!(
        observed.status.code(),
        Some(0),
        "native callback oracle: {observed:?}"
    );
    std::fs::remove_dir_all(scratch).unwrap();
}
