use super::*;

/// Machine 2 as written, with a consumer that requires `x <= limit` and an
/// entry that passes what machine 2 returns for its own formal to it.
fn through(helper: Machine, limit: u128, entry_requires: Vec<Proposition>) -> Vec<Machine> {
    let consumer = Machine {
        requires: vec![at_most(1, limit)],
        ..machine(vec![u32_type()], vec![returns(slot(0, u32_type()))])
    };
    let entry = Machine {
        requires: entry_requires,
        ..machine(
            vec![u32_type()],
            vec![returns(call(3, vec![call(2, vec![slot(0, u32_type())])]))],
        )
    };
    vec![entry, helper, consumer]
}

fn identity(requires: Vec<Proposition>) -> Machine {
    Machine {
        requires,
        ..machine(vec![u32_type()], vec![returns(slot(0, u32_type()))])
    }
}

fn with_local_results(machines: Vec<Machine>, local: &[u64]) -> structure_elaboration::Elaboration {
    let mut elaboration = lower(machines).unwrap();
    elaboration.local_results = local.iter().copied().map(id).collect();
    elaboration
}

fn accepted(machines: Vec<Machine>, local: &[u64]) -> crate::AcceptedClosure {
    match judge(Ok(with_local_results(machines, local))).unwrap() {
        RouteVerdict::Accepted(accepted) => *accepted,
        verdict => panic!("the program must be accepted: {verdict:?}"),
    }
}

/// A private helper returns its bounded input and writes no `ensures`. Its
/// caller may pass the result where that bound is required, and every
/// certificate involved, the summary's own included, is the kernel's.
#[test]
fn a_caller_uses_what_a_private_helper_returns() {
    let accepted = accepted(through(identity(vec![at_most(1, 10)]), 10, vec![at_most(1, 5)]), &[2]);
    let helper = &accepted.elaboration.module.machines[1];
    assert_eq!(helper.contract.ensures.len(), 1);
    assert!(!verdicts(&accepted.elaboration, &accepted.discharge, 2, guarantee).is_empty());
    assert!(!verdicts(&accepted.elaboration, &accepted.discharge, 2, published).is_empty());
    for (machine, result) in accepted.discharge.machines.iter() {
        for (obligation, verdict) in result.verdicts.iter() {
            if let Ok(Discharged::Certificate(evidence)) = verdict {
                assert_kernel_accepted(&accepted.elaboration, machine.get(), obligation, evidence);
            }
        }
    }
}

/// The same program when the helper is not one whose body its callers may
/// read (a public machine, a dependency, a requirement): only its written
/// contract crosses, and that says nothing of the result.
#[test]
fn a_boundary_machine_keeps_its_written_contract() {
    let machines = through(identity(vec![at_most(1, 10)]), 10, vec![at_most(1, 5)]);
    let unproved = rejected(machines);
    assert!(
        unproved
            .iter()
            .any(|row| row.machine == id(1) && requirement(&row.kind))
    );
}

/// A summary states what the body returns and no more: an unbounded input
/// returned as it is does not meet a bound, summarized or not.
#[test]
fn a_summary_does_not_bound_an_unbounded_result() {
    let RouteVerdict::Rejected { unproved, .. } = judge(Ok(with_local_results(
        through(identity(Vec::new()), 10, Vec::new()),
        &[2],
    )))
    .unwrap() else {
        panic!("nothing bounds the value passed on")
    };
    assert!(
        unproved
            .iter()
            .any(|row| row.machine == id(1) && requirement(&row.kind))
    );
}

/// Two normal returns: the input when it is at most 60, and 40 otherwise.
/// The summary is what both establish, a result of at most 60, and is no
/// alternative per return.
#[test]
fn returns_that_differ_are_summarized_by_what_all_establish() {
    let helper = machine(
        vec![u32_type()],
        vec![
            guarded(
                compare(Strict::LessOrEqual, slot(0, u32_type()), unsigned(60)),
                slot(0, u32_type()),
            ),
            returns(unsigned(40)),
        ],
    );
    let accepted = accepted(through(helper, 60, Vec::new()), &[2]);
    let summary = &accepted.elaboration.module.machines[1].contract.ensures[0];
    assert!(matches!(&summary.proposition, Proposition::LessOrEqual(..)));
    assert_eq!(
        verdicts(&accepted.elaboration, &accepted.discharge, 2, guarantee).len(),
        2
    );
}

/// A result that is computed, not copied, has no equation to cross with:
/// what crosses is the bound its definition gives it.
#[test]
fn a_computed_result_crosses_as_its_bounds() {
    let successor = || Machine {
        requires: vec![at_most(1, 10)],
        ..machine(
            vec![u32_type()],
            vec![returns(strict(
                Strict::Add,
                slot(0, u32_type()),
                unsigned(1),
            ))],
        )
    };
    accepted(through(successor(), 11, vec![at_most(1, 10)]), &[2]);
    // One less than what the body can return is not established.
    let RouteVerdict::Rejected { unproved, .. } = judge(Ok(with_local_results(
        through(successor(), 10, vec![at_most(1, 10)]),
        &[2],
    )))
    .unwrap() else {
        panic!("the successor of 10 is 11")
    };
    assert!(
        unproved
            .iter()
            .any(|row| row.machine == id(1) && requirement(&row.kind))
    );
}

/// A helper's summary uses the summary of the helper it calls.
#[test]
fn summaries_compose_callees_first() {
    let forward = Machine {
        requires: vec![at_most(1, 10)],
        ..machine(
            vec![u32_type()],
            vec![returns(call(4, vec![slot(0, u32_type())]))],
        )
    };
    let mut machines = through(forward, 10, vec![at_most(1, 5)]);
    machines.push(identity(vec![at_most(1, 10)]));
    let accepted = accepted(machines, &[2, 4]);
    for helper in [1, 3] {
        assert_eq!(
            accepted.elaboration.module.machines[helper]
                .contract
                .ensures
                .len(),
            1
        );
    }
}
