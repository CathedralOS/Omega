//! Byte-level write primitives: scalar fragments, stored-integer fit
//! validation, container reads and writes, and bit masks.

use crate::layout_reports::{IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport};
use crate::materialization::MaterializationDiagnostic;
use crate::materialization::field_values::ScalarFieldValue;
use crate::placement::{ByteOrder, MaterializationWrite, StoredIntegerFit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScalarFragment {
    pub(crate) container_byte_offset: u64,
    pub(crate) container_width_bits: u16,
    pub(crate) destination_lsb: u16,
    pub(crate) source_lsb: u16,
    pub(crate) width: u16,
}

pub(crate) fn scalar_fragment(
    entry: &LayoutFieldEntryReport,
    source_width_bits: u16,
) -> Result<ScalarFragment, MaterializationDiagnostic> {
    let (container, container_width, destination_lsb, source_lsb, width) =
        materializer_scalar_window(entry.placement, source_width_bits, "scalar", &entry.field)?;
    Ok(ScalarFragment {
        container_byte_offset: container,
        container_width_bits: container_width,
        destination_lsb,
        source_lsb,
        width,
    })
}

/// The scalar write window of one placement report: bit positions resolved
/// against the value's own width, then validated to fit a materializer's
/// single-container fragment. `field_kind` and `field_display` name the
/// checked field in each diagnostic.
pub(crate) fn materializer_scalar_window(
    placement: LayoutPlacementReport,
    placed_width_bits: u16,
    field_kind: &str,
    field_display: &str,
) -> Result<(u64, u16, u16, u16, u16), MaterializationDiagnostic> {
    let (container, container_width, destination_lsb, source_lsb, width) = match placement {
        LayoutPlacementReport::At { offset } => (
            offset,
            u64::from(placed_width_bits),
            0,
            0,
            u64::from(placed_width_bits),
        ),
        LayoutPlacementReport::IntegerAt {
            offset,
            stored_width,
            ..
        } => (offset, stored_width, 0, 0, stored_width),
        LayoutPlacementReport::Bits {
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        } => (
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        ),
    };
    if container_width == 0 || container_width > 64 || container_width % 8 != 0 || width == 0 {
        return Err(MaterializationDiagnostic(format!(
            "{field_kind} field `{field_display}` uses a materializer-incompatible placement"
        )));
    }
    let source_end = source_lsb.checked_add(width).ok_or_else(|| {
        MaterializationDiagnostic(format!("{field_kind} source bit range overflows"))
    })?;
    if source_end > u64::from(placed_width_bits) {
        return Err(MaterializationDiagnostic(format!(
            "{field_kind} field `{field_display}` placement reads through bit {source_end}, past its {placed_width_bits}-bit source"
        )));
    }
    let destination_end = destination_lsb.checked_add(width).ok_or_else(|| {
        MaterializationDiagnostic(format!("{field_kind} destination bit range overflows"))
    })?;
    if destination_end > container_width {
        return Err(MaterializationDiagnostic(format!(
            "{field_kind} field `{field_display}` placement writes through bit {destination_end}, past its {container_width}-bit container"
        )));
    }
    Ok((
        container,
        u16::try_from(container_width).expect("validated materializer container width"),
        u16::try_from(destination_lsb).expect("validated destination bit index"),
        u16::try_from(source_lsb).expect("validated source bit index"),
        u16::try_from(width).map_err(|_| {
            MaterializationDiagnostic(format!(
                "{field_kind} field `{field_display}` fragment width {width} is too large"
            ))
        })?,
    ))
}

pub(crate) fn validate_stored_integer_value(
    entry: &LayoutFieldEntryReport,
    value: &ScalarFieldValue,
) -> Result<(), MaterializationDiagnostic> {
    validate_stored_integer_bits(entry, value.width_bits, value.value, "scalar")
}

/// Whether an authored scalar carrier width is well formed: a nonzero byte
/// multiple of at most 64 bits. The materializer's own containers, an authored
/// stored-integer width, and a decode schema's carrier all read this one
/// vocabulary.
pub(crate) const fn byte_sized_width(width_bits: u16) -> bool {
    width_bits != 0 && width_bits <= 64 && width_bits % 8 == 0
}

/// Whether `source_value` — masked to its own carrier — lands inside the
/// authored stored-integer storage. The interval comes from the
/// interpretation's own [`IntegerInterpretation::stored_bounds`]; recomputing
/// it as bit arithmetic here is how the materializer, the replay validator and
/// the decoder drifted apart.
pub(crate) fn stored_value_fits(
    interpretation: IntegerInterpretation,
    stored_width_bits: u16,
    source_width_bits: u16,
    source_value: u64,
) -> bool {
    let (minimum, maximum) = interpretation.stored_bounds(u64::from(stored_width_bits));
    let semantic = match interpretation {
        IntegerInterpretation::Signed => signed_bits(source_value, source_width_bits),
        IntegerInterpretation::Unsigned => i128::from(source_value),
    };
    semantic >= minimum && semantic <= maximum
}

fn validate_stored_integer_bits(
    entry: &LayoutFieldEntryReport,
    source_width_bits: u16,
    source_value: u64,
    value_kind: &str,
) -> Result<(), MaterializationDiagnostic> {
    let LayoutPlacementReport::IntegerAt {
        stored_width,
        interpretation,
        ..
    } = entry.placement
    else {
        return Ok(());
    };
    let stored_width = u16::try_from(stored_width).map_err(|_| {
        MaterializationDiagnostic(format!(
            "{value_kind} field `{}` has an invalid stored-integer width",
            entry.field,
        ))
    })?;
    if !byte_sized_width(stored_width) {
        return Err(MaterializationDiagnostic(format!(
            "{value_kind} field `{}` has an invalid {stored_width}-bit stored-integer width",
            entry.field
        )));
    }
    if source_width_bits < stored_width {
        return Err(MaterializationDiagnostic(format!(
            "{value_kind} field `{}` has a {source_width_bits}-bit value narrower than its {stored_width}-bit storage",
            entry.field,
        )));
    }

    if !stored_value_fits(
        interpretation,
        stored_width,
        source_width_bits,
        source_value,
    ) {
        return Err(MaterializationDiagnostic(format!(
            "{value_kind} field `{}` value {source_value:#x} does not fit its {stored_width}-bit {} storage",
            entry.field,
            match interpretation {
                IntegerInterpretation::Signed => "signed",
                IntegerInterpretation::Unsigned => "unsigned",
            }
        )));
    }
    Ok(())
}

pub fn validate_write_source_value(
    write: &MaterializationWrite,
    source_value: u64,
    value_kind: &str,
) -> Result<(), MaterializationDiagnostic> {
    let Some(fit) = write.stored_integer_fit else {
        return Ok(());
    };
    validate_stored_integer_fit(&write.field, fit, source_value, value_kind)
}

pub fn validate_stored_integer_fit(
    field: &str,
    fit: StoredIntegerFit,
    source_value: u64,
    value_kind: &str,
) -> Result<(), MaterializationDiagnostic> {
    validate_stored_integer_fit_shape(field, fit, value_kind)?;
    let StoredIntegerFit {
        source_width_bits,
        stored_width_bits,
        interpretation,
    } = fit;

    if !stored_value_fits(
        interpretation,
        stored_width_bits,
        source_width_bits,
        source_value,
    ) {
        return Err(MaterializationDiagnostic(format!(
            "{value_kind} field `{field}` value {source_value:#x} does not fit its {stored_width_bits}-bit {} storage",
            match interpretation {
                IntegerInterpretation::Signed => "signed",
                IntegerInterpretation::Unsigned => "unsigned",
            }
        )));
    }
    Ok(())
}

pub fn validate_stored_integer_fit_shape(
    field: &str,
    fit: StoredIntegerFit,
    value_kind: &str,
) -> Result<(), MaterializationDiagnostic> {
    let StoredIntegerFit {
        source_width_bits,
        stored_width_bits,
        interpretation: _,
    } = fit;
    if source_width_bits == 0 || source_width_bits > 64 {
        return Err(MaterializationDiagnostic(format!(
            "{value_kind} field `{field}` has an invalid {source_width_bits}-bit source width"
        )));
    }
    if !byte_sized_width(stored_width_bits) || source_width_bits < stored_width_bits {
        return Err(MaterializationDiagnostic(format!(
            "{value_kind} field `{field}` has an invalid {stored_width_bits}-bit stored-integer fit constraint"
        )));
    }
    Ok(())
}

fn signed_bits(value: u64, width: u16) -> i128 {
    let value = value & low_mask(width);
    if value & (1_u64 << (width - 1)) == 0 {
        i128::from(value)
    } else {
        i128::from(value) - (1_i128 << width)
    }
}

pub fn apply_write(
    bytes: &mut [u8],
    byte_order: ByteOrder,
    write: &MaterializationWrite,
    source_value: u64,
) -> Result<(), MaterializationDiagnostic> {
    apply_fragment(
        bytes,
        byte_order,
        &write.field,
        write.container_byte_offset,
        write.container_width_bits,
        write.destination_lsb,
        write.source_lsb,
        write.width,
        source_value,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_fragment(
    bytes: &mut [u8],
    byte_order: ByteOrder,
    field: &str,
    container_byte_offset: u64,
    container_width_bits: u16,
    destination_lsb: u16,
    source_lsb: u16,
    width: u16,
    source_value: u64,
) -> Result<(), MaterializationDiagnostic> {
    let container_bytes = usize::from(container_width_bits / 8);
    let start = usize::try_from(container_byte_offset).map_err(|_| {
        MaterializationDiagnostic("container offset cannot be represented on this host".into())
    })?;
    let end = start
        .checked_add(container_bytes)
        .ok_or_else(|| MaterializationDiagnostic("container byte range overflows".into()))?;
    let materialization_len = bytes.len();
    let container_slice = bytes.get_mut(start..end).ok_or_else(|| {
        MaterializationDiagnostic(format!(
            "symbolic field `{}` writes outside the {}-byte materialization",
            field, materialization_len
        ))
    })?;
    let mut container_value = read_container(container_slice, byte_order);
    let fragment_mask = low_mask(width);
    let fragment = (source_value >> source_lsb) & fragment_mask;
    let destination_mask = fragment_mask << destination_lsb;
    container_value =
        (container_value & !destination_mask) | ((fragment << destination_lsb) & destination_mask);
    write_container(container_slice, byte_order, container_value);
    Ok(())
}

pub fn validate_write(
    materialization_len: usize,
    write: &MaterializationWrite,
) -> Result<(), MaterializationDiagnostic> {
    validate_fragment(
        materialization_len,
        &write.field,
        write.container_byte_offset,
        write.container_width_bits,
        write.destination_lsb,
        write.source_lsb,
        write.width,
    )?;
    if let Some(fit) = write.stored_integer_fit
        && (fit.stored_width_bits != write.container_width_bits
            || fit.stored_width_bits != write.width
            || write.destination_lsb != 0
            || write.source_lsb != 0)
    {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{}` has stored-integer fit evidence inconsistent with its write geometry",
            write.field
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn validate_fragment(
    materialization_len: usize,
    field: &str,
    container_byte_offset: u64,
    container_width_bits: u16,
    destination_lsb: u16,
    source_lsb: u16,
    width: u16,
) -> Result<(), MaterializationDiagnostic> {
    if container_width_bits == 0
        || container_width_bits > 64
        || !container_width_bits.is_multiple_of(8)
    {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{}` has invalid {}-bit container",
            field, container_width_bits
        )));
    }
    if width == 0
        || width > 64
        || source_lsb.checked_add(width).is_none_or(|end| end > 64)
        || destination_lsb
            .checked_add(width)
            .is_none_or(|end| end > container_width_bits)
    {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{}` has an invalid source or destination bit range",
            field
        )));
    }
    let start = usize::try_from(container_byte_offset).map_err(|_| {
        MaterializationDiagnostic("container offset cannot be represented on this host".into())
    })?;
    let end = start
        .checked_add(usize::from(container_width_bits / 8))
        .ok_or_else(|| MaterializationDiagnostic("container byte range overflows".into()))?;
    if end > materialization_len {
        return Err(MaterializationDiagnostic(format!(
            "symbolic field `{}` writes outside the {}-byte materialization",
            field, materialization_len
        )));
    }
    Ok(())
}

pub(crate) fn read_container(bytes: &[u8], byte_order: ByteOrder) -> u64 {
    bytes
        .iter()
        .enumerate()
        .fold(0_u64, |value, (index, byte)| {
            let shift = match byte_order {
                ByteOrder::LittleEndian => index * 8,
                ByteOrder::BigEndian => (bytes.len() - 1 - index) * 8,
            };
            value | (u64::from(*byte) << shift)
        })
}

fn write_container(bytes: &mut [u8], byte_order: ByteOrder, value: u64) {
    let byte_len = bytes.len();
    for (index, byte) in bytes.iter_mut().enumerate() {
        let shift = match byte_order {
            ByteOrder::LittleEndian => index * 8,
            ByteOrder::BigEndian => (byte_len - 1 - index) * 8,
        };
        *byte = ((value >> shift) & 0xff) as u8;
    }
}

pub(crate) const fn low_mask(width: u16) -> u64 {
    if width == 64 {
        u64::MAX
    } else {
        (1_u64 << width) - 1
    }
}
