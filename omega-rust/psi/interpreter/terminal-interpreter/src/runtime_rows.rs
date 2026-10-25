//! A live interpreter table owns one logical cell per row, including staged
//! call/edge tables. Reservations precede insertion and copying. Frame moves
//! and table joins transfer existing custody; replacing a row needs no new
//! cell. Keys and value payloads keep their separate semantic storage owners.

use crate::errors::{TerminalInterpretError, malformed};
use evaluation_accounting::{
    EvaluationAccountingError, EvaluationInvocationAccount, EvaluationStorageLease,
};
use std::collections::{BTreeMap, btree_map};

fn reserve_row(
    lease: &mut Option<EvaluationStorageLease>,
    account: Option<&EvaluationInvocationAccount>,
) -> Result<(), TerminalInterpretError> {
    if let Some(lease) = lease {
        lease.grow(1, 0).map_err(TerminalInterpretError::Storage)?;
    } else if let Some(account) = account {
        *lease = Some(
            account
                .reserve_storage(1, 0)
                .map_err(TerminalInterpretError::Storage)?,
        );
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) struct RuntimeRows<Key, Value> {
    rows: BTreeMap<Key, Value>,
    // Release the rows before their reservation.
    lease: Option<EvaluationStorageLease>,
    account: Option<EvaluationInvocationAccount>,
}

impl<Key: Ord, Value> RuntimeRows<Key, Value> {
    pub(crate) fn new(account: Option<&EvaluationInvocationAccount>) -> Self {
        Self {
            rows: BTreeMap::new(),
            lease: None,
            account: account.cloned(),
        }
    }

    pub(crate) fn from_rows(
        account: Option<&EvaluationInvocationAccount>,
        rows: impl IntoIterator<Item = (Key, Value)>,
    ) -> Result<Self, TerminalInterpretError> {
        let mut owned = Self::new(account);
        for (key, value) in rows {
            owned.insert(key, value)?;
        }
        Ok(owned)
    }

    pub(crate) fn insert(
        &mut self,
        key: Key,
        value: Value,
    ) -> Result<Option<Value>, TerminalInterpretError> {
        match self.rows.entry(key) {
            btree_map::Entry::Occupied(mut entry) => Ok(Some(entry.insert(value))),
            btree_map::Entry::Vacant(entry) => {
                reserve_row(&mut self.lease, self.account.as_ref())?;
                entry.insert(value);
                Ok(None)
            }
        }
    }

    /// Reserve a new receiving row and retain its mutable home before a
    /// transaction commits external writes or retires source custody.
    pub(crate) fn insert_vacant(
        &mut self,
        key: Key,
        value: Value,
    ) -> Result<&mut Value, TerminalInterpretError> {
        let btree_map::Entry::Vacant(entry) = self.rows.entry(key) else {
            return Err(malformed());
        };
        reserve_row(&mut self.lease, self.account.as_ref())?;
        Ok(entry.insert(value))
    }

    fn retire_removed_rows(&mut self) {
        if let Some(lease) = &mut self.lease {
            // Insertion has already checked every count against u64 storage
            // accounting. A removal cannot exceed the formerly admitted size.
            lease.truncate_cells(u64::try_from(self.rows.len()).unwrap_or(u64::MAX));
        }
    }

    pub(crate) fn remove(&mut self, key: &Key) -> Option<Value> {
        let removed = self.rows.remove(key);
        self.retire_removed_rows();
        removed
    }

    pub(crate) fn clear(&mut self) {
        self.rows.clear();
        self.lease = None;
    }

    pub(crate) fn retain(&mut self, keep: impl FnMut(&Key, &mut Value) -> bool) {
        self.rows.retain(keep);
        self.retire_removed_rows();
    }

    /// Suspend a frame's table while preserving the new frame's account.
    pub(crate) fn take(&mut self) -> Self {
        let empty = Self::new(self.account.as_ref());
        std::mem::replace(self, empty)
    }

    /// Check transfer identity before an enclosing transaction retires sources.
    /// Both reservations already coexist under the same checked sponsor total;
    /// transfer cannot add a charge or exceed that admitted total.
    pub(crate) fn validate_append(&self, other: &Self) -> Result<(), TerminalInterpretError> {
        if other.rows.is_empty() {
            return Ok(());
        }
        let compatible = match (&self.account, &other.account) {
            (Some(left), Some(right)) => left.same_invocation(right),
            (None, None) => true,
            _ => false,
        };
        if !compatible {
            return Err(TerminalInterpretError::Storage(
                EvaluationAccountingError::LeaseAccountMismatch,
            ));
        }
        Ok(())
    }

    pub(crate) fn append(&mut self, other: &mut Self) -> Result<(), TerminalInterpretError> {
        self.validate_append(other)?;
        if other.rows.is_empty() {
            return Ok(());
        }
        match (&mut self.lease, &mut other.lease) {
            (Some(destination), Some(source)) => destination
                .absorb(source)
                .map_err(TerminalInterpretError::Storage)?,
            (None, _) => self.lease = other.lease.take(),
            _ => {}
        }
        self.rows.append(&mut other.rows);
        self.retire_removed_rows();
        Ok(())
    }

    pub(crate) fn try_clone(&self) -> Result<Self, TerminalInterpretError>
    where
        Key: Clone,
        Value: Clone,
    {
        let cells = u64::try_from(self.rows.len()).map_err(|_| malformed())?;
        let lease = self
            .account
            .as_ref()
            .map(|account| account.reserve_storage(cells, 0))
            .transpose()
            .map_err(TerminalInterpretError::Storage)?;
        Ok(Self {
            rows: self.rows.clone(),
            lease,
            account: self.account.clone(),
        })
    }

    // Read-only access is explicit; no mutable map escape or infallible Clone
    // can silently create unfunded live rows.
    pub(crate) fn get(&self, key: &Key) -> Option<&Value> {
        self.rows.get(key)
    }
    pub(crate) fn get_mut(&mut self, key: &Key) -> Option<&mut Value> {
        self.rows.get_mut(key)
    }
    pub(crate) fn contains_key(&self, key: &Key) -> bool {
        self.rows.contains_key(key)
    }
    pub(crate) fn len(&self) -> usize {
        self.rows.len()
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub(crate) fn keys(&self) -> btree_map::Keys<'_, Key, Value> {
        self.rows.keys()
    }
    pub(crate) fn values(&self) -> btree_map::Values<'_, Key, Value> {
        self.rows.values()
    }
    pub(crate) fn iter(&self) -> btree_map::Iter<'_, Key, Value> {
        self.rows.iter()
    }
    pub(crate) fn iter_mut(&mut self) -> btree_map::IterMut<'_, Key, Value> {
        self.rows.iter_mut()
    }

    /// Borrow the logical table for read-only custody and projection judgments.
    /// Growth, removal and transfer still require this reservation owner.
    pub(crate) fn as_map(&self) -> &BTreeMap<Key, Value> {
        &self.rows
    }
}

impl<'rows, Key, Value> IntoIterator for &'rows RuntimeRows<Key, Value> {
    type Item = (&'rows Key, &'rows Value);
    type IntoIter = btree_map::Iter<'rows, Key, Value>;
    fn into_iter(self) -> Self::IntoIter {
        self.rows.iter()
    }
}

impl<Key, Value> std::borrow::Borrow<BTreeMap<Key, Value>> for RuntimeRows<Key, Value> {
    fn borrow(&self) -> &BTreeMap<Key, Value> {
        &self.rows
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeRows;
    use evaluation_accounting::{
        BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
        TerminalStorageScheduleIdentity,
    };

    fn sponsor(cells: u64) -> BuildEvaluationSponsor {
        BuildEvaluationSponsor::new_with_measurement(
            BuildEvaluationSponsorLimits::new(100, 8, 2, 2, cells, 100, 100, 100).unwrap(),
            EvaluationMeasurementIdentity::TerminalPsi {
                fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
                storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
            },
        )
        .unwrap()
    }

    #[test]
    fn replacement_keeps_its_cell_and_refused_growth_keeps_the_table() {
        let sponsor = sponsor(1);
        let account = sponsor
            .begin_invocation(sponsor.measurement(), 100)
            .unwrap();
        let mut rows = RuntimeRows::new(Some(&account));
        assert_eq!(rows.insert(1u32, 7u32).unwrap(), None);
        assert_eq!(rows.insert(1, 8).unwrap(), Some(7));
        assert!(rows.insert(2, 9).is_err());
        assert_eq!(
            rows.iter()
                .map(|(key, value)| (*key, *value))
                .collect::<Vec<_>>(),
            [(1, 8)]
        );
        assert_eq!(sponsor.live_storage_cells(), 1);
        assert_eq!(rows.remove(&2), None);
        assert_eq!(sponsor.live_storage_cells(), 1);
        assert_eq!(rows.remove(&1), Some(8));
        assert_eq!(sponsor.live_storage_cells(), 0);
        rows.insert(3, 10).unwrap();
        drop(rows);
        assert_eq!(sponsor.live_storage_cells(), 0);
    }

    #[test]
    fn frame_transfer_keeps_both_the_reservation_and_the_next_frames_account() {
        let sponsor = sponsor(2);
        let account = sponsor
            .begin_invocation(sponsor.measurement(), 100)
            .unwrap();
        let mut active = RuntimeRows::from_rows(Some(&account), [(1u32, 7u32), (2, 8)]).unwrap();
        let mut saved = active.take();
        assert_eq!(sponsor.live_storage_cells(), 2);
        assert_eq!(active.len(), 0);
        assert!(active.insert(3, 9).is_err());
        saved.clear();
        active.insert(3, 9).unwrap();
        assert_eq!(sponsor.live_storage_cells(), 1);
        drop(active);
        assert_eq!(sponsor.live_storage_cells(), 0);
    }

    #[test]
    fn state_join_transfers_cells_and_retires_replaced_destinations_without_recharging() {
        let sponsor = sponsor(4);
        let account = sponsor
            .begin_invocation(sponsor.measurement(), 100)
            .unwrap();
        let mut active = RuntimeRows::from_rows(Some(&account), [(1u32, 7u32), (2, 8)]).unwrap();
        let mut staged = RuntimeRows::from_rows(Some(&account), [(2, 20), (3, 30)]).unwrap();
        assert_eq!(sponsor.live_storage_cells(), 4);
        active.append(&mut staged).unwrap();
        assert_eq!(active.get(&2), Some(&20));
        assert_eq!(active.len(), 3);
        assert_eq!(staged.len(), 0);
        assert_eq!(sponsor.live_storage_cells(), 3);
        staged.insert(4, 40).unwrap();
        assert_eq!(sponsor.live_storage_cells(), 4);
        drop(active);
        assert_eq!(sponsor.live_storage_cells(), 1);
        drop(staged);
        assert_eq!(sponsor.live_storage_cells(), 0);
        assert_eq!(
            account.snapshot().unwrap().record().peak_live_storage_cells,
            4
        );
    }

    #[test]
    fn copies_reserve_their_overlap_and_partial_construction_retires_rows() {
        for limit in [3, 4] {
            let sponsor = sponsor(limit);
            let account = sponsor
                .begin_invocation(sponsor.measurement(), 100)
                .unwrap();
            let rows = RuntimeRows::from_rows(Some(&account), [(1u32, 7u32), (2, 8)]).unwrap();
            let copy = rows.try_clone();
            if limit == 3 {
                assert!(copy.is_err());
                assert_eq!(sponsor.live_storage_cells(), 2);
                assert!(RuntimeRows::from_rows(Some(&account), [(3, 9), (4, 10)]).is_err());
                assert_eq!(sponsor.live_storage_cells(), 2);
            } else {
                let mut copy = copy.unwrap();
                assert_eq!(sponsor.live_storage_cells(), 4);
                copy.insert(1, 99).unwrap();
                assert_eq!(rows.get(&1), Some(&7));
                assert_eq!(copy.get(&1), Some(&99));
                drop(copy);
                assert_eq!(sponsor.live_storage_cells(), 2);
            }
            drop(rows);
            assert_eq!(sponsor.live_storage_cells(), 0);
        }
    }

    #[test]
    fn sibling_invocations_share_limits_but_cannot_transfer_account_identity() {
        let sponsor = sponsor(2);
        let first = sponsor
            .begin_invocation(sponsor.measurement(), 100)
            .unwrap();
        let second = sponsor
            .begin_invocation(sponsor.measurement(), 100)
            .unwrap();
        let mut left = RuntimeRows::from_rows(Some(&first), [(1u32, 7u32)]).unwrap();
        let mut right = RuntimeRows::from_rows(Some(&second), [(2, 8)]).unwrap();
        assert!(left.insert(3, 9).is_err());
        assert!(left.append(&mut right).is_err());
        assert_eq!(left.get(&1), Some(&7));
        assert_eq!(right.get(&2), Some(&8));
        assert_eq!(sponsor.live_storage_cells(), 2);
        drop(left);
        assert_eq!(sponsor.live_storage_cells(), 1);
        right.insert(3, 9).unwrap();
        assert_eq!(sponsor.live_storage_cells(), 2);
        drop(right);
        assert_eq!(sponsor.live_storage_cells(), 0);
    }

    #[test]
    fn vacant_receiving_row_is_funded_before_mutation_and_never_replaces() {
        let sponsor = sponsor(1);
        let account = sponsor
            .begin_invocation(sponsor.measurement(), 100)
            .unwrap();
        let mut rows = RuntimeRows::new(Some(&account));
        let receiving = rows.insert_vacant(1u32, 7u32).unwrap();
        assert_eq!(sponsor.live_storage_cells(), 1);
        *receiving = 8;
        assert!(rows.insert_vacant(1, 99).is_err());
        assert!(rows.insert_vacant(2, 99).is_err());
        assert_eq!(rows.get(&1), Some(&8));
        assert_eq!(rows.get(&2), None);
        drop(rows);
        assert_eq!(sponsor.live_storage_cells(), 0);
    }
}
