//! The semantic module's canonical encoding: one `*_wire` file per section in
//! the order `module_wire` writes them, the byte cursors in `wire`, canonical
//! ordering rules, the structural-place encoders several sections share, and
//! the representation-foundation checks that run before any byte is written.
//! `encode_module` and `decode_module` sequence canonical ordering, representation
//! validation, and wire encoding here. Identity derives from those exact canonical
//! bytes. These checks establish representation shape, not proof admission.

pub(crate) mod block_wire;
pub(crate) mod canonical_order;
pub(crate) mod content_wire;
pub(crate) mod contract_wire;
#[cfg(test)]
mod current_format_tests;
pub(crate) mod dynamic_dispatch_wire;
pub(crate) mod integer_math_term_wire;
pub(crate) mod machine_wire;
pub(crate) mod mathematical_certificate_wire;
pub(crate) mod module_foundation_validation;
pub(crate) mod module_wire;
pub(crate) mod proof_declaration_wire;
pub(crate) mod proof_term_wire;
pub(crate) mod proposition_wire;
pub(crate) mod provider_candidate_wire;
pub(crate) mod quotient_correspondence_wire;
pub(crate) mod reach_application_wire;
pub(crate) mod scalar_qualification_wire;
pub(crate) mod scalar_term_wire;
pub(crate) mod scalar_wire;
#[cfg(test)]
mod structural_block_wire_tests;
pub(crate) mod structural_field_wire;
pub(crate) mod structural_place_wire;
pub(crate) mod structural_result_wire;
pub(crate) mod structural_signature_wire;
pub(crate) mod structural_type_wire;
pub(crate) mod wire;

pub(crate) use crate::codec_error::CodecError;
use canonical_order::{
    canonical_proposition_order_key, crash_routes_are_canonical, validate_canonical_order,
    validate_crash_route_predicates,
};
use contract_wire::{decode_crash_routes, encode_crash_routes};
use module_foundation_validation::validate_structural_foundation;
use module_wire::{decode_module_body, encode_raw};
use proposition_wire::{decode_proposition, encode_proposition};
use scalar_term_wire::{decode_scalar_term, encode_scalar_term};
use sha2::{Digest, Sha256};
use terminal_psi::{SemanticFingerprint, TerminalModule, TerminalPsiIdentity};
use terminal_verifier::validate_module_representation;
use wire::{Reader, Writer, decode_counted};

pub(crate) const MAGIC: &[u8; 8] = b"PSITERM\0";

pub(crate) const FORMAT_MARKER: u16 = 115;

pub(crate) const FINGERPRINT_DOMAIN: &[u8] = b"psi-terminal-semantic-fingerprint\0";

pub(crate) const MAX_PROPOSITION_DEPTH: usize = 256;

pub(crate) const MAX_SCALAR_TERM_DEPTH: usize = 256;

pub(crate) const MAX_CONTENT_TERM_DEPTH: usize = 256;

pub(crate) const MAX_CONTENT_IDENTITY_BYTES: usize = 1 << 20;

pub fn encode_module(module: &TerminalModule) -> Result<Vec<u8>, CodecError> {
    scalar_qualification_wire::validate(&module.scalar_qualifications)?;
    validate_canonical_order(module)?;
    validate_structural_foundation(module)?;
    validate_module_representation(module).map_err(CodecError::InvalidModule)?;
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
    scalar_qualification_wire::validate(&module.scalar_qualifications)?;
    validate_canonical_order(&module)?;
    validate_structural_foundation(&module)?;
    validate_module_representation(&module).map_err(CodecError::InvalidModule)?;
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
    let mut digest = Sha256::new();
    digest.update(FINGERPRINT_DOMAIN);
    let byte_len =
        u64::try_from(bytes.len()).expect("terminal-Psi bytes fit the u64 digest domain");
    digest.update(byte_len.to_le_bytes());
    digest.update(bytes);
    SemanticFingerprint::from_bytes(digest.finalize().into())
}
