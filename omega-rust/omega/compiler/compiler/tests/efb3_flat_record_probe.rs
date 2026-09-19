use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct, compile};
use std::fs;
use std::path::PathBuf;

struct Probe {
    root: PathBuf,
    main: PathBuf,
}

impl Probe {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-efb3-flat-record-probe-{}",
            std::process::id(),
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create probe fixture");
        let main = root.join("main.omg");
        fs::write(
            &main,
            r#"use omega::language::core::external_binding;
use omega::language::core::service;

pub data Point {
    x: i32;
    y: i32;
}

pub boundary trait Move {
    machine shift(p: &Point) -> i32 reaches Move;
}

macos_arm64 machine shift_binding() -> Binding<10, 5, 0> {
    Binding::DllImport {
        import: DllImport::MachODylibSymbol {
            install_name: "libm.dylib",
            symbol: "shift",
        },
    }
}

data MoveProvider { }
macos_arm64 machine MoveProvider::shift(p: &Point) -> i32
satisfies Move::shift
via shift_binding();

data Main { m: Service<Move>; p: Point; }
machine Main::main(&mut self) reaches Move {
    let rc: i32 = self.m.shift(&self.p);
    let keep: i32 = rc;
}
"#,
        )
        .expect("write probe source");
        fs::write(
            root.join("build.omg"),
            r#"machine build(builder: &mut Build) {
    builder.application("flat-record-probe");
    builder.select_provider<Move, MoveProvider>();
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
}
"#,
        )
        .expect("write probe build");
        Self { root, main }
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The flat-record normalized-foreign rung depends on the frontend carrying a
/// source-rooted borrowed record actual through to the Terminal boundary call.
/// This pins that precondition: `self.m.shift(&self.p)` must produce exactly
/// one structural argument whose place is rooted in `main`, navigates the
/// `p` field, and is shared-borrowed. Downstream normalized-foreign lowering,
/// machine-code custody, and the physical-child derivation still have to
/// materialize that record pointer and rejoin it; this test stops at the
/// Terminal artifact so it can run before those stages land.
#[test]
fn flat_record_via_call_reaches_terminal_as_source_rooted_structural_argument() {
    let probe = Probe::new();
    let request = CompileRequest::new(CompileOptions {
        root_path: probe.main.clone(),
        build_dir: Some(probe.root.join("build")),
        target_name: Some("macos_arm64".to_owned()),
    })
    .with_requested_product(RequestedCompileProduct::TerminalArtifact);
    let retained = compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| {
            panic!(
                "flat-record via call should produce a Terminal artifact:\n{}",
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        })
        .into_retained_terminal_artifact()
        .expect("retained terminal artifact");
    let module = terminal_codec::decode_module(retained.artifact().semantic_bytes())
        .expect("decode terminal module");
    let boundary_calls = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::BoundaryCall {
                arguments,
                structural_arguments,
                ..
            } => Some((arguments, structural_arguments)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(arguments, structural_arguments)] = boundary_calls.as_slice() else {
        panic!(
            "expected exactly one boundary call, found {}",
            boundary_calls.len()
        )
    };
    assert!(
        arguments.is_empty(),
        "borrowed record call carries no scalar lane"
    );
    let [argument] = structural_arguments.as_slice() else {
        panic!(
            "expected exactly one structural argument, found {}",
            structural_arguments.len()
        )
    };
    assert_eq!(
        argument.access,
        terminal_psi::StructuralAccess::SharedBorrow
    );
    assert_eq!(
        argument.path.as_slice(),
        [terminal_psi::StructuralPathSegment::Field("p".to_owned())],
        "structural argument must stay rooted at the authored `p` field"
    );
}

use compiler::{
    RetainedNativeRealizationRequest, SourceEvaluatedImportSettlement,
    realize_retained_native_artifact,
};
use effects::provider_plan::ProviderBinding;
use installation_evidence::ProviderExecutionEvidence;
use task_plans::{
    SameStackContributionAdmissionCandidate, SameStackContributionAdmissionReceiptId,
    SameStackProviderPlanCommitment, admit_same_stack_contribution,
};

#[derive(Debug)]
struct ProbeExecution {
    requirement: String,
    plan_report_identity: u64,
}

impl ProviderExecutionEvidence for ProbeExecution {
    fn requirement_identity(&self) -> &str {
        &self.requirement
    }
    fn provider_plan_report_identity(&self) -> u64 {
        self.plan_report_identity
    }
    fn provider_execution_report_identity(&self) -> u64 {
        0x464c_4154_0001
    }
    fn provider_execution_report_fingerprint(&self) -> u64 {
        0x464c_4154_0002
    }
    fn normalized_root_report_identity(&self) -> u64 {
        0x464c_4154_0003
    }
    fn boundary_contract_report_fingerprint(&self) -> u64 {
        0x464c_4154_0004
    }
}

/// Drive the flat-record `via` call past the Terminal artifact into retained
/// native realization. Source-custody legalization now projects the boundary
/// call into a dedicated `NormalizedForeignCall` legalized instruction that
/// carries the admitted provider execution, evaluated entry plan, ordered
/// scalar rows, and the source-rooted structural argument, and independent
/// replay re-derives and rejoins all of it. Selection, encoding, layout, and
/// text placement now transport the call as a per-plan normalized foreign
/// row and emit its import relocation. Image custody has landed: the emitted
/// image carries the foreign-call custody row rederived from the emitted
/// object, so independent replay rejoins the artifact's admitted provider
/// execution and same-stack contribution to the exact call site. This test
/// pins that completed rejoin — the image row must record the boundary-call
/// owner, Mach-O locator, admitted provider-execution coordinates, and
/// same-stack contribution the settlement supplied. The remaining frontier
/// is physical derivation of the normalized-foreign child: the fragment
/// route seals the object's effect roster empty, so the derivation cannot
/// yet claim the image custody row for its boundary occurrence and must
/// report that exact gap subject, flipping this probe when the physical leg
/// lands.
#[test]
fn flat_record_via_call_native_realization_probe() {
    let probe = Probe::new();
    let request = CompileRequest::new(CompileOptions {
        root_path: probe.main.clone(),
        build_dir: Some(probe.root.join("build")),
        target_name: Some("macos_arm64".to_owned()),
    })
    .with_requested_product(RequestedCompileProduct::TerminalArtifact);
    let retained = compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect("terminal compile")
        .into_retained_terminal_artifact()
        .expect("retained terminal artifact");
    let proposal = retained
        .native_realization_proposal()
        .expect("native proposal");
    eprintln!(
        "== external binding rows ==\n{:#?}",
        proposal.external_binding_rows()
    );
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
        panic!("expected one selected import, found {}", matches.len())
    };
    let requirement = row.requirement_identity.clone();
    let plan_report_identity = plan.report_fingerprint();
    let plan_commitment =
        SameStackProviderPlanCommitment::from_digest(*plan.identity_digest().as_bytes());
    let execution = ProbeExecution {
        requirement: requirement.clone(),
        plan_report_identity,
    };
    let same_stack = admit_same_stack_contribution(
        SameStackContributionAdmissionCandidate {
            provider_plan_report_identity: plan_report_identity,
            provider_plan_commitment: plan_commitment,
            requirement_identity: requirement.clone(),
            receipt: SameStackContributionAdmissionReceiptId::from_normalized_identity(
                0x464c_4154_0005,
            )
            .unwrap(),
            bytes: 64,
            alignment: 16,
        },
        plan_report_identity,
        plan_commitment,
        &requirement,
    )
    .expect("same-stack admission");
    let policy = native_realization::terminal_authority_policy_with_rows(
        proposal
            .external_binding_rows()
            .iter()
            .filter_map(|row| {
                let calling_conventions::ExternalBindingKind::Import { locator } = &row.binding
                else {
                    return None;
                };
                Some(native_realization::TerminalAuthorityPolicyRow::new(
                    native_realization::normalized_foreign_terminal_mechanism(
                        locator,
                        row.boundary_entry_plan.as_ref()?,
                    )
                    .expect("canonical mechanism"),
                    effects::TerminalAuthorityDisposition::from_classes([]),
                ))
            })
            .collect(),
    )
    .expect("policy");
    let permission_policy = native_realization::terminal_authority_permission_policy_with_rows(
        proposal
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
            .collect(),
    )
    .expect("permission policy");
    let image_request =
        native_realization::ExecutableImageEmissionRequest::direct(proposal.subsystem());
    let outcome = realize_retained_native_artifact(
        retained,
        RetainedNativeRealizationRequest {
            profile: &proof_admission::AdmissionProfile::default(),
            optimization_selections:
                &optimization_core::PostTerminalOptimizationSelections::default(),
            terminal_authority_policy: policy,
            accepted_package_terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            terminal_authority_permission_policy: Some(permission_policy),
            image_request,
            imports: &[SourceEvaluatedImportSettlement::new(
                &execution,
                &same_stack,
            )],
        },
    );
    let product = outcome.unwrap_or_else(|(_, diagnostics)| {
        panic!(
            "native realization should accept the emitted foreign-call custody:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let artifact = product
        .as_direct()
        .expect("the probe emits a direct image artifact");
    let module =
        terminal_codec::decode_module(artifact.semantic_bytes()).expect("decode terminal module");
    let boundary_sites = module
        .machines
        .iter()
        .flat_map(|machine| {
            machine.blocks.iter().flat_map(|block| {
                block.operations.iter().filter_map(|operation| {
                    matches!(
                        &operation.kind,
                        terminal_psi::OperationKind::BoundaryCall { .. }
                    )
                    .then_some((machine.id, operation.id))
                })
            })
        })
        .collect::<Vec<_>>();
    let [(machine, operation)] = boundary_sites.as_slice() else {
        panic!(
            "expected exactly one boundary call, found {}",
            boundary_sites.len()
        )
    };
    let foreign_calls = artifact.image().foreign_calls();
    let [call] = foreign_calls else {
        panic!(
            "the image must carry exactly one foreign-call custody row, found {}",
            foreign_calls.len()
        )
    };
    assert_eq!(call.machine, *machine);
    assert_eq!(call.owner.operation(), Some(*operation));
    let target::ForeignLocatorCandidate::MachODylibSymbol {
        install_name,
        symbol,
    } = call.locator.locator()
    else {
        panic!("the probe binding must retain its Mach-O dylib symbol locator")
    };
    assert_eq!(install_name.as_slice(), b"libm.dylib");
    assert_eq!(symbol.as_slice(), b"shift");
    assert_eq!(
        call.provider_execution.provider_plan_report_identity,
        plan_report_identity
    );
    assert_eq!(
        call.provider_execution.provider_execution_report_identity,
        0x464c_4154_0001
    );
    assert_eq!(
        call.provider_execution
            .provider_execution_report_fingerprint,
        0x464c_4154_0002
    );
    assert_eq!(
        call.provider_execution.normalized_root_report_identity,
        0x464c_4154_0003
    );
    assert_eq!(
        call.provider_execution.boundary_contract_report_fingerprint,
        0x464c_4154_0004
    );
    assert_eq!(
        call.same_stack_contribution.report_identity(),
        same_stack.report_identity()
    );
    assert_eq!(
        call.same_stack_contribution.commitment(),
        same_stack.commitment()
    );
    assert_eq!(
        call.same_stack_contribution.provider_plan_report_identity(),
        plan_report_identity
    );
    assert_eq!(
        call.same_stack_contribution.provider_plan_commitment(),
        same_stack.provider_plan_commitment()
    );
    assert_eq!(
        call.same_stack_contribution.requirement_identity(),
        requirement.as_str()
    );
    assert_eq!(call.same_stack_contribution.receipt(), same_stack.receipt());
    assert_eq!(call.same_stack_contribution.bytes(), 64);
    assert_eq!(call.same_stack_contribution.alignment(), 16);
    assert!(call.scalar_arguments.is_empty());
    assert!(artifact.physical_evidence().is_none());
    let gap = artifact
        .physical_evidence_gap()
        .expect("the retained artifact must name its physical derivation gap");
    assert!(
        matches!(
            gap.subject(),
            native_artifact::NativePhysicalEvidenceGapSubject::UnrealizedBoundaryOccurrence {
                occurrence
            } if occurrence.machine() == *machine && occurrence.operation() == *operation
        ),
        "the remaining frontier is the normalized-foreign physical child, got {:?}",
        gap.subject()
    );
}
