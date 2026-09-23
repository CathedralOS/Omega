//! Structural call closure, argument custody, and claim transfer.
//!
//! `call_operations.rs` builds the call operation, `structural_arguments.rs` builds the arguments and their claim transfers,
//! `argument_paths.rs` the projected paths, `boundary_admission.rs` decides
//! what a boundary call admits, `signatures.rs` the signatures it is checked
//! against and `affine_locals.rs` the unit's opening affine locals; the
//! remaining modules cover byte subslices, computation arguments and
//! reference forwarding.

mod affine_locals;
mod argument_paths;
mod boundary_admission;
pub(in crate::execution) mod byte_subslice;
mod call_operations;
mod computation_arguments;
pub(in crate::execution) mod element_subslice;
mod reference_forwarding;
mod result_arguments;
#[cfg(test)]
mod scalar_argument_tests;
mod service_forward;
mod signatures;
mod structural_arguments;

pub(crate) use affine_locals::{
    build_affine_array_construction_prefix, build_unit_trivial_affine_locals,
};
pub(crate) use argument_paths::{projected_argument_path, projected_borrowed_receiver_path};
pub(crate) use boundary_admission::provider_attachment_receiver_matches;
pub(in crate::execution) use call_operations::ExpectedCallValueResult;
pub(in crate::execution) use call_operations::build_call_operation;
pub(crate) use computation_arguments::structural_computation_argument;
pub(crate) use signatures::{
    ambient_self_scalar_graph_signature, entry_claims, free_fused_service_scalar_signature,
    free_structural_scalar_signature, free_structural_scalar_signature_traced,
    fused_service_scalar_signature, partial_affine_structural_signature,
    structural_scalar_signature, structural_scalar_signature_traced, structural_signature,
};
pub(crate) use structural_arguments::call_claim_transfers;
/// Rejoin a bodyless compiler-intrinsic satisfier to the exact boundary-trait
/// requirement whose call it realizes. Provider selection may resolve a call
/// to the target satisfier before checked Unit planning; that satisfier is not
/// an ordinary transitive machine body. Terminal projection grants no
/// execution authority here: the later sealed compiler catalog independently
/// decides whether the exact requirement/realization/target tuple is native.
pub(super) use validation::exact_compiler_intrinsic_boundary_requirement;
