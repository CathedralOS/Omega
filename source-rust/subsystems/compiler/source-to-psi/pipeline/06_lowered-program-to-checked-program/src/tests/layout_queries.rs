//! Layout observations use the same physical type in proof and execution.

use super::*;

#[test]
fn storage_address_exposes_the_referent_of_an_ordinary_reference() {
    let program = "data Cell { word: u64; }
        machine main(cell: @mutable Cell) -> bool {
            let first: address = address_of(@mutable cell.word);
            cell.word = 11u64;
            let alias: @u64 = @cell.word;
            address_of(alias) == first
        }";
    let typed = super::boundary_calls::typed(program);
    let verdict = super::entryless::judged_typed_result(&typed, false).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("both references denote the same live field: {verdict:?}");
    };
    let addresses = accepted.elaboration.module.machines.iter()
        .flat_map(|machine| machine.operations())
        .filter(|operation| matches!(operation.kind, terminal_psi::OperationKind::StorageAddress { .. }))
        .count();
    assert_eq!(addresses, 2);
    accepted.publish_invocation().unwrap();

    let typed = super::boundary_calls::typed(
        "machine main(value: u64) -> address { address_of(value) }",
    );
    let verdict = super::entryless::judged_typed_result(&typed, false).unwrap();
    let RouteVerdict::Rejected { violations, .. } = verdict else {
        panic!("an owned scalar cannot expose a referent: {verdict:?}");
    };
    assert!(violations.iter().any(|rule| rule.statement.contains("address_of needs an ordinary reference")));
}

#[test]
fn alignment_contracts_use_the_borrowed_location_across_a_call() {
    let program = "machine measured(cell: @u64) -> u64
            ensures result == align_of(cell) { align_of(cell) }
        machine forward(first: @u64, second: @u64) -> u64
            ensures result == align_of(first) { measured(@SUBJECT) }";
    for (subject, same) in [("first", true), ("second", false)] {
        let verdict = super::entryless::judged(&program.replace("SUBJECT", subject), false);
        match verdict {
            RouteVerdict::Accepted(_) if same => {}
            RouteVerdict::Rejected { unproved, .. } if !same => assert!(
                unproved.iter().any(|obligation| matches!(
                    obligation.kind,
                    ObligationKind::NormalGuarantee { .. }
                )),
                "distinct inputs need their own alignment proof: {unproved:?}",
            ),
            other => panic!("returning the alignment of {subject}: {other:?}"),
        }
    }
}

#[test]
fn slice_alignment_contracts_require_a_live_first_element() {
    let program = "machine measured(cells: @[u64]) -> u64
        GUARD
        ensures result == align_of(cells) { align_of(cells) }
        machine main() {
            let words: [u64; 2] = [41u64, 42u64];
            let window: @[u64] = @words[..];
            let observed: u64 = measured(window);
        }";
    let guarded = super::entryless::judged(
        &program.replace("GUARD", "requires cells.length > 0u64"),
        false,
    );
    assert!(matches!(guarded, RouteVerdict::Accepted(_)), "{guarded:?}");
    let unguarded = super::entryless::judged(&program.replace("GUARD", ""), false);
    assert!(
        matches!(&unguarded, RouteVerdict::Rejected { violations, .. }
            if violations.iter().any(|row| row.statement.contains("contract index is not proved within its collection extent"))),
        "{unguarded:?}"
    );
}

#[test]
fn a_lent_view_keeps_the_alignment_of_its_unchanged_backing() {
    let program = "machine needs(values: @[u64])
        requires values.length > 0u64, align_of(values) >= 8u64 {}
        machine forward(values: VIEW_TYPE)
        requires values.length > 0u64, align_of(values) >= 8u64 {
            needs(values);
        }";
    for view_type in ["@[u64]", "@mutable [u64]"] {
        let verdict = super::entryless::judged(&program.replace("VIEW_TYPE", view_type), false);
        assert!(
            matches!(verdict, RouteVerdict::Accepted(_)),
            "{view_type}: {verdict:?}"
        );
    }
}

#[test]
fn a_fresh_local_can_meet_an_alignment_requirement_without_a_runtime_query() {
    let program = "machine needs(cell: @u64) requires align_of(cell) >= REQUIRED {}
        machine main() { let cell: u64 = 7u64; needs(@cell); }";
    for (bound, accepted) in [("8u64", true), ("16u64", false)] {
        let verdict = super::entryless::judged(&program.replace("REQUIRED", bound), false);
        match verdict {
            RouteVerdict::Accepted(_) if accepted => {}
            RouteVerdict::Rejected { unproved, .. } if !accepted => {
                assert!(!unproved.is_empty(), "the call needs a larger alignment");
            }
            other => panic!("fresh u64 storage with required alignment {bound}: {other:?}"),
        }
    }
}

#[test]
fn writing_the_field_contents_preserves_its_alignment() {
    let program = "data Cell { word: u64; }
        machine measured(word: @u64) -> u64
        ensures result == align_of(word) { align_of(word) }
        machine change(cell: @mutable Cell) -> u64
        ensures result == align_of(cell.word) {
            let before: u64 = measured(@cell.word);
            cell.word = 27u64;
            before
        }";
    let verdict = super::entryless::judged(program, false);
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

#[test]
fn replacing_a_reference_cannot_transfer_the_previous_alignment() {
    let program = "data Cell { word: u64; }
        data Borrowed<'storage> { cell: @'storage Cell; }
        machine choose<'storage>(left: @'storage Cell, right: @'storage Cell) -> u64
        ensures result == align_of(right) {
            let mutable selected: Borrowed = Borrowed { cell: left };
            let before: u64 = align_of(selected.cell);
            selected.cell = right;
            before
        }";
    let verdict = super::entryless::judged(program, false);
    let RouteVerdict::Rejected { unproved, .. } = verdict else {
        panic!("replacing the descriptor cannot change an earlier observation: {verdict:?}");
    };
    assert!(
        unproved
            .iter()
            .any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { .. })),
        "{unproved:?}"
    );
}

#[test]
fn a_live_field_only_proves_the_alignment_its_allocation_preserves() {
    let program = "data Slots { tag: u8; value: u64; }
        machine main() -> u64 {
            let slots: Slots = Slots { tag: 1u8, value: 9u64 };
            align_of(slots.FIELD) - 8u64
        }";
    for (field, accepted) in [("value", true), ("tag", false)] {
        let typed = super::boundary_calls::typed(&program.replace("FIELD", field));
        let prepared = crate::preparation::prepare_typed_trees(typed, &[], &[])
            .expect("both fields denote accessible storage");
        let result = super::entryless::judged_typed_result(prepared.trees(), false)
            .expect("both observations lower");
        match result {
            RouteVerdict::Accepted(_) if accepted => {}
            RouteVerdict::Rejected { unproved, .. } if !accepted => {
                assert!(
                    !unproved.is_empty(),
                    "the subtraction needs its own lower bound"
                );
            }
            other => panic!("alignment of {field}: {other:?}"),
        }
    }
}

#[test]
fn primitive_layout_queries_keep_their_carrier_and_contract_obligations() {
    let positive = r#"// Expected exit: 0
// A layout observation keeps the same physical type in contracts and execution.
domain u32::Empty requires self > 0 && self < 0;
data Wrapped { value: u32; }
domain Wrapped::Tagged;
machine difference() -> u64
requires size_of::<u32>() == 4, align_of::<u32>() == 4
{ size_of::<u32>() - 4 }
machine main() crashes Trap {
    transition size_of::<u32>() == 4 && align_of::<u32>() == 4 &&
        size_of::<f64>() == 8 && align_of::<f64>() == 8 &&
        size_of::<address>() == 8 && align_of::<address>() == 8 &&
        size_of::<u32 in Empty>() == 4 &&
        size_of::<Wrapped in Tagged>() == 4 && align_of::<Wrapped in Tagged>() == 4 {
        true -> finish(difference()),
        false -> bad()
    }
    machine finish(value: u64) crashes Trap {
        transition value == 0 { true -> done(), false -> bad() }
    }
    machine done() {}
    machine bad() { crash Trap; }
}
"#;
    let negative = positive.replace(
        "requires size_of::<u32>() == 4",
        "requires size_of::<u32>() == 8",
    );
    assert_ne!(positive, negative);
    let reversed = positive.replace(
        "requires size_of::<u32>() == 4",
        "requires 4 == size_of::<u32>()",
    );
    let observed = [positive, negative.as_str(), reversed.as_str()].map(|text| {
        let source = super::boundary_calls::typed(text);
        let source = crate::preparation::prepare_typed_trees(source, &[], &[])
            .expect("the query keeps its actual type operand");
        super::entryless::judged_typed_result(source.trees(), false)
    });
    for (name, result) in [
        "matching layout",
        "changed required size",
        "reversed equality",
    ]
    .into_iter()
    .zip(&observed)
    {
        match result {
            Ok(RouteVerdict::Accepted(_)) => eprintln!("{name}: accepted"),
            refused => eprintln!("{name}: {refused:?}"),
        }
    }
    assert!(
        matches!(&observed[0], Ok(RouteVerdict::Accepted(_))),
        "{:?}",
        observed[0]
    );
    let Ok(RouteVerdict::Accepted(negative)) = &observed[1] else {
        panic!(
            "the portable call must retain its target prerequisite: {:?}",
            observed[1]
        );
    };
    assert_deferred_wrong_size(negative);
    assert!(
        matches!(&observed[2], Ok(RouteVerdict::Accepted(_))),
        "equality orientation must retain the same query fact: {:?}",
        observed[2]
    );
}

// Portable acceptance keeps this target prerequisite pending. The CLI twin
// must refuse it after selecting the actual layout; no early certificate
// may replace that check.
fn assert_deferred_wrong_size(closure: &crate::AcceptedClosure) {
    use numerics::scalar_predicates::ScalarValue;
    use terminal_psi::{
        LayoutMeasurement, ScalarApplication, ScalarRangeObservation, StructuralTypeShape,
    };
    let module = &closure.elaboration.module;
    let u32_carrier = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
    let observe = |observation, size| {
        let ScalarRangeObservation::Application(application) = observation else {
            return None;
        };
        let ScalarApplication::LayoutObservation {
            subject,
            measurement,
            ..
        } = module.scalars.scalar_applications.declaration(application)?
        else {
            return None;
        };
        // The call is guarded by the complete source transition. Supply the
        // other observations in that transition for this 64-bit test target.
        let shape = &module.structural_type(*subject)?.shape;
        let (bytes, alignment) = match shape {
            StructuralTypeShape::PrimitiveScalar(carrier) if *carrier == u32_carrier => (size, 4),
            StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(
                semantic_vocabulary::IeeeFloatFormat::Binary64,
            )) => (8, 8),
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)) if integer.is_address() => (8, 8),
            StructuralTypeShape::Record { fields, .. }
                if fields.len() == 1 && fields[0].field_type == terminal_psi::StructuralFieldType::Scalar(u32_carrier) =>
            {
                (size, 4)
            }
            _ => panic!("unexpected layout observation in the source transition: {shape:?}"),
        };
        let bytes = match measurement {
            LayoutMeasurement::Size => bytes,
            LayoutMeasurement::Alignment => alignment,
        };
        Some(ScalarValue::Integer(bignum::BigInt::from_u64(bytes)))
    };
    let mut pending = 0;
    let mut false_at_four = 0;
    for machine in &module.machines {
        for (obligation, condition) in terminal_semantics::layout_call_conditions(module, machine).unwrap() {
            let verdict = closure
                .discharge
                .machines
                .get(machine.id)
                .unwrap()
                .verdicts
                .get(obligation)
                .unwrap();
            let Ok(Discharged::TargetCondition(retained)) = verdict else {
                panic!("a target prerequisite was replaced by another discharge: {verdict:?}");
            };
            let crate::TargetCondition::Predicate(retained) = retained.as_ref() else {
                panic!("a call predicate was replaced by another target condition");
            };
            assert_eq!(retained, &condition);
            assert!(
                module
                    .scalars.scalar_applications
                    .conditions
                    .predicates()
                    .contains(retained)
            );
            assert_eq!(
                retained.role,
                semantic_vocabulary::ScalarConditionRole::Prerequisite
            );
            assert_eq!(
                condition.evaluate(&mut |value| observe(value, 8)),
                Some(ScalarValue::Boolean(true))
            );
            false_at_four += usize::from(
                condition.evaluate(&mut |value| observe(value, 4))
                    == Some(ScalarValue::Boolean(false)),
            );
            assert_eq!(condition.evaluate(&mut |_| None), None);
            pending += 1;
        }
    }
    assert_eq!(pending, 2, "both authored layout requirements stay pending");
    assert_eq!(
        false_at_four, 1,
        "the changed size-8 requirement refuses the size-4 observation"
    );
}

#[test]
fn layout_call_requirements_apply_only_on_their_incoming_path() {
    use numerics::scalar_predicates::ScalarValue;
    use terminal_psi::ScalarRangeObservation;

    let source = r#"
machine use_layout() requires size_of::<u32>() == 8u64 {}
machine main() {
    transition SELECT {
        true -> done(),
        false -> use_layout()
    }
    machine done() requires true {}
}
"#;
    for (guard, permitted) in [
        ("size_of::<u32>() == 4u64", [true, true, false, false]),
        ("size_of::<u32>() <= 16u64 && size_of::<u32>() != 16u64", [true, true, false, false]),
        ("size_of::<u32>() == 4u64 || size_of::<u32>() == 16u64", [true, true, true, false]),
    ] {
        let typed = super::boundary_calls::typed(&source.replace("SELECT", guard));
        let prepared = crate::preparation::prepare_typed_trees(typed, &[], &[]).unwrap();
        let Ok(RouteVerdict::Accepted(closure)) =
            super::entryless::judged_typed_result(prepared.trees(), false)
        else {
            panic!("the portable call must preserve its deployment prerequisite");
        };
        let module = &closure.elaboration.module;
        let conditions = module
            .machines
            .iter()
            .flat_map(|machine| terminal_semantics::layout_call_conditions(module, machine).unwrap())
            .map(|(_, condition)| condition)
            .collect::<Vec<_>>();
        assert_eq!(conditions.len(), 1, "the called machine has one target prerequisite");
        for (size, permitted) in [4, 8, 16, 32].into_iter().zip(permitted) {
            let checked = conditions[0].evaluate(&mut |value| {
                matches!(value, ScalarRangeObservation::Application(_))
                    .then(|| ScalarValue::Integer(bignum::BigInt::from_u64(size)))
            });
            assert_eq!(checked, Some(ScalarValue::Boolean(permitted)), "{guard} at size {size}");
        }
    }
}

#[test]
fn exact_arithmetic_keeps_the_selected_layout_as_a_target_prerequisite() {
    let source = "data Row { tag: u8; count: u64; }
        machine extent() -> u64 { size_of::<Row>() + align_of::<Row>() }";
    let typed = super::boundary_calls::typed(source);
    let typed = crate::preparation::prepare_typed_trees(typed, &[], &[]).unwrap();
    let verdict = super::entryless::judged_typed_result(typed.trees(), false).unwrap();
    let RouteVerdict::Accepted(closure) = verdict else {
        panic!("portable arithmetic should retain a range for the selected layout: {verdict:?}");
    };
    let conditions = &closure
        .elaboration
        .module
        .scalars
        .scalar_applications
        .conditions;
    let [range] = conditions.ranges() else {
        panic!("expected a single pending arithmetic condition");
    };
    use numerics::integer_bounds::IntegerBoundStep::{Add, Observation};
    use terminal_psi::ScalarRangeObservation::Application;
    assert!(matches!(conditions.steps(range).unwrap(),
        [Observation(Application(size)), Observation(Application(align)), Add] if size != align));
}

#[test]
fn record_geometry_carries_layout_values_through_copies_calls_and_arrivals() {
    let source =
        include_str!("../../../../../../../../samples/cli/probes/record_geometry/main.omg");
    let typed = super::boundary_calls::typed(source);
    let typed = crate::preparation::prepare_typed_trees(typed, &[], &[]).unwrap();
    let result = super::entryless::judged_typed_result(typed.trees(), false).unwrap();
    assert!(matches!(result, RouteVerdict::Accepted(_)), "{result:?}");
}

#[test]
fn conditional_target_arithmetic_owes_the_range_on_its_own_branch() {
    use bignum::BigInt;
    use terminal_psi::ScalarRangeObservation;

    for (guard, expected) in [("==", 0u128), (">", u128::from(u64::MAX) + 16)] {
        let source = format!(
            "data Row {{ tag: u8; count: u64; }}
            machine guarded() -> u64 {{
                let bytes: u64 = size_of::<Row>();
                match bytes {guard} 0u64 {{
                    true -> bytes + 18446744073709551615u64, _ -> 0u64
                }}
            }}"
        );
        let typed = super::boundary_calls::typed(&source);
        let typed = crate::preparation::prepare_typed_trees(typed, &[], &[]).unwrap();
        let verdict = super::entryless::judged_typed_result(typed.trees(), false).unwrap();
        let RouteVerdict::Accepted(closure) = verdict else {
            panic!("expected a portable conditional arithmetic check: {verdict:?}");
        };
        let conditions = &closure
            .elaboration
            .module
            .scalars
            .scalar_applications
            .conditions;
        let [range] = conditions.ranges() else {
            panic!("expected a single arithmetic range")
        };
        let actual = numerics::integer_bounds::evaluate(
            conditions.steps(range).unwrap(),
            &mut |observation| match observation {
                ScalarRangeObservation::Literal(value) => Ok(value.clone()),
                ScalarRangeObservation::Application(_) => Ok(BigInt::from_u64(16)),
                _ => Err("unknown observation in a closed layout condition".to_owned()),
            },
        )
        .unwrap();
        assert_eq!(
            actual,
            (BigInt::from_u128(expected), BigInt::from_u128(expected)),
            "{guard}"
        );
    }
}
