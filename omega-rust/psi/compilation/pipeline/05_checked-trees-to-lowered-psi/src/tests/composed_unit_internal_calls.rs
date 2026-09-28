//! Internal Unit-call leaves and exact target replay.

use super::{CheckedTrees, lower_machine};
use crate::TerminalMachineSelection;
use terminal_psi::OperationKind;
#[test]
fn composed_scalar_call_locals_replay_their_authored_computation() {
    for (prefix, argument) in [
        ("", "value"),
        ("let prior: u64 = value;", "prior"),
        ("let prior: u64 = identity(value);", "prior"),
    ] {
        let checked = crate::front_end::checked_program(&format!(
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
        lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
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
            lower_machine(&changed, TerminalMachineSelection::Name("Root::enter")).is_err(),
            "retained computation cannot replace its authored call custody"
        );
    }
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
fn internal_unit_call_rejects_a_violating_or_missing_erased_actual() {
    let checked = crate::front_end::checked_program(
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
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
        .expect("baseline lowers");
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
    let checked = crate::front_end::checked_program(
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
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
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
fn free_composed_helper_rejects_fabricated_provider_fields() {
    let mut checked = crate::front_end::checked_program(
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
    assert!(lower_machine(&checked, TerminalMachineSelection::Name("finish")).is_err());
}
