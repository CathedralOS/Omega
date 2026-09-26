//! Byte-level write primitives: scalar fragments, stored-integer fit
//! validation, container reads and writes, and bit masks.

use crate::layout_plans::layout_reports::{
    IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport,
};
use crate::layout_plans::materialization::MaterializationDiagnostic;
use crate::layout_plans::materialization::field_values::ScalarFieldValue;
use crate::layout_plans::placement::{ByteOrder, MaterializationWrite, StoredIntegerFit};

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
    let (container, container_width, destination_lsb, source_lsb, width) = match entry.placement {
        LayoutPlacementReport::At { offset } => (
            offset,
            u64::from(source_width_bits),
            0,
            0,
            u64::from(source_width_bits),
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
            "scalar field `{}` uses a materializer-incompatible placement",
            entry.field
        )));
    }
    let source_end = source_lsb
        .checked_add(width)
        .ok_or_else(|| MaterializationDiagnostic("scalar source bit range overflows".into()))?;
    if source_end > u64::from(source_width_bits) {
        return Err(MaterializationDiagnostic(format!(
            "scalar field `{}` placement reads through bit {source_end}, past its {}-bit source",
            entry.field, source_width_bits
        )));
    }
    let destination_end = destination_lsb.checked_add(width).ok_or_else(|| {
        MaterializationDiagnostic("scalar destination bit range overflows".into())
    })?;
    if destination_end > container_width {
        return Err(MaterializationDiagnostic(format!(
            "scalar field `{}` placement writes through bit {destination_end}, past its {container_width}-bit container",
            entry.field
        )));
    }

    Ok(ScalarFragment {
        container_byte_offset: container,
        container_width_bits: u16::try_from(container_width)
            .expect("validated materializer container width"),
        destination_lsb: u16::try_from(destination_lsb).expect("validated destination bit index"),
        source_lsb: u16::try_from(source_lsb).expect("validated source bit index"),
        width: u16::try_from(width).map_err(|_| {
            MaterializationDiagnostic(format!(
                "scalar field `{}` fragment width {width} is too large",
                entry.field
            ))
        })?,
    })
}

pub(crate) fn validate_stored_integer_value(
    entry: &LayoutFieldEntryReport,
    value: &ScalarFieldValue,
) -> Result<(), MaterializationDiagnostic> {
    validate_stored_integer_bits(entry, value.width_bits, value.value, "scalar")
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
    if stored_width == 0 || stored_width > 64 || !stored_width.is_multiple_of(8) {
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

    let fits = match interpretation {
        IntegerInterpretation::Signed => {
            let semantic = signed_bits(source_value, source_width_bits);
            let magnitude = 1_i128 << (stored_width - 1);
            semantic >= -magnitude && semantic < magnitude
        }
        IntegerInterpretation::Unsigned => source_value <= low_mask(stored_width),
    };
    if !fits {
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

pub(crate) fn validate_write_source_value(
    write: &MaterializationWrite,
    source_value: u64,
    value_kind: &str,
) -> Result<(), MaterializationDiagnostic> {
    let Some(fit) = write.stored_integer_fit else {
        return Ok(());
    };
    validate_stored_integer_fit(&write.field, fit, source_value, value_kind)
}

pub(crate) fn validate_stored_integer_fit(
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

    let fits = match interpretation {
        IntegerInterpretation::Signed => {
            let semantic = signed_bits(source_value, source_width_bits);
            let magnitude = 1_i128 << (stored_width_bits - 1);
            semantic >= -magnitude && semantic < magnitude
        }
        IntegerInterpretation::Unsigned => source_value <= low_mask(stored_width_bits),
    };
    if !fits {
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

pub(crate) fn validate_stored_integer_fit_shape(
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
    if stored_width_bits == 0
        || stored_width_bits > 64
        || !stored_width_bits.is_multiple_of(8)
        || source_width_bits < stored_width_bits
    {
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

pub(crate) fn apply_write(
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

pub(crate) fn validate_write(
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
pub(crate) fn validate_fragment(
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
