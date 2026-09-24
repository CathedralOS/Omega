//! Owned indexed leaves stop at the same lowering boundary a literal spells:
//! `lower_machine` reports the same refusal for `items[1]` and `items[0 + 1]`,
//! so the widened fold adds no new downstream divergence. Both now plan
//! through checking and stop where lowering meets the fixed-array literal the
//! locals are built from; the folded spelling is not the gap.

use checked_trees_to_lowered_psi::TerminalMachineSelection;

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
/// leaf a literal does — and both then meet the same consumer-side boundary.
#[test]
fn folded_index_faces_the_literal_lowering_boundary() {
    let refusal = |label: &str, source: &str| {
        let checked = crate::front_end::checked_program_result(source)
            .unwrap_or_else(|errors| panic!("{label}: {errors:#?}"));
        format!(
            "{:?}",
            checked_trees_to_lowered_psi::lower_machine(
                &checked,
                TerminalMachineSelection::Name("choose")
            )
            .unwrap_err()
        )
    };
    let literal = refusal("literal", LITERAL_SOURCE);
    assert_eq!(
        literal,
        r#"Unsupported("fixed array literal has no Terminal establishment")"#,
        "an owned indexed leaf stops at the fixed-array literal it reads"
    );
    assert_eq!(
        refusal("folded", FOLDED_SOURCE),
        literal,
        "the folded index meets the literal's boundary"
    );
}
