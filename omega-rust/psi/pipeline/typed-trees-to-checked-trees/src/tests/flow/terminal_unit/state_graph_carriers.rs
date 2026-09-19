//! Bounded owned byte carriers compare against byte-sequence literals through
//! a decomposed checked guard: the live-length observation dominates, then one
//! indexed-byte equality per literal position, so short-circuit evaluation
//! never reads past the carrier's live extent.
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;
use checked_trees::{
    CheckedBooleanExpression, CheckedComposedUnitControlTerminatorPlan,
    CheckedIntegerComparisonKind, CheckedScalarExpression, CheckedStructuralPredicatePathSegment,
    CheckedUnitEffectOperationPlan,
};
use typed_trees::types::PrimitiveType;

fn conjuncts(expression: &CheckedBooleanExpression) -> Vec<&CheckedBooleanExpression> {
    let mut flat = Vec::new();
    let mut frontier = expression;
    while let CheckedBooleanExpression::And { left, right } = frontier {
        flat.push(&**right);
        frontier = left;
    }
    flat.push(frontier);
    flat.reverse();
    flat
}

fn literal_operand(
    expression: &CheckedScalarExpression,
    digits: &str,
    landed_type: numerics::literals::LandedIntegerType,
) {
    let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
        panic!("expected an integer literal operand, found {expression:?}");
    };
    assert_eq!(literal.text(), digits);
    assert_eq!(
        literal.landing().map(|landing| landing.landed_type),
        Some(landed_type)
    );
}

#[test]
fn carrier_literal_guard_decomposes_into_length_and_bytes() {
    let checked = checked(
        r#"
        domain [u8; 5]::Utf8 requires valid_utf8(self);
        data Main { out: [u8; 5] in Utf8; }
        machine Main::main(&mut self) {
            self.out = "XXXXX";
            transition { _ -> check() }
            state check(&mut self) {
                transition self.out == "12345" { true -> ok() _ -> bad() }
            }
            state ok(&mut self) {}
            state bad(&mut self) {}
        }
    "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "Main::main"))
        .expect("a carrier literal guard keeps the composed machine plan");
    let check = &plan.states[1];
    let CheckedComposedUnitControlTerminatorPlan::Conditional { guard, .. } = &check.terminator
    else {
        panic!("the carrier guard survives as a conditional terminator");
    };
    let CheckedScalarExpression::Boolean(guard) = guard else {
        panic!("the composed guard stays boolean");
    };
    let conjuncts = conjuncts(guard);
    assert_eq!(conjuncts.len(), 1 + b"12345".len());
    let CheckedBooleanExpression::IntegerComparison { kind, left, right } = conjuncts[0] else {
        panic!("the live-length comparison leads the conjuncts");
    };
    assert_eq!(*kind, CheckedIntegerComparisonKind::Equal);
    assert!(
        matches!(left.as_ref(), CheckedScalarExpression::StructuralParameterByteLength {
            parameter_position: 0,
            path,
        } if path.as_slice() == [CheckedStructuralPredicatePathSegment::Field("out".to_owned())])
    );
    literal_operand(right, "5", numerics::literals::LandedIntegerType::U64);
    for (index, conjunct) in conjuncts[1..].iter().enumerate() {
        let CheckedBooleanExpression::IntegerComparison { kind, left, right } = conjunct else {
            panic!("each literal byte keeps its own equality");
        };
        assert_eq!(*kind, CheckedIntegerComparisonKind::Equal);
        let CheckedScalarExpression::StructuralParameterIndexedRead {
            parameter_position,
            path,
            index: read_index,
            primitive_type,
        } = left.as_ref()
        else {
            panic!("the indexed carrier read selects the bounded field");
        };
        assert_eq!(*parameter_position, 0);
        assert_eq!(*primitive_type, PrimitiveType::U8);
        assert_eq!(
            path.as_slice(),
            [CheckedStructuralPredicatePathSegment::Field(
                "out".to_owned()
            )]
        );
        literal_operand(
            read_index,
            &index.to_string(),
            numerics::literals::LandedIntegerType::U64,
        );
        literal_operand(
            right,
            &b"12345"[index].to_string(),
            numerics::literals::LandedIntegerType::U8,
        );
    }
}

#[test]
fn composed_body_retains_an_available_scalar_call() {
    let checked = checked(
        r#"
        domain [u8; 5]::Utf8 requires valid_utf8(self);
        machine narrow(value: u8 [0..=127]) -> u8 [0..=127] { value }
        data Main { out: [u8; 5] in Utf8; i: u64 in Wrapping; }
        machine Main::main(&mut self) {
            self.out = "XXXXX";
            self.i = 0;
            transition { _ -> step() }
            state step(&mut self) {
                self.out[self.i] = narrow(48);
                self.i = self.i + 1;
                transition self.i < 5 { true -> step() _ -> done() }
            }
            state done(&mut self) {}
        }
    "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "Main::main"))
        .expect("an available scalar call keeps the composed machine plan");
    let step = &plan.states[1];
    let call = step
        .operations
        .iter()
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::ScalarCall { result, .. } => Some(result),
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "the scalar call remains a retained operation: {:?}",
                step.operations
            )
        });
    assert_eq!(call.primitive_type, PrimitiveType::U8);
    // The byte store at the same statement consumes the call's scalar
    // binding; the pair must survive dependency pruning together.
    assert!(step.operations.iter().any(|operation| matches!(
        operation,
        CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store)
            if store.statement_index == call.statement_index
    )));
}
