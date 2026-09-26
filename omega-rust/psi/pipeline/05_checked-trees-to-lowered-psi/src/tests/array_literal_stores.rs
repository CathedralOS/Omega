//! A whole array literal stored into a primitive-array field lowers as one
//! primitive store per element, and those stores must rejoin the literal as
//! its complete, ordered element roster.

use super::lower_machine;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_psi::OperationKind;
use typed_trees_to_checked_trees::checked_trees::{CheckedTrees, CheckedUnitEffectOperationPlan};

const SOURCE: &str = r#"
    data Board { cells: [u16; 3]; }
    machine Board::reset(&mut self) { self.cells = [4, 7, 9]; }
"#;

fn lower(checked: &CheckedTrees) -> Result<usize, String> {
    lower_machine(checked, TerminalMachineSelection::Name("Board::reset"))
        .map(|lowered| {
            lowered
                .semantic_module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .filter(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::WriteOnlyPrimitiveStore { .. }
                    )
                })
                .count()
        })
        .map_err(|error| format!("{error:?}"))
}

fn edit_element_stores(
    checked: &mut CheckedTrees,
    edit: impl Fn(&mut Vec<CheckedUnitEffectOperationPlan>),
) {
    let plans = &mut checked.facts.flow.terminal_unit_effects.machines;
    let plan = plans
        .iter_mut()
        .find(|plan| {
            plan.operations.iter().any(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                )
            })
        })
        .expect("the literal plans element stores");
    edit(&mut plan.operations);
}

fn element_positions(operations: &[CheckedUnitEffectOperationPlan]) -> Vec<usize> {
    operations
        .iter()
        .enumerate()
        .filter(|(_, operation)| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
            )
        })
        .map(|(position, _)| position)
        .collect()
}

#[test]
fn an_array_literal_store_lowers_one_primitive_store_per_element() {
    let checked = crate::front_end::checked_program(SOURCE);
    assert_eq!(lower(&checked), Ok(3));
}

#[test]
fn an_array_literal_roster_missing_an_element_is_rejected() {
    let mut checked = crate::front_end::checked_program(SOURCE);
    edit_element_stores(&mut checked, |operations| {
        let last = *element_positions(operations).last().unwrap();
        operations.remove(last);
    });
    let error = lower(&checked).expect_err("a dropped element store must not lower");
    assert!(error.contains("complete roster"), "{error}");
}

#[test]
fn an_array_literal_roster_out_of_element_order_is_rejected() {
    let mut checked = crate::front_end::checked_program(SOURCE);
    edit_element_stores(&mut checked, |operations| {
        let positions = element_positions(operations);
        operations.swap(positions[0], positions[1]);
    });
    let error = lower(&checked).expect_err("reordered element stores must not lower");
    assert!(error.contains("complete roster"), "{error}");
}
