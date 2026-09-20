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
/// same-stack contribution the settlement supplied. Physical derivation now
/// realizes the call's boundary child through the retained relocation-free
/// object plan: the probe requires the exact unresolved import-field custody
/// record keyed to the call's `{caller, operation}` and four-byte branch
/// field.
#[test]
fn flat_record_via_call_native_realization_probe() {
    let probe = Probe::new();
    let (artifact, execution, same_stack) = realize_probe(&probe, 64);
    let plan_report_identity = execution.plan_report_identity;
    let requirement = execution.requirement;
    assert_flat_record_custody(&artifact, plan_report_identity, &requirement, &same_stack);
}

fn realize_probe(
    probe: &Probe,
    provider_stack_bytes: u64,
) -> (
    native_artifact::NativeArtifact,
    ProbeExecution,
    task_plans::AdmittedSameStackContribution,
) {
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
            bytes: provider_stack_bytes,
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
        .into_direct()
        .expect("the probe emits a direct image artifact");
    artifact
        .validate()
        .expect("native artifact independently replays");
    (artifact, execution, same_stack)
}

fn assert_flat_record_custody(
    artifact: &native_artifact::NativeArtifact,
    plan_report_identity: u64,
    requirement: &str,
    same_stack: &task_plans::AdmittedSameStackContribution,
) {
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
        requirement
    );
    assert_eq!(call.same_stack_contribution.receipt(), same_stack.receipt());
    assert_eq!(call.same_stack_contribution.bytes(), 64);
    assert_eq!(call.same_stack_contribution.alignment(), 16);
    assert!(call.scalar_arguments.is_empty());
    let evidence = artifact
        .physical_evidence()
        .expect("the fragment route must retain complete physical evidence for its boundary call");
    assert!(
        artifact.physical_evidence_gap().is_none(),
        "the retained artifact must have no physical derivation gap, got {:?}",
        artifact.physical_evidence_gap().map(|gap| gap.subject())
    );
    let boundary_children = evidence
        .children()
        .iter()
        .filter(|child| {
            matches!(
                child.occurrence(),
                native_artifact::NativePhysicalOccurrence::Boundary(_)
            )
        })
        .collect::<Vec<_>>();
    let [boundary_child] = boundary_children.as_slice() else {
        panic!(
            "physical evidence must realize exactly one boundary child, found {}",
            boundary_children.len()
        )
    };
    let native_artifact::PhysicalRelocationDisposition::UnresolvedNormalizedForeignCallImportField(
        field,
    ) = boundary_child.relocation()
    else {
        panic!(
            "the boundary child must retain its fragment import-field custody, got {:?}",
            boundary_child.relocation()
        )
    };
    assert_eq!(field.caller(), *machine);
    assert_eq!(field.operation(), *operation);
    assert_eq!(field.offset(), call.text_offset);
    assert_eq!(field.byte_width(), 4);
    assert_eq!(field.addend(), 0);
    assert_eq!(field.kind(), object_file::RelocationKind::Aarch64Branch26);
}

fn realize_mixed_arguments_probe() -> (Probe, native_artifact::NativeArtifact) {
    let probe = Probe::new();
    let install_name = "@executable_path/libshift.dylib";
    let source = fs::read_to_string(&probe.main)
        .unwrap()
        .replace("shift(p: &Point)", "shift(delta: i32, p: &Point, bias: i32)")
        .replace("Binding<10, 5, 0>", &format!("Binding<{}, 6, 0>", install_name.len()))
        .replace("libm.dylib", install_name)
        .replace("symbol: \"shift\"", "symbol: \"_shift\"")
        .replace(
            "let rc: i32 = self.m.shift(&self.p);\n    let keep: i32 = rc;",
            "self.p.x = 11;\n    self.p.y = 22;\n    let result: i32 = self.m.shift(5, &self.p, 7);\n    let reused: i32 = self.m.shift(result, &self.p, 9);",
        );
    fs::write(&probe.main, source).unwrap();
    // The native oracle calls libc to report its observations. Its admitted
    // stack contribution must include those calls, not only the leaf arithmetic.
    let (artifact, _, _) = realize_probe(&probe, 64 * 1024);
    assert_eq!(artifact.image().foreign_calls().len(), 2);
    for call in artifact.image().foreign_calls() {
        assert_eq!(call.scalar_arguments.len(), 2);
        assert_eq!(call.scalar_arguments[0].parameter_index, 0);
        assert_eq!(call.scalar_arguments[1].parameter_index, 2);
    }
    (probe, artifact)
}

#[test]
fn mixed_scalar_record_arguments_and_reused_result_replay() {
    let (_probe, artifact) = realize_mixed_arguments_probe();
    assert!(matches!(
        artifact.image().foreign_calls()[1].scalar_arguments[0].source,
        machine_code::InternalUnitScalarArgumentSourceRecord::Home(_)
    ));
}

/// This is the outer customer acceptance, not established by artifact replay.
/// Function-fragment object publication currently drops the import/relocation
/// rows before final image emission, leaving ARM64 BL immediates at zero.
/// EVALUATED-FOREIGN-BINDINGS owns that writer and its independent replay.
#[test]
#[ignore = "blocked on EVALUATED-FOREIGN-BINDINGS: fragment object import/relocation publication"]
fn mixed_scalar_record_arguments_and_reused_result_execute_natively() {
    let (probe, artifact) = realize_mixed_arguments_probe();
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        use std::os::unix::fs::PermissionsExt;
        use std::process::{Command, Stdio};
        let provider = probe.root.join("shift.c");
        fs::write(
            &provider,
            r#"
#include <stdint.h>
#include <stdio.h>
typedef struct { int32_t x; int32_t y; } Point;
static unsigned calls = 0;
int32_t shift(int32_t delta, const Point *point, int32_t bias) {
    calls += 1;
    if (point && point->x == 11 && point->y == 22) {
        if (calls == 1 && delta == 5 && bias == 7) return 45;
        if (calls == 2 && delta == 45 && bias == 9) {
            puts("mixed foreign arguments: PASS");
            fflush(stdout);
            return 87;
        }
    }
    puts("mixed foreign arguments: FAIL");
    fflush(stdout);
    return -1;
}
"#,
        )
        .unwrap();
        let compile = Command::new("cc")
            .arg("-dynamiclib")
            .arg(&provider)
            .arg("-o")
            .arg(probe.root.join("libshift.dylib"))
            .output()
            .expect("compile native ABI oracle");
        assert!(
            compile.status.success(),
            "{}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let executable = probe.root.join("mixed-arguments");
        fs::write(&executable, &artifact.image().output().bytes).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let mut child = Command::new(&executable)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("execute validated foreign-call image");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                let output = child.wait_with_output().unwrap();
                panic!("mixed foreign-call image timed out: {output:?}");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(output.stdout, b"mixed foreign arguments: PASS\n");
        assert!(output.stderr.is_empty(), "{output:?}");
    }
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        let _ = (probe, artifact);
        eprintln!("SKIP mixed foreign-call execution: requires macOS ARM64 and cc");
    }
}
