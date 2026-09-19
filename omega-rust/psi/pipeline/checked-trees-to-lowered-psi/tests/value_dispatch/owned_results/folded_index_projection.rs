//! Owned indexed leaves stop at the same lowering boundary a literal spells:
//! `lower_machine` reports the missing source-independent scalar plan for
//! `items[1]` and `items[0 + 1]` alike, so the widened fold adds no new
//! downstream divergence — the remaining gap is owned `FixedIndex` leaf
//! emission, not the folded spelling.

use crate::value_dispatch::check_source;

const LITERAL_SOURCE: &str = "data Cell { tag: u64; }
    data Pack { items: [Cell; 2]; }
    machine choose(flag: bool) -> u64 {
        let left: Pack = Pack { items: [Cell { tag: 41 }, Cell { tag: 42 }] };
        let right: Pack = Pack { items: [Cell { tag: 43 }, Cell { tag: 44 }] };
        let picked: Cell = match flag { true -> left.items[1], false -> right.items[0] };
        picked.tag
    }";

const FOLDED_SOURCE: &str = "data Cell { tag: u64; }
    data Pack { items: [Cell; 2]; }
    machine choose(flag: bool) -> u64 {
        let left: Pack = Pack { items: [Cell { tag: 41 }, Cell { tag: 42 }] };
        let right: Pack = Pack { items: [Cell { tag: 43 }, Cell { tag: 44 }] };
        let picked: Cell = match flag { true -> left.items[0 + 1], false -> right.items[0] };
        picked.tag
    }";

/// Both spellings check — the fold produces exactly the checked `FixedIndex`
/// leaf a literal does — and both then meet the documented consumer-side
/// boundary identically.
#[test]
fn folded_index_faces_the_literal_lowering_boundary() {
    for (label, source) in [("literal", LITERAL_SOURCE), ("folded", FOLDED_SOURCE)] {
        let checked = check_source(source).unwrap_or_else(|errors| panic!("{label}: {errors:#?}"));
        assert_eq!(
            format!(
                "{:?}",
                checked_trees_to_lowered_psi::lower_machine(&checked, "choose").unwrap_err()
            ),
            r#"Unsupported("machine has no source-independent checked scalar control plan")"#,
            "{label}: an owned indexed leaf meets the scalar-plan boundary, same as a literal"
        );
    }
}
