use assembled_syntax_to_checked_compilation::CheckedCompilation;
use checked_compilation_to_terminal_artifact::ProgramEntryTerminalArtifact;
use diagnostics::Diagnostic;
use optimization_core::PostTerminalOptimizationSelections;

/// Rejoin the checked compilation's package-approved terminal permissions.
///
/// The package-permission custody scan always runs. The receiver join runs
/// only when a receiving permission policy was explicitly supplied; `None`
/// means this production makes no receiver-admission claim and never
/// fabricates receiver rows from accepted package evidence.
pub(super) fn validate_terminal_authority_permissions(
    checked: &CheckedCompilation,
    terminal_authority_permission_policy: Option<&crate::TerminalAuthorityPermissionPolicy>,
) -> Result<(), Vec<Diagnostic>> {
    let permissions = checked
        .resolved_semantic_bindings()
        .flat_map(|binding| binding.terminal_authority_permissions());
    match terminal_authority_permission_policy {
        Some(policy) => crate::validate_package_terminal_authority_permissions(permissions, policy),
        None => crate::validate_package_terminal_authority_permission_custody(permissions),
    }
}

pub(super) fn realize(
    checked: &CheckedCompilation,
    admission: &super::admission::NativeCompilationAdmission,
    profile: &proof_admission::AdmissionProfile,
    terminal_authority_policy: crate::TerminalAuthorityPolicy,
    terminal_authority_permission_policy: Option<crate::TerminalAuthorityPermissionPolicy>,
    optimization_selections: &PostTerminalOptimizationSelections,
    prepared_terminal: ProgramEntryTerminalArtifact,
    prepared_input: &crate::PreparedNativeRealizationInput,
) -> Result<crate::NativeArtifact, Vec<Diagnostic>> {
    let (
        artifact,
        checked_program_entry,
        checked_boundary_operator_scope,
        boundary_application_coverage,
    ) = prepared_terminal.into_parts();
    let terminal_module = terminal_codec::decode_module(artifact.semantic_bytes()).map_err(
        |error| {
            vec![Diagnostic::error(format!(
                "native-artifact intrinsic settlement could not replay canonical Terminal semantics: {error}"
            ))]
        },
    )?;
    // The retained authored exclusion rows resolve into the canonical union
    // against this artifact's own module — the same trait-identity join the
    // product-admission checker applies — so a requested physical absence
    // reaches mechanism adjudication even without a receiver permission
    // policy.
    let behavior_exclusions = build_evaluation::authored_behavior_exclusion_set_in(
        checked.behavior_exclusions(),
        &terminal_module,
        &|symbol: symbols::SymbolHandle| {
            checked
                .typed
                .traits()
                .iter()
                .find(|definition| definition.symbol == symbol && definition.is_boundary)
                .map(|definition| definition.name.as_str().to_owned())
        },
    );
    let demanded_intrinsics =
        provider_planning::compiler_intrinsics::demanded_boundary_identities(&terminal_module)?;
    let intrinsic_proposals =
        provider_planning::compiler_intrinsics::derive_selected_intrinsic_settlement_proposals(
            checked.selected_provider_plans().plans(),
            checked.selected_provider_provenance(),
            &demanded_intrinsics,
        )?;
    let selected_plans = checked.selected_provider_plans().plans();
    let compiler_builtins = intrinsic_proposals
        .iter()
        .map(|proposal| crate::NativeCompilerBuiltinSettlement {
            requirement_identity: &proposal.requirement_identity,
            provider_plan: &selected_plans[proposal.plan_index],
            execution: proposal.execution,
        })
        .collect::<Vec<_>>();
    let calling_plans = admission.program_entry.calling_plans().map(|plans| {
        (
            &plans.semantic_calling_application,
            &plans.physical_calling_application,
            &plans.storage_entry,
        )
    });
    let program_entry = crate::NativeProgramEntrySettlement::new(
        admission.program_entry.source_signature(),
        calling_plans,
        admission.program_entry.fused_service_establishments(),
    )
    .with_checked_entry(&checked_program_entry);
    let _validated_program_entry = crate::validate_native_program_entry_settlement(
        &artifact,
        &checked_program_entry,
        program_entry,
        admission.target,
    )
    .map_err(|error| {
        vec![Diagnostic::error(format!(
            "native-artifact checked ProgramEntry settlement failed: {error}"
        ))]
    })?;
    // The authored Build.identifier supplies the Mach-O CodeDirectory signing
    // identity (wiki/spec/build/macos_application.md). Signed macOS GUI image
    // emission requires it: absence is an early realization configuration
    // error, not a source-semantic rejection. Console Mach-O and non-Mach-O
    // output keep the executable-leaf ad-hoc label fallback.
    let code_signature_identifier = checked
        .application_identifier()
        .map(|identifier| identifier.as_str().to_owned());
    if admission.target.object_format == target::ObjectFormat::MachO
        && matches!(
            checked.application_intent(),
            Some(build_evaluation::HostedApplicationIntent::Gui)
        )
        && code_signature_identifier.is_none()
    {
        return Err(vec![Diagnostic::error(
            "signed macOS GUI image emission requires the authored Build identifier",
        )]);
    }
    let request = crate::NativeRealizationRequest {
        checked_scope: Some(&checked_boundary_operator_scope),
        prepared_input: Some(prepared_input),
        target: admission.target,
        image_request: crate::ExecutableImageEmissionRequest::direct(checked.subsystem())
            .with_code_signature_identifier(code_signature_identifier),
        profile,
        terminal_authority_policy,
        terminal_authority_permission_policy,
        program_entry,
        optimization_selections,
        selected_provider_plans: checked.selected_provider_plans(),
        external_binding_rows: checked.external_binding_rows(),
        settlements: &[],
        compiler_builtins: &compiler_builtins,
        boundary_application_coverage: Some(&boundary_application_coverage),
        ieee_float_fma: &[],
        native_callbacks: &[],
        callback_thunks: &[],
        behavior_exclusions: &behavior_exclusions,
    };
    crate::realize_native_artifact(artifact, request)
        .map_err(|error| error.into_parts().1)?
        .into_direct()
        .map_err(|_| {
            vec![Diagnostic::error(
                "direct native realization returned a different image kind",
            )]
        })
}

#[cfg(test)]
mod tests {
    use crate::tests::fixtures::checked_source::checked;
    use crate::tests::fixtures::hosted::{hosted_calling_plans, paired_calling_plan_parts};
    use crate::validate_package_terminal_authority_permissions;
    use crate::{
        NativeCompilerBuiltinSettlement, NativeProgramEntrySettlement, NativeRealizationRequest,
    };
    use build_evaluation::{BehaviorExclusion, BehaviorExclusions};
    use effects::provider_plan::{
        ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
    };
    use effects::{
        ServiceTerminalAuthorityPermission, TerminalAuthorityClass, TerminalAuthorityDisposition,
        provider_plan::{ProviderPlanDigest, ServiceSchemaDigest},
    };
    use package_compilation::{AcceptedSemanticBinding, AcceptedSemanticBindingRole};
    use semantic_vocabulary::PackageKeyIdentity;
    use target_operations::CompilerBuiltinExecution;

    /// One reachable boundary invocation whose selected provider is a
    /// compiler intrinsic, so the mechanism-closure review exercises the
    /// intrinsic's physical class for the requirement's leaf.
    const SINK_SOURCE: &str = "boundary trait Sink { machine emit(text: u8); }
         data Main {}
         machine Main::launch(&mut self) reaches Sink {
             Sink::emit(7);
         }";

    fn sink_entry_artifact() -> (
        terminal_codec::CanonicalTerminalArtifact,
        terminal_psi::CheckedProgramEntryTerminalReceipt,
        program_entry_plan::SelectedProgramEntrySourceSignature,
        build_evaluation::SelectedProgramEntryCallingPlans,
    ) {
        let target_profile = target::TargetProfile::LinuxX64;
        let checked = checked(SINK_SOURCE);
        let selection = checked
            .facts
            .flow
            .terminal_machines
            .machines
            .iter()
            .find(|machine| machine.name == "Main::launch")
            .expect("source-selected entry");
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == selection.machine)
            .expect("checked source entry");
        let state = checked
            .machine_states(machine)
            .first()
            .expect("entry state");
        let receiver = checked
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.is_self)
            .expect("entry receiver");
        let signature =
            program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
                target_profile.program_entry_slot(),
                selection.machine,
                selection.machine,
                selection.name.clone(),
                "entry".into(),
                "test::Main::launch(&mut self) -> Unit".into(),
                program_entry_plan::ProgramEntrySourceReceiverSignature::ProvisionedMutable {
                    normalized_type_identity: checked
                        .normalized_type_identity(receiver.type_reference)
                        .into_string(),
                },
                Vec::new(),
            )
            .expect("selected source signature");
        let produced =
            terminal_production::TerminalProductionRequest::new(&checked, "Main::launch")
                .produce_program_entry_with_callback_custody(signature.identity().bytes(), ())
                .expect("Sink entry produces a Terminal artifact");
        let plans = hosted_calling_plans(target_profile);
        let artifact =
            terminal_codec::CanonicalTerminalArtifact::from_bytes(&produced.artifact().to_bytes())
                .expect("produced artifact replays from canonical bytes");
        let receipt = produced.receipt().clone();
        (artifact, receipt, signature, plans)
    }

    /// The `Sink::emit` requirement identity the produced artifact retains.
    fn sink_requirement(artifact: &terminal_codec::CanonicalTerminalArtifact) -> String {
        let module = terminal_codec::decode_module(artifact.semantic_bytes())
            .expect("decode Terminal module");
        module
            .boundary_machines
            .iter()
            .find(|boundary| boundary.identity.contains("Sink::emit"))
            .map(|boundary| boundary.identity.clone())
            .expect("produced artifact retains the Sink::emit boundary")
    }

    /// A selected plan binding the Sink requirement to a compiler intrinsic;
    /// the builtin settlement supplies the intrinsic's admitted mechanism.
    fn intrinsic_sink_plan(requirement: &str, target_name: &str) -> ProviderPlan {
        ProviderPlan {
            name: "HostedSink".into(),
            provider_type: "HostedSink".into(),
            provider_type_package_identity: None,
            target: target_name.into(),
            schema: ServiceSchema {
                trait_name: "Sink".into(),
                trait_package_identity: None,
                methods: vec![ServiceMethod {
                    name: "emit".into(),
                    requirement_owner: "Sink".into(),
                    requirement_owner_package_identity: None,
                    requirement_identity: requirement.into(),
                    parameter_count: 1,
                    parameter_type_identities: vec!["u8".into()],
                    entry_claims: Vec::new(),
                    has_result: false,
                    result_type_identity: None,
                    result_claims: Vec::new(),
                    service_reach: vec!["Sink".into()],
                    synchronous_invocations: Vec::new(),
                    may_suspend: false,
                    may_block: false,
                    terminates_guarantee: true,
                    termination_premises: Vec::new(),
                    calling_plan_report_fingerprint: None,
                    calling_plan_commitment: None,
                }],
            },
            rows: vec![ProviderPlanRow {
                method: "emit".into(),
                requirement_identity: requirement.into(),
                requirement_lifetime_partition: Vec::new(),
                binding: ProviderBinding::CompilerIntrinsic {
                    machine: "test::Sink::emit hosted write realization".into(),
                },
            }],
            origin_package_identity: None,
            origin_package: "test".into(),
        }
    }

    fn exclusion_request<'request>(
        signature: &'request program_entry_plan::SelectedProgramEntrySourceSignature,
        receipt: &'request terminal_psi::CheckedProgramEntryTerminalReceipt,
        plans: &'request build_evaluation::SelectedProgramEntryCallingPlans,
        profile: &'request proof_admission::AdmissionProfile,
        optimizations: &'request optimization_core::PostTerminalOptimizationSelections,
        providers: &'request effects::SelectedProviderPlanFacts,
        builtins: &'request [NativeCompilerBuiltinSettlement<'request>],
        behavior_exclusions: &'request BehaviorExclusions,
    ) -> NativeRealizationRequest<'request> {
        NativeRealizationRequest {
            checked_scope: None,
            prepared_input: None,
            target: signature.target_slot().owner.native_target(),
            image_request: image_emission::ExecutableImageEmissionRequest::direct(3),
            profile,
            terminal_authority_policy: crate::current_compiler_intrinsic_terminal_authority_policy(
            ),
            terminal_authority_permission_policy: None,
            program_entry: NativeProgramEntrySettlement::new(
                signature,
                Some(paired_calling_plan_parts(plans)),
                &[],
            )
            .with_checked_entry(receipt),
            optimization_selections: optimizations,
            selected_provider_plans: providers,
            external_binding_rows: &[],
            settlements: &[],
            compiler_builtins: builtins,
            boundary_application_coverage: None,
            ieee_float_fma: &[],
            native_callbacks: &[],
            callback_thunks: &[],
            behavior_exclusions,
        }
    }

    #[test]
    fn exclusion_taking_entry_reaches_mechanism_adjudication() {
        let (artifact, receipt, signature, plans) = sink_entry_artifact();
        let requirement = sink_requirement(&artifact);
        let target_name = signature.target_slot().owner.target_name();
        let plan = intrinsic_sink_plan(&requirement, target_name);
        let providers = effects::SelectedProviderPlanFacts::from_selected_plans(vec![plan.clone()])
            .expect("selected intrinsic plan validates");
        let builtins = [NativeCompilerBuiltinSettlement {
            requirement_identity: requirement.as_str(),
            provider_plan: &plan,
            execution: CompilerBuiltinExecution::HostedWriteByteI32,
        }];
        let profile = proof_admission::AdmissionProfile::default();
        let optimizations = optimization_core::PostTerminalOptimizationSelections::default();

        // A leaf with `permitted: None` — no receiver permission policy is
        // supplied — still exercises the intrinsic's ProcessOutput class, so
        // the requested absence must reject, and every closure leaf is named.
        let exclusions =
            BehaviorExclusions::from_selections([BehaviorExclusion::PhysicalAuthorityClass(
                TerminalAuthorityClass::ProcessOutput,
            )]);
        let error = crate::realize_native_artifact(
            terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
                .expect("artifact bytes replay"),
            exclusion_request(
                &signature,
                &receipt,
                &plans,
                &profile,
                &optimizations,
                &providers,
                &builtins,
                &exclusions,
            ),
        )
        .expect_err("the admitted intrinsic exercises the excluded class");
        let message = format!("{:?}", error.diagnostics());
        assert!(message.contains("Sink::emit"), "{message}");
        assert!(message.contains("ProcessOutput"), "{message}");

        // Excluding a class no leaf exercises — and the default empty union
        // alike — passes adjudication and proceeds to later stages, where
        // this minimal fixture then fails the hosted-exit shape check. The
        // union, not the mechanism inventory, decides the verdict.
        for excluded in [
            BehaviorExclusions::from_selections([BehaviorExclusion::PhysicalAuthorityClass(
                TerminalAuthorityClass::PortIo,
            )]),
            BehaviorExclusions::default(),
        ] {
            let error = crate::realize_native_artifact(
                terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
                    .expect("artifact bytes replay"),
                exclusion_request(
                    &signature,
                    &receipt,
                    &plans,
                    &profile,
                    &optimizations,
                    &providers,
                    &builtins,
                    &excluded,
                ),
            )
            .expect_err("the fixture entry stops at hosted-exit shape checking");
            let message = format!("{:?}", error.diagnostics());
            assert!(
                message.contains("InvalidHostedExitProcessShape"),
                "{message}"
            );
            assert!(!message.contains("behavior exclusions"), "{message}");
        }
    }

    fn accepted_binding() -> AcceptedSemanticBinding {
        let schema = ServiceSchemaDigest::from_digest([41; 32]);
        AcceptedSemanticBinding::new(
            AcceptedSemanticBindingRole::ConsoleExitProcessI32,
            PackageKeyIdentity::from_digest([42; 32]).expect("nonzero package identity"),
            "Console",
            schema,
            ProviderPlanDigest::from_digest([43; 32]),
        )
        .expect("accepted Console binding")
        .with_terminal_authority_permissions(vec![ServiceTerminalAuthorityPermission::new(
            schema,
            "Console::exit_process#exact",
            TerminalAuthorityDisposition::from_classes([
                TerminalAuthorityClass::ProcessTermination,
            ]),
        )])
        .expect("exact permission")
    }

    fn policy(permitted: TerminalAuthorityDisposition) -> crate::TerminalAuthorityPermissionPolicy {
        crate::terminal_authority_permission_policy_with_rows(vec![
            crate::TerminalAuthorityPermissionPolicyRow::new(
                ServiceSchemaDigest::from_digest([41; 32]),
                "Console::exit_process#exact",
                permitted,
            ),
        ])
        .expect("valid receiving policy")
    }

    #[test]
    fn package_permission_must_match_receiving_policy_exactly() {
        let binding = accepted_binding();
        let exact = policy(TerminalAuthorityDisposition::from_classes([
            TerminalAuthorityClass::ProcessTermination,
        ]));
        assert!(
            validate_package_terminal_authority_permissions(
                binding.terminal_authority_permissions().iter(),
                &exact,
            )
            .is_ok()
        );

        let substituted = policy(TerminalAuthorityDisposition::from_classes([
            TerminalAuthorityClass::ProcessOutput,
        ]));
        let diagnostics = validate_package_terminal_authority_permissions(
            binding.terminal_authority_permissions().iter(),
            &substituted,
        )
        .expect_err("changed classes must reject");
        assert!(diagnostics[0].message.contains("substitutes"));

        let missing = crate::current_terminal_authority_permission_policy();
        let diagnostics = validate_package_terminal_authority_permissions(
            binding.terminal_authority_permissions().iter(),
            &missing,
        )
        .expect_err("missing exact row must reject");
        assert!(diagnostics[0].message.contains("omits"));
    }

    #[test]
    fn duplicate_permissions_across_resolved_bindings_reject() {
        let first = accepted_binding();
        let second = first.clone();
        let exact = policy(TerminalAuthorityDisposition::from_classes([
            TerminalAuthorityClass::ProcessTermination,
        ]));
        let diagnostics = validate_package_terminal_authority_permissions(
            [&first, &second]
                .into_iter()
                .flat_map(|binding| binding.terminal_authority_permissions()),
            &exact,
        )
        .expect_err("cross-binding duplicate must reject");
        assert!(diagnostics[0].message.contains("repeat"));
    }

    #[test]
    fn custody_scan_without_receiving_policy_keeps_package_rows_unclaimed() {
        let binding = accepted_binding();
        assert!(
            crate::validate_package_terminal_authority_permission_custody(
                binding.terminal_authority_permissions().iter(),
            )
            .is_ok(),
            "ordinary production with no receiving policy retains package custody",
        );

        // Custody without a receiving policy is not deny-all: the accepted
        // package row stands on its own and is not compared against any
        // receiver rows.
        let second = binding.clone();
        let diagnostics = crate::validate_package_terminal_authority_permission_custody(
            [&binding, &second]
                .into_iter()
                .flat_map(|binding| binding.terminal_authority_permissions()),
        )
        .expect_err("custody-only scan still rejects repeated coordinates");
        assert!(diagnostics[0].message.contains("repeat"));
    }
}
