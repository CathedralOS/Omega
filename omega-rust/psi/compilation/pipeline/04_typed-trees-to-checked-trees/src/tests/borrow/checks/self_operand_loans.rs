//! A bare `self` operand forwarded to a reference formal retains the
//! receiver binding's loan through the invocation, as a named reference
//! binding does. Its access is rooted at the machine while `self.field`
//! operands are rooted at the attached field, so the pairwise argument check
//! must compare the two in the receiver's namespace to see them overlap.

use super::check_program;

const TAKE: &str = "data Main { arr: [i32; 5]; }
    machine Main::take(main: &mut Main, sub: &[i32]) -> i32 {
        main.arr[0] = 7;
        transition sub.len >= 1 { true -> (sub[0]) false -> 0 }
    }";

fn assert_accepted(source: &str) {
    check_program(source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
}

#[test]
fn bare_self_operand_retains_its_mutable_loan_beside_a_view_of_self() {
    let source = format!(
        "{TAKE}
        machine Main::run(&mut self) -> i32 {{
            let seen: i32 = Main::take(self, self.arr[0..2]);
            (seen)
        }}"
    );
    let diagnostics =
        check_program(&source).expect_err("take writes arr[0] through main while sub views it");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("state `take` receives `self` as both mutable and read-only")),
        "{diagnostics:#?}"
    );
}

#[test]
fn bare_self_operand_beside_a_view_of_another_root_accepts() {
    assert_accepted(&format!(
        "{TAKE}
        machine Main::run(&mut self, other: &Main) -> i32 {{
            let seen: i32 = Main::take(self, other.arr[0..2]);
            (seen)
        }}"
    ));
}

#[test]
fn bare_self_operand_to_a_shared_formal_beside_a_view_of_self_accepts() {
    assert_accepted(
        "data Main { arr: [i32; 5]; }
        machine Main::observe(main: &Main, sub: &[i32]) -> i32 {
            transition sub.len >= 1 { true -> (main.arr[1]) false -> 0 }
        }
        machine Main::run(&mut self) -> i32 {
            let seen: i32 = Main::observe(self, self.arr[0..2]);
            (seen)
        }",
    );
}

#[test]
fn bare_self_operand_to_a_shared_formal_beside_an_exclusive_view_of_self_rejects() {
    let source = "data Main { arr: [i32; 5]; }
        machine Main::poke(main: &Main, sub: &mut [i32]) -> i32 {
            transition sub.len >= 1 { true -> write(main, sub) false -> 0 }
            state write(main: &Main, sub: &mut [i32]) -> i32 requires sub.len >= 1 {
                sub[0] = 7;
                (main.arr[0])
            }
        }
        machine Main::run(&mut self) -> i32 {
            let seen: i32 = Main::poke(self, &mut self.arr[0..2]);
            (seen)
        }";
    let diagnostics =
        check_program(source).expect_err("poke writes arr[0] through sub while main observes it");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("state `poke` receives `arr[0..2]` as both mutable and read-only")),
        "{diagnostics:#?}"
    );
}
