//! Stated `requires` ordering premises discharge mutation and call-access
//! conflicts against a live borrow when they prove the written index disjoint
//! from the borrowed window. The premise is read-only relational evidence: it
//! cannot extend a lifetime, mint a loan, or move an index inside the borrowed
//! extent.

use super::check_program;

fn assert_still_active_rejection(source: &str) {
    let diagnostics = check_program(source)
        .expect_err("an unproven ordering must keep the write conservatively overlapping");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("is still active"),
        "expected a live-borrow conflict, got:\n{combined}"
    );
}

#[test]
fn stated_ordering_premise_admits_fixed_index_write_before_borrowed_window() {
    let source = r#"
        data Main { items: [i32; 4]; }

        machine Main::main(&mut self, cut: u64) -> u64
            requires 0 < cut && cut <= 4;
        {
            let left: &mut [i32] = self.items[cut..4];
            self.items[0] = 7;
            left.len
        }
    "#;

    check_program(source)
        .expect("`0 < cut` proves element 0 sits below the borrowed `[cut, 4)` window");
}

#[test]
fn stated_ordering_premise_admits_symbolic_index_write_before_borrowed_window() {
    let source = r#"
        data Main { items: [i32; 4]; }

        machine Main::main(&mut self, i: u64, cut: u64) -> u64
            requires i < cut && cut <= 4 && i < 4;
        {
            let left: &mut [i32] = self.items[cut..4];
            self.items[i] = 7;
            left.len
        }
    "#;

    check_program(source)
        .expect("`i < cut` proves the symbolic write precedes the borrowed window");
}

#[test]
fn stated_ordering_premise_admits_exclusive_argument_before_borrowed_window() {
    let source = r#"
        data Main { items: [i32; 4]; }

        machine take(slot: &mut i32) {
            slot = 7;
        }

        machine Main::main(&mut self, i: u64, cut: u64) -> u64
            requires i < cut && cut <= 4 && i < 4;
        {
            let left: &mut [i32] = self.items[cut..4];
            take(&mut self.items[i]);
            left.len
        }
    "#;

    check_program(source)
        .expect("`i < cut` proves the exclusive call operand disjoint from the borrowed window");
}

#[test]
fn stated_ordering_premise_admits_summed_index_write_before_borrowed_window() {
    // `i + j` is a two-symbol bound: `i + j < cut` proves the summed write
    // sits below the borrowed `[cut, 4)` window.
    let source = r#"
        data Main { items: [i32; 4]; }

        machine Main::main(&mut self, i: u64, j: u64, cut: u64) -> u64
            requires i + j < cut && cut <= 4 && i + j < 4;
        {
            let left: &mut [i32] = self.items[cut..4];
            self.items[i + j] = 7;
            left.len
        }
    "#;

    check_program(source)
        .expect("`i + j < cut` proves the summed write precedes the borrowed window");
}

#[test]
fn stated_ordering_premise_admits_summed_exclusive_argument_before_borrowed_window() {
    // `idx` is immutable, bound to `i + j`: the sum's terms come from the
    // local's initializer, so `i + j < cut` still discharges the borrow.
    let source = r#"
        data Main { items: [i32; 4]; }

        machine take(slot: &mut i32) {
            slot = 7;
        }

        machine Main::main(&mut self, i: u64, j: u64, cut: u64) -> u64
            requires i + j < cut && cut <= 4 && i + j < 4;
        {
            let idx: u64 = i + j;
            let left: &mut [i32] = self.items[cut..4];
            take(&mut self.items[idx]);
            left.len
        }
    "#;

    check_program(source).expect(
        "`i + j < cut` proves the summed exclusive operand disjoint from the borrowed window",
    );
}

#[test]
fn reordered_sum_premise_admits_the_same_canonical_bound() {
    // `j + i` normalizes to the same canonical sum as `i + j`, so a premise
    // stated on one spelling discharges a selector written in the other.
    let source = r#"
        data Main { items: [i32; 4]; }

        machine Main::main(&mut self, i: u64, j: u64, cut: u64) -> u64
            requires j + i < cut && cut <= 4 && i + j < 4;
        {
            let left: &mut [i32] = self.items[cut..4];
            self.items[i + j] = 7;
            left.len
        }
    "#;

    check_program(source)
        .expect("`j + i < cut` normalizes to the same sum and discharges `i + j < cut`");
}

#[test]
fn absent_ordering_premise_keeps_the_index_write_conflicting() {
    let source = r#"
        data Main { items: [i32; 4]; }

        machine Main::main(&mut self, i: u64, cut: u64) -> u64
            requires cut <= 4 && i < 4;
        {
            let left: &mut [i32] = self.items[cut..4];
            self.items[i] = 7;
            left.len
        }
    "#;

    assert_still_active_rejection(source);
}

#[test]
fn unproved_sum_ordering_keeps_the_index_write_conflicting() {
    // `i + j < 4` bounds the write without separating it from `[cut, 4)`;
    // only a premise relating the sum to `cut` can admit it.
    let source = r#"
        data Main { items: [i32; 4]; }

        machine Main::main(&mut self, i: u64, j: u64, cut: u64) -> u64
            requires cut <= 4 && i + j < 4;
        {
            let left: &mut [i32] = self.items[cut..4];
            self.items[i + j] = 7;
            left.len
        }
    "#;

    assert_still_active_rejection(source);
}

#[test]
fn non_strict_premise_cannot_admit_a_boundary_index_write() {
    // `i <= cut` still allows `i == cut`, which is inside `[cut, 4)`; only a
    // strict `i < cut` separates the point from the window start.
    let source = r#"
        data Main { items: [i32; 4]; }

        machine Main::main(&mut self, i: u64, cut: u64) -> u64
            requires i <= cut && cut <= 4 && i < 4;
        {
            let left: &mut [i32] = self.items[cut..4];
            self.items[i] = 7;
            left.len
        }
    "#;

    assert_still_active_rejection(source);
}

#[test]
fn stated_ordering_premise_cannot_move_the_write_inside_the_window() {
    // The same `i < cut` relation that disjoins `items[i]` from `[cut, 4)`
    // is containment evidence against `[0, cut)`: the write lands inside the
    // borrowed window and must still conflict.
    let source = r#"
        data Main { items: [i32; 4]; }

        machine Main::main(&mut self, i: u64, cut: u64) -> u64
            requires i < cut && cut <= 4 && i < 4;
        {
            let left: &mut [i32] = self.items[0..cut];
            self.items[i] = 7;
            left.len
        }
    "#;

    assert_still_active_rejection(source);
}
