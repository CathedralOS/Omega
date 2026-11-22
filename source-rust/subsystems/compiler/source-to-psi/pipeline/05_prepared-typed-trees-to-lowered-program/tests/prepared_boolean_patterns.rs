//! Stage 05 must branch on the constant value stage 04 prepared for each
//! pattern, and on nothing else. These tests keep the source fixed, change
//! one prepared fact as a faulty producer would, and require the lowered
//! branch to change with it.
//!
//! The spec rule is "To test a constant by value, write its qualified path"
//! (`wiki/spec/language/patterns.md`, "Bare names in dispatch arms"). The
//! program and what stage 04 hands over are described and asserted in
//! `04_typed-trees-to-prepared-typed-trees/tests/qualified_constant_patterns.rs`.
//!
//! `PreparedTypedTrees` cannot be built or edited outside stage 04. Each
//! altered product here is made by taking the prepared trees back with
//! `into_inner`, changing one fact, and running stage 04 again, so stage 05
//! still receives a product only stage 04 could issue.

#[path = "../../04_typed-trees-to-prepared-typed-trees/tests/support/qualified_boolean_constants.rs"]
mod program;

use program::{ConstantUse, INITIALIZERS, MACHINES, Prepared, constant_uses, prepare};
use semantic_vocabulary::IntegerValue;
use source_compiler::representations::source_state::TypedTrees;
use source_compiler::representations::source_state::constant::ConstDeclaration;
use source_compiler::representations::source_state::expression::ExpressionNode;
use source_compiler::test_support::lowering::{self, Elaboration};
use source_compiler::test_support::preparation::{self, PreparedTypedTrees};
use symbols::SymbolHandle;
use terminal_check_tables::Refusal;
use terminal_check_tables::compilation::TerminalCompilation;
use terminal_psi::{OperationKind, OperationResult, Terminator};

/// Stage 05's lowered program for `program` under the fixture's request, or
/// its refusal.
fn lower(fixture: &Prepared, program: &PreparedTypedTrees) -> Result<Elaboration, Refusal> {
    let lowered = lowering::prepare_compilation(
        program,
        fixture.compilation(),
        terminal_codec::TerminalPublication::Portable,
    )
    .unwrap();
    let TerminalCompilation::Prepared(lowered) = lowered.program else {
        panic!("the program binds no callable shape that could fail to form")
    };
    lowered.closure
}

/// The Boolean each machine compares its input with, for `choose`, `other`
/// and `plain` in that order. Every machine must have the same shape: the
/// first block compares the input with one Boolean constant and branches;
/// an equal input returns 17 and any other input returns 23.
fn tested_values(lowered: &Elaboration, trees: &TypedTrees) -> [bool; 3] {
    MACHINES.map(|name| {
        let machine = lowered
            .module
            .machines
            .iter()
            .find(|machine| {
                trees
                    .symbols
                    .name(*lowered.machines.get(machine.id).unwrap())
                    == name
            })
            .unwrap();
        let block = |id| machine.blocks.iter().find(|block| block.id == id).unwrap();
        let [input] = machine.parameters.as_slice() else {
            panic!("`{name}` takes one Boolean input")
        };
        let entry = block(machine.entry);
        let [constant, equal] = entry.operations.as_slice() else {
            panic!(
                "`{name}` starts with one comparison: {:?}",
                entry.operations
            )
        };
        let (OperationKind::BooleanConstant { value }, OperationResult::Scalar(tested)) =
            (&constant.kind, &constant.result)
        else {
            panic!("`{name}` compares with a Boolean constant: {constant:?}")
        };
        assert_eq!(
            equal.kind,
            OperationKind::BooleanEqual {
                left: input.id,
                right: tested.id
            },
            "{name}"
        );
        let Terminator::Conditional {
            condition,
            when_true,
            when_false,
        } = &entry.terminator
        else {
            panic!(
                "`{name}` branches on the comparison: {:?}",
                entry.terminator
            )
        };
        assert!(
            matches!(&equal.result, OperationResult::Scalar(result) if result.id == *condition),
            "{name}"
        );
        for (edge, returned) in [(when_true, 17), (when_false, 23)] {
            let arm = block(edge.target);
            let [literal] = arm.operations.as_slice() else {
                panic!("`{name}` returns a literal: {:?}", arm.operations)
            };
            assert_eq!(
                literal.kind,
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(returned)
                },
                "{name}"
            );
            assert!(
                matches!(
                    (&arm.terminator, &literal.result),
                    (Terminator::Return { value, .. }, OperationResult::Scalar(result))
                        if result.id == *value
                ),
                "{name}"
            );
        }
        *value
    })
}

/// Stage 04's product for the same trees after `change`. The source files,
/// the request and the constants named as inputs stay as they were.
fn prepared_with(
    prepared: &PreparedTypedTrees,
    change: impl FnOnce(&mut TypedTrees),
) -> PreparedTypedTrees {
    let values = prepared.values().to_vec();
    let mut trees = prepared.clone().into_inner();
    change(&mut trees);
    preparation::prepare_typed_trees(trees, &values, &[]).unwrap()
}

fn set_use(trees: &mut TypedTrees, constant: &ConstantUse, value: bool) {
    *trees.expression_table.expression_mut(constant.expression) = ExpressionNode::Boolean(value);
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

fn both_uses(prepared: &PreparedTypedTrees) -> [ConstantUse; 2] {
    constant_uses(prepared.trees()).try_into().unwrap()
}

#[test]
fn swapping_the_prepared_values_reverses_both_branches() {
    for initializer in INITIALIZERS {
        for east in [true, false] {
            let fixture = prepare(east, initializer);
            let prepared = &fixture.program;
            let lowered = lower(&fixture, prepared).unwrap();
            assert_eq!(
                tested_values(&lowered, prepared.trees()),
                [east, !east, true],
                "{initializer:?}"
            );

            // Control: stage 04 run again with nothing changed issues the
            // same product, so a later difference comes from the one change.
            let unchanged = prepared_with(prepared, |_| {});
            assert_eq!(&unchanged, prepared);
            let lowered = lower(&fixture, &unchanged).unwrap();
            assert_eq!(
                tested_values(&lowered, unchanged.trees()),
                [east, !east, true],
                "{initializer:?}"
            );

            // Give each use the other constant's value. Both source modules
            // and both declarations still say what they said.
            let [eastern, western] = both_uses(prepared);
            let swapped = prepared_with(prepared, |trees| {
                set_use(trees, &eastern, !east);
                set_use(trees, &western, east);
            });
            let lowered = lower(&fixture, &swapped).unwrap();
            assert_eq!(
                tested_values(&lowered, swapped.trees()),
                [!east, east, true],
                "{initializer:?}"
            );
        }
    }
}

#[test]
fn changing_one_use_leaves_the_constant_with_the_same_last_name_alone() {
    for initializer in INITIALIZERS {
        let east = true;
        let fixture = prepare(east, initializer);
        let prepared = &fixture.program;
        let [eastern, western] = both_uses(prepared);
        for (changed, value, expected) in [
            (&eastern, !east, [!east, !east, true]),
            (&western, east, [east, east, true]),
        ] {
            let altered = prepared_with(prepared, |trees| set_use(trees, changed, value));
            let lowered = lower(&fixture, &altered).unwrap();
            assert_eq!(
                tested_values(&lowered, altered.trees()),
                expected,
                "{initializer:?} after changing {}",
                changed.written
            );
        }
    }
}

#[test]
fn a_declaration_value_that_contradicts_its_initializer_is_refused() {
    for initializer in INITIALIZERS {
        let fixture = prepare(true, initializer);
        let prepared = &fixture.program;
        let [eastern, western] = both_uses(prepared);
        // Give `east\ON` the value of `west\ON` and leave everything else,
        // including the initializer stage 05 compares that value with.
        let altered = prepared_with(prepared, |trees| {
            let substitute = declaration_mut(trees, western.selected)
                .evaluated_value
                .clone();
            declaration_mut(trees, eastern.selected).evaluated_value = substitute;
        });
        let refusal = lower(&fixture, &altered).unwrap_err();
        assert!(
            refusal
                .detail
                .contains("constant encoding and materialized carrier differ"),
            "{initializer:?}: {refusal:?}"
        );
    }
}

/// Stage 05 reads a pattern's value where the pattern is used and nowhere
/// else. It checks a declaration against that declaration's own initializer
/// (the test above) and never compares a use with its declaration. Here
/// `east\ON` is changed to `false` consistently, value and initializer,
/// while the pattern that names it still holds `true`: lowering accepts the
/// product and `choose` keeps comparing with `true`.
///
/// So the two copies agree only because the code that writes the use makes
/// them agree: name resolution for a literal, and the source driver's
/// evaluation for a call. Stage 04's test asserts the agreement on the
/// product; stage 05 would not notice its loss.
#[test]
fn a_use_takes_no_value_from_its_declaration() {
    for initializer in INITIALIZERS {
        let east = true;
        let fixture = prepare(east, initializer);
        let prepared = &fixture.program;
        let [eastern, western] = both_uses(prepared);
        let altered = prepared_with(prepared, |trees| {
            let substitute = declaration_mut(trees, western.selected).clone();
            let declaration = declaration_mut(trees, eastern.selected);
            declaration.evaluated_value = substitute.evaluated_value;
            declaration.materialized_initializer = substitute.materialized_initializer;
        });
        let lowered = lower(&fixture, &altered).unwrap();
        assert_eq!(
            tested_values(&lowered, altered.trees()),
            [east, !east, true],
            "{initializer:?}"
        );
    }
}
