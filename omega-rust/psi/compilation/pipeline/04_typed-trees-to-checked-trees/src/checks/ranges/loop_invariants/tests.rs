//! Whose storage a monotone counter may live in.
//!
//! The inference recognized a counter, bound and collection only by the
//! receiver spelling, so the same loop stopped being a candidate the moment
//! its data arrived as a state parameter. These pin the root-neutral
//! admission and the two refusals that keep it from being a bare spelling
//! test: the root has to be the one binding the declaration reaches the
//! attached data through.

use crate::tests::front_end::checked_program_result;

fn rejections(source: &str) -> Vec<String> {
    match checked_program_result(source) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect(),
    }
}

fn index_is_proven(source: &str) -> bool {
    !rejections(source)
        .iter()
        .any(|message| message.contains("cannot prove index"))
}

/// `i` starts at 0 in the preheader and only increments, so `i >= 0` holds at
/// every loop state and `nums[i]` owes no authored `>= 0` guard. The receiver
/// form is the control; the other two are the same loop with the data named
/// explicitly, including the migrated shape where the entry keeps its
/// receiver and lends it into states that name the data.
#[test]
fn a_monotone_counter_proves_non_negative_through_either_storage_root() {
    let receiver = "data Main { nums: [i32; 2]; last: i32; i: i32; }
        machine Main::main(&mut self) {
            self.i = 0;
            transition { _ -> step() }
            state step(&mut self) {
                transition self.i < 2 { true -> add() _ -> done() }
            }
            state add(&mut self) {
                self.last = self.nums[self.i];
                self.i = self.i + 1;
                transition { _ -> step() }
            }
            state done() { }
        }";
    let parameter = "data Main { nums: [i32; 2]; last: i32; i: i32; }
        machine Main::main(main: &mut Main) {
            main.i = 0;
            transition { _ -> step(main) }
            state step(main: &mut Main) {
                transition main.i < 2 { true -> add(main) _ -> done() }
            }
            state add(main: &mut Main) {
                main.last = main.nums[main.i];
                main.i = main.i + 1;
                transition { _ -> step(main) }
            }
            state done() { }
        }";
    let mixed = "data Main { nums: [i32; 2]; last: i32; i: i32; }
        machine Main::main(&mut self) {
            self.i = 0;
            transition { _ -> step(&mut self) }
            state step(main: &mut Main) {
                transition main.i < 2 { true -> add(main) _ -> done() }
            }
            state add(main: &mut Main) {
                main.last = main.nums[main.i];
                main.i = main.i + 1;
                transition { _ -> step(main) }
            }
            state done() { }
        }";
    for (label, source) in [
        ("receiver", receiver),
        ("data parameter", parameter),
        ("mixed roots", mixed),
    ] {
        assert!(
            index_is_proven(source),
            "{label}: the monotone counter's own non-negativity must prove the \
             index: {:?}",
            rejections(source)
        );
    }
}

/// Two parameters of the attached data are two instances, so no single
/// binding is the declaration's view of its storage and the loop has no
/// counter candidate at all. Picking either one would analyze writes to one
/// instance as writes to the other.
#[test]
fn two_attached_parameters_leave_the_counter_unrecognized() {
    let source = "data Main { nums: [i32; 2]; last: i32; i: i32; }
        machine Main::main(main: &mut Main, spare: &mut Main) {
            main.i = 0;
            transition { _ -> step(main, spare) }
            state step(main: &mut Main, spare: &mut Main) {
                transition main.i < 2 { true -> add(main, spare) _ -> done() }
            }
            state add(main: &mut Main, spare: &mut Main) {
                main.last = main.nums[main.i];
                main.i = main.i + 1;
                transition { _ -> step(main, spare) }
            }
            state done() { }
        }";
    assert!(
        !index_is_proven(source),
        "an ambiguous storage root must not seed a counter invariant"
    );
}

/// A record parameter that is not this machine's attached data is ordinary
/// state-local storage: it does not survive a transition as the machine's
/// own, so a counter inside it is not a machine-wide loop invariant.
#[test]
fn a_foreign_record_parameter_is_not_this_machines_storage() {
    let source = "data Other { nums: [i32; 2]; last: i32; i: i32; }
        data Main { tick: i32; }
        machine Main::main(&mut self, other: &mut Other) {
            other.i = 0;
            transition { _ -> step(other) }
            state step(other: &mut Other) {
                transition other.i < 2 { true -> add(other) _ -> done() }
            }
            state add(other: &mut Other) {
                other.last = other.nums[other.i];
                other.i = other.i + 1;
                transition { _ -> step(other) }
            }
            state done() { }
        }";
    assert!(
        !index_is_proven(source),
        "a non-attached record parameter must not seed a counter invariant"
    );
}
