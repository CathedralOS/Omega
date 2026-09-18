//! Non-authorizing validation of one borrowed UEFI Boot Services occurrence.
//!
//! Header signature, aggregate coverage, reserved-zero, and CRC integrity are
//! checked against the exact target-owned table layout. The resulting carrier
//! still grants neither pointer provenance nor permission to invoke a service.

use crate::{
    UefiBootServicesNativeField, UefiBootServicesNativeFieldKind,
    UefiBootServicesNativeFieldLayout, ValidatedUefiBootServicesNativeLayout,
};
use diagnostics::Diagnostic;

pub const UEFI_BOOT_SERVICES_SIGNATURE: u64 = 0x5652_4553_544f_4f42;
const CRC32_POLYNOMIAL_REFLECTED: u32 = 0xedb8_8320;

#[derive(Debug)]
#[must_use = "validated UEFI Boot Services occurrence retains borrowed integrity evidence"]
pub struct ValidatedUefiBootServicesHeaderIntegrity<'occurrence> {
    layout: ValidatedUefiBootServicesNativeLayout,
    table_bytes: &'occurrence [u8],
    revision: u32,
    stored_crc32: u32,
}

impl<'occurrence> ValidatedUefiBootServicesHeaderIntegrity<'occurrence> {
    pub const fn layout(&self) -> &ValidatedUefiBootServicesNativeLayout {
        &self.layout
    }
    pub fn header_bytes(&self) -> &'occurrence [u8] {
        &self.table_bytes[..self.layout.table_header_size() as usize]
    }
    pub fn known_prefix_bytes(&self) -> &'occurrence [u8] {
        &self.table_bytes[..self.layout.known_prefix_byte_size() as usize]
    }
    pub const fn table_bytes(&self) -> &'occurrence [u8] {
        self.table_bytes
    }
    pub const fn revision(&self) -> u32 {
        self.revision
    }
    pub const fn header_size(&self) -> u32 {
        self.table_bytes.len() as u32
    }
    pub const fn stored_crc32(&self) -> u32 {
        self.stored_crc32
    }
}

#[derive(Debug)]
#[must_use = "UEFI Boot Services occurrence rejection retains layout and borrowed input"]
pub struct UefiBootServicesOccurrenceValidationError<'occurrence> {
    layout: ValidatedUefiBootServicesNativeLayout,
    supplied_bytes: &'occurrence [u8],
    diagnostic: Diagnostic,
}

impl<'occurrence> UefiBootServicesOccurrenceValidationError<'occurrence> {
    pub const fn layout(&self) -> &ValidatedUefiBootServicesNativeLayout {
        &self.layout
    }
    pub const fn supplied_bytes(&self) -> &'occurrence [u8] {
        self.supplied_bytes
    }
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }
    pub fn into_parts(
        self,
    ) -> (
        ValidatedUefiBootServicesNativeLayout,
        &'occurrence [u8],
        Diagnostic,
    ) {
        (self.layout, self.supplied_bytes, self.diagnostic)
    }
}
impl std::fmt::Display for UefiBootServicesOccurrenceValidationError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}
impl std::error::Error for UefiBootServicesOccurrenceValidationError<'_> {}

pub fn validate_uefi_boot_services_occurrence<'occurrence>(
    layout: ValidatedUefiBootServicesNativeLayout,
    supplied_bytes: &'occurrence [u8],
) -> Result<
    ValidatedUefiBootServicesHeaderIntegrity<'occurrence>,
    Box<UefiBootServicesOccurrenceValidationError<'occurrence>>,
> {
    let fields = match header_fields(&layout) {
        Ok(fields) => fields,
        Err(diagnostic) => return reject(layout, supplied_bytes, diagnostic),
    };
    let prefix = layout.known_prefix_byte_size() as usize;
    if supplied_bytes.len() < layout.table_header_size() as usize {
        return reject(
            layout,
            supplied_bytes,
            Diagnostic::error(
                "UEFI Boot Services occurrence is too short to inspect EFI_TABLE_HEADER",
            ),
        );
    }
    let signature = read_u64(supplied_bytes, fields[0]);
    if signature != UEFI_BOOT_SERVICES_SIGNATURE {
        return reject(
            layout,
            supplied_bytes,
            Diagnostic::error(format!(
                "UEFI Boot Services signature is {signature:#018x}; expected {UEFI_BOOT_SERVICES_SIGNATURE:#018x}"
            )),
        );
    }
    let revision = read_u32(supplied_bytes, fields[1]);
    let header_size = read_u32(supplied_bytes, fields[2]) as usize;
    if header_size < prefix {
        return reject(
            layout,
            supplied_bytes,
            Diagnostic::error(format!(
                "UEFI Boot Services HeaderSize is {header_size} bytes; it does not cover the {prefix}-byte target prefix"
            )),
        );
    }
    if header_size > supplied_bytes.len() {
        return reject(
            layout,
            supplied_bytes,
            Diagnostic::error(format!(
                "UEFI Boot Services HeaderSize is {header_size} bytes but only {} occurrence bytes were supplied",
                supplied_bytes.len()
            )),
        );
    }
    if read_u32(supplied_bytes, fields[4]) != 0 {
        return reject(
            layout,
            supplied_bytes,
            Diagnostic::error("UEFI Boot Services header Reserved field is nonzero"),
        );
    }
    let stored_crc32 = read_u32(supplied_bytes, fields[3]);
    let computed_crc32 = crc32_with_zeroed_field(
        &supplied_bytes[..header_size],
        fields[3].byte_offset() as usize,
    );
    if stored_crc32 != computed_crc32 {
        return reject(
            layout,
            supplied_bytes,
            Diagnostic::error(format!(
                "UEFI Boot Services CRC32 is {stored_crc32:#010x}; computed {computed_crc32:#010x}"
            )),
        );
    }
    Ok(ValidatedUefiBootServicesHeaderIntegrity {
        layout,
        table_bytes: &supplied_bytes[..header_size],
        revision,
        stored_crc32,
    })
}

fn header_fields(
    layout: &ValidatedUefiBootServicesNativeLayout,
) -> Result<[UefiBootServicesNativeFieldLayout; 5], Diagnostic> {
    use UefiBootServicesNativeField as Field;
    use UefiBootServicesNativeFieldKind as Kind;
    let fields = [
        required(layout, Field::HeaderSignature)?,
        required(layout, Field::HeaderRevision)?,
        required(layout, Field::HeaderSize)?,
        required(layout, Field::HeaderCrc32)?,
        required(layout, Field::HeaderReserved)?,
    ];
    let expected = [
        (0, 8, Kind::UnsignedInteger),
        (8, 4, Kind::UnsignedInteger),
        (12, 4, Kind::UnsignedInteger),
        (16, 4, Kind::UnsignedInteger),
        (20, 4, Kind::ReservedZero),
    ];
    if fields
        .iter()
        .copied()
        .zip(expected)
        .any(|(field, (offset, size, kind))| {
            field.byte_offset() != offset || field.byte_size() != size || field.kind() != kind
        })
    {
        return Err(Diagnostic::error(
            "UEFI Boot Services occurrence cannot join a drifted EFI_TABLE_HEADER layout",
        ));
    }
    Ok(fields)
}

fn required(
    layout: &ValidatedUefiBootServicesNativeLayout,
    field: UefiBootServicesNativeField,
) -> Result<UefiBootServicesNativeFieldLayout, Diagnostic> {
    layout
        .field_layout(field)
        .ok_or_else(|| Diagnostic::error(format!("UEFI Boot Services layout is missing {field:?}")))
}

fn reject<'occurrence>(
    layout: ValidatedUefiBootServicesNativeLayout,
    supplied_bytes: &'occurrence [u8],
    diagnostic: Diagnostic,
) -> Result<
    ValidatedUefiBootServicesHeaderIntegrity<'occurrence>,
    Box<UefiBootServicesOccurrenceValidationError<'occurrence>>,
> {
    Err(Box::new(UefiBootServicesOccurrenceValidationError {
        layout,
        supplied_bytes,
        diagnostic,
    }))
}

fn read_u32(bytes: &[u8], field: UefiBootServicesNativeFieldLayout) -> u32 {
    let offset = field.byte_offset() as usize;
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
fn read_u64(bytes: &[u8], field: UefiBootServicesNativeFieldLayout) -> u64 {
    let offset = field.byte_offset() as usize;
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}
fn crc32_with_zeroed_field(bytes: &[u8], crc32_offset: usize) -> u32 {
    let mut crc = u32::MAX;
    for (offset, byte) in bytes.iter().copied().enumerate() {
        let byte = if (crc32_offset..crc32_offset + 4).contains(&offset) {
            0
        } else {
            byte
        };
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let low_bit_mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (CRC32_POLYNOMIAL_REFLECTED & low_bit_mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TargetProfile, plan_uefi_boot_services_native_layout};

    fn layout() -> ValidatedUefiBootServicesNativeLayout {
        plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap()
    }
    fn valid_occurrence(header_size: usize) -> Vec<u8> {
        let mut bytes = vec![0; header_size];
        bytes[0..8].copy_from_slice(&UEFI_BOOT_SERVICES_SIGNATURE.to_le_bytes());
        bytes[8..12].copy_from_slice(&((2_u32 << 16) | 100).to_le_bytes());
        bytes[12..16].copy_from_slice(&(header_size as u32).to_le_bytes());
        for offset in (24..header_size).step_by(8) {
            bytes[offset..offset + 8].copy_from_slice(&(0x1000_u64 + offset as u64).to_le_bytes());
        }
        let crc = crc32_with_zeroed_field(&bytes, 16);
        bytes[16..20].copy_from_slice(&crc.to_le_bytes());
        bytes
    }

    #[test]
    fn validates_exact_and_forward_compatible_boot_services_occurrences() {
        for size in [376, 384] {
            let bytes = valid_occurrence(size);
            let integrity = validate_uefi_boot_services_occurrence(layout(), &bytes).unwrap();
            assert_eq!(integrity.header_size(), size as u32);
            assert_eq!(integrity.known_prefix_bytes().len(), 376);
            assert_eq!(integrity.table_bytes(), bytes);
        }
    }

    #[test]
    fn rejects_signature_coverage_reserved_and_crc_drift() {
        let mut bytes = valid_occurrence(376);
        bytes[0] ^= 1;
        assert!(
            validate_uefi_boot_services_occurrence(layout(), &bytes)
                .unwrap_err()
                .diagnostic()
                .message
                .contains("signature")
        );

        let mut bytes = valid_occurrence(376);
        bytes[12..16].copy_from_slice(&375_u32.to_le_bytes());
        assert!(
            validate_uefi_boot_services_occurrence(layout(), &bytes)
                .unwrap_err()
                .diagnostic()
                .message
                .contains("does not cover")
        );

        let mut bytes = valid_occurrence(376);
        bytes[20] = 1;
        assert!(
            validate_uefi_boot_services_occurrence(layout(), &bytes)
                .unwrap_err()
                .diagnostic()
                .message
                .contains("Reserved")
        );

        let mut bytes = valid_occurrence(376);
        bytes[152] ^= 1;
        assert!(
            validate_uefi_boot_services_occurrence(layout(), &bytes)
                .unwrap_err()
                .diagnostic()
                .message
                .contains("CRC32")
        );
    }
}
