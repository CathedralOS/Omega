// We implement Windows kernel32 time exports (QueryPerformanceCounter,
// QueryPerformanceFrequency, GetSystemTimePreciseAsFileTime, and Sleep) over
// the shared clock environment. Counter and frequency write one signed 64-bit
// word at 10^9 Hz and return a nonzero BOOL; FILETIME writes 100-nanosecond
// intervals since 1601-01-01 UTC; and Sleep waits finite milliseconds while
// rejecting INFINITE (0xFFFFFFFF).

use std::sync::{Arc, Mutex};

use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
use target::NormalizedForeignLocator;
use terminal_interpreter::{
    TerminalBoundaryStorageBuffer, TerminalEffect, TerminalEffectRejection, TerminalEffectResult,
    TerminalImportSignature, TerminalProgram,
};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralTypeShape};

use super::super::borrowed_storage::{CapturedStorage, StorageTransfer};
use super::{ClockImplementation, ClockNamespace, TerminalClockEnvironment, TerminalRuntimeError};

pub(super) const MONOTONIC: u128 = 1;
const REALTIME: u128 = 0;

const NANOSECONDS_PER_SECOND: i64 = 1_000_000_000;
const FILETIME_UNIX_EPOCH_INTERVALS: u64 = 11_644_473_600 * 10_000_000;
const INFINITE: u128 = 0xFFFF_FFFF;

#[derive(Clone, Copy)]
pub(super) enum WindowsWord {
    Counter,
    Frequency,
    FileTime,
}

impl WindowsWord {
    fn carrier(self) -> Option<IntegerType> {
        let sign = match self {
            Self::Counter | Self::Frequency => IntegerSign::Signed,
            Self::FileTime => IntegerSign::Unsigned,
        };
        IntegerType::new(sign, 64).ok()
    }
}

pub(super) fn select(
    locator: &NormalizedForeignLocator,
) -> Result<Option<(ClockNamespace, ClockImplementation)>, TerminalRuntimeError> {
    for (export, implementation) in [
        (
            b"QueryPerformanceCounter".as_slice(),
            ClockImplementation::WindowsWord(WindowsWord::Counter),
        ),
        (
            b"QueryPerformanceFrequency".as_slice(),
            ClockImplementation::WindowsWord(WindowsWord::Frequency),
        ),
        (
            b"GetSystemTimePreciseAsFileTime".as_slice(),
            ClockImplementation::WindowsWord(WindowsWord::FileTime),
        ),
        (b"Sleep".as_slice(), ClockImplementation::WindowsSleep),
    ] {
        let offered = target::normalize_foreign_locator(
            target::ForeignLocatorCandidate::PeByName {
                library: b"kernel32.dll".to_vec(),
                export: export.to_vec(),
            },
            locator.target(),
        )
        .map_err(|_| {
            TerminalRuntimeError::InvalidProvision("clock implementation has an invalid locator")
        })?;
        if offered == *locator {
            return Ok(Some((ClockNamespace::Windows, implementation)));
        }
    }
    Ok(None)
}

pub(super) fn word_loan_valid(
    program: &TerminalProgram,
    signature: &TerminalImportSignature,
    word: WindowsWord,
) -> bool {
    let [formal] = signature.structural_parameters.as_slice() else {
        return false;
    };
    let carrier = word.carrier().map(ScalarType::Integer);
    let shape = program
        .structural_type(formal.structural_type)
        .map(|row| &row.shape);
    matches!(shape, Some(StructuralTypeShape::PrimitiveScalar(scalar)) if Some(*scalar) == carrier)
        && formal.access == StructuralAccess::MutableBorrow
        && formal.multiplicity == StructuralMultiplicity::Unrestricted
        && formal.qualifications.is_empty()
        && formal.projected_qualifications.is_empty()
}

pub(super) fn write_word(
    effect: &TerminalEffect,
    buffers: &mut [TerminalBoundaryStorageBuffer],
    signature: &TerminalImportSignature,
    clock: &Arc<Mutex<TerminalClockEnvironment>>,
    word: WindowsWord,
) -> Result<TerminalEffectResult, TerminalEffectRejection> {
    let invalid = super::super::refusal("Windows time operands differ from the installed contract");
    let call = super::super::call_operands(effect, invalid)?;
    let [formal] = signature.structural_parameters.as_slice() else {
        return Err(invalid());
    };
    let carrier = word
        .carrier()
        .map(ScalarType::Integer)
        .ok_or_else(invalid)?;
    match buffers {
        [TerminalBoundaryStorageBuffer::Scalar(buffer)]
            if buffer.value().scalar_type() == carrier => {}
        _ => return Err(invalid()),
    }
    if !call.arguments.is_empty()
        || call.structural_arguments.len() != 1
        || call.structural_arguments[0].structural_type != formal.structural_type
        || !matches!(call.byte_sequence_arguments, [None])
    {
        return Err(invalid());
    }
    let mut storage = CapturedStorage::capture(
        0,
        buffers,
        StructuralAccess::MutableBorrow,
        None,
        8,
        StorageTransfer::ReplacePrefix,
    )?;
    let reading = |selector| {
        clock
            .lock()
            .map_err(|_| TerminalEffectRejection::new("clock snapshot lock is poisoned"))?
            .reading(ClockNamespace::Windows, selector)
    };
    let bytes = match word {
        WindowsWord::Counter => i64::try_from(reading(MONOTONIC)?)
            .map_err(|_| TerminalEffectRejection::new("performance counter exceeds i64"))?
            .to_le_bytes(),
        WindowsWord::Frequency => NANOSECONDS_PER_SECOND.to_le_bytes(),
        WindowsWord::FileTime => (reading(REALTIME)? / 100)
            .checked_add(FILETIME_UNIX_EPOCH_INTERVALS)
            .ok_or_else(|| TerminalEffectRejection::new("FILETIME exceeds u64"))?
            .to_le_bytes(),
    };
    let result = super::status_result(signature, 1)?;
    storage
        .bytes
        .get_mut(..8)
        .ok_or_else(invalid)?
        .copy_from_slice(&bytes);
    storage.stage(buffers, 8)?;
    Ok(result)
}

pub(super) fn sleep_nanoseconds(milliseconds: u128) -> Result<u64, TerminalEffectRejection> {
    if milliseconds == INFINITE {
        return Err(TerminalEffectRejection::new(
            "Sleep(INFINITE) requires an indefinite-wait model",
        ));
    }
    u64::try_from(milliseconds)
        .ok()
        .and_then(|milliseconds| milliseconds.checked_mul(1_000_000))
        .ok_or_else(|| TerminalEffectRejection::new("Sleep duration exceeds modeled nanoseconds"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kernel32(export: &str) -> NormalizedForeignLocator {
        target::normalize_foreign_locator(
            target::ForeignLocatorCandidate::PeByName {
                library: b"kernel32.dll".to_vec(),
                export: export.as_bytes().to_vec(),
            },
            target::TargetProfile::WindowsX64,
        )
        .unwrap()
    }

    #[test]
    fn std_time_host_exports_are_claimed_by_the_clock_model() {
        for export in [
            "QueryPerformanceCounter",
            "QueryPerformanceFrequency",
            "GetSystemTimePreciseAsFileTime",
            "Sleep",
        ] {
            assert!(
                matches!(
                    select(&kernel32(export)),
                    Ok(Some((ClockNamespace::Windows, _)))
                ),
                "{export}"
            );
        }
        assert!(matches!(select(&kernel32("GetTickCount64")), Ok(None)));
    }

    #[test]
    fn sleep_waits_finite_milliseconds_only() {
        assert_eq!(sleep_nanoseconds(30).ok(), Some(30_000_000));
        assert!(sleep_nanoseconds(INFINITE).is_err());
    }
}
