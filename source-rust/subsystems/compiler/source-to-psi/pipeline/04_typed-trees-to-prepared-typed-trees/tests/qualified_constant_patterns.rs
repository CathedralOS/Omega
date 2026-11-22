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
//! In this compiler the fact has two parts on the prepared trees. The pattern
//! `east\ON` arrives as the Boolean value written at that use, and the use
//! carries a selection that names the declaration `east\ON`. The declaration
//! keeps the same value in `evaluated_value`. Stage 05's side of this handoff
//! is tested in `05_prepared-typed-trees-to-lowered-program/tests/prepared_boolean_patterns.rs`.

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
            let path = |symbol| trees.symbols.display_path(symbol, "\\");

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
                    ("east\\ON".to_owned(), east),
                    ("west\\ON".to_owned(), !east)
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
            assert_eq!(demanded, ["east\\ON", "west\\ON"], "{initializer:?}");

            // Each use names the declaration of the path written there and
            // holds that declaration's value. The literal `true` in `plain`
            // is not among them: it selects no constant.
            let uses = constant_uses(trees);
            let [eastern, western] = uses.as_slice() else {
                panic!("expected the two qualified patterns, found {uses:?}")
            };
            for (constant, value) in [(eastern, east), (western, !east)] {
                assert_eq!(path(constant.selected), constant.written, "{initializer:?}");
                assert_eq!(
                    constant.value, value,
                    "{initializer:?} {}",
                    constant.written
                );
                let declaration = trees
                    .const_declaration_by_symbol(constant.selected)
                    .unwrap();
                assert_eq!(declared_value(declaration), value, "{initializer:?}");
            }
            assert_eq!(eastern.written, "east\\ON");
            assert_eq!(western.written, "west\\ON");
            assert_ne!(eastern.selected, western.selected);
        }
    }
}
