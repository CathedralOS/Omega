//! Exact physical-view legality, compressed into constant candidate spans.
//! The producer keeps each live fragment's coverage and every constraint
//! boundary; consumers intersect and authenticate spans without expanding them.
use crate::ValidatedAllocatorAvailability;
use crate::ValidatedLiveRanges;
use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};

pub(crate) mod compute;
pub(crate) mod validate;

use register_homes::{
    AllocationLegalityIdentity, AllocationLegalityPlan, AllocatorAvailabilityIdentity,
};
use selected_instructions::LiveRangeIdentity;

/// Derive exact per-point physical-view candidates and incompatible fixed-view
/// transitions without assigning homes or inserting copies.
pub fn analyze_allocation_legality(
    ranges: &ValidatedLiveRanges,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
) -> Result<ValidatedAllocationLegality, AllocationLegalityError> {
    let (plan, _) = compute::compute_terminal_allocation_legality(
        ranges,
        availability,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        None,
    )?;
    validate::admit(plan, &[], &[], &[])
}

/// Reuse proposals for functions whose complete live ranges and allocation
/// roots are unchanged; the new identity covers the whole plan.
pub fn analyze_allocation_legality_reusing(
    previous_ranges: &ValidatedLiveRanges,
    previous_legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
) -> Result<ValidatedAllocationLegality, AllocationLegalityError> {
    let (plan, carried) = compute::compute_terminal_allocation_legality(
        ranges,
        availability,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        Some((previous_ranges, previous_legality)),
    )?;
    validate::admit(
        plan,
        &carried,
        &previous_legality.function_digests,
        &previous_legality.function_counts,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocationLegalityValidationReceipt {
    pub(crate) identity: AllocationLegalityIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) virtual_register_count: usize,
    pub(crate) point_count: usize,
    pub(crate) candidate_count: usize,
    pub(crate) entry_transition_count: usize,
}

impl AllocationLegalityValidationReceipt {
    receipt_accessor_rows!(AllocationLegalityValidationReceipt {
        identity: AllocationLegalityIdentity,
        virtual_register_count: usize,
        point_count: usize,
        candidate_count: usize,
        entry_transition_count: usize,
        ranges: LiveRangeIdentity,
        register_environment: TargetRegisterEnvironmentIdentity,
        allocator_availability: AllocatorAvailabilityIdentity,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAllocationLegality {
    pub(crate) plan: std::sync::Arc<AllocationLegalityPlan>,
    pub(crate) receipt: AllocationLegalityValidationReceipt,
    /// Each function's share of the receipt identity, kept so a successor
    /// plan that carries a function over unchanged need not hash it again.
    pub(crate) function_digests: std::sync::Arc<[[u8; 32]]>,
    /// Report-only counts, bound to the same admitted function as its digest.
    pub(crate) function_counts: std::sync::Arc<[validate::FunctionCounts]>,
}

impl ValidatedAllocationLegality {
    pub fn plan(&self) -> &AllocationLegalityPlan {
        &self.plan
    }
    receipt_accessor_rows!(ref ValidatedAllocationLegality {
        receipt: AllocationLegalityValidationReceipt,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationLegalityError {
    RootMismatch,
    UnknownClass {
        function: usize,
        register: u32,
        class: u16,
    },
    UnknownFixedView {
        function: usize,
        register: u32,
        view: u16,
    },
    IllegalFixedView {
        function: usize,
        register: u32,
        view: u16,
    },
    NoCandidateViews {
        function: usize,
        register: u32,
        block: u32,
        point: u32,
    },
    PointOverflow {
        function: usize,
    },
    FunctionMismatch {
        function: usize,
    },
    VirtualRegisterMismatch {
        function: usize,
        register: u32,
    },
    NonCanonicalRows {
        function: usize,
        register: u32,
    },
}

failed_display!(
    AllocationLegalityError,
    "Terminal allocation-legality derivation failed"
);

impl std::error::Error for AllocationLegalityError {}
