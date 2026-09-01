use omega_compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileRequest, RequestedCompileProduct,
    SourceEvaluatedImportSettlement, compile, compile_to_checked_with_packages,
    realize_retained_terminal_artifact_with_source_evaluated_imports_and_policy,
};
use omega_effects::provider_plan::ProviderBinding;
use omega_installation_evidence::ProviderExecutionEvidence;
use omega_package_compilation::{
    BuildDeclarationKind, PackageCompilationInputs, PackageSourceBinding,
};
use omega_task_plans::{
    SameStackContributionAdmissionCandidate, SameStackContributionAdmissionReceiptId,
    SameStackProviderPlanCommitment, admit_same_stack_contribution,
};
use psi_core::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

fn fixture_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-{name}-{}-{}",
        std::process::id(),
        NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed),
    ))
}

const CALLBACK_USE: &str = r#"
data CallbackProvider { }

machine CallbackProvider::call(message: u64)
satisfies WindowProcedure::call
{
}

data RegistrarUser {
    registrar: WindowRegistrar;
    specification: Spread<ForeignRecord>;
}

machine RegistrarUser::configure(&mut self) {
    WindowRegistrar::register<CallbackProvider::call, CallbackProvider::call>(&self.specification);
}

data Main { }
machine Main::main() { }
"#;

struct Fixture {
    root: PathBuf,
    main: PathBuf,
    package: PackageKeyIdentity,
}

impl Fixture {
    fn new() -> Self {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(5)
            .expect("Omega repository root");
        let root = fixture_root("callback-terminal-custody");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create callback-custody fixture");
        fs::copy(
            repository.join("source/library/std/calling.omg"),
            root.join("calling.omg"),
        )
        .expect("copy package-local calling vocabulary");

        let source = fs::read_to_string(
            repository.join("source/library/std/tests/callback_materialization_closure.omg"),
        )
        .expect("read callback materialization fixture");
        let source = source
            .replacen(
                "boundary trait WindowProcedure {",
                "boundary trait WindowProcedure: Calling<RegistrarPolicy> {",
                1,
            )
            .replacen(
                "machine call(message: u64) -> u64;",
                "machine call(message: u64);",
                1,
            )
            .replace(
                "data Main { }\nmachine Main::main(&mut self) { }\n",
                CALLBACK_USE.trim_start_matches('\n'),
            );
        assert!(source.contains("CallbackProvider::call"));

        let main = root.join("main.omg");
        fs::write(&main, source).expect("write callback-custody source");
        fs::write(
            root.join("build.omg"),
            r#"machine build(builder: &mut Build) {
    builder.application("callback-terminal-custody");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
}
"#,
        )
        .expect("write callback-custody build policy");

        Self {
            root,
            main,
            package: PackageKeyIdentity::from_digest([71; 32])
                .expect("nonzero fixture package identity"),
        }
    }

    fn direct() -> Self {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(5)
            .expect("Omega repository root");
        let root = fixture_root("direct-callback-terminal-custody");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create direct callback-custody fixture");
        fs::copy(
            repository.join("source/library/std/calling.omg"),
            root.join("calling.omg"),
        )
        .expect("copy package-local calling vocabulary");
        let main = root.join("main.omg");
        let source = fs::read_to_string(
            repository.join("source/library/std/tests/direct_callback_parameter.omg"),
        )
        .expect("read direct callback source canary");
        fs::write(&main, source).expect("write package-aware direct callback source canary");
        fs::write(
            root.join("build.omg"),
            r#"machine build(builder: &mut Build) {
    builder.application("direct-callback-terminal-custody");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
}
"#,
        )
        .expect("write direct callback-custody build policy");
        Self {
            root,
            main,
            package: PackageKeyIdentity::from_digest([72; 32])
                .expect("nonzero direct fixture package identity"),
        }
    }

    fn direct_with_import() -> Self {
        let fixture = Self::direct();
        let source = fs::read_to_string(&fixture.main).expect("read direct callback fixture");
        let source = source
            .replacen(
                "use calling;",
                "use calling;\nuse omega::language::core::external_binding;",
                1,
            )
            .replacen(
                "data Main { }",
                r#"windows_x86_64 machine install_binding() -> Binding<12, 24, 0> {
    Binding::DllImport {
        import: DllImport::PeByName {
            library: "kernel32.dll",
            export: "FlushProcessWriteBuffers",
        },
    }
}

machine install_leaf<machine Handler>(kind: u64, module: u64)
where machine Handler satisfies HookProcedure::call;
satisfies HookRegistrar::install
via install_binding();

data Main { }"#,
                1,
            )
            .replace(
                "        output.call.stack_alignment = 16;",
                r#"        output.call.ordinary_clobbers.register_count = 13;
        output.call.ordinary_clobbers.registers[0] = MachineRegister::X86Rax;
        output.call.ordinary_clobbers.registers[1] = MachineRegister::X86Rcx;
        output.call.ordinary_clobbers.registers[2] = MachineRegister::X86Rdx;
        output.call.ordinary_clobbers.registers[3] = MachineRegister::X86R8;
        output.call.ordinary_clobbers.registers[4] = MachineRegister::X86R9;
        output.call.ordinary_clobbers.registers[5] = MachineRegister::X86R10;
        output.call.ordinary_clobbers.registers[6] = MachineRegister::X86R11;
        output.call.ordinary_clobbers.registers[7] = MachineRegister::X86Xmm { index: 0 };
        output.call.ordinary_clobbers.registers[8] = MachineRegister::X86Xmm { index: 1 };
        output.call.ordinary_clobbers.registers[9] = MachineRegister::X86Xmm { index: 2 };
        output.call.ordinary_clobbers.registers[10] = MachineRegister::X86Xmm { index: 3 };
        output.call.ordinary_clobbers.registers[11] = MachineRegister::X86Xmm { index: 4 };
        output.call.ordinary_clobbers.registers[12] = MachineRegister::X86Xmm { index: 5 };
        output.call.stack_alignment = 16;"#,
            )
            .replace(
                "        output.state.stack = EntryStack::ProviderSelected;",
                r#"        output.state.permitted_transitive_use.general_registers = true;
        output.state.permitted_transitive_use.vector_registers = true;
        output.state.permitted_transitive_use.flags = true;
        output.state.stack = EntryStack::ProviderSelected;"#,
            );
        fs::write(&fixture.main, source).expect("write direct callback import fixture");
        fixture
    }

    fn package_inputs(&self) -> PackageCompilationInputs {
        PackageCompilationInputs::new(
            self.package,
            BuildDeclarationKind::Application,
            vec![PackageSourceBinding::new(
                self.package,
                "callback-terminal-custody",
                self.root.clone(),
            )],
            Vec::new(),
        )
        .expect("callback fixture package graph")
    }

    fn request(&self, product: RequestedCompileProduct, tag: &str) -> CompileRequest {
        CompileRequest::new(CompileOptions {
            root_path: self.main.clone(),
            build_dir: Some(self.root.join(format!("build-{tag}"))),
            target_name: Some("windows_x86_64".to_owned()),
        })
        .with_package_inputs(self.package_inputs())
        .with_requested_product(product)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn replay_copy_terminal_artifact(
    artifact: &psi_terminal_codec::CanonicalTerminalArtifact,
) -> psi_terminal_codec::CanonicalTerminalArtifact {
    let module = psi_terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("decode copied Terminal semantics");
    let proof = psi_terminal_codec::decode_proof_bundle(artifact.proof_bytes())
        .expect("decode copied Terminal proof");
    let debug = artifact.debug_bytes().map(|bytes| {
        psi_terminal_codec::decode_debug_map(&module, bytes)
            .expect("decode copied Terminal debug map")
    });
    psi_terminal_codec::CanonicalTerminalArtifact::from_parts(&module, &proof, debug.as_ref())
        .expect("rebuild independently replayed Terminal artifact")
}

fn assert_custody_diagnostic(
    diagnostics: &[psi_diagnostics::Diagnostic],
    product: &str,
    expected_placements: usize,
) {
    assert_eq!(
        diagnostics.len(),
        1,
        "unexpected diagnostics: {diagnostics:#?}"
    );
    let message = diagnostics[0].message.as_str();
    assert!(
        message.contains(product),
        "unexpected diagnostic: {message}"
    );
    assert!(
        message.contains(&format!(
            "{expected_placements} validated callback placement(s)"
        )),
        "unexpected diagnostic: {message}"
    );
    assert!(
        message.matches("::call").count() >= expected_placements,
        "the diagnostic must name the retained callback rows: {message}"
    );
    assert!(
        message.contains("canonical Terminal callback-use custody is not implemented"),
        "unexpected diagnostic: {message}"
    );
}

#[test]
fn terminal_handoff_rejects_callbacks_outside_the_emitted_entry_closure() {
    let fixture = Fixture::new();
    let checked = compile_to_checked_with_packages(
        &fixture.main,
        Some("windows_x86_64"),
        fixture.package_inputs(),
    )
    .expect("callback program should reach checked compilation");
    assert_eq!(checked.callback_placements().len(), 2);
    compile(fixture.request(RequestedCompileProduct::Check, "check"))
        .expect("check-only compilation retains callback placements without executing them");
    let terminal = compile(fixture.request(RequestedCompileProduct::TerminalArtifact, "terminal"))
        .expect_err("unreachable callback placements cannot float beside an unrelated artifact");
    assert!(
        terminal.iter().any(|diagnostic| diagnostic
            .message
            .contains("resolves to 0 Terminal registrar occurrences")),
        "unexpected diagnostics: {terminal:#?}",
    );
}

#[test]
fn direct_callback_placement_binds_the_exact_terminal_registrar_occurrence() {
    let fixture = Fixture::direct();
    let terminal = compile(fixture.request(RequestedCompileProduct::TerminalArtifact, "terminal"))
        .expect("direct callback registrar should reach canonical Terminal custody");
    let retained = terminal
        .into_retained_terminal_artifact()
        .expect("Terminal report owns the exact direct callback sidecar");
    let [placement] = retained.callback_placements() else {
        panic!("one selected direct callback placement must survive");
    };
    assert!(matches!(
        placement
            .private_materialization
            .as_ref()
            .expect("direct callback target-closed materialization")
            .destination,
        omega_calling_conventions::NativePlace::Parameter(_)
    ));
    let proposal = retained
        .native_realization_proposal()
        .expect("Terminal product retains its native realization proposal");
    let [occurrence] = proposal.callback_occurrences() else {
        panic!("one exact Terminal registrar occurrence must be retained");
    };
    assert_eq!(occurrence.placement_index(), 0);
    let callback_thunk_identity = occurrence.callback_thunk_identity();
    assert_eq!(
        callback_thunk_identity.callback_thunk_placement_index(),
        Some(0)
    );
    let callback_continuation = callback_thunk_identity.associated_source_continuation();
    assert_eq!(callback_continuation.machine, placement.selected_machine);
    assert_eq!(callback_continuation.state, placement.selected_entry);
    assert_eq!(callback_continuation.segment_index, 0);
    assert_eq!(
        Some(callback_thunk_identity),
        omega_backend_plan::canonical_callback_thunk_identity(0, placement)
    );
    let thunk = occurrence.callback_thunk_artifact();
    assert_eq!(
        thunk.private_symbol(),
        &omega_backend_plan::canonical_callback_private_symbol(placement)
    );
    let receipt = thunk.lowering_receipt();
    assert_eq!(receipt.source_machine, placement.selected_machine);
    assert_eq!(receipt.source_entry, placement.selected_entry);
    let thunk_module = psi_terminal_codec::decode_module(thunk.artifact().semantic_bytes())
        .expect("decode canonical callback thunk semantics");
    let [thunk_machine] = thunk_module.machines.as_slice() else {
        panic!("the bounded callback thunk must contain one Terminal machine");
    };
    let (
        [thunk_parameter],
        psi_terminal::TerminalMachineResult::Scalar(thunk_result),
        [thunk_block],
    ) = (
        thunk_machine.parameters.as_slice(),
        &thunk_machine.result,
        thunk_machine.blocks.as_slice(),
    )
    else {
        panic!("the bounded callback thunk must retain one scalar parameter/result block");
    };
    let u64_type =
        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 64).expect("u64 Terminal type");
    assert_eq!(thunk_module.entry, receipt.terminal_machine);
    assert_eq!(thunk_machine.id, receipt.terminal_machine);
    assert_eq!(thunk_machine.entry, receipt.terminal_entry);
    assert_eq!(
        thunk_parameter.scalar_type,
        psi_core::ScalarType::Integer(u64_type)
    );
    assert_eq!(thunk_result.scalar_type, thunk_parameter.scalar_type);
    assert!(thunk_block.parameters.is_empty());
    assert!(thunk_block.operations.is_empty());
    assert!(matches!(
        &thunk_block.terminator,
        psi_terminal::Terminator::Return {
            value,
            cleanup_actions,
            ..
        } if *value == thunk_parameter.id && cleanup_actions.is_empty()
    ));
    let application = occurrence
        .direct_parameter_application()
        .expect("direct callback occurrence retains its target-closed telescope row");
    let materialization = placement
        .private_materialization
        .as_ref()
        .expect("direct callback target-closed materialization");
    let omega_calling_conventions::NativePlace::Parameter(destination) =
        &materialization.destination
    else {
        panic!("direct callback must target one native parameter");
    };
    assert_eq!(application.parameter, *destination);
    assert_eq!(application.native_ordinal, 1);
    assert_eq!(
        application.shape,
        omega_calling_conventions::ValueShape::integer(8, 8)
    );
    assert!(matches!(
        application.placement.locations.as_slice(),
        [omega_calling_conventions::ValueLocation::Register {
            register: omega_calling_conventions::MachineRegister::X86Rdx,
            value_byte_offset: 0,
            byte_size: 8,
        }]
    ));
    let module = psi_terminal_codec::decode_module(retained.artifact().semantic_bytes())
        .expect("decode canonical Terminal semantics");
    let matching = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == occurrence.terminal_operation())
        .collect::<Vec<_>>();
    let [operation] = matching.as_slice() else {
        panic!("occurrence must name one exact Terminal operation");
    };
    assert!(matches!(
        operation.kind,
        psi_terminal::OperationKind::BoundaryCall { .. }
    ));
    retained
        .validate()
        .expect("direct callback occurrence replays against its artifact");

    let drifted_thunk = omega_compilation_report::TerminalCallbackThunkArtifact::new(
        std::sync::Arc::from("__omega_callback_drifted"),
        replay_copy_terminal_artifact(thunk.artifact()),
        receipt,
    )
    .expect("a canonical thunk artifact can carry a mutation-test symbol");
    let drifted_thunk_proposal = omega_compilation_report::TerminalNativeRealizationProposal::new(
        retained.artifact(),
        proposal.target_profile(),
        proposal.native_target(),
        proposal.subsystem(),
        proposal.program_entry().clone(),
        proposal.selected_provider_plans().clone(),
        proposal.external_binding_rows().to_vec(),
        proposal.compiler_builtins().to_vec(),
        vec![
            omega_compilation_report::TerminalCallbackOccurrenceProposal::new(
                0,
                occurrence.terminal_operation(),
                Some(application.clone()),
                callback_thunk_identity,
                drifted_thunk,
            ),
        ],
        proposal.ieee_float_fma_occurrences().to_vec(),
        proposal.boundary_application_demands().clone(),
        proposal.boundary_application_realizations().clone(),
        proposal.checked_boundary_operator_scope().clone(),
    )
    .expect("artifact-local proposal replay does not own checked placement spelling");
    assert!(
        omega_compilation_report::RetainedTerminalArtifact::new_with_native_realization_proposal(
            replay_copy_terminal_artifact(retained.artifact()),
            retained.callback_placements().to_vec(),
            drifted_thunk_proposal,
        )
        .is_err(),
        "retained-product replay must reject callback private-symbol drift",
    );

    let wrong_operation = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id != occurrence.terminal_operation())
        .expect("the registrar literals produce a distinct Terminal operation")
        .id;
    assert!(
        omega_compilation_report::TerminalNativeRealizationProposal::new(
            retained.artifact(),
            proposal.target_profile(),
            proposal.native_target(),
            proposal.subsystem(),
            proposal.program_entry().clone(),
            proposal.selected_provider_plans().clone(),
            proposal.external_binding_rows().to_vec(),
            proposal.compiler_builtins().to_vec(),
            vec![
                omega_compilation_report::TerminalCallbackOccurrenceProposal::new(
                    0,
                    wrong_operation,
                    Some(application.clone()),
                    callback_thunk_identity,
                    occurrence.callback_thunk_artifact().clone(),
                )
            ],
            proposal.ieee_float_fma_occurrences().to_vec(),
            proposal.boundary_application_demands().clone(),
            proposal.boundary_application_realizations().clone(),
            proposal.checked_boundary_operator_scope().clone(),
        )
        .is_err(),
        "a non-boundary Terminal operation cannot replace the registrar call",
    );
    assert!(
        omega_compilation_report::TerminalNativeRealizationProposal::new(
            retained.artifact(),
            proposal.target_profile(),
            proposal.native_target(),
            proposal.subsystem(),
            proposal.program_entry().clone(),
            proposal.selected_provider_plans().clone(),
            proposal.external_binding_rows().to_vec(),
            proposal.compiler_builtins().to_vec(),
            vec![occurrence.clone(), occurrence.clone()],
            proposal.ieee_float_fma_occurrences().to_vec(),
            proposal.boundary_application_demands().clone(),
            proposal.boundary_application_realizations().clone(),
            proposal.checked_boundary_operator_scope().clone(),
        )
        .is_err(),
        "duplicate placement occurrence rows must reject",
    );
    let mut drifted_application = application.clone();
    drifted_application.native_ordinal = 0;
    let drifted_proposal = omega_compilation_report::TerminalNativeRealizationProposal::new(
        retained.artifact(),
        proposal.target_profile(),
        proposal.native_target(),
        proposal.subsystem(),
        proposal.program_entry().clone(),
        proposal.selected_provider_plans().clone(),
        proposal.external_binding_rows().to_vec(),
        proposal.compiler_builtins().to_vec(),
        vec![
            omega_compilation_report::TerminalCallbackOccurrenceProposal::new(
                0,
                occurrence.terminal_operation(),
                Some(drifted_application),
                callback_thunk_identity,
                occurrence.callback_thunk_artifact().clone(),
            ),
        ],
        proposal.ieee_float_fma_occurrences().to_vec(),
        proposal.boundary_application_demands().clone(),
        proposal.boundary_application_realizations().clone(),
        proposal.checked_boundary_operator_scope().clone(),
    )
    .expect("artifact-local replay cannot infer the checked native telescope");
    assert!(
        omega_compilation_report::RetainedTerminalArtifact::new_with_native_realization_proposal(
            replay_copy_terminal_artifact(retained.artifact()),
            retained.callback_placements().to_vec(),
            drifted_proposal,
        )
        .is_err(),
        "retained-product replay must reject native telescope drift",
    );
    let placement_index_drift = omega_backend_plan::canonical_callback_thunk_identity(1, placement)
        .expect("the same exact placement admits a distinct indexed thunk identity");
    assert!(
        omega_compilation_report::TerminalNativeRealizationProposal::new(
            retained.artifact(),
            proposal.target_profile(),
            proposal.native_target(),
            proposal.subsystem(),
            proposal.program_entry().clone(),
            proposal.selected_provider_plans().clone(),
            proposal.external_binding_rows().to_vec(),
            proposal.compiler_builtins().to_vec(),
            vec![
                omega_compilation_report::TerminalCallbackOccurrenceProposal::new(
                    0,
                    occurrence.terminal_operation(),
                    Some(application.clone()),
                    placement_index_drift,
                    occurrence.callback_thunk_artifact().clone(),
                ),
            ],
            proposal.ieee_float_fma_occurrences().to_vec(),
            proposal.boundary_application_demands().clone(),
            proposal.boundary_application_realizations().clone(),
            proposal.checked_boundary_operator_scope().clone(),
        )
        .is_err(),
        "a callback-thunk role for another placement index must reject",
    );
    let mut continuation_drift_placement = placement.clone();
    continuation_drift_placement.selected_entry = placement.registration_operation;
    let continuation_drift =
        omega_backend_plan::canonical_callback_thunk_identity(0, &continuation_drift_placement)
            .expect("a valid but unrelated continuation can form a mutation identity");
    let continuation_drift_proposal =
        omega_compilation_report::TerminalNativeRealizationProposal::new(
            retained.artifact(),
            proposal.target_profile(),
            proposal.native_target(),
            proposal.subsystem(),
            proposal.program_entry().clone(),
            proposal.selected_provider_plans().clone(),
            proposal.external_binding_rows().to_vec(),
            proposal.compiler_builtins().to_vec(),
            vec![
                omega_compilation_report::TerminalCallbackOccurrenceProposal::new(
                    0,
                    occurrence.terminal_operation(),
                    Some(application.clone()),
                    continuation_drift,
                    occurrence.callback_thunk_artifact().clone(),
                ),
            ],
            proposal.ieee_float_fma_occurrences().to_vec(),
            proposal.boundary_application_demands().clone(),
            proposal.boundary_application_realizations().clone(),
            proposal.checked_boundary_operator_scope().clone(),
        )
        .expect("artifact-only proposal replay cannot reconstruct checked continuation handles");
    assert!(
        omega_compilation_report::RetainedTerminalArtifact::new_with_native_realization_proposal(
            replay_copy_terminal_artifact(retained.artifact()),
            retained.callback_placements().to_vec(),
            continuation_drift_proposal,
        )
        .is_err(),
        "retained-product replay must reject callback-thunk continuation drift",
    );
    let missing = omega_compilation_report::TerminalNativeRealizationProposal::new(
        retained.artifact(),
        proposal.target_profile(),
        proposal.native_target(),
        proposal.subsystem(),
        proposal.program_entry().clone(),
        proposal.selected_provider_plans().clone(),
        proposal.external_binding_rows().to_vec(),
        proposal.compiler_builtins().to_vec(),
        Vec::new(),
        proposal.ieee_float_fma_occurrences().to_vec(),
        proposal.boundary_application_demands().clone(),
        proposal.boundary_application_realizations().clone(),
        proposal.checked_boundary_operator_scope().clone(),
    )
    .expect("artifact-local replay permits an empty occurrence catalog");
    let (artifact, placements, _) = retained.into_parts();
    assert!(
        omega_compilation_report::RetainedTerminalArtifact::new_with_native_realization_proposal(
            artifact, placements, missing,
        )
        .is_err(),
        "retained product replay requires one occurrence per callback placement",
    );

    let native = compile(fixture.request(RequestedCompileProduct::NativeArtifact, "native"))
        .expect_err("native production remains fenced after exact occurrence custody");
    assert_custody_diagnostic(&native, "native-artifact", 1);
}

#[derive(Debug)]
struct CallbackRegistrarExecution {
    requirement: String,
    provider_plan_report_identity: u64,
}

impl ProviderExecutionEvidence for CallbackRegistrarExecution {
    fn requirement_identity(&self) -> &str {
        &self.requirement
    }

    fn provider_plan_report_identity(&self) -> u64 {
        self.provider_plan_report_identity
    }

    fn provider_execution_report_identity(&self) -> u64 {
        0x4341_4c4c_4241_0001
    }

    fn provider_execution_report_fingerprint(&self) -> u64 {
        0x4341_4c4c_4241_0002
    }

    fn normalized_root_report_identity(&self) -> u64 {
        0x4341_4c4c_4241_0003
    }

    fn boundary_contract_report_fingerprint(&self) -> u64 {
        0x4341_4c4c_4241_0004
    }
}

#[test]
fn direct_callback_relocation_resolves_to_its_private_function() {
    let fixture = Fixture::direct_with_import();
    let retained = compile(fixture.request(RequestedCompileProduct::TerminalArtifact, "native"))
        .unwrap_or_else(|diagnostics| {
            panic!("direct callback import must reach Terminal: {diagnostics:#?}")
        })
        .into_retained_terminal_artifact()
        .expect("direct callback import retains its native proposal");
    let proposal = retained
        .native_realization_proposal()
        .expect("direct callback native proposal");
    let [occurrence] = proposal.callback_occurrences() else {
        panic!("one direct callback occurrence expected");
    };
    let callback_identity = occurrence.callback_thunk_identity();
    let callback_operation = occurrence.terminal_operation();
    let callback_symbol_name = occurrence
        .callback_thunk_artifact()
        .private_symbol()
        .to_owned();
    let callback_materialization = retained.callback_placements()[0]
        .private_materialization
        .as_ref()
        .expect("direct callback materialization");

    let imported = proposal
        .selected_provider_plans()
        .plans()
        .iter()
        .flat_map(|plan| {
            plan.rows.iter().filter_map(move |row| {
                matches!(row.binding, ProviderBinding::Import { .. }).then_some((plan, row))
            })
        })
        .collect::<Vec<_>>();
    let [(provider_plan, provider_row)] = imported.as_slice() else {
        panic!("one exact callback registrar import expected");
    };
    let provider_plan_report_identity = provider_plan.report_fingerprint();
    let provider_plan_commitment =
        SameStackProviderPlanCommitment::from_digest(*provider_plan.identity_digest().as_bytes());
    let provider_requirement = provider_row.requirement_identity.clone();
    let execution = CallbackRegistrarExecution {
        requirement: provider_requirement.clone(),
        provider_plan_report_identity,
    };
    let same_stack = admit_same_stack_contribution(
        SameStackContributionAdmissionCandidate {
            provider_plan_report_identity,
            provider_plan_commitment,
            requirement_identity: provider_requirement.clone(),
            receipt: SameStackContributionAdmissionReceiptId::from_normalized_identity(
                0x4341_4c4c_4241_0005,
            )
            .expect("callback registrar admission receipt"),
            bytes: 64,
            alignment: 16,
        },
        provider_plan_report_identity,
        provider_plan_commitment,
        &provider_requirement,
    )
    .expect("callback registrar same-stack contribution admits");
    let policy_rows = proposal
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
    let [(locator, boundary_entry_plan)] = policy_rows.as_slice() else {
        panic!("one exact callback registrar authority row expected");
    };
    let policy = omega_terminal_psi_to_native_artifact::terminal_authority_policy_with_rows(vec![
        omega_terminal_psi_to_native_artifact::TerminalAuthorityPolicyRow::new(
            omega_terminal_psi_to_native_artifact::normalized_foreign_terminal_mechanism_with_callback_materializations(
                locator,
                boundary_entry_plan,
                &callback_materialization.context,
            )
            .expect("callback registrar mechanism is canonical"),
            omega_effects::TerminalAuthorityDisposition::from_classes([]),
        ),
    ])
    .expect("callback registrar receiving policy");
    let artifact = realize_retained_terminal_artifact_with_source_evaluated_imports_and_policy(
        retained,
        &psi_proof_admission::AdmissionProfile::default(),
        &omega_optimization_core::OptimizationSelections::default(),
        policy,
        &[SourceEvaluatedImportSettlement::new(
            &execution,
            &same_stack,
        )],
    )
    .unwrap_or_else(|diagnostics| panic!("direct callback must realize: {diagnostics:#?}"));
    artifact
        .validate()
        .expect("direct callback native artifact replays");

    let object = artifact.object();
    let [foreign] = object.foreign_calls() else {
        panic!("one callback registrar call expected");
    };
    let callback = foreign
        .callback_address
        .as_ref()
        .expect("registrar call retains callback address custody");
    assert_eq!(callback.target.callback_function, callback_identity);
    assert_eq!(callback.target.terminal_operation, callback_operation);
    assert_eq!(callback.target.application.native_ordinal, 1);
    assert_eq!(
        callback.destination,
        omega_machine_code::CallbackAddressDestination::Register(
            omega_calling_conventions::MachineRegister::X86Rdx,
        )
    );
    assert_eq!(
        foreign.provider_execution.provider_plan_report_identity,
        provider_plan_report_identity,
    );
    let (private_symbol, private_plan) =
        omega_object_file::object_function_symbol(object.object(), callback_identity)
            .expect("callback identity resolves one private object symbol");
    assert_eq!(private_plan.name.as_str(), callback_symbol_name.as_ref());
    let callback_relocations = object
        .relocations()
        .records()
        .filter(|(_, relocation)| relocation.symbol_handle == private_symbol)
        .map(|(_, relocation)| relocation)
        .collect::<Vec<_>>();
    let [relocation] = callback_relocations.as_slice() else {
        panic!("one x86 callback-address relocation expected");
    };
    let omega_machine_code::CallbackAddressEncoding::X86_64Relative32 { relocation_offset } =
        callback.encoding
    else {
        panic!("Windows x64 callback address must use rel32");
    };
    assert_eq!(relocation.offset, relocation_offset);
    assert_eq!(relocation.byte_width, 4);
    assert_eq!(relocation.addend, 0);
    assert_eq!(
        relocation.kind,
        omega_object_file::RelocationKind::X86_64Relative32
    );
    let caller = object
        .functions()
        .iter()
        .find(|function| function.machine == foreign.machine)
        .expect("callback registrar function");
    assert_eq!(
        relocation.origin,
        omega_object_file::RelocationOrigin::SemanticOperation {
            function_symbol_handle: caller.symbol,
            operation_identity: callback_operation.get(),
        }
    );
    assert_eq!(object.relocations().record_count(), 2);
    let final_text = &artifact.image().output().final_text_bytes;
    let displacement = final_text
        .get(relocation_offset..relocation_offset + 4)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(i32::from_le_bytes)
        .expect("final callback rel32 field");
    let instruction_end = artifact
        .image()
        .output()
        .executable_regions
        .text_address
        .checked_add(relocation_offset as u64 + 4)
        .expect("callback instruction address");
    let actual_target = instruction_end
        .checked_add_signed(i64::from(displacement))
        .expect("callback rel32 target");
    let expected_target = artifact
        .image()
        .output()
        .executable_regions
        .text_address
        .checked_add(private_plan.offset as u64)
        .expect("private callback target");
    assert_eq!(actual_target, expected_target);

    let installation = omega_image_emission::build_installation_record_with_provider_executions(
        artifact.image(),
        psi_core::ProfileDecisionId::new(1).expect("profile decision"),
        [&execution],
    )
    .expect("source-derived callback installation");
    let [installed_private] = installation.private_functions() else {
        panic!("installation must retain one compiler-private callback function");
    };
    let [image_private] = artifact.image().private_functions() else {
        panic!("image must retain one compiler-private callback function");
    };
    assert_eq!(installed_private.identity, callback_identity);
    assert_eq!(installed_private.source_psi, image_private.source_psi);
    assert_eq!(installed_private.machine, image_private.function.machine);
    assert_eq!(installed_private.text_offset, private_plan.offset);
    assert_eq!(installed_private.byte_count, private_plan.size);
    assert_eq!(
        Some(&installed_private.fixed_integer_scalar_abi),
        image_private.function.fixed_integer_scalar_abi.as_ref()
    );
    let installation_bytes = omega_image_emission::encode_installation_record(&installation)
        .expect("source-derived callback installation encoding");
    let decoded = omega_image_emission::decode_installation_record(&installation_bytes)
        .expect("source-derived callback installation decoding");
    assert_eq!(decoded, installation);
    omega_image_emission::validate_installation_record(&decoded, artifact.image())
        .expect("source-derived callback installation replay");
}
