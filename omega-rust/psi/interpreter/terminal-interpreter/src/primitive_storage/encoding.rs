//! Source-selected codecs operate on the retained endpoint. Byte-array reads
//! and writes touch exactly the selected footprint; scalar cells retain their
//! original semantic representation and must satisfy the same stored range.
//! Captured region projection retains extent and scalar codec together. A
//! projected logical type cannot recover the ancestor's authored field storage.

use super::{PrimitiveLoan, PrimitiveStorage, PrimitiveStorageEncoding, PrimitiveStorageEndpoint};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::scalar_operations::terminal_scalar_belongs_to_type;
use crate::values::TerminalScalarValue;
use semantic_vocabulary::{
    IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, PlaceId, ScalarType,
};

#[derive(PartialEq, Eq)]
struct ReferenceByteRegionProjection {
    region: crate::reference::ReferenceByteRegion,
    scalar: Option<(ScalarType, ScalarType)>,
}

/// A replacement must fit the retained codec before any boundary publication.
pub(crate) fn scalar_fits_storage(stored: ScalarType, scalar: TerminalScalarValue) -> bool {
    terminal_semantics::reference_storage_view_is_compatible(stored, scalar.scalar_type())
        && encode(stored, scalar).is_ok()
}

impl TerminalExecution<'_> {
    /// Resolve scalar descendants of a retained aggregate region. Borrowed
    /// formals retain the same opaque view, so this works after forwarding as
    /// well as through the original reference carrier.
    pub(crate) fn reference_byte_region_access(
        &self,
        backing_view: &crate::values::TerminalStructuralValue,
        view: &crate::values::TerminalStructuralValue,
    ) -> Result<Option<(PrimitiveStorage, ScalarType)>, TerminalInterpretError> {
        if view.opaque_identity != backing_view.opaque_identity
            || !view.path.starts_with(&backing_view.path)
        {
            return Err(malformed());
        }
        let Some(projected) = self.project_reference_byte_region(backing_view, &view.path)? else {
            return Ok(None);
        };
        let (logical_scalar, stored_scalar) =
            projected
                .scalar
                .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                    "captured byte-region projection has no contiguous scalar geometry",
                ))?;
        Ok(Some((
            PrimitiveStorage {
                endpoint: projected.region.endpoint,
                encoding: Some(PrimitiveStorageEncoding {
                    source: stored_scalar,
                    view: stored_scalar,
                }),
            },
            logical_scalar,
        )))
    }

    /// Project a captured region under its declared type and retain its endpoint.
    /// Formation and scalar accesses share this lookup; forwarding never
    /// reconstructs bytes from the original formation expression.
    pub(crate) fn reference_byte_region(
        &self,
        view: &crate::values::TerminalStructuralValue,
    ) -> Result<Option<crate::reference::ReferenceByteRegion>, TerminalInterpretError> {
        Ok(self
            .project_reference_byte_region(view, &view.path)?
            .map(|projected| projected.region))
    }

    /// Keep the codec chosen along the captured ancestor's complete path.
    /// Starting again at a projected i64 would lose its IntegerAt byte width.
    /// Formation and access share this endpoint, extent and codec selection.
    fn project_reference_byte_region(
        &self,
        view: &crate::values::TerminalStructuralValue,
        projected_path: &[terminal_psi::StructuralPathSegment],
    ) -> Result<Option<ReferenceByteRegionProjection>, TerminalInterpretError> {
        let mut selected = None;
        for captured in self.reference_referents.values() {
            let Some(region) = &captured.byte_region else {
                continue;
            };
            if view.opaque_identity != captured.view.opaque_identity {
                continue;
            }
            let Some(path) = view.path.strip_prefix(captured.view.path.as_slice()) else {
                continue;
            };
            if crate::custody::resolve_structural_path_type(
                &self.program.structural_types(),
                captured.view.structural_type,
                path,
            )
            .ok()
                != Some(view.structural_type)
            {
                continue;
            }
            let path = projected_path
                .strip_prefix(captured.view.path.as_slice())
                .ok_or_else(malformed)?;
            let geometry = terminal_semantics::reference_storage_region(
                self.program.structural_types().values(),
                captured.view.structural_type,
                path,
            )
            .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                "captured byte region has no contiguous geometry",
            ))?;
            let offset = usize::try_from(geometry.byte_offset).map_err(|_| malformed())?;
            let length = usize::try_from(geometry.byte_length).map_err(|_| malformed())?;
            if offset
                .checked_add(length)
                .is_none_or(|end| end > region.byte_length)
            {
                return Err(malformed());
            }
            let projected = ReferenceByteRegionProjection {
                region: crate::reference::ReferenceByteRegion {
                    endpoint: shifted_byte_endpoint(&region.endpoint, offset)?,
                    byte_length: length,
                },
                // The endpoint already incorporates the ancestor-relative
                // offset. Equal endpoints compare codecs, not path origins.
                scalar: geometry
                    .scalar
                    .map(|scalar| (scalar.logical_scalar, scalar.stored_scalar)),
            };
            if selected
                .as_ref()
                .is_some_and(|previous| previous != &projected)
            {
                return Err(malformed());
            }
            selected = Some(projected);
        }
        Ok(selected)
    }

    pub(crate) fn apply_reference_storage_view(
        &self,
        loan: &mut PrimitiveLoan,
        byte_offset: usize,
        stored_scalar: ScalarType,
        logical_scalar: ScalarType,
    ) -> Result<(), TerminalInterpretError> {
        let width = usize::from(
            terminal_semantics::reference_storage_byte_width(stored_scalar)
                .ok_or_else(malformed)?,
        );
        if !terminal_semantics::reference_storage_view_is_compatible(stored_scalar, logical_scalar)
        {
            return Err(malformed());
        }
        if let Some(captured) = &mut loan.storage.encoding {
            if byte_offset != 0
                || terminal_semantics::reference_storage_byte_width(captured.source)
                    != terminal_semantics::reference_storage_byte_width(stored_scalar)
            {
                return Err(malformed());
            }
            // Re-view the already captured bytes; the original cell codec is
            // still needed to encode its wider logical carrier correctly.
            captured.view = stored_scalar;
        } else {
            // Typed cells retain their original value carrier: signed -1 is
            // encoded as signed bits before an unsigned view observes them.
            // A byte region instead assembles the requested stored width.
            let source_scalar = match loan.storage.endpoint {
                PrimitiveStorageEndpoint::Scalar(_) | PrimitiveStorageEndpoint::ScalarField(_) => {
                    loan.scalar_type
                }
                _ => stored_scalar,
            };
            self.position_reference_storage(loan, byte_offset, width)?;
            loan.storage.encoding = Some(PrimitiveStorageEncoding {
                source: source_scalar,
                view: stored_scalar,
            });
        }
        loan.scalar_type = logical_scalar;
        Ok(())
    }

    /// Capture one complete byte region against its original endpoint once.
    pub(crate) fn position_reference_storage(
        &self,
        loan: &mut PrimitiveLoan,
        byte_offset: usize,
        width: usize,
    ) -> Result<(), TerminalInterpretError> {
        if loan.storage.encoding.is_some() {
            return Err(malformed());
        }
        match &mut loan.storage.endpoint {
            PrimitiveStorageEndpoint::ArrayElement { array, index } => {
                if loan.scalar_type != byte_type()? {
                    return Err(malformed());
                }
                let start = index.checked_add(byte_offset).ok_or_else(malformed)?;
                let end = start.checked_add(width).ok_or_else(malformed)?;
                if end > self.stored_byte_array_length(array)? {
                    return Err(malformed());
                }
                *index = start;
            }
            PrimitiveStorageEndpoint::OwnedArrayElement { home, index } => {
                if loan.scalar_type != byte_type()? {
                    return Err(malformed());
                }
                let start = index.checked_add(byte_offset).ok_or_else(malformed)?;
                let end = start.checked_add(width).ok_or_else(malformed)?;
                let (machine, arrays) = if home.depth == self.call_stack.len() {
                    (self.current_machine, &self.scalar_array_values)
                } else {
                    let frame = self.call_stack.get(home.depth).ok_or_else(malformed)?;
                    (frame.current_machine, &frame.scalar_array_values)
                };
                let array = arrays.get(&home.place).ok_or_else(malformed)?;
                if machine != home.machine
                    || array.structural_type != home.structural_type
                    || array.elements.get(start..end).is_none_or(|bytes| {
                        bytes
                            .iter()
                            .any(|byte| byte.scalar_type() != loan.scalar_type)
                    })
                {
                    return Err(malformed());
                }
                *index = start;
            }
            PrimitiveStorageEndpoint::ViewByte { bytes, index } => {
                if loan.scalar_type != byte_type()? {
                    return Err(malformed());
                }
                let start = index.checked_add(byte_offset).ok_or_else(malformed)?;
                let end = start.checked_add(width).ok_or_else(malformed)?;
                if end > bytes.len() {
                    return Err(malformed());
                }
                *index = start;
            }
            PrimitiveStorageEndpoint::Scalar(_) | PrimitiveStorageEndpoint::ScalarField(_) => {
                let source_width = usize::from(
                    terminal_semantics::reference_storage_byte_width(loan.scalar_type)
                        .ok_or_else(malformed)?,
                );
                if byte_offset != 0 || width > source_width {
                    return Err(malformed());
                }
            }
        }
        Ok(())
    }

    pub(super) fn read_encoded_primitive(
        &self,
        source: PlaceId,
        endpoint: &PrimitiveStorageEndpoint,
        stored_scalar: ScalarType,
        logical_scalar: ScalarType,
    ) -> Result<TerminalScalarValue, TerminalInterpretError> {
        let stored = match endpoint {
            PrimitiveStorageEndpoint::Scalar(_) | PrimitiveStorageEndpoint::ScalarField(_) => {
                let scalar = self.endpoint_scalar(source, endpoint)?;
                // A typed scalar cell has no separately addressable byte payload.
                // Round-trip through the codec without replacing its backing.
                let bits = if scalar.scalar_type() == byte_type()? {
                    encode(byte_type()?, scalar)?
                } else {
                    encode(stored_scalar, scalar)?
                };
                decode(stored_scalar, bits)?
            }
            _ => {
                let width = terminal_semantics::reference_storage_byte_width(stored_scalar)
                    .ok_or_else(malformed)?;
                let mut bits = 0_u128;
                for byte_index in 0..usize::from(width) {
                    let storage = shifted_byte_endpoint(endpoint, byte_index)?;
                    let byte = self.read_primitive_storage(
                        source,
                        PrimitiveStorage::canonical(storage),
                        byte_type()?,
                        byte_type()?,
                        false,
                    )?;
                    let TerminalScalarValue::Integer {
                        value: IntegerValue::Unsigned(byte),
                        ..
                    } = byte
                    else {
                        return Err(malformed());
                    };
                    bits |= byte << (byte_index * 8);
                }
                decode(stored_scalar, bits)?
            }
        };
        if !terminal_semantics::reference_storage_view_is_compatible(stored_scalar, logical_scalar)
        {
            return Err(malformed());
        }
        convert_scalar(stored, logical_scalar)
    }

    pub(super) fn prepare_encoded_primitive(
        &self,
        writes: &mut super::PrimitiveStorageWrites,
        destination: PlaceId,
        endpoint: &PrimitiveStorageEndpoint,
        stored_scalar: ScalarType,
        scalar: TerminalScalarValue,
    ) -> Result<(), TerminalInterpretError> {
        if !terminal_semantics::reference_storage_view_is_compatible(
            stored_scalar,
            scalar.scalar_type(),
        ) {
            return Err(malformed());
        }
        let bits = encode(stored_scalar, scalar)?;
        match endpoint {
            PrimitiveStorageEndpoint::Scalar(_) | PrimitiveStorageEndpoint::ScalarField(_) => {
                let original = self.endpoint_scalar(destination, endpoint)?;
                let replacement = if original.scalar_type() == byte_type()? {
                    decode(byte_type()?, bits)?
                } else {
                    convert_scalar(decode(stored_scalar, bits)?, original.scalar_type())?
                };
                writes.stage(
                    self,
                    destination,
                    PrimitiveStorage::canonical(endpoint.clone()),
                    original.scalar_type(),
                    replacement,
                )
            }
            _ => {
                let width = terminal_semantics::reference_storage_byte_width(stored_scalar)
                    .ok_or_else(malformed)?;
                // Resolve every destination byte without reading its contents.
                // The enclosing transaction publishes only after all endpoints
                // and encodings have been prepared, including later leaves.
                for byte_index in 0..usize::from(width) {
                    let byte = TerminalScalarValue::Integer {
                        scalar_type: byte_integer_type()?,
                        value: IntegerValue::Unsigned((bits >> (byte_index * 8)) & 255),
                    };
                    writes.stage(
                        self,
                        destination,
                        PrimitiveStorage::canonical(shifted_byte_endpoint(endpoint, byte_index)?),
                        byte_type()?,
                        byte,
                    )?;
                }
                Ok(())
            }
        }
    }

    pub(crate) fn endpoint_scalar(
        &self,
        source: PlaceId,
        endpoint: &PrimitiveStorageEndpoint,
    ) -> Result<TerminalScalarValue, TerminalInterpretError> {
        let value = match endpoint {
            PrimitiveStorageEndpoint::Scalar(place) => self.structural_primitive_storage.get(place),
            PrimitiveStorageEndpoint::ScalarField(field) => {
                self.structural_scalar_fields.get(field)
            }
            _ => return Err(malformed()),
        }
        .copied()
        .ok_or(crate::errors::TerminalInterpretError::StructuralPrimitiveStorageMissing(source))?;
        if !terminal_scalar_belongs_to_type(value) {
            return Err(malformed());
        }
        Ok(value)
    }
}

pub(crate) fn shifted_byte_endpoint(
    endpoint: &PrimitiveStorageEndpoint,
    byte_offset: usize,
) -> Result<PrimitiveStorageEndpoint, TerminalInterpretError> {
    match endpoint {
        PrimitiveStorageEndpoint::ArrayElement { array, index } => {
            Ok(PrimitiveStorageEndpoint::ArrayElement {
                array: array.clone(),
                index: index.checked_add(byte_offset).ok_or_else(malformed)?,
            })
        }
        PrimitiveStorageEndpoint::OwnedArrayElement { home, index } => {
            Ok(PrimitiveStorageEndpoint::OwnedArrayElement {
                home: *home,
                index: index.checked_add(byte_offset).ok_or_else(malformed)?,
            })
        }
        PrimitiveStorageEndpoint::ViewByte { bytes, index } => {
            Ok(PrimitiveStorageEndpoint::ViewByte {
                bytes: bytes.clone(),
                index: index.checked_add(byte_offset).ok_or_else(malformed)?,
            })
        }
        PrimitiveStorageEndpoint::Scalar(_) | PrimitiveStorageEndpoint::ScalarField(_)
            if byte_offset == 0 =>
        {
            Ok(endpoint.clone())
        }
        _ => Err(malformed()),
    }
}

fn byte_integer_type() -> Result<IntegerType, TerminalInterpretError> {
    IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())
}
fn byte_type() -> Result<ScalarType, TerminalInterpretError> {
    Ok(ScalarType::Integer(byte_integer_type()?))
}

pub(super) fn convert_scalar(
    value: TerminalScalarValue,
    target: ScalarType,
) -> Result<TerminalScalarValue, TerminalInterpretError> {
    if !terminal_scalar_belongs_to_type(value) {
        return Err(malformed());
    }
    if terminal_semantics::scalar_reference_view_is_total(value.scalar_type(), target) {
        return super::scalar_reference_observation(value, target);
    }
    // The Boolean codec validates the landing value, not merely equal width.
    // This conversion grants no access or evidence to the source operation.
    if let (TerminalScalarValue::Integer { scalar_type, value }, ScalarType::Boolean) =
        (value, target)
        && let Some(pair) = terminal_semantics::ScalarRepresentationViewPair::classify(
            ScalarType::Integer(scalar_type),
            target,
        )
        && let Some(terminal_semantics::ScalarViewValue::Boolean(value)) =
            pair.view(terminal_semantics::ScalarViewValue::Integer(value))
    {
        return Ok(TerminalScalarValue::Boolean(value));
    }
    let (TerminalScalarValue::Integer { value, .. }, ScalarType::Integer(integer)) =
        (value, target)
    else {
        return Err(malformed());
    };
    if integer.is_address() {
        return Err(malformed());
    }
    let value = match (integer.sign(), value) {
        (IntegerSign::Signed, IntegerValue::Signed(value)) => IntegerValue::Signed(value),
        (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => IntegerValue::Unsigned(value),
        (IntegerSign::Signed, IntegerValue::Unsigned(value)) => {
            IntegerValue::Signed(i128::try_from(value).map_err(|_| malformed())?)
        }
        (IntegerSign::Unsigned, IntegerValue::Signed(value)) => {
            IntegerValue::Unsigned(u128::try_from(value).map_err(|_| malformed())?)
        }
    };
    if !integer.admits(value) {
        return Err(malformed());
    }
    Ok(TerminalScalarValue::Integer {
        scalar_type: integer,
        value,
    })
}

pub(super) fn encode(
    stored: ScalarType,
    scalar: TerminalScalarValue,
) -> Result<u128, TerminalInterpretError> {
    let scalar = convert_scalar(scalar, stored)?;
    let width = terminal_semantics::reference_storage_byte_width(stored).ok_or_else(malformed)?;
    let bits = match scalar {
        TerminalScalarValue::Boolean(value) => u128::from(value),
        TerminalScalarValue::Integer {
            value: IntegerValue::Unsigned(value),
            ..
        } => value,
        TerminalScalarValue::Integer {
            value: IntegerValue::Signed(value),
            ..
        } => value as u128,
        TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(bits)) => u128::from(bits),
        TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(bits)) => u128::from(bits),
    };
    let mask = if width == 16 {
        u128::MAX
    } else {
        (1_u128 << (usize::from(width) * 8)) - 1
    };
    Ok(bits & mask)
}

pub(super) fn decode(
    stored: ScalarType,
    bits: u128,
) -> Result<TerminalScalarValue, TerminalInterpretError> {
    let width = terminal_semantics::reference_storage_byte_width(stored).ok_or_else(malformed)?;
    let result = match stored {
        ScalarType::Boolean => boolean_bits(bits)?,
        ScalarType::Integer(integer) => {
            let value = match integer.sign() {
                IntegerSign::Unsigned => IntegerValue::Unsigned(bits),
                IntegerSign::Signed => {
                    let bit_count = usize::from(width) * 8;
                    let signed = if bit_count == 128 || bits & (1_u128 << (bit_count - 1)) == 0 {
                        bits as i128
                    } else {
                        (bits | !((1_u128 << bit_count) - 1)) as i128
                    };
                    IntegerValue::Signed(signed)
                }
            };
            TerminalScalarValue::Integer {
                scalar_type: integer,
                value,
            }
        }
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => TerminalScalarValue::IeeeFloat(
            IeeeFloatValue::Binary32(u32::try_from(bits).map_err(|_| malformed())?),
        ),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => TerminalScalarValue::IeeeFloat(
            IeeeFloatValue::Binary64(u64::try_from(bits).map_err(|_| malformed())?),
        ),
    };
    terminal_scalar_belongs_to_type(result)
        .then_some(result)
        .ok_or_else(malformed)
}

#[cfg(test)]
mod tests {
    use super::{convert_scalar, decode, encode, scalar_fits_storage};
    use crate::values::TerminalScalarValue;
    use semantic_vocabulary::{
        IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, ScalarType,
    };

    fn integer(sign: IntegerSign, bits: u16) -> IntegerType {
        IntegerType::new(sign, bits).expect("test integer carrier")
    }

    #[test]
    fn signed_byte_codec_extends_and_rejects_nonfitting_writes() {
        let stored = ScalarType::Integer(integer(IntegerSign::Signed, 8));
        let logical = integer(IntegerSign::Signed, 64);
        let negative = TerminalScalarValue::Integer {
            scalar_type: logical,
            value: IntegerValue::Signed(-128),
        };
        assert_eq!(encode(stored, negative).expect("signed byte"), 128);
        assert_eq!(
            convert_scalar(
                decode(stored, 128).expect("decode byte"),
                ScalarType::Integer(logical)
            )
            .expect("extend byte"),
            negative
        );
        for value in [-129, 128] {
            assert!(!scalar_fits_storage(
                stored,
                TerminalScalarValue::Integer {
                    scalar_type: logical,
                    value: IntegerValue::Signed(value)
                }
            ));
        }
        assert!(scalar_fits_storage(
            stored,
            TerminalScalarValue::Integer {
                scalar_type: logical,
                value: IntegerValue::Signed(127)
            }
        ));
    }

    #[test]
    fn arbitrary_byte_aligned_integer_widths_keep_exact_sign() {
        let stored = ScalarType::Integer(integer(IntegerSign::Signed, 24));
        let decoded = decode(stored, 0x80_0000).expect("signed 24-bit decode");
        assert_eq!(
            decoded,
            TerminalScalarValue::Integer {
                scalar_type: integer(IntegerSign::Signed, 24),
                value: IntegerValue::Signed(-8_388_608)
            }
        );
        let full = ScalarType::Integer(integer(IntegerSign::Signed, 128));
        assert_eq!(
            encode(
                full,
                decode(full, 1_u128 << 127).expect("signed 128-bit decode")
            )
            .expect("signed 128-bit encode"),
            1_u128 << 127
        );
    }

    #[test]
    fn boolean_and_ieee_codecs_keep_canonical_representation() {
        assert_eq!(
            decode(ScalarType::Boolean, 1).expect("canonical boolean"),
            TerminalScalarValue::Boolean(true)
        );
        assert!(decode(ScalarType::Boolean, 2).is_err());
        for byte in [0, 1] {
            assert_eq!(
                encode(
                    ScalarType::Boolean,
                    TerminalScalarValue::Integer {
                        scalar_type: integer(IntegerSign::Unsigned, 8),
                        value: IntegerValue::Unsigned(byte),
                    }
                )
                .expect("canonical Boolean byte"),
                byte
            );
        }
        assert!(
            encode(
                ScalarType::Boolean,
                TerminalScalarValue::Integer {
                    scalar_type: integer(IntegerSign::Unsigned, 8),
                    value: IntegerValue::Unsigned(2),
                }
            )
            .is_err()
        );
        let float = TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(0xfff8_0000_0000_0123));
        let stored = ScalarType::IeeeFloat(IeeeFloatFormat::Binary64);
        assert_eq!(
            decode(stored, encode(stored, float).expect("IEEE encode")).expect("IEEE decode"),
            float
        );
    }
}

/// Boolean scalar decoded from one stored bit.
pub(crate) fn boolean_bits(bits: u128) -> Result<TerminalScalarValue, TerminalInterpretError> {
    Ok(match bits {
        0 => TerminalScalarValue::Boolean(false),
        1 => TerminalScalarValue::Boolean(true),
        _ => return Err(malformed()),
    })
}
