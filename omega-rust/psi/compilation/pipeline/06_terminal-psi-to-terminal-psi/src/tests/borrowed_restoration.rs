//! An owned value moved out of storage the machine was lent with `&mut`.
//!
//! ownership.md, "Borrowed-storage invariant windows": "An owned move out of
//! a place reached through `&mut` may temporarily leave that place
//! unestablished ... Assignment of a valid owned replacement to the exact
//! vacant place closes the ownership window ... Every return or terminal
//! transfer from the region owing restoration ... must close the window."
//!
//! Programs remove an affine record or projected field from mutable incoming
//! storage, directly or through retained exclusive references. Accepted paths
//! restore that exact storage before handback; each failing twin breaks only
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
fn a_local_owned_field_is_restored_before_its_reference_is_released() {
    let program = |restore: &str| {
        format!(
            "machine take(value: Holder, next: Inner) -> Inner {{\n\
         let mut owner: Holder = value;\n\
         let local: &mut Holder = &mut owner;\n\
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
         let mut owner: Holder = value;\n\
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
         let mut owner: Separate = value;\n\
         let local: &mut Holder = &mut owner.lent;\n\
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
            "machine observe(value: &u32) {{ }}\n\
         machine stop(owner: &mut Inner) crashes Trap {{\n\
         let held: &u32 = &owner.value;\n\
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
         case Left(item: &'storage Inner);\n\
         case Right(item: &'storage Inner);\n\
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
         let mut owner: Holder = value;\n\
         let local: &mut Holder = &mut owner;\n\
         let taken: Inner = move local.inner;\n\
         let repair: &mut Holder = move local;\n\
         {restore}\n\
         taken\n}}\n"
        )
    };
    accepted(&program("repair.inner = next;"));
    rejected(&program(""), NOT_RESTORED);
}

#[test]
fn a_last_reference_use_does_not_authorize_parent_use_before_actual_release() {
    let source = format!(
        "{DATA}machine read(value: Holder, next: Inner) -> u32 {{\n\
         let mut owner: Holder = value;\n\
         let local: &mut Holder = &mut owner;\n\
         let taken: Inner = move local.inner;\n\
         local.inner = next;\n\
         owner.tag\n}}\n"
    );
    let RouteVerdict::Accepted(accepted) = super::entryless::judged(&source, false) else {
        panic!("the original restored source must be accepted");
    };
    let mut elaboration = accepted.elaboration;
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
    let release = block
        .operations
        .iter()
        .position(|operation| matches!(operation.kind, OperationKind::ReleaseReference { .. }))
        .unwrap();
    assert!(matches!(
        block.operations[release + 1].kind,
        OperationKind::ScalarStructuralField { .. }
    ));
    // Keep the same operations, custody and final cleanup, but move the
    // release after the parent observation. Future-use liveness is identical
    // at that observation; actual permission custody is not.
    block.operations.swap(release, release + 1);
    elaboration.module.unoptimized_behavior = elaboration
        .module
        .machines
        .iter()
        .map(terminal_psi::UnoptimizedMachineBehavior::capture)
        .collect();
    let outcome = judge(Ok(elaboration)).unwrap();
    assert!(
        matches!(outcome, RouteVerdict::Rejected { ref conflicts, .. } if !conflicts.is_empty()),
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
            "machine swap(holder: &mut Holder, next: Inner) -> Inner {{\n    \
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
            "machine swap(holder: &mut Holder, next: Inner, last: Inner, go: bool) {{\n    \
             let taken: Inner = holder.inner;\n    {before}\n    \
             transition go {{\n        true -> finish(holder, last)\n        _ -> finish(holder, last)\n    }}\n    \
             machine finish(holder: &mut Holder, last: Inner) {{\n        {after}\n    }}\n}}\n"
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
            "machine swap(holder: &mut Holder, next: Inner) -> Inner {{\n    \
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
            "machine observe(holder: &Holder) {{ }}\n\
             machine swap(holder: &mut Holder, next: Inner) -> Inner {{\n    \
             let taken: Inner = holder.inner;\n    {first}\n    {second}\n    taken\n}}\n"
        )
    };
    accepted(&program("holder.inner = next;", "observe(holder);"));
    rejected(
        &program("observe(holder);", "holder.inner = next;"),
        USED_VACANT,
    );
}

/// A shared reference lends no window: the move stays outside what this
/// route reads, and is never accepted.
#[test]
fn an_owned_move_through_a_shared_reference_is_not_accepted() {
    let outcome = verdict(
        "machine take(holder: &Holder) -> Inner {\n    let taken: Inner = holder.inner;\n    taken\n}\n",
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
            "machine swap(holder: &mut Inner, next: Inner) -> Inner {{\n    \
             let taken: Inner = move holder;\n    {restore}\n    taken\n}}\n"
        )
    };
    accepted(&program("holder = next;"));
    rejected(&program(""), NOT_RESTORED);
}

#[test]
fn a_whole_borrowed_hole_cannot_be_moved_a_second_time() {
    rejected(
        "machine swap(holder: &mut Inner, next: Inner) -> Inner {\n    \
         let taken: Inner = move holder;\n    \
         let again: Inner = move holder;\n    \
         holder = next;\n    taken\n}\n",
        USED_VACANT,
    );
}

#[test]
fn replacing_another_root_does_not_repair_the_borrowed_referent() {
    rejected(
        "machine swap(holder: &mut Inner, other: &mut Inner, next: Inner) -> Inner {\n    \
         let taken: Inner = move holder;\n    \
         other = next;\n    taken\n}\n",
        NOT_RESTORED,
    );
}

#[test]
fn taking_a_whole_referent_requires_its_child_loan_to_have_ended() {
    let program = |before: &str, during: &str| {
        format!(
            "machine observe(value: &u32) {{ }}\n\
             machine swap(holder: &mut Inner, next: Inner) -> Inner {{\n    \
             let held: &u32 = &holder.value;\n    {before}\n    \
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
            "machine take(holder: &Inner) -> Inner {\n    \
             let taken: Inner = move holder;\n    taken\n}\n"
        ),
        Err("accepted".into())
    );
}

const ROOT_RESTORATION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../tests/fixtures/run/ownership/borrowed_root_restoration/main.omg"
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
        "/../../../../../tests/fixtures/run/ownership/owned_borrowed_field_restoration/main.omg"
    )));
}

#[test]
fn a_borrowed_field_window_publishes_and_executes_the_original_storage() {
    let source = ROOT_RESTORATION
        .replace(
            "data Item { value: u32; }",
            "data Item { value: u32; }\ndata Box { item: Item; }",
        )
        .replace("owner: &mut Item", "owner: &mut Box")
        .replace("move owner;", "move owner.item;")
        .replace("owner = next;", "owner.item = next;")
        .replace("second: Item", "second: Box")
        .replace(
            "let mut owner: Item = Item { value: 3u32 };",
            "let mut owner: Box = Box { item: Item { value: 3u32 } };",
        )
        .replace("owner.value", "owner.item.value");
    execute_restoration(&source);
}

#[test]
fn an_untracked_copyable_carrier_does_not_erase_an_explicit_window() {
    let verdict = super::entryless::judged(
        "data CopyValue [copy] { value: u32; }\n\
         machine take(value: &mut CopyValue) -> CopyValue { value }\n",
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
        terminal_semantics::plain_affine::plan(machine, &module.structural_types),
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
            "machine swap(holder: &mut Inner, next: Inner) -> Inner {{\n    \
             let local: &mut Inner = &mut holder;\n    \
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
    "/../../../../../tests/fixtures/run/ownership/retained_borrowed_root_restoration/main.omg"
));

#[test]
fn a_local_reference_to_another_parameter_does_not_repair_a_window() {
    rejected(
        "machine swap(holder: &mut Inner, other: &mut Inner, next: Inner) -> Inner {\n    \
         let local: &mut Inner = &mut holder;\n    \
         let another: &mut Inner = &mut other;\n    \
         let taken: Inner = move local;\n    another = next;\n    taken\n}\n",
        NOT_RESTORED,
    );
}

#[test]
fn a_local_reference_cannot_extract_the_same_borrowed_value_twice() {
    rejected(
        "machine swap(holder: &mut Inner, next: Inner) -> Inner {\n    \
         let local: &mut Inner = &mut holder;\n    \
         let taken: Inner = move local;\n    let twice: Inner = move local;\n    \
         local = next;\n    taken\n}\n",
        USED_VACANT,
    );
}

#[test]
fn a_shared_local_reference_does_not_acquire_an_owned_window() {
    assert_ne!(
        verdict(
            "machine take(holder: &Inner) -> Inner {\n    \
             let local: &Inner = &holder;\n    let taken: Inner = move local;\n    taken\n}\n"
        ),
        Err("accepted".into())
    );
}

#[test]
fn a_window_through_exclusive_descendants_keeps_the_original_parameter() {
    let program = |restore: &str| {
        format!(
            "machine swap(holder: &mut Inner, next: Inner) -> Inner {{\n    \
             let parent: &mut Inner = &mut holder;\n    \
             let local: &mut Inner = &mut parent;\n    \
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
            "machine observe(value: &u32) {{ }}\n\
             machine swap(holder: &mut Inner, next: Inner) -> Inner {{\n    \
             let local: &mut Inner = &mut holder;\n    \
             let child: &u32 = &local.value;\n    {before}\n    \
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
            "data Item { value: u32; }\ndata Loan<'storage> { item: &'storage mut Item; }",
        )
        .replace(
            "let local: &mut Item = &mut owner;",
            "let local: Loan = Loan { item: &mut owner };",
        )
        .replace("move local;", "move local.item;")
        // Assigning the field replaces its descriptor. Transfer that same
        // permission to a local before repairing through its referent.
        .replace(
            "local = next;",
            "let repair: &mut Item = local.item;\n    repair = next;",
        );
    execute_restoration(&source);
}
