//! Internal Unit-call leaves and exact target replay.

use super::{CheckedTrees, LoweringError, checked_source, lower_machine};
use checked_trees::CheckedUnitEffectOperationPlan;
use terminal_psi::{Operation, OperationKind, OperationResult, Terminator};
#[test]
fn composed_scalar_call_locals_replay_their_authored_computation() {
    for (prefix, argument) in [
        ("", "value"),
        ("let prior: u64 = value;", "prior"),
        ("let prior: u64 = identity(value);", "prior"),
    ] {
        let checked = checked_source(&format!(
            "machine identity(value: u64) -> u64 {{ value }}
             data Root {{}}
             machine Root::enter(value: u64) {{
                 {prefix}
                 let retained: u64 = identity({argument});
                 transition retained == 7 {{ true -> yes() false -> no() }}
                 state yes() {{}}
                 state no() {{}}
             }}"
        ));
        lower_machine(&checked, "Root::enter")
            .expect("call locals compose with earlier pure bindings and call results");
        let machine = source_machine(&checked, "Root::enter");
        let root = checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .map(|(_, root)| root)
            .find(|root| root.machine == machine)
            .expect("call initializer computation")
            .root;
        let mut changed = checked.clone();
        changed
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(root)
            .authored_root = typed_trees::expression::ExpressionHandle::invalid();
        assert!(
            lower_machine(&changed, "Root::enter").is_err(),
            "retained computation cannot replace its authored call custody"
        );
    }
}

fn checked_composed_internal_calls() -> checked_trees::CheckedTrees {
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

fn source_machine(checked: &CheckedTrees, name: &str) -> symbols::SymbolHandle {
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
                    erased_arguments: _,
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
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("internal-call composed module verifies");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        terminal_codec::decode_module(&bytes).expect("decode"),
        lowered.semantic_module
    );
}

#[test]
fn internal_unit_call_carries_the_erased_lane_and_requires_obligations() {
    let checked = checked_source(
        r#"
            data Root {}
            machine Root::quiet(n: i32, bound [erased]: i32)
            requires
                n < bound
            {}
            machine Root::enter(flag: bool) {
                transition flag {
                    true -> yes()
                    _ -> no()
                }
                state yes() { Root::quiet(3, 5); }
                state no() { Root::quiet(8, 9); }
            }
        "#,
    );
    let lowered = lower_machine(&checked, "Root::enter")
        .expect("internal Unit callee with requires and an erased formal lowers");
    let [root, target] = lowered.semantic_module.machines.as_slice() else {
        panic!("composed root and its requires-bearing target form the whole closure")
    };
    assert!(!target.contract.requires.is_empty());
    assert_eq!(target.contract.erased_scalar_formals.len(), 1);
    for leaf in &root.blocks[1..] {
        let operation = leaf
            .operations
            .last()
            .expect("each state ends in its internal call");
        let OperationKind::CallUnit {
            callee,
            erased_arguments,
            requirement_obligations,
            ..
        } = &operation.kind
        else {
            panic!("the internal call emits a CallUnit")
        };
        assert_eq!(*callee, target.id);
        assert_eq!(erased_arguments.len(), 1, "one erased actual per formal");
        assert_eq!(
            requirement_obligations.len(),
            target.contract.requires.len(),
            "one obligation per requires row"
        );
    }
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("internal calls with discharged erased requires verify source-free");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        terminal_codec::decode_module(&bytes).expect("decode"),
        lowered.semantic_module
    );
}

#[test]
fn internal_unit_call_rejects_a_violating_or_missing_erased_actual() {
    let checked = checked_source(
        r#"
            data Root {}
            machine Root::quiet(n: i32, bound [erased]: i32)
            requires
                n < bound
            {}
            machine Root::enter(flag: bool) {
                transition flag {
                    true -> yes()
                    _ -> no()
                }
                state yes() { Root::quiet(3, 5); }
                state no() { Root::quiet(8, 9); }
            }
        "#,
    );
    let lowered = lower_machine(&checked, "Root::enter").expect("baseline lowers");
    let verify = |mutate: &mut dyn FnMut(&mut terminal_psi::TerminalMachine)| {
        let mut module = lowered.semantic_module.clone();
        mutate(&mut module.machines[0]);
        terminal_verifier::verify_module(
            &module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .map(|_| ())
    };
    assert!(
        verify(&mut |root| {
            let OperationKind::CallUnit {
                erased_arguments, ..
            } = &mut root.blocks[1].operations[1].kind
            else {
                unreachable!()
            };
            // Substitute the erased actual for a literal that falsifies `n < bound`.
            *erased_arguments = vec![semantic_vocabulary::ScalarTerm::Integer {
                scalar_type: semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Signed,
                    32,
                )
                .expect("i32 is valid"),
                value: semantic_vocabulary::IntegerValue::Signed(1),
            }];
        })
        .is_err(),
        "an erased actual violating the callee requires rejects"
    );
    assert!(
        verify(&mut |root| {
            let OperationKind::CallUnit {
                erased_arguments, ..
            } = &mut root.blocks[1].operations[1].kind
            else {
                unreachable!()
            };
            erased_arguments.clear();
        })
        .is_err(),
        "a missing erased argument row rejects"
    );
}

#[test]
fn internal_unit_call_cites_the_second_erased_formal_in_its_own_ordinal() {
    let checked = checked_source(
        r#"
            data Root {}
            machine Root::quiet(n: i32, low [erased]: i32, high [erased]: i32)
            requires
                n < high
            {}
            machine Root::enter(flag: bool) {
                transition flag {
                    true -> yes()
                    _ -> no()
                }
                state yes() { Root::quiet(7, 5, 9); }
                state no() { Root::quiet(8, 6, 11); }
            }
        "#,
    );
    let lowered = lower_machine(&checked, "Root::enter")
        .expect("internal Unit callee with two erased formals lowers");
    let [root, target] = lowered.semantic_module.machines.as_slice() else {
        panic!("composed root and its requires-bearing target form the whole closure")
    };
    let [low, high] = target.contract.erased_scalar_formals.as_slice() else {
        panic!("two erased formals publish in authored order")
    };
    // The authored clause cites the second erased formal; a dense-ordinal
    // mistake would name `low` instead.
    assert!(
        target.contract.requires.iter().any(|row| matches!(
            row,
            semantic_vocabulary::Proposition::LessThan(
                _,
                semantic_vocabulary::ScalarTerm::Value { id, .. }
            ) if *id == high.id
        )),
        "requires must cite the second erased formal: {:?}",
        target.contract.requires
    );
    assert!(
        !target.contract.requires.iter().any(|row| matches!(
            row,
            semantic_vocabulary::Proposition::LessThan(
                _,
                semantic_vocabulary::ScalarTerm::Value { id, .. }
            ) if *id == low.id
        )),
        "requires must not drift onto the first erased formal"
    );
    for leaf in &root.blocks[1..] {
        let operation = leaf
            .operations
            .last()
            .expect("each state ends in its internal call");
        let OperationKind::CallUnit {
            erased_arguments, ..
        } = &operation.kind
        else {
            panic!("the internal call emits a CallUnit")
        };
        assert_eq!(erased_arguments.len(), 2, "one erased actual per formal");
    }
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("actuals discharge the second-erased requires");
    // Rewriting the clause onto `low` must change the proof: 7 < 5 and 8 < 6
    // do not hold, so the drifted row rejects.
    let mut module = lowered.semantic_module.clone();
    let target = &mut module.machines[1];
    for row in &mut target.contract.requires {
        if let semantic_vocabulary::Proposition::LessThan(
            _,
            term @ semantic_vocabulary::ScalarTerm::Value { .. },
        ) = row
        {
            *term = semantic_vocabulary::ScalarTerm::Value {
                id: low.id,
                scalar_type: low.scalar_type,
            };
        }
    }
    assert!(
        terminal_verifier::verify_module(
            &module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err(),
        "a clause drifted onto the first erased formal rejects"
    );
}

#[test]
fn internal_unit_leaf_rejects_target_plan_and_identity_corruption() {
    let baseline = checked_composed_internal_calls();
    let rejects = |checked: &CheckedTrees| {
        let result = lower_machine(checked, "Root::enter");
        assert!(
            matches!(result, Err(LoweringError::Unsupported(_))),
            "unexpected result: {result:?}"
        );
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
    assert!(matches!(
        lower_machine(&missing, "Root::enter"),
        Err(LoweringError::InvalidUnitMachinePlan { machine, reason, .. })
            if machine == "Root::quiet"
                && reason == "attached Unit closure is missing a checked transitive machine plan"
    ));
}

#[test]
fn free_composed_attachment_matches_the_authored_declaration() {
    let baseline = checked_source(
        r#"
            data Owner {}
            machine quiet() {}
            machine finish(flag: bool) {
                transition flag { true -> yes() _ -> no() }
                state yes() { quiet(); }
                state no() { quiet(); }
            }
            machine Owner::finish(flag: bool) {
                transition flag { true -> yes() _ -> no() }
                state yes() { quiet(); }
                state no() { quiet(); }
            }
        "#,
    );
    for name in ["finish", "Owner::finish"] {
        let lowered = lower_machine(&baseline, name).expect("exact free or attached composed root");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("root verifies independently");
        let root = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .unwrap();
        assert_eq!(root.attachment.is_some(), name == "Owner::finish");
    }
    let free = source_machine(&baseline, "finish");
    let attached = source_machine(&baseline, "Owner::finish");
    let attachment = baseline
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(attached)
        .unwrap()
        .attachment_type_identity
        .clone()
        .unwrap();
    for (machine, name, replacement) in [
        (free, "finish", Some(attachment)),
        (attached, "Owner::finish", None),
        (
            attached,
            "Owner::finish",
            Some("foreign attachment".to_owned()),
        ),
    ] {
        let mut changed = baseline.clone();
        changed
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .find(|plan| plan.machine == machine)
            .unwrap()
            .attachment_type_identity = replacement;
        assert!(
            lower_machine(&changed, name).is_err(),
            "attachment substitution rejects for {name}"
        );
    }
}

#[test]
fn free_composed_helper_rejects_fabricated_provider_fields() {
    let mut checked = checked_source(
        r#"
        machine quiet() {}
        machine finish(flag: bool) {
            transition flag { true -> yes() _ -> no() }
            state yes() { quiet(); }
            state no() { quiet(); }
        }
    "#,
    );
    let free = source_machine(&checked, "finish");
    checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter_mut()
        .find(|plan| plan.machine == free)
        .unwrap()
        .provider_attachment_requirements
        .push(checked_trees::CheckedProviderAttachmentRequirementPlan {
            field_identity: "fabricated".to_owned(),
            provider_type_identity: "fabricated".to_owned(),
            boundary: free,
        });
    assert!(lower_machine(&checked, "finish").is_err());
}
