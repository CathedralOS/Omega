use crate::{
    CodecError,
    terms::table::{TableChildren, TermEncoding, decode_term_table, encode_term_table},
    wire::{Reader, Writer, decode_counted},
};
use semantic_vocabulary::{ContentProjectionExpression, ContentProjectionScalar};

pub fn encode_content_projection_expression(
    writer: &mut Writer,
    capacity: &ContentProjectionExpression,
) -> Result<(), CodecError> {
    match capacity {
        ContentProjectionExpression::IntervalSet(members) => {
            writer.u8(1);
            writer.len("program-local interval members", members.len())?;
            for (start, end) in members {
                encode_capacity_scalar(writer, start)?;
                encode_capacity_scalar(writer, end)?;
            }
        }
        ContentProjectionExpression::CountedQuantity(magnitude) => {
            writer.u8(2);
            encode_capacity_scalar(writer, magnitude)?;
        }
    }
    Ok(())
}

/// A capacity scalar is one postorder table (`crate::terms::table`).
fn encode_capacity_scalar(
    writer: &mut Writer,
    scalar: &ContentProjectionScalar,
) -> Result<(), CodecError> {
    encode_term_table::<CapacityTable>(writer, scalar)
}

pub fn decode_capacity_scalar(
    reader: &mut Reader<'_>,
) -> Result<ContentProjectionScalar, CodecError> {
    decode_term_table::<CapacityTable>(reader)
}

struct CapacityTable;

impl TermEncoding for CapacityTable {
    type Term = ContentProjectionScalar;
    const LABEL: &'static str = "capacity scalar";

    fn children<'term>(term: &'term Self::Term, children: &mut Vec<&'term Self::Term>) {
        match term {
            ContentProjectionScalar::Successor(inner) => children.push(inner),
            ContentProjectionScalar::Add(left, right)
            | ContentProjectionScalar::Subtract(left, right)
            | ContentProjectionScalar::Multiply(left, right) => {
                children.extend([left.as_ref(), right.as_ref()])
            }
            ContentProjectionScalar::SubjectField(_)
            | ContentProjectionScalar::RuntimeScalarEmbedding(_)
            | ContentProjectionScalar::Natural(_) => {}
        }
    }

    fn write_row(term: &Self::Term, writer: &mut Writer) -> Result<(), CodecError> {
        match term {
            ContentProjectionScalar::SubjectField(path) => {
                writer.u8(1);
                writer.strings("program-local capacity field path", path)?;
            }
            ContentProjectionScalar::RuntimeScalarEmbedding(path) => {
                writer.u8(2);
                writer.strings("program-local capacity field path", path)?;
            }
            ContentProjectionScalar::Natural(value) => {
                writer.u8(3);
                writer.string("program-local natural", value)?;
            }
            ContentProjectionScalar::Successor(_) => writer.u8(4),
            ContentProjectionScalar::Add(_, _) => writer.u8(5),
            ContentProjectionScalar::Subtract(_, _) => writer.u8(6),
            ContentProjectionScalar::Multiply(_, _) => writer.u8(7),
        }
        Ok(())
    }

    fn read_row(
        reader: &mut Reader<'_>,
        children: &mut TableChildren<'_, Self::Term>,
    ) -> Result<Self::Term, CodecError> {
        let tag = reader.u8()?;
        let arity = match tag {
            1..=3 => 0,
            4 => 1,
            5..=7 => 2,
            tag => return Err(CodecError::InvalidTag("ContentProjectionScalar", tag)),
        };
        let leaf = match tag {
            1 => Some(ContentProjectionScalar::SubjectField(
                reader.strings("program-local capacity field path")?,
            )),
            2 => Some(ContentProjectionScalar::RuntimeScalarEmbedding(
                reader.strings("program-local capacity field path")?,
            )),
            3 => Some(ContentProjectionScalar::Natural(
                reader.string("program-local natural")?,
            )),
            _ => None,
        };
        if let Some(leaf) = leaf {
            return Ok(leaf);
        }
        let mut operands = children.take(reader, arity)?.into_iter().map(Box::new);
        let mut operand = || {
            operands
                .next()
                .ok_or(CodecError::NonCanonicalTermTable(Self::LABEL))
        };
        Ok(match tag {
            4 => ContentProjectionScalar::Successor(operand()?),
            5 => ContentProjectionScalar::Add(operand()?, operand()?),
            6 => ContentProjectionScalar::Subtract(operand()?, operand()?),
            _ => ContentProjectionScalar::Multiply(operand()?, operand()?),
        })
    }
}

pub fn decode_content_projection_expression(
    reader: &mut Reader<'_>,
) -> Result<ContentProjectionExpression, CodecError> {
    match reader.u8()? {
        1 => Ok(ContentProjectionExpression::IntervalSet(decode_counted(
            reader,
            |reader| {
                Ok((
                    decode_capacity_scalar(reader)?,
                    decode_capacity_scalar(reader)?,
                ))
            },
        )?)),
        2 => Ok(ContentProjectionExpression::CountedQuantity(
            decode_capacity_scalar(reader)?,
        )),
        tag => Err(CodecError::InvalidTag("ContentProjectionExpression", tag)),
    }
}
