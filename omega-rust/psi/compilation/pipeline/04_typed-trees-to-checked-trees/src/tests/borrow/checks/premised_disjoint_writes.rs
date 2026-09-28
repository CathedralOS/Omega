//! Stated `requires` ordering premises discharge mutation and call-access
//! conflicts against a live borrow when they prove the written index disjoint
//! from the borrowed window. The premise is read-only relational evidence: it
//! cannot extend a lifetime, mint a loan, or move an index inside the borrowed
//! extent.

use super::check_program;

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
