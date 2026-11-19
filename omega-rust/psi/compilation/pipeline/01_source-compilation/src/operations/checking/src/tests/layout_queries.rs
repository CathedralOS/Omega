//! Layout observations use the same physical type in proof and execution.

use super::*;

#[test]
fn primitive_layout_queries_keep_their_carrier_and_contract_obligations() {
    let positive = r#"// Expected exit: 0
// A layout observation keeps the same physical type in contracts and execution.
domain u32::Empty requires self > 0 && self < 0;
data Wrapped { value: u32; }
domain Wrapped::Tagged;
machine difference() -> u64
requires size_of<u32>() == 4, align_of<u32>() == 4
{ size_of<u32>() - 4 }
machine main() crashes Trap {
    transition size_of<u32>() == 4 && align_of<u32>() == 4 &&
        size_of<f64>() == 8 && align_of<f64>() == 8 &&
        size_of<addr>() == 8 && align_of<addr>() == 8 &&
        size_of<u32 in Empty>() == 4 &&
        size_of<Wrapped in Tagged>() == 4 && align_of<Wrapped in Tagged>() == 4 {
        true -> finish(difference())
        false -> bad()
    }
    machine finish(value: u64) crashes Trap {
        transition value == 0 { true -> done() false -> bad() }
    }
    machine done() {}
    machine bad() { crash Trap; }
}
"#;
    let negative = positive.replace(
        "requires size_of<u32>() == 4",
        "requires size_of<u32>() == 8",
    );
    assert_ne!(positive, negative);
    let reversed = positive.replace(
        "requires size_of<u32>() == 4",
        "requires 4 == size_of<u32>()",
    );
    let observed = [positive, negative.as_str(), reversed.as_str()].map(|text| {
        let source = super::boundary_calls::typed(text);
        let (source, _) = crate::preparation::prepare_typed_trees(
            source,
            &crate::preparation::PreparationRequest::settled(),
        )
        .expect("the query keeps its actual type operand");
        super::entryless::judged_typed_result(&source, false)
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
fn assert_deferred_wrong_size(closure: &crate::checking::AcceptedClosure) {
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
        } = module.scalar_applications.declaration(application)?
        else {
            return None;
        };
        if !matches!(module.structural_type(*subject)?.shape,
            StructuralTypeShape::PrimitiveScalar(carrier) if carrier == u32_carrier)
        {
            return None;
        }
        let bytes = match measurement {
            LayoutMeasurement::Size => size,
            LayoutMeasurement::Alignment => 4,
        };
        Some(ScalarValue::Integer(bignum::BigInt::from_u64(bytes)))
    };
    let mut pending = 0;
    let mut false_at_four = 0;
    for machine in &module.machines {
        for (obligation, condition) in module.layout_call_conditions(machine).unwrap() {
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
            let crate::checking::TargetCondition::Predicate(retained) = retained.as_ref() else {
                panic!("a call predicate was replaced by another target condition");
            };
            assert_eq!(retained, &condition);
            assert!(
                module
                    .scalar_applications
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
