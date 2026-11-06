//! Applied-callable production publishes each invocation's exact rows.
//!
//! Inserts, replacement and retirement maintain the association before returning.
//! Readers never prepare storage or scan unrelated callable rows. Dense slots
//! locate invocation groups; each group retains the complete invocation handle,
//! including its generation. Row links preserve the arena's iteration order.
use super::{AppliedCallableOwner, AppliedCallableSignature};
use crate::signature::StateSignature;
use crate::static_applications::StaticMachineApplicationHandle;
use arena::{Arena, Handle};

type CallableRow = Handle<AppliedCallableSignature>;

#[derive(Clone, Default)]
struct InvocationGroup {
    invocation: StaticMachineApplicationHandle,
    next: Handle<InvocationGroup>,
    first: CallableRow,
    last: CallableRow,
}

#[derive(Clone, Copy, Default)]
struct RowLink {
    row: CallableRow,
    next: CallableRow,
}

#[derive(Clone, Default)]
pub(crate) struct AppliedCallableTable {
    arena: Arena<AppliedCallableSignature>,
    invocation_heads: Vec<Handle<InvocationGroup>>,
    invocation_groups: Arena<InvocationGroup>,
    row_links: Vec<RowLink>,
}

impl From<Arena<AppliedCallableSignature>> for AppliedCallableTable {
    fn from(arena: Arena<AppliedCallableSignature>) -> Self {
        let layout = AssociationLayout::measure(&arena)
            .expect("produced callable association must fit the host's storage sizes");
        Self::recover(arena, layout)
    }
}

impl PartialEq for AppliedCallableTable {
    fn eq(&self, other: &Self) -> bool {
        self.arena == other.arena
    }
}
impl Eq for AppliedCallableTable {}
impl std::fmt::Debug for AppliedCallableTable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.arena.fmt(formatter)
    }
}
impl std::ops::Deref for AppliedCallableTable {
    type Target = Arena<AppliedCallableSignature>;
    fn deref(&self) -> &Self::Target {
        &self.arena
    }
}

// Preserve the callable arena's existing encoding. Recovery publishes its
// association once from the recovered rows, before exposing the table.
impl compiler_storage::Record for AppliedCallableTable {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        compiler_storage::Record::write(&self.arena, output)
    }
    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        let arena = <Arena<AppliedCallableSignature> as compiler_storage::Record>::read(input)?;
        let layout = AssociationLayout::measure(&arena)
            .ok_or_else(|| input.error("applied callable association size overflow"))?;
        let allocation = layout
            .allocation_bytes()
            .ok_or_else(|| input.error("applied callable association allocation overflow"))?;
        input.allocate(allocation)?;
        Ok(Self::recover(arena, layout))
    }
}

// Recovery measures every association buffer before allocating. Exact initial
// capacities avoid both geometric growth and Vec's minimum growth capacity.
struct AssociationLayout {
    invocation_slots: usize,
    row_slots: usize,
    selections: usize,
}

impl AssociationLayout {
    fn measure(arena: &Arena<AppliedCallableSignature>) -> Option<Self> {
        let mut layout = Self {
            invocation_slots: 0,
            row_slots: 0,
            selections: 0,
        };
        for (handle, row) in arena.iter() {
            if let AppliedCallableOwner::MachineSelection { invocation, .. } = &row.owner {
                layout.invocation_slots = layout
                    .invocation_slots
                    .max((invocation.arena_index() as usize).checked_add(1)?);
                layout.row_slots = layout
                    .row_slots
                    .max((handle.arena_index() as usize).checked_add(1)?);
                layout.selections = layout.selections.checked_add(1)?;
            }
        }
        Some(layout)
    }

    fn allocation_bytes(&self) -> Option<usize> {
        self.invocation_slots
            .checked_mul(std::mem::size_of::<Handle<InvocationGroup>>())?
            .checked_add(self.row_slots.checked_mul(std::mem::size_of::<RowLink>())?)?
            .checked_add(
                self.selections
                    .checked_mul(std::mem::size_of::<InvocationGroup>())?,
            )?
            .checked_add(
                self.selections
                    .checked_mul(std::mem::size_of::<CallableRow>())?,
            )
    }
}

impl AppliedCallableTable {
    fn recover(arena: Arena<AppliedCallableSignature>, layout: AssociationLayout) -> Self {
        let mut rows = Vec::with_capacity(layout.selections);
        for (handle, row) in arena.iter() {
            if matches!(&row.owner, AppliedCallableOwner::MachineSelection { .. }) {
                rows.push(handle);
            }
        }
        let mut table = Self {
            arena,
            invocation_heads: vec![Handle::default(); layout.invocation_slots],
            invocation_groups: Arena::with_capacity(layout.selections),
            row_links: vec![RowLink::default(); layout.row_slots],
        };
        for row in rows {
            table.publish_row(row);
        }
        table
    }

    pub(super) fn insert(&mut self, row: AppliedCallableSignature) -> CallableRow {
        let handle = self.arena.insert(row);
        self.publish_row(handle);
        handle
    }

    pub(super) fn replace(
        &mut self,
        handle: CallableRow,
        row: AppliedCallableSignature,
    ) -> Result<(), &'static str> {
        if !self.arena.is_valid(handle) {
            return Err("applied callable replacement names a retired row");
        }
        self.remove_row(handle);
        *self.arena.get_mut(handle) = row;
        self.publish_row(handle);
        Ok(())
    }

    pub(super) fn free(&mut self, handle: CallableRow) -> bool {
        if !self.arena.is_valid(handle) {
            return false;
        }
        self.remove_row(handle);
        self.arena.free(handle)
    }

    pub(super) fn replace_signature(
        &mut self,
        handle: CallableRow,
        signature: StateSignature,
    ) -> Result<(), &'static str> {
        if !self.arena.is_valid(handle) {
            return Err("applied callable signature replacement names a retired row");
        }
        self.arena.get_mut(handle).signature = signature;
        Ok(())
    }

    pub(super) fn set_required(
        &mut self,
        handle: CallableRow,
        required: StateSignature,
    ) -> Result<(), &'static str> {
        if !self.arena.is_valid(handle) {
            return Err("applied callable requirement names a retired row");
        }
        let AppliedCallableOwner::MachineSelection {
            required: destination,
            ..
        } = &mut self.arena.get_mut(handle).owner
        else {
            return Err("applied callable requirement names a non-selection row");
        };
        *destination = Some(required);
        Ok(())
    }

    fn group(&self, invocation: StaticMachineApplicationHandle) -> Handle<InvocationGroup> {
        let mut handle = self
            .invocation_heads
            .get(invocation.arena_index() as usize)
            .copied()
            .unwrap_or_default();
        while handle.is_valid() {
            let group = self.invocation_groups.get(handle);
            if group.invocation == invocation {
                return handle;
            }
            handle = group.next;
        }
        Handle::default()
    }

    fn next_row(&self, row: CallableRow) -> CallableRow {
        let link = self.row_links[row.arena_index() as usize];
        debug_assert_eq!(link.row, row);
        link.next
    }

    fn publish_row(&mut self, row: CallableRow) {
        let AppliedCallableOwner::MachineSelection { invocation, .. } = self.arena.get(row).owner
        else {
            return;
        };
        let mut group = self.group(invocation);
        if !group.is_valid() {
            let slot = invocation.arena_index() as usize;
            self.invocation_heads
                .resize(self.invocation_heads.len().max(slot + 1), Handle::default());
            group = self.invocation_groups.insert(InvocationGroup {
                invocation,
                next: self.invocation_heads[slot],
                ..Default::default()
            });
            self.invocation_heads[slot] = group;
        }
        let slot = row.arena_index() as usize;
        self.row_links
            .resize(self.row_links.len().max(slot + 1), RowLink::default());
        self.row_links[slot] = RowLink {
            row,
            next: CallableRow::default(),
        };
        let tail = self.invocation_groups.get(group).last;
        if !tail.is_valid() {
            let group = self.invocation_groups.get_mut(group);
            group.first = row;
            group.last = row;
        } else if tail.arena_index() < row.arena_index() {
            // New rows normally append. Only a recycled arena slot needs to
            // find an earlier position within this one invocation's rows.
            self.row_links[tail.arena_index() as usize].next = row;
            self.invocation_groups.get_mut(group).last = row;
        } else {
            let mut previous = CallableRow::default();
            let mut next = self.invocation_groups.get(group).first;
            while next.is_valid() && next.arena_index() < row.arena_index() {
                previous = next;
                next = self.next_row(next);
            }
            self.row_links[slot].next = next;
            if previous.is_valid() {
                self.row_links[previous.arena_index() as usize].next = row;
            } else {
                self.invocation_groups.get_mut(group).first = row;
            }
        }
    }

    fn remove_row(&mut self, row: CallableRow) {
        let AppliedCallableOwner::MachineSelection { invocation, .. } = self.arena.get(row).owner
        else {
            return;
        };
        let group = self.group(invocation);
        let mut previous = CallableRow::default();
        let mut current = self.invocation_groups.get(group).first;
        while current.is_valid() && current != row {
            previous = current;
            current = self.next_row(current);
        }
        debug_assert_eq!(current, row);
        let next = self.next_row(row);
        if previous.is_valid() {
            self.row_links[previous.arena_index() as usize].next = next;
        } else {
            self.invocation_groups.get_mut(group).first = next;
        }
        if self.invocation_groups.get(group).last == row {
            self.invocation_groups.get_mut(group).last = previous;
        }
        self.row_links[row.arena_index() as usize] = RowLink::default();
        if self.invocation_groups.get(group).first.is_valid() {
            return;
        }
        let slot = invocation.arena_index() as usize;
        let successor = self.invocation_groups.get(group).next;
        let mut prior = Handle::<InvocationGroup>::default();
        let mut current = self.invocation_heads[slot];
        while current != group {
            prior = current;
            current = self.invocation_groups.get(current).next;
        }
        if prior.is_valid() {
            self.invocation_groups.get_mut(prior).next = successor;
        } else {
            self.invocation_heads[slot] = successor;
        }
        self.invocation_groups.free(group);
    }

    pub(super) fn machine_rows(
        &self,
        invocation: StaticMachineApplicationHandle,
    ) -> impl Iterator<Item = (CallableRow, &AppliedCallableSignature)> {
        let group = self.group(invocation);
        let mut next = if group.is_valid() {
            self.invocation_groups.get(group).first
        } else {
            CallableRow::default()
        };
        std::iter::from_fn(move || {
            if !next.is_valid() {
                return None;
            }
            let row = next;
            next = self.next_row(row);
            Some((row, self.arena.get(row)))
        })
    }
}

#[cfg(test)]
mod tests;
