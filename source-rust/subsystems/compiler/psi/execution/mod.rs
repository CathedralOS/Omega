//! Callable declarations, dispatch, suspension and task activation.

pub mod callable_reach;
pub mod callable_values;
mod contracts;
mod declarations;
mod dynamic_dispatch;
mod packages;
pub mod roots;
pub mod static_reach;
mod suspension;
pub mod task_activation;
pub mod unoptimized_behavior;

pub use declarations::ExecutionDeclarations;
pub use dynamic_dispatch::{
    TerminalDynamicClaimPath, TerminalDynamicDispatchCatalog, TerminalDynamicInterface,
    TerminalDynamicReferenceOrigin, TerminalDynamicReferenceRegion, TerminalDynamicSlot,
    TerminalDynamicStructuralFormal, TerminalDynamicTable,
};
pub use packages::{
    TerminalPackageCatalog, TerminalPackageEvent, TerminalPackageOpening, TerminalPackageSchema,
    TerminalPackageSite,
};
pub use suspension::{
    TerminalSuspensionCallPlan, TerminalSuspensionCallSite, TerminalSuspensionCallTarget,
    TerminalSuspensionLiveValue, TerminalSuspensionLoan, TerminalSuspensionLoanBacking,
    TerminalSuspensionLoanCapture, TerminalSuspensionLoanResource, TerminalSuspensionLoans,
    TerminalSuspensionPlace, TerminalSuspensionStorage, TerminalSuspensionValueLoans,
    TerminalSuspensionValueType, suspension_frontier_commitment,
};
