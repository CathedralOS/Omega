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

impl StructuralAccess {
    /// `true` when `granted` covers the access `self` requires — the
    /// ownership-covering compatibility lattice every ingress
    /// establishment/projection consumer applies (`Owned` grants every
    /// borrow kind; an `Owned` requirement admits nothing).
    pub fn admits(self, granted: StructuralAccess) -> bool {
        match self {
            StructuralAccess::SharedBorrow => matches!(
                granted,
                StructuralAccess::Owned
                    | StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
            ),
            StructuralAccess::MutableBorrow => matches!(
                granted,
                StructuralAccess::Owned | StructuralAccess::MutableBorrow
            ),
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
            StructuralAccess::SharedBorrow => matches!(
                referent,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            ),
            StructuralAccess::MutableBorrow => referent == StructuralAccess::MutableBorrow,
            StructuralAccess::WriteOnlyBorrow => matches!(
                referent,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            ),
            StructuralAccess::Owned => false,
        }
    }
}
