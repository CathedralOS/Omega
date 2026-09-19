use super::{
    checked_adapter_identity, replay_parts, unit_effect_plan_named, unit_effect_plan_named_mut,
};
use super::{fixture_roster, hosted_exit};
use crate::providers_float_and_console::native;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, Command, CompileRequest, CompilerOptions,
    RequestedCompileProduct, compile, compile_reviewed_repository_fixture,
    compile_rooted_backend_canary_without_output_for_target,
    compile_rooted_backend_canary_without_output_for_target_and_permission_policy,
    compile_rooted_canary_for_native_host, compile_rooted_canary_for_target, fs,
    hosted_main_program_entry_build_for, interpret, pass_canary,
    reviewed_repository_fixture_package_inputs,
};
use compiler::CheckedCompileRequest;

#[test]
fn fused_service_parameter_moves_through_one_exact_internal_hop() {
    let canary = pass_canary(fixture_roster::SERVICE_FUSED_ERASURE_COMPILE);
    let baseline = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("one-hop fused Service fixture should reach checked trees");
    let provenance = baseline.selected_provider_provenance().to_vec();
    selected_dispatch::validate_fused_service_terminal_custody(&baseline, &provenance)
        .expect("one exact whole-root Service hop should retain final Fused custody");
    for fenced in ["forwarding_twice", "forwarding_scalar_once"] {
        let symbol = baseline
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == fenced)
            .unwrap_or_else(|| panic!("fixture should retain `{fenced}` source"))
            .symbol;
        assert!(
            baseline
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(symbol)
                .is_none(),
            "`{fenced}` must remain outside the bounded checked forwarding rung"
        );
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&baseline, fenced).is_err(),
            "`{fenced}` must remain outside Terminal rather than dropping custody"
        );
    }

    let caller = unit_effect_plan_named(&baseline, "forwarding_once");
    let target = unit_effect_plan_named(&baseline, "forwarding_terminal");
    let [caller_parameter] = caller.structural_parameters.as_slice() else {
        panic!("forwarding caller should retain one structural parameter")
    };
    let [target_parameter] = target.structural_parameters.as_slice() else {
        panic!("forwarding target should retain one structural parameter")
    };
    let caller_receipt = caller_parameter
        .fused_service_erasure
        .as_ref()
        .expect("forwarding caller should retain exact Service custody");
    let target_receipt = target_parameter
        .fused_service_erasure
        .as_ref()
        .expect("forwarding target should retain exact Service custody");
    assert_eq!(caller.attachment_type_identity, None);
    assert_eq!(target.attachment_type_identity, None);
    assert!(caller.scalar_parameters.is_empty());
    assert!(target.scalar_parameters.is_empty());
    assert_eq!(
        (
            &caller_receipt.carrier_type_identity,
            caller_receipt.requirement,
            caller_receipt.provider_plan_digest,
        ),
        (
            &target_receipt.carrier_type_identity,
            target_receipt.requirement,
            target_receipt.provider_plan_digest,
        ),
    );
    let [
        checked_trees::CheckedUnitEffectOperationPlan::CallUnit {
            target_machine,
            target_state,
            structural_arguments,
            claim_transfers,
            ..
        },
        checked_trees::CheckedUnitEffectOperationPlan::Complete { .. },
    ] = caller.operations.as_slice()
    else {
        panic!("forwarding caller should retain one internal call and return")
    };
    assert_eq!(
        (*target_machine, *target_state),
        (target.machine, target.state)
    );
    let [argument] = structural_arguments.as_slice() else {
        panic!("forwarding caller should retain one structural argument")
    };
    assert_eq!(argument.source_parameter_index(), Some(0));
    assert!(argument.path.is_empty());
    assert_eq!(
        argument.access,
        checked_trees::CheckedStructuralAccess::Owned
    );
    assert!(claim_transfers.is_empty());

    let lowered = checked_trees_to_lowered_psi::lower_machine(&baseline, "forwarding_once")
        .expect("one exact whole-root Service hop should lower to Terminal Psi");
    assert!(
        lowered.semantic_module.machines.len() >= 2,
        "Terminal closure should retain the forwarding caller and helper"
    );
    let terminal_caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("forwarding caller should remain the Terminal entry");
    assert_eq!(terminal_caller.attachment, None);
    let terminal_target = terminal_caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::CallUnit { callee, .. } => Some(*callee),
            _ => None,
        })
        .expect("forwarding caller should retain one Terminal internal call");
    let terminal_target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == terminal_target)
        .expect("forwarding target should remain in the Terminal closure");
    assert_eq!(terminal_target.attachment, None);
    assert!(terminal_target.blocks.iter().any(|block| {
        block.operations.iter().any(|operation| {
            matches!(
                &operation.kind,
                terminal_psi::OperationKind::BoundaryCall { .. }
            )
        })
    }));

    let assert_rejects = |mutated: &checked_trees::CheckedTrees, label: &str| {
        let Err(diagnostics) =
            selected_dispatch::validate_fused_service_terminal_custody(mutated, &provenance)
        else {
            panic!("{label} should reject final Fused custody")
        };
        assert!(!diagnostics.is_empty());
        let Err(_) = checked_trees_to_lowered_psi::lower_machine(mutated, "forwarding_once") else {
            panic!("{label} should reject raw Terminal lowering")
        };
    };

    let mut wrong_source = baseline.clone().into_program();
    let checked_trees::CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &mut unit_effect_plan_named_mut(&mut wrong_source, "forwarding_once").operations[0]
    else {
        unreachable!()
    };
    structural_arguments[0].source =
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 };
    assert_rejects(&wrong_source, "a substituted forwarding source index");

    let mut projected = baseline.clone().into_program();
    let checked_trees::CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &mut unit_effect_plan_named_mut(&mut projected, "forwarding_once").operations[0]
    else {
        unreachable!()
    };
    structural_arguments[0]
        .path
        .push(checked_trees::CheckedUnitStructuralPathSegment::Field(
            "forged".to_owned(),
        ));
    assert_rejects(&projected, "a projected forwarding carrier");

    let mut digest_drift = baseline.clone().into_program();
    unit_effect_plan_named_mut(&mut digest_drift, "forwarding_terminal").structural_parameters
        [0]
    .fused_service_erasure
    .as_mut()
    .expect("target receipt")
    .provider_plan_digest[0] ^= 1;
    assert_rejects(&digest_drift, "a target selected-plan substitution");

    let mut duplicate = baseline.clone().into_program();
    let forwarding_call =
        unit_effect_plan_named(&duplicate, "forwarding_once").operations[0].clone();
    unit_effect_plan_named_mut(&mut duplicate, "forwarding_once")
        .operations
        .insert(1, forwarding_call);
    assert_rejects(&duplicate, "a second forwarding edge");
}

#[test]
fn provider_type_target_default_canary_selects_target_default() {
    let canary = pass_canary(fixture_roster::PROVIDER_TYPE_TARGET_DEFAULT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("target provider default should resolve the Pick slot");
    assert_eq!(
        checked.selected_program_entry_machine(),
        None,
        "targetless checking must not select an authored target entry"
    );
    let pick_plan = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "Pick")
        .expect("Pick must retain its selected target-default provider plan");
    assert_eq!(pick_plan.provider_type, "SecondProvider");
}

#[test]
fn component_owner_provider_override_canary_selects_complete_pick_plan() {
    let canary = pass_canary(fixture_roster::COMPONENT_OWNER_PROVIDER_OVERRIDE_COMPILE);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("component-owned build override should resolve the Pick slot");
    assert_eq!(
        checked.selected_program_entry_machine(),
        None,
        "targetless checking must not select an authored target entry"
    );
    let pick_plan = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "Pick")
        .expect("Pick must retain its selected component-owned provider plan");
    assert_eq!(pick_plan.provider_type, "SecondProvider");
    assert!(pick_plan.covers_schema());
    assert_eq!(pick_plan.rows.len(), 1);
    assert_eq!(pick_plan.rows[0].method, "choose");
    let expected = checked_adapter_identity(&checked, "SecondProvider::choose");
    assert!(matches!(
        &pick_plan.rows[0].binding,
        effects::provider_plan::ProviderBinding::CheckedAdapter { machine_identity, .. }
            if machine_identity == &expected
    ));
}

#[test]
fn test_owner_provider_override_canary_selects_complete_pick_plan() {
    let canary = pass_canary(fixture_roster::TEST_OWNER_PROVIDER_OVERRIDE_COMPILE);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("test-owned build override should resolve the Pick slot");
    assert_eq!(
        checked.selected_program_entry_machine(),
        None,
        "targetless checking must not select an authored target entry"
    );
    let pick_plan = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "Pick")
        .expect("Pick must retain its selected test-owned provider plan");
    assert_eq!(pick_plan.provider_type, "TestProvider");
    assert!(pick_plan.covers_schema());
    assert_eq!(pick_plan.rows.len(), 1);
    assert_eq!(pick_plan.rows[0].method, "choose");
    let expected = checked_adapter_identity(&checked, "TestProvider::choose");
    assert!(matches!(
        &pick_plan.rows[0].binding,
        effects::provider_plan::ProviderBinding::CheckedAdapter { machine_identity, .. }
            if machine_identity == &expected
    ));
}

#[test]
fn provider_type_target_default_override_canary_selects_build_override() {
    let canary = pass_canary(fixture_roster::PROVIDER_TYPE_TARGET_DEFAULT_OVERRIDE);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("build provider override should resolve the Pick slot");
    assert_eq!(
        checked.selected_program_entry_machine(),
        None,
        "targetless checking must not select an authored target entry"
    );
    let pick_plan = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "Pick")
        .expect("Pick must retain its selected build-override provider plan");
    assert_eq!(pick_plan.provider_type, "SecondProvider");
}

#[test]
fn adapter_satisfies_canary_selects_exact_checked_adapter_plan() {
    let canary = pass_canary(fixture_roster::ADAPTER_SATISFIES_COMPILE);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("checked adapter provider should resolve the Echo slot");
    assert_eq!(
        checked.selected_program_entry_machine(),
        None,
        "targetless checking must not select an authored target entry"
    );
    let echo_plan = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "Echo")
        .expect("Echo must retain its selected checked-adapter provider plan");
    assert_eq!(echo_plan.provider_type, "EchoProvider");
    assert!(echo_plan.covers_schema());
    assert_eq!(echo_plan.rows.len(), 1);
    let expected = checked_adapter_identity(&checked, "EchoProvider::echo_adapter");
    assert!(matches!(
        &echo_plan.rows[0].binding,
        effects::provider_plan::ProviderBinding::CheckedAdapter { machine_identity, .. }
            if machine_identity == &expected
    ));
}

#[test]
fn external_leaf_via_canary_selects_exact_free_import_plan() {
    // The leaf is one evaluated typed PE locator produced by `halt_binding`;
    // ordinary external `via` evaluation requires the canary's own selected
    // Windows target.
    let canary = pass_canary(fixture_roster::EXTERNAL_LEAF_VIA_COMPILE);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        Some("windows_x86_64"),
    ))
    .expect("evaluated external leaf should resolve the Shutdown slot for its Windows target");
    assert_eq!(
        checked.selected_program_entry_machine(),
        Some("Main::main"),
        "the reviewed Windows fixture selects the authored main entry"
    );
    let shutdown_plan = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "Shutdown")
        .expect("Shutdown must retain its selected free external-leaf plan");
    assert_eq!(shutdown_plan.provider_type, "");
    assert!(shutdown_plan.covers_schema());
    assert_eq!(shutdown_plan.rows.len(), 1);
    assert_eq!(shutdown_plan.rows[0].method, "halt");
    let effects::provider_plan::ProviderBinding::Import { evaluated } =
        &shutdown_plan.rows[0].binding
    else {
        panic!("Shutdown must retain one evaluated import binding, never a string-backed row");
    };
    assert_eq!(
        evaluated.locator().target(),
        target::TargetProfile::WindowsX64
    );
    assert_eq!(
        evaluated.locator().locator(),
        &target::ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"ExitProcess".to_vec(),
        }
    );
}

#[test]
fn external_leaf_dllimport_canary_selects_exact_free_import_plan() {
    // The leaf is one evaluated typed Mach-O locator produced by
    // `leaf_binding`; ordinary external `via` evaluation requires the
    // canary's own selected Darwin target.
    let canary = pass_canary(fixture_roster::EXTERNAL_LEAF_DLLIMPORT_COMPILE);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        Some("macos_arm64"),
    ))
    .expect("evaluated DllImport leaf should resolve the Leaf slot for its Darwin target");
    assert_eq!(
        checked.selected_program_entry_machine(),
        Some("Main::main"),
        "the reviewed Darwin fixture selects the authored main entry"
    );
    let leaf_plan = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "Leaf")
        .expect("Leaf must retain its selected free DllImport plan");
    assert_eq!(leaf_plan.provider_type, "");
    assert!(leaf_plan.covers_schema());
    assert_eq!(leaf_plan.rows.len(), 1);
    assert_eq!(leaf_plan.rows[0].method, "exit");
    let effects::provider_plan::ProviderBinding::Import { evaluated } = &leaf_plan.rows[0].binding
    else {
        panic!("Leaf must retain one evaluated import binding, never a string-backed row");
    };
    assert_eq!(
        evaluated.locator().target(),
        target::TargetProfile::MacosArm64
    );
    assert_eq!(
        evaluated.locator().locator(),
        &target::ForeignLocatorCandidate::MachODylibSymbol {
            install_name: b"libSystem.B.dylib".to_vec(),
            symbol: b"_exit".to_vec(),
        }
    );
}

#[test]
fn runtime_adapter_forwarding_exit_canary_runs() {
    // The selected concrete provider implements both friendly adapters over
    // its own byte leaf. Field-backed bounded carriers and literal-backed text
    // both cross the borrowed byte-view path; no extra service receiver is
    // injected into the provider implementation.
    let canary = pass_canary(fixture_roster::RUNTIME_ADAPTER_FORWARDING_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("forwarding-adapter canary should compile to checked trees");
    assert_eq!(
        checked.selected_program_entry_machine(),
        None,
        "targetless checking must not select an authored target entry"
    );
    let console_plan = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "Console")
        .expect("std Console must retain a selected nominal provider plan");
    assert_eq!(console_plan.provider_type, "ConsoleNativeProvider");
    assert_eq!(console_plan.rows.len(), 6);
    assert!(console_plan.covers_schema());
    for method in ["write", "write_line", "read_line"] {
        let expected =
            checked_adapter_identity(&checked, &format!("ConsoleNativeProvider::{method}"));
        assert!(console_plan.rows.iter().any(|row| {
            row.method == method
                && matches!(
                    &row.binding,
                    effects::provider_plan::ProviderBinding::CheckedAdapter { machine_identity, .. }
                        if machine_identity == &expected
                )
        }));
    }
    for method in ["read_byte", "write_byte", "exit_process"] {
        assert!(console_plan.rows.iter().any(|row| {
            row.method == method
                && matches!(
                    &row.binding,
                    effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine: name }
                        if name.contains(&format!("::{method}"))
                )
        }));
    }
    let main = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("canary main machine");
    let entry = checked
        .machine_states(main)
        .first()
        .expect("canary main entry state");
    let call_targets = checked
        .statement_table
        .statements(entry.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            checked_trees::statement::StatementNode::Call(call) => Some(call.target.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        call_targets.contains(&"ConsoleNativeProvider::write")
            && call_targets.contains(&"ConsoleNativeProvider::write_line"),
        "std Console composite calls must rewrite to the selected nominal adapters: {call_targets:?}"
    );
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.error, None,
        "forwarding adapter should interpret cleanly"
    );
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter routes through the adapter"
    );
    assert_eq!(
        outcome.stdout,
        b"Field\nLiteral\n\x80\xff".to_vec(),
        "interpreter stdout must come from the std write adapter"
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-adapter-forwarding-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("forwarding-adapter canary should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("forwarding-adapter canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("forwarding-adapter canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "native routes through the adapter"
    );
    assert_eq!(
        output.stdout,
        b"Field\nLiteral\n\x80\xff".to_vec(),
        "native stdout must come from the std write adapter"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn hosted_console_compiler_intrinsic_review_identities_are_exact() {
    use provider_planning::CompilerIntrinsicExecutionIdentity;

    let canary = pass_canary(fixture_roster::RUNTIME_ADAPTER_FORWARDING_EXIT);
    let main_path = canary.join("main.omg");
    for target in ["linux_x86_64", "linux_arm64", "macos_arm64"] {
        let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
            &main_path,
            Some(target),
        ))
        .expect("Linux Console provider should compile to checked trees");
        let (plan, retained) = checked
            .selected_provider_plans()
            .plans()
            .iter()
            .zip(checked.selected_provider_provenance())
            .find(|(plan, _)| plan.schema.trait_name == "Console")
            .expect("std Console must retain one selected provider plan");
        let exit = plan
            .rows
            .iter()
            .position(|row| row.method == "exit_process")
            .expect("Console plan must retain exit_process");
        assert_eq!(
            retained.row_compiler_intrinsic_executions[exit],
            Some(CompilerIntrinsicExecutionIdentity::HostedExitProcessI32),
        );
        let write_byte = plan
            .rows
            .iter()
            .position(|row| row.method == "write_byte")
            .expect("Console plan must retain write_byte");
        assert_eq!(
            retained.row_compiler_intrinsic_executions[write_byte],
            Some(CompilerIntrinsicExecutionIdentity::HostedWriteByteI32),
        );
        let derive = |requirement, realization, target| {
            selected_dispatch::derive_selected_compiler_intrinsic_execution_identity_for_row(
                &checked,
                plan,
                retained.provider.schema,
                &plan.rows[exit],
                requirement,
                realization,
                target,
            )
            .expect("exact compiler-intrinsic catalog derivation")
        };
        let read_byte = plan
            .rows
            .iter()
            .position(|row| row.method == "read_byte")
            .expect("Console plan must retain read_byte");
        assert_eq!(
            retained.row_compiler_intrinsic_executions[read_byte],
            Some(CompilerIntrinsicExecutionIdentity::HostedReadByte),
        );
        assert_eq!(
            derive(
                retained.provider.row_requirements[exit],
                retained.provider.row_realizations[exit],
                Some("linux_x86_64"),
            ),
            Some(selected_dispatch::SelectedCompilerIntrinsicExecutionIdentity::Unsupported),
            "package-owned std source requires the accepted package binding that produced retained closed custody",
        );
        for unsupported in [
            derive(
                retained.provider.row_requirements[read_byte],
                retained.provider.row_realizations[exit],
                Some("linux_x86_64"),
            ),
            derive(
                retained.provider.row_requirements[exit],
                retained.provider.row_realizations[read_byte],
                Some("linux_x86_64"),
            ),
            derive(
                retained.provider.row_requirements[exit],
                retained.provider.row_realizations[exit],
                Some("macos_arm64"),
            ),
        ] {
            assert_eq!(
                unsupported,
                Some(selected_dispatch::SelectedCompilerIntrinsicExecutionIdentity::Unsupported,),
                "requirement, realization, and selected target are independent catalog authority",
            );
        }
        {
            let method = "read_line";
            let index = plan
                .rows
                .iter()
                .position(|row| row.method == method)
                .unwrap_or_else(|| panic!("Console plan must retain {method}"));
            assert_eq!(
                retained.row_compiler_intrinsic_executions[index], None,
                "unsupported CompilerIntrinsic row `{method}` must remain outside the closed catalog",
            );
        }
    }
    let targetless =
        compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
            .expect("targetless Console provider should compile to checked trees");
    let (targetless_plan, targetless_retained) = targetless
        .selected_provider_plans()
        .plans()
        .iter()
        .zip(targetless.selected_provider_provenance())
        .find(|(plan, _)| plan.schema.trait_name == "Console")
        .expect("targetless std Console plan");
    let targetless_exit = targetless_plan
        .rows
        .iter()
        .position(|row| row.method == "exit_process")
        .expect("targetless Console exit row");
    assert_eq!(
        targetless_retained.row_compiler_intrinsic_executions[targetless_exit], None,
        "exact selected Linux target is independent catalog authority",
    );
}

#[test]
fn hosted_console_exit_catalog_settlement_emits_and_executes_native_image() {
    let canary = pass_canary(fixture_roster::ADAPTER_SATISFIES_COMPILE);
    for target in ["linux_x86_64", "linux_arm64", "macos_arm64"] {
        let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
            &canary.join("main.omg"),
            Some(target),
        ))
        .expect("Console permission preflight should reach checked custody");
        let permission_policy = native_realization::terminal_authority_permission_policy_with_rows(
            checked
                .selected_provider_plans()
                .plans()
                .iter()
                .flat_map(|plan| {
                    plan.rows
                        .iter()
                        .filter(|&row| {
                            matches!(
                                row.binding,
                                effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
                            )
                        })
                        .map(|row| {
                            native_realization::TerminalAuthorityPermissionPolicyRow::new(
                                plan.schema.identity_digest(),
                                row.requirement_identity.clone(),
                                effects::TerminalAuthorityDisposition::from_classes([
                                    effects::TerminalAuthorityClass::ProcessTermination,
                                ]),
                            )
                        })
                })
                .collect(),
        )
        .expect("exact Console exit permission policy");
        let accepted_permission = permission_policy.identity();
        let compilation = compile_rooted_backend_canary_without_output_for_target_and_permission_policy(
            &canary,
            target,
            permission_policy,
        )
            .unwrap_or_else(|diagnostics| {
                panic!("exact hosted Console exit catalog row should compile for {target}: {diagnostics:#?}")
            });
        let artifact = compilation
            .retained_native_artifact()
            .expect("NativeArtifact compilation should retain its exact product");
        let expected_magic: &[u8] = if target == "macos_arm64" {
            &[0xcf, 0xfa, 0xed, 0xfe]
        } else {
            b"\x7fELF"
        };
        assert!(
            artifact.image().output().bytes.starts_with(expected_magic),
            "selected hosted Console exit must retain the exact target image for {target}"
        );
        hosted_exit::assert_matching_host_exit(target, &artifact.image().output().bytes, 70);
        let [settlement] = artifact.image().boundary_settlements() else {
            panic!("exactly one Console exit boundary settlement for {target}")
        };
        assert!(matches!(
            settlement.settlement.realization,
            target_operations::BoundaryRealization::HostedExitProcessI32(_),
        ));
        assert_eq!(
            settlement.settlement.execution,
            native_realization::BoundaryExecutionRecord::CompilerBuiltin(
                target_operations::CompilerBuiltinExecution::HostedExitProcessI32,
            ),
            "the consuming lowerer must retain structural builtin custody for {target}",
        );
        assert_eq!(
            artifact.provider_executions().len(),
            0,
            "compiler builtins must not mint provider execution evidence for {target}",
        );
        let accepted_policy =
            native_realization::current_compiler_intrinsic_terminal_authority_policy();
        assert_eq!(
            artifact.terminal_authority_policy_identity(),
            accepted_policy.identity(),
            "native artifact must retain the exact receiving policy for {target}",
        );
        assert_eq!(
            accepted_policy
                .classify(effects::CompilerIntrinsicExecutionIdentity::HostedExitProcessI32)
                .expect("closed compiler-intrinsic policy classifies Linux exit-group")
                .classes(),
            &[effects::TerminalAuthorityClass::ProcessTermination],
        );
        let evidence = artifact
            .physical_evidence()
            .unwrap_or_else(|| panic!("{target} exit_group must retain complete D32 evidence"));
        assert!(
            matches!(
                artifact.physical_evidence_scope(),
                native::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
            ),
            "selected exit retains the validated source-to-physical projection",
        );
        assert!(
            artifact
                .boundary_application_coverage()
                .expect("checked native artifact retains exact D29 custody")
                .references()
                .is_empty(),
            "empty checked D29 custody is distinct from unavailable custody",
        );
        assert_eq!(evidence.projection().boundary_occurrences().len(), 1);
        let [child] = evidence.children() else {
            panic!("{target} exit_group must have exactly one physical child")
        };
        assert_eq!(child.projection(), evidence.projection().identity());
        assert_eq!(
            child.machine_span().byte_count(),
            settlement.settlement.byte_count
        );
        assert_eq!(child.object_span().offset(), settlement.text_offset);
        assert_eq!(child.final_image_span(), child.object_span());
        let native::PhysicalChildParent::BoundaryTraitSettlement(parent) = child.parent() else {
            panic!("exit_group child must retain its D41 parent")
        };
        assert_eq!(parent.target(), artifact.target());
        assert_eq!(
            parent.execution(),
            target_operations::BoundaryExecutionBinding::CompilerBuiltin(
                target_operations::CompilerBuiltinExecution::HostedExitProcessI32,
            ),
        );

        let artifact = compilation
            .into_retained_native_artifact()
            .expect("NativeArtifact report should transfer exact custody");
        let accepted_closure_review = artifact.terminal_authority_closure_review().identity();
        let parts = artifact.into_parts();
        let replayed = native::NativeArtifact::from_replayed_parts(replay_parts(&parts))
            .expect("unchanged physical evidence must replay");
        replayed
            .validate_for_terminal_authority_policies(
                accepted_policy.identity(),
                accepted_permission,
                accepted_closure_review,
            )
            .expect("unchanged receiving policies and closure review must replay");
        let mut unaccepted_closure_review = accepted_closure_review;
        unaccepted_closure_review[0] ^= 1;
        assert_eq!(
            replayed.validate_for_terminal_authority_policies(
                accepted_policy.identity(),
                accepted_permission,
                unaccepted_closure_review,
            ),
            Err("native artifact terminal-authority closure review is not accepted"),
            "structurally valid receipt data cannot substitute for the review identity accepted by the receiver",
        );

        let mut missing_d29_custody = replay_parts(&parts);
        missing_d29_custody.boundary_application_coverage = None;
        assert!(
            native::NativeArtifact::from_replayed_parts(missing_d29_custody).is_err(),
            "empty-D29 physical scope cannot survive removal of exact D29 custody",
        );

        let mut substituted_policy = replay_parts(&parts);
        substituted_policy.terminal_authority_policy_identity =
            effects::TerminalAuthorityPolicyIdentity::from_parts(
                accepted_policy.identity().version() + 1,
                accepted_policy.identity().commitment(),
            );
        assert!(
            native::NativeArtifact::from_replayed_parts(substituted_policy).is_err(),
            "the retained closure receipt must reject physical-policy substitution before receiver acceptance",
        );

        let mut missing = replay_parts(&parts);
        missing.physical_evidence = None;
        assert!(
            native::NativeArtifact::from_replayed_parts(missing).is_err(),
            "removing the only physical child must reject",
        );

        let mut unavailable = replay_parts(&parts);
        unavailable.physical_evidence_scope = native::NativePhysicalEvidenceScope::Unavailable;
        unavailable.physical_evidence = None;
        let unavailable = native::NativeArtifact::from_replayed_parts(unavailable)
            .expect("an explicitly unavailable projection retains no D32 claim");
        assert!(unavailable.physical_evidence().is_none());

        let mutate_child = |mutate: fn(&mut native::NativePhysicalChildParts)| {
            let mut corrupted = replay_parts(&parts);
            let evidence = corrupted
                .physical_evidence
                .take()
                .expect("exit_group physical evidence")
                .into_parts();
            let mut children = evidence.children;
            let mut child = children.remove(0).into_parts();
            mutate(&mut child);
            children.push(native::NativePhysicalChild::from_replayed_parts(child));
            corrupted.physical_evidence =
                Some(native::NativePhysicalEvidence::from_replayed_parts(
                    native::NativePhysicalEvidenceParts {
                        projection: evidence.projection,
                        children,
                        identity: evidence.identity,
                    },
                ));
            assert!(native::NativeArtifact::from_replayed_parts(corrupted).is_err());
        };
        mutate_child(|child| {
            child.object_span = native::NativeByteSpan::from_replayed_parts(
                child.object_span.offset() + 1,
                child.object_span.byte_count(),
            );
        });
        mutate_child(|child| {
            child.projection =
                optimization_core::NativeOptimizationProjectionIdentity::from_bytes([7; 32]);
        });
        mutate_child(|child| {
            let native::PhysicalChildParent::BoundaryTraitSettlement(parent) = child.parent.clone()
            else {
                panic!("exit_group child must retain its D41 parent")
            };
            let mut parent = parent.into_parts();
            parent.selected_plan_digest =
                native::NativeSelectedProviderPlanDigest::from_digest([11; 32]);
            child.parent = native::PhysicalChildParent::BoundaryTraitSettlement(
                native::BoundaryTraitSettlement::from_replayed_parts(parent),
            );
        });

        let mut duplicate = replay_parts(&parts);
        let evidence = duplicate
            .physical_evidence
            .take()
            .expect("exit_group physical evidence")
            .into_parts();
        let mut children = evidence.children;
        children.push(children[0].clone());
        duplicate.physical_evidence = Some(native::NativePhysicalEvidence::from_replayed_parts(
            native::NativePhysicalEvidenceParts {
                projection: evidence.projection,
                children,
                identity: evidence.identity,
            },
        ));
        assert!(native::NativeArtifact::from_replayed_parts(duplicate).is_err());

        let mut padded = replay_parts(&parts);
        let evidence = padded
            .physical_evidence
            .take()
            .expect("exit_group physical evidence")
            .into_parts();
        padded.physical_evidence = Some(native::NativePhysicalEvidence::from_replayed_parts(
            native::NativePhysicalEvidenceParts {
                projection: evidence.projection,
                children: evidence.children,
                identity: [13; 32],
            },
        ));
        assert!(native::NativeArtifact::from_replayed_parts(padded).is_err());
    }
}

#[test]
fn checked_physical_terminal_role_remains_explicit() {
    let port_canary = pass_canary(fixture_roster::ASM_PORT_OUT_FINAL_VALIDATION);
    let port_diagnostics =
        compile_rooted_backend_canary_without_output_for_target(&port_canary, "linux_x86_64")
            .expect_err("checked physical operations await their explicit D45 terminal role");
    assert!(
        port_diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("checked physical terminal operation unsupported")),
        "the bounded current-role review must fail closed on checked physical leaves: {port_diagnostics:#?}",
    );
}

#[test]
fn terminal_product_reloads_native_realization_without_checked_compilation() {
    let canary = pass_canary(fixture_roster::ADAPTER_SATISFIES_COMPILE);
    let package_inputs =
        reviewed_repository_fixture_package_inputs(&canary.join("main.omg"), Some("linux_x86_64"))
            .expect("derive reviewed repository fixture package inputs")
            .expect("Console canary has a standard-library dependency");
    let report = compiler::compile(
        CompileRequest::new(CompilerOptions {
            root_path: canary.join("main.omg"),
            build_dir: None,
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
        .with_package_inputs(package_inputs),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("Terminal product should retain its target-constrained native proposal");
    let retained = report
        .into_retained_terminal_artifact()
        .expect("Terminal report should transfer complete product custody");
    let (artifact, callback_placements, proposal) = retained.into_parts();
    assert!(callback_placements.is_empty());
    let proposal = proposal.expect("compiler Terminal product retains native proposal");
    proposal
        .validate_for_artifact(&artifact)
        .expect("native proposal must rejoin its canonical Terminal artifact");
    assert_eq!(proposal.target_profile(), target::TargetProfile::LinuxX64,);
    assert!(
        compilation_report::TerminalNativeRealizationProposal::new(
            &artifact,
            compilation_report::TerminalNativeRealizationInputs {
                target_profile: target::TargetProfile::MacosArm64,
                native_target: proposal.native_target(),
                subsystem: proposal.subsystem(),
                application_intent: proposal.application_intent(),
                application_identifier: proposal.application_identifier().cloned(),
                application_name: proposal.application_name().map(str::to_owned),
                post_terminal_optimizations: proposal.post_terminal_optimizations().clone(),
                program_entry: proposal.program_entry().clone(),
                checked_program_entry: proposal.checked_program_entry().clone(),
                selected_provider_plans: proposal.selected_provider_plans().clone(),
                external_binding_rows: proposal.external_binding_rows().to_vec(),
                package_terminal_authority_permissions: proposal
                    .package_terminal_authority_permissions()
                    .to_vec(),
                compiler_builtins: proposal.compiler_builtins().to_vec(),
                callback_occurrences: proposal.callback_occurrences().to_vec(),
                ieee_float_fma_occurrences: proposal.ieee_float_fma_occurrences().to_vec(),
                ieee_float_comparison_occurrences: proposal
                    .ieee_float_comparison_occurrences()
                    .to_vec(),
                integer_comparison_occurrences: proposal.integer_comparison_occurrences().to_vec(),
                boundary_application_demands: proposal.boundary_application_demands().clone(),
                boundary_application_realizations: proposal
                    .boundary_application_realizations()
                    .clone(),
                checked_boundary_operator_scope: proposal.checked_boundary_operator_scope().clone(),
                behavior_exclusions: proposal.behavior_exclusions().clone()
            }
        )
        .is_err(),
        "a target-profile substitution must not re-enter Terminal product custody",
    );
    let calling_plans = proposal.program_entry().calling_plans().map(|plans| {
        (
            &plans.semantic_calling_application,
            &plans.physical_calling_application,
            &plans.storage_entry,
        )
    });
    let program_entry = native_realization::NativeProgramEntrySettlement::new(
        proposal.program_entry().source_signature(),
        calling_plans,
        proposal.program_entry().fused_service_establishments(),
    )
    .with_checked_entry(proposal.checked_program_entry());
    let compiler_builtins = proposal
        .compiler_builtins()
        .iter()
        .map(
            |builtin| native_realization::NativeCompilerBuiltinSettlement {
                requirement_identity: builtin.requirement_identity(),
                provider_plan: &proposal.selected_provider_plans().plans()
                    [builtin.provider_plan_index()],
                execution: builtin.execution(),
            },
        )
        .collect::<Vec<_>>();
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let permission_policy = native_realization::terminal_authority_permission_policy_with_rows(
        proposal
            .compiler_builtins()
            .iter()
            .map(|builtin| {
                let plan =
                    &proposal.selected_provider_plans().plans()[builtin.provider_plan_index()];
                native_realization::TerminalAuthorityPermissionPolicyRow::new(
                    plan.schema.identity_digest(),
                    builtin.requirement_identity(),
                    effects::TerminalAuthorityDisposition::from_classes([
                        effects::TerminalAuthorityClass::ProcessTermination,
                    ]),
                )
            })
            .collect(),
    )
    .expect("exact Console exit permission policy");
    let native = native_realization::realize_native_artifact(
        artifact,
        native_realization::NativeRealizationRequest {
            checked_scope: Some(proposal.checked_boundary_operator_scope()),
            prepared_input: None,
            target: proposal.native_target(),
            image_request: native_realization::ExecutableImageEmissionRequest::direct(
                proposal.subsystem(),
            ),
            profile: &profile,
            terminal_authority_policy:
                native_realization::current_compiler_intrinsic_terminal_authority_policy(),
            terminal_authority_permission_policy: Some(permission_policy),
            program_entry,
            optimization_selections: &optimizations,
            selected_provider_plans: proposal.selected_provider_plans(),
            external_binding_rows: proposal.external_binding_rows(),
            settlements: &[],
            compiler_builtins: &compiler_builtins,
            boundary_application_coverage: Some(proposal.boundary_application_coverage()),
            ieee_float_fma: &[],
            native_callbacks: &[],
            callback_thunks: &[],
        },
    )
    .expect("retained Terminal product should realize natively without frontend state")
    .into_direct()
    .expect("direct image requested");
    assert!(native.image().output().bytes.starts_with(b"\x7fELF"));
    assert_eq!(native.provider_executions().len(), 0);
    assert_eq!(
        native
            .physical_evidence()
            .expect("reloaded Terminal product retains D32 evidence")
            .children()
            .len(),
        1,
    );
    let [settlement] = native.image().boundary_settlements() else {
        panic!("reloaded product should retain one Console exit settlement")
    };
    assert_eq!(
        settlement.settlement.execution,
        native_realization::BoundaryExecutionRecord::CompilerBuiltin(
            target_operations::CompilerBuiltinExecution::HostedExitProcessI32,
        ),
    );
    assert!(matches!(
        native.physical_evidence_scope(),
        native_realization::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    let replayed = native_realization::NativeArtifact::from_replayed_parts(native.into_parts())
        .expect("reloaded D32 artifact should replay from retained custody");
    let mut missing_child = replayed.into_parts();
    missing_child.physical_evidence = None;
    assert!(
        native_realization::NativeArtifact::from_replayed_parts(missing_child).is_err(),
        "reloaded D32 replay must reject removal of its surviving physical child",
    );
}

#[test]
fn runtime_boundary_capability_state_forwarding_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_BOUNDARY_CAPABILITY_STATE_FORWARDING_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("boundary capability should forward through a state parameter");
    assert_eq!(
        checked.selected_program_entry_machine(),
        None,
        "targetless checking must not select an authored target entry"
    );
    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
    assert_eq!(outcome.stdout, b"K".to_vec());

    let scratch = std::env::temp_dir().join(format!(
        "omega-boundary-capability-state-forwarding-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("boundary capability should compile through native storage planning");
    let executable = compilation
        .checked_native_executable_path()
        .expect("boundary-capability canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("boundary-capability forwarding canary should run");
    assert_eq!(output.status.code(), Some(70));
    assert_eq!(output.stdout, b"K".to_vec());
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_console_byte_literal_exit_canary_runs() {
    // Literal byte writes agree between the checked interpreter and native product.
    let canary = pass_canary(fixture_roster::RUNTIME_CONSOLE_BYTE_LITERAL_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("byte-literal canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.error, None,
        "byte-literal writes should interpret cleanly"
    );
    assert_eq!(outcome.exit_code, 70);
    assert_eq!(outcome.stdout, b"7\n".to_vec(), "interpreter literal bytes");

    let scratch = std::env::temp_dir().join(format!("omega-byte-literal-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("byte-literal canary should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("byte-literal canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("byte-literal canary should run");
    assert_eq!(output.status.code(), Some(70));
    assert_eq!(output.stdout, b"7\n".to_vec(), "native literal bytes");
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn rooted_fixture_explicit_deny_policy_still_rejects() {
    let canary = pass_canary(fixture_roster::RUNTIME_CONSOLE_BYTE_LITERAL_EXIT);
    let diagnostics =
        compile_rooted_backend_canary_without_output_for_target_and_permission_policy(
            &canary,
            "linux_x86_64",
            native_realization::current_terminal_authority_permission_policy(),
        )
        .expect_err("an explicitly empty receiving policy must not inherit fixture permissions");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("receiving terminal-authority policy omits the accepted permission")
    }));
}

#[test]
fn runtime_console_byte_sources_retain_checked_unit_plans_and_terminal_artifacts() {
    for fixture in [
        fixture_roster::RUNTIME_CONSOLE_BYTE_LITERAL_EXIT,
        fixture_roster::RUNTIME_CONSOLE_BYTE_INSPECTION_EXIT,
    ] {
        let compilation = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
            &pass_canary(fixture).join("main.omg"),
            Some("linux_x86_64"),
        ))
        .expect("console source should check");
        let checked = &compilation;
        let plans = &checked.facts.flow.terminal_unit_effects;
        let main = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("Main entry");
        assert!(
            plans.for_machine(main.symbol).is_some()
                || plans.composed_for_machine(main.symbol).is_some(),
            "{fixture}: missing Main plan; boundary plans: {:#?}",
            plans.boundary_machines
        );
        let root_path = pass_canary(fixture).join("main.omg");
        let package_inputs =
            reviewed_repository_fixture_package_inputs(&root_path, Some("linux_x86_64"))
                .expect("fixture package inputs");
        let mut request = CompileRequest::new(CompilerOptions {
            root_path,
            build_dir: None,
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact);
        if let Some(package_inputs) = package_inputs {
            request = request.with_package_inputs(package_inputs);
        }
        compiler::compile(request)
            .and_then(compiler::CompileOutcomes::into_single_report)
            .unwrap_or_else(|diagnostics| {
                panic!("{fixture}: Terminal production failed: {diagnostics:#?}")
            })
            .into_retained_terminal_artifact()
            .expect("console source retains its Terminal artifact")
            .validate()
            .unwrap_or_else(|error| panic!("{fixture}: Terminal artifact replay failed: {error}"));
    }
}

#[test]
fn runtime_console_byte_literal_linux_catalog_replays_both_targets() {
    let canary = pass_canary(fixture_roster::RUNTIME_CONSOLE_BYTE_LITERAL_EXIT);
    for target in ["linux_x86_64", "linux_arm64"] {
        compile_rooted_backend_canary_without_output_for_target(&canary, target).unwrap_or_else(
            |error| panic!("Linux write-byte catalog must compile for {target}: {error:?}"),
        );
    }
}

#[test]
fn runtime_console_byte_read_return_catalog_replays_supported_hosted_targets() {
    let canary = pass_canary(fixture_roster::RUNTIME_CONSOLE_BYTE_READ_RETURN);
    for target in ["linux_x86_64", "linux_arm64", "macos_arm64"] {
        let compilation = compile_rooted_backend_canary_without_output_for_target(&canary, target)
            .unwrap_or_else(|error| {
                panic!("hosted read-byte catalog must compile for {target}: {error:?}")
            });
        let artifact = compilation
            .into_retained_native_artifact()
            .expect("read-byte compilation retains native artifact");
        artifact
            .validate()
            .expect("read-byte artifact independently replays");
        let read_results = artifact
            .object()
            .boundary_settlements()
            .iter()
            .filter_map(|row| row.settlement.native_result.structural())
            .collect::<Vec<_>>();
        assert_eq!(read_results.len(), 1);
        let result = read_results[0];
        assert_eq!(
            result.layout.shape,
            calling_conventions::ValueShape::integer(8, 4)
        );
        assert!(result.home_byte_offset.is_multiple_of(4));
        assert!(
            artifact
                .object()
                .functions()
                .iter()
                .all(|function| function.unit_affine_cleanup.is_none()),
            "edge-owned no-code cleanup uses retained graph replay, not a singular projection"
        );
        assert!(artifact.image().boundary_settlements().iter().any(|row| {
            row.settlement.execution
                == native_realization::BoundaryExecutionRecord::CompilerBuiltin(
                    target_operations::CompilerBuiltinExecution::HostedReadByte,
                )
                && row.settlement.native_result.structural().is_some()
        }));
        let parts = artifact.into_parts();
        let mut stripped = replay_parts(&parts);
        stripped.object.clear_fragment_replay_for_test();
        assert!(
            native::NativeArtifact::from_replayed_parts(stripped).is_err(),
            "even an uninspected read requires exact return cleanup replay"
        );
    }
}

#[test]
fn runtime_console_byte_inspection_replays_validated_cross_target_artifacts() {
    // The structural result home, case tag, and selected payload must replay
    // through every admitted hosted byte-input path.
    let canary = pass_canary(fixture_roster::RUNTIME_CONSOLE_BYTE_INSPECTION_EXIT);
    for target in ["linux_x86_64", "linux_arm64", "macos_arm64"] {
        let compilation = compile_rooted_backend_canary_without_output_for_target(&canary, target)
            .unwrap_or_else(|error| {
                panic!("runtime byte inspection must compile for {target}: {error:?}")
            });
        compilation
            .retained_native_artifact()
            .unwrap_or_else(|| panic!("runtime byte inspection must retain {target} artifact"))
            .validate()
            .unwrap_or_else(|error| {
                panic!("runtime byte inspection must validate {target} artifact: {error}")
            });
    }
}

#[test]
fn runtime_console_byte_replay_cross_target_canary_compiles() {
    // The final-byte certificate must replay both byte composites under each
    // settled target adapter: Linux syscall, Darwin direct import, and the
    // complete Win64 GetStdHandle + file-I/O import pair.
    let canary = pass_canary(fixture_roster::RUNTIME_CONSOLE_BYTE_ECHO_EXIT);
    for target in [
        "linux_x86_64",
        "linux_arm64",
        "macos_arm64",
        "windows_x86_64",
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-console-byte-replay-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        compile_rooted_canary_for_target(&canary, scratch.join("out"), target).unwrap_or_else(
            |error| panic!("runtime byte replay must compile for {target}: {error:?}"),
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn runtime_console_line_replay_cross_target_canary_compiles() {
    // Required native acceptance for the shared checked reader over target byte
    // leaves. This remains open at ordinary caller/entry closure and Windows
    // byte-input support; cross-emission alone is not matching-host execution.
    for &canary_name in fixture_roster::CONSOLE_LINE_REPLAY_CANARIES {
        let canary = pass_canary(canary_name);
        for target in [
            "linux_x86_64",
            "linux_arm64",
            "macos_arm64",
            "windows_x86_64",
        ] {
            let scratch = std::env::temp_dir().join(format!(
                "omega-console-line-replay-{}-{target}-{}",
                canary_name.replace('/', "-"),
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&scratch);
            if canary_name == fixture_roster::RUNTIME_CONSOLE_LINE_FIXED_ARRAY_EXIT {
                compile_rooted_canary_for_target(&canary, scratch.join("out"), target)
                    .unwrap_or_else(|error| {
                        panic!(
                            "runtime line replay {canary_name} must compile for {target}: {error:?}"
                        )
                    });
                let _ = fs::remove_dir_all(&scratch);
                continue;
            }
            let src_dir = scratch.join("src");
            fs::create_dir_all(&src_dir).expect("runtime line replay scratch source");
            fs::copy(canary.join("main.omg"), src_dir.join("main.omg"))
                .expect("copy runtime line replay canary");
            fs::write(
                src_dir.join("build.omg"),
                hosted_main_program_entry_build_for(&canary, target),
            )
            .expect("write runtime line replay build source");
            compile(CanaryCompileSpec {
                root_path: src_dir.join("main.omg"),
                build_dir: Some(scratch.join("out")),
                target_name: Some(target.to_owned()),
                product: CanaryCompileProduct::NativeArtifactAndPublish,
            })
            .unwrap_or_else(|error| {
                panic!("runtime line replay {canary_name} must compile for {target}: {error:?}")
            });
            let _ = fs::remove_dir_all(&scratch);
        }
    }
}

#[test]
fn runtime_import_call_argument_exit_canary_runs() {
    // The authored-import argument fix: exit(70) through an external
    // DllImport leaf reaches libSystem with its argument intact. NATIVE
    // assert only -- the interpreter does not serve custom-capability
    // imports (its own rung). The leaf is one evaluated typed Mach-O
    // locator produced by `leaf_binding`; ordinary external `via`
    // evaluation requires the canary's own selected Darwin target.
    let canary = pass_canary(fixture_roster::RUNTIME_IMPORT_CALL_ARGUMENT_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &main_path,
        Some("macos_arm64"),
    ))
    .expect("evaluated DllImport leaf should resolve the Leaf slot for its Darwin target");
    assert_eq!(
        checked.selected_program_entry_machine(),
        Some("Main::main"),
        "the reviewed Darwin fixture selects the authored main entry"
    );
    let leaf_plan = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "Leaf")
        .expect("Leaf must retain its selected evaluated DllImport plan");
    assert_eq!(leaf_plan.provider_type, "");
    assert!(leaf_plan.covers_schema());
    assert_eq!(leaf_plan.rows.len(), 1);
    assert_eq!(leaf_plan.rows[0].method, "exit");
    let effects::provider_plan::ProviderBinding::Import { evaluated } = &leaf_plan.rows[0].binding
    else {
        panic!("Leaf must retain one evaluated import binding, never a string-backed row");
    };
    assert_eq!(
        evaluated.locator().target(),
        target::TargetProfile::MacosArm64
    );
    assert_eq!(
        evaluated.locator().locator(),
        &target::ForeignLocatorCandidate::MachODylibSymbol {
            install_name: b"libSystem.B.dylib".to_vec(),
            symbol: b"_exit".to_vec(),
        }
    );
    assert_eq!(
        evaluated.receipt().locator_identity_digest(),
        evaluated.locator().identity_digest()
    );

    let build_dir = std::env::temp_dir().join(format!("omega-import-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_target(&canary, build_dir.clone(), "macos_arm64")
        .expect("authored-import call canary should compile from its Darwin root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("authored-import call canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("authored-import call canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "the import call's argument must reach the callee: {:?}",
        output.status.code()
    );
    let _ = fs::remove_dir_all(&build_dir);
}
