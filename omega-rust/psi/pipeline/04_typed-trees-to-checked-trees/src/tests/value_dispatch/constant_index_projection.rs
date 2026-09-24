//! A constant-folded index names the same fixed ordinal a literal spells:
//! the custody gate follows `constant_integer_value`, and the checker then
//! bakes the folded ordinal into a `FixedIndex` segment, so `items[0 + 1]`
//! joins under the identical projection rule as `items[1]`.

use super::check;

#[test]
fn constant_folded_index_projects_an_owned_leaf() {
    for (label, source) in [
        (
            "local roots",
            "data Cell { tag: u64; }
             data Pack { items: [Cell; 2]; }
             machine choose(flag: bool) -> u64 {
                 let left: Pack = Pack { items: [Cell { tag: 41 }, Cell { tag: 42 }] };
                 let right: Pack = Pack { items: [Cell { tag: 43 }, Cell { tag: 44 }] };
                 let picked: Cell = match flag { true -> left.items[0 + 1], false -> right.items[1] };
                 picked.tag
             }",
        ),
        (
            "parameter roots",
            "data Cell { tag: u64; }
             data Pack { items: [Cell; 2]; }
             machine choose(flag: bool, x: Pack, y: Pack) -> u64 {
                 let picked: Cell = match flag { true -> x.items[4 - 3], false -> y.items[0] };
                 picked.tag
             }",
        ),
        (
            "nested fold under a field",
            "data Cell { tag: u64; }
             data Pack { items: [Cell; 2]; }
             data Shelf { pack: Pack; }
             machine choose(flag: bool, shelf: Shelf) -> u64 {
                 let picked: Cell = match flag { true -> shelf.pack.items[(6 / 3) - 1], false -> shelf.pack.items[0] };
                 picked.tag
             }",
        ),
    ] {
        check(source).unwrap_or_else(|errors| panic!("{label}: {errors:#?}"));
    }
}

/// The widened rule still names only fixed ordinals: a place read or a
/// partially dynamic fold has no statically checkable position, and a
/// constant fold that leaves `usize` (a negative ordinal) has no ordinal at
/// all — the join keeps rejecting them.
#[test]
fn non_ordinal_index_still_rejects_the_join() {
    for (label, index) in [
        ("place read", "slot"),
        ("partially dynamic fold", "1 + slot"),
        ("negative fold", "0 - 1"),
    ] {
        let source = format!(
            "data Cell {{ tag: u64; }}
             data Pack {{ items: [Cell; 2]; }}
             machine choose(flag: bool, slot: u64, x: Pack, y: Pack) -> u64 {{
                 let picked: Cell = match flag {{ true -> x.items[{index}], false -> y.items[1] }};
                 picked.tag
             }}"
        );
        let errors = check(&source)
            .expect_err("{label}: `{index}` has no fixed ordinal and must keep rejecting");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("custody join")),
            "{label}: {errors:#?}"
        );
    }
}
