//! Source and borrow ledgers cannot jointly substitute another live local.

use checked_trees::{
    CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

fn caller(checked: &checked_trees::CheckedTrees) -> usize {
    checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .position(|plan| {
            plan.operations.iter().any(|operation| {
                matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. })
            })
        })
        .expect("ordinary Unit caller")
}

#[test]
fn local_unit_call_rejects_changed_access_and_missing_ordered_effects() {
    let source = super::source("u64");
    let original = crate::front_end::checked_program(&source);
    let _ = super::artifact(&source);
    let caller = caller(&original);
    for mutation in 0..7 {
        let mut changed = original.clone();
        let operations = &mut changed.facts.flow.terminal_unit_effects.machines[caller].operations;
        match mutation {
            0..=2 => {
                operations.remove(mutation);
            }
            3 => operations.swap(0, 1),
            4 => operations.insert(1, operations[1].clone()),
            5 | 6 => {
                let CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } = &mut operations[1]
                else {
                    panic!("Unit call");
                };
                structural_arguments[0].access = if mutation == 5 {
                    CheckedStructuralAccess::SharedBorrow
                } else {
                    CheckedStructuralAccess::Owned
                };
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(
                &changed,
                TerminalMachineSelection::Name("observe")
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default()
            ))
            .is_err(),
            "Unit call/local roster mutation {mutation}"
        );
    }
}

#[test]
fn local_unit_actual_cannot_change_its_retained_owner_kind_to_a_parameter() {
    let source = super::source("u64");
    let mut changed = crate::front_end::checked_program(&source);
    let _ = super::artifact(&source);
    let caller = caller(&changed);
    let plan = &mut changed.facts.flow.terminal_unit_effects.machines[caller];
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &mut plan.operations[1]
    else {
        panic!("Unit call");
    };
    structural_arguments[0].source =
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 };
    assert!(
        terminal_production::TerminalProductionRequest::new(
            &changed,
            TerminalMachineSelection::Name("observe")
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default()
        ))
        .is_err()
    );
}
