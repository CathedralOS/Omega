//! Known contents of activation-local storage. A by-value copy gets a new
//! home. References inside the copy still select the original allocation.

mod contents;
mod join;
mod location;
pub(super) use join::Arrivals;
use location::Allocation;
pub(super) use location::Location;

use super::super::facts::Facts;
pub(super) use contents::Stored;
use semantic_vocabulary::PlaceId;
use std::cell::Cell;
use std::rc::Rc;
use terminal_psi::StructuralPathSegment as Segment;

#[derive(Clone, Default)]
pub(super) struct Storage {
    next_allocation: Rc<Cell<usize>>,
    contents: Rc<Facts<Allocation, Stored>>,
    /// A write through a selected reference establishes its value even
    /// when the individual candidate allocations only receive weak updates.
    selected: Rc<Vec<(Location, Stored)>>,
}

impl Storage {
    pub(super) fn join(&mut self, other: &Self, mut arrivals: Arrivals<'_>) {
        if !Rc::ptr_eq(&self.selected, &other.selected) {
            Rc::make_mut(&mut self.selected).retain_mut(|(place, value)| {
                if let Some((_, incoming)) = other.selected.iter().find(|(home, _)| home == place) {
                    arrivals.value(value, incoming);
                    true
                } else {
                    false
                }
            });
        }
        if !Rc::ptr_eq(&self.contents, &other.contents) {
            // Each fork allocates distinct homes. A reference to a home
            // created on just one arm can only have arrived from that arm.
            let contents = Rc::make_mut(&mut self.contents);
            for (id, incoming) in other.contents.entries() {
                if let Some(current) = contents.get_mut(id) {
                    arrivals.value(current, incoming);
                } else {
                    contents.insert(*id, incoming.clone());
                }
            }
        }
        arrivals.record(self);
    }

    pub(super) fn allocate(&mut self, contents: Stored) -> Location {
        self.allocate_with(contents, Allocation::Private)
    }

    pub(super) fn input(&mut self) -> Location {
        self.allocate_with(Stored::Unknown, Allocation::ExclusiveInput)
    }

    fn allocate_with(&mut self, contents: Stored, home: fn(usize) -> Allocation) -> Location {
        let id = self.next_allocation.get();
        let allocation = home(id);
        self.next_allocation.set(id + 1);
        Rc::make_mut(&mut self.contents).insert(allocation, contents);
        Location::allocation(allocation)
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
                Segment::Case(tag) if self.get(&location).tag() == Some(*tag) => {
                    location.path.push(segment.clone());
                }
                Segment::Referent if !location.is_choice() && location.includes_shared() => {
                    location.path.push(segment.clone());
                }
                Segment::Referent => {
                    let Stored::Reference(referent) = self.get(&location) else {
                        return None;
                    };
                    location = referent;
                }
                _ => return None,
            }
        }
        Some(location)
    }

    pub(super) fn get(&self, location: &Location) -> Stored {
        let mut result = None;
        if location.is_choice() {
            for (home, stored) in self.selected.iter() {
                if let Some(path) = home.suffix(location) {
                    return stored.project(path).clone();
                }
            }
        }
        location.each_home(|allocation, path| {
            let value = self
                .contents
                .get(&allocation)
                .unwrap_or(&Stored::Unknown)
                .project(path);
            match &mut result {
                Some(prior) => Stored::join(prior, value),
                None => result = Some(value.clone()),
            }
        });
        let mut value = result.unwrap_or_default();
        if location.is_choice() {
            for (home, stored) in self.selected.iter() {
                if let Some(path) = location.suffix(home) {
                    *value.project_mut(path) = stored.clone();
                }
            }
        }
        value
    }

    /// A caller can account for an unknown incoming value. It cannot
    /// establish progress for a private candidate that this activation left held.
    pub(super) fn only_caller_can_block(
        &self,
        location: &Location,
        mut can_block: impl FnMut(&Stored) -> bool,
    ) -> bool {
        let mut caller = false;
        let mut local = false;
        location.each_home(|home, path| {
            let value = self
                .contents
                .get(&home)
                .unwrap_or(&Stored::Unknown)
                .project(path);
            match home {
                Allocation::SharedInput(_) => caller = true,
                Allocation::ExclusiveInput(_) if matches!(value, Stored::Unknown) => caller = true,
                _ => local |= can_block(value),
            }
        });
        caller && !local
    }

    pub(super) fn set(&mut self, location: &Location, new: Stored) {
        let choices = Rc::make_mut(&mut self.selected);
        let shared = location.includes_shared();
        let mut contained = false;
        choices.retain_mut(|(home, stored)| {
            if !shared && let Some(path) = home.suffix(location) {
                *stored.project_mut(path) = new.clone();
                contained = true;
                return true;
            }
            !home.may_be_written_by(location)
        });
        let weak = location.is_choice();
        if weak && !contained && !shared {
            choices.push((location.clone(), new.clone()));
        }
        let contents = Rc::make_mut(&mut self.contents);
        location.each_home(|allocation, path| {
            if matches!(allocation, Allocation::SharedInput(_)) {
                return;
            }
            let value = contents
                .get_or_insert_with(allocation, Stored::default)
                .project_mut(path);
            if weak {
                value.join(&new);
            } else {
                *value = new.clone();
            }
        });
    }
}
