#![forbid(unsafe_code)]

//! Optimizer module role: executable entrance. Layout-independent selected-form encoding, replay, and optimization custody.
//!
//! Start at `selected_form_encoding.rs`: this stage serializes selected
//! instructions before any address-dependent layout and retains exact
//! selected and physical roots. `row_encoding` owns per-row byte production,
//! `frame_address` resolves symbolic frame addresses against retained frame
//! geometry, and `validation` admits the produced bytes independently of the
//! producer.

mod frame_address;
mod row_encoding;
mod selected_form_encoding;
mod validation;

pub use selected_form_encoding::*;

// Names the folders reach through the crate root, as they did before the
// route file existed.
use machine_code::{
    DeferredControlEncodingReason, SelectedFormDecodedFootprint, SelectedFormEncoding,
    SelectedFormEncodingCounts, SelectedFormEncodingIdentity, SelectedFormEncodingRow,
    SelectedFormEncodingState, SelectedFormInternalMachineFixup,
    SelectedFormInternalMachineFixupKind, SelectedFormInternalMachineFixupState,
    SelectedFormMachineDisposition, SelectedFormNormalizedForeignCallFixup,
    SelectedFormNormalizedForeignCallFixupKind, SelectedFormNormalizedForeignCallFixupState,
};
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
