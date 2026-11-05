//! Source-selected codecs operate on the retained endpoint. Byte-array reads
//! and writes touch exactly the selected footprint; scalar cells retain their
//! original semantic representation and must satisfy the same stored range.
//! Captured region projection retains extent and scalar codec together. A
//! projected logical type cannot recover the ancestor's authored field storage.

use super::{PrimitiveLoan, PrimitiveStorage, PrimitiveStorageEncoding, PrimitiveStorageEndpoint};
use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::boundary::values::TerminalScalarValue;
use crate::engine::custody::resolve_structural_path_type;
use crate::engine::execution::TerminalExecution;
use crate::engine::scalar_operations::terminal_scalar_belongs_to_type;
use semantic_vocabulary::{
    IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, PlaceId, ScalarType,
};
use terminal_semantics::{reference_storage_byte_width, reference_storage_view_is_compatible};

#[derive(PartialEq, Eq)]
struct ReferenceByteRegionProjection {
    region: crate::storage::reference::ReferenceByteRegion,
    scalar: Option<(ScalarType, ScalarType)>,
}

impl ReferenceByteRegionProjection {
    fn scalar_storage(self) -> Result<(PrimitiveStorage, ScalarType), TerminalInterpretError> {
        let (logical_scalar, stored_scalar) =
            self.scalar
                .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                    "captured byte-region projection has no contiguous scalar geometry",
                ))?;
        Ok((
            PrimitiveStorage {
                endpoint: self.region.endpoint,
                encoding: Some(PrimitiveStorageEncoding {
                    source: stored_scalar,
                    view: stored_scalar,
                }),
            },
            logical_scalar,
        ))
    }
}

/// A replacement must fit the retained codec before any boundary publication.
pub(crate) fn scalar_fits_storage(stored: ScalarType, scalar: TerminalScalarValue) -> bool {
    reference_storage_view_is_compatible(stored, scalar.scalar_type())
        && encode(stored, scalar).is_ok()
}

impl TerminalExecution<'_> {
    /// Resolve scalar descendants of a retained aggregate region. Borrowed
    /// formals retain the same opaque view, so this works after forwarding as
    /// well as through the original reference carrier.
    pub(crate) fn reference_byte_region_access(
        &self,
        backing_view: &crate::boundary::values::TerminalStructuralValue,
        view: &crate::boundary::values::TerminalStructuralValue,
    ) -> Result<Option<(PrimitiveStorage, ScalarType)>, TerminalInterpretError> {
        if view.opaque_identity != backing_view.opaque_identity
            || !view.path.starts_with(&backing_view.path)
        {
            return Err(malformed());
        }
        let Some(projected) = self.project_reference_byte_region(backing_view, &view.path)? else {
            return Ok(None);
        };
        projected.scalar_storage().map(Some)
    }

    /// A suspended ingress already owns this exact association. Project its
    /// complete declared path without consulting another activation's place
    /// namespace or looking for the now-retired supplying descriptor.
    pub(crate) fn captured_byte_region_access(
        &self,
        anchor: &crate::boundary::values::TerminalStructuralValue,
        region: &crate::storage::reference::ReferenceByteRegion,
        path: &[terminal_psi::StructuralPathSegment],
    ) -> Result<(PrimitiveStorage, ScalarType), TerminalInterpretError> {
        let mut projected_path = anchor.path.clone();
        projected_path.extend_from_slice(path);
        self.project_byte_regions(anchor, &projected_path, std::iter::once((anchor, region)))?
            .ok_or_else(malformed)?
            .scalar_storage()
    }

    /// Project a captured region under its declared type and retain its endpoint.
    /// Formation and scalar accesses share this lookup; forwarding never
    /// reconstructs bytes from the original formation expression.
    pub(crate) fn reference_byte_region(
        &self,
        view: &crate::boundary::values::TerminalStructuralValue,
    ) -> Result<Option<crate::storage::reference::ReferenceByteRegion>, TerminalInterpretError>
    {
        Ok(self
            .project_reference_byte_region(view, &view.path)?
            .map(|projected| projected.region))
    }

    /// Keep the codec chosen along the captured ancestor's complete path.
    /// Starting again at a projected i64 would lose its IntegerAt byte width.
    /// Formation and access share this endpoint, extent and codec selection.
    fn project_reference_byte_region(
        &self,
        view: &crate::boundary::values::TerminalStructuralValue,
        projected_path: &[terminal_psi::StructuralPathSegment],
    ) -> Result<Option<ReferenceByteRegionProjection>, TerminalInterpretError> {
        // Carrier retirement ends that descriptor, not a borrowed ingress's
        // retained storage. Both supply the same authenticated anchor/region
        // pair; no source expression or replacement typed cell is replayed.
        let references = self
            .reference_referents
            .values()
            .filter_map(|captured| Some((&captured.view, captured.byte_region.as_ref()?)));
        let arguments = self
            .aggregate_argument_backings
            .values()
            .filter_map(|backing| Some((&backing.value, backing.byte_region.as_ref()?)));
        self.project_byte_regions(view, projected_path, references.chain(arguments))
    }

    fn project_byte_regions<'captured>(
        &self,
        view: &crate::boundary::values::TerminalStructuralValue,
        projected_path: &[terminal_psi::StructuralPathSegment],
        captured: impl Iterator<
            Item = (
                &'captured crate::boundary::values::TerminalStructuralValue,
                &'captured crate::storage::reference::ReferenceByteRegion,
            ),
        >,
    ) -> Result<Option<ReferenceByteRegionProjection>, TerminalInterpretError> {
        let mut selected = None;
        for (anchor, region) in captured {
            if view.opaque_identity != anchor.opaque_identity {
                continue;
            }
            let Some(path) = view.path.strip_prefix(anchor.path.as_slice()) else {
                continue;
            };
            if resolve_structural_path_type(&self.structural_types(), anchor.structural_type, path)
                .ok()
                != Some(view.structural_type)
            {
                continue;
            }
            let path = projected_path
                .strip_prefix(anchor.path.as_slice())
                .ok_or_else(malformed)?;
            let geometry = terminal_semantics::reference_storage_region(
                self.structural_types().values(),
                anchor.structural_type,
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
                region: crate::storage::reference::ReferenceByteRegion {
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
        let width = usize::from(reference_storage_byte_width(stored_scalar).ok_or_else(malformed)?);
        if !reference_storage_view_is_compatible(stored_scalar, logical_scalar) {
            return Err(malformed());
        }
        if let Some(captured) = &mut loan.storage.encoding {
            if byte_offset != 0
                || reference_storage_byte_width(captured.source)
                    != reference_storage_byte_width(stored_scalar)
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
        let endpoint = match &loan.storage.endpoint {
            PrimitiveStorageEndpoint::Projection { .. } => "Projection",
            PrimitiveStorageEndpoint::ArrayElement { .. } => "ArrayElement",
            PrimitiveStorageEndpoint::OwnedArrayElement { .. } => "OwnedArrayElement",
            PrimitiveStorageEndpoint::RepresentationByte(_) => "RepresentationByte",
            PrimitiveStorageEndpoint::ViewByte { .. } => "ViewByte",
            PrimitiveStorageEndpoint::Scalar(_) => "Scalar",
            PrimitiveStorageEndpoint::ScalarField(_) => "ScalarField",
        };
        let scalar = loan.scalar_type;
        let codec = loan.storage.encoding;
        let referent = (
            loan.referent.opaque_identity,
            loan.referent.structural_type,
            loan.referent.path.len(),
        );
        // Retain metadata from the actual failed walk. No bytes are read for
        // the diagnostic, and successful positioning formats no context.
        let mut phase = "endpoint already has a retained codec";
        let mut available = None;
        let mut span_start = None;
        let mut span_end = None;
        let mut requested_frame = None;
        let mut actual_machine = None;
        let mut actual_type = None;
        let mut fragment_index = None;
        let mut fragment_count = None;
        let mut projection_types = None;
        let mut first_fragment = None;
        let mut first_mismatched_scalar = None;
        let outcome = (|| {
            if loan.storage.encoding.is_some() {
                return Err(malformed());
            }
            match &mut loan.storage.endpoint {
                PrimitiveStorageEndpoint::Projection {
                    fragments,
                    logical,
                    stored,
                } => {
                    projection_types = Some((*logical, *stored));
                    first_fragment = fragments.first().map(|fragment| {
                        (
                            fragment.logical,
                            fragment.stored,
                            fragment.source_bit_offset,
                            fragment.destination_bit_offset,
                            fragment.bit_length,
                            fragment.addressable_cell,
                            fragment.storage.encoding,
                        )
                    });
                    phase = "projection has no stored byte width";
                    let stored_width =
                        usize::from(reference_storage_byte_width(*stored).ok_or_else(malformed)?);
                    available = Some(stored_width);
                    fragment_count = Some(fragments.len());
                    phase = "projection end overflows";
                    let end = byte_offset.checked_add(width).ok_or_else(malformed)?;
                    span_end = Some(end);
                    phase = "region exceeds the stored projection";
                    if end > stored_width {
                        return Err(malformed());
                    }
                    let mut selected = Vec::new();
                    phase = "projection bit offset overflows";
                    let selected_start = byte_offset.checked_mul(8).ok_or_else(malformed)?;
                    phase = "projection bit end overflows";
                    let selected_end = end.checked_mul(8).ok_or_else(malformed)?;
                    for (position, fragment) in fragments.iter().enumerate() {
                        fragment_index = Some(position);
                        let start = usize::from(fragment.destination_bit_offset);
                        let fragment_end = start + usize::from(fragment.bit_length);
                        let first = start.max(selected_start);
                        let last = fragment_end.min(selected_end);
                        if first >= last {
                            continue;
                        }
                        let mut fragment = fragment.clone();
                        phase = "fragment source bit adjustment overflows";
                        fragment.source_bit_offset = fragment
                            .source_bit_offset
                            .checked_add(u8::try_from(first - start).map_err(|_| malformed())?)
                            .ok_or_else(malformed)?;
                        phase = "fragment destination bit offset is too large";
                        fragment.destination_bit_offset =
                            u8::try_from(first - selected_start).map_err(|_| malformed())?;
                        phase = "fragment bit length is too large";
                        fragment.bit_length =
                            u8::try_from(last - first).map_err(|_| malformed())?;
                        selected.push(fragment);
                    }
                    *fragments = selected;
                    // The caller installs its exact observed codec after positioning.
                    let _ = logical;
                }
                PrimitiveStorageEndpoint::ArrayElement { array, index } => {
                    phase = "array endpoint is not a byte carrier";
                    if loan.scalar_type != byte_type()? {
                        return Err(malformed());
                    }
                    phase = "array start overflows";
                    let start = index.checked_add(byte_offset).ok_or_else(malformed)?;
                    span_start = Some(start);
                    phase = "array end overflows";
                    let end = start.checked_add(width).ok_or_else(malformed)?;
                    span_end = Some(end);
                    use crate::boundary::structural_inputs::byte_arrays::storage::ArrayStorageHome;
                    requested_frame = match array.home {
                        ArrayStorageHome::Runtime => None,
                        ArrayStorageHome::Frame {
                            depth,
                            place,
                            structural_type,
                        } => Some((depth, place, structural_type)),
                    };
                    phase = "array extent is unavailable";
                    let length = self.stored_byte_array_length(array)?;
                    available = Some(length);
                    phase = "region exceeds the captured array";
                    if end > length {
                        return Err(malformed());
                    }
                    *index = start;
                }
                PrimitiveStorageEndpoint::OwnedArrayElement { home, index } => {
                    phase = "owned array endpoint is not a byte carrier";
                    if loan.scalar_type != byte_type()? {
                        return Err(malformed());
                    }
                    requested_frame = Some((home.depth, home.place, home.structural_type));
                    phase = "owned array start overflows";
                    let start = index.checked_add(byte_offset).ok_or_else(malformed)?;
                    span_start = Some(start);
                    phase = "owned array end overflows";
                    let end = start.checked_add(width).ok_or_else(malformed)?;
                    span_end = Some(end);
                    phase = "owned array activation is unavailable";
                    let (machine, arrays) = if home.depth == self.call_stack.len() {
                        (self.current_machine, &self.scalar_array_values)
                    } else {
                        let frame = self.call_stack.get(home.depth).ok_or_else(malformed)?;
                        (frame.current_machine, &frame.scalar_array_values)
                    };
                    actual_machine = Some((home.machine, machine));
                    phase = "owned array backing is unavailable";
                    let array = arrays.backing(&home.place).ok_or_else(malformed)?;
                    actual_type = Some(array.structural_type);
                    available = Some(array.len());
                    if machine != home.machine
                        || array.structural_type != home.structural_type
                        || array.cells().get(start..end).is_none_or(|bytes| {
                            bytes.iter().any(|byte| {
                                let observed = byte.scalar_type();
                                let mismatched = observed != loan.scalar_type;
                                if mismatched {
                                    first_mismatched_scalar = Some(observed);
                                }
                                mismatched
                            })
                        })
                    {
                        phase = if machine != home.machine {
                            "owned array machine differs"
                        } else if array.structural_type != home.structural_type {
                            "owned array type differs"
                        } else if first_mismatched_scalar.is_some() {
                            "owned array cell carrier differs"
                        } else {
                            "region exceeds the owned array"
                        };
                        return Err(malformed());
                    }
                    *index = start;
                }
                PrimitiveStorageEndpoint::RepresentationByte(byte) => {
                    phase = "representation endpoint is not a byte carrier";
                    if loan.scalar_type != byte_type()? {
                        return Err(malformed());
                    }
                    phase = "representation start overflows";
                    let start = byte.index.checked_add(byte_offset).ok_or_else(malformed)?;
                    span_start = Some(start);
                    phase = "region exceeds the representation or its end overflows";
                    if start.checked_add(width).is_none_or(|end| {
                        span_end = Some(end);
                        let length = byte.view.len();
                        available = Some(length);
                        end > length
                    }) {
                        return Err(malformed());
                    }
                    byte.index = start;
                }
                PrimitiveStorageEndpoint::ViewByte { bytes, index } => {
                    phase = "view endpoint is not a byte carrier";
                    if loan.scalar_type != byte_type()? {
                        return Err(malformed());
                    }
                    phase = "view start overflows";
                    let start = index.checked_add(byte_offset).ok_or_else(malformed)?;
                    span_start = Some(start);
                    phase = "view end overflows";
                    let end = start.checked_add(width).ok_or_else(malformed)?;
                    span_end = Some(end);
                    let length = bytes.len();
                    available = Some(length);
                    phase = "region exceeds the captured view";
                    if end > length {
                        return Err(malformed());
                    }
                    *index = start;
                }
                PrimitiveStorageEndpoint::Scalar(_) | PrimitiveStorageEndpoint::ScalarField(_) => {
                    phase = "scalar endpoint has no stored byte width";
                    let source_width = usize::from(
                        reference_storage_byte_width(loan.scalar_type).ok_or_else(malformed)?,
                    );
                    available = Some(source_width);
                    phase = "region exceeds a single scalar cell";
                    if byte_offset != 0 || width > source_width {
                        return Err(malformed());
                    }
                }
            }
            Ok(())
        })();
        outcome.map_err(|error| TerminalInterpretError::ReferenceFormation {
            stage: "position reference storage",
            detail: format!(
                "{phase}; endpoint={endpoint}, byte_offset={byte_offset}, width={width}, scalar={scalar:?}, codec={codec:?}, referent_identity_type_path_length={referent:?}, span_start={span_start:?}, span_end={span_end:?}, available={available:?}, requested_depth_place_type={requested_frame:?}, requested_actual_machine={actual_machine:?}, actual_type={actual_type:?}, current_depth={}, projection_logical_stored={projection_types:?}, fragment_count={fragment_count:?}, fragment_index={fragment_index:?}, first_fragment_logical_stored_offsets_length_addressable_codec={first_fragment:?}, first_mismatched_scalar={first_mismatched_scalar:?}: {error:?}",
                self.call_stack.len()
            ),
        })
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
                let width = reference_storage_byte_width(stored_scalar).ok_or_else(malformed)?;
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
        if !reference_storage_view_is_compatible(stored_scalar, logical_scalar) {
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
        if !reference_storage_view_is_compatible(stored_scalar, scalar.scalar_type()) {
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
                let width = reference_storage_byte_width(stored_scalar).ok_or_else(malformed)?;
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
        .ok_or(
            crate::boundary::errors::TerminalInterpretError::StructuralPrimitiveStorageMissing(
                source,
            ),
        )?
        .read()?;
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
        PrimitiveStorageEndpoint::Projection {
            fragments, stored, ..
        } => {
            let width = usize::from(reference_storage_byte_width(*stored).ok_or_else(malformed)?);
            if byte_offset >= width {
                return Err(malformed());
            }
            let mut selected = Vec::new();
            let byte_start = byte_offset.checked_mul(8).ok_or_else(malformed)?;
            let byte_end = byte_start.checked_add(8).ok_or_else(malformed)?;
            for fragment in fragments {
                let first = usize::from(fragment.destination_bit_offset);
                let last = first + usize::from(fragment.bit_length);
                let start = first.max(byte_start);
                let end = last.min(byte_end);
                if start >= end {
                    continue;
                }
                let mut fragment = fragment.clone();
                fragment.source_bit_offset = fragment
                    .source_bit_offset
                    .checked_add(u8::try_from(start - first).map_err(|_| malformed())?)
                    .ok_or_else(malformed)?;
                fragment.destination_bit_offset =
                    u8::try_from(start - byte_start).map_err(|_| malformed())?;
                fragment.bit_length = u8::try_from(end - start).map_err(|_| malformed())?;
                selected.push(fragment);
            }
            let coverage = selected.iter().try_fold(0u16, |covered, fragment| {
                (u16::from(fragment.destination_bit_offset) == covered)
                    .then(|| covered.checked_add(u16::from(fragment.bit_length)))
                    .flatten()
            });
            if coverage != Some(8) {
                return Err(malformed());
            }
            Ok(PrimitiveStorageEndpoint::Projection {
                fragments: selected,
                logical: byte_type()?,
                stored: byte_type()?,
            })
        }
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
        PrimitiveStorageEndpoint::RepresentationByte(byte) => {
            let index = byte.index.checked_add(byte_offset).ok_or_else(malformed)?;
            if index >= byte.view.len() {
                return Err(malformed());
            }
            Ok(PrimitiveStorageEndpoint::RepresentationByte(
                super::RepresentationByte {
                    view: byte.view.clone(),
                    index,
                },
            ))
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

pub(crate) fn encode(
    stored: ScalarType,
    scalar: TerminalScalarValue,
) -> Result<u128, TerminalInterpretError> {
    let scalar = convert_scalar(scalar, stored)?;
    let width = reference_storage_byte_width(stored).ok_or_else(malformed)?;
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

pub(crate) fn decode(
    stored: ScalarType,
    bits: u128,
) -> Result<TerminalScalarValue, TerminalInterpretError> {
    let width = reference_storage_byte_width(stored).ok_or_else(malformed)?;
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

/// Boolean scalar decoded from one stored bit.
pub(crate) fn boolean_bits(bits: u128) -> Result<TerminalScalarValue, TerminalInterpretError> {
    Ok(match bits {
        0 => TerminalScalarValue::Boolean(false),
        1 => TerminalScalarValue::Boolean(true),
        _ => return Err(malformed()),
    })
}

#[cfg(test)]
mod tests {
    use super::{convert_scalar, decode, encode, scalar_fits_storage};
    use crate::boundary::values::TerminalScalarValue;
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
