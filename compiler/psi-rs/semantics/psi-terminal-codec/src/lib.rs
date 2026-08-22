#![forbid(unsafe_code)]

//! Canonical binary encoding and semantic identity for terminal Psi.
//!
//! Only the semantic module is encoded here. Proof bundles, installation
//! records, and debug/source maps have separate identities and can be replaced
//! without changing [`TerminalPsiIdentity`].

mod artifact_manifest;
mod debug_map;
mod proof_bundle;
mod trust_graph;

pub use artifact_manifest::{
    ArtifactManifestError, SectionFingerprint, TerminalArtifactIdentity, TerminalArtifactManifest,
    build_artifact_manifest, validate_artifact_manifest,
};
pub use debug_map::{
    DebugFileId, DebugMapError, DebugSite, DebugSourceDigest, DebugSourceFile, DebugSourceOrigin,
    DebugSourceSpan, DebugSubject, TerminalDebugMap, decode_debug_map, encode_debug_map,
    source_digest, validate_debug_map,
};
pub use proof_bundle::{
    ProofBundleFingerprint, ProofCodecError, decode_proof_bundle, encode_proof_bundle,
    proof_bundle_fingerprint, render_verified_proof_synopsis,
};
pub use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity};
pub use trust_graph::{
    TerminalTrustGraphIdentity, TrustAcceptingPolicy, TrustDependencyDigest, TrustDependencyKind,
    TrustDependencyNode, TrustDependencyStatus, TrustGraphError, ValidatedTerminalTrustGraph,
    current_rust_operation_semantics_trust_identity, current_terminal_trust_graph,
    render_terminal_trust_graph, validate_terminal_trust_graph,
};

use psi_core::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, ClaimId, ContentAlgebra,
    ContentAlgebraKind, ContentConservation, ContentDomainId, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace, ContentTerm,
    IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatStructuralField, IntegerCarrier,
    IntegerSign, IntegerType, IntegerValue, ObligationId, PlaceId, Proposition, PropositionError,
    PropositionId, PsiSemanticId, ScalarTerm, ScalarType, ServiceId, StructuralCaseSubject,
    StructuralPlaceKind, StructuralTypeId,
};
use psi_terminal::{
    BindingRelevance, Block, BoundaryMachineDeclaration, ByteSequenceCarrier,
    ClaimContentProjection, ClaimTransfer, ClosedConformanceApplication,
    ClosedConformanceParameterBinding, ClosedConformanceParameterKind, ClosedConformanceRow,
    CompletionReceipt, ContentEntryClaim, ContentIdentityReshuffle, ContentPartitionComposition,
    ContentPlaceSubstitution, ContractClause, CrashCause, CrashPredicateTerm, CrashRouteBucket,
    CrashRouteGuard, EntryClaim, EvidenceContractLane, EvidenceContractLaneKind,
    EvidenceInterfaceIdentity, EvidencePackageInvocation, EvidencePackageOutputBinding,
    EvidencePackageRuntimeCall, EvidenceTermDeclaration, FloatMeaningEqualityProposition,
    FloatMeaningProjection, FloatMeaningProjectionOperation, FloatProjectionInput,
    FloatProjectionInputId, MachineContract, NominalAffineCleanup, Operation, OperationKind,
    OperationResult, ProofOnlyValueType, ProofPropositionId, ProofValueDeclaration, ProofValueId,
    PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
    PropositionBinderArgumentKind, PropositionBinderDeclaration, PropositionBinderKind,
    PropositionDeclaration, PropositionEvidence, ProviderCandidateConformance,
    ProviderParameterRefinement, ProviderSignatureParameter, ProviderUnitRefinement,
    ProviderUnitSignature, ServiceDeclaration, StructuralAffineDiscard, StructuralArgument,
    StructuralCaseDeclaration, StructuralDomainDeclaration, StructuralDomainRequirement,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge,
    TerminalAffineCleanupAction, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration, VocabularyMarker,
};
use psi_terminal_verifier::{ModuleError, validate_module_representation};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const MAGIC: &[u8; 8] = b"PSITERM\0";
const FORMAT_MARKER: u16 = 20;
const FINGERPRINT_DOMAIN: &[u8] = b"psi-terminal-semantic-fingerprint\0";
const MAX_PROPOSITION_DEPTH: usize = 256;
const MAX_SCALAR_TERM_DEPTH: usize = 256;
const MAX_CONTENT_TERM_DEPTH: usize = 256;
const MAX_CONTENT_IDENTITY_BYTES: usize = 1 << 20;

pub fn encode_module(module: &TerminalModule) -> Result<Vec<u8>, CodecError> {
    validate_canonical_order(module)?;
    validate_structural_foundation(module)?;
    validate_module_representation(module).map_err(CodecError::InvalidModule)?;
    encode_raw(module)
}

pub fn decode_module(bytes: &[u8]) -> Result<TerminalModule, CodecError> {
    let mut reader = Reader::new(bytes);
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(CodecError::InvalidMagic);
    }
    let format_marker = reader.u16()?;
    if format_marker != FORMAT_MARKER {
        return Err(CodecError::UnsupportedFormatMarker(format_marker));
    }
    let module = decode_module_body(&mut reader)?;
    if reader.remaining() != 0 {
        return Err(CodecError::TrailingBytes(reader.remaining()));
    }
    validate_canonical_order(&module)?;
    validate_structural_foundation(&module)?;
    validate_module_representation(&module).map_err(CodecError::InvalidModule)?;
    if encode_raw(&module)? != bytes {
        return Err(CodecError::NonCanonicalEncoding);
    }
    Ok(module)
}

pub fn semantic_fingerprint(module: &TerminalModule) -> Result<SemanticFingerprint, CodecError> {
    let bytes = encode_module(module)?;
    Ok(fingerprint_bytes(&bytes))
}

pub fn terminal_psi_identity(module: &TerminalModule) -> Result<TerminalPsiIdentity, CodecError> {
    Ok(TerminalPsiIdentity {
        vocabulary_marker: module.vocabulary_marker,
        program_fingerprint: semantic_fingerprint(module)?,
    })
}

fn fingerprint_bytes(bytes: &[u8]) -> SemanticFingerprint {
    let mut digest = Sha256::new();
    digest.update(FINGERPRINT_DOMAIN);
    let byte_len =
        u64::try_from(bytes.len()).expect("terminal-Psi bytes fit the u64 digest domain");
    digest.update(byte_len.to_le_bytes());
    digest.update(bytes);
    SemanticFingerprint::from_bytes(digest.finalize().into())
}

fn validate_canonical_order(module: &TerminalModule) -> Result<(), CodecError> {
    if !strictly_increasing(
        module
            .structural_types
            .iter()
            .map(|declaration| declaration.id),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "structural types by StructuralTypeId",
        ));
    }
    if !strictly_increasing(
        module
            .evidence_package_invocations
            .iter()
            .map(|invocation| (invocation.caller, invocation.ordinal)),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "evidence package invocations by caller and ordinal",
        ));
    }
    if !strictly_increasing(
        module
            .closed_conformance_applications
            .iter()
            .map(|application| {
                (
                    application.owner,
                    application.declaration_identity.as_str(),
                    application.fingerprint,
                )
            }),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "closed conformance applications by owner, declaration, and fingerprint",
        ));
    }
    for invocation in &module.evidence_package_invocations {
        if invocation
            .outputs
            .iter()
            .enumerate()
            .any(|(position, output)| u32::try_from(position).ok() != Some(output.output_position))
        {
            return Err(CodecError::NonCanonicalOrder(
                "evidence package outputs by position",
            ));
        }
    }
    for declaration in &module.structural_types {
        match &declaration.shape {
            StructuralTypeShape::Record { fields }
                if !strictly_increasing(fields.iter().map(|field| field.id)) =>
            {
                return Err(CodecError::NonCanonicalOrder(
                    "structural fields by StructuralFieldId",
                ));
            }
            StructuralTypeShape::Sum { cases }
                if !strictly_increasing(cases.iter().map(|case| case.id)) =>
            {
                return Err(CodecError::NonCanonicalOrder(
                    "structural cases by StructuralCaseId",
                ));
            }
            StructuralTypeShape::Sum { cases }
                if cases
                    .iter()
                    .any(|case| !strictly_increasing(case.fields.iter().map(|field| field.id))) =>
            {
                return Err(CodecError::NonCanonicalOrder(
                    "structural case fields by StructuralFieldId",
                ));
            }
            _ => {}
        }
    }
    if !strictly_increasing(
        module
            .structural_domains
            .iter()
            .map(|declaration| declaration.id),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "structural domains by StructuralDomainId",
        ));
    }
    if !strictly_increasing(module.services.iter().map(|declaration| declaration.id)) {
        return Err(CodecError::NonCanonicalOrder("services by ServiceId"));
    }
    if module
        .services
        .iter()
        .any(|declaration| !strictly_increasing(declaration.parents.iter().copied()))
    {
        return Err(CodecError::NonCanonicalOrder(
            "service parents by ServiceId",
        ));
    }
    if module
        .float_meaning_equalities
        .iter()
        .enumerate()
        .any(|(index, proposition)| {
            let Ok(index) = u32::try_from(index) else {
                return true;
            };
            proposition.id != ProofPropositionId(index) || proposition.left > proposition.right
        })
    {
        return Err(CodecError::NonCanonicalOrder(
            "float-meaning equalities by dense proposition ID and ordered operands",
        ));
    }
    if !strictly_increasing(
        module
            .boundary_machines
            .iter()
            .map(|declaration| declaration.id),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "boundary machines by BoundaryMachineId",
        ));
    }
    for declaration in &module.boundary_machines {
        validate_parameter_order(&declaration.structural_parameters)?;
        if !strictly_increasing(declaration.requires.iter().copied()) {
            return Err(CodecError::NonCanonicalOrder(
                "boundary requirements by argument index and domain",
            ));
        }
        if !strictly_increasing(declaration.published_service_ceiling.iter().copied()) {
            return Err(CodecError::NonCanonicalOrder(
                "boundary published service ceiling by ServiceId",
            ));
        }
    }
    if !strictly_increasing(module.provider_candidates.iter().map(|candidate| {
        (
            candidate.boundary,
            candidate.provider_identity.as_str(),
            candidate.candidate_identity.as_str(),
            candidate.candidate,
        )
    })) {
        return Err(CodecError::NonCanonicalOrder(
            "provider candidates by exact conformance identity",
        ));
    }
    if module
        .float_meaning_projections
        .iter()
        .enumerate()
        .any(|(index, projection)| {
            let Ok(index) = u32::try_from(index) else {
                return true;
            };
            projection.result.id != ProofValueId(index)
                || projection.source.id != FloatProjectionInputId(index)
        })
    {
        return Err(CodecError::NonCanonicalOrder(
            "float-meaning projections by dense proof value and source IDs",
        ));
    }
    if !strictly_increasing(module.proposition_declarations.iter().cloned().map(
        |mut declaration| {
            declaration.id = PropositionId::new(1).expect("one is nonzero");
            declaration
        },
    )) {
        return Err(CodecError::NonCanonicalOrder(
            "proposition declarations by semantic identity",
        ));
    }
    if !strictly_increasing(
        module
            .proposition_declarations
            .iter()
            .map(|declaration| declaration.id),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "proposition declarations by PropositionId",
        ));
    }
    if !strictly_increasing(module.proposition_applications.iter().cloned().map(
        |mut application| {
            application.id = PropositionId::new(1).expect("one is nonzero");
            application
        },
    )) {
        return Err(CodecError::NonCanonicalOrder(
            "proposition applications by semantic identity",
        ));
    }
    if !strictly_increasing(
        module
            .proposition_applications
            .iter()
            .map(|application| application.id),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "proposition applications by PropositionId",
        ));
    }
    if !strictly_increasing(
        module
            .evidence_terms
            .iter()
            .map(|term| (term.proposition, &term.interface, term.id)),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "evidence terms by proposition and EvidenceTermId",
        ));
    }
    if !strictly_increasing(module.evidence_terms.iter().map(|term| term.id)) {
        return Err(CodecError::NonCanonicalOrder(
            "evidence terms by EvidenceTermId",
        ));
    }
    if !strictly_increasing(
        module
            .evidence_contract_lanes
            .iter()
            .map(|lane| (lane.machine, lane.kind, lane.position)),
    ) {
        return Err(CodecError::NonCanonicalOrder(
            "evidence contract lanes by machine, kind, and position",
        ));
    }
    if !strictly_increasing(module.machines.iter().map(|machine| machine.id)) {
        return Err(CodecError::NonCanonicalOrder("machines by MachineId"));
    }
    for machine in &module.machines {
        validate_parameter_order(&machine.structural_parameters)?;
        if !strictly_increasing(machine.entry_claims.iter().map(|claim| claim.claim)) {
            return Err(CodecError::NonCanonicalOrder("entry claims by ClaimId"));
        }
        if !strictly_increasing(machine.published_service_ceiling.iter().copied()) {
            return Err(CodecError::NonCanonicalOrder(
                "machine published service ceiling by ServiceId",
            ));
        }
        if !crash_routes_are_canonical(&machine.contract.crash_routes) {
            return Err(CodecError::NonCanonicalOrder("crash route buckets"));
        }
        validate_crash_route_predicates(&machine.contract.crash_routes)?;
        if !strictly_increasing(machine.blocks.iter().map(|block| block.id)) {
            return Err(CodecError::NonCanonicalOrder("blocks by BlockId"));
        }
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            let crash_continuations = match &operation.kind {
                OperationKind::Call {
                    crash_continuations,
                    ..
                }
                | OperationKind::CallUnit {
                    crash_continuations,
                    ..
                }
                | OperationKind::CallStructuralScalar {
                    crash_continuations,
                    ..
                } => Some(crash_continuations),
                _ => None,
            };
            if let Some(crash_continuations) = crash_continuations {
                if !crash_routes_are_canonical(crash_continuations) {
                    return Err(CodecError::NonCanonicalOrder(
                        "call crash continuation buckets",
                    ));
                }
                validate_crash_route_predicates(crash_continuations)?;
            }
            match &operation.kind {
                OperationKind::CallUnit {
                    claim_transfers, ..
                }
                | OperationKind::CallStructuralScalar {
                    claim_transfers, ..
                } if !strictly_increasing(claim_transfers.iter().copied()) => {
                    return Err(CodecError::NonCanonicalOrder(
                        "unit-call claim transfers by claim and argument index",
                    ));
                }
                OperationKind::BoundaryCall {
                    completion_receipts,
                    ..
                } if !strictly_increasing(completion_receipts.iter().copied()) => {
                    return Err(CodecError::NonCanonicalOrder(
                        "boundary claim settlements by claim and argument index",
                    ));
                }
                _ => {}
            }
        }
        for block in &machine.blocks {
            if let Terminator::Crash { site_guard, .. } = &block.terminator {
                for predicate in site_guard {
                    validate_canonical_proposition(predicate.proposition(), 0)?;
                }
            }
            if let Terminator::ReturnUnitPartialAffine {
                residual_affine_discards,
                ..
            } = &block.terminator
            {
                let mut places_and_paths = BTreeSet::new();
                if residual_affine_discards.iter().any(|discard| {
                    !places_and_paths.insert((discard.place, discard.path.as_slice()))
                }) {
                    return Err(CodecError::NonCanonicalOrder(
                        "partial affine residual discards are unique",
                    ));
                }
            }
        }
        if !strictly_increasing(machine.structural_places.iter().map(|place| place.id)) {
            return Err(CodecError::NonCanonicalOrder(
                "structural places by PlaceId",
            ));
        }
        if !strictly_increasing(
            machine
                .content_entry_claims
                .iter()
                .map(|binding| binding.claim),
        ) {
            return Err(CodecError::NonCanonicalOrder(
                "content entry claims by ClaimId",
            ));
        }
        if machine
            .content_entry_claims
            .iter()
            .any(|binding| !strictly_increasing(binding.projections.iter()))
        {
            return Err(CodecError::NonCanonicalOrder(
                "entry-claim content projections by identity and algebra",
            ));
        }
        if !strictly_increasing(
            machine
                .content_identity_reshuffles
                .iter()
                .map(|reshuffle| reshuffle.claim),
        ) {
            return Err(CodecError::NonCanonicalOrder(
                "content identity reshuffles by ClaimId",
            ));
        }
        if machine
            .content_identity_reshuffles
            .iter()
            .any(|reshuffle| !strictly_increasing(reshuffle.projections.iter()))
        {
            return Err(CodecError::NonCanonicalOrder(
                "claim content projections by identity and algebra",
            ));
        }
        if !strictly_increasing(machine.content_partition_compositions.iter()) {
            return Err(CodecError::NonCanonicalOrder(
                "content partition compositions",
            ));
        }
        for composition in &machine.content_partition_compositions {
            if !strictly_increasing(
                composition
                    .source_structural_places
                    .iter()
                    .map(|place| place.id),
            ) {
                return Err(CodecError::NonCanonicalOrder(
                    "partition source structural places by PlaceId",
                ));
            }
            if !strictly_increasing(composition.input_claims.iter().copied()) {
                return Err(CodecError::NonCanonicalOrder(
                    "partition input claims by ClaimId",
                ));
            }
            if !strictly_increasing(composition.substitutions.iter()) {
                return Err(CodecError::NonCanonicalOrder(
                    "partition place substitutions",
                ));
            }
        }
        if !strictly_increasing(
            machine
                .contract
                .ensures
                .iter()
                .map(|clause| clause.obligation),
        ) {
            return Err(CodecError::NonCanonicalOrder("ensures by ObligationId"));
        }
        let propositions = machine.contract.requires.iter().chain(
            machine
                .contract
                .ensures
                .iter()
                .map(|clause| &clause.proposition),
        );
        for proposition in propositions {
            validate_canonical_proposition(proposition, 0)?;
        }
        if !canonical_propositions_strictly_increase(&machine.contract.requires)? {
            return Err(CodecError::NonCanonicalOrder("requires propositions"));
        }
    }
    Ok(())
}

fn validate_parameter_order(
    parameters: &[StructuralParameterDeclaration],
) -> Result<(), CodecError> {
    if parameters
        .iter()
        .enumerate()
        .any(|(index, parameter)| parameter.position != index as u32)
    {
        return Err(CodecError::NonCanonicalOrder(
            "structural parameters by dense position",
        ));
    }
    if parameters
        .iter()
        .any(|parameter| !strictly_increasing(parameter.qualifications.iter().copied()))
    {
        return Err(CodecError::NonCanonicalOrder(
            "structural parameter qualifications by StructuralDomainId",
        ));
    }
    Ok(())
}

/// Validate the closed representation foundation needed before the independent
/// semantic verifier learns these operations. This checks identities, exact
/// carrier relationships, signatures, and operation/result shape; it does not
/// prove qualifications, reach closure, or claim dataflow.
fn validate_structural_foundation(module: &TerminalModule) -> Result<(), CodecError> {
    require_unique_nonempty_identities(
        module
            .structural_types
            .iter()
            .map(|declaration| declaration.identity.as_str()),
        "structural type identity",
    )?;
    require_unique_nonempty_identities(
        module
            .structural_domains
            .iter()
            .map(|declaration| declaration.identity.as_str()),
        "structural domain identity",
    )?;
    require_unique_nonempty_identities(
        module
            .services
            .iter()
            .map(|declaration| declaration.identity.as_str()),
        "service identity",
    )?;
    require_unique_nonempty_identities(
        module
            .boundary_machines
            .iter()
            .map(|declaration| declaration.identity.as_str()),
        "boundary machine identity",
    )?;

    for declaration in &module.structural_types {
        match &declaration.shape {
            StructuralTypeShape::Record { fields } => {
                require_unique_nonempty_identities(
                    fields.iter().map(|field| field.identity.as_str()),
                    "structural field identity",
                )?;
                for field in fields {
                    match &field.field_type {
                        StructuralFieldType::Structural(field_type)
                            if !has_structural_type(module, *field_type) =>
                        {
                            return malformed(
                                "structural field references an unknown structural type",
                            );
                        }
                        StructuralFieldType::Erased { type_identity }
                            if !field.relevance.is_erased() || type_identity.is_empty() =>
                        {
                            return malformed(
                                "opaque structural field type must have erased relevance and a nonempty type identity",
                            );
                        }
                        StructuralFieldType::Scalar(_)
                        | StructuralFieldType::IeeeFloat(_)
                        | StructuralFieldType::Structural(_)
                            if field.relevance.is_erased() =>
                        {
                            return malformed(
                                "erased structural field must use its opaque semantic type identity",
                            );
                        }
                        _ => {}
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, .. } => {
                if !has_structural_type(module, *element) {
                    return malformed("fixed array references an unknown structural element type");
                }
            }
            StructuralTypeShape::Sum { cases } => {
                require_unique_nonempty_identities(
                    cases.iter().map(|case| case.identity.as_str()),
                    "structural case identity",
                )?;
                if cases.is_empty() {
                    return malformed("structural sum must declare at least one case");
                }
                for case in cases {
                    require_unique_nonempty_identities(
                        case.fields.iter().map(|field| field.identity.as_str()),
                        "structural case payload field identity",
                    )?;
                    for field in &case.fields {
                        match &field.field_type {
                            StructuralFieldType::Structural(field_type)
                                if !has_structural_type(module, *field_type) =>
                            {
                                return malformed(
                                    "structural case payload references an unknown structural type",
                                );
                            }
                            StructuralFieldType::Erased { type_identity }
                                if !field.relevance.is_erased() || type_identity.is_empty() =>
                            {
                                return malformed(
                                    "opaque structural case payload must have erased relevance and a nonempty type identity",
                                );
                            }
                            StructuralFieldType::Scalar(_)
                            | StructuralFieldType::IeeeFloat(_)
                            | StructuralFieldType::Structural(_)
                                if field.relevance.is_erased() =>
                            {
                                return malformed(
                                    "erased structural case payload must use its opaque semantic type identity",
                                );
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    validate_structural_type_graph(module)?;
    for domain in &module.structural_domains {
        if !has_structural_type(module, domain.carrier) {
            return malformed("structural domain references an unknown carrier type");
        }
    }
    for service in &module.services {
        if service
            .parents
            .iter()
            .any(|parent| *parent == service.id || !has_service(module, *parent))
        {
            return malformed("service references itself or an unknown parent");
        }
    }
    validate_service_parent_graph(module)?;
    for boundary in &module.boundary_machines {
        if boundary
            .attachment
            .is_some_and(|attachment| !has_structural_type(module, attachment))
        {
            return malformed("boundary machine has an unknown attachment type");
        }
        validate_structural_parameters(module, &boundary.structural_parameters)?;
        for requirement in &boundary.requires {
            let Some(parameter) = boundary
                .structural_parameters
                .get(requirement.argument_index as usize)
            else {
                return malformed("boundary requirement has an unknown argument index");
            };
            let Some(domain) = module
                .structural_domains
                .iter()
                .find(|domain| domain.id == requirement.domain)
            else {
                return malformed("boundary requirement references an unknown domain");
            };
            if domain.carrier != parameter.structural_type {
                return malformed("boundary requirement domain has the wrong carrier type");
            }
        }
        require_known_services(module, &boundary.published_service_ceiling)?;
    }
    for candidate in &module.provider_candidates {
        if candidate.requirement_identity.is_empty()
            || candidate.provider_identity.is_empty()
            || candidate.candidate_identity.is_empty()
        {
            return malformed("provider candidate identities must be nonempty");
        }
        if !module
            .boundary_machines
            .iter()
            .any(|boundary| boundary.id == candidate.boundary)
            || !module
                .machines
                .iter()
                .any(|machine| machine.id == candidate.candidate)
        {
            return malformed("provider candidate references an unknown terminal ID");
        }
        for parameter in &candidate.signature.parameters {
            if !has_structural_type(module, parameter.structural_type) {
                return malformed("provider signature references an unknown structural type");
            }
            if parameter.qualifications.iter().any(|domain| {
                !module
                    .structural_domains
                    .iter()
                    .any(|row| row.id == *domain)
            }) {
                return malformed("provider signature references an unknown structural domain");
            }
        }
        require_known_services(module, &candidate.refinement.realized_service_ceiling)?;
    }

    for machine in &module.machines {
        if machine
            .attachment
            .is_some_and(|attachment| !has_structural_type(module, attachment))
        {
            return malformed("machine has an unknown attachment type");
        }
        validate_structural_parameters(module, &machine.structural_parameters)?;
        require_known_services(module, &machine.published_service_ceiling)?;
        for parameter in &machine.structural_parameters {
            let Some(place) = machine
                .structural_places
                .iter()
                .find(|place| place.id == parameter.place)
            else {
                return malformed("structural parameter has no declared structural place");
            };
            if place.kind
                != (StructuralPlaceKind::Parameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                })
            {
                return malformed("structural parameter place kind disagrees with its signature");
            }
        }
        for claim in &machine.entry_claims {
            let Some(parameter) = machine
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == claim.input)
            else {
                return malformed("entry claim is not bound to a structural parameter");
            };
            if parameter.multiplicity == StructuralMultiplicity::Unrestricted {
                return malformed("entry claim cannot bind an unrestricted parameter");
            }
            validate_structural_path(module, parameter.structural_type, &claim.path)?;
        }
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            validate_operation_foundation(module, machine, operation)?;
        }
        for block in &machine.blocks {
            if let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &block.terminator {
                if !matches!(machine.result, TerminalMachineResult::Unit) {
                    return malformed("nominal affine cleanup requires a Unit result");
                }
                if cleanups.is_empty() {
                    return malformed("nominal affine cleanup list is empty");
                }
                if cleanups.len() != machine.structural_parameters.len() {
                    return malformed(
                        "nominal affine cleanup list does not cover every structural parameter",
                    );
                }
                if cleanups.iter().any(|cleanup| {
                    !machine
                        .structural_parameters
                        .iter()
                        .any(|parameter| parameter.place == cleanup.place)
                }) {
                    return malformed("nominal affine cleanup root is not a structural parameter");
                }
                let mut places = BTreeSet::new();
                for (cleanup, parameter) in cleanups
                    .iter()
                    .zip(machine.structural_parameters.iter().rev())
                {
                    if cleanup.place != parameter.place {
                        return malformed(
                            "nominal affine cleanup list is not in reverse parameter order",
                        );
                    }
                    if !places.insert(cleanup.place)
                        || parameter.multiplicity != StructuralMultiplicity::Affine
                        || !parameter.qualifications.is_empty()
                        || machine
                            .entry_claims
                            .iter()
                            .any(|claim| claim.input == cleanup.place)
                    {
                        return malformed(
                            "nominal affine cleanup is duplicated or not a claim-free qualified-free affine root",
                        );
                    }
                    if parameter.structural_type != cleanup.structural_type {
                        return malformed(
                            "nominal affine cleanup type does not match its structural parameter",
                        );
                    }
                }
            }
            let Terminator::ReturnUnitPartialAffine {
                trivial_affine_discards,
                residual_affine_discards,
                ..
            } = &block.terminator
            else {
                continue;
            };
            if !matches!(machine.result, TerminalMachineResult::Unit)
                || residual_affine_discards.is_empty()
            {
                return malformed(
                    "partial affine cleanup requires a Unit result and a residual action",
                );
            }
            for discard in residual_affine_discards {
                let Some(parameter) = machine
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == discard.place)
                else {
                    return malformed("partial affine cleanup root is not a structural parameter");
                };
                if parameter.multiplicity != StructuralMultiplicity::Affine
                    || discard.path.is_empty()
                    || trivial_affine_discards.contains(&discard.place)
                    || machine
                        .entry_claims
                        .iter()
                        .any(|claim| claim.input == discard.place)
                {
                    return malformed(
                        "partial affine cleanup is not a distinct claim-free affine path",
                    );
                }
                if validate_structural_path(module, parameter.structural_type, &discard.path)?
                    != discard.structural_type
                {
                    return malformed("partial affine cleanup leaf type does not match its path");
                }
            }
        }
    }
    Ok(())
}

fn validate_structural_path(
    module: &TerminalModule,
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Result<StructuralTypeId, CodecError> {
    for segment in path {
        let Some(declaration) = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)
        else {
            return malformed("structural path has an unknown structural type");
        };
        structural_type = match (segment, &declaration.shape) {
            (StructuralPathSegment::Field(identity), StructuralTypeShape::Record { fields }) => {
                if identity.is_empty() {
                    return malformed("structural path field identity cannot be empty");
                }
                let Some(field) = fields.iter().find(|field| field.identity == *identity) else {
                    return malformed("structural path has an unknown structural field");
                };
                if field.relevance.is_erased() {
                    return malformed("structural path cannot select an erased structural field");
                }
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return malformed("structural path must retain structural custody");
                };
                next
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) => {
                if index >= length {
                    return malformed("structural path fixed index is out of bounds");
                }
                *element
            }
            (StructuralPathSegment::Field(_), StructuralTypeShape::FixedArray { .. }) => {
                return malformed("structural path field requires a record type");
            }
            (StructuralPathSegment::FixedIndex(_), StructuralTypeShape::Record { .. }) => {
                return malformed("structural path fixed index requires a fixed-array type");
            }
            (_, StructuralTypeShape::Sum { .. }) => {
                return malformed("structural path cannot traverse a payload-less sum");
            }
        };
    }
    Ok(structural_type)
}

fn validate_structural_parameters(
    module: &TerminalModule,
    parameters: &[StructuralParameterDeclaration],
) -> Result<(), CodecError> {
    let mut places = BTreeSet::new();
    let mut self_count = 0_u32;
    for parameter in parameters {
        if !places.insert(parameter.place) {
            return malformed("structural parameters reuse a place identity");
        }
        self_count += u32::from(parameter.is_self);
        if self_count > 1 {
            return malformed("structural signature declares more than one self parameter");
        }
        if !has_structural_type(module, parameter.structural_type) {
            return malformed("structural parameter references an unknown type");
        }
        for qualification in &parameter.qualifications {
            let Some(domain) = module
                .structural_domains
                .iter()
                .find(|domain| domain.id == *qualification)
            else {
                return malformed("structural parameter references an unknown qualification");
            };
            if domain.carrier != parameter.structural_type {
                return malformed("structural parameter qualification has the wrong carrier");
            }
        }
    }
    Ok(())
}

fn validate_operation_foundation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    match &operation.kind {
        OperationKind::CallUnit {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } => {
            if operation.result != OperationResult::Unit {
                return malformed("unit call declares a scalar result");
            }
            let Some(callee) = module
                .machines
                .iter()
                .find(|candidate| candidate.id == *callee)
            else {
                return malformed("unit call references an unknown callee");
            };
            if callee.result != TerminalMachineResult::Unit
                || structural_arguments.len() != callee.structural_parameters.len()
            {
                return malformed("unit call has the wrong callee result or structural arity");
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &callee.structural_parameters,
            )?;
            validate_claim_indices(
                machine,
                structural_arguments,
                claim_transfers
                    .iter()
                    .map(|transfer| (transfer.claim, transfer.argument_index)),
            )?;
        }
        OperationKind::CallStructuralScalar {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } => {
            let Some(callee) = module
                .machines
                .iter()
                .find(|candidate| candidate.id == *callee)
            else {
                return malformed("structural scalar call references an unknown callee");
            };
            if callee.parameters.len() != 0
                || operation.result.scalar().map(|result| result.scalar_type)
                    != callee.result.scalar().map(|result| result.scalar_type)
                || operation.result == OperationResult::Unit
                || structural_arguments.len() != callee.structural_parameters.len()
            {
                return malformed(
                    "structural scalar call has the wrong callee signature or structural arity",
                );
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &callee.structural_parameters,
            )?;
            validate_claim_indices(
                machine,
                structural_arguments,
                claim_transfers
                    .iter()
                    .map(|transfer| (transfer.claim, transfer.argument_index)),
            )?;
        }
        OperationKind::BoundaryCall {
            boundary,
            structural_arguments,
            completion_receipts,
            ..
        } => {
            let Some(boundary) = module
                .boundary_machines
                .iter()
                .find(|candidate| candidate.id == *boundary)
            else {
                return malformed("boundary call references an unknown boundary");
            };
            if operation.result.scalar().map(|result| result.scalar_type) != boundary.result {
                return malformed("boundary call result disagrees with its declaration");
            }
            if structural_arguments.len() != boundary.structural_parameters.len() {
                return malformed("boundary call has the wrong structural arity");
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &boundary.structural_parameters,
            )?;
            validate_claim_indices(
                machine,
                structural_arguments,
                completion_receipts
                    .iter()
                    .map(|settlement| (settlement.claim, settlement.argument_index)),
            )?;
        }
        OperationKind::PortWrite { service, .. } => {
            if operation.result != OperationResult::Unit {
                return malformed("port write declares a scalar result");
            }
            if !has_service(module, *service) {
                return malformed("port write references an unknown service");
            }
        }
        OperationKind::EstablishTrivialAffineLocal { destination } => {
            if operation.result != OperationResult::Unit {
                return malformed("trivial affine local establishment declares a scalar result");
            }
            let Some(StructuralPlaceDeclaration {
                kind:
                    StructuralPlaceKind::TrivialAffineLocal {
                        structural_type, ..
                    },
                ..
            }) = machine
                .structural_places
                .iter()
                .find(|place| place.id == *destination)
            else {
                return malformed("trivial affine local establishment has no local declaration");
            };
            let Some(declaration) = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == *structural_type)
            else {
                return malformed("trivial affine local has an unknown structural type");
            };
            if !matches!(
                &declaration.shape,
                StructuralTypeShape::Record { fields } if fields.is_empty()
            ) {
                return malformed("trivial affine local must have an empty record type");
            }
        }
        OperationKind::Call { .. } => {
            if !matches!(operation.result, OperationResult::Scalar(_)) {
                return malformed("scalar call declares a Unit result");
            }
        }
        _ => {
            if !matches!(operation.result, OperationResult::Scalar(_)) {
                return malformed("scalar operation declares a Unit result");
            }
        }
    }
    Ok(())
}

fn validate_structural_arguments(
    module: &TerminalModule,
    machine: &TerminalMachine,
    arguments: &[StructuralArgument],
    expected: &[StructuralParameterDeclaration],
) -> Result<(), CodecError> {
    for (argument, expected) in arguments.iter().zip(expected) {
        let Some(actual) = machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == argument.place)
        else {
            return malformed("structural argument references an unknown parameter place");
        };
        let actual_type = validate_structural_path(module, actual.structural_type, &argument.path)?;
        if actual_type != expected.structural_type {
            return malformed("structural argument has the wrong concrete type");
        }
    }
    Ok(())
}

fn validate_claim_indices(
    machine: &TerminalMachine,
    arguments: &[StructuralArgument],
    claims: impl Iterator<Item = (ClaimId, u32)>,
) -> Result<(), CodecError> {
    for (claim, argument_index) in claims {
        let Some(argument) = arguments.get(argument_index as usize) else {
            return malformed("claim action has an unknown structural argument index");
        };
        let Some(entry_claim) = machine
            .entry_claims
            .iter()
            .find(|entry_claim| entry_claim.claim == claim)
        else {
            return malformed("claim action references an unknown entry claim");
        };
        if entry_claim.input != argument.place
            || (!argument.path.is_empty() && entry_claim.path != argument.path)
        {
            return malformed("claim action does not match its structural argument path");
        }
    }
    Ok(())
}

fn has_structural_type(module: &TerminalModule, id: StructuralTypeId) -> bool {
    module
        .structural_types
        .iter()
        .any(|declaration| declaration.id == id)
}

fn validate_structural_type_graph(module: &TerminalModule) -> Result<(), CodecError> {
    fn visit(
        module: &TerminalModule,
        id: StructuralTypeId,
        active: &mut BTreeSet<StructuralTypeId>,
        complete: &mut BTreeSet<StructuralTypeId>,
    ) -> Result<(), CodecError> {
        if complete.contains(&id) {
            return Ok(());
        }
        if !active.insert(id) {
            return malformed("structural type graph contains a by-value cycle");
        }
        let declaration = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == id)
            .expect("structural field targets were validated before graph traversal");
        match &declaration.shape {
            StructuralTypeShape::Record { fields } => {
                for field in fields {
                    if let StructuralFieldType::Structural(target) = &field.field_type {
                        visit(module, *target, active, complete)?;
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, .. } => {
                visit(module, *element, active, complete)?;
            }
            StructuralTypeShape::Sum { cases } => {
                for field in cases.iter().flat_map(|case| &case.fields) {
                    if let StructuralFieldType::Structural(target) = &field.field_type {
                        visit(module, *target, active, complete)?;
                    }
                }
            }
        }
        active.remove(&id);
        complete.insert(id);
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for declaration in &module.structural_types {
        visit(module, declaration.id, &mut active, &mut complete)?;
    }
    Ok(())
}

fn validate_service_parent_graph(module: &TerminalModule) -> Result<(), CodecError> {
    fn visit(
        module: &TerminalModule,
        id: ServiceId,
        active: &mut BTreeSet<ServiceId>,
        complete: &mut BTreeSet<ServiceId>,
    ) -> Result<(), CodecError> {
        if complete.contains(&id) {
            return Ok(());
        }
        if !active.insert(id) {
            return malformed("service parent graph contains a cycle");
        }
        let declaration = module
            .services
            .iter()
            .find(|declaration| declaration.id == id)
            .expect("service parent targets were validated before graph traversal");
        for parent in &declaration.parents {
            visit(module, *parent, active, complete)?;
        }
        active.remove(&id);
        complete.insert(id);
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for declaration in &module.services {
        visit(module, declaration.id, &mut active, &mut complete)?;
    }
    for declaration in &module.services {
        for parent in &declaration.parents {
            let parent = module
                .services
                .iter()
                .find(|candidate| candidate.id == *parent)
                .expect("service parent targets were validated before closure validation");
            if parent
                .parents
                .iter()
                .any(|ancestor| !declaration.parents.contains(ancestor))
            {
                return malformed("service parent closure is incomplete");
            }
        }
    }
    Ok(())
}

fn has_service(module: &TerminalModule, id: ServiceId) -> bool {
    module
        .services
        .iter()
        .any(|declaration| declaration.id == id)
}

fn require_known_services(
    module: &TerminalModule,
    services: &[ServiceId],
) -> Result<(), CodecError> {
    if services
        .iter()
        .any(|service| !has_service(module, *service))
    {
        return malformed("published service ceiling references an unknown service");
    }
    Ok(())
}

fn require_unique_nonempty_identities<'a>(
    identities: impl Iterator<Item = &'a str>,
    label: &'static str,
) -> Result<(), CodecError> {
    let mut seen = BTreeSet::new();
    for identity in identities {
        if identity.is_empty() || !seen.insert(identity) {
            return Err(CodecError::MalformedStructuralFoundation(label));
        }
    }
    Ok(())
}

fn malformed<T>(message: &'static str) -> Result<T, CodecError> {
    Err(CodecError::MalformedStructuralFoundation(message))
}

fn crash_routes_are_canonical(routes: &[CrashRouteBucket]) -> bool {
    !routes.windows(2).any(|pair| pair[0].cause >= pair[1].cause)
        && routes.iter().all(|bucket| {
            !bucket.alternatives.is_empty()
                && !bucket
                    .alternatives
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                && (!bucket.alternatives.contains(&CrashRouteGuard::Truth)
                    || bucket.alternatives == [CrashRouteGuard::Truth])
        })
}

fn validate_crash_route_predicates(routes: &[CrashRouteBucket]) -> Result<(), CodecError> {
    for predicate in routes
        .iter()
        .flat_map(|bucket| &bucket.alternatives)
        .filter_map(|guard| match guard {
            CrashRouteGuard::Truth => None,
            CrashRouteGuard::Predicate(predicate) => Some(predicate),
        })
    {
        validate_canonical_proposition(predicate.proposition(), 0)?;
    }
    Ok(())
}

fn validate_canonical_proposition(
    proposition: &Proposition,
    depth: usize,
) -> Result<(), CodecError> {
    if depth > MAX_PROPOSITION_DEPTH {
        return Err(CodecError::PropositionNestingTooDeep);
    }
    match proposition {
        Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => Ok(()),
        Proposition::Equal(left, right) => {
            validate_scalar_term_depth(left)?;
            validate_scalar_term_depth(right)?;
            if canonical_scalar_term_bytes(left)? > canonical_scalar_term_bytes(right)? {
                return Err(CodecError::NonCanonicalOrder("equality operands"));
            }
            Ok(())
        }
        Proposition::LessThan(left, right) | Proposition::LessOrEqual(left, right) => {
            validate_scalar_term_depth(left)?;
            validate_scalar_term_depth(right)
        }
        Proposition::IeeeFloatComparison { left, right, .. } => {
            if left > right {
                return Err(CodecError::NonCanonicalOrder("IEEE equality operands"));
            }
            Ok(())
        }
        Proposition::ByteSequenceEqual { left, right } => {
            if left > right {
                return Err(CodecError::NonCanonicalOrder(
                    "byte-sequence equality operands",
                ));
            }
            Ok(())
        }
        Proposition::StructuralCaseMembership { .. } => Ok(()),
        Proposition::Conjunction(conjuncts) => {
            if conjuncts
                .iter()
                .any(|conjunct| matches!(conjunct, Proposition::Conjunction(_)))
            {
                return Err(CodecError::NestedConjunction);
            }
            for conjunct in conjuncts {
                validate_canonical_proposition(conjunct, depth + 1)?;
            }
            if !canonical_propositions_strictly_increase(conjuncts)? {
                return Err(CodecError::NonCanonicalOrder("conjunction propositions"));
            }
            Ok(())
        }
        Proposition::Disjunction(disjuncts) => {
            if disjuncts
                .iter()
                .any(|disjunct| matches!(disjunct, Proposition::Disjunction(_)))
            {
                return Err(CodecError::NestedDisjunction);
            }
            for disjunct in disjuncts {
                validate_canonical_proposition(disjunct, depth + 1)?;
            }
            if !canonical_propositions_strictly_increase(disjuncts)? {
                return Err(CodecError::NonCanonicalOrder("disjunction propositions"));
            }
            Ok(())
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            validate_canonical_proposition(premise, depth + 1)?;
            validate_canonical_proposition(conclusion, depth + 1)
        }
        Proposition::ContentConservation(conservation) => {
            validate_content_term_depth(conservation.left(), 0)?;
            validate_content_term_depth(conservation.right(), 0)
        }
    }
}

fn canonical_propositions_strictly_increase(
    propositions: &[Proposition],
) -> Result<bool, CodecError> {
    let mut previous = None;
    for proposition in propositions {
        let bytes = canonical_proposition_bytes(proposition)?;
        if previous.as_ref().is_some_and(|previous| previous >= &bytes) {
            return Ok(false);
        }
        previous = Some(bytes);
    }
    Ok(true)
}

fn canonical_proposition_bytes(proposition: &Proposition) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    encode_proposition(&mut writer, proposition, 0)?;
    Ok(writer.finish())
}

/// Canonical bytewise ordering key for one terminal proposition. Producers
/// that construct canonical sets use the codec-owned order rather than Rust
/// enum declaration order.
pub fn canonical_proposition_order_key(proposition: &Proposition) -> Result<Vec<u8>, CodecError> {
    canonical_proposition_bytes(proposition)
}

fn canonical_scalar_term_bytes(term: &ScalarTerm) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    encode_scalar_term(&mut writer, term, 0)?;
    Ok(writer.finish())
}

fn validate_scalar_term_depth(term: &ScalarTerm) -> Result<(), CodecError> {
    let mut pending = vec![(term, 0_usize)];
    while let Some((term, depth)) = pending.pop() {
        if depth > MAX_SCALAR_TERM_DEPTH {
            return Err(CodecError::ScalarTermNestingTooDeep);
        }
        match term {
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => {
                pending.push((operand, depth + 1));
            }
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. }
            | ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. }
            | ScalarTerm::ExactIntegerDivide { left, right, .. }
            | ScalarTerm::ExactIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerDivide { left, right, .. }
            | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
            | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
            | ScalarTerm::SaturatingIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
                pending.push((left, depth + 1));
                pending.push((right, depth + 1));
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
            | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                pending.push((value, depth + 1));
                pending.push((count, depth + 1));
            }
            ScalarTerm::Value { .. }
            | ScalarTerm::BooleanField { .. }
            | ScalarTerm::IntegerField { .. }
            | ScalarTerm::Boolean(_)
            | ScalarTerm::Integer { .. } => {}
        }
    }
    Ok(())
}

fn validate_content_term_depth(term: &ContentTerm, depth: usize) -> Result<(), CodecError> {
    if depth > MAX_CONTENT_TERM_DEPTH {
        return Err(CodecError::ContentTermNestingTooDeep);
    }
    if let ContentTerm::Separate(terms) = term {
        for term in terms {
            validate_content_term_depth(term, depth + 1)?;
        }
    }
    Ok(())
}

fn strictly_increasing<T: Ord>(values: impl IntoIterator<Item = T>) -> bool {
    let mut previous = None;
    for value in values {
        if previous.as_ref().is_some_and(|previous| previous >= &value) {
            return false;
        }
        previous = Some(value);
    }
    true
}

fn encode_raw(module: &TerminalModule) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    writer.bytes(MAGIC);
    writer.u16(FORMAT_MARKER);
    writer.u16(module.vocabulary_marker.get());
    writer.id(module.entry);
    writer.len("structural types", module.structural_types.len())?;
    for declaration in &module.structural_types {
        encode_structural_type(&mut writer, declaration)?;
    }
    writer.len("structural domains", module.structural_domains.len())?;
    for declaration in &module.structural_domains {
        writer.id(declaration.id);
        writer.string("structural domain identity", &declaration.identity)?;
        writer.id(declaration.carrier);
    }
    writer.len("services", module.services.len())?;
    for declaration in &module.services {
        writer.id(declaration.id);
        writer.string("service identity", &declaration.identity)?;
        writer.len("service parents", declaration.parents.len())?;
        for parent in &declaration.parents {
            writer.id(*parent);
        }
    }
    writer.len("boundary machines", module.boundary_machines.len())?;
    for declaration in &module.boundary_machines {
        encode_boundary_machine(&mut writer, declaration)?;
    }
    writer.len("provider candidates", module.provider_candidates.len())?;
    for candidate in &module.provider_candidates {
        encode_provider_candidate(&mut writer, candidate)?;
    }
    writer.len(
        "float-meaning projections",
        module.float_meaning_projections.len(),
    )?;
    for projection in &module.float_meaning_projections {
        writer.u32(projection.result.id.0);
        writer.u8(match projection.result.value_type {
            ProofOnlyValueType::FloatMeaning => 1,
        });
        writer.u32(projection.source.id.0);
        writer.u8(match projection.source.format {
            IeeeFloatFormat::Binary32 => 1,
            IeeeFloatFormat::Binary64 => 2,
        });
        writer.u8(match projection.operation {
            FloatMeaningProjectionOperation::Meaning32 => 1,
            FloatMeaningProjectionOperation::Meaning64 => 2,
        });
    }
    writer.len(
        "float-meaning equalities",
        module.float_meaning_equalities.len(),
    )?;
    for proposition in &module.float_meaning_equalities {
        writer.u32(proposition.id.0);
        writer.u32(proposition.left.0);
        writer.u32(proposition.right.0);
    }
    writer.len(
        "proposition declarations",
        module.proposition_declarations.len(),
    )?;
    for declaration in &module.proposition_declarations {
        encode_proposition_declaration(&mut writer, declaration)?;
    }
    writer.len(
        "proposition applications",
        module.proposition_applications.len(),
    )?;
    for application in &module.proposition_applications {
        encode_proposition_application(&mut writer, application)?;
    }
    writer.len("evidence terms", module.evidence_terms.len())?;
    for term in &module.evidence_terms {
        writer.id(term.id);
        writer.id(term.proposition);
        encode_evidence_interface(&mut writer, &term.interface)?;
    }
    writer.len(
        "evidence contract lanes",
        module.evidence_contract_lanes.len(),
    )?;
    for lane in &module.evidence_contract_lanes {
        writer.id(lane.machine);
        writer.u8(match lane.kind {
            EvidenceContractLaneKind::Requires => 1,
            EvidenceContractLaneKind::Ensures => 2,
        });
        writer.u32(lane.position);
        writer.id(lane.term);
        writer.boolean(lane.output_field.is_some());
        if let Some(field) = &lane.output_field {
            writer.string("evidence output field", field)?;
        }
    }
    writer.len(
        "evidence package invocations",
        module.evidence_package_invocations.len(),
    )?;
    for invocation in &module.evidence_package_invocations {
        writer.id(invocation.caller);
        writer.u32(invocation.ordinal);
        writer.string(
            "evidence package target machine identity",
            &invocation.target_machine_identity,
        )?;
        writer.boolean(invocation.runtime_value.is_some());
        if let Some(runtime_value) = invocation.runtime_value {
            encode_scalar_type(&mut writer, runtime_value);
        }
        writer.boolean(invocation.runtime_call.is_some());
        if let Some(runtime_call) = invocation.runtime_call {
            writer.id(runtime_call.operation);
            writer.id(runtime_call.callee);
        }
        writer.len("evidence package outputs", invocation.outputs.len())?;
        for output in &invocation.outputs {
            writer.u32(output.output_position);
            writer.string("evidence package output field", &output.output_field)?;
            writer.id(output.callee_output);
            writer.boolean(output.output.is_some());
            if let Some(output) = output.output {
                writer.id(output);
            }
        }
    }
    writer.len(
        "closed conformance applications",
        module.closed_conformance_applications.len(),
    )?;
    for application in &module.closed_conformance_applications {
        writer.id(application.owner);
        writer.string(
            "closed conformance declaration identity",
            &application.declaration_identity,
        )?;
        writer.len("closed conformance telescope", application.telescope.len())?;
        for binding in &application.telescope {
            writer.string("closed conformance parameter", &binding.parameter)?;
            writer.u8(match binding.kind {
                ClosedConformanceParameterKind::Lifetime => 1,
                ClosedConformanceParameterKind::Type => 2,
                ClosedConformanceParameterKind::Const => 3,
                ClosedConformanceParameterKind::Machine => 4,
            });
            writer.string("closed conformance argument", &binding.argument)?;
        }
        writer.boolean(application.subject_identity.is_some());
        if let Some(subject) = &application.subject_identity {
            writer.string("closed conformance subject identity", subject)?;
        }
        writer.string(
            "closed conformance trait identity",
            &application.trait_identity,
        )?;
        writer.strings(
            "closed conformance trait arguments",
            &application.trait_arguments,
        )?;
        writer.len("closed conformance rows", application.rows.len())?;
        for row in &application.rows {
            writer.string(
                "closed conformance row declaring trait identity",
                &row.declaring_trait_identity,
            )?;
            writer.string(
                "closed conformance row requirement identity",
                &row.requirement_identity,
            )?;
            writer.string(
                "closed conformance row realization identity",
                &row.realization_identity,
            )?;
        }
        writer.u64(application.fingerprint);
    }
    writer.len("machines", module.machines.len())?;
    for machine in &module.machines {
        encode_machine(&mut writer, machine)?;
    }
    Ok(writer.finish())
}

fn encode_ieee_float_format(writer: &mut Writer, format: IeeeFloatFormat) {
    writer.u8(match format {
        IeeeFloatFormat::Binary32 => 1,
        IeeeFloatFormat::Binary64 => 2,
    });
}

fn encode_ieee_float_comparison_kind(writer: &mut Writer, kind: IeeeFloatComparisonKind) {
    writer.u8(match kind {
        IeeeFloatComparisonKind::Equal => 1,
        IeeeFloatComparisonKind::NotEqual => 2,
    });
}

fn decode_ieee_float_comparison_kind(
    reader: &mut Reader<'_>,
) -> Result<IeeeFloatComparisonKind, CodecError> {
    match reader.u8()? {
        1 => Ok(IeeeFloatComparisonKind::Equal),
        2 => Ok(IeeeFloatComparisonKind::NotEqual),
        tag => Err(CodecError::InvalidTag("IeeeFloatComparisonKind", tag)),
    }
}

fn decode_ieee_float_format(reader: &mut Reader<'_>) -> Result<IeeeFloatFormat, CodecError> {
    match reader.u8()? {
        1 => Ok(IeeeFloatFormat::Binary32),
        2 => Ok(IeeeFloatFormat::Binary64),
        tag => Err(CodecError::InvalidTag("IeeeFloatFormat", tag)),
    }
}

fn encode_ieee_float_field(
    writer: &mut Writer,
    field: &IeeeFloatStructuralField,
) -> Result<(), CodecError> {
    encode_canonical_structural_field(writer, field.root(), field.path(), "IEEE float field path")
}

fn decode_ieee_float_field(
    reader: &mut Reader<'_>,
) -> Result<IeeeFloatStructuralField, CodecError> {
    let (root, path) = decode_canonical_structural_field(reader)?;
    IeeeFloatStructuralField::new(root, path).map_err(CodecError::MalformedProposition)
}

fn encode_byte_sequence_field(
    writer: &mut Writer,
    field: &ByteSequenceStructuralField,
) -> Result<(), CodecError> {
    encode_canonical_structural_field(
        writer,
        field.root(),
        field.path(),
        "byte-sequence field path",
    )
}

fn decode_byte_sequence_field(
    reader: &mut Reader<'_>,
) -> Result<ByteSequenceStructuralField, CodecError> {
    let (root, path) = decode_canonical_structural_field(reader)?;
    ByteSequenceStructuralField::new(root, path).map_err(CodecError::MalformedProposition)
}

fn encode_canonical_structural_field(
    writer: &mut Writer,
    root: PlaceId,
    path: &[CanonicalStructuralPathSegment],
    length_label: &'static str,
) -> Result<(), CodecError> {
    writer.id(root);
    writer.len(length_label, path.len())?;
    for segment in path {
        match segment {
            CanonicalStructuralPathSegment::Field(field) => {
                writer.u8(1);
                writer.id(*field);
            }
            CanonicalStructuralPathSegment::FixedIndex(index) => {
                writer.u8(2);
                writer.u64(*index);
            }
            CanonicalStructuralPathSegment::Case(case) => {
                writer.u8(3);
                writer.id(*case);
            }
        }
    }
    Ok(())
}

fn decode_canonical_structural_field(
    reader: &mut Reader<'_>,
) -> Result<(PlaceId, Vec<CanonicalStructuralPathSegment>), CodecError> {
    let root = reader.id("PlaceId")?;
    let count = reader.count()?;
    let mut path = Vec::with_capacity(count as usize);
    for _ in 0..count {
        path.push(match reader.u8()? {
            1 => CanonicalStructuralPathSegment::Field(reader.id("StructuralFieldId")?),
            2 => CanonicalStructuralPathSegment::FixedIndex(reader.u64()?),
            3 => CanonicalStructuralPathSegment::Case(reader.id("StructuralCaseId")?),
            tag => {
                return Err(CodecError::InvalidTag(
                    "CanonicalStructuralPathSegment",
                    tag,
                ));
            }
        });
    }
    Ok((root, path))
}

fn encode_byte_sequence_carrier(writer: &mut Writer, carrier: ByteSequenceCarrier) {
    match carrier {
        ByteSequenceCarrier::BorrowedView => writer.u8(1),
        ByteSequenceCarrier::BoundedOwned { capacity } => {
            writer.u8(2);
            writer.u64(capacity);
        }
    }
}

fn decode_byte_sequence_carrier(
    reader: &mut Reader<'_>,
) -> Result<ByteSequenceCarrier, CodecError> {
    match reader.u8()? {
        1 => Ok(ByteSequenceCarrier::BorrowedView),
        2 => Ok(ByteSequenceCarrier::BoundedOwned {
            capacity: reader.u64()?,
        }),
        tag => Err(CodecError::InvalidTag("ByteSequenceCarrier", tag)),
    }
}

fn encode_structural_field(
    writer: &mut Writer,
    field: &StructuralFieldDeclaration,
) -> Result<(), CodecError> {
    writer.id(field.id);
    writer.string("structural field identity", &field.identity)?;
    writer.u8(match field.relevance {
        BindingRelevance::Relevant => 1,
        BindingRelevance::Erased => 2,
    });
    match &field.field_type {
        StructuralFieldType::Scalar(scalar_type) => {
            writer.u8(1);
            encode_scalar_type(writer, *scalar_type);
        }
        StructuralFieldType::IeeeFloat(format) => {
            writer.u8(4);
            encode_ieee_float_format(writer, *format);
        }
        StructuralFieldType::ByteSequence(carrier) => {
            writer.u8(5);
            encode_byte_sequence_carrier(writer, *carrier);
        }
        StructuralFieldType::Structural(structural_type) => {
            writer.u8(2);
            writer.id(*structural_type);
        }
        StructuralFieldType::Erased { type_identity } => {
            writer.u8(3);
            writer.string("erased structural field type identity", type_identity)?;
        }
    }
    Ok(())
}

fn encode_structural_type(
    writer: &mut Writer,
    declaration: &StructuralTypeDeclaration,
) -> Result<(), CodecError> {
    writer.id(declaration.id);
    writer.string("structural type identity", &declaration.identity)?;
    match &declaration.shape {
        StructuralTypeShape::Record { fields } => {
            writer.u8(1);
            writer.len("structural fields", fields.len())?;
            for field in fields {
                encode_structural_field(writer, field)?;
            }
        }
        StructuralTypeShape::FixedArray { element, length } => {
            writer.u8(2);
            writer.id(*element);
            writer.u64(*length);
        }
        StructuralTypeShape::Sum { cases } => {
            writer.u8(3);
            writer.len("structural cases", cases.len())?;
            for case in cases {
                writer.id(case.id);
                writer.string("structural case identity", &case.identity)?;
                writer.len("structural case payload fields", case.fields.len())?;
                for field in &case.fields {
                    encode_structural_field(writer, field)?;
                }
            }
        }
    }
    Ok(())
}

fn encode_boundary_machine(
    writer: &mut Writer,
    declaration: &BoundaryMachineDeclaration,
) -> Result<(), CodecError> {
    writer.id(declaration.id);
    writer.string("boundary machine identity", &declaration.identity)?;
    encode_optional_id(writer, declaration.attachment);
    encode_structural_parameters(writer, &declaration.structural_parameters)?;
    writer.boolean(declaration.result.is_some());
    if let Some(result) = declaration.result {
        encode_scalar_type(writer, result);
    }
    writer.len(
        "boundary structural requirements",
        declaration.requires.len(),
    )?;
    for requirement in &declaration.requires {
        writer.u32(requirement.argument_index);
        writer.id(requirement.domain);
    }
    encode_service_ceiling(writer, &declaration.published_service_ceiling)
}

fn encode_provider_candidate(
    writer: &mut Writer,
    candidate: &ProviderCandidateConformance,
) -> Result<(), CodecError> {
    writer.id(candidate.boundary);
    writer.string(
        "provider requirement identity",
        &candidate.requirement_identity,
    )?;
    writer.string("provider identity", &candidate.provider_identity)?;
    writer.string("provider candidate identity", &candidate.candidate_identity)?;
    writer.id(candidate.candidate);
    writer.len(
        "provider signature parameters",
        candidate.signature.parameters.len(),
    )?;
    for parameter in &candidate.signature.parameters {
        writer.u32(parameter.position);
        writer.u8(u8::from(parameter.is_self));
        writer.id(parameter.structural_type);
        writer.u8(match parameter.multiplicity {
            StructuralMultiplicity::Unrestricted => 1,
            StructuralMultiplicity::Affine => 2,
            StructuralMultiplicity::Linear => 3,
        });
        writer.len(
            "provider signature qualifications",
            parameter.qualifications.len(),
        )?;
        for qualification in &parameter.qualifications {
            writer.id(*qualification);
        }
    }
    writer.len(
        "provider positional refinements",
        candidate.refinement.positional_parameters.len(),
    )?;
    for parameter in &candidate.refinement.positional_parameters {
        writer.u32(parameter.boundary_index);
        writer.u32(parameter.candidate_index);
    }
    writer.len(
        "provider required domains",
        candidate.refinement.required_domains.len(),
    )?;
    for requirement in &candidate.refinement.required_domains {
        writer.u32(requirement.argument_index);
        writer.id(requirement.domain);
    }
    encode_service_ceiling(writer, &candidate.refinement.realized_service_ceiling)
}

fn encode_structural_parameters(
    writer: &mut Writer,
    parameters: &[StructuralParameterDeclaration],
) -> Result<(), CodecError> {
    writer.len("structural parameters", parameters.len())?;
    for parameter in parameters {
        writer.id(parameter.place);
        writer.u32(parameter.position);
        writer.u8(u8::from(parameter.is_self));
        writer.id(parameter.structural_type);
        writer.u8(match parameter.multiplicity {
            StructuralMultiplicity::Unrestricted => 1,
            StructuralMultiplicity::Affine => 2,
            StructuralMultiplicity::Linear => 3,
        });
        writer.len(
            "structural parameter qualifications",
            parameter.qualifications.len(),
        )?;
        for qualification in &parameter.qualifications {
            writer.id(*qualification);
        }
    }
    Ok(())
}

fn encode_service_ceiling(writer: &mut Writer, services: &[ServiceId]) -> Result<(), CodecError> {
    writer.len("published service ceiling", services.len())?;
    for service in services {
        writer.id(*service);
    }
    Ok(())
}

fn encode_optional_id<I: PsiSemanticId>(writer: &mut Writer, id: Option<I>) {
    match id {
        None => writer.u8(0),
        Some(id) => {
            writer.u8(1);
            writer.id(id);
        }
    }
}

fn encode_proposition_declaration(
    writer: &mut Writer,
    declaration: &PropositionDeclaration,
) -> Result<(), CodecError> {
    writer.id(declaration.id);
    writer.string("proposition name", &declaration.name)?;
    writer.len("proposition binders", declaration.binders.len())?;
    for binder in &declaration.binders {
        writer.string("proposition binder name", &binder.name)?;
        match &binder.kind {
            PropositionBinderKind::Type => writer.u8(1),
            PropositionBinderKind::Const { type_identity } => {
                writer.u8(2);
                writer.string("proposition const binder type", type_identity)?;
            }
            PropositionBinderKind::Machine => writer.u8(3),
        }
    }
    writer.len(
        "proposition parameter types",
        declaration.parameter_types.len(),
    )?;
    for parameter_type in &declaration.parameter_types {
        writer.string("proposition parameter type", parameter_type)?;
    }
    match &declaration.evidence {
        PropositionEvidence::FactOnly => writer.u8(1),
        PropositionEvidence::Witness { evidence_type } => {
            writer.u8(2);
            writer.string("proposition evidence type", evidence_type)?;
        }
    }
    Ok(())
}

fn encode_proposition_application(
    writer: &mut Writer,
    application: &PropositionApplicationIdentity,
) -> Result<(), CodecError> {
    writer.id(application.id);
    writer.id(application.declaration);
    writer.len(
        "proposition binder arguments",
        application.binder_arguments.len(),
    )?;
    for argument in &application.binder_arguments {
        writer.u8(match argument.kind {
            PropositionBinderArgumentKind::Type => 1,
            PropositionBinderArgumentKind::Const => 2,
            PropositionBinderArgumentKind::Machine => 3,
        });
        match &argument.evidence_projection {
            None => {
                writer.u8(0);
                writer.string("proposition binder argument", &argument.identity)?;
            }
            Some(projection) => {
                writer.u8(1);
                writer.id(projection.term);
                writer.string(
                    "evidence projection declaring trait",
                    &projection.declaring_trait_identity,
                )?;
                writer.len(
                    "evidence projection declaring trait arguments",
                    projection.declaring_trait_arguments.len(),
                )?;
                for argument in &projection.declaring_trait_arguments {
                    writer.string("evidence projection declaring trait argument", argument)?;
                }
                writer.string(
                    "evidence projection requirement",
                    &projection.requirement_identity,
                )?;
            }
        }
    }
    writer.len("proposition arguments", application.arguments.len())?;
    for argument in &application.arguments {
        writer.string("proposition argument", argument)?;
    }
    match &application.evidence_interface {
        None => writer.u8(0),
        Some(interface) => {
            writer.u8(1);
            encode_evidence_interface(writer, interface)?;
        }
    }
    Ok(())
}

fn encode_evidence_interface(
    writer: &mut Writer,
    interface: &EvidenceInterfaceIdentity,
) -> Result<(), CodecError> {
    writer.string(
        "evidence interface trait identity",
        &interface.trait_identity,
    )?;
    writer.len("evidence interface arguments", interface.arguments.len())?;
    for argument in &interface.arguments {
        writer.string("evidence interface argument", argument)?;
    }
    writer.len(
        "evidence interface requirements",
        interface.requirements.len(),
    )?;
    for requirement in &interface.requirements {
        writer.string(
            "evidence requirement declaring trait",
            &requirement.declaring_trait_identity,
        )?;
        writer.len(
            "evidence requirement declaring trait arguments",
            requirement.declaring_trait_arguments.len(),
        )?;
        for argument in &requirement.declaring_trait_arguments {
            writer.string("evidence requirement declaring trait argument", argument)?;
        }
        writer.string(
            "evidence requirement identity",
            &requirement.requirement_identity,
        )?;
    }
    Ok(())
}

fn encode_machine(writer: &mut Writer, machine: &TerminalMachine) -> Result<(), CodecError> {
    writer.id(machine.id);
    encode_optional_id(writer, machine.attachment);
    encode_declarations(writer, "machine parameters", &machine.parameters)?;
    encode_structural_parameters(writer, &machine.structural_parameters)?;
    match &machine.result {
        TerminalMachineResult::Unit => writer.u8(0),
        TerminalMachineResult::Scalar(result) => {
            writer.u8(1);
            encode_declaration(writer, *result);
        }
        TerminalMachineResult::Structural(result) => {
            writer.u8(2);
            writer.id(result.place);
            writer.id(result.structural_type);
            writer.u8(match result.multiplicity {
                StructuralMultiplicity::Unrestricted => 1,
                StructuralMultiplicity::Affine => 2,
                StructuralMultiplicity::Linear => 3,
            });
            writer.len(
                "structural result qualifications",
                result.qualifications.len(),
            )?;
            for qualification in &result.qualifications {
                writer.id(*qualification);
            }
        }
    }
    writer.len("structural places", machine.structural_places.len())?;
    for place in &machine.structural_places {
        writer.id(place.id);
        encode_structural_place_kind(writer, place.kind);
    }
    writer.len("entry claims", machine.entry_claims.len())?;
    for claim in &machine.entry_claims {
        writer.id(claim.claim);
        writer.id(claim.input);
        encode_structural_path(writer, "entry claim path", &claim.path)?;
    }
    encode_service_ceiling(writer, &machine.published_service_ceiling)?;
    writer.len("content entry claims", machine.content_entry_claims.len())?;
    for binding in &machine.content_entry_claims {
        encode_content_entry_claim(writer, binding)?;
    }
    writer.len(
        "content identity reshuffles",
        machine.content_identity_reshuffles.len(),
    )?;
    for reshuffle in &machine.content_identity_reshuffles {
        encode_content_identity_reshuffle(writer, reshuffle)?;
    }
    writer.len(
        "content partition compositions",
        machine.content_partition_compositions.len(),
    )?;
    for composition in &machine.content_partition_compositions {
        encode_content_partition_composition(writer, composition)?;
    }
    writer.id(machine.entry);
    writer.len("blocks", machine.blocks.len())?;
    for block in &machine.blocks {
        encode_block(writer, block)?;
    }
    encode_contract(writer, &machine.contract)
}

fn encode_content_entry_claim(
    writer: &mut Writer,
    binding: &ContentEntryClaim,
) -> Result<(), CodecError> {
    writer.id(binding.claim);
    encode_content_structural_place(writer, &binding.input)?;
    encode_claim_content_projections(writer, &binding.projections)
}

fn encode_content_partition_composition(
    writer: &mut Writer,
    composition: &ContentPartitionComposition,
) -> Result<(), CodecError> {
    writer.u64(composition.source_fingerprint);
    writer.len(
        "partition source structural places",
        composition.source_structural_places.len(),
    )?;
    for place in &composition.source_structural_places {
        writer.id(place.id);
        encode_structural_place_kind(writer, place.kind);
    }
    encode_content_conservation(writer, &composition.source)?;
    writer.len("partition input claims", composition.input_claims.len())?;
    for claim in &composition.input_claims {
        writer.id(*claim);
    }
    writer.len(
        "partition place substitutions",
        composition.substitutions.len(),
    )?;
    for substitution in &composition.substitutions {
        encode_content_structural_place(writer, &substitution.source)?;
        encode_content_structural_place(writer, &substitution.target)?;
    }
    encode_content_conservation(writer, &composition.derived)
}

fn encode_content_conservation(
    writer: &mut Writer,
    conservation: &ContentConservation,
) -> Result<(), CodecError> {
    encode_content_algebra(writer, conservation.algebra())?;
    encode_content_term(writer, conservation.left(), 0)?;
    encode_content_term(writer, conservation.right(), 0)
}

fn encode_content_identity_reshuffle(
    writer: &mut Writer,
    reshuffle: &ContentIdentityReshuffle,
) -> Result<(), CodecError> {
    writer.id(reshuffle.claim);
    encode_content_structural_place(writer, &reshuffle.input)?;
    encode_content_structural_place(writer, &reshuffle.output)?;
    encode_claim_content_projections(writer, &reshuffle.projections)
}

fn encode_claim_content_projections(
    writer: &mut Writer,
    projections: &[ClaimContentProjection],
) -> Result<(), CodecError> {
    writer.len("claim content projections", projections.len())?;
    for content in projections {
        writer.id(content.projection.domain);
        writer.u64(content.projection.projection_fingerprint);
        encode_content_algebra(writer, &content.algebra)?;
    }
    Ok(())
}

fn encode_declarations(
    writer: &mut Writer,
    label: &'static str,
    declarations: &[ValueDeclaration],
) -> Result<(), CodecError> {
    writer.len(label, declarations.len())?;
    for declaration in declarations {
        encode_declaration(writer, *declaration);
    }
    Ok(())
}

fn encode_declaration(writer: &mut Writer, declaration: ValueDeclaration) {
    writer.id(declaration.id);
    encode_scalar_type(writer, declaration.scalar_type);
}

fn encode_block(writer: &mut Writer, block: &Block) -> Result<(), CodecError> {
    writer.id(block.id);
    encode_declarations(writer, "block parameters", &block.parameters)?;
    writer.len("operations", block.operations.len())?;
    for operation in &block.operations {
        writer.id(operation.id);
        match operation.result {
            OperationResult::Unit => writer.u8(0),
            OperationResult::Scalar(result) => {
                writer.u8(1);
                encode_declaration(writer, result);
            }
        }
        match operation.kind.clone() {
            OperationKind::EstablishTrivialAffineLocal { destination } => {
                writer.u8(37);
                writer.id(destination);
            }
            OperationKind::Call {
                callee,
                arguments,
                requirement_obligations,
                crash_continuations,
            } => {
                writer.u8(33);
                writer.id(callee);
                writer.len("call arguments", arguments.len())?;
                for argument in arguments {
                    writer.id(argument);
                }
                writer.len(
                    "call requirement obligations",
                    requirement_obligations.len(),
                )?;
                for obligation in requirement_obligations {
                    writer.id(obligation);
                }
                encode_crash_routes(writer, &crash_continuations)?;
            }
            OperationKind::CallUnit {
                callee,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } => {
                writer.u8(34);
                writer.id(callee);
                encode_structural_arguments(writer, &structural_arguments)?;
                writer.len("unit-call claim transfers", claim_transfers.len())?;
                for transfer in claim_transfers {
                    writer.id(transfer.claim);
                    writer.u32(transfer.argument_index);
                }
                encode_obligation_ids(writer, &requirement_obligations)?;
                encode_crash_routes(writer, &crash_continuations)?;
            }
            OperationKind::CallStructuralScalar {
                callee,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } => {
                writer.u8(39);
                writer.id(callee);
                encode_structural_arguments(writer, &structural_arguments)?;
                writer.len(
                    "structural-scalar-call claim transfers",
                    claim_transfers.len(),
                )?;
                for transfer in claim_transfers {
                    writer.id(transfer.claim);
                    writer.u32(transfer.argument_index);
                }
                encode_obligation_ids(writer, &requirement_obligations)?;
                encode_crash_routes(writer, &crash_continuations)?;
            }
            OperationKind::BoundaryCall {
                boundary,
                structural_arguments,
                completion_receipts,
                requirement_obligations,
            } => {
                writer.u8(35);
                writer.id(boundary);
                encode_structural_arguments(writer, &structural_arguments)?;
                writer.len("boundary claim settlements", completion_receipts.len())?;
                for settlement in completion_receipts {
                    writer.id(settlement.claim);
                    writer.u32(settlement.argument_index);
                }
                encode_obligation_ids(writer, &requirement_obligations)?;
            }
            OperationKind::PortWrite {
                service,
                port,
                value,
            } => {
                writer.u8(36);
                writer.id(service);
                writer.u16(port);
                writer.u8(value);
            }
            OperationKind::IntegerConstant { value } => {
                writer.u8(1);
                encode_integer_value(writer, value);
            }
            OperationKind::BooleanConstant { value } => {
                writer.u8(2);
                writer.u8(u8::from(value));
            }
            OperationKind::BooleanStructuralField { source, field } => {
                writer.u8(38);
                writer.id(source);
                writer.id(field);
            }
            OperationKind::BooleanNot { operand } => {
                writer.u8(9);
                writer.id(operand);
            }
            OperationKind::BooleanEqual { left, right } => {
                writer.u8(10);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::IntegerEqual { left, right } => {
                writer.u8(11);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::IntegerLessThan { left, right } => {
                writer.u8(12);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::IntegerLessOrEqual { left, right } => {
                writer.u8(13);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::IntegerBitwiseNot { operand } => {
                writer.u8(19);
                writer.id(operand);
            }
            OperationKind::IntegerWiden { operand } => {
                writer.u8(20);
                writer.id(operand);
            }
            OperationKind::IntegerExactCast {
                operand,
                obligation,
            } => {
                writer.u8(21);
                writer.id(operand);
                writer.id(obligation);
            }
            OperationKind::IntegerBitwiseAnd { left, right } => {
                writer.u8(14);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::IntegerBitwiseOr { left, right } => {
                writer.u8(15);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::IntegerBitwiseXor { left, right } => {
                writer.u8(16);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::WrappingIntegerShiftLeft { value, count } => {
                writer.u8(17);
                writer.id(value);
                writer.id(count);
            }
            OperationKind::WrappingIntegerShiftRight { value, count } => {
                writer.u8(18);
                writer.id(value);
                writer.id(count);
            }
            OperationKind::ExactIntegerShiftLeft {
                value,
                count,
                obligation,
            } => {
                writer.u8(23);
                writer.id(value);
                writer.id(count);
                writer.id(obligation);
            }
            OperationKind::ExactIntegerShiftRight {
                value,
                count,
                obligation,
            } => {
                writer.u8(22);
                writer.id(value);
                writer.id(count);
                writer.id(obligation);
            }
            OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation,
            } => {
                writer.u8(24);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation,
            } => {
                writer.u8(25);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation,
            } => {
                writer.u8(26);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::ExactIntegerDivide {
                left,
                right,
                obligation,
            } => {
                writer.u8(27);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation,
            } => {
                writer.u8(28);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::WrappingIntegerDivide {
                left,
                right,
                obligation,
            } => {
                writer.u8(29);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::WrappingIntegerRemainder {
                left,
                right,
                obligation,
            } => {
                writer.u8(30);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::SaturatingIntegerDivide {
                left,
                right,
                obligation,
            } => {
                writer.u8(31);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::SaturatingIntegerRemainder {
                left,
                right,
                obligation,
            } => {
                writer.u8(32);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::WrappingIntegerAdd { left, right } => {
                writer.u8(3);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::SaturatingIntegerAdd { left, right } => {
                writer.u8(4);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::WrappingIntegerSubtract { left, right } => {
                writer.u8(5);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::SaturatingIntegerSubtract { left, right } => {
                writer.u8(6);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::WrappingIntegerMultiply { left, right } => {
                writer.u8(7);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::SaturatingIntegerMultiply { left, right } => {
                writer.u8(8);
                writer.id(left);
                writer.id(right);
            }
        }
    }
    match &block.terminator {
        Terminator::Jump {
            edge,
            target,
            arguments,
            trivial_affine_discards,
        } => {
            writer.u8(1);
            writer.id(*edge);
            writer.id(*target);
            writer.len("jump arguments", arguments.len())?;
            for argument in arguments {
                writer.id(*argument);
            }
            writer.len(
                "jump trivial affine discards",
                trivial_affine_discards.len(),
            )?;
            for place in trivial_affine_discards {
                writer.id(*place);
            }
        }
        Terminator::Return {
            edge,
            value,
            cleanup_actions,
        } => {
            writer.u8(2);
            writer.id(*edge);
            writer.id(*value);
            writer.len("scalar return cleanup actions", cleanup_actions.len())?;
            for action in cleanup_actions {
                encode_affine_cleanup_action(writer, action)?;
            }
        }
        Terminator::ReturnUnit {
            edge,
            trivial_affine_discards,
        } => {
            writer.u8(5);
            writer.id(*edge);
            writer.len(
                "return Unit trivial affine discards",
                trivial_affine_discards.len(),
            )?;
            for place in trivial_affine_discards {
                writer.id(*place);
            }
        }
        Terminator::ReturnUnitPartialAffine {
            edge,
            trivial_affine_discards,
            residual_affine_discards,
        } => {
            writer.u8(7);
            writer.id(*edge);
            writer.len(
                "partial Unit return trivial affine discards",
                trivial_affine_discards.len(),
            )?;
            for place in trivial_affine_discards {
                writer.id(*place);
            }
            writer.len(
                "partial Unit return residual affine discards",
                residual_affine_discards.len(),
            )?;
            for discard in residual_affine_discards {
                writer.id(discard.place);
                encode_structural_path(writer, "partial affine discard path", &discard.path)?;
                writer.id(discard.structural_type);
            }
        }
        Terminator::ReturnUnitNominalAffine { edge, cleanups } => {
            writer.u8(8);
            writer.id(*edge);
            writer.len("nominal affine cleanups", cleanups.len())?;
            for cleanup in cleanups {
                writer.id(cleanup.place);
                writer.id(cleanup.structural_type);
                writer.id(cleanup.cleanup_machine);
                encode_optional_id(writer, cleanup.cleanup_receiver);
                encode_obligation_ids(writer, &cleanup.requirement_obligations)?;
            }
        }
        Terminator::ReturnStructural {
            edge,
            source,
            returned_claims,
            trivial_affine_discards,
        } => {
            writer.u8(6);
            writer.id(*edge);
            writer.id(*source);
            writer.len("structural return claims", returned_claims.len())?;
            for claim in returned_claims {
                writer.id(*claim);
            }
            writer.len(
                "structural return trivial affine discards",
                trivial_affine_discards.len(),
            )?;
            for place in trivial_affine_discards {
                writer.id(*place);
            }
        }
        Terminator::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            writer.u8(3);
            writer.id(*condition);
            encode_successor_edge(writer, when_true)?;
            encode_successor_edge(writer, when_false)?;
        }
        Terminator::Crash {
            edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => {
            writer.u8(4);
            writer.id(*edge);
            writer.u8(match cause {
                CrashCause::Trap => 1,
                CrashCause::Abort => 2,
            });
            writer.len("crash site guard", site_guard.len())?;
            for predicate in site_guard {
                encode_crash_predicate(writer, predicate)?;
            }
            writer.len("crash frontier lower bound", frontier_lower_bound.len())?;
            for claim in frontier_lower_bound {
                writer.id(*claim);
            }
        }
    }
    Ok(())
}

fn encode_structural_arguments(
    writer: &mut Writer,
    arguments: &[StructuralArgument],
) -> Result<(), CodecError> {
    writer.len("structural arguments", arguments.len())?;
    for argument in arguments {
        writer.id(argument.place);
        encode_structural_path(writer, "structural argument path", &argument.path)?;
    }
    Ok(())
}

fn encode_affine_cleanup_action(
    writer: &mut Writer,
    action: &TerminalAffineCleanupAction,
) -> Result<(), CodecError> {
    match action {
        TerminalAffineCleanupAction::DiscardRoot(place) => {
            writer.u8(1);
            writer.id(*place);
        }
        TerminalAffineCleanupAction::DiscardResidual(discard) => {
            writer.u8(2);
            writer.id(discard.place);
            encode_structural_path(writer, "affine cleanup residual path", &discard.path)?;
            writer.id(discard.structural_type);
        }
        TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
            writer.u8(3);
            writer.id(cleanup.place);
            writer.id(cleanup.structural_type);
            writer.id(cleanup.cleanup_machine);
            encode_optional_id(writer, cleanup.cleanup_receiver);
            encode_obligation_ids(writer, &cleanup.requirement_obligations)?;
        }
    }
    Ok(())
}

fn encode_structural_path(
    writer: &mut Writer,
    label: &'static str,
    path: &[StructuralPathSegment],
) -> Result<(), CodecError> {
    writer.len(label, path.len())?;
    for segment in path {
        match segment {
            StructuralPathSegment::Field(identity) => {
                writer.u8(1);
                writer.string("structural path field", identity)?;
            }
            StructuralPathSegment::FixedIndex(index) => {
                writer.u8(2);
                writer.u64(*index);
            }
        }
    }
    Ok(())
}

fn encode_obligation_ids(
    writer: &mut Writer,
    obligations: &[ObligationId],
) -> Result<(), CodecError> {
    writer.len("requirement obligations", obligations.len())?;
    for obligation in obligations {
        writer.id(*obligation);
    }
    Ok(())
}

fn encode_successor_edge(writer: &mut Writer, successor: &SuccessorEdge) -> Result<(), CodecError> {
    writer.id(successor.edge);
    writer.id(successor.target);
    writer.len("conditional successor arguments", successor.arguments.len())?;
    for argument in &successor.arguments {
        writer.id(*argument);
    }
    writer.len(
        "conditional successor trivial affine discards",
        successor.trivial_affine_discards.len(),
    )?;
    for place in &successor.trivial_affine_discards {
        writer.id(*place);
    }
    Ok(())
}

fn encode_contract(writer: &mut Writer, contract: &MachineContract) -> Result<(), CodecError> {
    writer.id(contract.id);
    encode_crash_routes(writer, &contract.crash_routes)?;
    writer.len("requires", contract.requires.len())?;
    for proposition in &contract.requires {
        encode_proposition(writer, proposition, 0)?;
    }
    writer.len("ensures", contract.ensures.len())?;
    for clause in &contract.ensures {
        writer.id(clause.obligation);
        encode_proposition(writer, &clause.proposition, 0)?;
    }
    Ok(())
}

fn encode_crash_routes(
    writer: &mut Writer,
    crash_routes: &[CrashRouteBucket],
) -> Result<(), CodecError> {
    writer.len("crash route buckets", crash_routes.len())?;
    for bucket in crash_routes {
        writer.u8(match bucket.cause {
            CrashCause::Trap => 1,
            CrashCause::Abort => 2,
        });
        writer.len("crash route alternatives", bucket.alternatives.len())?;
        for guard in &bucket.alternatives {
            match guard {
                CrashRouteGuard::Truth => writer.u8(0),
                CrashRouteGuard::Predicate(predicate) => {
                    writer.u8(1);
                    encode_crash_predicate(writer, predicate)?;
                }
            }
        }
    }
    Ok(())
}

fn encode_crash_predicate(
    writer: &mut Writer,
    predicate: &CrashPredicateTerm,
) -> Result<(), CodecError> {
    encode_proposition(writer, predicate.proposition(), 0)
}

fn encode_proposition(
    writer: &mut Writer,
    proposition: &Proposition,
    depth: usize,
) -> Result<(), CodecError> {
    if depth > MAX_PROPOSITION_DEPTH {
        return Err(CodecError::PropositionNestingTooDeep);
    }
    match proposition {
        Proposition::Truth => writer.u8(1),
        Proposition::Falsehood => writer.u8(2),
        Proposition::Atom(id) => {
            writer.u8(3);
            writer.id(*id);
        }
        Proposition::Equal(left, right) => {
            writer.u8(4);
            encode_scalar_term(writer, left, 0)?;
            encode_scalar_term(writer, right, 0)?;
        }
        Proposition::LessThan(left, right) => {
            writer.u8(5);
            encode_scalar_term(writer, left, 0)?;
            encode_scalar_term(writer, right, 0)?;
        }
        Proposition::LessOrEqual(left, right) => {
            writer.u8(6);
            encode_scalar_term(writer, left, 0)?;
            encode_scalar_term(writer, right, 0)?;
        }
        Proposition::Conjunction(conjuncts) => {
            writer.u8(7);
            writer.len("conjuncts", conjuncts.len())?;
            for conjunct in conjuncts {
                encode_proposition(writer, conjunct, depth + 1)?;
            }
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            writer.u8(8);
            encode_proposition(writer, premise, depth + 1)?;
            encode_proposition(writer, conclusion, depth + 1)?;
        }
        Proposition::ContentConservation(conservation) => {
            writer.u8(9);
            encode_content_algebra(writer, conservation.algebra())?;
            encode_content_term(writer, conservation.left(), 0)?;
            encode_content_term(writer, conservation.right(), 0)?;
        }
        Proposition::Disjunction(disjuncts) => {
            writer.u8(10);
            writer.len("disjuncts", disjuncts.len())?;
            for disjunct in disjuncts {
                encode_proposition(writer, disjunct, depth + 1)?;
            }
        }
        Proposition::IeeeFloatComparison {
            kind,
            format,
            left,
            right,
        } => {
            writer.u8(11);
            encode_ieee_float_comparison_kind(writer, *kind);
            encode_ieee_float_format(writer, *format);
            encode_ieee_float_field(writer, left)?;
            encode_ieee_float_field(writer, right)?;
        }
        Proposition::ByteSequenceEqual { left, right } => {
            writer.u8(12);
            encode_byte_sequence_field(writer, left)?;
            encode_byte_sequence_field(writer, right)?;
        }
        Proposition::StructuralCaseMembership { subject, case } => {
            writer.u8(13);
            encode_canonical_structural_field(
                writer,
                subject.root(),
                subject.path(),
                "structural case subject path",
            )?;
            writer.id(*case);
        }
    }
    Ok(())
}

fn encode_structural_place_kind(writer: &mut Writer, kind: StructuralPlaceKind) {
    match kind {
        StructuralPlaceKind::Parameter { position, is_self } => {
            writer.u8(1);
            writer.u32(position);
            writer.u8(u8::from(is_self));
        }
        StructuralPlaceKind::Result => writer.u8(2),
        StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal,
            structural_type,
        } => {
            writer.u8(3);
            writer.u32(declaration_ordinal);
            writer.id(structural_type);
        }
    }
}

fn encode_content_algebra(writer: &mut Writer, algebra: &ContentAlgebra) -> Result<(), CodecError> {
    writer.u8(match algebra.kind {
        ContentAlgebraKind::IntervalSet => 1,
        ContentAlgebraKind::CountedQuantity => 2,
    });
    writer.string("content algebra parameter", &algebra.parameter)
}

fn encode_content_term(
    writer: &mut Writer,
    term: &ContentTerm,
    depth: usize,
) -> Result<(), CodecError> {
    if depth > MAX_CONTENT_TERM_DEPTH {
        return Err(CodecError::ContentTermNestingTooDeep);
    }
    match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => {
            writer.u8(1);
            writer.id(projection.domain);
            writer.u64(projection.projection_fingerprint);
            encode_content_structural_place(writer, subject)?;
        }
        ContentTerm::Separate(terms) => {
            writer.u8(2);
            writer.len("separated content terms", terms.len())?;
            for term in terms {
                encode_content_term(writer, term, depth + 1)?;
            }
        }
    }
    Ok(())
}

fn encode_content_structural_place(
    writer: &mut Writer,
    subject: &ContentStructuralPlace,
) -> Result<(), CodecError> {
    writer.u8(match subject.version {
        ContentPlaceVersion::Entry => 1,
        ContentPlaceVersion::Current => 2,
    });
    writer.id(subject.root);
    writer.len("content place segments", subject.segments.len())?;
    for segment in &subject.segments {
        match segment {
            ContentPlaceSegment::Case(name) => {
                writer.u8(3);
                writer.string("content case", name)?;
            }
            ContentPlaceSegment::Field(name) => {
                writer.u8(1);
                writer.string("content field", name)?;
            }
            ContentPlaceSegment::FixedIndex(index) => {
                writer.u8(2);
                writer.u64(*index);
            }
        }
    }
    Ok(())
}

fn encode_scalar_term(
    writer: &mut Writer,
    term: &ScalarTerm,
    depth: usize,
) -> Result<(), CodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(CodecError::ScalarTermNestingTooDeep);
    }
    match term {
        ScalarTerm::Value { id, scalar_type } => {
            writer.u8(1);
            writer.id(*id);
            encode_scalar_type(writer, *scalar_type);
        }
        ScalarTerm::BooleanField { root, path } => {
            writer.u8(34);
            writer.id(*root);
            writer.len("Boolean field path", path.len())?;
            for segment in path {
                match segment {
                    CanonicalStructuralPathSegment::Field(field) => {
                        writer.u8(1);
                        writer.id(*field);
                    }
                    CanonicalStructuralPathSegment::FixedIndex(index) => {
                        writer.u8(2);
                        writer.u64(*index);
                    }
                    CanonicalStructuralPathSegment::Case(case) => {
                        writer.u8(3);
                        writer.id(*case);
                    }
                }
            }
        }
        ScalarTerm::IntegerField {
            root,
            path,
            scalar_type,
        } => {
            writer.u8(35);
            writer.id(*root);
            writer.len("Integer field path", path.len())?;
            for segment in path {
                match segment {
                    CanonicalStructuralPathSegment::Field(field) => {
                        writer.u8(1);
                        writer.id(*field);
                    }
                    CanonicalStructuralPathSegment::FixedIndex(index) => {
                        writer.u8(2);
                        writer.u64(*index);
                    }
                    CanonicalStructuralPathSegment::Case(case) => {
                        writer.u8(3);
                        writer.id(*case);
                    }
                }
            }
            encode_integer_type(writer, *scalar_type);
        }
        ScalarTerm::Boolean(value) => {
            writer.u8(2);
            writer.u8(u8::from(*value));
        }
        ScalarTerm::BooleanNot { operand } => {
            writer.u8(10);
            encode_scalar_term(writer, operand, depth + 1)?;
        }
        ScalarTerm::BooleanEqual { left, right } => {
            writer.u8(11);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::IntegerEqual {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(12);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::IntegerLessThan {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(13);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(14);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => {
            writer.u8(20);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, operand, depth + 1)?;
        }
        ScalarTerm::IntegerWiden {
            source_type,
            target_type,
            operand,
        } => {
            writer.u8(21);
            encode_integer_type(writer, *source_type);
            encode_integer_type(writer, *target_type);
            encode_scalar_term(writer, operand, depth + 1)?;
        }
        ScalarTerm::IntegerExactCast {
            source_type,
            target_type,
            operand,
        } => {
            writer.u8(22);
            encode_integer_type(writer, *source_type);
            encode_integer_type(writer, *target_type);
            encode_scalar_term(writer, operand, depth + 1)?;
        }
        ScalarTerm::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(15);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::IntegerBitwiseOr {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(16);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::IntegerBitwiseXor {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(17);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => {
            writer.u8(18);
            encode_integer_type(writer, *value_type);
            encode_integer_type(writer, *count_type);
            encode_scalar_term(writer, value, depth + 1)?;
            encode_scalar_term(writer, count, depth + 1)?;
        }
        ScalarTerm::WrappingIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => {
            writer.u8(19);
            encode_integer_type(writer, *value_type);
            encode_integer_type(writer, *count_type);
            encode_scalar_term(writer, value, depth + 1)?;
            encode_scalar_term(writer, count, depth + 1)?;
        }
        ScalarTerm::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => {
            writer.u8(23);
            encode_integer_type(writer, *value_type);
            encode_integer_type(writer, *count_type);
            encode_scalar_term(writer, value, depth + 1)?;
            encode_scalar_term(writer, count, depth + 1)?;
        }
        ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => {
            writer.u8(24);
            encode_integer_type(writer, *value_type);
            encode_integer_type(writer, *count_type);
            encode_scalar_term(writer, value, depth + 1)?;
            encode_scalar_term(writer, count, depth + 1)?;
        }
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(25);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(26);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(27);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::ExactIntegerDivide {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(28);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(29);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(30);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(31);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::SaturatingIntegerDivide {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(32);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::SaturatingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(33);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::Integer { scalar_type, value } => {
            writer.u8(3);
            encode_integer_type(writer, *scalar_type);
            encode_integer_value(writer, *value);
        }
        ScalarTerm::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(4);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(5);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(6);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(7);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(8);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
        ScalarTerm::SaturatingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(9);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1)?;
            encode_scalar_term(writer, right, depth + 1)?;
        }
    }
    Ok(())
}

fn encode_scalar_type(writer: &mut Writer, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Boolean => writer.u8(1),
        ScalarType::Integer(integer_type) => {
            writer.u8(2);
            encode_integer_type(writer, integer_type);
        }
    }
}

fn encode_integer_type(writer: &mut Writer, integer_type: IntegerType) {
    writer.u8(match (integer_type.carrier(), integer_type.sign()) {
        (IntegerCarrier::Fixed, IntegerSign::Signed) => 1,
        (IntegerCarrier::Fixed, IntegerSign::Unsigned) => 2,
        (IntegerCarrier::Address, IntegerSign::Unsigned) => 3,
        (IntegerCarrier::Address, IntegerSign::Signed) => {
            unreachable!("address carriers are unsigned")
        }
    });
    writer.u16(integer_type.bits());
}

fn encode_integer_value(writer: &mut Writer, value: IntegerValue) {
    match value {
        IntegerValue::Signed(value) => {
            writer.u8(1);
            writer.bytes(&value.to_le_bytes());
        }
        IntegerValue::Unsigned(value) => {
            writer.u8(2);
            writer.bytes(&value.to_le_bytes());
        }
    }
}

fn decode_module_body(reader: &mut Reader<'_>) -> Result<TerminalModule, CodecError> {
    let vocabulary_marker_raw = reader.u16()?;
    let vocabulary_marker = VocabularyMarker::new(vocabulary_marker_raw).ok_or(
        CodecError::UnsupportedVocabularyMarker(vocabulary_marker_raw),
    )?;
    let entry = reader.id("MachineId")?;
    let structural_types = decode_counted(reader, decode_structural_type)?;
    let structural_domains = decode_counted(reader, |reader| {
        Ok(StructuralDomainDeclaration {
            id: reader.id("StructuralDomainId")?,
            identity: reader.string("structural domain identity")?,
            carrier: reader.id("StructuralTypeId")?,
        })
    })?;
    let services = decode_counted(reader, |reader| {
        Ok(ServiceDeclaration {
            id: reader.id("ServiceId")?,
            identity: reader.string("service identity")?,
            parents: decode_ids(reader, "ServiceId")?,
        })
    })?;
    let boundary_machines = decode_counted(reader, decode_boundary_machine)?;
    let provider_candidates = decode_counted(reader, decode_provider_candidate)?;
    let float_meaning_projections = decode_counted(reader, |reader| {
        Ok(FloatMeaningProjection {
            result: ProofValueDeclaration {
                id: ProofValueId(reader.u32()?),
                value_type: match reader.u8()? {
                    1 => ProofOnlyValueType::FloatMeaning,
                    tag => return Err(CodecError::InvalidTag("ProofOnlyValueType", tag)),
                },
            },
            source: FloatProjectionInput {
                id: FloatProjectionInputId(reader.u32()?),
                format: match reader.u8()? {
                    1 => IeeeFloatFormat::Binary32,
                    2 => IeeeFloatFormat::Binary64,
                    tag => return Err(CodecError::InvalidTag("IeeeFloatFormat", tag)),
                },
            },
            operation: match reader.u8()? {
                1 => FloatMeaningProjectionOperation::Meaning32,
                2 => FloatMeaningProjectionOperation::Meaning64,
                tag => {
                    return Err(CodecError::InvalidTag(
                        "FloatMeaningProjectionOperation",
                        tag,
                    ));
                }
            },
        })
    })?;
    let float_meaning_equalities = decode_counted(reader, |reader| {
        Ok(FloatMeaningEqualityProposition {
            id: ProofPropositionId(reader.u32()?),
            left: ProofValueId(reader.u32()?),
            right: ProofValueId(reader.u32()?),
        })
    })?;
    let count = reader.count()?;
    let mut proposition_declarations = Vec::with_capacity(count as usize);
    for _ in 0..count {
        proposition_declarations.push(decode_proposition_declaration(reader)?);
    }
    let count = reader.count()?;
    let mut proposition_applications = Vec::with_capacity(count as usize);
    for _ in 0..count {
        proposition_applications.push(decode_proposition_application(reader)?);
    }
    let evidence_terms = decode_counted(reader, |reader| {
        Ok(EvidenceTermDeclaration {
            id: reader.id("EvidenceTermId")?,
            proposition: reader.id("PropositionId")?,
            interface: decode_evidence_interface(reader)?,
        })
    })?;
    let evidence_contract_lanes = decode_counted(reader, |reader| {
        let machine = reader.id("MachineId")?;
        let kind = match reader.u8()? {
            1 => EvidenceContractLaneKind::Requires,
            2 => EvidenceContractLaneKind::Ensures,
            tag => return Err(CodecError::InvalidTag("EvidenceContractLaneKind", tag)),
        };
        Ok(EvidenceContractLane {
            machine,
            kind,
            position: reader.u32()?,
            term: reader.id("EvidenceTermId")?,
            output_field: reader
                .boolean()?
                .then(|| reader.string("evidence output field"))
                .transpose()?,
        })
    })?;
    let evidence_package_invocations = decode_counted(reader, |reader| {
        Ok(EvidencePackageInvocation {
            caller: reader.id("MachineId")?,
            ordinal: reader.u32()?,
            target_machine_identity: reader.string("evidence package target machine identity")?,
            runtime_value: reader
                .boolean()?
                .then(|| decode_scalar_type(reader))
                .transpose()?,
            runtime_call: reader
                .boolean()?
                .then(|| {
                    Ok(EvidencePackageRuntimeCall {
                        operation: reader.id("OperationId")?,
                        callee: reader.id("MachineId")?,
                    })
                })
                .transpose()?,
            outputs: decode_counted(reader, |reader| {
                Ok(EvidencePackageOutputBinding {
                    output_position: reader.u32()?,
                    output_field: reader.string("evidence package output field")?,
                    callee_output: reader.id("EvidenceTermId")?,
                    output: reader
                        .boolean()?
                        .then(|| reader.id("EvidenceTermId"))
                        .transpose()?,
                })
            })?,
        })
    })?;
    let closed_conformance_applications = decode_counted(reader, |reader| {
        Ok(ClosedConformanceApplication {
            owner: reader.id("MachineId")?,
            declaration_identity: reader.string("closed conformance declaration identity")?,
            telescope: decode_counted(reader, |reader| {
                Ok(ClosedConformanceParameterBinding {
                    parameter: reader.string("closed conformance parameter")?,
                    kind: match reader.u8()? {
                        1 => ClosedConformanceParameterKind::Lifetime,
                        2 => ClosedConformanceParameterKind::Type,
                        3 => ClosedConformanceParameterKind::Const,
                        4 => ClosedConformanceParameterKind::Machine,
                        tag => {
                            return Err(CodecError::InvalidTag(
                                "ClosedConformanceParameterKind",
                                tag,
                            ));
                        }
                    },
                    argument: reader.string("closed conformance argument")?,
                })
            })?,
            subject_identity: reader
                .boolean()?
                .then(|| reader.string("closed conformance subject identity"))
                .transpose()?,
            trait_identity: reader.string("closed conformance trait identity")?,
            trait_arguments: reader.strings("closed conformance trait arguments")?,
            rows: decode_counted(reader, |reader| {
                Ok(ClosedConformanceRow {
                    declaring_trait_identity: reader
                        .string("closed conformance row declaring trait identity")?,
                    requirement_identity: reader
                        .string("closed conformance row requirement identity")?,
                    realization_identity: reader
                        .string("closed conformance row realization identity")?,
                })
            })?,
            fingerprint: reader.u64()?,
        })
    })?;
    let machine_count = reader.count()?;
    let mut machines = Vec::new();
    for _ in 0..machine_count {
        machines.push(decode_machine(reader)?);
    }
    Ok(TerminalModule {
        vocabulary_marker,
        entry,
        structural_types,
        structural_domains,
        services,
        boundary_machines,
        provider_candidates,
        float_meaning_projections,
        float_meaning_equalities,
        proposition_declarations,
        proposition_applications,
        evidence_terms,
        evidence_contract_lanes,
        evidence_package_invocations,
        closed_conformance_applications,
        machines,
    })
}

fn decode_provider_candidate(
    reader: &mut Reader<'_>,
) -> Result<ProviderCandidateConformance, CodecError> {
    let boundary = reader.id("BoundaryMachineId")?;
    let requirement_identity = reader.string("provider requirement identity")?;
    let provider_identity = reader.string("provider identity")?;
    let candidate_identity = reader.string("provider candidate identity")?;
    let candidate = reader.id("MachineId")?;
    let parameters = decode_counted(reader, |reader| {
        let position = reader.u32()?;
        let is_self = reader.boolean()?;
        let structural_type = reader.id("StructuralTypeId")?;
        let multiplicity = match reader.u8()? {
            1 => StructuralMultiplicity::Unrestricted,
            2 => StructuralMultiplicity::Affine,
            3 => StructuralMultiplicity::Linear,
            tag => return Err(CodecError::InvalidTag("StructuralMultiplicity", tag)),
        };
        Ok(ProviderSignatureParameter {
            position,
            is_self,
            structural_type,
            multiplicity,
            qualifications: decode_ids(reader, "StructuralDomainId")?,
        })
    })?;
    let positional_parameters = decode_counted(reader, |reader| {
        Ok(ProviderParameterRefinement {
            boundary_index: reader.u32()?,
            candidate_index: reader.u32()?,
        })
    })?;
    let required_domains = decode_counted(reader, |reader| {
        Ok(StructuralDomainRequirement {
            argument_index: reader.u32()?,
            domain: reader.id("StructuralDomainId")?,
        })
    })?;
    Ok(ProviderCandidateConformance {
        boundary,
        requirement_identity,
        provider_identity,
        candidate_identity,
        candidate,
        signature: ProviderUnitSignature { parameters },
        refinement: ProviderUnitRefinement {
            positional_parameters,
            required_domains,
            realized_service_ceiling: decode_ids(reader, "ServiceId")?,
        },
    })
}

fn decode_structural_type(
    reader: &mut Reader<'_>,
) -> Result<StructuralTypeDeclaration, CodecError> {
    let id = reader.id("StructuralTypeId")?;
    let identity = reader.string("structural type identity")?;
    let shape = match reader.u8()? {
        1 => StructuralTypeShape::Record {
            fields: decode_counted(reader, decode_structural_field)?,
        },
        2 => StructuralTypeShape::FixedArray {
            element: reader.id("StructuralTypeId")?,
            length: reader.u64()?,
        },
        3 => StructuralTypeShape::Sum {
            cases: decode_counted(reader, |reader| {
                Ok(StructuralCaseDeclaration {
                    id: reader.id("StructuralCaseId")?,
                    identity: reader.string("structural case identity")?,
                    fields: decode_counted(reader, decode_structural_field)?,
                })
            })?,
        },
        tag => return Err(CodecError::InvalidTag("StructuralTypeShape", tag)),
    };
    Ok(StructuralTypeDeclaration {
        id,
        identity,
        shape,
    })
}

fn decode_structural_field(
    reader: &mut Reader<'_>,
) -> Result<StructuralFieldDeclaration, CodecError> {
    let id = reader.id("StructuralFieldId")?;
    let identity = reader.string("structural field identity")?;
    let relevance = match reader.u8()? {
        1 => BindingRelevance::Relevant,
        2 => BindingRelevance::Erased,
        tag => return Err(CodecError::InvalidTag("BindingRelevance", tag)),
    };
    let field_type = match reader.u8()? {
        1 => StructuralFieldType::Scalar(decode_scalar_type(reader)?),
        2 => StructuralFieldType::Structural(reader.id("StructuralTypeId")?),
        3 => StructuralFieldType::Erased {
            type_identity: reader.string("erased structural field type identity")?,
        },
        4 => StructuralFieldType::IeeeFloat(decode_ieee_float_format(reader)?),
        5 => StructuralFieldType::ByteSequence(decode_byte_sequence_carrier(reader)?),
        tag => return Err(CodecError::InvalidTag("StructuralFieldType", tag)),
    };
    Ok(StructuralFieldDeclaration {
        id,
        identity,
        relevance,
        field_type,
    })
}

fn decode_boundary_machine(
    reader: &mut Reader<'_>,
) -> Result<BoundaryMachineDeclaration, CodecError> {
    Ok(BoundaryMachineDeclaration {
        id: reader.id("BoundaryMachineId")?,
        identity: reader.string("boundary machine identity")?,
        attachment: decode_optional_id(reader, "StructuralTypeId")?,
        structural_parameters: decode_structural_parameters(reader)?,
        result: reader
            .boolean()?
            .then(|| decode_scalar_type(reader))
            .transpose()?,
        requires: decode_counted(reader, |reader| {
            Ok(StructuralDomainRequirement {
                argument_index: reader.u32()?,
                domain: reader.id("StructuralDomainId")?,
            })
        })?,
        published_service_ceiling: decode_ids(reader, "ServiceId")?,
    })
}

fn decode_structural_parameters(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralParameterDeclaration>, CodecError> {
    decode_counted(reader, |reader| {
        let place = reader.id("PlaceId")?;
        let position = reader.u32()?;
        let is_self = reader.boolean()?;
        let structural_type = reader.id("StructuralTypeId")?;
        let multiplicity = match reader.u8()? {
            1 => StructuralMultiplicity::Unrestricted,
            2 => StructuralMultiplicity::Affine,
            3 => StructuralMultiplicity::Linear,
            tag => return Err(CodecError::InvalidTag("StructuralMultiplicity", tag)),
        };
        Ok(StructuralParameterDeclaration {
            place,
            position,
            is_self,
            structural_type,
            multiplicity,
            qualifications: decode_ids(reader, "StructuralDomainId")?,
        })
    })
}

fn decode_counted<T>(
    reader: &mut Reader<'_>,
    mut decode: impl FnMut(&mut Reader<'_>) -> Result<T, CodecError>,
) -> Result<Vec<T>, CodecError> {
    let count = reader.count()?;
    let count = usize::try_from(count).map_err(|_| CodecError::UnexpectedEnd)?;
    if count > reader.remaining() {
        return Err(CodecError::UnexpectedEnd);
    }
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(decode(reader)?);
    }
    Ok(values)
}

fn decode_ids<I: PsiSemanticId>(
    reader: &mut Reader<'_>,
    label: &'static str,
) -> Result<Vec<I>, CodecError> {
    decode_counted(reader, |reader| reader.id(label))
}

fn decode_optional_id<I: PsiSemanticId>(
    reader: &mut Reader<'_>,
    label: &'static str,
) -> Result<Option<I>, CodecError> {
    match reader.u8()? {
        0 => Ok(None),
        1 => Ok(Some(reader.id(label)?)),
        tag => Err(CodecError::InvalidTag("OptionalSemanticId", tag)),
    }
}

fn decode_proposition_declaration(
    reader: &mut Reader<'_>,
) -> Result<PropositionDeclaration, CodecError> {
    let id = reader.id("PropositionId")?;
    let name = reader.string("proposition name")?;
    let binder_count = reader.count()?;
    let mut binders = Vec::with_capacity(binder_count as usize);
    for _ in 0..binder_count {
        let name = reader.string("proposition binder name")?;
        let kind = match reader.u8()? {
            1 => PropositionBinderKind::Type,
            2 => PropositionBinderKind::Const {
                type_identity: reader.string("proposition const binder type")?,
            },
            3 => PropositionBinderKind::Machine,
            tag => return Err(CodecError::InvalidTag("PropositionBinderKind", tag)),
        };
        binders.push(PropositionBinderDeclaration { name, kind });
    }
    let parameter_count = reader.count()?;
    let mut parameter_types = Vec::with_capacity(parameter_count as usize);
    for _ in 0..parameter_count {
        parameter_types.push(reader.string("proposition parameter type")?);
    }
    let evidence = match reader.u8()? {
        1 => PropositionEvidence::FactOnly,
        2 => PropositionEvidence::Witness {
            evidence_type: reader.string("proposition evidence type")?,
        },
        tag => return Err(CodecError::InvalidTag("PropositionEvidence", tag)),
    };
    Ok(PropositionDeclaration {
        id,
        name,
        binders,
        parameter_types,
        evidence,
    })
}

fn decode_proposition_application(
    reader: &mut Reader<'_>,
) -> Result<PropositionApplicationIdentity, CodecError> {
    let id = reader.id("PropositionId")?;
    let declaration = reader.id("PropositionId")?;
    let binder_count = reader.count()?;
    let mut binder_arguments = Vec::with_capacity(binder_count as usize);
    for _ in 0..binder_count {
        let kind = match reader.u8()? {
            1 => PropositionBinderArgumentKind::Type,
            2 => PropositionBinderArgumentKind::Const,
            3 => PropositionBinderArgumentKind::Machine,
            tag => {
                return Err(CodecError::InvalidTag("PropositionBinderArgumentKind", tag));
            }
        };
        let (identity, evidence_projection) = match reader.u8()? {
            0 => (reader.string("proposition binder argument")?, None),
            1 => (
                String::new(),
                Some(psi_terminal::EvidenceProjectionIdentity {
                    term: reader.id("EvidenceTermId")?,
                    declaring_trait_identity: reader
                        .string("evidence projection declaring trait")?,
                    declaring_trait_arguments: decode_counted(reader, |reader| {
                        reader.string("evidence projection declaring trait argument")
                    })?,
                    requirement_identity: reader.string("evidence projection requirement")?,
                }),
            ),
            tag => return Err(CodecError::InvalidTag("PropositionBinderArgument", tag)),
        };
        binder_arguments.push(PropositionBinderArgumentIdentity {
            kind,
            identity,
            evidence_projection,
        });
    }
    let argument_count = reader.count()?;
    let mut arguments = Vec::with_capacity(argument_count as usize);
    for _ in 0..argument_count {
        arguments.push(reader.string("proposition argument")?);
    }
    let evidence_interface = match reader.u8()? {
        0 => None,
        1 => Some(decode_evidence_interface(reader)?),
        tag => return Err(CodecError::InvalidTag("PropositionEvidenceInterface", tag)),
    };
    Ok(PropositionApplicationIdentity {
        id,
        declaration,
        binder_arguments,
        arguments,
        evidence_interface,
    })
}

fn decode_evidence_interface(
    reader: &mut Reader<'_>,
) -> Result<EvidenceInterfaceIdentity, CodecError> {
    Ok(EvidenceInterfaceIdentity {
        trait_identity: reader.string("evidence interface trait identity")?,
        arguments: decode_counted(reader, |reader| {
            reader.string("evidence interface argument")
        })?,
        requirements: decode_counted(reader, |reader| {
            Ok(psi_terminal::EvidenceRequirementIdentity {
                declaring_trait_identity: reader.string("evidence requirement declaring trait")?,
                declaring_trait_arguments: decode_counted(reader, |reader| {
                    reader.string("evidence requirement declaring trait argument")
                })?,
                requirement_identity: reader.string("evidence requirement identity")?,
            })
        })?,
    })
}

fn decode_machine(reader: &mut Reader<'_>) -> Result<TerminalMachine, CodecError> {
    let id = reader.id("MachineId")?;
    let attachment = decode_optional_id(reader, "StructuralTypeId")?;
    let parameters = decode_declarations(reader)?;
    let structural_parameters = decode_structural_parameters(reader)?;
    let result = match reader.u8()? {
        0 => TerminalMachineResult::Unit,
        1 => TerminalMachineResult::Scalar(decode_declaration(reader)?),
        2 => TerminalMachineResult::Structural(StructuralResultDeclaration {
            place: reader.id("PlaceId")?,
            structural_type: reader.id("StructuralTypeId")?,
            multiplicity: match reader.u8()? {
                1 => StructuralMultiplicity::Unrestricted,
                2 => StructuralMultiplicity::Affine,
                3 => StructuralMultiplicity::Linear,
                tag => return Err(CodecError::InvalidTag("StructuralMultiplicity", tag)),
            },
            qualifications: decode_ids(reader, "StructuralDomainId")?,
        }),
        tag => return Err(CodecError::InvalidTag("TerminalMachineResult", tag)),
    };
    let count = reader.count()?;
    let mut structural_places = Vec::new();
    for _ in 0..count {
        structural_places.push(StructuralPlaceDeclaration {
            id: reader.id("PlaceId")?,
            kind: decode_structural_place_kind(reader)?,
        });
    }
    let entry_claims = decode_counted(reader, |reader| {
        Ok(EntryClaim {
            claim: reader.id("ClaimId")?,
            input: reader.id("PlaceId")?,
            path: decode_structural_path(reader)?,
        })
    })?;
    let published_service_ceiling = decode_ids(reader, "ServiceId")?;
    let count = reader.count()?;
    let mut content_entry_claims = Vec::new();
    for _ in 0..count {
        content_entry_claims.push(decode_content_entry_claim(reader)?);
    }
    let count = reader.count()?;
    let mut content_identity_reshuffles = Vec::new();
    for _ in 0..count {
        content_identity_reshuffles.push(decode_content_identity_reshuffle(reader)?);
    }
    let count = reader.count()?;
    let mut content_partition_compositions = Vec::new();
    for _ in 0..count {
        content_partition_compositions.push(decode_content_partition_composition(reader)?);
    }
    let entry = reader.id("BlockId")?;
    let block_count = reader.count()?;
    let mut blocks = Vec::new();
    for _ in 0..block_count {
        blocks.push(decode_block(reader)?);
    }
    let contract = decode_contract(reader)?;
    Ok(TerminalMachine {
        id,
        attachment,
        parameters,
        structural_parameters,
        result,
        structural_places,
        entry_claims,
        published_service_ceiling,
        content_entry_claims,
        content_identity_reshuffles,
        content_partition_compositions,
        entry,
        blocks,
        contract,
    })
}

fn decode_content_entry_claim(reader: &mut Reader<'_>) -> Result<ContentEntryClaim, CodecError> {
    Ok(ContentEntryClaim {
        claim: reader.id("ClaimId")?,
        input: decode_content_structural_place(reader)?,
        projections: decode_claim_content_projections(reader)?,
    })
}

fn decode_content_partition_composition(
    reader: &mut Reader<'_>,
) -> Result<ContentPartitionComposition, CodecError> {
    let source_fingerprint = reader.u64()?;
    let source_place_count = reader.count()?;
    let mut source_structural_places = Vec::new();
    for _ in 0..source_place_count {
        source_structural_places.push(StructuralPlaceDeclaration {
            id: reader.id("PlaceId")?,
            kind: decode_structural_place_kind(reader)?,
        });
    }
    let source = decode_content_conservation(reader)?;
    let input_claim_count = reader.count()?;
    let mut input_claims = Vec::new();
    for _ in 0..input_claim_count {
        input_claims.push(reader.id("ClaimId")?);
    }
    let substitution_count = reader.count()?;
    let mut substitutions = Vec::new();
    for _ in 0..substitution_count {
        substitutions.push(ContentPlaceSubstitution {
            source: decode_content_structural_place(reader)?,
            target: decode_content_structural_place(reader)?,
        });
    }
    let derived = decode_content_conservation(reader)?;
    Ok(ContentPartitionComposition {
        source_fingerprint,
        source_structural_places,
        source,
        input_claims,
        substitutions,
        derived,
    })
}

fn decode_content_conservation(reader: &mut Reader<'_>) -> Result<ContentConservation, CodecError> {
    Ok(ContentConservation::new(
        decode_content_algebra(reader)?,
        decode_content_term(reader, 0)?,
        decode_content_term(reader, 0)?,
    ))
}

fn decode_content_identity_reshuffle(
    reader: &mut Reader<'_>,
) -> Result<ContentIdentityReshuffle, CodecError> {
    let claim = reader.id::<ClaimId>("ClaimId")?;
    let input = decode_content_structural_place(reader)?;
    let output = decode_content_structural_place(reader)?;
    let projections = decode_claim_content_projections(reader)?;
    Ok(ContentIdentityReshuffle {
        claim,
        input,
        output,
        projections,
    })
}

fn decode_claim_content_projections(
    reader: &mut Reader<'_>,
) -> Result<Vec<ClaimContentProjection>, CodecError> {
    let count = reader.count()?;
    let mut projections = Vec::new();
    for _ in 0..count {
        projections.push(ClaimContentProjection {
            projection: ContentProjectionIdentity {
                domain: reader.id("ContentDomainId")?,
                projection_fingerprint: reader.u64()?,
            },
            algebra: decode_content_algebra(reader)?,
        });
    }
    Ok(projections)
}

fn decode_declarations(reader: &mut Reader<'_>) -> Result<Vec<ValueDeclaration>, CodecError> {
    let count = reader.count()?;
    let mut declarations = Vec::new();
    for _ in 0..count {
        declarations.push(decode_declaration(reader)?);
    }
    Ok(declarations)
}

fn decode_declaration(reader: &mut Reader<'_>) -> Result<ValueDeclaration, CodecError> {
    Ok(ValueDeclaration {
        id: reader.id("ValueId")?,
        scalar_type: decode_scalar_type(reader)?,
    })
}

fn decode_block(reader: &mut Reader<'_>) -> Result<Block, CodecError> {
    let id = reader.id("BlockId")?;
    let parameters = decode_declarations(reader)?;
    let operation_count = reader.count()?;
    let mut operations = Vec::new();
    for _ in 0..operation_count {
        let operation_id = reader.id("OperationId")?;
        let result = match reader.u8()? {
            0 => OperationResult::Unit,
            1 => OperationResult::Scalar(decode_declaration(reader)?),
            tag => return Err(CodecError::InvalidTag("OperationResult", tag)),
        };
        let kind = match reader.u8()? {
            1 => OperationKind::IntegerConstant {
                value: decode_integer_value(reader)?,
            },
            2 => OperationKind::BooleanConstant {
                value: reader.boolean()?,
            },
            38 => OperationKind::BooleanStructuralField {
                source: reader.id("PlaceId")?,
                field: reader.id("StructuralFieldId")?,
            },
            3 => OperationKind::WrappingIntegerAdd {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            4 => OperationKind::SaturatingIntegerAdd {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            5 => OperationKind::WrappingIntegerSubtract {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            6 => OperationKind::SaturatingIntegerSubtract {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            7 => OperationKind::WrappingIntegerMultiply {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            8 => OperationKind::SaturatingIntegerMultiply {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            9 => OperationKind::BooleanNot {
                operand: reader.id("ValueId")?,
            },
            10 => OperationKind::BooleanEqual {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            11 => OperationKind::IntegerEqual {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            12 => OperationKind::IntegerLessThan {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            13 => OperationKind::IntegerLessOrEqual {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            14 => OperationKind::IntegerBitwiseAnd {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            15 => OperationKind::IntegerBitwiseOr {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            16 => OperationKind::IntegerBitwiseXor {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            17 => OperationKind::WrappingIntegerShiftLeft {
                value: reader.id("ValueId")?,
                count: reader.id("ValueId")?,
            },
            18 => OperationKind::WrappingIntegerShiftRight {
                value: reader.id("ValueId")?,
                count: reader.id("ValueId")?,
            },
            19 => OperationKind::IntegerBitwiseNot {
                operand: reader.id("ValueId")?,
            },
            20 => OperationKind::IntegerWiden {
                operand: reader.id("ValueId")?,
            },
            21 => OperationKind::IntegerExactCast {
                operand: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            22 => OperationKind::ExactIntegerShiftRight {
                value: reader.id("ValueId")?,
                count: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            23 => OperationKind::ExactIntegerShiftLeft {
                value: reader.id("ValueId")?,
                count: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            24 => OperationKind::ExactIntegerAdd {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            25 => OperationKind::ExactIntegerSubtract {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            26 => OperationKind::ExactIntegerMultiply {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            27 => OperationKind::ExactIntegerDivide {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            28 => OperationKind::ExactIntegerRemainder {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            29 => OperationKind::WrappingIntegerDivide {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            30 => OperationKind::WrappingIntegerRemainder {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            31 => OperationKind::SaturatingIntegerDivide {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            32 => OperationKind::SaturatingIntegerRemainder {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            33 => {
                let callee = reader.id("MachineId")?;
                let argument_count = reader.count()?;
                let mut arguments = Vec::with_capacity(
                    usize::try_from(argument_count).expect("u32 count fits usize"),
                );
                for _ in 0..argument_count {
                    arguments.push(reader.id("ValueId")?);
                }
                let requirement_count = reader.count()?;
                let mut requirement_obligations = Vec::with_capacity(
                    usize::try_from(requirement_count).expect("u32 count fits usize"),
                );
                for _ in 0..requirement_count {
                    requirement_obligations.push(reader.id("ObligationId")?);
                }
                let crash_continuations = decode_crash_routes(reader)?;
                OperationKind::Call {
                    callee,
                    arguments,
                    requirement_obligations,
                    crash_continuations,
                }
            }
            34 => OperationKind::CallUnit {
                callee: reader.id("MachineId")?,
                structural_arguments: decode_structural_arguments(reader)?,
                claim_transfers: decode_counted(reader, |reader| {
                    Ok(ClaimTransfer {
                        claim: reader.id("ClaimId")?,
                        argument_index: reader.u32()?,
                    })
                })?,
                requirement_obligations: decode_ids(reader, "ObligationId")?,
                crash_continuations: decode_crash_routes(reader)?,
            },
            35 => OperationKind::BoundaryCall {
                boundary: reader.id("BoundaryMachineId")?,
                structural_arguments: decode_structural_arguments(reader)?,
                completion_receipts: decode_counted(reader, |reader| {
                    Ok(CompletionReceipt {
                        claim: reader.id("ClaimId")?,
                        argument_index: reader.u32()?,
                    })
                })?,
                requirement_obligations: decode_ids(reader, "ObligationId")?,
            },
            36 => OperationKind::PortWrite {
                service: reader.id("ServiceId")?,
                port: reader.u16()?,
                value: reader.u8()?,
            },
            37 => OperationKind::EstablishTrivialAffineLocal {
                destination: reader.id("PlaceId")?,
            },
            39 => OperationKind::CallStructuralScalar {
                callee: reader.id("MachineId")?,
                structural_arguments: decode_structural_arguments(reader)?,
                claim_transfers: decode_counted(reader, |reader| {
                    Ok(ClaimTransfer {
                        claim: reader.id("ClaimId")?,
                        argument_index: reader.u32()?,
                    })
                })?,
                requirement_obligations: decode_ids(reader, "ObligationId")?,
                crash_continuations: decode_crash_routes(reader)?,
            },
            tag => return Err(CodecError::InvalidTag("OperationKind", tag)),
        };
        operations.push(Operation {
            id: operation_id,
            result,
            kind,
        });
    }
    let terminator = match reader.u8()? {
        1 => {
            let edge = reader.id("EdgeId")?;
            let target = reader.id("BlockId")?;
            let argument_count = reader.count()?;
            let mut arguments = Vec::new();
            for _ in 0..argument_count {
                arguments.push(reader.id("ValueId")?);
            }
            Terminator::Jump {
                edge,
                target,
                arguments,
                trivial_affine_discards: decode_counted(reader, |reader| reader.id("PlaceId"))?,
            }
        }
        2 => Terminator::Return {
            edge: reader.id("EdgeId")?,
            value: reader.id("ValueId")?,
            cleanup_actions: decode_counted(reader, decode_affine_cleanup_action)?,
        },
        3 => Terminator::Conditional {
            condition: reader.id("ValueId")?,
            when_true: decode_successor_edge(reader)?,
            when_false: decode_successor_edge(reader)?,
        },
        4 => {
            let edge = reader.id("EdgeId")?;
            let cause = match reader.u8()? {
                1 => CrashCause::Trap,
                2 => CrashCause::Abort,
                tag => return Err(CodecError::InvalidTag("CrashCause", tag)),
            };
            let guard_count = reader.count()?;
            let mut site_guard = Vec::with_capacity(guard_count as usize);
            for _ in 0..guard_count {
                site_guard.push(decode_crash_predicate(reader)?);
            }
            let claim_count = reader.count()?;
            let mut frontier_lower_bound = Vec::with_capacity(claim_count as usize);
            for _ in 0..claim_count {
                frontier_lower_bound.push(reader.id("ClaimId")?);
            }
            Terminator::Crash {
                edge,
                cause,
                site_guard,
                frontier_lower_bound,
            }
        }
        5 => Terminator::ReturnUnit {
            edge: reader.id("EdgeId")?,
            trivial_affine_discards: decode_counted(reader, |reader| reader.id("PlaceId"))?,
        },
        6 => Terminator::ReturnStructural {
            edge: reader.id("EdgeId")?,
            source: reader.id("PlaceId")?,
            returned_claims: decode_counted(reader, |reader| reader.id("ClaimId"))?,
            trivial_affine_discards: decode_counted(reader, |reader| reader.id("PlaceId"))?,
        },
        7 => Terminator::ReturnUnitPartialAffine {
            edge: reader.id("EdgeId")?,
            trivial_affine_discards: decode_counted(reader, |reader| reader.id("PlaceId"))?,
            residual_affine_discards: decode_counted(reader, |reader| {
                Ok(StructuralAffineDiscard {
                    place: reader.id("PlaceId")?,
                    path: decode_structural_path(reader)?,
                    structural_type: reader.id("StructuralTypeId")?,
                })
            })?,
        },
        8 => Terminator::ReturnUnitNominalAffine {
            edge: reader.id("EdgeId")?,
            cleanups: decode_counted(reader, |reader| {
                Ok(NominalAffineCleanup {
                    place: reader.id("PlaceId")?,
                    structural_type: reader.id("StructuralTypeId")?,
                    cleanup_machine: reader.id("MachineId")?,
                    cleanup_receiver: decode_optional_id(reader, "PlaceId")?,
                    requirement_obligations: decode_ids(reader, "ObligationId")?,
                })
            })?,
        },
        tag => return Err(CodecError::InvalidTag("Terminator", tag)),
    };
    Ok(Block {
        id,
        parameters,
        operations,
        terminator,
    })
}

fn decode_affine_cleanup_action(
    reader: &mut Reader<'_>,
) -> Result<TerminalAffineCleanupAction, CodecError> {
    match reader.u8()? {
        1 => Ok(TerminalAffineCleanupAction::DiscardRoot(
            reader.id("PlaceId")?,
        )),
        2 => Ok(TerminalAffineCleanupAction::DiscardResidual(
            StructuralAffineDiscard {
                place: reader.id("PlaceId")?,
                path: decode_structural_path(reader)?,
                structural_type: reader.id("StructuralTypeId")?,
            },
        )),
        3 => Ok(TerminalAffineCleanupAction::InvokeNominal(
            NominalAffineCleanup {
                place: reader.id("PlaceId")?,
                structural_type: reader.id("StructuralTypeId")?,
                cleanup_machine: reader.id("MachineId")?,
                cleanup_receiver: decode_optional_id(reader, "PlaceId")?,
                requirement_obligations: decode_ids(reader, "ObligationId")?,
            },
        )),
        tag => Err(CodecError::InvalidTag("TerminalAffineCleanupAction", tag)),
    }
}

fn decode_structural_arguments(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralArgument>, CodecError> {
    decode_counted(reader, |reader| {
        Ok(StructuralArgument {
            place: reader.id("PlaceId")?,
            path: decode_structural_path(reader)?,
        })
    })
}

fn decode_structural_path(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralPathSegment>, CodecError> {
    decode_counted(reader, |reader| match reader.u8()? {
        1 => Ok(StructuralPathSegment::Field(
            reader.string("structural path field")?,
        )),
        2 => Ok(StructuralPathSegment::FixedIndex(reader.u64()?)),
        tag => Err(CodecError::InvalidTag("StructuralPathSegment", tag)),
    })
}

fn decode_successor_edge(reader: &mut Reader<'_>) -> Result<SuccessorEdge, CodecError> {
    let edge = reader.id("EdgeId")?;
    let target = reader.id("BlockId")?;
    let argument_count = reader.count()?;
    let mut arguments = Vec::new();
    for _ in 0..argument_count {
        arguments.push(reader.id("ValueId")?);
    }
    Ok(SuccessorEdge {
        edge,
        target,
        arguments,
        trivial_affine_discards: decode_counted(reader, |reader| reader.id("PlaceId"))?,
    })
}

fn decode_contract(reader: &mut Reader<'_>) -> Result<MachineContract, CodecError> {
    let id = reader.id("ContractId")?;
    let crash_routes = decode_crash_routes(reader)?;
    let requires_count = reader.count()?;
    let mut requires = Vec::new();
    for _ in 0..requires_count {
        requires.push(decode_proposition(reader, 0)?);
    }
    let ensures_count = reader.count()?;
    let mut ensures = Vec::new();
    for _ in 0..ensures_count {
        ensures.push(ContractClause {
            obligation: reader.id("ObligationId")?,
            proposition: decode_proposition(reader, 0)?,
        });
    }
    Ok(MachineContract {
        id,
        crash_routes,
        requires,
        ensures,
    })
}

fn decode_crash_routes(reader: &mut Reader<'_>) -> Result<Vec<CrashRouteBucket>, CodecError> {
    let count = reader.count()?;
    let mut crash_routes = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let cause = match reader.u8()? {
            1 => CrashCause::Trap,
            2 => CrashCause::Abort,
            tag => return Err(CodecError::InvalidTag("CrashCause", tag)),
        };
        let alternative_count = reader.count()?;
        let mut alternatives = Vec::with_capacity(alternative_count as usize);
        for _ in 0..alternative_count {
            alternatives.push(match reader.u8()? {
                0 => CrashRouteGuard::Truth,
                1 => CrashRouteGuard::Predicate(decode_crash_predicate(reader)?),
                tag => return Err(CodecError::InvalidTag("CrashRouteGuard", tag)),
            });
        }
        crash_routes.push(CrashRouteBucket {
            cause,
            alternatives,
        });
    }
    Ok(crash_routes)
}

fn decode_crash_predicate(reader: &mut Reader<'_>) -> Result<CrashPredicateTerm, CodecError> {
    Ok(CrashPredicateTerm::new(decode_proposition(reader, 0)?))
}

fn decode_proposition(reader: &mut Reader<'_>, depth: usize) -> Result<Proposition, CodecError> {
    if depth > MAX_PROPOSITION_DEPTH {
        return Err(CodecError::PropositionNestingTooDeep);
    }
    Ok(match reader.u8()? {
        1 => Proposition::Truth,
        2 => Proposition::Falsehood,
        3 => Proposition::Atom(reader.id::<PropositionId>("PropositionId")?),
        4 => Proposition::Equal(
            decode_scalar_term(reader, 0)?,
            decode_scalar_term(reader, 0)?,
        ),
        5 => Proposition::LessThan(
            decode_scalar_term(reader, 0)?,
            decode_scalar_term(reader, 0)?,
        ),
        6 => Proposition::LessOrEqual(
            decode_scalar_term(reader, 0)?,
            decode_scalar_term(reader, 0)?,
        ),
        7 => {
            let count = reader.count()?;
            let mut conjuncts = Vec::new();
            for _ in 0..count {
                conjuncts.push(decode_proposition(reader, depth + 1)?);
            }
            Proposition::Conjunction(conjuncts)
        }
        8 => Proposition::Implication {
            premise: Box::new(decode_proposition(reader, depth + 1)?),
            conclusion: Box::new(decode_proposition(reader, depth + 1)?),
        },
        9 => {
            let algebra = decode_content_algebra(reader)?;
            let left = decode_content_term(reader, 0)?;
            let right = decode_content_term(reader, 0)?;
            Proposition::ContentConservation(ContentConservation::new(algebra, left, right))
        }
        10 => {
            let count = reader.count()?;
            let mut disjuncts = Vec::new();
            for _ in 0..count {
                disjuncts.push(decode_proposition(reader, depth + 1)?);
            }
            Proposition::Disjunction(disjuncts)
        }
        11 => Proposition::IeeeFloatComparison {
            kind: decode_ieee_float_comparison_kind(reader)?,
            format: decode_ieee_float_format(reader)?,
            left: decode_ieee_float_field(reader)?,
            right: decode_ieee_float_field(reader)?,
        },
        12 => Proposition::ByteSequenceEqual {
            left: decode_byte_sequence_field(reader)?,
            right: decode_byte_sequence_field(reader)?,
        },
        13 => {
            let (root, path) = decode_canonical_structural_field(reader)?;
            Proposition::StructuralCaseMembership {
                subject: StructuralCaseSubject::new(root, path),
                case: reader.id("StructuralCaseId")?,
            }
        }
        tag => return Err(CodecError::InvalidTag("Proposition", tag)),
    })
}

fn decode_structural_place_kind(
    reader: &mut Reader<'_>,
) -> Result<StructuralPlaceKind, CodecError> {
    Ok(match reader.u8()? {
        1 => StructuralPlaceKind::Parameter {
            position: reader.u32()?,
            is_self: reader.boolean()?,
        },
        2 => StructuralPlaceKind::Result,
        3 => StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal: reader.u32()?,
            structural_type: reader.id("StructuralTypeId")?,
        },
        tag => return Err(CodecError::InvalidTag("StructuralPlaceKind", tag)),
    })
}

fn decode_content_algebra(reader: &mut Reader<'_>) -> Result<ContentAlgebra, CodecError> {
    let kind = match reader.u8()? {
        1 => ContentAlgebraKind::IntervalSet,
        2 => ContentAlgebraKind::CountedQuantity,
        tag => return Err(CodecError::InvalidTag("ContentAlgebraKind", tag)),
    };
    Ok(ContentAlgebra {
        kind,
        parameter: reader.string("content algebra parameter")?,
    })
}

fn decode_content_term(reader: &mut Reader<'_>, depth: usize) -> Result<ContentTerm, CodecError> {
    if depth > MAX_CONTENT_TERM_DEPTH {
        return Err(CodecError::ContentTermNestingTooDeep);
    }
    Ok(match reader.u8()? {
        1 => {
            let projection = ContentProjectionIdentity {
                domain: reader.id::<ContentDomainId>("ContentDomainId")?,
                projection_fingerprint: reader.u64()?,
            };
            ContentTerm::Projection {
                projection,
                subject: decode_content_structural_place(reader)?,
            }
        }
        2 => {
            let count = reader.count()?;
            let mut terms = Vec::new();
            for _ in 0..count {
                terms.push(decode_content_term(reader, depth + 1)?);
            }
            ContentTerm::separate(terms).map_err(CodecError::MalformedProposition)?
        }
        tag => return Err(CodecError::InvalidTag("ContentTerm", tag)),
    })
}

fn decode_content_structural_place(
    reader: &mut Reader<'_>,
) -> Result<ContentStructuralPlace, CodecError> {
    let version = match reader.u8()? {
        1 => ContentPlaceVersion::Entry,
        2 => ContentPlaceVersion::Current,
        tag => return Err(CodecError::InvalidTag("ContentPlaceVersion", tag)),
    };
    let root = reader.id("PlaceId")?;
    let count = reader.count()?;
    let mut segments = Vec::new();
    for _ in 0..count {
        segments.push(match reader.u8()? {
            1 => ContentPlaceSegment::Field(reader.string("content field")?),
            2 => ContentPlaceSegment::FixedIndex(reader.u64()?),
            3 => ContentPlaceSegment::Case(reader.string("content case")?),
            tag => return Err(CodecError::InvalidTag("ContentPlaceSegment", tag)),
        });
    }
    Ok(ContentStructuralPlace {
        version,
        root,
        segments,
    })
}

fn decode_scalar_term(reader: &mut Reader<'_>, depth: usize) -> Result<ScalarTerm, CodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(CodecError::ScalarTermNestingTooDeep);
    }
    Ok(match reader.u8()? {
        1 => ScalarTerm::value(reader.id("ValueId")?, decode_scalar_type(reader)?),
        2 => ScalarTerm::boolean(reader.boolean()?),
        3 => {
            let scalar_type = decode_integer_type(reader)?;
            let value = decode_integer_value(reader)?;
            ScalarTerm::integer(scalar_type, value).map_err(CodecError::MalformedProposition)?
        }
        4 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_add(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        5 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::saturating_integer_add(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        6 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_subtract(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        7 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::saturating_integer_subtract(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        8 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_multiply(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        9 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::saturating_integer_multiply(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        10 => ScalarTerm::boolean_not(decode_scalar_term(reader, depth + 1)?)
            .map_err(CodecError::MalformedProposition)?,
        11 => {
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::boolean_equal(left, right).map_err(CodecError::MalformedProposition)?
        }
        12 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_equal(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        13 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_less_than(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        14 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_less_or_equal(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        15 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_bitwise_and(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        16 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_bitwise_or(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        17 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_bitwise_xor(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        18 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1)?;
            let count = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_shift_left(value_type, count_type, value, count)
                .map_err(CodecError::MalformedProposition)?
        }
        19 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1)?;
            let count = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_shift_right(value_type, count_type, value, count)
                .map_err(CodecError::MalformedProposition)?
        }
        20 => {
            let scalar_type = decode_integer_type(reader)?;
            let operand = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_bitwise_not(scalar_type, operand)
                .map_err(CodecError::MalformedProposition)?
        }
        21 => {
            let source_type = decode_integer_type(reader)?;
            let target_type = decode_integer_type(reader)?;
            let operand = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_widen(source_type, target_type, operand)
                .map_err(CodecError::MalformedProposition)?
        }
        22 => {
            let source_type = decode_integer_type(reader)?;
            let target_type = decode_integer_type(reader)?;
            let operand = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::integer_exact_cast(source_type, target_type, operand)
                .map_err(CodecError::MalformedProposition)?
        }
        23 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1)?;
            let count = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_shift_right(value_type, count_type, value, count)
                .map_err(CodecError::MalformedProposition)?
        }
        24 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1)?;
            let count = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_shift_left(value_type, count_type, value, count)
                .map_err(CodecError::MalformedProposition)?
        }
        25 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_add(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        26 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_subtract(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        27 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_multiply(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        28 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_divide(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        29 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::exact_integer_remainder(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        30 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_divide(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        31 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::wrapping_integer_remainder(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        32 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::saturating_integer_divide(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        33 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1)?;
            let right = decode_scalar_term(reader, depth + 1)?;
            ScalarTerm::saturating_integer_remainder(scalar_type, left, right)
                .map_err(CodecError::MalformedProposition)?
        }
        34 => {
            let root = reader.id("PlaceId")?;
            let count = reader.count()?;
            let mut path = Vec::new();
            for _ in 0..count {
                path.push(match reader.u8()? {
                    1 => CanonicalStructuralPathSegment::Field(reader.id("StructuralFieldId")?),
                    2 => CanonicalStructuralPathSegment::FixedIndex(reader.u64()?),
                    3 => CanonicalStructuralPathSegment::Case(reader.id("StructuralCaseId")?),
                    tag => {
                        return Err(CodecError::InvalidTag(
                            "CanonicalStructuralPathSegment",
                            tag,
                        ));
                    }
                });
            }
            ScalarTerm::boolean_field_path(root, path)
        }
        35 => {
            let root = reader.id("PlaceId")?;
            let count = reader.count()?;
            let mut path = Vec::new();
            for _ in 0..count {
                path.push(match reader.u8()? {
                    1 => CanonicalStructuralPathSegment::Field(reader.id("StructuralFieldId")?),
                    2 => CanonicalStructuralPathSegment::FixedIndex(reader.u64()?),
                    3 => CanonicalStructuralPathSegment::Case(reader.id("StructuralCaseId")?),
                    tag => {
                        return Err(CodecError::InvalidTag(
                            "CanonicalStructuralPathSegment",
                            tag,
                        ));
                    }
                });
            }
            ScalarTerm::integer_field_path(root, path, decode_integer_type(reader)?)
        }
        tag => return Err(CodecError::InvalidTag("ScalarTerm", tag)),
    })
}

fn decode_scalar_type(reader: &mut Reader<'_>) -> Result<ScalarType, CodecError> {
    Ok(match reader.u8()? {
        1 => ScalarType::Boolean,
        2 => ScalarType::Integer(decode_integer_type(reader)?),
        tag => return Err(CodecError::InvalidTag("ScalarType", tag)),
    })
}

fn decode_integer_type(reader: &mut Reader<'_>) -> Result<IntegerType, CodecError> {
    let tag = reader.u8()?;
    let bits = reader.u16()?;
    match tag {
        1 => IntegerType::new(IntegerSign::Signed, bits),
        2 => IntegerType::new(IntegerSign::Unsigned, bits),
        3 => IntegerType::address(bits),
        tag => return Err(CodecError::InvalidTag("IntegerSign", tag)),
    }
    .map_err(CodecError::MalformedProposition)
}

fn decode_integer_value(reader: &mut Reader<'_>) -> Result<IntegerValue, CodecError> {
    Ok(match reader.u8()? {
        1 => IntegerValue::Signed(i128::from_le_bytes(reader.array()?)),
        2 => IntegerValue::Unsigned(u128::from_le_bytes(reader.array()?)),
        tag => return Err(CodecError::InvalidTag("IntegerValue", tag)),
    })
}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn boolean(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn id(&mut self, id: impl PsiSemanticId) {
        self.bytes(&id.get().to_le_bytes());
    }

    fn len(&mut self, label: &'static str, len: usize) -> Result<(), CodecError> {
        self.u32(u32::try_from(len).map_err(|_| CodecError::CollectionTooLong(label))?);
        Ok(())
    }

    fn string(&mut self, label: &'static str, value: &str) -> Result<(), CodecError> {
        if value.len() > MAX_CONTENT_IDENTITY_BYTES {
            return Err(CodecError::StringTooLong(label));
        }
        self.len(label, value.len())?;
        self.bytes(value.as_bytes());
        Ok(())
    }

    fn strings(&mut self, label: &'static str, values: &[String]) -> Result<(), CodecError> {
        self.len(label, values.len())?;
        for value in values {
            self.string(label, value)?;
        }
        Ok(())
    }
}

struct Reader<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}

impl<'bytes> Reader<'bytes> {
    const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    fn take(&mut self, len: usize) -> Result<&'bytes [u8], CodecError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(CodecError::UnexpectedEnd)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(CodecError::UnexpectedEnd)?;
        self.offset = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], CodecError> {
        self.take(N)?
            .try_into()
            .map_err(|_| CodecError::UnexpectedEnd)
    }

    fn u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, CodecError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, CodecError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, CodecError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn count(&mut self) -> Result<u32, CodecError> {
        self.u32()
    }

    fn boolean(&mut self) -> Result<bool, CodecError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(CodecError::InvalidBoolean(value)),
        }
    }

    fn string(&mut self, label: &'static str) -> Result<String, CodecError> {
        let len = usize::try_from(self.count()?).map_err(|_| CodecError::StringTooLong(label))?;
        if len > MAX_CONTENT_IDENTITY_BYTES {
            return Err(CodecError::StringTooLong(label));
        }
        let bytes = self.take(len)?;
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| CodecError::InvalidUtf8(label))
    }

    fn strings(&mut self, label: &'static str) -> Result<Vec<String>, CodecError> {
        let count = self.count()?;
        (0..count).map(|_| self.string(label)).collect()
    }

    fn id<T: PsiSemanticId>(&mut self, label: &'static str) -> Result<T, CodecError> {
        let raw = self.u64()?;
        T::new(raw).ok_or(CodecError::ZeroIdentity(label))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    InvalidMagic,
    UnsupportedFormatMarker(u16),
    UnsupportedVocabularyMarker(u16),
    UnexpectedEnd,
    TrailingBytes(usize),
    InvalidBoolean(u8),
    InvalidTag(&'static str, u8),
    ZeroIdentity(&'static str),
    CollectionTooLong(&'static str),
    NonCanonicalOrder(&'static str),
    NonCanonicalEncoding,
    NestedConjunction,
    NestedDisjunction,
    PropositionNestingTooDeep,
    ScalarTermNestingTooDeep,
    ContentTermNestingTooDeep,
    StringTooLong(&'static str),
    InvalidUtf8(&'static str),
    MalformedStructuralFoundation(&'static str),
    MalformedProposition(PropositionError),
    InvalidModule(ModuleError),
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CodecError {}

#[cfg(test)]
mod resource_tests {
    use super::*;

    #[test]
    fn counted_decoder_rejects_impossible_capacity_before_allocation() {
        let bytes = u32::MAX.to_le_bytes();
        let mut reader = Reader::new(&bytes);
        assert_eq!(
            decode_counted::<u8>(&mut reader, |reader| reader.u8()),
            Err(CodecError::UnexpectedEnd)
        );
    }
}
