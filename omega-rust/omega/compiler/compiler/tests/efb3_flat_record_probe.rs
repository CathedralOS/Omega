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
    let object = artifact.object();
    assert_eq!(object.foreign_calls(), artifact.image().foreign_calls());
    assert_eq!(object.object().layout.normalized_imports.len(), 1);
    assert_eq!(object.relocations().record_count(), 2);
    let stack = image_emission::derive_stack_demand(object, object.entry()).unwrap();
    let call = &object.foreign_calls()[0];
    assert_eq!(
        stack.ceiling_bytes(),
        u64::from(call.caller_live_bytes) + 64 * 1024
    );
    assert!(
        stack
            .admitted_contribution_commitments()
            .contains(&call.same_stack_contribution.commitment())
    );
    let first = call
        .aarch64_floating_control
        .expect("first control envelope");
    let second = object.foreign_calls()[1]
        .aarch64_floating_control
        .expect("second control envelope");
    assert_ne!(first.saved_slot_byte_offset, second.saved_slot_byte_offset);
    assert!(first.restore_offset + first.restore_byte_count <= second.save_offset);
    for call in object.foreign_calls() {
        let control = call.aarch64_floating_control.unwrap();
        assert!(control.save_offset + control.save_byte_count <= call.text_offset);
        assert_eq!(control.restore_offset, call.text_offset + 4);
        assert_eq!(
            call.scalar_result.as_ref().unwrap().code_offset,
            control.restore_offset + control.restore_byte_count
        );
    }
}

#[test]
fn fragment_foreign_control_envelopes_reject_substituted_custody_and_bytes() {
    let (_probe, artifact) = realize_mixed_arguments_probe();
    let object = artifact.object();
    let source = object.fragment_source_for_test().unwrap();
    for mutation in 0..5 {
        let mut changed = object.clone();
        let original = changed.foreign_calls()[0].aarch64_floating_control.unwrap();
        match mutation {
            0 => changed.foreign_calls_mut_for_test()[0].aarch64_floating_control = None,
            1 => {
                let other_slot = changed.foreign_calls()[1]
                    .aarch64_floating_control
                    .unwrap()
                    .saved_slot_byte_offset;
                changed.foreign_calls_mut_for_test()[0]
                    .aarch64_floating_control
                    .as_mut()
                    .unwrap()
                    .saved_slot_byte_offset = other_slot;
            }
            2 => {
                changed.foreign_calls_mut_for_test()[0]
                    .aarch64_floating_control
                    .as_mut()
                    .unwrap()
                    .restore_offset = original.save_offset
            }
            3 => changed.text_bytes_mut_for_test()[original.restore_offset] ^= 1,
            _ => {
                changed.foreign_calls_mut_for_test()[0]
                    .aarch64_floating_control
                    .as_mut()
                    .unwrap()
                    .target = target::NativeTarget::linux_arm64()
            }
        }
        assert!(
            image_emission::validate_function_fragment_object_artifact(source, &changed).is_err(),
            "floating-control mutation {mutation}"
        );
    }
}

#[test]
fn fragment_foreign_imports_reject_substituted_object_records() {
    let (_probe, artifact) = realize_mixed_arguments_probe();
    let object = artifact.object();
    let source = object.fragment_source_for_test().unwrap();
    for mutation in 0..18 {
        let mut changed = object.clone();
        let import_symbol = changed.object().layout.normalized_imports[0].symbol;
        let relocation_handle = changed.relocations().records().next().unwrap().0;
        match mutation {
            0 => changed
                .object_mut_for_test()
                .layout
                .normalized_imports
                .clear(),
            1 => {
                let extra = changed.object().layout.normalized_imports[0].clone();
                changed
                    .object_mut_for_test()
                    .layout
                    .normalized_imports
                    .push(extra);
            }
            2 => {
                changed.object_mut_for_test().layout.normalized_imports[0].locator =
                    target::normalize_foreign_locator(
                        target::ForeignLocatorCandidate::MachODylibSymbol {
                            install_name: b"@executable_path/other.dylib".to_vec(),
                            symbol: b"_shift".to_vec(),
                        },
                        target::TargetProfile::MacosArm64,
                    )
                    .unwrap();
            }
            3 => changed
                .object_mut_for_test()
                .layout
                .symbols
                .get_mut(import_symbol)
                .name
                .push_str("changed"),
            4 => {
                *changed.relocations_mut_for_test() =
                    object_file::RelocationPlan::with_target(object.target())
            }
            5 => {
                let extra = changed.relocations().records().next().unwrap().1.clone();
                changed.relocations_mut_for_test().push_record(extra);
            }
            6 => {
                changed
                    .relocations_mut_for_test()
                    .record_set
                    .records
                    .get_mut(relocation_handle)
                    .offset += 4
            }
            7 => {
                changed
                    .relocations_mut_for_test()
                    .record_set
                    .records
                    .get_mut(relocation_handle)
                    .byte_width = 8
            }
            8 => {
                changed
                    .relocations_mut_for_test()
                    .record_set
                    .records
                    .get_mut(relocation_handle)
                    .addend = 4
            }
            9 => {
                changed
                    .relocations_mut_for_test()
                    .record_set
                    .records
                    .get_mut(relocation_handle)
                    .kind = object_file::RelocationKind::Absolute64
            }
            10 => {
                changed
                    .relocations_mut_for_test()
                    .record_set
                    .records
                    .get_mut(relocation_handle)
                    .section = object_file::SectionKind::Data
            }
            11 => {
                changed
                    .relocations_mut_for_test()
                    .record_set
                    .records
                    .get_mut(relocation_handle)
                    .symbol_handle = object.functions()[0].symbol
            }
            12 => {
                changed
                    .relocations_mut_for_test()
                    .record_set
                    .records
                    .get_mut(relocation_handle)
                    .origin = object_file::RelocationOrigin::SemanticOperation {
                    function_symbol_handle: object.functions()[0].symbol,
                    operation_identity: u64::MAX,
                }
            }
            13 => changed.foreign_calls_mut_for_test().clear(),
            14 => changed.foreign_calls_mut_for_test()[0].caller_live_bytes = 0,
            15 => changed.foreign_calls_mut_for_test().swap(0, 1),
            16 => {
                changed
                    .object_mut_for_test()
                    .layout
                    .symbols
                    .get_mut(import_symbol)
                    .section = object_file::SymbolSection::Section(object_file::SectionKind::Text)
            }
            _ => {
                let extra = changed.object().layout.symbols.get(import_symbol).clone();
                changed.object_mut_for_test().layout.symbols.insert(extra);
            }
        }
        assert!(
            image_emission::validate_function_fragment_object_artifact(source, &changed).is_err(),
            "mutation {mutation}"
        );
    }
}

/// Exercise the complete object-import, stack-provisioning, loader and native
/// ABI route; artifact replay alone does not establish foreign-call execution.
#[test]
fn mixed_scalar_record_arguments_and_reused_result_execute_natively() {
    execute_mixed_arguments_probe(false);
}

#[test]
fn returning_foreign_call_restores_floating_controls_before_the_next_call() {
    execute_mixed_arguments_probe(true);
}

/// An incoming parameter crosses one boundary call, then a ranked block value
/// crosses another four times. The oracle checks every value and their order;
/// a valid image or one successful call does not establish changing transport.
#[test]
fn ranked_foreign_call_executes_every_iteration() {
    let probe = Probe::new();
    fs::write(
        &probe.main,
        r#"use omega::language::core::external_binding;
boundary trait Trace {
    machine record(value: u64, first: u64, second: u64, third: u64,
        fourth: u64, fifth: u64, sixth: u64, seventh: u64, repeated: u64);
}
macos_arm64 machine record_binding() -> Binding<31, 6, 0> {
    Binding::DllImport {
        import: DllImport::MachODylibSymbol {
            install_name: "@executable_path/libshift.dylib",
            symbol: "_shift",
        },
    }
}
machine record_leaf(value: u64, first: u64, second: u64, third: u64,
    fourth: u64, fifth: u64, sixth: u64, seventh: u64, repeated: u64)
    satisfies Trace::record via record_binding();
data Main { }
machine Main::spin(&mut self, remaining: u64) terminates by remaining; reaches Trace {
    Trace::record(remaining, 1, 2, 3, 4, 5, 6, 7, remaining);
    transition remaining == 0 {
        true -> done()
        _ -> self.spin(remaining - 1)
    }
    state done(&mut self) { }
}
machine Main::record_once(&mut self, value: u64) reaches Trace {
    Trace::record(value, 1, 2, 3, 4, 5, 6, 7, value);
}
machine Main::main(&mut self) {
    self.record_once(9);
    self.spin(3);
}
"#,
    )
    .unwrap();
    fs::write(
        probe.root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("ranked-foreign-caller");
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
}
"#,
    )
    .unwrap();
    let (artifact, _, _) = realize_probe(&probe, 64 * 1024);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let ranked_machines = module
        .machines
        .iter()
        .filter(|machine| machine.ranked_scc.is_some())
        .collect::<Vec<_>>();
    let [ranked_machine] = ranked_machines.as_slice() else {
        panic!("the program must retain one ranked machine")
    };
    assert_eq!(artifact.image().foreign_calls().len(), 2);
    let call = artifact
        .image()
        .foreign_calls()
        .iter()
        .find(|call| call.machine == ranked_machine.id)
        .expect("four dynamic ranked calls must retain their static foreign-call occurrence");
    assert_eq!(call.machine, ranked_machine.id);
    let evidence = artifact
        .physical_evidence()
        .expect("ranked foreign calls must retain complete physical evidence");
    assert!(artifact.physical_evidence_gap().is_none());
    let boundary_children = evidence
        .children()
        .iter()
        .filter(|child| {
            matches!(
                child.relocation(),
                native_artifact::PhysicalRelocationDisposition::UnresolvedNormalizedForeignCallImportField(field)
                    if field.caller() == ranked_machine.id
            )
        })
        .collect::<Vec<_>>();
    let [child] = boundary_children.as_slice() else {
        panic!("one static foreign-call occurrence must have one physical child")
    };
    let native_artifact::PhysicalRelocationDisposition::UnresolvedNormalizedForeignCallImportField(
        field,
    ) = child.relocation()
    else {
        panic!("the ranked child must retain the exact unresolved import field")
    };
    assert_eq!(field.caller(), ranked_machine.id);
    assert_eq!(Some(field.operation()), call.owner.operation());
    assert_eq!(field.offset(), call.text_offset);
    let object = artifact.object();
    let source = object.fragment_source_for_test().unwrap();
    for foreign in object.foreign_calls() {
        assert_eq!(foreign.scalar_arguments.len(), 9);
        for argument in [&foreign.scalar_arguments[0], &foreign.scalar_arguments[8]] {
            assert!(matches!(
                argument.source,
                machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall { .. }
            ));
        }
        assert!(matches!(
            foreign.scalar_arguments[8].placement.locations.as_slice(),
            [calling_conventions::ValueLocation::Stack { .. }]
        ));
    }
    for mutation in 0..4 {
        let mut changed = object.clone();
        let argument = &mut changed.foreign_calls_mut_for_test()[0].scalar_arguments[0];
        let machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall {
            source_value,
            scalar_type,
            instruction,
        } = &mut argument.source
        else {
            panic!("parameter source must retain selected SSA call custody")
        };
        match mutation {
            0 => *source_value = semantic_vocabulary::ValueId::new(u64::MAX).unwrap(),
            1 => *scalar_type = semantic_vocabulary::ScalarType::Boolean,
            2 => instruction.0 += 1,
            _ => argument.code_offset += 4,
        }
        assert!(
            image_emission::validate_function_fragment_object_artifact(source, &changed).is_err(),
            "parameter custody mutation {mutation} must reject"
        );
    }
    let provider_source = r#"
#include <stdint.h>
#include <stdio.h>
static unsigned calls = 0;
static const uint64_t expected[] = {9, 3, 2, 1, 0};
void shift(uint64_t value, uint64_t first, uint64_t second, uint64_t third,
           uint64_t fourth, uint64_t fifth, uint64_t sixth, uint64_t seventh,
           uint64_t repeated) {
    if (calls >= 5 || value != expected[calls] || repeated != value ||
        first != 1 || second != 2 || third != 3 || fourth != 4 ||
        fifth != 5 || sixth != 6 || seventh != 7) {
        puts("ranked foreign arguments: FAIL");
        fflush(stdout);
        return;
    }
    calls += 1;
    if (calls == 5) {
        puts("ranked foreign arguments: PASS");
        fflush(stdout);
    }
}
"#;
    execute_foreign_probe(
        &probe,
        &artifact,
        provider_source,
        b"ranked foreign arguments: PASS\n",
    );
    let mut parts = artifact.into_parts();
    let mut changed = parts.physical_evidence.take().unwrap().into_parts();
    changed.children.clear();
    parts.physical_evidence =
        Some(native_artifact::NativePhysicalEvidence::from_replayed_parts(changed));
    assert!(
        native_artifact::NativeArtifact::from_replayed_parts(parts).is_err(),
        "a missing ranked child must fail independent replay"
    );
}

fn execute_mixed_arguments_probe(perturb_floating_controls: bool) {
    let (probe, artifact) = realize_mixed_arguments_probe();
    let provider_source = r#"
#include <stdint.h>
#include <stdio.h>
typedef struct { int32_t x; int32_t y; } Point;
static unsigned calls = 0;
static uint64_t original_fpcr;
int32_t shift(int32_t delta, const Point *point, int32_t bias) {
    calls += 1;
    if (point && point->x == 11 && point->y == 22) {
        if (calls == 1 && delta == 5 && bias == 7) {
            if (PERTURB_FLOATING_CONTROLS) {
                __asm__ volatile("mrs %0, fpcr" : "=r"(original_fpcr));
                uint64_t changed = original_fpcr ^ (1ull << 22);
                __asm__ volatile("msr fpcr, %0" : : "r"(changed));
            }
            return 45;
        }
        if (calls == 2 && delta == 45 && bias == 9) {
            if (PERTURB_FLOATING_CONTROLS) {
                uint64_t observed;
                __asm__ volatile("mrs %0, fpcr" : "=r"(observed));
                if (observed != original_fpcr) {
                    puts("foreign call changed floating controls");
                    fflush(stdout);
                    return -1;
                }
            }
            puts("mixed foreign arguments: PASS");
            fflush(stdout);
            return 87;
        }
    }
    puts("mixed foreign arguments: FAIL");
    fflush(stdout);
    return -1;
}
"#
    .replace(
        "PERTURB_FLOATING_CONTROLS",
        if perturb_floating_controls { "1" } else { "0" },
    );
    execute_foreign_probe(
        &probe,
        &artifact,
        &provider_source,
        b"mixed foreign arguments: PASS\n",
    );
}

fn execute_foreign_probe(
    probe: &Probe,
    artifact: &native_artifact::NativeArtifact,
    provider_source: &str,
    expected_stdout: &[u8],
) {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        use std::os::unix::fs::PermissionsExt;
        use std::process::{Command, Stdio};
        let provider = probe.root.join("shift.c");
        fs::write(&provider, provider_source).unwrap();
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
        let executable = probe.root.join("foreign-arguments");
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
                panic!("foreign-call image timed out: {output:?}");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(output.stdout, expected_stdout);
        assert!(output.stderr.is_empty(), "{output:?}");
    }
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        let _ = (probe, artifact, provider_source, expected_stdout);
        eprintln!("SKIP foreign-call execution: requires macOS ARM64 and cc");
    }
}
