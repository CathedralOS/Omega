#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::CheckedCompileRequest;
use compiler::{
    CompileOptions, CompileRequest, RequestedCompileProduct, RetainedNativeRealizationRequest,
    SourceEvaluatedImportSettlement, compile, compile_to_checked, realize_retained_native_artifact,
};
use effects::provider_plan::ProviderBinding;
use fixture_package_inputs::{
    bundled_standard_library_dependency_declaration, bundled_standard_library_root,
    candidate_program_entry_binding, fixture_package_identity, repository_fixture_package_inputs,
};
use installation_evidence::ProviderExecutionEvidence;
use package_compilation::PackageCompilationInputs;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use task_plans::{
    SameStackContributionAdmissionCandidate, SameStackContributionAdmissionReceiptId,
    SameStackProviderPlanCommitment, admit_same_stack_contribution,
};
use terminal_psi_to_abstract_operations::provider_admission;

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

/// The direct callback canary's entry header: the free `main` its build root
/// binds and the anchor before which the import-bound variant declares its
/// leaf provider. The canary carries no receiver data for a `Main::main`.
const DIRECT_ENTRY: &str = "machine main() reaches HookRegistrar {";

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
    registrar: Binding<WindowRegistrar>;
    specification: Spread<ForeignRecord>;
}

machine RegistrarUser::configure(&mut self) reaches WindowRegistrar {
    WindowRegistrar::register<CallbackProvider::call, CallbackProvider::call>(&self.specification);
}

data Main { }
machine Main::main() { }
"#;

/// The Windows x86-64 program entry is an authored standard-library contract
/// (`targets/windows_x86_64/entry.omg`), and that contract declares its calling
/// vocabulary through the bundled `std::calling` module. A fixture that also
/// copied `calling.omg` into its own package would declare the same vocabulary
/// twice in one program, so these fixtures take the standard library as an
/// ordinary package dependency and import `omega_language_std::calling`.
struct Fixture {
    root: PathBuf,
    main: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = fixture_root("callback-terminal-custody");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create callback-custody fixture");

        let source = fs::read_to_string(
            bundled_standard_library_root().join("tests/callback_materialization_closure.omg"),
        )
        .expect("read callback materialization fixture");
        let source = source
            .replacen("use calling;", "use omega_language_std::calling;", 1)
            .replacen(
                "use omega::language::core::layout;",
                "use omega::language::core::layout;\nuse omega::language::core::binding;",
                1,
            )
            .replacen(
                "boundary trait WindowProcedure {",
                "pub boundary trait WindowProcedure: Calling<RegistrarPolicy> {",
                1,
            )
            .replacen(
                "data RegistrarPolicy { }",
                "pub data RegistrarPolicy { }",
                1,
            )
            .replacen(
                "boundary trait WindowRegistrar:",
                "pub boundary trait WindowRegistrar:",
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
            format!(
                "machine build(builder: &mut Build) {{\n    \
                 Build::application(builder, \"callback_terminal_custody\");\n\
                 {}    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);\n}}\n",
                bundled_standard_library_dependency_declaration(),
            ),
        )
        .expect("write callback-custody build policy");

        Self { root, main }
    }

    fn reachable() -> Self {
        let fixture = Self::new();
        let source = fs::read_to_string(&fixture.main).expect("read callback fixture");
        let head = source
            .split("data CallbackProvider")
            .next()
            .expect("fixture head before callback use");
        let source = format!(
            "{head}{}",
            r#"data CallbackProvider { }

machine CallbackProvider::call(message: u64)
satisfies WindowProcedure::call
{
}

data Main {
    specification: Spread<ForeignRecord>;
}

machine Main::main(&mut self) reaches WindowRegistrar {
    WindowRegistrar::register<CallbackProvider::call, CallbackProvider::call>(&self.specification);
}
"#
        );
        fs::write(&fixture.main, source).expect("write reachable callback fixture");
        fixture
    }

    /// The closure registrar with its provider bound to an import: same two
    /// private slots and the authored `call(message: u64) -> u64` callback
    /// signature, but `register` resolves to `register_leaf`'s DLL import so
    /// the program can realize a native image. The calling policy's `build`
    /// arm additionally publishes the callback's result placement so the
    /// thunk's emitted scalar ABI has a declared signature to rejoin.
    fn reachable_with_import() -> Self {
        let root = fixture_root("callback-terminal-custody-two-slot-import");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create two-slot callback-import fixture");
        let source = fs::read_to_string(
            bundled_standard_library_root().join("tests/callback_materialization_closure.omg"),
        )
        .expect("read callback materialization fixture");
        let source = source
            .replacen("use calling;", "use omega_language_std::calling;", 1)
            .replacen(
                "use omega::language::core::layout;",
                "use omega::language::core::layout;\nuse omega::language::core::binding;\nuse omega::language::core::external_binding;",
                1,
            )
            .replacen(
                "boundary trait WindowProcedure {",
                "pub boundary trait WindowProcedure: Calling<RegistrarPolicy> {",
                1,
            )
            .replacen(
                "data RegistrarPolicy { }",
                "pub data RegistrarPolicy { }",
                1,
            )
            .replacen(
                "boundary trait WindowRegistrar:",
                "pub boundary trait WindowRegistrar:",
                1,
            )
            .replace(
                "        output.call.stack_alignment = 16;",
                r#"        output.call.ordinary_clobbers.register_count = 1;
        output.call.ordinary_clobbers.registers[0] = MachineRegister::X86R11;
        output.call.stack_alignment = 16;"#,
            )
            .replace(
                "        output.state.stack = EntryStack::ProviderSelected;",
                r#"        output.state.permitted_transitive_use.general_registers = true;
        output.state.stack = EntryStack::ProviderSelected;"#,
            )
            .replacen(
                "            _ -> accept(output)",
                "            _ -> finish(signature, output)",
                1,
            )
            .replacen(
                "    state bind_callback(",
                r#"    state finish(
        signature: BoundarySignature,
        output: BoundaryEntryPlan
    ) -> BoundaryPlanResult {
        transition signature.has_result {
            true -> check_result(signature, signature.result, output)
            _ -> accept(output)
        }
    }

    state check_result(
        signature: BoundarySignature,
        result: u64,
        output: BoundaryEntryPlan
    ) -> BoundaryPlanResult {
        transition result < 256 {
            true -> finish_with_result(signature, result, output)
            _ -> reject()
        }
    }

    state finish_with_result(
        signature: BoundarySignature,
        result: u64,
        output: BoundaryEntryPlan
    ) -> BoundaryPlanResult {
        let mut bound: BoundaryEntryPlan = output;
        bound.call.has_result = true;
        bound.call.result.shape.class = AbiValueClass::Integer;
        bound.call.result.shape.byte_size = signature.shapes[result].byte_size;
        bound.call.result.shape.alignment = signature.shapes[result].alignment;
        bound.call.result.location_count = 1;
        bound.call.result.locations[0] = ValueLocation::Register {
            register: MachineRegister::X86Rax,
            value_byte_offset: 0,
            byte_size: signature.shapes[result].byte_size,
        };
        transition { _ -> accept(bound) }
    }

    state bind_callback("#,
                1,
            )
            .replace(
                "data Main { }\nmachine Main::main(&mut self) { }\n",
                r#"data CallbackProvider { }

machine CallbackProvider::call(message: u64) -> u64
satisfies WindowProcedure::call
{
    message
}

windows_x86_64 machine register_binding() -> ForeignBinding<12, 24, 0> {
    ForeignBinding::DllImport {
        import: DllImport::PeByName {
            library: "kernel32.dll",
            export: "FlushProcessWriteBuffers",
        },
    }
}

machine register_leaf<machine Selected, machine SecondarySelected>(
    specification: &Spread<ForeignRecord>
)
where machine Selected satisfies WindowProcedure::call;
where machine SecondarySelected satisfies WindowProcedure::call;
satisfies WindowRegistrar::register
via register_binding();

data Main {
    specification: Spread<ForeignRecord>;
}

machine Main::main(&mut self) reaches WindowRegistrar {
    WindowRegistrar::register<CallbackProvider::call, CallbackProvider::call>(&self.specification);
}
"#,
            );
        assert!(source.contains("register_leaf"));
        let main = root.join("main.omg");
        fs::write(&main, source).expect("write two-slot callback-import source");
        fs::write(
            root.join("build.omg"),
            format!(
                "machine build(builder: &mut Build) {{\n    \
                 Build::application(builder, \"callback_terminal_custody\");\n\
                 {}    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);\n}}\n",
                bundled_standard_library_dependency_declaration(),
            ),
        )
        .expect("write two-slot callback-import build policy");
        Self { root, main }
    }

    fn direct() -> Self {
        let root = fixture_root("direct-callback-terminal-custody");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create direct callback-custody fixture");
        let main = root.join("main.omg");
        let source = fs::read_to_string(
            bundled_standard_library_root().join("tests/direct_callback_parameter.omg"),
        )
        .expect("read direct callback source canary")
        .replacen("use calling;", "use omega_language_std::calling;", 1);
        assert_eq!(
            source.matches(DIRECT_ENTRY).count(),
            1,
            "direct canary entry"
        );
        fs::write(&main, source).expect("write package-aware direct callback source canary");
        fs::write(
            root.join("build.omg"),
            format!(
                "machine build(builder: &mut Build) {{\n    \
                 Build::application(builder, \"direct_callback_terminal_custody\");\n\
                 {}    builder.roots.bind(windows_x86_64::ProgramEntry, main);\n}}\n",
                bundled_standard_library_dependency_declaration(),
            ),
        )
        .expect("write direct callback-custody build policy");
        Self { root, main }
    }

    fn direct_with_import() -> Self {
        let fixture = Self::direct();
        let source = fs::read_to_string(&fixture.main).expect("read direct callback fixture");
        let source = source
            .replacen(
                "use omega_language_std::calling;",
                "use omega_language_std::calling;\nuse omega::language::core::external_binding;",
                1,
            )
            .replacen(
                DIRECT_ENTRY,
                r#"windows_x86_64 machine install_binding() -> ForeignBinding<12, 24, 0> {
    ForeignBinding::DllImport {
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

machine main() reaches HookRegistrar {"#,
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

    /// The fixture application depends on the exact repository standard
    /// library through its authored row and accepts its checked
    /// `WindowsX86_64Application` entry schema. Without that accepted binding
    /// the bundled Windows entry contract cannot take the program-entry role
    /// the build declaration binds.
    fn package_inputs(&self) -> PackageCompilationInputs {
        let entry = candidate_program_entry_binding(
            "windows_x86_64",
            &bundled_standard_library_root(),
            fixture_package_identity(2),
        )
        .unwrap_or_else(|diagnostics| panic!("Windows entry fixture acceptance: {diagnostics:#?}"))
        .expect("Windows x86-64 binds a reviewed entry candidate");
        repository_fixture_package_inputs(&self.main)
            .expect("the callback fixture authors its std dependency")
            .with_accepted_semantic_bindings(vec![entry])
            .expect("callback fixture entry acceptance")
    }

    fn request(&self, product: RequestedCompileProduct, tag: &str) -> CompileRequest {
        CompileRequest::new(CompileOptions {
            root_path: self.main.clone(),
            build_dir: Some(self.root.join(format!("build-{tag}"))),
            target_name: Some("windows_x86_64".to_owned()),
        })
        .with_package_inputs(self.package_inputs())
        .with_requested_product(product)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn replay_copy_terminal_artifact(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
) -> terminal_codec::CanonicalTerminalArtifact {
    let module = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("decode copied Terminal semantics");
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes())
        .expect("decode copied Terminal proof");
    let debug = artifact.debug_bytes().map(|bytes| {
        terminal_codec::decode_debug_map(&module, bytes).expect("decode copied Terminal debug map")
    });
    terminal_codec::CanonicalTerminalArtifact::from_parts(
        &module,
        &proof,
        artifact.optimization(),
        debug.as_ref(),
    )
    .expect("rebuild independently replayed Terminal artifact")
}

/// Callback custody is no longer the native-production fence: it transports
/// through the artifact chain, so an authored registrar with no `via`
/// provider reaches ordinary provider resolution and rejects there.
fn assert_unprovided_registrar_diagnostic(
    diagnostics: &[diagnostics::Diagnostic],
    requirement: &str,
) {
    assert_eq!(
        diagnostics.len(),
        1,
        "unexpected diagnostics: {diagnostics:#?}"
    );
    let message = diagnostics[0].message.as_str();
    assert!(
        message.contains("terminal-authority closure review"),
        "unexpected diagnostic: {message}"
    );
    assert!(
        message.contains(requirement),
        "the diagnostic must name the unprovided requirement: {message}"
    );
    assert!(
        message.contains("resolves to 0 selected provider rows"),
        "unexpected diagnostic: {message}"
    );
}

/// One program holds exactly one calling vocabulary. The build declaration
/// binds `windows_x86_64::ProgramEntry`, which pulls the authored
/// `targets/windows_x86_64/entry.omg` contract and its `std::calling` module
/// into the program; a package that also copies `calling.omg` into its own
/// root and imports it locally declares the same vocabulary a second time.
/// The registrar materializations above bind one `CallbackBinderIdentity`, so
/// this shape must keep rejecting rather than silently selecting a copy.
///
/// Recorded stop (linux x86-64, re-witnessed 2026-09-21): the fixture does not
/// reach its designed assertion — `compile_to_checked` already rejects with
/// "target boundary schema `WindowsX86_64Application` retains 0 evaluated
/// calling plans for semantic requirement `ProgramStorageEntry::enter` instead
/// of exactly one", a boundary-schema regression that predates this suite's
/// transport legs. When that leg lands, the test resumes checking the
/// duplicate-vocabulary rejection below.
#[test]
fn a_package_local_calling_copy_rejects_beside_the_standard_library_entry() {
    let fixture = Fixture::new();
    fs::copy(
        bundled_standard_library_root().join("calling.omg"),
        fixture.root.join("calling.omg"),
    )
    .expect("copy a second package-local calling vocabulary");
    let source = fs::read_to_string(&fixture.main)
        .expect("read callback fixture")
        .replacen("use omega_language_std::calling;", "use calling;", 1);
    fs::write(&fixture.main, source).expect("write package-local calling import");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(fixture.package_inputs()),
        ..CheckedCompileRequest::new(&fixture.main, Some("windows_x86_64"))
    })
    .expect_err("one program cannot declare the calling vocabulary twice");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "duplicate data `CallbackBinderIdentity`"),
        "unexpected diagnostics: {diagnostics:#?}",
    );
}

#[test]
fn terminal_handoff_rejects_callbacks_outside_the_emitted_entry_closure() {
    let fixture = Fixture::new();
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(fixture.package_inputs()),
        ..CheckedCompileRequest::new(&fixture.main, Some("windows_x86_64"))
    })
    .expect("callback program should reach checked compilation");
    assert_eq!(checked.execution().callback_placements.as_slice().len(), 2);
    compile(fixture.request(RequestedCompileProduct::Check, "check"))
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect("check-only compilation retains callback placements without executing them");
    let terminal = compile(fixture.request(RequestedCompileProduct::TerminalArtifact, "terminal"))
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect_err("unreachable callback placements cannot float beside an unrelated artifact");
    assert!(
        terminal.iter().any(|diagnostic| diagnostic
            .message
            .contains("resolves to 0 Terminal registrar occurrences")),
        "unexpected diagnostics: {terminal:#?}",
    );
}

#[test]
fn terminal_report_rejects_native_proofs_and_freezes_published_context() {
    use package_compilation::build_execution::build_value::PccRequests;
    let fixture = Fixture::reachable();
    let report = || {
        compile(fixture.request(RequestedCompileProduct::TerminalArtifact, "proof-context"))
            .and_then(compiler::CompileOutcomes::into_single_report)
            .expect("retain Terminal report")
    };
    let directory = fixture.root.join("proof-context-publication");
    assert_eq!(
        report()
            .with_pcc_context(
                PccRequests {
                    psi: false,
                    native: true
                },
                Default::default()
            )
            .unwrap_err(),
        "a Terminal stop cannot satisfy a native proof-product request",
    );
    assert!(!directory.exists());
    let published = compilation_products::publication::publish_compilation(
        report()
            .with_pcc_context(
                PccRequests {
                    psi: true,
                    native: false,
                },
                Default::default(),
            )
            .unwrap(),
        &directory,
    )
    .expect("publish the requested Psi proof pair");
    assert_eq!(published.pcc_publications().len(), 1);
    assert_eq!(
        published
            .with_pcc_context(PccRequests::default(), Default::default())
            .unwrap_err(),
        "published proof-product context cannot be replaced",
    );
}

#[test]
fn reachable_private_callback_registrar_binds_its_terminal_occurrence() {
    let fixture = Fixture::reachable();
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(fixture.package_inputs()),
        ..CheckedCompileRequest::new(&fixture.main, Some("windows_x86_64"))
    })
    .expect("reachable callback program should reach checked compilation");
    assert_eq!(checked.execution().callback_placements.as_slice().len(), 2);
    let terminal = compile(fixture.request(RequestedCompileProduct::TerminalArtifact, "terminal"))
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect("the admitted registrar must close its thunk custody at Terminal");
    let expected_bytes = terminal
        .artifact()
        .expect("retained Terminal bytes")
        .to_bytes();
    let publication = fixture.root.join("published-terminal");
    assert!(!publication.exists(), "compilation does not publish files");
    let terminal = compilation_products::publication::publish_compilation(
        terminal
            .with_pcc_context(
                package_compilation::build_execution::build_value::PccRequests {
                    psi: true,
                    native: false,
                },
                proof_admission::AdmissionProfile::default(),
            )
            .expect("Terminal supports its Psi proof request"),
        &publication,
    )
    .expect("explicit Terminal publication installs its requested pair");
    assert_eq!(
        fs::read(publication.join("main.psi")).unwrap(),
        expected_bytes
    );
    let [proof_pair] = terminal.pcc_publications() else {
        panic!("exactly one requested Psi proof pair")
    };
    assert_eq!(proof_pair.artifact_path, publication.join("main.psi"));
    assert_eq!(proof_pair.sidecar_path, publication.join("main.psi.proof"));
    assert_eq!(
        fs::metadata(&proof_pair.sidecar_path).unwrap().len(),
        proof_pair.sidecar_byte_len
    );
    assert!(terminal.has_consistent_executable_publication_custody());
    let retained = terminal
        .into_retained_terminal_artifact()
        .expect("Terminal product owns the private callback sidecar");
    let placements = retained.callback_placements();
    let [first_placement, second_placement] = placements else {
        panic!("both private callback placements must survive");
    };
    for placement in [first_placement, second_placement] {
        let materialization = placement
            .private_materialization
            .as_ref()
            .expect("private callback placement retains its materialization");
        assert!(
            matches!(
                materialization.destination,
                calling_conventions::NativePlace::Field { .. }
            ),
            "the specification slots materialize as private layout fields",
        );
        assert!(
            materialization
                .direct_registrar_parameter_application
                .is_none(),
            "a field destination's root parameter is not a callback argument",
        );
    }
    // The private slots are compiler-owned layout rows, not authored fields:
    // both live under the same registrar parameter and closed layout but name
    // distinct slot paths, so neither can be reached as an ordinary field.
    let (
        calling_conventions::NativePlace::Field {
            parameter: first_parameter,
            layout: first_layout,
            field_path: first_field_path,
        },
        calling_conventions::NativePlace::Field {
            parameter: second_parameter,
            layout: second_layout,
            field_path: second_field_path,
        },
    ) = (
        &first_placement
            .private_materialization
            .as_ref()
            .expect("first materialization")
            .destination,
        &second_placement
            .private_materialization
            .as_ref()
            .expect("second materialization")
            .destination,
    )
    else {
        unreachable!("checked above")
    };
    assert_eq!(first_parameter, second_parameter);
    assert_eq!(first_layout, second_layout);
    assert_ne!(first_field_path, second_field_path);
    let proposal = retained
        .native_realization_proposal()
        .expect("Terminal product retains its native realization proposal");
    let occurrences = proposal.occurrences().callbacks.as_slice();
    let [first_occurrence, second_occurrence] = occurrences else {
        panic!("one occurrence per private callback placement must be retained");
    };
    // One authored registrar call materializes both private slots, so both
    // occurrences replay to the same canonical Terminal boundary call.
    assert_eq!(
        first_occurrence.terminal_operation(),
        second_occurrence.terminal_operation()
    );
    // Field destinations carry no direct callback argument: the occurrence
    // retains the root record parameter's application the private slot write
    // flows through.
    for (occurrence, placement) in [first_occurrence, second_occurrence]
        .iter()
        .zip([first_placement, second_placement])
    {
        assert_eq!(
            occurrence.destination_parameter_application(),
            placement
                .private_materialization
                .as_ref()
                .and_then(|materialization| materialization.root_parameter_application.as_ref()),
        );
    }
    let module = terminal_codec::decode_module(retained.artifact().semantic_bytes())
        .expect("decode canonical Terminal semantics");
    let matching = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == first_occurrence.terminal_operation())
        .collect::<Vec<_>>();
    let [operation] = matching.as_slice() else {
        panic!("each occurrence must name one exact Terminal operation");
    };
    assert!(matches!(
        operation.kind,
        terminal_psi::OperationKind::BoundaryCall { .. }
    ));
    let u64_type =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .expect("u64 Terminal type");
    for (position, occurrence) in occurrences.iter().enumerate() {
        let placement = &placements[position];
        assert_eq!(occurrence.placement_index(), position);
        let callback_thunk_identity = occurrence.callback_thunk_identity();
        assert_eq!(
            callback_thunk_identity.callback_thunk_placement_index(),
            Some(position)
        );
        let continuation = callback_thunk_identity.associated_source_continuation();
        assert_eq!(continuation.machine, placement.selected_machine);
        assert_eq!(continuation.state, placement.selected_entry);
        assert_eq!(continuation.segment_index, 0);
        assert_eq!(
            Some(callback_thunk_identity),
            backend_plan::canonical_callback_thunk_identity(position, placement)
        );
        let thunk = occurrence.callback_thunk_artifact();
        assert_eq!(
            thunk.private_symbol(),
            &backend_plan::canonical_callback_private_symbol(placement)
        );
        let receipt = thunk.lowering_receipt();
        assert_eq!(receipt.source_machine, placement.selected_machine);
        assert_eq!(receipt.source_entry, placement.selected_entry);
        // Both slots bind the same `u64 -> Unit` provider body: the thunk is
        // one Terminal machine taking the authored message parameter and
        // completing without a value.
        let thunk_module = terminal_codec::decode_module(thunk.artifact().semantic_bytes())
            .expect("decode canonical callback thunk semantics");
        let [thunk_machine] = thunk_module.machines.as_slice() else {
            panic!("the bounded callback thunk must contain one Terminal machine");
        };
        let ([thunk_parameter], [thunk_block]) = (
            thunk_machine.parameters.as_slice(),
            thunk_machine.blocks.as_slice(),
        ) else {
            panic!("the bounded callback thunk must retain one parameter and one block");
        };
        assert_eq!(thunk_module.entry, receipt.terminal_machine);
        assert_eq!(thunk_machine.id, receipt.terminal_machine);
        assert_eq!(thunk_machine.entry, receipt.terminal_entry);
        assert_eq!(
            thunk_parameter.scalar_type,
            semantic_vocabulary::ScalarType::Integer(u64_type)
        );
        assert!(matches!(
            thunk_machine.result,
            terminal_psi::TerminalMachineResult::Unit
        ));
        assert!(thunk_block.parameters.is_empty());
        assert!(thunk_block.operations.is_empty());
        assert!(matches!(
            &thunk_block.terminator,
            terminal_psi::Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } if trivial_affine_discards.is_empty()
        ));
    }
    assert_ne!(
        first_occurrence.callback_thunk_identity(),
        second_occurrence.callback_thunk_identity()
    );
    assert_ne!(
        first_occurrence.callback_thunk_artifact().private_symbol(),
        second_occurrence.callback_thunk_artifact().private_symbol()
    );
    retained
        .validate()
        .expect("private callback occurrences replay against their artifact");

    // Retained-product replay still binds every placement: a dropped
    // occurrence leaves a placement uncovered and rejects.
    let missing = compilation_products::TerminalNativeRealizationProposal::new(
        retained.artifact(),
        proposal.application().clone(),
        proposal.program_entry().clone(),
        proposal.post_terminal_optimizations().clone(),
        proposal.providers().clone(),
        compilation_products::TerminalNativeOccurrences {
            callbacks: vec![first_occurrence.clone()],
            ..proposal.occurrences().clone()
        },
        proposal.boundary_applications().clone(),
        proposal.package_terminal_authority_permissions().to_vec(),
        proposal.behavior_exclusions().clone(),
    )
    .expect("artifact-local replay permits a partial occurrence catalog");
    let (artifact, placements, _) = retained.into_parts();
    assert!(
        compilation_products::RetainedTerminalArtifact::new_with_native_realization_proposal(
            artifact, placements, missing,
        )
        .is_err(),
        "retained product replay requires one occurrence per callback placement",
    );

    let native = compile(fixture.request(RequestedCompileProduct::NativeArtifact, "native"))
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect_err("an unprovided callback registrar cannot realize a native artifact");
    assert_unprovided_registrar_diagnostic(&native, "WindowRegistrar::register");
}

/// Rollback is settled once for the entry and every emitted callback body.
#[test]
fn callback_thunks_follow_the_entrys_effective_optimization_selection() {
    let fixture = Fixture::reachable_with_import();
    let build_path = fixture.root.join("build.omg");
    let build = fs::read_to_string(&build_path).expect("read callback build");
    let build = build.replacen(
        "builder.roots.bind(",
        "Optimizations::enable(&mut builder.optimizations, \
         Optimization::ControlFlowCleanup);\n    builder.roots.bind(",
        1,
    );
    fs::write(build_path, build).expect("select callback optimization");
    let selected = optimization_core::OptimizationSelections::new([
        optimization_core::Optimization::ControlFlowCleanup,
    ])
    .expect("one optimization");
    for disabled in [false, true] {
        let rollback = if disabled {
            compiler::OptimizationRollback::new([
                optimization_core::Optimization::ControlFlowCleanup,
            ])
            .expect("one disabled optimization")
        } else {
            compiler::OptimizationRollback::default()
        };
        let effective = rollback.settle(&selected);
        let expected = effective.effective().project_psi();
        let retained = compile(
            fixture
                .request(
                    RequestedCompileProduct::TerminalArtifact,
                    if disabled { "rollback" } else { "selected" },
                )
                .with_optimization_rollback(rollback),
        )
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| panic!("callback selection must compile: {diagnostics:#?}"))
        .into_retained_terminal_artifact()
        .expect("retained callback product");
        assert_eq!(
            retained.artifact().optimization().selections(),
            expected.selections()
        );
        let occurrences = &retained
            .native_realization_proposal()
            .expect("native callback proposal")
            .occurrences()
            .callbacks;
        assert_eq!(occurrences.len(), 2);
        for occurrence in occurrences {
            assert_eq!(
                occurrence
                    .callback_thunk_artifact()
                    .artifact()
                    .optimization()
                    .selections(),
                expected.selections(),
                "callback thunk must use the same effective selection as its entry (rollback={disabled})",
            );
        }
    }
}

/// The two-slot closure registrar with its provider bound to an import
/// produces a native image: both private callback materializations transport
/// through object emission and final-text replay binds each record-field
/// store's rel32 to the exact private thunk function.
#[test]
fn reachable_two_slot_registrar_realizes_its_private_native_image() {
    assert_two_slot_native_image(optimization_core::OptimizationSelections::default());
}

#[test]
fn optimized_two_slot_registrar_preserves_private_callback_custody() {
    assert_two_slot_native_image(
        optimization_core::OptimizationSelections::new([
            optimization_core::Optimization::SelectedIncomingU12ExactAddImmediate,
            optimization_core::Optimization::X86RelaxConditionalBranchesToRel8V1,
        ])
        .expect("selected physical phases"),
    );
}

fn assert_two_slot_native_image(selections: optimization_core::OptimizationSelections) {
    let fixture = Fixture::reachable_with_import();
    let build_path = fixture.root.join("build.omg");
    let build = fs::read_to_string(&build_path).expect("read callback build");
    let selected = selections
        .as_slice()
        .iter()
        .map(|optimization| {
            format!(
                "Optimizations::enable(&mut builder.optimizations, \
                 Optimization::{});\n    ",
                optimization.build_case_name()
            )
        })
        .collect::<String>();
    fs::write(
        build_path,
        build.replacen(
            "builder.roots.bind(",
            &format!("{selected}builder.roots.bind("),
            1,
        ),
    )
    .expect("select native callback optimizations");
    let post_terminal = selections.project_post_terminal();
    let retained = compile(fixture.request(RequestedCompileProduct::TerminalArtifact, "native"))
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| {
            panic!("two-slot registrar must reach Terminal: {diagnostics:#?}")
        })
        .into_retained_terminal_artifact()
        .expect("two-slot registrar retains its native proposal");
    let proposal = retained
        .native_realization_proposal()
        .expect("two-slot registrar native proposal");
    let [first_occurrence, second_occurrence] = proposal.occurrences().callbacks.as_slice() else {
        panic!("two callback occurrences expected");
    };
    let [first_placement, second_placement] = retained.callback_placements() else {
        panic!("two private callback placements expected");
    };
    let first_materialization = first_placement
        .private_materialization
        .as_ref()
        .expect("first private materialization");
    let second_materialization = second_placement
        .private_materialization
        .as_ref()
        .expect("second private materialization");
    // The call-level context names both binders and both field demands; the
    // two placements share it.
    assert_eq!(
        first_materialization.context, second_materialization.context,
        "both slots of one call share the materialization context"
    );
    for placement in [first_placement, second_placement] {
        let calling_conventions::NativePlace::Field {
            field_path: placement_field_path,
            ..
        } = &placement
            .private_materialization
            .as_ref()
            .expect("private materialization")
            .destination
        else {
            panic!("placement destination is a private record field")
        };
        assert!(
            !placement_field_path.is_empty(),
            "private slot names a field path"
        );
    }
    let materialization_context = first_materialization.context.clone();
    // `retained` moves into realization; every per-occurrence coordinate the
    // replay needs is copied out first.
    let occurrences = [
        (
            first_occurrence.callback_thunk_identity(),
            first_occurrence.terminal_operation(),
            first_occurrence
                .callback_thunk_artifact()
                .private_symbol()
                .to_owned(),
        ),
        (
            second_occurrence.callback_thunk_identity(),
            second_occurrence.terminal_operation(),
            second_occurrence
                .callback_thunk_artifact()
                .private_symbol()
                .to_owned(),
        ),
    ];

    let imported = proposal
        .providers()
        .selection
        .plans()
        .iter()
        .flat_map(|plan| {
            plan.rows.iter().filter_map(move |row| {
                matches!(row.binding, ProviderBinding::Import { .. }).then_some((plan, row))
            })
        })
        .collect::<Vec<_>>();
    let [(provider_plan, provider_row)] = imported.as_slice() else {
        panic!("one exact two-slot registrar import expected");
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
                0x4341_4c4c_4241_0006,
            )
            .expect("two-slot registrar admission receipt"),
            bytes: 64,
            alignment: 16,
        },
        provider_plan_report_identity,
        provider_plan_commitment,
        &provider_requirement,
    )
    .expect("two-slot registrar same-stack contribution admits");
    let policy_rows = proposal
        .providers()
        .external_bindings
        .iter()
        .filter_map(|row| {
            let calling_conventions::ExternalBindingKind::Import { locator } = &row.binding else {
                return None;
            };
            Some((locator, row.boundary_entry_plan.as_ref()?))
        })
        .collect::<Vec<_>>();
    let [(locator, boundary_entry_plan)] = policy_rows.as_slice() else {
        panic!("one exact two-slot registrar authority row expected");
    };
    // The call-level context names both binders and both field demands; the
    // two placements share it.
    assert_eq!(
        first_materialization.context, second_materialization.context,
        "both slots of one call share the materialization context"
    );
    let policy = provider_admission::terminal_authority_policy_with_rows(vec![
        provider_admission::TerminalAuthorityPolicyRow::new(
            provider_admission::normalized_foreign_terminal_mechanism_with_callback_materializations(
                locator,
                boundary_entry_plan,
                &materialization_context,
            )
            .expect("two-slot registrar mechanism is canonical"),
            effects::TerminalAuthorityDisposition::from_classes([]),
        ),
    ])
    .expect("two-slot registrar receiving policy");
    let permission_policy =
        provider_admission::terminal_authority_permission_policy_with_rows(vec![
            provider_admission::TerminalAuthorityPermissionPolicyRow::new(
                provider_plan.schema.identity_digest(),
                provider_requirement.clone(),
                effects::TerminalAuthorityDisposition::from_classes([]),
            ),
        ])
        .expect("two-slot registrar exact service permission");
    let artifact = {
        let image_request = image_emission::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .application()
                .subsystem,
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &proof_admission::AdmissionProfile::default(),
                optimization_selections: post_terminal.selections(),
                terminal_authority_policy: policy,
                accepted_package_terminal_authority_permission_policy:
                    provider_admission::current_terminal_authority_permission_policy(),
                terminal_authority_permission_policy: Some(permission_policy),
                image_request,
                imports: &[SourceEvaluatedImportSettlement::new(
                    &execution,
                    &same_stack,
                )],
            },
        )
        .map(|artifact| match artifact {
            native_artifact::RequestedNativeArtifact::Direct(artifact) => artifact,
            native_artifact::RequestedNativeArtifact::DynamicElf(_) => {
                panic!("direct image request returned dynamic ELF custody")
            }
        })
        .map_err(|(_, diagnostics)| diagnostics)
    }
    .unwrap_or_else(|diagnostics| panic!("two-slot registrar must realize: {diagnostics:#?}"));
    artifact
        .validate()
        .expect("two-slot registrar native artifact replays");

    let object = artifact.object();
    let [foreign] = object.foreign_calls() else {
        panic!("one two-slot registrar call expected");
    };
    let [first_callback, second_callback] = foreign.callback_addresses.as_slice() else {
        panic!("registrar call retains two callback address custody rows");
    };
    let callbacks = [first_callback, second_callback];
    // Each callback store lands in its own private slot under the
    // `specification` parameter record at the plan's authored offsets.
    let mut destinations = std::collections::BTreeSet::new();
    for (callback, (identity, operation, _)) in callbacks.iter().zip(occurrences.iter()) {
        assert_eq!(callback.target.callback_function, *identity);
        assert_eq!(callback.target.terminal_operation, *operation);
        let machine_code::CallbackAddressDestination::RecordField {
            root_register,
            byte_offset,
        } = callback.destination
        else {
            panic!("private slot materializes as a record-field destination")
        };
        assert_eq!(root_register, calling_conventions::MachineRegister::X86Rcx);
        destinations.insert(byte_offset);
    }
    assert_eq!(
        destinations.iter().copied().collect::<Vec<_>>(),
        vec![8, 16],
        "each private slot lands at its authored field offset"
    );

    // Object replay: each callback's rel32 names its own private thunk symbol.
    let mut callback_relocations = Vec::new();
    for (identity, _, private_symbol_name) in occurrences.iter() {
        let (private_symbol, private_plan) =
            object_file::object_function_symbol(object.object(), *identity)
                .expect("callback identity resolves one private object symbol");
        assert_eq!(private_plan.name.as_str(), private_symbol_name.as_ref());
        for (_, relocation) in object
            .relocations()
            .records()
            .filter(|(_, relocation)| relocation.symbol_handle == private_symbol)
        {
            callback_relocations.push((identity, private_plan, relocation));
        }
    }
    assert_eq!(
        callback_relocations.len(),
        2,
        "two callback-address relocations expected"
    );
    let private_symbols = callback_relocations
        .iter()
        .map(|(_, _, relocation)| relocation.symbol_handle)
        .collect::<Vec<_>>();
    let call_relocations = object
        .relocations()
        .records()
        .filter(|(_, relocation)| !private_symbols.contains(&relocation.symbol_handle))
        .map(|(_, relocation)| relocation)
        .collect::<Vec<_>>();
    let [call_relocation] = call_relocations.as_slice() else {
        panic!("one registrar call relocation expected beside the callbacks")
    };
    assert_eq!(object.relocations().record_count(), 3);
    let caller = object
        .functions()
        .iter()
        .find(|function| function.machine == foreign.machine)
        .expect("two-slot registrar function");
    for (identity, private_plan, relocation) in &callback_relocations {
        assert_eq!(relocation.byte_width, 4);
        assert_eq!(relocation.addend, 0);
        assert_eq!(
            relocation.kind,
            object_file::RelocationKind::X86_64Relative32
        );
        assert_eq!(
            relocation.origin,
            object_file::RelocationOrigin::SemanticOperation {
                function_symbol_handle: caller.symbol,
                operation_identity: occurrences
                    .iter()
                    .find(|(occurrence_identity, _, _)| occurrence_identity == *identity)
                    .expect("relocation's occurrence")
                    .1
                    .get(),
            }
        );
        // Final text replay: the store group's rel32 field resolves to the
        // private thunk's own text extent.
        let final_text = &artifact.image().output().final_text_bytes;
        let displacement = final_text
            .get(relocation.offset..relocation.offset + 4)
            .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
            .map(i32::from_le_bytes)
            .expect("callback field-store rel32 field");
        let instruction_end = artifact
            .image()
            .output()
            .executable_regions
            .text_address
            .checked_add(relocation.offset as u64 + 4)
            .expect("callback store instruction address");
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
    }
    assert_eq!(call_relocation.byte_width, 4);

    // Physical replay: one foreign child retains both callback custody rows.
    let physical = artifact
        .physical_evidence()
        .expect("two-slot registrar retains D32 physical evidence");
    let callback_children = physical
        .children()
        .iter()
        .filter_map(|child| {
            let native_artifact::PhysicalRelocationDisposition::UnresolvedNormalizedForeignCall(
                foreign,
            ) = child.relocation()
            else {
                return None;
            };
            match foreign.callbacks() {
                [first, second] => Some((child, foreign, first, second)),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    let [(_physical_child, _physical_foreign, first_physical, second_physical)] =
        callback_children.as_slice()
    else {
        panic!("one two-slot registrar D32 child expected")
    };
    for (physical_callback, (identity, _, _)) in [first_physical, second_physical]
        .iter()
        .zip(occurrences.iter())
    {
        let native_artifact::NormalizedForeignCallbackRelocations::X86_64Relative32 {
            callback_function,
            ..
        } = physical_callback
        else {
            panic!("Windows x64 D32 callback custody must retain rel32 rows")
        };
        assert_eq!(*callback_function, *identity);
    }

    // Installation replay retains both private functions under the image.
    let installation = image_emission::build_installation_record_with_provider_executions(
        artifact.image(),
        semantic_vocabulary::ProfileDecisionId::new(1).expect("profile decision"),
        [&execution],
    )
    .expect("source-derived two-slot callback installation");
    let installed = installation.private_functions();
    assert_eq!(
        installed.len(),
        2,
        "two compiler-private callback functions"
    );
    let image_privates = artifact.image().private_functions();
    assert_eq!(image_privates.len(), 2);
    for (installed_private, (identity, _, _)) in installed.iter().zip(occurrences.iter()) {
        assert_eq!(installed_private.identity, *identity);
        let image_private = image_privates
            .iter()
            .find(|private| private.function.machine == installed_private.machine)
            .expect("image retains the same private function");
        assert_eq!(installed_private.source_psi, image_private.source_psi);
        assert_eq!(
            Some(&installed_private.scalar_abi),
            image_private.function.scalar_abi.as_ref()
        );
    }
    let installation_bytes = image_emission::encode_installation_record(&installation)
        .expect("two-slot callback installation encoding");
    let decoded = image_emission::decode_installation_record(&installation_bytes)
        .expect("two-slot callback installation decoding");
    assert_eq!(decoded, installation);
    image_emission::validate_installation_record(&decoded, artifact.image())
        .expect("two-slot callback installation validates against the image");
}

#[test]
fn repeated_callback_registrars_keep_distinct_ordered_terminal_occurrences() {
    let fixture = Fixture::direct();
    let source = fs::read_to_string(&fixture.main).expect("read direct callback");
    let call = "HookRegistrar::install<HookProvider::call>(1u64, 2u64);";
    assert_eq!(source.matches(call).count(), 1);
    let source = source.replace(
        call,
        &format!("{call}\n    HookRegistrar::install<HookProvider::call>(3u64, 4u64);"),
    );
    fs::write(&fixture.main, source).expect("write repeated callback calls");
    let terminal = compile(fixture.request(RequestedCompileProduct::TerminalArtifact, "repeated"))
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect("two callback calls should retain their own custody");
    let retained = terminal
        .into_retained_terminal_artifact()
        .expect("Terminal product");
    let placements = retained.callback_placements();
    let proposal = retained
        .native_realization_proposal()
        .expect("native proposal");
    let occurrences = proposal.occurrences().callbacks.as_slice();
    assert_eq!(placements.len(), 2);
    assert_eq!(occurrences.len(), 2);
    assert_ne!(placements[0].site, placements[1].site);
    assert_ne!(
        occurrences[0].terminal_operation(),
        occurrences[1].terminal_operation()
    );
    for (position, occurrence) in occurrences.iter().enumerate() {
        assert_eq!(occurrence.placement_index(), position);
        assert_eq!(
            Some(occurrence.callback_thunk_identity()),
            backend_plan::canonical_callback_thunk_identity(position, &placements[position]),
        );
    }
}

#[test]
fn direct_callback_placement_binds_the_exact_terminal_registrar_occurrence() {
    let fixture = Fixture::direct();
    let terminal = compile(fixture.request(RequestedCompileProduct::TerminalArtifact, "terminal"))
        .and_then(compiler::CompileOutcomes::into_single_report)
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
        calling_conventions::NativePlace::Parameter(_)
    ));
    let proposal = retained
        .native_realization_proposal()
        .expect("Terminal product retains its native realization proposal");
    let [occurrence] = proposal.occurrences().callbacks.as_slice() else {
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
        backend_plan::canonical_callback_thunk_identity(0, placement)
    );
    let thunk = occurrence.callback_thunk_artifact();
    assert_eq!(
        thunk.private_symbol(),
        &backend_plan::canonical_callback_private_symbol(placement)
    );
    let receipt = thunk.lowering_receipt();
    assert_eq!(receipt.source_machine, placement.selected_machine);
    assert_eq!(receipt.source_entry, placement.selected_entry);
    let thunk_module = terminal_codec::decode_module(thunk.artifact().semantic_bytes())
        .expect("decode canonical callback thunk semantics");
    let [thunk_machine] = thunk_module.machines.as_slice() else {
        panic!("the bounded callback thunk must contain one Terminal machine");
    };
    let (
        [thunk_parameter],
        terminal_psi::TerminalMachineResult::Scalar(thunk_result),
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
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .expect("u64 Terminal type");
    assert_eq!(thunk_module.entry, receipt.terminal_machine);
    assert_eq!(thunk_machine.id, receipt.terminal_machine);
    assert_eq!(thunk_machine.entry, receipt.terminal_entry);
    assert_eq!(
        thunk_parameter.scalar_type,
        semantic_vocabulary::ScalarType::Integer(u64_type)
    );
    assert_eq!(thunk_result.scalar_type, thunk_parameter.scalar_type);
    assert!(thunk_block.parameters.is_empty());
    assert!(thunk_block.operations.is_empty());
    assert!(matches!(
        &thunk_block.terminator,
        terminal_psi::Terminator::Return {
            value,
            cleanup_actions,
            ..
        } if *value == thunk_parameter.id && cleanup_actions.is_empty()
    ));
    let application = occurrence
        .destination_parameter_application()
        .expect("direct callback occurrence retains its target-closed telescope row");
    let materialization = placement
        .private_materialization
        .as_ref()
        .expect("direct callback target-closed materialization");
    let calling_conventions::NativePlace::Parameter(destination) = &materialization.destination
    else {
        panic!("direct callback must target one native parameter");
    };
    assert_eq!(application.parameter, *destination);
    assert_eq!(application.native_ordinal, 1);
    assert_eq!(
        application.shape,
        calling_conventions::ValueShape::integer(8, 8)
    );
    assert!(matches!(
        application.placement.locations.as_slice(),
        [calling_conventions::ValueLocation::Register {
            register: calling_conventions::MachineRegister::X86Rdx,
            value_byte_offset: 0,
            byte_size: 8,
        }]
    ));
    let module = terminal_codec::decode_module(retained.artifact().semantic_bytes())
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
        terminal_psi::OperationKind::BoundaryCall { .. }
    ));
    retained
        .validate()
        .expect("direct callback occurrence replays against its artifact");

    let drifted_thunk = compilation_products::TerminalCallbackThunkArtifact::new(
        std::sync::Arc::from("__omega_callback_drifted"),
        replay_copy_terminal_artifact(thunk.artifact()),
        receipt,
    )
    .expect("a canonical thunk artifact can carry a mutation-test symbol");
    let drifted_thunk_proposal = compilation_products::TerminalNativeRealizationProposal::new(
        retained.artifact(),
        proposal.application().clone(),
        proposal.program_entry().clone(),
        proposal.post_terminal_optimizations().clone(),
        proposal.providers().clone(),
        compilation_products::TerminalNativeOccurrences {
            callbacks: vec![
                compilation_products::TerminalCallbackOccurrenceProposal::new(
                    0,
                    occurrence.terminal_operation(),
                    Some(application.clone()),
                    callback_thunk_identity,
                    drifted_thunk,
                ),
            ],
            ..proposal.occurrences().clone()
        },
        proposal.boundary_applications().clone(),
        proposal.package_terminal_authority_permissions().to_vec(),
        proposal.behavior_exclusions().clone(),
    )
    .expect("artifact-local proposal replay does not own checked placement spelling");
    assert!(
        compilation_products::RetainedTerminalArtifact::new_with_native_realization_proposal(
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
        compilation_products::TerminalNativeRealizationProposal::new(
            retained.artifact(),
            proposal.application().clone(),
            proposal.program_entry().clone(),
            proposal.post_terminal_optimizations().clone(),
            proposal.providers().clone(),
            compilation_products::TerminalNativeOccurrences {
                callbacks: vec![
                    compilation_products::TerminalCallbackOccurrenceProposal::new(
                        0,
                        wrong_operation,
                        Some(application.clone()),
                        callback_thunk_identity,
                        occurrence.callback_thunk_artifact().clone(),
                    )
                ],
                ..proposal.occurrences().clone()
            },
            proposal.boundary_applications().clone(),
            proposal.package_terminal_authority_permissions().to_vec(),
            proposal.behavior_exclusions().clone()
        )
        .is_err(),
        "a non-boundary Terminal operation cannot replace the registrar call",
    );
    assert!(
        compilation_products::TerminalNativeRealizationProposal::new(
            retained.artifact(),
            proposal.application().clone(),
            proposal.program_entry().clone(),
            proposal.post_terminal_optimizations().clone(),
            proposal.providers().clone(),
            compilation_products::TerminalNativeOccurrences {
                callbacks: vec![occurrence.clone(), occurrence.clone()],
                ..proposal.occurrences().clone()
            },
            proposal.boundary_applications().clone(),
            proposal.package_terminal_authority_permissions().to_vec(),
            proposal.behavior_exclusions().clone()
        )
        .is_err(),
        "duplicate placement occurrence rows must reject",
    );
    let mut drifted_application = application.clone();
    drifted_application.native_ordinal = 0;
    let drifted_proposal = compilation_products::TerminalNativeRealizationProposal::new(
        retained.artifact(),
        proposal.application().clone(),
        proposal.program_entry().clone(),
        proposal.post_terminal_optimizations().clone(),
        proposal.providers().clone(),
        compilation_products::TerminalNativeOccurrences {
            callbacks: vec![
                compilation_products::TerminalCallbackOccurrenceProposal::new(
                    0,
                    occurrence.terminal_operation(),
                    Some(drifted_application),
                    callback_thunk_identity,
                    occurrence.callback_thunk_artifact().clone(),
                ),
            ],
            ..proposal.occurrences().clone()
        },
        proposal.boundary_applications().clone(),
        proposal.package_terminal_authority_permissions().to_vec(),
        proposal.behavior_exclusions().clone(),
    )
    .expect("artifact-local replay cannot infer the checked native telescope");
    assert!(
        compilation_products::RetainedTerminalArtifact::new_with_native_realization_proposal(
            replay_copy_terminal_artifact(retained.artifact()),
            retained.callback_placements().to_vec(),
            drifted_proposal,
        )
        .is_err(),
        "retained-product replay must reject native telescope drift",
    );
    let placement_index_drift = backend_plan::canonical_callback_thunk_identity(1, placement)
        .expect("the same exact placement admits a distinct indexed thunk identity");
    assert!(
        compilation_products::TerminalNativeRealizationProposal::new(
            retained.artifact(),
            proposal.application().clone(),
            proposal.program_entry().clone(),
            proposal.post_terminal_optimizations().clone(),
            proposal.providers().clone(),
            compilation_products::TerminalNativeOccurrences {
                callbacks: vec![
                    compilation_products::TerminalCallbackOccurrenceProposal::new(
                        0,
                        occurrence.terminal_operation(),
                        Some(application.clone()),
                        placement_index_drift,
                        occurrence.callback_thunk_artifact().clone(),
                    ),
                ],
                ..proposal.occurrences().clone()
            },
            proposal.boundary_applications().clone(),
            proposal.package_terminal_authority_permissions().to_vec(),
            proposal.behavior_exclusions().clone()
        )
        .is_err(),
        "a callback-thunk role for another placement index must reject",
    );
    let mut continuation_drift_placement = placement.clone();
    continuation_drift_placement.selected_entry = placement.registration_operation;
    let continuation_drift =
        backend_plan::canonical_callback_thunk_identity(0, &continuation_drift_placement)
            .expect("a valid but unrelated continuation can form a mutation identity");
    let continuation_drift_proposal = compilation_products::TerminalNativeRealizationProposal::new(
        retained.artifact(),
        proposal.application().clone(),
        proposal.program_entry().clone(),
        proposal.post_terminal_optimizations().clone(),
        proposal.providers().clone(),
        compilation_products::TerminalNativeOccurrences {
            callbacks: vec![
                compilation_products::TerminalCallbackOccurrenceProposal::new(
                    0,
                    occurrence.terminal_operation(),
                    Some(application.clone()),
                    continuation_drift,
                    occurrence.callback_thunk_artifact().clone(),
                ),
            ],
            ..proposal.occurrences().clone()
        },
        proposal.boundary_applications().clone(),
        proposal.package_terminal_authority_permissions().to_vec(),
        proposal.behavior_exclusions().clone(),
    )
    .expect("artifact-only proposal replay cannot reconstruct checked continuation handles");
    assert!(
        compilation_products::RetainedTerminalArtifact::new_with_native_realization_proposal(
            replay_copy_terminal_artifact(retained.artifact()),
            retained.callback_placements().to_vec(),
            continuation_drift_proposal,
        )
        .is_err(),
        "retained-product replay must reject callback-thunk continuation drift",
    );
    let missing = compilation_products::TerminalNativeRealizationProposal::new(
        retained.artifact(),
        proposal.application().clone(),
        proposal.program_entry().clone(),
        proposal.post_terminal_optimizations().clone(),
        proposal.providers().clone(),
        compilation_products::TerminalNativeOccurrences {
            callbacks: Vec::new(),
            ..proposal.occurrences().clone()
        },
        proposal.boundary_applications().clone(),
        proposal.package_terminal_authority_permissions().to_vec(),
        proposal.behavior_exclusions().clone(),
    )
    .expect("artifact-local replay permits an empty occurrence catalog");
    let (artifact, placements, _) = retained.into_parts();
    assert!(
        compilation_products::RetainedTerminalArtifact::new_with_native_realization_proposal(
            artifact, placements, missing,
        )
        .is_err(),
        "retained product replay requires one occurrence per callback placement",
    );

    let native = compile(fixture.request(RequestedCompileProduct::NativeArtifact, "native"))
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect_err("an unprovided callback registrar cannot realize a native artifact");
    assert_unprovided_registrar_diagnostic(&native, "HookRegistrar::install");
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

/// Compiler-derived import evidence reaches the same private-function,
/// relocation, image and installation replay as explicitly supplied evidence
/// in the two-slot registrar control above.
#[test]
fn direct_callback_relocation_resolves_to_its_private_function() {
    let fixture = Fixture::direct_with_import();
    let retained = compile(fixture.request(RequestedCompileProduct::TerminalArtifact, "native"))
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| {
            panic!("direct callback import must reach Terminal: {diagnostics:#?}")
        })
        .into_retained_terminal_artifact()
        .expect("direct callback import retains its native proposal");
    let proposal = retained
        .native_realization_proposal()
        .expect("direct callback native proposal");
    let [occurrence] = proposal.occurrences().callbacks.as_slice() else {
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
        .providers()
        .selection
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
    let provider_requirement = provider_row.requirement_identity.clone();
    let policy_rows = proposal
        .providers()
        .external_bindings
        .iter()
        .filter_map(|row| {
            let calling_conventions::ExternalBindingKind::Import { locator } = &row.binding else {
                return None;
            };
            Some((locator, row.boundary_entry_plan.as_ref()?))
        })
        .collect::<Vec<_>>();
    let [(locator, boundary_entry_plan)] = policy_rows.as_slice() else {
        panic!("one exact callback registrar authority row expected");
    };
    let policy = provider_admission::terminal_authority_policy_with_rows(vec![
        provider_admission::TerminalAuthorityPolicyRow::new(
            provider_admission::normalized_foreign_terminal_mechanism_with_callback_materializations(
                locator,
                boundary_entry_plan,
                &callback_materialization.context,
            )
            .expect("callback registrar mechanism is canonical"),
            effects::TerminalAuthorityDisposition::from_classes([]),
        ),
    ])
    .expect("callback registrar receiving policy");
    let permission_policy =
        provider_admission::terminal_authority_permission_policy_with_rows(vec![
            provider_admission::TerminalAuthorityPermissionPolicyRow::new(
                provider_plan.schema.identity_digest(),
                provider_requirement.clone(),
                effects::TerminalAuthorityDisposition::from_classes([]),
            ),
        ])
        .expect("callback registrar exact service permission");
    let artifact = {
        let image_request = image_emission::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .application()
                .subsystem,
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &proof_admission::AdmissionProfile::default(),
                optimization_selections:
                    &optimization_core::PostTerminalOptimizationSelections::default(),
                terminal_authority_policy: policy,
                accepted_package_terminal_authority_permission_policy:
                    provider_admission::current_terminal_authority_permission_policy(),
                terminal_authority_permission_policy: Some(permission_policy),
                image_request,
                imports: &[],
            },
        )
        .map(|artifact| match artifact {
            native_artifact::RequestedNativeArtifact::Direct(artifact) => artifact,
            native_artifact::RequestedNativeArtifact::DynamicElf(_) => {
                panic!("direct image request returned dynamic ELF custody")
            }
        })
        .map_err(|(_, diagnostics)| diagnostics)
    }
    .unwrap_or_else(|diagnostics| panic!("direct callback must realize: {diagnostics:#?}"));
    artifact
        .validate()
        .expect("direct callback native artifact replays");

    let object = artifact.object();
    let [foreign] = object.foreign_calls() else {
        panic!("one callback registrar call expected");
    };
    let [callback] = foreign.callback_addresses.as_slice() else {
        panic!("registrar call retains one callback address custody row");
    };
    assert_eq!(callback.target.callback_function, callback_identity);
    assert_eq!(callback.target.terminal_operation, callback_operation);
    assert_eq!(callback.target.application.native_ordinal, 1);
    assert_eq!(
        callback.destination,
        machine_code::CallbackAddressDestination::Register(
            calling_conventions::MachineRegister::X86Rdx,
        )
    );
    assert_eq!(
        foreign.provider_execution.provider_plan_report_identity,
        provider_plan_report_identity,
    );
    let (private_symbol, private_plan) =
        object_file::object_function_symbol(object.object(), callback_identity)
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
    let machine_code::CallbackAddressEncoding::X86_64Relative32 { relocation_offset } =
        callback.encoding
    else {
        panic!("Windows x64 callback address must use rel32");
    };
    assert_eq!(relocation.offset, relocation_offset);
    assert_eq!(relocation.byte_width, 4);
    assert_eq!(relocation.addend, 0);
    assert_eq!(
        relocation.kind,
        object_file::RelocationKind::X86_64Relative32
    );
    let caller = object
        .functions()
        .iter()
        .find(|function| function.machine == foreign.machine)
        .expect("callback registrar function");
    assert_eq!(
        relocation.origin,
        object_file::RelocationOrigin::SemanticOperation {
            function_symbol_handle: caller.symbol,
            operation_identity: callback_operation.get(),
        }
    );
    assert_eq!(object.relocations().record_count(), 2);
    let physical = artifact
        .physical_evidence()
        .expect("direct callback registrar retains D32 physical evidence");
    let callback_children = physical
        .children()
        .iter()
        .filter_map(|child| {
            let native_artifact::PhysicalRelocationDisposition::UnresolvedNormalizedForeignCall(
                foreign,
            ) = child.relocation()
            else {
                return None;
            };
            match foreign.callbacks() {
                [callback] => Some((child, foreign, callback)),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    let [(physical_child, physical_foreign, physical_callback)] = callback_children.as_slice()
    else {
        panic!("one direct callback registrar D32 child expected")
    };
    let native_artifact::NormalizedForeignCallbackRelocations::X86_64Relative32 {
        callback_function,
        relocation: physical_callback_relocation,
    } = physical_callback
    else {
        panic!("Windows x64 D32 callback custody must retain one rel32 row")
    };
    assert_eq!(*callback_function, callback_identity);
    assert_eq!(physical_callback_relocation.object_symbol(), private_symbol);
    assert_eq!(physical_callback_relocation.origin(), relocation.origin);
    assert_eq!(physical_callback_relocation.offset(), relocation.offset);
    assert_eq!(
        physical_callback_relocation.byte_width(),
        relocation.byte_width
    );
    assert_eq!(physical_callback_relocation.addend(), relocation.addend);
    assert_eq!(physical_callback_relocation.kind(), relocation.kind);
    assert_ne!(physical_foreign.object_symbol(), private_symbol);
    let physical_end =
        physical_child.object_span().offset() + physical_child.object_span().byte_count();
    for physical_offset in [
        physical_foreign.offset(),
        physical_callback_relocation.offset(),
    ] {
        assert!(physical_child.object_span().offset() <= physical_offset);
        assert!(physical_offset + 4 <= physical_end);
    }
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

    let installation = image_emission::build_installation_record_with_provider_executions(
        artifact.image(),
        semantic_vocabulary::ProfileDecisionId::new(1).expect("profile decision"),
        artifact.provider_executions(),
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
        Some(&installed_private.scalar_abi),
        image_private.function.scalar_abi.as_ref()
    );
    let installation_bytes = image_emission::encode_installation_record(&installation)
        .expect("source-derived callback installation encoding");
    let decoded = image_emission::decode_installation_record(&installation_bytes)
        .expect("source-derived callback installation decoding");
    assert_eq!(decoded, installation);
    image_emission::validate_installation_record(&decoded, artifact.image())
        .expect("source-derived callback installation replay");
}
