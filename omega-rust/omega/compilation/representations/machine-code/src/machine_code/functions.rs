//! Emitted program functions and separately identified compiler-private functions.

pub mod exit_contract;
pub use exit_contract::{
    WholeFunctionCrashEvidence, WholeFunctionEntryAssumption, WholeFunctionExitContract,
    WholeFunctionExitContractIdentity, WholeFunctionExitEvidence, WholeFunctionExitLayoutCustody,
    WholeFunctionExitPolicy, WholeFunctionFrameDisposition, WholeFunctionHardeningPolicy,
    WholeFunctionProcessExitEvidence, WholeFunctionReturnEvidence,
    WholeFunctionReturnFragmentEvidence, WholeFunctionReturnMechanism,
    WholeFunctionReturnValueEvidence, whole_function_exit_contract_identity,
};
