//! Owned match arms evaluating fresh structural call products. A call arm
//! runs only on its own edge and joins its once-evaluated result through the
//! same block parameter an existing local occupies; the displaced source's
//! residuals still die on the fresh arm's edge.

use super::CALL_VALUE_SOURCE;
use crate::value_dispatch::{
    TerminalExecutionResult, TerminalScalarValue, check_source, execute_machine, unsigned,
};

#[test]
fn owned_match_call_product_executes_both_arms() {
    let (first, second) = (0x8123_4567_89ab_cdef_u128, 0x0fed_cba9_8765_4321_u128);
    for selected in [false, true] {
        let (_, execution) = execute_machine(
            CALL_VALUE_SOURCE,
            "choose",
            &[
                TerminalScalarValue::Boolean(selected),
                unsigned(first),
                unsigned(second),
            ],
        );
        // The false arm moves `pair` whole; the true arm joins the fresh
        // `supply(first ^ 3, second ^ 5)` product evaluated on that arm only.
        let expected = if selected {
            (first ^ 3) ^ (second ^ 5)
        } else {
            first ^ second
        };
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(unsigned(expected)),
            "selected={selected}"
        );
    }
}

#[test]
fn owned_match_all_call_arms_execute_without_existing_sources() {
    let (first, second) = (0x0123_4567_89ab_cdef_u128, 0x8edc_ba98_7654_3210_u128);
    for selected in [false, true] {
        let (_, execution) = execute_machine(
            CALL_VALUE_SOURCE,
            "pick",
            &[
                TerminalScalarValue::Boolean(selected),
                unsigned(first),
                unsigned(second),
            ],
        );
        let expected = if selected {
            first ^ second
        } else {
            (second ^ 7) ^ (first ^ 11)
        };
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(unsigned(expected)),
            "selected={selected}"
        );
    }
}

#[test]
fn owned_match_record_arm_evaluates_fresh_call_fields_on_its_own_edge() {
    let (first, second) = (0x5a5a_5a5a_5a5a_5a5a_u128, 0xa5a5_a5a5_a5a5_a5a5_u128);
    for selected in [false, true] {
        let (_, execution) = execute_machine(
            CALL_VALUE_SOURCE,
            "gather",
            &[
                TerminalScalarValue::Boolean(selected),
                unsigned(first),
                unsigned(second),
            ],
        );
        // The true arm's record evaluates both `provide` calls on that arm;
        // the false arm moves `pair` whole.
        let expected = if selected {
            (first ^ 1) ^ (second ^ 4)
        } else {
            first ^ second
        };
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(unsigned(expected)),
            "selected={selected}"
        );
    }
}

#[test]
fn owned_match_call_arm_evaluates_nested_scalar_call_arguments() {
    // A fresh call arm's scalar arguments compose through ordinary call
    // operands: `combine` runs once, on the arm that selects `supply`.
    let source = "data Pair { first: u64; second: u64; }
        machine combine(left: u64, right: u64) -> u64 {
            left ^ right
        }
        machine supply(first: u64, second: u64) -> Pair {
            Pair { first: first, second: second }
        }
        machine choose(selected: bool, first: u64) -> u64 {
            let pair: Pair = Pair { first: first, second: first };
            let result: Pair = match selected {
                true -> supply(combine(first, 3), first),
                false -> pair
            };
            result.first ^ result.second
        }";
    let first = 0x0123_4567_89ab_cdef_u128;
    for selected in [false, true] {
        let (_, execution) = execute_machine(
            source,
            "choose",
            &[TerminalScalarValue::Boolean(selected), unsigned(first)],
        );
        let expected = if selected { (first ^ 3) ^ first } else { 0 };
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(unsigned(expected)),
            "selected={selected}"
        );
    }
}

#[test]
fn owned_match_call_product_edge_transports_the_exact_selected_owner() {
    let checked = check_source(CALL_VALUE_SOURCE).expect("call selection checks");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "choose")
        .expect("call selection lowers");
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let join = machine
        .blocks
        .iter()
        .find(|block| !block.structural_parameters.is_empty())
        .expect("one result continuation")
        .id;
    let edges = machine
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            terminal_psi::Terminator::Jump {
                target,
                structural_arguments,
                trivial_affine_discards,
                residual_affine_discards,
                ..
            } if *target == join => Some((
                structural_arguments.as_slice(),
                trivial_affine_discards.as_slice(),
                residual_affine_discards.as_slice(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(edges.len(), 2, "each arm edge joins the result");
    // Both edges bind the result parameter from a whole owned place.
    assert!(
        edges.iter().all(|(arguments, _, _)| {
            arguments.len() == 1
                && arguments[0].path.is_empty()
                && arguments[0].access == terminal_psi::StructuralAccess::Owned
        }),
        "each arm moves one whole owner into the result slot: {edges:?}"
    );
    // The fresh call arm's edge discards the displaced `pair`; the local
    // arm's edge transports it and discards nothing.
    let fresh = edges
        .iter()
        .find(|(_, trivial, _)| !trivial.is_empty())
        .expect("the fresh call arm edge kills the displaced local");
    let existing = edges
        .iter()
        .find(|(_, trivial, _)| trivial.is_empty())
        .expect("the local arm edge transports the live owner");
    assert_eq!(fresh.1.len(), 1, "only the displaced candidate dies");
    assert!(
        fresh.2.is_empty() && existing.2.is_empty(),
        "whole sources leave no projected residuals: {edges:?}"
    );
    assert_eq!(
        fresh.1[0], existing.0[0].place,
        "the discarded place on the fresh edge is the owner the local edge moves"
    );
    assert_ne!(
        fresh.0[0].place, existing.0[0].place,
        "the call product is an independent owner, not the local's place"
    );
}

#[test]
fn owned_match_call_product_rejects_mutated_edge_evidence() {
    let checked = check_source(CALL_VALUE_SOURCE).expect("call selection checks");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "choose")
        .expect("call selection lowers");
    let join = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .find(|block| !block.structural_parameters.is_empty())
        .expect("one result continuation")
        .id;
    for mutation in 0..3 {
        let mut changed = lowered.semantic_module.clone();
        for block in changed
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
        {
            let terminal_psi::Terminator::Jump {
                target,
                structural_arguments,
                trivial_affine_discards,
                ..
            } = &mut block.terminator
            else {
                continue;
            };
            if *target != join {
                continue;
            }
            match mutation {
                // Dropping the displaced discard leaks `pair` on the fresh
                // call arm's edge.
                0 => trivial_affine_discards.clear(),
                // The transported owner must stay a whole-place move.
                1 => {
                    for argument in structural_arguments.iter_mut() {
                        argument.path = vec![terminal_psi::StructuralPathSegment::from("forged")];
                    }
                }
                // Erasing the transported owner leaves the result parameter
                // unbound.
                _ => structural_arguments.clear(),
            }
        }
        assert!(
            terminal_verifier::verify_module(
                &changed,
                &lowered.proof_bundle,
                &crate::value_dispatch::AdmissionProfile::default()
            )
            .is_err(),
            "call-product edge mutation {mutation}"
        );
    }
}

#[test]
fn owned_match_call_arm_rejects_forwarded_existing_custody() {
    // A call arm may only contribute a fresh product: moving an existing
    // affine child through a call parameter is a transfer the selection
    // receipt cannot name.
    let errors = check_source(
        "data Payload { left: u64; right: u64; }
        data Pair { first: Payload; second: Payload; }
        machine wrap(child: Payload) -> Payload { child }
        machine choose(selected: bool, a: Pair, b: Pair) -> u64 {
            let result: Payload = match selected {
                true -> wrap(a.first),
                false -> b.second
            };
            result.left ^ result.right
        }",
    )
    .expect_err("a call moving an existing affine child stays rejected");
    assert!(
        errors
            .iter()
            .any(|error| error.to_string().contains("transfers owned input custody")),
        "expected the branch-local transfer diagnostic: {errors:#?}"
    );
}

#[test]
fn owned_match_record_arm_children_check_but_await_leaf_emission() {
    // Record fields moving existing children now check: the selection receipt
    // carries each field's exact moved path — a local's child through its
    // roster source, a call product's child through its once-evaluated
    // sourceless root — and the residual complements discharge on the same
    // edge. Emission still needs the leaf extraction edge, so lowering
    // remains the remaining leg for both leaf kinds.
    for arm in [
        "Pair { first: supply(1, 2).first, second: b.second }",
        "Pair { first: a.first, second: b.second }",
    ] {
        let checked = check_source(&format!(
            "data Payload {{ left: u64; right: u64; }}
            data Pair {{ first: Payload; second: Payload; }}
            machine supply(first: u64, second: u64) -> Pair {{
                Pair {{ first: Payload {{ left: first, right: second }}, second: Payload {{ left: second, right: first }} }}
            }}
            machine choose(selected: bool, a: Pair, b: Pair) -> u64 {{
                let result: Pair = match selected {{
                    true -> {arm},
                    false -> a
                }};
                result.first.left ^ result.second.right
            }}"
        ))
        .unwrap_or_else(|errors| panic!("{arm} keeps each field's moved child: {errors:#?}"));
        let error = checked_trees_to_lowered_psi::lower_machine(&checked, "choose")
            .expect_err("record field leaves await their extraction edge");
        assert!(
            format!("{error:?}").contains("projected selection"),
            "{arm}: the leaf emission gap is the remaining boundary: {error:?}"
        );
    }
}

#[test]
fn owned_match_subject_call_stays_rejected() {
    // Selection predicates keep their strict operand rule: a subject call
    // needs captured effect/loan evidence the owned-selection route does not
    // carry.
    let errors = check_source(
        "data Pair { first: u64; second: u64; }
        machine flag() -> bool { true }
        machine choose(a: Pair, b: Pair) -> u64 {
            let result: Pair = match flag() {
                true -> a,
                false -> b
            };
            result.first ^ result.second
        }",
    )
    .expect_err("a call in the match subject stays rejected");
    assert!(!errors.is_empty());
}
