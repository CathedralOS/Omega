//! Canonical wire form for proof-only unbounded integer syntax.

use semantic_vocabulary::{IntegerMathLiteral, IntegerMathTerm};

use super::scalar_wire::{decode_integer_type, encode_integer_type};
use super::wire::{Reader, Writer};
use super::{CodecError, MAX_SCALAR_TERM_DEPTH};

pub(super) fn encode_integer_math_term(
    writer: &mut Writer,
    term: &IntegerMathTerm,
    depth: usize,
) -> Result<(), CodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(CodecError::ScalarTermNestingTooDeep);
    }
    match term {
        IntegerMathTerm::IntegerLiteral(literal) => {
            writer.u8(1);
            writer.boolean(literal.negative());
            writer.bytes(&literal.magnitude().to_le_bytes());
        }
        IntegerMathTerm::MathValue { source_type, value } => {
            writer.u8(2);
            encode_integer_type(writer, *source_type);
            writer.id(*value);
        }
        IntegerMathTerm::Add(left, right)
        | IntegerMathTerm::Subtract(left, right)
        | IntegerMathTerm::Multiply(left, right) => {
            writer.u8(match term {
                IntegerMathTerm::Add(_, _) => 3,
                IntegerMathTerm::Subtract(_, _) => 4,
                IntegerMathTerm::Multiply(_, _) => 5,
                _ => unreachable!(),
            });
            encode_integer_math_term(writer, left, depth + 1)?;
            encode_integer_math_term(writer, right, depth + 1)?;
        }
        IntegerMathTerm::ShiftLeft { value, count } => {
            writer.u8(6);
            encode_integer_math_term(writer, value, depth + 1)?;
            encode_integer_math_term(writer, count, depth + 1)?;
        }
    }
    Ok(())
}

pub(super) fn decode_integer_math_term(
    reader: &mut Reader<'_>,
    depth: usize,
) -> Result<IntegerMathTerm, CodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(CodecError::ScalarTermNestingTooDeep);
    }
    Ok(match reader.u8()? {
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
        3 => IntegerMathTerm::Add(
            Box::new(decode_integer_math_term(reader, depth + 1)?),
            Box::new(decode_integer_math_term(reader, depth + 1)?),
        ),
        4 => IntegerMathTerm::Subtract(
            Box::new(decode_integer_math_term(reader, depth + 1)?),
            Box::new(decode_integer_math_term(reader, depth + 1)?),
        ),
        5 => IntegerMathTerm::Multiply(
            Box::new(decode_integer_math_term(reader, depth + 1)?),
            Box::new(decode_integer_math_term(reader, depth + 1)?),
        ),
        6 => IntegerMathTerm::ShiftLeft {
            value: Box::new(decode_integer_math_term(reader, depth + 1)?),
            count: Box::new(decode_integer_math_term(reader, depth + 1)?),
        },
        tag => return Err(CodecError::InvalidTag("IntegerMathTerm", tag)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ValueId};

    #[test]
    fn mathematical_integer_term_round_trips_exactly() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let term = IntegerMathTerm::Add(
            Box::new(IntegerMathTerm::MathValue {
                source_type: integer_type,
                value: ValueId::new(7).expect("value"),
            }),
            Box::new(IntegerMathTerm::Multiply(
                Box::new(IntegerMathTerm::literal(IntegerValue::Signed(-3))),
                Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(9))),
            )),
        );
        let mut writer = Writer::default();
        encode_integer_math_term(&mut writer, &term, 0).expect("encode");
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_integer_math_term(&mut reader, 0), Ok(term));
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn negative_zero_is_not_a_second_literal_encoding() {
        let mut bytes = vec![1, 1];
        bytes.extend_from_slice(&0_u128.to_le_bytes());
        assert_eq!(
            decode_integer_math_term(&mut Reader::new(&bytes), 0),
            Err(CodecError::MalformedProposition(
                semantic_vocabulary::PropositionError::NegativeZeroIntegerMathLiteral,
            ))
        );
    }
}
