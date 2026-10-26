//! Conditions over completed value selections, with shared Boolean structure.
//!
//! Fields projected from one saved selection keep one choice identity. Joining
//! their evidence therefore conjoins conditions, not lists of possible values.
//! Coverage restricts this DAG one choice at a time and memoizes the residuals;
//! it never materializes a Cartesian product of constructor fields or a DNF.
//! This is scratch for consumers of already produced captures, not another
//! evaluator or a source-level branch planner.

use arena::{Arena, Handle, HandleSpan};
use checked_trees::{FlowControlFacts, FlowOperandAlternative};

use super::operand_values::CapturedValueChoice;

type Selection = HandleSpan<FlowOperandAlternative>;
pub(crate) type SelectionCondition = Handle<Condition>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Condition {
    #[default]
    False,
    True,
    Arm(CapturedValueChoice),
    All(SelectionCondition, SelectionCondition),
    Any(SelectionCondition, SelectionCondition),
}

pub(crate) struct SelectionConditions {
    nodes: Arena<Condition>,
    truth: SelectionCondition,
    restrictions: Vec<(SelectionCondition, CapturedValueChoice, SelectionCondition)>,
    coverage: Vec<(SelectionCondition, bool)>,
    possibilities: Vec<(SelectionCondition, bool)>,
}

impl SelectionConditions {
    pub(crate) fn new() -> Self {
        let mut nodes = Arena::default();
        let truth = nodes.append(Condition::True);
        Self {
            nodes,
            truth,
            restrictions: Vec::new(),
            coverage: Vec::new(),
            possibilities: Vec::new(),
        }
    }

    pub(crate) fn always(&self) -> SelectionCondition {
        self.truth
    }

    pub(crate) fn never(&self) -> SelectionCondition {
        Handle::invalid()
    }

    pub(crate) fn choice(
        &mut self,
        control: &FlowControlFacts,
        choice: CapturedValueChoice,
    ) -> Option<SelectionCondition> {
        let rows = control.operand_alternatives.span(choice.selection)?;
        if rows.is_empty()
            || !rows.iter().enumerate().any(|(index, _)| {
                u32::try_from(index)
                    .is_ok_and(|index| choice.selection.handle_at(index) == choice.alternative)
            })
        {
            return None;
        }
        Some(self.intern(Condition::Arm(choice)))
    }

    pub(crate) fn choices(
        &mut self,
        control: &FlowControlFacts,
        choices: &[CapturedValueChoice],
    ) -> Option<SelectionCondition> {
        let mut result = self.always();
        for &choice in choices {
            let condition = self.choice(control, choice)?;
            result = self.all(result, condition);
        }
        Some(result)
    }

    pub(crate) fn all(
        &mut self,
        left: SelectionCondition,
        right: SelectionCondition,
    ) -> SelectionCondition {
        if !left.is_valid() || !right.is_valid() {
            return self.never();
        }
        if left == self.truth || left == right {
            return right;
        }
        if right == self.truth {
            return left;
        }
        if let (Condition::Arm(left_arm), Condition::Arm(right_arm)) =
            (*self.nodes.get(left), *self.nodes.get(right))
            && left_arm.selection == right_arm.selection
            && left_arm.alternative != right_arm.alternative
        {
            return self.never();
        }
        let (left, right) = ordered(left, right);
        self.intern(Condition::All(left, right))
    }

    pub(crate) fn any(
        &mut self,
        left: SelectionCondition,
        right: SelectionCondition,
    ) -> SelectionCondition {
        if left == self.truth || right == self.truth {
            return self.truth;
        }
        if !left.is_valid() || left == right {
            return right;
        }
        if !right.is_valid() {
            return left;
        }
        let (left, right) = ordered(left, right);
        self.intern(Condition::Any(left, right))
    }

    /// Every alternative is an obligation. A nested choice which is inactive
    /// on an outer arm disappears when that outer condition is restricted.
    pub(crate) fn covers(
        &mut self,
        control: &FlowControlFacts,
        condition: SelectionCondition,
    ) -> bool {
        if condition == self.truth {
            return true;
        }
        if !condition.is_valid() {
            return false;
        }
        if let Some((_, result)) = self.coverage.iter().find(|(known, _)| *known == condition) {
            return *result;
        }
        let Some(selection) = self.first_selection(condition) else {
            return false;
        };
        let Some(rows) = control.operand_alternatives.span(selection) else {
            return false;
        };
        let complete = !rows.is_empty()
            && rows.iter().enumerate().all(|(index, _)| {
                let Ok(index) = u32::try_from(index) else {
                    return false;
                };
                let reduced = self.restrict(
                    condition,
                    CapturedValueChoice {
                        selection,
                        alternative: selection.handle_at(index),
                    },
                );
                self.covers(control, reduced)
            });
        self.coverage.push((condition, complete));
        complete
    }

    pub(crate) fn possible(
        &mut self,
        control: &FlowControlFacts,
        condition: SelectionCondition,
    ) -> bool {
        if condition == self.truth {
            return true;
        }
        if !condition.is_valid() {
            return false;
        }
        if let Some((_, result)) = self
            .possibilities
            .iter()
            .find(|(known, _)| *known == condition)
        {
            return *result;
        }
        let Some(selection) = self.first_selection(condition) else {
            return false;
        };
        let Some(rows) = control.operand_alternatives.span(selection) else {
            return false;
        };
        let possible = rows.iter().enumerate().any(|(index, _)| {
            let Ok(index) = u32::try_from(index) else {
                return false;
            };
            let reduced = self.restrict(
                condition,
                CapturedValueChoice {
                    selection,
                    alternative: selection.handle_at(index),
                },
            );
            self.possible(control, reduced)
        });
        self.possibilities.push((condition, possible));
        possible
    }

    pub(crate) fn negate(
        &mut self,
        control: &FlowControlFacts,
        condition: SelectionCondition,
    ) -> SelectionCondition {
        match *self.nodes.get(condition) {
            Condition::False => self.always(),
            Condition::True => self.never(),
            Condition::Arm(choice) => {
                let Some(rows) = control.operand_alternatives.span(choice.selection) else {
                    return self.never();
                };
                let mut result = self.never();
                for (index, _) in rows.iter().enumerate() {
                    let Ok(index) = u32::try_from(index) else {
                        return self.never();
                    };
                    let alternative = choice.selection.handle_at(index);
                    if alternative != choice.alternative {
                        let arm = self.intern(Condition::Arm(CapturedValueChoice {
                            selection: choice.selection,
                            alternative,
                        }));
                        result = self.any(result, arm);
                    }
                }
                result
            }
            Condition::All(left, right) => {
                let left = self.negate(control, left);
                let right = self.negate(control, right);
                self.any(left, right)
            }
            Condition::Any(left, right) => {
                let left = self.negate(control, left);
                let right = self.negate(control, right);
                self.all(left, right)
            }
        }
    }

    fn intern(&mut self, node: Condition) -> SelectionCondition {
        if let Some((handle, _)) = self.nodes.iter().find(|(_, existing)| **existing == node) {
            return handle;
        }
        self.nodes.append(node)
    }

    fn first_selection(&self, condition: SelectionCondition) -> Option<Selection> {
        match *self.nodes.get(condition) {
            Condition::False | Condition::True => None,
            Condition::Arm(choice) => Some(choice.selection),
            Condition::All(left, right) | Condition::Any(left, right) => {
                match (self.first_selection(left), self.first_selection(right)) {
                    (Some(left), Some(right)) => Some(
                        if left.start().arena_index() < right.start().arena_index() {
                            left
                        } else {
                            right
                        },
                    ),
                    (left, right) => left.or(right),
                }
            }
        }
    }

    fn restrict(
        &mut self,
        condition: SelectionCondition,
        choice: CapturedValueChoice,
    ) -> SelectionCondition {
        if !condition.is_valid() || condition == self.truth {
            return condition;
        }
        if let Some((_, _, result)) = self
            .restrictions
            .iter()
            .find(|(known, selected, _)| *known == condition && *selected == choice)
        {
            return *result;
        }
        let result = match *self.nodes.get(condition) {
            Condition::False => self.never(),
            Condition::True => self.always(),
            Condition::Arm(selected) if selected.selection == choice.selection => {
                if selected.alternative == choice.alternative {
                    self.always()
                } else {
                    self.never()
                }
            }
            Condition::Arm(_) => condition,
            Condition::All(left, right) => {
                let left = self.restrict(left, choice);
                let right = self.restrict(right, choice);
                self.all(left, right)
            }
            Condition::Any(left, right) => {
                let left = self.restrict(left, choice);
                let right = self.restrict(right, choice);
                self.any(left, right)
            }
        };
        self.restrictions.push((condition, choice, result));
        result
    }
}

fn ordered(
    left: SelectionCondition,
    right: SelectionCondition,
) -> (SelectionCondition, SelectionCondition) {
    if left.arena_index() <= right.arena_index() {
        (left, right)
    } else {
        (right, left)
    }
}
