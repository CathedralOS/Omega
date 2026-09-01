//! Owned bounded-child lifecycle, limits, descriptors, and completion.
//!
//! [`BoundedProcessChild`] is the only launch/lifecycle owner. Completion
//! is returned only after explicit process-container closure and reaping.

mod child;
mod completion;
#[cfg(unix)]
mod descriptors;
pub(crate) mod limits;
#[cfg(windows)]
pub(crate) mod windows;

pub use child::BoundedProcessChild;
pub use completion::{BoundedProcessCompletion, BoundedProcessExitStatus};

#[cfg(all(test, unix))]
mod tests;
