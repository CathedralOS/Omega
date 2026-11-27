//! Stage 04 must hand stage 05 the constant each array extent named and that
//! constant's value. Lowering reads both from this output and never looks a
//! name up again, so a wrong pairing here becomes an array of the wrong
//! length.
//!
//! The spec rules are in `wiki/spec/language/constants.md`, "Evaluation": "A
//! constant position requests semantic evaluation. An initializer may call an
//! ordinary machine only when that concrete invocation is admitted for
//! evaluation". An array length is such a position, and
//! `wiki/spec/language/counts_and_addresses.md`, "Bounds and lowering", fixes
//! what the value means: "For a fixed array `[T; N]` it yields exactly `N`."
//!
//! The program has two constants with the same last name `N` and different
//! values, so each assertion below fails if an extent is paired with the
//! other module's constant.
//!
//! Resolution evaluates a literal initializer. The source driver evaluates a
//! call after typing. Both array types retain the selected constant, so
//! lowering reads the value from the same place in either case.

#[path = "support/constant_array_extents.rs"]
mod program;

use build_model::values::BuildTimeValue;
use program::{
    INITIALIZERS, Initializer, MACHINES, declared_value, extent_uses, literal_extents, path,
    prepare,
};
use source_compiler::representations::source_state::types::FixedArrayLength;
use source_compiler::test_support::preparation::PreparedValue;
use source_completion_accounting::{PureEvaluationRecord, PureTerminalEvaluationOutcome};

#[test]
fn each_extent_arrives_with_its_own_declaration_and_length() {
    for initializer in INITIALIZERS {
        for (east, west) in [(2, 3), (3, 2)] {
            let prepared = prepare(east, west, initializer, &MACHINES).program;
            let trees = prepared.trees();

            // The request needs exactly the two constants, each frozen with
            // the value its own module gave it.
            let mut constants: Vec<_> = prepared
                .constants()
                .map(|declaration| (path(trees, declaration.symbol), declared_value(declaration)))
                .collect();
            constants.sort();
            assert_eq!(
                constants,
                [
                    ("east/value.omg:N".to_owned(), Some(east)),
                    ("west/value.omg:N".to_owned(), Some(west))
                ],
                "{initializer:?}"
            );
            let mut demanded: Vec<_> = prepared
                .values()
                .iter()
                .map(|input| match input.value {
                    PreparedValue::Constant(symbol) => path(trees, symbol),
                    other => panic!("expected a named constant, found {other:?}"),
                })
                .collect();
            demanded.sort();
            assert_eq!(
                demanded,
                ["east/value.omg:N", "west/value.omg:N"],
                "{initializer:?}"
            );

            // Each extent names the declaration of the path written there
            // and reads that declaration's value as its length.
            let uses = extent_uses(trees);
            let [eastern, western] = uses.as_slice() else {
                panic!("expected the two extents that name a constant, found {uses:?}")
            };
            for (extent, length, owner) in [
                (eastern, east, "east/value.omg:N"),
                (western, west, "west/value.omg:N"),
            ] {
                assert_eq!(path(trees, extent.selected), owner, "{initializer:?}");
                assert_eq!(
                    extent.length,
                    Some(length),
                    "{initializer:?} {}",
                    extent.written
                );
                let declaration = trees.const_declaration_by_symbol(extent.selected).unwrap();
                assert_eq!(declared_value(declaration), Some(length), "{initializer:?}");
                assert!(matches!(extent.form, FixedArrayLength::ConstParameter { symbol, .. }
                    if symbol == extent.selected), "{:?}", extent.form);
            }
            assert_eq!(eastern.written, "|east|::N");
            assert_eq!(western.written, "|west|::N");
            assert_ne!(eastern.selected, western.selected);

            // `pair`, `triple` and `plain` wrote their extents as literals.
            // Those arrays name no constant.
            assert_eq!(literal_extents(trees), [2, 2, 3], "{initializer:?}");
        }
    }
}

/// Typing cannot know the length of `[u8; east\N]` when `east\N` is computed
/// by a call. It records which constant the extent names and gives the array
/// no length. The source driver's `SourcePreparationRequest::settle`
/// (`driver/prepare_typed_source.rs`) then runs each constant the request
/// reaches, once, before stage 04 freezes the trees. The values 2 and 3 first
/// exist in that step.
#[test]
fn typing_names_the_constant_and_the_source_driver_evaluates_it_before_stage_04() {
    for (east, west) in [(2, 3), (3, 2)] {
        let prepared = prepare(east, west, Initializer::Call, &MACHINES);

        // Typing says the same about both extents whatever the calls return:
        // each names its declaration, and neither the declaration nor the
        // array has a value.
        let typed = extent_uses(&prepared.typed);
        let [eastern, western] = typed.as_slice() else {
            panic!("expected the two extents that name a constant, found {typed:?}")
        };
        for (extent, owner) in [(eastern, "east/value.omg:N"), (western, "west/value.omg:N")] {
            assert_eq!(path(&prepared.typed, extent.selected), owner);
            assert!(
                matches!(extent.form, FixedArrayLength::ConstParameter { symbol, .. }
                    if symbol == extent.selected)
            );
            assert_eq!(extent.length, None);
            let declaration = prepared
                .typed
                .const_declaration_by_symbol(extent.selected)
                .unwrap();
            assert_eq!(declared_value(declaration), None);
        }

        // Preparation ran one evaluation for each constant and nothing else.
        let mut evaluated: Vec<_> = prepared
            .accounting
            .receipts()
            .iter()
            .map(|record| {
                let PureEvaluationRecord::Invocation(receipt) = record else {
                    panic!("expected a machine invocation, found {record:?}")
                };
                let PureTerminalEvaluationOutcome::Materialized { value, .. } = receipt.outcome()
                else {
                    panic!("the evaluation returned no value: {:?}", receipt.outcome())
                };
                (path(&prepared.typed, receipt.requester()), value.clone())
            })
            .collect();
        evaluated.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(
            evaluated,
            [
                (
                    "east/value.omg:N".to_owned(),
                    BuildTimeValue::Int(i64::try_from(east).unwrap())
                ),
                (
                    "west/value.omg:N".to_owned(),
                    BuildTimeValue::Int(i64::try_from(west).unwrap())
                ),
            ]
        );

        // The same array types now have the evaluated lengths. Their handles
        // and the declarations they name did not change.
        let frozen = extent_uses(prepared.program.trees());
        let [east_frozen, west_frozen] = frozen.as_slice() else {
            panic!("expected the two extents that name a constant, found {frozen:?}")
        };
        for (before, after, length) in [(eastern, east_frozen, east), (western, west_frozen, west)]
        {
            assert_eq!(after.array, before.array);
            assert_eq!(after.selected, before.selected);
            assert_eq!(after.form, before.form);
            assert_eq!(after.length, Some(length));
        }
    }
}

/// A request that reaches only `choose` asks for `east\N` and nothing of
/// `west\N`. The constant with the same last name stays unevaluated, and the
/// extent that names it stays without a length.
#[test]
fn a_request_that_reaches_one_extent_leaves_the_other_constant_unevaluated() {
    let prepared = prepare(2, 3, Initializer::Call, &["choose"]);
    let frozen = prepared.program;
    let trees = frozen.trees();
    let constants: Vec<_> = frozen
        .constants()
        .map(|declaration| (path(trees, declaration.symbol), declared_value(declaration)))
        .collect();
    assert_eq!(constants, [("east/value.omg:N".to_owned(), Some(2))]);
    assert_eq!(prepared.accounting.receipts().len(), 1);

    let uses = extent_uses(trees);
    let [eastern, western] = uses.as_slice() else {
        panic!("expected the two extents that name a constant, found {uses:?}")
    };
    assert_eq!(eastern.length, Some(2));
    assert_eq!(western.length, None);
    let decoy = trees.const_declaration_by_symbol(western.selected).unwrap();
    assert_eq!(path(trees, decoy.symbol), "west/value.omg:N");
    assert_eq!(declared_value(decoy), None);
}
