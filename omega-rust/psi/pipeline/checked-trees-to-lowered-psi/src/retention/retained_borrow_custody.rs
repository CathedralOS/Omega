//! Source-independent retention of checked foreign shared-borrow custody.
use super::{
    BoundaryContentGuarantee, BoundaryMachineDeclaration, BoundaryMachineId, BoundaryMachineResult,
    CheckedTrees, ContentPlaceVersion, DomainSemanticId, LoweringError, Multiplicity,
    StructuralAccess, StructuralMultiplicity, TerminalModule, checked_unit_boundary_identity,
    content_conservation, unsupported,
};
use checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};

fn unconstrained_type(checked: &CheckedTrees, mut ty: TypeReferenceHandle) -> TypeReferenceHandle {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        checked.type_reference_table.type_reference(ty)
    {
        ty = *base_type;
    }
    ty
}

fn lower_projection(
    checked: &CheckedTrees,
    projection: &language_semantics::content::ContentProjectionPlan,
) -> Result<terminal_psi::RetainedBorrowContentProjection, LoweringError> {
    let exact = checked
        .facts
        .qualifications
        .content
        .for_semantic_domain(projection.semantic_domain)
        .ok_or(LoweringError::Unsupported(
            "retained-borrow projection is absent from checked content facts",
        ))?;
    if exact != projection {
        return unsupported("retained-borrow projection drifted from checked content facts");
    }
    let projection = content_conservation::lower_structural_content_projection(
        checked,
        projection.semantic_domain,
        &projection.carrier_identity,
    )?
    .ok_or(LoweringError::Unsupported(
        "retained-borrow projection has no complete Terminal definition",
    ))?;
    Ok(terminal_psi::RetainedBorrowContentProjection {
        semantic_domain: DomainSemanticId::new(projection.identity.domain.get())
            .ok_or(LoweringError::InvalidContentDomainIdentity)?,
        carrier_identity: exact.carrier_identity.clone(),
        projection,
    })
}

fn lower_custody(
    checked: &CheckedTrees,
    fact: &checked_trees::RetainedBorrowCustodyFact,
) -> Result<terminal_psi::RetainedBorrowCustody, LoweringError> {
    let requirements = checked
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .flat_map(|definition| checked.trait_machine_signatures(definition))
        .filter(|signature| signature.symbol == fact.callable)
        .collect::<Vec<_>>();
    let [signature] = requirements.as_slice() else {
        return unsupported("retained-borrow callable is not one exact boundary requirement");
    };
    let callable_identity = checked_unit_boundary_identity(checked, fact.callable)?;
    let callable_lifetime_parameter_count = u32::try_from(signature.lifetime_parameters.len())
        .map_err(|_| {
            LoweringError::Unsupported("retained-borrow callable lifetime count exceeds u32")
        })?;
    if usize::try_from(fact.callable_lifetime_parameter_ordinal)
        .ok()
        .and_then(|ordinal| signature.lifetime_parameters.get(ordinal))
        != Some(&fact.lifetime)
    {
        return unsupported("retained-borrow callable lifetime ordinal does not replay");
    }

    let language_semantics::content::ContentPlaceRoot::Parameter {
        position,
        symbol,
        name,
        is_self,
    } = &fact.source.root
    else {
        return unsupported("retained-borrow source is not a direct parameter");
    };
    let parameter = checked
        .state_signature_parameters(signature)
        .get(*position as usize)
        .ok_or(LoweringError::Unsupported(
            "retained-borrow source parameter is out of range",
        ))?;
    if parameter.symbol != *symbol
        || parameter.name.as_str() != name
        || parameter.is_self != *is_self
        || *is_self
        || fact.source.version != language_semantics::content::ContentPlaceVersion::Entry
        || !fact.source.segments.is_empty()
        || fact.access != language_semantics::ReferenceAccess::Shared
    {
        return unsupported("retained-borrow source place or access does not replay");
    }
    let source_ty = unconstrained_type(checked, parameter.type_reference);
    let TypeReferenceNode::Reference {
        referee,
        access: language_core::ReferenceAccess::Shared,
        lifetime: Some(source_lifetime),
    } = checked.type_reference_table.type_reference(source_ty)
    else {
        return unsupported("retained-borrow source is not one explicit direct shared reference");
    };
    if source_lifetime != &fact.lifetime
        || checked
            .normalized_type_identity(unconstrained_type(checked, *referee))
            .into_string()
            != fact.source_projection.carrier_identity
    {
        return unsupported("retained-borrow source lifetime or nominal carrier drifted");
    }

    if fact.result.version != language_semantics::content::ContentPlaceVersion::Current
        || !matches!(
            fact.result.root,
            language_semantics::content::ContentPlaceRoot::Result
        )
        || !fact.result.segments.is_empty()
    {
        return unsupported("retained-borrow result place does not replay");
    }
    let result_ty = unconstrained_type(checked, signature.return_type);
    let TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = checked.type_reference_table.type_reference(result_ty)
    else {
        return unsupported("retained-borrow result is not the bounded nominal lifetime carrier");
    };
    if *base_symbol != fact.result_data {
        return unsupported("retained-borrow result nominal owner does not replay");
    }
    if lifetime_arguments.as_slice() != [fact.lifetime.clone()]
        || fact.result_lifetime_argument_ordinal != 0
    {
        return unsupported("retained-borrow result lifetime slot does not replay");
    }
    if !checked
        .type_reference_table
        .type_reference_handles(*arguments)
        .is_empty()
    {
        return unsupported("retained-borrow result has runtime generic arguments");
    }
    if checked.type_multiplicity(signature.return_type) != Multiplicity::Linear {
        return unsupported("retained-borrow result is not linear");
    }
    if fact.retained_semantic_domain != fact.result_projection.semantic_domain
        || fact.source_projection.algebra != fact.result_projection.algebra
    {
        return unsupported("retained-borrow result projection does not replay");
    }

    let source_identity = parameter.name.as_str().to_owned();
    let source_projection = lower_projection(checked, &fact.source_projection)?;
    let result_projection = lower_projection(checked, &fact.result_projection)?;
    Ok(terminal_psi::RetainedBorrowCustody {
        callable_identity,
        source: terminal_psi::RetainedBorrowPlace {
            version: ContentPlaceVersion::Entry,
            root: terminal_psi::RetainedBorrowPlaceRoot::Parameter {
                position: *position,
                identity: source_identity,
                is_self: false,
            },
            segments: Vec::new(),
        },
        result: terminal_psi::RetainedBorrowPlace {
            version: ContentPlaceVersion::Current,
            root: terminal_psi::RetainedBorrowPlaceRoot::Result,
            segments: Vec::new(),
        },
        access: StructuralAccess::SharedBorrow,
        callable_lifetime_parameter_count,
        callable_lifetime_parameter_ordinal: fact.callable_lifetime_parameter_ordinal,
        result_nominal_identity: fact.result_projection.carrier_identity.clone(),
        result_multiplicity: StructuralMultiplicity::Linear,
        result_lifetime_argument_count: 1,
        result_lifetime_argument_ordinal: fact.result_lifetime_argument_ordinal,
        result_lifetime_slot_is_erased: true,
        retained_semantic_domain: result_projection.semantic_domain,
        source_projection,
        result_projection,
    })
}

pub(crate) fn retain_foreign_borrow_custodies(
    checked: &CheckedTrees,
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    let mut rows = checked
        .facts
        .qualifications
        .content
        .retained_borrow_custodies
        .iter()
        .map(|fact| lower_custody(checked, fact))
        .collect::<Result<Vec<_>, _>>()?;
    rows.sort_by(|left, right| left.callable_identity.cmp(&right.callable_identity));
    if rows
        .windows(2)
        .any(|pair| pair[0].callable_identity == pair[1].callable_identity)
    {
        return unsupported("retained-borrow callable has multiple custody rows");
    }
    let mut next_boundary = module
        .boundary_machines
        .iter()
        .map(|boundary| boundary.id.get())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "retained-borrow boundary identity space is exhausted",
        ))?;
    for row in rows {
        if let Some(boundary) = module
            .boundary_machines
            .iter_mut()
            .find(|boundary| boundary.identity == row.callable_identity)
        {
            boundary
                .content_guarantees
                .push(BoundaryContentGuarantee::RetainedBorrow(row));
            boundary.content_guarantees.sort();
            continue;
        }
        let id = BoundaryMachineId::new(next_boundary).ok_or(LoweringError::Unsupported(
            "retained-borrow boundary identity is invalid",
        ))?;
        next_boundary = next_boundary
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "retained-borrow boundary identity space is exhausted",
            ))?;
        module.boundary_machines.push(BoundaryMachineDeclaration {
            parameter_order: Vec::new(),
            fixed_service_reach: Vec::new(),
            id,
            identity: row.callable_identity.clone(),
            attachment: None,
            scalar_parameters: Vec::new(),
            crash_routes: Vec::new(),
            structural_parameters: Vec::new(),
            result: BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: vec![BoundaryContentGuarantee::RetainedBorrow(row)],
            published_service_ceiling: Vec::new(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use semantic_vocabulary::{
        BlockId, BoundaryMachineId, ClaimId, ContractId, EdgeId, MachineId, OperationId, PlaceId,
        StructuralDomainId, StructuralPlaceKind, StructuralTypeId,
    };
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
    use terminal_psi::{
        Block, BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind,
        BoundaryStructuralResultDeclaration, CompletionReceipt, EntryClaim, MachineContract,
        Operation, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
        StructuralDomainDeclaration, StructuralDomainRequirement, StructuralMultiplicity,
        StructuralOperationResult, StructuralParameterDeclaration, StructuralPlaceDeclaration,
        StructuralResultClaimBinding, StructuralResultDeclaration, StructuralTypeDeclaration,
        StructuralTypeShape, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
        VocabularyMarker,
    };

    use super::{lower_custody, retain_foreign_borrow_custodies};

    fn checked(source: &str) -> checked_trees::CheckedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        typed_trees_to_checked_trees::lower_typed_trees(
            typed,
            &typed_trees_to_checked_trees::CheckingRequest::settled(),
        )
        .expect("check")
    }

    const RETAINED_BORROW_PROGRAM: &str = r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> { machine project(subject: &Self) -> A; }

        data Buffer [linear] {}
        domain Buffer::Owned;
        machine Owned::content(buffer: &Buffer) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        { CountedQuantity { magnitude: 1 } }

        data PendingRead<'storage> [linear] {}
        domain PendingRead::Retained
        established by Reader::submit;
        machine Retained::content(pending: &PendingRead) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        { CountedQuantity { magnitude: 1 } }

        boundary trait Reader {
            machine submit<'storage>(
                buffer: &'storage Buffer in Buffer::Owned
            ) -> PendingRead<'storage>
            ensures
                result in PendingRead::Retained;
        }

        data Main {}
        machine Main::main(&mut self) {}
    "#;

    /// The declared but never-invoked `Reader::submit` custody row, lowered
    /// exactly as a checked module would carry it.
    fn retained_custody() -> terminal_psi::RetainedBorrowCustody {
        let checked = checked(RETAINED_BORROW_PROGRAM);
        let [fact] = checked
            .facts
            .qualifications
            .content
            .retained_borrow_custodies
            .as_slice()
        else {
            panic!("fixture records exactly one retained-borrow custody fact");
        };
        lower_custody(&checked, fact).expect("custody row lowers")
    }

    /// A module whose caller invokes the retained-borrow boundary, then
    /// consumes the produced occurrence and the still-owned loan source. The
    /// boundary declaration omits the guarantee so retention must merge it.
    fn invoked_module(custody: &terminal_psi::RetainedBorrowCustody) -> TerminalModule {
        let buffer = StructuralTypeId::new(1).unwrap();
        let pending = StructuralTypeId::new(2).unwrap();
        let owned = StructuralDomainId::new(1).unwrap();
        let retained = StructuralDomainId::new(2).unwrap();
        let submit = BoundaryMachineId::new(1).unwrap();
        let reclaim = BoundaryMachineId::new(2).unwrap();
        let buffer_place = PlaceId::new(1).unwrap();
        let pending_place = PlaceId::new(2).unwrap();
        let restored_place = PlaceId::new(3).unwrap();
        let result_place = PlaceId::new(4).unwrap();
        let loan = ClaimId::new(1).unwrap();
        let boundary_parameter =
            |position, place, ty, multiplicity, access, domain| StructuralParameterDeclaration {
                place,
                position,
                is_self: false,
                structural_type: ty,
                multiplicity,
                access,
                qualifications: vec![domain],
                projected_qualifications: Vec::new(),
            };
        TerminalModule {
            scalar_qualifications: Default::default(),
            scalar_block_invariants: Vec::new(),
            operation_crash_contracts: Vec::new(),
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: MachineId::new(1).unwrap(),
            structural_types: vec![
                StructuralTypeDeclaration {
                    id: buffer,
                    identity: custody.source_projection.carrier_identity.clone(),
                    shape: StructuralTypeShape::Record { fields: Vec::new() },
                },
                StructuralTypeDeclaration {
                    id: pending,
                    identity: custody.result_nominal_identity.clone(),
                    shape: StructuralTypeShape::Record { fields: Vec::new() },
                },
            ],
            structural_domains: vec![
                StructuralDomainDeclaration {
                    id: owned,
                    semantic_domain: custody.source_projection.semantic_domain,
                    identity: "Buffer::Owned".into(),
                    carrier: buffer,
                    content_projection: Some(custody.source_projection.projection.clone()),
                    establishment_routes: Vec::new(),
                },
                StructuralDomainDeclaration {
                    id: retained,
                    semantic_domain: custody.retained_semantic_domain,
                    identity: "PendingRead::Retained".into(),
                    carrier: pending,
                    content_projection: Some(custody.result_projection.projection.clone()),
                    establishment_routes: Vec::new(),
                },
            ],
            services: Vec::new(),
            root_service_reach: Default::default(),
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: vec![
                BoundaryMachineDeclaration {
                    id: submit,
                    identity: custody.callable_identity.clone(),
                    attachment: None,
                    parameter_order: vec![BoundaryParameterKind::Structural],
                    scalar_parameters: Vec::new(),
                    crash_routes: Vec::new(),
                    structural_parameters: vec![boundary_parameter(
                        0,
                        PlaceId::new(10).unwrap(),
                        buffer,
                        StructuralMultiplicity::Unrestricted,
                        StructuralAccess::SharedBorrow,
                        owned,
                    )],
                    result: BoundaryMachineResult::Structural(
                        BoundaryStructuralResultDeclaration {
                            structural_type: pending,
                            multiplicity: StructuralMultiplicity::Linear,
                            qualifications: vec![retained],
                        },
                    ),
                    requires: vec![StructuralDomainRequirement {
                        argument_index: 0,
                        domain: owned,
                    }],
                    program_local_root_introductions: Vec::new(),
                    content_guarantees: Vec::new(),
                    fixed_service_reach: Vec::new(),
                    published_service_ceiling: Vec::new(),
                },
                BoundaryMachineDeclaration {
                    id: reclaim,
                    identity: "Reader::reclaim".to_owned(),
                    attachment: None,
                    parameter_order: vec![BoundaryParameterKind::Structural],
                    scalar_parameters: Vec::new(),
                    crash_routes: Vec::new(),
                    structural_parameters: vec![boundary_parameter(
                        0,
                        PlaceId::new(11).unwrap(),
                        pending,
                        StructuralMultiplicity::Linear,
                        StructuralAccess::Owned,
                        retained,
                    )],
                    result: BoundaryMachineResult::Structural(
                        BoundaryStructuralResultDeclaration {
                            structural_type: buffer,
                            multiplicity: StructuralMultiplicity::Linear,
                            qualifications: vec![owned],
                        },
                    ),
                    requires: Vec::new(),
                    program_local_root_introductions: Vec::new(),
                    content_guarantees: Vec::new(),
                    fixed_service_reach: Vec::new(),
                    published_service_ceiling: Vec::new(),
                },
            ],
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
                id: MachineId::new(1).unwrap(),
                attachment: None,
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: buffer_place,
                    position: 0,
                    is_self: false,
                    structural_type: buffer,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![owned],
                    projected_qualifications: Vec::new(),
                }],
                ranked_scc: None,
                entry_claims: vec![EntryClaim {
                    claim: loan,
                    input: buffer_place,
                    path: Vec::new(),
                }],
                published_service_ceiling: Vec::new(),
                parameters: Vec::new(),
                result: TerminalMachineResult::Structural(StructuralResultDeclaration {
                    place: result_place,
                    structural_type: buffer,
                    multiplicity: StructuralMultiplicity::Linear,
                    qualifications: vec![owned],
                    projected_qualifications: Vec::new(),
                    reference_sources: Vec::new(),
                }),
                structural_places: vec![
                    StructuralPlaceDeclaration {
                        id: buffer_place,
                        kind: StructuralPlaceKind::Parameter {
                            position: 0,
                            is_self: false,
                        },
                    },
                    StructuralPlaceDeclaration {
                        id: pending_place,
                        kind: StructuralPlaceKind::OperationResult {
                            producer: OperationId::new(1).unwrap(),
                            structural_type: pending,
                        },
                    },
                    StructuralPlaceDeclaration {
                        id: restored_place,
                        kind: StructuralPlaceKind::OperationResult {
                            producer: OperationId::new(2).unwrap(),
                            structural_type: buffer,
                        },
                    },
                    StructuralPlaceDeclaration {
                        id: result_place,
                        kind: StructuralPlaceKind::Result,
                    },
                ],
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: BlockId::new(1).unwrap(),
                blocks: vec![Block {
                    erased_proof_formals: Vec::new(),
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    parameters: Vec::new(),
                    id: BlockId::new(1).unwrap(),
                    operations: vec![
                        Operation {
                            static_reach_binding: None,
                            suspension_crossing: None,
                            id: OperationId::new(1).unwrap(),
                            result: OperationResult::Structural(StructuralOperationResult {
                                place: pending_place,
                                structural_type: pending,
                                multiplicity: StructuralMultiplicity::Linear,
                                qualifications: vec![retained],
                                projected_qualifications: Vec::new(),
                                qualification_establishments: Vec::new(),
                                claims: vec![StructuralResultClaimBinding {
                                    claim: loan,
                                    path: Vec::new(),
                                }],
                            }),
                            kind: OperationKind::BoundaryCall {
                                boundary: submit,
                                arguments: Vec::new(),
                                structural_arguments: vec![StructuralArgument {
                                    place: buffer_place,
                                    path: Vec::new(),
                                    access: StructuralAccess::SharedBorrow,
                                }],
                                completion_receipts: vec![CompletionReceipt {
                                    claim: loan,
                                    argument_index: 0,
                                }],
                            },
                        },
                        Operation {
                            static_reach_binding: None,
                            suspension_crossing: None,
                            id: OperationId::new(2).unwrap(),
                            result: OperationResult::Structural(StructuralOperationResult {
                                place: restored_place,
                                structural_type: buffer,
                                multiplicity: StructuralMultiplicity::Linear,
                                qualifications: vec![owned],
                                projected_qualifications: Vec::new(),
                                qualification_establishments: Vec::new(),
                                claims: vec![StructuralResultClaimBinding {
                                    claim: loan,
                                    path: Vec::new(),
                                }],
                            }),
                            kind: OperationKind::BoundaryCall {
                                boundary: reclaim,
                                arguments: Vec::new(),
                                structural_arguments: vec![StructuralArgument {
                                    place: pending_place,
                                    path: Vec::new(),
                                    access: StructuralAccess::Owned,
                                }],
                                completion_receipts: vec![CompletionReceipt {
                                    claim: loan,
                                    argument_index: 0,
                                }],
                            },
                        },
                    ],
                    terminator: Terminator::ReturnStructural {
                        edge: EdgeId::new(1).unwrap(),
                        source: restored_place,
                        returned_claims: vec![loan],
                        trivial_affine_discards: vec![buffer_place],
                    },
                }],
                contract: MachineContract {
                    erased_proof_formals: Vec::new(),
                    erased_scalar_formals: Vec::new(),
                    id: ContractId::new(1).unwrap(),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        }
    }

    #[test]
    fn invoked_retained_borrow_boundary_carries_and_verifies() {
        let custody = retained_custody();
        let mut module = invoked_module(&custody);
        retain_foreign_borrow_custodies(&checked(RETAINED_BORROW_PROGRAM), &mut module)
            .expect("the invoked callable's authored declaration carries the custody row");
        let [terminal_psi::BoundaryContentGuarantee::RetainedBorrow(carried)] =
            module.boundary_machines[0].content_guarantees.as_slice()
        else {
            panic!("the custody row merged onto the authored boundary declaration");
        };
        assert_eq!(*carried, custody);
        terminal_verifier::validate_module(&module)
            .expect("an invoked retained-borrow call keeps the caller's loan on the result");
    }

    #[test]
    fn retained_borrow_call_rejects_a_moved_source() {
        let custody = retained_custody();
        let mut module = invoked_module(&custody);
        module.boundary_machines[0].content_guarantees.push(
            terminal_psi::BoundaryContentGuarantee::RetainedBorrow(custody),
        );
        let OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        structural_arguments[0].access = StructuralAccess::Owned;
        assert!(matches!(
            terminal_verifier::validate_module(&module),
            Err(terminal_verifier::ModuleError::InvalidRetainedBorrowBoundaryCall {
                operation,
                boundary,
            }) if operation == OperationId::new(1).unwrap()
                && boundary == BoundaryMachineId::new(1).unwrap()
        ));
    }

    #[test]
    fn retained_borrow_call_requires_the_loan_bound_to_the_result() {
        let custody = retained_custody();
        let mut module = invoked_module(&custody);
        module.boundary_machines[0].content_guarantees.push(
            terminal_psi::BoundaryContentGuarantee::RetainedBorrow(custody),
        );
        let OperationResult::Structural(result) =
            &mut module.machines[0].blocks[0].operations[0].result
        else {
            unreachable!()
        };
        result.claims.clear();
        assert!(matches!(
            terminal_verifier::validate_module(&module),
            Err(terminal_verifier::ModuleError::InvalidRetainedBorrowBoundaryCall {
                operation,
                boundary,
            }) if operation == OperationId::new(1).unwrap()
                && boundary == BoundaryMachineId::new(1).unwrap()
        ));
    }
}
