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
use super::{bound, multiply};

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
    if let Some(proof) = multiply::prove(context, goal, assumptions, semantic_axioms, definitions) {
        return Some(proof);
    }
    if let Some(proof) =
        prove_direct_subtract_relation(context, goal, assumptions, semantic_axioms, definitions)
    {
        return Some(proof);
    }
    if let Some(proof) =
        prove_direct_add_relation(context, goal, assumptions, semantic_axioms, definitions)
    {
        return Some(proof);
    }
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
fn prove_direct_subtract_relation(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let Proposition::IntegerMathLessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    let (difference, carrier_bound, lower) = match (goal_left, goal_right) {
        (IntegerMathTerm::Subtract(_, _), IntegerMathTerm::IntegerLiteral(bound)) => {
            (goal_left, *bound, false)
        }
        (IntegerMathTerm::IntegerLiteral(bound), IntegerMathTerm::Subtract(_, _)) => {
            (goal_right, *bound, true)
        }
        _ => return None,
    };
    let IntegerMathTerm::Subtract(left, right) = difference else {
        unreachable!("matched direct mathematical subtraction")
    };
    let integer_type = match (left.as_ref(), right.as_ref()) {
        (
            IntegerMathTerm::MathValue { source_type, .. },
            IntegerMathTerm::MathValue {
                source_type: right_type,
                ..
            },
        ) if source_type == right_type => *source_type,
        (IntegerMathTerm::MathValue { source_type, .. }, IntegerMathTerm::IntegerLiteral(_))
        | (IntegerMathTerm::IntegerLiteral(_), IntegerMathTerm::MathValue { source_type, .. }) => {
            *source_type
        }
        _ => return None,
    };
    if integer_type.carrier() != psi_core::IntegerCarrier::Fixed {
        return None;
    }
    let left = lower_add_math_leaf(left, integer_type)?;
    let right = lower_add_math_leaf(right, integer_type)?;
    let expected_carrier_bound = if lower {
        integer_type.minimum_value()
    } else {
        integer_type.maximum_value()
    };
    if carrier_bound.as_integer_value(integer_type) != Some(expected_carrier_bound) {
        return None;
    }

    let target =
        ScalarTerm::exact_integer_subtract(integer_type, left.clone(), right.clone()).ok()?;
    if lower && integer_type.sign() == IntegerSign::Unsigned {
        let endpoint_goal = Proposition::LessOrEqual(right.clone(), left.clone());
        if let Some(endpoint_proof) = bound::prove(
            context,
            &endpoint_goal,
            assumptions,
            semantic_axioms,
            definitions,
        ) {
            let witness = IntegerAffineWitness {
                root: right.clone(),
                target: target.clone(),
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            };
            if let Ok(form) = check_integer_affine_witness(context, semantic_axioms, &witness)
                && let Ok(mapped) = map_integer_affine_bound(&form, &endpoint_proof.conclusion)
                && &mapped == goal
            {
                return Some(ProofNode {
                    conclusion: mapped,
                    rule: ProofRule::IntegerAffineBound {
                        root_bound: Box::new(endpoint_proof),
                        witness,
                    },
                });
            }
        }
    }
    if let Some(proof) = prove_correlated_subtract_relation(
        context,
        goal,
        integer_type,
        &left,
        &right,
        &target,
        lower,
        assumptions,
        semantic_axioms,
        definitions,
    ) {
        return Some(proof);
    }
    if let Some(proof) = prove_targeted_subtract_relation(
        context,
        goal,
        integer_type,
        &left,
        &right,
        &target,
        lower,
        false,
        assumptions,
        semantic_axioms,
        definitions,
    ) {
        return Some(proof);
    }
    let mut candidates =
        add_endpoint_candidates(integer_type, &left, lower, assumptions, semantic_axioms);
    for candidate in
        add_endpoint_candidates(integer_type, &right, !lower, assumptions, semantic_axioms)
    {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
    for left_bound in candidates {
        let Some(left_proof) = prove_add_operand_endpoint(
            context,
            integer_type,
            &left,
            left_bound,
            lower,
            assumptions,
            semantic_axioms,
            definitions,
        ) else {
            continue;
        };
        let right_bound =
            exact_add_operand_value(integer_type, &right, assumptions, semantic_axioms)
                .or_else(|| subtract_complement(integer_type, left_bound, lower))
                .unwrap_or_else(|| {
                    if lower {
                        integer_type.maximum_value()
                    } else {
                        integer_type.minimum_value()
                    }
                });
        let Some(right_proof) = prove_add_operand_endpoint(
            context,
            integer_type,
            &right,
            right_bound,
            !lower,
            assumptions,
            semantic_axioms,
            definitions,
        ) else {
            continue;
        };
        let evidence = ProofNode {
            conclusion: Proposition::Conjunction(vec![
                left_proof.conclusion.clone(),
                right_proof.conclusion.clone(),
            ]),
            rule: ProofRule::ConjunctionIntroduction(vec![left_proof, right_proof]),
        };
        let witness = IntegerAffineWitness {
            root: left.clone(),
            target: target.clone(),
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
        if let Some(proof) = relax_math_bound(goal, mapped_proof) {
            return Some(proof);
        }
    }
    prove_targeted_subtract_relation(
        context,
        goal,
        integer_type,
        &left,
        &right,
        &target,
        lower,
        true,
        assumptions,
        semantic_axioms,
        definitions,
    )
}

#[allow(clippy::too_many_arguments)]
fn prove_targeted_subtract_relation(
    context: &PropositionContext,
    goal: &Proposition,
    integer_type: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
    target: &ScalarTerm,
    lower: bool,
    allow_relaxation: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let operand_endpoints =
        |operand: &ScalarTerm, endpoint_lower: bool, definitions: &mut DefinitionIndex| {
            if let Some((actual, value)) = operand.integer_value() {
                return (actual == integer_type)
                    .then(|| {
                        prove_add_operand_endpoint(
                            context,
                            integer_type,
                            operand,
                            value,
                            endpoint_lower,
                            assumptions,
                            semantic_axioms,
                            definitions,
                        )
                    })
                    .flatten()
                    .into_iter()
                    .collect::<Vec<_>>();
            }
            multiply::targeted_operand_endpoints(
                context,
                integer_type,
                operand,
                endpoint_lower,
                assumptions,
                semantic_axioms,
                definitions,
            )
        };
    let left_endpoints = operand_endpoints(left, lower, definitions);
    if left_endpoints.is_empty() {
        return None;
    }
    let right_endpoints = operand_endpoints(right, !lower, definitions);
    if right_endpoints.is_empty() {
        return None;
    }
    let witness = IntegerAffineWitness {
        root: left.clone(),
        target: target.clone(),
        definition_axioms: Vec::new(),
        literal_axioms: Vec::new(),
    };
    let form = check_integer_affine_witness(context, semantic_axioms, &witness).ok()?;
    let mut relaxed = None;
    for left_endpoint in left_endpoints {
        for right_endpoint in &right_endpoints {
            let evidence = ProofNode {
                conclusion: Proposition::Conjunction(vec![
                    left_endpoint.conclusion.clone(),
                    right_endpoint.conclusion.clone(),
                ]),
                rule: ProofRule::ConjunctionIntroduction(vec![
                    left_endpoint.clone(),
                    right_endpoint.clone(),
                ]),
            };
            let Ok(mapped) = map_integer_affine_bound(&form, &evidence.conclusion) else {
                continue;
            };
            let mapped_proof = ProofNode {
                conclusion: mapped.clone(),
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(evidence),
                    witness: witness.clone(),
                },
            };
            if &mapped == goal {
                return Some(mapped_proof);
            }
            if relaxed.is_none() {
                relaxed = relax_math_bound(goal, mapped_proof);
            }
        }
    }
    allow_relaxation.then_some(relaxed).flatten()
}

#[allow(clippy::too_many_arguments)]
fn prove_correlated_subtract_relation(
    context: &PropositionContext,
    goal: &Proposition,
    integer_type: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
    target: &ScalarTerm,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    for (index, axiom) in semantic_axioms.iter().enumerate() {
        let Proposition::Equal(equal_left, equal_right) = axiom else {
            continue;
        };
        for (root, expression) in [(equal_left, equal_right), (equal_right, equal_left)] {
            let ScalarTerm::Value {
                scalar_type: ScalarType::Integer(root_type),
                ..
            } = root
            else {
                continue;
            };
            let ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left: endpoint,
                right: add_right,
            } = expression
            else {
                continue;
            };
            let expected_endpoint = if lower {
                integer_type.minimum_value()
            } else {
                integer_type.maximum_value()
            };
            if *root_type != integer_type
                || *scalar_type != integer_type
                || add_right.as_ref() != right
            {
                continue;
            }
            let literal_axiom =
                if endpoint.integer_value() == Some((integer_type, expected_endpoint)) {
                    None
                } else {
                    let Some(landing_index) = semantic_axioms[..index].iter().enumerate().find_map(
                        |(landing_index, landing)| {
                            let Proposition::Equal(landing_left, landing_right) = landing else {
                                return None;
                            };
                            let literal = if landing_left == endpoint.as_ref() {
                                landing_right
                            } else if landing_right == endpoint.as_ref() {
                                landing_left
                            } else {
                                return None;
                            };
                            (literal.integer_value() == Some((integer_type, expected_endpoint)))
                                .then_some(landing_index)
                        },
                    ) else {
                        continue;
                    };
                    Some(landing_index)
                };
            let endpoint_goal = if lower {
                Proposition::LessOrEqual(root.clone(), left.clone())
            } else {
                Proposition::LessOrEqual(left.clone(), root.clone())
            };
            let Some(endpoint_proof) = bound::prove(
                context,
                &endpoint_goal,
                assumptions,
                semantic_axioms,
                definitions,
            ) else {
                continue;
            };
            let witness = IntegerAffineWitness {
                root: root.clone(),
                target: target.clone(),
                definition_axioms: vec![index],
                literal_axioms: vec![literal_axiom],
            };
            let Some(form) = check_integer_affine_witness(context, semantic_axioms, &witness).ok()
            else {
                continue;
            };
            let Some(mapped) = map_integer_affine_bound(&form, &endpoint_proof.conclusion).ok()
            else {
                continue;
            };
            if &mapped != goal {
                continue;
            }
            return Some(ProofNode {
                conclusion: mapped,
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(endpoint_proof),
                    witness,
                },
            });
        }
    }
    None
}

fn prove_direct_add_relation(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let Proposition::IntegerMathLessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    let (sum, carrier_bound, lower) = match (goal_left, goal_right) {
        (IntegerMathTerm::Add(_, _), IntegerMathTerm::IntegerLiteral(bound)) => {
            (goal_left, *bound, false)
        }
        (IntegerMathTerm::IntegerLiteral(bound), IntegerMathTerm::Add(_, _)) => {
            (goal_right, *bound, true)
        }
        _ => return None,
    };
    let IntegerMathTerm::Add(left, right) = sum else {
        unreachable!("matched direct mathematical addition")
    };
    let integer_type = match (left.as_ref(), right.as_ref()) {
        (
            IntegerMathTerm::MathValue { source_type, .. },
            IntegerMathTerm::MathValue {
                source_type: right_type,
                ..
            },
        ) if source_type == right_type => *source_type,
        (IntegerMathTerm::MathValue { source_type, .. }, IntegerMathTerm::IntegerLiteral(_))
        | (IntegerMathTerm::IntegerLiteral(_), IntegerMathTerm::MathValue { source_type, .. }) => {
            *source_type
        }
        _ => return None,
    };
    if integer_type.carrier() != psi_core::IntegerCarrier::Fixed {
        return None;
    }
    let left = lower_add_math_leaf(left, integer_type)?;
    let right = lower_add_math_leaf(right, integer_type)?;
    let expected_carrier_bound = if lower {
        integer_type.minimum_value()
    } else {
        integer_type.maximum_value()
    };
    if carrier_bound.as_integer_value(integer_type) != Some(expected_carrier_bound) {
        return None;
    }

    let target = ScalarTerm::exact_integer_add(integer_type, left.clone(), right.clone()).ok()?;
    if let Some(proof) = prove_correlated_add_relation(
        context,
        goal,
        integer_type,
        &left,
        &right,
        &target,
        lower,
        assumptions,
        semantic_axioms,
        definitions,
    ) {
        return Some(proof);
    }
    if let Some(proof) = prove_targeted_add_relation(
        context,
        goal,
        integer_type,
        &left,
        &right,
        &target,
        lower,
        false,
        assumptions,
        semantic_axioms,
        definitions,
    ) {
        return Some(proof);
    }
    let mut candidates =
        direct_add_endpoint_candidates(integer_type, &left, assumptions, semantic_axioms);
    for candidate in
        direct_add_endpoint_candidates(integer_type, &right, assumptions, semantic_axioms)
    {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
    for candidate in
        add_endpoint_candidates(integer_type, &left, lower, assumptions, semantic_axioms)
    {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
    for candidate in
        add_endpoint_candidates(integer_type, &right, lower, assumptions, semantic_axioms)
    {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
    for left_bound in candidates {
        let Some(left_proof) = prove_add_operand_endpoint(
            context,
            integer_type,
            &left,
            left_bound,
            lower,
            assumptions,
            semantic_axioms,
            definitions,
        ) else {
            continue;
        };
        let right_bound =
            exact_add_operand_value(integer_type, &right, assumptions, semantic_axioms)
                .or_else(|| add_complement(integer_type, left_bound, lower))?;
        let Some(right_proof) = prove_add_operand_endpoint(
            context,
            integer_type,
            &right,
            right_bound,
            lower,
            assumptions,
            semantic_axioms,
            definitions,
        ) else {
            continue;
        };
        let evidence = ProofNode {
            conclusion: Proposition::Conjunction(vec![
                left_proof.conclusion.clone(),
                right_proof.conclusion.clone(),
            ]),
            rule: ProofRule::ConjunctionIntroduction(vec![left_proof, right_proof]),
        };
        let witness = IntegerAffineWitness {
            root: left.clone(),
            target: target.clone(),
            definition_axioms: Vec::new(),
            literal_axioms: Vec::new(),
        };
        let form = check_integer_affine_witness(context, semantic_axioms, &witness).ok()?;
        let mapped = match map_integer_affine_bound(&form, &evidence.conclusion) {
            Ok(mapped) => mapped,
            Err(_) => continue,
        };
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
        if let Some(proof) = relax_math_bound(goal, mapped_proof) {
            return Some(proof);
        }
    }
    prove_targeted_add_relation(
        context,
        goal,
        integer_type,
        &left,
        &right,
        &target,
        lower,
        true,
        assumptions,
        semantic_axioms,
        definitions,
    )
}

#[allow(clippy::too_many_arguments)]
fn prove_targeted_add_relation(
    context: &PropositionContext,
    goal: &Proposition,
    integer_type: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
    target: &ScalarTerm,
    lower: bool,
    allow_relaxation: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let operand_endpoints = |operand: &ScalarTerm, definitions: &mut DefinitionIndex| {
        if let Some((actual, value)) = operand.integer_value() {
            return (actual == integer_type)
                .then(|| {
                    prove_add_operand_endpoint(
                        context,
                        integer_type,
                        operand,
                        value,
                        lower,
                        assumptions,
                        semantic_axioms,
                        definitions,
                    )
                })
                .flatten()
                .into_iter()
                .collect::<Vec<_>>();
        }
        multiply::targeted_operand_endpoints(
            context,
            integer_type,
            operand,
            lower,
            assumptions,
            semantic_axioms,
            definitions,
        )
    };
    let left_endpoints = operand_endpoints(left, definitions);
    if left_endpoints.is_empty() {
        return None;
    }
    let right_endpoints = operand_endpoints(right, definitions);
    if right_endpoints.is_empty() {
        return None;
    }
    let witness = IntegerAffineWitness {
        root: left.clone(),
        target: target.clone(),
        definition_axioms: Vec::new(),
        literal_axioms: Vec::new(),
    };
    let form = check_integer_affine_witness(context, semantic_axioms, &witness).ok()?;
    let mut relaxed = None;
    for left_endpoint in left_endpoints {
        for right_endpoint in &right_endpoints {
            let evidence = ProofNode {
                conclusion: Proposition::Conjunction(vec![
                    left_endpoint.conclusion.clone(),
                    right_endpoint.conclusion.clone(),
                ]),
                rule: ProofRule::ConjunctionIntroduction(vec![
                    left_endpoint.clone(),
                    right_endpoint.clone(),
                ]),
            };
            let Ok(mapped) = map_integer_affine_bound(&form, &evidence.conclusion) else {
                continue;
            };
            let mapped_proof = ProofNode {
                conclusion: mapped.clone(),
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(evidence),
                    witness: witness.clone(),
                },
            };
            if &mapped == goal {
                return Some(mapped_proof);
            }
            if relaxed.is_none() {
                relaxed = relax_math_bound(goal, mapped_proof);
            }
        }
    }
    allow_relaxation.then_some(relaxed).flatten()
}

#[allow(clippy::too_many_arguments)]
fn prove_correlated_add_relation(
    context: &PropositionContext,
    goal: &Proposition,
    integer_type: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
    target: &ScalarTerm,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    for (index, axiom) in semantic_axioms.iter().enumerate() {
        let Proposition::Equal(equal_left, equal_right) = axiom else {
            continue;
        };
        for (root, expression) in [(equal_left, equal_right), (equal_right, equal_left)] {
            let ScalarTerm::Value {
                scalar_type: ScalarType::Integer(root_type),
                ..
            } = root
            else {
                continue;
            };
            let ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left: endpoint,
                right: subtract_right,
            } = expression
            else {
                continue;
            };
            let expected_endpoint = if lower {
                integer_type.minimum_value()
            } else {
                integer_type.maximum_value()
            };
            if *root_type != integer_type
                || *scalar_type != integer_type
                || subtract_right.as_ref() != right
            {
                continue;
            }
            let literal_axiom =
                if endpoint.integer_value() == Some((integer_type, expected_endpoint)) {
                    None
                } else {
                    let Some(landing_index) = semantic_axioms[..index].iter().enumerate().find_map(
                        |(landing_index, landing)| {
                            let Proposition::Equal(landing_left, landing_right) = landing else {
                                return None;
                            };
                            let literal = if landing_left == endpoint.as_ref() {
                                landing_right
                            } else if landing_right == endpoint.as_ref() {
                                landing_left
                            } else {
                                return None;
                            };
                            (literal.integer_value() == Some((integer_type, expected_endpoint)))
                                .then_some(landing_index)
                        },
                    ) else {
                        continue;
                    };
                    Some(landing_index)
                };
            let endpoint_goal = if lower {
                Proposition::LessOrEqual(root.clone(), left.clone())
            } else {
                Proposition::LessOrEqual(left.clone(), root.clone())
            };
            let Some(endpoint_proof) = bound::prove(
                context,
                &endpoint_goal,
                assumptions,
                semantic_axioms,
                definitions,
            ) else {
                continue;
            };
            let witness = IntegerAffineWitness {
                root: root.clone(),
                target: target.clone(),
                definition_axioms: vec![index],
                literal_axioms: vec![literal_axiom],
            };
            let Some(form) = check_integer_affine_witness(context, semantic_axioms, &witness).ok()
            else {
                continue;
            };
            let Some(mapped) = map_integer_affine_bound(&form, &endpoint_proof.conclusion).ok()
            else {
                continue;
            };
            if &mapped != goal {
                continue;
            }
            return Some(ProofNode {
                conclusion: mapped,
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(endpoint_proof),
                    witness,
                },
            });
        }
    }
    None
}

fn exact_add_operand_value(
    integer_type: IntegerType,
    operand: &ScalarTerm,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<IntegerValue> {
    if let Some((actual, value)) = operand.integer_value() {
        return (actual == integer_type).then_some(value);
    }
    cited_facts(assumptions, semantic_axioms).find_map(|(_, fact)| {
        let Proposition::Equal(left, right) = fact else {
            return None;
        };
        let (actual, value) = if left == operand {
            right.integer_value()
        } else if right == operand {
            left.integer_value()
        } else {
            None
        }?;
        (actual == integer_type).then_some(value)
    })
}

pub(super) fn lower_add_math_leaf(
    term: &IntegerMathTerm,
    integer_type: IntegerType,
) -> Option<ScalarTerm> {
    match term {
        IntegerMathTerm::MathValue { source_type, value } if *source_type == integer_type => {
            Some(ScalarTerm::value(*value, ScalarType::Integer(integer_type)))
        }
        IntegerMathTerm::IntegerLiteral(literal) => {
            ScalarTerm::integer(integer_type, literal.as_integer_value(integer_type)?).ok()
        }
        _ => None,
    }
}

pub(super) fn add_endpoint_candidates(
    integer_type: IntegerType,
    operand: &ScalarTerm,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Vec<IntegerValue> {
    let mut candidates = Vec::new();
    if let Some((actual, value)) = operand.integer_value()
        && actual == integer_type
    {
        candidates.push(value);
    }
    for (_, fact) in cited_facts(assumptions, semantic_axioms) {
        let (left, right) = match fact {
            Proposition::Equal(left, right) | Proposition::LessOrEqual(left, right) => {
                (left, right)
            }
            _ => continue,
        };
        for term in [left, right] {
            if let Some((actual, value)) = term.integer_value()
                && let Some(value) = if actual == integer_type {
                    Some(value)
                } else {
                    actual.exact_cast_value_to(integer_type, value)
                }
                && !candidates.contains(&value)
            {
                candidates.push(value);
            }
        }
    }
    let carrier = if lower {
        integer_type.minimum_value()
    } else {
        integer_type.maximum_value()
    };
    if !candidates.contains(&carrier) {
        candidates.push(carrier);
    }
    candidates
}

fn direct_add_endpoint_candidates(
    integer_type: IntegerType,
    operand: &ScalarTerm,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Vec<IntegerValue> {
    let mut candidates = Vec::new();
    if let Some((actual, value)) = operand.integer_value()
        && actual == integer_type
    {
        candidates.push(value);
    }
    for (_, fact) in cited_facts(assumptions, semantic_axioms) {
        let (left, right) = match fact {
            Proposition::Equal(left, right) | Proposition::LessOrEqual(left, right) => {
                (left, right)
            }
            _ => continue,
        };
        let literal = if left == operand {
            right
        } else if right == operand {
            left
        } else {
            continue;
        };
        let Some((actual, value)) = literal.integer_value() else {
            continue;
        };
        let Some(value) = (if actual == integer_type {
            Some(value)
        } else {
            actual.exact_cast_value_to(integer_type, value)
        }) else {
            continue;
        };
        if !candidates.contains(&value) {
            candidates.push(value);
        }
    }
    candidates
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prove_add_operand_endpoint(
    context: &PropositionContext,
    integer_type: IntegerType,
    operand: &ScalarTerm,
    bound_value: IntegerValue,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    if let Some((actual, value)) = operand.integer_value() {
        return (actual == integer_type && value == bound_value).then_some(ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        });
    }
    let carrier_endpoint = if lower {
        integer_type.minimum_value()
    } else {
        integer_type.maximum_value()
    };
    if bound_value == carrier_endpoint {
        return Some(ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        });
    }
    let literal = ScalarTerm::integer(integer_type, bound_value).ok()?;
    let endpoint = if lower {
        Proposition::LessOrEqual(literal, operand.clone())
    } else {
        Proposition::LessOrEqual(operand.clone(), literal)
    };
    bound::prove_candidate_endpoint(
        context,
        &endpoint,
        assumptions,
        semantic_axioms,
        definitions,
    )
}

fn add_complement(
    integer_type: IntegerType,
    operand_bound: IntegerValue,
    lower: bool,
) -> Option<IntegerValue> {
    match (integer_type.sign(), operand_bound) {
        (IntegerSign::Signed, IntegerValue::Signed(value)) => {
            let carrier = match if lower {
                integer_type.minimum_value()
            } else {
                integer_type.maximum_value()
            } {
                IntegerValue::Signed(carrier) => carrier,
                IntegerValue::Unsigned(_) => unreachable!("signed carrier has signed endpoints"),
            };
            Some(IntegerValue::Signed(
                carrier.checked_sub(value).unwrap_or(carrier),
            ))
        }
        (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) if !lower => {
            let IntegerValue::Unsigned(carrier) = integer_type.maximum_value() else {
                unreachable!("unsigned carrier has unsigned endpoint")
            };
            Some(IntegerValue::Unsigned(carrier.checked_sub(value)?))
        }
        _ => None,
    }
}

fn subtract_complement(
    integer_type: IntegerType,
    left_bound: IntegerValue,
    lower: bool,
) -> Option<IntegerValue> {
    match (integer_type.sign(), left_bound) {
        (IntegerSign::Signed, IntegerValue::Signed(left)) => {
            let carrier = match if lower {
                integer_type.minimum_value()
            } else {
                integer_type.maximum_value()
            } {
                IntegerValue::Signed(carrier) => carrier,
                IntegerValue::Unsigned(_) => unreachable!("signed carrier has signed endpoints"),
            };
            let value = IntegerValue::Signed(left.checked_sub(carrier)?);
            integer_type.admits(value).then_some(value)
        }
        (IntegerSign::Unsigned, IntegerValue::Unsigned(left)) if lower => {
            Some(IntegerValue::Unsigned(left))
        }
        _ => None,
    }
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

pub(super) fn relax_math_bound(goal: &Proposition, mapped: ProofNode) -> Option<ProofNode> {
    let (
        Proposition::IntegerMathLessOrEqual(goal_left, goal_right),
        Proposition::IntegerMathLessOrEqual(mapped_left, mapped_right),
    ) = (goal, &mapped.conclusion)
    else {
        return None;
    };
    if goal_left == mapped_left {
        if !closed_math_literal_less_or_equal(mapped_right, goal_right) {
            return None;
        }
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
        if !closed_math_literal_less_or_equal(goal_left, mapped_left) {
            return None;
        }
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

fn closed_math_literal_less_or_equal(left: &IntegerMathTerm, right: &IntegerMathTerm) -> bool {
    let (IntegerMathTerm::IntegerLiteral(left), IntegerMathTerm::IntegerLiteral(right)) =
        (left, right)
    else {
        return false;
    };
    match (left.negative(), right.negative()) {
        (true, false) => true,
        (false, true) => false,
        (false, false) => left.magnitude() <= right.magnitude(),
        (true, true) => right.magnitude() <= left.magnitude(),
    }
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
