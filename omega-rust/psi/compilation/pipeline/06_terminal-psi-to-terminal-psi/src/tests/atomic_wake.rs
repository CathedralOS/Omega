//! Keep the timer's observation and pending bit atomic through checked storage,
//! publication and execution. This is a sequential wake oracle, not a claim
//! about concurrent interrupt delivery or native retry-loop timing.

use super::*;

const WAKE: &str = r#"
// A timer interrupt records an observation and wakes ordinary kernel code.
// Repeated arrivals share one pending bit, so this state never grows a queue.
// The handler must receive this preallocated state through an installed
// internal claim; the hardware does not supply it as an entry parameter.
// Store the observation before publishing the bit. The consumer acquires and
// clears the bit before reading the observation.
public data TimerRootWakeState {
    observed_time: AtomicU64;
    wake_pending: AtomicU32;
}

// Result of one ordinary-task attempt to claim the coalesced wake marker.
// `Idle` carries no time because no publication was acquired. `Claimed`
// carries the latest observation visible after the matching Receive operation;
// it is a wake notification, not a count of interrupt arrivals.
public data TimerRootWakeClaim {
    case Idle;
    case Claimed(observed_time: u64);
}

// Record one already-captured monotonic observation, publish one coalesced wake
// marker, and return. Clock capture remains a distinct provider leaf. This
// machine allocates nothing, loops nowhere, and invokes no scheduler service.
public machine record_and_publish(
    wake: &TimerRootWakeState,
    observed_time: u64
) {
    wake.observed_time.store(observed_time, NoOrdering);
    let prior_pending: u32 = wake.wake_pending.fetch_or(1, Publish);
}

// Claim and clear at most one coalesced wake in the ordinary timer-service
// task. The Receive read-modify-write pairs with the hard root's Publish; only
// after a marker is acquired may the following relaxed load consume the
// recorded time. A concurrent later publication may make the same latest
// observation visible twice, but it cannot be lost by this clear-first shape.
// This leaf allocates nothing, loops nowhere, and invokes no scheduler service.
public machine claim_pending(wake: &TimerRootWakeState) -> TimerRootWakeClaim {
    let prior_pending: u32 = wake.wake_pending.fetch_and(0, Receive);
    transition (prior_pending & 1) == 1 {
        true -> claimed(wake)
        _ -> idle(wake)
    }

    machine claimed(wake: &TimerRootWakeState) -> TimerRootWakeClaim {
        let claimed_time: u64 = wake.observed_time.load(NoOrdering);
        TimerRootWakeClaim::Claimed { observed_time: claimed_time }
    }

    machine idle(wake: &TimerRootWakeState) -> TimerRootWakeClaim {
        TimerRootWakeClaim::Idle
    }
}

// Expected exit: 0
// A published timer wake is consumed once. Publications before consumption
// coalesce to the latest observation, and unrelated storage remains intact.

data WakeStorage {
    before: u64;
    wake: TimerRootWakeState;
    after: u64;
}

machine is_idle(wake: &TimerRootWakeState) -> bool {
    match claim_pending(wake) {
        TimerRootWakeClaim::Idle -> true,
        TimerRootWakeClaim::Claimed { observed_time } -> false,
    }
}

machine has_observation(wake: &TimerRootWakeState, expected: u64) -> bool {
    match claim_pending(wake) {
        TimerRootWakeClaim::Claimed { observed_time } -> observed_time == expected,
        TimerRootWakeClaim::Idle -> false,
    }
}

machine build_entry() crashes Trap {
    let mut storage: WakeStorage;
    storage.before = 101;
    storage.after = 103;
    let initially_idle: bool = is_idle(&storage.wake);
    record_and_publish(&storage.wake, 41);
    let first: bool = has_observation(&storage.wake, 41);
    let first_cleared: bool = is_idle(&storage.wake);
    record_and_publish(&storage.wake, 73);
    record_and_publish(&storage.wake, 97);
    let latest: bool = has_observation(&storage.wake, 97);
    let latest_cleared: bool = is_idle(&storage.wake);
    record_and_publish(&storage.wake, 113);
    let next_round: bool = has_observation(&storage.wake, 113);
    let next_cleared: bool = is_idle(&storage.wake);
    transition initially_idle && first && first_cleared && latest && latest_cleared
        && next_round && next_cleared && storage.before == 101 && storage.after == 103 {
        true -> done()
        _ -> wrong()
    }
    machine done() {}
    machine wrong() { crash Trap; }
}
"#;

#[test]
fn timer_wake_preserves_prior_bits_coalescing_and_neighbor_guards() {
    execute(WAKE);
}

fn execute(source: &str) {
    let verdict = super::build_purpose::judge_build(source).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("{verdict:?}");
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(10_000))
        .unwrap();
    assert_eq!(
        status,
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        )
    );
}

#[test]
fn projected_atomic_events_return_prior_bits_and_keep_neighbor_storage() {
    execute(
        r#"
    data Cell { before: u64; value: AtomicU64; pending: AtomicU32; after: u64; }
    machine publish(cell: &Cell, value: u64) {
        cell.value.store(value, NoOrdering);
        let prior: u32 = cell.pending.fetch_or(1, Publish);
    }
    machine take(cell: &Cell) -> u32 {
        let prior: u32 = cell.pending.fetch_and(0, Receive);
        prior
    }
    machine build_entry() crashes Trap {
        let mut cell: Cell;
        cell.before = 101;
        cell.after = 103;
        let idle: u32 = take(&cell);
        publish(&cell, 73);
        publish(&cell, 97);
        let pending: u32 = take(&cell);
        let observed: u64 = cell.value.load(NoOrdering);
        let cleared: u32 = take(&cell);
        publish(&cell, 113);
        let later: u32 = take(&cell);
        let next: u64 = cell.value.load(NoOrdering);
        transition idle == 0 && pending == 1 && observed == 97 && cleared == 0
            && later == 1 && next == 113 && cell.before == 101 && cell.after == 103 {
            true -> done()
            _ -> wrong()
        }
        machine done() {}
        machine wrong() { crash Trap; }
    }
    "#,
    );
}

fn refused(source: &str, detail: &str) {
    let result = super::build_purpose::judge_build(source);
    assert!(
        !matches!(result, Ok(RouteVerdict::Accepted(_))),
        "invalid atomic source was accepted"
    );
    let diagnostic = format!("{result:?}");
    assert!(diagnostic.contains(detail), "{diagnostic}");
}

#[test]
fn atomic_events_reject_plain_cells_illegal_orderings_and_mismatched_widths() {
    refused(
        "data Cell { value: u32; } machine build_entry() { let cell: Cell; cell.value.store(1, Publish); }",
        "dedicated atomic storage",
    );
    malformed_ordering(
        "data Cell { value: AtomicU32; } machine build_entry() { let cell: Cell; cell.value.store(1, Receive); }",
        "atomic store cannot use",
    );
    malformed_ordering(
        "data Cell { value: AtomicU32; } machine build_entry() -> u32 { let cell: Cell; cell.value.load(Publish) }",
        "atomic load cannot use",
    );
    refused(
        "data Cell { value: AtomicU32; } machine build_entry() { let cell: Cell; let wide: u64 = 1; cell.value.store(wide, Publish); }",
        "exact resident type",
    );
    refused(
        "data Cell { value: AtomicU32; } machine build_entry() { let cell: Cell; let prior: u64 = cell.value.fetch_or(1, Publish); }",
        "exact resident type",
    );
}

#[test]
fn atomic_load_is_not_a_plain_value_equation_or_permission_upgrade() {
    refused(
        "data Cell { value: AtomicU32; } machine read(cell: &Cell) -> u32 ensures result == 37 { cell.value.load(NoOrdering) } machine build_entry() { let cell: Cell; let value: u32 = read(&cell); }",
        "cannot prove ensures contract",
    );
    refused(
        "data Cell { value: AtomicU32; } machine write(cell: &write Cell) { cell.value.store(1, Publish); } machine build_entry() { let mut cell: Cell; write(&write cell); }",
        "write-only reference parameter",
    );
    refused(
        "data Cell { value: u32; } machine write(cell: &Cell) { cell.value = 1; } machine build_entry() { let cell: Cell; write(&cell); }",
        "mutable",
    );
}

fn malformed_ordering(source: &str, detail: &str) {
    let mut sources = source::SourceMap::default();
    let id = sources
        .add("atomic_ordering.omg".into(), source.to_owned())
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let error = tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens)
        .expect_err("illegal ordering was parsed");
    assert!(error.message.contains(detail), "{error:?}");
}

#[test]
fn atomic_access_still_conflicts_with_an_outstanding_exclusive_loan() {
    refused(
        r#"
        data Cell { value: AtomicU32; }
        machine read(cell: &mut Cell) -> u32 { cell.value.load(NoOrdering) }
        machine build_entry() -> u32 {
            let mut cell: Cell;
            let loan: &mut Cell = &mut cell;
            cell.value.store(1, Publish);
            read(loan)
        }
    "#,
        "ObserveWhileExclusive",
    );
}

#[test]
fn computed_subject_is_evaluated_once_and_transfers_its_owned_payload() {
    execute(
        r#"
        data Counter { value: u32; }
        data Owner { value: u64; }
        data Reply { case Ready(owner: Owner); }
        machine make(counter: &mut Counter) -> Reply requires counter.value < 2 {
            counter.value = counter.value + 1;
            Reply::Ready { owner: Owner { value: 97 } }
        }
        machine take(counter: &mut Counter) -> Owner requires counter.value < 2 {
            match make(counter) { Reply::Ready { owner } -> owner }
        }
        machine build_entry() crashes Trap {
            let mut counter: Counter;
            let result: Owner = take(&mut counter);
            transition counter.value == 1 && result.value == 97 {
                true -> done()
                _ -> wrong()
            }
            machine done() {}
            machine wrong() { crash Trap; }
        }
    "#,
    );
}

#[test]
fn computed_subject_cannot_supply_the_same_owned_payload_twice() {
    refused(
        r#"
        data Owner { value: u64; }
        data Reply { case Ready(owner: Owner); }
        data Pair { first: Owner; second: Owner; }
        machine make() -> Reply { Reply::Ready { owner: Owner { value: 97 } } }
        machine bad() -> Pair {
            match make() { Reply::Ready { owner } -> Pair { first: owner, second: owner } }
        }
        machine build_entry() { let pair: Pair = bad(); }
    "#,
        "used after its value was transferred or consumed",
    );
}

#[test]
fn computed_subject_is_retired_on_a_normal_value_continuation() {
    execute(
        r#"
        data Owner { value: u64; }
        data Reply { case Ready(owner: Owner); }
        machine make() -> Reply { Reply::Ready { owner: Owner { value: 97 } } }
        machine build_entry() crashes Trap {
            let result: Owner = match make() { Reply::Ready { owner } -> owner };
            transition result.value == 97 { true -> done() _ -> wrong() }
            machine done() {}
            machine wrong() { crash Trap; }
        }
    "#,
    );
}

#[test]
fn computed_subject_is_retired_on_a_real_backedge() {
    execute(
        r#"
        data Reply { case Ready(value: u32); }
        machine make(value: u32) -> Reply { Reply::Ready { value: value } }
        machine walk(value: u32) -> u32 {
            match make(value) { Reply::Ready { value } -> step(value) }
            machine step(value: u32) -> u32 {
                transition value < 2 { true -> walk(value + 1) _ -> done(value) }
                machine done(value: u32) -> u32 { value }
            }
        }
        machine build_entry() crashes Trap {
            let result: u32 = walk(0);
            transition result == 2 { true -> done() _ -> wrong() }
            machine done() {}
            machine wrong() { crash Trap; }
        }
    "#,
    );
}

// Borrowing a computed payload currently refuses in the reader before the
// cleanup partition. This is fail-closed coverage, not a retirement-loan test.
#[test]
fn a_computed_payload_borrow_requires_a_rooted_source() {
    refused(
        r#"
        data Owner { value: u64; }
        data Reply { case Ready(owner: Owner); }
        machine make() -> Reply { Reply::Ready { owner: Owner { value: 97 } } }
        data Loan<'storage> { owner: &'storage Owner; }
        machine build_entry() -> u64 {
            let loan: Loan = match make() { Reply::Ready { owner } -> Loan { owner: &owner } };
            loan.owner.value
        }
    "#,
        "call borrow of a value that is not a place",
    );
}
