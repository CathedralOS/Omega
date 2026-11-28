//! Only register allocation may issue the body consumed by machine lowering.
#![forbid(unsafe_code)]
pub struct AllocationIssuer(());
pub const ALLOCATION: AllocationIssuer = AllocationIssuer(());
