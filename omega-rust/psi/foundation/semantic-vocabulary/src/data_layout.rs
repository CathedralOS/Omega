//! Physical inputs supplied by an execution environment, never sampled from
//! the compiler or interpreter host. These parameters describe storage only.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalDataLayout {
    pub address_byte_size: u64,
    pub address_byte_alignment: u64,
}

impl PhysicalDataLayout {
    pub const fn is_valid(self) -> bool {
        self.address_byte_size > 0
            && self.address_byte_alignment.is_power_of_two()
            && self
                .address_byte_size
                .is_multiple_of(self.address_byte_alignment)
    }
}

/// The portable observation's exact subject, shared by executable operations
/// and erased proof terms. It carries no target number or access authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LayoutObservationKind {
    Size,
    Alignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LayoutObservationCarrier {
    Unit,
    Scalar(crate::ScalarType),
    Structural(crate::StructuralTypeId),
}
