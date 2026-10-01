//! A whole `&mut` receiver writes through the callee while the same call's
//! retained argument loans are live; disjoint storage and copied arguments
//! remain usable beside it.

use super::check_program;

const RECEIVER_WRITE: &str = "while its mutable receiver writes";

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
