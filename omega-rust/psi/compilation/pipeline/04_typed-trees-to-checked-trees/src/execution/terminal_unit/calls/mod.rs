//! Structural call closure, argument custody, and claim transfer.
//!
//! `call_operations.rs` builds the call operation, `structural_arguments.rs` builds the arguments and their claim transfers,
//! `argument_paths.rs` the projected paths, `parameter_argument_paths.rs` which
//! path a parameter-rooted argument lends, `argument_presentation.rs` owns
//! shared carrier compatibility, `boundary_admission.rs` decides
//! what a boundary call admits, `signatures.rs` the signatures it is checked
//! against and `affine_locals.rs` the unit's opening affine locals; the
//! remaining modules cover view subslices (one admission for every site that
//! narrows a byte or element view), computation arguments and reference
//! forwarding. `dynamic_call.rs` plans a selector call against its
//! instantiated slot through the same argument planners.

mod affine_locals;
mod argument_paths;
pub(crate) use argument_paths::{projected_evaluated_path, value_index_segment};
pub(super) mod argument_presentation;
mod boundary_admission;
mod call_operations;
mod computation_arguments;
mod dynamic_call;
mod parameter_argument_paths;
mod reference_forwarding;
mod result_arguments;
mod service_forward;
mod signatures;
mod structural_arguments;
pub(crate) use structural_arguments::structural_call_arguments;
mod structural_result_call;
mod trait_boundary_call;
pub(crate) mod view_subslice;

pub(crate) use result_arguments::{
    is_stored_reference, retained_reference_argument, validate_shared_temporary,
};

/// The callee as argument planning reads it: formals, result and supply.
///
/// An ordinary or boundary target is a typed machine state the planner may
/// also inspect. A selector call's target is its instantiated slot: every
/// implementation it can reach is a checked body, so its operands take
/// checked-body custody, but no single body is known to inspect.
#[derive(Clone, Copy)]
pub(crate) struct CallTarget<'program> {
    pub(crate) parameters: &'program [typed_trees::signature::StateParameter],
    pub(crate) return_type: typed_trees::types::TypeReferenceHandle,
    pub(crate) supply_mode: language_semantics::MachineSupplyMode,
    /// The target's own declaration, when it has one a caller can inspect.
    pub(crate) declaration: Option<(
        &'program typed_trees::machine::Machine,
        &'program typed_trees::state::State,
    )>,
}

impl<'program> CallTarget<'program> {
    /// Signature-only transport is independent of the declaration's trust tier.
    /// A missing checked body is not permission to pretend it is an opaque one.
    pub(crate) fn has_bodyless_signature(self) -> bool {
        self.declaration
            .is_some_and(|(machine, _)| machine.has_bodyless_signature())
    }

    pub(crate) fn declared(
        program: &'program dyn ::validation::CheckingInput,
        machine: &'program typed_trees::machine::Machine,
        state: &'program typed_trees::state::State,
    ) -> Self {
        Self {
            parameters: program.state_parameters(state),
            return_type: state.return_type,
            supply_mode: machine.supply_mode,
            declaration: Some((machine, state)),
        }
    }

    pub(crate) fn dynamic_slot(
        slot: &'program typed_trees::dynamic_traits::DynamicSlotSignature,
    ) -> Self {
        Self {
            parameters: &slot.parameters,
            return_type: slot.return_type,
            supply_mode: language_semantics::MachineSupplyMode::CheckedBody,
            declaration: None,
        }
    }
}

pub(crate) use affine_locals::build_affine_array_construction_prefix;
pub(crate) use argument_paths::{
    borrowed_argument_endpoint, borrowed_index_segment, byte_sequence_literal_argument,
    projected_argument_path, projected_borrowed_path, projected_observed_path,
    projected_selected_path,
};
pub(crate) use boundary_admission::{
    BoundaryTraitReceiver, boundary_authority_refusal, boundary_trait_receiver_source,
    parameter_binding_receiver_sources, provider_attachment_receiver_field,
    provider_attachment_receiver_matches,
};
pub(in crate::execution) use call_operations::ExpectedCallValueResult;
pub(in crate::execution) use call_operations::build_call_operation;
pub(crate) use computation_arguments::{
    structural_computation_argument, subslice_computation_argument,
};
pub(crate) use signatures::{
    ambient_self_scalar_signature, boundary_owner_scalar_signature, entry_claims,
    free_structural_scalar_signature, free_structural_scalar_signature_traced,
    parameter_signatures, structural_scalar_signature_traced, structural_signature,
};
pub(crate) use structural_arguments::{call_claim_transfers, computed_call_claim_transfers};
