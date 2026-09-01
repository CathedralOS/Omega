use omega_compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileRequest, RequestedCompileProduct,
    SourceEvaluatedImportSettlement, compile,
    realize_retained_terminal_artifact_with_source_evaluated_imports_and_policy,
};
use omega_effects::provider_plan::ProviderBinding;
use omega_installation_evidence::ProviderExecutionEvidence;
use omega_target::ForeignLocatorCandidate;
use omega_task_plans::{
    AdmittedSameStackContribution, SameStackContributionAdmissionCandidate,
    SameStackContributionAdmissionReceiptId, SameStackProviderPlanCommitment,
    admit_same_stack_contribution,
};
use std::fs;
use std::path::PathBuf;

const INSTALL_NAME: &[u8] = b"/usr/lib/libSystem.B.dylib";
const SYMBOL: &[u8] = b"_getpid";

struct Fixture {
    root: PathBuf,
    main: PathBuf,
    target: String,
}

impl Fixture {
    fn new() -> Self {
        Self::with_source(
            "macho",
            "macos_arm64",
            r#"use omega::language::core::external_binding;

target macos_arm64 {
}

boundary trait Process {
    machine ping();
}

macos_arm64 machine ping_binding() -> Binding<26, 7, 0> {
    Binding::DllImport {
        import: DllImport::MachODylibSymbol {
            install_name: "/usr/lib/libSystem.B.dylib",
            symbol: "_getpid",
        },
    }
}

machine ping_leaf() satisfies Process::ping via ping_binding();

data Main { process: Process; }
machine Main::main(&mut self) {
    self.process.ping();
}
"#,
            r#"machine build(builder: &mut Build) {
    builder.application("source-evaluated-macho-native");
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
}
"#,
        )
    }

    fn new_windows_x86_fma() -> Self {
        Self::with_source(
            "windows-x86-fma",
            "windows_x86_64",
            r#"use omega::language::core::external_binding;
use omega::language::core::float_operations;

target windows_x86_64 {
}

boundary trait Process {
    machine ping();
}

windows_x86_64 machine ping_binding() -> Binding<12, 24, 0> {
    Binding::DllImport {
        import: DllImport::PeByName {
            library: "kernel32.dll",
            export: "FlushProcessWriteBuffers",
        },
    }
}

machine ping_leaf() satisfies Process::ping via ping_binding();

data Main { process: Process; }
machine Main::main(&mut self) {
    let fused: f32 = F32::fused_multiply_add(
        1.00000011920928955078125f32,
        0.99999988079071044921875f32,
        -1.0f32,
    );
    self.process.ping();
}
"#,
            r#"machine build(builder: &mut Build) {
    builder.application("source-evaluated-windows-x86-fma");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;
}
"#,
        )
    }

    fn with_source(name: &str, target: &str, source: &str, build: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-source-evaluated-{name}-native-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create source-evaluated native fixture");
        let main = root.join("main.omg");
        fs::write(&main, source).expect("write source-evaluated native source");
        fs::write(root.join("build.omg"), build)
            .expect("write source-evaluated native build policy");
        Self {
            root,
            main,
            target: target.to_owned(),
        }
    }

    fn compile_terminal(&self) -> omega_compilation_report::RetainedTerminalArtifact {
        let request = CompileRequest::new(CompileOptions {
            root_path: self.main.clone(),
            build_dir: Some(self.root.join("build")),
            target_name: Some(self.target.clone()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly);
        compile(request)
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "structured source-evaluated import should reach retained Terminal custody:\n{}",
                    diagnostics
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            })
            .into_retained_terminal_artifact()
            .expect("Terminal compilation retains its native proposal")
    }
}

#[test]
fn retained_x86_fma_and_source_evaluated_import_compose_nested_mxcsr_custody() {
    let fixture = Fixture::new_windows_x86_fma();
    let retained = fixture.compile_terminal();
    let admission = admit_import(
        &retained,
        SameStackContributionAdmissionReceiptId::from_normalized_identity(0x5846_4d41_0005)
            .unwrap(),
    );
    let policy = terminal_authority_policy(&retained);
    let permission_policy = terminal_authority_permission_policy(&retained);
    let policy_identity = policy.identity();
    let artifact = realize_retained_terminal_artifact_with_source_evaluated_imports_and_policy(
        retained,
        &psi_proof_admission::AdmissionProfile::default(),
        &omega_optimization_core::OptimizationSelections::default(),
        policy,
        omega_terminal_psi_to_native_artifact::current_terminal_authority_permission_policy(),
        permission_policy,
        &[SourceEvaluatedImportSettlement::new(
            &admission.execution,
            &admission.same_stack,
        )],
    )
    .unwrap_or_else(|diagnostics| panic!("FMA plus import should realize: {diagnostics:#?}"));

    artifact
        .validate()
        .expect("combined native artifact replays");
    artifact
        .validate_for_terminal_authority_policy(policy_identity)
        .expect("combined artifact retains the exact accepted foreign policy");
    let function = artifact
        .object()
        .functions()
        .iter()
        .find(|function| !function.x86_scalar_fma_occurrences.is_empty())
        .expect("one FMA-bearing source function");
    let outer = function
        .x86_floating_control
        .expect("FMA function has canonical MXCSR custody");
    let [foreign] = artifact.object().foreign_calls() else {
        panic!("one source-evaluated foreign call")
    };
    let nested = foreign
        .x86_floating_control
        .expect("returning foreign call has nested complete-MXCSR custody");
    assert!(outer.install_offset + outer.install_byte_count <= nested.save_offset);
    assert!(nested.restore_offset + nested.restore_byte_count <= outer.restore_offset);
    assert_eq!(artifact.image().output().format, "pe64-x86_64-executable");
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Debug)]
struct TestProviderExecution {
    requirement: String,
    plan_report_identity: u64,
}

impl ProviderExecutionEvidence for TestProviderExecution {
    fn requirement_identity(&self) -> &str {
        &self.requirement
    }

    fn provider_plan_report_identity(&self) -> u64 {
        self.plan_report_identity
    }

    fn provider_execution_report_identity(&self) -> u64 {
        0x4d41_4348_4f01
    }

    fn provider_execution_report_fingerprint(&self) -> u64 {
        0x4d41_4348_4f02
    }

    fn normalized_root_report_identity(&self) -> u64 {
        0x4d41_4348_4f03
    }

    fn boundary_contract_report_fingerprint(&self) -> u64 {
        0x4d41_4348_4f04
    }
}

fn import_coordinates(
    retained: &omega_compilation_report::RetainedTerminalArtifact,
) -> (String, u64, SameStackProviderPlanCommitment) {
    let proposal = retained
        .native_realization_proposal()
        .expect("retained Terminal product has a native proposal");
    let matches = proposal
        .selected_provider_plans()
        .plans()
        .iter()
        .flat_map(|plan| {
            plan.rows.iter().filter_map(move |row| {
                matches!(row.binding, ProviderBinding::Import { .. }).then_some((plan, row))
            })
        })
        .collect::<Vec<_>>();
    let [(plan, row)] = matches.as_slice() else {
        panic!("one selected evaluated import expected")
    };
    (
        row.requirement_identity.clone(),
        plan.report_fingerprint(),
        SameStackProviderPlanCommitment::from_digest(*plan.identity_digest().as_bytes()),
    )
}

fn terminal_authority_policy(
    retained: &omega_compilation_report::RetainedTerminalArtifact,
) -> omega_terminal_psi_to_native_artifact::TerminalAuthorityPolicy {
    let proposal = retained
        .native_realization_proposal()
        .expect("retained Terminal product has a native proposal");
    let matches = proposal
        .external_binding_rows()
        .iter()
        .filter_map(|row| {
            let omega_calling_conventions::ExternalBindingKind::Import { locator } = &row.binding
            else {
                return None;
            };
            Some((locator, row.boundary_entry_plan.as_ref()?))
        })
        .collect::<Vec<_>>();
    let [(locator, boundary_entry_plan)] = matches.as_slice() else {
        panic!("one exact normalized import policy row expected")
    };
    omega_terminal_psi_to_native_artifact::terminal_authority_policy_with_rows(vec![
        omega_terminal_psi_to_native_artifact::TerminalAuthorityPolicyRow::new(
            omega_terminal_psi_to_native_artifact::normalized_foreign_terminal_mechanism(
                locator,
                boundary_entry_plan,
            )
            .expect("retained foreign boundary plan is canonical"),
            omega_effects::TerminalAuthorityDisposition::from_classes([]),
        ),
    ])
    .expect("receiving policy has one exact normalized import row")
}

fn terminal_authority_permission_policy(
    retained: &omega_compilation_report::RetainedTerminalArtifact,
) -> omega_terminal_psi_to_native_artifact::TerminalAuthorityPermissionPolicy {
    let proposal = retained
        .native_realization_proposal()
        .expect("retained Terminal product has a native proposal");
    let rows = proposal
        .selected_provider_plans()
        .plans()
        .iter()
        .flat_map(|plan| {
            plan.rows.iter().filter_map(move |row| {
                matches!(row.binding, ProviderBinding::Import { .. }).then(|| {
                    omega_terminal_psi_to_native_artifact::TerminalAuthorityPermissionPolicyRow::new(
                        plan.schema.identity_digest(),
                        row.requirement_identity.clone(),
                        omega_effects::TerminalAuthorityDisposition::from_classes([]),
                    )
                })
            })
        })
        .collect();
    omega_terminal_psi_to_native_artifact::terminal_authority_permission_policy_with_rows(rows)
        .expect("exact source-evaluated import permissions")
}

struct AdmittedImport {
    execution: TestProviderExecution,
    same_stack: AdmittedSameStackContribution,
    plan_report_identity: u64,
}

fn admit_import(
    retained: &omega_compilation_report::RetainedTerminalArtifact,
    receipt: SameStackContributionAdmissionReceiptId,
) -> AdmittedImport {
    let (requirement, plan_report_identity, plan_commitment) = import_coordinates(&retained);
    let execution = TestProviderExecution {
        requirement: requirement.clone(),
        plan_report_identity,
    };
    let same_stack = admit_same_stack_contribution(
        SameStackContributionAdmissionCandidate {
            provider_plan_report_identity: plan_report_identity,
            provider_plan_commitment: plan_commitment,
            requirement_identity: requirement.clone(),
            receipt,
            bytes: 64,
            alignment: 16,
        },
        plan_report_identity,
        plan_commitment,
        &requirement,
    )
    .expect("exact provider-plan custody admits the foreign leaf");
    AdmittedImport {
        execution,
        same_stack,
        plan_report_identity,
    }
}

#[test]
fn retained_source_evaluated_import_realizes_exact_macho_image() {
    let fixture = Fixture::new();
    let missing = fixture.compile_terminal();
    let missing_policy = terminal_authority_policy(&missing);
    let missing_permission_policy = terminal_authority_permission_policy(&missing);
    let diagnostics = realize_retained_terminal_artifact_with_source_evaluated_imports_and_policy(
        missing,
        &psi_proof_admission::AdmissionProfile::default(),
        &omega_optimization_core::OptimizationSelections::default(),
        missing_policy,
        omega_terminal_psi_to_native_artifact::current_terminal_authority_permission_policy(),
        missing_permission_policy,
        &[],
    )
    .expect_err("a demanded source-evaluated import requires external custody");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("has no supplied execution"))
    );

    let retained = fixture.compile_terminal();
    let admission = admit_import(
        &retained,
        SameStackContributionAdmissionReceiptId::from_normalized_identity(0x4d41_4348_4f05)
            .unwrap(),
    );
    let policy = terminal_authority_policy(&retained);
    let permission_policy = terminal_authority_permission_policy(&retained);
    let policy_identity = policy.identity();
    let artifact = realize_retained_terminal_artifact_with_source_evaluated_imports_and_policy(
        retained,
        &psi_proof_admission::AdmissionProfile::default(),
        &omega_optimization_core::OptimizationSelections::default(),
        policy,
        omega_terminal_psi_to_native_artifact::current_terminal_authority_permission_policy(),
        permission_policy,
        &[SourceEvaluatedImportSettlement::new(
            &admission.execution,
            &admission.same_stack,
        )],
    )
    .unwrap_or_else(|diagnostics| {
        panic!(
            "externally admitted import should realize a native Mach-O artifact:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    artifact.validate().expect("native artifact replays");
    artifact
        .validate_for_terminal_authority_policy(policy_identity)
        .expect("native artifact retains the exact accepted foreign policy");
    assert_eq!(artifact.target(), omega_target::NativeTarget::macos_arm64());
    assert_eq!(artifact.provider_executions().len(), 1);
    let [foreign_call] = artifact.object().foreign_calls() else {
        panic!("one retained Mach-O foreign call expected")
    };
    assert_eq!(foreign_call.x86_floating_control, None);
    let control = foreign_call
        .aarch64_floating_control
        .expect("source-evaluated Mach-O call preserves complete FPCR");
    assert_eq!(control.target, omega_target::NativeTarget::macos_arm64());
    assert_eq!(control.save_byte_count, 8);
    assert_eq!(control.restore_byte_count, 8);
    assert!(control.save_offset < foreign_call.text_offset);
    assert!(foreign_call.text_offset + 4 <= control.restore_offset);
    assert_eq!(artifact.image().output().format, "mach-o-arm64-executable");
    assert_eq!(artifact.image().output().final_image_imports, 1);
    let [normalized] = artifact
        .object()
        .object()
        .layout
        .normalized_imports
        .as_slice()
    else {
        panic!("one normalized object import expected")
    };
    let ForeignLocatorCandidate::MachODylibSymbol {
        install_name,
        symbol,
    } = normalized.locator.locator()
    else {
        panic!("structured Mach-O locator must survive object construction")
    };
    assert_eq!(install_name, INSTALL_NAME);
    assert_eq!(symbol, SYMBOL);

    let object_demand =
        omega_image_emission::derive_stack_demand(artifact.object(), artifact.object().entry())
            .expect("object stack demand includes the admitted foreign leaf");
    assert!(
        object_demand
            .admitted_contribution_report_identities()
            .contains(&admission.same_stack.report_identity())
    );
    assert!(
        object_demand
            .admitted_contribution_commitments()
            .contains(&admission.same_stack.commitment())
    );

    let installation =
        omega_image_emission::build_installation_record_with_selected_provider_plans_and_evidence(
            artifact.image(),
            psi_core::ProfileDecisionId::new(1).expect("profile decision"),
            artifact
                .selected_provider_plans()
                .iter()
                .map(|plan| plan.report_identity()),
            artifact.provider_executions().iter(),
            None,
        )
        .expect("installation retains the admitted foreign stack projection");
    let foreign_stacks = installation
        .functions()
        .iter()
        .flat_map(|function| &function.foreign_call_stacks)
        .collect::<Vec<_>>();
    let [foreign_stack] = foreign_stacks.as_slice() else {
        panic!("one installed foreign stack contribution expected")
    };
    assert_eq!(
        foreign_stack.provider_plan_report_identity,
        admission.plan_report_identity
    );
    assert_eq!(
        foreign_stack.contribution_report_identity,
        admission.same_stack.report_identity()
    );
    assert_eq!(
        foreign_stack.contribution_commitment,
        admission.same_stack.commitment()
    );
    assert_eq!(
        foreign_stack.contribution_bytes,
        admission.same_stack.bytes()
    );
    assert_eq!(
        foreign_stack.contribution_alignment,
        admission.same_stack.alignment()
    );

    let encoded = omega_image_emission::encode_installation_record(&installation)
        .expect("foreign stack installation encodes");
    let decoded = omega_image_emission::decode_installation_record(&encoded)
        .expect("foreign stack installation decodes");
    omega_image_emission::validate_installation_record(&decoded, artifact.image())
        .expect("decoded foreign stack installation rejoins the exact image");
    let installation_demand = omega_image_emission::derive_installation_stack_demand(
        &decoded,
        artifact.image(),
        artifact.object().entry(),
    )
    .expect("installation replays the admitted foreign stack demand");
    assert_eq!(installation_demand, object_demand);
}
