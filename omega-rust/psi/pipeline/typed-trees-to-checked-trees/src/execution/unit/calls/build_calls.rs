//! Structural call closure, argument custody, and claim transfer.
//!
//! `call_operations.rs` builds the call operation, `structural_arguments.rs` builds the arguments and their claim transfers,
//! `argument_paths.rs` the projected paths, `boundary_admission.rs` decides
//! what a boundary call admits, `signatures.rs` the signatures it is checked
//! against and `affine_locals.rs` the unit's opening affine locals; the
//! remaining modules cover byte subslices, computation arguments and
//! reference forwarding.

#[path = "affine_locals.rs"]
mod affine_locals;
#[path = "argument_paths.rs"]
mod argument_paths;
#[path = "boundary_admission.rs"]
mod boundary_admission;
#[path = "byte_subslice.rs"]
pub(super) mod byte_subslice;
#[path = "call_operations.rs"]
mod call_operations;
#[path = "computation_arguments/build_computation_arguments.rs"]
mod computation_arguments;
#[path = "reference_forwarding/build_reference_forwarding.rs"]
mod reference_forwarding;
#[path = "result_arguments/build_result_arguments.rs"]
mod result_arguments;
#[cfg(test)]
#[path = "scalar_argument_tests.rs"]
mod scalar_argument_tests;
#[path = "service_forward.rs"]
mod service_forward;
#[path = "signatures.rs"]
mod signatures;
#[path = "structural_arguments.rs"]
mod structural_arguments;

pub(crate) use affine_locals::{
    build_affine_array_construction_prefix, build_unit_trivial_affine_locals,
};
pub(crate) use argument_paths::projected_argument_path;
pub(crate) use boundary_admission::provider_attachment_receiver_matches;
pub(in crate::execution) use call_operations::ExpectedCallValueResult;
pub(in crate::execution) use call_operations::build_call_operation;
pub(crate) use computation_arguments::structural_computation_argument;
pub(crate) use signatures::{
    entry_claims, free_fused_service_scalar_signature, free_structural_scalar_signature,
    free_structural_scalar_signature_traced, fused_service_scalar_signature,
    partial_affine_structural_signature, structural_scalar_signature,
    structural_scalar_signature_traced, structural_signature,
};
pub(crate) use structural_arguments::call_claim_transfers;
/// Rejoin a bodyless compiler-intrinsic satisfier to the exact boundary-trait
/// requirement whose call it realizes. Provider selection may resolve a call
/// to the target satisfier before checked Unit planning; that satisfier is not
/// an ordinary transitive machine body. Terminal projection grants no
/// execution authority here: the later sealed compiler catalog independently
/// decides whether the exact requirement/realization/target tuple is native.
pub(super) use validation::exact_compiler_intrinsic_boundary_requirement;
