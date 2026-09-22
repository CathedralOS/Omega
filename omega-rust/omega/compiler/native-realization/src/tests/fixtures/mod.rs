//! Optimizer module role: stage group. Checked-source and hosted ProgramEntry test inputs.

pub(crate) mod checked_source;
pub(crate) mod hosted;

/// The empty behavior-exclusion union a fixture request carries unless a
/// test supplies its own; helpers that return a request borrow it.
pub(crate) static NO_BEHAVIOR_EXCLUSIONS: std::sync::LazyLock<
    build_evaluation::BehaviorExclusions,
> = std::sync::LazyLock::new(build_evaluation::BehaviorExclusions::default);
