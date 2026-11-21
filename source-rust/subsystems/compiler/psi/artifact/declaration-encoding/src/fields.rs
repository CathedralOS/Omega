//! A record lists its field markers in storage order.
//! Each marker supplies both the writer and the reader for that field.

use crate::{
    CodecError,
    wire::{
        Reader, Writer, bounded_count, decode_bytes, decode_ids, decode_optional_id,
        encode_optional_id,
    },
};
use arena::NumericId;
use semantic_vocabulary::{ContentProjectionIdentity, StructuralFieldId};
use terminal_psi::StructuralContentProjection;
pub trait Field<T> {
    fn write(writer: &mut Writer, value: &T) -> Result<(), CodecError>;
    fn read(reader: &mut Reader<'_>) -> Result<T, CodecError>;
}

pub struct ResultField<Success, Failure>(std::marker::PhantomData<(Success, Failure)>);
impl<Success, Failure, SuccessField: Field<Success>, FailureField: Field<Failure>>
    Field<Result<Success, Failure>> for ResultField<SuccessField, FailureField>
{
    fn write(writer: &mut Writer, value: &Result<Success, Failure>) -> Result<(), CodecError> {
        writer.boolean(value.is_ok());
        match value {
            Ok(value) => SuccessField::write(writer, value),
            Err(reason) => FailureField::write(writer, reason),
        }
    }

    fn read(reader: &mut Reader<'_>) -> Result<Result<Success, Failure>, CodecError> {
        Ok(if reader.boolean()? {
            Ok(SuccessField::read(reader)?)
        } else {
            Err(FailureField::read(reader)?)
        })
    }
}

fn label<T>() -> &'static str {
    let name = std::any::type_name::<T>();
    name.rsplit("::").next().unwrap_or(name)
}

pub struct Id;
impl<T: NumericId> Field<T> for Id {
    fn write(writer: &mut Writer, value: &T) -> Result<(), CodecError> {
        writer.id(*value);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<T, CodecError> {
        reader.id(label::<T>())
    }
}

pub struct OptionalId;
impl<T: NumericId> Field<Option<T>> for OptionalId {
    fn write(writer: &mut Writer, value: &Option<T>) -> Result<(), CodecError> {
        encode_optional_id(writer, *value);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Option<T>, CodecError> {
        decode_optional_id(reader, label::<T>())
    }
}

pub struct Ids;
impl<T: NumericId> Field<Vec<T>> for Ids {
    fn write(writer: &mut Writer, value: &Vec<T>) -> Result<(), CodecError> {
        writer.ids("operation identities", value)
    }
    fn read(reader: &mut Reader<'_>) -> Result<Vec<T>, CodecError> {
        decode_ids(reader, label::<T>())
    }
}

pub struct OptionalField;
impl Field<Option<StructuralFieldId>> for OptionalField {
    fn write(writer: &mut Writer, value: &Option<StructuralFieldId>) -> Result<(), CodecError> {
        writer.u64(value.map_or(0, StructuralFieldId::get));
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Option<StructuralFieldId>, CodecError> {
        Ok(StructuralFieldId::new(reader.u64()?))
    }
}

macro_rules! integer_fields {
    ($($marker:ident: $ty:ty => $write:ident / $read:ident),* $(,)?) => {$(
        pub struct $marker;
        impl Field<$ty> for $marker {
            fn write(writer: &mut Writer, value: &$ty) -> Result<(), CodecError> {
                writer.$write(*value);
                Ok(())
            }
            fn read(reader: &mut Reader<'_>) -> Result<$ty, CodecError> {
                reader.$read()
            }
        }
    )*};
}
integer_fields! {
    U8: u8 => u8 / u8,
    U16: u16 => u16 / u16,
    U32: u32 => u32 / u32,
    U64: u64 => u64 / u64,
    Flag: bool => boolean / boolean,
}

pub struct I128;
impl Field<i128> for I128 {
    fn write(writer: &mut Writer, value: &i128) -> Result<(), CodecError> {
        writer.bytes(&value.to_le_bytes());
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<i128, CodecError> {
        Ok(i128::from_le_bytes(reader.array()?))
    }
}

pub struct Bytes;
impl Field<Vec<u8>> for Bytes {
    fn write(writer: &mut Writer, value: &Vec<u8>) -> Result<(), CodecError> {
        writer.len("byte-sequence literal bytes", value.len())?;
        writer.bytes(value);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Vec<u8>, CodecError> {
        decode_bytes(reader)
    }
}

pub struct Index;
impl Field<terminal_psi::TerminalIdentityIndex> for Index {
    fn write(
        writer: &mut Writer,
        value: &terminal_psi::TerminalIdentityIndex,
    ) -> Result<(), CodecError> {
        writer.u32(value.0);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<terminal_psi::TerminalIdentityIndex, CodecError> {
        Ok(terminal_psi::TerminalIdentityIndex(reader.u32()?))
    }
}

pub struct Text;
impl Field<String> for Text {
    fn write(writer: &mut Writer, value: &String) -> Result<(), CodecError> {
        writer.string("wire text", value)
    }
    fn read(reader: &mut Reader<'_>) -> Result<String, CodecError> {
        reader.string("wire text")
    }
}

// A pair is just its two existing field encodings in order. Reuse those
// codecs for tuple-valued data rather than defining another domain marker.
impl<Left, Right, LeftField: Field<Left>, RightField: Field<Right>> Field<(Left, Right)>
    for (LeftField, RightField)
{
    fn write(writer: &mut Writer, value: &(Left, Right)) -> Result<(), CodecError> {
        LeftField::write(writer, &value.0)?;
        RightField::write(writer, &value.1)
    }

    fn read(reader: &mut Reader<'_>) -> Result<(Left, Right), CodecError> {
        Ok((LeftField::read(reader)?, RightField::read(reader)?))
    }
}

// Catalogs with known row prefixes can bound allocation before reading a row.
// The default preserves existing schemas, and the minimum never changes bytes.
pub struct List<K, const MIN_ELEMENT_BYTES: usize = 1>(std::marker::PhantomData<K>);
impl<T, K: Field<T>, const MIN_ELEMENT_BYTES: usize> Field<Vec<T>> for List<K, MIN_ELEMENT_BYTES> {
    fn write(writer: &mut Writer, value: &Vec<T>) -> Result<(), CodecError> {
        writer.len("wire roster", value.len())?;
        for element in value {
            K::write(writer, element)?;
        }
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Vec<T>, CodecError> {
        let count = bounded_count(reader, MIN_ELEMENT_BYTES)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| CodecError::CollectionTooLong("wire roster"))?;
        for _ in 0..count {
            values.push(K::read(reader)?);
        }
        Ok(values)
    }
}

pub struct Optional<K>(std::marker::PhantomData<K>);
impl<T, K: Field<T>> Field<Option<T>> for Optional<K> {
    fn write(writer: &mut Writer, value: &Option<T>) -> Result<(), CodecError> {
        writer.boolean(value.is_some());
        if let Some(value) = value {
            K::write(writer, value)?;
        }
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Option<T>, CodecError> {
        Ok(if reader.boolean()? {
            Some(K::read(reader)?)
        } else {
            None
        })
    }
}

#[macro_export]
macro_rules! wire_record {
    ($($marker:ident: $ty:ident { $($field:ident: $kind:ty),* $(,)? })*) => {$(
        pub struct $marker;
        impl $crate::fields::Field<$ty> for $marker {
            fn write(
                writer: &mut $crate::wire::Writer,
                value: &$ty,
            ) -> Result<(), $crate::CodecError> {
                let $ty { $($field),* } = value;
                $(<$kind as $crate::fields::Field<_>>::write(
                    writer, $field,
                )?;)*
                Ok(())
            }
            fn read(
                reader: &mut $crate::wire::Reader<'_>,
            ) -> Result<$ty, $crate::CodecError> {
                Ok($ty {
                    $($field: <$kind as $crate::fields::Field<_>>::read(
                        reader,
                    )?),*
                })
            }
        }
    )*};
}
pub use crate::wire_record;

pub struct Strings;
impl Field<Vec<String>> for Strings {
    fn write(writer: &mut Writer, value: &Vec<String>) -> Result<(), CodecError> {
        writer.strings("wire texts", value)
    }
    fn read(reader: &mut Reader<'_>) -> Result<Vec<String>, CodecError> {
        reader.strings("wire texts")
    }
}

#[macro_export]
macro_rules! wire_enum {
    ($($marker:ident: $ty:ident, $name:literal {
        $($tag:literal => $variant:ident
            $(( $($tuple_field:ident: $tuple_kind:ty),+ $(,)? ))?
            $({ $($field:ident: $kind:ty),* $(,)? })?
        ),* $(,)?
        $(; refuse $($refused:pat => $refusal:expr),+ $(,)?)?
    })*) => {$(
        pub struct $marker;
        impl $crate::fields::Field<$ty> for $marker {
            fn write(
                writer: &mut $crate::wire::Writer,
                value: &$ty,
            ) -> Result<(), $crate::CodecError> {
                match value {
                    $($ty::$variant $(($($tuple_field),+))? $({ $($field),* })? => {
                        writer.u8($tag);
                        $($(<$tuple_kind as $crate::fields::Field<_>>::write(writer, $tuple_field)?;)+)?
                        $($(<$kind as $crate::fields::Field<_>>::write(writer, $field)?;)*)?
                    })*
                    $($($refused => return Err($refusal),)+)?
                }
                Ok(())
            }
            fn read(
                reader: &mut $crate::wire::Reader<'_>,
            ) -> Result<$ty, $crate::CodecError> {
                Ok(match reader.u8()? {
                    $($tag => $ty::$variant
                        $(($(<$tuple_kind as $crate::fields::Field<_>>::read(reader)?),+))?
                        $({ $($field: <$kind as $crate::fields::Field<_>>::read(reader)?),* })?,
                    )*
                    tag => return Err($crate::CodecError::InvalidTag(
                        $name, tag,
                    )),
                })
            }
        }
    )*};
}
pub use crate::wire_enum;

#[macro_export]
macro_rules! u32_newtypes {
    ($($marker:ident: $ty:path;)*) => {$(
        pub struct $marker;
        impl Field<$ty> for $marker {
            fn write(writer: &mut Writer, value: &$ty) -> Result<(), CodecError> {
                writer.u32(value.0);
                Ok(())
            }
            fn read(reader: &mut Reader<'_>) -> Result<$ty, CodecError> {
                Ok($ty(reader.u32()?))
            }
        }
    )*};
}
pub use crate::u32_newtypes;

pub struct Digest;
impl Field<[u8; 32]> for Digest {
    fn write(writer: &mut Writer, value: &[u8; 32]) -> Result<(), CodecError> {
        writer.bytes(value);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<[u8; 32], CodecError> {
        reader.array()
    }
}

pub struct Nominal;
impl Field<terminal_psi::NominalDeclarationIdentity> for Nominal {
    fn write(
        writer: &mut Writer,
        value: &terminal_psi::NominalDeclarationIdentity,
    ) -> Result<(), CodecError> {
        writer.u32(value.identity().0);
        Ok(())
    }
    fn read(
        reader: &mut Reader<'_>,
    ) -> Result<terminal_psi::NominalDeclarationIdentity, CodecError> {
        Ok(terminal_psi::NominalDeclarationIdentity::new(
            terminal_psi::TerminalIdentityIndex(reader.u32()?),
        ))
    }
}

pub struct Proposition;
impl Field<semantic_vocabulary::Proposition> for Proposition {
    fn write(
        writer: &mut Writer,
        value: &semantic_vocabulary::Proposition,
    ) -> Result<(), CodecError> {
        crate::terms::proposition::encode_proposition(writer, value)
    }
    fn read(reader: &mut Reader<'_>) -> Result<semantic_vocabulary::Proposition, CodecError> {
        crate::terms::proposition::decode_proposition(reader)
    }
}

pub struct ContentExpression;
impl Field<semantic_vocabulary::ContentProjectionExpression> for ContentExpression {
    fn write(
        writer: &mut Writer,
        value: &semantic_vocabulary::ContentProjectionExpression,
    ) -> Result<(), CodecError> {
        crate::content_projections::encode_content_projection_expression(writer, value)
    }
    fn read(
        reader: &mut Reader<'_>,
    ) -> Result<semantic_vocabulary::ContentProjectionExpression, CodecError> {
        crate::content_projections::decode_content_projection_expression(reader)
    }
}

macro_rules! delegated_fields {
    ($($marker:ident: $ty:ty => $encode:path, $decode:path;)*) => {$(
        pub struct $marker;
        impl Field<$ty> for $marker {
            fn write(writer: &mut Writer, value: &$ty) -> Result<(), CodecError> {
                $encode(writer, value)
            }
            fn read(reader: &mut Reader<'_>) -> Result<$ty, CodecError> {
                $decode(reader)
            }
        }
    )*};
}

delegated_fields! {
    ContentAlgebra: semantic_vocabulary::ContentAlgebra
        => crate::terms::content::encode_content_algebra, crate::terms::content::decode_content_algebra;
}

wire_record! {
    ProjectionIdentity: ContentProjectionIdentity { domain: Id, projection_report_fingerprint: U64 }
    ContentProjection: StructuralContentProjection {
        identity: ProjectionIdentity,
        algebra: ContentAlgebra,
        expression: ContentExpression,
    }
}

pub const DECLARED_LIFETIME_FAMILY: bool = true;

pub struct Path<const LIFETIME_FAMILY: bool = false>;
impl<const LIFETIME_FAMILY: bool> Field<Vec<terminal_psi::StructuralPathSegment>>
    for Path<LIFETIME_FAMILY>
{
    fn write(
        writer: &mut Writer,
        value: &Vec<terminal_psi::StructuralPathSegment>,
    ) -> Result<(), CodecError> {
        if LIFETIME_FAMILY {
            crate::places::encode_declared_lifetime_path(writer, value)
        } else {
            crate::places::encode_structural_path(writer, "operation path", value)
        }
    }
    fn read(
        reader: &mut Reader<'_>,
    ) -> Result<Vec<terminal_psi::StructuralPathSegment>, CodecError> {
        if LIFETIME_FAMILY {
            crate::places::decode_declared_lifetime_path(reader)
        } else {
            crate::places::decode_structural_path(reader)
        }
    }
}
