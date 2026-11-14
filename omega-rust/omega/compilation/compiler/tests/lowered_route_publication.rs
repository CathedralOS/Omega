//! The first piece of a production path from the checker on the lowered
//! form: a program the route accepts is published as canonical Terminal Psi
//! from the route's own module (`AcceptedClosure::publish`), its evidence
//! seals are attached by Omega 00 to that module and fill the accepted
//! obligations' proof fingerprints in Omega 01, and the published program
//! runs in the Terminal interpreter to the exit its fixture pins
//! (`wiki/drafts/designs/check_on_lowered_form.md`, "Route one", Build:
//! "Discharged certificates travel in memory beside `TerminalModule` within
//! one compile so Omega 00 can fill
//! `AcceptedObligationFact::proof_bundle_fingerprint`").
//!
//! Each machine of the published module states the declaration identity of
//! the source machine it lowers, so the Build's selected entry is found on it
//! the way the program-entry receipt finds it on the product check's
//! program.
//!
//! The default compile takes this path for the application: when the route
//! accepts a compilation and its module publishes, the Terminal product is
//! that artifact and its seals stay beside it for Omega 00. An executable
//! child requests its calling declarations before publication and receives a
//! new judgment over the same source and full roster.
use psi_work::WorkMeter;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalStructuralInputs,
};
use terminal_psi_to_terminal_psi::RoutePublication;

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct NoEffects;

impl TerminalEffectHandler for NoEffects {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "the route's subset performs no effect",
        ))
    }
}

fn published_fixture(
    fixture: &str,
) -> (
    RoutePublication,
    terminal_psi::PreparedNominalRoot,
    [u8; 32],
    compiler::CheckedCompilation,
) {
    let main = repo_root().join(fixture).join("main.omg");
    let checked = std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            )
            .result
            .unwrap_or_else(|failure| {
                panic!(
                    "{fixture}: the program must check: {:?}",
                    failure.into_diagnostics()
                )
            })
        })
        .expect("compiler worker")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    });
    let accepted = checked
        .terminal_publication()
        .accepted()
        .expect("the product retains the actual accepted source");
    let publication = accepted
        .closure()
        .publish()
        .expect("the accepted module publishes");
    let (signature, application) = compiler::prepare_program_entry(&checked)
        .expect("the Build-selected entry has a real producer");
    let receipt = application
        .receipt
        .expect("entry preparation")
        .expect("the executable entry has a receipt");
    assert_eq!(
        receipt.terminal_psi_identity(),
        publication.artifact.manifest().semantic()
    );
    let root = terminal_psi::PreparedNominalRoot::new(
        signature.declaration(),
        "diagnostic name is not selection authority",
        None,
    )
    .expect("the selected entry has an exact source owner");
    (publication, root, signature.identity().bytes(), checked)
}

#[test]
fn an_accepted_program_is_published_from_the_routes_module_and_runs() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let (publication, root, signature, checked) =
        published_fixture("tests/fixtures/run/calls/scalar_contract_route_exit");

    // The artifact holds exactly the module the route checked.
    assert_eq!(
        publication.artifact.semantic_module(),
        checked.terminal_publication().accepted().unwrap().module()
    );

    // The ordinary product receipt selects the route's module by exact owner,
    // including its source signature and this artifact's semantic identity.
    let receipt = publication
        .entry_receipt(&root, signature)
        .expect("ordinary entry receipt");
    assert_eq!(
        receipt.terminal_psi_identity(),
        publication.artifact.manifest().semantic()
    );
    assert_eq!(
        receipt.terminal_entry(),
        root.resolve(publication.artifact.semantic_module())
            .unwrap()
            .id
    );
    let wrong_owner =
        terminal_psi::PreparedNominalRoot::new("other-package::main", "main", None).unwrap();
    assert!(
        publication.entry_receipt(&wrong_owner, signature).is_err(),
        "a same-leaf root from another owner cannot join"
    );

    // Omega 00 attaches the seals to that module, and each accepted
    // operation obligation carries its certificate's fingerprint.
    let expected: Vec<[u8; 32]> = publication
        .seals
        .seals
        .iter()
        .map(|seal| seal.fingerprint)
        .collect();
    assert!(
        !expected.is_empty(),
        "the program has discharged operations"
    );
    let input = terminal_psi_to_abstract_operations::lower_module_with_evidence(
        publication.artifact.semantic_module(),
        publication.seals.clone(),
    )
    .expect("Omega 00 lowers the published module with its seals")
    .into_optimization_artifact()
    .into_optimization_input();
    let (_, unit) =
        abstract_operations_to_abstract_operations::build_verified_psi_optimization_unit(
            input,
            psi_work::WorkSchedule::CURRENT.identity(),
        )
        .expect("Omega 01 builds the verified unit")
        .into_parts();
    let sealed: Vec<[u8; 32]> = unit
        .accepted_obligation_facts
        .iter()
        .map(|fact| fact.proof_bundle_fingerprint)
        .filter(|fingerprint| *fingerprint != [0; 32])
        .collect();
    for fingerprint in &expected {
        assert!(sealed.contains(fingerprint), "{expected:?} / {sealed:?}");
    }

    // The published program runs to completion.
    let bytes = terminal_codec::encode_module(publication.artifact.semantic_module())
        .expect("the published module encodes");
    let mut execution =
        TerminalExecution::start_artifact(&bytes, &[], TerminalStructuralInputs::default())
            .expect("the interpreter loads the published module");
    let status = execution
        .resume(&mut WorkMeter::with_budget(10_000_000), &mut NoEffects)
        .expect("the published program runs");
    let expected_exit = std::fs::read_to_string(
        repo_root()
            .join("tests/fixtures/run/calls/scalar_contract_route_exit/expected_native_exit.txt"),
    )
    .unwrap();
    assert_eq!(expected_exit.trim(), "0");
    assert!(
        matches!(
            status,
            terminal_interpreter::TerminalExecutionStatus::Complete(_)
        ),
        "{status:?}"
    );
}

/// The executable product uses its own fully judged descriptor request over
/// the same source and complete roster. Its artifact and evidence must equal
/// that accepted module's publication, rather than reusing the earlier Check
/// artifact, which has no calling metadata.
#[test]
fn the_default_compile_builds_the_application_from_the_routes_module() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let fixture = "tests/fixtures/run/calls/scalar_contract_route_exit";
    let (check_publication, _, _, checked) = published_fixture(fixture);
    let (signature, _) = compiler::prepare_program_entry(&checked).unwrap();
    let accepted = checked.terminal_publication().accepted().unwrap();
    let boundary = package_compilation::selected_program_entry_boundary(
        accepted.source(),
        signature.target_slot(),
        None,
        checked.sources().exact_toolchain_sources(),
    )
    .expect("the selected target's authenticated entry declaration");
    let mut roster = checked.terminal_publication().roster().unwrap().clone();
    roster.declaration_metadata.push(boundary);
    let executable = compilation_products::ProductTerminalPublication::judge(
        std::sync::Arc::clone(accepted.source()),
        roster,
        checked.terminal_publication().optimizations().clone(),
    )
    .expect("all original scope is judged with the executable descriptors");
    let executable = executable.accepted().unwrap();
    assert!(std::sync::Arc::ptr_eq(
        accepted.source(),
        executable.source()
    ));
    let publication = executable.closure().publish().unwrap();
    assert!(
        !publication
            .artifact
            .semantic_module()
            .calling_abi_signatures
            .is_empty()
    );
    assert_ne!(
        check_publication.artifact.manifest().semantic(),
        publication.artifact.manifest().semantic()
    );
    let product = default_product(fixture);

    assert_eq!(
        product.artifact().manifest().semantic(),
        publication.artifact.manifest().semantic()
    );
    assert_eq!(product.artifact().semantic_module(), executable.module());
    let evidence = product
        .evidence()
        .expect("the route's seals travel beside its artifact");
    assert_eq!(evidence.as_ref(), &publication.seals);
    assert!(!evidence.seals.is_empty());
    assert_eq!(
        evidence.module,
        terminal_codec::terminal_psi_identity(product.artifact().semantic_module()).unwrap()
    );
}

/// The Terminal product the default compile builds for `fixture`.
fn default_product(fixture: &str) -> terminal_psi_to_abstract_operations::RetainedTerminalArtifact {
    let main = repo_root().join(fixture).join("main.omg");
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            orchestration::produce_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
                compiler::CompilationProduct::Terminal(Default::default()),
            )
            .and_then(compiler::CompilationOutput::into_report)
            .map_err(compiler::CompilationProductError::into_diagnostics)?
            .into_retained_terminal_artifact()
            .ok_or_else(|| {
                vec![diagnostics::Diagnostic::error(
                    "the Terminal product retained no artifact",
                )]
            })
        })
        .expect("compiler worker")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    })
    .unwrap_or_else(|diagnostics| panic!("{fixture} must reach Terminal Psi: {diagnostics:#?}"))
}

fn repo_root() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate is nested four levels under the repository")
}

/// Every source machine keeps its exact declaration and nonempty contract
/// commitment when the actual accepted module is published. Entry selection
/// uses those canonical identities rather than a display name.
#[test]
fn the_published_module_states_each_machines_declaration_identity() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let fixture = "tests/fixtures/run/calls/scalar_contract_route_exit";
    let (publication, _, _, checked) = published_fixture(fixture);
    let published = publication.artifact.semantic_module();
    // The compilation's accepted source is the code published for the
    // compilation's other readers.
    let current = checked
        .terminal_publication()
        .accepted()
        .expect("the compilation holds its accepted source")
        .module();

    let text = |module: &terminal_psi::TerminalModule, identity| {
        module
            .identities
            .text(identity)
            .filter(|text: &&str| !text.is_empty())
            .expect("a declaration identity is interned and not empty")
            .to_owned()
    };
    let stated = |module: &terminal_psi::TerminalModule,
                  declaration: &terminal_psi::CheckedMachineDeclaration| {
        (
            text(module, declaration.machine.identity()),
            text(module, declaration.state.identity()),
            text(module, declaration.semantic_declaration),
            declaration
                .parameter_types
                .iter()
                .map(|identity| text(module, *identity))
                .collect::<Vec<_>>(),
            text(module, declaration.callable),
            text(module, declaration.result_type),
            declaration.published_may_suspend,
            declaration.published_may_block,
            declaration.specialization.is_some(),
        )
    };
    let current_declarations: Vec<_> = current
        .machines
        .iter()
        .filter_map(|machine| machine.declaration.as_ref())
        .collect();
    assert_eq!(
        published.machines.len(),
        6,
        "the fixture declares six machines"
    );
    for machine in &published.machines {
        let declaration = machine
            .declaration
            .as_ref()
            .expect("every source-produced machine states its declaration identity");
        let ours = stated(published, declaration);
        let theirs = current_declarations
            .iter()
            .find(|candidate| stated(current, candidate).0 == ours.0)
            .unwrap_or_else(|| panic!("the current program has no machine {}", ours.0));
        assert_eq!(ours, stated(current, theirs));
        assert_ne!(declaration.contract, [0; 32]);
        assert_eq!(declaration.contract, theirs.contract);
    }

    let Some(terminal_psi::TargetMachineCallable::Machine(current_entry)) = current.entry else {
        panic!("the current program names one entry machine");
    };
    let current_entry = current
        .machine(current_entry)
        .and_then(|machine| machine.declaration.as_ref())
        .expect("the current program's entry states its declaration");
    let selected = terminal_psi::PreparedNominalRoot::new(
        &text(current, current_entry.machine.identity()),
        "main",
        None,
    )
    .expect("the entry's declaration identity is not empty");
    let entry = selected
        .resolve(published)
        .expect("the selected entry is found once on the published module");
    assert_eq!(
        Some(terminal_psi::TargetMachineCallable::Machine(entry.id)),
        published.entry
    );
    assert_eq!(entry.result, terminal_psi::TerminalMachineResult::Unit);
}
