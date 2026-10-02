//! Canonical bound transport through fixed-integer policy translations.
//!
//! Saturating addition and subtraction map inclusive endpoints through the
//! carrier clamp. Unlike modular arithmetic they are monotone everywhere, but
//! a clamped plateau does not preserve strict order or permit inversion. The
//! shared definition walk asks the kernel to replay that distinction for both
//! SSA definitions and arithmetic written directly in a postcondition.
//!
//! `WrappingIntegerAdd`/`WrappingIntegerSubtract`/`WrappingIntegerDivide`
//! rows are affine in shape but modular in semantics. Unsigned wrapping
//! division maps bounds exactly; wrapping addition maps the bound on the side
//! its addend cannot cross unconditionally, while the other side is sound
//! only when the same definition supplies the checked no-wrap conjunct
//! (`operand <= maximum - addend` for a positive addend, `minimum - addend <=
//! operand` for a negative one). Subtracting a literal is adding its negation,
//! so an unsigned `operand - c` needs `c <= operand` for its upper side.
//! Traversed toward its operand the equation inverts those directions. A
//! guarantee may also spell the last step in place (`self.length - 1`); the
//! kernel reads that step from the target term itself. The proof kernel
//! independently reconstructs every requirement at admission, so this leg
//! only assembles the candidate root-bound conjunction from cited facts.

use std::cell::RefCell;
use std::rc::Rc;

use proof_admission::{
    IntegerAffineWitness, ProofNode, ProofRule, check_certificate, check_integer_affine_witness,
    integer_affine_wrapping_evidence, map_integer_affine_bound,
};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ScalarType};

use super::super::affine_custody::{self, CheckedWord, DefinitionIndex};
use super::super::integer_evidence::{integer_carrier_bound, projected_facts, relax};
use super::super::scope_identity::{self, ScopeDerived};
use super::{bound, exact, range};

#[cfg(test)]
mod tests;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    if let Some(proof) = definitions.cached_policy_translation_proof(goal) {
        return proof;
    }
    definitions.begin_policy_translation_proof(goal);
    let proof = prove_uncached(context, goal, assumptions, semantic_axioms, definitions);
    definitions.cache_policy_translation_proof(goal, proof.clone());
    proof
}

/// One order bound standing on a value term: either cited on the value itself
/// or transported there through a cited equality. A guard fact can name a
/// storage observation (`self.counter < 3`) while the definition chain roots
/// at the stored value; the checked endpoint substitution keeps the bound
/// exact instead of inventing a second citation.
pub(super) struct RootedBound {
    /// The bound proposition restated on `root`.
    pub(super) proposition: Proposition,
    pub(super) proof: ProofNode,
    pub(super) root: ScalarTerm,
}

pub(super) fn rooted_bounds(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Vec<RootedBound> {
    // Every endpoint below resolves its aliases against this same unchanged
    // roster; one session keeps each lookup proportional to the endpoint's
    // own incident equalities instead of rescanning the roster per endpoint.
    let session = exact::session(context, assumptions, semantic_axioms);
    let mut bounds = Vec::new();
    for fact in projected_facts(assumptions, semantic_axioms) {
        let (fact_left, fact_right) = match fact.proposition {
            Proposition::LessThan(left, right) | Proposition::LessOrEqual(left, right) => {
                (left, right)
            }
            _ => continue,
        };
        for (endpoint, position) in [(fact_left, 0usize), (fact_right, 1)] {
            if matches!(
                endpoint,
                ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_)
            ) {
                continue;
            }
            for (root, equality) in value_aliases(&session, endpoint) {
                let (proposition, proof) = if root == *endpoint {
                    (fact.proposition.clone(), fact.proof())
                } else {
                    let (left, right) = if position == 0 {
                        (root.clone(), fact_right.clone())
                    } else {
                        (fact_left.clone(), root.clone())
                    };
                    let proposition = match fact.proposition {
                        Proposition::LessThan(..) => Proposition::LessThan(left, right),
                        _ => Proposition::LessOrEqual(left, right),
                    };
                    (
                        proposition.clone(),
                        ProofNode {
                            conclusion: proposition,
                            rule: ProofRule::IntegerOrderSubstitution {
                                relation: Box::new(fact.proof()),
                                equality: Box::new(equality),
                                endpoint: position,
                            },
                        },
                    )
                };
                bounds.push(RootedBound {
                    proposition,
                    proof,
                    root,
                });
            }
        }
    }
    bounds
}

/// The value terms one endpoint can stand on through cited equalities: itself
/// when it already is a value, plus every cited `endpoint == value` or
/// `value == endpoint` partner. Each alias carries the equality certificate the
/// endpoint substitution must cite.
fn value_aliases(session: &exact::Session, endpoint: &ScalarTerm) -> Vec<(ScalarTerm, ProofNode)> {
    let mut aliases = session.value_aliases(endpoint);
    if matches!(endpoint, ScalarTerm::Value { .. })
        && !aliases.iter().any(|(root, _)| root == endpoint)
    {
        aliases.push((
            endpoint.clone(),
            ProofNode {
                conclusion: Proposition::Equal(endpoint.clone(), endpoint.clone()),
                rule: ProofRule::Primitive(proof_admission::PrimitiveJudgment::ReflexiveEquality),
            },
        ));
    }
    aliases
}

fn prove_uncached(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let (goal_left, goal_right, strict) = match goal {
        Proposition::LessThan(left, right) => (left, right, true),
        Proposition::LessOrEqual(left, right) => (left, right, false),
        _ => return None,
    };
    if let Some(proof) = prove_value_goal(context, goal, assumptions, semantic_axioms, definitions)
    {
        return Some(proof);
    }
    // A goal can name a storage observation (`0 <= self.i`) whose stored value
    // is the policy sum. Ask the same question of each value alias, then
    // substitute the observation back through the cited equality.
    let session = exact::session(context, assumptions, semantic_axioms);
    for (position, endpoint) in [(0usize, goal_left), (1, goal_right)] {
        if matches!(
            endpoint,
            ScalarTerm::Value { .. } | ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_)
        ) {
            continue;
        }
        for (alias, equality) in session.value_aliases(endpoint) {
            let (left, right) = if position == 0 {
                (alias, goal_right.clone())
            } else {
                (goal_left.clone(), alias)
            };
            let aliased = if strict {
                Proposition::LessThan(left, right)
            } else {
                Proposition::LessOrEqual(left, right)
            };
            let Some(relation) =
                prove_value_goal(context, &aliased, assumptions, semantic_axioms, definitions)
            else {
                continue;
            };
            let proof = ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::IntegerOrderSubstitution {
                    relation: Box::new(relation),
                    equality: Box::new(equality),
                    endpoint: position,
                },
            };
            if check_certificate(context, goal, assumptions, semantic_axioms, &proof).is_ok() {
                return Some(proof);
            }
        }
    }
    prove_expression_goal(context, goal, assumptions, semantic_axioms, definitions)
}

fn prove_value_goal(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    // Operation-semantic bounds are not cited in the source roster: a
    // remainder's `value < divisor` range exists only through its own
    // definition. Seed the same transport with each definition target's
    // synthesized bounds so chains like `48 + (x % 10) <= N` carry the
    // operand's derived range to the goal.
    let (goal_left, goal_right) = match goal {
        Proposition::LessThan(left, right) | Proposition::LessOrEqual(left, right) => (left, right),
        _ => return None,
    };
    // Both rosters are pure functions of the scope (the synthesized one of
    // the context and axioms alone), asked again at every goal under it.
    let rooted = shared_rooted_bounds(context, assumptions, semantic_axioms);
    let synthesized = shared_synthesized_operand_bounds(context, semantic_axioms);
    // Relative translation starts from an ordinary reflexive order, not a
    // guessed bound or the ranking goal. The same checked word determines
    // the displacement and demands every wrapping step's no-wrap premise.
    let relative = [goal_left, goal_right].map(|root| reflexive_root(context, root));
    for bound in relative
        .iter()
        .flatten()
        .chain(rooted.iter())
        .chain(synthesized.iter())
    {
        for target in [goal_left, goal_right] {
            if bound.root == *target || !matches!(target, ScalarTerm::Value { .. }) {
                continue;
            }
            let words = affine_custody::definition_words_to_target(
                context,
                semantic_axioms,
                definitions,
                &bound.root,
                target,
            );
            for word in words.iter() {
                if let Some(proof) = prove_word(
                    context,
                    goal,
                    assumptions,
                    semantic_axioms,
                    definitions,
                    bound,
                    target,
                    word,
                ) {
                    return Some(proof);
                }
            }
        }
    }
    // A wrapping sum's bound on the side its addend faces needs a root bound
    // on that same side, and an operand's carrier interval (`0 <= x` for an
    // unsigned counter) is one no fact cites. Seed it only from the values
    // the target's own defining rows read, so the search stays inside the
    // question being asked.
    for target in [goal_left, goal_right] {
        if !matches!(target, ScalarTerm::Value { .. }) {
            continue;
        }
        for bound in carrier_roots(context, semantic_axioms, target) {
            let words = affine_custody::definition_words_to_target(
                context,
                semantic_axioms,
                definitions,
                &bound.root,
                target,
            );
            for word in words.iter() {
                if let Some(proof) = prove_word(
                    context,
                    goal,
                    assumptions,
                    semantic_axioms,
                    definitions,
                    &bound,
                    target,
                    word,
                ) {
                    return Some(proof);
                }
            }
        }
    }
    None
}

/// A typed scalar root's self-order is the neutral premise for relative
/// additive transport. It establishes no displacement and grants no overflow
/// facts; the receiving affine mapper authenticates both independently.
fn reflexive_root(context: &PropositionContext, root: &ScalarTerm) -> Option<RootedBound> {
    if !matches!(
        root,
        ScalarTerm::Value {
            scalar_type: ScalarType::Integer(_),
            ..
        }
    ) {
        return None;
    }
    let proposition = Proposition::LessOrEqual(root.clone(), root.clone());
    let judgment = proof_admission::PrimitiveJudgment::ClosedIntegerRelation;
    proof_admission::decide_primitive(context, &proposition, judgment).ok()?;
    Some(RootedBound {
        proposition: proposition.clone(),
        proof: ProofNode {
            conclusion: proposition,
            rule: ProofRule::Primitive(judgment),
        },
        root: root.clone(),
    })
}

/// Most values a carrier-root walk visits below one goal endpoint.
const MAX_CARRIER_ROOTS: usize = 16;

/// Carrier bounds on the values `target`'s defining rows read, transitively:
/// `minimum <= x` and `x <= maximum` for each such fixed-integer value. The
/// kernel decides each bound as a primitive carrier judgment; the walk only
/// chooses which roots the transport may start from.
fn carrier_roots(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    target: &ScalarTerm,
) -> Vec<RootedBound> {
    let mut values: Vec<ScalarTerm> = Vec::new();
    let mut pending = vec![target.clone()];
    while let Some(value) = pending.pop() {
        for axiom in semantic_axioms {
            let Proposition::Equal(left, right) = axiom else {
                continue;
            };
            for (defined, expression) in [(left, right), (right, left)] {
                if *defined != value {
                    continue;
                }
                for input in defining_inputs(expression) {
                    if values.len() >= MAX_CARRIER_ROOTS {
                        break;
                    }
                    if input != target && !values.contains(input) {
                        values.push(input.clone());
                        pending.push(input.clone());
                    }
                }
            }
        }
    }
    let mut bounds = Vec::new();
    for value in values {
        let ScalarType::Integer(integer_type) = value.scalar_type() else {
            continue;
        };
        for (literal, lower) in [
            (integer_type.minimum_value(), true),
            (integer_type.maximum_value(), false),
        ] {
            let Ok(literal) = ScalarTerm::integer(integer_type, literal) else {
                continue;
            };
            let proposition = if lower {
                Proposition::LessOrEqual(literal, value.clone())
            } else {
                Proposition::LessOrEqual(value.clone(), literal)
            };
            if let Some(proof) = integer_carrier_bound(context, &proposition) {
                bounds.push(RootedBound {
                    proposition,
                    proof,
                    root: value.clone(),
                });
            }
        }
    }
    bounds
}

/// The value operands a definition chain can resume from, as the frontier
/// traverses them: every affine operand, and the minuend or dividend of a
/// subtraction or division.
fn defining_inputs(expression: &ScalarTerm) -> impl Iterator<Item = &ScalarTerm> {
    let inputs = match expression {
        ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerMultiply { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. } => {
            [Some(left.as_ref()), Some(right.as_ref())]
        }
        ScalarTerm::ExactIntegerSubtract { left, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, .. }
        | ScalarTerm::ExactIntegerDivide { left, .. }
        | ScalarTerm::WrappingIntegerDivide { left, .. } => [Some(left.as_ref()), None],
        _ => [None, None],
    };
    inputs
        .into_iter()
        .flatten()
        .filter(|input| matches!(input, ScalarTerm::Value { .. }))
}

/// A guarantee can name policy arithmetic over storage in place
/// (`self.length - 1 <= 3`) rather than through an SSA value. Stand the
/// expression's operand on a value (itself, or a value a cited equality
/// aliases it to), then map a root bound through the word to that value
/// and the expression's own step, which the kernel reads from the target
/// term. A storage operand rejoins the goal through the same cited
/// equality by value transport.
fn prove_expression_goal(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let (goal_left, goal_right, strict) = match goal {
        Proposition::LessThan(left, right) => (left, right, true),
        Proposition::LessOrEqual(left, right) => (left, right, false),
        _ => return None,
    };
    let session = exact::session(context, assumptions, semantic_axioms);
    for (position, endpoint) in [(0usize, goal_left), (1, goal_right)] {
        let Some(operand) = policy_expression_operand(endpoint) else {
            continue;
        };
        let mut stands = Vec::new();
        if matches!(operand, ScalarTerm::Value { .. }) {
            stands.push((operand.clone(), None));
        } else {
            stands.extend(
                session
                    .value_aliases(operand)
                    .into_iter()
                    .map(|(alias, equality)| (alias, Some(equality))),
            );
        }
        for (value, equality) in stands {
            let target = with_expression_operand(endpoint, &value);
            let (left, right) = if position == 0 {
                (target.clone(), goal_right.clone())
            } else {
                (goal_left.clone(), target.clone())
            };
            let rewritten = if strict {
                Proposition::LessThan(left, right)
            } else {
                Proposition::LessOrEqual(left, right)
            };
            let Some(relation) = prove_expression_target(
                context,
                &rewritten,
                assumptions,
                semantic_axioms,
                definitions,
                &value,
                &target,
            ) else {
                continue;
            };
            let proof = match equality {
                None => relation,
                Some(equality) => ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::ValueEqualityTransport {
                        premise: Box::new(relation),
                        equalities: vec![equality],
                    },
                },
            };
            if check_certificate(context, goal, assumptions, semantic_axioms, &proof).is_ok() {
                return Some(proof);
            }
        }
    }
    None
}

/// The operand of a policy sum or difference with one literal side.
fn policy_expression_operand(term: &ScalarTerm) -> Option<&ScalarTerm> {
    match term {
        ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. } => {
            if right.integer_value().is_some() {
                Some(left)
            } else if left.integer_value().is_some() {
                Some(right)
            } else {
                None
            }
        }
        ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. } => {
            right.integer_value().is_some().then_some(left.as_ref())
        }
        _ => None,
    }
}

/// `term` with its non-literal operand replaced by `value`.
fn with_expression_operand(term: &ScalarTerm, value: &ScalarTerm) -> ScalarTerm {
    let mut rewritten = term.clone();
    match &mut rewritten {
        ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. } => {
            if right.integer_value().is_some() {
                **left = value.clone();
            } else {
                **right = value.clone();
            }
        }
        ScalarTerm::WrappingIntegerSubtract { left, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, .. } => **left = value.clone(),
        _ => {}
    }
    rewritten
}

/// Prove a goal on `target`, one policy step over `value`, from any root
/// bound whose word reaches `value`: a cited or synthesized root, the
/// carrier interval of a value the chain reads, or a bound on `value`
/// itself with the empty word.
fn prove_expression_target(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    value: &ScalarTerm,
    target: &ScalarTerm,
) -> Option<ProofNode> {
    let rooted = shared_rooted_bounds(context, assumptions, semantic_axioms);
    let synthesized = shared_synthesized_operand_bounds(context, semantic_axioms);
    let carriers = carrier_roots(context, semantic_axioms, value);
    let relative = reflexive_root(context, value);
    for bound in relative
        .iter()
        .chain(rooted.iter())
        .chain(synthesized.iter())
        .chain(carriers.iter())
    {
        let words: Rc<[Vec<usize>]> = if bound.root == *value {
            Rc::from([Vec::new()])
        } else {
            affine_custody::definition_words_to_target(
                context,
                semantic_axioms,
                definitions,
                &bound.root,
                value,
            )
        };
        for word in words.iter() {
            let Some(literal_axioms) = affine_custody::literal_axioms(
                context,
                semantic_axioms,
                definitions,
                &bound.root,
                word,
                value,
            ) else {
                continue;
            };
            let witness = IntegerAffineWitness {
                root: bound.root.clone(),
                target: target.clone(),
                literal_axioms,
                definition_axioms: word.clone(),
            };
            let Ok(form) = check_integer_affine_witness(context, semantic_axioms, &witness) else {
                continue;
            };
            let Ok(evidence) = integer_affine_wrapping_evidence(&form, &bound.proposition) else {
                continue;
            };
            let checked = CheckedWord {
                form,
                evidence,
                literal_axioms: witness.literal_axioms,
            };
            // The step's own no-wrap evidence (`1 <= x` before `x - 1`) is a
            // bound on a wrapping sum in turn, so it may need this leg too.
            let Some(proof) = map_checked_word_with(
                context,
                assumptions,
                semantic_axioms,
                definitions,
                bound,
                &checked,
                target,
                word,
                |context, required, assumptions, semantic_axioms, definitions| {
                    bound::prove_candidate_endpoint(
                        context,
                        required,
                        assumptions,
                        semantic_axioms,
                        definitions,
                    )
                    .or_else(|| prove(context, required, assumptions, semantic_axioms, definitions))
                },
            ) else {
                continue;
            };
            let Some(proof) = (if proof.conclusion == *goal {
                Some(proof)
            } else {
                relax(goal, proof)
            }) else {
                continue;
            };
            if check_certificate(context, goal, assumptions, semantic_axioms, &proof).is_ok() {
                return Some(proof);
            }
        }
    }
    None
}

/// `rooted_bounds` shared across the goals asked under one scope. The
/// session is still resolved on every ask, exactly as the uncached walk
/// resolves it first, so the roster cache sees the same sequence of scopes.
fn shared_rooted_bounds(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Rc<Vec<RootedBound>> {
    thread_local! {
        static ROOTED: RefCell<ScopeDerived<Vec<RootedBound>>> =
            const { RefCell::new(ScopeDerived::new()) };
    }
    drop(exact::session(context, assumptions, semantic_axioms));
    scope_identity::derived_in_context(&ROOTED, context, assumptions, semantic_axioms, || {
        rooted_bounds(context, assumptions, semantic_axioms)
    })
}

/// `synthesized_operand_bounds` shared across every scope with these axioms.
fn shared_synthesized_operand_bounds(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
) -> Rc<Vec<RootedBound>> {
    thread_local! {
        static SYNTHESIZED: RefCell<ScopeDerived<Vec<RootedBound>>> =
            const { RefCell::new(ScopeDerived::new()) };
    }
    scope_identity::derived_in_context(&SYNTHESIZED, context, &[], semantic_axioms, || {
        synthesized_operand_bounds(context, semantic_axioms)
    })
}

/// Bounds each definition target earns from its operation semantics alone:
/// `x % 10` lands in `[0, 9]` without any cited relation. The transport
/// treats these like cited roots so policy chains carry them onward.
fn synthesized_operand_bounds(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
) -> Vec<RootedBound> {
    let mut bounds = Vec::new();
    for proposition in semantic_axioms {
        let Proposition::Equal(left, right) = proposition else {
            continue;
        };
        for candidate in [left, right] {
            if !matches!(candidate, ScalarTerm::Value { .. }) {
                continue;
            }
            for proof in range::target_bounds(context, candidate, semantic_axioms) {
                bounds.push(RootedBound {
                    proposition: proof.conclusion.clone(),
                    proof,
                    root: candidate.clone(),
                });
            }
        }
    }
    bounds
}

#[allow(clippy::too_many_arguments)]
fn prove_word(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    bound: &RootedBound,
    target: &ScalarTerm,
    definition_axioms: &[usize],
) -> Option<ProofNode> {
    let proof = map_word(
        context,
        assumptions,
        semantic_axioms,
        definitions,
        bound,
        target,
        definition_axioms,
    )?;
    if proof.conclusion != *goal {
        // A mapped bound like `v <= 57` closes the goal `v <= 255` through
        // transitivity with a closed literal relation.
        let relaxed = relax(goal, proof)?;
        return check_certificate(context, goal, assumptions, semantic_axioms, &relaxed)
            .is_ok()
            .then_some(relaxed);
    }
    check_certificate(context, goal, assumptions, semantic_axioms, &proof)
        .is_ok()
        .then_some(proof)
}

/// Map `bound` through `definition_axioms` to `target`, returning the
/// transport proof whose conclusion is the mapped relation. The goal check
/// stays with the caller so contradiction search can reuse one mapped bound
/// that no `prove` goal names.
#[allow(clippy::too_many_arguments)]
pub(super) fn map_word(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    bound: &RootedBound,
    target: &ScalarTerm,
    definition_axioms: &[usize],
) -> Option<ProofNode> {
    let checked = check_word(
        context,
        semantic_axioms,
        definitions,
        bound,
        target,
        definition_axioms,
    )?;
    if checked.evidence.is_empty()
        && !matches!(&bound.proposition, Proposition::LessOrEqual(left, right) if left == right)
        && !matches!(bound.proposition, Proposition::LessThan(_, _))
        && !definition_axioms.iter().any(|&index| {
            let Proposition::Equal(left, right) = &semantic_axioms[index] else {
                return false;
            };
            [left, right].iter().any(|term| is_policy_term(term))
        })
    {
        // A bare non-strict relation is already the ordinary affine path's
        // root bound. This leg exists for strict roots and for the wrapping
        // evidence conjunction the ordinary path cannot construct. A word
        // containing a policy definition has no affine coverage either:
        // its unconditional transports (an operand's derived range into the
        // target's upper bound) land only through this leg.
        return None;
    }
    map_checked_word(
        context,
        assumptions,
        semantic_axioms,
        definitions,
        bound,
        &checked,
        target,
        definition_axioms,
    )
}

/// The contradiction leg pool has no goal-directed ordinary path that already
/// produces these relations: a carrier root such as `0 <= counter` mapped
/// through `next = counter + 1` is exactly the `1 <= next` leg the strict
/// premise contradicts. Map every cited bound here; the de-duplication gate
/// above only belongs to the goal-checking caller.
#[allow(clippy::too_many_arguments)]
pub(super) fn map_derived_word(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    bound: &RootedBound,
    target: &ScalarTerm,
    definition_axioms: &[usize],
) -> Option<ProofNode> {
    let checked = check_word(
        context,
        semantic_axioms,
        definitions,
        bound,
        target,
        definition_axioms,
    )?;
    map_checked_word(
        context,
        assumptions,
        semantic_axioms,
        definitions,
        bound,
        &checked,
        target,
        definition_axioms,
    )
}

/// The kernel witness check for one `(bound, root, target, word)`, memoized
/// in the definition index: both the goal-directed path and the
/// contradiction leg pool meet the same candidates, so the check runs once
/// per word under this scope and each caller still assembles its own proof.
#[allow(clippy::too_many_arguments)]
fn check_word(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    bound: &RootedBound,
    target: &ScalarTerm,
    definition_axioms: &[usize],
) -> Option<Rc<CheckedWord>> {
    if let Some(checked) =
        definitions.cached_checked_word(&bound.proposition, &bound.root, target, definition_axioms)
    {
        return checked;
    }
    let checked = (|definitions: &mut DefinitionIndex| {
        let literal_axioms = affine_custody::literal_axioms(
            context,
            semantic_axioms,
            definitions,
            &bound.root,
            definition_axioms,
            target,
        )?;
        let witness = IntegerAffineWitness {
            root: bound.root.clone(),
            target: target.clone(),
            literal_axioms,
            definition_axioms: definition_axioms.to_vec(),
        };
        let form = check_integer_affine_witness(context, semantic_axioms, &witness).ok()?;
        let evidence = integer_affine_wrapping_evidence(&form, &bound.proposition).ok()?;
        Some(Rc::new(CheckedWord {
            form,
            evidence,
            literal_axioms: witness.literal_axioms,
        }))
    })(definitions);
    definitions.cache_checked_word(
        &bound.proposition,
        &bound.root,
        target,
        definition_axioms,
        checked.clone(),
    );
    checked
}

#[allow(clippy::too_many_arguments)]
fn map_checked_word(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    bound: &RootedBound,
    checked: &CheckedWord,
    target: &ScalarTerm,
    definition_axioms: &[usize],
) -> Option<ProofNode> {
    map_checked_word_with(
        context,
        assumptions,
        semantic_axioms,
        definitions,
        bound,
        checked,
        target,
        definition_axioms,
        bound::prove_candidate_endpoint,
    )
}

/// `map_checked_word` with the caller's producer for each no-wrap evidence
/// conjunct; the kernel rechecks every conjunct either way.
#[allow(clippy::too_many_arguments)]
fn map_checked_word_with(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    bound: &RootedBound,
    checked: &CheckedWord,
    target: &ScalarTerm,
    definition_axioms: &[usize],
    mut prove_evidence: impl FnMut(
        &PropositionContext,
        &Proposition,
        &[Proposition],
        &[Proposition],
        &mut DefinitionIndex,
    ) -> Option<ProofNode>,
) -> Option<ProofNode> {
    let witness = IntegerAffineWitness {
        root: bound.root.clone(),
        target: target.clone(),
        literal_axioms: checked.literal_axioms.clone(),
        definition_axioms: definition_axioms.to_vec(),
    };
    let mut children = Vec::with_capacity(checked.evidence.len() + 1);
    children.push(bound.proof.clone());
    for required in &checked.evidence {
        children.push(prove_evidence(
            context,
            required,
            assumptions,
            semantic_axioms,
            definitions,
        )?);
    }
    let root_bound = if children.len() == 1 {
        children
            .pop()
            .unwrap_or_else(|| unreachable!("one cited relation"))
    } else {
        ProofNode {
            conclusion: Proposition::Conjunction(
                children
                    .iter()
                    .map(|child| child.conclusion.clone())
                    .collect(),
            ),
            rule: ProofRule::ConjunctionIntroduction(children),
        }
    };
    let mapped = map_integer_affine_bound(&checked.form, &root_bound.conclusion).ok()?;
    Some(ProofNode {
        conclusion: mapped,
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(root_bound),
            witness,
        },
    })
}

fn is_policy_term(term: &ScalarTerm) -> bool {
    matches!(
        term,
        ScalarTerm::WrappingIntegerAdd { .. }
            | ScalarTerm::SaturatingIntegerAdd { .. }
            | ScalarTerm::WrappingIntegerSubtract { .. }
            | ScalarTerm::SaturatingIntegerSubtract { .. }
            | ScalarTerm::WrappingIntegerMultiply { .. }
            | ScalarTerm::WrappingIntegerDivide { .. }
            | ScalarTerm::WrappingIntegerRemainder { .. }
            | ScalarTerm::WrappingIntegerShiftLeft { .. }
            | ScalarTerm::WrappingIntegerShiftRight { .. }
    )
}
