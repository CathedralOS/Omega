//! Platform assumptions supplied for an installation, and the physical conditions
//! measured for its interrupt stacks. Installation admission checks these rows
//! against the selected environment, handler and arrivals.

#![forbid(unsafe_code)]

mod environment;
mod interrupt_stack;
mod review;

pub use review::{decode_installation_assumptions, encode_installation_assumptions};

pub use environment::{
    BootstrapAssumptionInterval, BootstrapEnvironmentAssumption, InstallationEnvironmentAssumption,
    InterruptDeliveryInterval, X86InterruptDeliveryAssumption, X86InterruptDeliveryProfile,
};
pub use interrupt_stack::{
    InstalledArrivalStackDemand, InstalledBodyFaultPremises, InstalledInterruptStackRequirement,
    InstalledPhysicalStackRoot,
};
