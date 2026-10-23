use crate::WorkDefect;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedPrecoloredSplitRequirementError {
    RootMismatch,
    FunctionMismatch {
        function: usize,
    },
    RegisterMismatch {
        function: usize,
        register: u32,
    },
    UnsupportedCrossBlockRange {
        function: usize,
        register: u32,
    },
    UnsupportedTiedRegister {
        function: usize,
        register: u32,
    },
    MissingSourceFragment {
        function: usize,
        register: u32,
    },
    NonCanonicalPointDomain {
        function: usize,
        register: u32,
        point: u32,
    },
    UnauthenticatedDomainBreak {
        function: usize,
        register: u32,
        point: u32,
    },
    AmbiguousFixedCutSite {
        function: usize,
        register: u32,
        point: u32,
    },
    UnsupportedFixedTransitionAccess {
        function: usize,
        register: u32,
        instruction: u32,
        operand: u16,
    },
    IntervalOverflow {
        function: usize,
        register: u32,
    },
    SegmentIdentityOverflow {
        function: usize,
        register: u32,
    },
    /// Bounded work accounting refused: the logical receipt overflowed or
    /// exceeded the selected budget.
    Work(WorkDefect),
    UsageMismatch,
    NonCanonicalFunctions,
}

failed_display!(
    FixedPrecoloredSplitRequirementError,
    "fixed/precolored split-requirement analysis failed"
);

impl std::error::Error for FixedPrecoloredSplitRequirementError {}
