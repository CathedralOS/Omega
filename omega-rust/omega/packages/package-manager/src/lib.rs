/*
Compiler and command operations for Omega's registry-free package manager.

We organize package management around explicit command admission, review
decisions, and atomic publication in `package_manager`. Application Build
schedules the admitted target-review requests, while `operations` exposes
compiler preparation and read-only source/package inspection.

The crate-wide `bounded_limits!` macro clamps every limit field against
`Self::default()` (the hard ceiling shape shared by all package limit records),
and `reserve!` folds `try_reserve_exact(n).map_err(|_| <$error>::AllocationFailed)?`
into one statement across text readers and projections.
*/

#![forbid(unsafe_code)]

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

macro_rules! reserve {
    ($items:expr, $additional:expr, $error:ty) => {
        $items
            .try_reserve_exact($additional)
            .map_err(|_| <$error>::AllocationFailed)?
    };
}
pub(crate) use reserve;

pub(crate) use optimization_core::codec_bytes;
mod build_session;
mod hex;
pub(crate) use hex::{Hex, hex_digest};
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
