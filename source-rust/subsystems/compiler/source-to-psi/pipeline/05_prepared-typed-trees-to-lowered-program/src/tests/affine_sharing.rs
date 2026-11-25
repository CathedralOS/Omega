//! Psi 05 plans storage, claims and reference ends with affine ownership
//! walks over the module as the loop has edited it so far. The walks over
//! one unchanged module share callee summaries through one `AffineSummaries`
//! table, and an edit starts another. Sharing must not change any answer:
//! each walk gives exactly what it gives alone.

use super::*;
use crate::body::{Borrowed, BorrowedReferent, Regions};
use language_core::ReferenceAccess;
use terminal_semantics::plain_affine::{self as affine, AffineSummaries, MachineAffine};

const LEFT: u64 = 1;
const RIGHT: u64 = 2;
const MIDDLE: u64 = 3;
const LEAF: u64 = 4;

/// Two callers lend a view of their array to a middle machine, which lends
/// it on to a leaf, so each caller's walk replays the summaries below it.
fn chain() -> crate::Elaboration {
    let array = ValueType::record(id(2));
    let view = ValueType::record(id(3));
    let lend = |callee: u64, actual: Expression| {
        Statement::discard(call(
            callee,
            vec![of_type(
                ExpressionKind::Borrow {
                    access: ReferenceAccess::Shared,
                    place: Box::new(actual),
                    home: None,
                },
                view,
            )],
            None,
        ))
    };
    let finish = || Statement::Arm {
        guard: ArmGuard::Always,
        target: Target::Unit,
        source: nowhere(),
    };
    let caller = || Body {
        slots: vec![array],
        statements: vec![
            lend(
                MIDDLE,
                of_type(
                    ExpressionKind::CollectionView {
                        base: Box::new(of_type(ExpressionKind::Slot(0), array)),
                        access: ReferenceAccess::Shared,
                        element: id(1),
                    },
                    view,
                ),
            ),
            finish(),
        ],
    };
    let mut bodies = IdTable::default();
    bodies.push(caller());
    bodies.push(caller());
    bodies.push(Body {
        slots: vec![view],
        statements: vec![lend(LEAF, of_type(ExpressionKind::Slot(0), view)), finish()],
    });
    bodies.push(Body {
        slots: vec![view],
        statements: vec![finish()],
    });
    let borrowing = || {
        let mut signature = shaped(vec![view], None);
        signature.borrowed.push(Borrowed {
            position: 0,
            access: ReferenceAccess::Shared,
            referent: BorrowedReferent::Slice,
            regions: Regions::NONE,
            home: None,
        });
        signature
    };
    let mut signatures = IdTable::default();
    signatures.push(shaped(vec![array], None));
    signatures.push(shaped(vec![array], None));
    signatures.push(borrowing());
    signatures.push(borrowing());
    crate::lower_bodies(
        id(LEFT),
        &bodies,
        &signatures,
        &super::array_views::array_types(StructuralAccess::SharedBorrow),
    )
    .expect("the view chain lowers")
}

fn ids(module: &terminal_psi::TerminalModule) -> Vec<MachineId> {
    module.machines.iter().map(|machine| machine.id).collect()
}

/// Every walk of `machine` that Psi 05 runs, through one shared table.
fn shared(
    module: &terminal_psi::TerminalModule,
    machine: MachineId,
    candidates: &terminal_semantics::reference_ends::Candidates,
    summaries: &AffineSummaries<'_>,
) -> String {
    let walks = MachineAffine::new(module, machine, Some(summaries)).unwrap();
    format!(
        "{:?}\n{:?}\n{:?}",
        walks.plan(),
        walks.plan_claims(),
        walks.plan_reference_ends(candidates),
    )
}

/// The same walks through the one-walk entry points, each replaying every
/// callee itself.
fn alone(
    module: &terminal_psi::TerminalModule,
    machine: MachineId,
    candidates: &terminal_semantics::reference_ends::Candidates,
) -> String {
    format!(
        "{:?}\n{:?}\n{:?}",
        affine::plan_in(module, machine),
        affine::plan_claims_in(module, machine),
        affine::plan_reference_ends(module, machine, candidates),
    )
}

#[test]
fn walks_sharing_the_candidates_table_answer_as_they_do_alone() {
    let module = chain().module;
    let candidates = terminal_semantics::reference_ends::candidates(&module)
        .ok()
        .expect("the chain has reference-end candidates");
    for order in [ids(&module), ids(&module).into_iter().rev().collect()] {
        let summaries = AffineSummaries::new(&module);
        let sharing = terminal_semantics::reference_ends::candidates_sharing(&module, &summaries)
            .ok()
            .expect("sharing finds the same candidates");
        assert_eq!(sharing, candidates);
        // Finding the candidates replayed the callees once for the walks.
        assert!(summaries.holds(id(MIDDLE)) && summaries.holds(id(LEAF)));
        for machine in order {
            let candidates = candidates.get(machine).unwrap();
            assert_eq!(
                shared(&module, machine, candidates, &summaries),
                alone(&module, machine, candidates),
                "{machine:?}"
            );
        }
    }
}

/// `plan_then_apply` with a walk as its plan and an application that edits
/// only `edited`, against the loop that plans each machine alone against
/// the module as edited so far. Returns what each walk planned, whether the
/// table it was given already held the middle machine, and the final module.
fn applied(
    workers: usize,
    edited: u64,
) -> (Vec<(MachineId, String, bool)>, terminal_psi::TerminalModule) {
    let mut module = chain().module;
    let walked = std::sync::Mutex::new(Vec::new());
    let budget = worker_budget::ExecutionBudget::with_workers(workers).unwrap();
    budget
        .run(|| {
            crate::machine_order::plan_then_apply(
                &mut module,
                |module, summaries, machine| {
                    let held = summaries.holds(id(MIDDLE));
                    let plan = MachineAffine::new(module, machine.id, Some(summaries))
                        .and_then(|walks| walks.plan());
                    let expected = affine::plan_in(module, machine.id);
                    walked
                        .lock()
                        .unwrap()
                        .push((machine.id, format!("{plan:?}"), held));
                    assert_eq!(format!("{plan:?}"), format!("{expected:?}"));
                    plan
                },
                |_| None,
                |machine, plan| {
                    plan.map_err(|_| machine.id)?;
                    if machine.id.get() == edited {
                        machine.attachment = Some(id(LEFT));
                    }
                    Ok::<(), MachineId>(())
                },
            )
        })
        .unwrap();
    (walked.into_inner().unwrap(), module)
}

#[test]
fn applying_plans_shares_one_table_until_an_application_edits_a_machine() {
    // In order on one worker, the left walk replays the middle machine into
    // the table, and the right walk, against the same module, finds it there.
    // The left edit starts a fresh table for the module it leaves.
    let (unedited, module) = applied(1, 0);
    let held = |walked: &[(MachineId, String, bool)]| {
        walked
            .iter()
            .map(|(machine, _, held)| (machine.get(), *held))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        held(&unedited),
        [(LEFT, false), (RIGHT, true), (MIDDLE, true), (LEAF, true)]
    );
    assert_eq!(module, chain().module);
    let (edited, module) = applied(1, LEFT);
    assert_eq!(
        held(&edited),
        [(LEFT, false), (RIGHT, false), (MIDDLE, true), (LEAF, true)]
    );
    assert_eq!(module.machines[0].attachment, Some(id(LEFT)));
    // Each walk asserted it planned what it plans alone. On several workers
    // every machine is planned in advance and the ones after the edit again.
    let (parallel, parallel_module) = applied(4, LEFT);
    assert_eq!(parallel_module, module);
    assert_eq!(parallel.len(), 2 * 4 - 1);
}
