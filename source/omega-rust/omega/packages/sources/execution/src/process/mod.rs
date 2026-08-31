//! Owned resolver-child lifecycle, limits, descriptors, and completion.
//!
//! [`ResolverExecutionChild`] is the only launch/lifecycle owner. Completion
//! is returned only after explicit process-container closure and reaping.

mod child;
mod completion;
#[cfg(unix)]
mod descriptors;
pub(crate) mod limits;
#[cfg(windows)]
pub(crate) mod windows;

pub use child::ResolverExecutionChild;
pub use completion::{ResolverExecutionCompletion, ResolverExecutionExitStatus};

#[cfg(all(test, unix))]
mod tests;
