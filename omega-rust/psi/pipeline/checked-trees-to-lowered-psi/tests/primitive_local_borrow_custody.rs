//! Primitive referent publication independently checks exact source borrow facts.

use checked_trees::{BorrowAccessKind, CheckedUnitEffectOperationPlan};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;

const SOURCE: &str = r#"
    machine reset(value: &mut u64) -> u64 { value = 0; 7 }
    machine enter(value: &mut u64) {
        let mut scratch: u64 = 91;
        let mut spare: u64 = 91;
        let returned: u64 = reset(&mut scratch);
        value = scratch;
    }
"#;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check")
}

#[test]
fn primitive_local_mutable_and_write_only_borrows_publish_with_exact_custody() {
    for source in [SOURCE.to_owned(), SOURCE.replace("&mut", "&write")] {
        let checked = checked(&source);
        let _ = terminal_production::TerminalProductionRequest::new(&checked, "enter")
            .produce_artifact()
            .expect("exact local borrow custody");
    }
}

#[test]
fn primitive_local_publication_rejects_missing_duplicate_and_drifted_borrow_facts() {
    let original = checked(SOURCE);
    let _ = terminal_production::TerminalProductionRequest::new(&original, "enter")
        .produce_artifact()
        .expect("original custody");
    let plan = original
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| {
            plan.operations.iter().any(|operation| {
                matches!(operation, CheckedUnitEffectOperationPlan::ScalarCall { .. })
            })
        })
        .expect("caller plan");
    let (state_handle, state) = original
        .facts
        .borrow
        .states
        .iter()
        .find(|(_, state)| state.machine_symbol == plan.machine && state.state_symbol == plan.state)
        .expect("borrow state");
    let call = original
        .facts
        .borrow
        .calls
        .span_or_empty(state.calls)
        .iter()
        .find(|call| call.statement_index == 2 && call.call_ordinal == 0)
        .expect("borrow call");
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
    let CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { symbol: spare, .. } =
        plan.operations[1]
    else {
        panic!("spare local");
    };
    for mutation in 0..13 {
        let mut changed = original.clone();
        let borrows = &mut changed.facts.borrow;
        match mutation {
            0 => borrows.calls.get_mut(call_handle).accesses = arena::HandleSpan::empty(),
            1 => borrows.argument_accesses.get_mut(access_handle).kind = BorrowAccessKind::Read,
            2 => {
                borrows.argument_accesses.get_mut(access_handle).kind = BorrowAccessKind::WriteOnly
            }
            3 => borrows.argument_accesses.get_mut(access_handle).root_symbol = spare,
            4 => borrows.calls.get_mut(call_handle).target_symbol = plan.machine,
            5 => borrows.calls.get_mut(call_handle).statement_index = 1,
            6 => borrows.calls.get_mut(call_handle).call_ordinal = 1,
            7 => {
                let duplicates = borrows
                    .argument_accesses
                    .insert_many([access.clone(), access.clone()]);
                borrows.calls.get_mut(call_handle).accesses = duplicates;
            }
            8 => {
                let duplicates = borrows.calls.insert_many([call.clone(), call.clone()]);
                borrows.states.get_mut(state_handle).calls = duplicates;
            }
            9 => {
                borrows.states.append(state.clone());
            }
            10 => borrows.states.get_mut(state_handle).machine_symbol = call.target_symbol,
            11 => {
                let path = borrows
                    .access_segments
                    .insert_many([facts::PlaceSegment::FixedIndex { index: 0 }]);
                borrows.argument_accesses.get_mut(access_handle).segments = path;
            }
            _ => {
                let mut conflicting = call.clone();
                conflicting.target_symbol = plan.machine;
                let duplicates = borrows.calls.insert_many([call.clone(), conflicting]);
                borrows.states.get_mut(state_handle).calls = duplicates;
            }
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "enter")
                .produce_artifact()
                .is_err(),
            "borrow custody mutation {mutation}"
        );
    }
}

#[test]
fn primitive_local_coherent_plan_and_borrow_substitution_cannot_replace_authored_actual() {
    let mut changed = checked(SOURCE);
    let _ = terminal_production::TerminalProductionRequest::new(&changed, "enter")
        .produce_artifact()
        .expect("original custody");
    let plan = changed
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| {
            plan.operations.iter().any(|operation| {
                matches!(operation, CheckedUnitEffectOperationPlan::ScalarCall { .. })
            })
        })
        .unwrap();
    let CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { symbol: spare, .. } =
        plan.operations[1]
    else {
        panic!("spare");
    };
    let CheckedUnitEffectOperationPlan::ScalarCall {
        structural_arguments,
        ..
    } = &mut plan.operations[2]
    else {
        panic!("call");
    };
    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol: scratch } =
        structural_arguments[0].source
    else {
        panic!("primitive argument");
    };
    structural_arguments[0].source =
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol: spare };
    let access_handles = changed
        .facts
        .borrow
        .argument_accesses
        .iter()
        .filter_map(|(handle, access)| (access.root_symbol == scratch).then_some(handle))
        .collect::<Vec<_>>();
    for handle in access_handles {
        changed
            .facts
            .borrow
            .argument_accesses
            .get_mut(handle)
            .root_symbol = spare;
    }
    assert!(
        terminal_production::TerminalProductionRequest::new(&changed, "enter")
            .produce_artifact()
            .is_err()
    );
}
