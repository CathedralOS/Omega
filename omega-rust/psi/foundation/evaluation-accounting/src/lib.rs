#![forbid(unsafe_code)]

//! Shared deterministic resource accounts and versioned measurement records.
//! This owner contains counters, atomic reservation and retirement leases only.
//! Execution engines decide real charge sites and retain their own fuel/storage
//! identities; no program IR, compile-time coordinator or host service is copied.
//! A failed charge mutates neither invocation nor shared allowance. Moving a
//! lease transfers existing custody and dropping the final owner releases it.
//! Counters are logical resources, never CPU time, allocator bytes or RSS claims.

mod cached_usage;
mod invocation;
mod sponsor;
mod usage;

pub use invocation::*;
pub use sponsor::*;
pub use usage::*;
