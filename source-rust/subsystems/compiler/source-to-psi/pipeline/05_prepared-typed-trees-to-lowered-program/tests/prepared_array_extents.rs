//! Stage 05 must give each array the length stage 04 prepared for its
//! extent, and refuse an array passed where another length is required.
//! These tests keep the source fixed, change one prepared fact as a faulty
//! producer would, and require the lowered result to change with it.
//!
//! The spec rule is `wiki/spec/language/counts_and_addresses.md`, "Bounds
//! and lowering": "For a fixed array `[T; N]` it yields exactly `N`." A
//! computed `N` gets its value by evaluation (`wiki/spec/language/constants.md`,
//! "Evaluation"). The program and what stage 04 hands over are described and
//! asserted in
//! `04_typed-trees-to-prepared-typed-trees/tests/computed_array_extents.rs`.
//!
//! In the source every test uses, `|east|::N` is 2 and `|west|::N` is 3. `choose`
//! passes an array of `|east|::N` bytes where two are required, and `other`
//! passes an array of `|west|::N` bytes where three are required, so the program
//! is valid as written.
//!
//! `PreparedTypedTrees` cannot be built or edited outside stage 04. Each
//! altered product here is made by taking the prepared trees back with
//! `into_inner`, changing one fact, and running stage 04 again, so stage 05
//! still receives a product only stage 04 could issue.

#[path = "../../04_typed-trees-to-prepared-typed-trees/tests/support/constant_array_extents.rs"]
mod program;

use program::{
    ExtentUse, Initializer, MACHINES, MAIN, compilation, extent_uses, machine, passed_argument,
    prepare,
};
use source_compiler::representations::source_state::TypedTrees;
use source_compiler::representations::source_state::constant::ConstDeclaration;
use source_compiler::representations::source_state::types::{FixedArrayLength, TypeReferenceNode};
use source_compiler::test_support::lowering::{self, Elaboration};
use source_compiler::test_support::preparation::{self, PreparedTypedTrees};
use symbols::SymbolHandle;
use terminal_check_tables::compilation::TerminalCompilation;
use terminal_check_tables::{Refusal, RefusalReason};
use terminal_psi::{ArrayExtent, StructuralAccess, StructuralTypeShape};

/// The source every test here uses: `|east|::N` is 2 and `|west|::N` is 3.
fn fixture(initializer: Initializer) -> PreparedTypedTrees {
    prepare(2, 3, initializer, &MACHINES).program
}

/// What stage 05 makes of `name` as the request's only entry: the length of
/// the array it borrows as its parameter, or the refusal.
///
/// Each machine is lowered on its own so that a refusal in one cannot hide
/// what stage 05 does with the others.
fn lowered_extent(program: &PreparedTypedTrees, name: &str) -> Result<u64, Refusal> {
    let entry = [machine(program.trees(), name)];
    let lowered = lowering::prepare_compilation(
        program,
        compilation(&entry),
        terminal_codec::TerminalPublication::Portable,
    )
    .unwrap();
    let TerminalCompilation::Prepared(lowered) = lowered.program else {
        panic!("the program binds no callable shape that could fail to form")
    };
    lowered
        .closure
        .map(|lowered| parameter_extent(&lowered, name))
}

/// The length of the array the entry borrows as its one parameter.
fn parameter_extent(lowered: &Elaboration, name: &str) -> u64 {
    let entry = &lowered.module.machines[0];
    let [parameter] = entry.structural_parameters.as_slice() else {
        panic!("`{name}` takes one array")
    };
    assert_eq!(parameter.access, StructuralAccess::SharedBorrow, "{name}");
    let array = lowered
        .module
        .types
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parameter.structural_type)
        .unwrap();
    let StructuralTypeShape::FixedArray {
        length: ArrayExtent::Literal(length),
        ..
    } = array.shape
    else {
        panic!(
            "`{name}` borrows an array of a known length: {:?}",
            array.shape
        )
    };
    length
}

/// `[choose, other, plain]`, each lowered on its own.
fn lowered_extents(program: &PreparedTypedTrees) -> [Result<u64, Refusal>; 3] {
    MACHINES.map(|name| lowered_extent(program, name))
}

/// Requires the refusal stage 05 gives `name` for passing an array of
/// `supplied` elements where `wanted` are required: a violation of the
/// program, reported at the argument, in those words.
fn assert_wrong_length(result: &Result<u64, Refusal>, name: &str, wanted: u64, supplied: u64) {
    let refusal = result
        .as_ref()
        .expect_err("an array of the wrong length was accepted");
    assert_eq!(refusal.reason, RefusalReason::Violation, "{refusal:?}");
    assert_eq!(
        refusal.detail,
        format!(
            "the parameter it is passed for expects an array with {wanted} elements \
             but the supplied array has {supplied} while reading the body of `{name}`"
        )
    );
    let span = refusal
        .source_span
        .expect("the refusal names its place")
        .span;
    assert_eq!(span.start..span.end, passed_argument(name), "{refusal:?}");
    assert_eq!(&MAIN[span.start..span.end], "values");
}

/// Stage 04's product for the same trees after `change`. The source files
/// and the constants named as inputs stay as they were.
fn prepared_with(
    prepared: &PreparedTypedTrees,
    change: impl FnOnce(&mut TypedTrees),
) -> PreparedTypedTrees {
    let values = prepared.values().to_vec();
    let mut trees = prepared.clone().into_inner();
    change(&mut trees);
    preparation::prepare_typed_trees(trees, &values, &[]).unwrap()
}

fn declaration_mut(trees: &mut TypedTrees, symbol: SymbolHandle) -> &mut ConstDeclaration {
    let handle = trees
        .roots
        .const_declarations
        .handles()
        .find(|&handle| trees.tables.const_declarations.get(handle).symbol == symbol)
        .unwrap();
    trees.tables.const_declarations.get_mut(handle)
}

/// Gives the constant `changed` the evaluated value of `source`, as the
/// source driver records one: the value and the initializer materialized
/// from it. Nothing else about either constant changes.
fn set_evaluated_value(trees: &mut TypedTrees, changed: SymbolHandle, source: SymbolHandle) {
    let substitute = declaration_mut(trees, source).clone();
    let declaration = declaration_mut(trees, changed);
    declaration.evaluated_value = substitute.evaluated_value;
    declaration.materialized_initializer = substitute.materialized_initializer;
}

/// Rewrites how one array type states its extent.
fn set_extent(trees: &mut TypedTrees, extent: &ExtentUse, length: FixedArrayLength) {
    let TypeReferenceNode::FixedArray { element_type, .. } =
        trees.type_reference_table.type_reference(extent.array)
    else {
        panic!("an extent belongs to an array type")
    };
    let element_type = *element_type;
    trees.type_reference_table.substitute_node(
        extent.array,
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        },
    );
}

fn both_extents(prepared: &PreparedTypedTrees) -> [ExtentUse; 2] {
    extent_uses(prepared.trees()).try_into().unwrap()
}

#[test]
fn an_unchanged_product_lowers_every_array_at_its_prepared_length() {
    for initializer in program::INITIALIZERS {
        let prepared = fixture(initializer);
        assert_eq!(
            lowered_extents(&prepared),
            [Ok(2), Ok(3), Ok(2)],
            "{initializer:?}"
        );

        // Control: stage 04 run again with nothing changed issues the same
        // product, so a later difference comes from the one change.
        let unchanged = prepared_with(&prepared, |_| {});
        assert_eq!(unchanged, prepared);
        assert_eq!(
            lowered_extents(&unchanged),
            [Ok(2), Ok(3), Ok(2)],
            "{initializer:?}"
        );
    }
}

/// The handoff this file is about. The source still says `extent()` returns
/// 2 for `|east|::N`; only the value recorded on that declaration becomes 3.
/// `choose` now passes three bytes where `pair` requires two, and stage 05
/// refuses it at that argument. `other` names the constant with the same last
/// name and `plain` names none, and both lower as before.
#[test]
fn changing_the_evaluated_extent_refuses_the_use_that_requires_the_old_length() {
    let prepared = fixture(Initializer::Call);
    let [eastern, western] = both_extents(&prepared);

    let east_is_three = prepared_with(&prepared, |trees| {
        set_evaluated_value(trees, eastern.selected, western.selected)
    });
    let [choose, other, plain] = lowered_extents(&east_is_three);
    assert_wrong_length(&choose, "choose", 2, 3);
    assert_eq!([other, plain], [Ok(3), Ok(2)]);

    // The same change to the other constant refuses only the other use.
    let west_is_two = prepared_with(&prepared, |trees| {
        set_evaluated_value(trees, western.selected, eastern.selected)
    });
    let [choose, other, plain] = lowered_extents(&west_is_two);
    assert_wrong_length(&other, "other", 3, 2);
    assert_eq!([choose, plain], [Ok(2), Ok(2)]);
}

/// A producer that paired an extent with the constant of the same last name
/// in the other module would hand over this product. Both declarations keep
/// their values; only the declaration the extent names changes. Stage 05
/// reads the length through that declaration, so the swapped use is refused.
#[test]
fn an_extent_that_names_the_other_constant_takes_that_constant_s_length() {
    let prepared = fixture(Initializer::Call);
    let [eastern, western] = both_extents(&prepared);

    let choose_names_west = prepared_with(&prepared, |trees| {
        set_extent(trees, &eastern, western.form.clone())
    });
    let [choose, other, plain] = lowered_extents(&choose_names_west);
    assert_wrong_length(&choose, "choose", 2, 3);
    assert_eq!([other, plain], [Ok(3), Ok(2)]);

    let other_names_east = prepared_with(&prepared, |trees| {
        set_extent(trees, &western, eastern.form.clone())
    });
    let [choose, other, plain] = lowered_extents(&other_names_east);
    assert_wrong_length(&other, "other", 3, 2);
    assert_eq!([choose, plain], [Ok(2), Ok(2)]);
}

/// An extent that names a constant with no evaluated value has no length.
/// Stage 05 refuses the machine that holds it as something it cannot lower.
/// It does not pick a length, and it does not call the program wrong.
#[test]
fn an_extent_whose_constant_has_no_value_is_not_given_a_length() {
    let prepared = fixture(Initializer::Call);
    let [eastern, _] = both_extents(&prepared);
    let unevaluated = prepared_with(&prepared, |trees| {
        declaration_mut(trees, eastern.selected).evaluated_value = None;
    });
    let [choose, other, plain] = lowered_extents(&unevaluated);
    let refusal = choose.expect_err("an extent with no value was given a length");
    assert_eq!(refusal.reason, RefusalReason::Unsupported, "{refusal:?}");
    assert!(
        refusal
            .detail
            .contains("array extent that names a constant with no evaluated array length"),
        "{refusal:?}"
    );
    assert_eq!([other, plain], [Ok(3), Ok(2)]);
}

#[test]
fn an_array_sized_by_a_literal_constant_reads_the_selected_declaration() {
    let prepared = fixture(Initializer::Literal);
    let [eastern, western] = both_extents(&prepared);
    assert!(
        matches!(eastern.form, FixedArrayLength::ConstParameter { symbol, .. } if symbol == eastern.selected)
    );
    let changed = prepared_with(&prepared, |trees| {
        set_evaluated_value(trees, eastern.selected, western.selected)
    });
    let [choose, other, plain] = lowered_extents(&changed);
    assert_wrong_length(&choose, "choose", 2, 3);
    assert_eq!([other, plain], [Ok(3), Ok(2)]);
}
