//! Derived fixed-integer endpoint bounds through the cited equality and
//! single-definition closure.
//!
//! Obligations regularly ask for a bound on a value whose only connection to
//! cited facts is a chain of exact `Equal` axioms: a computed result aliases a
//! field, the field aliases an earlier value, and that value carries the only
//! literal endpoint. No single-hop substitution or landed-literal affine
//! witness reaches those bounds. This producer closes the gap locally: it
//! derives `literal <= term` and `term <= literal` certificates in one bounded
//! forward pass that composes only rules the kernel already checks —
//! `IntegerOrderSubstitution` across cited equalities, `IntegerAffineBound`
//! over one-step definition witnesses with literal or landed-literal sibling
//! operands, `IntegerExactAddDefinitionBound` over two proved operand
//! endpoints, and the total-image bounds `range` already produces.

use proof_admission::{
    IntegerAffineWitness, PrimitiveJudgment, ProofNode, ProofRule, check_integer_affine_witness,
    map_integer_affine_bound,
};
use semantic_vocabulary::{
    IntegerCarrier, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType,
};

use super::super::integer_evidence::{
    ProjectedFact, closed_integer_relation, integer_carrier_bound, projected_facts, relax,
};
use super::range;

/// Rounds of equality hops and definition maps before the closure settles.
/// Chains observed in retained evidence stay far below this bound; the cap
/// only keeps pathological axiom sets finite.
const MAX_ROUNDS: usize = 64;
/// Bound on the derived proposition set. Each member is a distinct
/// `literal <= term` or `term <= literal` conclusion over this module's own
/// term and literal universe, so the set is finite; the cap is defensive.
const MAX_PROOFS: usize = 2048;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    let ScalarType::Integer(integer_type) = goal_left.scalar_type() else {
        return None;
    };
    if integer_type.carrier() != IntegerCarrier::Fixed
        || goal_right.scalar_type() != ScalarType::Integer(integer_type)
    {
        return None;
    }
    // The closure only proves literal endpoints; both-literal goals are the
    // closed-relation producer's domain and both-term goals stay with order.
    if goal_left.integer_value().is_none() && goal_right.integer_value().is_none() {
        return None;
    }
    for bound in closure(context, integer_type, assumptions, semantic_axioms) {
        if bound.conclusion == *goal {
            return Some(bound);
        }
        if let Some(proof) = relax(goal, bound) {
            return Some(proof);
        }
    }
    None
}

/// All derived endpoint proofs whose bounded side is `operand`, in the
/// requested orientation. Multiply operand selection consumes these the same
/// way it consumes cited endpoints.
pub(super) fn operand_endpoints(
    context: &PropositionContext,
    integer_type: IntegerType,
    operand: &ScalarTerm,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Vec<ProofNode> {
    closure(context, integer_type, assumptions, semantic_axioms)
        .into_iter()
        .filter(|proof| match &proof.conclusion {
            Proposition::LessOrEqual(left, right) if lower => {
                left.integer_value().is_some() && right == operand
            }
            Proposition::LessOrEqual(left, right) => {
                left == operand && right.integer_value().is_some()
            }
            _ => false,
        })
        .collect()
}

/// The bounded derivation itself: cited endpoint facts, carrier endpoints,
/// landed literal equalities, and total-image definition bounds seed a
/// proposition set that equality substitution and single-definition maps
/// extend until it stops growing.
fn closure(
    context: &PropositionContext,
    integer_type: IntegerType,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Vec<ProofNode> {
    let facts = projected_facts(assumptions, semantic_axioms);
    let mut equalities: Vec<ProofNode> = Vec::new();
    let mut proofs = seeds(context, integer_type, &facts, &mut equalities);
    let mut definitions: Vec<(usize, ScalarTerm, ScalarTerm)> = Vec::new();
    for (index, axiom) in semantic_axioms.iter().enumerate() {
        let Proposition::Equal(left, right) = axiom else {
            continue;
        };
        let (output, expression) = match (left, right) {
            (output @ ScalarTerm::Value { .. }, expression)
            | (expression, output @ ScalarTerm::Value { .. })
                if output.scalar_type() == ScalarType::Integer(integer_type)
                    && operation_expression(expression) =>
            {
                (output.clone(), (*expression).clone())
            }
            _ => continue,
        };
        definitions.push((index, output, expression));
    }
    // Total-image bounds on definition outputs seed the same closure: a
    // remainder or nonzero divide output already carries its image.
    for (_, output, _) in &definitions {
        for bound in range::target_bounds(context, output, semantic_axioms) {
            push_new(&mut proofs, bound);
        }
    }
    for _ in 0..MAX_ROUNDS {
        let mut discovered = Vec::new();
        for bound in &proofs {
            substitute_equalities(bound, &equalities, &mut discovered);
        }
        for (index, output, expression) in &definitions {
            add_definition_bounds(
                integer_type,
                *index,
                output,
                expression,
                &proofs,
                &equalities,
                &mut discovered,
            );
            affine_definition_bounds(
                context,
                integer_type,
                *index,
                output,
                expression,
                semantic_axioms,
                &proofs,
                &mut discovered,
            );
        }
        let mut fresh = false;
        for proof in discovered {
            fresh |= push_new(&mut proofs, proof);
            if proofs.len() >= MAX_PROOFS {
                return proofs;
            }
        }
        if !fresh {
            break;
        }
    }
    proofs
}

/// Seed the closure with every cited literal endpoint on an integer term,
/// the carrier endpoints of every integer term the facts mention, and the
/// both-direction endpoint consequences of cited literal equalities.
fn seeds(
    context: &PropositionContext,
    integer_type: IntegerType,
    facts: &[ProjectedFact<'_>],
    equalities: &mut Vec<ProofNode>,
) -> Vec<ProofNode> {
    let mut proofs = Vec::new();
    let mut terms = Vec::<&ScalarTerm>::new();
    for fact in facts {
        let proposition = fact.proposition;
        match proposition {
            Proposition::LessOrEqual(left, right) => {
                for term in [left, right] {
                    if term.scalar_type() == ScalarType::Integer(integer_type) {
                        terms.push(term);
                    }
                }
                if literal_endpoint(proposition, integer_type) {
                    push_new(&mut proofs, fact.proof());
                }
            }
            Proposition::LessThan(left, right) => {
                for term in [left, right] {
                    if term.scalar_type() == ScalarType::Integer(integer_type) {
                        terms.push(term);
                    }
                }
                if literal_endpoint(proposition, integer_type) {
                    let weakened = ProofNode {
                        conclusion: Proposition::LessOrEqual(left.clone(), right.clone()),
                        rule: ProofRule::IntegerOrderWeakening {
                            relation: Box::new(fact.proof()),
                        },
                    };
                    push_new(&mut proofs, weakened);
                }
            }
            Proposition::Equal(left, right)
                if left.scalar_type() == ScalarType::Integer(integer_type) =>
            {
                terms.push(left);
                terms.push(right);
                equalities.push(fact.proof());
                for (operand, literal) in [(left, right), (right, left)] {
                    if literal.integer_value().is_some()
                        && operand.integer_value().is_none()
                        && operand.scalar_type() == ScalarType::Integer(integer_type)
                    {
                        for lower in [true, false] {
                            if let Some(proof) = landed_literal_endpoint(
                                integer_type,
                                operand,
                                literal,
                                lower,
                                fact.proof(),
                            ) {
                                push_new(&mut proofs, proof);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    terms.sort();
    terms.dedup();
    for term in terms {
        for (value, lower) in [
            (integer_type.minimum_value(), true),
            (integer_type.maximum_value(), false),
        ] {
            let Ok(literal) = ScalarTerm::integer(integer_type, value) else {
                continue;
            };
            let conclusion = if lower {
                Proposition::LessOrEqual(literal, term.clone())
            } else {
                Proposition::LessOrEqual(term.clone(), literal)
            };
            if let Some(proof) = integer_carrier_bound(context, &conclusion) {
                push_new(&mut proofs, proof);
            }
        }
    }
    proofs
}

/// Does this fact already have one literal endpoint of the requested type?
fn literal_endpoint(proposition: &Proposition, integer_type: IntegerType) -> bool {
    let (left, right) = match proposition {
        Proposition::LessOrEqual(left, right) | Proposition::LessThan(left, right) => (left, right),
        _ => return false,
    };
    left.integer_value()
        .is_some_and(|(actual, _)| actual == integer_type)
        || right
            .integer_value()
            .is_some_and(|(actual, _)| actual == integer_type)
}

/// `operand = literal` orients both endpoints: `literal <= literal` closes
/// and the cited equality substitutes one side.
fn landed_literal_endpoint(
    integer_type: IntegerType,
    operand: &ScalarTerm,
    literal: &ScalarTerm,
    lower: bool,
    equality: ProofNode,
) -> Option<ProofNode> {
    let (actual, _) = literal.integer_value()?;
    if actual != integer_type {
        return None;
    }
    let closed =
        closed_integer_relation(Proposition::LessOrEqual(literal.clone(), literal.clone()))?;
    let conclusion = if lower {
        Proposition::LessOrEqual(literal.clone(), operand.clone())
    } else {
        Proposition::LessOrEqual(operand.clone(), literal.clone())
    };
    Some(ProofNode {
        conclusion,
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(closed),
            equality: Box::new(equality),
            endpoint: usize::from(lower),
        },
    })
}

/// Transport one derived endpoint bound along every cited equality that
/// touches its non-literal side.
fn substitute_equalities(bound: &ProofNode, equalities: &[ProofNode], out: &mut Vec<ProofNode>) {
    let Proposition::LessOrEqual(left, right) = &bound.conclusion else {
        return;
    };
    for (endpoint, old) in [(0, left), (1, right)] {
        if old.integer_value().is_some() {
            continue;
        }
        for equality in equalities {
            let Proposition::Equal(equal_left, equal_right) = &equality.conclusion else {
                continue;
            };
            let new = if equal_left == old {
                equal_right
            } else if equal_right == old {
                equal_left
            } else {
                continue;
            };
            if new == old {
                continue;
            }
            let conclusion = if endpoint == 0 {
                Proposition::LessOrEqual(new.clone(), right.clone())
            } else {
                Proposition::LessOrEqual(left.clone(), new.clone())
            };
            out.push(ProofNode {
                conclusion,
                rule: ProofRule::IntegerOrderSubstitution {
                    relation: Box::new(bound.clone()),
                    equality: Box::new(equality.clone()),
                    endpoint,
                },
            });
        }
    }
}

/// `output = left + right` exactly: two proved operand endpoints compose one
/// output endpoint through the kernel's exact-add definition rule.
fn add_definition_bounds(
    integer_type: IntegerType,
    definition_axiom: usize,
    output: &ScalarTerm,
    expression: &ScalarTerm,
    proofs: &[ProofNode],
    equalities: &[ProofNode],
    out: &mut Vec<ProofNode>,
) {
    let ScalarTerm::ExactIntegerAdd {
        scalar_type,
        left,
        right,
    } = expression
    else {
        return;
    };
    if *scalar_type != integer_type {
        return;
    }
    for lower in [true, false] {
        let left_evidence = operand_evidence(integer_type, left, lower, proofs, equalities);
        let right_evidence = operand_evidence(integer_type, right, lower, proofs, equalities);
        for left_bound in &left_evidence {
            for right_bound in &right_evidence {
                // The kernel resolves the mapped side from endpoint
                // orientations; two unoriented members (`Exact`/`Carrier`)
                // carry no direction and the pair must be rejected here
                // rather than emitted as an invalid node.
                if !oriented(left_bound, left, integer_type, lower)
                    && !oriented(right_bound, right, integer_type, lower)
                {
                    continue;
                }
                let Some(left_literal) = evidence_literal(left_bound, left, integer_type, lower)
                else {
                    continue;
                };
                let Some(right_literal) = evidence_literal(right_bound, right, integer_type, lower)
                else {
                    continue;
                };
                let Some(sum) = add_literals(integer_type, left_literal, right_literal) else {
                    continue;
                };
                let Ok(literal) = ScalarTerm::integer(integer_type, sum) else {
                    continue;
                };
                let conclusion = if lower {
                    Proposition::LessOrEqual(literal, output.clone())
                } else {
                    Proposition::LessOrEqual(output.clone(), literal)
                };
                out.push(ProofNode {
                    conclusion,
                    rule: ProofRule::IntegerExactAddDefinitionBound {
                        left_bound: Box::new(left_bound.clone()),
                        right_bound: Box::new(right_bound.clone()),
                        definition_axiom,
                    },
                });
            }
        }
    }
}

/// Map one proved root bound through one cited definition step: exact
/// subtract/multiply/divide/remainder and shifts with a literal or
/// landed-literal sibling operand, plus add in either operand order.
fn affine_definition_bounds(
    context: &PropositionContext,
    integer_type: IntegerType,
    definition_axiom: usize,
    output: &ScalarTerm,
    expression: &ScalarTerm,
    semantic_axioms: &[Proposition],
    proofs: &[ProofNode],
    out: &mut Vec<ProofNode>,
) {
    let pairs: Vec<(&ScalarTerm, &ScalarTerm)> = match expression {
        // A bitwise and is commutative like an add: either operand may be the
        // chain root while the other must land as the mask literal.
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type => {
            vec![
                (left.as_ref(), right.as_ref()),
                (right.as_ref(), left.as_ref()),
            ]
        }
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::ExactIntegerDivide {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer_type => vec![(left.as_ref(), right.as_ref())],
        ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            value,
            count,
            ..
        }
        | ScalarTerm::ExactIntegerShiftRight {
            value_type,
            value,
            count,
            ..
        } if *value_type == integer_type => vec![(value.as_ref(), count.as_ref())],
        _ => Vec::new(),
    };
    for (root, sibling) in pairs {
        if !matches!(root, ScalarTerm::Value { .. }) {
            continue;
        }
        let literal_axiom = if sibling.integer_value().is_some() {
            None
        } else {
            let Some(index) = semantic_axioms[..definition_axiom]
                .iter()
                .enumerate()
                .rev()
                .find_map(|(index, proposition)| match proposition {
                    Proposition::Equal(left, right)
                        if (left == sibling && right.integer_value().is_some())
                            || (right == sibling && left.integer_value().is_some()) =>
                    {
                        Some(index)
                    }
                    _ => None,
                })
            else {
                continue;
            };
            Some(index)
        };
        let witness = IntegerAffineWitness {
            root: root.clone(),
            target: output.clone(),
            definition_axioms: vec![definition_axiom],
            literal_axioms: vec![literal_axiom],
        };
        let Ok(form) = check_integer_affine_witness(context, semantic_axioms, &witness) else {
            continue;
        };
        for bound in proofs {
            let Proposition::LessOrEqual(left, right) = &bound.conclusion else {
                continue;
            };
            if left != root && right != root {
                continue;
            }
            let Ok(mapped) = map_integer_affine_bound(&form, &bound.conclusion) else {
                continue;
            };
            if !matches!(mapped, Proposition::LessOrEqual(..)) {
                continue;
            }
            out.push(ProofNode {
                conclusion: mapped,
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(bound.clone()),
                    witness: witness.clone(),
                },
            });
        }
    }
}

/// Every proof usable as one operand's endpoint evidence in the requested
/// orientation: derived literal bounds, cited literal equalities, and the
/// carrier `Truth` endpoint the kernel admits for a missing endpoint.
fn operand_evidence(
    integer_type: IntegerType,
    operand: &ScalarTerm,
    lower: bool,
    proofs: &[ProofNode],
    equalities: &[ProofNode],
) -> Vec<ProofNode> {
    // A literal operand admits only `Truth` evidence; the kernel reads the
    // operand itself as the exact endpoint.
    if operand.integer_value().is_some() {
        return vec![ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        }];
    }
    let mut evidence = Vec::new();
    for proof in proofs {
        let matches = match &proof.conclusion {
            Proposition::LessOrEqual(left, right) if lower => {
                right == operand
                    && left
                        .integer_value()
                        .is_some_and(|(actual, _)| actual == integer_type)
            }
            Proposition::LessOrEqual(left, right) => {
                left == operand
                    && right
                        .integer_value()
                        .is_some_and(|(actual, _)| actual == integer_type)
            }
            _ => false,
        };
        if matches {
            evidence.push(proof.clone());
        }
    }
    for equality in equalities {
        let Proposition::Equal(left, right) = &equality.conclusion else {
            continue;
        };
        let literal = if left == operand {
            right
        } else if right == operand {
            left
        } else {
            continue;
        };
        if literal
            .integer_value()
            .is_some_and(|(actual, _)| actual == integer_type)
        {
            evidence.push(equality.clone());
        }
    }
    evidence.push(ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    });
    evidence
}

/// Is this evidence an oriented literal bound on `operand` in the requested
/// direction — the only member kind that fixes a direction for a pair?
fn oriented(
    evidence: &ProofNode,
    operand: &ScalarTerm,
    integer_type: IntegerType,
    lower: bool,
) -> bool {
    match &evidence.conclusion {
        Proposition::LessOrEqual(left, right) if lower => {
            right == operand
                && left
                    .integer_value()
                    .is_some_and(|(actual, _)| actual == integer_type)
        }
        Proposition::LessOrEqual(left, right) => {
            left == operand
                && right
                    .integer_value()
                    .is_some_and(|(actual, _)| actual == integer_type)
        }
        _ => false,
    }
}

/// The literal one operand evidence member contributes on the requested side,
/// matching the kernel's `DirectAddEndpoint` projection: oriented bounds cite
/// their literal, exact equalities their literal sibling, and `Truth` the
/// carrier endpoint.
fn evidence_literal(
    evidence: &ProofNode,
    operand: &ScalarTerm,
    integer_type: IntegerType,
    lower: bool,
) -> Option<IntegerValue> {
    match &evidence.conclusion {
        Proposition::LessOrEqual(left, right) if lower && right == operand => left
            .integer_value()
            .and_then(|(actual, value)| (actual == integer_type).then_some(value)),
        Proposition::LessOrEqual(left, right) if !lower && left == operand => right
            .integer_value()
            .and_then(|(actual, value)| (actual == integer_type).then_some(value)),
        Proposition::Equal(left, right) => {
            let literal = if left == operand {
                right
            } else if right == operand {
                left
            } else {
                return None;
            };
            literal
                .integer_value()
                .and_then(|(actual, value)| (actual == integer_type).then_some(value))
        }
        Proposition::Truth => operand.integer_value().map_or_else(
            || {
                Some(if lower {
                    integer_type.minimum_value()
                } else {
                    integer_type.maximum_value()
                })
            },
            |(actual, value)| (actual == integer_type).then_some(value),
        ),
        _ => None,
    }
}

fn add_literals(
    integer_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
) -> Option<IntegerValue> {
    let value = match (left, right) {
        (IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
            IntegerValue::Signed(left.checked_add(right)?)
        }
        (IntegerValue::Unsigned(left), IntegerValue::Unsigned(right)) => {
            IntegerValue::Unsigned(left.checked_add(right)?)
        }
        _ => return None,
    };
    integer_type.admits(value).then_some(value)
}

fn operation_expression(term: &ScalarTerm) -> bool {
    matches!(
        term,
        ScalarTerm::ExactIntegerAdd { .. }
            | ScalarTerm::ExactIntegerSubtract { .. }
            | ScalarTerm::ExactIntegerMultiply { .. }
            | ScalarTerm::ExactIntegerDivide { .. }
            | ScalarTerm::ExactIntegerRemainder { .. }
            | ScalarTerm::ExactIntegerShiftLeft { .. }
            | ScalarTerm::ExactIntegerShiftRight { .. }
            | ScalarTerm::IntegerBitwiseAnd { .. }
    )
}

fn push_new(proofs: &mut Vec<ProofNode>, proof: ProofNode) -> bool {
    if proofs
        .iter()
        .any(|existing| existing.conclusion == proof.conclusion)
    {
        return false;
    }
    proofs.push(proof);
    true
}
