//! Canonical structural-domain, claim, and contract encoding.

use super::proposition_encoding::encode_proposition;
use super::*;
use psi_terminal::BoundaryMachineResult;

pub(super) fn encode_place_declaration(
    bytes: &mut CanonicalBytes,
    place: StructuralPlaceDeclaration,
) {
    bytes.id(place.id);
    match place.kind {
        StructuralPlaceKind::Parameter { position, is_self } => {
            bytes.u8(1);
            bytes.u32(position);
            bytes.boolean(is_self);
        }
        StructuralPlaceKind::Result => bytes.u8(2),
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } => {
            bytes.u8(3);
            bytes.id(producer);
            bytes.id(structural_type);
        }
        StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal,
            structural_type,
        } => {
            bytes.u8(4);
            bytes.u32(declaration_ordinal);
            bytes.id(structural_type);
        }
        StructuralPlaceKind::ProviderAttachment {
            attachment,
            field,
            boundary,
        } => {
            bytes.u8(5);
            bytes.id(attachment);
            bytes.id(field);
            bytes.id(boundary);
        }
        StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal,
            structural_type,
            construction,
        } => {
            bytes.u8(if construction.is_some() { 7 } else { 6 });
            bytes.u32(declaration_ordinal);
            bytes.id(structural_type);
            if let Some(construction) = construction {
                bytes.id(construction.root_structural_type);
                bytes.u64(construction.index);
            }
        }
    }
}

pub(super) fn encode_boundary_machine(
    bytes: &mut CanonicalBytes,
    declaration: &BoundaryMachineDeclaration,
) {
    bytes.id(declaration.id);
    bytes.string(&declaration.identity);
    encode_optional(
        bytes,
        declaration.attachment.as_ref(),
        |bytes, attachment| bytes.id(*attachment),
    );
    bytes.slice(&declaration.scalar_parameters, |bytes, parameter| {
        encode_scalar_type(bytes, *parameter)
    });
    bytes.slice(
        &declaration.structural_parameters,
        encode_structural_parameter,
    );
    match &declaration.result {
        BoundaryMachineResult::Unit => bytes.u8(0),
        BoundaryMachineResult::Scalar(result) => {
            bytes.u8(1);
            encode_scalar_type(bytes, *result);
        }
        BoundaryMachineResult::Structural(result) => {
            bytes.u8(2);
            bytes.id(result.structural_type);
            bytes.u8(match result.multiplicity {
                StructuralMultiplicity::Unrestricted => 1,
                StructuralMultiplicity::Affine => 2,
                StructuralMultiplicity::Linear => 3,
            });
            encode_ids(bytes, &result.qualifications);
        }
    }
    bytes.slice(&declaration.requires, encode_domain_requirement);
    bytes.slice(
        &declaration.program_local_root_introductions,
        encode_program_local_root_introduction,
    );
    bytes.slice(
        &declaration.content_guarantees,
        encode_boundary_content_guarantee,
    );
    encode_ids(bytes, &declaration.published_service_ceiling);
}

fn encode_boundary_content_guarantee(
    bytes: &mut CanonicalBytes,
    guarantee: &BoundaryContentGuarantee,
) {
    match guarantee {
        BoundaryContentGuarantee::Conservation(guarantee) => {
            bytes.u8(1);
            encode_content_conservation_guarantee(bytes, guarantee);
        }
        BoundaryContentGuarantee::RetainedBorrow(custody) => {
            bytes.u8(2);
            encode_retained_borrow_custody(bytes, custody);
        }
    }
}

fn encode_retained_borrow_place(bytes: &mut CanonicalBytes, place: &RetainedBorrowPlace) {
    bytes.u8(match place.version {
        ContentPlaceVersion::Entry => 1,
        ContentPlaceVersion::Current => 2,
    });
    match &place.root {
        RetainedBorrowPlaceRoot::Parameter {
            position,
            identity,
            is_self,
        } => {
            bytes.u8(1);
            bytes.u32(*position);
            bytes.string(identity);
            bytes.boolean(*is_self);
        }
        RetainedBorrowPlaceRoot::Result => bytes.u8(2),
    }
    bytes.slice(&place.segments, |bytes, segment| match segment {
        ContentPlaceSegment::Case(identity) => {
            bytes.u8(1);
            bytes.string(identity);
        }
        ContentPlaceSegment::Field(identity) => {
            bytes.u8(2);
            bytes.string(identity);
        }
        ContentPlaceSegment::FixedIndex(index) => {
            bytes.u8(3);
            bytes.u64(*index);
        }
    });
}

fn encode_retained_borrow_projection(
    bytes: &mut CanonicalBytes,
    projection: &RetainedBorrowContentProjection,
) {
    bytes.id(projection.semantic_domain);
    bytes.string(&projection.carrier_identity);
    bytes.id(projection.projection.identity.domain);
    bytes.u64(projection.projection.identity.projection_report_fingerprint);
    encode_content_algebra(bytes, &projection.projection.algebra);
    encode_content_projection_expression(bytes, &projection.projection.expression);
}

fn encode_retained_borrow_custody(bytes: &mut CanonicalBytes, custody: &RetainedBorrowCustody) {
    bytes.string(&custody.callable_identity);
    encode_retained_borrow_place(bytes, &custody.source);
    encode_retained_borrow_place(bytes, &custody.result);
    bytes.u8(match custody.access {
        StructuralAccess::Owned => 1,
        StructuralAccess::SharedBorrow => 2,
        StructuralAccess::MutableBorrow => 3,
        StructuralAccess::WriteOnlyBorrow => 4,
    });
    bytes.u32(custody.callable_lifetime_parameter_count);
    bytes.u32(custody.callable_lifetime_parameter_ordinal);
    bytes.string(&custody.result_nominal_identity);
    encode_multiplicity(bytes, custody.result_multiplicity);
    bytes.u32(custody.result_lifetime_argument_count);
    bytes.u32(custody.result_lifetime_argument_ordinal);
    bytes.boolean(custody.result_lifetime_slot_is_erased);
    bytes.id(custody.retained_semantic_domain);
    encode_retained_borrow_projection(bytes, &custody.source_projection);
    encode_retained_borrow_projection(bytes, &custody.result_projection);
}

pub(super) fn encode_domain_requirement(
    bytes: &mut CanonicalBytes,
    requirement: &StructuralDomainRequirement,
) {
    bytes.u32(requirement.argument_index);
    bytes.id(requirement.domain);
}

pub(super) fn encode_program_local_root_introduction(
    bytes: &mut CanonicalBytes,
    schema: &ProgramLocalRootIntroductionSchema,
) {
    bytes.u32(schema.argument_index);
    bytes.u32(schema.source_parameter_position);
    bytes.id(schema.qualification);
    bytes.id(schema.carrier);
    bytes.id(schema.projection.domain);
    bytes.u64(schema.projection.projection_report_fingerprint);
    encode_content_algebra(bytes, &schema.algebra);
    encode_content_projection_expression(bytes, &schema.capacity);
    bytes.u64(schema.compatibility_report_identity);
}

pub(super) fn encode_content_projection_expression(
    bytes: &mut CanonicalBytes,
    expression: &ContentProjectionExpression,
) {
    match expression {
        ContentProjectionExpression::IntervalSet(members) => {
            bytes.u8(1);
            bytes.len(members.len());
            for (start, end) in members {
                encode_content_projection_scalar(bytes, start);
                encode_content_projection_scalar(bytes, end);
            }
        }
        ContentProjectionExpression::CountedQuantity(magnitude) => {
            bytes.u8(2);
            encode_content_projection_scalar(bytes, magnitude);
        }
    }
}

pub(super) fn encode_content_projection_scalar(
    bytes: &mut CanonicalBytes,
    scalar: &ContentProjectionScalar,
) {
    // Content expressions may be intentionally deep. Encode their canonical
    // prefix form iteratively so retaining the verifier-owned domain catalog
    // does not turn semantic nesting depth into native thread-stack usage.
    let mut pending = vec![scalar];
    while let Some(scalar) = pending.pop() {
        match scalar {
            ContentProjectionScalar::SubjectField(path)
            | ContentProjectionScalar::RuntimeScalarEmbedding(path) => {
                bytes.u8(
                    if matches!(scalar, ContentProjectionScalar::SubjectField(_)) {
                        1
                    } else {
                        2
                    },
                );
                bytes.slice(path, |bytes, segment| bytes.string(segment));
            }
            ContentProjectionScalar::Natural(value) => {
                bytes.u8(3);
                bytes.string(value);
            }
            ContentProjectionScalar::Successor(inner) => {
                bytes.u8(4);
                pending.push(inner);
            }
            ContentProjectionScalar::Add(left, right)
            | ContentProjectionScalar::Subtract(left, right)
            | ContentProjectionScalar::Multiply(left, right) => {
                bytes.u8(match scalar {
                    ContentProjectionScalar::Add(_, _) => 5,
                    ContentProjectionScalar::Subtract(_, _) => 6,
                    ContentProjectionScalar::Multiply(_, _) => 7,
                    _ => unreachable!(),
                });
                pending.push(right);
                pending.push(left);
            }
        }
    }
}

pub(super) fn encode_content_conservation_guarantee(
    bytes: &mut CanonicalBytes,
    guarantee: &ContentConservationGuarantee,
) {
    bytes.u64(guarantee.report_fingerprint);
    bytes.slice(&guarantee.structural_places, |bytes, place| {
        encode_place_declaration(bytes, *place)
    });
    encode_content_conservation(bytes, &guarantee.conservation);
}

pub(super) fn encode_content_conservation(
    bytes: &mut CanonicalBytes,
    conservation: &ContentConservation,
) {
    encode_content_algebra(bytes, conservation.algebra());
    encode_content_term(bytes, conservation.left());
    encode_content_term(bytes, conservation.right());
}

pub(super) fn encode_provider_candidate(
    bytes: &mut CanonicalBytes,
    candidate: &ProviderCandidateConformance,
) {
    bytes.id(candidate.boundary);
    bytes.string(&candidate.requirement_identity);
    bytes.string(&candidate.provider_identity);
    bytes.string(&candidate.candidate_identity);
    bytes.id(candidate.candidate);
    bytes.slice(&candidate.signature.parameters, |bytes, parameter| {
        bytes.u32(parameter.position);
        bytes.boolean(parameter.is_self);
        bytes.id(parameter.structural_type);
        encode_multiplicity(bytes, parameter.multiplicity);
        encode_ids(bytes, &parameter.qualifications);
        bytes.slice(
            &parameter.projected_qualifications,
            |bytes, qualification| {
                bytes.slice(&qualification.path, encode_structural_path_segment);
                bytes.id(qualification.domain);
            },
        );
    });
    bytes.slice(
        &candidate.refinement.positional_parameters,
        |bytes, parameter| {
            bytes.u32(parameter.boundary_index);
            bytes.u32(parameter.candidate_index);
        },
    );
    bytes.slice(
        &candidate.refinement.required_domains,
        encode_domain_requirement,
    );
    encode_ids(bytes, &candidate.refinement.realized_service_ceiling);
}

pub(super) fn encode_structural_type(
    bytes: &mut CanonicalBytes,
    declaration: &StructuralTypeDeclaration,
) {
    bytes.id(declaration.id);
    bytes.string(&declaration.identity);
    match &declaration.shape {
        StructuralTypeShape::PrimitiveScalar(scalar_type) => {
            bytes.u8(6);
            encode_scalar_type(bytes, *scalar_type);
        }
        StructuralTypeShape::ByteSequence(carrier) => {
            bytes.u8(1);
            encode_byte_carrier(bytes, *carrier);
        }
        StructuralTypeShape::Record { fields } => {
            bytes.u8(2);
            bytes.slice(fields, encode_structural_field);
        }
        StructuralTypeShape::FixedArray { element, length } => {
            bytes.u8(3);
            bytes.id(*element);
            bytes.u64(*length);
        }
        StructuralTypeShape::Sum { cases } => {
            bytes.u8(4);
            bytes.len(cases.len());
            for case in cases {
                bytes.id(case.id);
                bytes.string(&case.identity);
                bytes.slice(&case.fields, encode_structural_field);
            }
        }
        StructuralTypeShape::Mixed { fields, cases } => {
            bytes.u8(5);
            bytes.slice(fields, encode_structural_field);
            bytes.len(cases.len());
            for case in cases {
                bytes.id(case.id);
                bytes.string(&case.identity);
                bytes.slice(&case.fields, encode_structural_field);
            }
        }
    }
}

pub(super) fn encode_structural_domain(
    bytes: &mut CanonicalBytes,
    declaration: &StructuralDomainDeclaration,
) {
    bytes.id(declaration.id);
    bytes.id(declaration.semantic_domain);
    bytes.string(&declaration.identity);
    bytes.id(declaration.carrier);
    encode_optional(
        bytes,
        declaration.content_projection.as_ref(),
        |bytes, projection| {
            bytes.id(projection.identity.domain);
            bytes.u64(projection.identity.projection_report_fingerprint);
            encode_content_algebra(bytes, &projection.algebra);
            encode_content_projection_expression(bytes, &projection.expression);
        },
    );
}

pub(super) fn encode_structural_field(
    bytes: &mut CanonicalBytes,
    field: &StructuralFieldDeclaration,
) {
    bytes.id(field.id);
    bytes.string(&field.identity);
    bytes.u8(match field.relevance {
        BindingRelevance::Relevant => 1,
        BindingRelevance::Erased => 2,
    });
    match &field.field_type {
        StructuralFieldType::Scalar(value) => {
            bytes.u8(1);
            encode_scalar_type(bytes, *value);
        }
        StructuralFieldType::IeeeFloat(value) => {
            bytes.u8(2);
            encode_float_format(bytes, *value);
        }
        StructuralFieldType::ByteSequence(value) => {
            bytes.u8(3);
            encode_byte_carrier(bytes, *value);
        }
        StructuralFieldType::Structural(value) => {
            bytes.u8(4);
            bytes.id(*value);
        }
        StructuralFieldType::Erased { type_identity } => {
            bytes.u8(5);
            bytes.string(type_identity);
        }
    }
}

pub(super) fn encode_byte_carrier(bytes: &mut CanonicalBytes, carrier: ByteSequenceCarrier) {
    match carrier {
        ByteSequenceCarrier::BorrowedView => bytes.u8(1),
        ByteSequenceCarrier::BoundedOwned { capacity } => {
            bytes.u8(2);
            bytes.u64(capacity);
        }
    }
}

pub(super) fn encode_structural_operation_result(
    bytes: &mut CanonicalBytes,
    result: &StructuralOperationResult,
) {
    bytes.id(result.place);
    bytes.id(result.structural_type);
    encode_multiplicity(bytes, result.multiplicity);
    encode_ids(bytes, &result.qualifications);
    encode_projected_qualification_roster(bytes, &result.projected_qualifications);
    bytes.len(result.claims.len());
    for claim in &result.claims {
        bytes.id(claim.claim);
        bytes.slice(&claim.path, encode_structural_path_segment);
    }
}

pub(super) fn encode_projected_qualification_roster(
    bytes: &mut CanonicalBytes,
    qualifications: &[psi_terminal::StructuralPathQualification],
) {
    bytes.slice(qualifications, |bytes, qualification| {
        bytes.slice(&qualification.path, encode_structural_path_segment);
        bytes.id(qualification.domain);
    });
}

pub(super) fn encode_completion_claim_source(
    bytes: &mut CanonicalBytes,
    source: &CompletionClaimSource,
) {
    bytes.id(source.claim);
    encode_optional(bytes, source.entry.as_ref(), encode_entry_claim);
    encode_optional(bytes, source.content.as_ref(), encode_content_entry_claim);
}

pub(super) fn encode_entry_claim(bytes: &mut CanonicalBytes, claim: &EntryClaim) {
    bytes.id(claim.claim);
    bytes.id(claim.input);
    bytes.slice(&claim.path, encode_structural_path_segment);
}

pub(super) fn encode_content_entry_claim(
    bytes: &mut CanonicalBytes,
    claim: &psi_terminal::ContentEntryClaim,
) {
    bytes.id(claim.claim);
    encode_content_place(bytes, &claim.input);
    bytes.slice(&claim.projections, encode_claim_projection);
}

pub(super) fn encode_claim_projection(
    bytes: &mut CanonicalBytes,
    projection: &ClaimContentProjection,
) {
    bytes.id(projection.projection.domain);
    bytes.u64(projection.projection.projection_report_fingerprint);
    encode_content_algebra(bytes, &projection.algebra);
}

pub(super) fn encode_content_algebra(bytes: &mut CanonicalBytes, algebra: &ContentAlgebra) {
    bytes.u8(match algebra.kind {
        ContentAlgebraKind::IntervalSet => 1,
        ContentAlgebraKind::CountedQuantity => 2,
    });
    bytes.string(&algebra.parameter);
}

pub(super) fn encode_content_place(bytes: &mut CanonicalBytes, place: &ContentStructuralPlace) {
    bytes.u8(match place.version {
        ContentPlaceVersion::Entry => 1,
        ContentPlaceVersion::Current => 2,
    });
    bytes.id(place.root);
    bytes.len(place.segments.len());
    for segment in &place.segments {
        match segment {
            ContentPlaceSegment::Field(value) => {
                bytes.u8(1);
                bytes.string(value);
            }
            ContentPlaceSegment::FixedIndex(value) => {
                bytes.u8(2);
                bytes.u64(*value);
            }
            ContentPlaceSegment::Case(value) => {
                bytes.u8(3);
                bytes.string(value);
            }
        }
    }
}

pub(super) fn encode_cleanup(bytes: &mut CanonicalBytes, action: &TerminalAffineCleanupAction) {
    match action {
        TerminalAffineCleanupAction::DiscardRoot(place) => {
            bytes.u8(1);
            bytes.id(*place);
        }
        TerminalAffineCleanupAction::DiscardResidual(discard) => {
            bytes.u8(2);
            bytes.id(discard.place);
            bytes.slice(&discard.path, encode_structural_path_segment);
            bytes.id(discard.structural_type);
        }
        TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
            bytes.u8(3);
            bytes.id(cleanup.place);
            bytes.id(cleanup.structural_type);
            bytes.id(cleanup.cleanup_machine);
            encode_optional(bytes, cleanup.cleanup_receiver.as_ref(), |bytes, place| {
                bytes.id(*place)
            });
            encode_ids(bytes, &cleanup.requirement_obligations);
        }
    }
}

pub(super) fn encode_crash_cause(bytes: &mut CanonicalBytes, cause: CrashCause) {
    bytes.u8(match cause {
        CrashCause::Trap => 1,
        CrashCause::Abort => 2,
    });
}

pub(super) fn encode_crash_predicate(bytes: &mut CanonicalBytes, predicate: &CrashPredicateTerm) {
    encode_proposition(bytes, predicate.proposition());
}

pub(super) fn encode_crash_route_bucket(bytes: &mut CanonicalBytes, bucket: &CrashRouteBucket) {
    encode_crash_cause(bytes, bucket.cause);
    bytes.slice(
        &bucket.alternatives,
        |bytes, alternative| match alternative {
            CrashRouteGuard::Truth => bytes.u8(1),
            CrashRouteGuard::Predicate(predicate) => {
                bytes.u8(2);
                encode_crash_predicate(bytes, predicate);
            }
        },
    );
}

pub(super) fn encode_evidence_interface(
    bytes: &mut CanonicalBytes,
    interface: &EvidenceInterfaceIdentity,
) {
    bytes.string(&interface.trait_identity);
    bytes.slice(&interface.arguments, |bytes, argument| {
        bytes.string(argument)
    });
    bytes.slice(&interface.requirements, |bytes, requirement| {
        bytes.string(&requirement.declaring_trait_identity);
        bytes.slice(&requirement.declaring_trait_arguments, |bytes, argument| {
            bytes.string(argument)
        });
        bytes.string(&requirement.requirement_identity);
    });
}

pub(super) fn encode_outcome_specific_call_evidence(
    bytes: &mut CanonicalBytes,
    evidence: &OutcomeSpecificCallEvidence,
) {
    bytes.id(evidence.guard.result_type);
    bytes.id(evidence.guard.result_case);
    bytes.u32(evidence.position);
    bytes.id(evidence.callee_obligation);
    bytes.id(evidence.callee_term);
    bytes.string(&evidence.output_field);
    bytes.id(evidence.callee_proposition);
    bytes.id(evidence.instantiated_proposition);
    bytes.id(evidence.output);
    encode_optional(
        bytes,
        evidence.result_substitution.as_ref(),
        |bytes, substitution| {
            bytes.u32(substitution.argument_position);
            bytes.id(substitution.callee_result);
            bytes.id(substitution.caller_result);
        },
    );
    bytes.id(evidence.validity.result);
    encode_ids(bytes, &evidence.validity.proposition_dependencies);
    encode_evidence_interface(bytes, &evidence.validity.evidence_interface);
    encode_ids(bytes, &evidence.validity.interface_dependencies);
}

pub(super) fn encode_machine_contract(bytes: &mut CanonicalBytes, contract: &MachineContract) {
    bytes.id(contract.id);
    bytes.slice(&contract.crash_routes, encode_crash_route_bucket);
    bytes.slice(&contract.requires, encode_proposition);
    bytes.slice(&contract.ensures, |bytes, clause| {
        bytes.id(clause.obligation);
        encode_proposition(bytes, &clause.proposition);
    });
    bytes.slice(&contract.outcome_specific_ensures, |bytes, row| {
        bytes.id(row.guard.result_type);
        bytes.id(row.guard.result_case);
        bytes.u32(row.position);
        bytes.id(row.obligation);
        encode_proposition(bytes, &row.proposition);
        encode_optional(bytes, row.evidence.as_ref(), |bytes, evidence| {
            bytes.id(evidence.term);
            bytes.string(&evidence.output_field);
        });
    });
}

pub(super) fn encode_evidence_contract_lane(
    bytes: &mut CanonicalBytes,
    lane: &EvidenceContractLane,
) {
    bytes.id(lane.machine);
    bytes.u8(match lane.kind {
        EvidenceContractLaneKind::Requires => 1,
        EvidenceContractLaneKind::Ensures => 2,
    });
    bytes.u32(lane.position);
    bytes.id(lane.term);
    encode_optional(bytes, lane.output_field.as_ref(), |bytes, output| {
        bytes.string(output)
    });
}
