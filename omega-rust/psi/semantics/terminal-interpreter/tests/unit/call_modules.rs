//! Write-only, structural scalar field, dynamic scalar and internal
//! structural call module fixtures.

use super::effect_modules::unit_module;
use super::{
    block_id, claim_id, contract_id, edge_id, empty_contract, machine_id, operation_id, place_id,
    structural_domain_id, structural_field_id, structural_type_id, value_id,
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use terminal_psi::{
    BindingRelevance, Block, ClaimTransfer, ClosedConformanceApplication,
    ClosedConformanceCallableResult, ClosedConformanceRealizationCallable, ClosedConformanceRow,
    CrashCause, CrashRouteBucket, CrashRouteGuard, EntryClaim, MachineContract, Operation,
    OperationKind, OperationResult, StructuralAccess, StructuralArgument,
    StructuralDomainDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralResultClaimBinding,
    StructuralResultClaimTransfer, StructuralResultDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, SuccessorEdge, TerminalDynamicConformanceSelection,
    TerminalDynamicDescriptorArgument, TerminalDynamicDescriptorParameter,
    TerminalDynamicDescriptorSource, TerminalDynamicDispatchCatalog, TerminalDynamicRequirement,
    TerminalIndirectDynamicDispatch, TerminalMachine, TerminalMachineResult, TerminalModule,
    TerminalParameterDynamicDispatch, TerminalReboundDynamicDescriptor, Terminator,
    ValueDeclaration, VocabularyMarker, closed_conformance_application_commitment,
    closed_conformance_application_report_fingerprint,
};

pub(super) fn write_only_primitive_call_module() -> TerminalModule {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let structural_type = structural_type_id(91);
    let caller_place = place_id(91);
    let callee_place = place_id(92);
    let parameter = |place, position| StructuralParameterDeclaration {
        place,
        position,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::WriteOnlyBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let place = |id, position| StructuralPlaceDeclaration {
        id,
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position,
            is_self: false,
        },
    };
    let mut module = unit_module();
    module.structural_types = vec![StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::WriteOnlyU8".into(),
        shape: StructuralTypeShape::PrimitiveScalar(scalar_type),
    }];
    let caller = &mut module.machines[0];
    caller.structural_parameters = vec![parameter(caller_place, 0)];
    caller.structural_places = vec![place(caller_place, 0)];
    caller.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        id: operation_id(91),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            erased_arguments: Vec::new(),
            arguments: Vec::new(),
            callee: machine_id(92),
            structural_arguments: vec![StructuralArgument {
                place: caller_place,
                path: Vec::new(),
                access: StructuralAccess::WriteOnlyBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }];

    module.machines.push(TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(92),
        attachment: None,
        structural_parameters: vec![parameter(callee_place, 0)],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![place(callee_place, 0)],
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(92),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(92),
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    static_reach_binding: None,
                    id: operation_id(92),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: value_id(92),
                        scalar_type,
                    }),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(7),
                    },
                },
                Operation {
                    static_reach_binding: None,
                    id: operation_id(93),
                    result: OperationResult::Unit,
                    kind: OperationKind::WriteOnlyPrimitiveStore {
                        path: Vec::new(),
                        destination: callee_place,
                        value: value_id(92),
                    },
                },
            ],
            terminator: Terminator::ReturnUnit {
                edge: edge_id(92),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(contract_id(92)),
    });
    module
}

pub(super) fn write_only_boolean_call_module() -> TerminalModule {
    let mut module = write_only_primitive_call_module();
    module.structural_types[0].identity = "test::WriteOnlyBool".into();
    module.structural_types[0].shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean);
    let constant = &mut module.machines[1].blocks[0].operations[0];
    constant.result = OperationResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(92),
        scalar_type: ScalarType::Boolean,
    });
    constant.kind = OperationKind::BooleanConstant { value: true };
    module
}

pub(super) fn structural_scalar_field_call_module() -> TerminalModule {
    let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let owner_type = structural_type_id(95);
    let item_type = structural_type_id(96);
    let caller_place = place_id(95);
    let callee_place = place_id(96);
    let parameter = |place, structural_type, access| StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: true,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let place = |id| StructuralPlaceDeclaration {
        id,
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: true,
        },
    };
    let mut module = unit_module();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: owner_type,
            identity: "test::Owner".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: structural_field_id(1),
                    identity: "item".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(item_type),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: item_type,
            identity: "test::Item".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: structural_field_id(1),
                    identity: "value".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(integer),
                }],
            },
        },
    ];
    let caller = &mut module.machines[0];
    caller.attachment = Some(owner_type);
    caller.structural_parameters = vec![parameter(
        caller_place,
        owner_type,
        StructuralAccess::MutableBorrow,
    )];
    caller.structural_places = vec![place(caller_place)];
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(3),
        scalar_type: integer,
    });
    caller.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            id: operation_id(1),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(1),
                scalar_type: integer,
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Signed(99),
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(2),
            result: OperationResult::Unit,
            kind: OperationKind::StructuralScalarFieldStore {
                destination: caller_place,
                path: vec![StructuralPathSegment::Field("item".into())],
                field: structural_field_id(1),
                value: value_id(1),
                range_obligation: None,
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(3),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(2),
                scalar_type: integer,
            }),
            kind: OperationKind::CallStructuralScalar {
                erased_arguments: Vec::new(),
                callee: machine_id(96),
                arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place: caller_place,
                    path: vec![StructuralPathSegment::Field("item".into())],
                    access: StructuralAccess::SharedBorrow,
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        },
    ];
    caller.blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(2),
        cleanup_actions: Vec::new(),
    };

    module.machines.push(TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(96),
        attachment: Some(item_type),
        parameters: Vec::new(),
        structural_parameters: vec![parameter(
            callee_place,
            item_type,
            StructuralAccess::SharedBorrow,
        )],
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(5),
            scalar_type: integer,
        }),
        structural_places: vec![place(callee_place)],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(96),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(96),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: operation_id(4),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(4),
                    scalar_type: integer,
                }),
                kind: OperationKind::IntegerStructuralField {
                    path: Vec::new(),
                    source: callee_place,
                    field: structural_field_id(1),
                },
            }],
            terminator: Terminator::Return {
                edge: edge_id(96),
                value: value_id(4),
                cleanup_actions: Vec::new(),
            },
        }],
        contract: empty_contract(contract_id(96)),
    });
    module
}

pub(super) fn rebound_dynamic_scalar_call_module() -> TerminalModule {
    let mut module = structural_scalar_field_call_module();
    let caller = machine_id(1);
    let realization = machine_id(96);
    let operation = operation_id(3);
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        panic!("owner is a record")
    };
    fields.push(StructuralFieldDeclaration {
        id: structural_field_id(2),
        identity: "selected".into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(structural_type_id(96)),
    });
    module.machines[0].structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    module.machines[0].blocks[0]
        .operations
        .retain(|operation| operation.id == operation_id(3));
    module.machines[1].structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    module.machines[1].blocks[0].operations[0].kind = OperationKind::IntegerConstant {
        value: IntegerValue::Signed(99),
    };

    let mut application = ClosedConformanceApplication {
        owner: caller,
        declaration_identity: "test::ItemSatisfiesMeasure".into(),
        telescope: Vec::new(),
        subject_identity: Some("test::Item".into()),
        trait_identity: "test::Measure".into(),
        trait_lifetime_arguments: Vec::new(),
        trait_arguments: Vec::new(),
        realization_callables: vec![ClosedConformanceRealizationCallable {
            source_callable_identity: "test::Item::measure#callable".into(),
            machine: realization,
            result: ClosedConformanceCallableResult::I32,
        }],
        rows: vec![ClosedConformanceRow {
            declaring_trait_identity: "test::Measure".into(),
            public_requirement_identity: "test::Measure::measure()".into(),
            family_tuple: Vec::new(),
            requirement_identity: "test::Measure::measure".into(),
            realization_identity: "test::Item::measure".into(),
            realization_callable_identity: Some("test::Item::measure#callable".into()),
        }],
        report_fingerprint: 0,
        commitment: Default::default(),
    };
    application.report_fingerprint =
        closed_conformance_application_report_fingerprint(&application);
    application.commitment = closed_conformance_application_commitment(&application);
    let application_report_fingerprint = application.report_fingerprint;
    let application_commitment = application.commitment;
    let selection = |ordinal, field: &str| TerminalDynamicConformanceSelection {
        owner: caller,
        ordinal,
        source: StructuralArgument {
            place: place_id(95),
            path: vec![StructuralPathSegment::Field(field.into())],
            access: StructuralAccess::SharedBorrow,
        },
        conformance_application_report_fingerprint: application_report_fingerprint,
        conformance_application_commitment: application_commitment,
    };
    module.closed_conformance_applications = vec![application];
    module.dynamic_dispatch = TerminalDynamicDispatchCatalog {
        parameters: Vec::new(),
        arguments: Vec::new(),
        selections: vec![selection(0, "item"), selection(1, "selected")],
        rebound_descriptors: vec![TerminalReboundDynamicDescriptor {
            owner: caller,
            ordinal: 0,
            initial_selection_ordinal: 0,
            rebound_selection_ordinal: 1,
        }],
        stored_descriptors: Vec::new(),
        direct_dispatches: Vec::new(),
        indirect_dispatches: vec![TerminalIndirectDynamicDispatch {
            owner: caller,
            operation,
            descriptor_ordinal: 0,
            declaring_trait_identity: "test::Measure".into(),
            public_requirement_identity: "test::Measure::measure()".into(),
            family_tuple: Vec::new(),
            requirement_identity: "test::Measure::measure".into(),
            realization_identity: "test::Item::measure".into(),
            realization_callable_identity: "test::Item::measure#callable".into(),
            realization,
        }],
        stored_dispatches: Vec::new(),
        parameter_dispatches: Vec::new(),
    };
    module.machines[0].blocks[0].operations[0].kind = OperationKind::CallDynamicScalar {
        descriptor_ordinal: 0,
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    module
}

pub(super) fn parameter_dynamic_scalar_call_module() -> TerminalModule {
    let mut module = rebound_dynamic_scalar_call_module();
    let caller = machine_id(1);
    let helper = machine_id(97);
    let caller_operation = operation_id(3);
    let helper_operation = operation_id(5);
    module.dynamic_dispatch.indirect_dispatches.clear();
    module.dynamic_dispatch.parameters = vec![TerminalDynamicDescriptorParameter {
        owner: helper,
        ordinal: 0,
        source_position: 0,
        trait_identity: "test::Measure".into(),
        access: StructuralAccess::SharedBorrow,
        requirements: vec![TerminalDynamicRequirement {
            slot: 0,
            declaring_trait_identity: "test::Measure".into(),
            public_requirement_identity: "test::Measure::measure()".into(),
            family_tuple: Vec::new(),
            result: ClosedConformanceCallableResult::I32,
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
        erased_arguments: Vec::new(),
        callee: helper,
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    module.machines.push(TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: helper,
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(6),
            scalar_type: integer,
        }),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(97),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(97),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: helper_operation,
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(7),
                    scalar_type: integer,
                }),
                kind: OperationKind::CallDynamicParameterScalar {
                    parameter_ordinal: 0,
                    requirement_slot: 0,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: Terminator::Return {
                edge: edge_id(97),
                value: value_id(7),
                cleanup_actions: Vec::new(),
            },
        }],
        contract: empty_contract(contract_id(97)),
    });
    module
}

/// One runtime descriptor parameter with two syntactic call predecessors.
/// Each predecessor supplies a distinct source place and closed conformance;
/// the callee consumes only the shared existential interface.
pub(super) fn joined_parameter_dynamic_scalar_call_module() -> TerminalModule {
    let mut module = parameter_dynamic_scalar_call_module();
    let caller = machine_id(1);
    let first_realization = machine_id(96);
    let second_realization = machine_id(98);
    let helper = machine_id(97);
    let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());

    let first_application = module.closed_conformance_applications[0].clone();
    let mut second_application = first_application.clone();
    second_application.declaration_identity = "test::ItemSatisfiesAlternateMeasure".into();
    second_application.realization_callables[0].source_callable_identity =
        "test::Item::alternate_measure#callable".into();
    second_application.realization_callables[0].machine = second_realization;
    second_application.rows[0].realization_identity = "test::Item::alternate_measure".into();
    second_application.rows[0].realization_callable_identity =
        Some("test::Item::alternate_measure#callable".into());
    second_application.report_fingerprint =
        closed_conformance_application_report_fingerprint(&second_application);
    second_application.commitment = closed_conformance_application_commitment(&second_application);
    module
        .closed_conformance_applications
        .push(second_application.clone());
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

    module.dynamic_dispatch.rebound_descriptors.clear();
    module.dynamic_dispatch.selections[0].conformance_application_report_fingerprint =
        first_application.report_fingerprint;
    module.dynamic_dispatch.selections[0].conformance_application_commitment =
        first_application.commitment;
    module.dynamic_dispatch.selections[1].conformance_application_report_fingerprint =
        second_application.report_fingerprint;
    module.dynamic_dispatch.selections[1].conformance_application_commitment =
        second_application.commitment;
    module.dynamic_dispatch.arguments = vec![
        TerminalDynamicDescriptorArgument {
            owner: caller,
            operation: operation_id(3),
            parameter_ordinal: 0,
            source: TerminalDynamicDescriptorSource::Selection { ordinal: 0 },
        },
        TerminalDynamicDescriptorArgument {
            owner: caller,
            operation: operation_id(7),
            parameter_ordinal: 0,
            source: TerminalDynamicDescriptorSource::Selection { ordinal: 1 },
        },
    ];

    let caller_machine = &mut module.machines[0];
    caller_machine.parameters = vec![ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(10),
        scalar_type: ScalarType::Boolean,
    }];
    caller_machine.blocks = vec![
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(1),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Conditional {
                condition: value_id(10),
                when_true: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(1),
                    target: block_id(2),
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
                when_false: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(2),
                    target: block_id(3),
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        },
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(2),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: operation_id(3),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(2),
                    scalar_type: integer,
                }),
                kind: OperationKind::CallStructuralScalar {
                    erased_arguments: Vec::new(),
                    callee: helper,
                    arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: Terminator::Return {
                edge: edge_id(3),
                value: value_id(2),
                cleanup_actions: Vec::new(),
            },
        },
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(3),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: operation_id(7),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(8),
                    scalar_type: integer,
                }),
                kind: OperationKind::CallStructuralScalar {
                    erased_arguments: Vec::new(),
                    callee: helper,
                    arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: Terminator::Return {
                edge: edge_id(4),
                value: value_id(8),
                cleanup_actions: Vec::new(),
            },
        },
    ];

    let mut second_machine = module
        .machines
        .iter()
        .find(|machine| machine.id == first_realization)
        .expect("first realization machine")
        .clone();
    second_machine.id = second_realization;
    second_machine.structural_parameters[0].place = place_id(98);
    second_machine.structural_places[0].id = place_id(98);
    second_machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(9),
        scalar_type: integer,
    });
    second_machine.entry = block_id(98);
    second_machine.blocks[0].id = block_id(98);
    second_machine.blocks[0].operations[0].id = operation_id(6);
    second_machine.blocks[0].operations[0].result = OperationResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(11),
        scalar_type: integer,
    });
    second_machine.blocks[0].operations[0].kind = OperationKind::IntegerConstant {
        value: IntegerValue::Signed(41),
    };
    second_machine.blocks[0].terminator = Terminator::Return {
        edge: edge_id(98),
        value: value_id(11),
        cleanup_actions: Vec::new(),
    };
    second_machine.contract = empty_contract(contract_id(98));
    module.machines.push(second_machine);
    module
}

pub(super) fn internal_structural_call_module(crashes: bool) -> TerminalModule {
    let structural_type = structural_type_id(1);
    let domain = structural_domain_id(1);
    let caller_source = place_id(1);
    let caller_result = place_id(2);
    let operation_result = place_id(3);
    let callee_source = place_id(4);
    let callee_result = place_id(5);
    let claim = claim_id(1);
    let crash_route = CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    };
    let caller = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(1),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: caller_source,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            access: StructuralAccess::Owned,
            qualifications: vec![domain],
            projected_qualifications: Vec::new(),
        }],
        ranked_scc: None,
        result: TerminalMachineResult::Structural(StructuralResultDeclaration {
            reference_sources: Vec::new(),
            place: caller_result,
            structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: vec![domain],
            projected_qualifications: Vec::new(),
        }),
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: caller_source,
                kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: caller_result,
                kind: semantic_vocabulary::StructuralPlaceKind::Result,
            },
            StructuralPlaceDeclaration {
                id: operation_result,
                kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                    producer: operation_id(1),
                    structural_type,
                },
            },
        ],
        entry_claims: vec![EntryClaim {
            claim,
            input: caller_source,
            path: Vec::new(),
        }],
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: operation_id(1),
                result: OperationResult::Structural(StructuralOperationResult {
                    place: operation_result,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Linear,
                    qualifications: vec![domain],
                    projected_qualifications: Vec::new(),
                    claims: vec![StructuralResultClaimBinding {
                        claim,
                        path: Vec::new(),
                    }],
                }),
                kind: OperationKind::CallStructural {
                    callee: machine_id(2),
                    structural_arguments: vec![StructuralArgument {
                        place: caller_source,
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    }],
                    claim_transfers: vec![ClaimTransfer {
                        claim,
                        argument_index: 0,
                    }],
                    returned_claim_transfers: vec![StructuralResultClaimTransfer {
                        callee_claim: claim,
                        caller_claim: claim,
                    }],
                    requirement_obligations: Vec::new(),
                    crash_continuations: if crashes {
                        vec![crash_route.clone()]
                    } else {
                        Default::default()
                    },
                    selected_evidence: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnStructural {
                edge: edge_id(1),
                source: operation_result,
                returned_claims: vec![claim],
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: contract_id(1),
            crash_routes: if crashes {
                vec![crash_route.clone()]
            } else {
                Default::default()
            },
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    let callee = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(2),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: callee_source,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            access: StructuralAccess::Owned,
            qualifications: vec![domain],
            projected_qualifications: Vec::new(),
        }],
        ranked_scc: None,
        result: TerminalMachineResult::Structural(StructuralResultDeclaration {
            reference_sources: Vec::new(),
            place: callee_result,
            structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: vec![domain],
            projected_qualifications: Vec::new(),
        }),
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: callee_source,
                kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: callee_result,
                kind: semantic_vocabulary::StructuralPlaceKind::Result,
            },
        ],
        entry_claims: vec![EntryClaim {
            claim,
            input: callee_source,
            path: Vec::new(),
        }],
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(2),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(2),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: if crashes {
                Terminator::Crash {
                    edge: edge_id(2),
                    cause: CrashCause::Trap,
                    site_guard: Vec::new(),
                    frontier_lower_bound: vec![claim],
                }
            } else {
                Terminator::ReturnStructural {
                    edge: edge_id(2),
                    source: callee_source,
                    returned_claims: vec![claim],
                    trivial_affine_discards: Vec::new(),
                }
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: contract_id(2),
            crash_routes: if crashes {
                vec![crash_route]
            } else {
                Default::default()
            },
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: caller.id,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::Resource".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        }],
        structural_domains: vec![StructuralDomainDeclaration {
            id: domain,
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1)
                .expect("semantic domain identity"),
            identity: "test::Owned".into(),
            carrier: structural_type,
            content_projection: None,
        }],
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
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![caller, callee],
    }
}

pub(super) fn multi_claim_internal_structural_call_module(crashes: bool) -> TerminalModule {
    let mut module = internal_structural_call_module(crashes);
    let element_type = structural_type_id(2);
    module.structural_types[0].shape = StructuralTypeShape::FixedArray {
        element: element_type,
        length: 2,
    };
    module.structural_types.push(StructuralTypeDeclaration {
        id: element_type,
        identity: "test::ResourceElement".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let paths = [
        vec![terminal_psi::StructuralPathSegment::FixedIndex(0)],
        vec![terminal_psi::StructuralPathSegment::FixedIndex(1)],
    ];

    for machine in &mut module.machines {
        machine.entry_claims = paths
            .iter()
            .enumerate()
            .map(|(index, path)| EntryClaim {
                claim: claim_id(index as u64 + 1),
                input: machine.structural_parameters[0].place,
                path: path.clone(),
            })
            .collect();
        match &mut machine.blocks[0].terminator {
            Terminator::ReturnStructural {
                returned_claims, ..
            } => *returned_claims = vec![claim_id(1), claim_id(2)],
            Terminator::Crash {
                frontier_lower_bound,
                ..
            } => *frontier_lower_bound = vec![claim_id(1), claim_id(2)],
            _ => unreachable!(),
        }
    }

    let operation = &mut module.machines[0].blocks[0].operations[0];
    let OperationResult::Structural(result) = &mut operation.result else {
        unreachable!()
    };
    result.claims = paths
        .iter()
        .enumerate()
        .map(|(index, path)| StructuralResultClaimBinding {
            claim: claim_id(index as u64 + 1),
            path: path.clone(),
        })
        .collect();
    let OperationKind::CallStructural {
        claim_transfers,
        returned_claim_transfers,
        ..
    } = &mut operation.kind
    else {
        unreachable!()
    };
    *claim_transfers = vec![
        ClaimTransfer {
            claim: claim_id(1),
            argument_index: 0,
        },
        ClaimTransfer {
            claim: claim_id(2),
            argument_index: 0,
        },
    ];
    *returned_claim_transfers = vec![
        StructuralResultClaimTransfer {
            callee_claim: claim_id(1),
            caller_claim: claim_id(1),
        },
        StructuralResultClaimTransfer {
            callee_claim: claim_id(2),
            caller_claim: claim_id(2),
        },
    ];
    module
}
