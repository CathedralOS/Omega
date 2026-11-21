use semantic_vocabulary::{IntegerMathLiteral, IntegerMathTerm};

use crate::CodecError;
use crate::terms::scalar::{decode_scalar_term, encode_scalar_term};
use crate::terms::scalar_types::{decode_integer_type, encode_integer_type};
use crate::terms::table::{TableChildren, TermEncoding, decode_term_table, encode_term_table};
use crate::wire::{Reader, Writer};

/// Encodes one integer math term as a postorder table; an embedded scalar
/// term is its own scalar table inside the embedding row.
pub fn encode_integer_math_term(
    writer: &mut Writer,
    term: &IntegerMathTerm,
) -> Result<(), CodecError> {
    encode_term_table::<IntegerMathTable>(writer, term)
}

pub fn decode_integer_math_term(reader: &mut Reader<'_>) -> Result<IntegerMathTerm, CodecError> {
    decode_term_table::<IntegerMathTable>(reader)
}

struct IntegerMathTable;

impl TermEncoding for IntegerMathTable {
    type Term = IntegerMathTerm;
    const LABEL: &'static str = "integer math term";

    fn children<'term>(term: &'term Self::Term, children: &mut Vec<&'term Self::Term>) {
        match term {
            IntegerMathTerm::Add(left, right)
            | IntegerMathTerm::Subtract(left, right)
            | IntegerMathTerm::Multiply(left, right)
            | IntegerMathTerm::ShiftLeft {
                value: left,
                count: right,
            } => children.extend([left.as_ref(), right.as_ref()]),
            IntegerMathTerm::Embed(_)
            | IntegerMathTerm::IntegerLiteral(_)
            | IntegerMathTerm::MathValue { .. } => {}
        }
    }

    fn write_row(term: &Self::Term, writer: &mut Writer) -> Result<(), CodecError> {
        match term {
            IntegerMathTerm::Embed(term) => {
                writer.u8(7);
                encode_scalar_term(writer, term)?;
            }
            IntegerMathTerm::IntegerLiteral(literal) => {
                writer.u8(1);
                writer.boolean(literal.negative());
                writer.bytes(&literal.magnitude().to_le_bytes());
            }
            IntegerMathTerm::MathValue { source_type, value } => {
                writer.u8(2);
                encode_integer_type(writer, *source_type)?;
                writer.id(*value);
            }
            IntegerMathTerm::Add(_, _) => writer.u8(3),
            IntegerMathTerm::Subtract(_, _) => writer.u8(4),
            IntegerMathTerm::Multiply(_, _) => writer.u8(5),
            IntegerMathTerm::ShiftLeft { .. } => writer.u8(6),
        }
        Ok(())
    }

    fn read_row(
        reader: &mut Reader<'_>,
        children: &mut TableChildren<'_, Self::Term>,
    ) -> Result<Self::Term, CodecError> {
        let mut pair = |reader: &mut Reader<'_>| {
            let mut operands = children.take(reader, 2)?.into_iter();
            match (operands.next(), operands.next()) {
                (Some(left), Some(right)) => Ok((Box::new(left), Box::new(right))),
                _ => Err(CodecError::NonCanonicalTermTable(Self::LABEL)),
            }
        };
        Ok(match reader.u8()? {
            7 => IntegerMathTerm::Embed(Box::new(decode_scalar_term(reader)?)),
            1 => IntegerMathTerm::IntegerLiteral(
                IntegerMathLiteral::new(
                    match reader.u8()? {
                        0 => false,
                        1 => true,
                        tag => return Err(CodecError::InvalidTag("Boolean", tag)),
                    },
                    u128::from_le_bytes(reader.array()?),
                )
                .map_err(CodecError::MalformedProposition)?,
            ),
            2 => IntegerMathTerm::MathValue {
                source_type: decode_integer_type(reader)?,
                value: reader.id("ValueId")?,
            },
            3 => {
                let (left, right) = pair(reader)?;
                IntegerMathTerm::Add(left, right)
            }
            4 => {
                let (left, right) = pair(reader)?;
                IntegerMathTerm::Subtract(left, right)
            }
            5 => {
                let (left, right) = pair(reader)?;
                IntegerMathTerm::Multiply(left, right)
            }
            6 => {
                let (value, count) = pair(reader)?;
                IntegerMathTerm::ShiftLeft { value, count }
            }
            tag => return Err(CodecError::InvalidTag("IntegerMathTerm", tag)),
        })
    }
}
