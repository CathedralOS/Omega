//! Storage ancestry is independent of the operation asking about it. Walk
//! projection bases without evaluating selectors; callers authorize each step
//! and decide whether a saved match subject must be resolved or may stay opaque.

use std::borrow::Borrow;

use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct};

use crate::body::{Expression, ExpressionKind};
use crate::{Unserved, refuse};

type Capture = arena::Handle<typed_trees::expression::CompletedValueCapture>;

pub(super) fn walk<'a, E>(
    mut place: &'a Expression,
    mut captured: impl FnMut(Capture) -> Result<Option<&'a Expression>, E>,
    mut projected: impl FnMut(&Expression, &Expression) -> Result<(), E>,
) -> Result<&'a Expression, E> {
    loop {
        match &place.kind {
            ExpressionKind::Field { base, .. }
            | ExpressionKind::Payload { base, .. }
            | ExpressionKind::Index { base, .. } => {
                projected(place, base)?;
                place = base;
            }
            ExpressionKind::SavedSubject(capture) => match captured(*capture)? {
                Some(subject) => place = subject,
                None => return Ok(place),
            },
            ExpressionKind::Slot(_)
            | ExpressionKind::Borrow { .. }
            | ExpressionKind::Integer(_)
            | ExpressionKind::ByteLiteral(_)
            | ExpressionKind::CollectionMeasure { .. }
            | ExpressionKind::Boolean(_)
            | ExpressionKind::MachineValue { .. }
            | ExpressionKind::Array { .. }
            | ExpressionKind::Record { .. }
            | ExpressionKind::Case { .. }
            | ExpressionKind::IsCase { .. }
            | ExpressionKind::Match { .. }
            | ExpressionKind::Strict { .. }
            | ExpressionKind::Not(_)
            | ExpressionKind::BitwiseNot(_)
            | ExpressionKind::Cast(_)
            | ExpressionKind::Requalify(_)
            | ExpressionKind::Establish { .. }
            | ExpressionKind::ShortCircuit { .. }
            | ExpressionKind::Call { .. } => return Ok(place),
        }
    }
}

/// A capture names exactly one subject in the active match scope. Each lookup
/// advances the cycle bound, including chains reached through projected fields.
pub(super) struct Captures<'a, C> {
    machine: MachineId,
    subjects: &'a [(Capture, C)],
    followed: usize,
}

impl<'a, C: Borrow<Expression>> Captures<'a, C> {
    pub(super) fn new(machine: MachineId, subjects: &'a [(Capture, C)]) -> Self {
        Self {
            machine,
            subjects,
            followed: 0,
        }
    }

    pub(super) fn resolve(&mut self, capture: Capture) -> Checked<Option<&'a Expression>> {
        let invalid = |reason| refuse(self.machine, Construct::Machine, Unserved::Call(reason));
        if self.followed == self.subjects.len() {
            return Err(invalid("cyclic saved subject capture"));
        }
        self.followed += 1;
        let mut found = self.subjects.iter().filter(|(known, _)| *known == capture);
        let Some((_, subject)) = found.next() else {
            return Err(invalid("saved subject outside its match capture"));
        };
        if found.next().is_some() {
            return Err(invalid("ambiguous saved subject capture"));
        }
        Ok(Some(subject.borrow()))
    }
}

pub(super) fn root_slot<C: Borrow<Expression>>(
    machine: MachineId,
    place: &Expression,
    subjects: &[(Capture, C)],
) -> Checked<Option<usize>> {
    let mut captures = Captures::new(machine, subjects);
    let root = walk(place, |capture| captures.resolve(capture), |_, _| Ok(()))?;
    Ok(match root.kind {
        ExpressionKind::Slot(slot) => Some(slot),
        _ => None,
    })
}

/// A saved subject already denotes captured storage; recognizing that fact
/// does not need to resolve its original slot or replay its index expressions.
pub(super) fn names_storage(place: &Expression) -> bool {
    let root = walk::<std::convert::Infallible>(place, |_| Ok(None), |_, _| Ok(()));
    match root {
        Ok(root) => matches!(
            root.kind,
            ExpressionKind::Slot(_) | ExpressionKind::SavedSubject(_)
        ),
        Err(never) => match never {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SourceOrigin, Unpositioned};

    fn expression(kind: ExpressionKind) -> Expression {
        Expression {
            kind,
            result: None,
            source: SourceOrigin::Unpositioned(Unpositioned::SpanAbsent),
        }
    }

    fn capture_pair() -> (Capture, Capture) {
        let mut typed = typed_trees::TypedTrees::default();
        let marker = typed
            .expression_table
            .insert(typed_trees::expression::ExpressionNode::Boolean(true));
        (
            typed.expression_table.insert_match_subject(marker),
            typed.expression_table.insert_match_subject(marker),
        )
    }

    #[test]
    fn projected_capture_visits_every_owner_without_visiting_selectors() {
        let machine = MachineId::new(1).unwrap();
        let (capture, _) = capture_pair();
        let subject = expression(ExpressionKind::Index {
            base: Box::new(expression(ExpressionKind::Field {
                base: Box::new(expression(ExpressionKind::Slot(3))),
                field: 0,
            })),
            index: Box::new(expression(ExpressionKind::Slot(9))),
        });
        let place = expression(ExpressionKind::Payload {
            base: Box::new(expression(ExpressionKind::SavedSubject(capture))),
            case: 0,
            field: 1,
        });
        let subjects = [(capture, subject)];
        let mut captures = Captures::new(machine, &subjects);
        let mut visited = Vec::new();
        let root = walk(
            &place,
            |capture| captures.resolve(capture),
            |projection, _| {
                visited.push(match projection.kind {
                    ExpressionKind::Field { .. } => "field",
                    ExpressionKind::Payload { .. } => "payload",
                    ExpressionKind::Index { .. } => "index",
                    _ => panic!("non-projection visited"),
                });
                Ok(())
            },
        )
        .unwrap();
        assert!(matches!(root.kind, ExpressionKind::Slot(3)));
        assert_eq!(visited, ["payload", "index", "field"]);
        assert_eq!(root_slot(machine, &place, &subjects).unwrap(), Some(3));
        let borrowed_subjects = [(capture, &subjects[0].1)];
        assert_eq!(
            root_slot(machine, &place, &borrowed_subjects).unwrap(),
            Some(3)
        );
        assert!(names_storage(&place));
        assert!(!names_storage(&expression(ExpressionKind::Boolean(true))));
        let refused: Result<_, &str> = walk(&place, |_| Ok(None), |_, _| Err("write denied"));
        assert_eq!(refused.unwrap_err(), "write denied");
    }

    #[test]
    fn unresolved_duplicate_and_cyclic_captures_never_supply_a_root() {
        let machine = MachineId::new(1).unwrap();
        let (capture, other) = capture_pair();
        let saved = || expression(ExpressionKind::SavedSubject(capture));
        let missing = [(other, expression(ExpressionKind::Slot(0)))];
        assert!(
            root_slot(machine, &saved(), &missing)
                .unwrap_err()
                .detail
                .contains("outside its match capture")
        );
        let duplicate = [
            (capture, expression(ExpressionKind::Slot(0))),
            (capture, expression(ExpressionKind::Slot(1))),
        ];
        assert!(
            root_slot(machine, &saved(), &duplicate)
                .unwrap_err()
                .detail
                .contains("ambiguous saved subject capture")
        );
        let cyclic = [(
            capture,
            expression(ExpressionKind::Field {
                base: Box::new(saved()),
                field: 0,
            }),
        )];
        assert!(
            root_slot(machine, &saved(), &cyclic)
                .unwrap_err()
                .detail
                .contains("cyclic saved subject capture")
        );
    }
}
