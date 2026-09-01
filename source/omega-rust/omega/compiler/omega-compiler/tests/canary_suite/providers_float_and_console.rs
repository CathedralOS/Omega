use super::*;

fn checked_adapter_identity(
    checked: &psi_checked_trees::CheckedTrees,
    machine_name: &str,
) -> String {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .unwrap_or_else(|| panic!("missing checked adapter `{machine_name}`"));
    checked
        .normalized_machine_overload_identity(machine)
        .expect("checked adapter must have an entry overload")
        .identity()
}

fn assert_selected_operator_terminal_call(canary: &Path, label: &str) {
    let root_path = canary.join("main.omg");
    let package_inputs =
        reviewed_repository_fixture_package_inputs(&root_path, Some("linux_x86_64"))
            .unwrap_or_else(|diagnostics| {
                panic!("{label} should derive reviewed package inputs: {diagnostics:#?}")
            });
    let mut request = CompileRequest::new(CompilerOptions {
        root_path,
        build_dir: None,
        target_name: Some("linux_x86_64".into()),
    })
    .with_requested_product(RequestedCompileProduct::TerminalArtifact);
    if let Some(package_inputs) = package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    let report = omega_compiler::compile(request).unwrap_or_else(|diagnostics| {
        panic!("{label} should produce a canonical Terminal artifact: {diagnostics:#?}")
    });
    let retained = report
        .into_retained_terminal_artifact()
        .unwrap_or_else(|| panic!("{label} should retain its Terminal artifact"));
    retained
        .validate()
        .unwrap_or_else(|error| panic!("{label} Terminal artifact should replay: {error}"));
    let module = psi_terminal_codec::decode_module(retained.artifact().semantic_bytes())
        .unwrap_or_else(|error| panic!("{label} Terminal semantics should decode: {error:?}"));
    let proposal = retained
        .native_realization_proposal()
        .unwrap_or_else(|| panic!("{label} should retain its native proposal"));
    let [operator_occurrence] = proposal.checked_boundary_operator_scope().occurrences() else {
        panic!("{label} should retain one exact checked-to-Terminal operator occurrence")
    };
    let [source_free_demand] = proposal.boundary_application_demands().rows() else {
        panic!("{label} should retain one exact source-free boundary application demand")
    };
    assert_eq!(
        source_free_demand.terminal_operation(),
        operator_occurrence.terminal_operation(),
        "{label} source-free demand should retain the exact Terminal occurrence",
    );
    assert!(
        !source_free_demand
            .requirement()
            .declaration()
            .canonical()
            .is_empty()
            && !source_free_demand.requirement().overload().is_empty(),
        "{label} source-free demand should retain exact nominal and overload identity",
    );
    assert_eq!(
        source_free_demand.application(),
        &omega_boundary_applications::BoundaryApplication::Empty,
        "{label} nongeneric operator should retain the canonical empty application",
    );
    let [realization] = proposal.boundary_application_realizations().rows() else {
        panic!("{label} should retain one exact D29 realization companion")
    };
    assert_eq!(
        realization.terminal_operation(),
        source_free_demand.terminal_operation(),
    );
    assert_ne!(realization.selected_plan_digest(), &[0; 32]);
    assert_eq!(
        realization.role(),
        omega_boundary_applications::BoundaryApplicationRealizationRole::NongenericCheckedBody,
    );
    let [coverage] = proposal.boundary_application_coverage().references() else {
        panic!("{label} should publish one reconstructible D29 coverage reference")
    };
    assert_eq!(
        coverage.terminal_operation(),
        source_free_demand.terminal_operation(),
    );
    assert!(
        module.machines.iter().any(|machine| {
            machine.blocks.iter().any(|block| {
                block
                    .operations
                    .iter()
                    .enumerate()
                    .any(|(call_index, operation)| {
                        if operation.id != operator_occurrence.terminal_operation() {
                            return false;
                        }
                        let psi_terminal::OperationKind::Call { .. } = &operation.kind else {
                            return false;
                        };
                        let psi_terminal::OperationResult::Scalar(result) = &operation.result
                        else {
                            return false;
                        };
                        block.operations[call_index + 1..].iter().any(|consumer| {
                            matches!(
                                &consumer.kind,
                                psi_terminal::OperationKind::BoundaryCall { arguments, .. }
                                    if arguments == &[result.id]
                            )
                        })
                    })
            })
        }),
        "{label} should pass the selected scalar Call result to its later boundary consumer"
    );
}

#[test]
fn runtime_adapter_dispatch_exit_canary_runs() {
    // PRV4 adapter dispatch: the boundary-trait call rewrites to the unique
    // satisfying adapter in both engines; the interpreter leg rides the
    // differential row.
    let canary = pass_canary("providers/runtime_adapter_dispatch_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("adapter-dispatch canary should compile to checked trees");
    assert_eq!(
        checked.selected_program_entry_machine(),
        None,
        "targetless checking must not select an authored target entry"
    );
    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.exit_code, 70, "interpreter dispatches the adapter");

    let scratch =
        std::env::temp_dir().join(format!("omega-adapter-dispatch-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("adapter-dispatch canary should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("adapter-dispatch canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("adapter-dispatch canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "native dispatches the adapter"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn checked_boundary_operator_dispatch_exit_canary_runs() {
    let canary = pass_canary("providers/checked_boundary_operator_dispatch_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("checked boundary-operator canary should compile to checked trees");
    assert!(!checked.facts.operators.boundary_applications.is_empty());
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter dispatches the selected checked operator body; error: {:?}",
        outcome.error,
    );

    assert_selected_operator_terminal_call(&canary, "checked boundary-operator canary");
}

#[test]
fn checked_fixed_operator_dispatch_exit_canary_runs() {
    let canary = pass_canary("providers/checked_fixed_operator_dispatch_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("checked fixed-token operator canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter dispatches the selected fixed-token operator body; error: {:?}",
        outcome.error,
    );
    assert_selected_operator_terminal_call(&canary, "checked fixed-token operator canary");
}

#[test]
fn runtime_result_domain_requirement_overload_exit_canary_runs() {
    // Result-domain identity survives requirement collection, provider-plan
    // selection, checked adapter dispatch, and both executable engines.
    let canary = pass_canary("providers/runtime_result_domain_requirement_overload_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("result-overloaded provider requirements should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter must dispatch each exact requirement overload; error: {:?}",
        outcome.error
    );

    let scratch = std::env::temp_dir().join(format!(
        "omega-result-overloaded-provider-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("result-overloaded provider requirements should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("result-overloaded provider canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("result-overloaded provider canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "native runtime must dispatch each exact requirement overload"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_selected_provider_adapter_exit_canary_runs() {
    // PRV4b/4c composition: a retained whole-provider selection is
    // authoritative for adapter dispatch. Selecting SecondProvider must ignore
    // FirstProvider in both engines.
    let canary = pass_canary("providers/provider_type_slot_selected");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("selected-provider adapter canary should compile to checked trees");
    assert_eq!(
        checked.selected_program_entry_machine(),
        None,
        "targetless checking must not select an authored target entry"
    );
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter must dispatch only the selected SecondProvider adapter; error: {:?}",
        outcome.error
    );

    let scratch = std::env::temp_dir().join(format!(
        "omega-selected-provider-adapter-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("selected-provider adapter canary should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("selected-provider adapter canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("selected-provider adapter canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "native dispatch must use only the selected SecondProvider adapter"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn provider_type_target_default_canary_selects_target_default() {
    let canary = pass_canary("providers/provider_type_target_default");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
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
    let canary = pass_canary("providers/component_owner_provider_override_compile");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
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
        omega_effects::provider_plan::ProviderBinding::CheckedAdapter { machine_identity, .. }
            if machine_identity == &expected
    ));
}

#[test]
fn test_owner_provider_override_canary_selects_complete_pick_plan() {
    let canary = pass_canary("providers/test_owner_provider_override_compile");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
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
        omega_effects::provider_plan::ProviderBinding::CheckedAdapter { machine_identity, .. }
            if machine_identity == &expected
    ));
}

#[test]
fn provider_type_target_default_override_canary_selects_build_override() {
    let canary = pass_canary("providers/provider_type_target_default_override");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
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
    let canary = pass_canary("providers/adapter_satisfies_compile");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
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
        omega_effects::provider_plan::ProviderBinding::CheckedAdapter { machine_identity, .. }
            if machine_identity == &expected
    ));
}

#[test]
fn external_leaf_via_canary_selects_exact_free_import_plan() {
    let canary = pass_canary("providers/external_leaf_via_compile");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("free external leaf should resolve the Shutdown slot");
    assert_eq!(
        checked.selected_program_entry_machine(),
        None,
        "targetless checking must not select an authored target entry"
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
    assert!(matches!(
        &shutdown_plan.rows[0].binding,
        omega_effects::provider_plan::ProviderBinding::StringBackedImportBootstrap { library, symbol }
            if library == "kernel32.dll" && symbol == "ExitProcess"
    ));
}

#[test]
fn external_leaf_dllimport_canary_selects_exact_free_import_plan() {
    let canary = pass_canary("providers/external_leaf_dllimport_compile");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("free DllImport leaf should resolve the Leaf slot");
    assert_eq!(
        checked.selected_program_entry_machine(),
        None,
        "targetless checking must not select an authored target entry"
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
    assert!(matches!(
        &leaf_plan.rows[0].binding,
        omega_effects::provider_plan::ProviderBinding::StringBackedImportBootstrap { library, symbol }
            if library == "libSystem.B.dylib" && symbol == "_exit"
    ));
}

#[test]
fn runtime_adapter_forwarding_exit_canary_runs() {
    // PRV4 standard self-forwarding adapter: the receiver forwards as argument
    // 0, and std Console::write reaches the write_byte leaf through that same
    // capability. Field-backed bounded carriers and literal-backed text both
    // cross the honest borrowed byte-view path.
    let canary = pass_canary("providers/runtime_adapter_forwarding_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
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
    for method in ["write", "write_line"] {
        let expected =
            checked_adapter_identity(&checked, &format!("ConsoleNativeProvider::{method}"));
        assert!(console_plan.rows.iter().any(|row| {
            row.method == method
                && matches!(
                    &row.binding,
                    omega_effects::provider_plan::ProviderBinding::CheckedAdapter { machine_identity, .. }
                        if machine_identity == &expected
                )
        }));
    }
    for method in ["read_line", "read_byte", "write_byte", "exit_process"] {
        assert!(console_plan.rows.iter().any(|row| {
            row.method == method
                && matches!(
                    &row.binding,
                    omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine: name }
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
            psi_checked_trees::statement::StatementNode::Call(call) => Some(call.target.as_str()),
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
        b"Field\nLiteral\n".to_vec(),
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
        b"Field\nLiteral\n".to_vec(),
        "native stdout must come from the std write adapter"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn linux_console_exit_compiler_intrinsic_review_identity_is_exact() {
    use omega_provider_planning::plans::CompilerIntrinsicExecutionIdentity;

    let canary = pass_canary("providers/runtime_adapter_forwarding_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, Some("linux_x86_64"))
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
        Some(CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32),
    );
    let derive = |requirement, realization, target| {
        omega_selected_dispatch::derive_selected_compiler_intrinsic_execution_identity_for_row(
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
        derive(
            retained.provider.row_requirements[exit],
            retained.provider.row_realizations[exit],
            Some("linux_x86_64"),
        ),
        Some(omega_selected_dispatch::SelectedCompilerIntrinsicExecutionIdentity::Unsupported),
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
            Some(omega_selected_dispatch::SelectedCompilerIntrinsicExecutionIdentity::Unsupported,),
            "requirement, realization, and selected target are independent catalog authority",
        );
    }
    for method in ["read_line", "read_byte", "write_byte"] {
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

    let targetless = compile_to_checked(&main_path, None)
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
fn linux_console_exit_catalog_settlement_emits_elf() {
    use omega_terminal_psi_to_native_artifact as native;

    fn replay_parts(parts: &native::NativeArtifactParts) -> native::NativeArtifactParts {
        let module = psi_terminal_codec::decode_module(parts.psi_artifact.semantic_bytes())
            .expect("replay Terminal semantics");
        let proof = psi_terminal_codec::decode_proof_bundle(parts.psi_artifact.proof_bytes())
            .expect("replay Terminal proof");
        let debug = parts
            .psi_artifact
            .debug_bytes()
            .map(|bytes| psi_terminal_codec::decode_debug_map(&module, bytes).expect("debug map"));
        native::NativeArtifactParts {
            target: parts.target,
            psi_artifact: psi_terminal_codec::CanonicalTerminalArtifact::from_parts(
                &module,
                &proof,
                debug.as_ref(),
            )
            .expect("reconstruct canonical Terminal artifact"),
            object: parts.object.clone(),
            image: parts.image.clone(),
            selected_provider_closure_report_identity: parts
                .selected_provider_closure_report_identity,
            selected_provider_closure_digest: parts.selected_provider_closure_digest,
            selected_provider_plans: parts.selected_provider_plans.clone(),
            provider_executions: parts.provider_executions.clone(),
            terminal_authority_policy_identity: parts.terminal_authority_policy_identity,
            boundary_application_coverage: parts.boundary_application_coverage.clone(),
            physical_evidence_scope: parts.physical_evidence_scope,
            physical_evidence: parts.physical_evidence.clone(),
        }
    }

    let canary = pass_canary("providers/adapter_satisfies_compile");
    for target in ["linux_x86_64", "linux_arm64"] {
        let compilation = compile_rooted_backend_canary_without_output_for_target(&canary, target)
            .unwrap_or_else(|diagnostics| {
                panic!("exact Linux Console exit catalog row should compile for {target}: {diagnostics:#?}")
            });
        let artifact = compilation
            .retained_native_artifact()
            .expect("NativeArtifact compilation should retain its exact product");
        assert!(
            artifact.image().output().bytes.starts_with(b"\x7fELF"),
            "selected Linux Console exit settlement must retain an ELF image for {target}",
        );
        let [settlement] = artifact.image().boundary_settlements() else {
            panic!("exactly one Console exit boundary settlement for {target}")
        };
        assert!(matches!(
            settlement.settlement.realization,
            omega_target_operations::BoundaryRealization::LinuxExitGroupI32(_),
        ));
        assert_eq!(
            settlement.settlement.execution,
            omega_terminal_psi_to_native_artifact::BoundaryExecutionRecord::CompilerBuiltin(
                omega_target_operations::CompilerBuiltinExecution::LinuxExitGroupI32,
            ),
            "the consuming lowerer must retain structural builtin custody for {target}",
        );
        assert_eq!(
            artifact.provider_executions().len(),
            0,
            "compiler builtins must not mint provider execution evidence for {target}",
        );
        let accepted_policy =
            omega_terminal_psi_to_native_artifact::current_compiler_intrinsic_terminal_authority_policy();
        assert_eq!(
            artifact.terminal_authority_policy_identity(),
            accepted_policy.identity(),
            "native artifact must retain the exact receiving policy for {target}",
        );
        assert_eq!(
            accepted_policy
                .classify(omega_effects::CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32)
                .expect("closed compiler-intrinsic policy classifies Linux exit-group")
                .classes(),
            &[omega_effects::TerminalAuthorityClass::ProcessTermination],
        );
        let evidence = artifact
            .physical_evidence()
            .unwrap_or_else(|| panic!("{target} exit_group must retain complete D32 evidence"));
        assert_eq!(
            artifact.physical_evidence_scope(),
            native::NativePhysicalEvidenceScope::UnoptimizedNoBoundaryOperatorApplications,
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
        let native::PhysicalChildParent::BoundaryTraitSettlement(parent) = child.parent();
        assert_eq!(parent.target(), artifact.target());
        assert_eq!(
            parent.execution(),
            omega_target_operations::BoundaryExecutionBinding::CompilerBuiltin(
                omega_target_operations::CompilerBuiltinExecution::LinuxExitGroupI32,
            ),
        );

        let artifact = compilation
            .into_retained_native_artifact()
            .expect("NativeArtifact report should transfer exact custody");
        let artifact_identity = artifact.identity();
        let parts = artifact.into_parts();
        let replayed = native::NativeArtifact::from_replayed_parts(replay_parts(&parts))
            .expect("unchanged physical evidence must replay");
        replayed
            .validate_for_terminal_authority_policy(accepted_policy.identity())
            .expect("unchanged receiving policy must replay");

        let mut missing_d29_custody = replay_parts(&parts);
        missing_d29_custody.boundary_application_coverage = None;
        assert!(
            native::NativeArtifact::from_replayed_parts(missing_d29_custody).is_err(),
            "empty-D29 physical scope cannot survive removal of exact D29 custody",
        );

        let mut substituted_policy = replay_parts(&parts);
        substituted_policy.terminal_authority_policy_identity =
            omega_effects::TerminalAuthorityPolicyIdentity::from_parts(
                accepted_policy.identity().version() + 1,
                accepted_policy.identity().commitment(),
            );
        let substituted_policy = native::NativeArtifact::from_replayed_parts(substituted_policy)
            .expect("a different policy identity describes a different valid artifact");
        assert_ne!(substituted_policy.identity(), artifact_identity);
        assert!(
            substituted_policy
                .validate_for_terminal_authority_policy(accepted_policy.identity())
                .is_err(),
            "the receiving authority must reject policy substitution",
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
                omega_optimization_core::NativeOptimizationProjectionIdentity::from_bytes([7; 32]);
        });
        mutate_child(|child| {
            let native::PhysicalChildParent::BoundaryTraitSettlement(parent) = child.parent.clone();
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

    let port_canary = pass_canary("inline_asm/asm_port_out_final_validation");
    let port_report =
        compile_rooted_backend_canary_without_output_for_target(&port_canary, "linux_x86_64")
            .expect("immediate-port checked assembly should retain its native artifact");
    let port_artifact = port_report
        .retained_native_artifact()
        .expect("immediate-port checked assembly should retain exact native custody");
    assert!(!port_artifact.object().port_effects().is_empty());
    assert!(
        port_artifact.physical_evidence().is_none(),
        "the first D32 lane must not publish partial evidence for an uncovered D29 port effect",
    );
}

#[test]
fn terminal_product_reloads_native_realization_without_checked_compilation() {
    let canary = pass_canary("providers/adapter_satisfies_compile");
    let report = omega_compiler::compile(
        CompileRequest::new(CompilerOptions {
            root_path: canary.join("main.omg"),
            build_dir: None,
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact),
    )
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
    assert_eq!(
        proposal.target_profile(),
        omega_target::TargetProfile::LinuxX64,
    );
    assert!(
        omega_compilation_report::TerminalNativeRealizationProposal::new(
            &artifact,
            omega_target::TargetProfile::MacosArm64,
            proposal.native_target(),
            proposal.subsystem(),
            proposal.program_entry().clone(),
            proposal.selected_provider_plans().clone(),
            proposal.external_binding_rows().to_vec(),
            proposal.compiler_builtins().to_vec(),
            proposal.callback_occurrences().to_vec(),
            proposal.ieee_float_fma_occurrences().to_vec(),
            proposal.boundary_application_demands().clone(),
            proposal.boundary_application_realizations().clone(),
            proposal.checked_boundary_operator_scope().clone(),
        )
        .is_err(),
        "a target-profile substitution must not re-enter Terminal product custody",
    );
    let calling_plans = proposal
        .program_entry()
        .calling_plans()
        .map(|plans| (&plans.semantic_boundary_entry_plan, &plans.storage_entry));
    let program_entry = omega_terminal_psi_to_native_artifact::NativeProgramEntrySettlement::new(
        proposal.program_entry().source_signature(),
        calling_plans,
    );
    let compiler_builtins = proposal
        .compiler_builtins()
        .iter()
        .map(
            |builtin| omega_terminal_psi_to_native_artifact::NativeCompilerBuiltinSettlement {
                requirement_identity: builtin.requirement_identity(),
                provider_plan: &proposal.selected_provider_plans().plans()
                    [builtin.provider_plan_index()],
                execution: builtin.execution(),
            },
        )
        .collect::<Vec<_>>();
    let profile = psi_proof_admission::AdmissionProfile::default();
    let optimizations = omega_optimization_core::OptimizationSelections::default();
    let native = omega_terminal_psi_to_native_artifact::realize_native_artifact_with_checked_boundary_operator_scope(
        artifact,
        proposal.checked_boundary_operator_scope(),
        omega_terminal_psi_to_native_artifact::NativeRealizationRequest {
            target: proposal.native_target(),
            subsystem: proposal.subsystem(),
            profile: &profile,
            terminal_authority_policy:
                omega_terminal_psi_to_native_artifact::current_compiler_intrinsic_terminal_authority_policy(),
            program_entry,
            optimization_selections: &optimizations,
            selected_provider_plans: proposal.selected_provider_plans(),
            external_binding_rows: proposal.external_binding_rows(),
            settlements: &[],
            compiler_builtins: &compiler_builtins,
            boundary_application_coverage: Some(proposal.boundary_application_coverage()),
            ieee_float_fma: &[],
            native_callbacks: &[],
        },
    )
    .expect("retained Terminal product and independent local admission should realize natively");
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
        omega_terminal_psi_to_native_artifact::BoundaryExecutionRecord::CompilerBuiltin(
            omega_target_operations::CompilerBuiltinExecution::LinuxExitGroupI32,
        ),
    );
}

#[test]
fn runtime_boundary_capability_state_forwarding_exit_canary_runs() {
    let canary = pass_canary("providers/runtime_boundary_capability_state_forwarding_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
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
    // write_byte(<integer literal>) -- the staged 1-byte data object path,
    // dead from birth until the HostCallArgumentKind::Integer fix (literals
    // fold to the Integer kind before any plan sees the call; the stager and
    // the selection arm matched only Expression-wrapped nodes). "7\n" then
    // exit 70, both engines.
    let canary = pass_canary("host/runtime_console_byte_literal_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
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
fn runtime_console_byte_replay_cross_target_canary_compiles() {
    // The final-byte certificate must replay both byte composites under each
    // settled target adapter: Linux syscall, Darwin direct import, and the
    // complete Win64 GetStdHandle + file-I/O import pair.
    let canary = pass_canary("host/runtime_console_byte_echo_exit");
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
    // Exercise every retained line-read storage shape under the Linux syscall,
    // Darwin direct-import, and Win64 two-import adapters.
    for canary_name in [
        "text/runtime_stdin_line_buffering_exit",
        "host/runtime_console_line_fixed_array_exit",
        "host/runtime_console_line_descriptor_exit",
    ] {
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
            if canary_name == "host/runtime_console_line_fixed_array_exit" {
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
                hosted_main_program_entry_build(target),
            )
            .expect("write runtime line replay target manifest");
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
    // imports (its own rung).
    let canary = pass_canary("providers/runtime_import_call_argument_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("free DllImport leaf should resolve the Leaf slot");
    assert_eq!(
        checked.selected_program_entry_machine(),
        None,
        "targetless checking must not select an authored target entry"
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
    assert!(matches!(
        &leaf_plan.rows[0].binding,
        omega_effects::provider_plan::ProviderBinding::StringBackedImportBootstrap { library, symbol }
            if library == "libSystem.B.dylib" && symbol == "_exit"
    ));

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

#[test]
fn mutual_cycle_tail_admitted_canary_runs() {
    // MR4 admission: a measured all-tail mutual pair with proven per-edge
    // decrease compiles and runs on CONSTANT stack -- every cross-machine
    // tail arm target lowers as a SetDispatchState jump in the one dispatch
    // loop. n = 100000 keeps the interpreter oracle inside its 10M-step
    // budget; the native probe ran 40M alternations on constant stack.
    let canary = pass_canary("calls/mutual_cycle_tail_admitted_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("admitted mutual tail cycle should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should run the admitted cycle to 0 (exit 70), got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-mutual-tail-admitted-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("admitted mutual tail cycle should compile natively");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("admitted mutual tail cycle should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the admitted cycle to run on constant stack (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn const_fold_unsigned_landed_ops_canary_runs() {
    // CM2 first rung: the state-values folder folds at the LANDED type (the
    // destination landing from the let's declared type). `0u32 - 2` wraps to
    // width (4294967294, not i64 -2) and the sign-sensitive ops fold unsigned:
    // `b >> 1` logical = 2147483647, `b / 3` = 1431655764, `b % 3` = 2; their
    // sum is guard-checked in the SAME state (the transition-arg delivery face
    // is a separate open bug, pinned pending). exit 71 = a fold regressed to
    // the bare-i64 window.
    let canary = pass_canary("arithmetic/const_fold_unsigned_landed_ops_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("landed-ops const-fold canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (u32 wrap + logical shift/unsigned div), got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-const-fold-landed-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("landed-ops const-fold canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("landed-ops const-fold canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("landed-ops const-fold canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected landed-type const folds (exit 70), got {:?} \
         (71 = a sign-sensitive fold ran in the bare-i64 window)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn const_fold_unsigned_shift_right_arg_canary_runs() {
    // The const-fold sign class END TO END through the transition-ARG delivery
    // path: the arg re-derives from substituted locals as a typeless nested
    // runtime binary, and the write's signedness falls back to the WRITE
    // TARGET's primitive (the callee param's u32) so `>>` emits logical shr.
    // exit 71 = the target-primitive fallback (or the landed fold) regressed.
    let canary = pass_canary("arithmetic/const_fold_unsigned_shift_right_arg_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("shift-right arg-delivery canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (u32 logical shift through the arg), got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-const-fold-shr-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("shift-right arg-delivery canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("shift-right arg canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("shift-right arg-delivery canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the arg-delivered logical shift (exit 70), got {:?} \
         (71 = the typeless arg write emitted `sar`)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn const_fold_unsigned_divide_arg_canary_runs() {
    // Unsigned DIVISION and MODULO through the transition-arg delivery path --
    // the other two sign-sensitive ops of the same class; both values ride
    // chained transition args, so both typeless writes must take the
    // target-primitive signedness fallback. exit 71 = a signed idiv slipped in.
    let canary = pass_canary("arithmetic/const_fold_unsigned_divide_arg_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("divide/mod arg-delivery canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (u32 unsigned div + mod through args), got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-const-fold-div-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("divide/mod arg-delivery canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("divide/mod arg canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("divide/mod arg-delivery canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected unsigned div/mod through the args (exit 70), got {:?} \
         (71 = a signed division/modulo fold or delivery)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn unsigned_min_max_wrapping_local_canary_runs() {
    // max/min on a `u64 in Wrapping` LOCAL: `z - 1` folds to the bit-faithful
    // but signless `-1` (u64-at-width-64 can't ride a positive i64), so the
    // operand probe finds no type and the non-table mutation write's operator
    // adjustment falls back to the WRITE TARGET's u64 -> MaxUnsigned picks
    // u64::MAX. exit 78 = a signed Max compared -1 < 5 again.
    let canary = pass_canary("arithmetic/unsigned_min_max_wrapping_local_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("unsigned min/max local canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 77,
        "interpreter oracle should exit 77 (unsigned max witness), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-minmax-local-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("unsigned min/max local canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("unsigned min/max local canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("unsigned min/max local canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected the target-fallback unsigned max (exit 77), got {:?} \
         (78 = a signed Max on the folded u64 local)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn unsigned_min_max_operand_position_canary_runs() {
    // The OPERAND-POSITION twin (carrier CR3 acceptance, promoted
    // 2026-07-18): `max(big, 5) + 0` has no write target for the max, so the
    // signedness must come from the CONSTANT itself -- the binding-capture
    // stamp + the operand-derived anonymous-destination fold carry big's
    // u64/Wrapping landing to the probe. exit 78 = a signed Max compared
    // -1 < 5 again.
    let canary = pass_canary("arithmetic/unsigned_min_max_operand_position_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("operand-position min/max canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 77,
        "interpreter oracle should exit 77 (unsigned max witness), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-minmax-operand-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("operand-position min/max canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("operand-position min/max canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("operand-position min/max canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected the landed-constant unsigned max (exit 77), got {:?} \
         (78 = a signed Max on the stamped u64 constant)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn suffix_boundary_magnitudes_canary_runs() {
    // CR4 magnitude fit -- the BOUNDARY pins: -128i8 / 127i8 / 255u8 /
    // i64::MIN all fit their suffixes (the parse-time negation fold makes
    // `-128i8` one literal valued -128) and must stay legal while 200i8 and
    // -1u8 are loud errors (the fail twins).
    let canary = pass_canary("arithmetic/suffix_boundary_magnitudes_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("suffix boundary canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (all boundary magnitudes intact), got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-suffix-boundary-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("suffix boundary canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("suffix boundary canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("suffix boundary canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the boundary magnitudes to arrive intact (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn float_value_call_return_canary_runs() {
    // The frame-slot value writer's FLOAT arm: a float value-call return
    // delivers instead of leaving the call-result slot ZII.
    let canary = pass_canary("calls/float_value_call_return_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("float return canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (passthru(3.5) == 3.5), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-float-vret-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("float return canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("float return canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the float return to deliver (exit 70), got {:?}          (71 = the call-result slot stayed ZII)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn expansion_float_local_guard_canary_runs() {
    // The float-literal guard arm in the EXPANSION path: an inlined callee's
    // float-local guard (`d == 0.0`) lowers instead of refusing.
    let canary = pass_canary("float/expansion_float_local_guard_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("expansion float guard canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (finite literal routes true), got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-exp-float-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("expansion float guard canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("expansion float guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the inlined float-local guard to lower and route true (exit 70), got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn float_value_call_runtime_arg_canary_runs() {
    // The arm-guard failure distance: an inlined callee's failed guard
    // (`d == 0.0` with d = inf - inf = NaN) lands on its SIBLING no-arm,
    // not the caller's failure trailer -- is_zeroish(inf) returns false.
    let canary = pass_canary("calls/float_value_call_runtime_arg_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("float runtime-arg canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (is_zeroish(inf) == false), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!("omega-float-varg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("float runtime-arg canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float runtime-arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the failed arm guard to route to the no-arm (exit 70), got {:?} (71 = the failure branch sailed past the sibling arm into the state trailer)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn f32_chain_per_op_rounding_canary_runs() {
    // F2c: an f32 arithmetic chain rounds per OP at f32 -- the nested
    // binary operand plans the 4-byte op from the literal's LANDED format
    // instead of defaulting float literals to F64 (which computed `addsd`
    // over f32 bit patterns and diverged from the interpreter).
    let canary = pass_canary("float/f32_chain_per_op_rounding_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("f32 chain canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (per-op f32 rounding), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-f32-chain-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("f32 chain canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("f32 chain canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected per-op f32 rounding natively (exit 70), got {:?} (71 = a double-width intermediate crept into the f32 chain)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_std_is_finite_canary_runs() {
    // std is_finite three-leg: finite -> true, runtime inf -> false,
    // runtime NaN -> false; the false legs ride the inlined arm-guard
    // failure branch (the no() arm).
    let canary = pass_canary("float/runtime_std_is_finite_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("is_finite canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (all three is_finite legs), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!("omega-std-isfin-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("is_finite canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("is_finite canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected all three is_finite legs to hold (exit 70), got {:?} (72 = finite leg, 73 = inf leg, 74 = NaN leg)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn bool_value_call_return_canary_runs() {
    // Bool-returning value calls deliver in all three faces (attached
    // let-bound, free let-bound, direct-in-guard) -- the 07-08-era
    // mis-delivery no longer reproduces; this pins the closed class.
    let canary = pass_canary("calls/bool_value_call_return_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("bool value-call canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (all three bool faces), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-bool-vcall-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bool value-call canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("bool value-call canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("bool value-call canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected all three bool value-call faces to deliver (exit 70), got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn struct_literal_transition_arg_canary_runs() {
    // The record arm of struct-literal ARG materialization (was missing:
    // a plain record arg planned nothing and the callee param stayed ZII).
    // Both field shapes: constant (int fast path) + runtime (general writer).
    let canary = pass_canary("calls/struct_literal_transition_arg_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("struct-literal arg canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (13 + 6 across both legs), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-struct-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("struct-literal arg canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("struct-literal arg canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("struct-literal arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected both struct-literal arg legs to deliver (exit 70), got {:?}          (71 = a leg's fields arrived ZII)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_indexed_element_copy_write_canary_runs() {
    // Place 1c-ii: the runtime-indexed whole-element slice write
    // (`exits[index] = e`, runtime index + runtime struct source) -- the
    // write face x86_64 refused with the zero-width blocker until the
    // materializer's shared-base indexed-target shape provided it.
    let canary = pass_canary("slices/runtime_indexed_element_copy_write_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("indexed element write canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (element 1 = {{9,4}}), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-idx-elem-write-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("indexed element write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed element write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed element write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the runtime-indexed element write to land (exit 70), got {:?}          (71 = the element missed or the source never materialized)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn suffix_landed_operand_position_canary_runs() {
    // CR4a: a width-suffixed literal is BORN LANDED, so `z - 1u64` folds at
    // the suffix's u64 landing and the operand-position max compares
    // UNSIGNED. exit 78 = the suffix was stripped and a signed max picked 5.
    let canary = pass_canary("arithmetic/suffix_landed_operand_position_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("suffix-landed canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 77,
        "interpreter oracle should exit 77 (unsigned max witness), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-suffix-landed-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("suffix-landed canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("suffix-landed canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("suffix-landed canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected the suffix-landed unsigned max (exit 77), got {:?} \
         (78 = the suffix stripped, signed Max)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn suffix_f32_single_rounding_canary_runs() {
    // F2a: the double-rounding witness -- an f32-suffixed literal parses
    // ONCE, correctly, to f32 (8388609.0); the old f64 route rounds twice
    // (8388610.0). Both engines key the landed read identically.
    let canary = pass_canary("float/suffix_f32_single_rounding_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("f32 single-rounding canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 77,
        "interpreter oracle should exit 77 (single-rounded f32 witness), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-f32-rounding-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("f32 single-rounding canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("f32 single-rounding canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected the single-rounded f32 witness (exit 77), got {:?} \
         (78 = the retired f64 double-rounding route)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn unsuffixed_f32_destination_single_rounding_canary_runs() {
    // F2b: the double-rounding witness UNSUFFIXED -- the destination's
    // declared f32 lands the format on the literal's text carrier
    // (land_float_literal_destinations, pre-fork on the typed tree), so all
    // three stamped faces (let local, field assignment, struct-literal
    // field) parse ONCE to f32 in both engines.
    let canary = pass_canary("float/unsuffixed_f32_destination_single_rounding_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("unsuffixed f32 destination canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 77,
        "interpreter oracle should exit 77 (all three faces single-rounded), got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-f32-dest-rounding-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("unsuffixed f32 destination canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("unsuffixed f32 destination canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected all three destination faces single-rounded (exit 77), got {:?} \
         (78 = let face, 79 = field-assignment face, 80 = struct-field face \
         still on the f64-then-narrow route)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn unsuffixed_f32_argument_single_rounding_canary_runs() {
    // F2c: typed call/transition parameter and return destinations stamp the
    // same f32 format before the native/interpreter fork.
    let canary = pass_canary("float/unsuffixed_f32_argument_single_rounding_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("unsuffixed f32 argument canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 77,
        "interpreter should single-round call, return, and transition faces"
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-f32-arg-rounding-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("unsuffixed f32 argument canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("unsuffixed f32 argument canary should run");
    assert_eq!(
        output.status.code(),
        Some(77),
        "expected call, return, and transition faces single-rounded (exit 77), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn f32_per_operation_rounding_canary_runs() {
    // F2d: the constant guard folder, substituted-local native operands, and
    // interpreter runtime evaluator all round after each binary32 operation.
    // At 2^24, two sequential +1 legs plateau; an f64-window fold followed by
    // one narrowing reaches 2^24+2.
    let canary = pass_canary("float/f32_per_operation_rounding_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("f32 per-operation rounding canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 77,
        "interpreter should round every f32 arithmetic node"
    );

    let build_dir = std::env::temp_dir().join(format!("omega-f32-per-op-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("f32 per-operation rounding canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("f32 per-operation rounding canary should run");
    assert_eq!(
        output.status.code(),
        Some(77),
        "expected constant and runtime f32 chains to plateau (exit 77), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn anonymous_exact_rat_const_canary_runs() {
    // F2e: anonymous decimal trees evaluate as exact rationals, then round
    // once at a destination or guard-context format. Specials are produced
    // before landing and rendered at the same format on both engines.
    let canary = pass_canary("float/anonymous_exact_rat_const_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("anonymous exact-Rat canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 77,
        "interpreter should consume exact folds"
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-float-exact-rat-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("anonymous exact-Rat canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("anonymous exact-Rat canary should run");
    assert_eq!(
        output.status.code(),
        Some(77),
        "expected all exact-Rat destination/guard/special faces (exit 77), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn finite_core_domain_range_discharge_canary_runs() {
    // F3a: the built-in value domain crosses the full type pipeline, and a
    // declared float range proves Finite when passed into a constrained state.
    let canary = pass_canary("float/finite_core_domain_range_discharge");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("Finite core-domain canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 77,
        "interpreter should preserve ranged f32 value"
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-finite-domain-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("Finite core-domain canary should compile natively");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("Finite core-domain canary should run");
    assert_eq!(
        output.status.code(),
        Some(77),
        "expected range-to-Finite discharge and value preservation (exit 77), got {:?}",
        output.status.code()
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn struct_literal_field_coercion_canary_runs() {
    // A struct-literal field init coerces the field value to the field's declared
    // width/domain (interpreter eval_struct_literal): `Point { x: a+b }` with
    // `a+b`=300 into a u8 field reads 44. The field is read DIRECTLY (`p.x`), so
    // the coercion must happen at construction. exit 71 = field carried raw 300.
    let canary = pass_canary("arithmetic/struct_literal_field_coercion");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("struct-literal coercion canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (struct field truncates to u8 width), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-struct-lit-coerce-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("struct-literal coercion canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("struct-literal coercion canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("struct-literal coercion canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected struct field init to truncate to u8 width (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn array_element_write_width_domain_canary_runs() {
    // An array-element write coerces the stored value to the element WIDTH and the
    // ARRAY's arithmetic DOMAIN (interpreter assignment_target_coercion): a u8
    // element given `a+b`=300 truncates to 44; a `[u8;N] in Saturating` element
    // clamps to 255. exit 72 = wrap element wrong; 73 = saturating did not clamp.
    let canary = pass_canary("arithmetic/array_element_write_width_domain");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("array-element coercion canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (element width truncation + array Saturating clamp), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-array-elem-coerce-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("array-element coercion canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("array-element coercion canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("array-element coercion canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected element width truncation + array Saturating clamp (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn int_transition_arg_width_wrap_canary_runs() {
    // An integer argument is wrapped to the param's declared width at the binding
    // (interpreter bind_frame apply_arithmetic_domain), matching native's
    // truncating store at the call boundary: `a+b`=300 into a u8 param reads 44.
    // exit 71 = the interpreter carried the un-wrapped 300 into the u8 param.
    let canary = pass_canary("arithmetic/int_transition_arg_width_wrap");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("int transition-arg width canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (int arg wraps to u8 param width), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-int-transition-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("int transition-arg width canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("int transition-arg canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("int transition-arg width canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected int arg to wrap to the u8 param width (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn f32_transition_arg_rounding_canary_runs() {
    // An f32 passed through a transition ARGUMENT rounds to f32 at the param
    // binding (interpreter bind_frame), not just at stores: accumulating via
    // inline `+ 1.0` args past 2^24 plateaus at 16777216, matching native.
    // exit 71 = the interpreter carried f64 through params to 16777218.
    let canary = pass_canary("arithmetic/f32_transition_arg_rounding");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("f32 transition-arg canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (f32 rounds at param binding), got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-f32-transition-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("f32 transition-arg canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("f32 transition-arg canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f32 transition-arg to round at param binding (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn f32_field_store_rounding_canary_runs() {
    // An f32 field/local rounds each stored result to f32 (interpreter store
    // rounding, matching native SSE): stepping past 2^24 by `+ 1.0` plateaus at
    // 16777216. Regression lock for the interpreter Value::Float store-rounding
    // fix; exit 71 = the interpreter kept f64 and over-accumulated to 16777218.
    let canary = pass_canary("arithmetic/f32_field_store_rounding");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("f32 field store canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (f32 store rounds, plateaus at 16777216), got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-f32-field-store-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("f32 field store canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("f32 field store canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f32 field store to round to f32 and plateau (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn const_fold_cast_signedness_canary_runs() {
    // Const-folded integer casts stay correct across truncation + sign
    // reinterpret, including a wrapping-produced high-bit value cast to a signed
    // type (`(0u32-1) as i8 == -1`). Positive contrast to the const-fold
    // arithmetic miscompile class: a cast node carries its target type, so
    // folding never drops width/signedness. Guards against a future arithmetic-
    // folder fix breaking cast folding. exit 71 = a fold dropped width/sign.
    let canary = pass_canary("arithmetic/const_fold_cast_signedness");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("const-fold cast canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 for const-folded casts, got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-const-fold-cast-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("const-fold cast canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("const-fold cast canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("const-fold cast canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected const-folded casts (truncate + sign reinterpret) to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}
