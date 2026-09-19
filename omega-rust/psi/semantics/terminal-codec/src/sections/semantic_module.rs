//! The semantic module's canonical encoding: one `*_wire` file per section in
//! the order `module_wire` writes them, the byte cursors in `wire`, canonical
//! ordering rules, the structural-place encoders several sections share, and
//! the representation-foundation checks that run before any byte is written.
//! The crate root owns `encode_module` and `decode_module`; this module owns
//! every section beneath them and the depth and size limits they enforce.

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
#[allow(unused_imports)]
use crate::{CanonicalTerminalArtifact, CanonicalTerminalArtifactError};
#[allow(unused_imports)]
use crate::{decode_module, encode_module, semantic_fingerprint, terminal_psi_identity};
#[allow(unused_imports)]
use module_wire::encode_raw;
#[allow(unused_imports)]
use proposition_wire::{decode_proposition, encode_proposition};
#[allow(unused_imports)]
use scalar_term_wire::{decode_scalar_term, encode_scalar_term};
#[allow(unused_imports)]
use sha2::{Digest, Sha256};
#[allow(unused_imports)]
use terminal_psi::TerminalModule;
#[allow(unused_imports)]
use terminal_verifier::validate_module_representation;
#[allow(unused_imports)]
use wire::decode_counted;

pub(crate) const MAGIC: &[u8; 8] = b"PSITERM\0";

pub(crate) const FORMAT_MARKER: u16 = 101;

pub(crate) const FINGERPRINT_DOMAIN: &[u8] = b"psi-terminal-semantic-fingerprint\0";

pub(crate) const MAX_PROPOSITION_DEPTH: usize = 256;

pub(crate) const MAX_SCALAR_TERM_DEPTH: usize = 256;

pub(crate) const MAX_CONTENT_TERM_DEPTH: usize = 256;

pub(crate) const MAX_CONTENT_IDENTITY_BYTES: usize = 1 << 20;
