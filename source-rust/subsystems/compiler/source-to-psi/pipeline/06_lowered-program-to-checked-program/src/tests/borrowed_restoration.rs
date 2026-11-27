//! An owned value moved out of storage the machine was lent with `@mutable`.
//!
//! ownership.md, "Borrowed-storage invariant windows": "An owned move out of
//! a place reached through `@mutable` may temporarily leave that place
//! unestablished ... Assignment of a valid owned replacement to the exact
//! vacant place closes the ownership window ... Every return or terminal
//! transfer from the region owing restoration ... must close the window."
//!
//! Programs remove an affine record or projected field from exclusive incoming
//! or owned storage, directly or through retained exclusive references. Accepted
//! paths restore that exact storage before handback; each failing twin breaks only
//! the store, observation or loan obligation it challenges.

use super::*;

const DATA: &str = "data Inner { value: u32; }\ndata Holder { inner: Inner; tag: u32; }\n";

fn verdict(body: &str) -> Result<Vec<String>, String> {
    let text = format!("{DATA}{body}");
    match super::entryless::judged(&text, false) {
        RouteVerdict::Rejected {
            violations,
            unestablished,
            unproved,
            conflicts,
            ..
        } => Ok(violations
            .iter()
            .map(|violation| violation.statement.clone())
            .chain(unestablished.iter().map(|used| used.detail.clone()))
            .chain(unproved.iter().map(|owed| format!("unproved: {owed:?}")))
            .chain(conflicts.iter().map(|loan| format!("conflict: {loan:?}")))
            .collect()),
        RouteVerdict::Accepted(_) => Err("accepted".into()),
        RouteVerdict::Incomplete(_) => Err("incomplete".into()),
        RouteVerdict::FallBack(refusal) => Err(format!("not supported: {}", refusal.detail)),
        RouteVerdict::Laws(_) | RouteVerdict::UnprovedLaws(_) => {
            Err("a mathematical signature".into())
        }
    }
}

fn accepted(body: &str) {
    assert_eq!(verdict(body).unwrap_err(), "accepted", "{body}");
}

fn rejected(body: &str, rule: &str) {
    let violations = verdict(body).unwrap_or_else(|other| panic!("{other}: {body}"));
    assert!(
        violations.iter().any(|violation| violation.contains(rule)),
        "{violations:?}"
    );
}

const NOT_RESTORED: &str =
    "the place does not hold a value again where the machine returns the borrow";
const USED_VACANT: &str = "owned storage is used after its value was transferred or consumed";

#[test]
fn a_whole_local_owned_home_is_repaired_before_its_reference_ends() {
    let program = |during: &str, repair: &str| {
        format!(
            "machine take(value: Inner, next: Inner) -> Inner {{\n\
             let mutable owner: Inner = value;\n\
             let local: @mutable Inner = @mutable owner;\n\
             let taken: Inner = move local;\n\
             {during}\n\
             {repair}\n\
             taken\n}}\n"
        )
    };
    accepted(&program("", "local = next;"));
    rejected(&program("", ""), NOT_RESTORED);
    rejected(
        &program("let again: Inner = move local;", "local = next;"),
        USED_VACANT,
    );
    rejected(
        &program("let peek: u32 = owner.value;", "local = next;"),
        USED_VACANT,
    );
}

fn missing_whole_local_repair() -> terminal_check_tables::program::Elaboration {
    let source = format!(
        "{DATA}machine take(value: Inner, next: Inner) -> Inner {{\n\
         let mutable owner: Inner = value;\n\
         let local: @mutable Inner = @mutable owner;\n\
         let taken: Inner = move local;\n\
         taken\n}}\n"
    );
    let typed = super::boundary_calls::typed(&source);
    crate::lowering::elaborate_for(
        &typed,
        crate::lowering::Compilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &[],
            subjects: &[None],
            purpose: source_provenance::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
    .expect("lowering must retain the missing repair for the receiving checker")
}

#[test]
fn claim_verification_preserves_the_exact_unavailable_storage() {
    use terminal_check_tables::program::SourceOrigin as TerminalSourceOrigin;
    use terminal_semantics::plain_affine::{Error, Site, UnavailableReason};

    let mut elaboration = missing_whole_local_repair();
    let machine = elaboration.module.machines[0].id;
    let plan = terminal_semantics::plain_affine::verify_in(&elaboration.module, machine)
        .expect("the supplied cleanup schedule must still be verified");
    let expected = plan
        .unavailable
        .first()
        .expect("the whole local home is still owed back when its reference ends");
    assert_eq!(expected.reason, UnavailableReason::Unrestored);
    let error = terminal_semantics::plain_affine::verify_claims_in(&elaboration.module, machine)
        .unwrap_err();
    assert_eq!(error.site(), Some(expected.site));
    let Error::Unavailable(actual) = error else {
        panic!("claim verification must retain its exact storage error: {error:?}");
    };
    assert_eq!(actual.site, expected.site);
    assert_eq!(actual.place, expected.place);
    assert_eq!(actual.reason, expected.reason);
    assert!(actual.message().contains(NOT_RESTORED));

    let concerns = match actual.site {
        Site::Operation(operation) => Construct::Operation(operation),
        Site::Edge(edge) => Construct::Edge(edge),
    };
    let sources = elaboration.sources.get(machine).unwrap();
    let origin = match actual.site {
        Site::Operation(operation) => sources.operations.get(operation),
        Site::Edge(edge) => sources.edges.get(edge),
    };
    let expected_span = origin
        .copied()
        .and_then(TerminalSourceOrigin::authored_span)
        .or_else(|| {
            elaboration
                .names
                .get(machine)
                .and_then(|names| names.declared)
        })
        .expect("the whole-local program retains authored source correspondence");
    let RouteVerdict::Rejected { unestablished, .. } = judge(Ok(elaboration.clone())).unwrap()
    else {
        panic!("the identified missing repair must be an ordinary source rejection");
    };
    let used = unestablished
        .iter()
        .find(|used| used.machine == machine && used.at == concerns)
        .expect("the source rejection must retain the exact unavailable construct");
    assert_eq!(
        used.site,
        Some(TerminalSourceOrigin::Authored(expected_span))
    );
    assert!(used.detail.contains(actual.message()));

    // Direct proof preparation must refuse too, rather than pass the debt
    // onward as if it were a completed claim-transport check.
    let refused =
        crate::discharge(&mut elaboration, &mut Default::default()).unwrap_err();
    assert_eq!(refused.reason, RefusalReason::Unestablished);
    assert_eq!(refused.detail, actual.message());
    assert_eq!(refused.site.in_machine().unwrap().1, concerns);
}

#[test]
fn malformed_return_claims_are_refused_before_unrestored_storage() {
    use terminal_semantics::plain_affine::{Error, Site};

    let mut elaboration = missing_whole_local_repair();
    let machine = elaboration.module.machines[0].id;
    assert!(matches!(
        terminal_semantics::plain_affine::verify_claims_in(&elaboration.module, machine),
        Err(Error::Unavailable(_))
    ));
    let (edge, returned_claims) = elaboration.module.machines[0]
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            Terminator::ReturnStructural {
                edge,
                returned_claims,
                ..
            } => Some((*edge, returned_claims)),
            _ => None,
        })
        .expect("the original missing-repair twin returns the detached value");
    assert!(returned_claims.is_empty());
    returned_claims.push(terminal_psi::StructuralResultClaimBinding {
        claim: id(u64::MAX),
        path: Vec::new(),
    });
    let error = terminal_semantics::plain_affine::verify_claims_in(&elaboration.module, machine)
        .unwrap_err();
    assert_eq!(error.site(), Some(Site::Edge(edge)));
    assert!(matches!(
        error,
        Error::Malformed {
            detail: "return claims differ from actual returned custody",
            ..
        }
    ));
    let refused = judge(Ok(elaboration)).unwrap_err();
    assert_eq!(refused.reason, RefusalReason::ForeignIdentity);
    assert!(
        refused
            .detail
            .contains("return claims differ from actual returned custody")
    );
}

#[test]
fn a_whole_local_repair_must_fill_the_captured_home() {
    rejected(
        "machine take(value: Inner, other: Inner, next: Inner) -> Inner {\n\
         let mutable owner: Inner = value;\n\
         let mutable unrelated: Inner = other;\n\
         let local: @mutable Inner = @mutable owner;\n\
         let taken: Inner = move $local;\n\
         unrelated = next;\n\
         taken\n}\n",
        NOT_RESTORED,
    );
}

#[test]
fn a_whole_owned_home_remains_captured_when_its_reference_moves() {
    let program = |repair: &str| {
        format!(
            "machine take(value: Inner, next: Inner) -> Inner {{\n\
             let mutable owner: Inner = value;\n\
             let local: @mutable Inner = @mutable owner;\n\
             let taken: Inner = move local;\n\
             let retained: @mutable Inner = move local;\n\
             {repair}\n\
             taken\n}}\n"
        )
    };
    accepted(&program("retained = next;"));
    rejected(&program(""), NOT_RESTORED);
}

#[test]
fn ordinary_whole_owned_transfer_does_not_owe_borrowed_storage_repair() {
    accepted(
        "machine take(value: Inner) -> Inner {\n\
         let mutable owner: Inner = value;\n\
         let taken: Inner = move owner;\n\
         taken\n}\n",
    );
}

#[test]
fn a_whole_qualified_owned_origin_remains_explicitly_unsupported() {
    let source = "domain Inner::Stamped established by stamp;\n\
         machine stamp(value: Inner) -> Inner in Stamped { value as +Stamped }\n\
         machine take(value: Inner in Stamped, next: Inner in Stamped) -> Inner in Stamped {\n\
         let mutable owner: Inner in Stamped = value;\n\
         let local: @mutable Inner in Stamped = @mutable owner;\n\
         let taken: Inner in Stamped = move local;\n\
         $local = next;\n\
         taken\n}\n";
    let refusal = verdict(source).unwrap_err();
    assert!(
        refusal.contains(
            "whole-referent restoration requires unqualified nonlinear owned storage without claims"
        ),
        "{refusal}"
    );
}

#[test]
fn a_local_owned_field_is_restored_before_its_reference_is_released() {
    let program = |restore: &str| {
        format!(
            "machine take(value: Holder, next: Inner) -> Inner {{\n\
         let mutable owner: Holder = value;\n\
         let local: @mutable Holder = @mutable owner;\n\
         let taken: Inner = move local.inner;\n\
         {restore}\n\
         taken\n}}\n"
        )
    };
    accepted(&program("local.inner = next;"));
    // No later observation of owner can stand in for the loan's repair debt:
    // discarding its incomplete owned complement must still reject.
    rejected(&program(""), NOT_RESTORED);
    rejected(&program("local.tag = 2;"), NOT_RESTORED);
}

#[test]
fn an_ordinary_owned_partial_move_owes_no_borrowed_storage_repair() {
    accepted(
        "machine take(value: Holder) -> Inner {\n\
         let mutable owner: Holder = value;\n\
         let taken: Inner = move owner.inner;\n\
         taken\n}\n",
    );
}

#[test]
fn a_local_window_restores_only_the_storage_its_reference_captured() {
    let program = |restore: &str| {
        format!(
            "data Separate {{ lent: Holder; other: Inner; }}\n\
         machine take(value: Separate, next: Inner) -> Inner {{\n\
         let mutable owner: Separate = value;\n\
         let local: @mutable Holder = @mutable owner.lent;\n\
         let taken: Inner = move local.inner;\n\
         let detached: Inner = move owner.other;\n\
         {restore}\n\
         taken\n}}\n"
        )
    };
    accepted(&program("local.inner = next;"));
    rejected(&program(""), NOT_RESTORED);
}

#[test]
fn a_dead_shared_reference_is_released_before_parent_write_on_a_crashing_path() {
    let program = |before: &str, after: &str| {
        format!(
            "machine observe(value: @u32) {{ }}\n\
         machine stop(owner: @mutable Inner) crashes Trap {{\n\
         let held: @u32 = @owner.value;\n\
         {before}\n\
         owner.value = 7;\n\
         {after}\n\
         crash Trap;\n}}\n"
        )
    };
    accepted(&program("observe(held);", ""));
    rejected(&program("", "observe(held);"), "conflict:");
}

#[test]
fn an_unselected_sum_payload_is_never_released_as_a_live_reference() {
    let program = format!(
        "{DATA}data Either<'storage> {{\n\
         case Left(item: @'storage Inner);\n\
         case Right(item: @'storage Inner);\n\
         }}\n\
         machine idle(value: Either, flag: bool) -> bool {{ !flag }}\n"
    );
    let outcome = super::entryless::judged(&program, true);
    let RouteVerdict::Accepted(accepted) = outcome else {
        panic!("an arbitrary incoming case must keep guarded cleanup: {outcome:?}");
    };
    assert!(
        accepted.elaboration.module.machines[0]
            .operations()
            .all(|operation| { !matches!(operation.kind, OperationKind::ReleaseReference { .. }) })
    );
}

#[test]
fn an_owned_field_window_keeps_its_loan_after_the_carrier_moves() {
    let program = |restore: &str| {
        format!(
            "machine take(value: Holder, next: Inner) -> Inner {{\n\
         let mutable owner: Holder = value;\n\
         let local: @mutable Holder = @mutable owner;\n\
         let taken: Inner = move local.inner;\n\
         let repair: @mutable Holder = move local;\n\
         {restore}\n\
         taken\n}}\n"
        )
    };
    accepted(&program("repair.inner = next;"));
    rejected(&program(""), NOT_RESTORED);
}

/// A replacement is evaluated before the overwritten field gives up its
/// permission. Disposing a dead carrier earlier would make this second
/// exclusive borrow appear valid while the first still occupies the field.
#[test]
fn a_field_replacement_keeps_the_old_loan_during_rhs_evaluation() {
    rejected(
        "data Held<'storage> { item: @'storage mutable Inner; }\n\
         machine replace(owner: @mutable Inner) {\n\
             let mutable held: Held = Held { item: @mutable owner };\n\
             held.item = @mutable owner;\n\
         }\n",
        "TwiceExclusive",
    );
}

#[test]
fn a_last_reference_use_does_not_authorize_parent_use_before_actual_release() {
    let source = format!(
        "{DATA}machine read(value: Holder, next: Inner) -> u32 {{\n\
         let mutable owner: Holder = value;\n\
         let local: @mutable Holder = @mutable owner;\n\
         let taken: Inner = move local.inner;\n\
         local.inner = next;\n\
         owner.tag\n}}\n"
    );
    assert!(
        matches!(
            super::entryless::judged(&source, false),
            RouteVerdict::Accepted(_)
        ),
        "the original restored source must be accepted"
    );
    // Re-lower the source so receiving analysis owns fresh knowledge rows;
    // its accepted output is not an unjudged input to the same stage.
    let typed = super::boundary_calls::typed(&source);
    let mut elaboration = crate::lowering::elaborate_for(
        &typed,
        crate::lowering::Compilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &[],
            subjects: &[None],
            purpose: source_provenance::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
    let block = elaboration.module.machines[0]
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .operations
                .iter()
                .any(|operation| matches!(operation.kind, OperationKind::ReleaseReference { .. }))
        })
        .expect("the source contains an actual release");
    let observation = block
        .operations
        .iter()
        .rposition(|operation| {
            matches!(
                operation.kind,
                OperationKind::ScalarStructuralField { .. }
                    | OperationKind::PrimitiveScalarRead { .. }
            )
        })
        .expect("the source observes its parent's scalar field");
    let release = block.operations[..observation]
        .iter()
        .rposition(|operation| matches!(operation.kind, OperationKind::ReleaseReference { .. }))
        .expect("the parent observation follows its final reference release");
    // Keep the same operations, custody and final cleanup, but move the
    // release after the parent observation. Future-use liveness is identical
    // at that observation; actual permission custody is not.
    let delayed = block.operations.remove(release);
    block.operations.insert(observation, delayed);
    elaboration.module.execution.unoptimized_behavior = elaboration
        .module
        .machines
        .iter()
        .map(terminal_psi::UnoptimizedMachineBehavior::capture)
        .collect();
    let outcome = judge(Ok(elaboration)).unwrap();
    assert!(
        matches!(outcome, RouteVerdict::Rejected { ref conflicts, .. }
            if conflicts.iter().any(|conflict|
                matches!(conflict.conflict, crate::LoanConflict::ObserveWhileExclusive))),
        "{outcome:?}"
    );
}

/// The window is opened by the move and closed by the store into the same
/// field; the twin never stores, so the caller would get its `Holder` back
/// with nothing in `inner`.
#[test]
fn a_moved_field_must_hold_a_value_again_when_the_machine_returns() {
    let program = |restore: &str| {
        format!(
            "machine swap(holder: @mutable Holder, next: Inner) -> Inner {{\n    \
             let taken: Inner = holder.inner;\n    {restore}\n    taken\n}}\n"
        )
    };
    accepted(&program("holder.inner = next;"));
    rejected(&program("holder.tag = 1;"), NOT_RESTORED);
}

/// "Ancestors are unavailable for ... transfer": a transition hands the lent
/// `Holder` to another state, so the window must be closed before it. The
/// twin transfers the `Holder` with its field still vacant, meaning to
/// store in the state it enters.
#[test]
fn the_lent_value_is_not_handed_to_another_state_while_a_field_is_vacant() {
    let program = |before: &str, after: &str| {
        format!(
            "machine swap(holder: @mutable Holder, next: Inner, last: Inner, go: bool) {{\n    \
             let taken: Inner = holder.inner;\n    {before}\n    \
             transition go {{\n        true -> finish(holder, last),\n        _ -> finish(holder, last)\n    }}\n    \
             machine finish(holder: @mutable Holder, last: Inner) {{\n        {after}\n    }}\n}}\n"
        )
    };
    accepted(&program("holder.inner = next;", "holder.inner = last;"));
    rejected(
        &program("holder.tag = 2;", "holder.inner = last;"),
        USED_VACANT,
    );
}

/// "While the window is open, the absent subtree cannot be read ... or
/// moved again": the twin moves the field a second time before anything was
/// stored into it. A sibling field stays usable ("Disjoint established
/// siblings remain usable").
#[test]
fn a_vacant_field_is_not_read_before_it_is_restored() {
    let program = |between: &str| {
        format!(
            "machine swap(holder: @mutable Holder, next: Inner) -> Inner {{\n    \
             let taken: Inner = holder.inner;\n    {between}\n    \
             holder.inner = next;\n    taken\n}}\n"
        )
    };
    accepted(&program("holder.tag = 3;"));
    rejected(&program("let again: Inner = holder.inner;"), USED_VACANT);
}

/// "Ancestors are unavailable for whole-value observation, calls, transfer":
/// the twin lends the whole `Holder` on while its field is vacant; the
/// accepted program lends it after the store.
#[test]
fn the_lent_value_is_not_passed_on_while_a_field_is_vacant() {
    let program = |first: &str, second: &str| {
        format!(
            "machine observe(holder: @Holder) {{ }}\n\
             machine swap(holder: @mutable Holder, next: Inner) -> Inner {{\n    \
             let taken: Inner = holder.inner;\n    {first}\n    {second}\n    taken\n}}\n"
        )
    };
    accepted(&program("holder.inner = next;", "observe(holder);"));
    rejected(
        &program("observe(holder);", "holder.inner = next;"),
        USED_VACANT,
    );
}

use terminal_semantics::plain_affine::CallFence;

const REACHES_SERVICE: &str = CallFence::ServiceReach.message();
const ENTERS_CALLBACK: &str = CallFence::Callback.message();
const UNKNOWN_ACCESS: &str = CallFence::UnknownAccess.message();

/// `Clock::tick` declares `reaches none`; `Clock::sample` reaches the
/// `Clock` service itself, so a provider may hold its authority.
const CLOCK: &str = "public boundary trait Clock {\n    \
                     machine tick() reaches none crashes none;\n    \
                     machine sample() reaches Clock crashes none;\n}\n\
                     machine helper() reaches Clock { Clock::tick(); }\n\
                     machine relay() { helper(); }\n\
                     machine observe(value: @u32) { }\n";

/// The same `swap` with two statements in either order: `holder.inner =
/// next;` closes the window, so whatever runs first runs with it open.
fn window_call(first: &str, second: &str) -> String {
    format!(
        "{CLOCK}machine swap(holder: @mutable Holder, next: Inner) -> Inner {{\n    \
         let taken: Inner = holder.inner;\n    {first}\n    {second}\n    taken\n}}\n"
    )
}

/// ownership.md, "Boundary calls during restoration": reject the call "if
/// a carried reference, callback or provider authority could expose the
/// incomplete connection". A helper that reaches the `Clock` service runs
/// after the store in the accepted program and before it in the twin.
#[test]
fn a_call_that_reaches_a_service_waits_until_the_window_closes() {
    accepted(&window_call("holder.inner = next;", "helper();"));
    rejected(
        &window_call("helper();", "holder.inner = next;"),
        REACHES_SERVICE,
    );
}

/// A boundary requirement whose contract reaches no service and enters no
/// callback may run inside the window; one that reaches the `Clock`
/// service waits for the store. Both twins call at the same position.
#[test]
fn a_boundary_call_in_the_window_is_judged_by_its_declared_reach() {
    accepted(&window_call("Clock::tick();", "holder.inner = next;"));
    rejected(
        &window_call("Clock::sample();", "holder.inner = next;"),
        REACHES_SERVICE,
    );
    accepted(&window_call("holder.inner = next;", "Clock::sample();"));
}

/// `relay` declares no reach and makes no boundary call itself; it is
/// fenced because the helper it calls reaches a service. A call to a quiet
/// machine that borrows a disjoint established sibling stays usable inside
/// the window ("Disjoint established siblings remain usable").
#[test]
fn a_call_is_fenced_by_what_its_callees_reach() {
    rejected(
        &window_call("relay();", "holder.inner = next;"),
        REACHES_SERVICE,
    );
    accepted(&window_call(
        "observe(@holder.tag);",
        "holder.inner = next;",
    ));
}

/// A local `@mutable` reference to an owned local opens the same window in
/// the storage it captured, and the same fence holds there.
#[test]
fn a_window_through_a_local_reference_fences_the_same_calls() {
    let program = |first: &str, second: &str| {
        format!(
            "{CLOCK}machine keep() -> Inner {{\n    \
             let mutable holder: Holder = Holder {{ inner: Inner {{ value: 3u32 }}, tag: 1u32 }};\n    \
             let local: @mutable Holder = @mutable holder;\n    \
             let taken: Inner = move local.inner;\n    {first}\n    {second}\n    taken\n}}\n"
        )
    };
    let store = "local.inner = Inner { value: 7u32 };";
    accepted(&program(store, "helper();"));
    rejected(&program("helper();", store), REACHES_SERVICE);
}

/// `bump` moves `holder.inner` into `step` and stores the result back, so
/// the call runs with the field vacant.
fn bump(step: &str) -> String {
    format!(
        "{CLOCK}machine step(inner: Inner) -> Inner {step}\n\
         machine bump(holder: @mutable Holder) {{\n    holder.inner = step(holder.inner);\n}}\n"
    )
}

/// dependent_values.md, "Invariant windows": a call need not close the
/// window "when its complete access, effect and outcome contracts prove
/// that it cannot observe or expose the broken coupling". A `step` that
/// reaches nothing may take the detached value and return its replacement;
/// the twin's `step` reaches the `Clock` service.
#[test]
fn a_quiet_call_may_consume_the_detached_value_and_return_its_replacement() {
    accepted(&bump("{ inner }"));
    rejected(
        &bump("reaches Clock { Clock::tick(); inner }"),
        REACHES_SERVICE,
    );
}

/// The provider requirements a window test may call with the field it
/// detached. Each machine differs from `step` in one clause of its contract
/// or in its parameter; `Step` is a machine value with `step`'s contract.
const SESSION: &str = "boundary trait Handler {}\n\
                       public boundary trait Session {\n    \
                       machine step(inner: Inner) -> Inner reaches none crashes none;\n    \
                       machine wait(inner: Inner) -> Inner reaches none crashes none blocks;\n    \
                       machine enter(inner: Inner) -> Inner reaches none crashes none invokes Handler;\n    \
                       machine sample(inner: Inner) -> Inner reaches Session crashes none;\n    \
                       machine peek(holder: @Holder) reaches none crashes none;\n    \
                       machine count(tag: @u32) reaches none crashes none;\n}\n\
                       data Step = machine(inner: Inner) -> Inner reaches none crashes none;\n\
                       machine named(inner: Inner) -> Inner { inner }\n";

/// `update` moves `holder.inner` into `call` and stores what it returns, so
/// the call runs with the field vacant.
fn provider_update(call: &str) -> String {
    format!(
        "{SESSION}machine update(holder: @mutable Holder, chosen: Step) {{\n    \
         holder.inner = {call}(holder.inner);\n}}\n"
    )
}

/// ownership.md, "Boundary calls during restoration": "Admit the call when
/// it can access only the detached session and its outcomes permit
/// restoration", and "`blocks` alone permits waiting on the worker while the
/// caller retains exclusive custody". Both providers receive only the
/// detached field and return its replacement; the blocking twin's call
/// carries the `block` marker its contract asks for.
#[test]
fn a_provider_may_consume_only_the_detached_field() {
    accepted(&provider_update("Session::step"));
    accepted(&provider_update("block Session::wait"));
}

/// "Reject it if a carried reference, callback or provider authority could
/// expose the incomplete connection", and "Unknown access or outcomes
/// require refusal". Each twin differs from the accepted `Session::step` in
/// the callee alone: `enter` may run a `Handler` callback, `sample` reaches
/// the `Session` service, and the machine value `chosen` names no callee.
/// The named ordinary machine with the same contract is admitted.
#[test]
fn a_call_that_could_observe_the_incomplete_owner_is_refused() {
    rejected(&provider_update("Session::enter"), ENTERS_CALLBACK);
    rejected(&provider_update("Session::sample"), REACHES_SERVICE);
    rejected(&provider_update("chosen"), UNKNOWN_ACCESS);
    accepted(&provider_update("named"));
}

/// The arguments at the call site are judged with the contract. Lending the
/// incomplete holder is a use of the vacant storage; lending its established
/// sibling `tag` is not.
#[test]
fn a_provider_is_judged_by_the_storage_its_arguments_lend() {
    let program = |call: &str| {
        format!(
            "{SESSION}machine swap(holder: @mutable Holder, next: Inner) -> Inner {{\n    \
             let taken: Inner = holder.inner;\n    {call};\n    \
             holder.inner = next;\n    taken\n}}\n"
        )
    };
    accepted(&program("Session::count(@holder.tag)"));
    rejected(&program("Session::peek(@holder)"), USED_VACANT);
}

/// "every path that hands the storage back first restores it": the
/// admitted provider leaves the field vacant until the machine stores a
/// replacement, and the twin returns the borrow without one.
#[test]
fn an_admitted_provider_does_not_excuse_a_handback_before_repair() {
    let program = |repair: &str| {
        format!(
            "{SESSION}machine update(holder: @mutable Holder, spare: Inner) -> Inner {{\n    \
             let next: Inner = Session::step(holder.inner);\n    {repair}\n    next\n}}\n"
        )
    };
    accepted(&program("holder.inner = spare;"));
    rejected(&program(""), NOT_RESTORED);
}

const HALT: &str = "public data Empty requires false {}\n\
                    public machine halt() -> Empty reaches Clock crashes Trap { Clock::tick(); crash Trap; }\n\
                    machine stop() -> Empty crashes Trap { crash Trap; }\n";

/// A call to a machine whose result is empty never returns, so it ends its
/// block as an `InvokeNoNormal` terminator rather than a call operation. It
/// is fenced the same way: `halt` reaches the `Clock` service and waits for
/// the store, while `stop` reaches nothing and may run before it.
#[test]
fn a_call_that_never_returns_is_fenced_by_what_it_reaches() {
    let program = |first: &str, second: &str| {
        format!(
            "{CLOCK}{HALT}machine swap(holder: @mutable Holder, next: Inner) -> Inner crashes Trap {{\n    \
             let taken: Inner = holder.inner;\n    {first}\n    {second}\n    taken\n}}\n"
        )
    };
    let store = "holder.inner = next;";
    accepted(&program(store, "let stopped: Empty = halt();"));
    rejected(
        &program("let stopped: Empty = halt();", store),
        REACHES_SERVICE,
    );
    accepted(&program("let stopped: Empty = stop();", store));
}

fn accepted_elaboration(body: &str) -> terminal_check_tables::program::Elaboration {
    match super::entryless::judged(&format!("{DATA}{body}"), false) {
        RouteVerdict::Accepted(accepted) => accepted.elaboration,
        other => panic!("{other:?}: {body}"),
    }
}

fn machine_named(
    elaboration: &terminal_check_tables::program::Elaboration,
    name: &str,
) -> MachineId {
    elaboration
        .module
        .machines
        .iter()
        .map(|machine| machine.id)
        .find(|id| {
            elaboration
                .names
                .get(*id)
                .is_some_and(|names| names.machine == name)
        })
        .unwrap_or_else(|| panic!("no machine `{name}`"))
}

/// Adds `kind` to the end of `machine`'s first block. Source cannot spell a
/// port write in an ordinary helper or a computed call to an unknown
/// machine value, so these tests add one to an accepted module.
fn append(module: &mut terminal_psi::TerminalModule, machine: MachineId, kind: OperationKind) {
    let machine = module
        .machines
        .iter_mut()
        .find(|row| row.id == machine)
        .unwrap();
    let next = machine
        .operations()
        .map(|operation| operation.id.get())
        .max()
        .unwrap_or(0)
        + 1;
    machine.blocks[0]
        .operations
        .push(terminal_psi::Operation::unsuspended(
            semantic_vocabulary::OperationId::new(next).unwrap(),
            terminal_psi::OperationResult::Unit,
            kind,
        ));
}

/// The sites where the module-aware walk of `machine` fences a call, each
/// with the part of the callee's contract that fences it.
fn fenced_sites(
    module: &terminal_psi::TerminalModule,
    machine: MachineId,
) -> Vec<(terminal_semantics::plain_affine::Site, CallFence)> {
    terminal_semantics::plain_affine::verify_in(module, machine)
        .expect("the accepted walk still completes")
        .unavailable
        .into_iter()
        .filter_map(|row| match row.reason {
            terminal_semantics::plain_affine::UnavailableReason::FencedCall(fence) => {
                Some((row.site, fence))
            }
            _ => None,
        })
        .collect()
}

const TICKET: &str = "data Ticket dropped by stamp { id: u32; }\n\
                      machine stamp(ticket: Ticket) { let { id } = move ticket; }\n\
                      machine make() -> Ticket { Ticket { id: 1u32 } }\n";

/// A discarded `Ticket` is disposed by an owned cleanup call to `stamp` on
/// the edge after the discard. Source refuses an automatic cleanup that
/// reaches a service, so the test gives `stamp` a port write, an
/// instruction service. The walk must then fence the cleanup edge that runs
/// inside the window, and not the one that runs after the store.
#[test]
fn owned_cleanup_on_an_edge_is_fenced_by_what_its_cleanup_machine_reaches() {
    let program = |first: &str, second: &str| {
        format!(
            "{CLOCK}{TICKET}machine swap(holder: @mutable Holder, next: Inner) -> Inner {{\n    \
             let taken: Inner = holder.inner;\n    {first}\n    {second}\n    taken\n}}\n"
        )
    };
    let (store, discard) = ("holder.inner = next;", "_ = make();");
    for (first, second, inside) in [(discard, store, true), (store, discard, false)] {
        let elaboration = accepted_elaboration(&program(first, second));
        let (swap, stamp) = (
            machine_named(&elaboration, "swap"),
            machine_named(&elaboration, "stamp"),
        );
        let mut module = elaboration.module;
        assert!(fenced_sites(&module, swap).is_empty());
        append(
            &mut module,
            stamp,
            OperationKind::PortWrite {
                service: semantic_vocabulary::ServiceId::new(1).unwrap(),
                port: 0x80,
                value: 0,
            },
        );
        let sites = fenced_sites(&module, swap);
        if inside {
            assert!(
                matches!(
                    sites.as_slice(),
                    [(
                        terminal_semantics::plain_affine::Site::Edge(_),
                        CallFence::ServiceReach
                    )]
                ),
                "{sites:?}"
            );
        } else {
            assert!(sites.is_empty(), "{sites:?}");
        }
    }
}

/// Decoding a published module checks again the value availability of each
/// machine with an `EliminateEmpty` terminator, here the empty `Never` case
/// that the `match` leaves out. That check must judge the quiet `step` call
/// made while `holder.inner` is vacant with the module's own callee table,
/// as stage 06 did. A walk without the module refused this call as not
/// supported, so decoding refused the accepted module.
#[test]
fn decoding_judges_a_call_inside_the_window_by_its_module_callee() {
    let elaboration = accepted_elaboration(&format!(
        "{CLOCK}public data Empty requires false {{}}\n\
         data Pick {{ case Some(value: u32); case Never(value: Empty); }}\n\
         machine step(inner: Inner) -> Inner {{ inner }}\n\
         machine bump(holder: @mutable Holder, pick: Pick) -> u32 {{\n    \
         holder.inner = step(holder.inner);\n    \
         match pick {{ Pick::Some {{ value }} -> value }}\n}}\n"
    ));
    let bump = machine_named(&elaboration, "bump");
    let module = elaboration.module;
    assert!(
        module
            .machine(bump)
            .unwrap()
            .blocks
            .iter()
            .any(|block| matches!(
                block.terminator,
                terminal_psi::Terminator::EliminateEmpty { .. }
            )),
        "the left-out empty case ends in an elimination"
    );
    let closed = terminal_codec::ClosedTerminalModule::new(module).unwrap();
    terminal_codec::ClosedTerminalModule::from_semantic_bytes(closed.semantic_bytes())
        .expect("the accepted module decodes");
}

/// The `update` of `provider_update` with a `match` that leaves out an
/// empty case, so decoding the published module walks it again.
fn eliminating_update(call: &str) -> terminal_check_tables::program::Elaboration {
    accepted_elaboration(&format!(
        "{SESSION}public data Empty requires false {{}}\n\
         data Pick {{ case Some(value: u32); case Never(value: Empty); }}\n\
         machine update(holder: @mutable Holder, pick: Pick) -> u32 {{\n    \
         holder.inner = {call}(holder.inner);\n    \
         match pick {{ Pick::Some {{ value }} -> value }}\n}}\n"
    ))
}

/// terminal-psi/ownership.md: a decoder "must independently reconstruct
/// the window rather than trust producer approval". Decoding judges the
/// provider call inside the window by the requirement the module declares:
/// the accepted module decodes, and the same module whose `Session::step`
/// envelope may also call its first parameter, as `invokes` may say of a
/// callable parameter, is refused.
#[test]
fn decoding_judges_a_provider_call_inside_the_window_by_its_declared_contract() {
    let decode = |module: terminal_psi::TerminalModule| {
        terminal_codec::ClosedTerminalModule::new(module).and_then(|closed| {
            terminal_codec::ClosedTerminalModule::from_semantic_bytes(closed.semantic_bytes())
        })
    };
    let elaboration = eliminating_update("Session::step");
    let mut module = elaboration.module;
    decode(module.clone()).expect("the accepted module decodes");
    let [Some(step)] = module.execution.callables.boundary_envelopes.as_mut_slice() else {
        panic!("`Session::step` is the one requirement the module calls");
    };
    step.envelope
        .invocations
        .push(terminal_psi::TerminalCallableInvocation::Parameter(0));
    let error = decode(module).expect_err("the forged callback is judged");
    assert!(
        format!("{error:?}").contains("empty elimination observes unavailable storage"),
        "{error:?}"
    );
}

/// A callee that calls a computed machine value runs code the module does
/// not name, so a call to it inside the window is fenced even though
/// `step` declares no reach.
#[test]
fn a_computed_call_in_a_callee_fences_the_call_to_it() {
    let elaboration = accepted_elaboration(&bump("{ inner }"));
    let (caller, step) = (
        machine_named(&elaboration, "bump"),
        machine_named(&elaboration, "step"),
    );
    let mut module = elaboration.module;
    assert!(fenced_sites(&module, caller).is_empty());
    append(
        &mut module,
        step,
        OperationKind::CallMachine {
            callee: semantic_vocabulary::ValueId::new(1).unwrap(),
            contract: semantic_vocabulary::CallableContractId::new(1).unwrap(),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    );
    assert!(
        matches!(
            fenced_sites(&module, caller).as_slice(),
            [(
                terminal_semantics::plain_affine::Site::Operation(_),
                CallFence::UnknownAccess
            )]
        ),
        "the call to `step` is fenced"
    );
}

/// A shared reference lends no window: the move stays outside what this
/// route reads, and is never accepted.
#[test]
fn an_owned_move_through_a_shared_reference_is_not_accepted() {
    let outcome = verdict(
        "machine take(holder: @Holder) -> Inner {\n    let taken: Inner = holder.inner;\n    taken\n}\n",
    );
    assert_ne!(outcome, Err("accepted".into()));
}

/// The empty path is the incoming referent itself. Moving its contents must
/// retain that storage and its loan so the subsequent store repairs the same
/// caller-owned home rather than establishing an unrelated local.
#[test]
fn a_moved_whole_referent_must_be_repaired_before_normal_handback() {
    let program = |restore: &str| {
        format!(
            "machine swap(holder: @mutable Inner, next: Inner) -> Inner {{\n    \
             let taken: Inner = move holder;\n    {restore}\n    taken\n}}\n"
        )
    };
    accepted(&program("holder = next;"));
    rejected(&program(""), NOT_RESTORED);
}

#[test]
fn a_whole_borrowed_hole_cannot_be_moved_a_second_time() {
    rejected(
        "machine swap(holder: @mutable Inner, next: Inner) -> Inner {\n    \
         let taken: Inner = move $holder;\n    \
         let again: Inner = move $holder;\n    \
         $holder = next;\n    taken\n}\n",
        USED_VACANT,
    );
}

#[test]
fn replacing_another_root_does_not_repair_the_borrowed_referent() {
    rejected(
        "machine swap(holder: @mutable Inner, other: @mutable Inner, next: Inner) -> Inner {\n    \
         let taken: Inner = move $holder;\n    \
         $other = next;\n    taken\n}\n",
        NOT_RESTORED,
    );
}

#[test]
fn taking_a_whole_referent_requires_its_child_loan_to_have_ended() {
    let program = |before: &str, during: &str| {
        format!(
            "machine observe(value: @u32) {{ }}\n\
             machine swap(holder: @mutable Inner, next: Inner) -> Inner {{\n    \
             let held: @u32 = @holder.value;\n    {before}\n    \
             let taken: Inner = move holder;\n    {during}\n    \
             holder = next;\n    taken\n}}\n"
        )
    };
    accepted(&program("observe(held);", ""));
    rejected(&program("", "observe(held);"), "conflict:");
}

#[test]
fn an_owned_whole_referent_cannot_be_taken_through_a_shared_loan() {
    assert_ne!(
        verdict(
            "machine take(holder: @Inner) -> Inner {\n    \
             let taken: Inner = move $holder;\n    taken\n}\n"
        ),
        Err("accepted".into())
    );
}

const ROOT_RESTORATION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/tests/fixtures/ownership/run/borrowed_root_restoration/main.omg"
));

fn execute_restoration(source: &str) {
    let source = source.replace("machine main()", "machine build_entry()");
    let verdict = super::build_purpose::judge_build(&source).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("the restored mutable backing must be checked: {verdict:?}");
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(10_000))
        .unwrap();
    assert!(
        matches!(
            status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Unit
            )
        ),
        "the caller must observe both removed three and installed seven: {status:?}"
    );
}

#[test]
fn a_whole_borrowed_window_publishes_and_executes_the_original_storage() {
    execute_restoration(ROOT_RESTORATION);
}

#[test]
fn an_owned_field_window_publishes_and_executes_the_original_local_storage() {
    execute_restoration(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../../source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/tests/fixtures/ownership/run/owned_borrowed_field_restoration/main.omg"
    )));
}

#[test]
fn a_whole_local_owned_window_publishes_and_executes_the_original_storage() {
    execute_restoration(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../../source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/tests/fixtures/ownership/run/owned_borrowed_root_restoration/main.omg"
    )));
}

#[test]
fn a_borrowed_field_window_publishes_and_executes_the_original_storage() {
    let source = ROOT_RESTORATION
        .replace(
            "data Item { value: u32; }",
            "data Item { value: u32; }\ndata Box { item: Item; }",
        )
        .replace("owner: @mutable Item", "owner: @mutable Box")
        .replace("move $owner;", "move owner.item;")
        .replace("$owner = next;", "owner.item = next;")
        .replace("second: Item", "second: Box")
        .replace(
            "let mutable owner: Item = Item { value: 3u32 };",
            "let mutable owner: Box = Box { item: Item { value: 3u32 } };",
        )
        .replace("owner.value", "owner.item.value");
    execute_restoration(&source);
}

#[test]
fn an_untracked_copyable_carrier_does_not_erase_an_explicit_window() {
    let verdict = super::entryless::judged(
        "data CopyValue [copy] { value: u32; }\n\
         machine take(value: @mutable CopyValue) -> CopyValue { $value }\n",
        false,
    );
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("the ordinary copy must be checked first: {verdict:?}");
    };
    let mut module = accepted.elaboration.module;
    let machine = &mut module.machines[0];
    let operation = machine
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::StructuralLeafCopy { .. }))
        .expect("the checked body copies its incoming value");
    let OperationKind::StructuralLeafCopy { source, ref path } = operation.kind else {
        unreachable!()
    };
    // An incoming representation can request a destructive window even on
    // a copyable carrier. It must be governed or refused, never skipped by
    // the no-affine-roots fast path after changing just this operation.
    operation.kind = OperationKind::MoveStructuralField {
        source,
        path: path.clone(),
        field: None,
    };
    assert!(matches!(
        terminal_semantics::plain_affine::plan(machine, &module.types.structural_types),
        Err(terminal_semantics::plain_affine::Error::Unsupported {
            detail: "a borrowed move requires tracked storage contents",
            ..
        })
    ));
}

/// A retained descriptor spells access through its own exclusive loan while
/// the hole and its repair remain on the original incoming parameter.
#[test]
fn an_exclusive_local_can_open_and_repair_its_captured_parameter_window() {
    let program = |restore: &str| {
        format!(
            "machine swap(holder: @mutable Inner, next: Inner) -> Inner {{\n    \
             let local: @mutable Inner = @mutable holder;\n    \
             let taken: Inner = move local;\n    {restore}\n    taken\n}}\n"
        )
    };
    accepted(&program("local = next;"));
    rejected(&program(""), NOT_RESTORED);
}

#[test]
fn a_retained_exclusive_window_executes_the_original_caller_storage() {
    execute_restoration(RETAINED_ROOT_RESTORATION);
}

const RETAINED_ROOT_RESTORATION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/tests/fixtures/ownership/run/retained_borrowed_root_restoration/main.omg"
));

#[test]
fn a_local_reference_to_another_parameter_does_not_repair_a_window() {
    rejected(
        "machine swap(holder: @mutable Inner, other: @mutable Inner, next: Inner) -> Inner {\n    \
         let local: @mutable Inner = @mutable holder;\n    \
         let another: @mutable Inner = @mutable other;\n    \
         let taken: Inner = move $local;\n    $another = next;\n    taken\n}\n",
        NOT_RESTORED,
    );
}

#[test]
fn a_local_reference_cannot_extract_the_same_borrowed_value_twice() {
    rejected(
        "machine swap(holder: @mutable Inner, next: Inner) -> Inner {\n    \
         let local: @mutable Inner = @mutable holder;\n    \
         let taken: Inner = move $local;\n    let twice: Inner = move $local;\n    \
         $local = next;\n    taken\n}\n",
        USED_VACANT,
    );
}

#[test]
fn a_shared_local_reference_does_not_acquire_an_owned_window() {
    assert_ne!(
        verdict(
            "machine take(holder: @Inner) -> Inner {\n    \
             let local: @Inner = @holder;\n    let taken: Inner = move $local;\n    taken\n}\n"
        ),
        Err("accepted".into())
    );
}

#[test]
fn a_window_through_exclusive_descendants_keeps_the_original_parameter() {
    let program = |restore: &str| {
        format!(
            "machine swap(holder: @mutable Inner, next: Inner) -> Inner {{\n    \
             let parent: @mutable Inner = @mutable holder;\n    \
             let local: @mutable Inner = @mutable parent;\n    \
             let taken: Inner = move local;\n    {restore}\n    taken\n}}\n"
        )
    };
    accepted(&program("local = next;"));
    rejected(&program(""), NOT_RESTORED);
}

#[test]
fn a_retained_window_cannot_bypass_a_live_descendant_loan() {
    let program = |before: &str, during: &str| {
        format!(
            "machine observe(value: @u32) {{ }}\n\
             machine swap(holder: @mutable Inner, next: Inner) -> Inner {{\n    \
             let local: @mutable Inner = @mutable holder;\n    \
             let child: @u32 = @local.value;\n    {before}\n    \
             let taken: Inner = move local;\n    {during}\n    \
             local = next;\n    taken\n}}\n"
        )
    };
    accepted(&program("observe(child);", ""));
    rejected(&program("", "observe(child);"), "conflict:");
}

#[test]
fn an_exclusive_reference_field_opens_the_same_captured_window() {
    let source = RETAINED_ROOT_RESTORATION
        .replace(
            "data Item { value: u32; }",
            "data Item { value: u32; }\ndata Loan<'storage> { item: @'storage mutable Item; }",
        )
        .replace(
            "let local: @mutable Item = @mutable owner;",
            "let local: Loan = Loan { item: @mutable owner };",
        )
        .replace("move $local;", "move $local.item;")
        // Assigning the field replaces its descriptor. Transfer that same
        // permission to a local before repairing through its referent.
        .replace(
            "$local = next;",
            "let repair: @mutable Item = local.item;\n    $repair = next;",
        );
    execute_restoration(&source);
}

/// Ending a non-owning address spelling cannot fill the field moved from it.
#[test]
fn a_dead_borrowed_block_address_still_owes_its_missing_field() {
    let program = |restore: &str| {
        format!(
            r#"
        {DATA}
        machine take(holder: @mutable Holder, next: Inner, again: bool) -> Inner
        requires true
        {{
            let taken: Inner = move holder.inner;
            {restore}
            transition again {{
                true -> take(holder, taken, false),
                false -> taken
            }}
        }}
    "#
        )
    };
    let elaborate = |typed: &crate::source_state::TypedTrees| {
        crate::lowering::elaborate_for(
            typed,
            crate::lowering::Compilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[],
                subjects: &[None],
                purpose: source_provenance::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
    };
    let typed = super::boundary_calls::typed(&program("holder.inner = next;"));
    let form = elaborate(&typed).unwrap();
    let mut dead_moved_header = false;
    for machine in &form.module.machines {
        let names = terminal_semantics::plain_affine::borrowed_address_liveness(
            machine,
            &form.module.types.structural_types,
        )
        .unwrap();
        for source in machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match operation.kind {
                OperationKind::MoveStructuralField { source, .. } => Some(source),
                _ => None,
            })
        {
            if !names.is_address_header(source) {
                continue;
            }
            dead_moved_header |= machine.blocks.iter().any(|block| {
                matches!(
                    block.terminator,
                    terminal_psi::Terminator::ReturnStructural { .. }
                ) && names.is_live(source, (block.id, block.operations.len() as u32)) == Some(false)
            });
        }
    }
    assert!(
        dead_moved_header,
        "the moved field must belong to a borrowed block address that ends before return"
    );
    let restored = judge(Ok(form)).unwrap();
    assert!(
        matches!(restored, RouteVerdict::Accepted(_)),
        "{restored:?}"
    );
    let invalid = super::boundary_calls::typed(&program(""));
    let refused = match elaborate(&invalid) {
        Ok(form) => judge(Ok(form)).unwrap(),
        Err(refusal) => RouteVerdict::FallBack(refusal),
    };
    let owes_restoration = match &refused {
        RouteVerdict::Rejected { violations, .. } => violations
            .iter()
            .any(|violation| violation.statement.contains(NOT_RESTORED)),
        RouteVerdict::FallBack(refusal) => refusal
            .detail
            .contains("borrowed block storage still owes restoration at its last use"),
        _ => false,
    };
    assert!(owes_restoration, "{refused:?}");
}

#[test]
fn replacing_a_borrowed_sum_returns_only_the_new_tags_claims() {
    let source = r#"
data Token [linear] { value: u64; }
data Choice { case Left(token: Token); case Right(token: Token); }
data Boxed { marker: u64; choice: Choice; }
machine read(token: Token) -> u64 { let { value } = move token; value }
machine take(choice: Choice) -> u64 {
    transition choice {
        Choice::Left { token } -> read(token),
        Choice::Right { token } -> read(token)
    }
}
machine repair(choice: @mutable Choice) {
    let old: Choice = move $choice;
    let value: u64 = take(old);
    $choice = Choice::Right { token: Token { value: ((value as +Wrapping) + 1) as -Wrapping } };
}
machine build_entry() crashes Trap {
    let mutable boxed: Boxed = Boxed { marker: 19u64, choice: Choice::Left { token: Token { value: 41u64 } } };
    repair(@mutable boxed.choice);
    let marker: u64 = boxed.marker;
    let result: u64 = take(boxed.choice);
    transition marker == 19u64 && result == 42u64 { true -> done(), _ -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
"#;
    let verdict = super::build_purpose::judge_build(source).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("{verdict:?}");
    };
    let module = &accepted.elaboration.module;
    let caller = module
        .machines
        .iter()
        .find(|machine| !machine.loan_completions.is_empty())
        .expect("the caller needs a new claim for each possible tag");
    let [completion] = caller.loan_completions.as_slice() else {
        panic!("expected one repaired field");
    };
    assert_eq!(completion.claims.len(), 2);
    assert_eq!(completion.path.len(), 1);
    let old_claim = caller
        .operations()
        .filter_map(|operation| operation.result.as_structural())
        .flat_map(|result| &result.claims)
        .find(|claim| claim.path.is_empty())
        .unwrap()
        .claim;
    for mutation in 0..3 {
        let mut forged = module.clone();
        let changed = forged
            .machines
            .iter_mut()
            .find(|row| row.id == caller.id)
            .unwrap();
        match mutation {
            0 => changed.loan_completions.clear(),
            1 => changed.loan_completions[0].path.clear(),
            _ => changed.loan_completions[0].claims[0].claim = old_claim,
        }
        let error = terminal_semantics::plain_affine::verify_claims_in(&forged, caller.id)
            .expect_err("receiving must reconstruct the exact field and its new claims");
        assert!(
            format!("{error:?}").contains("loan completion"),
            "{error:?}"
        );
    }
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance.invoke(&mut terminal_work::WorkMeter::with_budget(10_000));
    assert!(
        matches!(
            status,
            Ok(terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Unit
            ))
        ),
        "{status:?}"
    );
    let missing = source.replace(
        "$choice = Choice::Right { token: Token { value: ((value as +Wrapping) + 1) as -Wrapping } };",
        "",
    );
    let rejected = super::build_purpose::judge_build(&missing).unwrap();
    assert!(
        matches!(rejected, RouteVerdict::Rejected { ref unestablished, .. } if unestablished.iter().any(|row| row.detail.contains("a replacement must be stored"))),
        "{rejected:?}"
    );
}
