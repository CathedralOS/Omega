//! Plain owned data is judged from actual transfers on the lowered form. The
//! positive retains call-result/local identities through publication and runs;
//! the negative borrows the same local after an earlier operand consumed it.
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use fixture_package_inputs::{
    FixtureFrontierCheck, check_fixture_keeping_typed_frontier, repo_root,
};
use lowered_discharge::{RouteVerdict, judge_compilation};
use structure_elaboration::Compilation;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionStatus, TerminalStructuralInputs,
};

static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn check(fixture: &str) -> FixtureFrontierCheck {
    let main = repo_root().join(fixture).join("main.omg");
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            check_fixture_keeping_typed_frontier(&main, "linux_x86_64")
        })
        .expect("compiler worker")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    })
}

struct NoEffects;

impl TerminalEffectHandler for NoEffects {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "plain data performs no effect",
        ))
    }
}

#[test]
fn owned_call_results_and_locals_publish_and_execute_the_routes_module() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let FixtureFrontierCheck::Accepted { frontier, .. } =
        check("tests/fixtures/pass/calls/runtime_affine_record_call_results_reused_exit")
    else {
        panic!("the original owned-data witness must check");
    };
    // Match the default router's complete scope, including every generic
    // check form it claims; one undecided form refuses the whole compilation.
    let mut verdicts = judge_compilation(
        &frontier.typed,
        Compilation {
            entries: &frontier.entries,
            subjects: &[frontier.subject],
            purpose: source::DependencyScope::Product,
            selected: &frontier.selected,
            bindings: &frontier.bindings,
        },
    )
    .into_iter();
    let closure = match verdicts.next().expect("the entry closure is judged") {
        Ok(RouteVerdict::Accepted(closure)) => closure,
        other => panic!("the owned-data route must accept: {other:?}"),
    };
    for verdict in verdicts {
        assert!(
            matches!(verdict, Ok(RouteVerdict::Accepted(_))),
            "every check form of the default scope must accept: {verdict:?}"
        );
    }
    assert!(
        closure
            .elaboration
            .module
            .structural_types
            .iter()
            .any(|ty| !ty.copyable)
    );
    let publication = closure.publish().expect("owned-data publication");
    assert_eq!(
        publication.artifact.semantic_module(),
        &closure.elaboration.module
    );
    let input = terminal_psi_to_abstract_operations::lower_module_with_evidence(
        publication.artifact.semantic_module(),
        publication.seals.clone(),
    )
    .expect("Omega 00 carries the checked affine operations and their seals")
    .into_optimization_artifact()
    .into_optimization_input();
    abstract_operations_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        psi_work::WorkSchedule::CURRENT.identity(),
    )
    .expect("Omega 01 verifies the same owned homes and their cleanup");
    let bytes = terminal_codec::encode_module(publication.artifact.semantic_module())
        .expect("the owned-data module encodes");
    let mut execution =
        TerminalExecution::start_artifact(&bytes, &[], TerminalStructuralInputs::default())
            .expect("the interpreter loads the route's own module");
    let status = execution
        .resume(
            &mut psi_work::WorkMeter::with_budget(10_000_000),
            &mut NoEffects,
        )
        .expect("the route's owned values execute");
    assert!(
        matches!(status, TerminalExecutionStatus::Complete(_)),
        "{status:?}"
    );
}

#[test]
fn borrowing_after_an_earlier_operand_moves_the_local_is_rejected() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let FixtureFrontierCheck::Rejected { frontier, .. } =
        check("tests/fixtures/fail/borrows/owned_address_after_operand_move")
    else {
        panic!("the move-before-borrow witness must reject after the typed frontier");
    };
    let verdicts = judge_compilation(
        &frontier.typed,
        Compilation {
            entries: &frontier.entries,
            subjects: &[frontier.subject],
            purpose: source::DependencyScope::Product,
            selected: &frontier.selected,
            bindings: &frontier.bindings,
        },
    );
    let mut unestablished = Vec::new();
    for verdict in verdicts {
        match verdict {
            Ok(RouteVerdict::Accepted(_)) => {}
            Ok(RouteVerdict::Rejected {
                unestablished: uses,
                ..
            }) => unestablished.extend(uses),
            other => panic!("no check form may make the default scope undecided: {other:?}"),
        }
    }
    let expected = std::fs::read_to_string(
        repo_root()
            .join("tests/fixtures/fail/borrows/owned_address_after_operand_move/expected.txt"),
    )
    .unwrap();
    assert!(
        unestablished
            .iter()
            .any(|entry| entry.message().contains(expected.trim())),
        "the ownership rejection must retain its pinned transferred/consumed diagnostic: {unestablished:?}"
    );
}

/// Exercise the same whole transfers without repeating source acquisition: a
/// factory result enters a named home, moves to a call argument, and is consumed.
#[test]
fn whole_owned_transfers_consume_their_homes_once() {
    use semantic_vocabulary::{
        IdTable, IntegerSign, IntegerType, IntegerValue, PsiSemanticId, ScalarType,
    };
    use structure_elaboration::{
        Body, CrashCeiling, Expression, ExpressionKind, RecordField, RecordType, Signature,
        SourceOrigin, Statement, Target, Termination, Unpositioned, ValueType,
    };
    fn id<T: PsiSemanticId>(raw: u64) -> T {
        T::new(raw).unwrap()
    }
    let source = SourceOrigin::Unpositioned(Unpositioned::SpanAbsent);
    let record = ValueType::Record(id(1));
    let scalar = ValueType::Scalar(ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
    ));
    let expression = |kind, result| Expression {
        kind,
        result,
        source,
    };
    let invoke = |callee, arguments, result| {
        expression(
            ExpressionKind::Call {
                callee: semantic_vocabulary::TargetMachineCallable::Machine(id(callee)),
                arguments,
                designated: None,
            },
            result,
        )
    };
    let finish = |target| Statement::Arm {
        guard: None,
        target,
        source,
    };
    let mut records = IdTable::default();
    records.push(RecordType {
        copyable: false,
        identity: "Owned".into(),
        shape: structure_elaboration::StorageShape::Record(vec![RecordField {
            id: id(1),
            identity: "number".into(),
            value_type: scalar,
        }]),
        requires: Vec::new(),
    });
    let mut bodies = IdTable::default();
    bodies.push(Body {
        slots: vec![record],
        statements: vec![
            Statement::Bind {
                slot: 0,
                value: invoke(2, Vec::new(), Some(record)),
            },
            Statement::Evaluate(invoke(
                3,
                vec![expression(ExpressionKind::Slot(0), Some(record))],
                None,
            )),
            finish(Target::Unit),
        ],
    });
    bodies.push(Body {
        slots: Vec::new(),
        statements: vec![finish(Target::Value(expression(
            ExpressionKind::Record {
                fields: vec![(
                    0,
                    expression(
                        ExpressionKind::Integer(IntegerValue::Unsigned(7)),
                        Some(scalar),
                    ),
                )],
            },
            Some(record),
        )))],
    });
    bodies.push(Body {
        slots: vec![record],
        statements: vec![finish(Target::Unit)],
    });
    let mut signatures = IdTable::default();
    for (parameters, result) in [
        (Vec::new(), None),
        (Vec::new(), Some(record)),
        (vec![record], None),
    ] {
        signatures.push(Signature {
            parameters,
            erased: Vec::new(),
            borrowed: Vec::new(),
            result,
            requires: Vec::new(),
            ensures: Vec::new(),
            reaches: structure_elaboration::Ceiling::Inferred,
            invokes: structure_elaboration::Ceiling::Inferred,
            crashes: CrashCeiling::Authored(Vec::new()),
            termination: Termination::Silent,
        });
    }
    let closure = match lowered_discharge::judge(structure_elaboration::lower_bodies(
        id(1),
        &bodies,
        &signatures,
        &records,
    )) {
        Ok(RouteVerdict::Accepted(closure)) => closure,
        other => panic!("whole transfers must check: {other:?}"),
    };
    let module = &closure.elaboration.module;
    assert!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| block.terminator.cleanup_rosters())
            .flat_map(|(_, actions)| actions)
            .all(|action| !matches!(
                action,
                terminal_psi::TerminalAffineCleanupAction::RetireHome(_)
            )),
        "a whole transfer has already consumed its source home"
    );
    let execute = |module: &terminal_psi::TerminalModule| {
        let bytes = terminal_codec::encode_module(module).expect("the focused module encodes");
        let mut execution =
            TerminalExecution::start_artifact(&bytes, &[], TerminalStructuralInputs::default())
                .expect("the focused module loads");
        execution.resume(
            &mut psi_work::WorkMeter::with_budget(1_000_000),
            &mut NoEffects,
        )
    };
    let status =
        execute(module).expect("whole owned transports execute without a second retirement");
    assert!(
        matches!(status, TerminalExecutionStatus::Complete(_)),
        "{status:?}"
    );
    let mut duplicate = module.clone();
    let block = duplicate.machines[0]
        .blocks
        .iter_mut()
        .find(|block| {
            block.operations.iter().any(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::StoreStructuralField { field: None, .. }
                )
            })
        })
        .unwrap();
    let source = block
        .operations
        .iter()
        .find_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::StoreStructuralField {
                field: None, value, ..
            } => Some(value.place),
            _ => None,
        })
        .unwrap();
    let terminal_psi::Terminator::Jump {
        cleanup_actions, ..
    } = &mut block.terminator
    else {
        panic!("initialization has a normal continuation")
    };
    cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::RetireHome(
        source,
    ));
    assert!(
        matches!(
            execute(&duplicate),
            Err(terminal_interpreter::TerminalInterpretError::ExecutionSite { cause, .. })
                if matches!(*cause, terminal_interpreter::TerminalInterpretError::VerifiedStructuralPlaceMissing(place) if place == source)
        ),
        "an extra retirement of an already consumed home must still reject"
    );
}
