use omega_package_review::OrdinaryPackageObligationLedger;

pub(super) const MAXIMUM_RETAINED_ORDINARY_LEDGER_BYTES: usize = 64 * 1024 * 1024;

pub(super) fn retained_obligation_ledger_bytes(
    ledger: &OrdinaryPackageObligationLedger,
) -> Option<usize> {
    let mut bytes = std::mem::size_of_val(ledger)
        .checked_add(std::mem::size_of_val(ledger.rows()))?
        .checked_add(std::mem::size_of_val(
            ledger.dependency_closure().packages(),
        ))?
        .checked_add(std::mem::size_of_val(
            ledger.dependency_closure().dependencies(),
        ))?;
    for row in ledger.rows() {
        bytes = bytes
            .checked_add(row.key_bytes().len())?
            .checked_add(row.canonical_bytes().len())?;
    }
    for dependency in ledger.dependency_closure().dependencies() {
        bytes = bytes.checked_add(dependency.alias().len())?;
    }
    Some(bytes)
}

pub(super) fn reserve_retained_obligation_ledger_bytes(
    current: usize,
    additional: usize,
) -> Option<usize> {
    current
        .checked_add(additional)
        .filter(|total| *total <= MAXIMUM_RETAINED_ORDINARY_LEDGER_BYTES)
}

#[cfg(test)]
mod tests {
    use super::{MAXIMUM_RETAINED_ORDINARY_LEDGER_BYTES, reserve_retained_obligation_ledger_bytes};

    #[test]
    fn retained_obligation_ledger_budget_is_aggregate_and_overflow_safe() {
        assert_eq!(
            reserve_retained_obligation_ledger_bytes(MAXIMUM_RETAINED_ORDINARY_LEDGER_BYTES - 1, 1,),
            Some(MAXIMUM_RETAINED_ORDINARY_LEDGER_BYTES)
        );
        assert_eq!(
            reserve_retained_obligation_ledger_bytes(MAXIMUM_RETAINED_ORDINARY_LEDGER_BYTES, 1,),
            None
        );
        assert_eq!(
            reserve_retained_obligation_ledger_bytes(usize::MAX, 1),
            None
        );
    }
}
