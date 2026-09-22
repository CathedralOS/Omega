//! Canonical recursive proof-term wire format.
//!
//! Proof terms are proof-only erased actuals — they own no runtime value and
//! carry only semantic identities. This module owns the exact variant tags,
//! recursive field order, and decode-time structural validation for them, plus
//! the `ErasedProofFormal` roster encoding.

use semantic_vocabulary::{ProofTerm, ProofTermField};
use terminal_psi::ErasedProofFormal;

use super::scalar_term_wire::{decode_scalar_term, encode_scalar_term};
use super::wire::{Reader, Writer};
use super::{CodecError, MAX_SCALAR_TERM_DEPTH};

const PROOF_TERM_CONSTRUCTION: u8 = 1;
const PROOF_TERM_FORMAL: u8 = 2;
const PROOF_TERM_SCALAR: u8 = 3;

pub(crate) fn encode_proof_terms(
    writer: &mut Writer,
    terms: &[ProofTerm],
) -> Result<(), CodecError> {
    writer.len("proof terms", terms.len())?;
    for term in terms {
        encode_proof_term(writer, term, 0)?;
    }
    Ok(())
}

fn encode_proof_term(
    writer: &mut Writer,
    term: &ProofTerm,
    depth: usize,
) -> Result<(), CodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(CodecError::ScalarTermNestingTooDeep);
    }
    match term {
        ProofTerm::Construction {
            type_identity,
            case_identity,
            fields,
        } => {
            writer.u8(PROOF_TERM_CONSTRUCTION);
            writer.string("proof term type identity", type_identity)?;
            writer.boolean(case_identity.is_some());
            if let Some(case_identity) = case_identity {
                writer.string("proof term case identity", case_identity)?;
            }
            writer.len("proof term fields", fields.len())?;
            for field in fields {
                encode_proof_term_field(writer, field, depth + 1)?;
            }
        }
        ProofTerm::Formal { position } => {
            writer.u8(PROOF_TERM_FORMAL);
            writer.u32(*position);
        }
        ProofTerm::Scalar(scalar) => {
            writer.u8(PROOF_TERM_SCALAR);
            encode_scalar_term(writer, scalar, depth)?;
        }
    }
    Ok(())
}

fn encode_proof_term_field(
    writer: &mut Writer,
    field: &ProofTermField,
    depth: usize,
) -> Result<(), CodecError> {
    writer.string("proof term field identity", &field.field_identity)?;
    encode_proof_term(writer, &field.term, depth)
}

pub(crate) fn decode_proof_terms(reader: &mut Reader<'_>) -> Result<Vec<ProofTerm>, CodecError> {
    let count = reader.count()?;
    let mut terms = Vec::with_capacity(usize::try_from(count).expect("u32 count fits usize"));
    for _ in 0..count {
        terms.push(decode_proof_term(reader, 0)?);
    }
    Ok(terms)
}

fn decode_proof_term(reader: &mut Reader<'_>, depth: usize) -> Result<ProofTerm, CodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(CodecError::ScalarTermNestingTooDeep);
    }
    match reader.u8()? {
        PROOF_TERM_CONSTRUCTION => {
            let type_identity = reader.string("proof term type identity")?;
            let case_identity = if reader.boolean()? {
                Some(reader.string("proof term case identity")?)
            } else {
                None
            };
            let field_count = reader.count()?;
            let mut fields =
                Vec::with_capacity(usize::try_from(field_count).expect("u32 count fits usize"));
            for _ in 0..field_count {
                let field_identity = reader.string("proof term field identity")?;
                fields.push(ProofTermField {
                    field_identity,
                    term: decode_proof_term(reader, depth + 1)?,
                });
            }
            let term = ProofTerm::Construction {
                type_identity,
                case_identity,
                fields,
            };
            term.validate().map_err(CodecError::MalformedProposition)?;
            Ok(term)
        }
        PROOF_TERM_FORMAL => Ok(ProofTerm::Formal {
            position: reader.u32()?,
        }),
        PROOF_TERM_SCALAR => {
            let term = ProofTerm::Scalar(decode_scalar_term(reader, depth)?);
            term.validate().map_err(CodecError::MalformedProposition)?;
            Ok(term)
        }
        tag => Err(CodecError::InvalidTag("proof term", tag)),
    }
}

pub(crate) fn encode_proof_formals(
    writer: &mut Writer,
    formals: &[ErasedProofFormal],
) -> Result<(), CodecError> {
    writer.len("erased proof formals", formals.len())?;
    for formal in formals {
        writer.u32(formal.source_position);
        writer.string("erased proof formal type", &formal.type_identity)?;
    }
    Ok(())
}

pub(crate) fn decode_proof_formals(
    reader: &mut Reader<'_>,
) -> Result<Vec<ErasedProofFormal>, CodecError> {
    let count = reader.count()?;
    let mut formals = Vec::with_capacity(usize::try_from(count).expect("u32 count fits usize"));
    for _ in 0..count {
        let source_position = reader.u32()?;
        let type_identity = reader.string("erased proof formal type")?;
        if type_identity.is_empty() {
            return Err(CodecError::MalformedStructuralFoundation(
                "empty erased proof formal type",
            ));
        }
        formals.push(ErasedProofFormal {
            source_position,
            type_identity,
        });
    }
    Ok(formals)
}
