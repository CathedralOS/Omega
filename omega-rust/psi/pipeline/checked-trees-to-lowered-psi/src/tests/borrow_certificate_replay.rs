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

#[test]
fn published_borrow_certificates_replay_at_the_lowering_boundary() {
    let checked = checked_source(PREMISED_WRITE);
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
