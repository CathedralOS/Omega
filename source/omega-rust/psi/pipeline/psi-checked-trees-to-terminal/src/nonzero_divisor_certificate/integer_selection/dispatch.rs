//! Canonical integer proposition-kind proof dispatch.

use psi_core::{
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarTerm, ScalarType,
};
use psi_proof_admission::{
    IntegerAffineWitness, PrimitiveJudgment, ProofNode, ProofRule, check_integer_affine_witness,
    lift_fixed_integer_relation, map_integer_affine_bound,
};

use super::super::affine_custody::DefinitionIndex;
use super::super::integer_evidence::cited_facts;
use super::bound;

pub(super) fn prove_atomic(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<Option<ProofNode>> {
    match goal {
        Proposition::Truth => Some(Some(ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        })),
        Proposition::LessOrEqual(_, _) => Some(bound::prove(
            context,
            goal,
            assumptions,
            semantic_axioms,
            definitions,
        )),
        Proposition::IntegerMathEqual(_, _)
        | Proposition::IntegerMathLessThan(_, _)
        | Proposition::IntegerMathLessOrEqual(_, _) => Some(prove_math_relation(
            context,
            goal,
            assumptions,
            semantic_axioms,
            definitions,
        )),
        _ => None,
    }
}

fn prove_math_relation(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    if let Some(proof) =
        prove_direct_shift_left_relation(context, goal, assumptions, semantic_axioms, definitions)
    {
        return Some(proof);
    }
    if math_relation_is_closed(goal) {
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
        });
    }
    let scalar_goal = scalar_relation_for_math_goal(goal)?;
    let scalar_proof = match &scalar_goal {
        Proposition::LessOrEqual(_, _) => bound::prove(
            context,
            &scalar_goal,
            assumptions,
            semantic_axioms,
            definitions,
        )?,
        _ => return None,
    };
    (lift_fixed_integer_relation(&scalar_goal).as_ref() == Some(goal)).then(|| ProofNode {
        conclusion: goal.clone(),
        rule: scalar_proof.rule,
    })
}

fn math_relation_is_closed(goal: &Proposition) -> bool {
    fn closed(term: &IntegerMathTerm) -> bool {
        match term {
            IntegerMathTerm::IntegerLiteral(_) => true,
            IntegerMathTerm::MathValue { .. } => false,
            IntegerMathTerm::Add(left, right)
            | IntegerMathTerm::Subtract(left, right)
            | IntegerMathTerm::Multiply(left, right) => closed(left) && closed(right),
            IntegerMathTerm::ShiftLeft { value, count } => closed(value) && closed(count),
        }
    }
    match goal {
        Proposition::IntegerMathEqual(left, right)
        | Proposition::IntegerMathLessThan(left, right)
        | Proposition::IntegerMathLessOrEqual(left, right) => closed(left) && closed(right),
        _ => false,
    }
}

fn prove_direct_shift_left_relation(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let Proposition::IntegerMathLessOrEqual(left, right) = goal else {
        return None;
    };
    let (shifted, carrier_bound, lower) = match (left, right) {
        (IntegerMathTerm::ShiftLeft { .. }, IntegerMathTerm::IntegerLiteral(bound)) => {
            (left, *bound, false)
        }
        (IntegerMathTerm::IntegerLiteral(bound), IntegerMathTerm::ShiftLeft { .. }) => {
            (right, *bound, true)
        }
        _ => return None,
    };
    let IntegerMathTerm::ShiftLeft { value, count } = shifted else {
        unreachable!("matched direct mathematical left shift")
    };
    let (value_type, value) = lower_math_leaf(value)?;
    let (count_type, count) = lower_math_leaf(count)?;
    if value_type.sign() == IntegerSign::Unsigned && lower {
        return None;
    }
    let target =
        ScalarTerm::exact_integer_shift_left(value_type, count_type, value.clone(), count.clone())
            .ok()?;

    let (maximum_count, mut auxiliary) = direct_shift_count_evidence(
        context,
        value_type,
        count_type,
        &count,
        assumptions,
        semantic_axioms,
        definitions,
    )?;
    let carrier_value = carrier_bound.as_integer_value(value_type)?;
    let preimage = shift_preimage(carrier_value, maximum_count)?;
    let literal = ScalarTerm::integer(value_type, preimage).ok()?;
    let root_goal = if lower {
        Proposition::LessOrEqual(literal, value.clone())
    } else {
        Proposition::LessOrEqual(value.clone(), literal)
    };
    let root_proof = bound::prove(
        context,
        &root_goal,
        assumptions,
        semantic_axioms,
        definitions,
    )
    .or_else(|| {
        super::shift::prove_recursive(
            context,
            &root_goal,
            assumptions,
            semantic_axioms,
            definitions,
        )
    })
    .or_else(|| super::shift::prove(context, &root_goal, assumptions, semantic_axioms));
    let root_proof = root_proof?;
    let evidence = if auxiliary.is_empty() {
        root_proof
    } else {
        let mut proofs = Vec::with_capacity(auxiliary.len() + 1);
        proofs.push(root_proof);
        proofs.append(&mut auxiliary);
        ProofNode {
            conclusion: Proposition::Conjunction(
                proofs
                    .iter()
                    .map(|proof| proof.conclusion.clone())
                    .collect(),
            ),
            rule: ProofRule::ConjunctionIntroduction(proofs),
        }
    };
    let witness = IntegerAffineWitness {
        root: value,
        target,
        definition_axioms: Vec::new(),
        literal_axioms: Vec::new(),
    };
    let form = check_integer_affine_witness(context, semantic_axioms, &witness).ok()?;
    let mapped = map_integer_affine_bound(&form, &evidence.conclusion).ok()?;
    let mapped_proof = ProofNode {
        conclusion: mapped.clone(),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(evidence),
            witness,
        },
    };
    if &mapped == goal {
        return Some(mapped_proof);
    }
    relax_math_bound(goal, mapped_proof)
}

fn lower_math_leaf(term: &IntegerMathTerm) -> Option<(IntegerType, ScalarTerm)> {
    match term {
        IntegerMathTerm::MathValue { source_type, value } => Some((
            *source_type,
            ScalarTerm::value(*value, ScalarType::Integer(*source_type)),
        )),
        IntegerMathTerm::IntegerLiteral(_) => None,
        _ => None,
    }
}

fn direct_shift_count_evidence(
    context: &PropositionContext,
    value_type: IntegerType,
    count_type: IntegerType,
    count: &ScalarTerm,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<(u32, Vec<ProofNode>)> {
    if let Some((actual, literal)) = count.integer_value() {
        let count = integer_value_as_u32((actual == count_type).then_some(literal)?)?;
        return (count < u32::from(value_type.bits())).then_some((count, Vec::new()));
    }

    if let Some((proof, literal)) =
        cited_facts(assumptions, semantic_axioms).find_map(|(citation, proposition)| {
            let Proposition::Equal(left, right) = proposition else {
                return None;
            };
            let literal = if left == count {
                right.integer_value()
            } else if right == count {
                left.integer_value()
            } else {
                None
            }?;
            (literal.0 == count_type).then(|| (citation.proof(proposition), literal.1))
        })
    {
        let count = integer_value_as_u32(literal)?;
        if count < u32::from(value_type.bits()) {
            return Some((count, vec![proof]));
        }
    }

    let mut proofs = Vec::with_capacity(2);
    if count_type.sign() == IntegerSign::Signed {
        let lower = Proposition::LessOrEqual(
            ScalarTerm::integer(count_type, IntegerValue::Signed(0)).ok()?,
            count.clone(),
        );
        proofs.push(bound::prove(
            context,
            &lower,
            assumptions,
            semantic_axioms,
            definitions,
        )?);
    }
    for maximum in 0..u32::from(value_type.bits()) {
        let value = match count_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(i128::from(maximum)),
            IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(maximum)),
        };
        let Ok(literal) = ScalarTerm::integer(count_type, value) else {
            continue;
        };
        let upper = Proposition::LessOrEqual(count.clone(), literal);
        if let Some(proof) =
            bound::prove(context, &upper, assumptions, semantic_axioms, definitions)
        {
            proofs.push(proof);
            return Some((maximum, proofs));
        }
    }
    let maximum = integer_value_as_u32(count_type.maximum_value())?;
    (maximum < u32::from(value_type.bits())).then_some((maximum, proofs))
}

fn integer_value_as_u32(value: IntegerValue) -> Option<u32> {
    match value {
        IntegerValue::Signed(value) => u32::try_from(value).ok(),
        IntegerValue::Unsigned(value) => u32::try_from(value).ok(),
    }
}

fn shift_preimage(value: IntegerValue, count: u32) -> Option<IntegerValue> {
    match value {
        IntegerValue::Signed(value) => Some(IntegerValue::Signed(value >> count)),
        IntegerValue::Unsigned(value) => Some(IntegerValue::Unsigned(value >> count)),
    }
}

fn relax_math_bound(goal: &Proposition, mapped: ProofNode) -> Option<ProofNode> {
    let (
        Proposition::IntegerMathLessOrEqual(goal_left, goal_right),
        Proposition::IntegerMathLessOrEqual(mapped_left, mapped_right),
    ) = (goal, &mapped.conclusion)
    else {
        return None;
    };
    if goal_left == mapped_left {
        let tail = ProofNode {
            conclusion: Proposition::IntegerMathLessOrEqual(
                mapped_right.clone(),
                goal_right.clone(),
            ),
            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
        };
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(mapped),
                middle_less_or_equal_right: Box::new(tail),
            },
        });
    }
    if goal_right == mapped_right {
        let head = ProofNode {
            conclusion: Proposition::IntegerMathLessOrEqual(goal_left.clone(), mapped_left.clone()),
            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
        };
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(head),
                middle_less_or_equal_right: Box::new(mapped),
            },
        });
    }
    None
}

fn scalar_relation_for_math_goal(goal: &Proposition) -> Option<Proposition> {
    let (kind, left, right) = match goal {
        Proposition::IntegerMathEqual(left, right) => (0, left, right),
        Proposition::IntegerMathLessThan(left, right) => (1, left, right),
        Proposition::IntegerMathLessOrEqual(left, right) => (2, left, right),
        _ => return None,
    };
    let (source_type, value) = match (left, right) {
        (IntegerMathTerm::MathValue { source_type, value }, _)
        | (_, IntegerMathTerm::MathValue { source_type, value }) => (*source_type, *value),
        _ => return None,
    };
    let convert = |term: &IntegerMathTerm| match term {
        IntegerMathTerm::MathValue {
            source_type: term_type,
            value: term_value,
        } if *term_type == source_type && *term_value == value => {
            Some(ScalarTerm::value(value, ScalarType::Integer(source_type)))
        }
        IntegerMathTerm::IntegerLiteral(literal) => {
            ScalarTerm::integer(source_type, literal.as_integer_value(source_type)?).ok()
        }
        _ => None,
    };
    let left = convert(left)?;
    let right = convert(right)?;
    Some(match kind {
        0 => Proposition::Equal(left, right),
        1 => Proposition::LessThan(left, right),
        _ => Proposition::LessOrEqual(left, right),
    })
}
