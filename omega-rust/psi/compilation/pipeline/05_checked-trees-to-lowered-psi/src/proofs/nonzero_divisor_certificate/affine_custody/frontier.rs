//! Fixed affine-witness candidate frontier for certificate production.
//!
//! The frontier grows definition words from one root, one source-ordered
//! layer per definition, and keeps only prefixes the kernel's witness check
//! replays successfully. Each word step resumes the chain at a definition's
//! value endpoint, or at a value addend when an add row is traversed toward
//! its operand, and cites the step's sibling either as an inline literal or
//! as the unique earlier row that lands it on one.

use std::rc::Rc;

use proof_admission::IntegerAffineWitness;
use semantic_vocabulary::{
    IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm, ScalarType,
};

use super::DefinitionIndex;

const MAX_DEFINITIONS: usize = 14;

fn definition_words(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
) -> Rc<[Vec<usize>]> {
    if let Some(words) = definitions.cached_words(root) {
        return words;
    }

    // This is candidate pruning, not proof authority: only prefixes replayed
    // successfully by the kernel advance, and the completed proof is checked
    // again before it leaves affine custody.
    let mut words = Vec::new();
    let mut frontier = vec![Entry {
        word: Vec::new(),
        start: 0,
        current: root.clone(),
    }];
    for depth in 0..MAX_DEFINITIONS {
        frontier = expand(
            context,
            semantic_axioms,
            definitions,
            root,
            frontier,
            &mut words,
            depth + 1 < MAX_DEFINITIONS,
        );
        if frontier.is_empty() {
            break;
        }
    }
    let words = Rc::from(words);
    definitions.cache_words(root, Rc::clone(&words));
    words
}

pub(in crate::proofs::nonzero_divisor_certificate) fn definition_words_to_target(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    target: &ScalarTerm,
) -> Rc<[Vec<usize>]> {
    if let Some(words) = definitions.cached_words_to_target(root, target) {
        return words;
    }
    let words = definition_words(context, semantic_axioms, definitions, root)
        .iter()
        .filter(|word| {
            word.last().is_some_and(|&index| {
                let Proposition::Equal(left, right) = &semantic_axioms[index] else {
                    unreachable!("definition words contain only equality rows")
                };
                // A word can end at an endpoint or, traversing a wrapping
                // add or an exact add backward, at one of its operand values.
                left == target
                    || right == target
                    || add_operand(left, target)
                    || add_operand(right, target)
            })
        })
        .cloned()
        .collect::<Rc<[Vec<usize>]>>();
    definitions.cache_words_to_target(root, target, Rc::clone(&words));
    words
}

/// `definition_words_to_target` for each of `targets` from one root, in
/// target order; a target equal to the root is not asked and reads empty.
pub(in crate::proofs::nonzero_divisor_certificate) fn definition_words_to_targets(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    targets: &[ScalarTerm],
) -> Vec<Rc<[Vec<usize>]>> {
    let known = definitions.cached_words_to_targets(root, targets);
    targets
        .iter()
        .zip(known)
        .map(|(target, known)| match known {
            Some(words) => words,
            None if target == root => Rc::from([]),
            None => definition_words_to_target(context, semantic_axioms, definitions, root, target),
        })
        .collect()
}

fn add_operand(side: &ScalarTerm, target: &ScalarTerm) -> bool {
    let (ScalarTerm::WrappingIntegerAdd { left, right, .. }
    | ScalarTerm::ExactIntegerAdd { left, right, .. }) = side
    else {
        return false;
    };
    left.as_ref() == target || right.as_ref() == target
}

pub(in crate::proofs::nonzero_divisor_certificate) fn literal_axioms(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    definition_axioms: &[usize],
    target: &ScalarTerm,
) -> Option<Vec<Option<usize>>> {
    if let Some(literal_axioms) = definitions.cached_literal_axioms(root, definition_axioms, target)
    {
        return literal_axioms;
    }
    let literal_axioms =
        select_literal_axioms(context, semantic_axioms, root, definition_axioms, target);
    definitions.cache_literal_axioms(root, definition_axioms, target, literal_axioms.clone());
    literal_axioms
}

/// One frontier cursor: the word so far, the first row a successor may cite,
/// and the value the chain has reached.
struct Entry {
    word: Vec<usize>,
    start: usize,
    current: ScalarTerm,
}

/// Extend every entry by each later candidate row whose prefix the kernel
/// replays, recording every such word and, unless this is the last layer,
/// keeping it as a successor entry.
fn expand(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    frontier: Vec<Entry>,
    words: &mut Vec<Vec<usize>>,
    retain_successors: bool,
) -> Vec<Entry> {
    let mut next = Vec::new();
    for entry in frontier {
        let extensions = definitions
            .candidates_from(&entry.current, entry.start)
            .map(|index| {
                let mut word = entry.word.clone();
                word.push(index);
                (index, word)
            })
            .collect::<Vec<_>>();
        for (index, word) in extensions {
            if let Some(next_target) = checked_target(
                context,
                semantic_axioms,
                definitions,
                root,
                &word,
                &semantic_axioms[index],
            ) {
                if retain_successors {
                    words.push(word.clone());
                    next.push(Entry {
                        word,
                        start: index + 1,
                        current: next_target.clone(),
                    });
                } else {
                    words.push(word);
                }
            }
        }
    }
    next
}

/// The first value `definition` can resume the chain at whose completed
/// prefix passes the kernel's witness check.
fn checked_target<'a>(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    definition_axioms: &[usize],
    definition: &'a Proposition,
) -> Option<&'a ScalarTerm> {
    resume_values(definition).find(|target| {
        let Some(literal_axioms) = literal_axioms(
            context,
            semantic_axioms,
            definitions,
            root,
            definition_axioms,
            target,
        ) else {
            return false;
        };
        definitions
            .affine_form(
                context,
                semantic_axioms,
                &IntegerAffineWitness {
                    root: root.clone(),
                    target: (*target).clone(),
                    literal_axioms,
                    definition_axioms: definition_axioms.to_vec(),
                },
            )
            .is_some()
    })
}

/// The values a definition can resume a chain at: its `Value` endpoints cover
/// the forward direction, and each `Value` addend of an add side covers the
/// checked backward direction toward that operand.
fn resume_values(definition: &Proposition) -> impl Iterator<Item = &ScalarTerm> {
    let endpoints = match definition {
        Proposition::Equal(left, right) => [left, right],
        _ => unreachable!("definition index contains only equality rows"),
    };
    endpoints.into_iter().flat_map(|side| {
        let mut targets: Vec<&ScalarTerm> = Vec::with_capacity(3);
        if matches!(side, ScalarTerm::Value { .. }) {
            targets.push(side);
        }
        if let ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerAdd { left, right, .. } = side
        {
            for operand in [left.as_ref(), right.as_ref()] {
                if matches!(operand, ScalarTerm::Value { .. }) {
                    targets.push(operand);
                }
            }
        }
        targets
    })
}

/// Walk `definition_axioms` from `root` and choose, per step, the one
/// resolvable reading and its landed-sibling citation; the walk must end at
/// `target`.
fn select_literal_axioms(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    definition_axioms: &[usize],
    target: &ScalarTerm,
) -> Option<Vec<Option<usize>>> {
    let ScalarType::Integer(integer_type) = root.scalar_type() else {
        return None;
    };
    let mut current = root.clone();
    let mut literal_axioms = Vec::with_capacity(definition_axioms.len());
    for &definition_index in definition_axioms {
        let definition = semantic_axioms.get(definition_index)?;
        context.validate(definition).ok()?;
        let Proposition::Equal(left, right) = definition else {
            return None;
        };
        let mut selected = None;
        for (endpoint, expression) in [(left, right), (right, left)] {
            for (next, sibling) in step_readings(
                endpoint,
                expression,
                &current,
                ScalarType::Integer(integer_type),
            ) {
                let literal_axiom = match sibling.integer_value() {
                    Some((actual, IntegerValue::Signed(_) | IntegerValue::Unsigned(_)))
                        if actual == integer_type =>
                    {
                        Some(None)
                    }
                    None if matches!(sibling, ScalarTerm::Value { .. }) => unique_landing(
                        context,
                        semantic_axioms,
                        definition_index,
                        sibling,
                        integer_type,
                    )
                    .map(Some),
                    _ => None,
                };
                let Some(literal_axiom) = literal_axiom else {
                    continue;
                };
                // More than one resolvable traversal forks the chain the same
                // way the kernel rejects an ambiguous definition.
                if selected.replace((next.clone(), literal_axiom)).is_some() {
                    return None;
                }
            }
        }
        let (next, literal_axiom) = selected?;
        literal_axioms.push(literal_axiom);
        current = next;
    }
    (current == *target).then_some(literal_axioms)
}

/// The unique earlier row that lands `sibling` on an integer literal of the
/// chain's carrier.
fn unique_landing(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definition_index: usize,
    sibling: &ScalarTerm,
    integer_type: IntegerType,
) -> Option<usize> {
    let mut matches = semantic_axioms[..definition_index]
        .iter()
        .enumerate()
        .filter_map(|(index, proposition)| {
            context.validate(proposition).ok()?;
            let Proposition::Equal(left, right) = proposition else {
                return None;
            };
            [(left, right), (right, left)]
                .into_iter()
                .find_map(|(value, literal)| {
                    (value == sibling
                        && matches!(
                            literal.integer_value(),
                            Some((actual, IntegerValue::Signed(_) | IntegerValue::Unsigned(_)))
                                if actual == integer_type
                        ))
                    .then_some(index)
                })
        });
    let index = matches.next()?;
    matches.next().is_none().then_some(index)
}

/// Every `(next, sibling)` reading one definition side admits for `current`.
/// A forward reading resumes at `target` and cites the other operand as the
/// landed sibling. An add reading traversed toward its operand resumes at
/// that operand instead; two `Value` addends yield both readings and the
/// caller keeps whichever sibling lands uniquely.
fn step_readings<'a>(
    target: &'a ScalarTerm,
    expression: &'a ScalarTerm,
    current: &ScalarTerm,
    expected: ScalarType,
) -> Vec<(&'a ScalarTerm, &'a ScalarTerm)> {
    let mut readings = Vec::with_capacity(2);
    if !matches!(target, ScalarTerm::Value { .. }) || target.scalar_type() != expected {
        return readings;
    }
    if let Some(sibling) = forward_sibling(expression, current, expected) {
        readings.push((target, sibling));
    }
    if target == current {
        readings.extend(add_operands(expression, expected));
    }
    readings
}

fn forward_sibling<'a>(
    expression: &'a ScalarTerm,
    current: &ScalarTerm,
    expected: ScalarType,
) -> Option<&'a ScalarTerm> {
    let (left, right, subtraction) = match expression {
        // A bitwise and reads like a commutative operand pair: the current
        // value resumes at either side and the other side is the sibling
        // mask the literal search must land.
        ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerMultiply { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. } => {
            (left.as_ref(), right.as_ref(), false)
        }
        // The kernel traverses a wrapping add on either carrier sign and
        // demands no-wrap evidence on the side the addend faces.
        ScalarTerm::WrappingIntegerAdd { left, right, .. } => {
            (left.as_ref(), right.as_ref(), false)
        }
        ScalarTerm::ExactIntegerDivide { left, right, .. }
        | ScalarTerm::WrappingIntegerDivide { left, right, .. }
        | ScalarTerm::ExactIntegerRemainder { left, right, .. }
        | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
        | ScalarTerm::ExactIntegerSubtract { left, right, .. } => {
            (left.as_ref(), right.as_ref(), true)
        }
        _ => return None,
    };
    if expression.scalar_type() != expected {
        return None;
    }
    if left == current {
        Some(right)
    } else if !subtraction && right == current {
        Some(left)
    } else {
        None
    }
}

fn add_operands(expression: &ScalarTerm, expected: ScalarType) -> Vec<(&ScalarTerm, &ScalarTerm)> {
    let mut readings = Vec::new();
    let (left, right) = match expression {
        ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerAdd { left, right, .. } => (left.as_ref(), right.as_ref()),
        _ => return readings,
    };
    if expression.scalar_type() != expected {
        return readings;
    }
    for (operand, sibling) in [(left, right), (right, left)] {
        if matches!(operand, ScalarTerm::Value { .. }) {
            readings.push((operand, sibling));
        }
    }
    readings
}
