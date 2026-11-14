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
//! that artifact and its seals stay beside it for Omega 00. Everything else
//! a product reads (provider selection, calling policies, the Build's
//! companions) still comes from the product check's program.
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use fixture_package_inputs::{
    FixtureProductCheck, check_fixture_keeping_typed_frontier, discover_fixture,
    fixture_build_snapshot, produce_fixture, repo_root, review_fixture,
};
use lowered_discharge::{RouteVerdict, judge};
use psi_work::WorkMeter;
use structure_elaboration::{Compilation, elaborate};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalStructuralInputs,
};

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

fn accepted(
    fixture: &str,
) -> (
    Box<lowered_discharge::AcceptedClosure>,
    terminal_psi::PreparedNominalRoot,
    [u8; 32],
    Box<compiler::CheckedCompilation>,
) {
    let main = repo_root().join(fixture).join("main.omg");
    let checked = std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            check_fixture_keeping_typed_frontier(&main, "linux_x86_64")
        })
        .expect("compiler worker")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    });
    let (frontier, checked) = match checked {
        FixtureProductCheck::Accepted { frontier, checked } => (frontier, checked),
        _ => panic!("{fixture}: the program must check"),
    };
    assert!(
        lowered_discharge::check_bound_callable_shapes(&frontier.typed, &frontier.bindings)
            .expect("entry shape identities resolve")
            .is_empty(),
        "publication requires the bound continuation shapes as well as body proofs"
    );
    match judge(elaborate(
        &frontier.typed,
        Compilation {
            entries: &frontier.entries,
            subjects: &[frontier.subject],
            purpose: source::DependencyScope::Product,
            selected: &frontier.selected,
            bindings: &frontier.bindings,
            supplies: &[],
        },
    )) {
        Ok(RouteVerdict::Accepted(closure)) => {
            // The entry root and its signature identity belong to the selected
            // program entry. A checked compilation carries the target and its
            // settled provider plans only, and nothing on the lowered route
            // selects the entry from the accepted module yet.
            let _: (
                Box<lowered_discharge::AcceptedClosure>,
                Box<compiler::CheckedCompilation>,
            ) = (closure, checked);
            panic!("{fixture}: no product route yet: the selected program entry has no producer")
        }
        other => panic!("{fixture}: the route must accept: {other:?}"),
    }
}

#[test]
fn an_accepted_program_is_published_from_the_routes_module_and_runs() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let (closure, root, signature, _) =
        accepted("tests/fixtures/run/calls/scalar_contract_route_exit");
    let publication = closure.publish().expect("the accepted module publishes");

    // The artifact holds exactly the module the route checked.
    assert_eq!(
        publication.artifact.semantic_module(),
        &closure.elaboration.module
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

/// The default compile builds the application of a program the route accepts
/// from the route's own module: the Terminal product holds exactly the
/// artifact `AcceptedClosure::publish` produces, not the product check's
/// program, and the evidence seals bound to that artifact's identity are
/// retained beside it, so native production hands Omega 00 the module the
/// kernel judged with the certificates that judged it.
#[test]
fn the_default_compile_builds_the_application_from_the_routes_module() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let fixture = "tests/fixtures/run/calls/scalar_contract_route_exit";
    let (closure, _, _, _) = accepted(fixture);
    let publication = closure.publish().expect("the accepted module publishes");
    let product = default_product(fixture);

    assert_eq!(
        product.artifact().manifest().semantic(),
        publication.artifact.manifest().semantic()
    );
    assert_eq!(
        product.artifact().semantic_module(),
        &closure.elaboration.module
    );
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
    let target = "linux_x86_64";
    let main = repo_root().join(fixture).join("main.omg");
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(
            scope,
            || -> Result<_, Vec<diagnostics::Diagnostic>> {
                let discovered = discover_fixture(
                    &main,
                    package_compilation::BuildExecutionInputs {
                        snapshot: fixture_build_snapshot(&main)?,
                        ..Default::default()
                    },
                )?;
                let profile = target::TargetProfile::from_omega_target_name(Some(target))
                    .map_err(|error| vec![error])?;
                let mut prepared = review_fixture(
                    discovered,
                    &[profile],
                    compiler::CompilationProduct::Terminal(Default::default()),
                )?;
                produce_fixture(&mut prepared, Some(target))?
                    .into_report()
                    .map_err(compiler::CompilationProductError::into_diagnostics)?
                    .into_retained_terminal_artifact()
                    .ok_or_else(|| {
                        vec![diagnostics::Diagnostic::error(
                            "the Terminal product retained no artifact",
                        )]
                    })
            },
        )
        .expect("compiler worker")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    })
    .unwrap_or_else(|diagnostics| panic!("{fixture} must reach Terminal Psi: {diagnostics:#?}"))
}

/// The route states, for every machine of the module it publishes, the
/// declaration identity the product check's program states for the same
/// source machine: the exact-owner machine and state, the semantic
/// declaration, the parameter and result types, the callable and the
/// published suspension and blocking. Only the contract commitment differs,
/// because each checker commits to the contract in its own form. The
/// Build's selected entry, named as the current program names it, is
/// therefore found on the published module exactly once, with a Unit
/// result, which is what the program-entry receipt requires of it.
#[test]
fn the_published_module_states_each_machines_declaration_identity() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let fixture = "tests/fixtures/run/calls/scalar_contract_route_exit";
    let (closure, _, _, checked) = accepted(fixture);
    let publication = closure.publish().expect("the accepted module publishes");
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
        assert_ne!(declaration.contract, theirs.contract);
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
