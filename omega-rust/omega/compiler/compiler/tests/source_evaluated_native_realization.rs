//! Fixtures shared by the source-evaluated native realization tests:
//! replayed artifact parts, mutation rejections, the fixture project,
//! admitted imports and flattened Mach-O installation.

#[path = "source_evaluated_native_realization/linux_arm64_hosted_receiver.rs"]
mod linux_arm64_hosted_receiver;
#[path = "source_evaluated_native_realization/linux_dynamic_realization.rs"]
mod linux_dynamic_realization;
#[path = "source_evaluated_native_realization/linux_hosted_receiver.rs"]
mod linux_hosted_receiver;
#[path = "source_evaluated_native_realization/macho_and_terminal_imports.rs"]
mod macho_and_terminal_imports;
#[path = "source_evaluated_native_realization/ranked_control.rs"]
mod ranked_control;
#[path = "source_evaluated_native_realization/windows_hosted_receiver.rs"]
mod windows_hosted_receiver;
#[path = "source_evaluated_native_realization/windows_imports_and_mxcsr_custody.rs"]
mod windows_imports_and_mxcsr_custody;

#[path = "support/console_acceptance.rs"]
mod console_acceptance;
#[path = "support/linux_entry_acceptance.rs"]
mod linux_entry_acceptance;
#[path = "support/windows_entry_acceptance.rs"]
mod windows_entry_acceptance;

use compiler::{
    CompileOptions, CompileRequest, RequestedCompileProduct, RetainedNativeRealizationRequest,
    SourceEvaluatedImportSettlement, compile, realize_retained_native_artifact,
};
use effects::provider_plan::ProviderBinding;
use installation_evidence::ProviderExecutionEvidence;
use std::fs;
use std::path::PathBuf;
use task_plans::{
    AdmittedSameStackContribution, SameStackContributionAdmissionCandidate,
    SameStackContributionAdmissionReceiptId, SameStackProviderPlanCommitment,
    admit_same_stack_contribution,
};

use native_realization as native;

const INSTALL_NAME: &[u8] = b"/usr/lib/libSystem.B.dylib";

const SYMBOL: &[u8] = b"_getpid";

const SCALAR_SYMBOL: &[u8] = b"_sleep";

fn replay_native_artifact_parts(
    parts: &native::NativeArtifactParts,
) -> native::NativeArtifactParts {
    let module = terminal_codec::decode_module(parts.psi_artifact.semantic_bytes())
        .expect("replay Terminal semantics");
    let proof = terminal_codec::decode_proof_bundle(parts.psi_artifact.proof_bytes())
        .expect("replay Terminal proof");
    let debug = parts
        .psi_artifact
        .debug_bytes()
        .map(|bytes| terminal_codec::decode_debug_map(&module, bytes).expect("debug map"));
    native::NativeArtifactParts {
        target: parts.target,
        psi_artifact: terminal_codec::CanonicalTerminalArtifact::from_parts(
            &module,
            &proof,
            parts.psi_artifact.optimization(),
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

/// Replay a copy of the admitted evidence after mutating its retained child
/// set. Independent artifact validation re-derives the exact
/// survivor/physical-child bijection, so a missing, duplicate, substituted, or
/// role-swapped child must never re-enter as accepted custody.
fn assert_physical_children_mutation_rejected(
    parts: &native::NativeArtifactParts,
    mutate: impl FnOnce(&mut Vec<native::NativePhysicalChild>),
) {
    let mut replay = replay_native_artifact_parts(parts);
    let evidence = replay
        .physical_evidence
        .take()
        .expect("admitted foreign-call physical evidence")
        .into_parts();
    let mut children = evidence.children;
    mutate(&mut children);
    replay.physical_evidence = Some(native::NativePhysicalEvidence::from_replayed_parts(
        native::NativePhysicalEvidenceParts {
            projection: evidence.projection,
            children,
            identity: evidence.identity,
        },
    ));
    assert!(
        native::NativeArtifact::from_replayed_parts(replay).is_err(),
        "mutated admitted-provider physical child set must not replay",
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
    package_inputs: Option<package_compilation::PackageCompilationInputs>,
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
machine Main::main(&mut self) reaches Process {
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
machine Main::main(&mut self) reaches Process {
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
machine Main::main(&mut self) reaches Process {
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
machine Main::main(&mut self) reaches Process {{
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

    /// A top-level `boundary requirement` (not a trait member) satisfied by an
    /// external `via` leaf: the direct call keeps the requirement seam, so the
    /// demanded boundary is the requirement machine's normalized overload
    /// identity and the same identity keys the selected import row.
    fn new_linux_boundary_requirement_named(name: &str) -> Self {
        Self::with_source(
            name,
            "linux_x86_64",
            r#"use omega::language::core::external_binding;

pub data ForeignMath {}

pub boundary requirement ForeignMath::exit_with(code: i32);

data ForeignMathProvider {}

linux_x86_64 machine foreign_exit_binding() -> Binding<9, 4, 11> {
    Binding::DllImport {
        import: DllImport::ElfVersioned {
            object: "libc.so.6",
            symbol: "exit",
            version: "GLIBC_2.2.5",
        },
    }
}

machine ForeignMathProvider::exit_leaf(code: i32)
satisfies ForeignMath::exit_with via foreign_exit_binding();

data Main {}
machine Main::main(&mut self) {
    ForeignMath::exit_with(70);
}
"#,
            r#"machine build(builder: &mut Build) {
    builder.application("source-evaluated-linux-requirement-native");
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
machine Main::main(&mut self) reaches Delay {
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
machine Main::main(&mut self) reaches Process {
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
            package_inputs: None,
        }
    }

    fn with_package_inputs(
        mut self,
        inputs: package_compilation::PackageCompilationInputs,
    ) -> Self {
        self.package_inputs = Some(inputs);
        self
    }

    fn compile_terminal(&self) -> compilation_report::RetainedTerminalArtifact {
        let mut request = CompileRequest::new(CompileOptions {
            root_path: self.main.clone(),
            build_dir: Some(self.root.join("build")),
            target_name: Some(self.target.clone()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact);
        if let Some(inputs) = &self.package_inputs {
            // Package acceptance declares terminal authority; the receiving
            // permission policy mirrors the exact accepted rows, as the
            // production compile route does.
            let permission_policy =
                native_realization::terminal_authority_permission_policy_with_rows(
                    inputs
                        .accepted_semantic_bindings()
                        .flat_map(|binding| binding.terminal_authority_permissions())
                        .cloned()
                        .collect(),
                )
                .expect("fixture accepted permissions form a receiving policy");
            request = request
                .with_terminal_authority_permission_policy(permission_policy)
                .with_package_inputs(inputs.clone());
        }
        compile(request).and_then(compiler::CompileOutcomes::into_single_report)
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
    retained: &compilation_report::RetainedTerminalArtifact,
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
    retained: &compilation_report::RetainedTerminalArtifact,
) -> native_realization::TerminalAuthorityPolicy {
    let proposal = retained
        .native_realization_proposal()
        .expect("retained Terminal product has a native proposal");
    let matches = proposal
        .external_binding_rows()
        .iter()
        .filter_map(|row| {
            let calling_conventions::ExternalBindingKind::Import { locator } = &row.binding else {
                return None;
            };
            Some((locator, row.boundary_entry_plan.as_ref()?))
        })
        .collect::<Vec<_>>();
    assert!(
        !matches.is_empty(),
        "at least one normalized import policy row expected"
    );
    native_realization::terminal_authority_policy_with_rows(
        matches
            .into_iter()
            .map(|(locator, boundary_entry_plan)| {
                native_realization::TerminalAuthorityPolicyRow::new(
                    native_realization::normalized_foreign_terminal_mechanism(
                        locator,
                        boundary_entry_plan,
                    )
                    .expect("retained foreign boundary plan is canonical"),
                    effects::TerminalAuthorityDisposition::from_classes([]),
                )
            })
            .collect(),
    )
    .expect("receiving policy has exact normalized import rows")
}

fn terminal_authority_permission_policy(
    retained: &compilation_report::RetainedTerminalArtifact,
) -> native_realization::TerminalAuthorityPermissionPolicy {
    let proposal = retained
        .native_realization_proposal()
        .expect("retained Terminal product has a native proposal");
    let rows = proposal
        .selected_provider_plans()
        .plans()
        .iter()
        .flat_map(|plan| {
            plan.rows
                .iter()
                .filter(|&row| matches!(row.binding, ProviderBinding::Import { .. }))
                .map(|row| {
                    native_realization::TerminalAuthorityPermissionPolicyRow::new(
                        plan.schema.identity_digest(),
                        row.requirement_identity.clone(),
                        effects::TerminalAuthorityDisposition::from_classes([]),
                    )
                })
        })
        .collect();
    native_realization::terminal_authority_permission_policy_with_rows(rows)
        .expect("exact source-evaluated import permissions")
}

struct AdmittedImport {
    execution: TestProviderExecution,
    same_stack: AdmittedSameStackContribution,
    plan_report_identity: u64,
}

fn admit_import(
    retained: &compilation_report::RetainedTerminalArtifact,
    receipt: SameStackContributionAdmissionReceiptId,
) -> AdmittedImport {
    let (requirement, plan_report_identity, plan_commitment) = import_coordinates(retained);
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
    retained: &compilation_report::RetainedTerminalArtifact,
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

/// Run the executable-installation ladder over one canonical artifact image:
/// container admission, placement-authority claim, materialization under the
/// supplied symbolic resolver, frozen placement, final validation, and the
/// installation receipt. The result is the `InstalledCode` occurrence that
/// `bind_installed_artifact` may join to the emitted image only when the
/// artifact carries the complete encoded bytes and the resolver materializes
/// the complete final image.
fn install_flattened_macho_image(
    code: Vec<u8>,
    entries: Vec<executable_installation::ArtifactEntry>,
    relocations: Vec<executable_installation::DecodedArtifactRelocation>,
    placement_base: u64,
    resolve: impl FnMut(layout_plans::RelocationTarget) -> Option<u64>,
) -> executable_installation::InstalledCode {
    let scope = layout_plans::ArtifactInstallationScopeId::from_normalized_identity(0x5001)
        .expect("artifact installation scope");
    let constraints = layout_plans::PlacementConstraints::new(
        None,
        16,
        layout_plans::PlacementPhase::Load,
        None,
        Some(scope),
    )
    .expect("placement constraints");
    let extent_len = u64::try_from(code.len()).expect("extent length");
    let contracts = executable_installation::MachineContractSetId::from_normalized_identity(0x5002)
        .expect("contract set");
    let footprint = executable_installation::MachineFootprintId::from_normalized_identity(0x5003)
        .expect("footprint");
    let artifact = executable_installation::Artifact::from_canonical_decode(
        executable_installation::ArtifactId::from_normalized_identity(0x5004)
            .expect("artifact identity"),
        target::NativeTarget::macos_arm64().architecture,
        code,
        contracts,
        footprint,
        executable_installation::PlacementPlanId::from_normalized_identity(0x5005)
            .expect("placement plan"),
        constraints,
        executable_installation::EntrySetId::from_normalized_identity(0x5006).expect("entry set"),
        entries,
        executable_installation::RelocationSetId::from_normalized_identity(0x5007)
            .expect("relocation set"),
        relocations,
        executable_installation::ArtifactAuthorityCommitments::from_canonical_evidence(
            contracts,
            b"test-machine-contracts-v1",
            footprint,
            b"test-machine-footprint-v1",
            None,
            Some((scope, b"test-installation-scope-v1")),
        ),
    )
    .expect("canonical artifact");
    let admitted = executable_installation::admit_executable(
        &artifact,
        executable_installation::ArtifactAdmissionEvidence::from_validator(
            executable_installation::AdmissionReceiptId::from_normalized_identity(0x5008)
                .expect("admission receipt"),
            &artifact,
            true,
        ),
    )
    .expect("admitted artifact");
    let rights = extents::ExtentRights::from_normalized_identities([
        extents::ExtentRightId::from_normalized_identity(0x5009).expect("extent right"),
    ]);
    let extent = extents::ExtentRootGrant::from_admitted_provider(
        extents::ExtentProviderIssuance::from_normalized_identities([
            0x500a, 0x500b, 0x500c, 0x500d, 0x500e, 0x500f, 0x5010, 0x5011, 0x5012, 0x5013, 0x5014,
            0x5015, 0x5016,
        ])
        .expect("extent issuance"),
        extents::ExtentLineageId::from_normalized_identity(0x5017).expect("lineage"),
        extents::AddressSpaceId::from_normalized_identity(0x5018).expect("address space"),
        rights.clone(),
        extents::ExtentProvenanceId::from_normalized_identity(0x5019).expect("provenance"),
        extents::MappingEraId::from_normalized_identity(0x501a).expect("era"),
    )
    .mint(placement_base, extent_len)
    .expect("placement extent");
    let placement = executable_installation::CodePlacementAuthority::from_admitted_provider(
        executable_installation::CodePlacementId::from_normalized_identity(0x501b)
            .expect("placement"),
        // The authority's scope identity must equal the artifact installation
        // scope the placement site carries.
        executable_installation::InstallationScopeId::from_normalized_identity(0x5001)
            .expect("installation scope"),
        executable_installation::InstallationAudience::DormantLocal,
        &extent,
        rights,
        constraints,
        layout_plans::PlacementSite {
            base_address: placement_base,
            phase: layout_plans::PlacementPhase::Load,
            machine_regime: None,
            installation_scope: Some(scope),
        },
    )
    .claim(extent)
    .expect("placement claim");
    let materialized =
        executable_installation::materialize_admitted_artifact(&admitted, &placement, resolve)
            .expect("materialized artifact");
    let frozen = executable_installation::materialize_and_freeze(
        &admitted,
        placement,
        materialized.clone(),
        executable_installation::MaterializationReceipt::from_materialized(
            &materialized,
            executable_installation::MachineFootprintId::from_normalized_identity(0x501d)
                .expect("materialized footprint"),
            true,
        ),
    )
    .expect("frozen artifact");
    let validation = executable_installation::FinalValidationCertificate::from_validator(
        executable_installation::FinalValidationId::from_normalized_identity(0x501e)
            .expect("validation"),
        &frozen,
        true,
    );
    let validated = executable_installation::validate_final_placement(frozen, &validation)
        .expect("validated artifact");
    let authority = executable_installation::InstallAuthority::from_admitted_provider(&validated);
    let receipt = executable_installation::InstallationReceipt::from_provider(
        executable_installation::InstalledCodeId::from_normalized_identity(0x501f)
            .expect("installed code"),
        &validated,
        true,
        executable_installation::WxEnforcement::HardwareEnforced,
    );
    executable_installation::install_validated(validated, authority, receipt)
        .expect("installed code")
}

fn realize_linux_dynamic(
    retained: compilation_report::RetainedTerminalArtifact,
    receipt: u64,
) -> native::RequestedNativeArtifact {
    realize_linux_dynamic_outcome(retained, receipt).unwrap_or_else(|(_, diagnostics)| {
        panic!("import-bearing Linux request should realize: {diagnostics:#?}")
    })
}

fn realize_linux_dynamic_outcome(
    retained: compilation_report::RetainedTerminalArtifact,
    receipt: u64,
) -> Result<
    native::RequestedNativeArtifact,
    (
        native::ExecutableImageEmissionRequest,
        Vec<diagnostics::Diagnostic>,
    ),
> {
    let admission = admit_import(
        &retained,
        SameStackContributionAdmissionReceiptId::from_normalized_identity(receipt).unwrap(),
    );
    let policy = terminal_authority_policy(&retained);
    let permission_policy = terminal_authority_permission_policy(&retained);
    let interpreter = target::normalize_elf_interpreter_plan(
        b"/lib64/ld-linux-x86-64.so.2".to_vec(),
        target::TargetProfile::LinuxX64,
    )
    .expect("canonical Linux x86-64 interpreter");
    realize_retained_native_artifact(
        retained,
        RetainedNativeRealizationRequest {
            profile: &proof_admission::AdmissionProfile::default(),
            optimization_selections:
                &optimization_core::PostTerminalOptimizationSelections::default(),
            terminal_authority_policy: policy,
            accepted_package_terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            terminal_authority_permission_policy: Some(permission_policy),
            image_request: native::ExecutableImageEmissionRequest::dynamic_elf(interpreter),
            imports: &[SourceEvaluatedImportSettlement::new(
                &admission.execution,
                &admission.same_stack,
            )],
        },
    )
}
