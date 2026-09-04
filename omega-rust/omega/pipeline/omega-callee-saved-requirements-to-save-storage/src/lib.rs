#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Callee-saved requirements to validated abstract save storage.
//!
//! The executable entrance assigns canonical area-relative storage through
//! target-owned preservation groups and independently replays that assignment.
//! It grants no concrete frame geometry, instruction, unwind, or emission authority.

mod callee_save_storage;

use omega_register_homes_to_callee_saved_requirements::{
    AllocatedCalleeSavedFunctionKind, AllocatedCalleeSavedRequirementIdentity,
    AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedUnitRequirement,
    CalleeSavedModificationWitness, FunctionAllocatedCalleeSavedRequirements,
    ValidatedAllocatedCalleeSavedRequirements,
};
use omega_target_to_register_environment::{
    FrameAbiPreservationConvention, ValidatedTargetRegisterEnvironment,
};

pub use callee_save_storage::*;
