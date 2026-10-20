//! Canonical recursive proof-term wire format.
//!
//! Proof terms are proof-only erased actuals — they own no runtime value and
//! carry only semantic identities. This module owns the exact variant tags,
//! recursive field order, and decode-time structural validation for them, plus
//! the `ErasedProofFormal` roster encoding.

use semantic_vocabulary::{ProofTerm, ProofTermField};
use terminal_psi::ErasedProofFormal;
use terminal_psi::TerminalIdentityIndex;

use super::scalar_term_wire::{decode_scalar_term, encode_scalar_term};
use super::wire::{Reader, Writer};
use super::{CodecError, MAX_SCALAR_TERM_DEPTH};

const PROOF_TERM_CONSTRUCTION: u8 = 1;
const PROOF_TERM_FORMAL: u8 = 2;
const PROOF_TERM_SCALAR: u8 = 3;
const PROOF_TERM_ARRAY: u8 = 4;
const PROOF_TERM_MATHEMATICAL: u8 = 5;

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
        ProofTerm::Mathematical {
            type_identity,
            certificate,
        } => {
            writer.u8(PROOF_TERM_MATHEMATICAL);
            writer.string("proof term type identity", type_identity)?;
            writer.len("mathematical proof certificate", certificate.len())?;
            writer.bytes(certificate);
        }
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
        ProofTerm::Array {
            type_identity,
            elements,
        } => {
            writer.u8(PROOF_TERM_ARRAY);
            writer.string("proof term type identity", type_identity)?;
            writer.len("proof array elements", elements.len())?;
            for element in elements {
                encode_proof_term(writer, element, depth + 1)?;
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
        PROOF_TERM_MATHEMATICAL => {
            let type_identity = reader.string("proof term type identity")?;
            let length = usize::try_from(reader.count()?).expect("u32 fits usize");
            let certificate = reader.take(length)?.to_vec();
            super::mathematical_certificate_wire::decode_mathematical_certificate(&certificate)?;
            let term = ProofTerm::Mathematical {
                type_identity,
                certificate,
            };
            term.validate().map_err(CodecError::MalformedProposition)?;
            Ok(term)
        }
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
        PROOF_TERM_ARRAY => {
            let type_identity = reader.string("proof term type identity")?;
            let count = reader.count()?;
            let mut elements =
                Vec::with_capacity(usize::try_from(count).expect("u32 count fits usize"));
            for _ in 0..count {
                elements.push(decode_proof_term(reader, depth + 1)?);
            }
            let term = ProofTerm::Array {
                type_identity,
                elements,
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
        writer.u32(formal.type_identity.0);
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
        let type_identity = TerminalIdentityIndex(reader.u32()?);
        if type_identity.is_absent() {
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

#[cfg(test)]
mod mathematical_tests {
    use super::*;
    use proof_admission::{Declaration, MathematicalCertificate, Term, TermArena};

    #[test]
    fn mathematical_actual_round_trips_complete_signature_and_rejects_wire_changes() {
        let mut arena = TermArena::new();
        let two = arena.insert(Term::Two);
        let zero = arena.insert(Term::TwoZero);
        let declaration = Declaration::assumption(0, two);
        let subject = arena.insert(Term::Constant {
            declaration: 0,
            levels: vec![],
        });
        let certificate = MathematicalCertificate {
            signature: vec![declaration],
            level_arity: 0,
            context: vec![],
            term: subject,
            expected: two,
        };
        let bytes = super::super::mathematical_certificate_wire::encode_mathematical_certificate(
            &arena,
            &certificate,
        )
        .unwrap();
        let term = ProofTerm::Mathematical {
            type_identity: "Example<0>".into(),
            certificate: bytes,
        };
        let mut writer = Writer::default();
        encode_proof_terms(&mut writer, std::slice::from_ref(&term)).unwrap();
        let encoded = writer.finish();
        let mut reader = Reader::new(&encoded);
        assert_eq!(decode_proof_terms(&mut reader).unwrap(), vec![term]);
        assert_eq!(reader.remaining(), 0);

        let mut changed = encoded.clone();
        changed.pop();
        assert!(decode_proof_terms(&mut Reader::new(&changed)).is_err());
        let empty = ProofTerm::Mathematical {
            type_identity: String::new(),
            certificate:
                super::super::mathematical_certificate_wire::encode_mathematical_certificate(
                    &arena,
                    &MathematicalCertificate {
                        term: zero,
                        ..certificate
                    },
                )
                .unwrap(),
        };
        let mut writer = Writer::default();
        encode_proof_terms(&mut writer, &[empty]).unwrap();
        assert!(decode_proof_terms(&mut Reader::new(&writer.finish())).is_err());
    }
}
