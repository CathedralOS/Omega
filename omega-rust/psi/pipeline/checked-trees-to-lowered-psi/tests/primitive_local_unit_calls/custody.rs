//! Source and borrow ledgers cannot jointly substitute another live local.

use checked_trees::{
    BorrowAccessKind, CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};

fn source() -> String {
    super::source("u64").replace(
        "replace(&mut scratch, replacement);",
        "let mut spare: u64 = initial; replace(&mut scratch, replacement);",
    )
}

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
fn local_unit_borrow_rejects_missing_duplicate_and_changed_call_evidence() {
    let source = source();
    let original = super::checked(&source);
    let _ = super::artifact(&source);
    let plan = &original.facts.flow.terminal_unit_effects.machines[caller(&original)];
    let (state_handle, state) = original
        .facts
        .borrow
        .states
        .iter()
        .find(|(_, state)| state.machine_symbol == plan.machine && state.state_symbol == plan.state)
        .unwrap();
    let call = original
        .facts
        .borrow
        .calls
        .span_or_empty(state.calls)
        .iter()
        .find(|call| call.statement_index == 2 && call.call_ordinal == 0)
        .unwrap();
    let call_handle = original
        .facts
        .borrow
        .calls
        .iter()
        .find_map(|(handle, candidate)| std::ptr::eq(candidate, call).then_some(handle))
        .unwrap();
    let access_handle = call.accesses.start();
    let access = original
        .facts
        .borrow
        .argument_accesses
        .get(access_handle)
        .clone();
    for mutation in 0..8 {
        let mut changed = original.clone();
        let borrow = &mut changed.facts.borrow;
        match mutation {
            0 => borrow.calls.get_mut(call_handle).accesses = arena::HandleSpan::empty(),
            1 => borrow.argument_accesses.get_mut(access_handle).kind = BorrowAccessKind::Read,
            2 => borrow.argument_accesses.get_mut(access_handle).kind = BorrowAccessKind::WriteOnly,
            3 => borrow.calls.get_mut(call_handle).target_symbol = plan.machine,
            4 => borrow.calls.get_mut(call_handle).statement_index = 1,
            5 => borrow.calls.get_mut(call_handle).call_ordinal = 1,
            6 => {
                let duplicates = borrow
                    .argument_accesses
                    .insert_many([access.clone(), access.clone()]);
                borrow.calls.get_mut(call_handle).accesses = duplicates;
            }
            7 => {
                let duplicates = borrow.calls.insert_many([call.clone(), call.clone()]);
                borrow.states.get_mut(state_handle).calls = duplicates;
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "observe")
                .produce_artifact()
                .is_err(),
            "Unit borrow custody mutation {mutation}"
        );
    }
}

#[test]
fn coherent_local_and_borrow_substitution_still_rejects_the_wrong_authored_actual() {
    let source = source();
    let mut changed = super::checked(&source);
    let _ = super::artifact(&source);
    let caller = caller(&changed);
    let plan = &mut changed.facts.flow.terminal_unit_effects.machines[caller];
    let CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { symbol: spare, .. } =
        plan.operations[1]
    else {
        panic!("spare local");
    };
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &mut plan.operations[2]
    else {
        panic!("Unit call");
    };
    let CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol: scratch } =
        structural_arguments[0].source
    else {
        panic!("local actual");
    };
    structural_arguments[0].source =
        CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol: spare };
    let handles = changed
        .facts
        .borrow
        .argument_accesses
        .iter()
        .filter_map(|(handle, access)| (access.root_symbol == scratch).then_some(handle))
        .collect::<Vec<_>>();
    for handle in handles {
        changed
            .facts
            .borrow
            .argument_accesses
            .get_mut(handle)
            .root_symbol = spare;
    }
    assert!(
        terminal_production::TerminalProductionRequest::new(&changed, "observe")
            .produce_artifact()
            .is_err()
    );
}

#[test]
fn local_unit_call_rejects_changed_access_and_missing_ordered_effects() {
    let source = super::source("u64");
    let original = super::checked(&source);
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
            terminal_production::TerminalProductionRequest::new(&changed, "observe")
                .produce_artifact()
                .is_err(),
            "Unit call/local roster mutation {mutation}"
        );
    }
}

#[test]
fn write_only_local_unit_actual_retains_its_attenuated_access() {
    let source = super::source("u64")
        .replace("destination: &mut", "destination: &write")
        .replace("replace(&mut scratch", "replace(&write scratch");
    let artifact = super::artifact(&source);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let argument = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::CallUnit {
                structural_arguments,
                ..
            } => structural_arguments.first(),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        argument.access,
        terminal_psi::StructuralAccess::WriteOnlyBorrow
    );
    let initial = super::integer(
        semantic_vocabulary::IntegerSign::Unsigned,
        64,
        semantic_vocabulary::IntegerValue::Unsigned(9),
    );
    let replacement = super::integer(
        semantic_vocabulary::IntegerSign::Unsigned,
        64,
        semantic_vocabulary::IntegerValue::Unsigned(37),
    );
    super::execute(&artifact, initial, replacement, &[initial, replacement]);
}

#[test]
fn local_unit_actual_cannot_change_its_retained_owner_kind_to_a_parameter() {
    let source = super::source("u64");
    let mut changed = super::checked(&source);
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
        terminal_production::TerminalProductionRequest::new(&changed, "observe")
            .produce_artifact()
            .is_err()
    );
}
