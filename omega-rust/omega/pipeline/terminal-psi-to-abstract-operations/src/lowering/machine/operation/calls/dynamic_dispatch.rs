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
                && row.family_tuple == dispatch.family_tuple
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

#[cfg(test)]
mod tests {
    //! The family tuple is a row coordinate: a dispatch rejoins the table row
    //! whose `(declaring trait, complete requirement overload, canonical value
    //! tuple)` all match, not merely the two identities.

    use semantic_vocabulary::{BlockId, ContractId, MachineId, OperationId, PlaceId};
    use terminal_psi::{
        ClosedConformanceApplication, ClosedConformanceCallableResult,
        ClosedConformanceRealizationCallable, ClosedConformanceRow, MachineContract, Operation,
        OperationKind, OperationResult, StructuralAccess, StructuralArgument,
        TerminalDynamicConformanceSelection, TerminalDynamicDispatchCatalog,
        TerminalIndirectDynamicDispatch, TerminalMachine, TerminalMachineResult,
        TerminalReboundDynamicDescriptor, closed_conformance_application_commitment,
        closed_conformance_application_report_fingerprint,
    };

    use super::lower_rebound_dynamic_dispatch;

    const WIDTH_16: &str = "named(integer-const(16))";
    const WIDTH_32: &str = "named(integer-const(32))";
    const WIDTH_64: &str = "named(integer-const(64))";

    fn machine() -> TerminalMachine {
        TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(1).unwrap(),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).unwrap(),
            blocks: Vec::new(),
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }
    }

    fn family_row(tuple: &str, realization: MachineId) -> ClosedConformanceRow {
        let width = realization.get();
        ClosedConformanceRow {
            declaring_trait_identity: "test::Scanner".to_owned(),
            public_requirement_identity: "test::Scanner::scan()".to_owned(),
            family_tuple: vec![tuple.to_owned()],
            requirement_identity: "test::Scanner::scan".to_owned(),
            realization_identity: format!("test::Carrier::scan<{width}>"),
            realization_callable_identity: Some(format!("test::Carrier::scan<{width}>::callable")),
        }
    }

    fn widths_application(owner: MachineId) -> ClosedConformanceApplication {
        let mut application = ClosedConformanceApplication {
            owner,
            declaration_identity: "test::CarrierImplementsScanner".to_owned(),
            telescope: Vec::new(),
            subject_identity: Some("test::Carrier".to_owned()),
            trait_identity: "test::Scanner".to_owned(),
            trait_lifetime_arguments: Vec::new(),
            trait_arguments: Vec::new(),
            realization_callables: vec![
                ClosedConformanceRealizationCallable {
                    source_callable_identity: "test::Carrier::scan<91>::callable".to_owned(),
                    machine: MachineId::new(91).unwrap(),
                    result: ClosedConformanceCallableResult::Unit,
                },
                ClosedConformanceRealizationCallable {
                    source_callable_identity: "test::Carrier::scan<92>::callable".to_owned(),
                    machine: MachineId::new(92).unwrap(),
                    result: ClosedConformanceCallableResult::Unit,
                },
            ],
            rows: vec![
                family_row(WIDTH_16, MachineId::new(91).unwrap()),
                family_row(WIDTH_32, MachineId::new(92).unwrap()),
            ],
            report_fingerprint: 0,
            commitment: Default::default(),
        };
        application.report_fingerprint =
            closed_conformance_application_report_fingerprint(&application);
        application.commitment = closed_conformance_application_commitment(&application);
        application
    }

    fn catalog_for(
        owner: MachineId,
        application: &ClosedConformanceApplication,
        dispatch: TerminalIndirectDynamicDispatch,
    ) -> TerminalDynamicDispatchCatalog {
        let selection = |ordinal| TerminalDynamicConformanceSelection {
            owner,
            ordinal,
            source: StructuralArgument {
                place: PlaceId::new(11).unwrap(),
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            },
            conformance_application_report_fingerprint: application.report_fingerprint,
            conformance_application_commitment: application.commitment,
        };
        TerminalDynamicDispatchCatalog {
            selections: vec![selection(0), selection(1)],
            rebound_descriptors: vec![TerminalReboundDynamicDescriptor {
                owner,
                ordinal: 3,
                initial_selection_ordinal: 0,
                rebound_selection_ordinal: 1,
            }],
            indirect_dispatches: vec![dispatch],
            ..Default::default()
        }
    }

    fn dispatch(
        owner: MachineId,
        tuple: &str,
        realization: MachineId,
    ) -> TerminalIndirectDynamicDispatch {
        let width = realization.get();
        TerminalIndirectDynamicDispatch {
            owner,
            operation: OperationId::new(42).unwrap(),
            descriptor_ordinal: 3,
            declaring_trait_identity: "test::Scanner".to_owned(),
            public_requirement_identity: "test::Scanner::scan()".to_owned(),
            family_tuple: vec![tuple.to_owned()],
            requirement_identity: "test::Scanner::scan".to_owned(),
            realization_identity: format!("test::Carrier::scan<{width}>"),
            realization_callable_identity: format!("test::Carrier::scan<{width}>::callable"),
            realization,
        }
    }

    #[test]
    fn rebound_dispatch_rejoins_the_exact_family_tuple_row() {
        let machine = machine();
        let operation = Operation {
            static_reach_binding: None,
            id: OperationId::new(42).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::CallDynamicUnit {
                descriptor_ordinal: 3,
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        };
        let application = widths_application(machine.id);
        let width_32 = MachineId::new(92).unwrap();
        let width_64 = MachineId::new(93).unwrap();

        let catalog = catalog_for(
            machine.id,
            &application,
            dispatch(machine.id, WIDTH_32, width_32),
        );
        let lowered = lower_rebound_dynamic_dispatch(
            &machine,
            &operation,
            3,
            None,
            &catalog,
            std::slice::from_ref(&application),
        )
        .expect("the tuple-32 dispatch rejoins the tuple-32 family row");
        assert_eq!(lowered.dispatch.realization, width_32);

        let mismatched = catalog_for(
            machine.id,
            &application,
            dispatch(machine.id, WIDTH_16, width_32),
        );
        assert!(
            lower_rebound_dynamic_dispatch(
                &machine,
                &operation,
                3,
                None,
                &mismatched,
                std::slice::from_ref(&application),
            )
            .is_err(),
            "the width-16 tuple cannot borrow the width-32 realization's row",
        );

        let absent = catalog_for(
            machine.id,
            &application,
            dispatch(machine.id, WIDTH_64, width_64),
        );
        assert!(
            lower_rebound_dynamic_dispatch(
                &machine,
                &operation,
                3,
                None,
                &absent,
                std::slice::from_ref(&application),
            )
            .is_err(),
            "a tuple absent from the roster rejoins no row",
        );
    }
}
