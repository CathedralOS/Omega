//! A custom collection's selected `[]` operator keeps its `requires`.
//!
//! The ranges seam discharges indexing over builtin array and slice geometry
//! only. A nominal collection has no storage-bound judgment there, so its
//! selected operator's preconditions are proven by the selected-operator
//! discharge over the actual operands — never dropped, never refused outright.

use crate::tests::front_end::checked_program_result;

fn program(requires: &str) -> String {
    format!(
        "data Buffer<Element, const Count: u64> {{}}
        data Position {{}}
        data Indexing {{}}
        boundary machine [] Indexing::index<Element, const Count: u64>(
            items: Buffer<Element, Count>,
            position: Position
        ) -> i32
        requires {requires};
        data IndexingProvider {{}}
        machine IndexingProvider::index<Value, const Length: u64>(
            items: Buffer<Value, Length>,
            position: Position
        ) -> i32
        satisfies Indexing::index
        {{
            7
        }}
        machine consume(items: Buffer<i32, 4>, position: Position) {{
            let selected: i32 = items[position];
        }}"
    )
}

#[test]
fn an_unprovable_custom_index_requires_still_rejects_by_name() {
    // The use reaches the selected operator as a value call into the
    // specialization `Count == 4` selected, so the rejection is the value-call
    // discharge rather than the spelled-operator one. Both name the clause its
    // author wrote: a reader given only the substituted `4 == 0` cannot find
    // the `requires` it came from, and the specialization's own minted name is
    // the compiler's spelling, never a reader's.
    let errors = checked_program_result(&program("Count == 0"))
        .expect_err("an unprovable requires on a custom collection must not disappear");
    let named = errors
        .iter()
        .any(|error| error.to_string().contains("`Count == 0`"));
    assert!(named, "{errors:#?}");
    assert!(
        errors
            .iter()
            .all(|error| !error.to_string().contains("$specialized$")),
        "{errors:#?}"
    );
}
