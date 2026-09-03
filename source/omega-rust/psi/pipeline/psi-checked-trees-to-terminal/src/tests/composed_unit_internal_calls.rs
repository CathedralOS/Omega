//! Internal Unit-call leaves and exact target replay.

use super::*;

fn checked_composed_internal_calls() -> psi_checked_trees::CheckedTrees {
    checked_source(
        r#"
            data Root {}
            machine Root::quiet() {}
            machine Root::enter(flag: bool) {
                transition flag {
                    true -> yes()
                    _ -> no()
                }
                state yes() { Root::quiet(); }
                state no() { Root::quiet(); }
            }
        "#,
    )
}

fn source_machine(checked: &CheckedTrees, name: &str) -> psi_symbols::SymbolHandle {
    checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|selection| selection.name == name)
        .map(|selection| selection.machine)
        .expect("named checked terminal machine")
}

#[test]
fn lowers_both_internal_unit_leaves_to_one_canonical_target() {
    let checked = checked_composed_internal_calls();
    let lowered = lower_machine(&checked, "Root::enter")
        .expect("both internal leaves should lower with their exact target closure");
    let [root, target] = lowered.semantic_module.machines.as_slice() else {
        panic!("composed root and its deduplicated target should be the whole closure")
    };
    assert_eq!(lowered.semantic_module.entry, root.id);
    assert_eq!(root.blocks.len(), 3);
    for leaf in &root.blocks[1..] {
        assert!(matches!(
            leaf.operations.as_slice(),
            [Operation {
                result: OperationResult::Unit,
                kind: OperationKind::CallUnit {
                    arguments,
                    callee,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                },
                ..
            }] if arguments.is_empty() && *callee == target.id
                && structural_arguments.is_empty()
                && claim_transfers.is_empty()
                && requirement_obligations.is_empty()
                && crash_continuations.is_empty()
        ));
    }
    assert!(target.structural_parameters.is_empty());
    assert!(target.blocks[0].operations.is_empty());
    assert!(matches!(
        target.blocks[0].terminator,
        Terminator::ReturnUnit {
            ref trivial_affine_discards,
            ..
        } if trivial_affine_discards.is_empty()
    ));
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("internal-call composed module verifies");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes).expect("decode"),
        lowered.semantic_module
    );
}

#[test]
fn internal_unit_leaf_rejects_target_plan_and_identity_corruption() {
    let baseline = checked_composed_internal_calls();
    let rejects = |checked: &CheckedTrees| {
        assert!(matches!(
            lower_machine(checked, "Root::enter"),
            Err(LoweringError::Unsupported(_))
        ));
    };

    let mut state = baseline.clone();
    let wrong_state = state.facts.flow.terminal_unit_effects.composed_machines[0].states[0].state;
    let CheckedUnitEffectOperationPlan::CallUnit { target_state, .. } =
        &mut state.facts.flow.terminal_unit_effects.composed_machines[0].states[1].operations[0]
    else {
        unreachable!()
    };
    *target_state = wrong_state;
    rejects(&state);

    let mut contract = baseline.clone();
    let CheckedUnitEffectOperationPlan::CallUnit {
        target_contract_report_fingerprint,
        ..
    } = &mut contract.facts.flow.terminal_unit_effects.composed_machines[0].states[1].operations[0]
    else {
        unreachable!()
    };
    *target_contract_report_fingerprint ^= 1;
    rejects(&contract);

    let mut missing = baseline;
    let quiet = source_machine(&missing, "Root::quiet");
    missing
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .retain(|plan| plan.machine != quiet);
    rejects(&missing);
}
