//! Optimizer module role: executable entrance. Transition-free physical-home assignment entrance.

use register_model::{
    TargetRegisterEnvironmentConstraintKeys, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
};
use selected_instructions_to_selected_instructions::{
    ValidatedAllocationLegality, ValidatedLiveRanges,
};
pub(crate) mod compute;
mod physical_requirement;
mod receipt;

use register_homes::{AllocationLegalityIdentity, AllocatorAvailabilityIdentity};
pub use register_homes::{
    FunctionRegisterHomes, RegisterHomeDecodeError, RegisterHomeIdentity, RegisterHomePlan,
    VirtualRegisterHome, register_home_identity,
};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::LiveRangeIdentity;

/// Assign deterministic physical views for the bounded transition-free,
/// spill-free lane. The result grants no emission or publication authority.
pub(crate) fn assign_register_homes(
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
) -> Result<ValidatedRegisterHomes, RegisterHomeError> {
    assign_register_homes_reusing(
        legality,
        ranges,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        None,
    )
}

/// `assign_register_homes` reusing unchanged functions' homes from `reuse`.
pub(crate) fn assign_register_homes_reusing(
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    reuse: Option<&mut compute::FunctionHomeReuse>,
) -> Result<ValidatedRegisterHomes, RegisterHomeError> {
    let plan = compute::compute_terminal_register_homes(
        legality,
        ranges,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        reuse,
    )?;
    let receipt = receipt::build(&plan, ranges);
    Ok(ValidatedRegisterHomes {
        plan: plan.into(),
        receipt,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterHomeValidationReceipt {
    pub(crate) identity: RegisterHomeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) function_count: usize,
    pub(crate) assignment_count: usize,
    pub(crate) tied_pair_count: usize,
    pub(crate) tied_component_count: usize,
    pub(crate) early_clobber_count: usize,
}

impl RegisterHomeValidationReceipt {
    receipt_accessor_rows!(RegisterHomeValidationReceipt {
        identity: RegisterHomeIdentity,
        legality: AllocationLegalityIdentity,
        ranges: LiveRangeIdentity,
        register_environment: TargetRegisterEnvironmentIdentity,
        allocator_availability: AllocatorAvailabilityIdentity,
        function_count: usize,
        assignment_count: usize,
        tied_pair_count: usize,
        tied_component_count: usize,
        early_clobber_count: usize,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRegisterHomes {
    pub(crate) plan: std::sync::Arc<RegisterHomePlan>,
    pub(crate) receipt: RegisterHomeValidationReceipt,
}

impl ValidatedRegisterHomes {
    /// Share current immutable data; the returned artifact grants no new authority.
    pub fn shared_plan(&self) -> std::sync::Arc<RegisterHomePlan> {
        std::sync::Arc::clone(&self.plan)
    }

    pub fn plan(&self) -> &RegisterHomePlan {
        &self.plan
    }
    receipt_accessor_rows!(ref ValidatedRegisterHomes {
        receipt: RegisterHomeValidationReceipt,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterHomeError {
    RootMismatch,
    FunctionMismatch {
        function: usize,
    },
    VirtualRegisterMismatch {
        function: usize,
        register: u32,
    },
    UnresolvedEntryTransitions {
        function: usize,
        register: u32,
        count: usize,
    },
    NoLivePoints {
        function: usize,
        register: u32,
    },
    IntervalOverflow {
        function: usize,
        register: u32,
    },
    NoCommonCandidate {
        function: usize,
        register: u32,
    },
    UnknownOrIncompatibleView {
        function: usize,
        register: u32,
        view: u16,
    },
    NoCompatibleHome {
        function: usize,
        register: u32,
    },
    UnsupportedTiedTopology {
        function: usize,
        instruction: u32,
    },
    /// The transition-free allocator cannot realize this exact successor
    /// binding without a physical edge move. It never drops the transfer.
    UnsupportedEdgeTransfer {
        function: usize,
        edge: u64,
    },
    TiedRegistersInterfere {
        function: usize,
        lower: u32,
        higher: u32,
    },
    NoCommonTiedComponent {
        function: usize,
        leader: u32,
        member_count: usize,
    },
}

impl RegisterHomeError {
    /// The verdicts runtime-spill recovery relieves: placement proved no
    /// compatible home for the failed domain, this exact value's candidate
    /// intersection is empty, or its entry transitions remain unresolved at
    /// its own points. Every other failure is the route's own to surface,
    /// and recovery never reinterprets one as pressure.
    pub(crate) const fn admits_runtime_recovery(&self) -> bool {
        matches!(
            self,
            Self::NoCompatibleHome { .. }
                | Self::NoCommonCandidate { .. }
                | Self::UnresolvedEntryTransitions { .. }
        )
    }
}

impl std::fmt::Display for RegisterHomeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Terminal register-home assignment failed: {self:?}"
        )
    }
}

impl std::error::Error for RegisterHomeError {}
