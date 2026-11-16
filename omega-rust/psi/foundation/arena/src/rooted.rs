//! Declaration roots name logical rows, not the allocation holding a record.
//! Projection appends row associations and keeps the records immutable. A write
//! copies one shared record into private output; it never detaches its cohort.

use std::alloc::Layout;
use std::fmt;
use std::ops::{Index, Range};
use std::sync::Arc;

use compiler_storage::shared_allocation_bytes;

use crate::arena::{RevisionStamp, refuse_unresolved, refuse_unresolved_span};
use crate::{Arena, ArenaSlotStorage, Handle, HandleSpan};

// Association pages bound a point write independently of the size of a record
// batch. Records themselves retain the producer's original Vec allocation.
const ASSOCIATIONS_PER_PAGE: usize = 128;

fn decoded_association_allocation<T>(length: usize) -> Option<usize> {
    let page_count = length.div_ceil(ASSOCIATIONS_PER_PAGE);
    // Page metadata includes the owning Arena box, exact free rosters and
    // the temporary count/vector/occupancy buffers used during partitioning.
    let page = shared_allocation_bytes(Layout::new::<Arena<RecordAddress<T>>>())?
        .checked_add(std::mem::size_of::<ArenaSlotStorage>() + 6 * std::mem::size_of::<usize>())?;
    length
        .checked_mul(std::mem::size_of::<RecordAddress<T>>() + 10)?
        .checked_add(page_count.checked_mul(page)?)?
        .checked_add(shared_allocation_bytes(Layout::new::<T>())?)?
        .checked_add(shared_allocation_bytes(Layout::new::<Vec<T>>())?)?
        .checked_add(std::mem::size_of::<Arc<Vec<T>>>())
}

struct RecordAddress<T> {
    batch: u32,
    row: u32,
    exclusive: bool,
    marker: std::marker::PhantomData<fn() -> T>,
}

impl<T> Default for RecordAddress<T> {
    fn default() -> Self {
        Self {
            batch: 0,
            row: 0,
            exclusive: false,
            marker: std::marker::PhantomData,
        }
    }
}
impl<T> Copy for RecordAddress<T> {}
impl<T> Clone for RecordAddress<T> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Versioned declaration rows over shared immutable record batches. A filtered
/// roster gets distinct handles while retaining its predecessors' record values.
/// Reads expose a borrowed view rather than manufacturing a contiguous slice.
pub struct RootedArena<T> {
    associations: Vec<Arc<Arena<RecordAddress<T>>>>,
    batches: Vec<Arc<Vec<T>>>,
    // Invalid handles keep the exact sentinel supplied by the owning arena.
    dummy: Arc<T>,
    // This is the Arena free order across association pages. Each page remains
    // the authority for the selected slot's generation and occupancy.
    free_slots: Vec<u32>,
    length: usize,
    active_count: usize,
    fresh_generation: u32,
    fresh: bool,
    explicit_slots: bool,
    revision: RevisionStamp,
    rewrite_revision: u64,
}

impl<T> Clone for RootedArena<T> {
    fn clone(&self) -> Self {
        Self {
            associations: self.associations.clone(),
            batches: self.batches.clone(),
            dummy: Arc::clone(&self.dummy),
            free_slots: self.free_slots.clone(),
            length: self.length,
            active_count: self.active_count,
            fresh_generation: self.fresh_generation,
            fresh: self.fresh,
            explicit_slots: self.explicit_slots,
            revision: self.revision.copy(),
            rewrite_revision: self.rewrite_revision,
        }
    }
}

impl<T: Default> Default for RootedArena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Default> RootedArena<T> {
    pub fn new() -> Self {
        Arena::new().into()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let mut result = Self::new();
        result.reserve(capacity);
        result
    }
}

impl<T> RootedArena<T> {
    pub(super) fn from_storage_parts(
        dummy: T,
        items: Vec<T>,
        mut slots: Option<ArenaSlotStorage>,
        fresh_generation: u32,
        fresh: bool,
    ) -> Self {
        let length = items.len();
        let free_slots = slots
            .as_mut()
            .map_or_else(Vec::new, |slots| std::mem::take(&mut slots.free_indices));
        let active_count = length - free_slots.len();
        let page_count = length.div_ceil(ASSOCIATIONS_PER_PAGE);
        let mut page_free_slots = slots.as_ref().map(|_| {
            let mut free_counts = vec![0usize; page_count];
            for slot in &free_slots {
                free_counts[(*slot as usize - 1) / ASSOCIATIONS_PER_PAGE] += 1;
            }
            let mut rosters = free_counts
                .into_iter()
                .map(Vec::with_capacity)
                .collect::<Vec<Vec<u32>>>();
            for slot in &free_slots {
                let position = *slot as usize - 1;
                rosters[position / ASSOCIATIONS_PER_PAGE]
                    .push((position % ASSOCIATIONS_PER_PAGE + 1) as u32);
            }
            rosters.into_iter()
        });
        let mut associations = Vec::with_capacity(page_count);
        for start in (0..length).step_by(ASSOCIATIONS_PER_PAGE) {
            let end = (start + ASSOCIATIONS_PER_PAGE).min(length);
            let addresses = (start..end)
                .map(|row| RecordAddress {
                    batch: 1,
                    row: row as u32,
                    exclusive: true,
                    marker: std::marker::PhantomData,
                })
                .collect();
            let page_slots = slots.as_ref().map(|slots| ArenaSlotStorage {
                generations: slots.generations[start..end].to_vec(),
                occupied: slots.occupied[start..end].to_vec(),
                free_indices: page_free_slots
                    .as_mut()
                    .expect("explicit association metadata")
                    .next()
                    .expect("one free roster per association page"),
            });
            associations.push(Arc::new(
                Arena::from_storage(
                    RecordAddress::default(),
                    addresses,
                    page_slots,
                    fresh_generation,
                    fresh,
                )
                .expect("partitioned valid arena storage"),
            ));
        }
        Self {
            associations,
            batches: if items.is_empty() {
                Vec::new()
            } else {
                vec![Arc::new(items)]
            },
            dummy: Arc::new(dummy),
            free_slots,
            length,
            active_count,
            fresh_generation,
            fresh,
            explicit_slots: slots.is_some(),
            revision: RevisionStamp::unstamped(),
            rewrite_revision: 0,
        }
    }
}

impl<T> RootedArena<T> {
    fn association_handle(handle: Handle<T>) -> Handle<RecordAddress<T>> {
        Handle::from_parts(
            (handle.arena_index() - 1) % ASSOCIATIONS_PER_PAGE as u32 + 1,
            handle.generation(),
        )
    }

    fn record(&self, address: RecordAddress<T>) -> &T {
        let batch = address
            .batch
            .checked_sub(1)
            .expect("logical declaration row has a retained record");
        &self.batches[batch as usize][address.row as usize]
    }

    #[inline]
    fn record_at_slot(&self, slot: usize) -> &T {
        let page = &self.associations[slot / ASSOCIATIONS_PER_PAGE];
        self.record(page.storage_slice()[slot % ASSOCIATIONS_PER_PAGE])
    }

    fn address(&self, handle: Handle<T>) -> Option<RecordAddress<T>> {
        let slot = handle.arena_index().checked_sub(1)? as usize;
        self.associations
            .get(slot / ASSOCIATIONS_PER_PAGE)?
            .try_get(Self::association_handle(handle))
            .copied()
    }

    fn stored_rows(&self) -> impl Iterator<Item = (&RecordAddress<T>, u32, bool)> {
        self.associations.iter().flat_map(|page| {
            let storage = page.storage_view();
            storage
                .items
                .iter()
                .enumerate()
                .map(move |(slot, address)| {
                    (
                        address,
                        storage
                            .slots
                            .map_or(storage.fresh_generation, |slots| slots.generations[slot]),
                        storage.slots.is_none_or(|slots| slots.occupied[slot]),
                    )
                })
        })
    }

    pub fn revision(&self) -> u64 {
        self.revision.get()
    }
    pub fn append_epoch(&self) -> Option<(u64, usize)> {
        self.fresh.then_some((self.rewrite_revision, self.length))
    }
    pub fn len(&self) -> usize {
        self.active_count
    }
    pub fn is_empty(&self) -> bool {
        self.active_count == 0
    }
    pub fn try_get(&self, handle: Handle<T>) -> Option<&T> {
        Some(self.record(self.address(handle)?))
    }
    pub fn is_valid(&self, handle: Handle<T>) -> bool {
        self.address(handle).is_some()
    }
    #[track_caller]
    pub fn get(&self, handle: Handle<T>) -> &T {
        if !handle.is_valid() {
            return &self.dummy;
        }
        self.try_get(handle)
            .unwrap_or_else(|| refuse_unresolved("get", handle.arena_index(), handle.generation()))
    }

    pub fn span(&self, span: HandleSpan<T>) -> Option<RootRows<'_, T>> {
        if !span.is_empty() {
            let start = span.start().arena_index().checked_sub(1)? as usize;
            let end = start
                .checked_add(span.len())
                .filter(|end| *end <= self.length)?;
            if self.fresh && span.start().generation() == self.fresh_generation {
                return Some(RootRows { arena: self, span });
            }
            let mut position = start;
            while position < end {
                let page = position / ASSOCIATIONS_PER_PAGE;
                let count =
                    (end - position).min(ASSOCIATIONS_PER_PAGE - position % ASSOCIATIONS_PER_PAGE);
                self.associations.get(page)?.span(HandleSpan::from_parts(
                    Handle::from_parts(
                        (position % ASSOCIATIONS_PER_PAGE + 1) as u32,
                        span.start().generation(),
                    ),
                    count as u32,
                ))?;
                position += count;
            }
        }
        Some(RootRows { arena: self, span })
    }

    #[track_caller]
    pub fn span_or_empty(&self, span: HandleSpan<T>) -> RootRows<'_, T> {
        self.span(span)
            .unwrap_or_else(|| refuse_unresolved_span("span_or_empty", span))
    }

    pub fn iter(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        self.associations
            .iter()
            .enumerate()
            .flat_map(move |(page, rows)| {
                rows.iter().map(move |(handle, address)| {
                    (
                        Handle::from_parts(
                            handle.arena_index() + (page * ASSOCIATIONS_PER_PAGE) as u32,
                            handle.generation(),
                        ),
                        self.record(*address),
                    )
                })
            })
    }

    /// Read every logical slot, including retired rows, in storage order.
    pub fn stored_values(&self) -> impl Iterator<Item = &T> {
        self.stored_rows()
            .map(|(address, _, _)| self.record(*address))
    }

    pub fn reserve(&mut self, additional: usize) {
        if additional == 0 {
            return;
        }
        let end = self
            .length
            .checked_add(additional)
            .filter(|end| *end <= u32::MAX as usize)
            .expect("declaration reservation exceeds slot domain");
        self.associations
            .reserve(end.div_ceil(ASSOCIATIONS_PER_PAGE) - self.associations.len());
        if let Some(batch) = self.batches.last_mut().and_then(Arc::get_mut) {
            batch.reserve(additional);
        } else {
            self.batches.push(Arc::new(Vec::with_capacity(additional)));
        }
    }

    fn append_record(&mut self, value: T) -> RecordAddress<T> {
        if self
            .batches
            .last()
            .is_none_or(|batch| Arc::strong_count(batch) != 1)
        {
            self.batches.push(Arc::new(Vec::new()));
        }
        let batch = self.batches.len();
        let rows = Arc::get_mut(&mut self.batches[batch - 1]).expect("private declaration output");
        let row = u32::try_from(rows.len()).expect("declaration batch row overflow");
        rows.push(value);
        RecordAddress {
            batch: u32::try_from(batch).expect("declaration batch count overflow"),
            row,
            exclusive: true,
            marker: std::marker::PhantomData,
        }
    }

    fn replace_record(&mut self, address: RecordAddress<T>, value: T) -> RecordAddress<T> {
        if address.exclusive
            && let Some(batch) = Arc::get_mut(&mut self.batches[address.batch as usize - 1])
        {
            batch[address.row as usize] = value;
            address
        } else {
            self.append_record(value)
        }
    }

    fn append_address(&mut self, address: RecordAddress<T>) -> Handle<T> {
        let slot = u32::try_from(self.length).expect("declaration slot overflow");
        let page = self.length / ASSOCIATIONS_PER_PAGE;
        if page == self.associations.len() {
            self.associations
                .push(Arc::new(Arena::with_capacity_and_generation(
                    ASSOCIATIONS_PER_PAGE,
                    self.fresh_generation,
                )));
        }
        let local = Arc::make_mut(&mut self.associations[page]).append(address);
        self.length += 1;
        self.active_count += 1;
        self.revision.invalidate();
        Handle::from_parts(
            slot.checked_add(1).expect("declaration slot overflow"),
            local.generation(),
        )
    }

    pub fn append(&mut self, value: T) -> Handle<T> {
        let address = self.append_record(value);
        self.append_address(address)
    }

    pub fn insert(&mut self, value: T) -> Handle<T> {
        let Some(slot) = self.free_slots.pop() else {
            return self.append(value);
        };
        let address = self.append_record(value);
        let position = slot as usize - 1;
        let local =
            Arc::make_mut(&mut self.associations[position / ASSOCIATIONS_PER_PAGE]).insert(address);
        assert_eq!(
            local.arena_index(),
            (position % ASSOCIATIONS_PER_PAGE + 1) as u32,
            "declaration free order disagrees with its owning page"
        );
        self.active_count += 1;
        self.rewrite();
        Handle::from_parts(slot, local.generation())
    }

    pub fn insert_many(&mut self, values: impl IntoIterator<Item = T>) -> HandleSpan<T> {
        let values = values.into_iter();
        self.reserve(values.size_hint().0);
        let mut span = HandleSpan::empty();
        for value in values {
            span.push_contiguous(self.append(value));
        }
        span
    }

    /// Replace a complete record without first copying the value being replaced.
    #[track_caller]
    pub fn set(&mut self, handle: Handle<T>, value: T) {
        let Some(address) = self.address(handle) else {
            refuse_unresolved("set", handle.arena_index(), handle.generation());
        };
        let replacement = self.replace_record(address, value);
        if replacement.batch != address.batch || replacement.row != address.row {
            let slot = handle.arena_index() as usize - 1;
            *Arc::make_mut(&mut self.associations[slot / ASSOCIATIONS_PER_PAGE])
                .get_mut(Self::association_handle(handle)) = replacement;
        }
        self.rewrite();
    }

    fn rewrite(&mut self) {
        self.revision.invalidate();
        self.rewrite_revision = self
            .rewrite_revision
            .checked_add(1)
            .expect("declaration revision overflow");
    }

    fn share_address(&mut self, handle: Handle<T>) -> RecordAddress<T> {
        let position = handle.arena_index() as usize - 1;
        let row = Arc::make_mut(&mut self.associations[position / ASSOCIATIONS_PER_PAGE])
            .get_mut(Self::association_handle(handle));
        row.exclusive = false;
        *row
    }

    /// Produce a distinct logical roster without copying its record values.
    pub fn project(
        &mut self,
        span: HandleSpan<T>,
        mut selected: impl FnMut(&T) -> bool,
    ) -> HandleSpan<T> {
        self.span_or_empty(span);
        let mut projected = HandleSpan::empty();
        for handle in span.handles() {
            if selected(self.get(handle)) {
                let address = self.share_address(handle);
                projected.push_contiguous(self.append_address(address));
            }
        }
        projected
    }

    pub fn append_to_span(&mut self, span: &mut HandleSpan<T>, value: T) -> Handle<T> {
        self.span_or_empty(*span);
        if !span.is_empty()
            && (span.start().generation() != self.fresh_generation
                || span.start().arena_index() as usize + span.len() != self.length + 1)
        {
            *span = self.project(*span, |_| true);
        }
        let handle = self.append(value);
        span.push_contiguous(handle);
        handle
    }
}

impl<T: Default> RootedArena<T> {
    pub fn free(&mut self, handle: Handle<T>) -> bool {
        let Some(address) = self.address(handle) else {
            return false;
        };
        let replacement = self.replace_record(address, T::default());
        let slot = handle.arena_index() as usize - 1;
        let freed = Arc::make_mut(&mut self.associations[slot / ASSOCIATIONS_PER_PAGE])
            .retire_with_replacement(Self::association_handle(handle), || replacement);
        if freed {
            self.active_count -= 1;
            self.free_slots.push(handle.arena_index());
            self.fresh = false;
            self.explicit_slots = true;
            self.rewrite();
        }
        freed
    }
}

impl<T: Clone> RootedArena<T> {
    #[track_caller]
    pub fn get_mut(&mut self, handle: Handle<T>) -> &mut T {
        let Some(mut address) = self.address(handle) else {
            refuse_unresolved("get_mut", handle.arena_index(), handle.generation());
        };
        if !address.exclusive
            || address.batch == 0
            || Arc::strong_count(&self.batches[address.batch as usize - 1]) != 1
        {
            address = self.append_record(self.record(address).clone());
            let slot = handle.arena_index() as usize - 1;
            *Arc::make_mut(&mut self.associations[slot / ASSOCIATIONS_PER_PAGE])
                .get_mut(Self::association_handle(handle)) = address;
        }
        self.rewrite();
        &mut Arc::get_mut(&mut self.batches[address.batch as usize - 1])
            .expect("private declaration record")[address.row as usize]
    }
}

pub struct RootRows<'arena, T> {
    arena: &'arena RootedArena<T>,
    span: HandleSpan<T>,
}
impl<T> Copy for RootRows<'_, T> {}
impl<T> Clone for RootRows<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'arena, T> RootRows<'arena, T> {
    pub fn len(self) -> usize {
        self.span.len()
    }
    pub fn is_empty(self) -> bool {
        self.span.is_empty()
    }
    #[inline]
    pub fn get(self, position: usize) -> Option<&'arena T> {
        // The view checked the complete span once. Its immutable arena borrow
        // keeps those slots live, even when another fork changes its own rows.
        (position < self.len()).then(|| {
            self.arena
                .record_at_slot(self.span.start().arena_index() as usize - 1 + position)
        })
    }
    #[track_caller]
    pub fn at(self, position: usize) -> &'arena T {
        self.get(position)
            .expect("declaration view position out of bounds")
    }
    pub fn first(self) -> Option<&'arena T> {
        self.get(0)
    }
    pub fn last(self) -> Option<&'arena T> {
        self.len()
            .checked_sub(1)
            .and_then(|position| self.get(position))
    }
    pub fn iter(self) -> RootRowsIter<'arena, T> {
        RootRowsIter {
            rows: self,
            positions: 0..self.len(),
        }
    }
}
impl<T> Index<usize> for RootRows<'_, T> {
    type Output = T;
    fn index(&self, position: usize) -> &T {
        self.get(position)
            .expect("declaration view position out of bounds")
    }
}
pub struct RootRowsIter<'arena, T> {
    rows: RootRows<'arena, T>,
    positions: Range<usize>,
}
impl<'arena, T> Iterator for RootRowsIter<'arena, T> {
    type Item = &'arena T;
    fn next(&mut self) -> Option<Self::Item> {
        self.rows.get(self.positions.next()?)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.positions.size_hint()
    }
}
impl<T> DoubleEndedIterator for RootRowsIter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.rows.get(self.positions.next_back()?)
    }
}
impl<T> ExactSizeIterator for RootRowsIter<'_, T> {}
impl<'arena, T> IntoIterator for RootRows<'arena, T> {
    type Item = &'arena T;
    type IntoIter = RootRowsIter<'arena, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'arena, T> IntoIterator for &RootRows<'arena, T> {
    type Item = &'arena T;
    type IntoIter = RootRowsIter<'arena, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<T: fmt::Debug> fmt::Debug for RootRows<'_, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.iter()).finish()
    }
}
impl<T: fmt::Debug> fmt::Debug for RootedArena<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.iter()).finish()
    }
}
impl<T: PartialEq> PartialEq for RootedArena<T> {
    fn eq(&self, other: &Self) -> bool {
        *self.dummy == *other.dummy
            && self.length == other.length
            && self.fresh_generation == other.fresh_generation
            && self.free_slots == other.free_slots
            && self.stored_rows().zip(other.stored_rows()).all(
                |((left, lg, lo), (right, rg, ro))| {
                    lg == rg && lo == ro && self.record(*left) == other.record(*right)
                },
            )
    }
}
impl<T: Eq> Eq for RootedArena<T> {}

impl<T: compiler_storage::Record> compiler_storage::Record for RootedArena<T> {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        output.record(|output| {
            (*self.dummy).write(output)?;
            output.record(|output| {
                output.sequence(self.length)?;
                for (address, _, _) in self.stored_rows() {
                    self.record(*address).write(output)?;
                }
                Ok(())
            })?;
            self.explicit_slots.write(output)?;
            if self.explicit_slots {
                output.record(|output| {
                    output.record(|output| {
                        output.sequence(self.length)?;
                        for (_, generation, _) in self.stored_rows() {
                            generation.write(output)?;
                        }
                        Ok(())
                    })?;
                    output.record(|output| {
                        output.sequence(self.length)?;
                        for (_, _, occupied) in self.stored_rows() {
                            occupied.write(output)?;
                        }
                        Ok(())
                    })?;
                    self.free_slots.write(output)
                })?;
            }
            self.fresh_generation.write(output)?;
            self.fresh.write(output)
        })
    }
    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        let arena = Arena::<T>::read(input)?;
        let length = arena.storage_view().items.len();
        let additional = decoded_association_allocation::<T>(length)
            .ok_or_else(|| input.error("declaration association allocation overflow"))?;
        input.allocate(additional)?;
        Ok(Self::from(arena))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct Tracked {
        copies: Arc<AtomicUsize>,
        value: usize,
    }
    impl Clone for Tracked {
        fn clone(&self) -> Self {
            self.copies.fetch_add(1, Ordering::Relaxed);
            Self {
                copies: Arc::clone(&self.copies),
                value: self.value,
            }
        }
    }

    #[test]
    fn forks_and_projected_versions_copy_only_the_written_record() {
        let copies = Arc::new(AtomicUsize::new(0));
        let mut original = RootedArena::new();
        let roots = original.insert_many((0..400).map(|value| Tracked {
            copies: Arc::clone(&copies),
            value,
        }));
        let mut fork = original.clone();
        assert!(Arc::ptr_eq(
            &original.associations[0],
            &fork.associations[0]
        ));
        let projected = fork.project(roots, |record| record.value < 100 && record.value % 2 == 0);
        assert_ne!(roots.start(), projected.start());
        assert!(std::ptr::eq(
            original.get(roots.start()),
            fork.get(projected.start())
        ));
        assert_eq!(copies.load(Ordering::Relaxed), 0);
        fork.get_mut(projected.start()).value = 900;
        assert_eq!(copies.load(Ordering::Relaxed), 1);
        assert_eq!(fork.get(roots.start()).value, 0);
        assert_eq!(original.get(roots.start()).value, 0);
        fork.get_mut(projected.start()).value = 901;
        assert_eq!(copies.load(Ordering::Relaxed), 1);
        original.get_mut(roots.start()).value = 902;
        assert_eq!(copies.load(Ordering::Relaxed), 2);
        assert_eq!(fork.get(roots.start()).value, 0);
        assert_eq!(fork.get(projected.start()).value, 901);
        assert!(std::ptr::eq(
            original.get(roots.handle_at(1)),
            fork.get(roots.handle_at(1))
        ));
        assert!(Arc::ptr_eq(
            &original.associations[2],
            &fork.associations[2]
        ));
    }

    #[test]
    fn aliases_protect_both_versions_without_an_external_fork() {
        let mut rows = RootedArena::new();
        let roots = rows.insert_many([String::from("first"), String::from("second")]);
        let projected = rows.project(roots, |_| true);
        rows.get_mut(roots.start()).push_str(" original");
        rows.get_mut(projected.handle_at(1)).push_str(" projected");
        assert_eq!(rows.get(projected.start()), "first");
        assert_eq!(rows.get(roots.handle_at(1)), "second");
    }

    #[test]
    fn reserved_batches_survive_append_and_replacement_without_record_copies() {
        let copies = Arc::new(AtomicUsize::new(0));
        let mut rows = RootedArena::with_capacity(65);
        let original_pointer = rows.batches[0].as_ptr();
        let roots = rows.insert_many((0..65).map(|value| Tracked {
            copies: Arc::clone(&copies),
            value,
        }));
        assert_eq!(rows.batches.len(), 1);
        assert_eq!(rows.batches[0].as_ptr(), original_pointer);
        let original = rows.clone();
        rows.set(
            roots.start(),
            Tracked {
                copies: Arc::clone(&copies),
                value: 99,
            },
        );
        assert_eq!(copies.load(Ordering::Relaxed), 0);
        assert_eq!(original.get(roots.start()).value, 0);
        assert_eq!(rows.get(roots.start()).value, 99);
    }

    #[test]
    fn logical_codec_and_retirement_order_match_the_contiguous_arena() {
        let mut contiguous = Arena::new();
        let roots = contiguous.insert_many((0..300u32).map(|value| value + 1));
        let limits = compiler_storage::RecordLimits::default();
        let bytes = compiler_storage::encode(&contiguous, limits).unwrap();
        let mut rooted: RootedArena<u32> = compiler_storage::decode(&bytes, limits).unwrap();
        for offset in [240, 1, 129, 2] {
            assert!(contiguous.free(roots.handle_at(offset)));
            assert!(rooted.free(roots.handle_at(offset)));
        }
        for value in [400, 401, 402] {
            assert_eq!(rooted.insert(value), contiguous.insert(value));
        }
        let projection = rooted.project(HandleSpan::from_parts(roots.handle_at(3), 100), |value| {
            *value % 2 == 0
        });
        let expected = contiguous
            .span_or_empty(HandleSpan::from_parts(roots.handle_at(3), 100))
            .iter()
            .copied()
            .filter(|value| *value % 2 == 0)
            .collect::<Vec<_>>();
        let expected_projection = contiguous.insert_many(expected);
        assert_eq!(projection, expected_projection);
        assert_eq!(
            compiler_storage::encode(&rooted, limits).unwrap(),
            compiler_storage::encode(&contiguous, limits).unwrap()
        );
        let decoded: RootedArena<u32> =
            compiler_storage::decode(&compiler_storage::encode(&rooted, limits).unwrap(), limits)
                .unwrap();
        assert_eq!(rooted, decoded);
        assert!(!rooted.is_valid(roots.handle_at(240)));
        assert!(rooted.span(roots).is_none());
        assert!(rooted.try_get(Handle::invalid()).is_none());
        assert_eq!(*rooted.get(Handle::invalid()), 0);
    }

    #[test]
    fn views_cross_association_pages_and_retained_span_extension_keeps_old_rows() {
        let mut rows = RootedArena::new();
        let mut roots = rows.insert_many(0..300u32);
        let original = roots;
        rows.append(1000);
        rows.append_to_span(&mut roots, 300);
        assert_ne!(original.start(), roots.start());
        assert_eq!(
            rows.span_or_empty(roots)
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            (0..301u32).collect::<Vec<_>>()
        );
        assert_eq!(rows.span_or_empty(original).len(), 300);
        let stale = HandleSpan::from_parts(
            Handle::from_parts(roots.start().arena_index(), 2),
            roots.count(),
        );
        assert!(rows.span(stale).is_none());
        assert!(std::ptr::eq(
            rows.get(original.start()),
            rows.get(roots.start())
        ));
        rows.get_mut(original.start()).clone_from(&99);
        assert_eq!(*rows.get(roots.start()), 0);
    }

    #[test]
    fn shared_storage_does_not_make_nan_equal_to_itself() {
        let mut rows = RootedArena::new();
        rows.append(f64::NAN);
        assert_ne!(rows, rows.clone());
    }

    #[test]
    fn retired_partial_pages_fit_the_charged_decode_allocation() {
        use compiler_storage::{RecordLimits, decode, encode};
        let mut contiguous = Arena::new();
        let roots = contiguous.insert_many(0..193u32);
        for handle in roots.handles() {
            assert!(contiguous.free(handle));
        }
        let bytes = encode(&contiguous, RecordLimits::default()).unwrap();
        let length = roots.len();
        let pages = length.div_ceil(ASSOCIATIONS_PER_PAGE);
        let additional = decoded_association_allocation::<u32>(length).unwrap();
        // The ordinary decoder charges the value, generation, occupancy and
        // free-list vectors. Rooted storage moves that free list, not copies it.
        let total = 13 * length + additional;
        let limits = RecordLimits {
            allocation: total,
            ..RecordLimits::default()
        };
        let decoded: RootedArena<u32> = decode(&bytes, limits).unwrap();
        assert!(
            decode::<RootedArena<u32>>(
                &bytes,
                RecordLimits {
                    allocation: total - 1,
                    ..limits
                }
            )
            .is_err()
        );
        let mut owned = decoded.associations.capacity()
            * std::mem::size_of::<Arc<Arena<RecordAddress<u32>>>>()
            + decoded.batches.capacity() * std::mem::size_of::<Arc<Vec<u32>>>()
            + std::mem::size_of::<Vec<u32>>()
            + 2 * std::mem::size_of::<usize>()
            + std::mem::size_of::<u32>()
            + 2 * std::mem::size_of::<usize>();
        for page in &decoded.associations {
            owned += std::mem::size_of::<Arena<RecordAddress<u32>>>()
                + 2 * std::mem::size_of::<usize>()
                + page.storage_allocation_bytes();
        }
        // Include the temporary counts, page-vector headers and occupancy
        // validation scratch that no longer exists on the returned value.
        owned += pages * (std::mem::size_of::<usize>() + std::mem::size_of::<Vec<u32>>()) + length;
        assert!(
            additional >= owned,
            "charged {additional} bytes, constructed {owned} bytes"
        );
        assert_eq!(
            decoded.associations[1]
                .storage_view()
                .slots
                .unwrap()
                .free_indices
                .capacity(),
            65
        );
    }

    #[test]
    fn retirement_retains_each_replacement_and_decode_does_not_invoke_default() {
        static DEFAULTS: AtomicUsize = AtomicUsize::new(0);
        #[derive(Debug, PartialEq, Eq, compiler_storage::Record)]
        struct Stateful(u32);
        impl Default for Stateful {
            fn default() -> Self {
                Self(DEFAULTS.fetch_add(1, Ordering::Relaxed) as u32)
            }
        }
        let mut rows = RootedArena::new();
        let roots = rows.insert_many([Stateful(100), Stateful(101)]);
        let before = DEFAULTS.load(Ordering::Relaxed);
        for handle in roots.handles() {
            assert!(rows.free(handle));
        }
        assert_eq!(DEFAULTS.load(Ordering::Relaxed), before + 2);
        assert_eq!(
            rows.stored_values()
                .map(|value| value.0)
                .collect::<Vec<_>>(),
            [before as u32, before as u32 + 1]
        );
        let limits = compiler_storage::RecordLimits::default();
        let bytes = compiler_storage::encode(&rows, limits).unwrap();
        let restored: RootedArena<Stateful> = compiler_storage::decode(&bytes, limits).unwrap();
        assert_eq!(rows, restored);
        assert_eq!(DEFAULTS.load(Ordering::Relaxed), before + 2);
    }

    #[test]
    fn retiring_an_exclusive_record_releases_its_owned_payload() {
        #[derive(Default)]
        struct Payload(Arc<AtomicUsize>);
        impl Drop for Payload {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        let dropped = Arc::new(AtomicUsize::new(0));
        let mut rows = RootedArena::new();
        let handle = rows.append(Payload(Arc::clone(&dropped)));
        assert!(rows.free(handle));
        assert_eq!(dropped.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn over_aligned_zero_sized_sentinels_are_charged_before_sharing() {
        #[repr(align(4096))]
        #[derive(Default, compiler_storage::Record)]
        struct Aligned;
        let original = Arena::<Aligned>::new();
        let limits = compiler_storage::RecordLimits::default();
        let bytes = compiler_storage::encode(&original, limits).unwrap();
        let required = decoded_association_allocation::<Aligned>(0).unwrap();
        assert!(shared_allocation_bytes(Layout::new::<Aligned>()).unwrap() >= 4096);
        assert!(required >= 4096);
        assert!(
            compiler_storage::decode::<RootedArena<Aligned>>(
                &bytes,
                compiler_storage::RecordLimits {
                    allocation: required - 1,
                    ..limits
                }
            )
            .is_err()
        );
        assert!(
            compiler_storage::decode::<RootedArena<Aligned>>(
                &bytes,
                compiler_storage::RecordLimits {
                    allocation: required,
                    ..limits
                }
            )
            .is_ok()
        );
    }

    #[test]
    fn validated_views_keep_their_rows_when_a_fork_retires_and_reuses_slots() {
        let mut original = RootedArena::new();
        let roots = original.insert_many(0..300u32);
        let mut fork = original.clone();
        let view = original.span_or_empty(roots);
        assert!(fork.free(roots.handle_at(129)));
        let reused = fork.insert(900);
        assert_eq!(reused.arena_index(), roots.handle_at(129).arena_index());
        assert_ne!(reused.generation(), roots.start().generation());
        assert!(fork.span(roots).is_none());
        fork.set(roots.handle_at(1), 901);
        assert_eq!(
            view.iter().copied().collect::<Vec<_>>(),
            (0..300u32).collect::<Vec<_>>()
        );
        assert_eq!(view.get(300), None);
    }
}
