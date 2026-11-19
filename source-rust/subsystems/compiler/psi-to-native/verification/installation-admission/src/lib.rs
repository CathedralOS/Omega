//! Validate installation guards where the retained handler bodies meet an installer's frame.
//! Layout and publication retain this receipt and the same admitted provider entries.
#![forbid(unsafe_code)]

pub mod arrival;
mod authority;
mod resources;

pub use authority::BoundInstallationAuthority;
pub use resources::{
    EntryInstallationResources, InstalledHandlerEntry, PlannedInstallationResources,
    admit_planned_guards,
};
pub mod cpu;
pub mod ram;
pub mod selected;
