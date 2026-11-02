//! Exact relative synchronous acquisition and object-bound deletion requests.
//! https://learn.microsoft.com/windows-hardware/drivers/ddi/ntifs/nf-ntifs-ntopenfile
//! https://learn.microsoft.com/windows/win32/api/ntdef/ns-ntdef-_object_attributes
//! https://learn.microsoft.com/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle

use super::super::super::native_provider::IntegerRecordField::{
    ReferenceArray, ReferenceRecord, Scalar,
};
use super::{Effect, Formal, rejection};
use semantic_vocabulary::IntegerSign::{Signed as I, Unsigned as U};
use terminal_interpreter::TerminalEffectRejection;
use terminal_psi::StructuralAccess::{MutableBorrow, SharedBorrow};

pub(super) const ROWS: &[(&[u8], &[u8], &[Formal], Formal, Effect)] = &[
    (
        b"ntdll.dll",
        b"NtOpenFile",
        &[
            Formal::IntegerReference(I, 64, MutableBorrow),
            Formal::Unsigned(32),
            Formal::IntegerRecord {
                fields: &[
                    Scalar(U, 32),
                    Scalar(U, 32),
                    Scalar(I, 64),
                    ReferenceRecord(
                        &[
                            Scalar(U, 16),
                            Scalar(U, 16),
                            Scalar(U, 32),
                            ReferenceArray(U, 16, 4096, SharedBorrow),
                        ],
                        SharedBorrow,
                    ),
                    Scalar(U, 32),
                    Scalar(U, 32),
                    Scalar(U, 64),
                    Scalar(U, 64),
                ],
                access: SharedBorrow,
            },
            Formal::IntegerRecord {
                fields: &[Scalar(I, 64), Scalar(U, 64)],
                access: MutableBorrow,
            },
            Formal::Unsigned(32),
            Formal::Unsigned(32),
        ],
        Formal::Signed(32),
        Effect::OpenRelative,
    ),
    (
        b"kernel32.dll",
        b"SetFileInformationByHandle",
        &[
            Formal::Signed(64),
            Formal::Unsigned(32),
            Formal::IntegerRecord {
                fields: &[Scalar(U, 8)],
                access: SharedBorrow,
            },
            Formal::Unsigned(32),
        ],
        Formal::Signed(32),
        Effect::Disposition,
    ),
];

/// Translate only the root HANDLE in a private native ABI header. The input
/// header and both captured reference endpoints remain unchanged and alive.
pub(super) fn translate_attributes(
    record: &super::super::super::native_provider::ReadonlyRecord<'_>,
    translate: impl FnOnce(i64) -> Result<i64, TerminalEffectRejection>,
) -> Result<Option<Vec<u8>>, TerminalEffectRejection> {
    let invalid = || rejection("relative NT open changed its complete object-attributes contract");
    let name = record.sole_reference()?;
    let characters = name.sole_reference()?;
    if name.bytes().len() != 16 || characters.bytes().len() != 8192 {
        return Err(invalid());
    }
    let length = u16::from_le_bytes(name.bytes()[0..2].try_into().unwrap());
    let capacity = u16::from_le_bytes(name.bytes()[2..4].try_into().unwrap());
    if length == 0 || length > 8190 || length % 2 != 0 || capacity != 8192 {
        return Err(invalid());
    }
    let mut bytes = record.bytes().to_vec();
    if bytes.len() != 48 {
        return Err(invalid());
    }
    let u32_at = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    let u64_at = |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
    if u32_at(0) != 48 || u32_at(24) != 4096 || u64_at(32) != 0 || u64_at(40) != 0 {
        return Err(invalid());
    }
    let guest = i64::from_le_bytes(bytes[8..16].try_into().unwrap());
    if guest == 0 || guest == -1 {
        return Err(invalid());
    }
    bytes[8..16].copy_from_slice(&translate(guest)?.to_le_bytes());
    Ok(Some(bytes))
}
