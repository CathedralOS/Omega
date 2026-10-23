use crate::WorkDefect;

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
    /// Bounded work accounting refused: the logical receipt overflowed or
    /// exceeded the selected budget.
    Work(WorkDefect),
    UsageMismatch,
    NonCanonicalFunctions,
}

failed_display!(
    FixedPrecoloredSegmentHomeError,
    "fixed/precolored segment-home assignment failed"
);

impl std::error::Error for FixedPrecoloredSegmentHomeError {}
