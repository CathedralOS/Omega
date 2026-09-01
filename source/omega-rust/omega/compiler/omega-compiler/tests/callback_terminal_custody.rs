use omega_compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileRequest, RequestedCompileProduct, compile,
    compile_to_checked_with_packages,
};
use omega_package_compilation::{
    BuildDeclarationKind, PackageCompilationInputs, PackageSourceBinding,
};
use psi_core::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};

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
        let root = std::env::temp_dir().join(format!(
            "omega-callback-terminal-custody-{}",
            std::process::id(),
        ));
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
        let root = std::env::temp_dir().join(format!(
            "omega-direct-callback-terminal-custody-{}",
            std::process::id(),
        ));
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
