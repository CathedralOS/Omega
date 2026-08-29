//! Owned resolver-child lifecycle, limits, descriptors, and completion.
//!
//! [`ResolverExecutionChild`] is the only launch/lifecycle owner. Completion
//! observations are issued only after explicit container closure and reaping.

mod child;
#[cfg(unix)]
mod descriptors;
pub(crate) mod limits;
mod observation;

pub use child::ResolverExecutionChild;
pub use observation::{
    ResolverExecutionCompletionObservation, ResolverExecutionExitStatus,
    ResolverExecutionTerminationDisposition,
};

#[cfg(all(test, unix))]
mod tests;
