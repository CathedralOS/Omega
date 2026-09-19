use super::fixture_roster;
use super::{
    assert_selected_operator_native_physical_call, assert_selected_operator_terminal_call,
    fused_service_field_mut, fused_service_parameter_mut, fused_service_parameter_plan_mut,
};
use crate::{
    Command, CompileRequest, CompilerOptions, RequestedCompileProduct,
    compile_reviewed_repository_fixture, compile_rooted_backend_canary_without_output_for_target,
    compile_rooted_canary_for_native_host, fs, interpret, pass_canary,
};
use compiler::CheckedCompileRequest;

#[test]
fn checked_operator_fragment_publication_retains_complete_d29_call_interval() {
    let canary = pass_canary(fixture_roster::CHECKED_OPERATOR_FRAGMENT_PUBLICATION);
    let reports = assert_selected_operator_native_physical_call(
        &canary,
        "checked operator fragment publication",
        boundary_applications::BoundaryApplicationRealizationRole::NongenericCheckedBody,
        0,
    );
    for (report, opcode_prefix) in reports.iter().zip([1, 0]) {
        let native = report.retained_native_artifact().unwrap();
        let physical = native.physical_evidence().unwrap();
        assert!(physical.projection().boundary_occurrences().is_empty());
        let [child] = physical.children() else {
            panic!("the ordinary returning entry must retain exactly one D29 child")
        };
        let native_realization::NativePhysicalOccurrence::Operator(identity) = child.occurrence()
        else {
            panic!("the physical child must name the checked operator")
        };
        let [occurrence] = physical.projection().operator_occurrences() else {
            panic!("the projection must retain exactly one checked operator")
        };
        assert_eq!(identity, occurrence.identity());
        let caller = native.object().entry_function();
        assert_eq!(caller.machine, occurrence.machine());
        assert!(caller.internal_unit_scalar_calls.is_empty());
        assert!(caller.unit_scalar_homes.is_empty());
        let [call] = caller.unit_call_stacks.as_slice() else {
            panic!("the shared object must retain exactly one physical call")
        };
        let rows = native
            .object()
            .semantic_code_attribution()
            .iter()
            .filter(|row| {
                row.machine == occurrence.machine()
                    && row.attribution.operation_ordinal == occurrence.operation_ordinal()
            })
            .collect::<Vec<_>>();
        let [row] = rows.as_slice() else {
            panic!("call transport and opcode must have one coalesced attribution interval")
        };
        assert_eq!(child.machine_span().offset(), row.attribution.code_offset);
        assert_eq!(
            child.machine_span().byte_count(),
            row.attribution.byte_count
        );
        assert_eq!(child.object_span().offset(), row.text_offset);
        assert_eq!(child.object_span().byte_count(), row.attribution.byte_count);
        let opcode = call.text_offset.checked_sub(opcode_prefix).unwrap();
        assert!(row.text_offset <= opcode);
        assert!(call.text_offset + 4 <= row.text_offset + row.attribution.byte_count);
        // Coalesced transport belongs to the child when allocation emits it.
        // Constants may already occupy argument registers and an unused result
        // needs no transfer, so a bare opcode is also the complete interval.
        assert!(row.attribution.byte_count >= 4 + opcode_prefix);
    }
}

#[test]
fn runtime_adapter_dispatch_exit_canary_runs() {
    // PRV4 adapter dispatch: the boundary-trait call rewrites to the unique
    // satisfying adapter in both engines; the interpreter leg rides the
    // differential row.
    let canary = pass_canary(fixture_roster::RUNTIME_ADAPTER_DISPATCH_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
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
    let canary = pass_canary(fixture_roster::CHECKED_BOUNDARY_OPERATOR_DISPATCH_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("checked boundary-operator canary should compile to checked trees");
    assert!(!checked.facts.operators.boundary_applications.is_empty());
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter dispatches the selected checked operator body; error: {:?}",
        outcome.error,
    );

    assert_selected_operator_terminal_call(&canary, "checked boundary-operator canary", true);
}

#[test]
fn checked_boundary_operator_const_length_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::CHECKED_BOUNDARY_OPERATOR_CONST_LENGTH_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("checked boundary-operator const-length canary should compile to checked trees");
    assert!(!checked.facts.operators.boundary_applications.is_empty());
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "the selected provider body folded the array length to 9, so byte 8 writes; \
         builtin `%` would fold 1 and reject the index: {:?}",
        outcome.error,
    );
}

#[test]
fn checked_named_boundary_operator_const_length_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::CHECKED_NAMED_BOUNDARY_OPERATOR_CONST_LENGTH_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("named boundary-operator const-length canary should compile to checked trees");
    assert!(!checked.facts.operators.boundary_applications.is_empty());
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "the selected provider body folded the array length to 7, so byte 6 writes; \
         builtin `%` would fold 1 and reject the index: {:?}",
        outcome.error,
    );
}

#[test]
fn provider_boundary_range_endpoint_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::PROVIDER_BOUNDARY_RANGE_ENDPOINT_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("provider boundary range-endpoint canary should compile to checked trees");
    assert!(!checked.facts.operators.boundary_applications.is_empty());
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "the selected provider body folded the endpoint to 7, so `take_bounded(7)` checks; \
         builtin `%` would fold 1 and reject the argument: {:?}",
        outcome.error,
    );
}

#[test]
fn checked_fixed_operator_dispatch_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::CHECKED_FIXED_OPERATOR_DISPATCH_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("checked fixed-token operator canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter dispatches the selected fixed-token operator body; error: {:?}",
        outcome.error,
    );
    assert_selected_operator_terminal_call(&canary, "checked fixed-token operator canary", false);
}

#[test]
fn checked_boundary_operator_physical_custody_canary_compiles() {
    let canary = pass_canary(fixture_roster::CHECKED_BOUNDARY_OPERATOR_PHYSICAL_CUSTODY);
    assert_selected_operator_native_physical_call(
        &canary,
        "checked boundary-operator physical-custody canary",
        boundary_applications::BoundaryApplicationRealizationRole::NongenericCheckedBody,
        0,
    );

    let dynamic_exit = pass_canary(fixture_roster::CHECKED_BOUNDARY_OPERATOR_DISPATCH_EXIT);
    let diagnostics =
        compile_rooted_backend_canary_without_output_for_target(&dynamic_exit, "linux_x86_64")
            .expect_err("the immediate-only exit settlement must still reject a call-result input");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("InvalidHostedExitProcessShape") })
    );
}

#[test]
fn checked_fixed_operator_physical_custody_canary_compiles() {
    let canary = pass_canary(fixture_roster::CHECKED_FIXED_OPERATOR_PHYSICAL_CUSTODY);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("fixed-token physical-custody canary should check");
    let selected_subtracts = checked
        .facts
        .operators
        .resolved_uses()
        .filter(|operator_use| operator_use.spelling == language_core::OperatorSpelling::Subtract)
        .filter_map(|operator_use| checked.facts.operators.selected_candidate(operator_use))
        .filter(|candidate| candidate.is_boundary)
        .count();
    assert_eq!(selected_subtracts, 1, "one exact boundary `-` selection");
    assert_selected_operator_native_physical_call(
        &canary,
        "checked fixed-token operator physical-custody canary",
        boundary_applications::BoundaryApplicationRealizationRole::NongenericCheckedBody,
        0,
    );

    let dynamic_exit = pass_canary(fixture_roster::CHECKED_FIXED_OPERATOR_DISPATCH_EXIT);
    let diagnostics =
        compile_rooted_backend_canary_without_output_for_target(&dynamic_exit, "linux_x86_64")
            .expect_err("the immediate-only exit settlement must reject a fixed-token call result");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("InvalidHostedExitProcessShape") })
    );
}

#[test]
fn specialized_boundary_operator_physical_custody_canary_compiles() {
    let canary = pass_canary(fixture_roster::SPECIALIZED_BOUNDARY_OPERATOR_PHYSICAL_CUSTODY);
    assert_selected_operator_native_physical_call(
        &canary,
        "specialized boundary-operator physical-custody canary",
        boundary_applications::BoundaryApplicationRealizationRole::SpecializedCheckedBody,
        0,
    );
}

#[test]
fn specialized_fixed_operator_physical_custody_canary_compiles() {
    let canary = pass_canary(fixture_roster::SPECIALIZED_FIXED_OPERATOR_PHYSICAL_CUSTODY);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("specialized fixed-token physical-custody canary should check");
    let selected_remainders = checked
        .facts
        .operators
        .resolved_uses()
        .filter(|operator_use| operator_use.spelling == language_core::OperatorSpelling::Modulo)
        .filter_map(|operator_use| checked.facts.operators.selected_candidate(operator_use))
        .filter(|candidate| candidate.is_boundary)
        .count();
    assert_eq!(
        selected_remainders, 1,
        "one exact generic boundary `%` selection"
    );
    assert_selected_operator_native_physical_call(
        &canary,
        "specialized fixed-token operator physical-custody canary",
        boundary_applications::BoundaryApplicationRealizationRole::SpecializedCheckedBody,
        0,
    );

    let substituted = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        Some("linux_x86_64"),
    ))
    .expect("specialized fixed-token false twin should reach checked custody");
    let selected_provider_plans = substituted.selected_provider_plans().clone();
    let mut substituted = substituted.into_program();
    let [application] = substituted
        .facts
        .operators
        .boundary_applications
        .as_mut_slice()
    else {
        panic!("one specialized fixed-token application")
    };
    let [checked_trees::CheckedBoundaryOperatorApplicationArgument::Type { binder_symbol, .. }] =
        application.arguments.as_mut_slice()
    else {
        panic!("one specialized fixed-token type argument")
    };
    *binder_symbol = symbols::SymbolHandle::invalid();
    let diagnostics =
        selected_dispatch::derive_checked_specialized_operator_application_realizations(
            &substituted,
            &selected_provider_plans,
        )
        .expect_err("a substituted fixed-token D29 application must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("resolves to 0 exact specializations for one application")
        }),
        "substituted fixed-token diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn specialized_structural_fixed_operator_terminal_custody_canary_compiles() {
    let canary =
        pass_canary(fixture_roster::SPECIALIZED_STRUCTURAL_FIXED_OPERATOR_TERMINAL_CUSTODY);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        Some("linux_x86_64"),
    ))
    .expect("structural fixed-token custody canary should check");
    let produced = terminal_production::TerminalProductionRequest::new(&checked, "exercise")
        .produce_checked_artifact()
        .expect("structural fixed-token application should reach Terminal");
    produced
        .boundary_operator_scope()
        .validate_for_artifact(produced.artifact())
        .expect("checked D29 scope should bind the exact Terminal artifact");
    let [application] = produced.boundary_operator_scope().applications() else {
        panic!("one exact structural fixed-token application")
    };
    let [
        checked_trees::CheckedBoundaryOperatorApplicationArgument::Type { .. },
        checked_trees::CheckedBoundaryOperatorApplicationArgument::Const {
            binder_ordinal,
            value,
            ..
        },
    ] = application.arguments.as_slice()
    else {
        panic!("one exact type+const structural application")
    };
    assert_eq!(*binder_ordinal, 1);
    assert_eq!(
        value,
        &language_semantics::const_value::CanonicalConstIdentity::integer("u64", 4),
    );
    let [occurrence] = produced.boundary_operator_scope().occurrences() else {
        panic!("one exact checked-to-Terminal structural operator occurrence")
    };
    let module = terminal_codec::decode_module(produced.artifact().semantic_bytes())
        .expect("structural fixed-token Terminal semantics should decode");
    assert!(module.machines.iter().any(|machine| {
        machine.blocks.iter().any(|block| {
            block.operations.iter().any(|operation| {
                operation.id == occurrence.terminal_operation()
                    && matches!(
                        operation.kind,
                        terminal_psi::OperationKind::CallStructuralScalar { .. }
                    )
            })
        })
    }));
    let realizations =
        selected_dispatch::derive_checked_specialized_operator_application_realizations(
            &checked,
            checked.selected_provider_plans(),
        )
        .expect("structural fixed-token application should rejoin its specialization");
    let [realization] = realizations.as_slice() else {
        panic!("one exact specialized structural realization")
    };
    assert_eq!(realization.application_arguments, application.arguments);

    let mut substituted = checked.clone().into_program();
    let [substituted_application] = substituted
        .facts
        .operators
        .boundary_applications
        .as_mut_slice()
    else {
        panic!("one structural fixed-token false-twin application")
    };
    let [
        _,
        checked_trees::CheckedBoundaryOperatorApplicationArgument::Const { binder_symbol, .. },
    ] = substituted_application.arguments.as_mut_slice()
    else {
        panic!("one structural type+const false-twin application")
    };
    *binder_symbol = symbols::SymbolHandle::invalid();
    let diagnostics =
        selected_dispatch::derive_checked_specialized_operator_application_realizations(
            &substituted,
            checked.selected_provider_plans(),
        )
        .expect_err("a substituted structural const application must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("resolves to 0 exact specializations for one application")
    }));

    let mut drifted_plan = checked.clone().into_program();
    let [selected_structural] = drifted_plan
        .facts
        .flow
        .terminal_structural_scalar_returns
        .selected_operator_machines
        .as_mut_slice()
    else {
        panic!("one selected structural Terminal plan")
    };
    selected_structural.provider_plan_commitment =
        checked_trees::CheckedProviderPlanCommitment::default();
    selected_dispatch::validate_selected_operator_terminal_custody(
        &drifted_plan,
        checked.selected_provider_plans(),
    )
    .expect_err("a substituted structural selected plan must reject");
}

#[test]
fn nested_checked_boundary_operator_physical_custody_canary_compiles() {
    let canary = pass_canary(fixture_roster::NESTED_CHECKED_BOUNDARY_OPERATOR_PHYSICAL_CUSTODY);
    assert_selected_operator_native_physical_call(
        &canary,
        "nested checked boundary-operator physical-custody canary",
        boundary_applications::BoundaryApplicationRealizationRole::NongenericCheckedBody,
        1,
    );

    let missing_helper_contract = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        Some("linux_x86_64"),
    ))
    .expect("nested checked-body false twin should reach checked custody");
    let entry = missing_helper_contract
        .selected_program_entry_machine()
        .expect("nested checked-body false twin retains its entry")
        .to_owned();
    let mut missing_helper_contract = missing_helper_contract.into_program();
    let helper = missing_helper_contract
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "retain")
        .expect("nested checked-body helper")
        .symbol;
    missing_helper_contract
        .facts
        .contract_plans
        .machines
        .iter_mut()
        .find(|contract| contract.machine == helper)
        .expect("nested checked-body helper contract")
        .closed_scalar_values = Default::default();
    let rejected =
        terminal_production::TerminalProductionRequest::new(&missing_helper_contract, &entry)
            .produce_with_callback_custody(())
            .expect_err("a nested scalar helper cannot lose its independently replayable contract");
    assert!(matches!(
        rejected.error(),
        terminal_production::TerminalArtifactProductionError::Lowering(
            checked_trees_to_lowered_psi::LoweringError::Unsupported(
                "machine must have exactly one requires and one ensures clause"
            )
        )
    ));
}

#[test]
fn runtime_result_domain_requirement_overload_exit_canary_runs() {
    // Result-domain identity survives requirement collection, provider-plan
    // selection, checked adapter dispatch, and both executable engines.
    let canary = pass_canary(fixture_roster::RUNTIME_RESULT_DOMAIN_REQUIREMENT_OVERLOAD_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
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
    let canary = pass_canary(fixture_roster::PROVIDER_TYPE_SLOT_SELECTED);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
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
fn fused_service_erasure_rejoins_typed_source_and_selected_plan() {
    let canary = pass_canary(fixture_roster::SERVICE_FUSED_ERASURE_COMPILE);
    let baseline = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("fused Service fixture should reach checked trees");
    let provenance = baseline.selected_provider_provenance().to_vec();
    assert!(
        baseline.selected_program_entry().is_none(),
        "targetless checking must not establish an authored target root"
    );
    selected_dispatch::validate_fused_service_terminal_custody(&baseline, &provenance)
        .expect("exact typed Service, Fused provenance, and selected plan should rejoin");
    checked_trees_to_lowered_psi::lower_machine(&baseline, "Main::main")
        .expect("authorized fused Service carrier should erase during Terminal lowering");
    let parameter_plan = baseline
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| {
            plan.structural_parameters
                .iter()
                .any(|parameter| parameter.fused_service_erasure.is_some())
        })
        .expect("direct Service parameter machine should retain a checked Unit plan");
    let [parameter] = parameter_plan.structural_parameters.as_slice() else {
        panic!("direct Service parameter machine should retain one structural parameter")
    };
    let [scalar_parameter] = parameter_plan.scalar_parameters.as_slice() else {
        panic!("direct Service parameter machine should retain one scalar parameter")
    };
    let receipt = parameter
        .fused_service_erasure
        .as_ref()
        .expect("direct Service parameter should retain exact Fused custody");
    assert_eq!(parameter.multiplicity, language_core::Multiplicity::Affine);
    assert_eq!(
        parameter.access,
        checked_trees::CheckedStructuralAccess::Owned
    );
    assert_eq!(parameter.qualifications.len(), 1);
    assert_eq!(scalar_parameter.source_position, 1);
    assert_eq!(
        scalar_parameter.primitive_type,
        typed_trees::types::PrimitiveType::I32
    );
    assert!(receipt.source_parameter.is_valid());
    let parameter_machine = baseline
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == parameter_plan.machine)
        .expect("direct Service parameter machine remains in typed trees");
    let [parameter_state] = baseline.typed.machine_states(parameter_machine) else {
        panic!("direct Service parameter machine should have one state")
    };
    let [
        typed_trees::statement::StatementNode::Call(first_settled_call),
        typed_trees::statement::StatementNode::Call(second_settled_call),
    ] = baseline
        .typed
        .statement_table
        .statements(parameter_state.statement_nodes)
    else {
        panic!("direct Service parameter machine should retain two settled calls")
    };
    for settled_call in [first_settled_call, second_settled_call] {
        assert!(
            !settled_call.receiver_symbol.is_valid(),
            "each exact Service parameter receiver should settle to direct adapter dispatch"
        );
    }
    assert_eq!(
        parameter_plan
            .operations
            .iter()
            .filter(|operation| matches!(
                operation,
                checked_trees::CheckedUnitEffectOperationPlan::BoundaryCall { .. }
                    | checked_trees::CheckedUnitEffectOperationPlan::BoundaryScalarCall { .. }
            ))
            .count(),
        2,
        "both direct Service calls must retain ordered checked operations"
    );
    let lowered_parameter =
        checked_trees_to_lowered_psi::lower_machine(&baseline, "ParameterHarness::run").expect(
            "authorized direct Service and scalar parameters should lower through Terminal",
        );
    let terminal_parameter_machine = lowered_parameter
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered_parameter.semantic_module.entry)
        .expect("lowered direct Service parameter machine should remain the Terminal entry");
    let [terminal_scalar_parameter] = terminal_parameter_machine.parameters.as_slice() else {
        panic!("Terminal direct Service helper should retain one scalar parameter")
    };
    assert_eq!(
        terminal_scalar_parameter.scalar_type,
        semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
                .unwrap()
        )
    );
    let scalar_call_arguments = terminal_parameter_machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::BoundaryCall { arguments, .. } => Some(arguments),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(scalar_call_arguments.len(), 2);
    assert!(
        scalar_call_arguments
            .iter()
            .all(|arguments| arguments.as_slice() == [terminal_scalar_parameter.id]),
        "both ordered Service calls must consume the exact Terminal scalar parameter"
    );

    let mut parameter_downgrade = baseline.clone().into_program();
    fused_service_parameter_mut(&mut parameter_downgrade).fused_service_erasure = None;
    let parameter_downgrade_error = selected_dispatch::validate_fused_service_terminal_custody(
        &parameter_downgrade,
        &provenance,
    )
    .expect_err("removing a direct Service parameter receipt must reject");
    assert!(parameter_downgrade_error.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("lost its exact Fused erasure settlement")
    }));
    let raw_lowering_error =
        checked_trees_to_lowered_psi::lower_machine(&parameter_downgrade, "ParameterHarness::run")
            .expect_err("raw Terminal lowering must also reject a removed Service receipt");
    assert!(
        raw_lowering_error
            .to_string()
            .contains("lost its Fused erasure receipt")
    );

    let mut scalar_parameter_removal = baseline.clone().into_program();
    fused_service_parameter_plan_mut(&mut scalar_parameter_removal)
        .scalar_parameters
        .clear();
    let scalar_parameter_error = selected_dispatch::validate_fused_service_terminal_custody(
        &scalar_parameter_removal,
        &provenance,
    )
    .expect_err("removing a routed Service scalar parameter must reject");
    assert!(scalar_parameter_error.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("scalar parameters do not rejoin the exact immutable typed source partition")
    }));
    let raw_scalar_parameter_error = checked_trees_to_lowered_psi::lower_machine(
        &scalar_parameter_removal,
        "ParameterHarness::run",
    )
    .expect_err("raw Terminal lowering must reject a removed routed Service scalar parameter");
    assert!(
        raw_scalar_parameter_error.to_string().contains(
            "direct Unit scalar parameters do not rejoin the exact typed source partition"
        )
    );

    let mut parameter_symbol_substitution = baseline.clone().into_program();
    fused_service_parameter_mut(&mut parameter_symbol_substitution)
        .fused_service_erasure
        .as_mut()
        .expect("parameter receipt")
        .source_parameter = symbols::SymbolHandle::invalid();
    let parameter_symbol_error = selected_dispatch::validate_fused_service_terminal_custody(
        &parameter_symbol_substitution,
        &provenance,
    )
    .expect_err("a substituted Service parameter symbol must reject");
    assert!(parameter_symbol_error.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("substituted its exact typed parameter symbol")
            || diagnostic
                .message
                .contains("rejoins 0 exact typed parameters")
    }));

    let mut parameter_access_substitution = baseline.clone().into_program();
    fused_service_parameter_mut(&mut parameter_access_substitution).access =
        checked_trees::CheckedStructuralAccess::SharedBorrow;
    let parameter_access_error = selected_dispatch::validate_fused_service_terminal_custody(
        &parameter_access_substitution,
        &provenance,
    )
    .expect_err("a borrowed checked Service parameter must reject");
    assert!(parameter_access_error.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("not the exact direct owned affine source parameter")
    }));

    let mut parameter_operation_removal = baseline.clone().into_program();
    let parameter_machine = parameter_operation_removal
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| {
            plan.structural_parameters
                .iter()
                .any(|parameter| parameter.fused_service_erasure.is_some())
        })
        .expect("direct Service parameter machine");
    let operation_index = parameter_machine
        .operations
        .iter()
        .position(|operation| {
            matches!(
                operation,
                checked_trees::CheckedUnitEffectOperationPlan::BoundaryCall { .. }
                    | checked_trees::CheckedUnitEffectOperationPlan::BoundaryScalarCall { .. }
            )
        })
        .expect("direct Service boundary operation");
    parameter_machine.operations.remove(operation_index);
    let parameter_operation_error = selected_dispatch::validate_fused_service_terminal_custody(
        &parameter_operation_removal,
        &provenance,
    )
    .expect_err("removing one ordered Service operation must reject");
    assert!(parameter_operation_error.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("expected one ordered operation per call")
    }));

    let mut classifier_twins = baseline.clone().into_program();
    let service_field_type = {
        let main = classifier_twins
            .typed
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Main")
            .expect("fixture should retain Main");
        classifier_twins
            .typed
            .data_members(main)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.name.as_str() == "service" => {
                    Some(field.type_reference)
                }
                _ => None,
            })
            .expect("fixture should retain Main.service")
    };
    let (service_base, bound_constraints) = match classifier_twins
        .typed
        .type_reference_table
        .type_reference(service_field_type)
        .clone()
    {
        typed_trees::types::TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => (base_type, constraints),
        other => panic!("Service fixture should retain its Bound shell, got {other:?}"),
    };
    let reference_base = classifier_twins.typed.type_reference_table.insert(
        typed_trees::types::TypeReferenceNode::Reference {
            referee: service_base,
            access: language_core::ReferenceAccess::Shared,
            lifetime: None,
        },
    );
    let reference_wrapped = classifier_twins.typed.type_reference_table.insert(
        typed_trees::types::TypeReferenceNode::Constrained {
            base_type: reference_base,
            constraints: bound_constraints,
        },
    );
    let reference_error = typed_trees::service::classify_exact_bound_service_carrier(
        &classifier_twins.typed,
        reference_wrapped,
    )
    .expect_err("a reference-wrapped Service false twin must reject");
    assert!(reference_error.contains("routes only a closed `Service<R>` carrier"));

    let mut constraints = classifier_twins
        .typed
        .type_reference_table
        .constraints(bound_constraints)
        .to_vec();
    constraints.push(typed_trees::types::TypeConstraintNode::Named(
        typed_trees::name::Identifier::generated_static("extra"),
    ));
    let constraints = classifier_twins
        .typed
        .type_reference_table
        .insert_constraints(constraints);
    let extra_constraint = classifier_twins.typed.type_reference_table.insert(
        typed_trees::types::TypeReferenceNode::Constrained {
            base_type: service_base,
            constraints,
        },
    );
    let constraint_error = typed_trees::service::classify_exact_bound_service_carrier(
        &classifier_twins.typed,
        extra_constraint,
    )
    .expect_err("a Service carrier with an extra constraint must reject");
    assert!(constraint_error.contains("may carry only the exact toolchain-owned `Bound` domain"));

    let mut downgraded = baseline.clone().into_program();
    let provider_type_identity = match fused_service_field_mut(&mut downgraded) {
        checked_trees::CheckedUnitStructuralFieldType::FusedServiceBacked {
            provider_type_identity,
            ..
        } => provider_type_identity.clone(),
        _ => unreachable!(),
    };
    *fused_service_field_mut(&mut downgraded) =
        checked_trees::CheckedUnitStructuralFieldType::ProviderBacked {
            provider_type_identity,
        };
    let downgrade =
        selected_dispatch::validate_fused_service_terminal_custody(&downgraded, &provenance)
            .expect_err("a fused-to-legacy field downgrade must reject");
    assert!(downgrade.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("lost its exact Fused erasure settlement")
    }));

    let mut digest_substitution = baseline.clone().into_program();
    let (requirement, substituted_digest) = match fused_service_field_mut(&mut digest_substitution)
    {
        checked_trees::CheckedUnitStructuralFieldType::FusedServiceBacked { erasure, .. } => {
            erasure.provider_plan_digest[0] ^= 1;
            (erasure.requirement, erasure.provider_plan_digest)
        }
        _ => unreachable!(),
    };
    digest_substitution
        .typed
        .fused_service_erasures
        .iter_mut()
        .find(|authorization| authorization.requirement == requirement)
        .expect("fixture should retain one matching Fused authorization")
        .provider_plan_digest = substituted_digest;
    let digest_error = selected_dispatch::validate_fused_service_terminal_custody(
        &digest_substitution,
        &provenance,
    )
    .expect_err("a jointly substituted receipt and authorization digest must reject");
    assert!(digest_error.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rejoins 0 exact Fused selected-provider plans")
    }));

    let mut requirement_substitution = baseline.clone().into_program();
    let other = requirement_substitution
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Other")
        .expect("fixture should retain the valid false-twin boundary")
        .symbol;
    match fused_service_field_mut(&mut requirement_substitution) {
        checked_trees::CheckedUnitStructuralFieldType::FusedServiceBacked { erasure, .. } => {
            erasure.requirement = other
        }
        _ => unreachable!(),
    }
    let requirement_error = selected_dispatch::validate_fused_service_terminal_custody(
        &requirement_substitution,
        &provenance,
    )
    .expect_err("a valid-boundary requirement substitution must reject");
    assert!(requirement_error.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("substituted its boundary requirement")
    }));

    let mut missing_authority = baseline.clone().into_program();
    missing_authority.typed.fused_service_erasures.clear();
    let authority_error =
        selected_dispatch::validate_fused_service_terminal_custody(&missing_authority, &provenance)
            .expect_err("a missing compiler-owned Fused authorization must reject");
    assert!(authority_error.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("lacks compiler-owned Fused erasure authority")
    }));
}

#[test]
fn selected_program_entry_retains_one_exact_fused_service_establishment() {
    let canary = pass_canary(fixture_roster::SERVICE_FUSED_ROOT_ESTABLISHMENT);
    let baseline = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        Some("linux_x86_64"),
    ))
    .expect("selected Fused Service root should reach checked trees");
    let selected = baseline
        .selected_program_entry()
        .expect("target compilation should retain its selected ProgramEntry");
    let [establishment] = selected.fused_service_establishments() else {
        panic!("selected Main root should retain one direct Service establishment")
    };
    assert_eq!(selected.source_signature().machine_name(), "Main::main");
    assert_eq!(establishment.field_identity(), "service");
    assert!(establishment.requirement_identity().starts_with("Ping#"));
    assert_eq!(establishment.bound_domain_identity(), "Bound");
    assert_eq!(
        establishment.source_signature_identity(),
        selected.source_signature().identity()
    );
    assert_eq!(
        establishment.target_slot(),
        target::TargetProfile::LinuxX64.program_entry_slot()
    );
    assert!(establishment.carrier_type_identity().contains("Service"));
    assert!(establishment.carrier_base_identity().contains("Service"));

    let provenance = baseline.selected_provider_provenance().to_vec();
    let derived = selected_dispatch::derive_fused_program_entry_establishments(
        &baseline,
        selected.source_signature(),
        &provenance,
    )
    .expect("the selected root receipt should independently rederive");
    assert_eq!(derived, selected.fused_service_establishments());

    // An unselected Service field prevents shape collection, so diagnosing
    // the missing attachment first hides the provider the project must supply.
    let mut unselected = baseline.clone().into_program();
    unselected.typed.fused_service_erasures.clear();
    unselected.facts.flow.terminal_unit_effects.machines.clear();
    unselected
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .clear();
    let missing_provider = selected_dispatch::derive_fused_program_entry_establishments(
        &unselected,
        selected.source_signature(),
        &[],
    )
    .expect_err("an unselected entry service must reject before the missing attachment");
    assert_eq!(missing_provider.len(), 1);
    assert_eq!(
        missing_provider[0].message,
        "selected ProgramEntry Service field `Main::service` requires a selected Fused provider for boundary `Ping`",
    );

    let mut substituted = baseline.clone().into_program();
    match fused_service_field_mut(&mut substituted) {
        checked_trees::CheckedUnitStructuralFieldType::FusedServiceBacked { erasure, .. } => {
            erasure.provider_plan_digest[0] ^= 1
        }
        _ => unreachable!(),
    }
    assert!(
        selected_dispatch::derive_fused_program_entry_establishments(
            &substituted,
            selected.source_signature(),
            &provenance,
        )
        .is_err(),
        "a selected-root Terminal receipt substitution must reject"
    );

    let report = compiler::compile(
        CompileRequest::new(CompilerOptions {
            root_path: canary.join("main.omg"),
            build_dir: None,
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("selected Fused root should produce a retained Terminal proposal");
    let artifact = report
        .artifact()
        .expect("Terminal compilation should retain its canonical artifact");
    let proposal = report
        .terminal_native_realization_proposal()
        .expect("Terminal compilation should retain its native proposal");
    let [proposed] = proposal.program_entry().fused_service_establishments() else {
        panic!("Terminal proposal should retain one exact root establishment")
    };
    assert_eq!(proposed, establishment);
    proposal
        .validate_for_artifact(artifact)
        .expect("Terminal proposal should independently replay root establishment");
}

const CHECKED_BOUNDARY_REQUIREMENT_DISPATCH_EXIT: &str =
    "providers/checked_boundary_requirement_dispatch_exit";
const CHECKED_BOUNDARY_REQUIREMENT_TERMINAL_EXIT: &str =
    "providers/checked_boundary_requirement_terminal_exit";

/// The requirement-side settlement row every direct-call position retains:
/// one dispatch row joining the requirement's entry state to the selected
/// adapter's entry state, keyed on the exact requirement owner.
fn requirement_dispatch_row(
    checked: &compiler::CheckedCompilation,
    label: &str,
) -> (
    symbols::SymbolHandle,
    symbols::SymbolHandle,
    symbols::SymbolHandle,
) {
    let entry_symbol = |machine_name: &str| {
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
            .unwrap_or_else(|| panic!("{label} should retain machine `{machine_name}`"));
        checked
            .typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{label} machine `{machine_name}` has an entry"))
            .symbol
    };
    let requirement = entry_symbol("CheckedMath::offset_zero");
    let realization = entry_symbol("CheckedMathProvider::offset_zero_impl");
    let owner = checked
        .typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "CheckedMath")
        .unwrap_or_else(|| panic!("{label} should retain the requirement owner"))
        .symbol;
    let rows = checked
        .facts
        .boundary_adapter_dispatch
        .iter()
        .filter(|row| row.requirement == requirement)
        .collect::<Vec<_>>();
    let [row] = rows.as_slice() else {
        panic!("{label} should settle one requirement dispatch row: {rows:?}");
    };
    assert_eq!(
        row.receiver, owner,
        "{label} row is keyed on the exact owner"
    );
    assert_eq!(row.realization_state, realization);
    assert!(!row.forward_receiver && row.family_tuple.is_empty());
    (requirement, realization, owner)
}

/// The requirement-side association a settled direct call retains: one
/// dispatch row joining the requirement's entry state to the selected
/// adapter's entry state, the settled call redirected to that adapter, and the
/// selected-dispatch journal restoring the authored call to the requirement.
fn assert_selected_requirement_association(
    checked: &compiler::CheckedCompilation,
    label: &str,
) -> (symbols::SymbolHandle, symbols::SymbolHandle) {
    let (requirement, realization, _owner) = requirement_dispatch_row(checked, label);
    let settled_calls = checked
        .typed
        .expression_table
        .expression_entries()
        .filter_map(|(handle, expression)| match expression {
            typed_trees::expression::ExpressionNode::Call(call)
                if call.target_symbol == realization && !call.receiver.is_valid() =>
            {
                Some(handle)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [settled_call] = settled_calls.as_slice() else {
        panic!("{label} should redirect exactly one direct call to the adapter entry");
    };
    let authored = checked
        .pre_selected_dispatch_source_trees(&checked.typed)
        .unwrap_or_else(|diagnostics| panic!("{label} journal should restore: {diagnostics:#?}"));
    let typed_trees::expression::ExpressionNode::Call(authored_call) =
        authored.expression_table.expression(*settled_call)
    else {
        panic!("{label} restored occurrence should be a call");
    };
    assert_eq!(
        authored_call.target_symbol, requirement,
        "{label} the journaled occurrence names the requirement",
    );
    assert_eq!(authored_call.target.as_str(), "offset_zero");
    // The direct call is the requirement's own D29 demand, keyed on the
    // requirement machine symbol with the canonical empty application, the
    // same row a named boundary operator use retains; the adapter route
    // publishes no Terminal occurrence for it, so coverage stays empty.
    let requirement_machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| {
            checked
                .typed
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == requirement)
        })
        .unwrap_or_else(|| panic!("{label} requirement machine"));
    let demands = checked
        .facts
        .operators
        .boundary_applications
        .iter()
        .map(|demand| (demand.requirement_symbol, demand.arguments.len()))
        .collect::<Vec<_>>();
    assert_eq!(
        demands,
        vec![(requirement_machine.symbol, 0)],
        "{label} the direct requirement call is one requirement-keyed D29 demand",
    );
    (requirement, realization)
}

#[test]
fn checked_boundary_requirement_dispatch_exit_canary_runs() {
    // The tokenless sibling of `checked_boundary_operator_dispatch_exit`: the
    // `boundary requirement` form with the same contracts and adapter selects
    // through the top-level requirement route, interprets identically, and
    // carries the same contracts through the Terminal leg.
    let canary = pass_canary(CHECKED_BOUNDARY_REQUIREMENT_DISPATCH_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("checked boundary-requirement canary should compile to checked trees");
    assert_selected_requirement_association(&checked, "checked boundary-requirement canary");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter dispatches the selected checked requirement body; error: {:?}",
        outcome.error,
    );
    assert_selected_requirement_terminal_call(&canary, "checked boundary-requirement canary");
}

/// The Terminal leg of a selected top-level requirement: the canonical
/// Terminal artifact replays, its entry calls the adapter's ordinary checked
/// body as an in-module scalar machine, no boundary-operator occurrence or
/// boundary declaration names the requirement, and the checked compilation
/// behind it keeps the requirement-side association and journal.
fn assert_selected_requirement_terminal_call(canary: &std::path::Path, label: &str) {
    assert_selected_requirement_terminal_call_with(
        canary,
        label,
        assert_selected_requirement_association,
    );
}

/// The Terminal leg parameterized by the call position's association check:
/// value-position and statement-position call sites share one artifact
/// shape, differing only in where the settled call lives.
fn assert_selected_requirement_terminal_call_with(
    canary: &std::path::Path,
    label: &str,
    association: fn(
        &compiler::CheckedCompilation,
        &str,
    ) -> (symbols::SymbolHandle, symbols::SymbolHandle),
) {
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &main_path,
        Some("linux_x86_64"),
    ))
    .unwrap_or_else(|diagnostics| panic!("{label} should check: {diagnostics:#?}"));
    association(&checked, label);

    let package_inputs =
        crate::reviewed_repository_fixture_package_inputs(&main_path, Some("linux_x86_64"))
            .unwrap_or_else(|diagnostics| {
                panic!("{label} should derive package inputs: {diagnostics:#?}")
            });
    let mut request = CompileRequest::new(CompilerOptions {
        root_path: main_path,
        build_dir: None,
        target_name: Some("linux_x86_64".into()),
    })
    .with_requested_product(RequestedCompileProduct::TerminalArtifact);
    if let Some(package_inputs) = package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    let report = compiler::compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| {
            panic!("{label} should produce a canonical Terminal artifact: {diagnostics:#?}")
        });
    let retained = report
        .into_retained_terminal_artifact()
        .unwrap_or_else(|| panic!("{label} should retain its Terminal artifact"));
    retained
        .validate()
        .unwrap_or_else(|error| panic!("{label} Terminal artifact should replay: {error}"));
    let module = terminal_codec::decode_module(retained.artifact().semantic_bytes())
        .unwrap_or_else(|error| panic!("{label} Terminal semantics should decode: {error:?}"));
    let proposal = retained
        .native_realization_proposal()
        .unwrap_or_else(|| panic!("{label} should retain its native proposal"));
    assert!(
        proposal
            .checked_boundary_operator_scope()
            .occurrences()
            .is_empty()
            && proposal.boundary_application_demands().rows().is_empty(),
        "{label} publishes no boundary-operator occurrence for a top-level requirement",
    );
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap_or_else(|| panic!("{label} Terminal module has its entry machine"));
    let callees = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::Call { callee, .. }
            | terminal_psi::OperationKind::CallUnit { callee, .. } => Some(*callee),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        callees
            .iter()
            .filter_map(|callee| module.machines.iter().find(|machine| machine.id == *callee))
            .any(|machine| {
                machine.parameters.len() == 1
                    && matches!(
                        machine.result,
                        terminal_psi::TerminalMachineResult::Scalar(_)
                    )
            }),
        "{label} entry should call the adapter as an in-module scalar machine",
    );
    assert!(
        module
            .boundary_machines
            .iter()
            .all(|declaration| !declaration.identity.contains("offset_zero")),
        "{label} neither the requirement nor its realization is a Terminal boundary declaration",
    );
}

#[test]
fn checked_boundary_requirement_terminal_exit_canary_retains_requirement_occurrence() {
    let canary = pass_canary(CHECKED_BOUNDARY_REQUIREMENT_TERMINAL_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("checked boundary-requirement Terminal canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter leg; error: {:?}",
        outcome.error
    );
    assert_selected_requirement_terminal_call(
        &canary,
        "checked boundary-requirement Terminal canary",
    );
}

const CHECKED_BOUNDARY_REQUIREMENT_STATEMENT_CALL_EXIT: &str =
    "providers/checked_boundary_requirement_statement_call_exit";

/// The statement-position sibling of `assert_selected_requirement_association`:
/// the same settled dispatch row, but the redirected call lives in the
/// caller's statement table as an explicit `_ =` discard, and the journal
/// restores the authored requirement statement call. Requirement-use facts
/// are expression-keyed today, so a statement call retains its requirement
/// identity through the settled row, the flow occurrence's authored span and
/// the journal rather than a named-use demand row.
fn assert_selected_requirement_statement_call(
    checked: &compiler::CheckedCompilation,
    label: &str,
) -> (symbols::SymbolHandle, symbols::SymbolHandle) {
    let (requirement, realization, owner) = requirement_dispatch_row(checked, label);
    let settled_calls = checked
        .typed
        .machines()
        .iter()
        .flat_map(|machine| checked.typed.machine_states(machine))
        .flat_map(|state| {
            checked
                .typed
                .statement_table
                .iter_statements(state.statement_nodes)
        })
        .filter_map(|(handle, statement)| match statement {
            typed_trees::statement::StatementNode::Call(call)
                if call.target_symbol == realization
                    && call.receiver.is_empty()
                    && !call.receiver_symbol.is_valid() =>
            {
                Some((handle, call))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(settled_handle, settled_call)] = settled_calls.as_slice() else {
        panic!("{label} should redirect exactly one statement call to the adapter entry");
    };
    assert!(
        settled_call.discards_result,
        "{label} `_ =` stays an explicit result discard after settlement"
    );
    assert_eq!(
        settled_call.target.as_str(),
        "CheckedMathProvider::offset_zero_impl",
        "{label} settled statement call names the adapter machine",
    );

    let authored = checked
        .pre_selected_dispatch_source_trees(&checked.typed)
        .unwrap_or_else(|diagnostics| panic!("{label} journal should restore: {diagnostics:#?}"));
    let typed_trees::statement::StatementNode::Call(authored_call) =
        authored.statement_table.statement(*settled_handle)
    else {
        panic!("{label} restored statement should be a call");
    };
    assert_eq!(
        authored_call.target_symbol, requirement,
        "{label} the journaled statement call names the requirement",
    );
    assert_eq!(authored_call.target.as_str(), "offset_zero");
    assert_eq!(
        authored_call.receiver_symbol, owner,
        "{label} the journaled receiver is the requirement owner",
    );
    assert!(
        authored_call.discards_result,
        "{label} the journaled statement keeps the explicit discard",
    );
    (requirement, realization)
}

#[test]
fn checked_boundary_requirement_statement_call_exit_canary_runs() {
    // The `_ = Owner::name(...);` explicit discard is the same settled route
    // as the value-position canary: the statement call redirects to the
    // selected checked adapter, strict result use is preserved, and the
    // journal restores the authored requirement call.
    let canary = pass_canary(CHECKED_BOUNDARY_REQUIREMENT_STATEMENT_CALL_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("statement-position boundary-requirement canary should compile to checked trees");
    assert_selected_requirement_statement_call(
        &checked,
        "statement-position boundary-requirement canary",
    );
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter dispatches the selected checked requirement body; error: {:?}",
        outcome.error,
    );
    assert_selected_requirement_terminal_call_with(
        &canary,
        "statement-position boundary-requirement canary",
        assert_selected_requirement_statement_call,
    );
}

/// The external-satisfier sibling of `assert_selected_requirement_association`:
/// the covering plan is a `via` leaf with an evaluated import binding, so
/// settlement produces no adapter dispatch row and never retargets the direct
/// call — the requirement is itself a boundary declaration, the settled call
/// stays on the requirement entry, and the requirement's normalized overload
/// identity is the join key every downstream stage keys on.
fn assert_selected_external_requirement(
    checked: &compiler::CheckedCompilation,
    label: &str,
) -> (symbols::SymbolHandle, String) {
    let requirement_machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "ForeignMath::absolute")
        .unwrap_or_else(|| panic!("{label} should retain the requirement machine"));
    let requirement = checked
        .typed
        .machine_states(requirement_machine)
        .first()
        .unwrap_or_else(|| panic!("{label} requirement machine has an entry"))
        .symbol;
    let requirement_identity = checked
        .typed
        .normalized_machine_overload_identity(requirement_machine)
        .unwrap_or_else(|| panic!("{label} requirement retains a normalized overload identity"))
        .identity();

    let plan = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "ForeignMath::absolute")
        .unwrap_or_else(|| panic!("{label} selects the requirement's `via` plan"));
    assert_eq!(plan.provider_type, "ForeignMathProvider");
    assert!(plan.covers_schema());
    let [row] = plan.rows.as_slice() else {
        panic!("{label} should settle exactly one requirement row");
    };
    assert_eq!(row.method, "absolute");
    assert_eq!(row.requirement_identity, requirement_identity);
    let effects::provider_plan::ProviderBinding::Import { evaluated } = &row.binding else {
        panic!("{label} row is the leaf's evaluated import binding, never an adapter");
    };
    assert_eq!(
        evaluated.locator().locator(),
        &target::ForeignLocatorCandidate::ElfVersioned {
            object: b"libc.so.6".to_vec(),
            symbol: b"abs".to_vec(),
            version: b"GLIBC_2.2.5".to_vec(),
        }
    );
    assert_eq!(checked.evaluated_via_bindings().rows().len(), 2);

    assert!(
        checked
            .facts
            .boundary_adapter_dispatch
            .iter()
            .all(|row| row.requirement != requirement),
        "{label} settles no adapter dispatch row for the requirement: the call keeps its seam",
    );
    let calls = checked
        .typed
        .expression_table
        .expression_entries()
        .filter_map(|(_handle, expression)| match expression {
            typed_trees::expression::ExpressionNode::Call(call)
                if call.target.as_str() == "absolute" =>
            {
                Some(call)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [settled_call] = calls.as_slice() else {
        panic!("{label} should retain exactly one `absolute` call site");
    };
    assert_eq!(
        settled_call.target_symbol, requirement,
        "{label} the settled call still targets the requirement entry, not an adapter",
    );
    // The value call is the requirement's own D29 demand, keyed on the
    // requirement machine symbol — the same row a selected adapter records;
    // the seam is that the demand now resolves to the requirement boundary
    // machine itself rather than an adapter-checked occurrence.
    let demands = checked
        .facts
        .operators
        .boundary_applications
        .iter()
        .map(|demand| (demand.requirement_symbol, demand.arguments.len()))
        .collect::<Vec<_>>();
    assert!(
        demands.contains(&(requirement_machine.symbol, 0)),
        "{label} the direct requirement call is a requirement-keyed D29 demand: {demands:?}",
    );
    (requirement, requirement_identity)
}

/// The Terminal leg of an externally satisfied requirement: opposite of the
/// adapter route — the requirement IS the retained boundary declaration, and
/// the entry keeps an exact `BoundaryCall` into it under the requirement's
/// normalized overload identity.
fn assert_selected_external_requirement_terminal_call(
    canary: &std::path::Path,
    label: &str,
) -> String {
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &main_path,
        Some("linux_x86_64"),
    ))
    .unwrap_or_else(|diagnostics| panic!("{label} should check: {diagnostics:#?}"));
    let (_requirement, requirement_identity) =
        assert_selected_external_requirement(&checked, label);

    let package_inputs =
        crate::reviewed_repository_fixture_package_inputs(&main_path, Some("linux_x86_64"))
            .unwrap_or_else(|diagnostics| {
                panic!("{label} should derive package inputs: {diagnostics:#?}")
            });
    let mut request = CompileRequest::new(CompilerOptions {
        root_path: main_path,
        build_dir: None,
        target_name: Some("linux_x86_64".into()),
    })
    .with_requested_product(RequestedCompileProduct::TerminalArtifact);
    if let Some(package_inputs) = package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    let report = compiler::compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| {
            panic!("{label} should produce a canonical Terminal artifact: {diagnostics:#?}")
        });
    let retained = report
        .into_retained_terminal_artifact()
        .unwrap_or_else(|| panic!("{label} should retain its Terminal artifact"));
    retained
        .validate()
        .unwrap_or_else(|error| panic!("{label} Terminal artifact should replay: {error}"));
    let module = terminal_codec::decode_module(retained.artifact().semantic_bytes())
        .unwrap_or_else(|error| panic!("{label} Terminal semantics should decode: {error:?}"));
    let proposal = retained
        .native_realization_proposal()
        .unwrap_or_else(|| panic!("{label} should retain its native proposal"));

    let matching = module
        .boundary_machines
        .iter()
        .filter(|declaration| declaration.identity == requirement_identity)
        .collect::<Vec<_>>();
    let [boundary] = matching.as_slice() else {
        panic!("{label} retains exactly one boundary declaration for the requirement");
    };
    assert_eq!(
        boundary.scalar_parameters.len(),
        1,
        "{label} boundary declaration keeps the authored scalar signature",
    );
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap_or_else(|| panic!("{label} Terminal module has its entry machine"));
    let boundary_calls = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::BoundaryCall { boundary: id, .. } => Some(*id),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        boundary_calls.iter().any(|id| *id == boundary.id),
        "{label} entry keeps an exact BoundaryCall into the requirement boundary",
    );

    let proposal_rows = proposal
        .selected_provider_plans()
        .plans()
        .iter()
        .flat_map(|plan| plan.rows.iter())
        .filter(|row| {
            matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::Import { .. }
            )
        })
        .collect::<Vec<_>>();
    assert!(
        proposal_rows
            .iter()
            .any(|row| row.requirement_identity == requirement_identity),
        "{label} retains the requirement's selected import row",
    );
    assert!(
        proposal
            .external_binding_rows()
            .iter()
            .any(|row| row.requirement_identity == requirement_identity
                && matches!(
                    row.binding,
                    calling_conventions::ExternalBindingKind::Import { .. }
                )),
        "{label} retains the external binding row the settlement join keys on",
    );
    requirement_identity
}

#[test]
fn external_boundary_requirement_via_exit_canary_keeps_requirement_seam() {
    // The `via` route on a top-level boundary requirement: the selected plan's
    // binding is the leaf's evaluated import, so the direct call is not
    // redirected to an adapter — the requirement is itself the boundary
    // declaration the call lowers through. The checked interpreter has no
    // provider for authored bindings and fails closed on that call; the
    // production native route demands the requirement's settlement, which the
    // no-imports production path reports instead of silently emitting.
    let canary = pass_canary(fixture_roster::EXTERNAL_BOUNDARY_REQUIREMENT_VIA_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &main_path,
        Some("linux_x86_64"),
    ))
    .unwrap_or_else(|diagnostics| {
        panic!("external-via requirement canary should check: {diagnostics:#?}")
    });
    let (_requirement, _identity) =
        assert_selected_external_requirement(&checked, "external-via requirement canary");

    let outcome = interpret(&checked, &[]);
    let error = outcome.error.unwrap_or_else(|| {
        panic!("interpreter must fail closed on an externally satisfied requirement call")
    });
    assert!(
        error.contains("bodyless requirement") && error.contains("no provider"),
        "interpreter reports the missing provider instead of running an empty body: {error}",
    );

    assert_selected_external_requirement_terminal_call(&canary, "external-via requirement canary");

    // The requirement seam survives to the native proposal: the demanded
    // boundary carries a normalized foreign mechanism the receiving policy
    // does not classify without an evaluated import settlement, so the
    // production route fails closed naming the requirement callable rather
    // than silently emitting the foreign call.
    let diagnostics =
        compile_rooted_backend_canary_without_output_for_target(&canary, "linux_x86_64")
            .expect_err("the production route supplies no import settlements");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("does not classify normalized foreign mechanism")
                && diagnostic.message.contains("ForeignMath::absolute")
        }),
        "the demanded requirement boundary reaches native mechanism classification: {diagnostics:#?}",
    );
}
