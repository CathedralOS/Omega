//! When a deferred check must hold relative to the operation it protects.

/// A call must prove its prerequisites before running. A promise about its
/// completed result is checked afterward and cannot justify that earlier call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum ScalarConditionRole {
    Prerequisite,
    Result,
}
