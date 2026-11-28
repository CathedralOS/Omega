//! Follow a projected place back to its value before each assignment.
//! A field write replaces that field's packages and leaves sibling paths alone.
use super::*;

pub(crate) type CopyPoint = Option<usize>;

#[derive(Default)]
pub(crate) struct CopyHistory(Vec<Change>);

struct Change {
    previous: CopyPoint,
    kind: ChangeKind,
}

enum ChangeKind {
    Assign(Assignment),
    Join(usize),
}

impl CopyHistory {
    fn push(&mut self, previous: CopyPoint, kind: ChangeKind) -> CopyPoint {
        let point = self.0.len();
        self.0.push(Change { previous, kind });
        Some(point)
    }

    pub(crate) fn assign(&mut self, previous: CopyPoint, assignment: Assignment) -> CopyPoint {
        if previous.is_none() && assignment.value.is_none() {
            return None;
        }
        self.push(previous, ChangeKind::Assign(assignment))
    }

    pub(crate) fn join(&mut self, left: CopyPoint, right: CopyPoint) -> CopyPoint {
        if left == right {
            return left;
        }
        left?;
        self.push(left, ChangeKind::Join(right?))
    }

    pub(crate) fn related(
        &self,
        program: &TypedTrees,
        state: &State,
        head: CopyPoint,
        left: &PackageSubject,
        right: &PackageSubject,
    ) -> bool {
        let mut paths = vec![(head, left.clone(), right.clone())];
        // A common predecessor of two arms can be visited twice with the same
        // pair. Expand each distinct question at a join only once.
        let mut joins: Vec<Vec<(PackageSubject, PackageSubject)>> = Vec::new();
        while let Some((mut point, mut left, mut right)) = paths.pop() {
            while left != right {
                let Some(index) = point else { return false };
                let change = &self.0[index];
                match &change.kind {
                    ChangeKind::Assign(assignment) => {
                        for place in [&mut left, &mut right] {
                            if !assignment.before(program, state, place) {
                                return false;
                            }
                        }
                    }
                    ChangeKind::Join(other) => {
                        if joins.len() <= index {
                            joins.resize_with(index + 1, Vec::new);
                        }
                        let pair = (left.clone(), right.clone());
                        if joins[index].contains(&pair) {
                            break;
                        }
                        joins[index].push(pair);
                        paths.push((Some(*other), left.clone(), right.clone()));
                    }
                }
                point = change.previous;
            }
        }
        true
    }
}
