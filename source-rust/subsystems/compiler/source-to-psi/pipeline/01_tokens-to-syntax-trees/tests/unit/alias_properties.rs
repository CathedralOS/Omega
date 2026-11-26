//! What the parser records for the properties written on a type alias.
//!
//! An alias "may assert its target's multiplicity under the ordinary property
//! rules. A carry list is invalid on both the alias and its target"
//! (`wiki/spec/language/generics.md#transparent-type-aliases`). The parser
//! cannot know the expansion's multiplicity, so the fact it owes typing is the
//! assertion as written; typing compares it with the expansion.

use language_core::Multiplicity;
use syntax_trees::item::Item;

fn asserted(source: &str) -> Option<Multiplicity> {
    let syntax = crate::tests::parse_source(source);
    syntax
        .root_items()
        .find_map(|item| match item {
            Item::TypeAlias(alias) => Some(alias.asserted_multiplicity),
            _ => None,
        })
        .expect("one alias")
}

#[test]
fn an_alias_records_the_multiplicity_it_asserts() {
    assert_eq!(
        asserted("data Count [copy] = u64;"),
        Some(Multiplicity::Unrestricted)
    );
    assert_eq!(
        asserted("data Held [linear] = Lease;"),
        Some(Multiplicity::Linear)
    );
    assert_eq!(asserted("data Count = u64;"), None);
}

/// A carry list is refused wherever it is written, before the alias rule is
/// read (`properties_and_requirements.rs`); a cleanup is refused by the alias
/// rule.
#[test]
fn an_alias_with_a_carry_list_or_a_cleanup_is_refused() {
    for (source, rule) in [
        (
            "data Held [carry(suspension: forbidden, cpu: same, thread: any, address: stable)] = u8;",
            "`carry(...)` is not a type property",
        ),
        (
            "data Held [linear, carry(suspension: forbidden, cpu: same, thread: any, address: stable)] = u8;",
            "`carry(...)` is not a type property",
        ),
        (
            "data Held dropped by finish = Lease;",
            "it may assert only `copy` or `linear`",
        ),
    ] {
        let error = crate::tests::parse_error(source);
        assert!(error.message.contains(rule), "{source}: {}", error.message);
    }
}
