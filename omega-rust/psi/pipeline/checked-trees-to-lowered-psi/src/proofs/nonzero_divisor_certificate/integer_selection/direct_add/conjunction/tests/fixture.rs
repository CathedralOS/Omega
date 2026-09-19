use proof_admission::accept_certificate;
use semantic_vocabulary::{
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarTerm, ScalarType, ValueId,
};

use crate::proofs::nonzero_divisor_certificate::affine_custody::DefinitionIndex;

use super::super::model::{SearchBudget, SearchOutcome};

pub(super) struct Fixture {
    pub(super) integer_type: IntegerType,
    pub(super) context: PropositionContext,
    pub(super) goal: Proposition,
    pub(super) target: ScalarTerm,
    pub(super) left: ScalarTerm,
    pub(super) right: ScalarTerm,
    pub(super) axioms: Vec<Proposition>,
    pub(super) lower: bool,
}

impl Fixture {
    pub(super) fn prove(&self, budget: SearchBudget) -> SearchOutcome {
        let definitions = DefinitionIndex::new(&self.axioms);
        super::super::prove_with_budget(
            &self.context,
            &self.goal,
            self.integer_type,
            &self.left,
            &self.right,
            &self.target,
            self.lower,
            &[],
            &self.axioms,
            &definitions,
            budget,
        )
    }

    pub(super) fn admit(&self, outcome: &SearchOutcome) {
        accept_certificate(
            &self.context,
            &self.goal,
            &[],
            &self.axioms,
            outcome.proof.as_ref().expect("proof"),
        )
        .expect("the independent kernel admits the produced certificate");
    }
}

pub(super) fn fork_join(sign: IntegerSign, bits: u16, lower: bool, commute_join: bool) -> Fixture {
    let integer_type = IntegerType::new(sign, bits).expect("fixed integer type");
    let values = match sign {
        IntegerSign::Signed => [
            IntegerValue::Signed(-3),
            IntegerValue::Signed(5),
            IntegerValue::Signed(7),
        ],
        IntegerSign::Unsigned => [
            IntegerValue::Unsigned(3),
            IntegerValue::Unsigned(5),
            IntegerValue::Unsigned(7),
        ],
    };
    graph(integer_type, values, lower, commute_join, false)
}

pub(super) fn shared_join(sign: IntegerSign, bits: u16, lower: bool) -> Fixture {
    let integer_type = IntegerType::new(sign, bits).expect("fixed integer type");
    let values = match sign {
        IntegerSign::Signed => [
            IntegerValue::Signed(-3),
            IntegerValue::Signed(5),
            IntegerValue::Signed(7),
        ],
        IntegerSign::Unsigned => [
            IntegerValue::Unsigned(3),
            IntegerValue::Unsigned(5),
            IntegerValue::Unsigned(7),
        ],
    };
    graph(integer_type, values, lower, false, true)
}

pub(super) fn outer_fork_join(
    sign: IntegerSign,
    bits: u16,
    lower: bool,
    commute_outer: bool,
) -> Fixture {
    let mut fixture = fork_join(sign, bits, lower, false);
    let join = value(7, fixture.integer_type);
    fixture.axioms.push(Proposition::Equal(
        join.clone(),
        ScalarTerm::exact_integer_add(
            fixture.integer_type,
            fixture.left.clone(),
            fixture.right.clone(),
        )
        .expect("computed join"),
    ));
    let root = value(1, fixture.integer_type);
    if commute_outer {
        set_top(&mut fixture, join, root);
    } else {
        set_top(&mut fixture, root, join);
    }
    fixture
}

pub(super) fn two_computed_joins(sign: IntegerSign, bits: u16, lower: bool) -> Fixture {
    let mut fixture = outer_fork_join(sign, bits, lower, false);
    let second_join = value(8, fixture.integer_type);
    fixture.axioms.push(Proposition::Equal(
        second_join.clone(),
        ScalarTerm::exact_integer_add(
            fixture.integer_type,
            value(7, fixture.integer_type),
            value(5, fixture.integer_type),
        )
        .expect("second computed join"),
    ));
    let root = value(1, fixture.integer_type);
    set_top(&mut fixture, root, second_join);
    fixture
}

/// dice_roller shape: each roll is `remainder(dividend, 6) + 1` and the total
/// chains them as `((r1 + r2) + r3) + r4`. Remainder-defined operands terminate
/// the chain through their bounded image, and three nested computed joins
/// compose the partial sums.
pub(super) fn remainder_leaf_joins(sign: IntegerSign, bits: u16, lower: bool) -> Fixture {
    let integer_type = IntegerType::new(sign, bits).expect("fixed integer type");
    let values = (1..=16)
        .map(|id| value(id, integer_type))
        .collect::<Vec<_>>();
    let context = PropositionContext::from_value_types((1..=16).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(integer_type),
        )
    }))
    .expect("roll context");
    let number = |magnitude: u64| match sign {
        IntegerSign::Signed => IntegerValue::Signed(i128::from(magnitude)),
        IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(magnitude)),
    };
    let add = |left: ScalarTerm, right: ScalarTerm| {
        ScalarTerm::exact_integer_add(integer_type, left, right).expect("exact add")
    };
    let six = ScalarTerm::integer(integer_type, number(6)).expect("divisor literal");
    let one = values[2].clone();
    let mut axioms = vec![Proposition::Equal(
        one.clone(),
        literal(integer_type, number(1)),
    )];
    // rolls r1..r4 at values 4, 7, 10, 13; dividends at 1, 5, 8, 11 and
    // remainders at 2, 6, 9, 12.
    let dividends = [100_u64, 77, 44, 9];
    let dividend_slots = [0_usize, 4, 7, 10];
    let remainder_slots = [1_usize, 5, 8, 11];
    let rolls = [3_usize, 6, 9, 12];
    let mut roll_values = Vec::new();
    for index in 0..dividends.len() {
        let dividend_value = values[dividend_slots[index]].clone();
        let remainder_value = values[remainder_slots[index]].clone();
        let roll = rolls[index];
        let dividend = dividends[index];
        axioms.push(Proposition::Equal(
            dividend_value.clone(),
            literal(integer_type, number(dividend)),
        ));
        axioms.push(Proposition::Equal(
            remainder_value.clone(),
            ScalarTerm::exact_integer_remainder(integer_type, dividend_value, six.clone())
                .expect("exact remainder"),
        ));
        let roll_value = values[roll].clone();
        axioms.push(Proposition::Equal(
            roll_value.clone(),
            add(remainder_value, one.clone()),
        ));
        roll_values.push(roll_value);
    }
    // s1 = r1 + r2, s2 = s1 + r3, s3 = s2 + r4 at values 14, 15, 16.
    let first_sum = values[13].clone();
    axioms.push(Proposition::Equal(
        first_sum.clone(),
        add(roll_values[0].clone(), roll_values[1].clone()),
    ));
    let second_sum = values[14].clone();
    axioms.push(Proposition::Equal(
        second_sum.clone(),
        add(first_sum, roll_values[2].clone()),
    ));
    let third_sum = values[15].clone();
    axioms.push(Proposition::Equal(
        third_sum.clone(),
        add(second_sum, roll_values[3].clone()),
    ));
    let mut fixture = Fixture {
        integer_type,
        context,
        goal: Proposition::Truth,
        target: ScalarTerm::integer(integer_type, number(0)).expect("placeholder"),
        left: roll_values[0].clone(),
        right: roll_values[1].clone(),
        axioms,
        lower,
    };
    set_top(&mut fixture, third_sum, roll_values[1].clone());
    fixture
}

fn graph(
    integer_type: IntegerType,
    leaves: [IntegerValue; 3],
    lower: bool,
    commute_join: bool,
    shared: bool,
) -> Fixture {
    let values = (1..=8)
        .map(|id| value(id, integer_type))
        .collect::<Vec<_>>();
    let context = PropositionContext::from_value_types((1..=8).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(integer_type),
        )
    }))
    .expect("graph context");
    let exact_add = |left: ScalarTerm, right: ScalarTerm| {
        ScalarTerm::exact_integer_add(integer_type, left, right).expect("exact add")
    };
    let inner = exact_add(values[1].clone(), values[2].clone());
    let middle = exact_add(values[0].clone(), values[3].clone());
    let bridge = exact_add(values[2].clone(), values[0].clone());
    let axioms = vec![
        Proposition::Equal(values[0].clone(), literal(integer_type, leaves[0])),
        Proposition::Equal(values[1].clone(), literal(integer_type, leaves[1])),
        Proposition::Equal(values[2].clone(), literal(integer_type, leaves[2])),
        Proposition::Equal(values[3].clone(), inner),
        Proposition::Equal(values[4].clone(), middle),
        Proposition::Equal(values[5].clone(), bridge),
    ];
    let (left, right) = if shared {
        (values[4].clone(), values[4].clone())
    } else if commute_join {
        (values[5].clone(), values[4].clone())
    } else {
        (values[4].clone(), values[5].clone())
    };
    let target = exact_add(left.clone(), right.clone());
    let math_sum = IntegerMathTerm::Add(
        Box::new(math_value(&left, integer_type)),
        Box::new(math_value(&right, integer_type)),
    );
    let carrier = IntegerMathTerm::literal(if lower {
        integer_type.minimum_value()
    } else {
        integer_type.maximum_value()
    });
    let goal = if lower {
        Proposition::IntegerMathLessOrEqual(carrier, math_sum)
    } else {
        Proposition::IntegerMathLessOrEqual(math_sum, carrier)
    };
    Fixture {
        integer_type,
        context,
        goal,
        target,
        left,
        right,
        axioms,
        lower,
    }
}

fn set_top(fixture: &mut Fixture, left: ScalarTerm, right: ScalarTerm) {
    fixture.target =
        ScalarTerm::exact_integer_add(fixture.integer_type, left.clone(), right.clone())
            .expect("top exact add");
    let sum = IntegerMathTerm::Add(
        Box::new(math_value(&left, fixture.integer_type)),
        Box::new(math_value(&right, fixture.integer_type)),
    );
    let carrier = IntegerMathTerm::literal(if fixture.lower {
        fixture.integer_type.minimum_value()
    } else {
        fixture.integer_type.maximum_value()
    });
    fixture.goal = if fixture.lower {
        Proposition::IntegerMathLessOrEqual(carrier, sum)
    } else {
        Proposition::IntegerMathLessOrEqual(sum, carrier)
    };
    fixture.left = left;
    fixture.right = right;
}

pub(super) fn value(id: u64, integer_type: IntegerType) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(id).expect("value id"),
        ScalarType::Integer(integer_type),
    )
}

pub(super) fn literal(integer_type: IntegerType, value: IntegerValue) -> ScalarTerm {
    ScalarTerm::integer(integer_type, value).expect("admitted literal")
}

fn math_value(value: &ScalarTerm, integer_type: IntegerType) -> IntegerMathTerm {
    let ScalarTerm::Value { id, .. } = value else {
        panic!("fixture endpoints are values")
    };
    IntegerMathTerm::MathValue {
        source_type: integer_type,
        value: *id,
    }
}
