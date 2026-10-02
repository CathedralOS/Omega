use crate::tests::front_end::checked_program as checked;
use checked_trees::{CheckedStructuralAccess, CheckedUnitStructuralArgumentSourcePlan};

/// Source loan admission already proves the guarded sibling write disjoint.
/// The emitted range plan must retain that mutable loan rather than silently
/// replacing it with a shared descriptor. The local holds this loan once.
#[test]
fn guarded_mutable_window_retains_exclusive_range_plan() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/omega/pass/borrows/borrow_guarded_window_write_and_call/main.omg"
    ));
    let checked = checked(source.split("machine main()").next().unwrap());
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "write_outside_window")
        .unwrap()
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "guarded window omission: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(machine)
            )
        });
    let ranges = plan
        .states
        .iter()
        .flat_map(|state| &state.operations)
        .filter_map(|operation| match operation {
            checked_trees::CheckedUnitEffectOperationPlan::EstablishViewSubslice {
                result,
                source,
            } => Some((result, source)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(result, source)] = ranges.as_slice() else {
        panic!("one mutable window: {ranges:?}")
    };
    assert_eq!(source.access, CheckedStructuralAccess::MutableBorrow);
    assert_eq!(
        result.multiplicity,
        language_semantics::Multiplicity::Affine
    );
    assert!(matches!(
        source.source,
        CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
            root: checked_trees::CheckedStorageRoot::Parameter { index: 0 },
            start: Some(_),
            end: Some(_),
            ..
        }
    ));
    assert!(matches!(
        source.path.as_slice(),
        [checked_trees::CheckedUnitStructuralPathSegment::Field(_)]
    ));
    let state_plan = plan
        .states
        .iter()
        .find(|state| {
            state.operations.iter().any(|operation| {
                matches!(
                    operation,
                    checked_trees::CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. }
                )
            })
        })
        .expect("range state");
    let machine = crate::lookup::machine_by_symbol(&checked.typed, machine).unwrap();
    let state = checked
        .typed
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_plan.state)
        .unwrap();
    let ordinal = result.statement_index as usize;
    let typed_trees::statement::StatementNode::LocalData(local) = &checked
        .typed
        .statement_table
        .statements(state.statement_nodes)[ordinal]
    else {
        panic!("range binding")
    };
    let admit = |facts: &checked_trees::CheckFacts,
                 parameters: &[checked_trees::CheckedUnitStructuralParameterPlan]| {
        super::admit(
            &checked.typed,
            facts,
            machine,
            state,
            parameters,
            local.type_reference,
            local.initial_value,
            ordinal,
            checked_trees::CheckedSubsliceSite::LocalBinding,
        )
    };
    assert!(admit(&checked.facts, &state_plan.structural_parameters).is_some());
    let mut changed = checked.facts.clone();
    let loan = changed
        .borrow
        .loans
        .iter()
        .find_map(|(handle, loan)| (loan.owner_symbol == local.symbol).then_some(handle))
        .expect("captured range loan");
    changed.borrow.loans.get_mut(loan).kind = checked_trees::BorrowAccessKind::Read;
    assert!(
        admit(&changed, &state_plan.structural_parameters).is_none(),
        "read loan cannot publish mutable range"
    );
    let mut parameters = state_plan.structural_parameters.clone();
    parameters[0].access = CheckedStructuralAccess::SharedBorrow;
    assert!(
        admit(&checked.facts, &parameters).is_none(),
        "shared root cannot publish mutable range"
    );
}
