//! Validates structural, boundary, and effect operation custody.
//! Scalar operands do not change structural conservation: both call encodings
//! share the linear transfer, returned-claim and callee-content joins. Scalar
//! types and dominance are checked by ordinary operation validation separately.
//!
//! `unit_operation.rs` validates one Unit operation statically;
//! `payloadless_calls.rs` recognizes exact payloadless calls and case-return
//! exits, `primitive_calls.rs` validates primitive structural calls,
//! `contract_places.rs` and `structural_arguments.rs` validate contract
//! places and structural arguments, `claim_transfers.rs` validates service
//! reach and claim transfers, `crash_continuations.rs` validates crash
//! continuations, `structural_paths.rs` derives canonical prefixes and
//! store paths and `boundary_requirements.rs` validates boundary
//! requirements and completion receipts.

mod boundary_requirements;
mod claim_transfers;
mod contract_places;
mod crash_continuations;
mod payloadless_calls;
mod primitive_calls;
mod structural_arguments;
mod structural_paths;
mod unit_operation;

pub(crate) use claim_transfers::{validate_service_reach, validate_unit_call_claim_transfers};
pub(crate) use contract_places::validate_unit_call_contract_places;
pub(crate) use crash_continuations::validate_unit_call_crash_continuations;
pub(crate) use payloadless_calls::{
    exact_payloadless_case_return_exits, exact_payloadless_structural_call,
};
pub(crate) use structural_arguments::{
    StructuralArgumentSourcePolicy, is_unrestricted_mutable_subloan,
    is_unrestricted_shared_subloan, structural_access_can_supply, validate_structural_arguments,
};
pub(crate) use structural_paths::{
    canonical_field_path, structural_argument_canonical_prefix, structural_field_store_write_path,
};
pub(crate) use unit_operation::validate_unit_operation_static;
