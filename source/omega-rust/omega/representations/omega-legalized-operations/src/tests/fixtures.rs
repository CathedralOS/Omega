use super::*;

pub(super) fn call_aware_plan() -> LegalizedOperationPlan {
    let extent_type = id::<StructuralTypeId>(1);
    let granted = id::<StructuralDomainId>(1);
    let image_place = id(1);
    let storage_place = id(2);
    let shape = ValueShape::integer(16, 8);
    let call_plan = evaluate_call_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![shape, shape],
            result: None,
        },
    )
    .expect("two-Extent Microsoft x64 call plan");
    let parameters = [image_place, storage_place]
        .into_iter()
        .enumerate()
        .map(|(position, place)| LegalizedCallUnitParameter {
            semantic: StructuralParameterDeclaration {
                place,
                position: position as u32,
                is_self: false,
                structural_type: extent_type,
                multiplicity: StructuralMultiplicity::Linear,
                access: StructuralAccess::Owned,
                qualifications: vec![granted],
            },
            target: omega_target_operations::TargetStructuralParameter {
                place,
                structural_type: extent_type,
                multiplicity: StructuralMultiplicity::Linear,
                access: StructuralAccess::Owned,
                shape,
                placement: call_plan.parameters[position].clone(),
            },
        })
        .collect::<Vec<_>>();
    let arguments = parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| LegalizedCallUnitArgument {
            semantic: StructuralArgument {
                place: parameter.semantic.place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
            target: omega_target_operations::TargetStructuralArgument {
                place: parameter.semantic.place,
                access: StructuralAccess::Owned,
                path: Vec::new(),
                root_structural_type: extent_type,
                structural_type: extent_type,
                shape,
                source_byte_offset: 0,
                fixed_array_length: None,
                element_stride: None,
                source: call_plan.parameters[position].clone(),
                destination: call_plan.parameters[position].clone(),
            },
        })
        .collect::<Vec<_>>();
    let call = id::<OperationId>(1);
    let return_edge = id::<EdgeId>(1);
    LegalizedOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        },
        optimization_unit: OptimizationUnitIdentity::from_canonical_bytes(b"unit"),
        fuel_schedule: FuelScheduleIdentity::new(1).expect("fuel schedule"),
        target: NativeTarget::from_omega_target_name(Some("uefi_x64")).expect("UEFI target"),
        entry: id(1),
        functions: Vec::new(),
        unit_functions: Vec::new(),
        structural_unit_functions: vec![LegalizedStructuralUnitFunction {
            machine: id(1),
            attachment: None,
            provenance: TerminalPsiProvenance {
                operations: vec![call],
                edges: vec![return_edge],
            },
            recipe: StructuralUnitLegalizationRecipe::AuthoredCallThenReturnUnitV1,
            structural_types: vec![StructuralTypeDeclaration {
                id: extent_type,
                identity: "omega::core::Extent".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![
                        StructuralFieldDeclaration {
                            id: id::<StructuralFieldId>(1),
                            identity: "base".into(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(psi_core::ScalarType::Integer(
                                IntegerType::address(64).expect("addr"),
                            )),
                        },
                        StructuralFieldDeclaration {
                            id: id::<StructuralFieldId>(2),
                            identity: "length".into(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(psi_core::ScalarType::Integer(
                                IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
                            )),
                        },
                    ],
                },
            }],
            call_plan,
            parameters,
            structural_places: vec![
                StructuralPlaceDeclaration {
                    id: image_place,
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                },
                StructuralPlaceDeclaration {
                    id: storage_place,
                    kind: StructuralPlaceKind::Parameter {
                        position: 1,
                        is_self: false,
                    },
                },
            ],
            entry_claims: vec![
                EntryClaim {
                    claim: id(1),
                    input: image_place,
                    path: Vec::new(),
                },
                EntryClaim {
                    claim: id(2),
                    input: storage_place,
                    path: Vec::new(),
                },
            ],
            published_service_ceiling: Vec::new(),
            entry_block: id(1),
            boundary_settlements: Vec::new(),
            call: Some(LegalizedCallUnit {
                source: LegalizedCallUnitSource::AuthoredCallUnit,
                operation: call,
                callee: id(2),
                arguments,
                claim_transfers: vec![
                    ClaimTransfer {
                        claim: id(1),
                        argument_index: 0,
                    },
                    ClaimTransfer {
                        claim: id(2),
                        argument_index: 1,
                    },
                ],
                fuel: vec![FuelSettlement {
                    site: PsiProvenance::Operation(call),
                    units: 2,
                }],
                effect: EffectLink {
                    input: 0,
                    output: 1,
                },
                ownership: vec![OwnershipEvent::ClaimTransfer(vec![id(1), id(2)])],
            }),
            return_edge,
            return_fuel: vec![FuelSettlement {
                site: PsiProvenance::Edge(return_edge),
                units: 1,
            }],
            return_effect: EffectLink {
                input: 1,
                output: 2,
            },
            return_ownership: vec![OwnershipEvent::Cleanup(Vec::new())],
        }],
    }
}

pub(super) fn installed_provider_plan() -> LegalizedOperationPlan {
    let mut plan = call_aware_plan();
    let function = &mut plan.structural_unit_functions[0];
    function.recipe = StructuralUnitLegalizationRecipe::InstalledProviderCallThenReturnUnitV1;
    let boundary = id::<BoundaryMachineId>(1);
    let provider = ProviderCandidateConformance {
        boundary,
        requirement_identity: "ProgramEntry::enter".into(),
        provider_identity: "UefiProgramProvider".into(),
        candidate_identity: "UefiProgramProvider::enter".into(),
        candidate: id(2),
        signature: psi_terminal::ProviderUnitSignature {
            parameters: function
                .parameters
                .iter()
                .map(|parameter| psi_terminal::ProviderSignatureParameter {
                    position: parameter.semantic.position,
                    is_self: parameter.semantic.is_self,
                    structural_type: parameter.semantic.structural_type,
                    multiplicity: parameter.semantic.multiplicity,
                    access: parameter.semantic.access,
                    qualifications: parameter.semantic.qualifications.clone(),
                })
                .collect(),
        },
        refinement: psi_terminal::ProviderUnitRefinement {
            positional_parameters: vec![
                psi_terminal::ProviderParameterRefinement {
                    boundary_index: 0,
                    candidate_index: 0,
                },
                psi_terminal::ProviderParameterRefinement {
                    boundary_index: 1,
                    candidate_index: 1,
                },
            ],
            required_domains: Vec::new(),
            realized_service_ceiling: Vec::new(),
        },
    };
    let completion_claim_sources = function
        .entry_claims
        .iter()
        .cloned()
        .map(|entry| CompletionClaimSource {
            claim: entry.claim,
            entry: Some(entry),
            content: None,
        })
        .collect::<Vec<_>>();
    let completion_receipts = function
        .entry_claims
        .iter()
        .enumerate()
        .map(|(argument_index, claim)| CompletionReceipt {
            claim: claim.claim,
            argument_index: argument_index as u32,
        })
        .collect::<Vec<_>>();
    let call = function.call.as_mut().expect("structural call");
    call.source = LegalizedCallUnitSource::InstalledProvider {
        boundary,
        provider,
        completion_claim_sources,
        completion_receipts,
    };
    call.ownership = vec![OwnershipEvent::ClaimCompletion(vec![id(1), id(2)])];
    plan
}
