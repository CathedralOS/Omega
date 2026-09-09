//! Call results retain source identity independently of their shared array type.

use super::{checked_source, reject};
use checked_trees::{CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan};

fn fixture(tail: bool) -> CheckedTrees {
    let completion = if tail { "second_row()" } else { "first" };
    checked_source(&format!(
        "machine first_row() -> [u8; 2] {{ [7, 9] }}
         machine second_row() -> [u8; 2] {{ [11, 13] }}
         machine selected() -> [u8; 2] {{
             let before: [u8; 2] = [1, 2];
             let first: [u8; 2] = first_row();
             let second: [u8; 2] = second_row();
             let after: [u8; 2] = [3, 4];
             {completion}
         }}"
    ))
}

fn selected(checked: &mut CheckedTrees) -> &mut CheckedUnitEffectMachinePlan {
    let symbol = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "selected")
        .expect("selected source machine")
        .symbol;
    checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|machine| machine.machine == symbol)
        .expect("selected ordinary array body")
}

#[test]
fn boundary_array_results_require_payload_support_even_when_unused_or_empty() {
    for length in [0, 2] {
        for direct in [false, true] {
            let (declarations, call) = if direct {
                (
                    format!(
                        "boundary trait Factory {{}}
                         pub data Maker {{}}
                         boundary machine Maker::create() -> [u8; {length}]
                             reaches Factory ensures true;"
                    ),
                    "Maker::create()",
                )
            } else {
                (
                    format!(
                        "boundary trait Factory {{
                             machine create() -> [u8; {length}] reaches Factory;
                         }}"
                    ),
                    "Factory::create()",
                )
            };
            for discard in [false, true] {
                let statement = if discard {
                    format!("_ = {call};")
                } else {
                    format!("let row: [u8; {length}] = {call};")
                };
                let checked = checked_source(&format!(
                    "{declarations}
                     machine selected() reaches Factory {{ {statement} }}"
                ));
                reject(
                    &checked,
                    &format!(
                        "boundary array payload length {length}, direct {direct}, discard {discard}"
                    ),
                );
            }
        }
    }
}

#[test]
fn array_call_result_source_target_and_return_custody_reject_substitution() {
    for tail in [false, true] {
        let original = fixture(tail);
        checked_trees_to_lowered_psi::lower_machine(&original, "selected")
            .expect("array calls compose with earlier and later constructors before corruption");
        for mutation in [
            "same typed returned call",
            "source statement",
            "source ordinal",
            "source site",
            "target machine",
            "target state",
            "body commitment",
            "result multiplicity",
            "affine disposal",
            "dropped unused call",
        ] {
            let mut changed = original.clone();
            let plan = selected(&mut changed);
            let calls = plan
                .operations
                .iter()
                .enumerate()
                .filter_map(|(position, operation)| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::StructuralCall { .. }
                    )
                    .then_some(position)
                })
                .collect::<Vec<_>>();
            assert_eq!(calls.len(), if tail { 3 } else { 2 });
            let CheckedUnitEffectOperationPlan::StructuralCall {
                result: other_result,
                source_site: other_site,
                target_machine: other_machine,
                target_state: other_state,
                ..
            } = plan.operations[calls[1]].clone()
            else {
                panic!("second source call");
            };
            if mutation == "same typed returned call" {
                plan.structural_result = Some(other_result);
            } else if mutation == "dropped unused call" {
                plan.operations.remove(calls[1]);
            } else {
                let CheckedUnitEffectOperationPlan::StructuralCall {
                    coordinate,
                    result,
                    source_site,
                    target_machine,
                    target_state,
                    target_contract_commitment,
                    discard_result_on_return,
                    ..
                } = &mut plan.operations[calls[0]]
                else {
                    panic!("first source call");
                };
                match mutation {
                    "source statement" => coordinate.statement_index += 1,
                    "source ordinal" => coordinate.call_ordinal += 1,
                    "source site" => {
                        assert_ne!(*source_site, other_site, "distinct authored call sites");
                        *source_site = other_site;
                    }
                    "target machine" => *target_machine = other_machine,
                    "target state" => *target_state = other_state,
                    "body commitment" => {
                        *target_contract_commitment =
                            checked_trees::MachineContractCommitment::from_digest([0; 32]);
                    }
                    "result multiplicity" => {
                        result.multiplicity = language_semantics::Multiplicity::Affine
                    }
                    "affine disposal" => *discard_result_on_return = true,
                    _ => unreachable!(),
                }
            }
            reject(&changed, mutation);
        }
    }
}

#[test]
fn array_call_callee_body_requires_one_exact_result_owner() {
    let original = fixture(false);
    checked_trees_to_lowered_psi::lower_machine(&original, "selected")
        .expect("complete ordinary callee bodies lower before corruption");
    for mutation in [
        "missing body",
        "duplicate body",
        "erased result",
        "result identity",
    ] {
        let mut changed = original.clone();
        let symbol = changed
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "first_row")
            .expect("first callee")
            .symbol;
        let plans = &mut changed.facts.flow.terminal_unit_effects.machines;
        let position = plans
            .iter()
            .position(|plan| plan.machine == symbol)
            .unwrap();
        match mutation {
            "missing body" => {
                plans.remove(position);
            }
            "duplicate body" => plans.push(plans[position].clone()),
            "erased result" => plans[position].structural_result = None,
            "result identity" => {
                plans[position]
                    .structural_result
                    .as_mut()
                    .unwrap()
                    .type_identity = "[u16; 2]".into();
            }
            _ => unreachable!(),
        }
        reject(&changed, mutation);
    }
}

#[test]
fn array_call_results_do_not_enable_unimplemented_array_arguments() {
    let checked = checked_source(
        "machine make() -> [u8; 2] { [7, 9] }
         machine consume(row: [u8; 2]) {}
         machine selected() { let row: [u8; 2] = make(); consume(row); }",
    );
    reject(
        &checked,
        "array payload argument transport remains unsupported",
    );
}
