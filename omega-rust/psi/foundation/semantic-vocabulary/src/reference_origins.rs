//! Extent of a returned borrow's relation to supplied storage. This describes
//! provenance, never an address calculation or permission to form a new loan.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum ReferenceOriginExtent {
    /// The typed projection identifies the result's referent family.
    Projected,
    /// The returned footprint is contained in the supplied input region. Its
    /// referent type belongs to the result, not to the region's endpoint type.
    /// Concrete formation still owns address, bounds, access and lineage.
    WithinRegion,
}
