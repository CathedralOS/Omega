//! Plain owned data is judged from actual transfers on the lowered form. The
//! positive retains call-result/local identities through publication and runs;
//! the negative borrows the same local after an earlier operand consumed it.
//! Each program is typed without running its Build
//! (`support/unbuilt_frontier.rs`).
#[path = "support/unbuilt_frontier.rs"]
mod unbuilt_frontier;

use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionStatus, TerminalStructuralInputs,
};
use terminal_psi_to_terminal_psi::RouteVerdict;
use typed_trees_to_terminal_psi::Compilation;
use unbuilt_frontier::{fixture_frontier, repository};

static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let frontier = fixture_frontier(
        "tests/fixtures/pass/calls/runtime_affine_record_call_results_reused_exit",
    );
    // Match the default router's complete scope, including every generic
    // check form it claims; one undecided form refuses the whole compilation.
    let mut verdicts = terminal_psi_to_terminal_psi::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &frontier.typed,
            Compilation {
                entries: &frontier.entries,
                subjects: &[frontier.subject],
                purpose: source::DependencyScope::Product,
                selected: &frontier.selected,
                bindings: &frontier.bindings,
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Portable,
        )
        .program,
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
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let frontier = fixture_frontier("tests/fixtures/fail/borrows/owned_address_after_operand_move");
    let verdicts = terminal_psi_to_terminal_psi::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &frontier.typed,
            Compilation {
                entries: &frontier.entries,
                subjects: &[frontier.subject],
                purpose: source::DependencyScope::Product,
                selected: &frontier.selected,
                bindings: &frontier.bindings,
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Portable,
        )
        .program,
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
    let expected = std::fs::read_to_string(repository(
        "tests/fixtures/fail/borrows/owned_address_after_operand_move/expected.txt",
    ))
    .unwrap();
    assert!(
        unestablished
            .iter()
            .any(|entry| entry.message().contains(expected.trim())),
        "the ownership rejection must retain its pinned transferred/consumed diagnostic: {unestablished:?}"
    );
}

fn execute_refilled_owner(closure: &terminal_psi_to_terminal_psi::AcceptedClosure) {
    let publication = closure.publish().expect("refilled ownership publishes");
    assert!(
        publication
            .artifact
            .semantic_module()
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .any(|block| matches!(
                block.terminator,
                terminal_psi::Terminator::Conditional { .. }
            )),
        "the interpreted module retains its exact-value execution oracle"
    );
    let input = terminal_psi_to_abstract_operations::lower_module_with_evidence(
        publication.artifact.semantic_module(),
        publication.seals.clone(),
    )
    .expect("Omega 00 carries the exact restoration")
    .into_optimization_artifact()
    .into_optimization_input();
    let (_, unit) =
        abstract_operations_to_abstract_operations::build_verified_psi_optimization_unit(
            input,
            psi_work::WorkSchedule::CURRENT.identity(),
        )
        .expect("Omega 01 verifies the restored owned home")
        .into_parts();
    let frontiers = optimization_unit_semantics::reconstruct_ownership_frontier_facts_with_admitted_cycle_machines(&unit, &[])
        .expect("the independent current ownership walk reconstructs the restored owner");
    let unit = optimization_unit::attach_ownership_frontier_facts(unit, frontiers)
        .expect("the exact reconstructed owner facts attach without substitution");
    optimization_unit_semantics::validate_psi_optimization_unit_with_admitted_cycle_machines(
        &unit,
        &[],
    )
    .expect("the retained owner facts preserve the actual edge cleanup transitions");
    let bytes = terminal_codec::encode_module(publication.artifact.semantic_module())
        .expect("restored module encodes");
    let mut execution =
        TerminalExecution::start_artifact(&bytes, &[], TerminalStructuralInputs::default())
            .expect("restored module loads");
    let status = execution
        .resume(
            &mut psi_work::WorkMeter::with_budget(1_000_000),
            &mut NoEffects,
        )
        .expect("the old moved value and restored owner execute independently");
    assert!(
        matches!(status, TerminalExecutionStatus::Complete(_)),
        "{status:?}"
    );
}

#[test]
fn refilling_the_moved_field_restores_its_owner_but_writing_a_sibling_does_not() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let source = r#"
data Item { value: u64; }
data Pair { first: Item; second: Item; }
machine main() crashes Trap {
    let mut pair: Pair = Pair {
        first: Item { value: 3 }, second: Item { value: 5 }
    };
    let previous: Item = pair.first;
    pair.first = Item { value: 7 };
    let restored: Pair = pair;
    let total: u64 = restored.first.value + restored.second.value + previous.value;
    transition total == 15 {
        true -> done()
        false -> bad()
    }
    machine done() {}
    machine bad() { crash Trap; }
}
"#;
    for wrong_sibling in [false, true] {
        let source = if wrong_sibling {
            source.replace("pair.first =", "pair.second =")
        } else {
            source.to_owned()
        };
        let frontier = unbuilt_frontier::staged_frontier(
            if wrong_sibling {
                "refilled-sibling"
            } else {
                "refilled-field"
            },
            &source,
        );
        let verdicts = terminal_psi_to_terminal_psi::judge_compilation(
            typed_trees_to_terminal_psi::prepare_compilation(
                &frontier.typed,
                Compilation {
                    entries: &frontier.entries,
                    subjects: &[frontier.subject],
                    purpose: source::DependencyScope::Product,
                    selected: &frontier.selected,
                    bindings: &frontier.bindings,
                    supplies: &[],
                },
                terminal_codec::TerminalPublication::Portable,
            )
            .program,
        );
        if wrong_sibling {
            assert!(
                verdicts.iter().any(|verdict| matches!(
                    verdict,
                    Ok(RouteVerdict::Rejected { unestablished, .. })
                        if unestablished.iter().any(|used| used.detail
                            == "owned storage is used after its value was transferred or consumed")
                )),
                "writing the sibling cannot restore the missing field: {verdicts:?}"
            );
            continue;
        }
        let mut verdicts = verdicts.into_iter();
        let closure = match verdicts.next().expect("entry closure") {
            Ok(RouteVerdict::Accepted(closure)) => closure,
            other => panic!("the exact refilled owner must be usable: {other:?}"),
        };
        for verdict in verdicts {
            assert!(
                matches!(verdict, Ok(RouteVerdict::Accepted(_))),
                "{verdict:?}"
            );
        }
        execute_refilled_owner(&closure);
    }
}

#[test]
fn a_whole_subrecord_can_be_refilled_after_all_its_owned_children_move() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let source = r#"
data Item { value: u64; }
data Pair { first: Item; second: Item; }
data Outer { selected: Pair; other: Pair; }
machine main() crashes Trap {
    let mut outer: Outer = Outer {
        selected: Pair { first: Item { value: 3 }, second: Item { value: 5 } },
        other: Pair { first: Item { value: 13 }, second: Item { value: 17 } }
    };
    let first: Item = outer.selected.first;
    let second: Item = outer.selected.second;
    outer.selected = Pair { first: Item { value: 7 }, second: Item { value: 11 } };
    let restored: Outer = outer;
    let total: u64 = first.value + second.value
        + restored.selected.first.value + restored.selected.second.value
        + restored.other.first.value + restored.other.second.value;
    transition total == 56 {
        true -> done()
        false -> bad()
    }
    machine done() {}
    machine bad() { crash Trap; }
}
"#;
    for wrong_region in [false, true] {
        let source = if wrong_region {
            source.replace("outer.selected =", "outer.other =")
        } else {
            source.to_owned()
        };
        let frontier = unbuilt_frontier::staged_frontier(
            if wrong_region {
                "subrecord-wrong-region"
            } else {
                "subrecord-refill"
            },
            &source,
        );
        let verdicts = terminal_psi_to_terminal_psi::judge_compilation(
            typed_trees_to_terminal_psi::prepare_compilation(
                &frontier.typed,
                Compilation {
                    entries: &frontier.entries,
                    subjects: &[frontier.subject],
                    purpose: source::DependencyScope::Product,
                    selected: &frontier.selected,
                    bindings: &frontier.bindings,
                    supplies: &[], // This source-only check runs no Build provider selections.
                },
                terminal_codec::TerminalPublication::Portable,
            )
            .program,
        );
        if wrong_region {
            assert!(verdicts.iter().any(|verdict| matches!(verdict,
                Ok(RouteVerdict::Rejected { unestablished, .. })
                    if unestablished.iter().any(|used| used.detail == "owned storage is used after its value was transferred or consumed")
            )), "a sibling store cannot fill the moved subrecord: {verdicts:?}");
            continue;
        }
        let mut verdicts = verdicts.into_iter();
        let closure = match verdicts.next().expect("entry closure") {
            Ok(RouteVerdict::Accepted(closure)) => closure,
            other => panic!("all moved children leave the exact subrecord vacant: {other:?}"),
        };
        for verdict in verdicts {
            assert!(
                matches!(verdict, Ok(RouteVerdict::Accepted(_))),
                "{verdict:?}"
            );
        }
        execute_refilled_owner(&closure);
    }
}

#[test]
fn partial_and_whole_branch_transfers_rejoin_before_the_subrecord_is_refilled() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let source = r#"
data Choice [copy] { case Part; case Whole; }
data Item { value: u32; }
data Pair { first: Item; second: Item; }
data Outer { selected: Pair; other: Pair; }
machine take_item(item: Item) -> u32 { item.value }
machine take_pair(pair: Pair) -> u32 { pair.second.value }
machine restored(choice: Choice) -> u64 {
    let mut outer: Outer = Outer {
        selected: Pair { first: Item { value: 3 }, second: Item { value: 5 } },
        other: Pair { first: Item { value: 13 }, second: Item { value: 17 } }
    };
    let consumed: u32 = match choice {
        Choice::Part -> take_item(outer.selected.first)
        Choice::Whole -> take_pair(outer.selected)
    };
    outer.selected = Pair { first: Item { value: 7 }, second: Item { value: 11 } };
    let owner: Outer = outer;
    (consumed as u64)
        + (owner.selected.first.value as u64) + (owner.selected.second.value as u64)
        + (owner.other.first.value as u64) + (owner.other.second.value as u64)
}
machine main() crashes Trap {
    let partial: u64 = restored(Choice::Part {});
    let whole: u64 = restored(Choice::Whole {});
    transition partial == 51 && whole == 53 {
        true -> done()
        false -> bad()
    }
    machine done() {}
    machine bad() { crash Trap; }
}
"#;
    for (reversed, wrong_region) in [(false, false), (true, false), (false, true)] {
        let mut source = source.to_owned();
        if reversed {
            source = source.replace("case Part; case Whole;", "case Whole; case Part;")
                .replace(
                    "Choice::Part -> take_item(outer.selected.first)\n        Choice::Whole -> take_pair(outer.selected)",
                    "Choice::Whole -> take_pair(outer.selected)\n        Choice::Part -> take_item(outer.selected.first)",
                );
        }
        if wrong_region {
            source = source.replace("outer.selected =", "outer.other =");
        }
        let frontier = unbuilt_frontier::staged_frontier(
            if wrong_region {
                "join-wrong-region"
            } else if reversed {
                "join-refill-reversed"
            } else {
                "join-refill"
            },
            &source,
        );
        let verdicts = terminal_psi_to_terminal_psi::judge_compilation(
            typed_trees_to_terminal_psi::prepare_compilation(
                &frontier.typed,
                Compilation {
                    entries: &frontier.entries,
                    subjects: &[frontier.subject],
                    purpose: source::DependencyScope::Product,
                    selected: &frontier.selected,
                    bindings: &frontier.bindings,
                    supplies: &[],
                },
                terminal_codec::TerminalPublication::Portable,
            )
            .program,
        );
        if wrong_region {
            assert!(verdicts.iter().any(|verdict| matches!(verdict,
                Ok(RouteVerdict::Rejected { unestablished, .. })
                    if unestablished.iter().any(|used| used.detail == "owned storage is used after its value was transferred or consumed")
            )), "a sibling replacement cannot restore the joined region: {verdicts:?}");
            continue;
        }
        let mut verdicts = verdicts.into_iter();
        let closure = match verdicts.next().expect("entry closure") {
            Ok(RouteVerdict::Accepted(closure)) => closure,
            other => panic!("partial and whole moves share the exact vacant join: {other:?}"),
        };
        for verdict in verdicts {
            assert!(
                matches!(verdict, Ok(RouteVerdict::Accepted(_))),
                "{verdict:?}"
            );
        }
        execute_refilled_owner(&closure);
    }
}

/// Exercise the same whole transfers without repeating source acquisition: a
/// factory result enters a named home, moves to a call argument, and is consumed.
#[test]
fn whole_owned_transfers_consume_their_homes_once() {
    use semantic_vocabulary::{
        IdTable, IntegerSign, IntegerType, IntegerValue, PsiSemanticId, ScalarType,
    };
    use typed_trees_to_terminal_psi::{
        Body, CrashCeiling, Expression, ExpressionKind, RecordField, RecordType, Signature,
        SourceOrigin, Statement, Target, Termination, Unpositioned, ValueType,
    };
    fn id<T: PsiSemanticId>(raw: u64) -> T {
        T::new(raw).unwrap()
    }
    let source = SourceOrigin::Unpositioned(Unpositioned::SpanAbsent);
    let record = ValueType::record(id(1));
    let scalar = ValueType::plain(ScalarType::Integer(
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
        shape: typed_trees_to_terminal_psi::StorageShape::Record(vec![RecordField {
            id: id(1),
            identity: "number".into(),
            value_type: scalar,
            byte_predicates: semantic_vocabulary::ByteSequencePredicateSet::EMPTY,
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
            reaches: typed_trees_to_terminal_psi::Ceiling::Inferred,
            invokes: typed_trees_to_terminal_psi::Ceiling::Inferred,
            crashes: CrashCeiling::Authored(Vec::new()),
            termination: Termination::Silent,
        });
    }
    let closure = match terminal_psi_to_terminal_psi::judge(
        typed_trees_to_terminal_psi::lower_bodies(id(1), &bodies, &signatures, &records),
    ) {
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

/// A value moved on one path into a join, whole or in part, carried from the
/// route's verdict through publication, Omega 00, Omega 01's ownership
/// validation and the interpreter. `flag && takes(owned)` moves `owned`
/// into `takes` only when `flag` holds; the arrival that did not move it
/// discards the same storage on its edge, so both enter the join with one
/// frontier.
mod transferred_on_one_path {
    use semantic_vocabulary::{
        IdTable, IntegerSign, IntegerType, IntegerValue, PsiSemanticId, ScalarType,
    };
    use typed_trees_to_terminal_psi::{
        Body, CrashCeiling, Expression, ExpressionKind, RecordField, RecordType, Signature,
        SourceOrigin, Statement, Target, Termination, Unpositioned, ValueType,
    };

    use super::{
        NoEffects, RouteVerdict, TerminalExecution, TerminalExecutionStatus,
        TerminalStructuralInputs,
    };

    fn id<T: PsiSemanticId>(raw: u64) -> T {
        T::new(raw).unwrap()
    }

    const SOURCE: SourceOrigin = SourceOrigin::Unpositioned(Unpositioned::SpanAbsent);

    fn number() -> ValueType {
        ValueType::plain(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        ))
    }

    fn boolean() -> ValueType {
        ValueType::plain(ScalarType::Boolean)
    }

    /// `Inner { number: u64 }`, record 1, and `Pair { a: Inner, b: Inner }`,
    /// record 2, neither `[copy]`.
    fn records() -> IdTable<semantic_vocabulary::StructuralTypeId, RecordType> {
        let field = |raw, identity: &str, value_type| RecordField {
            id: id(raw),
            identity: identity.into(),
            value_type,
            byte_predicates: semantic_vocabulary::ByteSequencePredicateSet::EMPTY,
        };
        let mut records = IdTable::default();
        records.push(RecordType {
            copyable: false,
            identity: "Inner".into(),
            shape: typed_trees_to_terminal_psi::StorageShape::Record(vec![field(
                3,
                "number",
                number(),
            )]),
            requires: Vec::new(),
        });
        records.push(RecordType {
            copyable: false,
            identity: "Pair".into(),
            shape: typed_trees_to_terminal_psi::StorageShape::Record(vec![
                field(1, "a", ValueType::record(id(1))),
                field(2, "b", ValueType::record(id(1))),
            ]),
            requires: Vec::new(),
        });
        records
    }

    fn expression(kind: ExpressionKind, result: Option<ValueType>) -> Expression {
        Expression {
            kind,
            result,
            source: SOURCE,
        }
    }

    fn slot(slot: usize, value_type: ValueType) -> Expression {
        expression(ExpressionKind::Slot(slot), Some(value_type))
    }

    fn field(base: Expression, field: usize, value_type: ValueType) -> Expression {
        expression(
            ExpressionKind::Field {
                base: Box::new(base),
                field,
            },
            Some(value_type),
        )
    }

    fn invoke(callee: u64, arguments: Vec<Expression>, result: ValueType) -> Expression {
        expression(
            ExpressionKind::Call {
                callee: semantic_vocabulary::TargetMachineCallable::Machine(id(callee)),
                arguments,
                designated: None,
            },
            Some(result),
        )
    }

    fn finish(value: Expression) -> Statement {
        Statement::Arm {
            guard: None,
            target: Target::Value(value),
            source: SOURCE,
        }
    }

    fn inner(value: u64) -> Expression {
        expression(
            ExpressionKind::Record {
                fields: vec![(
                    0,
                    expression(
                        ExpressionKind::Integer(IntegerValue::Unsigned(value.into())),
                        Some(number()),
                    ),
                )],
            },
            Some(ValueType::record(id(1))),
        )
    }

    /// Machine 1 binds `owned` (slot 0) from machine 3 and `flag` (slot 1)
    /// from machine 4, evaluates `flag && takes(moved)`, where machine 2 is
    /// `takes`, and returns `result`. `whole` makes `owned` an `Inner` moved
    /// whole; otherwise it is a `Pair` whose field `a` is moved.
    fn judged(
        whole: bool,
        result: Expression,
    ) -> Result<RouteVerdict, terminal_check_tables::Refusal> {
        judged_with_restoration(whole, result, false, true)
    }

    fn judged_with_restoration(
        whole: bool,
        result: Expression,
        restore: bool,
        flag: bool,
    ) -> Result<RouteVerdict, terminal_check_tables::Refusal> {
        let owned_type = ValueType::record(id(if whole { 1 } else { 2 }));
        let inner_type = ValueType::record(id(1));
        let moved = if whole {
            slot(0, owned_type)
        } else {
            field(slot(0, owned_type), 0, inner_type)
        };
        let made = if whole {
            inner(7)
        } else {
            expression(
                ExpressionKind::Record {
                    fields: vec![(0, inner(7)), (1, inner(9))],
                },
                Some(owned_type),
            )
        };
        let mut bodies = IdTable::default();
        bodies.push(Body {
            slots: vec![owned_type, boolean()],
            statements: {
                let mut statements = vec![
                    Statement::Bind {
                        slot: 0,
                        value: invoke(3, Vec::new(), owned_type),
                    },
                    Statement::Bind {
                        slot: 1,
                        value: invoke(4, Vec::new(), boolean()),
                    },
                    Statement::Evaluate(expression(
                        ExpressionKind::ShortCircuit {
                            conjunction: true,
                            left: Box::new(slot(1, boolean())),
                            right: Box::new(invoke(2, vec![moved], boolean())),
                        },
                        Some(boolean()),
                    )),
                ];
                if restore {
                    statements.push(Statement::Store {
                        destination: field(slot(0, owned_type), 0, inner_type),
                        value: inner(11),
                        source: SOURCE,
                    });
                }
                statements.push(finish(result));
                statements
            },
        });
        bodies.push(Body {
            slots: vec![inner_type],
            statements: vec![finish(expression(
                ExpressionKind::Boolean(true),
                Some(boolean()),
            ))],
        });
        bodies.push(Body {
            slots: Vec::new(),
            statements: vec![finish(made)],
        });
        bodies.push(Body {
            slots: Vec::new(),
            statements: vec![finish(expression(
                ExpressionKind::Boolean(flag),
                Some(boolean()),
            ))],
        });
        let mut signatures = IdTable::default();
        for (parameters, result) in [
            (Vec::new(), number()),
            (vec![inner_type], boolean()),
            (Vec::new(), owned_type),
            (Vec::new(), boolean()),
        ] {
            signatures.push(Signature {
                parameters,
                erased: Vec::new(),
                borrowed: Vec::new(),
                result: Some(result),
                requires: Vec::new(),
                ensures: Vec::new(),
                reaches: typed_trees_to_terminal_psi::Ceiling::Inferred,
                invokes: typed_trees_to_terminal_psi::Ceiling::Inferred,
                crashes: CrashCeiling::Authored(Vec::new()),
                termination: Termination::Silent,
            });
        }
        terminal_psi_to_terminal_psi::judge(typed_trees_to_terminal_psi::lower_bodies(
            id(1),
            &bodies,
            &signatures,
            &records(),
        ))
    }

    /// The accepted program published, lowered by Omega 00, verified by
    /// Omega 01 and run to completion.
    fn carried_through(
        verdict: Result<RouteVerdict, terminal_check_tables::Refusal>,
    ) -> terminal_psi::TerminalModule {
        carried_through_with_value(verdict, None)
    }

    fn carried_through_with_value(
        verdict: Result<RouteVerdict, terminal_check_tables::Refusal>,
        expected: Option<u128>,
    ) -> terminal_psi::TerminalModule {
        let closure = match verdict {
            Ok(RouteVerdict::Accepted(closure)) => closure,
            other => panic!("the program must be accepted: {other:?}"),
        };
        let publication = closure.publish().expect("publication");
        let input = terminal_psi_to_abstract_operations::lower_module_with_evidence(
            publication.artifact.semantic_module(),
            publication.seals.clone(),
        )
        .expect("Omega 00 carries the join's disposal")
        .into_optimization_artifact()
        .into_optimization_input();
        let (_, unit) =
            abstract_operations_to_abstract_operations::build_verified_psi_optimization_unit(
                input,
                psi_work::WorkSchedule::CURRENT.identity(),
            )
            .expect("Omega 01 builds the unit")
            .into_parts();
        // Default compilation does not re-verify its own stage output, so the
        // ownership replay that checks each edge's cleanup and each join's
        // agreement runs here, as at a trust boundary.
        optimization_unit_semantics::validate_psi_optimization_unit_with_admitted_cycle_machines(
            &unit,
            &[],
        )
        .expect("one ownership frontier at the join");
        let bytes = terminal_codec::encode_module(publication.artifact.semantic_module())
            .expect("the module encodes");
        let mut execution =
            TerminalExecution::start_artifact(&bytes, &[], TerminalStructuralInputs::default())
                .expect("the interpreter loads the module");
        let status = execution
            .resume(
                &mut psi_work::WorkMeter::with_budget(1_000_000),
                &mut NoEffects,
            )
            .expect("the program executes");
        assert!(
            matches!(status, TerminalExecutionStatus::Complete(_)),
            "{status:?}"
        );
        if let Some(expected) = expected {
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(
                    terminal_interpreter::TerminalExecutionResult::Scalar(
                        terminal_interpreter::TerminalScalarValue::Integer {
                            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                            value: IntegerValue::Unsigned(expected),
                        },
                    ),
                ),
                "restoring the field installs its new value on either path"
            );
        }
        publication.artifact.semantic_module().clone()
    }

    fn a_use_after_transfer(verdict: Result<RouteVerdict, terminal_check_tables::Refusal>) -> bool {
        matches!(verdict, Ok(RouteVerdict::Rejected { unestablished, .. })
            if unestablished.iter().any(|used| used.detail
                == "owned storage is used after its value was transferred or consumed"))
    }

    #[test]
    fn a_value_moved_whole_on_one_path_is_disposed_of_on_the_other() {
        let constant = || {
            expression(
                ExpressionKind::Integer(IntegerValue::Unsigned(0)),
                Some(number()),
            )
        };
        carried_through(judged(true, constant()));
        assert!(a_use_after_transfer(judged(
            true,
            field(slot(0, ValueType::record(id(1))), 0, number())
        )));
    }

    #[test]
    fn a_field_vacated_at_a_join_can_be_restored_after_either_arrival() {
        for flag in [false, true] {
            let inner_type = ValueType::record(id(1));
            let result = field(
                field(slot(0, ValueType::record(id(2))), 0, inner_type),
                0,
                number(),
            );
            let module = carried_through_with_value(
                judged_with_restoration(false, result, true, flag),
                Some(11),
            );
            assert!(module.machines[0].operations().any(|operation| matches!(
                &operation.kind,
                terminal_psi::OperationKind::StoreStructuralField {
                    disposition: terminal_psi::StructuralStoreDisposition::Vacant,
                    path,
                    ..
                } if !path.is_empty()
            )));
        }
    }

    #[test]
    fn a_field_moved_on_one_path_is_discarded_on_the_other_and_its_sibling_stays_owned() {
        let pair = || slot(0, ValueType::record(id(2)));
        let inner_type = ValueType::record(id(1));
        let module = carried_through(judged(
            false,
            field(field(pair(), 1, inner_type), 0, number()),
        ));
        // The arrival that did not move `a` discards it on its edge and keeps
        // the home, which `b` still occupies.
        assert!(
            module.machines[0]
                .blocks
                .iter()
                .flat_map(|block| block.terminator.cleanup_rosters())
                .any(|(_, actions)| {
                    actions.iter().any(|action| {
                        matches!(
                            action,
                            terminal_psi::TerminalAffineCleanupAction::DiscardResidual(_)
                        )
                    }) && !actions.iter().any(|action| {
                        matches!(
                            action,
                            terminal_psi::TerminalAffineCleanupAction::RetireHome(_)
                        )
                    })
                })
        );
        assert!(a_use_after_transfer(judged(
            false,
            field(field(pair(), 0, inner_type), 0, number())
        )));
    }
}
