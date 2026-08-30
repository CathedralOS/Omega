//! Non-authorizing validation of one borrowed UEFI system-table occurrence.
//!
//! The [UEFI table-header definition][header] requires the system-table
//! signature, a zero reserved field, and a CRC over the runtime `HeaderSize`
//! extent with the stored CRC field treated as zero. This module joins those
//! checks to the target-owned x86-64 layout without interpreting or granting
//! authority to any pointer-shaped field in the table.
//!
//! A validated header-integrity carrier is therefore non-authenticating CRC
//! consistency and geometry evidence only. It is not pointer provenance,
//! firmware-lifecycle evidence, a provider, or permission to dereference
//! `ConOut`, `BootServices`, or any other field. Any bytes beyond the known
//! target prefix are CRC-covered but remain structurally opaque.
//!
//! [header]: https://uefi.org/specs/UEFI/2.11/04_EFI_System_Table.html#efi-table-header

use crate::{
    UefiSystemTableNativeField, UefiSystemTableNativeFieldKind, UefiSystemTableNativeFieldLayout,
    ValidatedUefiSystemTableNativeLayout,
};
use psi_diagnostics::Diagnostic;

/// Signature fixed by the UEFI specification for `EFI_SYSTEM_TABLE`.
pub const UEFI_SYSTEM_TABLE_SIGNATURE: u64 = 0x5453_5953_2049_4249;

const U32_SIZE: usize = size_of::<u32>();
const U64_SIZE: usize = size_of::<u64>();
const CRC32_POLYNOMIAL_REFLECTED: u32 = 0xedb8_8320;

#[derive(Clone, Copy)]
struct HeaderGeometry {
    signature: UefiSystemTableNativeFieldLayout,
    revision: UefiSystemTableNativeFieldLayout,
    header_size: UefiSystemTableNativeFieldLayout,
    crc32: UefiSystemTableNativeFieldLayout,
    reserved: UefiSystemTableNativeFieldLayout,
}

/// Integrity-checked borrowed bytes joined to the target-owned UEFI x86-64
/// system-table layout.
///
/// This carrier deliberately exposes only header metadata and byte views. It
/// does not project pointer fields or confer provider, lifecycle, bootstrap,
/// or execution authority.
#[derive(Debug)]
#[must_use = "validated UEFI system-table occurrence retains borrowed integrity evidence"]
pub struct ValidatedUefiSystemTableHeaderIntegrity<'occurrence> {
    layout: ValidatedUefiSystemTableNativeLayout,
    table_bytes: &'occurrence [u8],
    revision: u32,
    stored_crc32: u32,
}

impl<'occurrence> ValidatedUefiSystemTableHeaderIntegrity<'occurrence> {
    pub const fn layout(&self) -> &ValidatedUefiSystemTableNativeLayout {
        &self.layout
    }

    /// The exact parsed and CRC-covered `EFI_TABLE_HEADER` prefix. Revision is
    /// retained as metadata rather than checked against a global minimum.
    pub fn header_bytes(&self) -> &'occurrence [u8] {
        &self.table_bytes[..self.layout.table_header_size() as usize]
    }

    /// The target-owned system-table prefix covered by the runtime header.
    pub fn known_prefix_bytes(&self) -> &'occurrence [u8] {
        &self.table_bytes[..self.layout.known_prefix_byte_size() as usize]
    }

    /// The exact table extent covered by the verified runtime CRC.
    pub const fn table_bytes(&self) -> &'occurrence [u8] {
        self.table_bytes
    }

    /// The runtime revision retained for later capability-specific checks.
    /// This validation intentionally imposes no global minimum revision.
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

/// Rejected occurrence validation with both the target layout and borrowed
/// input retained for inspection or retry.
#[derive(Debug)]
#[must_use = "UEFI system-table occurrence rejection retains its layout and borrowed input"]
pub struct UefiSystemTableOccurrenceValidationError<'occurrence> {
    layout: ValidatedUefiSystemTableNativeLayout,
    supplied_bytes: &'occurrence [u8],
    diagnostic: Diagnostic,
}

impl<'occurrence> UefiSystemTableOccurrenceValidationError<'occurrence> {
    pub const fn layout(&self) -> &ValidatedUefiSystemTableNativeLayout {
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
        ValidatedUefiSystemTableNativeLayout,
        &'occurrence [u8],
        Diagnostic,
    ) {
        (self.layout, self.supplied_bytes, self.diagnostic)
    }
}

impl std::fmt::Display for UefiSystemTableOccurrenceValidationError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiSystemTableOccurrenceValidationError<'_> {}

/// Validate one accessible byte occurrence against a target-owned UEFI
/// x86-64 system-table layout.
///
/// `HeaderSize` may exceed the currently known 120-byte prefix, and all such
/// extension bytes participate in the CRC. Supplied trailing bytes beyond
/// `HeaderSize` are outside the successful carrier; a rejection instead
/// retains the exact complete input for inspection or retry.
pub fn validate_uefi_system_table_occurrence<'occurrence>(
    layout: ValidatedUefiSystemTableNativeLayout,
    supplied_bytes: &'occurrence [u8],
) -> Result<
    ValidatedUefiSystemTableHeaderIntegrity<'occurrence>,
    Box<UefiSystemTableOccurrenceValidationError<'occurrence>>,
> {
    let geometry = match header_geometry(&layout) {
        Ok(geometry) => geometry,
        Err(diagnostic) => {
            return Err(Box::new(UefiSystemTableOccurrenceValidationError {
                layout,
                supplied_bytes,
                diagnostic,
            }));
        }
    };
    let minimum_header_size = layout.table_header_size() as usize;
    if supplied_bytes.len() < minimum_header_size {
        return reject(
            layout,
            supplied_bytes,
            format!(
                "UEFI system-table occurrence has {} bytes; at least {minimum_header_size} bytes are required to inspect EFI_TABLE_HEADER",
                supplied_bytes.len()
            ),
        );
    }

    let signature = read_u64(supplied_bytes, geometry.signature);
    if signature != UEFI_SYSTEM_TABLE_SIGNATURE {
        return reject(
            layout,
            supplied_bytes,
            format!(
                "UEFI system-table occurrence has signature {signature:#018x}; expected EFI_SYSTEM_TABLE signature {UEFI_SYSTEM_TABLE_SIGNATURE:#018x}"
            ),
        );
    }

    let revision = read_u32(supplied_bytes, geometry.revision);
    let header_size = read_u32(supplied_bytes, geometry.header_size);
    let table_byte_len = header_size as usize;
    let known_prefix_byte_size = layout.known_prefix_byte_size() as usize;
    if table_byte_len < known_prefix_byte_size {
        return reject(
            layout,
            supplied_bytes,
            format!(
                "UEFI system-table HeaderSize is {header_size} bytes; it does not cover the target-owned {known_prefix_byte_size}-byte prefix"
            ),
        );
    }
    if table_byte_len > supplied_bytes.len() {
        return reject(
            layout,
            supplied_bytes,
            format!(
                "UEFI system-table HeaderSize is {header_size} bytes but only {} occurrence bytes were supplied",
                supplied_bytes.len()
            ),
        );
    }

    let reserved = read_u32(supplied_bytes, geometry.reserved);
    if reserved != 0 {
        return reject(
            layout,
            supplied_bytes,
            format!("UEFI system-table header Reserved field is {reserved:#010x}; expected zero"),
        );
    }

    let stored_crc32 = read_u32(supplied_bytes, geometry.crc32);
    let computed_crc32 = system_table_crc32(
        &supplied_bytes[..table_byte_len],
        geometry.crc32.byte_offset() as usize,
    );
    if stored_crc32 != computed_crc32 {
        return reject(
            layout,
            supplied_bytes,
            format!(
                "UEFI system-table CRC32 is {stored_crc32:#010x}; computed {computed_crc32:#010x} over HeaderSize {header_size} with the CRC32 field zeroed"
            ),
        );
    }

    Ok(ValidatedUefiSystemTableHeaderIntegrity {
        layout,
        table_bytes: &supplied_bytes[..table_byte_len],
        revision,
        stored_crc32,
    })
}

fn header_geometry(
    layout: &ValidatedUefiSystemTableNativeLayout,
) -> Result<HeaderGeometry, Diagnostic> {
    use UefiSystemTableNativeField as Field;

    let geometry = HeaderGeometry {
        signature: required_field(layout, Field::HeaderSignature)?,
        revision: required_field(layout, Field::HeaderRevision)?,
        header_size: required_field(layout, Field::HeaderSize)?,
        crc32: required_field(layout, Field::HeaderCrc32)?,
        reserved: required_field(layout, Field::HeaderReserved)?,
    };
    validate_header_geometry(layout, geometry)?;
    Ok(geometry)
}

fn validate_header_geometry(
    layout: &ValidatedUefiSystemTableNativeLayout,
    geometry: HeaderGeometry,
) -> Result<(), Diagnostic> {
    use UefiSystemTableNativeFieldKind as Kind;

    let rows = [
        (geometry.signature, U64_SIZE, Kind::UnsignedInteger),
        (geometry.revision, U32_SIZE, Kind::UnsignedInteger),
        (geometry.header_size, U32_SIZE, Kind::UnsignedInteger),
        (geometry.crc32, U32_SIZE, Kind::UnsignedInteger),
        (geometry.reserved, U32_SIZE, Kind::ReservedZero),
    ];
    let mut next_offset = 0_usize;
    for (row, byte_size, kind) in rows {
        if row.byte_offset() as usize != next_offset
            || row.byte_size() as usize != byte_size
            || row.kind() != kind
        {
            return Err(Diagnostic::error(
                "UEFI system-table occurrence validation cannot join a drifted EFI_TABLE_HEADER field layout",
            ));
        }
        next_offset += byte_size;
    }
    if next_offset != layout.table_header_size() as usize {
        return Err(Diagnostic::error(
            "UEFI system-table occurrence validation cannot join a layout whose EFI_TABLE_HEADER rows do not exactly cover its header extent",
        ));
    }
    Ok(())
}

fn required_field(
    layout: &ValidatedUefiSystemTableNativeLayout,
    field: UefiSystemTableNativeField,
) -> Result<UefiSystemTableNativeFieldLayout, Diagnostic> {
    layout.field_layout(field).ok_or_else(|| {
        Diagnostic::error(format!(
            "UEFI system-table occurrence validation cannot join a layout missing {field:?}"
        ))
    })
}

fn reject<'occurrence>(
    layout: ValidatedUefiSystemTableNativeLayout,
    supplied_bytes: &'occurrence [u8],
    message: String,
) -> Result<
    ValidatedUefiSystemTableHeaderIntegrity<'occurrence>,
    Box<UefiSystemTableOccurrenceValidationError<'occurrence>>,
> {
    Err(Box::new(UefiSystemTableOccurrenceValidationError {
        layout,
        supplied_bytes,
        diagnostic: Diagnostic::error(message),
    }))
}

fn read_u32(bytes: &[u8], field: UefiSystemTableNativeFieldLayout) -> u32 {
    let offset = field.byte_offset() as usize;
    u32::from_le_bytes(bytes[offset..offset + U32_SIZE].try_into().unwrap())
}

fn read_u64(bytes: &[u8], field: UefiSystemTableNativeFieldLayout) -> u64 {
    let offset = field.byte_offset() as usize;
    u64::from_le_bytes(bytes[offset..offset + U64_SIZE].try_into().unwrap())
}

fn system_table_crc32(table_bytes: &[u8], crc32_offset: usize) -> u32 {
    crc32_ieee(table_bytes.iter().enumerate().map(|(offset, byte)| {
        if (crc32_offset..crc32_offset + U32_SIZE).contains(&offset) {
            0
        } else {
            *byte
        }
    }))
}

fn crc32_ieee(bytes: impl IntoIterator<Item = u8>) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
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
    use crate::{plan_uefi_system_table_native_layout, TargetProfile};

    const TEST_REVISION: u32 = (2 << 16) | 100;

    fn layout() -> ValidatedUefiSystemTableNativeLayout {
        plan_uefi_system_table_native_layout(TargetProfile::UefiX64).unwrap()
    }

    fn geometry() -> HeaderGeometry {
        let layout = layout();
        header_geometry(&layout).unwrap()
    }

    fn field_range(field: UefiSystemTableNativeFieldLayout) -> std::ops::Range<usize> {
        let start = field.byte_offset() as usize;
        start..start + field.byte_size() as usize
    }

    fn valid_occurrence(header_size: usize, supplied_size: usize) -> Vec<u8> {
        assert!(header_size >= 120);
        assert!(supplied_size >= header_size);
        let geometry = geometry();
        let mut bytes = vec![0; supplied_size];
        bytes[field_range(geometry.signature)]
            .copy_from_slice(&UEFI_SYSTEM_TABLE_SIGNATURE.to_le_bytes());
        bytes[field_range(geometry.revision)].copy_from_slice(&TEST_REVISION.to_le_bytes());
        bytes[field_range(geometry.header_size)]
            .copy_from_slice(&(header_size as u32).to_le_bytes());
        for (offset, byte) in bytes[24..header_size].iter_mut().enumerate() {
            *byte = (offset as u8).wrapping_mul(17).wrapping_add(3);
        }
        let crc = system_table_crc32(&bytes[..header_size], geometry.crc32.byte_offset() as usize);
        bytes[field_range(geometry.crc32)].copy_from_slice(&crc.to_le_bytes());
        bytes
    }

    fn rejection(bytes: &[u8]) -> Box<UefiSystemTableOccurrenceValidationError<'_>> {
        validate_uefi_system_table_occurrence(layout(), bytes).unwrap_err()
    }

    #[test]
    fn crc_implementation_matches_the_standard_check_value() {
        assert_eq!(crc32_ieee(*b"123456789"), 0xcbf4_3926);
        let bytes = valid_occurrence(120, 120);
        assert_eq!(read_u32(&bytes, geometry().crc32), 0x481f_d9ca);
    }

    #[test]
    fn header_geometry_replay_rejects_a_non_header_row() {
        let layout = layout();
        let mut geometry = header_geometry(&layout).unwrap();
        geometry.reserved = layout
            .field_layout(UefiSystemTableNativeField::ConsoleOut)
            .unwrap();
        assert!(validate_header_geometry(&layout, geometry)
            .unwrap_err()
            .message
            .contains("drifted EFI_TABLE_HEADER field layout"));
    }

    #[test]
    fn validates_the_known_prefix_without_granting_field_projections() {
        let bytes = valid_occurrence(120, 120);
        let validated = validate_uefi_system_table_occurrence(layout(), &bytes).unwrap();
        assert_eq!(validated.layout().profile(), TargetProfile::UefiX64);
        assert_eq!(validated.revision(), TEST_REVISION);
        assert_eq!(validated.header_size(), 120);
        assert_eq!(validated.header_bytes(), &bytes[..24]);
        assert_eq!(validated.known_prefix_bytes(), &bytes[..120]);
        assert_eq!(validated.table_bytes(), bytes);
        assert_eq!(validated.stored_crc32(), read_u32(&bytes, geometry().crc32));
        assert!(std::ptr::eq(
            validated.table_bytes().as_ptr(),
            bytes.as_ptr()
        ));
    }

    #[test]
    fn accepts_and_checks_a_forward_compatible_header_extent() {
        let bytes = valid_occurrence(136, 144);
        let validated = validate_uefi_system_table_occurrence(layout(), &bytes).unwrap();
        assert_eq!(validated.header_size(), 136);
        assert_eq!(validated.table_bytes(), &bytes[..136]);
        assert_eq!(validated.header_bytes(), &bytes[..24]);
        assert_eq!(validated.known_prefix_bytes(), &bytes[..120]);

        let mut trailing_mutation = bytes.clone();
        trailing_mutation[143] ^= 1;
        let trailing_validated =
            validate_uefi_system_table_occurrence(layout(), &trailing_mutation).unwrap();
        assert_eq!(trailing_validated.table_bytes(), &trailing_mutation[..136]);

        let mut corrupt = bytes;
        corrupt[135] ^= 1;
        assert!(rejection(&corrupt)
            .diagnostic()
            .message
            .contains("computed"));
    }

    #[test]
    fn rejects_short_header_and_wrong_signature_recoverably() {
        let short = vec![0; 23];
        let error = rejection(&short);
        assert!(error.diagnostic().message.contains("at least 24 bytes"));
        let (returned_layout, returned_bytes, diagnostic) = error.into_parts();
        assert_eq!(returned_layout.profile(), TargetProfile::UefiX64);
        assert!(std::ptr::eq(returned_bytes.as_ptr(), short.as_ptr()));
        assert!(diagnostic.message.contains("at least 24 bytes"));

        let mut wrong_signature = valid_occurrence(120, 120);
        wrong_signature[0] ^= 1;
        assert!(rejection(&wrong_signature)
            .diagnostic()
            .message
            .contains("expected EFI_SYSTEM_TABLE signature"));
    }

    #[test]
    fn rejects_header_size_below_prefix_or_beyond_supplied_bytes() {
        let mut below_prefix = valid_occurrence(120, 120);
        below_prefix[field_range(geometry().header_size)].copy_from_slice(&119_u32.to_le_bytes());
        assert!(rejection(&below_prefix)
            .diagnostic()
            .message
            .contains("does not cover"));

        let mut beyond_supplied = valid_occurrence(120, 120);
        beyond_supplied[field_range(geometry().header_size)]
            .copy_from_slice(&121_u32.to_le_bytes());
        assert!(rejection(&beyond_supplied)
            .diagnostic()
            .message
            .contains("only 120 occurrence bytes were supplied"));

        let mut header_only = vec![0; 24];
        header_only[field_range(geometry().signature)]
            .copy_from_slice(&UEFI_SYSTEM_TABLE_SIGNATURE.to_le_bytes());
        header_only[field_range(geometry().header_size)].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(rejection(&header_only)
            .diagnostic()
            .message
            .contains("only 24 occurrence bytes were supplied"));
    }

    #[test]
    fn rejects_nonzero_reserved_field_and_crc_mismatch() {
        let mut nonzero_reserved = valid_occurrence(120, 120);
        nonzero_reserved[geometry().reserved.byte_offset() as usize] = 1;
        assert!(rejection(&nonzero_reserved)
            .diagnostic()
            .message
            .contains("expected zero"));

        let mut crc_mismatch = valid_occurrence(120, 120);
        crc_mismatch[64] ^= 1;
        assert!(rejection(&crc_mismatch)
            .diagnostic()
            .message
            .contains("CRC32"));
    }
}
