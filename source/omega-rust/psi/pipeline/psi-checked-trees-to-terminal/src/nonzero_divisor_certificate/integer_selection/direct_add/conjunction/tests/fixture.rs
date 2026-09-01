use psi_core::{
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarTerm, ScalarType, ValueId,
};
use psi_proof_admission::accept_certificate;

use crate::nonzero_divisor_certificate::affine_custody::DefinitionIndex;

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

fn graph(
    integer_type: IntegerType,
    leaves: [IntegerValue; 3],
    lower: bool,
    commute_join: bool,
    shared: bool,
) -> Fixture {
    let values = (1..=6)
        .map(|id| value(id, integer_type))
        .collect::<Vec<_>>();
    let context = PropositionContext::from_value_types((1..=6).map(|id| {
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
