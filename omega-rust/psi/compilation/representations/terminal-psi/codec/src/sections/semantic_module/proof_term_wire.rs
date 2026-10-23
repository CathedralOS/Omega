//! Canonical recursive proof-term wire format.
//!
//! Proof terms are proof-only erased actuals — they own no runtime value and
//! carry only semantic identities. This module owns the exact variant tags,
//! recursive field order, and decode-time structural validation for them, plus
//! the `ErasedProofFormal` roster encoding.

use semantic_vocabulary::{MathematicalArgument, ProofFormalScope, ProofTerm, ProofTermField};
use terminal_psi::ErasedProofFormal;
use terminal_psi::TerminalIdentityIndex;

use super::scalar_term_wire::{decode_scalar_term, encode_scalar_term};
use super::wire::{Reader, Writer, bounded_count, decode_bytes, decode_counted};
use super::{CodecError, MAX_SCALAR_TERM_DEPTH};

const PROOF_TERM_CONSTRUCTION: u8 = 1;
const PROOF_TERM_FORMAL: u8 = 2;
const PROOF_TERM_SCALAR: u8 = 3;
const PROOF_TERM_ARRAY: u8 = 4;
const PROOF_TERM_MATHEMATICAL: u8 = 5;
const PROOF_TERM_MATHEMATICAL_APPLICATION: u8 = 6;
const PROOF_TERM_INVOCATION_FORMAL: u8 = 7;
// Scalar application lanes have their own tag. Existing erased-only terms keep
// their canonical bytes; mixed terms explicitly encode every argument kind.
const PROOF_TERM_MIXED_MATHEMATICAL_APPLICATION: u8 = 8;

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
        ProofTerm::MathematicalApplication {
            type_identity,
            certificate,
            arguments,
            formal_scope,
        } => {
            let mixed = arguments
                .iter()
                .any(|argument| matches!(argument, MathematicalArgument::Scalar { .. }));
            writer.u8(if mixed {
                PROOF_TERM_MIXED_MATHEMATICAL_APPLICATION
            } else {
                PROOF_TERM_MATHEMATICAL_APPLICATION
            });
            writer.string("proof term type identity", type_identity)?;
            writer.len("mathematical function certificate", certificate.len())?;
            writer.bytes(certificate);
            writer.u8(match formal_scope {
                ProofFormalScope::Block => 0,
                ProofFormalScope::Invocation => 1,
            });
            writer.len("mathematical function arguments", arguments.len())?;
            for argument in arguments {
                match argument {
                    MathematicalArgument::Formal { position } => {
                        if mixed {
                            writer.u8(0);
                        }
                        writer.u32(*position);
                    }
                    MathematicalArgument::Scalar {
                        type_identity,
                        term,
                    } => {
                        writer.u8(1);
                        writer.string("mathematical scalar source carrier", type_identity)?;
                        encode_scalar_term(writer, term, depth + 1)?;
                    }
                }
            }
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
        ProofTerm::InvocationFormal { position } => {
            writer.u8(PROOF_TERM_INVOCATION_FORMAL);
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
    decode_counted(reader, |reader| decode_proof_term(reader, 0))
}

fn decode_proof_term(reader: &mut Reader<'_>, depth: usize) -> Result<ProofTerm, CodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(CodecError::ScalarTermNestingTooDeep);
    }
    match reader.u8()? {
        PROOF_TERM_MATHEMATICAL => {
            let type_identity = reader.string("proof term type identity")?;
            let certificate = decode_bytes(reader)?;
            super::mathematical_certificate_wire::decode_mathematical_certificate(&certificate)?;
            let term = ProofTerm::Mathematical {
                type_identity,
                certificate,
            };
            term.validate().map_err(CodecError::MalformedProposition)?;
            Ok(term)
        }
        tag @ (PROOF_TERM_MATHEMATICAL_APPLICATION | PROOF_TERM_MIXED_MATHEMATICAL_APPLICATION) => {
            let type_identity = reader.string("proof term type identity")?;
            let certificate = decode_bytes(reader)?;
            super::mathematical_certificate_wire::decode_mathematical_certificate(&certificate)?;
            let formal_scope = match reader.u8()? {
                0 => ProofFormalScope::Block,
                1 => ProofFormalScope::Invocation,
                tag => return Err(CodecError::InvalidTag("proof formal scope", tag)),
            };
            let minimum_argument_bytes = if tag == PROOF_TERM_MIXED_MATHEMATICAL_APPLICATION {
                5
            } else {
                4
            };
            let count = bounded_count(reader, minimum_argument_bytes)?;
            let mut arguments = Vec::with_capacity(count);
            for _ in 0..count {
                let kind = if tag == PROOF_TERM_MIXED_MATHEMATICAL_APPLICATION {
                    reader.u8()?
                } else {
                    0
                };
                arguments.push(match kind {
                    0 => MathematicalArgument::Formal {
                        position: reader.u32()?,
                    },
                    1 => MathematicalArgument::Scalar {
                        type_identity: reader.string("mathematical scalar source carrier")?,
                        term: decode_scalar_term(reader, depth + 1)?,
                    },
                    tag => return Err(CodecError::InvalidTag("mathematical argument kind", tag)),
                });
            }
            let term = ProofTerm::MathematicalApplication {
                type_identity,
                certificate,
                arguments,
                formal_scope,
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
            let fields = decode_counted(reader, |reader| {
                Ok(ProofTermField {
                    field_identity: reader.string("proof term field identity")?,
                    term: decode_proof_term(reader, depth + 1)?,
                })
            })?;
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
            let elements = decode_counted(reader, |reader| decode_proof_term(reader, depth + 1))?;
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
        PROOF_TERM_INVOCATION_FORMAL => Ok(ProofTerm::InvocationFormal {
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
    decode_counted(reader, |reader| {
        let source_position = reader.u32()?;
        let type_identity = TerminalIdentityIndex(reader.u32()?);
        if type_identity.is_absent() {
            return Err(CodecError::MalformedStructuralFoundation(
                "empty erased proof formal type",
            ));
        }
        Ok(ErasedProofFormal {
            source_position,
            type_identity,
        })
    })
}

#[cfg(test)]
mod mathematical_tests {
    use super::*;
    use proof_admission::{Declaration, MathematicalCertificate, Term, TermArena};

    #[test]
    fn mixed_application_wire_tags_and_argument_roster_are_explicit() {
        // A serialization fixture, not semantic receiving authority. Engine
        // acceptance and corruption controls use source-produced certificates.
        let mut arena = TermArena::new();
        let two = arena.insert(Term::Two);
        let zero = arena.insert(Term::TwoZero);
        let certificate =
            super::super::mathematical_certificate_wire::encode_mathematical_certificate(
                &arena,
                &MathematicalCertificate {
                    signature: vec![],
                    level_arity: 0,
                    context: vec![],
                    term: zero,
                    expected: two,
                },
            )
            .unwrap();
        let term = ProofTerm::MathematicalApplication {
            type_identity: "Example".into(),
            certificate,
            arguments: vec![
                MathematicalArgument::Scalar {
                    type_identity: "Primitive".into(),
                    term: semantic_vocabulary::ScalarTerm::Value {
                        id: semantic_vocabulary::ValueId::new(1).unwrap(),
                        scalar_type: semantic_vocabulary::ScalarType::Boolean,
                    },
                },
                MathematicalArgument::Formal { position: 0 },
            ],
            formal_scope: ProofFormalScope::Invocation,
        };
        let mut writer = Writer::default();
        encode_proof_terms(&mut writer, std::slice::from_ref(&term)).unwrap();
        let encoded = writer.finish();
        assert_eq!(encoded[4], PROOF_TERM_MIXED_MATHEMATICAL_APPLICATION);
        assert_eq!(
            decode_proof_terms(&mut Reader::new(&encoded)).unwrap(),
            vec![term]
        );
        let mut prefix = Reader::new(&encoded);
        assert_eq!(prefix.count().unwrap(), 1);
        prefix.u8().unwrap();
        prefix.string("identity").unwrap();
        let bytes = prefix.count().unwrap() as usize;
        prefix.take(bytes).unwrap();
        prefix.u8().unwrap();
        prefix.count().unwrap();
        let argument_offset = encoded.len() - prefix.remaining();
        let mut changed = encoded.clone();
        changed[argument_offset] = 255;
        assert!(matches!(
            decode_proof_terms(&mut Reader::new(&changed)),
            Err(CodecError::InvalidTag("mathematical argument kind", 255))
        ));
        let mut missing = encoded.clone();
        missing.pop();
        assert!(decode_proof_terms(&mut Reader::new(&missing)).is_err());
        // The old erased-only spelling remains tag 6 byte-for-byte. An
        // alternative tag-8 spelling with no Scalar is noncanonical: module
        // ingress performs this same decode/re-encode byte comparison.
        let mut erased_only = decode_proof_terms(&mut Reader::new(&encoded)).unwrap();
        let ProofTerm::MathematicalApplication { arguments, .. } = &mut erased_only[0] else {
            unreachable!()
        };
        arguments.retain(|argument| matches!(argument, MathematicalArgument::Formal { .. }));
        let mut writer = Writer::default();
        encode_proof_terms(&mut writer, &erased_only).unwrap();
        let erased_bytes = writer.finish();
        assert_eq!(erased_bytes[4], PROOF_TERM_MATHEMATICAL_APPLICATION);
        let mut alternative = erased_bytes.clone();
        alternative[4] = PROOF_TERM_MIXED_MATHEMATICAL_APPLICATION;
        alternative.insert(argument_offset, 0);
        let decoded = decode_proof_terms(&mut Reader::new(&alternative)).unwrap();
        let mut writer = Writer::default();
        encode_proof_terms(&mut writer, &decoded).unwrap();
        let canonical = writer.finish();
        assert_eq!(canonical, erased_bytes);
        assert_ne!(canonical, alternative);
        let mut count_overflow = encoded.clone();
        count_overflow[argument_offset - 4..argument_offset]
            .copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            decode_proof_terms(&mut Reader::new(&count_overflow)),
            Err(CodecError::UnexpectedEnd)
        ));
    }

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
