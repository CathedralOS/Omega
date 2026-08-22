#![forbid(unsafe_code)]

//! Canonical binary encoding and semantic identity for terminal Psi.
//!
//! Only the semantic module is encoded here. Proof bundles, installation
//! records, and debug/source maps have separate identities and can be replaced
//! without changing [`TerminalPsiIdentity`].

mod artifact_manifest;
mod block_wire;
mod content_wire;
mod contract_wire;
mod debug_map;
mod machine_wire;
mod proof_bundle;
mod proof_declaration_wire;
mod proposition_wire;
mod provider_candidate_wire;
mod scalar_term_wire;
mod scalar_wire;
mod structural_field_wire;
mod structural_signature_wire;
mod structural_type_wire;
mod trust_graph;
mod wire;

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

use block_wire::{decode_block, encode_block};
use machine_wire::{decode_machine, encode_machine};

use proof_declaration_wire::{
    decode_evidence_interface, decode_proposition_application, decode_proposition_declaration,
    encode_evidence_interface, encode_proposition_application, encode_proposition_declaration,
};
use proposition_wire::{decode_proposition, encode_proposition};
use provider_candidate_wire::{decode_provider_candidate, encode_provider_candidate};
use psi_core::{
    ClaimId, ContentTerm, IeeeFloatFormat, ObligationId, Proposition, PropositionError,
    PropositionId, PsiSemanticId, ScalarTerm, ServiceId, StructuralPlaceKind, StructuralTypeId,
};
use psi_terminal::{
    ClosedConformanceApplication, ClosedConformanceParameterBinding,
    ClosedConformanceParameterKind, ClosedConformanceRow, CrashRouteBucket, CrashRouteGuard,
    EvidenceContractLane, EvidenceContractLaneKind, EvidencePackageInvocation,
    EvidencePackageOutputBinding, EvidencePackageRuntimeCall, EvidenceTermDeclaration,
    FloatMeaningEqualityProposition, FloatMeaningProjection, FloatMeaningProjectionOperation,
    FloatProjectionInput, FloatProjectionInputId, NominalAffineCleanup, Operation, OperationKind,
    OperationResult, ProofOnlyValueType, ProofPropositionId, ProofValueDeclaration, ProofValueId,
    ServiceDeclaration, StructuralAffineDiscard, StructuralArgument, StructuralDomainDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, VocabularyMarker,
};
use psi_terminal_verifier::{ModuleError, validate_module_representation};
use scalar_term_wire::{decode_scalar_term, encode_scalar_term};
use scalar_wire::{decode_scalar_type, encode_scalar_type};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use structural_signature_wire::{decode_boundary_machine, encode_boundary_machine};
use structural_type_wire::{decode_structural_type, encode_structural_type};
use wire::{Reader, Writer};

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

fn encode_optional_id<I: PsiSemanticId>(writer: &mut Writer, id: Option<I>) {
    match id {
        None => writer.u8(0),
        Some(id) => {
            writer.u8(1);
            writer.id(id);
        }
    }
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
