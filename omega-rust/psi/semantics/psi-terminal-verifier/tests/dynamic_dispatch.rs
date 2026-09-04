use psi_core::{
    BlockId, ClaimId, ContractId, EdgeId, MachineId, ObligationId, OperationId, PlaceId,
    PsiSemanticId, ScalarType, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use psi_terminal::{
    BindingRelevance, Block, ClaimTransfer, ClosedConformanceApplication,
    ClosedConformanceCallableResult, ClosedConformanceParameterBinding,
    ClosedConformanceRealizationCallable, ClosedConformanceRow, CrashCause, CrashRouteBucket,
    CrashRouteGuard, MachineContract, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralArgument, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalDirectDynamicDispatch,
    TerminalDynamicConformanceSelection, TerminalDynamicDescriptorArgument,
    TerminalDynamicDescriptorParameter, TerminalDynamicDescriptorSource,
    TerminalDynamicDispatchCatalog, TerminalDynamicRequirement, TerminalIndirectDynamicDispatch,
    TerminalMachine, TerminalMachineResult, TerminalModule, TerminalParameterDynamicDispatch,
    TerminalReboundDynamicDescriptor, Terminator, ValueDeclaration, VocabularyMarker,
    closed_conformance_application_commitment, closed_conformance_application_report_fingerprint,
};
use psi_terminal_verifier::{ModuleError, validate_module};

const CARRIER_IDENTITY: &str =
    "package:0101010101010101010101010101010101010101010101010101010101010101::Carrier";
const OTHER_CARRIER_IDENTITY: &str =
    "package:0202020202020202020202020202020202020202020202020202020202020202::OtherCarrier";
const OWNER_IDENTITY: &str =
    "package:0303030303030303030303030303030303030303030303030303030303030303::Owner";

fn id<Identity: PsiSemanticId>(raw: u64) -> Identity {
    Identity::new(raw).expect("test identity is nonzero")
}

fn empty_contract(raw: u64) -> MachineContract {
    MachineContract {
        id: id::<ContractId>(raw),
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    }
}

fn parameter(
    place: u64,
    structural_type: u64,
    multiplicity: StructuralMultiplicity,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: id::<PlaceId>(place),
        position: 0,
        is_self: false,
        structural_type: id::<StructuralTypeId>(structural_type),
        multiplicity,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn parameter_place(place: u64) -> StructuralPlaceDeclaration {
    StructuralPlaceDeclaration {
        id: id::<PlaceId>(place),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }
}

fn closed_application(owner: MachineId, realization: MachineId) -> ClosedConformanceApplication {
    let mut application = ClosedConformanceApplication {
        owner,
        declaration_identity: "package::CarrierImplementsMeasure".into(),
        telescope: Vec::<ClosedConformanceParameterBinding>::new(),
        subject_identity: Some(CARRIER_IDENTITY.into()),
        trait_identity: "package::Measure".into(),
        trait_lifetime_arguments: Vec::new(),
        trait_arguments: Vec::new(),
        realization_callables: vec![ClosedConformanceRealizationCallable {
            source_callable_identity: "package::Carrier::measure#callable".into(),
            machine: realization,
            result: ClosedConformanceCallableResult::Bool,
        }],
        rows: vec![ClosedConformanceRow {
            declaring_trait_identity: "package::Measure".into(),
            public_requirement_identity: "package::Measure::measure()".into(),
            requirement_identity: "package::Measure::measure".into(),
            realization_identity: "package::Carrier::measure".into(),
            realization_callable_identity: Some("package::Carrier::measure#callable".into()),
        }],
        report_fingerprint: 0,
        commitment: Default::default(),
    };
    refresh_application_identity(&mut application);
    application
}

fn refresh_application_identity(application: &mut ClosedConformanceApplication) {
    application.report_fingerprint = closed_conformance_application_report_fingerprint(application);
    application.commitment = closed_conformance_application_commitment(application);
}

fn dynamic_dispatch_module() -> TerminalModule {
    let caller = id::<MachineId>(1);
    let realization = id::<MachineId>(2);
    let operation = id::<OperationId>(1);
    let source = StructuralArgument {
        place: id::<PlaceId>(1),
        path: vec![StructuralPathSegment::Field("carrier".into())],
        access: StructuralAccess::SharedBorrow,
    };
    let application = closed_application(caller, realization);
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: caller,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: id::<StructuralTypeId>(1),
                identity: OWNER_IDENTITY.into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: id(1),
                        identity: "carrier".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(id::<StructuralTypeId>(2)),
                    }],
                },
            },
            StructuralTypeDeclaration {
                id: id::<StructuralTypeId>(2),
                identity: CARRIER_IDENTITY.into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
            StructuralTypeDeclaration {
                id: id::<StructuralTypeId>(3),
                identity: OTHER_CARRIER_IDENTITY.into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
        ],
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: vec![application.clone()],
        dynamic_dispatch: TerminalDynamicDispatchCatalog {
            parameters: Vec::new(),
            arguments: Vec::new(),
            selections: vec![TerminalDynamicConformanceSelection {
                owner: caller,
                ordinal: 0,
                source: source.clone(),
                conformance_application_report_fingerprint: application.report_fingerprint,
                conformance_application_commitment: application.commitment,
            }],
            rebound_descriptors: Vec::new(),
            stored_descriptors: Vec::new(),
            direct_dispatches: vec![TerminalDirectDynamicDispatch {
                owner: caller,
                operation,
                selection_ordinal: 0,
                declaring_trait_identity: "package::Measure".into(),
                public_requirement_identity: "package::Measure::measure()".into(),
                requirement_identity: "package::Measure::measure".into(),
                realization_identity: "package::Carrier::measure".into(),
                realization_callable_identity: "package::Carrier::measure#callable".into(),
                realization,
            }],
            indirect_dispatches: Vec::new(),
            stored_dispatches: Vec::new(),
            parameter_dispatches: Vec::new(),
        },
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![
            TerminalMachine {
                id: caller,
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: vec![parameter(1, 1, StructuralMultiplicity::Unrestricted)],
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: vec![parameter_place(1)],
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: id::<BlockId>(1),
                blocks: vec![Block {
                    id: id::<BlockId>(1),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: operation,
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: id::<ValueId>(1),
                            scalar_type: ScalarType::Boolean,
                        }),
                        kind: OperationKind::CallStructuralScalar {
                            callee: realization,
                            arguments: Vec::new(),
                            structural_arguments: vec![source],
                            claim_transfers: Vec::new(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                        },
                    }],
                    terminator: Terminator::ReturnUnit {
                        edge: id::<EdgeId>(1),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: empty_contract(1),
            },
            TerminalMachine {
                id: realization,
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: vec![parameter(2, 2, StructuralMultiplicity::Unrestricted)],
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(ValueDeclaration {
                    id: id::<ValueId>(2),
                    scalar_type: ScalarType::Boolean,
                }),
                structural_places: vec![parameter_place(2)],
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: id::<BlockId>(2),
                blocks: vec![Block {
                    id: id::<BlockId>(2),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: id::<OperationId>(2),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: id::<ValueId>(3),
                            scalar_type: ScalarType::Boolean,
                        }),
                        kind: OperationKind::BooleanConstant { value: true },
                    }],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: id::<EdgeId>(2),
                        value: id::<ValueId>(3),
                    },
                }],
                contract: empty_contract(2),
            },
        ],
    }
}

fn rebound_dynamic_dispatch_module() -> TerminalModule {
    let mut module = dynamic_dispatch_module();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        panic!("owner is a record")
    };
    fields.push(StructuralFieldDeclaration {
        id: id(2),
        identity: "selected".into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(id::<StructuralTypeId>(2)),
    });
    let mut latest = module.dynamic_dispatch.selections[0].clone();
    latest.ordinal = 1;
    latest.source.path = vec![StructuralPathSegment::Field("selected".into())];
    module.dynamic_dispatch.selections.push(latest);
    module.dynamic_dispatch.rebound_descriptors = vec![TerminalReboundDynamicDescriptor {
        owner: id::<MachineId>(1),
        ordinal: 0,
        initial_selection_ordinal: 0,
        rebound_selection_ordinal: 1,
    }];
    let direct = module.dynamic_dispatch.direct_dispatches.remove(0);
    module.dynamic_dispatch.indirect_dispatches = vec![TerminalIndirectDynamicDispatch {
        owner: direct.owner,
        operation: direct.operation,
        descriptor_ordinal: 0,
        declaring_trait_identity: direct.declaring_trait_identity,
        public_requirement_identity: direct.public_requirement_identity,
        requirement_identity: direct.requirement_identity,
        realization_identity: direct.realization_identity,
        realization_callable_identity: direct.realization_callable_identity,
        realization: direct.realization,
    }];
    module.machines[0].blocks[0].operations[0].kind = OperationKind::CallDynamicScalar {
        descriptor_ordinal: 0,
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    module
}

fn changed_conformance_rebound_dynamic_dispatch_module() -> TerminalModule {
    let mut module = rebound_dynamic_dispatch_module();
    let latest = module.closed_conformance_applications[0].clone();
    let mut initial = latest.clone();
    initial.declaration_identity = "package::CarrierImplementsPrimaryMeasure".into();
    initial.realization_callables.clear();
    for row in &mut initial.rows {
        row.realization_callable_identity = None;
    }
    refresh_application_identity(&mut initial);
    module.dynamic_dispatch.selections[0].conformance_application_report_fingerprint =
        initial.report_fingerprint;
    module.dynamic_dispatch.selections[0].conformance_application_commitment = initial.commitment;
    module.closed_conformance_applications.push(initial);
    module
        .closed_conformance_applications
        .sort_by(|left, right| {
            (
                left.owner,
                &left.declaration_identity,
                left.report_fingerprint,
            )
                .cmp(&(
                    right.owner,
                    &right.declaration_identity,
                    right.report_fingerprint,
                ))
        });
    module
}

fn parameter_dynamic_dispatch_module() -> TerminalModule {
    let mut module = rebound_dynamic_dispatch_module();
    let caller = id::<MachineId>(1);
    let helper = id::<MachineId>(3);
    let caller_operation = id::<OperationId>(1);
    let helper_operation = id::<OperationId>(3);
    module.dynamic_dispatch.indirect_dispatches.clear();
    module.dynamic_dispatch.parameters = vec![TerminalDynamicDescriptorParameter {
        owner: helper,
        ordinal: 0,
        source_position: 0,
        trait_identity: "package::Measure".into(),
        access: StructuralAccess::SharedBorrow,
        requirements: vec![TerminalDynamicRequirement {
            slot: 0,
            declaring_trait_identity: "package::Measure".into(),
            public_requirement_identity: "package::Measure::measure()".into(),
            result: ClosedConformanceCallableResult::Bool,
        }],
    }];
    module.dynamic_dispatch.arguments = vec![TerminalDynamicDescriptorArgument {
        owner: caller,
        operation: caller_operation,
        parameter_ordinal: 0,
        source: TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal: 0 },
    }];
    module.dynamic_dispatch.parameter_dispatches = vec![TerminalParameterDynamicDispatch {
        owner: helper,
        operation: helper_operation,
        parameter_ordinal: 0,
        requirement_slot: 0,
    }];
    module.machines[0].blocks[0].operations[0].kind = OperationKind::CallStructuralScalar {
        callee: helper,
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    module.machines.push(TerminalMachine {
        id: helper,
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            id: id::<ValueId>(4),
            scalar_type: ScalarType::Boolean,
        }),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: id::<BlockId>(3),
        blocks: vec![Block {
            id: id::<BlockId>(3),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: helper_operation,
                result: OperationResult::Scalar(ValueDeclaration {
                    id: id::<ValueId>(5),
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::CallDynamicParameterScalar {
                    parameter_ordinal: 0,
                    requirement_slot: 0,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: id::<EdgeId>(3),
                value: id::<ValueId>(5),
            },
        }],
        contract: empty_contract(3),
    });
    module
}

fn direct_call_mut(module: &mut TerminalModule) -> &mut OperationKind {
    &mut module.machines[0].blocks[0].operations[0].kind
}

fn validation_error(module: &TerminalModule) -> ModuleError {
    validate_module(module).expect_err("corrupt dynamic dispatch must fail closed")
}

#[test]
fn admits_exact_source_free_direct_dynamic_dispatch() {
    validate_module(&dynamic_dispatch_module()).expect("exact direct dynamic dispatch is valid");
}

#[test]
fn admits_exact_rebound_descriptor_and_indirect_dispatch() {
    validate_module(&rebound_dynamic_dispatch_module())
        .expect("exact rebound descriptor and indirect call are valid");
}

#[test]
fn admits_changed_conformance_rebound_and_rejects_interface_substitution() {
    let module = changed_conformance_rebound_dynamic_dispatch_module();
    validate_module(&module).expect("distinct conformances for one exact interface are valid");

    let mut changed_trait = module.clone();
    let initial = changed_trait
        .closed_conformance_applications
        .iter_mut()
        .find(|application| {
            application.declaration_identity == "package::CarrierImplementsPrimaryMeasure"
        })
        .expect("initial changed conformance application");
    initial.trait_identity = "package::DifferentTrait".into();
    refresh_application_identity(initial);
    changed_trait.dynamic_dispatch.selections[0].conformance_application_report_fingerprint =
        initial.report_fingerprint;
    changed_trait.dynamic_dispatch.selections[0].conformance_application_commitment =
        initial.commitment;
    changed_trait
        .closed_conformance_applications
        .sort_by(|left, right| {
            (
                left.owner,
                &left.declaration_identity,
                left.report_fingerprint,
            )
                .cmp(&(
                    right.owner,
                    &right.declaration_identity,
                    right.report_fingerprint,
                ))
        });
    assert_eq!(
        validation_error(&changed_trait),
        ModuleError::InvalidReboundDynamicDescriptor {
            owner: id::<MachineId>(1),
            ordinal: 0,
        },
        "a rebound may change conformance, but not its existential interface"
    );
}

#[test]
fn admits_exact_dynamic_descriptor_parameter_argument_and_dispatch() {
    validate_module(&parameter_dynamic_dispatch_module())
        .expect("exact dynamic descriptor parameter flow is valid");
}

#[test]
fn admits_exact_mutable_descriptor_flow_and_rejects_access_substitution() {
    let mut module = parameter_dynamic_dispatch_module();
    module.machines[0].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    for selection in &mut module.dynamic_dispatch.selections {
        selection.source.access = StructuralAccess::MutableBorrow;
    }
    module.dynamic_dispatch.parameters[0].access = StructuralAccess::MutableBorrow;
    validate_module(&module).expect("exact mutable descriptor flow is valid");

    module.dynamic_dispatch.selections[1].source.access = StructuralAccess::SharedBorrow;
    assert!(
        validate_module(&module).is_err(),
        "a mutable descriptor cannot silently substitute a shared rebound source"
    );
}

#[test]
fn rejects_missing_or_substituted_dynamic_descriptor_parameter_custody() {
    let mut missing_argument = parameter_dynamic_dispatch_module();
    missing_argument.dynamic_dispatch.arguments.clear();
    assert_eq!(
        validation_error(&missing_argument),
        ModuleError::InvalidClosedConformanceApplication {
            owner: id::<MachineId>(1),
            declaration: "package::CarrierImplementsMeasure".into(),
        }
    );

    let mut substituted_interface = parameter_dynamic_dispatch_module();
    substituted_interface.dynamic_dispatch.parameters[0].trait_identity = "package::Other".into();
    assert_eq!(
        validation_error(&substituted_interface),
        ModuleError::InvalidDynamicDescriptorArgument {
            owner: id::<MachineId>(1),
            operation: id::<OperationId>(1),
            parameter_ordinal: 0,
        }
    );

    let mut substituted_slot = parameter_dynamic_dispatch_module();
    let OperationKind::CallDynamicParameterScalar {
        requirement_slot, ..
    } = &mut substituted_slot.machines[2].blocks[0].operations[0].kind
    else {
        panic!("parameter dispatch operation expected")
    };
    *requirement_slot = 1;
    assert_eq!(
        validation_error(&substituted_slot),
        ModuleError::InvalidParameterDynamicDispatch {
            owner: id::<MachineId>(3),
            operation: id::<OperationId>(3),
        }
    );
}

#[test]
fn rejects_rebound_version_source_and_dispatch_drift() {
    let expected_descriptor = ModuleError::InvalidReboundDynamicDescriptor {
        owner: id::<MachineId>(1),
        ordinal: 0,
    };

    let mut reversed = rebound_dynamic_dispatch_module();
    reversed.dynamic_dispatch.rebound_descriptors[0].initial_selection_ordinal = 1;
    reversed.dynamic_dispatch.rebound_descriptors[0].rebound_selection_ordinal = 0;
    assert_eq!(validation_error(&reversed), expected_descriptor.clone());

    let mut changed_source = rebound_dynamic_dispatch_module();
    let StructuralTypeShape::Record { fields } = &mut changed_source.structural_types[0].shape
    else {
        panic!("owner is a record")
    };
    fields.push(StructuralFieldDeclaration {
        id: id(3),
        identity: "other".into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(id::<StructuralTypeId>(3)),
    });
    changed_source.dynamic_dispatch.selections[1].source.path =
        vec![StructuralPathSegment::Field("other".into())];
    assert_eq!(validation_error(&changed_source), expected_descriptor);

    let expected_dispatch = ModuleError::InvalidIndirectDynamicDispatch {
        owner: id::<MachineId>(1),
        operation: id::<OperationId>(1),
    };
    let mut stale_operation = rebound_dynamic_dispatch_module();
    let OperationKind::CallDynamicScalar {
        descriptor_ordinal, ..
    } = &mut stale_operation.machines[0].blocks[0].operations[0].kind
    else {
        panic!("fixture indirect call")
    };
    *descriptor_ordinal = 1;
    assert_eq!(
        validation_error(&stale_operation),
        expected_dispatch.clone()
    );

    let mut row_drift = rebound_dynamic_dispatch_module();
    row_drift.dynamic_dispatch.indirect_dispatches[0].requirement_identity =
        "package::Measure::different".into();
    assert!(
        validate_module(&row_drift).is_err(),
        "an indirect row outside the closed application must reject"
    );
}

#[test]
fn rejects_duplicate_and_orphan_rebound_rows() {
    let mut duplicate_descriptor = rebound_dynamic_dispatch_module();
    duplicate_descriptor
        .dynamic_dispatch
        .rebound_descriptors
        .push(duplicate_descriptor.dynamic_dispatch.rebound_descriptors[0].clone());
    assert_eq!(
        validation_error(&duplicate_descriptor),
        ModuleError::DuplicateReboundDynamicDescriptor {
            owner: id::<MachineId>(1),
            ordinal: 0,
        }
    );

    let mut duplicate_dispatch = rebound_dynamic_dispatch_module();
    duplicate_dispatch
        .dynamic_dispatch
        .indirect_dispatches
        .push(duplicate_dispatch.dynamic_dispatch.indirect_dispatches[0].clone());
    assert_eq!(
        validation_error(&duplicate_dispatch),
        ModuleError::DuplicateIndirectDynamicDispatch {
            owner: id::<MachineId>(1),
            operation: id::<OperationId>(1),
        }
    );

    let mut orphan_descriptor = rebound_dynamic_dispatch_module();
    orphan_descriptor
        .dynamic_dispatch
        .indirect_dispatches
        .clear();
    orphan_descriptor.machines[0].blocks[0].operations.clear();
    assert!(
        validate_module(&orphan_descriptor).is_err(),
        "an unconsumed rebound descriptor must reject"
    );
}

#[test]
fn admits_an_unrestricted_shared_boolean_field_realization() {
    let mut module = dynamic_dispatch_module();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
        panic!("carrier is a record")
    };
    fields.push(StructuralFieldDeclaration {
        id: id::<psi_core::StructuralFieldId>(2),
        identity: "ready".into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
    });
    module.machines[1].blocks[0].operations[0].kind = OperationKind::BooleanStructuralField {
        source: id::<PlaceId>(2),
        field: id::<psi_core::StructuralFieldId>(2),
    };

    validate_module(&module).expect("an exact shared unrestricted field read is valid");
}

#[test]
fn rejects_a_write_only_boolean_field_realization() {
    let mut module = dynamic_dispatch_module();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
        panic!("carrier is a record")
    };
    fields.push(StructuralFieldDeclaration {
        id: id::<psi_core::StructuralFieldId>(2),
        identity: "ready".into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
    });
    module.machines[0].structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
    module.machines[1].structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
    module.dynamic_dispatch.selections[0].source.access = StructuralAccess::WriteOnlyBorrow;
    let OperationKind::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        panic!("caller contains the direct structural call")
    };
    structural_arguments[0].access = StructuralAccess::WriteOnlyBorrow;
    module.machines[1].blocks[0].operations[0].kind = OperationKind::BooleanStructuralField {
        source: id::<PlaceId>(2),
        field: id::<psi_core::StructuralFieldId>(2),
    };

    assert_eq!(
        validation_error(&module),
        ModuleError::StructuralObservationRequiresReadableAccess {
            operation: id::<OperationId>(2),
            source: id::<PlaceId>(2),
        }
    );
}

#[test]
fn rejects_conformance_subject_for_another_source_carrier() {
    let mut module = dynamic_dispatch_module();
    module.closed_conformance_applications[0].subject_identity =
        Some(OTHER_CARRIER_IDENTITY.into());
    refresh_application_identity(&mut module.closed_conformance_applications[0]);
    let application = &module.closed_conformance_applications[0];
    module.dynamic_dispatch.selections[0].conformance_application_report_fingerprint =
        application.report_fingerprint;
    module.dynamic_dispatch.selections[0].conformance_application_commitment =
        application.commitment;

    assert_eq!(
        validation_error(&module),
        ModuleError::InvalidDirectDynamicDispatch {
            owner: id::<MachineId>(1),
            operation: id::<OperationId>(1),
        }
    );
}

#[test]
fn rejects_nonempty_direct_dynamic_call_lanes() {
    let expected = ModuleError::InvalidDirectDynamicDispatch {
        owner: id::<MachineId>(1),
        operation: id::<OperationId>(1),
    };

    let mut claims = dynamic_dispatch_module();
    let OperationKind::CallStructuralScalar {
        claim_transfers, ..
    } = direct_call_mut(&mut claims)
    else {
        panic!("fixture direct call");
    };
    claim_transfers.push(ClaimTransfer {
        claim: id::<ClaimId>(1),
        argument_index: 0,
    });
    assert_eq!(validation_error(&claims), expected.clone());

    let mut requirements = dynamic_dispatch_module();
    let OperationKind::CallStructuralScalar {
        requirement_obligations,
        ..
    } = direct_call_mut(&mut requirements)
    else {
        panic!("fixture direct call");
    };
    requirement_obligations.push(id::<ObligationId>(1));
    assert_eq!(validation_error(&requirements), expected.clone());

    let mut crashes = dynamic_dispatch_module();
    let OperationKind::CallStructuralScalar {
        crash_continuations,
        ..
    } = direct_call_mut(&mut crashes)
    else {
        panic!("fixture direct call");
    };
    crash_continuations.push(CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    });
    assert_eq!(validation_error(&crashes), expected);
}

#[test]
fn rejects_duplicate_and_orphan_dynamic_rows() {
    let mut duplicate_selection = dynamic_dispatch_module();
    duplicate_selection
        .dynamic_dispatch
        .selections
        .push(duplicate_selection.dynamic_dispatch.selections[0].clone());
    assert_eq!(
        validation_error(&duplicate_selection),
        ModuleError::DuplicateDynamicConformanceSelection {
            owner: id::<MachineId>(1),
            ordinal: 0,
        }
    );

    let mut duplicate_dispatch = dynamic_dispatch_module();
    duplicate_dispatch
        .dynamic_dispatch
        .direct_dispatches
        .push(duplicate_dispatch.dynamic_dispatch.direct_dispatches[0].clone());
    assert_eq!(
        validation_error(&duplicate_dispatch),
        ModuleError::DuplicateDirectDynamicDispatch {
            owner: id::<MachineId>(1),
            operation: id::<OperationId>(1),
        }
    );

    let mut orphan_selection = dynamic_dispatch_module();
    let mut orphan = orphan_selection.dynamic_dispatch.selections[0].clone();
    orphan.ordinal = 1;
    orphan_selection.dynamic_dispatch.selections.push(orphan);
    assert_eq!(
        validation_error(&orphan_selection),
        ModuleError::OrphanDynamicConformanceSelection {
            owner: id::<MachineId>(1),
            ordinal: 1,
        }
    );
}

#[test]
fn rejects_broken_application_row_callable_and_operation_joins() {
    let mut broken_application = dynamic_dispatch_module();
    broken_application.dynamic_dispatch.selections[0].conformance_application_commitment =
        Default::default();
    assert!(validate_module(&broken_application).is_err());

    let mut broken_row = dynamic_dispatch_module();
    broken_row.dynamic_dispatch.direct_dispatches[0].requirement_identity =
        "package::Measure::different".into();
    assert!(validate_module(&broken_row).is_err());

    let mut broken_callable = dynamic_dispatch_module();
    broken_callable.dynamic_dispatch.direct_dispatches[0].realization = id::<MachineId>(1);
    assert!(validate_module(&broken_callable).is_err());

    let mut broken_operation = dynamic_dispatch_module();
    broken_operation.machines[0].blocks[0].operations[0].kind =
        OperationKind::BooleanConstant { value: true };
    assert!(validate_module(&broken_operation).is_err());
}
