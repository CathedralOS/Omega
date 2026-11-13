use super::*;
use semantic_vocabulary::ServiceId;

fn service(position: u64) -> ServiceId {
    ServiceId::new(position).unwrap()
}

/// An entry that returns what machine 2 returns for its formal. The module
/// declares `Mark` and `Forbidden`, in that order. `rows` are what each
/// machine publishes and `ceilings` what each header wrote; each test states
/// them itself and leaves the lowered calls alone.
fn calling(
    rows: [Vec<ServiceId>; 2],
    ceilings: [Option<Vec<ServiceId>>; 2],
) -> structure_elaboration::Elaboration {
    let helper = machine(vec![u32_type()], vec![returns(slot(0, u32_type()))]);
    let entry = machine(
        vec![u32_type()],
        vec![returns(call(2, vec![slot(0, u32_type())]))],
    );
    let mut elaboration = lower(vec![entry, helper]).unwrap();
    let mut identities = std::mem::take(&mut elaboration.module.identities).into_builder();
    for (position, name) in [(1, "Forbidden"), (2, "Mark")] {
        elaboration
            .module
            .services
            .push(terminal_psi::ServiceDeclaration {
                id: service(position),
                identity: identities.intern(name),
                parents: Vec::new(),
            });
        elaboration.reach.names.push(name.to_owned());
    }
    elaboration.module.identities = identities.build();
    elaboration.module.root_service_reach.concrete = rows[0].clone();
    for ((machine, row), ceiling) in elaboration
        .module
        .machines
        .iter_mut()
        .zip(rows)
        .zip(ceilings)
    {
        machine.published_service_ceiling = row;
        machine.declared_service_reach = ceiling.clone().unwrap_or_default();
        elaboration.reach.ceilings.push(ceiling);
    }
    elaboration
}

/// `reaches Forbidden + Mark` over a call that contributes `Mark`: a ceiling
/// is an upper bound, and slack in it is no error.
#[test]
fn a_call_inside_the_written_ceiling_is_accepted() {
    let both = vec![service(1), service(2)];
    let verdict = judge(Ok(calling(
        [vec![service(2)], vec![service(2)]],
        [Some(both), None],
    )))
    .unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

/// `reaches Mark` over a call that contributes `Forbidden`: rejected at the
/// call, naming the ceiling and what the call adds to it.
#[test]
fn a_call_beyond_the_written_ceiling_is_rejected_at_the_call() {
    let verdict = judge(Ok(calling(
        [vec![service(2)], vec![service(1)]],
        [Some(vec![service(2)]), None],
    )))
    .unwrap();
    let RouteVerdict::Rejected { exceeded, .. } = verdict else {
        panic!("the call must be rejected: {verdict:?}");
    };
    let [call] = exceeded.as_slice() else {
        panic!("one call exceeds the ceiling: {exceeded:?}");
    };
    assert_eq!(call.machine, id(1));
    assert_eq!(call.callee, id(2));
    assert_eq!(call.undeclared, vec![service(1)]);
    assert!(
        call.message().contains(
            "publishes service reach `Mark` but its checked body reaches undeclared service \
             `Forbidden`"
        ),
        "{}",
        call.message()
    );
}

/// `reaches none` is a ceiling of no service, not an omitted clause.
#[test]
fn an_empty_ceiling_rejects_any_contributed_service() {
    let verdict = judge(Ok(calling(
        [Vec::new(), vec![service(2)]],
        [Some(Vec::new()), None],
    )))
    .unwrap();
    let RouteVerdict::Rejected { exceeded, .. } = verdict else {
        panic!("the call must be rejected: {verdict:?}");
    };
    assert_eq!(exceeded.len(), 1);
    assert!(exceeded[0].message().contains("service reach `<none>`"));
}

/// A caller that wrote no ceiling and publishes less than its callee does
/// would hide that reach from its own callers. Only the row changed, so
/// this is a fault of the tables and never an acceptance.
#[test]
fn a_published_row_that_omits_a_callees_reach_is_never_accepted() {
    let refused = judge(Ok(calling([Vec::new(), vec![service(2)]], [None, None]))).unwrap_err();
    assert_eq!(refused.reason, RefusalReason::ForeignIdentity);
}

/// A row out of order or naming a service the module does not declare is
/// refused before any call is read.
#[test]
fn a_malformed_row_is_never_accepted() {
    let unordered = vec![service(2), service(1)];
    let refused = judge(Ok(calling([unordered.clone(), unordered], [None, None]))).unwrap_err();
    assert_eq!(refused.reason, RefusalReason::ForeignIdentity);
    let unknown = vec![service(3)];
    let refused = judge(Ok(calling([unknown.clone(), unknown], [None, None]))).unwrap_err();
    assert_eq!(refused.reason, RefusalReason::ForeignIdentity);
}

/// A machine that publishes more than the ceiling it wrote is a fault, even
/// when no call of its own contributes the excess.
#[test]
fn a_published_row_beyond_the_written_ceiling_is_never_accepted() {
    let refused = judge(Ok(calling(
        [vec![service(1)], Vec::new()],
        [Some(Vec::new()), None],
    )))
    .unwrap_err();
    assert_eq!(refused.reason, RefusalReason::ForeignIdentity);
}

/// A declared reach that is not the ceiling the header wrote is a fault.
#[test]
fn a_declared_reach_that_is_not_the_ceiling_is_never_accepted() {
    let mut elaboration = calling([Vec::new(), Vec::new()], [Some(vec![service(2)]), None]);
    elaboration.module.machines[0].declared_service_reach = vec![service(1)];
    let refused = judge(Ok(elaboration)).unwrap_err();
    assert_eq!(refused.reason, RefusalReason::ForeignIdentity);
}
