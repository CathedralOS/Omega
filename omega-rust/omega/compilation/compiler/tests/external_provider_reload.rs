//! Native persistence rejoins canonical external satisfaction without a saved
//! source-classified numeric execution row. Exercise a call and comparisons.

use super::{execute_build, fixture_package_inputs};
use compiler::{
    CheckedCompileRequest, CompilationSession, CompileOptions, CompileRequest,
    RequestedCompileProduct,
};
use std::sync::Arc;
use terminal_psi_to_abstract_operations::provider_admission;

#[test]
fn numeric_native_inputs_reload_and_reject_substituted_retained_meaning() {
    let recovered = reload_with_numeric_control(
        "tests/omega/pass/float/named_provider_negate_is_nan_exit/main.omg",
        true,
    );
    let proposal = recovered.native_realization_proposal().unwrap();
    assert!(
        proposal
            .providers()
            .intrinsic_executions
            .iter()
            .flatten()
            .all(|execution| execution.is_none_or(|execution| !execution.is_numeric()))
    );
    assert!(
        recovered
            .artifact()
            .semantic_module()
            .external_providers
            .iter()
            .any(|candidate| candidate.numeric_intrinsic.is_some())
    );
}

#[test]
fn checked_native_inputs_reload_from_canonical_inherited_instances() {
    let recovered =
        reload("tests/omega/pass/providers/canonical_inherited_provider_instances_exit/main.omg");
    let proposal = recovered.native_realization_proposal().unwrap();
    let rows = proposal
        .providers()
        .selection
        .specialized_requirement_rows();
    assert_eq!(rows.len(), 4, "three finite instances and inherited spare");
    assert_eq!(
        proposal.providers().checked_adapters.len(),
        5,
        "three echo instances, spare and choose"
    );
}

#[test]
fn unused_source_provider_demand_does_not_require_terminal_instances() {
    reload("tests/omega/pass/providers/uncalled_boundary_provider_exit/main.omg");
}

// A real checked proposal is the public source of scoped coverage. Raw rows
// cannot shorten it while preserving the same outer artifact and choices.
#[test]
fn checked_native_scope_rejects_same_context_provider_and_callback_omissions() {
    use terminal_psi_to_abstract_operations::{
        NativeExecutionSelection, VerifiedNativeArtifactInput,
    };
    let retained =
        reload("tests/omega/pass/providers/canonical_inherited_provider_instances_exit/main.omg");
    let artifact = retained.artifact();
    let proposal = retained.native_realization_proposal().unwrap();
    let root = artifact.semantic_module().entry.unwrap();
    let scope = proposal
        .callable_execution_scope(artifact, root, None)
        .unwrap();
    assert!(
        !scope.local_adapters().is_empty(),
        "real selected adapters required"
    );
    let failure = VerifiedNativeArtifactInput::prepare(
        artifact,
        proposal.target_profile(),
        root,
        &[],
        &NativeExecutionSelection {
            scope: Some(&scope),
            scalar_applications: None,
            providers: &[],
            plans: &proposal.providers().selection,
            callbacks: scope.callbacks(),
            entry_call: scope.entry_call(),
        },
    )
    .expect_err("the same context must not authorize an omitted selected adapter");
    assert!(
        failure.to_string().contains("checked proposal inputs"),
        "{failure}"
    );

    let retained =
        reload("tests/omega/pass/layouts/private_callback_inherited_policy_exit/main.omg");
    let artifact = retained.artifact();
    let proposal = retained.native_realization_proposal().unwrap();
    let root = artifact.semantic_module().entry.unwrap();
    let scope = proposal
        .callable_execution_scope(artifact, root, None)
        .unwrap();
    assert!(
        !scope.callbacks().is_empty(),
        "a real foreign callback is required"
    );
    let failure = VerifiedNativeArtifactInput::prepare(
        artifact,
        proposal.target_profile(),
        root,
        &[],
        &NativeExecutionSelection {
            scope: Some(&scope),
            scalar_applications: None,
            providers: scope.local_adapters(),
            plans: &proposal.providers().selection,
            callbacks: &[],
            entry_call: scope.entry_call(),
        },
    )
    .expect_err("the same context must not authorize an omitted callback roster");
    assert!(
        failure.to_string().contains("entry or callback inputs"),
        "{failure}"
    );
}

#[test]
fn checked_native_scope_rejects_other_handoff_contexts() {
    use terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput;
    let retained =
        reload("tests/omega/pass/providers/canonical_inherited_provider_instances_exit/main.omg");
    let artifact = retained.artifact();
    let proposal = retained.native_realization_proposal().unwrap();
    let work = terminal_psi_to_abstract_operations::prepare_native_callable_execution(
        proposal,
        artifact,
        artifact.semantic_module().entry.unwrap(),
    )
    .unwrap();
    let (input, execution) = work.lower(None).unwrap();
    let source = input.into_optimization_input();
    let scope = Arc::new(execution.scope().clone());
    let digest = *execution.selection().identity_digest().as_bytes();
    let attach = |mut receiving: VerifiedPsiOptimizationInput, selection| {
        receiving.retain_native_selections(
            source.selected_program_entry_call().cloned(),
            source.target_layout_selections().unwrap().clone(),
            source.scalar_applications().cloned(),
            Some(Arc::clone(&scope)),
            selection,
        )
    };
    attach(source.clone(), digest).unwrap();
    let mut other_entry = source.plan().clone();
    other_entry.entry = artifact
        .semantic_module()
        .machines
        .iter()
        .find(|machine| machine.id != other_entry.entry)
        .expect("real fixture has a second checked machine")
        .id;
    assert!(
        attach(
            VerifiedPsiOptimizationInput::lowered(
                other_entry,
                source.context().module().clone(),
                source.target_machine_selections().cloned()
            ),
            digest
        )
        .is_err()
    );
    let windows =
        terminal_installation::admit_target_machine_selections(artifact.view(), "windows_x86_64");
    assert!(
        attach(
            VerifiedPsiOptimizationInput::lowered(
                source.plan().clone(),
                source.context().module().clone(),
                Some(windows)
            ),
            digest
        )
        .is_err()
    );
    let mut other_choices = digest;
    other_choices[0] ^= 1;
    assert!(attach(source.clone(), other_choices).is_err());
    let other = reload("tests/omega/pass/providers/uncalled_boundary_provider_exit/main.omg");
    let other_proposal = other.native_realization_proposal().unwrap();
    let other_work = terminal_psi_to_abstract_operations::prepare_native_callable_execution(
        other_proposal,
        other.artifact(),
        other.artifact().semantic_module().entry.unwrap(),
    )
    .unwrap();
    let (other_input, _) = other_work.lower(None).unwrap();
    assert!(attach(other_input.into_optimization_input(), digest).is_err());
    let mut receiving = source;
    assert!(
        receiving
            .retain_selected_providers(
                Vec::new(),
                Vec::new(),
                terminal_psi::TerminalIdentityTable::from(Vec::new()),
                other_choices
            )
            .is_err()
    );
}

#[test]
fn checked_native_scope_derives_assumptions_and_preserves_the_semantic_entry() {
    use terminal_psi_to_abstract_operations::{
        NativeExecutionSelection, VerifiedNativeArtifactInput,
    };
    let retained = reload(
        "tests/omega/pass/providers/runtime_closed_provider_and_assumed_chain_exit/main.omg",
    );
    let artifact = retained.artifact();
    let proposal = retained.native_realization_proposal().unwrap();
    let scope = proposal
        .callable_execution_scope(artifact, artifact.semantic_module().entry.unwrap(), None)
        .unwrap();
    let coordinates = proposal
        .providers()
        .selection
        .assumed_binding_coordinates(artifact.semantic_module())
        .unwrap();
    assert_eq!(
        coordinates.len(),
        2,
        "the original two-link assumed chain must be present"
    );
    let mut retained_suppliers = 0;
    for (required, supplied) in coordinates {
        assert!(scope.contains_boundary(required));
        assert!(scope.contains_boundary(supplied));
        for candidate in artifact
            .semantic_module()
            .provider_candidates
            .iter()
            .filter(|candidate| candidate.boundary == supplied)
        {
            retained_suppliers += 1;
            assert!(
                scope.contains_machine(candidate.candidate),
                "the supplier body must remain in the scope"
            );
        }
    }

    assert!(
        retained_suppliers > 0,
        "the assumed chain must reach a real checked supplier body"
    );

    let retained = reload_at(
        "tests/omega/pass/build/uefi_selected_context_entry/main.omg",
        "uefi_x86_64",
        false,
    );
    let artifact = retained.artifact();
    let proposal = retained.native_realization_proposal().unwrap();
    let scope = proposal.execution_scope(artifact, &[]).unwrap();
    assert!(
        scope.entry_call().is_some(),
        "the fixture must actually cross its semantic ProgramEntry bridge"
    );
    let root = terminal_psi::TargetMachineCallable::Machine(scope.entry());
    let failure = VerifiedNativeArtifactInput::prepare(
        artifact,
        proposal.target_profile(),
        root,
        &[],
        &NativeExecutionSelection {
            scope: Some(&scope),
            scalar_applications: None,
            providers: scope.local_adapters(),
            plans: &proposal.providers().selection,
            callbacks: scope.callbacks(),
            entry_call: None,
        },
    )
    .expect_err("the same context must not drop its checked semantic entry");
    assert!(
        failure.to_string().contains("entry or callback inputs"),
        "{failure}"
    );
}

#[test]
fn checked_native_scope_binds_completed_direct_and_family_choices() {
    let retained = reload("tests/omega/pass/float/named_provider_negate_is_nan_exit/main.omg");
    let artifact = retained.artifact();
    let proposal = retained.native_realization_proposal().unwrap();
    let initial = proposal.providers().selection.clone();
    let remapped = proposal
        .providers()
        .direct_boundary_leaves
        .iter()
        .filter(|leaf| {
            leaf.plan.rows.iter().any(|row| {
                leaf.plan.identities.text(row.requirement_identity) != leaf.callable_identity
            })
        })
        .collect::<Vec<_>>();
    assert!(
        !remapped.is_empty(),
        "the existing fixture must actually retarget a direct callable to its target member"
    );
    for leaf in remapped {
        let [row] = leaf.plan.rows.as_slice() else {
            panic!("a direct leaf has exactly one row");
        };
        assert_eq!(row.requirement_identity, row.declared_requirement_identity);
        assert!(
            row.calling_signature.is_none(),
            "retargeting must preserve the synthetic direct role"
        );
        leaf.validate_bound_calling_custody(artifact.semantic_module())
            .unwrap();
    }
    let work = terminal_psi_to_abstract_operations::prepare_native_callable_execution(
        proposal,
        artifact,
        artifact.semantic_module().entry.unwrap(),
    )
    .unwrap();
    let (input, execution) = work.lower(None).unwrap();
    let scope = execution.scope();
    assert!(
        !scope.machines().is_empty(),
        "the checked scope must contain executable functions"
    );
    assert!(
        scope.machines().len() < artifact.semantic_module().machines.len(),
        "the real fixture must leave some canonical functions outside its native scope"
    );
    assert!(
        scope
            .machines()
            .iter()
            .all(|machine| artifact.semantic_module().machine(*machine).is_some()),
        "the checked scope must be a subset of the original canonical functions"
    );
    let mut lowered_machines = input
        .plan()
        .functions
        .iter()
        .map(|function| function.machine)
        .collect::<Vec<_>>();
    lowered_machines.sort_unstable();
    assert_eq!(
        lowered_machines,
        scope.machines(),
        "lowering must publish exactly the authenticated function scope"
    );
    let mut expected_boundaries = artifact
        .semantic_module()
        .boundary_machines
        .iter()
        .filter(|boundary| scope.contains_boundary(boundary.id))
        .map(|boundary| boundary.id)
        .collect::<Vec<_>>();
    expected_boundaries.sort_unstable();
    let mut lowered_boundaries = input
        .plan()
        .boundary_machines
        .iter()
        .map(|boundary| boundary.id)
        .collect::<Vec<_>>();
    lowered_boundaries.sort_unstable();
    assert_eq!(
        lowered_boundaries, expected_boundaries,
        "lowering must publish exactly the authenticated boundary scope"
    );
    assert_ne!(
        execution.selection().identity_digest(),
        initial.identity_digest(),
        "the fixture must actually add direct or target-family provider choices"
    );
    assert_eq!(execution.source_selection(), &initial);
    let plan = input.plan().clone();
    let facts = input.into_optimization_input().into_lowering_facts();
    facts
        .native_execution_scope_for(
            plan.psi,
            proposal.target_profile().native_target(),
            plan.entry,
            *execution.selection().identity_digest().as_bytes(),
        )
        .unwrap()
        .expect("completed selection retains the same checked function scope");
    assert!(
        facts
            .native_execution_scope_for(
                plan.psi,
                proposal.target_profile().native_target(),
                plan.entry,
                *initial.identity_digest().as_bytes(),
            )
            .is_err(),
        "the preliminary choice must not masquerade as the completed native selection"
    );
    // This source has a real foreign satisfying machine, so it exercises the
    // retained direct contract rather than inventing a leaf for a Build row.
    let retained = reload("tests/omega/pass/boundary/named_target_foreign_provider_exit/main.omg");
    let module = retained.artifact().semantic_module();
    let foreign = retained
        .native_realization_proposal()
        .unwrap()
        .providers()
        .direct_boundary_leaves
        .iter()
        .filter(|leaf| {
            matches!(
                leaf.execution,
                backend_plan::DirectBoundaryExecution::Foreign(Ok(_))
            )
        })
        .collect::<Vec<_>>();
    assert!(
        !foreign.is_empty(),
        "the authored foreign machine must retain a real direct leaf"
    );
    for leaf in foreign {
        leaf.validate_bound_calling_custody(module)
            .expect("original foreign contract rejoins its module");
        let backend_plan::DirectBoundaryExecution::Foreign(Ok(binding)) = &leaf.execution else {
            unreachable!()
        };
        assert!(!binding.calling_applications.is_empty());
        for (calling_index, calling) in binding.calling_applications.iter().enumerate() {
            let original = leaf
                .satisfied_schema
                .calling_application(&calling.abi)
                .expect("actual foreign ABI retains its original checked boundary contract");
            assert_eq!(calling.commitment, original.commitment.as_bytes());
            assert_eq!(calling.report_fingerprint, original.report_fingerprint);
            let wrong = module
                .calling_abi_signatures
                .iter()
                .find(|declaration| {
                    declaration.id != calling.abi.id()
                        && module.identities.text(declaration.requirement_identity)
                            != Some(leaf.satisfied_requirement_identity.as_str())
                })
                .expect("another real declaration ABI supplies the substitution control")
                .identity(&module.identities)
                .unwrap();
            let mut changed = leaf.clone();
            let backend_plan::DirectBoundaryExecution::Foreign(Ok(binding)) =
                &mut changed.execution
            else {
                unreachable!()
            };
            binding.calling_applications[calling_index].requirement_identity =
                wrong.requirement_identity.clone();
            binding.calling_applications[calling_index].abi = wrong;
            binding.calling_applications[calling_index].commitment = binding.calling_applications
                [calling_index]
                .recomputed_commitment()
                .unwrap();
            assert_eq!(
                changed.validate_bound_calling_custody(module).unwrap_err(),
                "direct foreign execution changed its public declaration ABI"
            );
        }
    }
}

#[test]
fn native_target_catalogs_reload_without_losing_later_native_choices() {
    use effects::provider_plan::ProviderBinding;
    let retained =
        reload("tests/omega/pass/build/native_row_outranks_scoped_implementations_exit/main.omg");
    let proposal = retained.native_realization_proposal().unwrap();
    assert_eq!(proposal.target_profile(), target::TargetProfile::MacosArm64);
    let mut requirement = None;
    for target in [
        target::TargetProfile::MacosArm64,
        target::TargetProfile::LinuxArm64,
        target::TargetProfile::LinuxX64,
        target::TargetProfile::WindowsX64,
    ] {
        let publication = proposal
            .target_provider_proposals()
            .iter()
            .find(|row| row.target == target.target_name())
            .expect("each authored target keeps its catalog");
        assert_eq!(
            publication.terminal_psi,
            retained.artifact().view().identity()
        );
        let selected = publication
            .selection
            .as_ref()
            .unwrap_or_else(|errors| panic!("{}: {}", target.target_name(), errors.join("; ")));
        let distance = selected
            .plans
            .iter()
            .filter(|row| {
                row.plan
                    .schema
                    .identity_text(row.plan.schema.declaration.trait_name)
                    == "Distance"
            })
            .collect::<Vec<_>>();
        let [distance] = distance.as_slice() else {
            panic!("each target must select exactly one Distance plan");
        };
        assert_eq!(distance.plan.target, target.target_name());
        let [row] = distance.plan.rows.as_slice() else {
            panic!("the original one-method boundary stays whole");
        };
        let identity = distance.plan.identities.text(row.requirement_identity);
        if let Some(original) = &requirement {
            assert_eq!(original, identity);
        } else {
            requirement = Some(identity.to_owned());
        }
        match target {
            target::TargetProfile::MacosArm64 | target::TargetProfile::LinuxArm64 => {
                assert!(matches!(
                    row.binding,
                    ProviderBinding::CheckedAdapter { .. }
                ));
            }
            target::TargetProfile::LinuxX64 | target::TargetProfile::WindowsX64 => {
                let ProviderBinding::Import { evaluated } = &row.binding else {
                    panic!(
                        "a later target must retain its executed native binding, not another target's adapter"
                    );
                };
                assert_eq!(evaluated.locator().target(), target);
                let expected = match target {
                    target::TargetProfile::LinuxX64 => {
                        target::ForeignLocatorCandidate::ElfVersioned {
                            object: b"libc.so.6".to_vec(),
                            symbol: b"abs".to_vec(),
                            version: b"GLIBC_2.2.5".to_vec(),
                        }
                    }
                    _ => target::ForeignLocatorCandidate::PeByName {
                        library: b"msvcrt.dll".to_vec(),
                        export: b"abs".to_vec(),
                    },
                };
                assert_eq!(evaluated.locator().locator(), &expected);
            }
            _ => unreachable!(),
        }
    }
}

fn reload(fixture: &str) -> terminal_psi_to_abstract_operations::RetainedTerminalArtifact {
    reload_with_numeric_control(fixture, false)
}

fn reload_with_numeric_control(
    fixture: &str,
    inspect_numeric_substitution: bool,
) -> terminal_psi_to_abstract_operations::RetainedTerminalArtifact {
    reload_at(fixture, "macos_arm64", inspect_numeric_substitution)
}

fn reload_at(
    fixture: &str,
    target: &str,
    inspect_numeric_substitution: bool,
) -> terminal_psi_to_abstract_operations::RetainedTerminalArtifact {
    let fixture = fixture_package_inputs::repo_root().join(fixture);
    let prepared =
        fixture_package_inputs::reviewed_repository_fixture_package_inputs(&fixture, Some(target))
            .expect("review numeric fixture inputs");
    let mut session = CompilationSession::default();
    let build = execute_build(
        &mut session,
        CheckedCompileRequest {
            package_inputs: prepared.package_inputs.clone(),
            ..CheckedCompileRequest::new(&prepared.root_path, Some(target))
        },
    );
    let checked = session
        .check_package_build(build)
        .expect("checked numeric source");
    let mut request = CompileRequest::new(CompileOptions {
        root_path: prepared.root_path,
        build_dir: None,
        target_name: Some(target.into()),
    })
    .with_requested_product(RequestedCompileProduct::TerminalArtifact);
    if let Some(inputs) = prepared.package_inputs {
        request = request.with_package_inputs(inputs);
    }
    let retained = orchestration::compile_sources(&mut session, request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect("numeric Terminal production")
        .into_retained_terminal_artifact()
        .expect("retained numeric proposal");
    let proposal = retained
        .native_realization_proposal()
        .expect("native proposal");
    let selections = optimization_core::OptimizationRollback::default()
        .settle(&checked.build().config.settings.optimizations);
    let request = checked
        .terminal_native_proposal_request(selections.effective(), &[])
        .expect("source-owned derivation inputs");
    // Ordinary selection never reconstructs checked instance rows. Native
    // preparation and persistence must consume canonical associations directly.
    let selection = request
        .boundaries
        .selected_provider_plans
        .clone()
        .without_checked_specializations();
    assert_eq!(request.boundaries.selected_provider_plans, &selection);
    let permissions = provider_admission::terminal_authority_permission_policy_with_rows(
        proposal.package_terminal_authority_permissions().to_vec(),
        proposal
            .package_terminal_authority_permission_identities()
            .clone(),
    )
    .expect("package permission policy");
    if inspect_numeric_substitution {
        let numeric_request = checked
            .terminal_native_proposal_request(selections.effective(), &[])
            .expect("source-owned numeric derivation inputs");
        reject_substituted_numeric_meaning(&retained, numeric_request, &permissions);
    }
    let bytes = native_compiler::retain_native_derivation_inputs(
        proposal.program_entry().checked_entry(),
        proposal.program_entry().physical_adapter().is_some(),
        proposal.target_provider_proposals(),
        &request,
        &permissions,
        &[],
    )
    .expect("persist native derivation inputs");
    let artifact = Arc::new(
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&retained.artifact().to_bytes())
            .expect("reload canonical Terminal"),
    );
    drop(request);
    drop(selection);
    drop(checked);
    drop(session);
    let (recovered, recovered_permissions, installation_assumptions) =
        terminal_psi_to_abstract_operations::recover_native_derivation_inputs(artifact, &bytes)
            .expect("recover native inputs without source");
    assert!(installation_assumptions.is_empty());
    assert_eq!(recovered_permissions, permissions);
    assert_eq!(recovered, retained);
    let proposal = recovered.native_realization_proposal().unwrap();
    let work = terminal_psi_to_abstract_operations::prepare_native_callable_execution(
        proposal,
        recovered.artifact(),
        recovered
            .artifact()
            .semantic_module()
            .entry
            .expect("fixture default entry"),
    )
    .unwrap_or_else(|diagnostics| panic!("{fixture:?}: native preparation: {diagnostics:?}"));
    // Array lengths and layout conditions need the same checked scalar completion
    // after reload as ordinary native compilation, before lowering may use them.
    let mut accounting = build_time_evaluation::SourceCompletionAccounting::default();
    let completed = build_time_evaluation::complete_scalar_applications(
        recovered.shared_artifact(),
        target,
        None,
        work.scalar_application_demand(),
        None,
        &mut accounting,
        |scope| {
            let provider = proposal
                .target_provider_proposals()
                .iter()
                .find(|provider| provider.target == scope)
                .ok_or_else(|| format!("missing {scope} provider proposal"))?;
            let providers = provider
                .selection
                .as_ref()
                .map_err(|reasons| reasons.join("; "))?
                .admit(recovered.artifact().view(), scope)?;
            let profile = target::TargetProfile::from_canonical_target_name(scope)
                .map_err(|diagnostic| diagnostic.message)?;
            Ok(build_time_evaluation::ScalarScopeEvaluation {
                application_roots: None,
                import_realizations: None,
                layout_target: profile
                    .layout_target_facts()
                    .ok_or("scalar evaluation target has no immutable layout catalogue")?,
                carrier_layout: profile
                    .native_realization()
                    .map(runtime_abi::target_carrier_layout),
                targets: terminal_installation::admit_target_machine_selections(
                    recovered.artifact().view(),
                    scope,
                ),
                providers,
                address_model: Ok(target::TargetSemantics::close_address_model(profile).map(
                    |model| terminal_installation::ScalarAddressModel {
                        target: scope.to_owned(),
                        carrier: model.carrier(),
                        semantics_version: model.semantics_version(),
                        commitment: model.commitment(),
                    },
                )),
            })
        },
    )
    .unwrap_or_else(|reason| panic!("{fixture:?}: scalar completion: {reason}"));
    work.lower(Some(
        terminal_psi_to_abstract_operations::NativeScalarApplications {
            completed: &completed,
            provider_declarations: &proposal.providers().declarations,
        },
    ))
    .unwrap_or_else(|diagnostics| panic!("{fixture:?}: reloaded native lowering: {diagnostics:?}"));
    recovered
}

// The same checked request must reject a replaced numeric meaning both in
// memory and after serialization. Transport must not turn bad supplied custody
// into None before the native selection owner can reconcile it.
fn reject_substituted_numeric_meaning(
    retained: &terminal_psi_to_abstract_operations::RetainedTerminalArtifact,
    request: terminal_psi_to_abstract_operations::TerminalNativeProposalRequest<'_>,
    permissions: &provider_admission::TerminalAuthorityPermissionPolicy,
) {
    use effects::CompilerIntrinsicExecutionIdentity as Execution;
    use numerics::literals::FloatFormat;

    let module = retained.artifact().semantic_module();
    let mut selected_numeric = None;
    'plans: for (plan_index, provenance) in request
        .boundaries
        .selected_provider_provenance
        .iter()
        .enumerate()
    {
        for (row_index, row) in provenance.plan.rows.iter().enumerate() {
            if !matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
            ) {
                continue;
            }
            let Some(demand) = module.boundary_application_demands.iter().find(|demand| {
                demand.requirement().overload()
                    == provenance.plan.identities.text(row.requirement_identity)
                    && demand.requirement().package()
                        == provenance.plan.schema.declaration.trait_package_identity
            }) else {
                continue;
            };
            let execution = provider_selection::selected_external_execution(
                &module.external_providers,
                &module.identities,
                demand.requirement().declaration(),
                provenance.plan,
                row,
                provenance.row_executions[row_index],
                provenance.row_numeric_realizations[row_index].as_deref(),
            )
            .expect("actual selected numeric declaration");
            if let Some(execution) = execution.filter(|execution| execution.is_numeric()) {
                selected_numeric = Some((plan_index, row_index, execution));
                break 'plans;
            }
        }
    }
    let (plan_index, row_index, execution) =
        selected_numeric.expect("fixture has a reached numeric row");
    let wrong = if execution == Execution::NamedFloatNegation(FloatFormat::F32) {
        Execution::NamedFloatNegation(FloatFormat::F64)
    } else {
        Execution::NamedFloatNegation(FloatFormat::F32)
    };
    let mut executions = request
        .boundaries
        .selected_provider_provenance
        .iter()
        .map(|row| row.row_executions.to_vec())
        .collect::<Vec<_>>();
    executions[plan_index][row_index] = Some(wrong);
    let request = terminal_psi_to_abstract_operations::TerminalNativeProposalRequest {
        boundaries: terminal_psi_to_abstract_operations::BoundaryApplicationSettlement {
            selected_provider_plans: request.boundaries.selected_provider_plans,
            selected_provider_provenance: request
                .boundaries
                .selected_provider_provenance
                .iter()
                .zip(&executions)
                .map(
                    |(provenance, rows)| provider_admission::CompilerIntrinsicPlanProvenance {
                        plan: provenance.plan,
                        semantic_schema: provenance.semantic_schema,
                        row_executions: rows,
                        row_numeric_realizations: provenance.row_numeric_realizations,
                    },
                )
                .collect(),
            calling_applications: request.boundaries.calling_applications,
        },
        ..request
    };
    let proposal = retained.native_realization_proposal().unwrap();
    let bytes = native_compiler::retain_native_derivation_inputs(
        proposal.program_entry().checked_entry(),
        proposal.program_entry().physical_adapter().is_some(),
        proposal.target_provider_proposals(),
        &request,
        permissions,
        &[],
    )
    .expect("transport supplied numeric custody without erasing it");
    let artifact = Arc::new(
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&retained.artifact().to_bytes())
            .expect("source-free canonical artifact"),
    );
    let direct = terminal_psi_to_abstract_operations::project_terminal_native_realization_proposal(
        &artifact,
        proposal.program_entry().checked_entry().clone(),
        request,
    )
    .expect("projection retains numeric custody until execution selection");
    let (recovered, _, _) = terminal_psi_to_abstract_operations::recover_native_derivation_inputs(
        Arc::clone(&artifact),
        &bytes,
    )
    .expect("recovery retains numeric custody until execution selection");
    for proposal in [&direct, recovered.native_realization_proposal().unwrap()] {
        assert_eq!(
            proposal.providers().intrinsic_executions[plan_index][row_index],
            Some(wrong)
        );
        let failure = match terminal_psi_to_abstract_operations::prepare_native_callable_execution(
            proposal,
            &artifact,
            artifact
                .semantic_module()
                .entry
                .expect("fixture default entry"),
        )
        .and_then(|work| work.lower(None))
        {
            Ok(_) => panic!("substituted numeric meaning authorized native preparation"),
            Err(errors) => format!("{errors:?}"),
        };
        assert!(
            failure.contains("numeric meaning disagrees with selected provider custody"),
            "{failure}"
        );
    }
}
