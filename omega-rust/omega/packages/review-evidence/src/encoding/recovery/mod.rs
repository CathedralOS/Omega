//! Bounded recovery of canonical rows, source envelopes, and inert policy.
//!
//! This owner validates canonical row framing for local reconstruction and
//! owns both directions of the public recovered-row envelope. It does not
//! produce package-review comparison rows; that belongs to `encode`.
//! `policy/` restores receipt-free typed policy without compiler replay.

mod policy;

pub use policy::{
    PackagePolicyRecoveryError, PackagePolicyRecoveryLimits, PackagePolicyRecoveryUsage,
};
