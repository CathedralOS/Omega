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
    Transfer {
        source: PackageSubject,
        destination: PackageSubject,
    },
    Write(PackageSubject),
    Join(usize),
}

impl CopyHistory {
    fn push(&mut self, previous: CopyPoint, kind: ChangeKind) -> CopyPoint {
        let point = self.0.len();
        self.0.push(Change { previous, kind });
        Some(point)
    }

    pub(crate) fn transfer(
        &mut self,
        previous: CopyPoint,
        source: PackageSubject,
        destination: PackageSubject,
    ) -> CopyPoint {
        if source == destination {
            return previous;
        }
        self.push(
            previous,
            ChangeKind::Transfer {
                source,
                destination,
            },
        )
    }

    pub(crate) fn written(
        &mut self,
        previous: CopyPoint,
        place: Option<&PackageSubject>,
    ) -> CopyPoint {
        previous?;
        self.push(previous, ChangeKind::Write(place?.clone()))
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
                    ChangeKind::Transfer {
                        source,
                        destination,
                    } => {
                        for place in [&mut left, &mut right] {
                            if !before_transfer(program, state, place, source, destination) {
                                return false;
                            }
                        }
                    }
                    ChangeKind::Write(place) => {
                        if write_replaces(program, state, place, &left)
                            || write_replaces(program, state, place, &right)
                        {
                            return false;
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

/// Translate a read of a copied field to the same field of the input.
/// An uncertain overlapping assignment cannot establish a common origin.
fn before_transfer(
    program: &TypedTrees,
    state: &State,
    place: &mut PackageSubject,
    source: &PackageSubject,
    destination: &PackageSubject,
) -> bool {
    if place.root == destination.root
        && let Some(suffix) = place.steps.strip_prefix(destination.steps.as_slice())
    {
        let mut origin = source.clone();
        origin.steps.extend_from_slice(suffix);
        *place = origin;
        return true;
    }
    !write_replaces(program, state, destination, place)
}
