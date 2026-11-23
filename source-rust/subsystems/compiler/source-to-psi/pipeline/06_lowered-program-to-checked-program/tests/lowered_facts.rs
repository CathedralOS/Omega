//! Stage 06, the lowered-form checker, must judge a program by the facts
//! stage 05 wrote into it: where each branch goes, what each call passes
//! and owes, which type a return has, when zeroed storage may be used, which
//! call a suspension parks and what each match arm disposes. Each test
//! lowers one source program, confirms the checker's verdict on it, changes
//! one of those facts as a faulty stage 05 would, and requires the verdict
//! to change.
//!
//! Two kinds of answer count. A change that leaves the lowered form
//! disagreeing with itself (a terminator and its edge row, a call and its
//! invocation row) is refused as a foreign identity. A change made
//! consistently in every place stage 05 records the fact gives a program
//! that means something else, and the checker then judges that program:
//! it accepts it or rejects it with the proof it cannot make.
//!
//! The moves, stores, reads, construction operands and ranks lowering hands
//! the checker are tested in `owned_place_transfers.rs`,
//! `construction_operands.rs` and `rank_edges.rs`.

#[path = "support/source_handoff.rs"]
mod support;

use semantic_vocabulary::{MachineId, ObligationId};
use source_compiler::test_support::lowering::Elaboration;
use support::{Program, Verdict, the_operation};
use terminal_check_tables::edges::Destination;
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::{Construct, Refusal};
use terminal_psi::{
    ExecutionSite, OperationKind, TerminalAffineCleanupAction, TerminalMachine,
    TerminalSuspensionCallTarget, Terminator,
};

/// The refusal detail of a lowered form that disagrees with itself.
#[track_caller]
fn refused(verdict: Verdict) -> String {
    verdict.refused().detail
}

/// The machine and construct a refusal names.
#[track_caller]
fn in_machine(refusal: &Refusal) -> (MachineId, Construct) {
    refusal
        .site
        .in_machine()
        .unwrap_or_else(|| panic!("the refusal names a machine: {refusal:?}"))
}

/// The kinds of the obligations the checker could not prove.
#[track_caller]
fn unproved(verdict: Verdict) -> Vec<String> {
    let rejection = verdict.rejected();
    assert!(
        rejection.violations.is_empty()
            && rejection.unestablished.is_empty()
            && rejection.conflicts.is_empty(),
        "{rejection:?}"
    );
    rejection.unproved
}

fn safety_of(operation: semantic_vocabulary::OperationId) -> String {
    format!(
        "{:?}",
        ObligationKind::OperationSafety {
            site: ExecutionSite::Operation(operation)
        }
    )
}

const GUARDED: &str =
    "machine guarded(n: u8) -> u8 { match n > 0 { true -> n - 1, false -> 0u8, } }\n";

/// Exchange the blocks the conditional branch of `machine` enters, in the
/// terminator and, when `rows` is set, in the two edge rows as well.
fn swap_branches(lowered: &mut Elaboration, position: usize, rows: bool) {
    let machine = &mut lowered.module.machines[position];
    let id = machine.id;
    let mut found = None;
    for block in &mut machine.blocks {
        if let Terminator::Conditional {
            when_true,
            when_false,
            ..
        } = &mut block.terminator
        {
            std::mem::swap(&mut when_true.target, &mut when_false.target);
            found = Some([
                (when_true.edge, when_true.target),
                (when_false.edge, when_false.target),
            ]);
        }
    }
    let edges = found.expect("the machine branches");
    if rows {
        let tables = lowered.tables.machines.get_mut(id).unwrap();
        for (edge, target) in edges {
            tables.edges.get_mut(edge).unwrap().destination = Destination::Block(target);
        }
    }
}

#[test]
fn a_branch_carries_its_guard_into_the_block_it_enters() {
    let lowered = Program::of(GUARDED);
    lowered.judged(|_| {}).accepted();
    let subtract = the_operation(lowered.machine("guarded"), |kind| {
        matches!(kind, OperationKind::ExactIntegerSubtract { .. })
    })
    .id;
    let position = lowered.position("guarded");

    // The terminator alone sends `n > 0` to the other block: its edge rows
    // now disagree with it.
    assert_eq!(
        refused(lowered.judged(|lowered| swap_branches(lowered, position, false))),
        "an edge row does not go where its terminator does in machine `guarded`"
    );
    // Both records exchanged: `n - 1` now runs when `n` is 0, and the checker
    // can no longer prove it.
    assert_eq!(
        unproved(lowered.judged(|lowered| swap_branches(lowered, position, true))),
        [safety_of(subtract)]
    );
    // The source twin with its arms exchanged is rejected the same way.
    let twin = Program::of(
        "machine guarded(n: u8) -> u8 { match n > 0 { true -> 0u8, false -> n - 1, } }\n",
    );
    let subtract = the_operation(twin.machine("guarded"), |kind| {
        matches!(kind, OperationKind::ExactIntegerSubtract { .. })
    })
    .id;
    assert_eq!(unproved(twin.judged(|_| {})), [safety_of(subtract)]);
}

const CALLS: &str = "machine sub(a: u8, b: u8) -> u8 requires a >= b { a - b }
machine relay(x: u8, y: u8) -> u8 requires x >= y { sub(x, y) }
machine checked(a: u8, b: u8) -> u8 requires a < 10, b < 20 { a }
machine caller(x: u8, y: u8) -> u8 requires x < 10, y < 20 { checked(x, y) }
";

/// Change the arguments of `name`'s one call, in the operation and, when
/// `row` is set, in its invocation row too.
fn change_call(
    lowered: &Program,
    name: &str,
    row: bool,
    change: impl Fn(&mut Vec<semantic_vocabulary::ValueId>, &mut Vec<ObligationId>),
) -> Verdict {
    let position = lowered.position(name);
    let id = lowered.machine(name).id;
    lowered.judged(|elaboration| {
        for block in &mut elaboration.module.machines[position].blocks {
            for operation in &mut block.operations {
                if let OperationKind::Call {
                    arguments,
                    requirement_obligations,
                    ..
                } = &mut operation.kind
                {
                    change(arguments, requirement_obligations);
                }
            }
        }
        if row {
            let tables = elaboration.tables.machines.get_mut(id).unwrap();
            let invocations: Vec<_> = tables.invocations.iter().map(|(id, _)| id).collect();
            for invocation in invocations {
                let row = tables.invocations.get_mut(invocation).unwrap();
                let mut actuals: Vec<_> = row
                    .actuals
                    .iter()
                    .map(|actual| match actual {
                        terminal_check_tables::edges::Actual::Value(value) => *value,
                        other => panic!("{other:?}"),
                    })
                    .collect();
                change(&mut actuals, &mut row.requirements);
                row.actuals = actuals
                    .into_iter()
                    .map(terminal_check_tables::edges::Actual::Value)
                    .collect();
            }
        }
    })
}

#[test]
fn a_call_is_judged_with_the_actuals_it_passes() {
    let lowered = Program::of(CALLS);
    lowered.judged(|_| {}).accepted();
    let swap = |arguments: &mut Vec<_>, _: &mut Vec<_>| arguments.swap(0, 1);
    assert_eq!(
        refused(change_call(&lowered, "relay", false, swap)),
        "an invocation row's actuals are not the call's arguments in machine `relay`"
    );
    // `sub(y, x)` owes `y >= x`, which `relay`'s `x >= y` does not give.
    let rejected = unproved(change_call(&lowered, "relay", true, swap));
    let tables = lowered.tables("relay");
    let (invocation, _) = tables.invocations.iter().next().unwrap();
    assert_eq!(
        rejected,
        [format!(
            "{:?}",
            ObligationKind::CallRequirement {
                invocation,
                clause: 0
            }
        )]
    );
}

#[test]
fn a_call_owes_one_requirement_for_each_clause_of_its_callee() {
    let lowered = Program::of(CALLS);
    for (label, change) in [
        ("cleared", Vec::clear as fn(&mut Vec<ObligationId>)),
        ("first dropped", |owed: &mut Vec<ObligationId>| {
            owed.remove(0);
        }),
        ("reversed", |owed: &mut Vec<ObligationId>| owed.reverse()),
    ] {
        assert_eq!(
            refused(change_call(&lowered, "caller", false, |_, owed| change(
                owed
            ))),
            "a call does not owe one requirement per requires clause of its callee in machine `caller`",
            "{label}"
        );
    }
}

/// Either byte is a legal return. Lowering decides which formal the source names.
#[test]
fn a_scalar_return_keeps_its_authored_operand_and_the_checker_judges_its_type() {
    for (name, index) in [("left", 0), ("right", 1)] {
        let program = Program::of(&format!(
            "machine pick(left: u8, right: u8) -> u8 {{ {name} }}"
        ));
        let machine = program.machine("pick");
        assert_eq!(
            support::returned(machine),
            machine.contract.parameters[index].id
        );
        program.judged(|_| {}).accepted();
        let other = machine.contract.parameters[1 - index].id;
        program
            .judged_in("pick", |body| {
                for block in &mut body.blocks {
                    if let Terminator::Return { value, .. } = &mut block.terminator {
                        *value = other;
                    }
                }
            })
            .accepted();
    }
}

/// Even when both views have exactly one byte, the length used by a read
/// must come from measuring the descriptor that supplies that byte.
#[test]
fn a_byte_read_keeps_its_own_measurement_when_the_other_length_is_equal() {
    let program = Program::of(
        "machine read(left: @[u8], right: @[u8]) -> u8 requires left.length == 1u64 && right.length == 1u64 { left[0u64] }",
    );
    let machine = program.machine("read");
    let left = machine.structural_parameters[0].place;
    let right = machine.structural_parameters[1].place;
    assert_ne!(left, right);
    let read = the_operation(machine, |kind| {
        matches!(kind, OperationKind::ByteSequenceRead { .. })
    });
    let OperationKind::ByteSequenceRead { source, length, .. } = &read.kind else {
        unreachable!()
    };
    assert_eq!(*source, left);
    let measure = machine
        .operations()
        .find(|op| {
            op.result
                .scalar_ref()
                .is_some_and(|value| value.id == *length)
        })
        .unwrap();
    assert!(
        matches!(&measure.kind, OperationKind::ByteSequenceLength { source } if source.place == left && source.path.is_empty())
    );
    program.judged(|_| {}).accepted();

    let verdict = program.judged_in("read", |body| {
        let changed = body
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find(|op| op.id == measure.id)
            .unwrap();
        let OperationKind::ByteSequenceLength { source } = &mut changed.kind else {
            unreachable!()
        };
        source.place = right;
    });
    let Verdict::Unserved(refusal) = verdict else {
        panic!("expected a measurement refusal, got {verdict:?}")
    };
    assert_eq!(
        in_machine(&refusal),
        (machine.id, Construct::Operation(read.id))
    );
    assert!(
        refusal.detail.contains("measurement directly before it"),
        "{refusal:?}"
    );
}

/// `first` returns its `bool` formal where it declares a `u8` result. The
/// checker refuses the module and names `first`, not `other`, which is the
/// module's first machine and is unchanged.
#[test]
fn a_returned_value_has_the_declared_result_type() {
    let lowered = Program::of(
        "machine other(a: u8) -> u8 { a }\nmachine first(a: u8, flag: bool) -> u8 { a }\n",
    );
    lowered.judged(|_| {}).accepted();
    let first = lowered.machine("first");
    let flag = first.contract.parameters[1].id;
    let refusal = lowered
        .judged_in("first", |machine| {
            for block in &mut machine.blocks {
                if let Terminator::Return { value, .. } = &mut block.terminator {
                    *value = flag;
                }
            }
        })
        .refused();
    assert_eq!(
        in_machine(&refusal),
        (first.id, Construct::Machine),
        "{refusal:?}"
    );
    assert_eq!(
        refusal.detail,
        "invalid declared scalar qualifications: normal return does not preserve its complete \
         declared scalar type in machine `first`"
    );
}

#[test]
fn zeroed_storage_must_meet_its_record_predicate_before_it_is_used() {
    let lowered = Program::of(
        "data Small requires self.value < 9 { value: u8; }
data Nonzero requires self.value > 0 { value: u8; }
machine look(c: @Small) {}
machine peer(c: @Nonzero) {}
machine small() { let mutable cell: Small; look(@cell); }
machine nonzero() { let mutable cell: Nonzero; peer(@cell); }
",
    );
    let rejection = lowered.judged(|_| {}).rejected();
    assert_eq!(
        rejection.unestablished,
        [
            "zeroed storage of a gated type is used before its requires holds: the bounds do not decide the comparison"
        ]
    );
    // Only `nonzero` is wrong: the same program without it is accepted.
    Program::of(
        "data Small requires self.value < 9 { value: u8; }
machine look(c: @Small) {}
machine small() { let mutable cell: Small; look(@cell); }
",
    )
    .judged(|_| {})
    .accepted();
}

const SUSPEND: &str = "machine step(n: u8) -> u8 suspends { n }
machine other(n: u8) -> u8 suspends { n }
machine use_it(n: u8, keep: u8, dead: u8) -> u8 suspends { let answer: u8 = suspend step(n); answer +% keep }
";

#[test]
fn a_suspension_site_commits_to_the_call_it_parks() {
    let lowered = Program::of(SUSPEND);
    lowered.judged(|_| {}).accepted();
    let other = lowered.machine("other").id;
    let crossing = lowered
        .machine("use_it")
        .operations()
        .find_map(|operation| operation.suspension_crossing)
        .expect("`use_it` parks a call");
    assert_eq!(
        refused(lowered.judged(|elaboration| {
            for site in &mut elaboration.module.execution.suspension_call_sites {
                if site.crossing == crossing {
                    site.target = TerminalSuspensionCallTarget::Machine(other);
                }
            }
        })),
        "a suspension site does not commit to the live frontier of its call in machine `use_it`"
    );
    assert_eq!(
        refused(lowered.judged(|elaboration| {
            for plan in &mut elaboration.module.execution.suspension_call_plans {
                if plan.crossing == crossing {
                    plan.target = TerminalSuspensionCallTarget::Machine(other);
                }
            }
        })),
        "a suspension plan is not the live frontier of its call in machine `use_it`"
    );
}

const OWNED: &str = "data Cell { v: u8; }
data Pair { left: Cell; right: Cell; }
machine choose(p: Pair, flag: bool) -> Cell { let chosen: Cell = match flag { true -> move p.left, false -> move p.right, }; chosen }
";

fn cleanups(machine: &mut TerminalMachine) -> Vec<&mut Vec<TerminalAffineCleanupAction>> {
    machine
        .blocks
        .iter_mut()
        .flat_map(|block| block.terminator.cleanup_rosters_mut())
        .map(|(_, actions)| actions)
        .collect()
}

/// Each arm of a match on owned fields moves its own field and disposes the
/// other before the join. Exchanging the two arms' disposals is refused.
#[test]
fn each_match_arm_disposes_its_own_sibling_before_the_join() {
    let lowered = Program::of(OWNED);
    lowered.judged(|_| {}).accepted();
    assert_eq!(
        refused(lowered.judged_in("choose", |machine| {
            let mut lists = cleanups(machine)
                .into_iter()
                .filter(|actions| !actions.is_empty())
                .collect::<Vec<_>>();
            assert_eq!(lists.len(), 2, "each arm disposes one sibling");
            let second = std::mem::take(lists[1]);
            let first = std::mem::replace(lists[0], second);
            *lists[1] = first;
        })),
        "normal edge affine disposal differs from its canonical schedule in machine `choose`"
    );
}

/// The checker judges every machine of the program, not only the one a test
/// reads: `main` does nothing and is unchanged, and removing the constant
/// `helper` returns from `helper`'s body still makes the program refused,
/// at the return that now names a value nothing produced.
#[test]
fn an_operation_removed_from_another_machine_is_refused() {
    let program = Program::of("machine main() {} machine helper() -> u8 { 7u8 }");
    program.judged(|_| {}).accepted();
    let detail = refused(program.judged_in("helper", |machine| {
        machine.blocks[0].operations.clear();
    }));
    assert_eq!(
        detail,
        "invalid declared scalar qualifications: scalar transfer names an absent value in machine `helper`"
    );
}
