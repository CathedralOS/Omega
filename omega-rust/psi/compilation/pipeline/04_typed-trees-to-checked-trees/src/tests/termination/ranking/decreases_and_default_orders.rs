use crate::tests::front_end::checked_program_result;

#[test]
fn rejects_terminating_slice_distance_machine_with_stalled_index() {
    let source = r#"
    data Entry {
        value: i32;
    }

    data Main {
        entries: [Entry; 4];
    }

    machine Main::main(&mut self) {
        let view: &[Entry] = self.entries.as_slice();
        let value: u64 = self.walk(view, 0);
    }

    machine Main::walk(&mut self, entries: &[Entry], index: u64)
    terminates by (index, entries.len) -> Nat::BoundedDistance;
    -> u64
    {
        transition index < entries.len {
            true -> self.walk(entries, index)
            false -> index
        }
    }
    "#;

    let diagnostics = checked_program_result(source).expect_err("termination check should fail");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove the `terminates by` ranking")
    }));
}

#[test]
fn rejects_named_bounded_distance_view_over_single_subject() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.countdown(2);
    }

    machine Main::countdown(&mut self, remaining: u64)
    terminates by remaining -> Nat::BoundedDistance;
    {
        transition remaining > 0 {
            true -> self.countdown(remaining - 1)
            false -> 0
        }
    }
    "#;

    let diagnostics =
        checked_program_result(source).expect_err("the view ranks a (lower, upper) pair only");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove the `terminates by` ranking")
    }));
}
