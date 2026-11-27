//! Known contents of activation-local storage. A by-value copy gets a new
//! home. References inside the copy still select the original allocation.

mod contents;
mod join;
mod location;
pub(super) use join::Arrivals;
use location::Allocation;
pub(super) use location::Location;

use super::super::facts::Facts;
use super::condition::Assumption;
pub(super) use contents::Stored;
use semantic_vocabulary::PlaceId;
use std::cell::Cell;
use std::rc::Rc;
use terminal_psi::StructuralPathSegment as Segment;

#[derive(Clone, Default)]
pub(super) struct Storage {
    next_allocation: Rc<Cell<usize>>,
    pub(super) conditions: Rc<Vec<Assumption>>,
    contents: Rc<Facts<Allocation, Stored>>,
    /// A write through a selected reference establishes its value even
    /// when the individual candidate allocations only receive weak updates.
    selected: Rc<Vec<(Location, Stored)>>,
}

impl Storage {
    pub(super) fn requirements<'a>(
        &self,
        prior: &'a [semantic_vocabulary::Proposition],
    ) -> std::borrow::Cow<'a, [semantic_vocabulary::Proposition]> {
        let mut result = std::borrow::Cow::Borrowed(prior);
        if !self.conditions.is_empty() {
            result
                .to_mut()
                .extend(self.conditions.iter().filter_map(|test| test.proposition()));
        }
        result
    }

    pub(super) fn assume(&mut self, condition: Assumption) {
        if !self.conditions.contains(&condition) {
            Rc::make_mut(&mut self.conditions).push(condition);
        }
    }

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
        if !Rc::ptr_eq(&self.conditions, &other.conditions) {
            Rc::make_mut(&mut self.conditions)
                .retain(|condition| other.conditions.contains(condition));
        }
    }

    pub(super) fn allocate(&mut self, contents: Stored) -> Location {
        self.allocate_with(contents, Allocation::Private)
    }

    pub(super) fn input(&mut self) -> Location {
        self.allocate_with(Stored::INCOMING, Allocation::ExclusiveInput)
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
        bindings: &super::activation::Bindings,
        root: PlaceId,
        path: &[Segment],
    ) -> Option<Location> {
        let mut location = bindings.places.get(&root)?.clone();
        for (i, segment) in path.iter().enumerate() {
            if let Segment::RuntimeIndex { index, .. } = segment {
                let root_type = bindings.machine.structural_place_type(root)?;
                let extent =
                    super::super::spin::fixed_extent(bindings.module, root_type, &path[..i])?;
                location = Self::index(location, bindings.values.get(index).copied(), extent)?;
            } else {
                location = self.project(location, std::slice::from_ref(segment))?;
            }
        }
        Some(location)
    }

    /// The runtime bounds judgment admits exactly one element. An unresolved
    /// selector can choose any in-bounds element, even in a very large array.
    pub(super) fn index(
        mut location: Location,
        operand: Option<super::super::progress::ScalarFact>,
        extent: u64,
    ) -> Option<Location> {
        if extent == 0 {
            return None;
        }
        let segment = match operand.and_then(|value| value.literal()) {
            Some(value) => {
                let index = super::super::progress::ScalarFact::Literal(value).index()?;
                if index >= extent {
                    return None;
                }
                Segment::FixedIndex(index)
            }
            None if extent == 1 => Segment::FixedIndex(0),
            None => {
                let id = operand.and_then(|value| match value {
                    super::super::progress::ScalarFact::EntryInput(input) => Some(input.id),
                    _ => None,
                });
                location.select(extent, id);
                return Some(location);
            }
        };
        location.path.push(segment);
        Some(location)
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
        if location.tracks_selection() {
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
                .read(path);
            match &mut result {
                Some(prior) => Stored::join(prior, &value),
                None => result = Some(value),
            }
        });
        let mut value = result.unwrap_or_default();
        if location.tracks_selection() {
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
                .read(path);
            match home {
                Allocation::SharedInput(_) => caller = true,
                Allocation::ExclusiveInput(_) if value.is_incoming() => caller = true,
                _ => local |= can_block(&value),
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
        if location.tracks_selection() && !contained && !shared {
            choices.push((location.clone(), new.clone()));
        }
        let contents = Rc::make_mut(&mut self.contents);
        location.each_home(|allocation, path| {
            if matches!(allocation, Allocation::SharedInput(_)) {
                return;
            }
            let value = contents.get_or_insert_with(allocation, Stored::default);
            value.write(path, &new, weak);
        });
    }
}
