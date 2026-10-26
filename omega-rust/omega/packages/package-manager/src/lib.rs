#![forbid(unsafe_code)]

//! The compiler and command operations for Omega's registry-free package
//! manager.
//!
//! Start with [`package_manager`] for command admission, decisions and publication.
//! Application Build schedules the admitted target-review requests.
//! [`operations`] also exposes compiler preparation and read-only inspection.

/// `bounded()` clamps every limit field against `Self::default()` — the hard
/// ceiling shape all package limit records share.
macro_rules! bounded_limits {
    ($vis:vis fn; $($field:ident),+ $(,)?) => {
        $vis fn bounded(self) -> Self {
            let hard = Self::default();
            Self {
                $($field: self.$field.min(hard.$field),)+
            }
        }
    };
}
pub(crate) use bounded_limits;

pub(crate) use optimization_core::codec_bytes;
mod build_session;
mod hex;
pub(crate) use hex::Hex;
pub mod declarations;
pub(crate) use optimization_core::digest_encoder;
pub mod lock;
pub mod operations;
pub mod package_manager;
pub mod resolution;
pub mod review;

pub use build_session::{PackageBuildSession, PackageSourceBindings};

pub use package_manager::{
    PackageCommand, PackageCommandError, PackageCommandKind, PackageCommandOptions,
    PackageCommandOutcome, PackageCommandPreparation, PackageCommandReview,
    PackageCommandReviewRequest, PackageCommandStatus, PackageCommandTransaction,
    PreparedPackageCommand, discard_package_review,
};

// The unit tests compile real packages on several threads; mimalloc's
// per-thread heaps avoid the system allocator's zone locks. Test-only: the
// product keeps the system allocator.
