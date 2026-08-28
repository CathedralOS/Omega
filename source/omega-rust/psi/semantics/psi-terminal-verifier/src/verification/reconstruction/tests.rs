use super::*;
use psi_core::{IntegerSign, IntegerType, IntegerValue, ScalarType};

fn value(id: u64, integer_type: IntegerType) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(id).expect("value id"),
        ScalarType::Integer(integer_type),
    )
}

#[test]
fn exact_shift_count_selects_only_complete_prior_canonical_evidence() {
    let value_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let count_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let count = value(2, count_type);
    let goal = CanonicalScalarGoal::ExactShiftCount {
        value_type,
        count_type,
        count: count.clone(),
    };
    let lower = Proposition::LessOrEqual(
        ScalarTerm::integer(count_type, IntegerValue::Signed(0)).expect("i8 zero"),
        count.clone(),
    );
    let upper = Proposition::LessOrEqual(
        count,
        ScalarTerm::integer(count_type, IntegerValue::Signed(63)).expect("i8 shift maximum"),
    );
    assert!(canonical_goal_has_closed_prior_certificate(
        &goal,
        std::slice::from_ref(&lower),
        std::slice::from_ref(&upper),
    ));
    assert!(!canonical_goal_has_closed_prior_certificate(
        &goal,
        &[],
        std::slice::from_ref(&upper),
    ));
    assert!(!canonical_goal_has_closed_prior_certificate(
        &goal,
        std::slice::from_ref(&lower),
        &[Proposition::LessOrEqual(
            value(3, count_type),
            ScalarTerm::integer(count_type, IntegerValue::Signed(63)).expect("i8 shift maximum"),
        )],
    ));

    let narrow_count_type = IntegerType::new(IntegerSign::Unsigned, 5).expect("u5");
    assert!(canonical_goal_has_closed_prior_certificate(
        &CanonicalScalarGoal::ExactShiftCount {
            value_type,
            count_type: narrow_count_type,
            count: value(4, narrow_count_type),
        },
        &[],
        &[],
    ));
}

#[test]
fn exact_division_selects_canonical_certificate_only_for_complete_prior_facts() {
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let unsigned_right = value(2, unsigned);
    let unsigned_goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: unsigned,
        left: value(1, unsigned),
        right: unsigned_right.clone(),
    };
    assert!(exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[],
        &[Proposition::LessOrEqual(
            ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
            unsigned_right.clone(),
        )],
    ));
    assert!(exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[],
        &[Proposition::LessOrEqual(
            ScalarTerm::integer(unsigned, IntegerValue::Unsigned(2)).expect("stronger u8 floor"),
            unsigned_right.clone(),
        )],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[],
        &[Proposition::LessOrEqual(
            ScalarTerm::integer(unsigned, IntegerValue::Unsigned(0)).expect("weak u8 floor"),
            unsigned_right.clone(),
        )],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[],
        &[Proposition::LessOrEqual(
            ScalarTerm::integer(unsigned, IntegerValue::Unsigned(2)).expect("stronger u8 floor"),
            value(9, unsigned),
        )],
    ));
    assert!(exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[Proposition::Equal(
            unsigned_right.clone(),
            ScalarTerm::integer(unsigned, IntegerValue::Unsigned(5)).expect("u8 literal"),
        )],
        &[],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[Proposition::Equal(
            unsigned_right,
            ScalarTerm::integer(unsigned, IntegerValue::Unsigned(0)).expect("u8 literal"),
        )],
        &[],
    ));

    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    for (literal, expected) in [(-3, true), (1, true), (0, false), (-1, false)] {
        assert_eq!(
            exact_division_has_closed_prior_certificate(
                &CanonicalScalarGoal::ExactDivisionDefined {
                    integer_type: signed,
                    left: value(3, signed),
                    right: value(4, signed),
                },
                &[Proposition::Equal(
                    value(4, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(literal)).expect("i8 literal"),
                )],
                &[],
            ),
            expected,
            "signed literal {literal}",
        );
    }
    assert!(!exact_division_has_closed_prior_certificate(
        &CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(3, signed),
            right: value(4, signed),
        },
        &[],
        &[],
    ));

    let negative_one_goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(5, signed),
        right: value(6, signed),
    };
    let negative_one = Proposition::Equal(
        value(6, signed),
        ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 literal"),
    );
    assert!(exact_division_has_closed_prior_certificate(
        &negative_one_goal,
        &[],
        &[Proposition::LessOrEqual(
            ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            value(6, signed),
        )],
    ));
    assert!(exact_division_has_closed_prior_certificate(
        &negative_one_goal,
        &[],
        &[Proposition::LessOrEqual(
            ScalarTerm::integer(signed, IntegerValue::Signed(3))
                .expect("stronger i8 positive floor"),
            value(6, signed),
        )],
    ));
    assert!(exact_division_has_closed_prior_certificate(
        &negative_one_goal,
        &[Proposition::LessOrEqual(
            value(6, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(-2)).expect("i8 -2"),
        )],
        &[],
    ));
    assert!(exact_division_has_closed_prior_certificate(
        &negative_one_goal,
        &[Proposition::LessOrEqual(
            value(6, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(-3))
                .expect("stronger i8 negative ceiling"),
        )],
        &[],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &negative_one_goal,
        &[Proposition::LessOrEqual(
            value(6, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(-1))
                .expect("weak i8 negative ceiling"),
        )],
        &[],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &negative_one_goal,
        &[],
        &[Proposition::LessOrEqual(
            ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
            value(6, signed),
        )],
    ));
    assert!(exact_division_has_closed_prior_certificate(
        &negative_one_goal,
        &[
            negative_one.clone(),
            Proposition::Equal(
                value(5, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-7)).expect("i8 literal"),
            ),
        ],
        &[],
    ));
    assert!(exact_division_has_closed_prior_certificate(
        &negative_one_goal,
        std::slice::from_ref(&Proposition::Equal(
            value(6, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
        )),
        &[Proposition::LessOrEqual(
            ScalarTerm::integer(signed, IntegerValue::Signed(-120))
                .expect("stronger i8 lower bound"),
            value(5, signed),
        )],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &negative_one_goal,
        std::slice::from_ref(&negative_one),
        &[],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &negative_one_goal,
        &[
            negative_one,
            Proposition::Equal(
                value(5, signed),
                ScalarTerm::integer(signed, signed.minimum_value()).expect("i8 minimum"),
            ),
        ],
        &[],
    ));

    let exact_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(signed, IntegerValue::Signed(-127)).expect("i8 minimum + 1"),
        value(5, signed),
    );
    assert!(exact_division_has_closed_prior_certificate(
        &negative_one_goal,
        std::slice::from_ref(&Proposition::Equal(
            value(6, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
        )),
        std::slice::from_ref(&exact_bound),
    ));
    assert!(exact_division_has_closed_prior_certificate(
        &negative_one_goal,
        &[
            Proposition::Equal(
                value(6, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
            ),
            exact_bound,
        ],
        &[],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &negative_one_goal,
        std::slice::from_ref(&Proposition::Equal(
            value(6, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
        )),
        &[Proposition::LessOrEqual(
            ScalarTerm::integer(signed, signed.minimum_value()).expect("i8 minimum"),
            value(5, signed),
        )],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &negative_one_goal,
        std::slice::from_ref(&Proposition::Equal(
            value(6, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
        )),
        &[Proposition::LessOrEqual(
            ScalarTerm::integer(signed, IntegerValue::Signed(-127)).expect("i8 minimum + 1"),
            value(9, signed),
        )],
    ));

    let i1 = IntegerType::new(IntegerSign::Signed, 1).expect("i1");
    let i1_goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: i1,
        left: value(7, i1),
        right: value(8, i1),
    };
    assert!(!exact_division_has_closed_prior_certificate(
        &i1_goal,
        &[],
        &[Proposition::LessOrEqual(
            value(8, i1),
            ScalarTerm::integer(i1, IntegerValue::Signed(-1)).expect("i1 -1"),
        )],
    ));
    let i1_divisor_bound = Proposition::LessOrEqual(
        value(8, i1),
        ScalarTerm::integer(i1, IntegerValue::Signed(-1)).expect("i1 -1"),
    );
    let i1_dividend_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(i1, IntegerValue::Signed(0)).expect("i1 zero"),
        value(7, i1),
    );
    assert!(exact_division_has_closed_prior_certificate(
        &i1_goal,
        &[],
        &[i1_divisor_bound.clone(), i1_dividend_bound.clone()],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &i1_goal,
        &[],
        &[
            i1_divisor_bound,
            Proposition::LessOrEqual(
                ScalarTerm::integer(i1, IntegerValue::Signed(0)).expect("i1 zero"),
                value(9, i1),
            ),
        ],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &i1_goal,
        &[],
        &[
            Proposition::LessOrEqual(
                value(9, i1),
                ScalarTerm::integer(i1, IntegerValue::Signed(-1)).expect("i1 -1"),
            ),
            i1_dividend_bound,
        ],
    ));
    assert!(exact_division_has_closed_prior_certificate(
        &i1_goal,
        &[
            Proposition::Equal(
                value(8, i1),
                ScalarTerm::integer(i1, IntegerValue::Signed(-1)).expect("i1 -1"),
            ),
            Proposition::Equal(
                value(7, i1),
                ScalarTerm::integer(i1, IntegerValue::Signed(0)).expect("i1 zero"),
            ),
        ],
        &[],
    ));
}

#[test]
fn exact_division_selects_two_exact_transitive_safe_divisor_facts() {
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let unsigned_goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: unsigned,
        left: value(1, unsigned),
        right: value(2, unsigned),
    };
    let unsigned_head = Proposition::LessOrEqual(
        ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
        value(3, unsigned),
    );
    let unsigned_tail = Proposition::LessOrEqual(value(3, unsigned), value(2, unsigned));
    assert!(exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[],
        &[unsigned_head.clone(), unsigned_tail.clone()],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[],
        std::slice::from_ref(&unsigned_head),
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[],
        &[
            unsigned_head,
            Proposition::LessOrEqual(value(3, unsigned), value(4, unsigned)),
        ],
    ));

    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    assert!(exact_division_has_closed_prior_certificate(
        &signed_goal,
        &[],
        &[
            Proposition::LessOrEqual(
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
                value(3, signed),
            ),
            Proposition::LessOrEqual(value(3, signed), value(2, signed)),
        ],
    ));
    assert!(exact_division_has_closed_prior_certificate(
        &signed_goal,
        &[],
        &[
            Proposition::LessOrEqual(value(2, signed), value(3, signed)),
            Proposition::LessOrEqual(
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-2)).expect("i8 -2"),
            ),
        ],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &signed_goal,
        &[],
        &[
            Proposition::LessOrEqual(value(2, signed), value(3, signed)),
            Proposition::LessOrEqual(
                value(4, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-2)).expect("i8 -2"),
            ),
        ],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &signed_goal,
        &[],
        &[
            Proposition::LessOrEqual(
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            ),
            Proposition::LessOrEqual(value(2, signed), value(3, signed)),
        ],
    ));
}

#[test]
fn exact_division_selects_complete_signed_joint_prior_bounds() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    let divisor_bound = Proposition::LessOrEqual(
        value(2, signed),
        ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
    );
    let dividend_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(signed, IntegerValue::Signed(-127)).expect("i8 minimum + 1"),
        value(1, signed),
    );
    assert!(exact_division_has_closed_prior_certificate(
        &goal,
        &[],
        &[divisor_bound.clone(), dividend_bound.clone()],
    ));
    assert!(exact_division_has_closed_prior_certificate(
        &goal,
        &[Proposition::Equal(
            value(1, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(-7)).expect("nonminimum i8 dividend"),
        )],
        std::slice::from_ref(&divisor_bound),
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &goal,
        &[Proposition::Equal(
            value(1, signed),
            ScalarTerm::integer(signed, signed.minimum_value()).expect("minimum i8 dividend"),
        )],
        std::slice::from_ref(&divisor_bound),
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &goal,
        &[Proposition::Equal(
            value(3, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(-7))
                .expect("wrong nonminimum i8 dividend"),
        )],
        std::slice::from_ref(&divisor_bound),
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &goal,
        &[],
        std::slice::from_ref(&divisor_bound),
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &goal,
        &[],
        std::slice::from_ref(&dividend_bound),
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &goal,
        &[],
        &[
            divisor_bound.clone(),
            Proposition::LessOrEqual(
                ScalarTerm::integer(signed, IntegerValue::Signed(-127)).expect("i8 minimum + 1"),
                value(3, signed),
            ),
        ],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &goal,
        &[],
        &[
            Proposition::LessOrEqual(
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
            ),
            dividend_bound,
        ],
    ));
}

#[test]
fn exact_division_selects_exact_retained_canonical_goal_or_arm() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    let canonical = goal
        .kernel_proposition()
        .expect("exact goal projects")
        .expect("exact goal has a kernel proposition");
    assert!(exact_division_has_closed_prior_certificate(
        &goal,
        &[],
        std::slice::from_ref(&canonical),
    ));
    assert!(exact_division_has_closed_prior_certificate(
        &goal,
        std::slice::from_ref(&canonical),
        &[],
    ));
    let Proposition::Disjunction(disjuncts) = &canonical else {
        panic!("signed exact goal is an ordered disjunction")
    };
    let joint_arm = disjuncts[2].clone();
    assert!(exact_division_has_closed_prior_certificate(
        &goal,
        &[],
        std::slice::from_ref(&joint_arm),
    ));
    let Proposition::Conjunction(joint_conjuncts) = joint_arm else {
        panic!("signed exceptional arm is a conjunction")
    };
    assert!(!exact_division_has_closed_prior_certificate(
        &goal,
        &[],
        &[Proposition::Conjunction(vec![
            joint_conjuncts[1].clone(),
            joint_conjuncts[0].clone(),
        ])],
    ));
    let redirected = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(3, signed),
    }
    .kernel_proposition()
    .expect("redirected exact goal projects")
    .expect("redirected exact goal has a kernel proposition");
    assert!(!exact_division_has_closed_prior_certificate(
        &goal,
        &[],
        &[redirected],
    ));
}

#[test]
fn exact_division_selects_literal_equalities_from_requirements() {
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let unsigned_goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: unsigned,
        left: value(1, unsigned),
        right: value(2, unsigned),
    };
    assert!(exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[],
        &[Proposition::Equal(
            value(2, unsigned),
            ScalarTerm::integer(unsigned, IntegerValue::Unsigned(5)).expect("safe u8 divisor"),
        )],
    ));
    assert!(exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[],
        &[
            Proposition::Equal(
                value(2, unsigned),
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(0))
                    .expect("stale zero u8 divisor"),
            ),
            Proposition::Equal(
                value(2, unsigned),
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(5)).expect("safe u8 divisor"),
            ),
        ],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[],
        &[Proposition::Equal(
            value(2, unsigned),
            ScalarTerm::integer(unsigned, IntegerValue::Unsigned(0)).expect("zero u8 divisor"),
        )],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[],
        &[Proposition::Equal(
            value(3, unsigned),
            ScalarTerm::integer(unsigned, IntegerValue::Unsigned(5))
                .expect("redirected u8 divisor"),
        )],
    ));

    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    let divisor_bound = Proposition::LessOrEqual(
        value(2, signed),
        ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
    );
    assert!(exact_division_has_closed_prior_certificate(
        &signed_goal,
        &[],
        &[
            divisor_bound.clone(),
            Proposition::Equal(
                value(1, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-7)).expect("safe i8 dividend"),
            ),
        ],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &signed_goal,
        &[],
        &[
            divisor_bound,
            Proposition::Equal(
                value(1, signed),
                ScalarTerm::integer(signed, signed.minimum_value()).expect("minimum i8 dividend"),
            ),
        ],
    ));
}

#[test]
fn exact_division_selects_exact_endpoint_equality_transport() {
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let unsigned_goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: unsigned,
        left: value(1, unsigned),
        right: value(2, unsigned),
    };
    let intermediate_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
        value(3, unsigned),
    );
    let divisor_equality = Proposition::Equal(value(3, unsigned), value(2, unsigned));
    assert!(exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[],
        &[intermediate_bound.clone(), divisor_equality.clone()],
    ));
    assert!(exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[],
        &[
            intermediate_bound.clone(),
            Proposition::Equal(value(2, unsigned), value(3, unsigned)),
        ],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[],
        std::slice::from_ref(&divisor_equality),
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[],
        &[
            intermediate_bound,
            Proposition::Equal(value(3, unsigned), value(1, unsigned)),
        ],
    ));

    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    assert!(exact_division_has_closed_prior_certificate(
        &signed_goal,
        &[],
        &[
            Proposition::LessOrEqual(
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-2)).expect("i8 -2"),
            ),
            Proposition::Equal(value(3, signed), value(2, signed)),
        ],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &signed_goal,
        &[],
        &[
            Proposition::LessOrEqual(
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
            ),
            Proposition::Equal(value(3, signed), value(2, signed)),
        ],
    ));
    let signed_divisor_bound = Proposition::LessOrEqual(
        value(2, signed),
        ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
    );
    let intermediate_dividend_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(signed, IntegerValue::Signed(-127)).expect("i8 minimum + 1"),
        value(3, signed),
    );
    assert!(exact_division_has_closed_prior_certificate(
        &signed_goal,
        &[],
        &[
            signed_divisor_bound.clone(),
            intermediate_dividend_bound.clone(),
            Proposition::Equal(value(3, signed), value(1, signed)),
        ],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &signed_goal,
        &[],
        &[
            intermediate_dividend_bound.clone(),
            Proposition::Equal(value(3, signed), value(1, signed)),
        ],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &signed_goal,
        &[],
        &[
            signed_divisor_bound,
            intermediate_dividend_bound,
            Proposition::Equal(value(3, signed), value(2, signed)),
        ],
    ));
}

#[test]
fn i1_exact_division_selects_transport_for_both_joint_endpoints() {
    let i1 = IntegerType::new(IntegerSign::Signed, 1).expect("i1");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: i1,
        left: value(1, i1),
        right: value(2, i1),
    };
    let divisor_bound = Proposition::LessOrEqual(
        value(3, i1),
        ScalarTerm::integer(i1, IntegerValue::Signed(-1)).expect("i1 -1"),
    );
    let divisor_equality = Proposition::Equal(value(3, i1), value(2, i1));
    let dividend_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(i1, IntegerValue::Signed(0)).expect("i1 zero"),
        value(4, i1),
    );
    let dividend_equality = Proposition::Equal(value(4, i1), value(1, i1));
    assert!(exact_division_has_closed_prior_certificate(
        &goal,
        &[],
        &[
            divisor_bound.clone(),
            divisor_equality.clone(),
            dividend_bound.clone(),
            dividend_equality.clone(),
        ],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &goal,
        &[],
        &[
            divisor_bound.clone(),
            divisor_equality.clone(),
            dividend_bound.clone(),
        ],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &goal,
        &[],
        &[
            divisor_bound,
            Proposition::Equal(value(3, i1), value(1, i1)),
            dividend_bound,
            Proposition::Equal(value(4, i1), value(2, i1)),
        ],
    ));
}

#[test]
fn exact_division_selects_closed_transitivity_under_endpoint_transport() {
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let unsigned_goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: unsigned,
        left: value(1, unsigned),
        right: value(2, unsigned),
    };
    assert!(exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[],
        &[
            Proposition::LessOrEqual(
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(2))
                    .expect("stronger u8 floor"),
                value(3, unsigned),
            ),
            Proposition::Equal(value(3, unsigned), value(2, unsigned)),
        ],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[],
        &[
            Proposition::LessOrEqual(
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(0)).expect("weak u8 floor"),
                value(3, unsigned),
            ),
            Proposition::Equal(value(3, unsigned), value(2, unsigned)),
        ],
    ));

    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    assert!(exact_division_has_closed_prior_certificate(
        &signed_goal,
        &[],
        &[
            Proposition::LessOrEqual(
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-3)).expect("stronger i8 ceiling"),
            ),
            Proposition::Equal(value(3, signed), value(2, signed)),
        ],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &signed_goal,
        &[],
        &[
            Proposition::LessOrEqual(
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("weak i8 ceiling"),
            ),
            Proposition::Equal(value(3, signed), value(2, signed)),
        ],
    ));
}

#[test]
fn exact_division_selects_two_citation_transitivity_under_endpoint_transport() {
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let unsigned_goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: unsigned,
        left: value(1, unsigned),
        right: value(2, unsigned),
    };
    assert!(exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[],
        &[
            Proposition::LessOrEqual(
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
                value(4, unsigned),
            ),
            Proposition::LessOrEqual(value(4, unsigned), value(3, unsigned)),
            Proposition::Equal(value(3, unsigned), value(2, unsigned)),
        ],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &unsigned_goal,
        &[],
        &[
            Proposition::LessOrEqual(
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
                value(4, unsigned),
            ),
            Proposition::LessOrEqual(value(1, unsigned), value(3, unsigned)),
            Proposition::Equal(value(3, unsigned), value(2, unsigned)),
        ],
    ));

    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    assert!(exact_division_has_closed_prior_certificate(
        &signed_goal,
        &[],
        &[
            Proposition::LessOrEqual(value(3, signed), value(4, signed)),
            Proposition::LessOrEqual(
                value(4, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-2)).expect("i8 -2"),
            ),
            Proposition::Equal(value(3, signed), value(2, signed)),
        ],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &signed_goal,
        &[],
        &[
            Proposition::LessOrEqual(value(3, signed), value(4, signed)),
            Proposition::LessOrEqual(
                value(4, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
            ),
            Proposition::Equal(value(3, signed), value(2, signed)),
        ],
    ));
}

#[test]
fn exact_division_selects_two_citation_dividend_floor_under_endpoint_transport() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    let divisor_bound = Proposition::LessOrEqual(
        value(2, signed),
        ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
    );
    let dividend_floor = Proposition::LessOrEqual(
        ScalarTerm::integer(signed, IntegerValue::Signed(-127)).expect("i8 minimum + 1"),
        value(4, signed),
    );
    let middle_bound = Proposition::LessOrEqual(value(4, signed), value(3, signed));
    let dividend_equality = Proposition::Equal(value(3, signed), value(1, signed));
    assert!(exact_division_has_closed_prior_certificate(
        &goal,
        std::slice::from_ref(&divisor_bound),
        &[
            dividend_floor.clone(),
            middle_bound.clone(),
            dividend_equality.clone(),
        ],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &goal,
        std::slice::from_ref(&divisor_bound),
        &[dividend_floor.clone(), dividend_equality.clone()],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &goal,
        &[divisor_bound],
        &[
            dividend_floor,
            Proposition::LessOrEqual(value(4, signed), value(2, signed)),
            dividend_equality,
        ],
    ));
}

#[test]
fn i1_exact_division_selects_two_citation_transport_for_both_endpoints() {
    let i1 = IntegerType::new(IntegerSign::Signed, 1).expect("i1");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: i1,
        left: value(1, i1),
        right: value(2, i1),
    };
    let facts = [
        Proposition::LessOrEqual(value(3, i1), value(4, i1)),
        Proposition::LessOrEqual(
            value(4, i1),
            ScalarTerm::integer(i1, IntegerValue::Signed(-1)).expect("i1 -1"),
        ),
        Proposition::Equal(value(3, i1), value(2, i1)),
        Proposition::LessOrEqual(
            ScalarTerm::integer(i1, IntegerValue::Signed(0)).expect("i1 zero"),
            value(6, i1),
        ),
        Proposition::LessOrEqual(value(6, i1), value(5, i1)),
        Proposition::Equal(value(5, i1), value(1, i1)),
    ];
    assert!(exact_division_has_closed_prior_certificate(
        &goal,
        &facts[..3],
        &facts[3..],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &goal,
        &facts[..3],
        &[facts[3].clone(), facts[5].clone()],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &goal,
        &[facts[0].clone(), facts[2].clone()],
        &facts[3..],
    ));
}

#[test]
fn exact_division_selects_two_citation_bounds_for_both_signed_joint_conjuncts() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    let facts = [
        Proposition::LessOrEqual(value(2, signed), value(3, signed)),
        Proposition::LessOrEqual(
            value(3, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
        ),
        Proposition::LessOrEqual(
            ScalarTerm::integer(signed, IntegerValue::Signed(-127)).expect("i8 minimum + 1"),
            value(4, signed),
        ),
        Proposition::LessOrEqual(value(4, signed), value(1, signed)),
    ];
    assert!(exact_division_has_closed_prior_certificate(
        &goal,
        &facts[..2],
        &facts[2..],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &goal,
        std::slice::from_ref(&facts[0]),
        &facts[2..],
    ));
    assert!(!exact_division_has_closed_prior_certificate(
        &goal,
        &facts[..2],
        std::slice::from_ref(&facts[2]),
    ));
}

#[test]
fn exact_division_selects_single_definition_affine_safe_divisor() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=4).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("four i8 values");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    let root_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
        value(3, signed),
    );
    let definition = Proposition::Equal(
        ScalarTerm::exact_integer_add(
            signed,
            value(3, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
        )
        .expect("exact add"),
        value(2, signed),
    );
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&definition),
        std::slice::from_ref(&root_bound),
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&definition),
        &[],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[Proposition::Equal(
            value(4, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            )
            .expect("redirected exact add"),
        )],
        &[root_bound],
    ));
}

#[test]
fn exact_division_selects_uniquely_landed_affine_sibling() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=5).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("five i8 values");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    let root_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
        value(3, signed),
    );
    let landing = Proposition::Equal(
        value(4, signed),
        ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
    );
    let definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_add(signed, value(3, signed), value(4, signed))
            .expect("exact add"),
    );

    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        &[landing.clone(), definition.clone()],
        std::slice::from_ref(&root_bound),
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&definition),
        std::slice::from_ref(&root_bound),
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[landing.clone(), landing, definition.clone()],
        std::slice::from_ref(&root_bound),
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[
            Proposition::Equal(
                value(5, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            ),
            definition,
        ],
        std::slice::from_ref(&root_bound),
    ));
}

#[test]
fn exact_division_reconstructs_landed_affine_source_through_partial_cast() {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(i8_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(i8_type)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(i16_type)),
        (ValueId::new(4).unwrap(), ScalarType::Integer(i16_type)),
        (ValueId::new(5).unwrap(), ScalarType::Integer(i16_type)),
    ])
    .expect("mixed cast context");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: i8_type,
        left: value(1, i8_type),
        right: value(2, i8_type),
    };
    let root_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(i16_type, IntegerValue::Signed(0)).expect("i16 zero"),
        value(3, i16_type),
    );
    let landing = Proposition::Equal(
        value(4, i16_type),
        ScalarTerm::integer(i16_type, IntegerValue::Signed(1)).expect("i16 one"),
    );
    let affine = Proposition::Equal(
        value(5, i16_type),
        ScalarTerm::exact_integer_add(i16_type, value(3, i16_type), value(4, i16_type))
            .expect("exact add"),
    );
    let cast = Proposition::Equal(
        value(2, i8_type),
        ScalarTerm::integer_exact_cast(i16_type, i8_type, value(5, i16_type)).expect("exact cast"),
    );

    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        &[landing.clone(), affine.clone(), cast.clone()],
        std::slice::from_ref(&root_bound),
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[affine.clone(), cast.clone()],
        std::slice::from_ref(&root_bound),
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[cast, landing, affine],
        std::slice::from_ref(&root_bound),
    ));
}

#[test]
fn exact_division_reconstructs_direct_partial_cast_into_landed_affine_suffix() {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(i8_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(i8_type)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(i16_type)),
        (ValueId::new(4).unwrap(), ScalarType::Integer(i8_type)),
        (ValueId::new(5).unwrap(), ScalarType::Integer(i8_type)),
    ])
    .expect("mixed post-cast context");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: i8_type,
        left: value(1, i8_type),
        right: value(2, i8_type),
    };
    let root_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(i16_type, IntegerValue::Signed(0)).expect("i16 zero"),
        value(3, i16_type),
    );
    let cast = Proposition::Equal(
        value(4, i8_type),
        ScalarTerm::integer_exact_cast(i16_type, i8_type, value(3, i16_type)).expect("exact cast"),
    );
    let landing = Proposition::Equal(
        value(5, i8_type),
        ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("i8 one"),
    );
    let affine = Proposition::Equal(
        value(2, i8_type),
        ScalarTerm::exact_integer_add(i8_type, value(4, i8_type), value(5, i8_type))
            .expect("exact add"),
    );

    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        &[cast.clone(), landing.clone(), affine.clone()],
        std::slice::from_ref(&root_bound),
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[cast.clone(), landing.clone(), affine.clone()],
        &[],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[landing, affine, cast],
        std::slice::from_ref(&root_bound),
    ));
}

#[test]
fn exact_division_selects_stronger_affine_endpoint_bounds() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=3).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("three i8 values");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    let zero = ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero");
    let positive_root_bound = Proposition::LessOrEqual(zero.clone(), value(3, signed));
    let positive_definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_add(
            signed,
            value(3, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(2)).expect("i8 two"),
        )
        .expect("exact add"),
    );
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&positive_definition),
        std::slice::from_ref(&positive_root_bound),
    ));

    let negative_root_bound = Proposition::LessOrEqual(value(3, signed), zero);
    let negative_definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_subtract(
            signed,
            value(3, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(3)).expect("i8 three"),
        )
        .expect("exact subtract"),
    );
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&negative_definition),
        std::slice::from_ref(&negative_root_bound),
    ));

    let weak_definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_add(
            signed,
            value(3, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
        )
        .expect("exact add zero"),
    );
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[weak_definition],
        &[positive_root_bound],
    ));
}

#[test]
fn exact_division_selects_landed_literal_affine_root() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=4).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("four i8 values");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    let zero = ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero");
    let landed_root = Proposition::Equal(value(3, signed), zero.clone());
    let positive_definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_add(
            signed,
            value(3, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
        )
        .expect("exact add"),
    );
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&positive_definition),
        std::slice::from_ref(&landed_root),
    ));

    let negative_definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_subtract(
            signed,
            value(3, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(2)).expect("i8 two"),
        )
        .expect("exact subtract"),
    );
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&negative_definition),
        std::slice::from_ref(&landed_root),
    ));

    let unsafe_definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_add(signed, value(3, signed), zero).expect("exact add zero"),
    );
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&unsafe_definition),
        std::slice::from_ref(&landed_root),
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[positive_definition],
        &[Proposition::Equal(
            value(4, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
        )],
    ));
}

#[test]
fn exact_division_selects_checked_contiguous_cast_root_bound() {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let context = PropositionContext::from_value_types([
        (
            ValueId::new(1).expect("dividend"),
            ScalarType::Integer(i8_type),
        ),
        (
            ValueId::new(2).expect("divisor"),
            ScalarType::Integer(i8_type),
        ),
        (
            ValueId::new(3).expect("root"),
            ScalarType::Integer(i16_type),
        ),
        (
            ValueId::new(4).expect("middle"),
            ScalarType::Integer(i16_type),
        ),
        (
            ValueId::new(5).expect("wide root"),
            ScalarType::Integer(i32_type),
        ),
        (
            ValueId::new(6).expect("redirected root"),
            ScalarType::Integer(i32_type),
        ),
        (
            ValueId::new(7).expect("redirected bound"),
            ScalarType::Integer(i32_type),
        ),
        (
            ValueId::new(8).expect("third alias"),
            ScalarType::Integer(i32_type),
        ),
    ])
    .expect("cast values");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: i8_type,
        left: value(1, i8_type),
        right: value(2, i8_type),
    };
    let cast = Proposition::Equal(
        value(2, i8_type),
        ScalarTerm::integer_exact_cast(i16_type, i8_type, value(3, i16_type))
            .expect("partial exact cast"),
    );
    let positive_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(i16_type, IntegerValue::Signed(1)).expect("i16 one"),
        value(3, i16_type),
    );
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&cast),
        std::slice::from_ref(&positive_bound),
    ));
    let negative_bound = Proposition::LessOrEqual(
        value(3, i16_type),
        ScalarTerm::integer(i16_type, IntegerValue::Signed(-2)).expect("i16 -2"),
    );
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&cast),
        std::slice::from_ref(&negative_bound),
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&cast),
        &[],
    ));
    let wide_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(i32_type, IntegerValue::Signed(1)).expect("i32 one"),
        value(5, i32_type),
    );
    let first_cast = Proposition::Equal(
        value(4, i16_type),
        ScalarTerm::integer_exact_cast(i32_type, i16_type, value(5, i32_type))
            .expect("first partial cast"),
    );
    let second_cast = Proposition::Equal(
        value(2, i8_type),
        ScalarTerm::integer_exact_cast(i16_type, i8_type, value(4, i16_type))
            .expect("second partial cast"),
    );
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast.clone(), second_cast.clone()],
        std::slice::from_ref(&wide_bound),
    ));
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast.clone(), second_cast.clone()],
        &[Proposition::Equal(
            value(5, i32_type),
            ScalarTerm::integer(i32_type, IntegerValue::Signed(1)).expect("i32 one"),
        )],
    ));
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast.clone(), second_cast.clone()],
        &[Proposition::Equal(
            value(5, i32_type),
            ScalarTerm::integer(i32_type, IntegerValue::Signed(-2)).expect("i32 -2"),
        )],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast.clone(), second_cast.clone()],
        &[Proposition::Equal(
            value(6, i32_type),
            ScalarTerm::integer(i32_type, IntegerValue::Signed(1)).expect("i32 one"),
        )],
    ));
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast.clone(), second_cast.clone()],
        &[Proposition::Equal(
            value(5, i32_type),
            ScalarTerm::integer(i32_type, IntegerValue::Signed(2)).expect("i32 two"),
        )],
    ));
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast.clone(), second_cast.clone()],
        &[Proposition::Equal(
            value(5, i32_type),
            ScalarTerm::integer(i32_type, IntegerValue::Signed(-3)).expect("i32 -3"),
        )],
    ));
    for weak in [0, -1] {
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[Proposition::Equal(
                value(5, i32_type),
                ScalarTerm::integer(i32_type, IntegerValue::Signed(weak))
                    .expect("weak i32 literal"),
            )],
        ));
    }
    let root_alias = Proposition::Equal(value(5, i32_type), value(6, i32_type));
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast.clone(), second_cast.clone()],
        &[
            Proposition::LessOrEqual(
                ScalarTerm::integer(i32_type, IntegerValue::Signed(1)).expect("i32 one"),
                value(6, i32_type),
            ),
            root_alias.clone(),
        ],
    ));
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast.clone(), second_cast.clone()],
        &[
            Proposition::LessOrEqual(
                ScalarTerm::integer(i32_type, IntegerValue::Signed(2)).expect("i32 two"),
                value(6, i32_type),
            ),
            root_alias.clone(),
        ],
    ));
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast.clone(), second_cast.clone()],
        &[
            Proposition::LessOrEqual(
                value(6, i32_type),
                ScalarTerm::integer(i32_type, IntegerValue::Signed(-3)).expect("i32 -3"),
            ),
            root_alias.clone(),
        ],
    ));
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast.clone(), second_cast.clone()],
        &[
            Proposition::LessOrEqual(
                value(6, i32_type),
                ScalarTerm::integer(i32_type, IntegerValue::Signed(-2)).expect("i32 -2"),
            ),
            root_alias.clone(),
        ],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast.clone(), second_cast.clone()],
        &[Proposition::LessOrEqual(
            ScalarTerm::integer(i32_type, IntegerValue::Signed(1)).expect("i32 one"),
            value(6, i32_type),
        )],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast.clone(), second_cast.clone()],
        &[
            Proposition::LessOrEqual(
                ScalarTerm::integer(i32_type, IntegerValue::Signed(1)).expect("i32 one"),
                value(7, i32_type),
            ),
            root_alias.clone(),
        ],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast.clone(), second_cast.clone()],
        &[
            Proposition::LessOrEqual(
                value(6, i32_type),
                ScalarTerm::integer(i32_type, IntegerValue::Signed(-1)).expect("i32 -1"),
            ),
            root_alias.clone(),
        ],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast.clone(), second_cast.clone()],
        &[
            Proposition::LessOrEqual(
                ScalarTerm::integer(i32_type, IntegerValue::Signed(0)).expect("i32 zero"),
                value(6, i32_type),
            ),
            root_alias.clone(),
        ],
    ));
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast.clone(), second_cast.clone()],
        &[
            root_alias.clone(),
            Proposition::Equal(
                value(6, i32_type),
                ScalarTerm::integer(i32_type, IntegerValue::Signed(2)).expect("i32 two"),
            ),
        ],
    ));
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast.clone(), second_cast.clone()],
        &[
            root_alias.clone(),
            Proposition::Equal(
                value(6, i32_type),
                ScalarTerm::integer(i32_type, IntegerValue::Signed(-3)).expect("i32 -3"),
            ),
        ],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast.clone(), second_cast.clone()],
        &[Proposition::Equal(
            value(6, i32_type),
            ScalarTerm::integer(i32_type, IntegerValue::Signed(2)).expect("i32 two"),
        )],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast.clone(), second_cast.clone()],
        &[
            root_alias.clone(),
            Proposition::Equal(
                value(7, i32_type),
                ScalarTerm::integer(i32_type, IntegerValue::Signed(2)).expect("i32 two"),
            ),
        ],
    ));
    for weak in [0, -1] {
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[
                root_alias.clone(),
                Proposition::Equal(
                    value(6, i32_type),
                    ScalarTerm::integer(i32_type, IntegerValue::Signed(weak))
                        .expect("weak i32 literal"),
                ),
            ],
        ));
    }
    let middle_alias = Proposition::Equal(value(6, i32_type), value(7, i32_type));
    let two_alias_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(i32_type, IntegerValue::Signed(1)).expect("i32 one"),
        value(7, i32_type),
    );
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast.clone(), second_cast.clone()],
        &[
            two_alias_bound.clone(),
            middle_alias.clone(),
            root_alias.clone(),
        ],
    ));
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast.clone(), second_cast.clone()],
        &[
            Proposition::LessOrEqual(
                value(7, i32_type),
                ScalarTerm::integer(i32_type, IntegerValue::Signed(-2)).expect("i32 -2"),
            ),
            middle_alias.clone(),
            root_alias.clone(),
        ],
    ));
    for rejected in [
        vec![two_alias_bound.clone(), root_alias.clone()],
        vec![
            two_alias_bound.clone(),
            Proposition::Equal(value(6, i32_type), value(8, i32_type)),
            root_alias.clone(),
        ],
        vec![
            Proposition::LessOrEqual(
                ScalarTerm::integer(i32_type, IntegerValue::Signed(0)).expect("i32 zero"),
                value(7, i32_type),
            ),
            middle_alias.clone(),
            root_alias.clone(),
        ],
        vec![
            Proposition::LessOrEqual(
                ScalarTerm::integer(i32_type, IntegerValue::Signed(1)).expect("i32 one"),
                value(8, i32_type),
            ),
            Proposition::Equal(value(7, i32_type), value(8, i32_type)),
            middle_alias,
            root_alias.clone(),
        ],
    ] {
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &rejected,
        ));
    }
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[second_cast.clone(), first_cast.clone()],
        std::slice::from_ref(&wide_bound),
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[first_cast, second_cast.clone(), second_cast],
        std::slice::from_ref(&wide_bound),
    ));
}

#[test]
fn exact_division_selects_affine_root_literal_through_one_alias() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=5).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("five i8 values");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    let root_alias = Proposition::Equal(value(3, signed), value(4, signed));
    let landed_alias = Proposition::Equal(
        value(4, signed),
        ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
    );
    let positive_definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_add(
            signed,
            value(3, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
        )
        .expect("exact add"),
    );
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&positive_definition),
        &[root_alias.clone(), landed_alias.clone()],
    ));
    let negative_definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_subtract(
            signed,
            value(3, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(2)).expect("i8 two"),
        )
        .expect("exact subtract"),
    );
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&negative_definition),
        &[root_alias.clone(), landed_alias.clone()],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&positive_definition),
        std::slice::from_ref(&landed_alias),
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&positive_definition),
        &[
            root_alias.clone(),
            Proposition::Equal(
                value(5, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
            ),
        ],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[positive_definition],
        &[
            root_alias,
            Proposition::Equal(value(4, signed), value(5, signed)),
            Proposition::Equal(
                value(5, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
            ),
        ],
    ));
}

#[test]
fn exact_division_selects_affine_bound_through_target_alias() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=5).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("five i8 values");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    let target_alias = Proposition::Equal(value(4, signed), value(2, signed));
    let zero = ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero");

    let positive_root_bound = Proposition::LessOrEqual(zero.clone(), value(3, signed));
    let positive_definition = Proposition::Equal(
        value(4, signed),
        ScalarTerm::exact_integer_add(
            signed,
            value(3, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
        )
        .expect("exact add"),
    );
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&positive_definition),
        &[positive_root_bound.clone(), target_alias.clone()],
    ));

    let negative_root_bound = Proposition::LessOrEqual(value(3, signed), zero);
    let negative_definition = Proposition::Equal(
        value(4, signed),
        ScalarTerm::exact_integer_subtract(
            signed,
            value(3, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(2)).expect("i8 two"),
        )
        .expect("exact subtract"),
    );
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&negative_definition),
        &[negative_root_bound, target_alias],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&positive_definition),
        std::slice::from_ref(&positive_root_bound),
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[positive_definition],
        &[
            positive_root_bound,
            Proposition::Equal(value(4, signed), value(5, signed)),
        ],
    ));
}

#[test]
fn exact_division_selects_affine_bound_through_two_target_aliases() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=6).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("six i8 values");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    let outer_alias = Proposition::Equal(value(2, signed), value(4, signed));
    let inner_alias = Proposition::Equal(value(4, signed), value(5, signed));
    let positive_root_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
        value(3, signed),
    );
    let positive_definition = Proposition::Equal(
        value(5, signed),
        ScalarTerm::exact_integer_add(
            signed,
            value(3, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
        )
        .expect("exact add"),
    );
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&positive_definition),
        &[
            positive_root_bound.clone(),
            outer_alias.clone(),
            inner_alias.clone(),
        ],
    ));
    let negative_root_bound = Proposition::LessOrEqual(
        value(3, signed),
        ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
    );
    let negative_definition = Proposition::Equal(
        value(5, signed),
        ScalarTerm::exact_integer_subtract(
            signed,
            value(3, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(2)).expect("i8 two"),
        )
        .expect("exact subtract"),
    );
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&negative_definition),
        &[
            negative_root_bound,
            outer_alias.clone(),
            inner_alias.clone(),
        ],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&positive_definition),
        &[positive_root_bound.clone(), outer_alias.clone()],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&positive_definition),
        &[
            positive_root_bound.clone(),
            outer_alias.clone(),
            Proposition::Equal(value(4, signed), value(6, signed)),
        ],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[Proposition::Equal(
            value(6, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            )
            .expect("exact add"),
        )],
        &[
            positive_root_bound,
            outer_alias,
            inner_alias,
            Proposition::Equal(value(5, signed), value(6, signed)),
        ],
    ));
}

#[test]
fn exact_division_selects_alias_substituted_affine_root_bound() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=5).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("five i8 values");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    let alias_equality = Proposition::Equal(value(3, signed), value(4, signed));
    let alias_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
        value(4, signed),
    );
    let definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_add(
            signed,
            value(3, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
        )
        .expect("exact add"),
    );
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&definition),
        &[alias_equality.clone(), alias_bound.clone()],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&definition),
        std::slice::from_ref(&alias_bound),
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&definition),
        std::slice::from_ref(&alias_equality),
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[definition],
        &[
            Proposition::Equal(value(5, signed), value(4, signed)),
            alias_bound,
        ],
    ));
}

#[test]
fn exact_division_selects_bound_through_two_affine_root_aliases() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=7).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("seven i8 values");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    let root_to_middle_alias = Proposition::Equal(value(3, signed), value(4, signed));
    let middle_to_bound_alias = Proposition::Equal(value(4, signed), value(5, signed));
    let lower_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
        value(5, signed),
    );
    let definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_add(
            signed,
            value(3, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
        )
        .expect("exact add"),
    );
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&definition),
        &[
            root_to_middle_alias.clone(),
            middle_to_bound_alias.clone(),
            lower_bound.clone(),
        ],
    ));
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&definition),
        &[
            root_to_middle_alias.clone(),
            middle_to_bound_alias.clone(),
            Proposition::LessOrEqual(
                value(5, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-3)).expect("i8 -3"),
            ),
        ],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&definition),
        &[root_to_middle_alias.clone(), lower_bound.clone()],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&definition),
        &[
            root_to_middle_alias.clone(),
            Proposition::Equal(value(4, signed), value(6, signed)),
            lower_bound,
        ],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[definition],
        &[
            root_to_middle_alias,
            middle_to_bound_alias,
            Proposition::Equal(value(5, signed), value(6, signed)),
            Proposition::LessOrEqual(
                ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
                value(6, signed),
            ),
        ],
    ));
}

#[test]
fn exact_division_selects_transitive_bound_on_affine_root_alias() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=6).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("six i8 values");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    let root_alias = Proposition::Equal(value(3, signed), value(4, signed));
    let lower_to_middle = Proposition::LessOrEqual(
        ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
        value(5, signed),
    );
    let middle_to_alias = Proposition::LessOrEqual(value(5, signed), value(4, signed));
    let definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_add(
            signed,
            value(3, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
        )
        .expect("exact add"),
    );
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&definition),
        &[
            root_alias.clone(),
            lower_to_middle.clone(),
            middle_to_alias.clone(),
        ],
    ));

    let alias_to_middle = Proposition::LessOrEqual(value(4, signed), value(5, signed));
    let middle_to_ceiling = Proposition::LessOrEqual(
        value(5, signed),
        ScalarTerm::integer(signed, IntegerValue::Signed(-3)).expect("i8 -3"),
    );
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&definition),
        &[root_alias.clone(), alias_to_middle, middle_to_ceiling,],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&definition),
        &[lower_to_middle.clone(), middle_to_alias.clone()],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&definition),
        &[
            root_alias.clone(),
            lower_to_middle.clone(),
            Proposition::LessOrEqual(value(6, signed), value(4, signed)),
        ],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[definition],
        &[
            Proposition::Equal(value(3, signed), value(6, signed)),
            lower_to_middle,
            middle_to_alias,
        ],
    ));
}

#[test]
fn exact_division_selects_transitively_reconstructed_affine_root_bound() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=5).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("five i8 values");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    let lower_to_middle = Proposition::LessOrEqual(
        ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
        value(4, signed),
    );
    let middle_to_root = Proposition::LessOrEqual(value(4, signed), value(3, signed));
    let definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_add(
            signed,
            value(3, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
        )
        .expect("exact add"),
    );
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&definition),
        &[lower_to_middle.clone(), middle_to_root.clone()],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&definition),
        std::slice::from_ref(&lower_to_middle),
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        std::slice::from_ref(&definition),
        std::slice::from_ref(&middle_to_root),
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[definition],
        &[
            lower_to_middle,
            Proposition::LessOrEqual(value(5, signed), value(3, signed)),
        ],
    ));
}

#[test]
fn exact_division_selects_two_definition_affine_safe_divisor() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=4).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("four i8 values");
    let goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    let root_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
        value(3, signed),
    );
    let definitions = [
        Proposition::Equal(
            value(4, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            )
            .expect("first exact add"),
        ),
        Proposition::Equal(
            value(2, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(4, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            )
            .expect("second exact add"),
        ),
    ];
    assert!(exact_division_has_prior_certificate(
        &context,
        &goal,
        &definitions,
        std::slice::from_ref(&root_bound),
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &definitions[..1],
        std::slice::from_ref(&root_bound),
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &goal,
        &[definitions[1].clone(), definitions[0].clone()],
        &[root_bound],
    ));
}

#[test]
fn exact_division_selects_three_through_five_definition_affine_safe_divisors() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=8).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("eight i8 values");
    let three_step_goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(6, signed),
    };
    let four_step_goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(7, signed),
    };
    let five_step_goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(8, signed),
    };
    let six_step_goal = CanonicalScalarGoal::ExactDivisionDefined {
        integer_type: signed,
        left: value(1, signed),
        right: value(2, signed),
    };
    let three_step_root_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(signed, IntegerValue::Signed(-2)).expect("i8 -2"),
        value(3, signed),
    );
    let four_step_root_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(signed, IntegerValue::Signed(-3)).expect("i8 -3"),
        value(3, signed),
    );
    let five_step_root_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(signed, IntegerValue::Signed(-4)).expect("i8 -4"),
        value(3, signed),
    );
    let six_step_root_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(signed, IntegerValue::Signed(-5)).expect("i8 -5"),
        value(3, signed),
    );
    let definitions = [
        Proposition::Equal(
            value(4, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            )
            .expect("first exact add"),
        ),
        Proposition::Equal(
            value(5, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(4, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            )
            .expect("second exact add"),
        ),
        Proposition::Equal(
            value(6, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(5, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            )
            .expect("third exact add"),
        ),
        Proposition::Equal(
            value(7, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(6, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            )
            .expect("fourth exact add"),
        ),
        Proposition::Equal(
            value(8, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(7, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            )
            .expect("fifth exact add"),
        ),
        Proposition::Equal(
            value(2, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(8, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            )
            .expect("sixth exact add"),
        ),
    ];
    assert!(exact_division_has_prior_certificate(
        &context,
        &three_step_goal,
        &definitions,
        std::slice::from_ref(&three_step_root_bound),
    ));
    assert!(exact_division_has_prior_certificate(
        &context,
        &four_step_goal,
        &definitions,
        std::slice::from_ref(&four_step_root_bound),
    ));
    assert!(exact_division_has_prior_certificate(
        &context,
        &five_step_goal,
        &definitions,
        std::slice::from_ref(&five_step_root_bound),
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &five_step_goal,
        &definitions[..4],
        std::slice::from_ref(&five_step_root_bound),
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &five_step_goal,
        &[
            definitions[4].clone(),
            definitions[3].clone(),
            definitions[2].clone(),
            definitions[1].clone(),
            definitions[0].clone(),
        ],
        &[five_step_root_bound],
    ));
    assert!(!exact_division_has_prior_certificate(
        &context,
        &six_step_goal,
        &definitions,
        &[six_step_root_bound],
    ));
}
