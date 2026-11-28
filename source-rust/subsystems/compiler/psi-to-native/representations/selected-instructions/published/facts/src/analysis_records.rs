use optimization_core::OptimizationUnitIdentity;
use register_homes::AllocatorAvailabilityIdentity;
use register_model::{
    PhysicalRegisterModelIdentity, RegisterReservationProfileIdentity,
    TargetRegisterEnvironmentIdentity,
};
use semantic_vocabulary::WorkScheduleIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LivenessValidationReceipt {
    pub optimization_unit: OptimizationUnitIdentity,
    pub work_schedule: WorkScheduleIdentity,
}
impl LivenessValidationReceipt {
    pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn work_schedule(self) -> WorkScheduleIdentity {
        self.work_schedule
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveRangeValidationReceipt {
    pub optimization_unit: OptimizationUnitIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub virtual_register_count: usize,
    pub virtual_fragment_count: usize,
}
impl LiveRangeValidationReceipt {
    pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn work_schedule(self) -> WorkScheduleIdentity {
        self.work_schedule
    }
    pub const fn virtual_register_count(self) -> usize {
        self.virtual_register_count
    }
    pub const fn virtual_fragment_count(self) -> usize {
        self.virtual_fragment_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocationLegalityValidationReceipt {
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub virtual_register_count: usize,
    pub point_count: usize,
    pub candidate_count: usize,
    pub entry_transition_count: usize,
}
impl AllocationLegalityValidationReceipt {
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(self) -> AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn virtual_register_count(self) -> usize {
        self.virtual_register_count
    }
    pub const fn point_count(self) -> usize {
        self.point_count
    }
    pub const fn candidate_count(self) -> usize {
        self.candidate_count
    }
    pub const fn entry_transition_count(self) -> usize {
        self.entry_transition_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocatorAvailabilityValidationReceipt {
    pub identity: AllocatorAvailabilityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub physical: PhysicalRegisterModelIdentity,
    pub reservations: RegisterReservationProfileIdentity,
}
impl AllocatorAvailabilityValidationReceipt {
    pub const fn identity(self) -> AllocatorAvailabilityIdentity {
        self.identity
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn physical(self) -> PhysicalRegisterModelIdentity {
        self.physical
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveRangeFunctionCounts {
    pub virtual_register_count: usize,
    pub virtual_fragment_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegalityFunctionCounts {
    pub virtual_register_count: usize,
    pub point_count: usize,
    pub candidate_count: usize,
    pub entry_transition_count: usize,
}
