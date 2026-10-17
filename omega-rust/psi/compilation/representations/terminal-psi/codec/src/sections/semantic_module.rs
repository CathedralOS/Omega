//! The semantic module's canonical encoding: one `*_wire` file per section in
//! the order `module_wire` writes them, the byte cursors in `wire`, canonical
//! ordering rules and the structural-place encoders several sections share.
//! Encoding and identity preserve the producer's canonical bytes without
//! rechecking its structural foundations. Fingerprinting is used throughout
//! compilation; replaying payload histories there repeats semantic work for
//! every identity consumer. Foundation validation belongs to external decoding,
//! also exercised by the corpus transport checks, not producer serialization.
//!
//! Decoding checks canonical wire and representation-foundation shape. Semantic
//! checking belongs to the producer; this codec does not replay it.

pub(crate) mod block_wire;
pub(crate) mod canonical_order;
pub(crate) mod content_wire;
pub(crate) mod contract_wire;
pub(crate) mod dynamic_result_admission;
pub(crate) mod integer_math_term_wire;
mod machine_declaration_wire;
pub(crate) mod machine_wire;
pub(crate) mod mathematical_certificate_wire;
mod module_foundation_validation;
pub(crate) mod module_wire;
pub(crate) mod proof_declaration_wire;
pub(crate) mod proof_term_wire;
pub(crate) mod proposition_wire;
pub(crate) mod provider_candidate_wire;
pub(crate) mod reach_application_wire;
pub(crate) mod scalar_qualification_wire;
pub(crate) mod scalar_term_wire;
pub(crate) mod scalar_wire;
pub(crate) mod structural_field_wire;
pub(crate) mod structural_place_wire;
pub(crate) mod structural_result_wire;
pub(crate) mod structural_signature_wire;
pub(crate) mod structural_type_wire;
pub(crate) mod wire;

#[cfg(test)]
mod target_family_tests;

pub(crate) use crate::codec_error::CodecError;
use canonical_order::{
    canonical_proposition_order_key, crash_routes_are_canonical, validate_canonical_order,
    validate_crash_route_predicates,
};
use contract_wire::{decode_crash_routes, encode_crash_routes};
use module_foundation_validation::validate_structural_foundation;
use module_wire::{decode_module_body, encode_raw};

#[cfg(test)]
mod qualified_cleanup_receiving_tests;
use proposition_wire::{decode_proposition, encode_proposition};
use scalar_term_wire::{decode_scalar_term, encode_scalar_term};
use terminal_psi::{SemanticFingerprint, TerminalModule, TerminalPsiIdentity};
use wire::{Reader, Writer, decode_counted};

pub(crate) const MAGIC: &[u8; 8] = b"PSITERM\0";

// Closed boundary application demands follow the target-family roster. Returns
// and call transfers additionally preserve typed paths and their Case guards.
// Machine and callback bodies retain their checked declaration identities.
// Normal exclusive calls also retain their caller-local storage custody joins.
// Boundary requirements retain their exact nominal service owner.
// Suspension storage operands retain the same access axis as ordinary calls.
// Owned state arrivals can bind their live claims into block-local names.
// Stored references retain identity-only targets without demanding storage.
// Computed dispatch slots retain their public relative claim paths.
// Captured call loans name the evaluated actual and its exact borrowed leaf.
// Target families retain their exact checked published-head commitment.
// Boundary structural results retain declaration-owned descendant memberships.
// Target layout applications retain invariant Schema inputs and guarded geometries.
// Identities live in the module-owned interned table rather than per-row text.
// Dynamic slots retain structural result shapes and public result claim paths.
// Boundary owner/role rows and dynamic results share this combined format;
// earlier candidate format 142 did not yet encode the owner/role fields.
// Reborrow custody retains opaque selector formation occurrences.
// Structural declarations retain checked no-code affine cleanup authority.
// State-exit reborrow certificates are not an executable Terminal product.
// Provider candidates retain checked receiving applications and ordered regions.
// Descriptor cleanup admits exclusive pointers without disposal of their referees.
// Whole byte operands retain their exact descriptor projection and access.
// Returned storage origins retain admitted declaration-relative interpretations.
// Placed input rows retain their dense formal coordinate and carrier identity.
// Returned origins distinguish precise projections from public input regions.
// Placed views, quotients and captured closed conformances share this schema.
// Conditional descriptor origins share immediate supplier dependencies.
// Mixed structural calls carry outcome-specific selections.
// Element writes retain full descriptor operands; stored element descriptors
// retain their declaration-side backing access ceiling.
// Fixed-array memberships and stored descriptors share the combined schema.
pub(crate) const FORMAT_MARKER: u16 = 162;

pub(crate) const FINGERPRINT_DOMAIN: &[u8] = b"psi-terminal-semantic-fingerprint\0";

pub(crate) const MAX_PROPOSITION_DEPTH: usize = 256;

pub(crate) const MAX_SCALAR_TERM_DEPTH: usize = 256;

pub(crate) const MAX_CONTENT_TERM_DEPTH: usize = 256;

pub(crate) const MAX_CONTENT_IDENTITY_BYTES: usize = 1 << 20;

/// Encode the producer's module in canonical wire order, without admitting its
/// structural relationships. A fingerprint of these bytes is identity, not
/// evidence of validity. Untrusted bytes must enter through [`decode_module`].
pub fn encode_module(module: &TerminalModule) -> Result<Vec<u8>, CodecError> {
    scalar_qualification_wire::validate(&module.scalar_qualifications)?;
    validate_canonical_order(module)?;
    encode_raw(module)
}

/// Encode one structural declaration using the exact module-section wire format.
///
/// This omits the module header and declaration count. Encoding alone does not
/// validate the referenced-type closure or establish any value invariant.
pub fn encode_structural_type_declaration(
    declaration: &terminal_psi::StructuralTypeDeclaration,
) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    structural_type_wire::encode_structural_type(&mut writer, declaration)?;
    Ok(writer.finish())
}

/// How many times this process has run [`decode_module`] to completion or
/// rejection. Canonical decoding replays every module validation and re-encodes
/// the module to compare bytes, so it is the compiler's most expensive repeated
/// admission step; this counter is the instrument that keeps a realization
/// honest about how often it pays for it. It is a measurement, not control
/// state: nothing branches on its value.
static MODULE_DECODES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The running [`decode_module`] count for this process.
pub fn module_decode_count() -> u64 {
    MODULE_DECODES.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn decode_module(bytes: &[u8]) -> Result<TerminalModule, CodecError> {
    MODULE_DECODES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
    scalar_qualification_wire::validate(&module.scalar_qualifications)?;
    validate_canonical_order(&module)?;
    validate_structural_foundation(&module)?;
    dynamic_result_admission::validate_dynamic_descriptor_results(&module)?;
    let canonical = encode_raw(&module)?;
    if canonical != bytes {
        return Err(CodecError::NonCanonicalEncoding);
    }
    Ok(module)
}

/// Encode one canonical ordered crash-continuation roster without a Terminal
/// module envelope. This is the shared semantic leaf used by later
/// identity-bearing optimizer artifacts that must retain exact call routes.
pub fn encode_crash_route_buckets(
    routes: &[terminal_psi::CrashRouteBucket],
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
) -> Result<Vec<terminal_psi::CrashRouteBucket>, CodecError> {
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

/// Decode a complete canonical proposition ordering key.
pub fn decode_canonical_proposition(
    bytes: &[u8],
) -> Result<semantic_vocabulary::Proposition, CodecError> {
    let mut reader = Reader::new(bytes);
    let proposition = decode_proposition(&mut reader, 0)?;
    if reader.remaining() != 0 {
        return Err(CodecError::TrailingBytes(reader.remaining()));
    }
    if canonical_proposition_order_key(&proposition)? != bytes {
        return Err(CodecError::NonCanonicalEncoding);
    }
    Ok(proposition)
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
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(FINGERPRINT_DOMAIN);
    let byte_len =
        u64::try_from(bytes.len()).expect("terminal-Psi bytes fit the u64 digest domain");
    digest.raw(&byte_len.to_le_bytes());
    digest.raw(&bytes);
    SemanticFingerprint::from_bytes(digest.finish())
}
