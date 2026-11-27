//! A shared Boolean expression for the edges that retry. Each node refers
//! only to earlier nodes; repeated joins share their predecessors.

mod stable_words;

use super::{SpinCondition, Subject};
use stable_words::StableWords;
use std::ops::Not;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Guard {
    node: usize,
    inverted: bool,
}

pub(super) const NEVER: Guard = Guard {
    node: 0,
    inverted: false,
};
pub(super) const ALWAYS: Guard = Guard {
    node: 0,
    inverted: true,
};

impl Not for Guard {
    type Output = Self;
    fn not(mut self) -> Self {
        self.inverted = !self.inverted;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Node {
    Observation(Subject, SpinCondition),
    Both(Guard, Guard),
    Select(Guard, Guard, Guard),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super::super) struct Predicate {
    nodes: Vec<Node>,
    pub(super) retry: Guard,
}

impl Predicate {
    pub(super) fn new() -> Self {
        Self {
            nodes: Vec::new(),
            retry: NEVER,
        }
    }

    fn push(&mut self, node: Node) -> Guard {
        self.nodes.push(node);
        Guard {
            node: self.nodes.len(),
            inverted: false,
        }
    }

    pub(super) fn observe(&mut self, subject: Subject, test: SpinCondition) -> Guard {
        self.push(Node::Observation(subject, test))
    }

    pub(super) fn both(&mut self, a: Guard, b: Guard) -> Guard {
        if a == NEVER || b == NEVER || a == !b {
            NEVER
        } else if a == ALWAYS || a == b {
            b
        } else if b == ALWAYS {
            a
        } else {
            self.push(Node::Both(a, b))
        }
    }

    pub(super) fn either(&mut self, a: Guard, b: Guard) -> Guard {
        !self.both(!a, !b)
    }

    /// This phi is used only on paths that reach its block. On this incoming
    /// edge it has the new value; on every previous arrival it keeps its value.
    pub(super) fn select(&mut self, guard: Guard, yes: Guard, no: Guard) -> Guard {
        let on_edge = |value: Guard, edge: Guard| {
            if value == edge {
                ALWAYS
            } else if value == !edge {
                NEVER
            } else {
                value
            }
        };
        let yes = on_edge(yes, guard);
        let no = on_edge(no, !guard);
        if guard == ALWAYS || yes == no {
            yes
        } else if guard == NEVER {
            no
        } else if yes == ALWAYS {
            self.either(guard, no)
        } else if no == ALWAYS {
            self.either(!guard, yes)
        } else if yes == NEVER {
            self.both(!guard, no)
        } else if no == NEVER {
            self.both(guard, yes)
        } else {
            self.push(Node::Select(guard, yes, no))
        }
    }

    pub(in super::super) fn inputs(
        &mut self,
    ) -> impl Iterator<Item = (&mut Subject, &mut SpinCondition)> {
        self.nodes.iter_mut().filter_map(|node| match node {
            Node::Observation(subject, test) => Some((subject, test)),
            Node::Both(..) | Node::Select(..) => None,
        })
    }

    pub(in super::super::super) fn evaluate<K: Eq + std::hash::Hash>(
        &self,
        mut observe: impl FnMut(&Subject, SpinCondition) -> (Observation, Option<(K, SpinCondition)>),
    ) -> Observation {
        let mut answers = vec![Observation::Known(false); self.nodes.len() + 1];
        let mut words = StableWords::default();
        for (index, node) in self.nodes.iter().enumerate() {
            if let Node::Observation(subject, test) = node {
                let (answer, stable_word) = observe(subject, *test);
                answers[index + 1] = answer;
                if let Some((word, known_test)) = stable_word
                    && answer == Observation::Unproved
                {
                    words.record(word, index + 1, known_test);
                }
            }
        }
        let result = self.reduce(&mut answers);
        if result == Observation::Unproved {
            words
                .decide(&mut answers, |answers| self.reduce(answers))
                .unwrap_or(result)
        } else {
            result
        }
    }

    fn reduce(&self, answers: &mut [Observation]) -> Observation {
        let value = |answers: &[Observation], guard: Guard| {
            let answer = answers[guard.node];
            if guard.inverted { !answer } else { answer }
        };
        for (index, node) in self.nodes.iter().enumerate() {
            let answer = match node {
                Node::Observation(..) => continue,
                Node::Both(left, right) => value(answers, *left).and(value(answers, *right)),
                Node::Select(test, yes, no) => {
                    let yes = value(answers, *yes);
                    let no = value(answers, *no);
                    match value(answers, *test) {
                        Observation::Known(true) => yes,
                        Observation::Known(false) => no,
                        _ if yes == no => yes,
                        unknown => [unknown, yes, no]
                            .into_iter()
                            .filter(|choice| !matches!(choice, Observation::Known(_)))
                            .reduce(Observation::and)
                            .unwrap_or(Observation::Unproved),
                    }
                }
            };
            answers[index + 1] = answer;
        }
        value(answers, self.retry)
    }
}

/// Keep caller-supplied progress separate from a locally unresolved word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super::super) enum Observation {
    Known(bool),
    Unproved,
    Supplied,
    PendingProvider,
}

impl Observation {
    fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::Known(false), _) | (_, Self::Known(false)) => Self::Known(false),
            (Self::Known(true), value) | (value, Self::Known(true)) => value,
            (Self::Unproved, _) | (_, Self::Unproved) => Self::Unproved,
            (Self::Supplied, _) | (_, Self::Supplied) => Self::Supplied,
            (Self::PendingProvider, Self::PendingProvider) => Self::PendingProvider,
        }
    }
}

impl Not for Observation {
    type Output = Self;
    fn not(self) -> Self {
        match self {
            Self::Known(value) => Self::Known(!value),
            unknown => unknown,
        }
    }
}
