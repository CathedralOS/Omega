//! Typing must hand preparation the field each place of a positional
//! pattern selected. The parser recorded only the place
//! (`01_tokens-to-syntax-trees/tests/positional_patterns.rs`); after typing
//! nothing records it any more, so every later stage reads the field typing
//! chose and cannot notice a wrong one.
//!
//! The spec rule is "A positional pattern binds fields by position; binding
//! names need not match declaration names. These positions are semantic
//! declaration order" (`wiki/spec/language/patterns.md`, "Data and case
//! decomposition"), with "positional patterns have the corresponding exact
//! arity. An explicit `..` may waive the remaining components."
//!
//! In this compiler the fact has two parts on the typed trees. Each read of
//! a binding is a member with the selected field's symbol and name, and the
//! pattern's roster has one row per field with that field's symbol and name. The
//! program in `support/positional_fields.rs` binds place 0 under the name
//! of the field at place 1, so each assertion below fails if a name was
//! looked up where a place should have been counted. Lowering's side of
//! this handoff is tested in
//! `05_prepared-typed-trees-to-lowered-program/tests/positional_field_reads.rs`.

#[path = "support/positional_fields.rs"]
mod program;

use program::{MAIN, arm_roster, field, field_reads, let_pattern, let_roster, typed};
use symbols::SymbolHandle;

#[test]
fn each_place_reads_the_field_declared_at_that_place() {
    let (_, trees) = typed(MAIN).unwrap();
    let (zeta, alpha) = (field(&trees, "zeta"), field(&trees, "alpha"));
    assert_ne!(zeta, alpha);

    // One read per binding, in source order. `written` is what the source
    // says where the read came from: the component of a `let` pattern, or
    // the use of an arm's binding.
    let reads: Vec<_> = field_reads(&trees)
        .into_iter()
        .map(|read| (read.written, read.member, read.selected))
        .collect();
    assert_eq!(
        reads,
        [
            // `let (alpha, _) = pair;`: place 0 is `zeta`, whatever it is called.
            ("alpha", "zeta".to_owned(), zeta),
            // `let (first, ..) = pair;`
            ("first", "zeta".to_owned(), zeta),
            // `(_, second) -> second`: place 1 is `alpha`.
            ("second", "alpha".to_owned(), alpha),
            // `let Pair { zeta as _, alpha as named } = pair;` reads by name.
            ("", "alpha".to_owned(), alpha),
        ]
    );
}

#[test]
fn each_roster_names_one_row_per_field_and_no_place() {
    let (_, trees) = typed(MAIN).unwrap();
    let (zeta, alpha) = (field(&trees, "zeta"), field(&trees, "alpha"));
    let row = |name: &str, symbol: SymbolHandle, binds: Option<&str>| {
        (name.to_owned(), symbol, binds.map(str::to_owned))
    };

    assert_eq!(
        let_roster(&trees, "by_place"),
        [row("zeta", zeta, Some("alpha")), row("alpha", alpha, None)]
    );
    // `..` became one waived row for each field no component took, so the
    // roster accounts for every field exactly as a full pattern does.
    assert_eq!(
        let_roster(&trees, "with_rest"),
        [row("zeta", zeta, Some("first")), row("alpha", alpha, None)]
    );
    assert_eq!(
        arm_roster(&trees),
        (
            vec![row("zeta", zeta, None), row("alpha", alpha, Some("second"))],
            false
        )
    );
    // A named pattern carries the same field identities as positional rows.
    assert_eq!(
        let_roster(&trees, "by_name"),
        [row("zeta", zeta, None), row("alpha", alpha, Some("named"))]
    );
    let pair = trees.data_definition_by_name_str("Pair").unwrap().symbol;
    for name in ["by_place", "with_rest", "by_name"] {
        assert_eq!(let_pattern(&trees, name).pattern_subject, pair, "{name}");
    }
}

#[test]
fn named_patterns_select_the_record_behind_a_borrow_field_path_or_block_local() {
    let (_, trees) = typed(program::REFERENCED).unwrap();
    let pair = trees.data_definition_by_name_str("Pair").unwrap().symbol;
    for name in ["through_borrow", "through_field", "through_block"] {
        assert_eq!(let_pattern(&trees, name).pattern_subject, pair, "{name}");
        let roster = let_roster(&trees, name);
        assert_eq!(roster[0].1, field(&trees, "zeta"), "{name}");
        assert_eq!(roster[1].1, field(&trees, "alpha"), "{name}");
    }
}

/// A pattern whose subject gives its places no field is refused here, at the
/// pattern, because no later stage could still place it.
#[test]
fn a_pattern_its_subject_cannot_place_is_refused_at_the_pattern() {
    const DECLARATIONS: &str = "data Pair [copy] { zeta: u8; alpha: u8; }
data Other [copy] { zeta: u8; alpha: u8; }
data Shape [copy] { case Round(radius: u8); case Flat; }
";
    for (body, says, at) in [
        (
            "let (only,) = pair; only",
            "this pattern has 1 component, but `Pair` has 2 fields",
            "only",
        ),
        (
            "let (a, b, c) = pair; b",
            "this pattern has 3 components, but `Pair` has only 2 fields",
            "c",
        ),
        (
            "let (a, b, c, ..) = pair; b",
            "this pattern has 3 components, but `Pair` has only 2 fields",
            "c",
        ),
        (
            "match pair { (a, b, c) -> b, }",
            "this pattern has 3 components, but `Pair` has only 2 fields",
            "c",
        ),
        (
            "match pair { (only,) -> only, }",
            "this pattern has 1 component, but `Pair` has 2 fields",
            "only",
        ),
        (
            "let (a, b) = shape; b",
            "`shape` is the sum `Shape`",
            "a, b",
        ),
        (
            "match shape { (a, ..) -> a, }",
            "`shape` is the sum `Shape`",
            "a, ..",
        ),
        ("let (a, b) = count; b", "`count` is not a record", "a, b"),
        (
            "let Other { zeta as _, alpha } = pair; alpha",
            "this pattern names `Other`, but `pair` is a `Pair`",
            "Other",
        ),
        (
            "let Shape { radius } = shape; radius",
            "`Shape` is a sum",
            "Shape",
        ),
        (
            "let { missing, .. } = pair; missing",
            "`Pair` has no field `missing`",
            "missing",
        ),
    ] {
        let text: &'static str = Box::leak(
            format!(
                "{DECLARATIONS}machine read(pair: Pair, shape: Shape, count: u8) -> u8 {{ {body} }}"
            )
            .into_boxed_str(),
        );
        let (message, found) = typed(text).expect_err(body);
        assert!(message.contains(says), "{body}: {message}");
        assert_eq!(found, at, "{body}: {message}");
    }
}
