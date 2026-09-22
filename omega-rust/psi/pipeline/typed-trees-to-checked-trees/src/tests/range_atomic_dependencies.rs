use crate::tests::front_end::checked_program_result;

fn check(source: &str, accepted: bool) {
    match checked_program_result(source) {
        Ok(_) => assert!(accepted, "stale atomic bounds accepted: {source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic.is_error()),
                "expected an error: {diagnostics:#?}\n{source}"
            );
            assert!(
                diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.is_error())
                    .all(|diagnostic| diagnostic.message.contains("cannot prove subslice range")),
                "expected only subslice range errors, not earlier proof or authority failures: \
                 {diagnostics:#?}\n{source}"
            );
        }
    }
}

// A `place.load(ordering)` observation reads exactly its resident place, so a
// premise on the load survives writes that cannot reach that place and is
// retired by any write that can.
#[test]
fn atomic_store_retires_only_the_resident_place_premise() {
    fn source(mutation: &str) -> String {
        format!(
            "data Host {{ counter: AtomicU32; other: AtomicU32; unrelated: i64; }}
             machine Host::window(&mut self, items: &[i32; 4]) -> u64
             requires 0 <= self.counter.load(NoOrdering)
                 && self.counter.load(NoOrdering) <= 4; {{
                 {mutation}
                 let view: &[i32] = items[0..self.counter.load(NoOrdering)];
                 view.len
             }}"
        )
    }
    for (mutation, accepted) in [
        ("", true),
        ("self.unrelated = 1;", true),
        ("self.other.store(9, NoOrdering);", true),
        ("self.counter.store(9, NoOrdering);", false),
    ] {
        check(&source(mutation), accepted);
    }
}

// Every writing atomic axis is an assignment carrier: the resident place it
// writes is the carrier's target, and a premise on that place is retired by
// the operation while a premise on a sibling survives. The stored operand's
// own reads do not leak into the write — a swap that reads `counter` but
// writes `other` leaves the `counter` premise intact.
#[test]
fn writing_atomic_axes_retire_only_the_resident_place_premise() {
    fn source(mutation: &str) -> String {
        format!(
            "data Host {{ counter: AtomicU32; other: AtomicU32; unrelated: i64; }}
             machine Host::window(&mut self, items: &[i32; 4]) -> u64
             requires 0 <= self.counter.load(NoOrdering)
                 && self.counter.load(NoOrdering) <= 4; {{
                 {mutation}
                 let view: &[i32] = items[0..self.counter.load(NoOrdering)];
                 view.len
             }}"
        )
    }
    for (mutation, accepted) in [
        // A store writes the resident place only.
        ("self.other.store(9, NoOrdering);", true),
        ("self.counter.store(9, NoOrdering);", false),
        // A swap reads the resident prior into its result local and writes the
        // replacement; the resident place is still the carrier target.
        ("let prior: u32 = self.other.swap(9, NoOrdering);", true),
        ("let prior: u32 = self.counter.swap(9, NoOrdering);", false),
        // The replacement operand's reads stay on the read side: this swap
        // reads `counter` through `load` but writes `other`.
        (
            "let prior: u32 = self.other.swap(self.counter.load(NoOrdering), NoOrdering);",
            true,
        ),
        // Read-modify-write and decisive compare-exchange follow the same
        // carrier contract.
        (
            "let prior: u32 = self.other.fetch_add(1, NoOrdering);",
            true,
        ),
        (
            "let prior: u32 = self.counter.fetch_add(1, NoOrdering);",
            false,
        ),
        (
            "let prior: u32 = self.other.compare_exchange(1, 9, NoOrdering, NoOrdering);",
            true,
        ),
        (
            "let prior: u32 = self.counter.compare_exchange(1, 9, NoOrdering, NoOrdering);",
            false,
        ),
    ] {
        check(&source(mutation), accepted);
    }
}
