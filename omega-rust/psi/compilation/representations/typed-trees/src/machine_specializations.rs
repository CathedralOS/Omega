/*
The specialization receipts of a program, and the one index that finds an
instance's receipts without reading the others.

Stage 05, proof scope and build-time evaluation ask "which receipt made this
instance?" once per dispatched call, and declaration queries ask it while
walking instance chains. Filtering every receipt for each question cost
D*S symbol comparisons for D questions over S receipts. The rows stay in the
order they were recorded, because a row's position names it (conformance
sites and stage 04's capture table store it).

Stage 04 pushes receipts while it is still looking instances up, and the
copied instance symbols arrive in no promised slot order. One sorted index
would shift on almost every push, and a table by symbol slot would size
itself by the highest slot rather than by the receipts. So `by_instance` is a
few sorted runs, each holding the row numbers of one contiguous range of rows
ordered by `(instance, row)`; older ranges come first. A push appends a run
of one row and merges the newest runs while the newer is at least as long as
the one before it. Run lengths then fall strictly from oldest to newest, so
there are at most log2(S) + 1 runs, every row is merged O(log S) times, and
storage is one position per receipt. A lookup binary-searches each run, oldest
first, so an instance's receipts come out in recording order.

The index is part of the table, not a cache beside it: every way of changing
the rows goes through this type and leaves the index exact. Reads see the rows
as a slice. Changing a row's instance in place is possible only inside
[`MachineSpecializations::edit`], which rebuilds the index afterwards.
Decoding rebuilds it from the rows alone, so a stored program never carries a
stale index and its size follows the decoded rows. Callers still judge what
they find: zero rows, two rows, or a stale binding application are refused
where they were before.
*/

use crate::MachineSpecialization;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default)]
pub struct MachineSpecializations {
    rows: Vec<MachineSpecialization>,
    /// Row numbers, one per row, as consecutive sorted runs; see the module
    /// note. A function of `rows` and of how they were recorded.
    by_instance: Vec<usize>,
    /// The length of each run in `by_instance`, oldest first.
    runs: Vec<usize>,
}

/// The total order the index sorts instances by: the parts symbol equality
/// compares.
fn instance_order(instance: SymbolHandle) -> (u32, u32) {
    (instance.arena_index(), instance.generation())
}

impl MachineSpecializations {
    pub fn from_rows(rows: Vec<MachineSpecialization>) -> Self {
        let mut table = Self {
            rows,
            by_instance: Vec::new(),
            runs: Vec::new(),
        };
        table.reindex();
        table
    }

    /// One run over every row.
    fn reindex(&mut self) {
        self.by_instance.clear();
        self.by_instance.extend(0..self.rows.len());
        self.runs.clear();
        if !self.rows.is_empty() {
            self.runs.push(self.rows.len());
        }
        let rows = &self.rows;
        // Stable, so equal instances keep row order.
        self.by_instance
            .sort_by_key(|&row| instance_order(rows[row].instance));
    }

    /// Every receipt recorded for `instance`, with its row number, in
    /// recording order. Uniqueness is the caller's judgement.
    pub fn for_instance_rows(
        &self,
        instance: SymbolHandle,
    ) -> impl Iterator<Item = (usize, &MachineSpecialization)> + '_ {
        let key = instance_order(instance);
        let order = move |row: &usize| instance_order(self.rows[*row].instance);
        let mut start = 0;
        self.runs
            .iter()
            .flat_map(move |&length| {
                let run = &self.by_instance[start..start + length];
                start += length;
                let first = run.partition_point(|row| order(row) < key);
                let end = first + run[first..].partition_point(|row| order(row) == key);
                &run[first..end]
            })
            .map(|&row| (row, &self.rows[row]))
    }

    /// Every receipt recorded for `instance`, in recording order.
    pub fn for_instance(
        &self,
        instance: SymbolHandle,
    ) -> impl Iterator<Item = &MachineSpecialization> + '_ {
        self.for_instance_rows(instance).map(|(_, row)| row)
    }

    pub fn push(&mut self, receipt: MachineSpecialization) {
        self.by_instance.push(self.rows.len());
        self.rows.push(receipt);
        self.runs.push(1);
        while let [.., older, newer] = self.runs[..] {
            if newer < older {
                break;
            }
            self.runs.pop();
            let merged = older + newer;
            if let Some(older) = self.runs.last_mut() {
                *older = merged;
            }
            let start = self.by_instance.len() - merged;
            let rows = &self.rows;
            // Row numbers are distinct, so the full key is a total order.
            self.by_instance[start..]
                .sort_unstable_by_key(|&row| (instance_order(rows[row].instance), row));
        }
    }

    pub fn retain(&mut self, keep: impl FnMut(&MachineSpecialization) -> bool) {
        self.rows.retain(keep);
        self.reindex();
    }

    pub fn clear(&mut self) {
        self.rows.clear();
        self.by_instance.clear();
        self.runs.clear();
    }

    /// Change the rows freely, then rebuild the index once.
    pub fn edit<R>(&mut self, change: impl FnOnce(&mut Vec<MachineSpecialization>) -> R) -> R {
        let result = change(&mut self.rows);
        self.reindex();
        result
    }

    /// The closed conformance applications of one row. They do not touch the
    /// instance, so the index stays exact.
    pub fn conformance_applications_mut(
        &mut self,
        row: usize,
    ) -> Option<&mut Vec<crate::ClosedConformanceApplication>> {
        self.rows
            .get_mut(row)
            .map(|receipt| &mut receipt.conformance_applications)
    }
}

/// Two tables are equal when their rows are: the index answers the same
/// queries whichever run shape recording left it in.
impl PartialEq for MachineSpecializations {
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows
    }
}

impl Eq for MachineSpecializations {}

impl std::ops::Deref for MachineSpecializations {
    type Target = [MachineSpecialization];

    fn deref(&self) -> &Self::Target {
        &self.rows
    }
}

impl<'table> IntoIterator for &'table MachineSpecializations {
    type Item = &'table MachineSpecialization;
    type IntoIter = std::slice::Iter<'table, MachineSpecialization>;

    fn into_iter(self) -> Self::IntoIter {
        self.rows.iter()
    }
}

impl From<Vec<MachineSpecialization>> for MachineSpecializations {
    fn from(rows: Vec<MachineSpecialization>) -> Self {
        Self::from_rows(rows)
    }
}

impl FromIterator<MachineSpecialization> for MachineSpecializations {
    fn from_iter<I: IntoIterator<Item = MachineSpecialization>>(rows: I) -> Self {
        Self::from_rows(rows.into_iter().collect())
    }
}

/// Only the rows are stored; reading rebuilds the index from them.
impl compiler_storage::Record for MachineSpecializations {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        self.rows.write(output)
    }

    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        Vec::<MachineSpecialization>::read(input).map(Self::from_rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(instance: u32, template: u32) -> MachineSpecialization {
        MachineSpecialization {
            instance: SymbolHandle::from_parts(instance, 1),
            template: SymbolHandle::from_parts(template, 1),
            ..Default::default()
        }
    }

    fn templates(table: &MachineSpecializations, instance: u32) -> Vec<u32> {
        table
            .for_instance(SymbolHandle::from_parts(instance, 1))
            .map(|row| row.template.arena_index())
            .collect()
    }

    #[test]
    fn each_instance_finds_exactly_its_receipts_in_recording_order() {
        // Instances arrive out of order and repeat; another generation of the
        // same slot is a different instance.
        let mut table = MachineSpecializations::default();
        for (order, instance) in [7, 3, 7, 9, 3, 1].into_iter().enumerate() {
            table.push(receipt(instance, 100 + order as u32));
        }
        table.push(MachineSpecialization {
            instance: SymbolHandle::from_parts(7, 2),
            ..receipt(0, 200)
        });
        assert_eq!(templates(&table, 7), [100, 102]);
        assert_eq!(templates(&table, 3), [101, 104]);
        assert_eq!(templates(&table, 9), [103]);
        assert_eq!(templates(&table, 1), [105]);
        assert!(templates(&table, 4).is_empty());
        let rows = table
            .for_instance_rows(SymbolHandle::from_parts(7, 2))
            .map(|(row, _)| row)
            .collect::<Vec<_>>();
        assert_eq!(rows, [6]);
        assert_eq!(table[1].template.arena_index(), 101, "rows keep order");
        // Descending slots, interleaved with lookups, as stage 04 may push.
        let mut table = MachineSpecializations::default();
        for round in 0..3u32 {
            for instance in (0..200u32).rev() {
                table.push(receipt(instance, round * 1000 + instance));
                assert_eq!(templates(&table, instance).len(), round as usize + 1);
            }
        }
        assert_eq!(templates(&table, 42), [42, 1042, 2042]);
        assert!(
            table.runs.len() <= 11,
            "600 rows keep at most log2 + 1 runs"
        );
        let rebuilt = MachineSpecializations::from_rows(table.to_vec());
        for instance in 0..200 {
            assert_eq!(templates(&table, instance), templates(&rebuilt, instance));
        }
    }

    #[test]
    fn index_storage_follows_the_receipts_not_their_symbol_slots() {
        // Sparse instances at the ends of the slot range: the index holds one
        // position per receipt, whether pushed or decoded.
        let high = |generation| SymbolHandle::from_parts(u32::MAX, generation);
        let mut table = MachineSpecializations::default();
        table.push(MachineSpecialization {
            instance: high(1),
            ..receipt(0, 10)
        });
        table.push(receipt(1, 11));
        table.push(MachineSpecialization {
            instance: high(2),
            ..receipt(0, 12)
        });
        let stored = compiler_storage::encode(&table, Default::default()).expect("encode");
        let read: MachineSpecializations =
            compiler_storage::decode(&stored, Default::default()).expect("decode");
        for table in [&table, &read] {
            assert_eq!(table.by_instance.len(), 3);
            assert!(table.by_instance.capacity() < 64);
            let found = |instance| {
                table
                    .for_instance(instance)
                    .map(|row| row.template.arena_index())
                    .collect::<Vec<_>>()
            };
            assert_eq!(found(high(1)), [10]);
            assert_eq!(found(high(2)), [12]);
            assert_eq!(found(SymbolHandle::from_parts(1, 1)), [11]);
        }
    }

    #[test]
    fn every_change_leaves_the_index_exact() {
        let mut table = [receipt(5, 1), receipt(6, 2), receipt(5, 3)]
            .into_iter()
            .collect::<MachineSpecializations>();
        table.edit(|rows| rows[1].instance = SymbolHandle::from_parts(5, 1));
        assert_eq!(templates(&table, 5), [1, 2, 3]);
        assert!(templates(&table, 6).is_empty());
        table.retain(|row| row.template.arena_index() != 2);
        assert_eq!(templates(&table, 5), [1, 3]);
        let stored = compiler_storage::encode(&table, Default::default()).expect("encode");
        let read: MachineSpecializations =
            compiler_storage::decode(&stored, Default::default()).expect("decode");
        assert_eq!(read, table);
        assert_eq!(templates(&read, 5), [1, 3]);
        table.clear();
        assert!(templates(&table, 5).is_empty());
    }
}
