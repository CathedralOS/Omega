//! Publication must execute the program the lowered-form checker accepted.
//! These tests join Build's selected entry to that program, carry its proof
//! certificates into native lowering, and run its authored Psi result. A root
//! with the same name from another owner must refuse.
//!
//! Each machine of the published module states the declaration identity of
//! the source machine it lowers, so the Build's selected entry is found on it
//! the way the program-entry receipt finds it on the product check's
//! program.
//!
//! The default compile takes this path: when the checker accepts a compilation
//! and its module publishes, the Terminal product is that artifact and its
//! seals stay beside it for native planning. The shared source roster includes
//! the Build-selected entry declaration before publication.
#[path = "../../../../tests/support/fixture_repository.rs"]
mod fixture_repository;

#[path = "support/artifact_trust.rs"]
mod artifact_trust;

use verified_optimization_input::RetainedTerminalArtifact;
use source_compiler::checking::RoutePublication;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalStructuralInputs,
};
use terminal_work::WorkMeter;

// The lock serializes access to process-wide counters. It stores no fixture
// state, so each test can still report its own result after an earlier panic.
fn serialize_fixture_compilation() -> std::sync::MutexGuard<'static, ()> {
    static COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());
    COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn an_indexed_discriminator_keeps_its_pre_dispatch_evidence() {
    use optimization_unit::{AcceptedObligationFact, AcceptedObligationOwner};

    let _compilation = serialize_fixture_compilation();
    let product = default_product("tests/fixtures/run/calls/indexed_case_discriminator");
    let proposal = product
        .native_realization_proposal()
        .expect("the selected application retains its native proposal");
    let seals = product
        .evidence()
        .expect("the checker sealed the indexed read");
    let execution = proposal
        .prepare_native_execution(product.authorization(), &[])
        .unwrap()
        .with_operation_evidence(product.artifact(), Some(seals.clone()))
        .unwrap();
    let (input, _) = artifact_lowering::lower_native_execution(proposal, execution, &[], None)
        .expect("lower the checked entry and its selected callees with their evidence");
    let (_, unit) = abstract_optimization::build_verified_psi_optimization_unit(
        input.into_optimization_input(),
        terminal_work::WorkSchedule::CURRENT.identity(),
    )
    .unwrap()
    .into_parts();
    let frontiers =
        optimization_unit_semantics::reconstruct_ownership_frontier_facts_with_admitted_cycle_machines(
            &unit, &[],
        )
        .unwrap();
    let unit = optimization_unit::attach_ownership_frontier_facts(unit, frontiers).unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit_with_admitted_cycle_machines(
        &unit,
        &[],
    )
    .expect("the source's indexed dispatch has complete receiving evidence");
    let (position, fact) = unit
        .accepted_obligation_facts
        .iter()
        .enumerate()
        .find(|(_, fact)| matches!(fact.owner, AcceptedObligationOwner::Terminator(_)))
        .expect("the discriminator's bound owns the pre-branch read");
    assert_ne!(fact.proof_bundle_fingerprint, [0; 32]);
    let AcceptedObligationOwner::Terminator(block) = fact.owner else {
        unreachable!()
    };
    let seal = seals
        .seals
        .iter()
        .find(|seal| seal.machine == fact.machine && seal.obligation == fact.obligation)
        .unwrap();
    assert_eq!(fact.proof_bundle_fingerprint, seal.fingerprint);
    for wrong_owner in [
        AcceptedObligationOwner::Edge(semantic_vocabulary::EdgeId::new(block.get()).unwrap()),
        AcceptedObligationOwner::from(semantic_vocabulary::OperationId::new(block.get()).unwrap()),
    ] {
        let mut changed = unit.clone();
        changed.accepted_obligation_facts[position] = AcceptedObligationFact::new(
            fact.psi,
            fact.proof_bundle_fingerprint,
            fact.machine,
            wrong_owner,
            fact.obligation,
            fact.proposition.clone(),
        );
        changed
            .accepted_obligation_facts
            .sort_by_key(|fact| (fact.machine, fact.owner, fact.obligation));
        changed.identity = optimization_unit::recompute_psi_optimization_unit_identity(&changed);
        assert!(matches!(
            optimization_unit_semantics::validate_psi_optimization_unit_with_admitted_cycle_machines(&changed, &[]),
            Err(optimization_unit_semantics::OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)
        ), "a numerically equal operation or chosen edge cannot own the discriminator's read");
    }

    let bytes = terminal_codec::encode_module(product.artifact().semantic_module()).unwrap();
    let mut execution =
        TerminalExecution::start_artifact(&bytes, &[], TerminalStructuralInputs::default())
            .unwrap();
    assert!(matches!(
        execution
            .resume(&mut WorkMeter::with_budget(10_000_000), &mut NoEffects)
            .unwrap(),
        terminal_interpreter::TerminalExecutionStatus::Complete(_)
    ));

    let fixture = repo_root().join("tests/fixtures/fail/calls/indexed_case_discriminator");
    let main = fixture.join("main.omg");
    let diagnostics = std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            )
            .result
            .expect_err("a bound of three cannot authorize reading a two-element array")
            .into_diagnostics()
        })
        .expect("compiler worker")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    });
    let expected = std::fs::read_to_string(fixture.join("expected.txt")).unwrap();
    assert!(
        format!("{diagnostics:?}").contains(expected.trim()),
        "the bound-only twin must refuse the match's storage obligation: {diagnostics:?}",
    );
}

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
    let (signature, application) =
        compiler::prepare_program_entry(&checked.clone().into_published().unwrap())
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
    let _compilation = serialize_fixture_compilation();
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

    // Native planning joins the seals to this module. Each accepted
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
    let input = artifact_lowering::test_support::lower_artifact(
        &artifact_trust::authorize(&publication.artifact),
        Some(publication.seals.clone()),
    )
    .expect("native planning lowers the published module with its seals");
    let (_, unit) = abstract_optimization::build_verified_psi_optimization_unit(
        input,
        terminal_work::WorkSchedule::CURRENT.identity(),
    )
    .expect("native planning constructs the verified optimization unit")
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

/// The executable product carries the calling metadata already requested by
/// the shared source roster. Rejudging that roster reproduces its artifact
/// and its operation evidence.
#[test]
fn the_default_compile_builds_the_application_from_the_routes_module() {
    let _compilation = serialize_fixture_compilation();
    let fixture = "tests/fixtures/run/calls/scalar_contract_route_exit";
    let (check_publication, _, _, checked) = published_fixture(fixture);
    let (signature, _) =
        compiler::prepare_program_entry(&checked.clone().into_published().unwrap()).unwrap();
    let accepted = checked.terminal_publication().accepted().unwrap();
    let catalog = checked
        .source_inspection()
        .inventory()
        .services_for(accepted.source().as_ref())
        .expect("the accepted source's retained Product service catalog");
    let boundary = source_compiler::entry_contracts::selected_program_entry_boundary(
        catalog,
        &package_compilation::source_imports::source_target(signature.target_slot().owner, None),
    )
    .expect("the selected target's authenticated entry declaration");
    let roster = checked.terminal_publication().roster().unwrap().clone();
    assert!(roster.declaration_metadata.contains(&boundary));
    let executable = source_compiler::SourcePublication::judge(
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
    assert_eq!(
        check_publication.artifact.manifest().semantic(),
        publication.artifact.manifest().semantic(),
        "the same frozen source and full roster produce the same artifact"
    );
    assert_eq!(check_publication.seals, publication.seals);
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
fn default_product(fixture: &str) -> RetainedTerminalArtifact {
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
    fixture_repository::root()
}

#[test]
fn empty_payload_coverage_survives_the_actual_product_and_interpreter() {
    let _compilation = serialize_fixture_compilation();
    let product = default_product("tests/fixtures/run/calls/uninhabited_payload_coverage");
    let module = product.artifact().semantic_module();
    assert!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .any(|block| {
                matches!(
                    block.terminator,
                    terminal_psi::Terminator::EliminateEmpty { .. }
                )
            })
    );
    let bytes = terminal_codec::encode_module(module).expect("the actual product encodes");
    let mut execution =
        TerminalExecution::start_artifact(&bytes, &[], TerminalStructuralInputs::default())
            .expect("the actual product is admitted by the interpreter");
    let status = execution
        .resume(&mut WorkMeter::with_budget(10_000_000), &mut NoEffects)
        .expect("both owned and borrowed Ok observations execute");
    assert!(matches!(
        status,
        terminal_interpreter::TerminalExecutionStatus::Complete(_)
    ));

    let main =
        repo_root().join("tests/fixtures/fail/calls/inhabited_payload_missing_case/main.omg");
    let diagnostics = std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            )
            .result
            .expect_err("an inhabited Err payload still needs its own arm")
            .into_diagnostics()
        })
        .expect("compiler worker")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    });
    assert!(
        format!("{diagnostics:?}").contains("a dispatch selects no arm for a declared case"),
        "the payload argument alone removes exclusion evidence: {diagnostics:?}",
    );
}

/// Every source machine keeps its exact declaration and nonempty contract
/// commitment when the actual accepted module is published. Entry selection
/// uses those canonical identities rather than a display name.
#[test]
fn the_published_module_states_each_machines_declaration_identity() {
    let _compilation = serialize_fixture_compilation();
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
    let source = checked.source_inspection().program();
    let main = repo_root()
        .join(fixture)
        .join("main.omg")
        .canonicalize()
        .unwrap();
    let mut authored = Vec::new();
    for machine in source.machines() {
        let file = source
            .symbols
            .symbol_source_span(machine.symbol)
            .and_then(|span| source.symbols.source_file(span));
        if file.is_some_and(|file| file.path == main) {
            let identity = source
                .exact_owner_nominal_identity(machine.symbol, &[])
                .and_then(|id| source.type_identity_text(id))
                .expect("each authored machine has its exact package declaration");
            let root =
                terminal_psi::PreparedNominalRoot::new(&identity, machine.name.as_str(), None)
                    .unwrap();
            root.resolve(published)
                .expect("the published module contains this exact declaration once");
            authored.push(machine.name.as_str());
        }
    }
    authored.sort_unstable();
    assert_eq!(
        authored,
        [
            "main",
            "report_large",
            "report_small",
            "scaled",
            "settle",
            "within"
        ]
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

#[cfg(test)]
fn canonical_fixture(
    module: &terminal_psi::TerminalModule,
) -> terminal_codec::CanonicalTerminalArtifact {
    let record = terminal_codec::build_identity_optimization_execution_record(module).unwrap();
    terminal_codec::CanonicalTerminalArtifact::from_parts(module.clone(), record, None).unwrap()
}

#[cfg(test)]
fn authorize_fixture(
    artifact: terminal_codec::CanonicalTerminalArtifact,
) -> psi_authorization::AuthorizedTerminalArtifact {
    artifact_trust::authorize(&artifact)
}

#[cfg(test)]
mod evidence_seal_tests {
    // A scalar program from the lowered-form route, discharged with kernel
    // evidence, reaches this stage with its seals in memory: the accepted
    // fact carries the seal's fingerprint, a module without seals keeps the
    // zero fingerprint, and a seal that does not match is refused.
    use arena::IdTable;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId, Proposition, ScalarTerm,
        ScalarType,
    };
    use source_compiler::lowering::{
        Body, CrashCeiling, Expression, ExpressionKind, Signature, SourceOrigin, Statement, Strict,
        Target, Termination, Unpositioned,
    };
    use terminal_codec::{EvidenceSeals, ObligationEvidenceSeal};

    use abstract_optimization::{
        VerifiedPsiOptimizationUnitBuildError, build_verified_psi_optimization_unit,
    };
    use artifact_lowering::test_support::lower_artifact;
    use verified_optimization_input::{ObligationEvidence, VerifiedPsiOptimizationInput};

    fn u32_type() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap())
    }

    fn slot(slot: usize) -> Expression {
        Expression {
            call_context: None,
            kind: ExpressionKind::Slot(slot),
            result: Some(u32_type().into()),
            source: SourceOrigin::Unpositioned(Unpositioned::SpanAbsent),
        }
    }

    fn at_most(position: u64) -> Proposition {
        let ScalarType::Integer(integer) = u32_type() else {
            unreachable!()
        };
        Proposition::LessOrEqual(
            ScalarTerm::value(
                semantic_vocabulary::ValueId::new(position).unwrap(),
                u32_type(),
            ),
            ScalarTerm::integer(integer, IntegerValue::Unsigned(100)).unwrap(),
        )
    }

    /// `add(a, b) requires a <= 100, b <= 100 { return a + b }`, lowered,
    /// discharged and sealed.
    fn sealed_add() -> (terminal_psi::TerminalModule, Vec<ObligationEvidenceSeal>) {
        let sum = Expression {
            call_context: None,
            kind: ExpressionKind::Strict {
                operation: Strict::Add,
                policy: None,
                left: Box::new(slot(0)),
                right: Box::new(slot(1)),
            },
            result: Some(u32_type().into()),
            source: SourceOrigin::Unpositioned(Unpositioned::SpanAbsent),
        };
        let mut bodies: IdTable<MachineId, Body> = IdTable::default();
        bodies.push(Body {
            slots: vec![u32_type().into(), u32_type().into()],
            statements: vec![Statement::Arm {
                guard: None,
                target: Target::Value(sum),
                source: SourceOrigin::Unpositioned(Unpositioned::SpanAbsent),
            }],
        });
        let mut signatures: IdTable<MachineId, Signature> = IdTable::default();
        signatures.push(Signature {
            membership_ensures: Vec::new(),
            operational: Default::default(),
            parameters: vec![u32_type().into(), u32_type().into()],
            erased: Vec::new(),
            borrowed: Vec::new(),
            result: Some(u32_type().into()),
            requires: vec![at_most(1), at_most(2)],
            ensures: Vec::new(),
            reaches: source_compiler::lowering::Ceiling::Inferred,
            invokes: source_compiler::lowering::Ceiling::Inferred,
            crashes: CrashCeiling::Authored(Vec::new()),
            termination: Termination::Silent,
            ranking: None,
        });
        let entry = MachineId::new(1).unwrap();
        let mut elaboration = source_compiler::lowering::lower_bodies(
            entry,
            &bodies,
            &signatures,
            &IdTable::default(),
        )
        .unwrap();
        let mut activity = source_compiler::checking::ProofSearchActivity::default();
        let discharge =
            source_compiler::checking::discharge(&mut elaboration, &mut activity).unwrap();
        let seals = discharge.operation_seals(&elaboration).unwrap();
        assert_eq!(seals.len(), 1, "the add's safety obligation is discharged");
        (elaboration.module, seals)
    }

    /// The receiving test host trusts its controlled producer; seals remain a
    /// separate optional input, not the authority to lower the artifact.
    fn build(
        module: &terminal_psi::TerminalModule,
        seals: Vec<ObligationEvidenceSeal>,
    ) -> Result<Vec<[u8; 32]>, VerifiedPsiOptimizationUnitBuildError> {
        let evidence = EvidenceSeals::for_module(module, seals).unwrap();
        let authorized = super::authorize_fixture(super::canonical_fixture(module));
        let input = lower_artifact(&authorized, Some(evidence))
            .unwrap();
        fingerprints(input)
    }

    /// Lowers `module` from its bytes, as a stored artifact or a cache read is.
    fn build_from_bytes(
        module: &terminal_psi::TerminalModule,
    ) -> Result<Vec<[u8; 32]>, VerifiedPsiOptimizationUnitBuildError> {
        let bytes = super::canonical_fixture(module).to_bytes();
        let decoded = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).unwrap();
        let authorized = super::authorize_fixture(decoded);
        let input = lower_artifact(&authorized, None)
            .unwrap();
        assert_eq!(input.evidence(), &ObligationEvidence::NoSeals);
        fingerprints(input)
    }

    fn fingerprints(
        input: VerifiedPsiOptimizationInput,
    ) -> Result<Vec<[u8; 32]>, VerifiedPsiOptimizationUnitBuildError> {
        let (_, unit) = build_verified_psi_optimization_unit(
            input,
            terminal_work::WorkSchedule::CURRENT.identity(),
        )?
        .into_parts();
        Ok(unit
            .accepted_obligation_facts
            .iter()
            .map(|fact| fact.proof_bundle_fingerprint)
            .collect())
    }

    #[test]
    fn a_checked_seal_fills_the_fingerprint_and_no_seal_keeps_zero() {
        let (module, seals) = sealed_add();
        let expected = seals[0].fingerprint;
        assert_eq!(build(&module, seals).unwrap(), vec![expected]);
        assert_eq!(build(&module, Vec::new()).unwrap(), vec![[0; 32]]);
    }

    #[test]
    fn a_trusted_artifact_read_back_from_bytes_has_no_seals_and_zero_fingerprints() {
        // A fresh receiving-host decision authorizes the same artifact after
        // its byte round trip. Optional seals are absent, not required for trust.
        let (module, _seals) = sealed_add();
        assert_eq!(build_from_bytes(&module).unwrap(), vec![[0; 32]]);
    }

    #[test]
    fn identical_checked_bytes_cannot_supply_their_own_native_authorization() {
        let (module, _seals) = sealed_add();
        let original = super::canonical_fixture(&module);
        let decoded =
            terminal_codec::CanonicalTerminalArtifact::from_bytes(&original.to_bytes()).unwrap();
        assert_eq!(
            original.manifest().identity(),
            decoded.manifest().identity()
        );
        let result =
            psi_authorization::authorize_terminal_artifact(std::sync::Arc::new(decoded), None)
                .map(|authorized| lower_artifact(&authorized, None));
        assert!(matches!(
            result,
            Err(psi_authorization::ArtifactTrustError::ProducerTrustRequired)
        ));
    }

    #[test]
    fn seals_for_another_module_are_refused() {
        // Seals from an earlier build name that build's module identity; a
        // changed module (here a different `requires` bound) does not accept
        // them.
        let (module, seals) = sealed_add();
        let evidence = EvidenceSeals::for_module(&module, seals).unwrap();
        let mut changed = module.clone();
        changed.machines[0].contract.requires.pop();
        let authorized = super::authorize_fixture(super::canonical_fixture(&changed));
        let refused = lower_artifact(&authorized, Some(evidence))
            .expect_err("seals for another module are refused");
        assert_eq!(format!("{refused:?}"), "EvidenceForAnotherModule");
    }

    #[test]
    fn a_seal_that_does_not_match_is_refused() {
        let (module, seals) = sealed_add();
        // Changed after sealing: the fingerprint no longer matches its parts.
        let mut altered = seals.clone();
        altered[0].goal = Proposition::Truth;
        assert!(matches!(
            build(&module, altered),
            Err(VerifiedPsiOptimizationUnitBuildError::EvidenceSealMismatch { .. })
        ));
        // Sealed intact, but for a goal the module does not state.
        let original = &seals[0];
        let resealed = ObligationEvidenceSeal::seal(
            original.machine,
            original.obligation,
            Proposition::Truth,
            original.assumptions.clone(),
            original.axioms.clone(),
            original.certificate.clone(),
        )
        .unwrap();
        assert!(matches!(
            build(&module, vec![resealed]),
            Err(VerifiedPsiOptimizationUnitBuildError::EvidenceSealMismatch { .. })
        ));
        // A seal for an obligation no accepted fact owns.
        let mut stray = seals[0].clone();
        stray.obligation = ObligationId::new(99).unwrap();
        let stray = ObligationEvidenceSeal::seal(
            stray.machine,
            stray.obligation,
            stray.goal,
            stray.assumptions,
            stray.axioms,
            stray.certificate,
        )
        .unwrap();
        assert!(matches!(
            build(&module, vec![stray]),
            Err(VerifiedPsiOptimizationUnitBuildError::UnattachedEvidenceSeal { .. })
        ));
    }
}
