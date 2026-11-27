//! Known contents of activation-local storage. A by-value copy gets a new
//! home. References inside the copy still select the original allocation.

mod contents;

use super::super::facts::Facts;
pub(super) use contents::Stored;
use semantic_vocabulary::PlaceId;
use std::cell::Cell;
use std::rc::Rc;
use terminal_psi::StructuralPathSegment as Segment;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Location {
    allocation: usize,
    path: Vec<Segment>,
}

#[derive(Clone, Default)]
pub(super) struct Storage {
    next_allocation: Rc<Cell<usize>>,
    contents: Rc<Facts<usize, Stored>>,
}

impl Storage {
    pub(super) fn join(&mut self, other: &Self) {
        if Rc::ptr_eq(&self.contents, &other.contents) {
            return;
        }
        Rc::make_mut(&mut self.contents).retain(|location, value| {
            if let Some(incoming) = other.contents.get(location) {
                value.join(incoming);
                true
            } else {
                false
            }
        });
    }

    pub(super) fn allocate(&mut self, contents: Stored) -> Location {
        let allocation = self.next_allocation.get();
        self.next_allocation.set(allocation + 1);
        Rc::make_mut(&mut self.contents).insert(allocation, contents);
        Location {
            allocation,
            path: Vec::new(),
        }
    }

    pub(super) fn locate(
        &self,
        places: &Facts<PlaceId, Location>,
        root: PlaceId,
        path: &[Segment],
    ) -> Option<Location> {
        self.project(places.get(&root)?.clone(), path)
    }

    pub(super) fn project(&self, mut location: Location, path: &[Segment]) -> Option<Location> {
        for segment in path {
            match segment {
                Segment::Field(_) | Segment::FixedIndex(_) => location.path.push(segment.clone()),
                Segment::Referent => {
                    let Stored::Reference(referent) = self.get(&location) else {
                        return None;
                    };
                    location = referent.clone();
                }
                _ => return None,
            }
        }
        Some(location)
    }

    pub(super) fn get(&self, location: &Location) -> &Stored {
        let mut value = self
            .contents
            .get(&location.allocation)
            .unwrap_or(&Stored::Unknown);
        for field in &location.path {
            value = value.field(field);
        }
        value
    }

    pub(super) fn set(&mut self, location: &Location, new: Stored) {
        let mut value = Rc::make_mut(&mut self.contents)
            .get_or_insert_with(location.allocation, Stored::default);
        for field in &location.path {
            value = value.field_mut(field);
        }
        *value = new;
    }
}
