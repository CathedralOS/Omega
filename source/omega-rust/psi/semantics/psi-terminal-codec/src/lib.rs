#![forbid(unsafe_code)]

//! Canonical binary encoding and semantic identity for terminal Psi.
//!
//! Only the semantic module is encoded here. Proof bundles, installation
//! records, and debug/source maps have separate identities and can be replaced
//! without changing [`TerminalPsiIdentity`].

mod artifact_manifest;
mod block_wire;
mod canonical_artifact;
mod canonical_order;
mod content_wire;
mod contract_wire;
mod debug_map;
mod integer_math_term_wire;
mod machine_wire;
mod module_wire;
mod obligation_ledger;
mod program_local_root_catalog;
mod proof_bundle;
mod proof_declaration_wire;
mod proposition_wire;
mod provider_candidate_wire;
mod publication;
mod quotient_correspondence_wire;
mod scalar_term_wire;
mod scalar_wire;
mod structural_field_wire;
mod structural_signature_wire;
mod structural_type_wire;
mod terminal_trace_v1_profile;
mod trust_graph;
mod wire;

pub use artifact_manifest::{
    ArtifactManifestError, SectionFingerprint, TerminalArtifactIdentity, TerminalArtifactManifest,
    build_artifact_manifest, validate_artifact_manifest,
};
pub use canonical_artifact::{CanonicalTerminalArtifact, CanonicalTerminalArtifactError};
pub use canonical_order::canonical_proposition_order_key;
pub use debug_map::{
    DebugFileId, DebugMapError, DebugSite, DebugSourceDigest, DebugSourceFile, DebugSourceOrigin,
    DebugSourceSpan, DebugSubject, TerminalDebugMap, decode_debug_map, encode_debug_map,
    source_digest, validate_debug_map,
};
pub use obligation_ledger::{
    TerminalObligationLedger, TerminalObligationLedgerFingerprint,
    build_terminal_obligation_ledger, decode_terminal_obligation_ledger,
    encode_terminal_obligation_ledger, terminal_obligation_ledger_fingerprint,
    validate_terminal_obligation_ledger,
};
pub use program_local_root_catalog::{
    ProgramLocalRootProducerCatalogError, VerifiedProgramLocalRootProducerCatalog,
    VerifiedProgramLocalRootProducerSchema,
};
pub use proof_bundle::{
    ProofBundleFingerprint, ProofCodecError, decode_proof_bundle, encode_proof_bundle,
    proof_bundle_fingerprint, render_verified_native_ranked_countdown_synopsis,
    render_verified_proof_synopsis,
};
pub use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity};
pub use publication::{
    PublishedTerminalSemanticArtifact, TerminalSemanticArtifactPublication,
    TerminalSemanticPublicationError,
};
pub use terminal_trace_v1_profile::{
    TerminalTraceV1ProfileAcceptanceError, TerminalTraceV1ProfileBuildError,
    TerminalTraceV1ProfileCodecError, accept_terminal_trace_v1_profile,
    decode_terminal_trace_v1_profile, encode_terminal_trace_v1_profile,
    reconstruct_canonical_terminal_trace_v1_profile,
};
pub use trust_graph::{
    TerminalTrustGraphIdentity, TrustAcceptingPolicy, TrustDependencyDigest, TrustDependencyKind,
    TrustDependencyNode, TrustDependencyStatus, TrustGraphError, ValidatedTerminalTrustGraph,
    current_rust_operation_semantics_trust_identity, current_terminal_trust_graph,
    render_terminal_trust_graph, validate_terminal_trust_graph,
};

use block_wire::{decode_block, encode_block};
use canonical_order::{
    crash_routes_are_canonical, validate_canonical_order, validate_crash_route_predicates,
};
use contract_wire::{decode_crash_routes, encode_crash_routes};
use module_wire::{decode_module_body, encode_raw};

use proposition_wire::{decode_proposition, encode_proposition};
use psi_core::{
    ClaimId, ObligationId, PropositionError, PsiSemanticId, ServiceId, StructuralPlaceKind,
    StructuralTypeId,
};
use psi_terminal::{
    NominalAffineCleanup, Operation, OperationKind, OperationResult, StructuralAffineDiscard,
    StructuralArgument, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralTypeShape, TerminalAffineCleanupAction, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator,
};
use psi_terminal_verifier::{ModuleError, validate_module_representation};
use scalar_term_wire::{decode_scalar_term, encode_scalar_term};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use wire::{Reader, Writer};

const MAGIC: &[u8; 8] = b"PSITERM\0";
const FORMAT_MARKER: u16 = 48;
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

/// Encode one canonical ordered crash-continuation roster without a Terminal
/// module envelope. This is the shared semantic leaf used by later
/// identity-bearing optimizer artifacts that must retain exact call routes.
pub fn encode_crash_route_buckets(
    routes: &[psi_terminal::CrashRouteBucket],
) -> Result<Vec<u8>, CodecError> {
    if !crash_routes_are_canonical(routes) {
        return Err(CodecError::NonCanonicalOrder(
            "crash routes by cause and guard",
        ));
    }
    validate_crash_route_predicates(routes)?;
    let mut writer = Writer::default();
    encode_crash_routes(&mut writer, routes)?;
    Ok(writer.finish())
}

/// Decode one complete canonical crash-continuation roster.
pub fn decode_crash_route_buckets(
    bytes: &[u8],
) -> Result<Vec<psi_terminal::CrashRouteBucket>, CodecError> {
    let mut reader = Reader::new(bytes);
    let routes = decode_crash_routes(&mut reader)?;
    if reader.remaining() != 0 {
        return Err(CodecError::TrailingBytes(reader.remaining()));
    }
    if encode_crash_route_buckets(&routes)? != bytes {
        return Err(CodecError::NonCanonicalEncoding);
    }
    Ok(routes)
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
            StructuralTypeShape::PrimitiveScalar(_) => {}
            StructuralTypeShape::ByteSequence(psi_terminal::ByteSequenceCarrier::BorrowedView) => {}
            StructuralTypeShape::ByteSequence(_) => {
                return malformed("first-class byte-sequence type must be a borrowed view");
            }
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
                            if type_identity.is_empty() =>
                        {
                            return malformed(
                                "opaque structural field type must have a nonempty type identity",
                            );
                        }
                        StructuralFieldType::Erased { .. }
                            if !field.relevance.is_erased()
                                && !module.machines.iter().any(|machine| {
                                    machine.structural_places.iter().any(|place| {
                                        matches!(
                                            place.kind,
                                            StructuralPlaceKind::ProviderAttachment {
                                                attachment,
                                                field: provider_field,
                                                ..
                                            } if attachment == declaration.id
                                                && provider_field == field.id
                                        )
                                    })
                                }) =>
                        {
                            return malformed(
                                "provider-backed attachment specialization is incomplete",
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
            StructuralTypeShape::Mixed { fields, cases } => {
                require_unique_nonempty_identities(
                    fields.iter().map(|field| field.identity.as_str()),
                    "mixed structural field identity",
                )?;
                require_unique_nonempty_identities(
                    cases.iter().map(|case| case.identity.as_str()),
                    "mixed structural case identity",
                )?;
                if cases.is_empty() {
                    return malformed("mixed structural type must declare at least one case");
                }
                for case in cases {
                    require_unique_nonempty_identities(
                        case.fields.iter().map(|field| field.identity.as_str()),
                        "mixed structural case payload field identity",
                    )?;
                }
                for field in fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                {
                    match &field.field_type {
                        StructuralFieldType::Structural(field_type)
                            if !has_structural_type(module, *field_type) =>
                        {
                            return malformed(
                                "mixed structural field references an unknown structural type",
                            );
                        }
                        StructuralFieldType::Erased { type_identity }
                            if !field.relevance.is_erased() || type_identity.is_empty() =>
                        {
                            return malformed(
                                "opaque mixed structural field must have erased relevance and a nonempty type identity",
                            );
                        }
                        StructuralFieldType::Scalar(_)
                        | StructuralFieldType::IeeeFloat(_)
                        | StructuralFieldType::Structural(_)
                            if field.relevance.is_erased() =>
                        {
                            return malformed(
                                "erased mixed structural field must use its opaque semantic type identity",
                            );
                        }
                        _ => {}
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
    require_known_services(module, &module.root_service_reach.concrete)?;
    for dependency in &module.root_service_reach.installation_dependencies {
        if dependency.requirement_identity.is_empty() {
            return malformed("installation reach requirement identity must be nonempty");
        }
        require_known_services(module, &dependency.upper_bound)?;
    }
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
        validate_provider_attachment_foundation(module, machine)?;
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
        for place in &machine.structural_places {
            let StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            } = place.kind
            else {
                continue;
            };
            let mut producers = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| operation.id == producer);
            let Some(operation) = producers.next() else {
                return malformed("structural operation-result place has no producer");
            };
            if producers.next().is_some() {
                return malformed("structural operation-result place has duplicate producers");
            }
            let Some(result) = operation.result.structural() else {
                return malformed(
                    "structural operation-result place producer has no structural result",
                );
            };
            if result.place != place.id || result.structural_type != structural_type {
                return malformed("structural operation-result place disagrees with its producer");
            }
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

fn validate_provider_attachment_foundation(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<(), CodecError> {
    let provider_roots = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::ProviderAttachment {
                attachment,
                field,
                boundary,
            } => Some((attachment, field, boundary)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let provider_fields = machine
        .attachment
        .and_then(|attachment| {
            module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == attachment)
        })
        .and_then(|attachment| match &attachment.shape {
            StructuralTypeShape::Record { fields } => Some(
                fields
                    .iter()
                    .filter(|field| {
                        !field.relevance.is_erased()
                            && matches!(field.field_type, StructuralFieldType::Erased { .. })
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .unwrap_or_default();
    if provider_roots.is_empty() && provider_fields.is_empty() {
        return Ok(());
    }
    let [provider_field] = provider_fields.as_slice() else {
        return malformed("provider-backed attachment specialization is incomplete");
    };
    let Some(attachment) = machine.attachment else {
        return malformed("provider-backed attachment specialization is incomplete");
    };
    let mut boundaries = BTreeSet::new();
    if provider_roots.is_empty()
        || machine
            .structural_parameters
            .iter()
            .any(|parameter| parameter.is_self)
        || provider_roots
            .iter()
            .any(|(root_attachment, field, boundary)| {
                *root_attachment != attachment
                    || *field != provider_field.id
                    || !boundaries.insert(*boundary)
                    || module
                        .boundary_machines
                        .iter()
                        .find(|declaration| declaration.id == *boundary)
                        .is_none_or(|declaration| declaration.attachment.is_some())
            })
    {
        return malformed("provider-backed attachment specialization is incomplete");
    }
    let called = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::BoundaryCall { boundary, .. } => Some(boundary),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    if called != boundaries {
        return malformed("provider-backed attachment specialization is incomplete");
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
            (
                StructuralPathSegment::Field(identity),
                StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. },
            ) => {
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
            (StructuralPathSegment::FixedIndex(_), StructuralTypeShape::Mixed { .. }) => {
                return malformed("structural path fixed index requires a fixed-array type");
            }
            (_, StructuralTypeShape::Sum { .. }) => {
                return malformed("structural path cannot traverse a payload-less sum");
            }
            (_, StructuralTypeShape::PrimitiveScalar(_)) => {
                return malformed("primitive-scalar structural type has no projected children");
            }
            (_, StructuralTypeShape::ByteSequence(_)) => {
                return malformed("byte-sequence structural type has no projected children");
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
        OperationKind::WriteOnlyPrimitiveStore { destination, value } => {
            if operation.result != OperationResult::Unit {
                return malformed("write-only primitive store declares a non-Unit result");
            }
            let Some(parameter) = machine
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == *destination)
            else {
                return malformed("write-only primitive store destination is not a parameter");
            };
            if !matches!(
                parameter.access,
                psi_terminal::StructuralAccess::MutableBorrow
                    | psi_terminal::StructuralAccess::WriteOnlyBorrow
            ) || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == *destination)
                || machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == *destination)
                || !matches!(
                    machine.structural_places.iter().find(|place| place.id == *destination),
                    Some(StructuralPlaceDeclaration {
                        kind: StructuralPlaceKind::Parameter { position, is_self },
                        ..
                    }) if *position == parameter.position && *is_self == parameter.is_self
                )
            {
                return malformed("write-only primitive store has invalid destination custody");
            }
            let Some(expected) = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == parameter.structural_type)
                .and_then(|declaration| match declaration.shape {
                    StructuralTypeShape::PrimitiveScalar(scalar_type) => Some(scalar_type),
                    _ => None,
                })
            else {
                return malformed("write-only primitive store requires a primitive-scalar root");
            };
            let actual = machine
                .parameters
                .iter()
                .chain(machine.result.scalar_ref())
                .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
                .chain(machine.blocks.iter().flat_map(|block| {
                    block
                        .operations
                        .iter()
                        .filter_map(|candidate| candidate.result.scalar_ref())
                }))
                .find(|declaration| declaration.id == *value)
                .map(|declaration| declaration.scalar_type);
            if actual != Some(expected) {
                return malformed("write-only primitive store value type does not match referent");
            }
        }
        OperationKind::EstablishPayloadlessCase { result_case } => {
            let Some(result) = operation.result.structural() else {
                return malformed("payloadless case establishment has no structural result");
            };
            if result.multiplicity != psi_terminal::StructuralMultiplicity::Unrestricted
                || !result.qualifications.is_empty()
                || !result.claims.is_empty()
            {
                return malformed("payloadless case establishment has an invalid result surface");
            }
            if !matches!(
                machine.structural_places.iter().find(|place| place.id == result.place),
                Some(StructuralPlaceDeclaration {
                    kind: StructuralPlaceKind::OperationResult { producer, structural_type },
                    ..
                }) if *producer == operation.id && *structural_type == result.structural_type
            ) {
                return malformed("payloadless case establishment has no matching result place");
            }
            let Some(declaration) = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == result.structural_type)
            else {
                return malformed("payloadless case establishment has an unknown structural type");
            };
            let StructuralTypeShape::Sum { cases } = &declaration.shape else {
                return malformed("payloadless case establishment requires a sum type");
            };
            if !cases
                .iter()
                .any(|case| case.id == *result_case && case.fields.is_empty())
            {
                return malformed(
                    "payloadless case establishment requires an exact payloadless member",
                );
            }
        }
        OperationKind::EstablishByteSequenceLiteral { destination, .. } => {
            if operation.result != OperationResult::Unit {
                return malformed("byte-sequence literal establishment declares a scalar result");
            }
            let Some(StructuralPlaceDeclaration {
                kind:
                    StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type, ..
                    },
                ..
            }) = machine
                .structural_places
                .iter()
                .find(|place| place.id == *destination)
            else {
                return malformed("byte-sequence literal establishment has no literal declaration");
            };
            let Some(declaration) = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == *structural_type)
            else {
                return malformed("byte-sequence literal has an unknown structural type");
            };
            if !matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(psi_terminal::ByteSequenceCarrier::BorrowedView)
            ) {
                return malformed("byte-sequence literal must use a borrowed byte-sequence type");
            }
        }
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
        OperationKind::CallStructural {
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
            selected_evidence,
        } => {
            let Some(callee) = module
                .machines
                .iter()
                .find(|candidate| candidate.id == *callee)
            else {
                return malformed("structural call references an unknown callee");
            };
            let Some(expected_result) = callee.result.structural() else {
                return malformed("structural call references a non-structural-result callee");
            };
            let Some(actual_result) = operation.result.structural() else {
                return malformed("structural call has no structural operation result");
            };
            let exact_payloadless = callee.parameters.is_empty()
                && callee.structural_parameters.is_empty()
                && callee.entry_claims.is_empty()
                && callee.content_entry_claims.is_empty()
                && callee.contract.requires.is_empty()
                && callee.contract.ensures.is_empty()
                && callee.contract.crash_routes.is_empty()
                && module
                    .evidence_contract_lanes
                    .iter()
                    .all(|lane| lane.machine != callee.id)
                && structural_arguments.is_empty()
                && claim_transfers.is_empty()
                && returned_claim_transfers.is_empty()
                && requirement_obligations.is_empty()
                && crash_continuations.is_empty()
                && actual_result.multiplicity == psi_terminal::StructuralMultiplicity::Unrestricted
                && expected_result.multiplicity
                    == psi_terminal::StructuralMultiplicity::Unrestricted
                && actual_result.qualifications.is_empty()
                && expected_result.qualifications.is_empty()
                && actual_result.claims.is_empty()
                && callee_exact_payloadless_return(callee);
            if !callee.parameters.is_empty()
                || (!selected_evidence.is_empty() && !exact_payloadless)
                || (!exact_payloadless
                    && (structural_arguments.len() != 1 || callee.structural_parameters.len() != 1))
                || actual_result.structural_type != expected_result.structural_type
                || actual_result.multiplicity != expected_result.multiplicity
                || actual_result.qualifications != expected_result.qualifications
            {
                return malformed(
                    "structural call has the wrong callee signature or structural arity",
                );
            }
            let Some(StructuralPlaceDeclaration {
                kind:
                    StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    },
                ..
            }) = machine
                .structural_places
                .iter()
                .find(|place| place.id == actual_result.place)
            else {
                return malformed(
                    "structural call result has no operation-result place declaration",
                );
            };
            if *producer != operation.id || *structural_type != actual_result.structural_type {
                return malformed("structural call result place disagrees with its producer");
            }
            if !has_structural_type(module, actual_result.structural_type) {
                return malformed("structural call result has an unknown structural type");
            }
            for qualification in &actual_result.qualifications {
                if !module
                    .structural_domains
                    .iter()
                    .any(|domain| domain.id == *qualification)
                {
                    return malformed("structural call result has an unknown structural domain");
                }
            }
            if exact_payloadless {
                return Ok(());
            }
            if actual_result.claims.is_empty()
                || claim_transfers.is_empty()
                || claim_transfers
                    .iter()
                    .any(|transfer| transfer.argument_index != 0)
                || returned_claim_transfers.is_empty()
            {
                return malformed("structural call requires a nonempty whole-root claim map");
            }
            let mut result_paths = Vec::with_capacity(actual_result.claims.len());
            for binding in &actual_result.claims {
                validate_structural_path(module, actual_result.structural_type, &binding.path)?;
                if result_paths
                    .iter()
                    .any(|previous: &Vec<StructuralPathSegment>| {
                        previous.starts_with(&binding.path) || binding.path.starts_with(previous)
                    })
                {
                    return malformed("structural call result has overlapping claim paths");
                }
                result_paths.push(binding.path.clone());
            }
            let caller_result_claims = actual_result
                .claims
                .iter()
                .map(|binding| (binding.claim, binding.path.as_slice()))
                .collect::<BTreeMap<_, _>>();
            let callee_claims = callee
                .entry_claims
                .iter()
                .map(|claim| (claim.claim, claim.path.as_slice()))
                .collect::<BTreeMap<_, _>>();
            let transferred_caller_claims = claim_transfers
                .iter()
                .map(|transfer| transfer.claim)
                .collect::<BTreeSet<_>>();
            let returned_callee_claims = returned_claim_transfers
                .iter()
                .map(|transfer| transfer.callee_claim)
                .collect::<BTreeSet<_>>();
            let returned_caller_claims = returned_claim_transfers
                .iter()
                .map(|transfer| transfer.caller_claim)
                .collect::<BTreeSet<_>>();
            if callee_claims.is_empty()
                || callee_claims.len() != callee.entry_claims.len()
                || caller_result_claims.len() != actual_result.claims.len()
                || transferred_caller_claims.len() != claim_transfers.len()
                || returned_callee_claims.len() != returned_claim_transfers.len()
                || returned_caller_claims.len() != returned_claim_transfers.len()
                || returned_callee_claims != callee_claims.keys().copied().collect()
                || returned_caller_claims != caller_result_claims.keys().copied().collect()
                || transferred_caller_claims != caller_result_claims.keys().copied().collect()
                || returned_claim_transfers.iter().any(|transfer| {
                    callee_claims.get(&transfer.callee_claim)
                        != caller_result_claims.get(&transfer.caller_claim)
                })
            {
                return malformed(
                    "structural call returned claims disagree with its result bindings",
                );
            }
            let expected_callee_returns = callee
                .entry_claims
                .iter()
                .map(|claim| claim.claim)
                .collect::<Vec<_>>();
            if callee.blocks.iter().any(|block| {
                matches!(
                    &block.terminator,
                    Terminator::ReturnStructural {
                        returned_claims,
                        ..
                    } if returned_claims != &expected_callee_returns
                )
            }) {
                return malformed(
                    "structural callee return does not preserve its exact entry claim map",
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
            arguments,
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
            if arguments.len() != boundary.scalar_parameters.len() {
                return malformed("boundary call has the wrong scalar arity");
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

fn callee_exact_payloadless_return(callee: &TerminalMachine) -> bool {
    let Some(result) = callee.result.structural() else {
        return false;
    };
    let mut has_return = false;
    for block in &callee.blocks {
        let Terminator::ReturnStructural {
            source,
            returned_claims,
            ..
        } = &block.terminator
        else {
            continue;
        };
        has_return = true;
        if !returned_claims.is_empty() {
            return false;
        }
        let Some(producer) = callee.structural_places.iter().find_map(|place| {
            (place.id == *source)
                .then_some(place.kind)
                .and_then(|kind| match kind {
                    StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    } if structural_type == result.structural_type => Some(producer),
                    _ => None,
                })
        }) else {
            return false;
        };
        let Some(operation) = callee
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| operation.id == producer)
        else {
            return false;
        };
        if !matches!(
            operation.kind,
            OperationKind::EstablishPayloadlessCase { .. }
        ) || !operation
            .result
            .structural()
            .is_some_and(|operation_result| {
                operation_result.place == *source
                    && operation_result.structural_type == result.structural_type
                    && operation_result.multiplicity
                        == psi_terminal::StructuralMultiplicity::Unrestricted
                    && operation_result.qualifications.is_empty()
                    && operation_result.claims.is_empty()
            })
        {
            return false;
        }
    }
    has_return
        && callee
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .all(|operation| {
                !matches!(
                    operation.kind,
                    OperationKind::Call { .. }
                        | OperationKind::CallUnit { .. }
                        | OperationKind::CallStructuralScalar { .. }
                        | OperationKind::CallStructural { .. }
                        | OperationKind::BoundaryCall { .. }
                )
            })
}

fn validate_structural_arguments(
    module: &TerminalModule,
    machine: &TerminalMachine,
    arguments: &[StructuralArgument],
    expected: &[StructuralParameterDeclaration],
) -> Result<(), CodecError> {
    for (argument, expected) in arguments.iter().zip(expected) {
        let Some(actual_type) = structural_place_type(machine, argument.place) else {
            return malformed("structural argument references an unknown structural place");
        };
        let actual_type = validate_structural_path(module, actual_type, &argument.path)?;
        if actual_type != expected.structural_type {
            return malformed("structural argument has the wrong concrete type");
        }
    }
    Ok(())
}

fn structural_place_type(
    machine: &TerminalMachine,
    place: psi_core::PlaceId,
) -> Option<StructuralTypeId> {
    machine
        .structural_parameters
        .iter()
        .find_map(|parameter| (parameter.place == place).then_some(parameter.structural_type))
        .or_else(|| {
            machine.structural_places.iter().find_map(|declaration| {
                if declaration.id != place {
                    return None;
                }
                match declaration.kind {
                    StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type, ..
                    }
                    | StructuralPlaceKind::TrivialAffineLocal {
                        structural_type, ..
                    }
                    | StructuralPlaceKind::OperationResult {
                        structural_type, ..
                    } => Some(structural_type),
                    StructuralPlaceKind::Parameter { .. }
                    | StructuralPlaceKind::ProviderAttachment { .. }
                    | StructuralPlaceKind::Result => None,
                }
            })
        })
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
            StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::ByteSequence(_) => {}
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
            StructuralTypeShape::Mixed { fields, cases } => {
                for field in fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                {
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
        structural_signature_wire::encode_structural_access(writer, argument.access);
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
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } => {
            writer.u8(6);
            writer.id(producer);
            writer.id(structural_type);
        }
        StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal,
            structural_type,
        } => {
            writer.u8(4);
            writer.u32(declaration_ordinal);
            writer.id(structural_type);
        }
        StructuralPlaceKind::ProviderAttachment {
            attachment,
            field,
            boundary,
        } => {
            writer.u8(5);
            writer.id(attachment);
            writer.id(field);
            writer.id(boundary);
        }
        StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal,
            structural_type,
            construction,
        } => {
            writer.u8(if construction.is_some() { 7 } else { 3 });
            writer.u32(declaration_ordinal);
            writer.id(structural_type);
            if let Some(construction) = construction {
                writer.id(construction.root_structural_type);
                writer.u64(construction.index);
            }
        }
    }
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
            access: structural_signature_wire::decode_structural_access(reader)?,
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
        6 => StructuralPlaceKind::OperationResult {
            producer: reader.id("OperationId")?,
            structural_type: reader.id("StructuralTypeId")?,
        },
        4 => StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: reader.u32()?,
            structural_type: reader.id("StructuralTypeId")?,
        },
        5 => StructuralPlaceKind::ProviderAttachment {
            attachment: reader.id("StructuralTypeId")?,
            field: reader.id("StructuralFieldId")?,
            boundary: reader.id("BoundaryMachineId")?,
        },
        3 => StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal: reader.u32()?,
            structural_type: reader.id("StructuralTypeId")?,
            construction: None,
        },
        7 => StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal: reader.u32()?,
            structural_type: reader.id("StructuralTypeId")?,
            construction: Some(psi_core::AffineConstructionElement {
                root_structural_type: reader.id("StructuralTypeId")?,
                index: reader.u64()?,
            }),
        },
        tag => return Err(CodecError::InvalidTag("StructuralPlaceKind", tag)),
    })
}

#[cfg(test)]
mod structural_place_wire_tests {
    use psi_core::{OperationId, PsiSemanticId, StructuralPlaceKind, StructuralTypeId};

    use super::{CodecError, decode_structural_place_kind, encode_structural_place_kind};
    use crate::wire::{Reader, Writer};

    fn id<T: PsiSemanticId>(raw: u64) -> T {
        T::new(raw).expect("test ids are nonzero")
    }

    #[test]
    fn operation_result_place_uses_stable_wire_tag_six() {
        let kind = StructuralPlaceKind::OperationResult {
            producer: id::<OperationId>(1),
            structural_type: id::<StructuralTypeId>(2),
        };
        let mut writer = Writer::default();
        encode_structural_place_kind(&mut writer, kind);
        let bytes = writer.finish();
        assert_eq!(bytes[0], 6);

        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_structural_place_kind(&mut reader), Ok(kind));
        assert_eq!(reader.remaining(), 0);

        let mut invalid = bytes;
        invalid[0] = 8;
        assert_eq!(
            decode_structural_place_kind(&mut Reader::new(&invalid)),
            Err(CodecError::InvalidTag("StructuralPlaceKind", 8))
        );
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
    ObligationLedgerMismatch,
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

#[cfg(test)]
mod crash_route_roster_tests {
    use psi_terminal::{CrashCause, CrashRouteBucket, CrashRouteGuard};

    use super::{CodecError, decode_crash_route_buckets, encode_crash_route_buckets};

    fn route(cause: CrashCause) -> CrashRouteBucket {
        CrashRouteBucket {
            cause,
            alternatives: vec![CrashRouteGuard::Truth],
        }
    }

    #[test]
    fn standalone_crash_route_roster_round_trips_and_rejects_corruption() {
        let routes = vec![route(CrashCause::Trap), route(CrashCause::Abort)];
        let encoded = encode_crash_route_buckets(&routes).unwrap();
        assert_eq!(decode_crash_route_buckets(&encoded).unwrap(), routes);

        let truncated = &encoded[..encoded.len() - 1];
        assert!(decode_crash_route_buckets(truncated).is_err());
    }

    #[test]
    fn standalone_crash_route_roster_rejects_noncanonical_order() {
        let routes = vec![route(CrashCause::Abort), route(CrashCause::Trap)];
        assert_eq!(
            encode_crash_route_buckets(&routes),
            Err(CodecError::NonCanonicalOrder(
                "crash routes by cause and guard"
            ))
        );
    }
}
