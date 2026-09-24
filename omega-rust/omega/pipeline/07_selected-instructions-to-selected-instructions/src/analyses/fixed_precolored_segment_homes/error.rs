use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedPrecoloredSegmentHomeError {
    RootMismatch,
    FunctionMismatch {
        function: usize,
    },
    SegmentMismatch {
        function: usize,
        register: u32,
        segment: u32,
    },
    MissingIncomingDomain {
        function: usize,
        register: u32,
        block: u32,
    },
    EmptyDomain {
        function: usize,
        register: u32,
        segment: u32,
    },
    UnknownOrIncompatibleView {
        function: usize,
        register: u32,
        view: u16,
    },
    DomainIdentityOverflow {
        function: usize,
    },
    SegmentPressure {
        function: usize,
        register: u32,
        segment: u32,
    },
    WorkOverflow,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
    UsageMismatch,
    NonCanonicalFunctions,
}

impl std::fmt::Display for FixedPrecoloredSegmentHomeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "fixed/precolored segment-home assignment failed: {self:?}"
        )
    }
}

impl std::error::Error for FixedPrecoloredSegmentHomeError {}
