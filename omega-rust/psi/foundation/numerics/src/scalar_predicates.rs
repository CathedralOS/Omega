//! Selected scalar predicates, shared by source proofs and portable conditions.
//!
//! The caller owns leaf observations and comparison authority. This evaluator
//! neither executes arithmetic nor discovers or grants operator meaning.

/// Target-neutral mathematical value of an already-observed scalar. Unknown
/// is the zero arena entry and never supplies a proof premise.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum ScalarValue {
    #[default]
    Unknown,
    Integer(crate::bignum::BigInt),
    Boolean(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredicateValueKind {
    Integer,
    Boolean,
}

/// Builtin logical/comparison meaning already selected by the source checker.
/// This vocabulary cannot name arithmetic or user-defined operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum PredicateOperator {
    And,
    Or,
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

impl PredicateOperator {
    pub fn evaluate_comparison<T: Ord>(self, left: &T, right: &T) -> Option<bool> {
        Some(match self {
            Self::Equal => left == right,
            Self::NotEqual => left != right,
            Self::Less => left < right,
            Self::LessOrEqual => left <= right,
            Self::Greater => left > right,
            Self::GreaterOrEqual => left >= right,
            Self::And | Self::Or => return None,
        })
    }
}

/// Selected Boolean structure, independent of the representation supplying its
/// children and scalar observations. Arithmetic meaning stays with the owner
/// that supplies a leaf value, not this predicate interpreter.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum PredicateNode<Reference, Leaf> {
    #[default]
    Missing,
    Value(ScalarValue),
    Leaf(Leaf),
    Forward(Reference),
    Not(Reference),
    Binary {
        operator: PredicateOperator,
        left: Reference,
        right: Reference,
    },
}

impl<Reference: Copy, Leaf> PredicateNode<Reference, Leaf> {
    /// Establish operand formation independently of selective evaluation.
    /// A skipped child still needs its declared kind; truth grants no type.
    pub fn value_kind(
        &self,
        reference: &mut impl FnMut(Reference) -> Result<PredicateValueKind, &'static str>,
        leaf: &mut impl FnMut(&Leaf) -> Result<PredicateValueKind, &'static str>,
    ) -> Result<PredicateValueKind, &'static str> {
        use PredicateValueKind::{Boolean, Integer};
        match self.try_map(reference, leaf)? {
            PredicateNode::Missing | PredicateNode::Value(ScalarValue::Unknown) => {
                Err("scalar predicate contains an absent value")
            }
            PredicateNode::Value(ScalarValue::Boolean(_)) => Ok(Boolean),
            PredicateNode::Value(ScalarValue::Integer(_)) => Ok(Integer),
            PredicateNode::Leaf(kind) | PredicateNode::Forward(kind) => Ok(kind),
            PredicateNode::Not(Boolean) => Ok(Boolean),
            PredicateNode::Not(Integer) => {
                Err("scalar predicate negation requires a Boolean operand")
            }
            PredicateNode::Binary {
                operator,
                left,
                right,
            } => {
                let formed = match operator {
                    PredicateOperator::And | PredicateOperator::Or => {
                        left == Boolean && right == Boolean
                    }
                    PredicateOperator::Equal | PredicateOperator::NotEqual => left == right,
                    PredicateOperator::Less
                    | PredicateOperator::LessOrEqual
                    | PredicateOperator::Greater
                    | PredicateOperator::GreaterOrEqual => left == Integer && right == Integer,
                };
                if formed {
                    Ok(Boolean)
                } else {
                    Err("scalar predicate operator has incompatible operand kinds")
                }
            }
        }
    }

    /// Change coordinates and leaf custody without changing selected meaning.
    /// Every child is mapped, including a child evaluation may short circuit.
    pub fn try_map<MappedReference, MappedLeaf, Error>(
        &self,
        reference: &mut impl FnMut(Reference) -> Result<MappedReference, Error>,
        leaf: &mut impl FnMut(&Leaf) -> Result<MappedLeaf, Error>,
    ) -> Result<PredicateNode<MappedReference, MappedLeaf>, Error> {
        Ok(match self {
            Self::Missing => PredicateNode::Missing,
            Self::Value(value) => PredicateNode::Value(value.clone()),
            Self::Leaf(value) => PredicateNode::Leaf(leaf(value)?),
            Self::Forward(child) => PredicateNode::Forward(reference(*child)?),
            Self::Not(child) => PredicateNode::Not(reference(*child)?),
            Self::Binary {
                operator,
                left,
                right,
            } => PredicateNode::Binary {
                operator: *operator,
                left: reference(*left)?,
                right: reference(*right)?,
            },
        })
    }
}

/// One selective evaluator serves live source proofs and retained predicates.
/// The worklist owns traversal; a captured graph needs no source-tree access.
pub fn evaluate_nodes<Reference: Copy + PartialEq, Leaf>(
    root: Reference,
    node: &mut impl FnMut(Reference) -> Option<PredicateNode<Reference, Leaf>>,
    resolve_leaf: &mut impl FnMut(Leaf) -> Option<ScalarValue>,
    comparison_is_admitted: &impl Fn(&ScalarValue, &ScalarValue) -> bool,
) -> Option<ScalarValue> {
    enum Step<Reference> {
        Enter(Reference),
        Leave,
        Not,
        Left(PredicateOperator, Reference),
        Right(PredicateOperator, ScalarValue),
    }
    let mut pending = vec![Step::Enter(root)];
    let mut active = Vec::new();
    let mut value = None;
    while let Some(step) = pending.pop() {
        match step {
            Step::Enter(reference) => {
                if active.contains(&reference) {
                    return None;
                }
                active.push(reference);
                pending.push(Step::Leave);
                match node(reference)? {
                    PredicateNode::Missing => return None,
                    PredicateNode::Value(result) => value = Some(result),
                    PredicateNode::Leaf(leaf) => value = Some(resolve_leaf(leaf)?),
                    PredicateNode::Forward(child) => pending.push(Step::Enter(child)),
                    PredicateNode::Not(child) => {
                        pending.push(Step::Not);
                        pending.push(Step::Enter(child));
                    }
                    PredicateNode::Binary {
                        operator,
                        left,
                        right,
                    } => {
                        pending.push(Step::Left(operator, right));
                        pending.push(Step::Enter(left));
                    }
                }
            }
            Step::Leave => {
                active.pop()?;
            }
            Step::Not => {
                let ScalarValue::Boolean(operand) = value.take()? else {
                    return None;
                };
                value = Some(ScalarValue::Boolean(!operand));
            }
            Step::Left(operator, right) => {
                let left = value.take()?;
                // A skipped operand supplies no value premise or comparison
                // authority. Preserve source left-to-right short circuiting.
                match (&left, operator) {
                    (ScalarValue::Boolean(false), PredicateOperator::And)
                    | (ScalarValue::Boolean(true), PredicateOperator::Or) => value = Some(left),
                    _ => {
                        pending.push(Step::Right(operator, left));
                        pending.push(Step::Enter(right));
                    }
                }
            }
            Step::Right(operator, left) => {
                let right = value.take()?;
                if !matches!(operator, PredicateOperator::And | PredicateOperator::Or)
                    && !comparison_is_admitted(&left, &right)
                {
                    return None;
                }
                let result = match (left, right) {
                    (ScalarValue::Boolean(left), ScalarValue::Boolean(right)) => match operator {
                        PredicateOperator::And => left && right,
                        PredicateOperator::Or => left || right,
                        PredicateOperator::Equal => left == right,
                        PredicateOperator::NotEqual => left != right,
                        _ => return None,
                    },
                    (ScalarValue::Integer(left), ScalarValue::Integer(right)) => {
                        operator.evaluate_comparison(&left, &right)?
                    }
                    _ => return None,
                };
                value = Some(ScalarValue::Boolean(result));
            }
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bignum::BigInt;

    #[test]
    fn scalar_predicates_preserve_selected_comparisons_and_authority() {
        let wide = BigInt::from_u64(1).shl_bits(260);
        for (operator, expected) in [
            (PredicateOperator::Equal, false),
            (PredicateOperator::NotEqual, true),
            (PredicateOperator::Less, true),
            (PredicateOperator::LessOrEqual, true),
            (PredicateOperator::Greater, false),
            (PredicateOperator::GreaterOrEqual, false),
        ] {
            let nodes: [PredicateNode<usize, ()>; 3] = [
                PredicateNode::Value(ScalarValue::Integer(wide.negate())),
                PredicateNode::Value(ScalarValue::Integer(wide.clone())),
                PredicateNode::Binary {
                    operator,
                    left: 0,
                    right: 1,
                },
            ];
            assert_eq!(
                evaluate_nodes(
                    2,
                    &mut |reference| nodes.get(reference).cloned(),
                    &mut |_| None,
                    &|_, _| true
                ),
                Some(ScalarValue::Boolean(expected))
            );
            assert_eq!(
                evaluate_nodes(
                    2,
                    &mut |reference| nodes.get(reference).cloned(),
                    &mut |_| None,
                    &|_, _| false
                ),
                None
            );
        }
    }

    #[test]
    fn scalar_predicates_short_circuit_without_observing_skipped_leaves() {
        for (operator, left, skipped) in [
            (PredicateOperator::And, false, true),
            (PredicateOperator::And, true, false),
            (PredicateOperator::Or, true, true),
            (PredicateOperator::Or, false, false),
        ] {
            let nodes = [
                PredicateNode::Value(ScalarValue::Boolean(left)),
                PredicateNode::Leaf(73u32),
                PredicateNode::Binary {
                    operator,
                    left: 0usize,
                    right: 1,
                },
            ];
            let mut observed = Vec::new();
            let result = evaluate_nodes(
                2,
                &mut |reference| nodes.get(reference).cloned(),
                &mut |leaf| {
                    observed.push(leaf);
                    None
                },
                &|_, _| false,
            );
            assert_eq!(observed.is_empty(), skipped);
            assert_eq!(result, skipped.then_some(ScalarValue::Boolean(left)));
        }
    }

    #[test]
    fn scalar_predicates_do_not_infer_boolean_comparison_authority() {
        let nodes: [PredicateNode<usize, ()>; 4] = [
            PredicateNode::Value(ScalarValue::Boolean(true)),
            PredicateNode::Value(ScalarValue::Boolean(false)),
            PredicateNode::Binary {
                operator: PredicateOperator::NotEqual,
                left: 0,
                right: 1,
            },
            PredicateNode::Not(2),
        ];
        assert_eq!(
            evaluate_nodes(
                3,
                &mut |reference| nodes.get(reference).cloned(),
                &mut |_| None,
                &|_, _| true
            ),
            Some(ScalarValue::Boolean(false))
        );
        assert_eq!(
            evaluate_nodes(
                3,
                &mut |reference| nodes.get(reference).cloned(),
                &mut |_| None,
                &|_, _| false
            ),
            None
        );
    }

    #[test]
    fn scalar_predicates_refuse_missing_cyclic_and_mismatched_values() {
        let cyclic: [PredicateNode<usize, ()>; 2] =
            [PredicateNode::Forward(1), PredicateNode::Not(0)];
        assert_eq!(
            evaluate_nodes(
                0,
                &mut |reference| cyclic.get(reference).cloned(),
                &mut |_| None,
                &|_, _| true
            ),
            None
        );
        assert_eq!(
            evaluate_nodes(
                7,
                &mut |reference| cyclic.get(reference).cloned(),
                &mut |_| None,
                &|_, _| true
            ),
            None
        );
        let mixed: [PredicateNode<usize, ()>; 3] = [
            PredicateNode::Value(ScalarValue::Integer(BigInt::from_u64(1))),
            PredicateNode::Value(ScalarValue::Boolean(true)),
            PredicateNode::Binary {
                operator: PredicateOperator::Equal,
                left: 0,
                right: 1,
            },
        ];
        assert_eq!(
            evaluate_nodes(
                2,
                &mut |reference| mixed.get(reference).cloned(),
                &mut |_| None,
                &|_, _| true
            ),
            None
        );
    }

    #[test]
    fn scalar_predicate_publication_maps_both_children_before_evaluation() {
        let selected: PredicateNode<u32, ()> = PredicateNode::Binary {
            operator: PredicateOperator::And,
            left: 11,
            right: 12,
        };
        let mut mapped = Vec::new();
        let result = selected.try_map(
            &mut |child| {
                mapped.push(child);
                if child == 12 {
                    Err("missing formation")
                } else {
                    Ok(child + 100)
                }
            },
            &mut |leaf| Ok(*leaf),
        );
        assert_eq!(mapped, vec![11, 12]);
        assert_eq!(result, Err("missing formation"));
    }
}
