//! A whole `&mut` receiver reads through the callee while the same call's
//! exclusive argument loans suspend the storage they lend. Reads of disjoint
//! storage, reads beside shared loans, and copies evaluated before the call
//! remain usable.

use super::check_program;

fn assert_receiver_read_rejected(source: &str, expected: &str) {
    let diagnostics =
        check_program(source).expect_err("the receiver read overlaps an exclusive argument loan");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected)),
        "{source}: {diagnostics:#?}"
    );
}

fn assert_accepted(source: &str) {
    check_program(source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
}

const COUNT_READ: &str =
    "receives `self.count` as mutable while its mutable receiver reads `self.count`";

fn with_caller(callee: &str, argument: &str) -> String {
    format!(
        "data Inner {{ count: u64; }}
        data Main {{ count: u64; other: u64; arr: [u64; 4]; inner: Inner; }}
        machine Inner::peek(&self) -> u64 {{ (self.count) }}
        machine Main::peek_count(&self) -> u64 {{ (self.count) }}
        machine Main::peek_other(&self) -> u64 {{ (self.other) }}
        machine observe(value: &u64) -> u64 {{ (value) }}
        {callee}
        machine Main::main(&mut self) -> u64 {{
            let seen: u64 = self.callee({argument});
            (seen)
        }}"
    )
}

#[test]
fn receiver_read_rejects_the_field_an_exclusive_argument_lends() {
    for callee in [
        // Reading after the argument write observes the aliased store.
        "machine Main::callee(&mut self, slot: &mut u64) -> u64 {
            slot = 5;
            let seen: u64 = self.count;
            (seen)
        }",
        // Order does not matter: the loan suspends the field for the call.
        "machine Main::callee(&mut self, slot: &mut u64) -> u64 {
            let seen: u64 = self.count;
            slot = 5;
            (seen)
        }",
        // So does a callee that never writes through the loan.
        "machine Main::callee(&mut self, slot: &mut u64) -> u64 {
            (self.count)
        }",
        // A receiver helper's own read is the callee's read.
        "machine Main::callee(&mut self, slot: &mut u64) -> u64 {
            slot = 5;
            (self.peek_count())
        }",
        // So is a view of the field lent to another machine or kept local.
        "machine Main::callee(&mut self, slot: &mut u64) -> u64 {
            slot = 5;
            (observe(&self.count))
        }",
        "machine Main::callee(&mut self, slot: &mut u64) -> u64 {
            let view: &u64 = &self.count;
            slot = 5;
            (view)
        }",
        // A guard and a successor state run inside the same invocation.
        "machine Main::callee(&mut self, slot: &mut u64) -> u64 {
            slot = 5;
            transition self.count > 3 { true -> 1 false -> 0 }
        }",
        "machine Main::callee(&mut self, slot: &mut u64) -> u64 {
            slot = 5;
            transition { _ -> finish() }
            state finish(&mut self) -> u64 { (self.count) }
        }",
    ] {
        assert_receiver_read_rejected(&with_caller(callee, "&mut self.count"), COUNT_READ);
    }
}

#[test]
fn receiver_read_rejects_value_receivers_and_write_only_loans() {
    assert_receiver_read_rejected(
        "data Main { count: u64; }
        machine Main::callee(&mut self, slot: &mut u64) -> u64 {
            slot = 5;
            (self.count)
        }
        machine Main::value_receiver_form(main: &mut Main) -> u64 {
            let seen: u64 = main.callee(&mut main.count);
            (seen)
        }",
        "receives `main.count` as mutable while its mutable receiver reads `main.count`",
    );
    assert_receiver_read_rejected(
        "data Main { count: u64; }
        machine Main::callee(&mut self, slot: &write u64) -> u64 {
            slot = 5;
            (self.count)
        }
        machine Main::main(&mut self) -> u64 {
            let seen: u64 = self.callee(&write self.count);
            (seen)
        }",
        "receives `self.count` as write-only while its mutable receiver reads `self.count`",
    );
}

#[test]
fn receiver_read_rejects_a_projected_helper_read_of_the_lent_child() {
    assert_receiver_read_rejected(
        &with_caller(
            "machine Main::callee(&mut self, slot: &mut u64) -> u64 {
                slot = 5;
                (self.inner.peek())
            }",
            "&mut self.inner.count",
        ),
        "receives `self.inner.count` as mutable while its mutable receiver reads `self.inner.count`",
    );
}

#[test]
fn receiver_read_rejects_the_element_an_exclusive_argument_lends() {
    assert_receiver_read_rejected(
        &with_caller(
            "machine Main::callee(&mut self, slot: &mut u64) -> u64 {
                slot = 5;
                (self.arr[0])
            }",
            "&mut self.arr[0]",
        ),
        "receives `self.arr[0]` as mutable while its mutable receiver reads `self.arr[0]`",
    );
}

#[test]
fn receiver_reads_of_storage_disjoint_from_the_exclusive_loan_accept() {
    for (callee, argument) in [
        (
            "machine Main::callee(&mut self, slot: &mut u64) -> u64 {
                slot = 5;
                (self.other)
            }",
            "&mut self.count",
        ),
        (
            "machine Main::callee(&mut self, slot: &mut u64) -> u64 {
                slot = 5;
                (self.peek_other())
            }",
            "&mut self.count",
        ),
        (
            "machine Main::callee(&mut self, slot: &mut u64) -> u64 {
                slot = 5;
                (self.inner.peek())
            }",
            "&mut self.count",
        ),
        (
            "machine Main::callee(&mut self, slot: &mut u64) -> u64 {
                slot = 5;
                transition { _ -> finish() }
                state finish(&mut self) -> u64 { (self.other) }
            }",
            "&mut self.count",
        ),
        (
            "machine Main::callee(&mut self, slot: &mut u64) -> u64 {
                slot = 5;
                (self.arr[3])
            }",
            "&mut self.arr[0]",
        ),
    ] {
        assert_accepted(&with_caller(callee, argument));
    }
}

#[test]
fn receiver_read_beside_a_shared_loan_or_a_copy_of_the_field_accepts() {
    // Shared loans and reads coexist; a copy ends before invocation.
    for (callee, argument) in [
        (
            "machine Main::callee(&mut self, seen: &u64) -> u64 {
                self.other = 1;
                (self.count)
            }",
            "&self.count",
        ),
        (
            "machine Main::callee(&mut self, value: u64) -> u64 {
                self.other = value;
                (self.count)
            }",
            "self.count",
        ),
    ] {
        assert_accepted(&with_caller(callee, argument));
    }
}

#[test]
fn shared_receiver_already_rejects_an_exclusive_child_argument() {
    assert_receiver_read_rejected(
        &with_caller(
            "machine Main::callee(&self, slot: &mut u64) -> u64 {
                slot = 5;
                (self.count)
            }",
            "&mut self.count",
        ),
        "receives shared receiver overlapping another argument in the same call",
    );
}
