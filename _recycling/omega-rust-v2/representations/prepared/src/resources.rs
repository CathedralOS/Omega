use std::sync::Arc;

/// Root-selected limits for one preparation of a package.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparationGrant {
    pub work_budget: u64,
    pub peak_live_cells: u64,
}

/// A checked maximum can establish admission before execution. These numbers
/// describe a ceiling. Actual usage requires its own versioned measurement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertifiedResources {
    pub checked_subject: Arc<[u8]>,
    pub work_schedule: u32,
    pub maximum_work: u64,
    pub maximum_live_cells: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparationResourceReceipt {
    pub grant: PreparationGrant,
    pub bound: CertifiedResources,
}
