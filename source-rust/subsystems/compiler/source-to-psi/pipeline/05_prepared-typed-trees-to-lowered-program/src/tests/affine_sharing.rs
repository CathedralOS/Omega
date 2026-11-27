//! Psi 05 plans storage, claims and reference ends with affine ownership
//! walks over the module as the loop has edited it so far. The walks over
//! one unchanged module share one `ModuleWalks` round: the module's
//! `ModuleAnalyses` bundle, whose borrowed-storage rows each walk reads, and
//! one table of callee summaries. An edit starts another round. Sharing must
//! not change any answer: each walk gives exactly what it gives alone, and a
//! loop that edits the module ends with the module that planning each
//! machine alone produces.

use super::*;
use crate::body::{Borrowed, BorrowedReferent, Regions};
use crate::lower::machine_order::ModuleWalks;
use language_core::ReferenceAccess;
use terminal_semantics::DescriptorRows;
use terminal_semantics::descriptor_results::DescriptorSlotKind;
use terminal_semantics::plain_affine as affine;

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
        assembly_target_requirement: Default::default(),
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
        assembly_target_requirement: Default::default(),
        slots: vec![view],
        statements: vec![lend(LEAF, of_type(ExpressionKind::Slot(0), view)), finish()],
    });
    bodies.push(Body {
        assembly_target_requirement: Default::default(),
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

/// Every walk of `machine` that Psi 05 runs, through one shared round.
fn shared(
    round: &ModuleWalks<'_>,
    machine: MachineId,
    candidates: &terminal_semantics::reference_ends::Candidates,
) -> String {
    let walks = round.affine(machine).unwrap();
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
fn walks_sharing_one_round_answer_as_they_do_alone() {
    let module = chain().module;
    let candidates = terminal_semantics::reference_ends::candidates(&module)
        .ok()
        .expect("the chain has reference-end candidates");
    for order in [ids(&module), ids(&module).into_iter().rev().collect()] {
        let round = ModuleWalks::new(&module);
        let sharing = round
            .reference_end_candidates()
            .ok()
            .expect("sharing finds the same candidates");
        assert_eq!(sharing, candidates);
        // Finding the candidates replayed the callees once for the walks.
        let analyses = round.analyses().expect("the chain repeats no machine");
        assert!(analyses.holds_summary(id(MIDDLE)) && analyses.holds_summary(id(LEAF)));
        // No body calls dynamically, so every closure reads the bundle's
        // shared rows rather than rows adapted for that closure alone.
        assert!(matches!(
            analyses.descriptor_closure(DescriptorSlotKind::BorrowedStorage, [id(LEFT)]),
            Ok(DescriptorRows::Shared { .. })
        ));
        for machine in order {
            let candidates = candidates.get(machine).unwrap();
            assert_eq!(
                shared(&round, machine, candidates),
                alone(&module, machine, candidates),
                "{machine:?}"
            );
        }
    }
}

/// `plan_then_apply` with a walk as its plan and an application that edits
/// only `edited`, against the loop that plans each machine alone against
/// the module as edited so far. Returns what each walk planned, whether the
/// round it was given already held the middle machine, and the final module.
fn applied(
    workers: usize,
    edited: u64,
) -> (Vec<(MachineId, String, bool)>, terminal_psi::TerminalModule) {
    let mut module = chain().module;
    let walked = std::sync::Mutex::new(Vec::new());
    let budget = worker_budget::ExecutionBudget::with_workers(workers).unwrap();
    budget
        .run(|| {
            crate::lower::machine_order::plan_then_apply(
                &mut module,
                |round, machine| {
                    let held = round
                        .analyses()
                        .is_some_and(|analyses| analyses.holds_summary(id(MIDDLE)));
                    let plan = round.affine(machine.id).and_then(|walks| walks.plan());
                    let expected = affine::plan_in(round.module(), machine.id);
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
                        Ok::<bool, MachineId>(true)
                    } else {
                        Ok(false)
                    }
                },
            )
        })
        .unwrap();
    (walked.into_inner().unwrap(), module)
}

#[test]
fn applying_plans_shares_one_round_until_an_application_edits_a_machine() {
    // In order on one worker, the left walk replays the middle machine into
    // the table, and the right walk, against the same module, finds it there.
    // The left edit starts a fresh round for the module it leaves.
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

/// Storage planning as `finish_storage` runs it: each walk reads its round's
/// bundle and table, and an application that changes a machine ends the
/// round. The loop must leave exactly the module that planning each machine
/// alone, against the module as edited so far, leaves.
#[test]
fn storage_rounds_through_shared_walks_leave_the_module_planning_alone_leaves() {
    // The chain arrives with its storage installed, so its plans change no
    // machine. Marking the left caller as well edits the module, and the
    // walks after it must run in a new round over the edited module.
    let install = |machine: &mut terminal_psi::TerminalMachine, plan: &affine::Plan| {
        let installed = crate::install_storage_plan(machine, plan).expect("the plan installs");
        if machine.id == id(LEFT) {
            machine.attachment = Some(id(LEFT));
            return true;
        }
        installed
    };
    let mut expected = chain().module;
    for ordinal in 0..expected.machines.len() {
        let plan = affine::plan_in(&expected, expected.machines[ordinal].id)
            .expect("each machine plans alone");
        let mut machine = expected.machines[ordinal].clone();
        install(&mut machine, &plan);
        expected.machines[ordinal] = machine;
    }
    assert_ne!(expected, chain().module);
    for parallel in [false, true] {
        let mut module = chain().module;
        crate::lower::machine_order::plan_then_apply_with(
            &mut module,
            parallel,
            |round, machine| round.affine(machine.id).and_then(|walks| walks.plan()),
            |plan| {
                plan.as_ref()
                    .ok()
                    .map(|plan| plan.graphed_machines.as_slice())
            },
            |machine, plan| {
                let plan = plan.expect("each machine plans through its round");
                Ok::<bool, ()>(install(machine, &plan))
            },
        )
        .expect("every plan installs");
        assert_eq!(module, expected, "parallel: {parallel}");
    }
}
