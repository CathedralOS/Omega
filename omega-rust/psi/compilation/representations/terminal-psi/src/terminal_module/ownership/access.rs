#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralMultiplicity {
    Unrestricted,
    Affine,
    Linear,
}

/// Semantic access carried by a structural parameter or call argument.
/// Borrowed variants intentionally share a physical pointer representation;
/// this closed axis prevents semantic authority from being erased by ABI
/// equivalence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralAccess {
    Owned,
    SharedBorrow,
    MutableBorrow,
    WriteOnlyBorrow,
}

/// Shared-or-mutable borrows — the loan kinds that read the referent.
#[macro_export]
macro_rules! reading_borrow_pair_pattern {
    () => {
        $crate::StructuralAccess::SharedBorrow | $crate::StructuralAccess::MutableBorrow
    };
}

/// Mutable-or-write-only borrows — the loan kinds holding exclusive access.
#[macro_export]
macro_rules! exclusive_borrow_pair_pattern {
    () => {
        $crate::StructuralAccess::MutableBorrow | $crate::StructuralAccess::WriteOnlyBorrow
    };
}

/// Shared or mutable borrow — accesses that read through a borrow.
#[macro_export]
macro_rules! borrowed_access_pair_pattern {
    () => {
        $crate::ownership::StructuralAccess::SharedBorrow
            | $crate::ownership::StructuralAccess::MutableBorrow
    };
}

/// Mutable borrow or owned — the accesses that may mutate the target.
#[macro_export]
macro_rules! mutating_access_pair_pattern {
    () => {
        $crate::ownership::StructuralAccess::MutableBorrow
            | $crate::ownership::StructuralAccess::Owned
    };
}

/// Affine or unrestricted — multiplicities that do not require exactly-once use.
#[macro_export]
macro_rules! nonlinear_multiplicity_pair_pattern {
    () => {
        $crate::ownership::StructuralMultiplicity::Affine
            | $crate::ownership::StructuralMultiplicity::Unrestricted
    };
}

impl StructuralAccess {
    /// `true` when `granted` covers the access `self` requires — the
    /// ownership-covering compatibility lattice every ingress
    /// establishment/projection consumer applies (`Owned` grants every
    /// borrow kind; an `Owned` requirement admits nothing).
    pub fn admits(self, granted: StructuralAccess) -> bool {
        match self {
            StructuralAccess::SharedBorrow => matches!(
                granted,
                StructuralAccess::Owned | crate::borrowed_access_pair_pattern!()
            ),
            StructuralAccess::MutableBorrow => {
                matches!(granted, crate::mutating_access_pair_pattern!())
            }
            StructuralAccess::WriteOnlyBorrow => matches!(
                granted,
                StructuralAccess::Owned
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
            ),
            StructuralAccess::Owned => false,
        }
    }

    /// `true` when `referent` access satisfies `self` through a `Referent`
    /// projection — the loan-chain compatibility axis. A loan can never
    /// grant ownership, so `Owned` referents admit nothing and an `Owned`
    /// source never qualifies.
    pub fn admits_referent(self, referent: StructuralAccess) -> bool {
        match self {
            StructuralAccess::SharedBorrow => {
                matches!(referent, crate::borrowed_access_pair_pattern!())
            }
            StructuralAccess::MutableBorrow => referent == StructuralAccess::MutableBorrow,
            StructuralAccess::WriteOnlyBorrow => matches!(
                referent,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            ),
            StructuralAccess::Owned => false,
        }
    }
}
