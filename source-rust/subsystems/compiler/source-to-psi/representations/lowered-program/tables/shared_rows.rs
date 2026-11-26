//! Entering a lowered loop through different machines can produce identical
//! tables of places and scalar values. Keep one copy of each distinct table.
//! Row identities are still local to the machine that reads them.

use std::collections::HashSet;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use arena::IdTable;
use semantic_vocabulary::ValueId;

use super::{ExactPlaceId, MachineTables, bindings::PlaceRow, values::ValueRow};

#[derive(Debug, Clone)]
pub struct CheckingRows<Id, T>(Arc<IdTable<Id, T>>);

impl<Id, T> Default for CheckingRows<Id, T> {
    fn default() -> Self {
        Self(Arc::default())
    }
}

impl<Id, T> Deref for CheckingRows<Id, T> {
    type Target = IdTable<Id, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Id: Clone, T: Clone> DerefMut for CheckingRows<Id, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.0)
    }
}

impl<Id: Eq + Hash, T: Eq + Hash> CheckingRows<Id, T> {
    fn share(&mut self, tables: &mut HashSet<Arc<IdTable<Id, T>>>) {
        if let Some(existing) = tables.get(self.0.as_ref()) {
            self.0 = Arc::clone(existing);
        } else {
            tables.insert(Arc::clone(&self.0));
        }
    }
}

#[derive(Default)]
pub struct LoopCheckingRows {
    values: HashSet<Arc<IdTable<ValueId, ValueRow>>>,
    places: HashSet<Arc<IdTable<ExactPlaceId, PlaceRow>>>,
}

impl LoopCheckingRows {
    pub fn retain(&mut self, machine: &mut MachineTables) {
        machine.values.share(&mut self.values);
        machine.places.share(&mut self.places);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::PlaceOrigin;
    use crate::values::ValueOrigin;
    use semantic_vocabulary::{PlaceId, ScalarType};

    fn tables(position: u32) -> MachineTables {
        let mut tables = MachineTables::default();
        tables.values.push(ValueRow {
            scalar_type: ScalarType::Boolean,
            origin: ValueOrigin::MachineParameter { position },
            rigid: None,
        });
        tables.places.push(PlaceRow {
            origin: PlaceOrigin::Root(PlaceId::new(1).unwrap()),
            children: Vec::new(),
        });
        tables
    }

    #[test]
    fn sharing_places_keeps_each_entrys_writes() {
        use crate::bindings::{Content, WriteRow, record_write};
        use crate::{MachineId, ProgramPoint};
        use semantic_vocabulary::BlockId;

        let mut store = LoopCheckingRows::default();
        let mut untouched = tables(0);
        let mut written = tables(0);
        let place = written.places.keys().next().unwrap();
        record_write(
            MachineId::new(1).unwrap(),
            &written.places,
            &mut written.writes,
            WriteRow {
                place,
                at: ProgramPoint::BlockEntry(BlockId::new(1).unwrap()),
                content: Content::Overwritten,
            },
        )
        .unwrap();

        store.retain(&mut untouched);
        store.retain(&mut written);
        assert!(Arc::ptr_eq(&untouched.places.0, &written.places.0));
        assert!(untouched.writes.is_empty());
        assert_eq!(written.writes.values().next().unwrap().place, place);
    }

    #[test]
    fn loop_entries_share_equal_rows_and_keep_later_writes_local() {
        let mut store = LoopCheckingRows::default();
        let mut first = tables(0);
        let mut same = tables(0);
        let mut different = tables(1);
        store.retain(&mut first);
        store.retain(&mut same);
        store.retain(&mut different);
        assert!(Arc::ptr_eq(&first.places.0, &same.places.0));
        assert!(Arc::ptr_eq(&first.values.0, &same.values.0));
        assert!(Arc::ptr_eq(&first.places.0, &different.places.0));
        assert!(!Arc::ptr_eq(&first.values.0, &different.values.0));

        let id = ValueId::new(1).unwrap();
        let replacement = different.values.get(id).unwrap().origin;
        same.values.get_mut(id).unwrap().origin = replacement;
        assert_ne!(same.values.get(id), first.values.get(id));
        assert_eq!(same.values.get(id), different.values.get(id));
    }
}
