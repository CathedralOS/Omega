//! Ordinary, structural, boundary, and service-effect call tags.

use crate::optimization_unit::identity::carrier_encoding::{
    encode_abstract_result, encode_access, encode_ids, encode_scalar_type,
    encode_structural_argument,
};
use crate::optimization_unit::identity::structural_encoding::{
    encode_completion_claim_source, encode_crash_route_bucket,
    encode_outcome_specific_call_evidence, encode_structural_operation_result,
};

use super::{AbstractOperation, CanonicalBytes};
pub(super) fn encode(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
    use AbstractOperation as O;
    match operation {
        O::DynamicDescriptorParameter { parameter } => {
            bytes.u8(55);
            encode_dynamic_descriptor_parameter(bytes, parameter);
        }
        O::StoreDynamicDescriptor {
            psi_operation,
            stored,
        } => {
            bytes.u8(60);
            bytes.id(*psi_operation);
            encode_stored_dynamic_descriptor(bytes, stored);
        }
        O::CallStructural {
            psi_operation,
            callee,
            arguments,
            structural_arguments,
            dynamic_arguments,
            claim_transfers,
            result,
            requirement_obligations,
            crash_continuations,
        } => {
            use abstract_operations::AbstractStructuralCallResult as R;
            let layout = OrdinaryCallLayout::of(arguments, dynamic_arguments);
            match result {
                R::Unit | R::Scalar(_) => {
                    // Each (result, layout) pair keeps the tag and field order
                    // of the variant it replaced, so folding the Unit and
                    // scalar-result calls into one variant moved no identity.
                    bytes.u8(match (result, layout) {
                        (R::Unit, OrdinaryCallLayout::Plain) => 3,
                        (R::Unit, OrdinaryCallLayout::Descriptors) => 59,
                        (R::Unit, OrdinaryCallLayout::ScalarsAndDescriptors) => 94,
                        (_, OrdinaryCallLayout::Plain) => 4,
                        (_, OrdinaryCallLayout::Descriptors) => 54,
                        (_, OrdinaryCallLayout::ScalarsAndDescriptors) => 95,
                    });
                    bytes.id(*psi_operation);
                    if let Some(result) = result.scalar() {
                        encode_abstract_result(bytes, result);
                    }
                    bytes.id(*callee);
                    encode_ordinary_call_arguments(
                        bytes,
                        layout,
                        arguments,
                        structural_arguments,
                        dynamic_arguments,
                    );
                    encode_ordinary_call_tail(
                        bytes,
                        claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                    );
                }
                R::Structural {
                    result,
                    returned_claim_transfers,
                    selected_evidence,
                } => {
                    // A structural result keeps tag 5 and its legacy field
                    // order. It never forwarded descriptors: stage 00 never
                    // produces that shape and the optimization-unit contract
                    // rejects it, but it takes the fresh tag 96 with its
                    // descriptor roster so that no two distinct rows share one
                    // encoding.
                    let forwards_descriptors = !dynamic_arguments.is_empty();
                    bytes.u8(if forwards_descriptors { 96 } else { 5 });
                    bytes.id(*psi_operation);
                    encode_structural_operation_result(bytes, result);
                    bytes.id(*callee);
                    encode_ids(bytes, arguments);
                    bytes.slice(structural_arguments, encode_structural_argument);
                    if forwards_descriptors {
                        bytes.slice(dynamic_arguments, encode_dynamic_descriptor_argument);
                    }
                    bytes.slice(claim_transfers, |bytes, transfer| {
                        bytes.id(transfer.claim);
                        bytes.u32(transfer.argument_index);
                    });
                    bytes.slice(returned_claim_transfers, |bytes, transfer| {
                        bytes.id(transfer.callee_claim);
                        bytes.id(transfer.caller_claim);
                    });
                    encode_ids(bytes, requirement_obligations);
                    bytes.slice(crash_continuations, encode_crash_route_bucket);
                    bytes.slice(selected_evidence, encode_outcome_specific_call_evidence);
                }
            }
        }
        O::CallDynamic {
            psi_operation,
            dispatch,
            result,
            requirement_obligations,
            crash_continuations,
        } => {
            use abstract_operations::AbstractDynamicCallResult as R;
            use abstract_operations::AbstractDynamicDispatch as D;
            // Each (custody, result) pair keeps the tag of the variant it
            // replaced, so merging the variants moved no identity. Stored
            // custody with a Unit result had no variant; it takes a fresh tag.
            bytes.u8(match (dispatch, result) {
                (D::Rebound(_), R::Scalar(_)) => 52,
                (D::Parameter(_), R::Scalar(_)) => 53,
                (D::Rebound(_), R::Unit) => 57,
                (D::Parameter(_), R::Unit) => 58,
                (D::Stored(_), R::Scalar(_)) => 61,
                (D::Stored(_), R::Unit) => 93,
            });
            bytes.id(*psi_operation);
            if let R::Scalar(result) = result {
                encode_abstract_result(bytes, *result);
            }
            match dispatch {
                D::Rebound(dispatch) => encode_rebound_dynamic_dispatch(bytes, dispatch),
                D::Stored(dispatch) => encode_stored_dynamic_dispatch(bytes, dispatch),
                D::Parameter(dispatch) => encode_parameter_dynamic_dispatch(bytes, dispatch),
            }
            encode_ids(bytes, requirement_obligations);
            bytes.slice(crash_continuations, encode_crash_route_bucket);
        }
        O::BoundaryCall {
            psi_operation,
            result,
            boundary,
            arguments,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
        } => {
            bytes.u8(6);
            bytes.id(*psi_operation);
            match result {
                abstract_operations::AbstractBoundaryResult::Unit => bytes.u8(0),
                abstract_operations::AbstractBoundaryResult::Scalar(result) => {
                    bytes.u8(1);
                    encode_abstract_result(bytes, *result);
                }
                abstract_operations::AbstractBoundaryResult::Structural(result) => {
                    bytes.u8(2);
                    encode_structural_operation_result(bytes, result);
                }
            }
            bytes.id(*boundary);
            encode_ids(bytes, arguments);
            bytes.slice(structural_arguments, encode_structural_argument);
            bytes.slice(completion_claim_sources, encode_completion_claim_source);
            bytes.slice(completion_receipts, |bytes, receipt| {
                bytes.id(receipt.claim);
                bytes.u32(receipt.argument_index);
            });
        }
        O::PortWrite {
            psi_operation,
            service,
            port,
            value,
        } => {
            bytes.u8(7);
            bytes.id(*psi_operation);
            bytes.id(*service);
            bytes.u16(*port);
            bytes.u8(*value);
        }
        O::Call {
            psi_operation,
            result,
            scalar_type,
            callee,
            arguments,
            requirement_obligations,
            crash_continuations,
        } => {
            bytes.u8(8);
            bytes.id(*psi_operation);
            bytes.id(*result);
            encode_scalar_type(bytes, *scalar_type);
            bytes.id(*callee);
            encode_ids(bytes, arguments);
            encode_ids(bytes, requirement_obligations);
            bytes.slice(crash_continuations, encode_crash_route_bucket);
        }
        _ => unreachable!("operation family routing admitted a non-call operation"),
    }
}

/// The argument layout of one Unit or scalar-result structural-lane call.
/// Each layout keeps the tag and field order of the variant it replaced, so
/// folding descriptor arguments into the call moved no identity. A call
/// forwarding descriptors had no scalar argument roster. Scalar arguments
/// beside descriptor arguments had no variant: stage 00 never produces that
/// shape and the optimization-unit contract rejects it, but it takes fresh tags
/// with both rosters so that no two distinct rows share one encoding.
#[derive(Clone, Copy)]
enum OrdinaryCallLayout {
    Plain,
    Descriptors,
    ScalarsAndDescriptors,
}

impl OrdinaryCallLayout {
    fn of(
        arguments: &[semantic_vocabulary::ValueId],
        dynamic_arguments: &[abstract_operations::AbstractDynamicDescriptorArgument],
    ) -> Self {
        match (arguments.is_empty(), dynamic_arguments.is_empty()) {
            (_, true) => Self::Plain,
            (true, false) => Self::Descriptors,
            (false, false) => Self::ScalarsAndDescriptors,
        }
    }
}

fn encode_ordinary_call_arguments(
    bytes: &mut CanonicalBytes,
    layout: OrdinaryCallLayout,
    arguments: &[semantic_vocabulary::ValueId],
    structural_arguments: &[terminal_psi::StructuralArgument],
    dynamic_arguments: &[abstract_operations::AbstractDynamicDescriptorArgument],
) {
    if !matches!(layout, OrdinaryCallLayout::Descriptors) {
        encode_ids(bytes, arguments);
    }
    bytes.slice(structural_arguments, encode_structural_argument);
    if !matches!(layout, OrdinaryCallLayout::Plain) {
        bytes.slice(dynamic_arguments, encode_dynamic_descriptor_argument);
    }
}

fn encode_ordinary_call_tail(
    bytes: &mut CanonicalBytes,
    claim_transfers: &[terminal_psi::ClaimTransfer],
    requirement_obligations: &[semantic_vocabulary::ObligationId],
    crash_continuations: &[terminal_psi::CrashRouteBucket],
) {
    bytes.slice(claim_transfers, |bytes, transfer| {
        bytes.id(transfer.claim);
        bytes.u32(transfer.argument_index);
    });
    encode_ids(bytes, requirement_obligations);
    bytes.slice(crash_continuations, encode_crash_route_bucket);
}

fn encode_rebound_dynamic_dispatch(
    bytes: &mut CanonicalBytes,
    dynamic_dispatch: &abstract_operations::AbstractReboundDynamicDispatch,
) {
    encode_dynamic_selection(bytes, &dynamic_dispatch.initial);
    encode_dynamic_selection(bytes, &dynamic_dispatch.rebound);
    bytes.id(dynamic_dispatch.descriptor.owner);
    bytes.u32(dynamic_dispatch.descriptor.ordinal);
    bytes.u32(dynamic_dispatch.descriptor.initial_selection_ordinal);
    bytes.u32(dynamic_dispatch.descriptor.rebound_selection_ordinal);
    encode_closed_conformance_application(bytes, &dynamic_dispatch.application);
    if dynamic_dispatch.initial_application != dynamic_dispatch.application {
        bytes.u8(1);
        encode_closed_conformance_application(bytes, &dynamic_dispatch.initial_application);
    }
    bytes.id(dynamic_dispatch.dispatch.owner);
    bytes.id(dynamic_dispatch.dispatch.operation);
    bytes.u32(dynamic_dispatch.dispatch.descriptor_ordinal);
    bytes.string(&dynamic_dispatch.dispatch.declaring_trait_identity);
    bytes.string(&dynamic_dispatch.dispatch.public_requirement_identity);
    bytes.slice(
        &dynamic_dispatch.dispatch.family_tuple,
        |bytes, identity| {
            bytes.string(identity);
        },
    );
    bytes.string(&dynamic_dispatch.dispatch.requirement_identity);
    bytes.string(&dynamic_dispatch.dispatch.realization_identity);
    bytes.string(&dynamic_dispatch.dispatch.realization_callable_identity);
    bytes.id(dynamic_dispatch.dispatch.realization);
}

fn encode_stored_dynamic_descriptor(
    bytes: &mut CanonicalBytes,
    stored: &abstract_operations::AbstractStoredDynamicDescriptor,
) {
    encode_dynamic_selection(bytes, &stored.selection);
    bytes.id(stored.descriptor.owner);
    bytes.u32(stored.descriptor.ordinal);
    bytes.id(stored.descriptor.establishment_operation);
    bytes.u32(stored.descriptor.selection_ordinal);
    bytes.string(&stored.descriptor.aggregate_type_identity);
    bytes.string(&stored.descriptor.field_identity);
    encode_closed_conformance_application(bytes, &stored.application);
}

fn encode_stored_dynamic_dispatch(
    bytes: &mut CanonicalBytes,
    dynamic_dispatch: &abstract_operations::AbstractStoredDynamicDispatch,
) {
    encode_stored_dynamic_descriptor(bytes, &dynamic_dispatch.stored);
    bytes.id(dynamic_dispatch.dispatch.owner);
    bytes.id(dynamic_dispatch.dispatch.operation);
    bytes.u32(dynamic_dispatch.dispatch.descriptor_ordinal);
    bytes.string(&dynamic_dispatch.dispatch.declaring_trait_identity);
    bytes.string(&dynamic_dispatch.dispatch.public_requirement_identity);
    bytes.slice(
        &dynamic_dispatch.dispatch.family_tuple,
        |bytes, identity| {
            bytes.string(identity);
        },
    );
    bytes.string(&dynamic_dispatch.dispatch.requirement_identity);
    bytes.string(&dynamic_dispatch.dispatch.realization_identity);
    bytes.string(&dynamic_dispatch.dispatch.realization_callable_identity);
    bytes.id(dynamic_dispatch.dispatch.realization);
}

fn encode_parameter_dynamic_dispatch(
    bytes: &mut CanonicalBytes,
    dynamic_dispatch: &abstract_operations::AbstractParameterDynamicDispatch,
) {
    encode_dynamic_descriptor_parameter(bytes, &dynamic_dispatch.parameter);
    bytes.id(dynamic_dispatch.dispatch.owner);
    bytes.id(dynamic_dispatch.dispatch.operation);
    bytes.u32(dynamic_dispatch.dispatch.parameter_ordinal);
    bytes.u32(dynamic_dispatch.dispatch.requirement_slot);
}

fn encode_dynamic_descriptor_argument(
    bytes: &mut CanonicalBytes,
    argument: &abstract_operations::AbstractDynamicDescriptorArgument,
) {
    bytes.id(argument.argument.owner);
    bytes.id(argument.argument.operation);
    bytes.u32(argument.argument.parameter_ordinal);
    match argument.argument.source {
        terminal_psi::TerminalDynamicDescriptorSource::Selection { ordinal } => {
            bytes.u8(3);
            bytes.u32(ordinal);
        }
        terminal_psi::TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal } => {
            bytes.u8(1);
            bytes.u32(ordinal);
        }
        terminal_psi::TerminalDynamicDescriptorSource::Parameter { ordinal } => {
            bytes.u8(2);
            bytes.u32(ordinal);
        }
    }
    encode_dynamic_descriptor_parameter(bytes, &argument.target);
    match &argument.source {
        abstract_operations::AbstractDynamicDescriptorSource::Selection {
            selection,
            application,
        } => {
            bytes.u8(3);
            encode_dynamic_selection(bytes, selection);
            encode_closed_conformance_application(bytes, application);
        }
        abstract_operations::AbstractDynamicDescriptorSource::Rebound {
            initial,
            rebound,
            descriptor,
            initial_application,
            application,
        } => {
            bytes.u8(1);
            encode_dynamic_selection(bytes, initial);
            encode_dynamic_selection(bytes, rebound);
            bytes.id(descriptor.owner);
            bytes.u32(descriptor.ordinal);
            bytes.u32(descriptor.initial_selection_ordinal);
            bytes.u32(descriptor.rebound_selection_ordinal);
            encode_closed_conformance_application(bytes, application);
            if initial_application != application {
                bytes.u8(1);
                encode_closed_conformance_application(bytes, initial_application);
            }
        }
        abstract_operations::AbstractDynamicDescriptorSource::Parameter(parameter) => {
            bytes.u8(2);
            encode_dynamic_descriptor_parameter(bytes, parameter);
        }
    }
}

fn encode_dynamic_descriptor_parameter(
    bytes: &mut CanonicalBytes,
    parameter: &terminal_psi::TerminalDynamicDescriptorParameter,
) {
    bytes.id(parameter.owner);
    bytes.u32(parameter.ordinal);
    bytes.u32(parameter.source_position);
    bytes.string(&parameter.trait_identity);
    encode_access(bytes, parameter.access);
    bytes.slice(&parameter.requirements, |bytes, requirement| {
        bytes.u32(requirement.slot);
        bytes.string(&requirement.declaring_trait_identity);
        bytes.string(&requirement.public_requirement_identity);
        bytes.slice(&requirement.family_tuple, |bytes, identity| {
            bytes.string(identity);
        });
        bytes.u8(match requirement.result {
            terminal_psi::ClosedConformanceCallableResult::Unit => 1,
            terminal_psi::ClosedConformanceCallableResult::I32 => 2,
            terminal_psi::ClosedConformanceCallableResult::Bool => 3,
        });
    });
}

fn encode_dynamic_selection(
    bytes: &mut CanonicalBytes,
    selection: &terminal_psi::TerminalDynamicConformanceSelection,
) {
    bytes.id(selection.owner);
    bytes.u32(selection.ordinal);
    encode_structural_argument(bytes, &selection.source);
    bytes.u64(selection.conformance_application_report_fingerprint);
    bytes.bytes(&selection.conformance_application_commitment.as_bytes());
}

fn encode_closed_conformance_application(
    bytes: &mut CanonicalBytes,
    application: &terminal_psi::ClosedConformanceApplication,
) {
    bytes.id(application.owner);
    bytes.string(&application.declaration_identity);
    bytes.slice(&application.telescope, |bytes, binding| {
        bytes.string(&binding.parameter);
        bytes.u8(match binding.kind {
            terminal_psi::ClosedConformanceParameterKind::Lifetime => 1,
            terminal_psi::ClosedConformanceParameterKind::Type => 2,
            terminal_psi::ClosedConformanceParameterKind::Const => 3,
            terminal_psi::ClosedConformanceParameterKind::Machine => 4,
        });
        bytes.string(&binding.argument);
    });
    bytes.boolean(application.subject_identity.is_some());
    if let Some(subject) = &application.subject_identity {
        bytes.string(subject);
    }
    bytes.string(&application.trait_identity);
    bytes.slice(&application.trait_lifetime_arguments, |bytes, argument| {
        bytes.string(argument);
    });
    bytes.slice(&application.trait_arguments, |bytes, argument| {
        bytes.string(argument);
    });
    bytes.slice(&application.realization_callables, |bytes, callable| {
        bytes.string(&callable.source_callable_identity);
        bytes.id(callable.machine);
        bytes.u8(match callable.result {
            terminal_psi::ClosedConformanceCallableResult::Unit => 1,
            terminal_psi::ClosedConformanceCallableResult::I32 => 2,
            terminal_psi::ClosedConformanceCallableResult::Bool => 3,
        });
    });
    bytes.slice(&application.rows, |bytes, row| {
        bytes.string(&row.declaring_trait_identity);
        bytes.string(&row.public_requirement_identity);
        bytes.slice(&row.family_tuple, |bytes, identity| {
            bytes.string(identity);
        });
        bytes.string(&row.requirement_identity);
        bytes.string(&row.realization_identity);
        bytes.boolean(row.realization_callable_identity.is_some());
        if let Some(identity) = &row.realization_callable_identity {
            bytes.string(identity);
        }
    });
    bytes.u64(application.report_fingerprint);
    bytes.bytes(&application.commitment.as_bytes());
}
