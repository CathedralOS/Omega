//! Stage 06 must refuse a lowered program in which a match arm selects a
//! case of a sum its subject does not hold. These tests take stage 05's
//! output for one fixed source program, change that one fact as a faulty
//! stage 05 would, and hand the result to the checker.
//!
//! The program (`05_prepared-typed-trees-to-lowered-program/tests/support/
//! sum_dispatch.rs`) has two unrelated sums, `Left` and `Right`. Each
//! declares `Small(value: ...)` first and `Nothing` second; `Left::Small`
//! carries a `u8` and `Right::Small` a `bool`. A case's name, its position
//! and its field's name therefore match between the sums, and only the case
//! id says which sum an arm selects from. Stage 05's side of this is
//! asserted in that stage's `tests/variant_parent_identity.rs`.
//!
//! The rule: "The complete declared case roster determines its successors"
//! and "Selected payload fields become scalar parameters on that case's
//! edge" (`wiki/spec/terminal-psi/control_flow.md`); a payload field named
//! through a `Case` step is "a relevant scalar payload field of that exact
//! declared case", and "matching case spellings in another type supply no
//! authority" (`wiki/spec/terminal-psi/structural_access.md`).
//!
//! The checker answers a wrong case in three places, and which one answers
//! first depends on a fact that has nothing to do with cases:
//!
//! - A machine that publishes a write frame (each machine that matches on a
//!   sum here does) is refused before its dispatch is looked at. The
//!   write-frame check finds no storage flow for a body whose dispatch is
//!   malformed, and reports that as a frame that "differs from its body".
//! - Leaving the write frame out is allowed, and then the checker's own
//!   rules about cases answer: a dispatch "lists a case of another sum", or
//!   a case "is not of the sum it is applied to".
//! - A payload lent to a call through a `Case` step is refused when the
//!   path is read, before either of the above.
//!
//! The first test keeps the unchanged program and the two changes the
//! contract allows, so a checker that refused everything would fail.

#[path = "../../05_prepared-typed-trees-to-lowered-program/tests/support/sum_dispatch.rs"]
mod program;

use program::{
    Prepared, Sum, dispatch, dispatch_edges_mut, machine_position, payload_loan, payload_loan_mut,
    prepare, sum,
};
use semantic_vocabulary::{MachineId, StructuralCaseId};
use source_compiler::test_support::checking::{
    CompilationRefusal, RouteVerdict, judge_compilation,
};
use source_compiler::test_support::lowering::Elaboration;
use terminal_check_tables::compilation::{PreparedCompilation, TerminalCompilation};
use terminal_check_tables::{Construct, Refusal, RefusalReason, RefusalSite};
use terminal_psi::{StructuralPathSegment, TerminalMachine};

struct Fixture {
    prepared: Prepared,
    /// Stage 05's output, as stage 06 receives it.
    lowered: PreparedCompilation,
    left: Sum,
    right: Sum,
}

impl Fixture {
    fn new() -> Self {
        let prepared = prepare();
        let lowered = prepared.lower();
        let closure = lowered.closure.as_ref().unwrap();
        let (left, right) = (sum(closure, "Left"), sum(closure, "Right"));
        Self {
            prepared,
            lowered,
            left,
            right,
        }
    }

    fn position(&self, name: &str) -> usize {
        machine_position(&self.prepared, self.lowered.closure.as_ref().unwrap(), name)
    }

    fn machine(&self, name: &str) -> &TerminalMachine {
        &self.lowered.closure.as_ref().unwrap().module.machines[self.position(name)]
    }

    /// The machines that match on a sum: the sum each one's subject holds,
    /// then the unrelated sum.
    fn matches(&self) -> [(&'static str, &Sum, &Sum); 3] {
        [
            ("choose", &self.left, &self.right),
            ("other", &self.right, &self.left),
            ("lend", &self.left, &self.right),
        ]
    }

    /// Stage 06's verdict on stage 05's output after `change`.
    fn judged(&self, change: impl FnOnce(&mut Elaboration)) -> Verdict {
        let mut altered = self.lowered.clone();
        change(altered.closure.as_mut().unwrap());
        let mut verdicts = judge_compilation(TerminalCompilation::Prepared(altered)).into_iter();
        let verdict = verdicts.next().expect("one module, one verdict");
        assert!(verdicts.next().is_none(), "the program is one module");
        match verdict {
            Ok(RouteVerdict::Accepted(_)) => Verdict::Accepted,
            Ok(RouteVerdict::FallBack(refusal)) => Verdict::Unserved(refusal),
            Err(CompilationRefusal::Elaboration(refusal)) => Verdict::Refused(refusal),
            other => panic!("{other:?}"),
        }
    }
}

#[derive(Debug)]
enum Verdict {
    Accepted,
    /// The lowered program disagrees with itself: the checker's answer to a
    /// product no correct stage 05 issues.
    Refused(Refusal),
    /// The checker does not judge this form and accepts nothing in it.
    Unserved(Refusal),
}

impl Verdict {
    fn refused(self) -> Refusal {
        match self {
            Self::Refused(refusal) => {
                assert_eq!(
                    refusal.reason,
                    RefusalReason::ForeignIdentity,
                    "{refusal:?}"
                );
                refusal
            }
            other => panic!("the changed program must be refused: {other:?}"),
        }
    }
}

/// Make the edge of `machine`'s dispatch that selects `from` select `to`.
/// Nothing else about the edge changes: its target, its payload fields and
/// its place in the roster stay.
fn reselect(machine: &mut TerminalMachine, from: StructuralCaseId, to: StructuralCaseId) {
    let mut edges = dispatch_edges_mut(machine)
        .iter_mut()
        .filter(|edge| edge.case == from);
    let edge = edges.next().expect("the dispatch lists the case");
    assert!(edges.next().is_none());
    edge.case = to;
}

/// Leave out the write frame `machine` publishes. A caller then assumes the
/// machine may write anything it was handed, which claims less than any
/// frame does.
fn omit_write_frame(machine: &mut TerminalMachine) {
    assert!(
        machine.contract.published_writes.take().is_some(),
        "stage 05 publishes a write frame for every machine of this program"
    );
}

fn in_machine(refusal: &Refusal) -> (MachineId, Construct) {
    refusal
        .site
        .in_machine()
        .unwrap_or_else(|| panic!("the refusal names a machine: {refusal:?}"))
}

#[test]
fn the_lowered_program_and_changes_the_contract_allows_are_accepted() {
    let fixture = Fixture::new();
    assert!(matches!(fixture.judged(|_| {}), Verdict::Accepted));
    for (name, _, _) in fixture.matches() {
        let position = fixture.position(name);

        // A roster lists every case once; the order of its edges says
        // nothing.
        let reordered = fixture.judged(|lowered| {
            dispatch_edges_mut(&mut lowered.module.machines[position]).reverse();
        });
        assert!(
            matches!(reordered, Verdict::Accepted),
            "{name}: {reordered:?}"
        );

        let unframed =
            fixture.judged(|lowered| omit_write_frame(&mut lowered.module.machines[position]));
        assert!(
            matches!(unframed, Verdict::Accepted),
            "{name}: {unframed:?}"
        );
    }
}

/// The one changed fact: an edge selects the case at the same position, with
/// the same name, in the unrelated sum. As stage 05 issues the program, each
/// machine publishes a write frame, and the write-frame check refuses the
/// module first.
#[test]
fn an_edge_that_selects_the_other_sums_case_is_refused() {
    let fixture = Fixture::new();
    for (name, own, foreign) in fixture.matches() {
        let position = fixture.position(name);
        let machine = fixture.machine(name).id;
        for (ours, theirs) in own.cases.iter().zip(&foreign.cases) {
            let refusal = fixture
                .judged(|lowered| {
                    reselect(&mut lowered.module.machines[position], ours.id, theirs.id)
                })
                .refused();
            assert!(
                refusal.detail.starts_with(&format!(
                    "published write frame for {machine:?} differs from its body"
                )),
                "{name}: {refusal:?}"
            );
        }
    }
}

/// The same change in a machine that publishes no write frame, which the
/// first test shows is accepted on its own. Now the checker's rules about
/// cases answer, at the machine and the dispatch that were changed.
#[test]
fn without_a_write_frame_the_same_edge_is_refused_as_a_case_of_another_sum() {
    let fixture = Fixture::new();
    for (name, own, foreign) in fixture.matches() {
        let position = fixture.position(name);
        let machine = fixture.machine(name);
        let (dispatching, _, edges) = dispatch(machine);
        for (ours, theirs) in own.cases.iter().zip(&foreign.cases) {
            let refusal = fixture
                .judged(|lowered| {
                    let machine = &mut lowered.module.machines[position];
                    omit_write_frame(machine);
                    reselect(machine, ours.id, theirs.id);
                })
                .refused();
            let edge = edges.iter().find(|edge| edge.case == ours.id).unwrap();
            // A case with a payload is caught where the checker reads the
            // payload its edge binds; a case with none, where it compares
            // the roster with the sum's declared cases.
            let (site, rule) = if ours.fields.is_empty() {
                (
                    Construct::Block(dispatching.id),
                    "a dispatch lists a case of another sum",
                )
            } else {
                (
                    Construct::Edge(edge.edge),
                    "a case is not of the sum it is applied to",
                )
            };
            assert_eq!(
                in_machine(&refusal),
                (machine.id, site),
                "{name}: {refusal:?}"
            );
            assert_eq!(refusal.detail, format!("{rule} in machine `{name}`"));
        }
    }
}

/// The other way to state the wrong sum: the dispatch keeps its edges, and
/// the formal it reads is declared as the unrelated sum.
#[test]
fn a_subject_declared_as_the_other_sum_is_refused() {
    let fixture = Fixture::new();
    for (name, own, foreign) in fixture.matches() {
        let position = fixture.position(name);
        let machine = fixture.machine(name).id;
        let redeclare = |machine: &mut TerminalMachine| {
            let [formal] = machine.contract.structural_parameters.as_mut_slice() else {
                panic!("`{name}` takes one sum")
            };
            assert_eq!(formal.structural_type, own.id);
            formal.structural_type = foreign.id;
        };

        let refusal = fixture
            .judged(|lowered| redeclare(&mut lowered.module.machines[position]))
            .refused();
        assert!(
            refusal.detail.starts_with(&format!(
                "published write frame for {machine:?} differs from its body"
            )),
            "{name}: {refusal:?}"
        );

        let refusal = fixture
            .judged(|lowered| {
                let machine = &mut lowered.module.machines[position];
                omit_write_frame(machine);
                redeclare(machine);
            })
            .refused();
        assert_eq!(in_machine(&refusal).0, machine, "{name}: {refusal:?}");
        assert_eq!(
            refusal.detail,
            format!("a case is not of the sum it is applied to in machine `{name}`")
        );
    }
}

/// `lend` hands `look` the payload of `Left::Small` by the path `Case`,
/// `value`. With the `Case` step changed to `Right::Small`, which has a
/// field of the same name and another type, the path names no storage of a
/// `Left`, and the checker says so at the call.
#[test]
fn a_payload_lent_through_the_other_sums_case_is_refused() {
    let fixture = Fixture::new();
    let position = fixture.position("lend");
    let machine = fixture.machine("lend");
    let (call, _) = payload_loan(machine);
    let (ours, theirs) = (fixture.left.cases[0].id, fixture.right.cases[0].id);
    let retarget = |machine: &mut TerminalMachine| {
        let [step, _] = payload_loan_mut(machine).path.as_mut_slice() else {
            panic!("the loan is a case step and a field step")
        };
        assert_eq!(*step, StructuralPathSegment::Case(ours));
        *step = StructuralPathSegment::Case(theirs);
    };
    for unframed in [false, true] {
        let verdict = fixture.judged(|lowered| {
            let machine = &mut lowered.module.machines[position];
            if unframed {
                omit_write_frame(machine);
            }
            retarget(machine);
        });
        let Verdict::Unserved(refusal) = verdict else {
            panic!("the changed loan must be refused: {verdict:?}")
        };
        assert_eq!(refusal.reason, RefusalReason::Unsupported, "{refusal:?}");
        assert_eq!(
            refusal.site,
            RefusalSite::Machine(machine.id, Construct::Operation(call))
        );
        assert_eq!(
            refusal.detail,
            "a storage path that is not a path of fields and cases in machine `lend`"
        );
    }
}

/// Recorded in `OWNER_CODE_CONCERNS.md` ("A wrong case in a dispatch is
/// reported as a write-frame mismatch in the first machine"). The wrong edge
/// is in `other`, and the refusal's site and the name at the end of its text
/// are `choose`, the first machine of the module. Whoever repairs that
/// changes this test to expect `other`.
#[test]
fn a_write_frame_refusal_is_reported_at_the_first_machine() {
    let fixture = Fixture::new();
    let (choose, other) = (fixture.machine("choose").id, fixture.machine("other").id);
    assert_ne!(choose, other);
    let position = fixture.position("other");
    let (ours, theirs) = (fixture.right.cases[0].id, fixture.left.cases[0].id);
    let refusal = fixture
        .judged(|lowered| reselect(&mut lowered.module.machines[position], ours, theirs))
        .refused();
    assert_eq!(in_machine(&refusal), (choose, Construct::Machine));
    assert_eq!(
        refusal.detail,
        format!("published write frame for {other:?} differs from its body in machine `choose`")
    );
}
