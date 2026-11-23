//! Stage 04 must hand stage 05 the constant each pattern named and that
//! constant's value. Lowering reads both from this output and never looks a
//! name up again, so a wrong pairing here becomes a wrong branch.
//!
//! The spec rule is "To test a constant by value, write its qualified path"
//! (`wiki/spec/language/patterns.md`, "Bare names in dispatch arms"). The
//! program has two constants with the same last name `ON` and opposite
//! values, so each assertion below fails if a use is paired with the other
//! module's constant.
//!
//! A pattern keeps its selected declaration. Each declaration supplies its
//! evaluated value to lowering.

#[path = "support/qualified_boolean_constants.rs"]
mod program;

use program::{INITIALIZERS, constant_uses, declared_value, prepare};
use source_compiler::test_support::preparation::PreparedValue;

#[test]
fn each_qualified_pattern_arrives_with_its_own_declaration_and_value() {
    for initializer in INITIALIZERS {
        for east in [true, false] {
            let prepared = prepare(east, initializer).program;
            let trees = prepared.trees();
            let path = |symbol| {
                let span = trees
                    .symbols
                    .symbol_source_span(symbol)
                    .expect("constant declaration span");
                let sources = trees.symbols.source_map().unwrap();
                let file = sources.file_at(span).unwrap();
                format!(
                    "{}:{}",
                    file.logical_filename().unwrap(),
                    trees.symbols.name(symbol)
                )
            };

            // The request needs exactly the two constants, each frozen with
            // the value its own module gave it.
            let mut constants: Vec<_> = prepared
                .constants()
                .map(|declaration| (path(declaration.symbol), declared_value(declaration)))
                .collect();
            constants.sort();
            assert_eq!(
                constants,
                [
                    ("east/value.omg:ON".to_owned(), east),
                    ("west/value.omg:ON".to_owned(), !east)
                ],
                "{initializer:?}"
            );
            let mut demanded: Vec<_> = prepared
                .values()
                .iter()
                .map(|input| match input.value {
                    PreparedValue::Constant(symbol) => path(symbol),
                    other => panic!("expected a named constant, found {other:?}"),
                })
                .collect();
            demanded.sort();
            assert_eq!(
                demanded,
                ["east/value.omg:ON", "west/value.omg:ON"],
                "{initializer:?}"
            );

            // Each use names the declaration of the path written there. The literal `true` in `plain`
            // is not among them: it selects no constant.
            let uses = constant_uses(trees);
            let [eastern, western] = uses.as_slice() else {
                panic!("expected the two qualified patterns, found {uses:?}")
            };
            for (constant, value, owner) in [
                (eastern, east, "east/value.omg:ON"),
                (western, !east, "west/value.omg:ON"),
            ] {
                assert_eq!(path(constant.selected), owner, "{initializer:?}");
                let declaration = trees
                    .const_declaration_by_symbol(constant.selected)
                    .unwrap();
                assert_eq!(declared_value(declaration), value, "{initializer:?}");
            }
            assert_eq!(eastern.written, "|east|::ON");
            assert_eq!(western.written, "|west|::ON");
            assert_ne!(eastern.selected, western.selected);
        }
    }
}
