//! This crate is the checked publication boundary's consumer. The authored
//! borrow certificate ledgers travel inside the checked trees as records:
//! the producing pass's public replay entry checks every published family
//! here, so post-publication evidence is checkable rather than trusted.

use super::checked_source;

const PREMISED_WRITE: &str = r#"
    data Main { items: [i32; 4]; }

    machine Main::main(&mut self, i: u64, cut: u64) -> u64
        requires i < cut && cut <= 4 && i < 4;
    {
        let left: &mut [i32] = self.items[cut..4];
        self.items[i] = 7;
        left.len
    }
"#;

// A call-established premise carried through a mutable result binding must
// replay post-publication like the immutable spelling.
const MUTABLE_RESULT_PREMISED_WRITE: &str = r#"
    data Main { items: [i32; 4]; }

    machine choose(value: u64 [2..=4]) -> u64 [0..=4]
        ensures result >= 2;
    { value }

    machine Main::main(&mut self, seed: u64 [2..=4]) -> u64 {
        let mut split_point: u64 [0..=4] = choose(seed);
        let left: &mut [i32] = self.items[split_point..4];
        self.items[0] = 7;
        left.len
    }
"#;

#[test]
fn published_borrow_certificates_replay_at_the_lowering_boundary() {
    for source in [PREMISED_WRITE, MUTABLE_RESULT_PREMISED_WRITE] {
        let checked = checked_source(source);
        assert!(
            checked
                .facts
                .borrow
                .mutation_certificates
                .iter()
                .next()
                .is_some(),
            "the premised write must author mutation certificates"
        );
        typed_trees_to_checked_trees::replay_checked_borrow_certificates(
            &checked.typed,
            &checked.facts,
        )
        .expect("published borrow certificates replay independently");
    }
}
