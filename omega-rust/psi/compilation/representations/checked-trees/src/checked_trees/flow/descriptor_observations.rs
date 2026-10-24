//! Descriptor observation publication owns its exact context association.
//! The paged arena is canonical; the sparse sorted context roster is derived
//! lookup storage. Appending updates one context batch, and readers visit only
//! the observations contributing to their selected contexts. Page transfer and
//! reset stay with this owner so an index cannot outlive its observation rows.

use super::FlowDescriptorContextObservation;
use arena::{Handle, PagedArenaSet};
use facts::FactContextHandle;

#[derive(Debug, Clone, Default)]
struct ContextObservations {
    context: FactContextHandle,
    rows: Vec<Handle<FlowDescriptorContextObservation>>,
}

#[derive(Debug, Clone, Default)]
pub struct FlowDescriptorObservationTable {
    rows: PagedArenaSet<FlowDescriptorContextObservation>,
    by_context: Vec<ContextObservations>,
}

// The index is derived lookup storage, not another semantic representation.
impl PartialEq for FlowDescriptorObservationTable {
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows
    }
}
impl Eq for FlowDescriptorObservationTable {}

fn key(context: FactContextHandle) -> (u32, u32) {
    (context.generation(), context.arena_index())
}

impl FlowDescriptorObservationTable {
    fn index(&mut self, context: FactContextHandle, row: Handle<FlowDescriptorContextObservation>) {
        let position = match self
            .by_context
            .binary_search_by_key(&key(context), |batch| key(batch.context))
        {
            Ok(position) => position,
            Err(position) => {
                self.by_context.insert(
                    position,
                    ContextObservations {
                        context,
                        rows: Vec::new(),
                    },
                );
                position
            }
        };
        self.by_context[position].rows.push(row);
    }

    fn rebuild_index(&mut self) {
        self.by_context.clear();
        let mut rows = self
            .rows
            .iter()
            .map(|(handle, row)| (row.context, handle))
            .collect::<Vec<_>>();
        rows.sort_by_key(|(context, row)| (key(*context), row.generation(), row.arena_index()));
        for (context, row) in rows {
            if self
                .by_context
                .last()
                .is_none_or(|batch| batch.context != context)
            {
                self.by_context.push(ContextObservations {
                    context,
                    rows: Vec::new(),
                });
            }
            self.by_context
                .last_mut()
                .expect("context batch appended above")
                .rows
                .push(row);
        }
    }

    pub fn append(
        &mut self,
        row: FlowDescriptorContextObservation,
    ) -> Handle<FlowDescriptorContextObservation> {
        let context = row.context;
        let handle = self.rows.append(row);
        self.index(context, handle);
        handle
    }

    pub fn for_context(
        &self,
        context: FactContextHandle,
    ) -> impl Iterator<Item = &FlowDescriptorContextObservation> {
        let position = self
            .by_context
            .binary_search_by_key(&key(context), |batch| key(batch.context))
            .ok();
        position
            .into_iter()
            .flat_map(|position| self.by_context[position].rows.iter())
            .map(|handle| self.rows.get(*handle))
    }

    pub fn reset_retain_capacity(&mut self) {
        self.rows.reset_retain_capacity();
        self.by_context.clear();
    }

    pub fn begin_page(&mut self, generation: u32) {
        // The arena admits only a new, disjoint generation. Existing row
        // handles and their context associations therefore remain unchanged.
        self.rows.begin_page(generation);
    }

    pub fn take_additional_pages(&mut self) -> Self {
        let mut taken = Self {
            rows: self.rows.take_additional_pages(),
            by_context: Vec::new(),
        };
        self.rebuild_index();
        taken.rebuild_index();
        taken
    }

    pub fn adopt_additional_pages(&mut self, other: Self) {
        self.rows.adopt_additional_pages(other.rows);
        self.rebuild_index();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(context: FactContextHandle, index: u32) -> FlowDescriptorContextObservation {
        FlowDescriptorContextObservation {
            context,
            observation: Handle::from_parts(index, 1),
        }
    }

    #[test]
    fn descriptor_context_batches_preserve_all_observations_and_generations() {
        let context = FactContextHandle::from_parts(5, 1);
        let replacement = FactContextHandle::from_parts(5, 2);
        let unrelated = FactContextHandle::from_parts(6, 1);
        let mut table = FlowDescriptorObservationTable::default();
        table.append(observation(context, 1));
        table.append(observation(unrelated, 2));
        table.append(observation(context, 3));
        table.append(observation(replacement, 4));
        assert_eq!(
            table
                .for_context(context)
                .map(|row| row.observation.arena_index())
                .collect::<Vec<_>>(),
            [1, 3]
        );
        assert_eq!(
            table
                .for_context(replacement)
                .map(|row| row.observation.arena_index())
                .collect::<Vec<_>>(),
            [4]
        );
        assert_eq!(table.for_context(unrelated).count(), 1);
        assert_eq!(table.for_context(FactContextHandle::invalid()).count(), 0);
    }

    #[test]
    fn descriptor_context_batches_follow_page_transfer_and_reset() {
        let context = FactContextHandle::from_parts(5, 1);
        let mut source = FlowDescriptorObservationTable::default();
        source.append(observation(context, 1));
        source.begin_page(2);
        source.append(observation(context, 2));
        let taken = source.take_additional_pages();
        assert_eq!(
            source
                .for_context(context)
                .map(|row| row.observation.arena_index())
                .collect::<Vec<_>>(),
            [1]
        );
        assert_eq!(
            taken
                .for_context(context)
                .map(|row| row.observation.arena_index())
                .collect::<Vec<_>>(),
            [2]
        );
        source.adopt_additional_pages(taken);
        assert_eq!(
            source
                .for_context(context)
                .map(|row| row.observation.arena_index())
                .collect::<Vec<_>>(),
            [1, 2]
        );
        source.reset_retain_capacity();
        assert_eq!(source.for_context(context).count(), 0);
        source.append(observation(context, 3));
        assert_eq!(
            source
                .for_context(context)
                .map(|row| row.observation.arena_index())
                .collect::<Vec<_>>(),
            [3]
        );
    }
    #[test]
    fn descriptor_context_batches_keep_disjoint_pages_and_clones() {
        let old_context = FactContextHandle::from_parts(5, 1);
        let new_context = FactContextHandle::from_parts(6, 1);
        let mut table = FlowDescriptorObservationTable::default();
        table.begin_page(2);
        table.append(observation(old_context, 1));
        let retained = table.clone();
        table.begin_page(3);
        assert_eq!(table.for_context(old_context).count(), 1);
        table.append(observation(new_context, 2));
        assert_eq!(table.for_context(old_context).count(), 1);
        assert_eq!(table.for_context(new_context).count(), 1);
        assert_eq!(retained.for_context(old_context).count(), 1);
        assert_eq!(retained.for_context(new_context).count(), 0);
    }
}
