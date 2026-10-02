//! Call results retain source identity independently of their shared array type.

use super::reject;
use checked_trees::{CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan};
use checked_trees_to_lowered_psi::TerminalMachineSelection;

fn fixture(tail: bool) -> CheckedTrees {
    let completion = if tail { "second_row()" } else { "first" };
    crate::front_end::checked_program(&format!(
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

/// The ordinary (non-boundary) call with a structural result, if `operation` is one.
fn ordinary_structural_call(
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<&checked_trees::CheckedUnitCallPlan> {
    operation
        .call()
        .filter(|call| !call.is_boundary() && call.structural_result().is_some())
}

#[test]
fn boundary_array_results_retain_exact_call_custody_when_unused_or_empty() {
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
                let checked = crate::front_end::checked_program(&format!(
                    "{declarations}
                     machine selected() reaches Factory {{ {statement} }}"
                ));
                checked_trees_to_lowered_psi::lower_machine(
                    &checked,
                    TerminalMachineSelection::Name("selected"),
                )
                .unwrap_or_else(|error| panic!(
                    "boundary array payload length {length}, direct {direct}, discard {discard}: {error:?}"
                ));
                let mut changed = checked.clone();
                let call = selected(&mut changed)
                    .operations
                    .iter_mut()
                    .find_map(CheckedUnitEffectOperationPlan::call_mut)
                    .expect("authored boundary call");
                let (result, _) = call.structural_result_mut().expect("array result custody");
                result.multiplicity = language_semantics::Multiplicity::Affine;
                reject(&changed, "boundary array result multiplicity");
            }
        }
    }
}

#[test]
fn array_call_result_source_target_and_return_custody_reject_substitution() {
    for tail in [false, true] {
        let original = fixture(tail);
        checked_trees_to_lowered_psi::lower_machine(
            &original,
            TerminalMachineSelection::Name("selected"),
        )
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
                    ordinary_structural_call(operation).map(|_| position)
                })
                .collect::<Vec<_>>();
            assert_eq!(calls.len(), if tail { 3 } else { 2 });
            let Some(other) = ordinary_structural_call(&plan.operations[calls[1]]).cloned() else {
                panic!("second source call");
            };
            let other_result = other
                .structural_result()
                .expect("structural call result")
                .clone();
            let other_site = other
                .source_site()
                .expect("an ordinary structural call retains its source site");
            let other_machine = other.target.machine;
            let other_state = other.target.state;
            if mutation == "same typed returned call" {
                plan.structural_result = Some(other_result.into());
            } else if mutation == "dropped unused call" {
                plan.operations.remove(calls[1]);
            } else {
                let Some(call) = plan.operations[calls[0]].call_mut() else {
                    panic!("first source call");
                };
                match mutation {
                    "source statement" => call.coordinate.statement_index += 1,
                    "source ordinal" => call.coordinate.call_ordinal += 1,
                    "source site" => {
                        let source_site = call
                            .source_site_mut()
                            .expect("an ordinary structural call retains its source site");
                        assert_ne!(*source_site, other_site, "distinct authored call sites");
                        *source_site = other_site;
                    }
                    "target machine" => call.target.machine = other_machine,
                    "target state" => call.target.state = other_state,
                    "body commitment" => {
                        *call
                            .contract_commitment_mut()
                            .expect("an ordinary structural call retains its commitment") =
                            checked_trees::MachineContractCommitment::from_digest([0; 32]);
                    }
                    "result multiplicity" => {
                        let (result, _) = call
                            .structural_result_mut()
                            .expect("structural call result");
                        result.multiplicity = language_semantics::Multiplicity::Affine
                    }
                    "affine disposal" => {
                        let (_, discard_result_on_return) = call
                            .structural_result_mut()
                            .expect("structural call result");
                        *discard_result_on_return = true
                    }
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
    checked_trees_to_lowered_psi::lower_machine(
        &original,
        TerminalMachineSelection::Name("selected"),
    )
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
