//! A machine issues scalar producers first, then names the storage it observes.
//! Two entries to the same local loop can issue different producers and still
//! have exactly the same sequence of storage observations. The split preserves
//! their original dense ValueIds while sharing either sequence independently.

use arena::IdTable;
use semantic_vocabulary::ValueId;
use std::{collections::HashSet, sync::Arc};

use super::{Checked, Construct, MachineId, checked_row, values::ValueRow};

#[derive(Debug, Default, Clone)]
pub struct MachineValues {
    produced: Arc<Vec<ValueRow>>,
    observation_tail: Arc<Vec<ValueRow>>,
}

impl MachineValues {
    pub fn len(&self) -> usize {
        self.produced.len() + self.observation_tail.len()
    }

    pub fn is_empty(&self) -> bool {
        self.produced.is_empty() && self.observation_tail.is_empty()
    }

    pub fn push(&mut self, row: ValueRow) -> ValueId {
        let id = ValueId::new(self.len() as u64 + 1).expect("issued value IDs are nonzero");
        // A checking pass can append a derived value after observations. It
        // stays at the end of the same logical table.
        let section = if self.observation_tail.is_empty() && !row.origin.is_storage_observation() {
            &mut self.produced
        } else {
            &mut self.observation_tail
        };
        Arc::make_mut(section).push(row);
        id
    }

    pub fn get(&self, id: ValueId) -> Option<&ValueRow> {
        let offset = usize::try_from(id.get() - 1).ok()?;
        match offset.checked_sub(self.produced.len()) {
            Some(i) => self.observation_tail.get(i),
            None => self.produced.get(offset),
        }
    }

    pub fn checked(
        &self,
        id: ValueId,
        machine: MachineId,
        concerns: Construct,
    ) -> Checked<&ValueRow> {
        checked_row(self.get(id), machine, concerns)
    }

    pub fn get_mut(&mut self, id: ValueId) -> Option<&mut ValueRow> {
        let offset = usize::try_from(id.get() - 1).ok()?;
        let (section, i) = match offset.checked_sub(self.produced.len()) {
            Some(i) if i < self.observation_tail.len() => (&mut self.observation_tail, i),
            Some(_) => return None,
            None => (&mut self.produced, offset),
        };
        Arc::make_mut(section).get_mut(i)
    }

    pub fn identity_at(&self, index: usize) -> Option<ValueId> {
        (index < self.len()).then(|| ValueId::new(index as u64 + 1).expect("nonzero value ID"))
    }

    pub fn keys(&self) -> impl ExactSizeIterator<Item = ValueId> + DoubleEndedIterator {
        (0..self.len()).map(|i| ValueId::new(i as u64 + 1).expect("nonzero value ID"))
    }

    pub fn values(&self) -> impl DoubleEndedIterator<Item = &ValueRow> {
        self.produced.iter().chain(self.observation_tail.iter())
    }

    pub fn iter(&self) -> impl Iterator<Item = (ValueId, &ValueRow)> {
        self.keys().zip(self.values())
    }

    pub fn map<T>(&self, mut derive: impl FnMut(ValueId, &ValueRow) -> T) -> IdTable<ValueId, T> {
        IdTable::from_rows(self.iter().map(|(id, row)| (id, derive(id, row))))
            .expect("issued value IDs are dense")
    }

    pub(super) fn share(&mut self, sections: &mut HashSet<Arc<Vec<ValueRow>>>) {
        for section in [&mut self.produced, &mut self.observation_tail] {
            if let Some(existing) = sections.get(section.as_ref()) {
                *section = Arc::clone(existing);
            } else {
                sections.insert(Arc::clone(section));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExactPlaceId, NumericId, values::ValueOrigin};
    use semantic_vocabulary::ScalarType;

    fn row(origin: ValueOrigin) -> ValueRow {
        ValueRow {
            scalar_type: ScalarType::Boolean,
            origin,
            rigid: None,
        }
    }

    #[test]
    fn equal_observations_keep_the_ids_of_each_entry() {
        let first = row(ValueOrigin::MachineParameter { position: 0 });
        let second = row(ValueOrigin::MachineParameter { position: 1 });
        let observed = row(ValueOrigin::StorageContent {
            place: ExactPlaceId::new(1).unwrap(),
            after: None,
        });
        let mut short = MachineValues::default();
        let mut long = MachineValues::default();
        short.push(first);
        long.push(first);
        long.push(second);
        let short_observation = short.push(observed);
        let long_observation = long.push(observed);
        let mut sections = HashSet::new();
        short.share(&mut sections);
        long.share(&mut sections);
        assert!(!Arc::ptr_eq(&short.produced, &long.produced));
        assert!(Arc::ptr_eq(&short.observation_tail, &long.observation_tail));
        assert_ne!(short_observation, long_observation);
        assert_eq!(short.get(short_observation), long.get(long_observation));
        assert_eq!(long.get(short_observation), Some(&second));

        let appended = long.push(row(ValueOrigin::MachineResult));
        assert_eq!(long.identity_at(3), Some(appended));
        assert!(short.get(appended).is_none());
        *short.get_mut(short_observation).unwrap() = second;
        assert_eq!(long.get(long_observation), Some(&observed));
        assert_eq!(short.get(short_observation), Some(&second));
        assert_eq!(
            long.map(|_, row| row.origin).get(appended),
            Some(&ValueOrigin::MachineResult)
        );
    }
}
