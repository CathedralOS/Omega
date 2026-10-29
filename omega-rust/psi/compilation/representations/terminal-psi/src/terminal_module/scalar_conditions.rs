//! Conditional integer judgments owned by the portable program.
//!
//! A retained condition is a debt, never a proof. Its target-independent bound
//! expression names exact applications or an exact result of a complete scalar
//! entry. Target admission supplies those observations and the address model;
//! absence, an unexecuted occurrence, or a final return value cannot substitute.

use arena::{Arena, Handle, HandleSpan};
use numerics::bignum::BigInt;
use numerics::integer_bounds::IntegerBoundStep;
use numerics::scalar_predicates::{PredicateNode, PredicateValueKind, ScalarValue, evaluate_nodes};
use semantic_vocabulary::{MachineId, ScalarApplicationId, ScalarType, ValueId};

use super::ProofDeclarationId;

/// The declaration use which demands this condition. Module-wide requirements
/// are explicit; failed owner lookup must never become module-wide ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarConditionSubject {
    Module,
    Declaration(ProofDeclarationId),
    Application(ScalarApplicationId),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ScalarRangeObservation {
    #[default]
    Missing,
    Literal(BigInt),
    Application(ScalarApplicationId),
    /// The value belongs to the application's root activation, not an arbitrary
    /// recursive invocation of the same body. Repeated occurrences cannot use
    /// last-writer-wins; admission must retain the exact occurrence or refuse.
    EntryResult {
        application: ScalarApplicationId,
        machine: MachineId,
        result: ValueId,
        scalar_type: ScalarType,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalarRangeRequirement {
    Fixed { minimum: BigInt, maximum: BigInt },
    AddressModel,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ScalarRangeCondition {
    #[default]
    Missing,
    Range {
        subject: ScalarConditionSubject,
        requirement: ScalarRangeRequirement,
        steps: HandleSpan<IntegerBoundStep<ScalarRangeObservation>>,
    },
}

/// One module-owned arena for condition rows and their captured bounds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScalarConditions {
    ranges: Arena<ScalarRangeCondition>,
    steps: Arena<IntegerBoundStep<ScalarRangeObservation>>,
    predicates: Arena<ScalarPredicateCondition>,
}

/// A checked Boolean graph. Leaves retain exact scalar observations; the graph
/// grants no formation premise for computing those observations.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScalarPredicateNode(
    pub PredicateNode<Handle<ScalarPredicateNode>, ScalarRangeObservation>,
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarPredicateCondition {
    pub subject: ScalarConditionSubject,
    pub description: String,
    nodes: Arena<ScalarPredicateNode>,
    root: Handle<ScalarPredicateNode>,
}

impl Default for ScalarPredicateCondition {
    fn default() -> Self {
        Self {
            subject: ScalarConditionSubject::Module,
            description: String::new(),
            nodes: Arena::new(),
            root: Handle::invalid(),
        }
    }
}

impl ScalarPredicateCondition {
    pub fn nodes(&self) -> &Arena<ScalarPredicateNode> {
        &self.nodes
    }
    pub fn root(&self) -> Handle<ScalarPredicateNode> {
        self.root
    }

    /// The caller supplies independently admitted observations. An absent
    /// observation stays unknown, including after another computation succeeds.
    pub fn evaluate(
        &self,
        observe: &mut impl FnMut(ScalarRangeObservation) -> Option<ScalarValue>,
    ) -> Option<ScalarValue> {
        evaluate_nodes(
            self.root,
            &mut |reference| {
                self.nodes
                    .is_valid(reference)
                    .then(|| self.nodes.get(reference).0.clone())
            },
            &mut |observation| match observation {
                ScalarRangeObservation::Missing => None,
                ScalarRangeObservation::Literal(value) => Some(ScalarValue::Integer(value)),
                observed => match observe(observed)? {
                    ScalarValue::Integer(value) => Some(ScalarValue::Integer(value)),
                    _ => None,
                },
            },
            &|_, _| true,
        )
    }
}

impl ScalarConditions {
    pub fn ranges(&self) -> &[ScalarRangeCondition] {
        self.ranges.storage_slice()
    }

    pub fn predicates(&self) -> &[ScalarPredicateCondition] {
        self.predicates.storage_slice()
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty() && self.predicates.is_empty()
    }

    pub fn retain_predicate(
        &mut self,
        subject: ScalarConditionSubject,
        description: String,
        nodes: Arena<ScalarPredicateNode>,
        root: Handle<ScalarPredicateNode>,
    ) -> Result<(), &'static str> {
        let mut preceding = Vec::new();
        for (reference, node) in nodes.iter() {
            if matches!(
                node.0,
                PredicateNode::Missing
                    | PredicateNode::Value(ScalarValue::Unknown)
                    | PredicateNode::Leaf(ScalarRangeObservation::Missing)
            ) {
                return Err("scalar predicate contains an absent node or observation");
            }
            let kind = node.0.value_kind(
                &mut |child| {
                    preceding
                        .iter()
                        .find(|(reference, _)| *reference == child)
                        .map(|(_, kind)| *kind)
                        .ok_or("scalar predicate lost a preceding child")
                },
                &mut |_| Ok(PredicateValueKind::Integer),
            )?;
            preceding.push((reference, kind));
        }
        if !nodes.is_valid(root) {
            return Err("scalar predicate lost its root");
        }
        if preceding
            .iter()
            .find(|(reference, _)| *reference == root)
            .map(|(_, kind)| *kind)
            != Some(PredicateValueKind::Boolean)
        {
            return Err("scalar predicate root is not Boolean");
        }
        self.predicates.insert(ScalarPredicateCondition {
            subject,
            description,
            nodes,
            root,
        });
        Ok(())
    }

    /// References retained by all conditions, including values with no
    /// surviving executable use. Optimizers must preserve these identities.
    pub fn observations(&self) -> impl Iterator<Item = &ScalarRangeObservation> {
        self.steps
            .storage_slice()
            .iter()
            .filter_map(|step| match step {
                IntegerBoundStep::Observation(observation) => Some(observation),
                _ => None,
            })
            .chain(
                self.predicates
                    .storage_slice()
                    .iter()
                    .flat_map(|condition| {
                        condition
                            .nodes
                            .storage_slice()
                            .iter()
                            .filter_map(|node| match &node.0 {
                                PredicateNode::Leaf(observation) => Some(observation),
                                _ => None,
                            })
                    }),
            )
    }

    pub fn steps(
        &self,
        condition: &ScalarRangeCondition,
    ) -> Result<&[IntegerBoundStep<ScalarRangeObservation>], &'static str> {
        let ScalarRangeCondition::Range { steps, .. } = condition else {
            return Err("scalar condition is absent");
        };
        self.steps
            .span(*steps)
            .ok_or("scalar condition lost its captured bounds")
    }

    /// Retain a formed bound expression without establishing its truth.
    pub fn retain_range(
        &mut self,
        subject: ScalarConditionSubject,
        requirement: ScalarRangeRequirement,
        steps: Vec<IntegerBoundStep<ScalarRangeObservation>>,
    ) -> Result<(), &'static str> {
        if let ScalarRangeRequirement::Fixed { minimum, maximum } = &requirement
            && minimum > maximum
        {
            return Err("scalar condition has inverted required bounds");
        }
        let mut depth = 0usize;
        for step in &steps {
            match step {
                IntegerBoundStep::Missing => {
                    return Err("scalar condition contains an absent step");
                }
                IntegerBoundStep::Bounds { low, high } => {
                    if low > high {
                        return Err("scalar condition has inverted captured bounds");
                    }
                    depth += 1;
                }
                IntegerBoundStep::Observation(ScalarRangeObservation::Missing) => {
                    return Err("scalar condition contains an absent observation");
                }
                IntegerBoundStep::Observation(_) => depth += 1,
                IntegerBoundStep::Add
                | IntegerBoundStep::Subtract
                | IntegerBoundStep::Multiply
                | IntegerBoundStep::Union
                | IntegerBoundStep::Interval
                | IntegerBoundStep::Intersect => {
                    if depth < 2 {
                        return Err("scalar condition has missing bound operands");
                    }
                    depth -= 1;
                }
            }
        }
        if depth != 1 {
            return Err("scalar condition does not produce one bound interval");
        }
        let steps = self.steps.insert_many(steps);
        self.ranges.insert(ScalarRangeCondition::Range {
            subject,
            requirement,
            steps,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use numerics::scalar_predicates::PredicateOperator;

    #[test]
    fn scalar_predicate_formation_checks_root_and_all_operator_kinds() {
        for operator in [
            PredicateOperator::And,
            PredicateOperator::Or,
            PredicateOperator::Less,
            PredicateOperator::GreaterOrEqual,
        ] {
            let mut nodes = Arena::new();
            let integer = nodes.insert(ScalarPredicateNode(PredicateNode::Value(
                ScalarValue::Integer(BigInt::from_u64(1)),
            )));
            let boolean = nodes.insert(ScalarPredicateNode(PredicateNode::Value(
                ScalarValue::Boolean(true),
            )));
            let root = nodes.insert(ScalarPredicateNode(PredicateNode::Binary {
                operator,
                left: integer,
                right: boolean,
            }));
            assert_eq!(
                ScalarConditions::default().retain_predicate(
                    ScalarConditionSubject::Module,
                    "bad kinds".into(),
                    nodes,
                    root
                ),
                Err("scalar predicate operator has incompatible operand kinds")
            );
        }
        let mut nodes = Arena::new();
        let root = nodes.insert(ScalarPredicateNode(PredicateNode::Value(
            ScalarValue::Integer(BigInt::from_u64(1)),
        )));
        assert_eq!(
            ScalarConditions::default().retain_predicate(
                ScalarConditionSubject::Module,
                "integer root".into(),
                nodes,
                root
            ),
            Err("scalar predicate root is not Boolean")
        );
    }

    #[test]
    fn scalar_predicate_observations_cannot_change_declared_kind_or_literal() {
        let mut nodes = Arena::new();
        let left = nodes.insert(ScalarPredicateNode(PredicateNode::Leaf(
            ScalarRangeObservation::Application(ScalarApplicationId::new(1).unwrap()),
        )));
        let right = nodes.insert(ScalarPredicateNode(PredicateNode::Leaf(
            ScalarRangeObservation::Literal(BigInt::from_u64(24)),
        )));
        let root = nodes.insert(ScalarPredicateNode(PredicateNode::Binary {
            operator: PredicateOperator::Equal,
            left,
            right,
        }));
        let mut conditions = ScalarConditions::default();
        conditions
            .retain_predicate(
                ScalarConditionSubject::Module,
                "exact integer observations".into(),
                nodes,
                root,
            )
            .unwrap();
        assert_eq!(
            conditions.predicates()[0].evaluate(&mut |_| Some(ScalarValue::Boolean(true))),
            None
        );
        let mut observations = 0;
        assert_eq!(
            conditions.predicates()[0].evaluate(&mut |_| {
                observations += 1;
                Some(ScalarValue::Integer(BigInt::from_u64(24)))
            }),
            Some(ScalarValue::Boolean(true))
        );
        assert_eq!(observations, 1);
    }
}
