//! Lookups visit only this application's signatures. The table keeps
//! separate lists for selected machines, requirement calls with their
//! lexical owner, and callback contracts. Insertion, replacement and
//! retirement maintain the lists, handle generations and arena order.

use super::{AppliedCallableOwner, AppliedCallableSignature};
use crate::source_state::static_applications::StaticMachineApplicationHandle;
use arena::{Arena, Handle, RootedArena};
use symbols::SymbolHandle;

type CallableRow = Handle<AppliedCallableSignature>;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum InvocationPurpose {
    #[default]
    MachineSelection,
    Requirement(SymbolHandle),
    StaticCallback,
}

fn invocation_owner(
    row: &AppliedCallableSignature,
) -> Option<(StaticMachineApplicationHandle, InvocationPurpose)> {
    match row.owner {
        AppliedCallableOwner::MachineSelection { invocation, .. } => {
            Some((invocation, InvocationPurpose::MachineSelection))
        }
        AppliedCallableOwner::RequirementInvocation {
            application,
            lexical_owner,
            ..
        } => Some((application, InvocationPurpose::Requirement(lexical_owner))),
        AppliedCallableOwner::StaticCallback { application, .. } => {
            Some((application, InvocationPurpose::StaticCallback))
        }
        _ => None,
    }
}

#[derive(Clone, Default)]
struct InvocationGroup {
    invocation: StaticMachineApplicationHandle,
    purpose: InvocationPurpose,
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
    // Continuations share unchanged signatures, but keep their invocation
    // links private so replacement and slot reuse cannot change a predecessor.
    arena: RootedArena<AppliedCallableSignature>,
    invocation_heads: Vec<Handle<InvocationGroup>>,
    invocation_groups: Arena<InvocationGroup>,
    row_links: Vec<RowLink>,
}

impl From<Arena<AppliedCallableSignature>> for AppliedCallableTable {
    fn from(arena: Arena<AppliedCallableSignature>) -> Self {
        let arena = RootedArena::from(arena);
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
    type Target = RootedArena<AppliedCallableSignature>;
    fn deref(&self) -> &Self::Target {
        &self.arena
    }
}

impl record_storage::Record for AppliedCallableTable {
    fn write(
        &self,
        output: &mut record_storage::RecordWriter,
    ) -> Result<(), record_storage::RecordError> {
        output.record(|output| record_storage::Record::write(&self.arena, output))
    }
    fn read<Context>(
        input: &mut record_storage::RecordReader<'_, Context>,
    ) -> Result<Self, record_storage::RecordError> {
        input.record(|input| {
            let arena =
                <RootedArena<AppliedCallableSignature> as record_storage::Record>::read(input)?;
            let layout = AssociationLayout::measure(&arena)
                .ok_or_else(|| input.error("applied callable association size overflow"))?;
            let allocation = layout
                .allocation_bytes()
                .ok_or_else(|| input.error("applied callable association allocation overflow"))?;
            input.allocate(allocation)?;
            Ok(Self::recover(arena, layout))
        })
    }
}

struct AssociationLayout {
    invocation_slots: usize,
    row_slots: usize,
    associated_rows: usize,
}

impl AssociationLayout {
    fn measure(arena: &RootedArena<AppliedCallableSignature>) -> Option<Self> {
        let mut layout = Self {
            invocation_slots: 0,
            row_slots: 0,
            associated_rows: 0,
        };
        for (handle, row) in arena.iter() {
            if let Some((invocation, _)) = invocation_owner(row) {
                layout.invocation_slots = layout
                    .invocation_slots
                    .max((invocation.arena_index() as usize).checked_add(1)?);
                layout.row_slots = layout
                    .row_slots
                    .max((handle.arena_index() as usize).checked_add(1)?);
                layout.associated_rows = layout.associated_rows.checked_add(1)?;
            }
        }
        Some(layout)
    }

    fn allocation_bytes(&self) -> Option<usize> {
        self.invocation_slots
            .checked_mul(std::mem::size_of::<Handle<InvocationGroup>>())?
            .checked_add(self.row_slots.checked_mul(std::mem::size_of::<RowLink>())?)?
            .checked_add(
                self.associated_rows
                    .checked_mul(std::mem::size_of::<InvocationGroup>())?,
            )?
            .checked_add(
                self.associated_rows
                    .checked_mul(std::mem::size_of::<CallableRow>())?,
            )
    }
}

impl AppliedCallableTable {
    fn recover(arena: RootedArena<AppliedCallableSignature>, layout: AssociationLayout) -> Self {
        let mut rows = Vec::with_capacity(layout.associated_rows);
        for (handle, row) in arena.iter() {
            if invocation_owner(row).is_some() {
                rows.push(handle);
            }
        }
        let mut table = Self {
            arena,
            invocation_heads: vec![Handle::default(); layout.invocation_slots],
            invocation_groups: Arena::with_capacity(layout.associated_rows),
            row_links: vec![RowLink::default(); layout.row_slots],
        };
        for row in rows {
            table.publish_row(row);
        }
        table
    }

    pub(crate) fn insert(&mut self, row: AppliedCallableSignature) -> CallableRow {
        let handle = self.arena.insert(row);
        self.publish_row(handle);
        handle
    }

    pub(crate) fn replace(
        &mut self,
        handle: CallableRow,
        row: AppliedCallableSignature,
    ) -> Result<(), &'static str> {
        if !self.arena.is_valid(handle) {
            return Err("applied callable replacement names a retired row");
        }
        self.remove_row(handle);
        self.arena.set(handle, row);
        self.publish_row(handle);
        Ok(())
    }

    pub(crate) fn free(&mut self, handle: CallableRow) -> bool {
        if !self.arena.is_valid(handle) {
            return false;
        }
        self.remove_row(handle);
        self.arena.free(handle)
    }

    fn group(
        &self,
        invocation: StaticMachineApplicationHandle,
        purpose: InvocationPurpose,
    ) -> Handle<InvocationGroup> {
        let mut handle = self
            .invocation_heads
            .get(invocation.arena_index() as usize)
            .copied()
            .unwrap_or_default();
        while handle.is_valid() {
            let group = self.invocation_groups.get(handle);
            if group.invocation == invocation && group.purpose == purpose {
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
        let Some((invocation, purpose)) = invocation_owner(self.arena.get(row)) else {
            return;
        };
        let mut group = self.group(invocation, purpose);
        if !group.is_valid() {
            let slot = invocation.arena_index() as usize;
            self.invocation_heads
                .resize(self.invocation_heads.len().max(slot + 1), Handle::default());
            group = self.invocation_groups.insert(InvocationGroup {
                invocation,
                purpose,
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
        let Some((invocation, purpose)) = invocation_owner(self.arena.get(row)) else {
            return;
        };
        let group = self.group(invocation, purpose);
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

    pub(crate) fn callback_rows(
        &self,
        invocation: StaticMachineApplicationHandle,
    ) -> impl Iterator<Item = (CallableRow, &AppliedCallableSignature)> {
        self.invocation_rows(invocation, InvocationPurpose::StaticCallback)
    }

    pub(crate) fn declaration_rows(
        &self,
        invocation: StaticMachineApplicationHandle,
        owner: SymbolHandle,
    ) -> impl Iterator<Item = (CallableRow, &AppliedCallableSignature)> {
        self.invocation_rows(invocation, InvocationPurpose::Requirement(owner))
    }

    pub(crate) fn machine_rows(
        &self,
        invocation: StaticMachineApplicationHandle,
    ) -> impl Iterator<Item = (CallableRow, &AppliedCallableSignature)> {
        self.invocation_rows(invocation, InvocationPurpose::MachineSelection)
    }

    fn invocation_rows(
        &self,
        invocation: StaticMachineApplicationHandle,
        purpose: InvocationPurpose,
    ) -> impl Iterator<Item = (CallableRow, &AppliedCallableSignature)> {
        let group = self.group(invocation, purpose);
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
#[path = "../tests/table.rs"]
pub(crate) mod tests;
