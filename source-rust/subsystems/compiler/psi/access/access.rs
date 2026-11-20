#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralMultiplicity {
    Unrestricted,
    Affine,
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralAccess {
    Owned,
    SharedBorrow,
    MutableBorrow,
    WriteOnlyBorrow,
}

#[macro_export]
macro_rules! reading_borrow_pair_pattern {
    () => {
        $crate::StructuralAccess::SharedBorrow | $crate::StructuralAccess::MutableBorrow
    };
}

#[macro_export]
macro_rules! exclusive_borrow_pair_pattern {
    () => {
        $crate::StructuralAccess::MutableBorrow | $crate::StructuralAccess::WriteOnlyBorrow
    };
}

#[macro_export]
macro_rules! borrowed_access_pair_pattern {
    () => {
        $crate::ownership::StructuralAccess::SharedBorrow
            | $crate::ownership::StructuralAccess::MutableBorrow
    };
}

#[macro_export]
macro_rules! mutating_access_pair_pattern {
    () => {
        $crate::ownership::StructuralAccess::MutableBorrow
            | $crate::ownership::StructuralAccess::Owned
    };
}

#[macro_export]
macro_rules! nonlinear_multiplicity_pair_pattern {
    () => {
        $crate::ownership::StructuralMultiplicity::Affine
            | $crate::ownership::StructuralMultiplicity::Unrestricted
    };
}

impl StructuralAccess {
    /// Forms a loan without transferring ownership of the granted storage.
    pub fn admits(self, granted: StructuralAccess) -> bool {
        self != Self::Owned && structural_access_covers(granted, self)
    }

    /// Attenuates a referent's loan; owning its reference carrier grants no referent ownership.
    pub fn admits_referent(self, referent: StructuralAccess) -> bool {
        referent != Self::Owned && self.admits(referent)
    }
}

/// Compares access modes without establishing custody, lifetime, or storage validity.
/// Callers must separately check those obligations and whether the requested operation
/// reads contents, mutates storage, or transfers ownership.
pub fn structural_access_covers(
    available: StructuralAccess,
    requested: StructuralAccess,
) -> bool {
    available == requested
        || available == StructuralAccess::Owned
        || available == StructuralAccess::MutableBorrow
            && matches!(
                requested,
                StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
            )
}

#[cfg(test)]
mod tests {
    use super::{StructuralAccess, structural_access_covers};
    use StructuralAccess::{MutableBorrow, Owned, SharedBorrow, WriteOnlyBorrow};

    const MODES: [StructuralAccess; 4] = [Owned, SharedBorrow, MutableBorrow, WriteOnlyBorrow];

    #[test]
    fn structural_access_coverage_preserves_read_and_write_limits() {
        let expected = [
            [true, true, true, true],
            [false, true, false, false],
            [false, true, true, true],
            [false, false, false, true],
        ];
        for (available_index, available) in MODES.into_iter().enumerate() {
            for (requested_index, requested) in MODES.into_iter().enumerate() {
                assert_eq!(
                    structural_access_covers(available, requested),
                    expected[available_index][requested_index],
                    "{available:?} -> {requested:?}"
                );
            }
        }
    }

    #[test]
    fn structural_access_borrows_never_manufacture_ownership() {
        let loans = [
            [false, true, true, true],
            [false, true, false, false],
            [false, true, true, true],
            [false, false, false, true],
        ];
        let referents = [
            [false, false, false, false],
            [false, true, false, false],
            [false, true, true, true],
            [false, false, false, true],
        ];
        for (available_index, available) in MODES.into_iter().enumerate() {
            for (requested_index, requested) in MODES.into_iter().enumerate() {
                assert_eq!(
                    requested.admits(available),
                    loans[available_index][requested_index],
                    "loan {available:?} -> {requested:?}"
                );
                assert_eq!(
                    requested.admits_referent(available),
                    referents[available_index][requested_index],
                    "referent {available:?} -> {requested:?}"
                );
            }
        }
    }
}
