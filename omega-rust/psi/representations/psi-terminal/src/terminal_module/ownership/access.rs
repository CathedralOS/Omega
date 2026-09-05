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
