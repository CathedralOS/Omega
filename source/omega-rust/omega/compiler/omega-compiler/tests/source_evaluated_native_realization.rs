use omega_compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileRequest, RequestedCompileProduct,
    SourceEvaluatedImportSettlement, compile,
    realize_retained_terminal_artifact_with_source_evaluated_imports_and_policy,
    realize_retained_terminal_artifact_with_source_evaluated_imports_and_policy_for_image,
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

use omega_terminal_psi_to_native_artifact as native;

const INSTALL_NAME: &[u8] = b"/usr/lib/libSystem.B.dylib";
const SYMBOL: &[u8] = b"_getpid";
const SCALAR_SYMBOL: &[u8] = b"_sleep";

fn replay_native_artifact_parts(
    parts: &native::NativeArtifactParts,
) -> native::NativeArtifactParts {
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
        selected_provider_closure_report_identity: parts.selected_provider_closure_report_identity,
        selected_provider_closure_digest: parts.selected_provider_closure_digest,
        selected_provider_plans: parts.selected_provider_plans.clone(),
        provider_executions: parts.provider_executions.clone(),
        terminal_authority_policy_identity: parts.terminal_authority_policy_identity,
        terminal_authority_permission_policy_identity: parts
            .terminal_authority_permission_policy_identity,
        terminal_authority_closure_review: parts.terminal_authority_closure_review.clone(),
        boundary_application_coverage: parts.boundary_application_coverage.clone(),
        physical_evidence_scope: parts.physical_evidence_scope.clone(),
        physical_evidence: parts.physical_evidence.clone(),
    }
}

fn assert_physical_child_mutation_rejected(
    parts: &native::NativeArtifactParts,
    mutate: impl FnOnce(&mut native::NativePhysicalChildParts),
) {
    let mut replay = replay_native_artifact_parts(parts);
    let evidence = replay
        .physical_evidence
        .take()
        .expect("admitted foreign-call physical evidence")
        .into_parts();
    let [child] = evidence.children.as_slice() else {
        panic!("one admitted-provider physical child")
    };
    let mut child = child.clone().into_parts();
    mutate(&mut child);
    replay.physical_evidence = Some(native::NativePhysicalEvidence::from_replayed_parts(
        native::NativePhysicalEvidenceParts {
            projection: evidence.projection,
            children: vec![native::NativePhysicalChild::from_replayed_parts(child)],
            identity: evidence.identity,
        },
    ));
    assert!(
        native::NativeArtifact::from_replayed_parts(replay).is_err(),
        "mutated admitted-provider physical custody must not replay",
    );
}

fn assert_d41_parent_mutation_rejected(
    parts: &native::NativeArtifactParts,
    mutate: impl FnOnce(&mut native::BoundaryTraitSettlementParts),
) {
    assert_physical_child_mutation_rejected(parts, |child| {
        let native::PhysicalChildParent::BoundaryTraitSettlement(parent) = child.parent.clone()
        else {
            panic!("admitted foreign-call child must retain its D41 parent")
        };
        let mut parent = parent.into_parts();
        mutate(&mut parent);
        child.parent = native::PhysicalChildParent::BoundaryTraitSettlement(
            native::BoundaryTraitSettlement::from_replayed_parts(parent),
        );
    });
}

struct Fixture {
    root: PathBuf,
    main: PathBuf,
    target: String,
}

impl Fixture {
    fn new() -> Self {
        Self::new_named("macho")
    }

    fn new_named(name: &str) -> Self {
        Self::with_source(
            name,
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

    fn new_windows_u32_result_chain() -> Self {
        Self::with_source(
            "windows-u32-result-chain",
            "windows_x86_64",
            r#"use omega::language::core::external_binding;

target windows_x86_64 {
}

boundary trait Process {
    machine current_id() -> u32;
    machine sleep(milliseconds: u32);
}

windows_x86_64 machine current_id_binding() -> Binding<12, 19, 0> {
    Binding::DllImport {
        import: DllImport::PeByName {
            library: "kernel32.dll",
            export: "GetCurrentProcessId",
        },
    }
}

windows_x86_64 machine sleep_binding() -> Binding<12, 5, 0> {
    Binding::DllImport {
        import: DllImport::PeByName {
            library: "kernel32.dll",
            export: "Sleep",
        },
    }
}

machine current_id_leaf() -> u32
    satisfies Process::current_id
    via current_id_binding();

machine sleep_leaf(milliseconds: u32)
    satisfies Process::sleep
    via sleep_binding();

data Main { process: Process; }
machine Main::main(&mut self) {
    let current: u32 = self.process.current_id();
    self.process.sleep(current);
}
"#,
            r#"machine build(builder: &mut Build) {
    builder.application("source-evaluated-windows-u32-result-chain");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
}
"#,
        )
    }

    fn new_linux_named(name: &str, include_marker: bool) -> Self {
        let marker = if include_marker {
            "self.process.ping();"
        } else {
            ""
        };
        let source = format!(
            r#"use omega::language::core::external_binding;

target linux_x86_64 {{
}}

boundary trait Process {{
    machine ping();
}}

linux_x86_64 machine ping_binding() -> Binding<9, 6, 11> {{
    Binding::DllImport {{
        import: DllImport::ElfVersioned {{
            object: "libc.so.6",
            symbol: "getpid",
            version: "GLIBC_2.2.5",
        }},
    }}
}}

machine ping_leaf() satisfies Process::ping via ping_binding();

data Main {{ process: Process; }}
machine Main::main(&mut self) {{
    {marker}
    self.process.ping();
}}
"#,
        );
        Self::with_source(
            name,
            "linux_x86_64",
            &source,
            r#"machine build(builder: &mut Build) {
    builder.application("source-evaluated-linux-native");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}
"#,
        )
    }

    fn new_macos_u32_argument() -> Self {
        Self::new_macos_u32_argument_named("macho-u32-argument")
    }

    fn new_macos_u32_argument_named(name: &str) -> Self {
        Self::with_source(
            name,
            "macos_arm64",
            r#"use omega::language::core::external_binding;

target macos_arm64 {
}

boundary trait Delay {
    machine wait(seconds: u32);
}

macos_arm64 machine wait_binding() -> Binding<26, 6, 0> {
    Binding::DllImport {
        import: DllImport::MachODylibSymbol {
            install_name: "/usr/lib/libSystem.B.dylib",
            symbol: "_sleep",
        },
    }
}

machine wait_leaf(seconds: u32) satisfies Delay::wait via wait_binding();

data Main { delay: Delay; }
machine Main::main(&mut self) {
    self.delay.wait(3);
}
"#,
            r#"machine build(builder: &mut Build) {
    builder.application("source-evaluated-macho-u32-argument-native");
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
}
"#,
        )
    }

    fn new_macos_i32_result() -> Self {
        Self::new_macos_i32_result_named("macho-i32-result")
    }

    fn new_macos_i32_result_named(name: &str) -> Self {
        Self::with_source(
            name,
            "macos_arm64",
            r#"use omega::language::core::external_binding;

target macos_arm64 {
}

boundary trait Process {
    machine process_id() -> i32;
}

macos_arm64 machine process_id_binding() -> Binding<26, 7, 0> {
    Binding::DllImport {
        import: DllImport::MachODylibSymbol {
            install_name: "/usr/lib/libSystem.B.dylib",
            symbol: "_getpid",
        },
    }
}

machine process_id_leaf() -> i32
    satisfies Process::process_id
    via process_id_binding();

data Main { process: Process; }
machine Main::main(&mut self) {
    let observed_pid: i32 = self.process.process_id();
}
"#,
            r#"machine build(builder: &mut Build) {
    builder.application("source-evaluated-macho-i32-result-native");
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
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

#[test]
fn windows_evaluated_u32_result_reaches_a_later_pe_import_through_exact_home_custody() {
    let fixture = Fixture::new_windows_u32_result_chain();
    let retained = fixture.compile_terminal();
    let admissions = admit_imports(&retained, 0x5749_4e52_0001);
    assert_eq!(
        admissions.len(),
        2,
        "both evaluated PE leaves require custody"
    );
    let settlements = admissions
        .iter()
        .map(|admission| {
            SourceEvaluatedImportSettlement::new(&admission.execution, &admission.same_stack)
        })
        .collect::<Vec<_>>();
    let policy = terminal_authority_policy(&retained);
    let permission_policy = terminal_authority_permission_policy(&retained);
    let artifact = realize_retained_terminal_artifact_with_source_evaluated_imports_and_policy(
        retained,
        &psi_proof_admission::AdmissionProfile::default(),
        &omega_optimization_core::OptimizationSelections::default(),
        policy,
        omega_terminal_psi_to_native_artifact::current_terminal_authority_permission_policy(),
        permission_policy,
        &settlements,
    )
    .unwrap_or_else(|diagnostics| {
        panic!("Windows evaluated result chain should realize: {diagnostics:#?}")
    });

    artifact
        .validate()
        .expect("Windows evaluated result artifact independently replays");
    assert_eq!(artifact.target(), omega_target::NativeTarget::windows_x64());
    let [producer, consumer] = artifact.object().foreign_calls() else {
        panic!("the source result chain must retain two PE calls")
    };
    let result = producer
        .scalar_result
        .as_ref()
        .expect("GetCurrentProcessId retains its exact u32 result home");
    let [argument] = consumer.scalar_arguments.as_slice() else {
        panic!("Sleep retains the preceding result as its sole argument")
    };
    assert_eq!(
        argument.source,
        omega_machine_code::InternalUnitScalarArgumentSourceRecord::Home(result.home)
    );
    assert_eq!(
        result.home.scalar_type,
        psi_core::ScalarType::Integer(
            psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 32).unwrap()
        )
    );
    assert_eq!(artifact.image().output().format, "pe64-x86_64-executable");
    assert_eq!(artifact.image().output().final_image_imports, 2);
    assert_eq!(artifact.image().output().final_image_relocations, 2);
}

#[test]
fn windows_evaluated_result_rejects_cross_wired_same_stack_custody() {
    let fixture = Fixture::new_windows_u32_result_chain();
    let retained = fixture.compile_terminal();
    let admissions = admit_imports(&retained, 0x5749_4e52_1001);
    let [first, second] = admissions.as_slice() else {
        panic!("the Windows result chain must retain exactly two import admissions")
    };
    let cross_wired = [
        SourceEvaluatedImportSettlement::new(&first.execution, &second.same_stack),
        SourceEvaluatedImportSettlement::new(&second.execution, &first.same_stack),
    ];
    let policy = terminal_authority_policy(&retained);
    let permission_policy = terminal_authority_permission_policy(&retained);
    let diagnostics = realize_retained_terminal_artifact_with_source_evaluated_imports_and_policy(
        retained,
        &psi_proof_admission::AdmissionProfile::default(),
        &omega_optimization_core::OptimizationSelections::default(),
        policy,
        omega_terminal_psi_to_native_artifact::current_terminal_authority_permission_policy(),
        permission_policy,
        &cross_wired,
    )
    .expect_err("same-stack custody from the sibling PE leaf cannot authorize this result chain");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not carry same-stack custody for the exact selected provider row")
    }));
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
    assert!(
        !matches.is_empty(),
        "at least one normalized import policy row expected"
    );
    omega_terminal_psi_to_native_artifact::terminal_authority_policy_with_rows(
        matches
            .into_iter()
            .map(|(locator, boundary_entry_plan)| {
                omega_terminal_psi_to_native_artifact::TerminalAuthorityPolicyRow::new(
                    omega_terminal_psi_to_native_artifact::normalized_foreign_terminal_mechanism(
                        locator,
                        boundary_entry_plan,
                    )
                    .expect("retained foreign boundary plan is canonical"),
                    omega_effects::TerminalAuthorityDisposition::from_classes([]),
                )
            })
            .collect(),
    )
    .expect("receiving policy has exact normalized import rows")
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

fn admit_imports(
    retained: &omega_compilation_report::RetainedTerminalArtifact,
    receipt_seed: u64,
) -> Vec<AdmittedImport> {
    let proposal = retained
        .native_realization_proposal()
        .expect("retained Terminal product has a native proposal");
    proposal
        .selected_provider_plans()
        .plans()
        .iter()
        .flat_map(|plan| {
            plan.rows.iter().filter_map(move |row| {
                matches!(row.binding, ProviderBinding::Import { .. }).then_some((plan, row))
            })
        })
        .enumerate()
        .map(|(index, (plan, row))| {
            let plan_report_identity = plan.report_fingerprint();
            let plan_commitment =
                SameStackProviderPlanCommitment::from_digest(*plan.identity_digest().as_bytes());
            let requirement = row.requirement_identity.clone();
            let execution = TestProviderExecution {
                requirement: requirement.clone(),
                plan_report_identity,
            };
            let receipt_identity = receipt_seed
                .checked_add(u64::try_from(index).expect("import index fits u64"))
                .expect("test admission receipt identity does not overflow");
            let same_stack = admit_same_stack_contribution(
                SameStackContributionAdmissionCandidate {
                    provider_plan_report_identity: plan_report_identity,
                    provider_plan_commitment: plan_commitment,
                    requirement_identity: requirement.clone(),
                    receipt: SameStackContributionAdmissionReceiptId::from_normalized_identity(
                        receipt_identity,
                    )
                    .expect("nonzero test admission receipt identity"),
                    bytes: 64,
                    alignment: 16,
                },
                plan_report_identity,
                plan_commitment,
                &requirement,
            )
            .expect("exact provider-plan custody admits each foreign leaf");
            AdmittedImport {
                execution,
                same_stack,
                plan_report_identity,
            }
        })
        .collect()
}

fn realize_linux_dynamic(
    retained: omega_compilation_report::RetainedTerminalArtifact,
    receipt: u64,
) -> native::RequestedNativeArtifact {
    let admission = admit_import(
        &retained,
        SameStackContributionAdmissionReceiptId::from_normalized_identity(receipt).unwrap(),
    );
    let policy = terminal_authority_policy(&retained);
    let permission_policy = terminal_authority_permission_policy(&retained);
    let interpreter = omega_target::normalize_elf_interpreter_plan(
        b"/lib64/ld-linux-x86-64.so.2".to_vec(),
        omega_target::TargetProfile::LinuxX64,
    )
    .expect("canonical Linux x86-64 interpreter");
    realize_retained_terminal_artifact_with_source_evaluated_imports_and_policy_for_image(
        retained,
        &psi_proof_admission::AdmissionProfile::default(),
        &omega_optimization_core::OptimizationSelections::default(),
        policy,
        omega_terminal_psi_to_native_artifact::current_terminal_authority_permission_policy(),
        permission_policy,
        native::ExecutableImageEmissionRequest::dynamic_elf(interpreter),
        &[SourceEvaluatedImportSettlement::new(
            &admission.execution,
            &admission.same_stack,
        )],
    )
    .unwrap_or_else(|(_, diagnostics)| {
        panic!("import-bearing Linux request should realize: {diagnostics:#?}")
    })
}

#[test]
fn import_bearing_linux_compiler_route_retains_non_installable_dynamic_candidate() {
    let fixture = Fixture::new_linux_named("linux-dynamic", false);
    let candidate = realize_linux_dynamic(fixture.compile_terminal(), 0x454c_4600_0001);
    let native::RequestedNativeArtifact::DynamicElf(candidate) = candidate else {
        panic!("normalized ELF imports must select dynamic native custody")
    };
    candidate
        .validate()
        .expect("dynamic native candidate independently replays");
    assert_eq!(candidate.target(), omega_target::NativeTarget::linux_x64());
    assert_eq!(
        candidate.object().object().layout.normalized_imports.len(),
        1
    );
    assert_eq!(candidate.image().output().final_image_imports, 1);
    assert!(candidate.image().output().bytes.starts_with(b"\x7fELF"));

    let replay = native::DynamicElfNativeArtifact::from_replayed_parts(candidate.into_parts())
        .expect("exact dynamic native parts replay");
    let mut psi_substitution = replay.into_parts();

    let donor_fixture = Fixture::new_linux_named("linux-dynamic-marker", true);
    let donor = realize_linux_dynamic(donor_fixture.compile_terminal(), 0x454c_4600_0002);
    let native::RequestedNativeArtifact::DynamicElf(donor) = donor else {
        unreachable!("donor has one normalized ELF import")
    };
    let donor_parts = donor.into_parts();
    assert_ne!(
        psi_substitution.psi_artifact.manifest().identity(),
        donor_parts.psi_artifact.manifest().identity(),
    );
    psi_substitution.psi_artifact = donor_parts.psi_artifact;
    assert!(
        native::DynamicElfNativeArtifact::from_replayed_parts(psi_substitution).is_err(),
        "substituting only canonical Terminal PSI must fail closed",
    );

    let object_candidate = realize_linux_dynamic(fixture.compile_terminal(), 0x454c_4600_0003);
    let native::RequestedNativeArtifact::DynamicElf(object_candidate) = object_candidate else {
        unreachable!("fixture has one normalized ELF import")
    };
    let mut object_substitution = object_candidate.into_parts();
    object_substitution.object = donor_parts.object;
    assert!(
        native::DynamicElfNativeArtifact::from_replayed_parts(object_substitution).is_err(),
        "substituting the outer object while retaining the requested image must fail closed",
    );

    let rejected = fixture.compile_terminal();
    let admission = admit_import(
        &rejected,
        SameStackContributionAdmissionReceiptId::from_normalized_identity(0x454c_4600_0004)
            .unwrap(),
    );
    let policy = terminal_authority_policy(&rejected);
    let permission_policy = terminal_authority_permission_policy(&rejected);
    let (request, diagnostics) =
        realize_retained_terminal_artifact_with_source_evaluated_imports_and_policy_for_image(
            rejected,
            &psi_proof_admission::AdmissionProfile::default(),
            &omega_optimization_core::OptimizationSelections::default(),
            policy,
            omega_terminal_psi_to_native_artifact::current_terminal_authority_permission_policy(),
            permission_policy,
            native::ExecutableImageEmissionRequest::direct(91),
            &[SourceEvaluatedImportSettlement::new(
                &admission.execution,
                &admission.same_stack,
            )],
        )
        .expect_err("import-bearing ELF cannot enter direct image custody");
    assert!(matches!(
        request,
        native::ExecutableImageEmissionRequest::Direct { subsystem: 91 }
    ));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires an exact normalized interpreter")
    }));
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
    assert_eq!(
        artifact.image().output().final_data_bytes,
        [0; 8],
        "one referenced Mach-O import has one exact lazy-binding pointer slot",
    );
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

    assert_eq!(
        artifact.physical_evidence_scope(),
        native::NativePhysicalEvidenceScope::UnoptimizedCompleteBoundaryEvidence,
        "source-reviewed imports require complete unoptimized D32 custody",
    );
    let physical = artifact
        .physical_evidence()
        .expect("source-evaluated import retains complete D32 evidence");
    assert_eq!(physical.projection().operator_occurrences().len(), 0);
    assert_eq!(physical.projection().boundary_occurrences().len(), 1);
    let [child] = physical.children() else {
        panic!("one source boundary call must produce exactly one D41 child")
    };
    assert!(matches!(
        child.occurrence(),
        native::NativePhysicalOccurrence::Boundary(_),
    ));
    assert_eq!(child.projection(), physical.projection().identity());
    let native::PhysicalChildParent::BoundaryTraitSettlement(parent) = child.parent() else {
        panic!("source-evaluated import must retain its D41 settlement parent")
    };
    assert_eq!(
        parent.requirement_identity(),
        admission.execution.requirement
    );
    assert_eq!(parent.target(), artifact.target());
    let [selected_plan] = artifact.selected_provider_plans() else {
        panic!("one exact selected provider plan expected")
    };
    assert_eq!(parent.selected_plan_digest(), selected_plan.plan_digest());
    assert_eq!(
        *parent.selected_plan_digest().as_bytes(),
        admission.same_stack.provider_plan_commitment().as_bytes(),
        "D41 parent and opaque same-stack leaf must bind the same selected plan",
    );

    let native::BoundaryTraitSettlementRole::AdmittedProvider {
        execution,
        realization,
    } = parent.role()
    else {
        panic!("source-evaluated import must be an admitted-provider D41 role")
    };
    let execution_record = foreign_call.provider_execution;
    let expected_execution =
        omega_target_operations::ProviderExecutionBinding::from_execution_record(
            omega_target_operations::ProviderPlanReportIdentity::new(
                execution_record.provider_plan_report_identity,
            )
            .expect("selected provider-plan report identity"),
            execution_record.provider_execution_report_identity,
            execution_record.provider_execution_report_fingerprint,
            execution_record.normalized_root_report_identity,
            execution_record.boundary_contract_report_fingerprint,
        )
        .expect("complete admitted-provider execution record");
    assert_eq!(*execution, expected_execution);
    assert_eq!(parent.execution(), expected_execution.into());
    assert_eq!(realization.locator, foreign_call.locator);
    assert_eq!(realization.locator, normalized.locator);
    assert_eq!(
        realization.boundary_entry_plan,
        foreign_call.boundary_entry_plan,
    );
    assert_eq!(realization.same_stack_contribution, admission.same_stack,);
    assert_eq!(
        realization.same_stack_contribution,
        foreign_call.same_stack_contribution,
    );
    let [image_foreign_call] = artifact.image().foreign_calls() else {
        panic!("one final-image foreign call expected")
    };
    assert_eq!(image_foreign_call, foreign_call);

    let object_function = artifact
        .object()
        .functions()
        .iter()
        .find(|function| function.machine == foreign_call.machine)
        .expect("foreign call owning object function");
    let matching_attributions = artifact
        .object()
        .semantic_code_attribution()
        .iter()
        .filter(|attribution| {
            attribution.machine == foreign_call.machine
                && attribution.attribution.site
                    == omega_machine_code::SemanticCodeSite::Operation(
                        parent.occurrence().operation(),
                    )
                && attribution.attribution.operation_ordinal == foreign_call.operation_ordinal
        })
        .collect::<Vec<_>>();
    let [attribution] = matching_attributions.as_slice() else {
        panic!("one full semantic interval must own the zero-argument import")
    };
    let matching_relocations = artifact
        .object()
        .relocations()
        .records()
        .filter(|(_, relocation)| relocation.symbol_handle == normalized.symbol)
        .collect::<Vec<_>>();
    let [(_, object_relocation)] = matching_relocations.as_slice() else {
        panic!("one unresolved object relocation must target the normalized import")
    };
    assert_eq!(
        child.machine_span().offset(),
        attribution.attribution.code_offset,
    );
    assert_eq!(child.object_span().offset(), attribution.text_offset);
    assert_eq!(child.final_image_span(), child.object_span());
    assert_eq!(
        child.machine_span().byte_count(),
        attribution.attribution.byte_count,
    );
    assert_eq!(
        child.object_span().byte_count(),
        attribution.attribution.byte_count,
    );
    let object_interval_end = child.object_span().offset() + child.object_span().byte_count();
    assert!(child.object_span().offset() <= object_relocation.offset);
    assert!(object_relocation.offset + object_relocation.byte_width <= object_interval_end);
    assert!(child.object_span().byte_count() > object_relocation.byte_width);
    let machine_span = child.machine_span();
    let object_span = child.object_span();
    assert_eq!(
        &object_function.bytes(artifact.object())
            [machine_span.offset()..machine_span.offset() + machine_span.byte_count()],
        &artifact.object().text_bytes()
            [object_span.offset()..object_span.offset() + object_span.byte_count()],
    );
    let object_instruction = u32::from_le_bytes(
        artifact.object().text_bytes()
            [object_relocation.offset..object_relocation.offset + object_relocation.byte_width]
            .try_into()
            .expect("one AArch64 branch instruction"),
    );
    let final_instruction = u32::from_le_bytes(
        artifact.image().output().final_text_bytes
            [object_relocation.offset..object_relocation.offset + object_relocation.byte_width]
            .try_into()
            .expect("one final AArch64 branch instruction"),
    );
    assert_eq!(object_instruction & 0xfc00_0000, 0x9400_0000);
    assert_eq!(final_instruction & 0xfc00_0000, 0x9400_0000);
    assert_ne!(
        object_instruction, final_instruction,
        "Mach-O import lowering must relocate the admitted call to its exact image thunk",
    );

    let native::PhysicalRelocationDisposition::UnresolvedNormalizedForeignCall(relocation) =
        child.relocation()
    else {
        panic!("admitted foreign call must retain unresolved import relocation custody")
    };
    let boundary_plan_identity = omega_calling_conventions::validate_boundary_entry_plan(
        realization.boundary_entry_plan.clone(),
        &omega_calling_conventions::CallSignature {
            parameters: Vec::new(),
            result: None,
        },
    )
    .expect("retained zero-argument boundary entry plan revalidates")
    .contract_commitment_digest();
    assert_eq!(
        *relocation.locator_identity(),
        realization.locator.identity_digest().as_bytes(),
    );
    assert_eq!(relocation.boundary_plan_identity(), &boundary_plan_identity);
    assert_eq!(relocation.object_symbol(), normalized.symbol);
    assert_eq!(relocation.origin(), object_relocation.origin);
    assert_eq!(relocation.offset(), object_relocation.offset);
    assert_eq!(relocation.byte_width(), object_relocation.byte_width);
    assert_eq!(relocation.addend(), object_relocation.addend);
    assert_eq!(relocation.kind(), object_relocation.kind);
    assert_ne!(relocation.final_image_symbol_identity(), &[0; 32]);

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

    let parts = artifact.into_parts();
    let mut missing_provider = replay_native_artifact_parts(&parts);
    missing_provider.provider_executions.clear();
    assert!(
        native::NativeArtifact::from_replayed_parts(missing_provider).is_err(),
        "D41 replay must reject removal of its admitted provider execution",
    );
    let mut duplicate_provider = replay_native_artifact_parts(&parts);
    duplicate_provider
        .provider_executions
        .push(duplicate_provider.provider_executions[0].clone());
    assert!(
        native::NativeArtifact::from_replayed_parts(duplicate_provider).is_err(),
        "D41 replay must reject duplicate admitted provider execution",
    );

    let mut missing_child = replay_native_artifact_parts(&parts);
    let evidence = missing_child
        .physical_evidence
        .take()
        .expect("admitted foreign-call physical evidence")
        .into_parts();
    missing_child.physical_evidence = Some(native::NativePhysicalEvidence::from_replayed_parts(
        native::NativePhysicalEvidenceParts {
            projection: evidence.projection,
            children: Vec::new(),
            identity: evidence.identity,
        },
    ));
    assert!(
        native::NativeArtifact::from_replayed_parts(missing_child).is_err(),
        "D32 replay must reject a missing D41 child",
    );

    let mut duplicate_child = replay_native_artifact_parts(&parts);
    let evidence = duplicate_child
        .physical_evidence
        .take()
        .expect("admitted foreign-call physical evidence")
        .into_parts();
    let mut children = evidence.children;
    children.push(children[0].clone());
    duplicate_child.physical_evidence = Some(native::NativePhysicalEvidence::from_replayed_parts(
        native::NativePhysicalEvidenceParts {
            projection: evidence.projection,
            children,
            identity: evidence.identity,
        },
    ));
    assert!(
        native::NativeArtifact::from_replayed_parts(duplicate_child).is_err(),
        "D32 replay must reject duplicate D41 children",
    );

    assert_d41_parent_mutation_rejected(&parts, |parent| {
        parent.requirement_identity.push_str("::substituted");
    });
    assert_d41_parent_mutation_rejected(&parts, |parent| {
        parent.selected_plan_digest =
            native::NativeSelectedProviderPlanDigest::from_digest([7; 32]);
    });
    assert_d41_parent_mutation_rejected(&parts, |parent| {
        let native::BoundaryTraitSettlementRole::AdmittedProvider { execution, .. } =
            &mut parent.role
        else {
            panic!("fixture D41 parent is admitted-provider custody")
        };
        *execution = omega_target_operations::ProviderExecutionBinding::from_execution_record(
            omega_target_operations::ProviderPlanReportIdentity::new(
                admission.plan_report_identity,
            )
            .expect("selected provider-plan report identity"),
            0x4d41_4348_ffff,
            execution.provider_execution_report_fingerprint(),
            execution.normalized_root_report_identity(),
            execution.boundary_contract_report_fingerprint(),
        )
        .expect("substituted provider execution is structurally complete");
    });
    assert_d41_parent_mutation_rejected(&parts, |parent| {
        let native::BoundaryTraitSettlementRole::AdmittedProvider { realization, .. } =
            &mut parent.role
        else {
            panic!("fixture D41 parent is admitted-provider custody")
        };
        realization.locator = omega_target::normalize_foreign_locator(
            ForeignLocatorCandidate::MachODylibSymbol {
                install_name: INSTALL_NAME.to_vec(),
                symbol: b"_substituted_getpid".to_vec(),
            },
            omega_target::TargetProfile::MacosArm64,
        )
        .expect("substituted locator remains structurally valid");
    });
    assert_d41_parent_mutation_rejected(&parts, |parent| {
        let native::BoundaryTraitSettlementRole::AdmittedProvider { realization, .. } =
            &mut parent.role
        else {
            panic!("fixture D41 parent is admitted-provider custody")
        };
        realization.boundary_entry_plan.state.preemption =
            omega_calling_conventions::Preemption::ProviderDefined;
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.relocation = native::PhysicalRelocationDisposition::DirectInstructionBytes;
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.machine_bytes_digest[0] ^= 1;
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.object_bytes_digest[0] ^= 1;
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.final_image_bytes_digest[0] ^= 1;
    });
}

#[test]
fn optimized_source_evaluated_import_retains_exact_d32_d41_custody() {
    for (label, fixture, receipt_identity) in [
        (
            "unit",
            Fixture::new_named("optimized-macho"),
            0x4d41_4348_5f05,
        ),
        (
            "fixed-u32-argument",
            Fixture::new_macos_u32_argument_named("optimized-macho-u32-argument"),
            0x4d41_4348_6f05,
        ),
        (
            "fixed-i32-result",
            Fixture::new_macos_i32_result_named("optimized-macho-i32-result"),
            0x4d41_4348_7f05,
        ),
    ] {
        let retained = fixture.compile_terminal();
        let admission = admit_import(
            &retained,
            SameStackContributionAdmissionReceiptId::from_normalized_identity(receipt_identity)
                .unwrap(),
        );
        let policy = terminal_authority_policy(&retained);
        let permission_policy = terminal_authority_permission_policy(&retained);
        let optimizations = omega_optimization_core::OptimizationSelections::new([
            omega_optimization_core::Optimization::ControlFlowCleanup,
        ])
        .expect("one verified Psi optimization selection");
        let artifact = realize_retained_terminal_artifact_with_source_evaluated_imports_and_policy(
            retained,
            &psi_proof_admission::AdmissionProfile::default(),
            &optimizations,
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
                "optimized {label} import should retain physical custody:\n{}",
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });

        artifact
            .validate()
            .unwrap_or_else(|error| panic!("optimized {label} artifact must replay: {error}"));
        assert!(matches!(
            artifact.physical_evidence_scope(),
            native::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_),
        ));
        let physical = artifact
            .physical_evidence()
            .unwrap_or_else(|| panic!("optimized {label} import must retain D32 evidence"));
        let [occurrence] = physical.projection().boundary_occurrences() else {
            panic!("optimized {label} projection must retain one foreign-call survivor")
        };
        let [child] = physical.children() else {
            panic!("optimized {label} survivor must produce one D41 child")
        };
        assert_eq!(
            child.occurrence(),
            native::NativePhysicalOccurrence::Boundary(occurrence.identity()),
        );
        assert_eq!(child.projection(), physical.projection().identity());
        let native::PhysicalChildParent::BoundaryTraitSettlement(parent) = child.parent() else {
            panic!("optimized {label} import must retain an admitted-provider D41 parent")
        };
        assert!(matches!(
            parent.role(),
            native::BoundaryTraitSettlementRole::AdmittedProvider { .. },
        ));
        assert_eq!(parent.occurrence().identity(), occurrence.identity());
        assert_eq!(
            parent.requirement_identity(),
            admission.execution.requirement,
        );

        let mut missing_child = artifact.into_parts();
        let evidence = missing_child
            .physical_evidence
            .take()
            .expect("optimized normalized import has physical evidence")
            .into_parts();
        missing_child.physical_evidence =
            Some(native::NativePhysicalEvidence::from_replayed_parts(
                native::NativePhysicalEvidenceParts {
                    projection: evidence.projection,
                    children: Vec::new(),
                    identity: evidence.identity,
                },
            ));
        assert!(
            native::NativeArtifact::from_replayed_parts(missing_child).is_err(),
            "optimized {label} replay must reject removal of its exact D41 child",
        );
    }
}

#[test]
fn retained_source_evaluated_fixed_u32_import_requires_complete_d32_custody() {
    let fixture = Fixture::new_macos_u32_argument();
    let retained = fixture.compile_terminal();
    let admission = admit_import(
        &retained,
        SameStackContributionAdmissionReceiptId::from_normalized_identity(0x4d41_4348_0605)
            .unwrap(),
    );
    let policy = terminal_authority_policy(&retained);
    let permission_policy = terminal_authority_permission_policy(&retained);
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
            "fixed-scalar admitted import should realize complete D32 evidence:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    artifact
        .validate()
        .expect("fixed-scalar native artifact replays");
    assert_eq!(
        artifact.physical_evidence_scope(),
        native::NativePhysicalEvidenceScope::UnoptimizedCompleteBoundaryEvidence,
    );
    let physical = artifact
        .physical_evidence()
        .expect("fixed-scalar import retains complete D32 evidence");
    assert_eq!(physical.projection().operator_occurrences().len(), 0);
    assert_eq!(physical.projection().boundary_occurrences().len(), 1);
    let [child] = physical.children() else {
        panic!("one fixed-scalar source call must produce exactly one D41 child")
    };
    assert!(matches!(
        child.occurrence(),
        native::NativePhysicalOccurrence::Boundary(_),
    ));
    assert_eq!(child.projection(), physical.projection().identity());

    let [foreign_call] = artifact.object().foreign_calls() else {
        panic!("one fixed-scalar Mach-O foreign call expected")
    };
    assert_eq!(foreign_call.boundary_entry_plan.call.parameters.len(), 1);
    let [scalar_argument] = foreign_call.scalar_arguments.as_slice() else {
        panic!("one exact fixed-scalar object argument row expected")
    };
    assert_eq!(scalar_argument.parameter_index, 0);
    assert_eq!(
        scalar_argument.placement,
        foreign_call.boundary_entry_plan.call.parameters[0],
    );
    assert!(matches!(
        scalar_argument.source,
        omega_machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            scalar_type,
            value: psi_core::IntegerValue::Unsigned(3),
            ..
        } if scalar_type
            == psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 32).unwrap()
    ));
    let ForeignLocatorCandidate::MachODylibSymbol {
        install_name,
        symbol,
    } = foreign_call.locator.locator()
    else {
        panic!("structured scalar Mach-O locator must survive object construction")
    };
    assert_eq!(install_name, INSTALL_NAME);
    assert_eq!(symbol, SCALAR_SYMBOL);

    let native::PhysicalChildParent::BoundaryTraitSettlement(parent) = child.parent() else {
        panic!("fixed-scalar import must retain its D41 settlement parent")
    };
    assert_eq!(
        parent.requirement_identity(),
        admission.execution.requirement
    );
    assert_eq!(parent.target(), artifact.target());
    assert_eq!(
        parent.occurrence().operation_ordinal(),
        foreign_call.operation_ordinal,
    );
    let [selected_plan] = artifact.selected_provider_plans() else {
        panic!("one exact fixed-scalar provider plan expected")
    };
    assert_eq!(parent.selected_plan_digest(), selected_plan.plan_digest());
    let native::BoundaryTraitSettlementRole::AdmittedProvider {
        execution,
        realization,
    } = parent.role()
    else {
        panic!("fixed-scalar import must retain admitted-provider D41 custody")
    };
    assert_eq!(parent.execution(), (*execution).into());
    assert_eq!(realization.locator, foreign_call.locator);
    assert_eq!(
        realization.boundary_entry_plan,
        foreign_call.boundary_entry_plan,
    );
    assert_eq!(realization.boundary_entry_plan.call.parameters.len(), 1);
    assert_eq!(realization.same_stack_contribution, admission.same_stack);
    assert_eq!(
        realization.same_stack_contribution,
        foreign_call.same_stack_contribution,
    );

    let matching_attributions = artifact
        .object()
        .semantic_code_attribution()
        .iter()
        .filter(|attribution| {
            attribution.machine == foreign_call.machine
                && attribution.attribution.site
                    == omega_machine_code::SemanticCodeSite::Operation(
                        parent.occurrence().operation(),
                    )
                && attribution.attribution.operation_ordinal == foreign_call.operation_ordinal
        })
        .collect::<Vec<_>>();
    let [attribution] = matching_attributions.as_slice() else {
        panic!("one full semantic interval must own the fixed-scalar import")
    };
    assert_eq!(
        child.machine_span().offset(),
        attribution.attribution.code_offset
    );
    assert_eq!(child.object_span().offset(), attribution.text_offset);
    assert_eq!(
        child.object_span().byte_count(),
        attribution.attribution.byte_count,
    );
    let object_interval_end = child.object_span().offset() + child.object_span().byte_count();
    assert!(child.object_span().offset() <= scalar_argument.code_offset);
    assert!(scalar_argument.code_offset + scalar_argument.byte_count <= object_interval_end);
    let [normalized] = artifact
        .object()
        .object()
        .layout
        .normalized_imports
        .as_slice()
    else {
        panic!("one normalized fixed-scalar object import expected")
    };
    let matching_relocations = artifact
        .object()
        .relocations()
        .records()
        .filter(|(_, relocation)| relocation.symbol_handle == normalized.symbol)
        .collect::<Vec<_>>();
    let [(_, object_relocation)] = matching_relocations.as_slice() else {
        panic!("one unresolved object relocation must target the scalar import")
    };
    assert!(child.object_span().offset() <= object_relocation.offset);
    assert!(object_relocation.offset + object_relocation.byte_width <= object_interval_end);
    assert!(child.object_span().byte_count() > object_relocation.byte_width);

    let parts = artifact.into_parts();
    assert_d41_parent_mutation_rejected(&parts, |parent| {
        let native::BoundaryTraitSettlementRole::AdmittedProvider { realization, .. } =
            &mut parent.role
        else {
            panic!("fixture D41 parent is admitted-provider custody")
        };
        realization.boundary_entry_plan.call.parameters.clear();
    });
    assert_d41_parent_mutation_rejected(&parts, |parent| {
        let native::BoundaryTraitSettlementRole::AdmittedProvider { realization, .. } =
            &mut parent.role
        else {
            panic!("fixture D41 parent is admitted-provider custody")
        };
        realization.boundary_entry_plan.call.parameters[0]
            .locations
            .clear();
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.relocation = native::PhysicalRelocationDisposition::DirectInstructionBytes;
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.machine_bytes_digest[0] ^= 1;
    });
}

#[test]
fn retained_source_evaluated_fixed_i32_result_requires_complete_d32_custody() {
    let fixture = Fixture::new_macos_i32_result();
    let retained = fixture.compile_terminal();
    let admission = admit_import(
        &retained,
        SameStackContributionAdmissionReceiptId::from_normalized_identity(0x4d41_4348_0705)
            .unwrap(),
    );
    let policy = terminal_authority_policy(&retained);
    let permission_policy = terminal_authority_permission_policy(&retained);
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
            "fixed-result admitted import should realize complete D32 evidence:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    artifact.validate().expect("fixed-result artifact replays");
    assert_eq!(
        artifact.physical_evidence_scope(),
        native::NativePhysicalEvidenceScope::UnoptimizedCompleteBoundaryEvidence,
    );
    let physical = artifact
        .physical_evidence()
        .expect("fixed-result import retains complete D32 evidence");
    let [child] = physical.children() else {
        panic!("one fixed-result source call must produce one D41 child")
    };
    let [foreign_call] = artifact.object().foreign_calls() else {
        panic!("one fixed-result object call expected")
    };
    assert!(foreign_call.scalar_arguments.is_empty());
    let scalar_result = foreign_call
        .scalar_result
        .as_ref()
        .expect("object call retains its fixed scalar result");
    let i32_type = psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32).unwrap();
    let i32_shape = omega_calling_conventions::ValueShape::integer(4, 4);
    assert_eq!(
        scalar_result.home.scalar_type,
        psi_core::ScalarType::Integer(i32_type)
    );
    assert_eq!(scalar_result.home.shape, i32_shape);
    assert_eq!(
        foreign_call.boundary_entry_plan.call.result.as_ref(),
        Some(&scalar_result.source),
    );

    let native::PhysicalChildParent::BoundaryTraitSettlement(parent) = child.parent() else {
        panic!("fixed-result import must retain its D41 parent")
    };
    assert_eq!(
        parent.requirement_identity(),
        admission.execution.requirement,
    );
    assert_eq!(
        parent.occurrence().operation_ordinal(),
        foreign_call.operation_ordinal,
    );
    let native::BoundaryTraitSettlementRole::AdmittedProvider {
        execution,
        realization,
    } = parent.role()
    else {
        panic!("fixed-result import must retain admitted-provider custody")
    };
    assert_eq!(parent.execution(), (*execution).into());
    assert_eq!(realization.locator, foreign_call.locator);
    assert_eq!(
        realization.boundary_entry_plan,
        foreign_call.boundary_entry_plan,
    );
    assert_eq!(realization.same_stack_contribution, admission.same_stack);

    let module = psi_terminal_codec::decode_module(artifact.psi_artifact().semantic_bytes())
        .expect("decode fixed-result Terminal semantics");
    let matching_operations = module
        .machines
        .iter()
        .filter(|machine| machine.id == parent.occurrence().machine())
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == parent.occurrence().operation())
        .collect::<Vec<_>>();
    let [operation] = matching_operations.as_slice() else {
        panic!("D41 parent must rejoin one Terminal result producer")
    };
    let psi_terminal::OperationResult::Scalar(terminal_result) = operation.result else {
        panic!("Terminal boundary call must retain its scalar result")
    };
    assert_eq!(terminal_result.id, scalar_result.home.source_value);
    assert_eq!(
        terminal_result.scalar_type,
        psi_core::ScalarType::Integer(i32_type),
    );

    let matching_attributions = artifact
        .object()
        .semantic_code_attribution()
        .iter()
        .filter(|attribution| {
            attribution.machine == foreign_call.machine
                && attribution.attribution.site
                    == omega_machine_code::SemanticCodeSite::Operation(operation.id)
                && attribution.attribution.operation_ordinal == foreign_call.operation_ordinal
        })
        .collect::<Vec<_>>();
    let [attribution] = matching_attributions.as_slice() else {
        panic!("one full semantic interval must own the fixed-result import")
    };
    assert_eq!(
        child.machine_span().offset(),
        attribution.attribution.code_offset,
    );
    assert_eq!(child.object_span().offset(), attribution.text_offset);
    assert_eq!(
        child.object_span().byte_count(),
        attribution.attribution.byte_count,
    );
    assert_eq!(child.final_image_span(), child.object_span());
    let object_end = child.object_span().offset() + child.object_span().byte_count();
    assert!(child.object_span().offset() <= scalar_result.code_offset);
    assert!(scalar_result.code_offset + scalar_result.byte_count <= object_end);

    let [normalized] = artifact
        .object()
        .object()
        .layout
        .normalized_imports
        .as_slice()
    else {
        panic!("one normalized fixed-result import expected")
    };
    let matching_relocations = artifact
        .object()
        .relocations()
        .records()
        .filter(|(_, relocation)| relocation.symbol_handle == normalized.symbol)
        .collect::<Vec<_>>();
    let [(_, object_relocation)] = matching_relocations.as_slice() else {
        panic!("one unresolved relocation must target the fixed-result import")
    };
    assert!(child.object_span().offset() <= object_relocation.offset);
    assert!(object_relocation.offset + object_relocation.byte_width <= object_end);
    assert!(child.object_span().byte_count() > object_relocation.byte_width);
    let native::PhysicalRelocationDisposition::UnresolvedNormalizedForeignCall(relocation) =
        child.relocation()
    else {
        panic!("fixed-result child must retain unresolved import custody")
    };
    assert_eq!(relocation.offset(), object_relocation.offset);
    assert_eq!(relocation.byte_width(), object_relocation.byte_width);
    let [image_foreign_call] = artifact.image().foreign_calls() else {
        panic!("one fixed-result image call expected")
    };
    assert_eq!(image_foreign_call, foreign_call);

    let parts = artifact.into_parts();
    assert_d41_parent_mutation_rejected(&parts, |parent| {
        let native::BoundaryTraitSettlementRole::AdmittedProvider { realization, .. } =
            &mut parent.role
        else {
            panic!("fixture D41 parent is admitted-provider custody")
        };
        realization.boundary_entry_plan.call.result = None;
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.relocation = native::PhysicalRelocationDisposition::DirectInstructionBytes;
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.final_image_bytes_digest[0] ^= 1;
    });
}
