//! Owned match arms evaluating fresh structural call products. A call arm
//! runs only on its own edge and joins its once-evaluated result through the
//! same block parameter an existing local occupies; the displaced source's
//! residuals still die on the fresh arm's edge.

use super::CALL_VALUE_SOURCE;
use checked_trees_to_lowered_psi::TerminalMachineSelection;

#[test]
fn owned_match_call_product_rejects_mutated_edge_evidence() {
    let checked =
        crate::front_end::checked_program_result(CALL_VALUE_SOURCE).expect("call selection checks");
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("choose"),
    )
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
    let errors = crate::front_end::checked_program_result(
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
        let checked = crate::front_end::checked_program_result(&format!(
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
        let error = checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("choose"),
        )
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
    let errors = crate::front_end::checked_program_result(
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
