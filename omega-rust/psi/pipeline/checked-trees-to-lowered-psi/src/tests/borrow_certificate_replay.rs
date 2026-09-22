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

// A segmented premise bound — a member projection of an immutable
// parameter — must replay post-publication like the whole-value spellings.
const PROJECTED_PREMISED_WRITE: &str = r#"
    data Pair { first: u64 [0..=4]; second: u64; }
    data Main { items: [i32; 4]; }

    machine Main::main(&mut self, pair: Pair) -> u64
        requires pair.first >= 2;
    {
        let left: &mut [i32] = self.items[pair.first..4];
        self.items[0] = 7;
        left.len
    }
"#;

// A guarantee established on a statement-site call's exclusive-borrow arg
// (the callee's write is the establishment) must replay post-publication
// like the expression-site spellings.
const STATEMENT_CALL_PREMISED_WRITE: &str = r#"
    data Main { items: [i32; 4]; }

    machine ordain(slot: &mut u64 [0..=4]) ensures slot >= 2 { slot = 2; }

    machine Main::main(&mut self) -> u64 {
        let mut cut: u64 [0..=4] = 0;
        ordain(&mut cut);
        let held: &mut [i32] = self.items[cut..4];
        self.items[0] = 3;
        held.len
    }
"#;

// A domain-membership premise on a projected (member) subject — a segmented
// premise bound — must replay post-publication like the bare-Name spelling.
const PROJECTED_DOMAIN_PREMISED_WRITE: &str = r#"
    domain u64::Upper requires self >= 2;
    data Pair { first: u64 [0..=4]; second: u64; }
    data Main { items: [i32; 4]; }

    machine Main::main(&mut self, pair: Pair) -> u64
        requires pair.first in u64::Upper;
    {
        let held: &mut [i32] = self.items[pair.first..4];
        self.items[0] = 3;
        held.len
    }
"#;

// A statement-site guarantee established through a member-of-self exclusive
// borrow actual — the attached-field's canonical identity — must replay
// post-publication like the bare-local spelling.
const STATEMENT_CALL_MEMBER_PREMISED_WRITE: &str = r#"
    data Main { items: [i32; 4]; cut: u64 [0..=4]; }

    machine ordain(slot: &mut u64 [0..=4]) ensures slot >= 2 { slot = 2; }

    machine Main::main(&mut self) -> u64 {
        self.cut = 0;
        ordain(&mut self.cut);
        let held: &mut [i32] = self.items[self.cut..4];
        self.items[0] = 3;
        held.len
    }
"#;

#[test]
fn published_borrow_certificates_replay_at_the_lowering_boundary() {
    for source in [
        PREMISED_WRITE,
        MUTABLE_RESULT_PREMISED_WRITE,
        PROJECTED_PREMISED_WRITE,
        STATEMENT_CALL_PREMISED_WRITE,
        PROJECTED_DOMAIN_PREMISED_WRITE,
        STATEMENT_CALL_MEMBER_PREMISED_WRITE,
    ] {
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
