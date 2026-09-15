use crate::tests::flow::terminal_unit::{
    BindingRelevance, CheckedUnitEffectOperationPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypeShape, Multiplicity, PrimitiveType,
    checked, machine_named, record_fields,
};

#[test]
fn retains_completion_receipt_for_result_bearing_boundary_call() {
    let checked = checked(
        r#"
        pub data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self) -> i32
        reaches PortIo
        ensures true;

        data Root {}

        machine Root::enter(receipt: Receipt) -> i32
        reaches PortIo
        {
            let status: i32 = receipt.settle();
            status
        }
        "#,
    );

    let plan = checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .for_machine(machine_named(&checked, "enter"))
        .expect("result-bearing boundary call should retain a checked custody plan");
    assert_eq!(plan.entry_claims.len(), 1);
    assert_eq!(plan.result_type, PrimitiveType::I32);
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        completion_receipts,
        ..
    } = &plan.boundary_call
    else {
        panic!("scalar boundary plan should retain one bodyless boundary call")
    };
    assert_eq!(completion_receipts.len(), 1);
    assert_eq!(
        completion_receipts[0].claim_identity,
        plan.entry_claims[0].claim_identity
    );
}

#[test]
fn retains_disjoint_sibling_custody_inside_one_affine_aggregate() {
    let checked = checked(
        r#"
        pub data Token [linear] { value: u64; }
        pub data Envelope { #7 left: Token; #9 right: Token; }

        pub domain Envelope::Pending;

        boundary machine Envelope::settle(self)
        reaches PortIo
        requires
            self in Envelope::Pending
        ensures true;

        data Helper {}

        machine Helper::run(envelope: Envelope in Pending)
        reaches PortIo
        {
            envelope.settle();
        }

        data Root {}

        machine Root::enter(envelope: Envelope in Pending)
        reaches PortIo
        {
            Helper::run(envelope);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("multi-field aggregate root plan");
    let helper = plans
        .for_machine(machine_named(&checked, "run"))
        .expect("multi-field aggregate helper plan");
    for machine in [root, helper] {
        assert_eq!(
            machine.structural_parameters[0].multiplicity,
            Multiplicity::Affine
        );
        assert_eq!(machine.entry_claims.len(), 2);
        assert_eq!(
            machine.entry_claims[0].path,
            [CheckedUnitStructuralPathSegment::Field("#7".to_owned())]
        );
        assert_eq!(
            machine.entry_claims[1].path,
            [CheckedUnitStructuralPathSegment::Field("#9".to_owned())]
        );
    }
    let CheckedUnitEffectOperationPlan::CallUnit {
        claim_transfers, ..
    } = &root.operations[0]
    else {
        panic!("root should transfer both sibling claims to helper")
    };
    assert_eq!(claim_transfers.len(), 2);
    assert!(
        claim_transfers
            .iter()
            .all(|transfer| transfer.argument_index == 0)
    );
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        completion_receipts,
        ..
    } = &helper.operations[0]
    else {
        panic!("helper should settle both sibling claims at the boundary")
    };
    assert_eq!(completion_receipts.len(), 2);
    assert!(
        completion_receipts
            .iter()
            .all(|settlement| settlement.argument_index == 0)
    );
}

#[test]
fn retains_nested_record_field_custody_for_unit_call_closure() {
    let checked = checked(
        r#"
        pub data Token [linear] { value: u64; }
        pub data Pocket { #9 token: Token; }
        pub data Envelope { #7 pocket: Pocket; }

        pub domain Envelope::Pending;

        boundary machine Envelope::settle(self)
        reaches PortIo
        requires
            self in Envelope::Pending
        ensures true;

        data Helper {}

        machine Helper::run(envelope: Envelope in Pending)
        reaches PortIo
        {
            envelope.settle();
        }

        data Root {}

        machine Root::enter(envelope: Envelope in Pending)
        reaches PortIo
        {
            Helper::run(envelope);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("nested aggregate root plan");
    let helper = plans
        .for_machine(machine_named(&checked, "run"))
        .expect("nested aggregate helper plan");
    for machine in [root, helper] {
        assert_eq!(
            machine.structural_parameters[0].multiplicity,
            Multiplicity::Affine
        );
        assert_eq!(machine.entry_claims.len(), 1);
        assert_eq!(
            machine.entry_claims[0].path,
            [
                CheckedUnitStructuralPathSegment::Field("#7".to_owned()),
                CheckedUnitStructuralPathSegment::Field("#9".to_owned()),
            ]
        );
    }
    let CheckedUnitEffectOperationPlan::CallUnit {
        claim_transfers, ..
    } = &root.operations[0]
    else {
        panic!("root should transfer nested custody to helper")
    };
    assert_eq!(claim_transfers.len(), 1);
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        completion_receipts,
        ..
    } = &helper.operations[0]
    else {
        panic!("helper should settle nested custody at the boundary")
    };
    assert_eq!(completion_receipts.len(), 1);
}

#[test]
fn retains_literal_fixed_array_boundary_settlements_with_sibling_claims() {
    let checked = checked(
        r#"
        pub data Receipt [linear] { value: u64; }

        boundary machine Receipt::settle(self)
        reaches PortIo
        ensures true;

        data Root {}

        machine Root::enter(receipts: [Receipt; 2])
        reaches PortIo
        {
            Receipt::settle(receipts[0]);
            Receipt::settle(receipts[1]);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("literal fixed-array settlement plan");
    assert_eq!(root.structural_parameters.len(), 1);
    assert_eq!(
        root.structural_parameters[0].multiplicity,
        Multiplicity::Linear
    );
    assert_eq!(root.entry_claims.len(), 2);
    for (index, claim) in root.entry_claims.iter().enumerate() {
        assert_eq!(claim.parameter_index, 0);
        assert_eq!(
            claim.path,
            [CheckedUnitStructuralPathSegment::FixedIndex(
                u64::try_from(index).unwrap()
            )]
        );
    }

    let array = plans
        .structural_types
        .iter()
        .find(|shape| {
            matches!(
                shape.shape,
                CheckedUnitStructuralTypeShape::FixedArray { .. }
            )
        })
        .expect("fixed-array structural shape");
    let CheckedUnitStructuralTypeShape::FixedArray {
        element_type_identity,
        length,
    } = &array.shape
    else {
        panic!("expected fixed-array shape")
    };
    assert!(element_type_identity.contains("Receipt"));
    assert_eq!(*length, 2);

    assert_eq!(root.operations.len(), 3);
    for (index, operation) in root.operations[..2].iter().enumerate() {
        let CheckedUnitEffectOperationPlan::BoundaryCall {
            structural_arguments,
            completion_receipts,
            ..
        } = operation
        else {
            panic!("each literal element should settle at the boundary")
        };
        assert_eq!(structural_arguments.len(), 1);
        assert_eq!(
            structural_arguments[0].path,
            [CheckedUnitStructuralPathSegment::FixedIndex(
                u64::try_from(index).unwrap()
            )]
        );
        assert!(structural_arguments[0].type_identity.contains("Receipt"));
        assert_eq!(completion_receipts.len(), 1);
        assert_eq!(completion_receipts[0].argument_index, 0);
        assert_eq!(
            completion_receipts[0].claim_identity,
            root.entry_claims[index].claim_identity
        );
    }
}

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
fn fences_four_index_direct_shared_projection_without_widening_legacy_calls() {
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

    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, "Root::forward"))
            .is_none(),
        "four direct indexes must not widen the legacy non-write projected-call cohort"
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

#[test]
fn retains_reverse_declaration_affine_discards_on_unit_return() {
    let checked = checked(
        r#"
        data Ticket { value: u64; }
        data Root {}

        machine Root::enter(first: Ticket, second: Ticket) {}
        "#,
    );

    let root = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("affine Unit root plan");
    assert_eq!(root.structural_parameters.len(), 2);
    assert!(root.entry_claims.is_empty());
    let [
        CheckedUnitEffectOperationPlan::Complete {
            trivial_affine_discards,
            ..
        },
    ] = root.operations.as_slice()
    else {
        panic!("affine Unit root should contain only its return edge")
    };
    assert_eq!(trivial_affine_discards, &[1, 0]);
}

#[test]
fn transferred_affine_parameter_is_not_also_discarded_on_return() {
    let checked = checked(
        r#"
        data Ticket { value: u64; }
        data Helper {}
        machine Helper::run(ticket: Ticket) {}
        data Root {}
        machine Root::enter(ticket: Ticket) {
            Helper::run(ticket);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("affine transfer root plan");
    let helper = plans
        .for_machine(machine_named(&checked, "run"))
        .expect("affine transfer helper plan");
    let CheckedUnitEffectOperationPlan::Complete {
        trivial_affine_discards: root_discards,
        ..
    } = root.operations.last().unwrap()
    else {
        unreachable!()
    };
    let CheckedUnitEffectOperationPlan::Complete {
        trivial_affine_discards: helper_discards,
        ..
    } = helper.operations.last().unwrap()
    else {
        unreachable!()
    };
    assert!(root_discards.is_empty());
    assert_eq!(helper_discards, &[0]);
}

#[test]
fn omits_nonconstant_port_and_retains_supported_payloadless_sum() {
    let checked = checked(
        r#"
        data DynamicPort { port: u16; }
        machine DynamicPort::write(&mut self)
        reaches PortIo
        {
            asm { out self.port, 7 }
        }

        data SupportedSum {
            case Empty;
        }
        data NestedRoot { value: SupportedSum; }
        machine NestedRoot::run() {}
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(
        plans
            .for_machine(machine_named(&checked, "write"))
            .is_none()
    );
    assert!(plans.for_machine(machine_named(&checked, "run")).is_some());
    let sum = plans
        .structural_types
        .iter()
        .find(|shape| shape.identity.contains("SupportedSum"))
        .expect("the closed payload-less sum has an exact structural declaration");
    let CheckedUnitStructuralTypeShape::Sum { cases } = &sum.shape else {
        panic!("payload-less sum must not be represented as an empty record")
    };
    assert!(
        matches!(cases.as_slice(), [case] if case.identity == "Empty" && case.fields.is_empty())
    );
    let root = plans
        .structural_types
        .iter()
        .find(|shape| shape.identity.contains("NestedRoot"))
        .expect("the enclosing record has an exact structural declaration");
    assert!(matches!(
        record_fields(root),
        [field]
            if matches!(&field.field_type,
                CheckedUnitStructuralFieldType::Structural { type_identity }
                    if type_identity == &sum.identity)
    ));
}

#[test]
fn retains_opaque_erased_field_identity_in_unit_structural_shape() {
    let checked = checked(
        r#"
        data Evidence { case Only; }
        data Certified {
            value: u64;
            proof [erased]: Evidence;
        }
        machine Certified::run(&self) {}
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(plans.for_machine(machine_named(&checked, "run")).is_some());
    let certified = plans
        .structural_types
        .iter()
        .find(|shape| shape.identity.contains("Certified"))
        .expect("certified structural shape");
    let certified_fields = record_fields(certified);
    assert_eq!(certified_fields.len(), 2);
    assert_eq!(certified_fields[0].relevance, BindingRelevance::Relevant);
    assert_eq!(certified_fields[1].relevance, BindingRelevance::Erased);
    assert!(matches!(
        &certified_fields[1].field_type,
        CheckedUnitStructuralFieldType::Erased { type_identity }
            if type_identity.contains("Evidence")
    ));
    assert!(
        plans
            .structural_types
            .iter()
            .all(|shape| !shape.identity.contains("Evidence")),
        "opaque erased field carriers do not enter the executable structural graph"
    );
}

#[test]
fn presents_projected_owned_byte_carrier_at_a_borrowed_boundary_view() {
    let checked = checked(
        r#"
        domain [u8; 16]::Utf8
        requires
            valid_utf8(self);

        boundary trait Console {
            machine read_line(out_line: &mut [u8])
            reaches Console;
        }

        data Root { line: [u8; 16] in Utf8; }
        machine Root::enter(self)
        reaches Console
        {
            Console::read_line(&mut self.line);
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let boundary = plans
        .boundary_machines
        .iter()
        .find(|boundary| boundary.structural_parameters.len() == 1)
        .expect("read_line boundary structural parameter");
    let view_identity = boundary.structural_parameters[0].type_identity.clone();
    assert!(matches!(
        plans
            .structural_types
            .iter()
            .find(|shape| shape.identity == view_identity)
            .expect("borrowed byte-sequence shape")
            .shape,
        CheckedUnitStructuralTypeShape::ByteSequence(
            checked_trees::CheckedByteSequenceCarrier::BorrowedView
        )
    ));
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("projected boundary caller plan");
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        structural_arguments,
        ..
    } = &root.operations[0]
    else {
        panic!("read_line should retain a bodyless boundary call")
    };
    // The argument names the attachment field it projects, and presents the
    // requirement's borrowed view: the lowering derives the bound from this
    // exact place rather than materializing a view here.
    assert!(matches!(
        structural_arguments.as_slice(),
        [argument]
            if argument.source_parameter_index() == Some(0)
                && matches!(
                    argument.path.as_slice(),
                    [CheckedUnitStructuralPathSegment::Field(field)] if field.ends_with("line")
                )
                && argument.type_identity == view_identity
                && argument.access == checked_trees::CheckedStructuralAccess::MutableBorrow
    ));
}

#[test]
fn projected_fixed_byte_arrays_lend_initialized_storage_to_boundary_views() {
    // Raw byte arrays lend their complete initialized extent. They do not
    // become bounded owners; non-byte array/view pairs remain outside this route.
    let source = r#"
        boundary trait Console {
            machine read_line(out_line: &mut [u8])
            reaches Console;
        }

        data Root { line: [u8; 16]; }
        machine Root::enter(self)
        reaches Console
        {
            Console::read_line(&mut self.line);
        }
        "#;
    for (element, admitted) in [("u8", true), ("u16", false)] {
        let checked = checked(&source.replace("u8", element));
        let plans = &checked.facts.flow.terminal_unit_effects;
        assert_eq!(
            plans
                .for_machine(machine_named(&checked, "enter"))
                .is_some(),
            admitted
        );
        if admitted {
            assert!(plans.structural_types.iter().any(|declaration| matches!(
                declaration.shape,
                CheckedUnitStructuralTypeShape::FixedArray { length: 16, .. }
            )));
        }
    }
}
