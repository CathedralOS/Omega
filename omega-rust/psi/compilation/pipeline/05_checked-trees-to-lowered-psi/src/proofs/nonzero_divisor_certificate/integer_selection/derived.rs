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
//! operands, `IntegerExactAddDefinitionBound` and
//! `IntegerExactSubtractDefinitionBound` over two proved operand endpoints,
//! and the total-image bounds `range` already produces.
//! Prior nonliteral products use four-corner bounds and explicit equality
//! transport in `multiplication`; no new trusted arithmetic rule is needed.

mod multiplication;

use std::rc::Rc;

use proof_admission::{
    IntegerAffineWitness, PrimitiveJudgment, ProofNode, ProofRule, check_integer_affine_witness,
    map_integer_affine_bound,
};
use semantic_vocabulary::{
    IntegerCarrier, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType,
};

use super::super::affine_custody::DefinitionIndex;
use super::super::cast_custody;
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
    for bound in memoized_closure(context, integer_type, assumptions, semantic_axioms).iter() {
        if bound.conclusion == *goal {
            return Some(bound.clone());
        }
        if let Some(proof) = relax(goal, bound.clone()) {
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
    endpoints(
        &memoized_closure(context, integer_type, assumptions, semantic_axioms),
        operand,
        lower,
    )
}

fn endpoints(closure: &[ProofNode], operand: &ScalarTerm, lower: bool) -> Vec<ProofNode> {
    closure
        .iter()
        .filter(|proof| match &proof.conclusion {
            Proposition::LessOrEqual(left, right) if lower => {
                left.integer_value().is_some() && right == operand
            }
            Proposition::LessOrEqual(left, right) => {
                left == operand && right.integer_value().is_some()
            }
            _ => false,
        })
        .cloned()
        .collect()
}

/// The closure for this scope and type, derived once: it does not depend on
/// the goal, and each derived goal or operand-endpoint query would otherwise
/// rebuild it, including its kernel-checked cast-image bounds.
fn memoized_closure(
    context: &PropositionContext,
    integer_type: IntegerType,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Rc<[ProofNode]> {
    let mut definitions = DefinitionIndex::new(context, assumptions, semantic_axioms);
    if let Some(closure) = definitions.cached_derived_closure(integer_type) {
        return closure;
    }
    let mut prefixes = PrefixClosures::adopt(context, assumptions, semantic_axioms);
    let derived: Rc<[ProofNode]> = closure(
        context,
        integer_type,
        assumptions,
        semantic_axioms,
        &mut prefixes,
    )
    .into();
    prefixes.release();
    definitions.cache_derived_closure(integer_type, derived.clone());
    derived
}

/// Closures derived for prefixes of one axiom roster under one context and
/// assumption roster. A cast image asks for its source's closure over the
/// axioms before the cast, and that source closure asks the same of its own
/// casts, so one derivation visits every cast prefix of its roster; the
/// prefix length and integer type name each closure. Successive obligations
/// of one machine derive under the same context and assumptions over rosters
/// that extend one another, so they share every prefix before the point where
/// their rosters diverge.
///
/// These closures do not go through `DefinitionIndex`'s scope store: that
/// store is a bounded most-recently-used window with fewer slots than a long
/// roster has cast prefixes. Cycling the prefixes through it evicted each one
/// before its reuse, and a scope's own entry while its sources were derived,
/// so every query re-derived every shorter prefix again: exponential in the
/// number of cast prefixes.
struct PrefixClosures {
    context: PropositionContext,
    assumptions: Vec<Proposition>,
    /// Every stored prefix length names a prefix of this roster.
    roster: Vec<Proposition>,
    derived: Vec<(usize, IntegerType, Rc<[ProofNode]>)>,
}

thread_local! {
    /// The prefix closures the most recent outer derivation on this thread
    /// left behind, one scope family at a time.
    static PREFIX_CLOSURES: std::cell::RefCell<Option<PrefixClosures>> =
        const { std::cell::RefCell::new(None) };
}

impl PrefixClosures {
    /// The stored closures still valid for a derivation over `roster`: the
    /// same context and assumptions, restricted to the prefixes both rosters
    /// share. Any other scope starts a fresh family.
    fn adopt(
        context: &PropositionContext,
        assumptions: &[Proposition],
        roster: &[Proposition],
    ) -> Self {
        let previous = PREFIX_CLOSURES.with(|cell| cell.borrow_mut().take());
        match previous {
            Some(mut prefixes)
                if prefixes.assumptions == assumptions && prefixes.context == *context =>
            {
                let shared = prefixes
                    .roster
                    .iter()
                    .zip(roster)
                    .take_while(|(stored, current)| stored == current)
                    .count();
                if shared < roster.len() {
                    prefixes.derived.retain(|(length, _, _)| *length <= shared);
                    prefixes.roster = roster.to_vec();
                }
                prefixes
            }
            _ => Self {
                context: context.clone(),
                assumptions: assumptions.to_vec(),
                roster: roster.to_vec(),
                derived: Vec::new(),
            },
        }
    }

    fn release(self) {
        PREFIX_CLOSURES.with(|cell| *cell.borrow_mut() = Some(self));
    }
}

/// The closure over `prefix`, a prefix of the roster `prefixes` covers,
/// derived at most once per scope family.
fn prefix_closure(
    context: &PropositionContext,
    integer_type: IntegerType,
    assumptions: &[Proposition],
    prefix: &[Proposition],
    prefixes: &mut PrefixClosures,
) -> Rc<[ProofNode]> {
    if let Some((_, _, derived)) = prefixes
        .derived
        .iter()
        .find(|(length, derived_type, _)| *length == prefix.len() && *derived_type == integer_type)
    {
        return derived.clone();
    }
    let derived: Rc<[ProofNode]> =
        closure(context, integer_type, assumptions, prefix, prefixes).into();
    prefixes
        .derived
        .push((prefix.len(), integer_type, derived.clone()));
    derived
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
    prefixes: &mut PrefixClosures,
) -> Vec<ProofNode> {
    let facts = projected_facts(assumptions, semantic_axioms);
    let mut equalities: Vec<ProofNode> = Vec::new();
    let exact = super::exact::session(context, assumptions, semantic_axioms);
    let mut proofs = seeds(context, integer_type, &facts, &mut equalities, &exact);
    // A dependent bound may end at live storage rather than a literal:
    // `coordinate <= limit`, with a separately proved `limit == 2`.
    // Normalize that endpoint through its cited equality chain before the
    // literal-bound closure runs. The order substitution carries both proofs;
    // it does not rewrite the caller's assumptions or invent a field bound.
    for fact in &facts {
        let Proposition::LessOrEqual(left, right) = fact.proposition else {
            continue;
        };
        for (endpoint, term) in [left, right].into_iter().enumerate() {
            if term.integer_value().is_some()
                || term.scalar_type() != ScalarType::Integer(integer_type)
            {
                continue;
            }
            let Some(equality) = exact.literal_equality(term) else {
                continue;
            };
            let Proposition::Equal(_, literal) = &equality.conclusion else {
                continue;
            };
            let conclusion = if endpoint == 0 {
                Proposition::LessOrEqual(literal.clone(), right.clone())
            } else {
                Proposition::LessOrEqual(left.clone(), literal.clone())
            };
            push_new(
                &mut proofs,
                ProofNode {
                    conclusion,
                    rule: ProofRule::IntegerOrderSubstitution {
                        relation: Box::new(fact.proof()),
                        equality: Box::new(equality),
                        endpoint,
                    },
                },
            );
        }
    }
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
    // An exact cast's output carries its source carrier's image: `v as i64`
    // from an `i32` lies within the `i32` endpoints, which a later sum of
    // widened operands needs before its own definition bound applies.
    for bound in cast_image_bounds(
        context,
        integer_type,
        assumptions,
        semantic_axioms,
        prefixes,
    ) {
        push_new(&mut proofs, bound);
    }
    for _ in 0..MAX_ROUNDS {
        let mut discovered = Vec::new();
        for bound in &proofs {
            substitute_equalities(bound, &equalities, &mut discovered);
        }
        for (index, output, expression) in &definitions {
            multiplication::definition_bounds(
                context,
                assumptions.len(),
                semantic_axioms,
                *index,
                output,
                expression,
                &proofs,
                &mut discovered,
            );
            exact_definition_bounds(
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
    exact: &super::exact::Session,
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
    let mut literal_candidates = MAX_PROOFS;
    for term in terms {
        // Closed definitions may introduce a literal absent from the cited
        // roster. Its equality carries every defining premise and a kernel
        // decision, so both endpoints use the ordinary substitution rule.
        let literal_equality = if literal_candidates > 0 && matches!(term, ScalarTerm::Value { .. })
        {
            literal_candidates -= 1;
            exact.literal_equality(term)
        } else {
            None
        };
        if let Some(equality) = literal_equality
            && let Proposition::Equal(_, literal) = &equality.conclusion
        {
            for lower in [true, false] {
                if let Some(proof) =
                    landed_literal_endpoint(integer_type, term, literal, lower, equality.clone())
                {
                    push_new(&mut proofs, proof);
                }
            }
        }
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
fn exact_definition_bounds(
    integer_type: IntegerType,
    definition_axiom: usize,
    output: &ScalarTerm,
    expression: &ScalarTerm,
    proofs: &[ProofNode],
    equalities: &[ProofNode],
    out: &mut Vec<ProofNode>,
) {
    // A subtraction is antitone in its right operand: the output's lower
    // bound spends the subtrahend's upper endpoint.
    let (scalar_type, left, right, subtract) = match expression {
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } => (scalar_type, left, right, false),
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } => (scalar_type, left, right, true),
        _ => return,
    };
    if *scalar_type != integer_type {
        return;
    }
    for lower in [true, false] {
        let right_lower = lower != subtract;
        let left_evidence = operand_evidence(integer_type, left, lower, proofs, equalities);
        let right_evidence = operand_evidence(integer_type, right, right_lower, proofs, equalities);
        for left_bound in &left_evidence {
            for right_bound in &right_evidence {
                // The kernel resolves the mapped side from endpoint
                // orientations; two unoriented members (`Exact`/`Carrier`)
                // carry no direction and the pair must be rejected here
                // rather than emitted as an invalid node.
                if !oriented(left_bound, left, integer_type, lower)
                    && !oriented(right_bound, right, integer_type, right_lower)
                {
                    continue;
                }
                let Some(left_literal) = evidence_literal(left_bound, left, integer_type, lower)
                else {
                    continue;
                };
                let Some(right_literal) =
                    evidence_literal(right_bound, right, integer_type, right_lower)
                else {
                    continue;
                };
                let combined = if subtract {
                    subtract_literals(integer_type, left_literal, right_literal)
                } else {
                    add_literals(integer_type, left_literal, right_literal)
                };
                let Some(combined) = combined else {
                    continue;
                };
                let Ok(literal) = ScalarTerm::integer(integer_type, combined) else {
                    continue;
                };
                let conclusion = if lower {
                    Proposition::LessOrEqual(literal, output.clone())
                } else {
                    Proposition::LessOrEqual(output.clone(), literal)
                };
                let left_bound = Box::new(left_bound.clone());
                let right_bound = Box::new(right_bound.clone());
                out.push(ProofNode {
                    conclusion,
                    rule: if subtract {
                        ProofRule::IntegerExactSubtractDefinitionBound {
                            left_bound,
                            right_bound,
                            definition_axiom,
                        }
                    } else {
                        ProofRule::IntegerExactAddDefinitionBound {
                            left_bound,
                            right_bound,
                            definition_axiom,
                        }
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
        // Bitwise and/or are commutative like an add: either operand may be
        // the chain root while the other lands as the mask literal or the
        // bounded sibling image.
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::IntegerBitwiseOr {
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
        | ScalarTerm::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::WrappingIntegerRemainder {
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
    let bitwise = matches!(
        expression,
        ScalarTerm::IntegerBitwiseAnd { .. } | ScalarTerm::IntegerBitwiseOr { .. }
    );
    for (root, sibling) in pairs {
        if !matches!(root, ScalarTerm::Value { .. }) {
            continue;
        }
        let literal_axiom = if sibling.integer_value().is_some() {
            None
        } else {
            let index = semantic_axioms[..definition_axiom]
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
                });
            match index {
                Some(index) => Some(index),
                // A non-literal bitwise sibling can still admit from its own
                // bound axioms: the kernel resolves them without a cited
                // landing, so emit the candidate and let checking gate it.
                None if bitwise => None,
                None => continue,
            }
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

fn subtract_literals(
    integer_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
) -> Option<IntegerValue> {
    let value = match (left, right) {
        (IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
            IntegerValue::Signed(left.checked_sub(right)?)
        }
        (IntegerValue::Unsigned(left), IntegerValue::Unsigned(right)) => {
            IntegerValue::Unsigned(left.checked_sub(right)?)
        }
        _ => return None,
    };
    integer_type.admits(value).then_some(value)
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

/// Independently proved source endpoints of each exact-cast output.
/// A returned contract may expose its operand expression without an SSA name;
/// use that exact expression, never manufacture a definition to name it.
fn cast_image_bounds(
    context: &PropositionContext,
    integer_type: IntegerType,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    prefixes: &mut PrefixClosures,
) -> Vec<ProofNode> {
    let mut bounds = Vec::new();
    for axiom in semantic_axioms {
        let Proposition::Equal(
            output @ ScalarTerm::Value { .. },
            ScalarTerm::IntegerExactCast { .. } | ScalarTerm::IntegerWiden { .. },
        ) = axiom
        else {
            continue;
        };
        if output.scalar_type() != ScalarType::Integer(integer_type) {
            continue;
        }
        let Some((root, first_cast)) = cast_custody::source_root(output, semantic_axioms) else {
            continue;
        };
        let ScalarType::Integer(root_type) = root.scalar_type() else {
            continue;
        };
        let mut source_bounds = Vec::new();
        for (root_value, lower) in [
            (root_type.minimum_value(), true),
            (root_type.maximum_value(), false),
        ] {
            let Ok(root_literal) = ScalarTerm::integer(root_type, root_value) else {
                continue;
            };
            let root_bound = if lower {
                Proposition::LessOrEqual(root_literal, root.clone())
            } else {
                Proposition::LessOrEqual(root.clone(), root_literal)
            };
            if let Some(proof) = integer_carrier_bound(context, &root_bound) {
                push_new(&mut source_bounds, proof);
            }
        }
        // The earlier ledger prefix excludes the cast's own output facts.
        // Recursive source queries strictly shorten that prefix, so a later
        // conversion cannot provide its own prerequisite bound.
        source_bounds.extend(cast_source_bounds(
            context,
            &root,
            assumptions,
            &semantic_axioms[..first_cast],
            prefixes,
        ));
        for root_bound in source_bounds {
            let Proposition::LessOrEqual(left, right) = &root_bound.conclusion else {
                continue;
            };
            let (endpoint, lower) = if right == &root {
                (left, true)
            } else if left == &root {
                (right, false)
            } else {
                continue;
            };
            let Some(literal) = cast_custody::remap_integer_literal(endpoint, integer_type) else {
                continue;
            };
            let goal = if lower {
                Proposition::LessOrEqual(literal, output.clone())
            } else {
                Proposition::LessOrEqual(output.clone(), literal)
            };
            if let Some(bound) = cast_custody::prove_from_root(
                context,
                &goal,
                assumptions,
                semantic_axioms,
                &root,
                root_bound,
            ) {
                push_new(&mut bounds, bound);
            }
        }
    }
    bounds
}

fn cast_source_bounds(
    context: &PropositionContext,
    source: &ScalarTerm,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    prefixes: &mut PrefixClosures,
) -> Vec<ProofNode> {
    if matches!(source, ScalarTerm::Value { .. })
        && let ScalarType::Integer(integer_type) = source.scalar_type()
    {
        // A stored cast result needs the source's proved arithmetic range,
        // not just its carrier range. The caller cuts the axiom roster before
        // this cast, so recursive source closures cannot use their own result.
        let derived = prefix_closure(
            context,
            integer_type,
            assumptions,
            semantic_axioms,
            prefixes,
        );
        return [false, true]
            .into_iter()
            .flat_map(|lower| endpoints(&derived, source, lower))
            .collect();
    }
    let ScalarTerm::ExactIntegerRemainder {
        scalar_type: integer_type,
        left,
        ..
    } = source
    else {
        return Vec::new();
    };
    let witness = IntegerAffineWitness {
        root: left.as_ref().clone(),
        target: source.clone(),
        definition_axioms: Vec::new(),
        literal_axioms: Vec::new(),
    };
    let Ok(form) = check_integer_affine_witness(context, semantic_axioms, &witness) else {
        return Vec::new();
    };
    let derived = prefix_closure(
        context,
        *integer_type,
        assumptions,
        semantic_axioms,
        prefixes,
    );
    let mut bounds = Vec::new();
    for lower in [true, false] {
        for bound in endpoints(&derived, left, lower) {
            let Ok(conclusion) = map_integer_affine_bound(&form, &bound.conclusion) else {
                continue;
            };
            let proof = ProofNode {
                conclusion,
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(bound),
                    witness: witness.clone(),
                },
            };
            push_new(&mut bounds, proof);
        }
    }
    bounds
}

fn operation_expression(term: &ScalarTerm) -> bool {
    matches!(
        term,
        ScalarTerm::ExactIntegerAdd { .. }
            | ScalarTerm::WrappingIntegerAdd { .. }
            | ScalarTerm::ExactIntegerSubtract { .. }
            | ScalarTerm::ExactIntegerMultiply { .. }
            | ScalarTerm::WrappingIntegerMultiply { .. }
            | ScalarTerm::ExactIntegerDivide { .. }
            | ScalarTerm::WrappingIntegerDivide { .. }
            | ScalarTerm::ExactIntegerRemainder { .. }
            | ScalarTerm::WrappingIntegerRemainder { .. }
            | ScalarTerm::ExactIntegerShiftLeft { .. }
            | ScalarTerm::ExactIntegerShiftRight { .. }
            | ScalarTerm::IntegerBitwiseAnd { .. }
            | ScalarTerm::IntegerBitwiseOr { .. }
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
