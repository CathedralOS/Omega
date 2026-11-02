//! Exact single-entry NT directory queries over a captured fixed output.
//!
//! The namespace supplies only this instance's synchronous handles. Optional
//! events, APCs and name-filter pointers stay null; the source issuer retains
//! its private nonalert file object through completion. Actual NT status and
//! IO_STATUS_BLOCK information return unchanged, including end/error results.
//! https://learn.microsoft.com/windows-hardware/drivers/ddi/ntifs/nf-ntifs-ntquerydirectoryfile
//! https://learn.microsoft.com/windows-hardware/drivers/ddi/ntifs/ns-ntifs-_file_names_information
//! https://learn.microsoft.com/windows-hardware/drivers/ddi/ntifs/ns-ntifs-_file_directory_information

use super::super::super::native_provider::IntegerRecordField::{Array, Scalar};
use super::{Effect, Formal, scalar};
use semantic_vocabulary::IntegerSign::{Signed as I, Unsigned as U};
use terminal_interpreter::TerminalScalarValue;
use terminal_psi::StructuralAccess::MutableBorrow;

// Source records retain both leading DWORDs in one complete QWORD. Native
// storage admission rejoins every field/array lane and the chosen geometry;
// record names and equal-sized padding do not establish the contract.
pub(super) const ROWS: &[(&[u8], &[u8], &[Formal], Formal, Effect)] = &[
    (
        b"ntdll.dll",
        b"NtQueryDirectoryFile",
        &[
            Formal::Signed(64),
            Formal::Unsigned(64),
            Formal::Unsigned(64),
            Formal::Unsigned(64),
            Formal::IntegerRecord {
                fields: &[Scalar(I, 64), Scalar(U, 64)],
                access: MutableBorrow,
            },
            Formal::IntegerRecord {
                fields: &[Scalar(U, 64), Scalar(U, 32), Array(U, 16, 4096)],
                access: MutableBorrow,
            },
            Formal::Unsigned(32),
            Formal::Unsigned(32),
            Formal::Unsigned(8),
            Formal::Unsigned(64),
            Formal::Unsigned(8),
        ],
        Formal::Signed(32),
        Effect::Directory {
            class: 12,
            bytes: 8208,
        },
    ),
    (
        b"ntdll.dll",
        b"NtQueryDirectoryFile",
        &[
            Formal::Signed(64),
            Formal::Unsigned(64),
            Formal::Unsigned(64),
            Formal::Unsigned(64),
            Formal::IntegerRecord {
                fields: &[Scalar(I, 64), Scalar(U, 64)],
                access: MutableBorrow,
            },
            Formal::IntegerRecord {
                fields: &[
                    Scalar(U, 64),
                    Scalar(U, 64),
                    Scalar(U, 64),
                    Scalar(U, 64),
                    Scalar(U, 64),
                    Scalar(I, 64),
                    Scalar(I, 64),
                    Scalar(U, 32),
                    Scalar(U, 32),
                    Array(U, 16, 4096),
                ],
                access: MutableBorrow,
            },
            Formal::Unsigned(32),
            Formal::Unsigned(32),
            Formal::Unsigned(8),
            Formal::Unsigned(64),
            Formal::Unsigned(8),
        ],
        Formal::Signed(32),
        Effect::Directory {
            class: 1,
            bytes: 8256,
        },
    ),
];

pub(super) fn arguments(effect: Effect, arguments: &[TerminalScalarValue]) -> bool {
    let Effect::Directory { class, bytes } = effect else {
        return false;
    };
    arguments.len() == 9
        && scalar(arguments[1], 0)
        && scalar(arguments[2], 0)
        && scalar(arguments[3], 0)
        && scalar(arguments[4], i128::from(bytes))
        && scalar(arguments[5], i128::from(class))
        && scalar(arguments[6], 1)
        && scalar(arguments[7], 0)
        && (scalar(arguments[8], 0) || class == 1 && scalar(arguments[8], 1))
}
