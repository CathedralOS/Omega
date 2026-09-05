//! Stored, rebound, parameter, and argument dynamic-dispatch custody reconstruction.

use abstract_operations::{
    AbstractDynamicDescriptorArgument, AbstractDynamicDescriptorSource,
    AbstractParameterDynamicDispatch, AbstractReboundDynamicDispatch,
    AbstractStoredDynamicDescriptor, AbstractStoredDynamicDispatch,
};
use terminal_psi::{
    ClosedConformanceApplication, Operation, TerminalDynamicDescriptorSource,
    TerminalDynamicDispatchCatalog, TerminalMachine,
};

use crate::lowering::LoweringError;

pub(in crate::lowering::machine::operation) fn lower_stored_descriptor(
    machine: &TerminalMachine,
    operation: &Operation,
    descriptor_ordinal: u32,
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    closed_conformance_applications: &[ClosedConformanceApplication],
) -> Result<AbstractStoredDynamicDescriptor, LoweringError> {
    let descriptors = dynamic_dispatch
        .stored_descriptors
        .iter()
        .filter(|descriptor| {
            descriptor.owner == machine.id
                && descriptor.ordinal == descriptor_ordinal
                && descriptor.establishment_operation == operation.id
        })
        .collect::<Vec<_>>();
    let [descriptor] = descriptors.as_slice() else {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    };
    rejoin_stored_descriptor(
        machine,
        operation,
        descriptor,
        dynamic_dispatch,
        closed_conformance_applications,
    )
}

pub(super) fn lower_stored_dynamic_dispatch(
    machine: &TerminalMachine,
    operation: &Operation,
    descriptor_ordinal: u32,
    expected_result: Option<semantic_vocabulary::ScalarType>,
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    closed_conformance_applications: &[ClosedConformanceApplication],
) -> Result<AbstractStoredDynamicDispatch, LoweringError> {
    let dispatches = dynamic_dispatch
        .stored_dispatches
        .iter()
        .filter(|dispatch| {
            dispatch.owner == machine.id
                && dispatch.operation == operation.id
                && dispatch.descriptor_ordinal == descriptor_ordinal
        })
        .collect::<Vec<_>>();
    let [dispatch] = dispatches.as_slice() else {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    };
    let descriptors = dynamic_dispatch
        .stored_descriptors
        .iter()
        .filter(|descriptor| {
            descriptor.owner == machine.id && descriptor.ordinal == descriptor_ordinal
        })
        .collect::<Vec<_>>();
    let [descriptor] = descriptors.as_slice() else {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    };
    let stored = rejoin_stored_descriptor(
        machine,
        operation,
        descriptor,
        dynamic_dispatch,
        closed_conformance_applications,
    )?;
    let callable_count = stored
        .application
        .realization_callables
        .iter()
        .filter(|callable| {
            callable.source_callable_identity == dispatch.realization_callable_identity
                && callable.machine == dispatch.realization
                && closed_result_scalar(callable.result) == expected_result
        })
        .count();
    let lowered = AbstractStoredDynamicDispatch {
        stored,
        dispatch: (*dispatch).clone(),
    };
    if callable_count != 1 || !lowered.has_complete_custody(machine.id, operation.id) {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    }
    Ok(lowered)
}

fn rejoin_stored_descriptor(
    machine: &TerminalMachine,
    operation: &Operation,
    descriptor: &terminal_psi::TerminalStoredDynamicDescriptor,
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    closed_conformance_applications: &[ClosedConformanceApplication],
) -> Result<AbstractStoredDynamicDescriptor, LoweringError> {
    let selections = dynamic_dispatch
        .selections
        .iter()
        .filter(|selection| {
            selection.owner == machine.id && selection.ordinal == descriptor.selection_ordinal
        })
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    };
    let applications = closed_conformance_applications
        .iter()
        .filter(|application| {
            application.owner == machine.id
                && application.report_fingerprint
                    == selection.conformance_application_report_fingerprint
                && application.commitment == selection.conformance_application_commitment
        })
        .collect::<Vec<_>>();
    let [application] = applications.as_slice() else {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    };
    let lowered = AbstractStoredDynamicDescriptor {
        selection: (*selection).clone(),
        descriptor: descriptor.clone(),
        application: (*application).clone(),
    };
    if !lowered.has_complete_custody(machine.id, descriptor.establishment_operation) {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    }
    Ok(lowered)
}

pub(super) fn lower_rebound_dynamic_dispatch(
    machine: &TerminalMachine,
    operation: &Operation,
    descriptor_ordinal: u32,
    expected_result: Option<semantic_vocabulary::ScalarType>,
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    closed_conformance_applications: &[ClosedConformanceApplication],
) -> Result<AbstractReboundDynamicDispatch, LoweringError> {
    let descriptors = dynamic_dispatch
        .rebound_descriptors
        .iter()
        .filter(|descriptor| {
            descriptor.owner == machine.id && descriptor.ordinal == descriptor_ordinal
        })
        .collect::<Vec<_>>();
    let [descriptor] = descriptors.as_slice() else {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    };
    let selections = |ordinal| {
        dynamic_dispatch
            .selections
            .iter()
            .filter(|selection| selection.owner == machine.id && selection.ordinal == ordinal)
            .collect::<Vec<_>>()
    };
    let initial = selections(descriptor.initial_selection_ordinal);
    let rebound = selections(descriptor.rebound_selection_ordinal);
    let dispatches = dynamic_dispatch
        .indirect_dispatches
        .iter()
        .filter(|dispatch| {
            dispatch.owner == machine.id
                && dispatch.operation == operation.id
                && dispatch.descriptor_ordinal == descriptor_ordinal
        })
        .collect::<Vec<_>>();
    let ([initial], [rebound], [dispatch]) = (
        initial.as_slice(),
        rebound.as_slice(),
        dispatches.as_slice(),
    ) else {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    };
    let initial_applications = closed_conformance_applications
        .iter()
        .filter(|application| {
            application.owner == machine.id
                && application.report_fingerprint
                    == initial.conformance_application_report_fingerprint
                && application.commitment == initial.conformance_application_commitment
        })
        .collect::<Vec<_>>();
    let applications = closed_conformance_applications
        .iter()
        .filter(|application| {
            application.owner == machine.id
                && application.report_fingerprint
                    == rebound.conformance_application_report_fingerprint
                && application.commitment == rebound.conformance_application_commitment
        })
        .collect::<Vec<_>>();
    let ([initial_application], [application]) =
        (initial_applications.as_slice(), applications.as_slice())
    else {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    };
    let rows = application
        .rows
        .iter()
        .filter(|row| {
            row.declaring_trait_identity == dispatch.declaring_trait_identity
                && row.public_requirement_identity == dispatch.public_requirement_identity
                && row.requirement_identity == dispatch.requirement_identity
                && row.realization_identity == dispatch.realization_identity
                && row.realization_callable_identity.as_deref()
                    == Some(dispatch.realization_callable_identity.as_str())
        })
        .collect::<Vec<_>>();
    let callables = application
        .realization_callables
        .iter()
        .filter(|callable| {
            callable.source_callable_identity == dispatch.realization_callable_identity
                && callable.machine == dispatch.realization
                && closed_result_scalar(callable.result) == expected_result
        })
        .collect::<Vec<_>>();
    if !matches!(rows.as_slice(), [_]) || !matches!(callables.as_slice(), [_]) {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    }
    Ok(AbstractReboundDynamicDispatch {
        initial: (*initial).clone(),
        rebound: (*rebound).clone(),
        descriptor: (*descriptor).clone(),
        initial_application: (*initial_application).clone(),
        application: (*application).clone(),
        dispatch: (*dispatch).clone(),
    })
}

pub(super) fn lower_parameter_dynamic_dispatch(
    machine: &TerminalMachine,
    operation: &Operation,
    parameter_ordinal: u32,
    requirement_slot: u32,
    expected_result: Option<semantic_vocabulary::ScalarType>,
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
) -> Result<AbstractParameterDynamicDispatch, LoweringError> {
    let parameters = dynamic_dispatch
        .parameters
        .iter()
        .filter(|parameter| parameter.owner == machine.id && parameter.ordinal == parameter_ordinal)
        .collect::<Vec<_>>();
    let dispatches = dynamic_dispatch
        .parameter_dispatches
        .iter()
        .filter(|dispatch| {
            dispatch.owner == machine.id
                && dispatch.operation == operation.id
                && dispatch.parameter_ordinal == parameter_ordinal
                && dispatch.requirement_slot == requirement_slot
        })
        .collect::<Vec<_>>();
    let ([parameter], [dispatch]) = (parameters.as_slice(), dispatches.as_slice()) else {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    };
    let requirements = parameter
        .requirements
        .iter()
        .filter(|requirement| {
            requirement.slot == requirement_slot
                && closed_result_scalar(requirement.result) == expected_result
        })
        .collect::<Vec<_>>();
    if !matches!(requirements.as_slice(), [_]) {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    }
    Ok(AbstractParameterDynamicDispatch {
        parameter: (*parameter).clone(),
        dispatch: (*dispatch).clone(),
    })
}

fn closed_result_scalar(
    result: terminal_psi::ClosedConformanceCallableResult,
) -> Option<semantic_vocabulary::ScalarType> {
    match result {
        terminal_psi::ClosedConformanceCallableResult::Unit => None,
        terminal_psi::ClosedConformanceCallableResult::I32 => {
            Some(semantic_vocabulary::ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
                    .expect("the closed i32 result is valid"),
            ))
        }
        terminal_psi::ClosedConformanceCallableResult::Bool => {
            Some(semantic_vocabulary::ScalarType::Boolean)
        }
    }
}

pub(super) fn lower_dynamic_arguments(
    machine: &TerminalMachine,
    operation: &Operation,
    callee: semantic_vocabulary::MachineId,
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    closed_conformance_applications: &[ClosedConformanceApplication],
) -> Result<Vec<AbstractDynamicDescriptorArgument>, LoweringError> {
    let mut parameters = dynamic_dispatch
        .parameters
        .iter()
        .filter(|parameter| parameter.owner == callee)
        .collect::<Vec<_>>();
    parameters.sort_by_key(|parameter| parameter.ordinal);
    let arguments = dynamic_dispatch
        .arguments
        .iter()
        .filter(|argument| argument.owner == machine.id && argument.operation == operation.id)
        .collect::<Vec<_>>();
    if parameters.len() != arguments.len() {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    }

    parameters
        .into_iter()
        .map(|parameter| {
            let matches = arguments
                .iter()
                .filter(|argument| argument.parameter_ordinal == parameter.ordinal)
                .collect::<Vec<_>>();
            let [argument] = matches.as_slice() else {
                return Err(LoweringError::InvalidDynamicCall(operation.id));
            };
            let source = match argument.source {
                TerminalDynamicDescriptorSource::Selection { ordinal } => {
                    let selections = dynamic_dispatch
                        .selections
                        .iter()
                        .filter(|selection| {
                            selection.owner == machine.id && selection.ordinal == ordinal
                        })
                        .collect::<Vec<_>>();
                    let [selection] = selections.as_slice() else {
                        return Err(LoweringError::InvalidDynamicCall(operation.id));
                    };
                    let applications = closed_conformance_applications
                        .iter()
                        .filter(|application| {
                            application.owner == machine.id
                                && application.report_fingerprint
                                    == selection.conformance_application_report_fingerprint
                                && application.commitment
                                    == selection.conformance_application_commitment
                        })
                        .collect::<Vec<_>>();
                    let [application] = applications.as_slice() else {
                        return Err(LoweringError::InvalidDynamicCall(operation.id));
                    };
                    AbstractDynamicDescriptorSource::Selection {
                        selection: (**selection).clone(),
                        application: (**application).clone(),
                    }
                }
                TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal } => {
                    lower_rebound_argument_source(
                        machine,
                        operation,
                        ordinal,
                        dynamic_dispatch,
                        closed_conformance_applications,
                    )?
                }
                TerminalDynamicDescriptorSource::Parameter { ordinal } => {
                    let sources = dynamic_dispatch
                        .parameters
                        .iter()
                        .filter(|source| source.owner == machine.id && source.ordinal == ordinal)
                        .collect::<Vec<_>>();
                    let [source] = sources.as_slice() else {
                        return Err(LoweringError::InvalidDynamicCall(operation.id));
                    };
                    AbstractDynamicDescriptorSource::Parameter((*source).clone())
                }
            };
            Ok(AbstractDynamicDescriptorArgument {
                argument: (**argument).clone(),
                target: parameter.clone(),
                source,
            })
        })
        .collect()
}

fn lower_rebound_argument_source(
    machine: &TerminalMachine,
    operation: &Operation,
    descriptor_ordinal: u32,
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    closed_conformance_applications: &[ClosedConformanceApplication],
) -> Result<AbstractDynamicDescriptorSource, LoweringError> {
    let descriptors = dynamic_dispatch
        .rebound_descriptors
        .iter()
        .filter(|descriptor| {
            descriptor.owner == machine.id && descriptor.ordinal == descriptor_ordinal
        })
        .collect::<Vec<_>>();
    let [descriptor] = descriptors.as_slice() else {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    };
    let selections = |ordinal| {
        dynamic_dispatch
            .selections
            .iter()
            .filter(|selection| selection.owner == machine.id && selection.ordinal == ordinal)
            .collect::<Vec<_>>()
    };
    let initial = selections(descriptor.initial_selection_ordinal);
    let rebound = selections(descriptor.rebound_selection_ordinal);
    let ([initial], [rebound]) = (initial.as_slice(), rebound.as_slice()) else {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    };
    let initial_applications = closed_conformance_applications
        .iter()
        .filter(|application| {
            application.owner == machine.id
                && application.report_fingerprint
                    == initial.conformance_application_report_fingerprint
                && application.commitment == initial.conformance_application_commitment
        })
        .collect::<Vec<_>>();
    let applications = closed_conformance_applications
        .iter()
        .filter(|application| {
            application.owner == machine.id
                && application.report_fingerprint
                    == rebound.conformance_application_report_fingerprint
                && application.commitment == rebound.conformance_application_commitment
        })
        .collect::<Vec<_>>();
    let ([initial_application], [application]) =
        (initial_applications.as_slice(), applications.as_slice())
    else {
        return Err(LoweringError::InvalidDynamicCall(operation.id));
    };
    Ok(AbstractDynamicDescriptorSource::Rebound {
        initial: (*initial).clone(),
        rebound: (*rebound).clone(),
        descriptor: (*descriptor).clone(),
        initial_application: (*initial_application).clone(),
        application: (*application).clone(),
    })
}
