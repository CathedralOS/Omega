use omega_core::allocations::AllocationDelta;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackendPlanPhaseTiming {
    pub phase: &'static str,
    pub microseconds: u128,
    pub allocations: AllocationDelta,
}
