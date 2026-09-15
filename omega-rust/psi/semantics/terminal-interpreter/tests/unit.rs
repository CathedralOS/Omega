//! Fixtures shared by the unit interpretation tests: effect artifact
//! sections, structural places, call modules and identities.

#[path = "unit/affine_cleanups.rs"]
mod affine_cleanups;
#[path = "unit/affine_identity_calls.rs"]
mod affine_identity_calls;
#[path = "unit/boundary_borrows.rs"]
mod boundary_borrows;
#[path = "unit/bounded_fields.rs"]
mod bounded_fields;
#[path = "unit/byte_sequence_forwarding.rs"]
mod byte_sequence_forwarding;
#[path = "unit/byte_sequence_length.rs"]
mod byte_sequence_length;
#[path = "unit/byte_sequence_read.rs"]
mod byte_sequence_read;
#[path = "unit/byte_sequence_scalar_calls.rs"]
mod byte_sequence_scalar_calls;
#[path = "unit/byte_sequence_scalar_view_transfers.rs"]
mod byte_sequence_scalar_view_transfers;
#[path = "unit/byte_sequence_subslice.rs"]
mod byte_sequence_subslice;
#[path = "unit/case_membership.rs"]
mod case_membership;
#[path = "unit/claims_and_effects.rs"]
mod claims_and_effects;
#[path = "unit/cyclic_receiver.rs"]
mod cyclic_receiver;
#[path = "unit/ieee_float_comparisons.rs"]
mod ieee_float_comparisons;
#[path = "unit/indexed_structural_store.rs"]
mod indexed_structural_store;
#[path = "unit/primitive_arrays.rs"]
mod primitive_arrays;
#[path = "unit/primitive_locals.rs"]
mod primitive_locals;
#[path = "unit/records.rs"]
mod records;
#[path = "unit/reference_records.rs"]
mod reference_records;
#[path = "unit/result_residuals.rs"]
mod result_residuals;
#[path = "unit/scalar_arrays.rs"]
mod scalar_arrays;
#[path = "unit/scalar_cases.rs"]
mod scalar_cases;
#[path = "unit/scalar_qualifications.rs"]
mod scalar_qualifications;
#[path = "unit/scalar_returns_and_nominal_modules.rs"]
mod scalar_returns_and_nominal_modules;
#[path = "unit/unit_results_and_fuel.rs"]
mod unit_results_and_fuel;

use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, ContractId, EdgeId, EvidenceIdentity, IeeeFloatFormat,
    IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId, OperationId,
    PlaceId, Proposition, ScalarTerm, ScalarType, ServiceId, StructuralCaseId, StructuralDomainId,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_codec::{decode_module, encode_module, encode_proof_section};
use terminal_fuel::{FuelChargeSite, FuelExhaustion, TerminalFuelMeter, TerminalFuelSchedule};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalInterpretError,
    TerminalScalarValue, TerminalStructuralPrimitiveValue, TerminalStructuralValue,
    interpret_terminal_artifact_measured,
};
use terminal_psi::{
    BindingRelevance, Block, BoundaryMachineDeclaration, ByteSequenceCarrier, ClaimTransfer,
    ClosedConformanceApplication, ClosedConformanceCallableResult,
    ClosedConformanceRealizationCallable, ClosedConformanceRow, CompletionReceipt, CrashCause,
    CrashRouteBucket, CrashRouteGuard, EntryClaim, MachineContract, NominalAffineCleanup,
    Operation, OperationKind, OperationResult, ServiceDeclaration, StructuralAccess,
    StructuralAffineDiscard, StructuralArgument, StructuralDomainDeclaration,
    StructuralDomainRequirement, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralResultClaimBinding,
    StructuralResultClaimTransfer, StructuralResultDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, SuccessorEdge, TerminalAffineCleanupAction,
    TerminalDynamicConformanceSelection, TerminalDynamicDescriptorArgument,
    TerminalDynamicDescriptorParameter, TerminalDynamicDescriptorSource,
    TerminalDynamicDispatchCatalog, TerminalDynamicRequirement, TerminalIndirectDynamicDispatch,
    TerminalMachine, TerminalMachineResult, TerminalModule, TerminalParameterDynamicDispatch,
    TerminalReboundDynamicDescriptor, Terminator, ValueDeclaration, VocabularyMarker,
    closed_conformance_application_commitment, closed_conformance_application_report_fingerprint,
};
use terminal_verifier::{
    ModuleError, ObligationEvidence, ProofBundle, VerificationError, verify_module,
};

fn reference_release_module() -> TerminalModule {
    let mut module = write_only_primitive_call_module();
    module.machines.truncate(1);
    let reference_type = structural_type_id(94);
    module.structural_types.push(StructuralTypeDeclaration {
        id: reference_type,
        identity: "test::MutableU8Reference".into(),
        shape: StructuralTypeShape::Reference {
            referent: structural_type_id(91),
            access: StructuralAccess::MutableBorrow,
        },
    });
    let caller = &mut module.machines[0];
    caller.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(94),
        kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
            producer: operation_id(94),
            structural_type: reference_type,
        },
    });
    caller.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            id: operation_id(94),
            result: OperationResult::Structural(StructuralOperationResult {
                place: place_id(94),
                structural_type: reference_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishReference {
                source: StructuralArgument {
                    place: place_id(91),
                    path: Vec::new(),
                    access: StructuralAccess::MutableBorrow,
                },
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(95),
            result: OperationResult::Unit,
            kind: OperationKind::ReleaseReference {
                source: place_id(94),
            },
        },
    ];
    module
}

fn assert_write_only_store_atomic(
    module: TerminalModule,
    opaque_identity: u64,
    initial_value: TerminalScalarValue,
    written_value: TerminalScalarValue,
) {
    let semantic = encode_module(&module).expect("primitive-store semantics encode");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let structural = TerminalStructuralValue {
        opaque_identity,
        structural_type: structural_type_id(91),
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let initial = TerminalStructuralPrimitiveValue {
        argument_index: 0,
        value: initial_value,
    };
    let written = TerminalStructuralPrimitiveValue {
        argument_index: 0,
        value: written_value,
    };
    let mut execution =
        TerminalExecution::start_artifact_with_structural_arguments_and_primitive_values(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
            &[structural],
            &[initial],
        )
        .expect("verified Boolean store starts");
    let mut meter = TerminalFuelMeter::with_allowance(2);

    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Operation(operation_id(93)),
            required_units: 1,
            remaining_units: 0,
        })
    );
    assert_eq!(execution.structural_primitive_values(), vec![initial]);
    assert_eq!(meter.usage().total_units(), 2);

    meter.replenish(3).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(execution.structural_primitive_values(), vec![written]);
    assert_eq!(meter.usage().total_units(), 5);
    assert_eq!(
        meter
            .usage()
            .at(FuelChargeSite::Operation(operation_id(93)))
            .unwrap()
            .executions(),
        1,
        "the primitive store commits exactly once after replenishment"
    );
}

struct BoundaryResultHandler {
    result: Result<TerminalEffectResult, TerminalEffectRejection>,
    requests: usize,
    effects: Vec<TerminalEffect>,
}

impl TerminalEffectHandler for BoundaryResultHandler {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        panic!("result-bearing boundary must use the explicit result handler");
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        self.requests += 1;
        let result = self.result.clone()?;
        self.effects.push(effect.clone());
        Ok(result)
    }
}

#[derive(Default)]
struct RecordingHandler {
    effects: Vec<TerminalEffect>,
}

impl TerminalEffectHandler for RecordingHandler {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        self.effects.push(effect.clone());
        Ok(())
    }
}

struct RejectingHandler;

impl TerminalEffectHandler for RejectingHandler {
    fn handle_effect(&mut self, _effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new("mock rejection"))
    }
}

struct RejectScalarBoundaryArguments;

impl TerminalEffectHandler for RejectScalarBoundaryArguments {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        match effect {
            TerminalEffect::BoundaryCall { arguments, .. }
                if arguments
                    == &[
                        TerminalScalarValue::Boolean(true),
                        TerminalScalarValue::Boolean(false),
                    ] =>
            {
                Err(TerminalEffectRejection::new(
                    "mock policy rejects the scalar boundary argument",
                ))
            }
            _ => Err(TerminalEffectRejection::new(
                "scalar boundary arguments were not resolved in declaration order",
            )),
        }
    }
}

fn byte_sequence_literal_module(bytes: Vec<u8>) -> TerminalModule {
    let structural_type = structural_type_id(1);
    let literal = place_id(1);
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::BorrowedBytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        }],
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            fixed_service_reach: Vec::new(),
            id: boundary_id(1),
            identity: "test::write_line".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: place_id(2),
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
            crash_routes: Vec::new(),
        }],
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
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine_id(1),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: vec![StructuralPlaceDeclaration {
                id: literal,
                kind: semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal: 0,
                    structural_type,
                },
            }],
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(1),
            blocks: vec![Block {
                structural_parameters: Vec::new(),
                id: block_id(1),
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        static_reach_binding: None,
                        id: operation_id(1),
                        result: OperationResult::Unit,
                        kind: OperationKind::EstablishByteSequenceLiteral {
                            destination: literal,
                            bytes,
                        },
                    },
                    Operation {
                        static_reach_binding: None,
                        id: operation_id(2),
                        result: OperationResult::Unit,
                        kind: OperationKind::BoundaryCall {
                            boundary: boundary_id(1),
                            arguments: Vec::new(),
                            structural_arguments: vec![StructuralArgument {
                                place: literal,
                                access: StructuralAccess::SharedBorrow,
                                path: Vec::new(),
                            }],
                            completion_receipts: Vec::new(),
                        },
                    },
                ],
                terminator: Terminator::ReturnUnit {
                    edge: edge_id(1),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: empty_contract(contract_id(1)),
        }],
    }
}

fn effect_artifact_sections() -> (Vec<u8>, Vec<u8>) {
    (
        encode_module(&effect_module()).expect("effect semantics encode"),
        encode_proof_section(&effect_module(), &ProofBundle::default())
            .expect("empty proof encodes"),
    )
}

fn scalar_boundary_effect_module() -> TerminalModule {
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            fixed_service_reach: Vec::new(),
            id: boundary_id(1),
            identity: "test::observe".into(),
            attachment: None,
            scalar_parameters: vec![ScalarType::Boolean, ScalarType::Boolean],
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
            crash_routes: Vec::new(),
        }],
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
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine_id(1),
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
            entry: block_id(1),
            blocks: vec![Block {
                structural_parameters: Vec::new(),
                id: block_id(1),
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        static_reach_binding: None,
                        id: operation_id(1),
                        result: OperationResult::Scalar(ValueDeclaration {
                            qualifications: Default::default(),
                            id: value_id(1),
                            scalar_type: ScalarType::Boolean,
                        }),
                        kind: OperationKind::BooleanConstant { value: true },
                    },
                    Operation {
                        static_reach_binding: None,
                        id: operation_id(2),
                        result: OperationResult::Scalar(ValueDeclaration {
                            qualifications: Default::default(),
                            id: value_id(2),
                            scalar_type: ScalarType::Boolean,
                        }),
                        kind: OperationKind::BooleanConstant { value: false },
                    },
                    Operation {
                        static_reach_binding: None,
                        id: operation_id(3),
                        result: OperationResult::Unit,
                        kind: OperationKind::BoundaryCall {
                            boundary: boundary_id(1),
                            arguments: vec![value_id(1), value_id(2)],
                            structural_arguments: Vec::new(),
                            completion_receipts: Vec::new(),
                        },
                    },
                ],
                terminator: Terminator::ReturnUnit {
                    edge: edge_id(1),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: empty_contract(contract_id(1)),
        }],
    }
}

fn structural_boundary_effect_module() -> TerminalModule {
    let mut module = scalar_boundary_effect_module();
    let structural_type = structural_type_id(1);
    let place = place_id(1);
    let producer = operation_id(3);
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::ByteRead".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    module.boundary_machines[0].result = terminal_psi::BoundaryMachineResult::Structural(
        terminal_psi::BoundaryStructuralResultDeclaration {
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
        },
    );
    module.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: place,
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            },
        });
    module.machines[0].blocks[0].operations[2].result =
        OperationResult::Structural(StructuralOperationResult {
            place,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        });
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!("scalar boundary fixture returns Unit")
    };
    trivial_affine_discards.push(place);
    module
}

fn effect_module() -> TerminalModule {
    let structural_type = structural_type_id(1);
    let domain = structural_domain_id(1);
    let service = service_id(1);
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::Device".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        }],
        structural_domains: vec![StructuralDomainDeclaration {
            id: domain,
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1)
                .expect("semantic domain identity"),
            identity: "test::Ready".into(),
            carrier: structural_type,
            content_projection: None,
        }],
        services: vec![ServiceDeclaration {
            id: service,
            identity: "test::PortIo".into(),
            parents: Vec::new(),
        }],
        root_service_reach: terminal_psi::TerminalRootServiceReach {
            concrete: vec![service],
            installation_dependencies: Vec::new(),
        },
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            fixed_service_reach: Vec::new(),
            id: boundary_id(1),
            identity: "test::acknowledge".into(),
            attachment: Some(structural_type),
            scalar_parameters: Vec::new(),
            structural_parameters: vec![structural_parameter(place_id(3), structural_type, domain)],
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: vec![StructuralDomainRequirement {
                argument_index: 0,
                domain,
            }],
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
            crash_routes: Vec::new(),
        }],
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
        machines: vec![
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(1),
                attachment: Some(structural_type),
                parameters: Vec::new(),
                structural_parameters: vec![structural_parameter(
                    place_id(1),
                    structural_type,
                    domain,
                )],
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: vec![structural_place(place_id(1))],
                entry_claims: vec![EntryClaim {
                    claim: claim_id(1),
                    input: place_id(1),
                    path: Vec::new(),
                }],
                published_service_ceiling: vec![service],
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(1),
                blocks: vec![Block {
                    structural_parameters: Vec::new(),
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            static_reach_binding: None,
                            id: operation_id(1),
                            result: OperationResult::Unit,
                            kind: OperationKind::CallUnit {
                                arguments: Vec::new(),
                                callee: machine_id(2),
                                structural_arguments: vec![StructuralArgument {
                                    place: place_id(1),
                                    access: StructuralAccess::Owned,
                                    path: Vec::new(),
                                }],
                                claim_transfers: vec![ClaimTransfer {
                                    claim: claim_id(1),
                                    argument_index: 0,
                                }],
                                requirement_obligations: Vec::new(),
                                crash_continuations: Vec::new(),
                            },
                        },
                        Operation {
                            static_reach_binding: None,
                            id: operation_id(2),
                            result: OperationResult::Unit,
                            kind: OperationKind::PortWrite {
                                service,
                                port: 0x20,
                                value: 0x20,
                            },
                        },
                    ],
                    terminator: Terminator::ReturnUnit {
                        edge: edge_id(1),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: empty_contract(contract_id(1)),
            },
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(2),
                attachment: Some(structural_type),
                parameters: Vec::new(),
                structural_parameters: vec![structural_parameter(
                    place_id(2),
                    structural_type,
                    domain,
                )],
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: vec![structural_place(place_id(2))],
                entry_claims: vec![EntryClaim {
                    claim: claim_id(1),
                    input: place_id(2),
                    path: Vec::new(),
                }],
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(2),
                blocks: vec![Block {
                    structural_parameters: Vec::new(),
                    id: block_id(2),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        static_reach_binding: None,
                        id: operation_id(3),
                        result: OperationResult::Unit,
                        kind: OperationKind::BoundaryCall {
                            boundary: boundary_id(1),
                            arguments: Vec::new(),
                            structural_arguments: vec![StructuralArgument {
                                place: place_id(2),
                                access: StructuralAccess::Owned,
                                path: Vec::new(),
                            }],
                            completion_receipts: vec![CompletionReceipt {
                                claim: claim_id(1),
                                argument_index: 0,
                            }],
                        },
                    }],
                    terminator: Terminator::ReturnUnit {
                        edge: edge_id(2),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: empty_contract(contract_id(2)),
            },
        ],
    }
}

fn structural_parameter(
    place: PlaceId,
    structural_type: StructuralTypeId,
    domain: StructuralDomainId,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: true,
        structural_type,
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications: vec![domain],
        projected_qualifications: Vec::new(),
    }
}

fn structural_place(id: PlaceId) -> StructuralPlaceDeclaration {
    StructuralPlaceDeclaration {
        id,
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: true,
        },
    }
}

fn structural_value(opaque_identity: u64) -> TerminalStructuralValue {
    TerminalStructuralValue {
        opaque_identity,
        structural_type: structural_type_id(1),
        qualifications: vec![structural_domain_id(1)],
        path: Vec::new(),
    }
}

fn empty_contract(id: ContractId) -> MachineContract {
    MachineContract {
        id,
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    }
}

fn artifact_sections() -> (Vec<u8>, Vec<u8>) {
    (
        encode_module(&unit_module()).expect("unit semantics encode"),
        encode_proof_section(&unit_module(), &ProofBundle::default()).expect("empty proof encodes"),
    )
}

fn payloadless_case_module() -> TerminalModule {
    let structural_type = structural_type_id(1);
    let result_case = structural_case_id(1);
    let operation_place = place_id(1);
    let result_place = place_id(2);
    let mut module = unit_module();
    module.structural_types = vec![StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::Outcome".into(),
        shape: StructuralTypeShape::Sum {
            cases: vec![terminal_psi::StructuralCaseDeclaration {
                id: result_case,
                identity: "Success".into(),
                fields: Vec::new(),
            }],
        },
    }];
    let machine = &mut module.machines[0];
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: result_place,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    });
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: operation_place,
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: operation_id(1),
                structural_type,
            },
        },
        StructuralPlaceDeclaration {
            id: result_place,
            kind: semantic_vocabulary::StructuralPlaceKind::Result,
        },
    ];
    machine.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        id: operation_id(1),
        result: OperationResult::Structural(StructuralOperationResult {
            place: operation_place,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishScalarCase {
            result_case,
            fields: vec![],
        },
    }];
    machine.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(1),
        source: operation_place,
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    module
}

fn payloadless_call_module() -> TerminalModule {
    let mut module = payloadless_case_module();
    let structural_type = structural_type_id(1);
    let mut callee = module.machines.remove(0);
    callee.id = machine_id(2);
    callee.entry = block_id(2);
    callee.blocks[0].id = block_id(2);
    let Terminator::ReturnStructural { edge, .. } = &mut callee.blocks[0].terminator else {
        unreachable!()
    };
    *edge = edge_id(2);
    callee.contract.id = contract_id(2);

    let caller = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(1),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Structural(StructuralResultDeclaration {
            reference_sources: Vec::new(),
            place: place_id(4),
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }),
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: place_id(3),
                kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                    producer: operation_id(2),
                    structural_type,
                },
            },
            StructuralPlaceDeclaration {
                id: place_id(4),
                kind: semantic_vocabulary::StructuralPlaceKind::Result,
            },
        ],
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: block_id(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: operation_id(2),
                result: OperationResult::Structural(StructuralOperationResult {
                    place: place_id(3),
                    structural_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: Vec::new(),
                }),
                kind: OperationKind::CallStructural {
                    callee: machine_id(2),
                    structural_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    returned_claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                    selected_evidence: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnStructural {
                edge: edge_id(1),
                source: place_id(3),
                returned_claims: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: contract_id(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    module.machines = vec![caller, callee];
    module
}

fn unit_module() -> TerminalModule {
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: Vec::new(),
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
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine_id(1),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(1),
            blocks: vec![Block {
                structural_parameters: Vec::new(),
                id: block_id(1),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: edge_id(1),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: contract_id(1),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn nearest_fma_module(operands: [IeeeFloatValue; 3]) -> TerminalModule {
    let format = operands[0].format();
    assert!(operands.iter().all(|operand| operand.format() == format));
    let mut module = unit_module();
    let operand_ids = [value_id(1), value_id(2), value_id(3)];
    let result = ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(4),
        scalar_type: ScalarType::IeeeFloat(format),
    };
    let machine = &mut module.machines[0];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(5),
        scalar_type: result.scalar_type,
    });
    machine.blocks[0].operations = operands
        .into_iter()
        .zip(operand_ids)
        .enumerate()
        .map(|(index, (value, id))| Operation {
            static_reach_binding: None,
            id: operation_id(index as u64 + 1),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id,
                scalar_type: ScalarType::IeeeFloat(format),
            }),
            kind: OperationKind::IeeeFloatConstant { value },
        })
        .chain(std::iter::once(Operation {
            static_reach_binding: None,
            id: operation_id(4),
            result: OperationResult::Scalar(result),
            kind: OperationKind::NearestIeeeFloatFusedMultiplyAdd {
                left: operand_ids[0],
                right: operand_ids[1],
                addend: operand_ids[2],
            },
        }))
        .collect();
    machine.blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: result.id,
        cleanup_actions: Vec::new(),
    };
    module
}

fn write_only_primitive_call_module() -> TerminalModule {
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

fn write_only_boolean_call_module() -> TerminalModule {
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

fn structural_scalar_field_call_module() -> TerminalModule {
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

fn rebound_dynamic_scalar_call_module() -> TerminalModule {
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

fn parameter_dynamic_scalar_call_module() -> TerminalModule {
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
fn joined_parameter_dynamic_scalar_call_module() -> TerminalModule {
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
                    trivial_affine_discards: Vec::new(),
                },
                when_false: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(2),
                    target: block_id(3),
                    arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        },
        Block {
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

fn nominal_affine_module() -> TerminalModule {
    let token = StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "Token".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![token.clone()],
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
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(1),
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: place_id(1),
                    position: 0,
                    is_self: false,
                    structural_type: token.id,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }],
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: vec![StructuralPlaceDeclaration {
                    id: place_id(1),
                    kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                }],
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(1),
                blocks: vec![Block {
                    structural_parameters: Vec::new(),
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnitNominalAffine {
                        edge: edge_id(1),
                        cleanups: vec![NominalAffineCleanup {
                            place: place_id(1),
                            structural_type: token.id,
                            cleanup_machine: machine_id(2),
                            cleanup_receiver: None,
                            requirement_obligations: Vec::new(),
                        }],
                    },
                }],
                contract: empty_contract(contract_id(1)),
            },
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(2),
                attachment: Some(token.id),
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
                entry: block_id(2),
                blocks: vec![Block {
                    structural_parameters: Vec::new(),
                    id: block_id(2),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: edge_id(2),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: empty_contract(contract_id(2)),
            },
        ],
    }
}

fn ordered_empty_nominal_affine_module(same_target: bool) -> TerminalModule {
    let mut module = nominal_affine_module();
    let first_type = structural_type_id(1);
    let second_type = if same_target {
        first_type
    } else {
        let second_type = structural_type_id(2);
        module.structural_types.push(StructuralTypeDeclaration {
            id: second_type,
            identity: "SecondToken".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    identity: "payload".into(),
                    id: semantic_vocabulary::StructuralFieldId::new(2).unwrap(),
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            64,
                        )
                        .unwrap(),
                    )),
                    relevance: BindingRelevance::Relevant,
                }],
            },
        });
        second_type
    };
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![StructuralFieldDeclaration {
            identity: "payload".into(),
            id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
            field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    32,
                )
                .unwrap(),
            )),
            relevance: BindingRelevance::Relevant,
        }],
    };
    let second_cleanup_machine = if same_target {
        machine_id(2)
    } else {
        let mut target = module.machines[1].clone();
        target.id = machine_id(3);
        target.attachment = Some(second_type);
        target.entry = block_id(3);
        target.blocks[0].id = block_id(3);
        target.blocks[0].terminator = Terminator::ReturnUnit {
            edge: edge_id(3),
            trivial_affine_discards: Vec::new(),
        };
        target.contract.id = contract_id(3);
        module.machines.push(target);
        machine_id(3)
    };
    let caller = &mut module.machines[0];
    caller
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(2),
            position: 1,
            is_self: false,
            structural_type: second_type,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(2),
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    caller.blocks[0].terminator = Terminator::ReturnUnitNominalAffine {
        edge: edge_id(1),
        cleanups: vec![
            NominalAffineCleanup {
                place: place_id(2),
                structural_type: second_type,
                cleanup_machine: second_cleanup_machine,
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            },
            NominalAffineCleanup {
                place: place_id(1),
                structural_type: first_type,
                cleanup_machine: machine_id(2),
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            },
        ],
    };
    module
}

fn ordered_one_executable_nominal_affine_module() -> TerminalModule {
    let mut module = ordered_empty_nominal_affine_module(false);
    let helper_type = structural_type_id(3);
    module.structural_types.push(StructuralTypeDeclaration {
        id: helper_type,
        identity: "Helper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let mut helper = module.machines[1].clone();
    helper.id = machine_id(4);
    helper.attachment = Some(helper_type);
    helper.entry = block_id(4);
    helper.blocks[0].id = block_id(4);
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(4),
        trivial_affine_discards: Vec::new(),
    };
    helper.contract.id = contract_id(4);
    module.machines[2].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(1),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            arguments: Vec::new(),
            callee: helper.id,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    module.machines.push(helper);
    module
}

fn three_ordered_empty_nominal_affine_module(same_target: bool) -> TerminalModule {
    let mut module = ordered_empty_nominal_affine_module(same_target);
    let third_type = if same_target {
        structural_type_id(1)
    } else {
        let third_type = structural_type_id(3);
        module.structural_types.push(StructuralTypeDeclaration {
            id: third_type,
            identity: "ThirdToken".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    identity: "payload".into(),
                    id: semantic_vocabulary::StructuralFieldId::new(3).unwrap(),
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            64,
                        )
                        .unwrap(),
                    )),
                    relevance: BindingRelevance::Relevant,
                }],
            },
        });
        third_type
    };
    let third_cleanup_machine = if same_target {
        machine_id(2)
    } else {
        let mut target = module.machines[1].clone();
        target.id = machine_id(4);
        target.attachment = Some(third_type);
        target.entry = block_id(4);
        target.blocks[0].id = block_id(4);
        target.blocks[0].terminator = Terminator::ReturnUnit {
            edge: edge_id(4),
            trivial_affine_discards: Vec::new(),
        };
        target.contract.id = contract_id(4);
        module.machines.push(target);
        machine_id(4)
    };
    let caller = &mut module.machines[0];
    caller
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(3),
            position: 2,
            is_self: false,
            structural_type: third_type,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(3),
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 2,
            is_self: false,
        },
    });
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &mut caller.blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups.insert(
        0,
        NominalAffineCleanup {
            place: place_id(3),
            structural_type: third_type,
            cleanup_machine: third_cleanup_machine,
            cleanup_receiver: None,
            requirement_obligations: Vec::new(),
        },
    );
    module
}

fn ordered_two_distinct_executable_nominal_affine_module() -> TerminalModule {
    let mut module = ordered_one_executable_nominal_affine_module();
    let helper_type = structural_type_id(4);
    module.structural_types.push(StructuralTypeDeclaration {
        id: helper_type,
        identity: "SecondHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let mut helper = module.machines[3].clone();
    helper.id = machine_id(5);
    helper.attachment = Some(helper_type);
    helper.entry = block_id(5);
    helper.blocks[0].id = block_id(5);
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(5),
        trivial_affine_discards: Vec::new(),
    };
    helper.contract.id = contract_id(5);
    module.machines[1].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(2),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            arguments: Vec::new(),
            callee: helper.id,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    module.machines.push(helper);
    module
}

fn ordered_shared_executable_nominal_affine_module() -> TerminalModule {
    let mut module = ordered_empty_nominal_affine_module(true);
    let helper_type = structural_type_id(2);
    module.structural_types.push(StructuralTypeDeclaration {
        id: helper_type,
        identity: "SharedHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let mut helper = module.machines[1].clone();
    helper.id = machine_id(3);
    helper.attachment = Some(helper_type);
    helper.entry = block_id(3);
    helper.blocks[0].id = block_id(3);
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(3),
        trivial_affine_discards: Vec::new(),
    };
    helper.contract.id = contract_id(3);
    module.machines[1].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(1),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            arguments: Vec::new(),
            callee: helper.id,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    module.machines.push(helper);
    module
}

fn three_ordered_shared_executable_nominal_affine_module() -> TerminalModule {
    let mut module = ordered_shared_executable_nominal_affine_module();
    let caller = &mut module.machines[0];
    caller
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(3),
            position: 2,
            is_self: false,
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(3),
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 2,
            is_self: false,
        },
    });
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &mut caller.blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups.insert(
        0,
        NominalAffineCleanup {
            place: place_id(3),
            structural_type: structural_type_id(1),
            cleanup_machine: machine_id(2),
            cleanup_receiver: None,
            requirement_obligations: Vec::new(),
        },
    );
    module
}

fn executable_nominal_affine_module() -> TerminalModule {
    let mut module = nominal_affine_module();
    let helper_type = StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "Helper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    module.structural_types.push(helper_type.clone());
    module.machines[1].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(1),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            arguments: Vec::new(),
            callee: machine_id(3),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    module.machines.push(TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(3),
        attachment: Some(helper_type.id),
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
        entry: block_id(3),
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: block_id(3),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(3),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(contract_id(3)),
    });
    module
}

fn two_helper_nominal_affine_module() -> TerminalModule {
    let mut module = executable_nominal_affine_module();
    let second_helper_type = StructuralTypeDeclaration {
        id: structural_type_id(3),
        identity: "SecondHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    module.structural_types.push(second_helper_type.clone());
    module.machines[1].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(2),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            arguments: Vec::new(),
            callee: machine_id(4),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    module.machines.push(TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(4),
        attachment: Some(second_helper_type.id),
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
        entry: block_id(4),
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: block_id(4),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(4),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(contract_id(4)),
    });
    module
}

fn three_helper_nominal_affine_module() -> TerminalModule {
    let mut module = two_helper_nominal_affine_module();
    let third_helper_type = StructuralTypeDeclaration {
        id: structural_type_id(4),
        identity: "ThirdHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    module.structural_types.push(third_helper_type.clone());
    module.machines[1].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(3),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            arguments: Vec::new(),
            callee: machine_id(5),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    let mut third_helper = module.machines[2].clone();
    third_helper.id = machine_id(5);
    third_helper.attachment = Some(third_helper_type.id);
    third_helper.entry = block_id(5);
    third_helper.blocks[0].id = block_id(5);
    third_helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(5),
        trivial_affine_discards: Vec::new(),
    };
    third_helper.contract.id = contract_id(5);
    module.machines.push(third_helper);
    module
}

fn partial_affine_field_module() -> TerminalModule {
    let token = StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "Token".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    let pair = StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "Pair".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![
                StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                    identity: "left".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(token.id),
                },
                StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(2).unwrap(),
                    identity: "right".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(token.id),
                },
            ],
        },
    };
    let caller = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(1),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: place_id(1),
            position: 0,
            is_self: false,
            structural_type: pair.id,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: place_id(1),
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: block_id(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: operation_id(1),
                result: OperationResult::Unit,
                kind: OperationKind::CallUnit {
                    arguments: Vec::new(),
                    callee: machine_id(2),
                    structural_arguments: vec![StructuralArgument {
                        place: place_id(1),
                        access: StructuralAccess::Owned,
                        path: vec![StructuralPathSegment::Field("right".into())],
                    }],
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnUnitPartialAffine {
                edge: edge_id(1),
                trivial_affine_discards: Vec::new(),
                residual_affine_discards: vec![StructuralAffineDiscard {
                    place: place_id(1),
                    path: vec![StructuralPathSegment::Field("left".into())],
                    structural_type: token.id,
                }],
            },
        }],
        contract: empty_contract(contract_id(1)),
    };
    let callee = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(2),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: place_id(2),
            position: 0,
            is_self: false,
            structural_type: token.id,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: place_id(2),
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(2),
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: block_id(2),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(2),
                trivial_affine_discards: vec![place_id(2)],
            },
        }],
        contract: empty_contract(contract_id(2)),
    };
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: caller.id,
        structural_types: vec![token, pair],
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
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![caller, callee],
    }
}

fn internal_structural_call_module(crashes: bool) -> TerminalModule {
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

fn multi_claim_internal_structural_call_module(crashes: bool) -> TerminalModule {
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

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).unwrap()
}

fn block_id(raw: u64) -> BlockId {
    BlockId::new(raw).unwrap()
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).unwrap()
}

fn contract_id(raw: u64) -> ContractId {
    ContractId::new(raw).unwrap()
}

fn boundary_id(raw: u64) -> BoundaryMachineId {
    BoundaryMachineId::new(raw).unwrap()
}

fn operation_id(raw: u64) -> OperationId {
    OperationId::new(raw).unwrap()
}

fn obligation_id(raw: u64) -> ObligationId {
    ObligationId::new(raw).unwrap()
}

fn place_id(raw: u64) -> PlaceId {
    PlaceId::new(raw).unwrap()
}

fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).unwrap()
}

fn claim_id(raw: u64) -> ClaimId {
    ClaimId::new(raw).unwrap()
}

fn structural_type_id(raw: u64) -> StructuralTypeId {
    StructuralTypeId::new(raw).unwrap()
}

fn structural_field_id(raw: u64) -> StructuralFieldId {
    StructuralFieldId::new(raw).unwrap()
}

fn structural_case_id(raw: u64) -> StructuralCaseId {
    StructuralCaseId::new(raw).unwrap()
}

fn structural_domain_id(raw: u64) -> StructuralDomainId {
    StructuralDomainId::new(raw).unwrap()
}

fn service_id(raw: u64) -> ServiceId {
    ServiceId::new(raw).unwrap()
}
