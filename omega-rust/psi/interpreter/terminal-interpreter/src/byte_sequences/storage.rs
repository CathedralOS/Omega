//! One owned byte payload and its reservation. Views share this owner; a new
//! backing reserves its entire length before allocating. Capacity is not a
//! logical byte count, and this owner does not account runtime descriptor rows.

use crate::errors::{TerminalInterpretError, malformed};
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};

#[derive(Debug)]
pub(crate) struct ByteSequenceStorage {
    bytes: Vec<u8>,
    // Drop bytes before releasing their reservation.
    lease: Option<EvaluationStorageLease>,
    account: Option<EvaluationInvocationAccount>,
}

impl AsRef<[u8]> for ByteSequenceStorage {
    fn as_ref(&self) -> &[u8] {
        self.bytes()
    }
}

impl ByteSequenceStorage {
    pub(crate) fn zeroed(
        length: usize,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<Self, TerminalInterpretError> {
        let payload_bytes = u64::try_from(length).map_err(|_| malformed())?;
        let lease = account
            .map(|account| account.reserve_storage(0, payload_bytes))
            .transpose()
            .map_err(TerminalInterpretError::Storage)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(|reason| {
            TerminalInterpretError::StorageAllocationFailed {
                requested_bytes: length,
                reason: reason.to_string(),
            }
        })?;
        bytes.resize(length, 0);
        Ok(Self {
            bytes,
            lease,
            account: account.cloned(),
        })
    }

    pub(crate) fn copy(
        bytes: &[u8],
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<Self, TerminalInterpretError> {
        let mut storage = Self::zeroed(bytes.len(), account)?;
        storage.bytes.copy_from_slice(bytes);
        Ok(storage)
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(crate) fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }
    pub(crate) fn account(&self) -> Option<&EvaluationInvocationAccount> {
        self.account.as_ref()
    }

    /// Resize the same owned payload. Reserve growth before allocation, retain
    /// the old contents on refusal, and release a shortened tail after mutation.
    pub(crate) fn replace(&mut self, bytes: &[u8]) -> Result<(), TerminalInterpretError> {
        let previous = self.bytes.len();
        if bytes.len() > previous {
            let added = bytes.len() - previous;
            let added_bytes = u64::try_from(added).map_err(|_| malformed())?;
            if let Some(lease) = &mut self.lease {
                lease
                    .grow(0, added_bytes)
                    .map_err(TerminalInterpretError::Storage)?;
            }
            if let Err(reason) = self.bytes.try_reserve_exact(added) {
                if let Some(lease) = &mut self.lease {
                    lease
                        .shrink(0, added_bytes)
                        .map_err(TerminalInterpretError::Storage)?;
                }
                return Err(TerminalInterpretError::StorageAllocationFailed {
                    requested_bytes: bytes.len(),
                    reason: reason.to_string(),
                });
            }
        }
        self.bytes.clear();
        self.bytes.extend_from_slice(bytes);
        if bytes.len() < previous
            && let Some(lease) = &mut self.lease
        {
            lease
                .shrink(
                    0,
                    u64::try_from(previous - bytes.len()).map_err(|_| malformed())?,
                )
                .map_err(TerminalInterpretError::Storage)?;
        }
        Ok(())
    }

    pub(crate) fn from_values(
        length: usize,
        values: impl IntoIterator<Item = Result<u8, TerminalInterpretError>>,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<Self, TerminalInterpretError> {
        let mut storage = Self::zeroed(length, account)?;
        let mut values = values.into_iter();
        for byte in &mut storage.bytes {
            *byte = values.next().ok_or_else(malformed)??;
        }
        if values.next().is_some() {
            return Err(malformed());
        }
        Ok(storage)
    }
}

#[cfg(test)]
mod tests {
    use super::ByteSequenceStorage;
    use crate::byte_sequences::ByteSequenceView;
    use evaluation_accounting::{
        BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
        TerminalStorageScheduleIdentity,
    };

    fn sponsor(bytes: u64) -> BuildEvaluationSponsor {
        let measurement = EvaluationMeasurementIdentity::TerminalPsi {
            fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
            storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
        };
        BuildEvaluationSponsor::new_with_measurement(
            BuildEvaluationSponsorLimits::new(100, 8, 2, 2, 100, bytes, 100, 100).unwrap(),
            measurement,
        )
        .unwrap()
    }

    #[test]
    fn views_retain_the_full_backing_once_until_its_last_owner_drops() {
        let sponsor = sponsor(5);
        let account = sponsor
            .begin_invocation(sponsor.measurement(), 100)
            .unwrap();
        let original =
            ByteSequenceView::new(ByteSequenceStorage::copy(b"abcd", Some(&account)).unwrap());
        let alias = original.subslice(1, 2).unwrap();
        drop(original);
        assert_eq!(alias.bytes(), b"b");
        assert_eq!(sponsor.live_payload_bytes(), 4);
        assert!(ByteSequenceStorage::zeroed(2, Some(&account)).is_err());
        assert_eq!(sponsor.live_payload_bytes(), 4);
        drop(alias);
        assert_eq!(sponsor.live_payload_bytes(), 0);
        assert_eq!(
            account.snapshot().unwrap().record().peak_live_payload_bytes,
            4
        );
    }

    #[test]
    fn copy_on_write_reserves_the_overlapping_window_before_mutation() {
        for limit in [5, 6] {
            let sponsor = sponsor(limit);
            let account = sponsor
                .begin_invocation(sponsor.measurement(), 100)
                .unwrap();
            let original =
                ByteSequenceView::new(ByteSequenceStorage::copy(b"abcd", Some(&account)).unwrap());
            let mut window = original.subslice(1, 3).unwrap();
            let changed = window.replace_byte(0, b'X');
            assert_eq!(original.bytes(), b"abcd");
            if limit == 5 {
                assert!(changed.is_err());
                assert_eq!(window.bytes(), b"bc");
                assert_eq!(sponsor.live_payload_bytes(), 4);
            } else {
                changed.unwrap();
                assert_eq!(window.bytes(), b"Xc");
                assert_eq!(sponsor.live_payload_bytes(), 6);
                window.replace_byte(1, b'Y').unwrap();
                assert_eq!(
                    sponsor.live_payload_bytes(),
                    6,
                    "unique replacement allocates no backing"
                );
            }
            drop(original);
            assert_eq!(sponsor.live_payload_bytes(), if limit == 5 { 4 } else { 2 });
            drop(window);
            assert_eq!(sponsor.live_payload_bytes(), 0);
        }
    }

    #[test]
    fn staged_growth_shares_the_budget_and_refuses_without_replacing_contents() {
        let sponsor = sponsor(8);
        let first = sponsor
            .begin_invocation(sponsor.measurement(), 100)
            .unwrap();
        let second = sponsor
            .begin_invocation(sponsor.measurement(), 100)
            .unwrap();
        let mut storage = ByteSequenceStorage::copy(b"abc", Some(&first)).unwrap();
        let sibling = ByteSequenceStorage::zeroed(4, Some(&second)).unwrap();
        assert!(storage.replace(b"12345").is_err());
        assert_eq!(storage.bytes(), b"abc");
        assert_eq!(sponsor.live_payload_bytes(), 7);
        drop(sibling);
        storage.replace(b"12345").unwrap();
        assert_eq!(sponsor.live_payload_bytes(), 5);
        storage.replace(b"x").unwrap();
        assert_eq!(sponsor.live_payload_bytes(), 1);
        drop(storage);
        assert_eq!(sponsor.live_payload_bytes(), 0);
        assert!(ByteSequenceStorage::zeroed(usize::MAX, Some(&first)).is_err());
        assert_eq!(sponsor.live_payload_bytes(), 0);
    }

    #[test]
    fn malformed_scalar_copy_retires_its_private_reservation() {
        let sponsor = sponsor(8);
        let account = sponsor
            .begin_invocation(sponsor.measurement(), 100)
            .unwrap();
        assert!(
            ByteSequenceStorage::from_values(
                2,
                [Ok(1), Err(crate::errors::malformed())],
                Some(&account)
            )
            .is_err()
        );
        assert_eq!(sponsor.live_payload_bytes(), 0);
        assert!(ByteSequenceStorage::from_values(2, [Ok(1)], Some(&account)).is_err());
        assert_eq!(sponsor.live_payload_bytes(), 0);
    }
}
