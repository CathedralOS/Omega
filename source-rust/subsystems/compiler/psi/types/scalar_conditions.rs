use arena::{Arena, Handle, HandleSpan};
use bignum::BigInt;
use numerics::integer_bounds::IntegerBoundStep;
use numerics::scalar_predicates::{
    PredicateNode, PredicateValueKind, ScalarValue, evaluate_nodes,
};
use semantic_vocabulary::{
    MachineId, ScalarApplicationId, ScalarConditionRole, ScalarType, ValueId,
};

use super::{ProofDeclarationId, ScalarApplication, ScalarApplications};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
        role: ScalarConditionRole,
        requirement: ScalarRangeRequirement,
        steps: HandleSpan<IntegerBoundStep<ScalarRangeObservation>>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ScalarCompletionCondition {
    #[default]
    Missing,
    Complete {
        subject: ScalarConditionSubject,
        application: ScalarApplicationId,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScalarConditions {
    ranges: Arena<ScalarRangeCondition>,
    steps: Arena<IntegerBoundStep<ScalarRangeObservation>>,
    predicates: Arena<ScalarPredicateCondition>,
    completions: Arena<ScalarCompletionCondition>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScalarPredicateNode(
    pub PredicateNode<Handle<ScalarPredicateNode>, ScalarRangeObservation>,
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarPredicateCondition {
    pub subject: ScalarConditionSubject,
    pub role: ScalarConditionRole,
    pub description: String,
    nodes: Arena<ScalarPredicateNode>,
    root: Handle<ScalarPredicateNode>,
}

impl Default for ScalarPredicateCondition {
    fn default() -> Self {
        Self {
            subject: ScalarConditionSubject::Module,
            role: ScalarConditionRole::Prerequisite,
            description: String::new(),
            nodes: Arena::new(),
            root: Handle::invalid(),
        }
    }
}

impl ScalarPredicateCondition {
    fn new(
        subject: ScalarConditionSubject,
        role: ScalarConditionRole,
        description: String,
        nodes: Arena<ScalarPredicateNode>,
        root: Handle<ScalarPredicateNode>,
    ) -> Result<Self, &'static str> {
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
        Ok(Self {
            subject,
            role,
            description,
            nodes,
            root,
        })
    }

    pub fn nodes(&self) -> &Arena<ScalarPredicateNode> {
        &self.nodes
    }
    pub fn root(&self) -> Handle<ScalarPredicateNode> {
        self.root
    }

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
    pub fn contains_subject(&self, subject: ScalarConditionSubject) -> bool {
        self.ranges().iter().any(|condition| {
            matches!(condition, ScalarRangeCondition::Range { subject: owner, .. } if *owner == subject)
        }) || self.predicates().iter().any(|condition| condition.subject == subject)
            || self.completions().iter().any(|condition| {
                matches!(condition, ScalarCompletionCondition::Complete { subject: owner, .. } if *owner == subject)
            })
    }

    pub fn completions(&self) -> &[ScalarCompletionCondition] {
        self.completions.storage_slice()
    }

    pub fn retain_completion(
        &mut self,
        subject: ScalarConditionSubject,
        application: ScalarApplicationId,
    ) {
        self.completions
            .insert(ScalarCompletionCondition::Complete {
                subject,
                application,
            });
    }

    pub fn ranges(&self) -> &[ScalarRangeCondition] {
        self.ranges.storage_slice()
    }

    pub fn predicates(&self) -> &[ScalarPredicateCondition] {
        self.predicates.storage_slice()
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty() && self.predicates.is_empty() && self.completions.is_empty()
    }

    pub fn retain_predicate(
        &mut self,
        subject: ScalarConditionSubject,
        role: ScalarConditionRole,
        description: String,
        nodes: Arena<ScalarPredicateNode>,
        root: Handle<ScalarPredicateNode>,
    ) -> Result<(), &'static str> {
        let condition = ScalarPredicateCondition::new(subject, role, description, nodes, root)?;
        self.insert_predicate(condition);
        Ok(())
    }

    pub fn insert_predicate(&mut self, condition: ScalarPredicateCondition) {
        self.predicates.insert(condition);
    }

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
                        condition.nodes.storage_slice().iter().filter_map(|node| {
                            match &node.0 {
                                PredicateNode::Leaf(observation) => Some(observation),
                                _ => None,
                            }
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

    pub fn retain_range(
        &mut self,
        subject: ScalarConditionSubject,
        role: ScalarConditionRole,
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
            role,
            requirement,
            steps,
        });
        Ok(())
    }
}

impl ScalarApplications {
    /// A ground comparison can follow its observations into target
    /// closure. Runtime values still need the ordinary source proof.
    pub(crate) fn layout_predicate(
        &self,
        subject: Option<ProofDeclarationId>,
        description: impl FnOnce() -> String,
        goal: &semantic_vocabulary::Proposition,
    ) -> Result<Option<ScalarPredicateCondition>, &'static str> {
        let mut builder = LayoutPredicate {
            applications: self,
            nodes: Arena::new(),
            observes_target: false,
        };
        let Some(root) = builder.proposition(goal, 0) else {
            return Ok(None);
        };
        if !builder.observes_target {
            return Ok(None);
        }
        ScalarPredicateCondition::new(
            ScalarConditionSubject::Declaration(
                subject.ok_or("layout call requirement has no exact caller declaration")?,
            ),
            ScalarConditionRole::Prerequisite,
            description(),
            builder.nodes,
            root,
        )
        .map(Some)
    }
}

struct LayoutPredicate<'module> {
    applications: &'module ScalarApplications,
    nodes: Arena<ScalarPredicateNode>,
    observes_target: bool,
}

impl LayoutPredicate<'_> {
    fn push(
        &mut self,
        node: PredicateNode<Handle<ScalarPredicateNode>, ScalarRangeObservation>,
    ) -> Option<Handle<ScalarPredicateNode>> {
        (self.nodes.storage_slice().len() < 4096)
            .then(|| self.nodes.insert(ScalarPredicateNode(node)))
    }

    fn term(
        &mut self,
        term: &semantic_vocabulary::ScalarTerm,
    ) -> Option<Handle<ScalarPredicateNode>> {
        use semantic_vocabulary::{IntegerValue, ScalarTerm};
        let node = match term {
            ScalarTerm::Application {
                application,
                scalar_type,
            } => {
                let declared = match self.applications.declaration(*application)? {
                    ScalarApplication::LayoutObservation { scalar_type, .. }
                    | ScalarApplication::Evaluate { scalar_type, .. } => *scalar_type,
                    ScalarApplication::Missing => return None,
                };
                if declared != *scalar_type || !matches!(declared, ScalarType::Integer(_)) {
                    return None;
                }
                self.observes_target = true;
                PredicateNode::Leaf(ScalarRangeObservation::Application(*application))
            }
            ScalarTerm::Integer { value, .. } => {
                let value = match value {
                    IntegerValue::Unsigned(value) => BigInt::from_u128(*value),
                    IntegerValue::Signed(value) => BigInt::from_i128(*value),
                };
                PredicateNode::Value(ScalarValue::Integer(value))
            }
            ScalarTerm::Boolean(value) => PredicateNode::Value(ScalarValue::Boolean(*value)),
            _ => return None,
        };
        self.push(node)
    }

    fn proposition(
        &mut self,
        goal: &semantic_vocabulary::Proposition,
        depth: usize,
    ) -> Option<Handle<ScalarPredicateNode>> {
        use numerics::scalar_predicates::PredicateOperator;
        use semantic_vocabulary::Proposition;
        if depth >= 64 {
            return None;
        }
        let (operator, left, right) = match goal {
            Proposition::Truth | Proposition::Falsehood => {
                return self.push(PredicateNode::Value(ScalarValue::Boolean(matches!(
                    goal,
                    Proposition::Truth
                ))));
            }
            Proposition::Equal(left, right) => (PredicateOperator::Equal, left, right),
            Proposition::LessThan(left, right) => (PredicateOperator::Less, left, right),
            Proposition::LessOrEqual(left, right) => {
                (PredicateOperator::LessOrEqual, left, right)
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                let premise = self.proposition(premise, depth + 1)?;
                let left = self.push(PredicateNode::Not(premise))?;
                let right = self.proposition(conclusion, depth + 1)?;
                return self.push(PredicateNode::Binary {
                    operator: PredicateOperator::Or,
                    left,
                    right,
                });
            }
            Proposition::Conjunction(terms) | Proposition::Disjunction(terms) => {
                let operator = if matches!(goal, Proposition::Conjunction(_)) {
                    PredicateOperator::And
                } else {
                    PredicateOperator::Or
                };
                let mut terms = terms.iter();
                let mut left = self.proposition(terms.next()?, depth + 1)?;
                for term in terms {
                    let right = self.proposition(term, depth + 1)?;
                    left = self.push(PredicateNode::Binary {
                        operator,
                        left,
                        right,
                    })?;
                }
                return Some(left);
            }
            _ => return None,
        };
        let left = self.term(left)?;
        let right = self.term(right)?;
        self.push(PredicateNode::Binary {
            operator,
            left,
            right,
        })
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
                    semantic_vocabulary::ScalarConditionRole::Prerequisite,
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
                semantic_vocabulary::ScalarConditionRole::Prerequisite,
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
                semantic_vocabulary::ScalarConditionRole::Prerequisite,
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
