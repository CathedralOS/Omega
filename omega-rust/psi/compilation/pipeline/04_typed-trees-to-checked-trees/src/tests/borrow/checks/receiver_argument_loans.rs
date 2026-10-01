//! A whole `&mut` receiver writes through the callee while the same call's
//! retained argument loans are live; disjoint storage and copied arguments
//! remain usable beside it.

use super::check_program;

const RECEIVER_WRITE: &str = "while its mutable receiver writes";

#[test]
fn receiver_summary_preserves_shared_atomic_access_through_a_boundary_wrapper() {
    assert_accepted(
        "boundary trait Output { machine notify() reaches Output; }
        data Task { canceled: AtomicBool; }
        data Slot { case Held(task: Task); case Empty; }
        data Main { tasks: [Slot; 2]; }
        machine Task::cancel(&self) reaches Output invokes Output; {
            self.canceled.store(true, GlobalOrder);
            Output::notify();
        }
        machine Main::cancel(&self) reaches Output invokes Output; {
            transition self.tasks[0] {
                Slot::Held { task } -> cancel_one(&task)
                _ -> done()
            }
            state cancel_one(&self, task: &Task) { task.cancel(); }
            state done(&self) {}
        }",
    );
}

#[test]
fn receiver_summary_keeps_plain_atomic_replacement_exclusive() {
    assert_receiver_write_rejected(
        "boundary trait Output { machine notify() reaches Output; }
        data Main { canceled: AtomicBool; }
        machine Main::replace(&mut self, observed: &AtomicBool) reaches Output invokes Output; {
            self.canceled.store(true, GlobalOrder);
            self.canceled = false;
            Output::notify();
        }
        machine Main::run(&mut self) reaches Output invokes Output; {
            self.replace(&self.canceled);
        }",
    );
}

#[test]
fn receiver_summary_atomic_access_still_conflicts_with_an_exclusive_loan() {
    let diagnostics = check_program(
        "boundary trait Output { machine notify() reaches Output; }
        data Task { canceled: AtomicBool; }
        machine Task::cancel(&self) reaches Output invokes Output; {
            self.canceled.store(true, GlobalOrder);
            Output::notify();
        }
        machine hold(value: &mut AtomicBool) {}
        machine run(task: &mut Task) reaches Output invokes Output; {
            let held: &mut AtomicBool = &mut task.canceled;
            task.cancel();
            hold(held);
        }",
    )
    .expect_err("shared atomic access cannot bypass an existing exclusive loan");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("still active")),
        "{diagnostics:#?}"
    );
}

#[test]
fn receiver_summary_shared_atomic_writes_still_invalidate_storage_facts() {
    let program = crate::tests::front_end::typed_program(
        "boundary trait Output { machine notify() reaches Output; }
        data Main { canceled: AtomicBool; }
        machine Main::cancel(&self) reaches Output invokes Output; {
            self.canceled.store(true, GlobalOrder);
            Output::notify();
        }
        machine Main::run(&self) reaches Output invokes Output; { self.cancel(); }",
    );
    let caller = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("caller");
    let state = &program.machine_states(caller)[0];
    let borrow = crate::borrow::build_borrow_facts(&program);
    let borrow_state = borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|candidate| candidate.state_symbol == state.symbol)
        .expect("borrow state");
    let [call] = borrow.calls.span_or_empty(borrow_state.calls) else {
        panic!("one call")
    };
    let writes = crate::flow::call_mutated_places(
        &program,
        caller.symbol,
        state.symbol,
        &borrow,
        call,
        &crate::flow::StateMutationSummaryCache::default(),
        validation::CallFrameResolver::new(&program).as_ref(),
    )
    .expect("a complete storage frame");
    assert_eq!(
        writes
            .iter()
            .map(|place| place.label(&program))
            .collect::<Vec<_>>(),
        ["Main::run.canceled"]
    );
}

#[test]
fn receiver_summary_retains_boundary_signature_effects_through_states() {
    assert_accepted(
        "boundary trait Output { machine observe(value: &i32) reaches Output; }
        data Main { value: i32; }
        machine Main::send(&mut self, value: &i32) reaches Output invokes Output; {
            transition { _ -> forward(value) }
            state forward(&mut self, value: &i32) {
                Output::observe(value);
            }
        }
        machine Main::run(&mut self) reaches Output invokes Output; {
            self.send(&self.value);
        }",
    );
}

#[test]
fn receiver_summary_retains_boundary_writes_beside_disjoint_loans() {
    assert_accepted(
        "boundary trait Output {
            machine observe(count: &mut i32, value: &i32) reaches Output;
        }
        data Main { count: i32; value: i32; }
        machine Main::send(&mut self, value: &i32) reaches Output invokes Output; {
            Output::observe(&mut self.count, value);
        }
        machine Main::run(&mut self) reaches Output invokes Output; {
            self.send(&self.value);
        }",
    );
}

#[test]
fn receiver_summary_keeps_overlapping_boundary_writes_rejected() {
    assert_receiver_write_rejected(
        "boundary trait Output { machine update(value: &mut i32) reaches Output; }
        data Main { value: i32; }
        machine Main::send(&mut self, value: &i32) reaches Output invokes Output; {
            Output::update(&mut self.value);
            let observed: i32 = value;
        }
        machine Main::run(&mut self) reaches Output invokes Output; {
            self.send(&self.value);
        }",
    );
}

fn assert_receiver_write_rejected(source: &str) {
    let diagnostics =
        check_program(source).expect_err("the receiver write overlaps a retained argument loan");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(RECEIVER_WRITE)),
        "{source}: {diagnostics:#?}"
    );
}

fn assert_accepted(source: &str) {
    check_program(source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
}

const OVERWRITE_THEN_READ: &str = "data Main { arr: [i32; 5]; count: i32; }
    machine Main::overwrite_then_read(&mut self, sub: &[i32]) -> i32 {
        self.arr[0] = 7;
        transition sub.len >= 1 { true -> (sub[0]) false -> 0 }
    }";

#[test]
fn receiver_write_rejects_a_shared_view_of_the_written_element() {
    for caller in [
        "machine Main::value_receiver_form(main: &mut Main) -> i32 {
            let seen: i32 = main.overwrite_then_read(main.arr[0..2]);
            (seen)
        }",
        "machine Main::self_receiver_form(&mut self) -> i32 {
            let seen: i32 = self.overwrite_then_read(self.arr[0..2]);
            (seen)
        }",
    ] {
        assert_receiver_write_rejected(&format!("{OVERWRITE_THEN_READ}\n{caller}"));
    }
}

#[test]
fn receiver_write_rejects_explicit_borrows_of_the_written_field() {
    assert_receiver_write_rejected(
        "data Main { count: i32; }
        machine Main::write_both(&mut self, slot: &mut i32) -> i32 {
            self.count = 7;
            slot = 9;
            (self.count)
        }
        machine Main::value_receiver_form(main: &mut Main) -> i32 {
            let seen: i32 = main.write_both(&mut main.count);
            (seen)
        }",
    );
    assert_receiver_write_rejected(
        "data Main { count: i32; }
        machine Main::write_then_read(&mut self, seen: &i32) -> i32 {
            self.count = 7;
            (seen)
        }
        machine Main::value_receiver_form(main: &mut Main) -> i32 {
            let value: i32 = main.write_then_read(&main.count);
            (value)
        }",
    );
}

#[test]
fn receiver_write_beside_disjoint_storage_view_accepts() {
    // A different field, and a subslice disjoint from the written element.
    assert_accepted(
        "data Main { arr: [i32; 5]; count: i32; }
        machine Main::bump_then_read(&mut self, sub: &[i32]) -> i32 {
            self.count = 7;
            transition sub.len >= 1 { true -> (sub[0]) false -> 0 }
        }
        machine Main::value_receiver_form(main: &mut Main) -> i32 {
            let seen: i32 = main.bump_then_read(main.arr[0..2]);
            (seen)
        }",
    );
    assert_accepted(&format!(
        "{OVERWRITE_THEN_READ}
        machine Main::value_receiver_form(main: &mut Main) -> i32 {{
            let seen: i32 = main.overwrite_then_read(main.arr[1..3]);
            (seen)
        }}"
    ));
}

#[test]
fn receiver_write_beside_a_copied_scalar_of_the_written_field_accepts() {
    for caller in [
        "machine Main::value_receiver_form(main: &mut Main) { main.set(main.count); }",
        "machine Main::self_receiver_form(&mut self) { self.set(self.count + 1); }",
    ] {
        assert_accepted(&format!(
            "data Main {{ count: i32 in Wrapping; }}
            machine Main::set(&mut self, value: i32 in Wrapping) {{ self.count = value; }}
            {caller}"
        ));
    }
}
