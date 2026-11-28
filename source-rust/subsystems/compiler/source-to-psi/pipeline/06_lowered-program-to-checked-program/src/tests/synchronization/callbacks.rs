use super::{execute_unit, reject_deadlock};

// The callback is a runtime machine value. Either handler has the same checked
// signature; the selected descriptor determines which lock its body reacquires.
const SYNCHRONOUS_HANDLER: &str = r"
data Lock { word: AtomicU32; }
data Locks { primary: Lock; auxiliary: Lock; }
data Handler = machine(locks: @Locks) reaches none crashes none;

machine acquire(lock: @Lock) {
    let previous: u32 = lock.word.swap(1, ReceivePublish);
    transition previous == 1 {
        true -> acquire(lock),
        _ -> done()
    }
    machine done() {}
}

machine primary_handler(locks: @Locks) {
    acquire(@locks.primary);
    locks.primary.word.store(0, Publish);
}

machine auxiliary_handler(locks: @Locks) {
    acquire(@locks.auxiliary);
    locks.auxiliary.word.store(0, Publish);
}

machine service(handler: Handler, locks: @Locks) {
    acquire(@locks.primary);
    handler(locks);
    locks.primary.word.store(0, Publish);
}

machine build_entry() {
    let mutable locks: Locks;
    locks.primary.word.store(0, Publish);
    locks.auxiliary.word.store(0, Publish);
    let handler: Handler = auxiliary_handler;
    service(handler, @locks);
}
";

#[test]
fn a_synchronous_callback_can_acquire_an_independent_lock() {
    execute_unit(SYNCHRONOUS_HANDLER);
}

#[test]
fn a_callback_waiting_for_its_callers_held_lock_deadlocks() {
    let reentry = SYNCHRONOUS_HANDLER.replace("= auxiliary_handler;", "= primary_handler;");
    reject_deadlock(&reentry);
}

#[test]
fn a_callback_can_reacquire_the_lock_its_caller_already_released() {
    let released = SYNCHRONOUS_HANDLER
        .replace("= auxiliary_handler;", "= primary_handler;")
        .replace(
            "handler(locks);\n    locks.primary.word.store(0, Publish);",
            "locks.primary.word.store(0, Publish);\n    handler(locks);",
        );
    execute_unit(&released);
}

#[test]
fn two_invocations_of_one_service_keep_their_individual_callback_selections() {
    let reentered = SYNCHRONOUS_HANDLER.replace(
        "service(handler, @locks);",
        "service(handler, @locks);\n    let next: Handler = primary_handler;\n    service(next, @locks);",
    );
    reject_deadlock(&reentered);

    let read_only = reentered.replace(
        "acquire(@locks.primary);\n    locks.primary.word.store(0, Publish);",
        "let observed: u32 = locks.primary.word.load(Receive);",
    );
    execute_unit(&read_only);
}
