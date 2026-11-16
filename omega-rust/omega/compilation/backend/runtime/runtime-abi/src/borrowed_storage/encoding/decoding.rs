use super::*;
use semantic_vocabulary::{IeeeFloatFormat, IntegerSign, IntegerType, StructuralFieldId};

pub(super) fn decode(
    bytes: &[u8],
) -> Result<NativeBorrowedStorageLayouts, BorrowedStorageEncodingError> {
    let mut cursor = Cursor { bytes };
    let result = (|| {
        if cursor.take(33)? != b"omega.native-borrowed-storage.v1\0" {
            return Err("unknown layout encoding");
        }
        let layouts = cursor.list(38, decode_layout)?;
        if !cursor.bytes.is_empty() {
            return Err("trailing layout bytes");
        }
        Ok(layouts)
    })();
    let layouts = result.map_err(|reason| BorrowedStorageEncodingError { layout: 0, reason })?;
    let sealed = NativeBorrowedStorageLayouts::seal(layouts)?;
    if sealed.canonical_bytes() != bytes {
        return Err(BorrowedStorageEncodingError {
            layout: 0,
            reason: "noncanonical layout encoding",
        });
    }
    Ok(sealed)
}

struct Cursor<'a> {
    bytes: &'a [u8],
}

impl<'a> Cursor<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], &'static str> {
        let result = self
            .bytes
            .get(..length)
            .ok_or("truncated layout encoding")?;
        self.bytes = &self.bytes[length..];
        Ok(result)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], &'static str> {
        self.take(N)?
            .try_into()
            .map_err(|_| "truncated layout encoding")
    }

    fn byte(&mut self) -> Result<u8, &'static str> {
        Ok(self.array::<1>()?[0])
    }

    fn u64(&mut self) -> Result<u64, &'static str> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, &'static str> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn length(&mut self) -> Result<usize, &'static str> {
        usize::try_from(self.u64()?).map_err(|_| "layout length exceeds address space")
    }

    fn id<T>(&mut self, make: impl FnOnce(u64) -> Option<T>) -> Result<T, &'static str> {
        make(self.u64()?).ok_or("invalid semantic identity")
    }

    fn list<T>(
        &mut self,
        minimum_row_bytes: usize,
        mut read: impl FnMut(&mut Self) -> Result<T, &'static str>,
    ) -> Result<Vec<T>, &'static str> {
        let count = self.length()?;
        let minimum_bytes = count
            .checked_mul(minimum_row_bytes)
            .ok_or("layout row byte count overflow")?;
        if minimum_bytes > self.bytes.len() {
            return Err("layout row count exceeds payload");
        }
        let mut rows = Vec::new();
        rows.try_reserve_exact(count)
            .map_err(|_| "layout row allocation exceeds limits")?;
        for _ in 0..count {
            rows.push(read(self)?);
        }
        Ok(rows)
    }

    fn path(&mut self) -> Result<Vec<CanonicalStructuralPathSegment>, &'static str> {
        use semantic_vocabulary::structural_path_encoding::SegmentTag;
        self.list(1, |cursor| match SegmentTag::from_byte(cursor.byte()?) {
            Some(SegmentTag::Field) => Ok(CanonicalStructuralPathSegment::Field(
                cursor.id(StructuralFieldId::new)?,
            )),
            Some(SegmentTag::FixedIndex) => {
                Ok(CanonicalStructuralPathSegment::FixedIndex(cursor.u64()?))
            }
            Some(SegmentTag::Case) => Ok(CanonicalStructuralPathSegment::Case(
                cursor.id(StructuralCaseId::new)?,
            )),
            Some(SegmentTag::Referent) => Ok(CanonicalStructuralPathSegment::Referent),
            _ => Err("invalid borrowed storage path segment"),
        })
    }

    fn repetitions(&mut self) -> Result<Vec<NativeStorageRepetition>, &'static str> {
        self.list(20, |cursor| {
            Ok(NativeStorageRepetition {
                path_position: cursor.length()?,
                length: cursor.u64()?,
                byte_stride: cursor.u32()?,
            })
        })
    }

    fn scalar(&mut self) -> Result<ScalarType, &'static str> {
        match self.byte()? {
            1 => Ok(ScalarType::Boolean),
            2 => {
                let carrier = self.byte()?;
                let sign = match self.byte()? {
                    1 => IntegerSign::Signed,
                    2 => IntegerSign::Unsigned,
                    _ => return Err("unknown integer sign"),
                };
                let bits = u16::from_le_bytes(self.array()?);
                let integer = match carrier {
                    1 => IntegerType::new(sign, bits),
                    2 if sign == IntegerSign::Unsigned => IntegerType::address(bits),
                    _ => return Err("unknown integer carrier"),
                }
                .map_err(|_| "invalid integer width")?;
                semantic_vocabulary::ClosedIntegerType::try_from(integer)
                    .map_err(|_| "unclosed scalar type")?;
                Ok(ScalarType::Integer(integer))
            }
            3 => match self.byte()? {
                1 => Ok(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)),
                2 => Ok(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)),
                _ => Err("unknown float format"),
            },
            _ => Err("unknown scalar type"),
        }
    }
}

fn decode_layout(cursor: &mut Cursor<'_>) -> Result<NativeBorrowedStorageLayout, &'static str> {
    let structural_type = cursor.id(StructuralTypeId::new)?;
    let byte_size = cursor.u32()?;
    let alignment = u16::from_le_bytes(cursor.array()?);
    let fields = cursor.list(22, |cursor| {
        Ok((
            cursor.path()?,
            cursor.repetitions()?,
            cursor.u32()?,
            cursor.scalar()?,
            cursor.scalar()?,
        ))
    })?;
    let discriminators = cursor.list(36, |cursor| {
        Ok((
            cursor.path()?,
            cursor.repetitions()?,
            cursor.id(StructuralTypeId::new)?,
            cursor.u32()?,
            cursor.list(12, |cursor| {
                Ok(NativeStorageCaseTag {
                    case: cursor.id(StructuralCaseId::new)?,
                    native_tag: i32::from_le_bytes(cursor.array()?),
                })
            })?,
        ))
    })?;
    let regions = cursor.list(24, |cursor| {
        Ok((
            cursor.path()?,
            cursor.repetitions()?,
            cursor.u32()?,
            cursor.u32()?,
        ))
    })?;
    Ok(NativeBorrowedStorageLayout::new(
        structural_type,
        byte_size,
        alignment,
        fields,
        discriminators,
        regions,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_budgets_refuse_overflow_and_short_payload_before_reading() {
        let overflowing = u64::MAX.to_le_bytes();
        assert_eq!(
            Cursor {
                bytes: &overflowing
            }
            .list::<u8>(38, |_| panic!("row must not be read")),
            Err("layout row byte count overflow")
        );
        let mut short = 2_u64.to_le_bytes().to_vec();
        short.extend_from_slice(&[0; 39]);
        assert_eq!(
            Cursor { bytes: &short }.list::<u8>(20, |_| panic!("row must not be read")),
            Err("layout row count exceeds payload")
        );
        let mut exact = 2_u64.to_le_bytes().to_vec();
        exact.extend_from_slice(&[1, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(
            Cursor { bytes: &exact }.list(4, Cursor::u32).unwrap(),
            [1, 2]
        );
    }
}
