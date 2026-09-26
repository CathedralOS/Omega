use crate::tests::flow::terminal_unit::{
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment, checked, machine_named,
};
use typed_trees_to_checked_trees::checked_trees::CheckedStructuralAccess;

#[test]
fn retains_literal_fixed_array_projection_for_direct_unit_calls_with_sibling_custody() {
    let checked = checked(
        r#"
        pub data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self)
        reaches PortIo
        ensures true;

        data Helper {}
        machine Helper::run(receipt: Receipt)
        reaches PortIo
        {
            Receipt::settle(receipt);
        }

        data Root {}
        machine Root::enter(receipts: [Receipt; 2])
        reaches PortIo
        {
            Helper::run(receipts[0]);
            Helper::run(receipts[1]);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("literal fixed-index ordinary calls should retain a complete checked plan");
    assert!(plans.for_machine(machine_named(&checked, "run")).is_some());
    assert_eq!(root.entry_claims.len(), 2);
    assert_eq!(root.operations.len(), 3);
    for (index, operation) in root.operations[..2].iter().enumerate() {
        let CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            claim_transfers,
            ..
        } = operation
        else {
            panic!("each literal element should transfer through an ordinary Unit call")
        };
        assert_eq!(
            structural_arguments[0].path,
            [CheckedUnitStructuralPathSegment::FixedIndex(index as u64)]
        );
        assert_eq!(claim_transfers.len(), 1);
        assert_eq!(claim_transfers[0].argument_index, 0);
        assert_eq!(
            claim_transfers[0].claim_identity,
            root.entry_claims[index].claim_identity
        );
    }
    let CheckedUnitEffectOperationPlan::Complete {
        trivial_affine_discards,
        ..
    } = &root.operations[2]
    else {
        unreachable!()
    };
    assert!(trivial_affine_discards.is_empty());
}

#[test]
fn fences_projected_unit_calls_outside_the_one_parameter_unit_slice() {
    let caller_with_extra_parameter = checked(
        r#"
        pub data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self)
        reaches PortIo
        ensures true;

        data Helper {}
        machine Helper::run(receipt: Receipt)
        reaches PortIo
        {
            Receipt::settle(receipt);
        }

        data Spare {}
        data Root {}
        machine Root::enter(receipts: [Receipt; 1], spare: Spare)
        reaches PortIo
        {
            Helper::run(receipts[0]);
        }
        "#,
    );
    assert!(
        caller_with_extra_parameter
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&caller_with_extra_parameter, "enter"))
            .is_none(),
        "a projected caller with another structural parameter must stay outside the slice"
    );

    let caller_with_scalar_parameter = checked(
        r#"
        pub data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self)
        reaches PortIo
        ensures true;

        data Helper {}
        machine Helper::run(receipt: Receipt)
        reaches PortIo
        {
            Receipt::settle(receipt);
        }

        data Root {}
        machine Root::enter(receipts: [Receipt; 1], flag: bool)
        reaches PortIo
        {
            Helper::run(receipts[0]);
        }
        "#,
    );
    assert!(
        caller_with_scalar_parameter
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&caller_with_scalar_parameter, "enter"))
            .is_none(),
        "a projected caller with a scalar parameter must stay outside the slice"
    );

    let callee_with_two_parameters = checked(
        r#"
        pub data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self)
        reaches PortIo
        ensures true;

        data Helper {}
        machine Helper::run(first: Receipt, second: Receipt)
        reaches PortIo
        {
            Receipt::settle(first);
            Receipt::settle(second);
        }

        data Root {}
        machine Root::enter(receipts: [Receipt; 2])
        reaches PortIo
        {
            Helper::run(receipts[0], receipts[1]);
        }
        "#,
    );
    assert!(
        callee_with_two_parameters
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&callee_with_two_parameters, "enter"))
            .is_none(),
        "a projected call with two callee parameters and arguments must stay outside the slice"
    );
}

#[test]
fn fences_nested_fixed_array_projection_without_partial_plan() {
    let checked = checked(
        r#"
        pub data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self)
        reaches PortIo
        ensures true;

        data Helper {}
        machine Helper::run(receipt: Receipt)
        reaches PortIo
        {
            Receipt::settle(receipt);
        }

        data Root {}
        machine Root::enter(receipts: [[Receipt; 2]; 2])
        reaches PortIo
        {
            Helper::run(receipts[0][0]);
            Helper::run(receipts[0][1]);
            Helper::run(receipts[1][0]);
            Helper::run(receipts[1][1]);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(
        plans
            .for_machine(machine_named(&checked, "enter"))
            .is_none(),
        "nested indexed custody must not publish a partial checked plan"
    );
    assert!(
        plans.structural_types.iter().all(|shape| !matches!(
            shape.shape,
            CheckedUnitStructuralTypeShape::FixedArray { .. }
        )),
        "a rejected nested array must not leave a retained placeholder shape"
    );
}

#[test]
fn four_index_direct_shared_projection_plans_on_shared_borrow_calls() {
    let checked = checked(
        r#"
        data Sink {}
        machine Sink::inspect(value: &u16) {}

        data Root {}
        machine Root::forward(values: &[[[[u16; 2]; 2]; 2]; 2]) {
            Sink::inspect(&values[0][0][0][0]);
        }
        "#,
    );

    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Root::forward"))
        .expect("a shared borrow of a literal-indexed projection is a checked call");
    assert!(
        plan.operations.iter().any(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } if structural_arguments.iter().any(|argument| {
                    argument.access == CheckedStructuralAccess::SharedBorrow
                        && argument.path.len() == 4
                })
            )
        }),
        "the four-segment projection reaches the callee as one shared borrow"
    );
}

#[test]
fn fences_dynamic_fixed_array_projection_for_direct_unit_calls() {
    let checked = checked(
        r#"
        data Ticket { value: u64; }

        data Helper {}
        machine Helper::run(ticket: Ticket) {}

        data Root { index: u64 [0..=1]; }
        machine Root::enter(&self, tickets: [Ticket; 2])
        {
            Helper::run(tickets[self.index]);
        }
        "#,
    );

    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, "enter"))
            .is_none(),
        "runtime-indexed custody must remain outside the checked terminal slice"
    );
}
