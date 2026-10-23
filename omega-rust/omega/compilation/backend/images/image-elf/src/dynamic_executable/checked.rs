//! Checked arithmetic, little-endian field access, and the fixed ELF64 record
//! decodes for the dynamic lane, each failing with a diagnostic that names the
//! context it was checking.
//!
//! Past the first serializing stage, every byte this lane touches is read back
//! through the same three operations: a bounded little-endian field, a
//! width-typed fixup field, or one exact table of fixed-width records. Those
//! layouts and their bound arithmetic live here once, so a decode fix or a
//! tightened bound lands in one place instead of diverging per stage. What a
//! stage *expects* to find in the decoded bytes stays with that stage: this
//! module answers what the bytes say, never whether they are the expected
//! table.
use diagnostics::Diagnostic;

use crate::constants::{
    ELF64_DYNAMIC_ROW_SIZE, ELF64_DYNAMIC_VALUE_OFFSET, ELF64_SECTION_HEADER_SIZE,
};

pub(super) fn require(condition: bool, message: &'static str) -> Result<(), Diagnostic> {
    condition
        .then_some(())
        .ok_or_else(|| Diagnostic::error(message))
}

pub(super) fn checked_sum(
    left: usize,
    right: usize,
    context: &'static str,
) -> Result<usize, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows usize")))
}

pub(super) fn checked_product(
    left: usize,
    right: usize,
    context: &'static str,
) -> Result<usize, Diagnostic> {
    left.checked_mul(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows usize")))
}

pub(super) fn checked_u32(value: usize, context: &'static str) -> Result<u32, Diagnostic> {
    u32::try_from(value).map_err(|_| Diagnostic::error(format!("{context} exceeds Elf64_Word")))
}

fn read_array<const N: usize>(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<[u8; N], Diagnostic> {
    let end = checked_sum(offset, N, context)?;
    bytes
        .get(offset..end)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| Diagnostic::error(format!("truncated {context}")))
}

pub(super) fn read_u8(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<u8, Diagnostic> {
    bytes
        .get(offset)
        .copied()
        .ok_or_else(|| Diagnostic::error(format!("truncated {context}")))
}

pub(super) fn read_u16(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<u16, Diagnostic> {
    Ok(u16::from_le_bytes(read_array::<2>(bytes, offset, context)?))
}

pub(super) fn read_u32(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<u32, Diagnostic> {
    Ok(u32::from_le_bytes(read_array::<4>(bytes, offset, context)?))
}

pub(super) fn read_u64(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<u64, Diagnostic> {
    Ok(u64::from_le_bytes(read_array::<8>(bytes, offset, context)?))
}

pub(super) fn read_i64(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<i64, Diagnostic> {
    Ok(i64::from_le_bytes(read_array::<8>(bytes, offset, context)?))
}

/// Read one fixup field: the unsigned little-endian value of the only widths
/// this lane encodes, refusing a width it does not.
pub(super) fn read_field(
    bytes: &[u8],
    offset: usize,
    width: u8,
    context: &'static str,
) -> Result<u64, Diagnostic> {
    match width {
        4 => Ok(u64::from(read_u32(bytes, offset, context)?)),
        8 => read_u64(bytes, offset, context),
        _ => Err(Diagnostic::error(format!(
            "{context} has an unsupported field width"
        ))),
    }
}

/// The checked mutable span of the field `read_field` reads.
pub(super) fn field_mut<'a>(
    bytes: &'a mut [u8],
    offset: usize,
    width: u8,
    context: &'static str,
) -> Result<&'a mut [u8], Diagnostic> {
    let end = checked_sum(offset, usize::from(width), context)?;
    bytes
        .get_mut(offset..end)
        .ok_or_else(|| Diagnostic::error(format!("{context} exceeds its storage")))
}

pub(super) fn checked_sum_u64(
    left: u64,
    right: u64,
    context: &str,
    bound: &'static str,
) -> Result<u64, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows {bound}")))
}

pub(super) fn checked_product_u64(
    left: u64,
    right: u64,
    context: &str,
    bound: &'static str,
) -> Result<u64, Diagnostic> {
    left.checked_mul(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows {bound}")))
}

/// One decoded `Elf64_Shdr`: the 64-byte record layout the section-header
/// stages serialize, re-read and place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DecodedElf64SectionHeader {
    pub(super) name_offset: u32,
    pub(super) section_type: u32,
    pub(super) flags: u64,
    pub(super) address: u64,
    pub(super) file_offset: u64,
    pub(super) payload_size: u64,
    pub(super) link: u32,
    pub(super) info: u32,
    pub(super) alignment: u64,
    pub(super) entry_size: u64,
}

/// Decode exactly `expected_count` 64-byte `Elf64_Shdr` records from
/// little-endian bytes, refusing a truncated row or trailing bytes.
pub(super) fn decode_section_header_rows(
    bytes: &[u8],
    expected_count: usize,
) -> Result<Vec<DecodedElf64SectionHeader>, Diagnostic> {
    let expected_size = checked_product(
        expected_count,
        ELF64_SECTION_HEADER_SIZE,
        "decoded ELF64 section-header table size",
    )?;
    require(
        bytes.len() == expected_size,
        "ELF64 section-header table has a truncated row or trailing bytes",
    )?;
    let mut rows = Vec::with_capacity(expected_count);
    for index in 0..expected_count {
        let offset = checked_product(
            index,
            ELF64_SECTION_HEADER_SIZE,
            "decoded ELF64 section-header row",
        )?;
        let field =
            |field_offset: usize, context: &'static str| checked_sum(offset, field_offset, context);
        rows.push(DecodedElf64SectionHeader {
            name_offset: read_u32(bytes, offset, "Elf64_Shdr.sh_name")?,
            section_type: read_u32(
                bytes,
                field(4, "Elf64_Shdr.sh_type offset")?,
                "Elf64_Shdr.sh_type",
            )?,
            flags: read_u64(
                bytes,
                field(8, "Elf64_Shdr.sh_flags offset")?,
                "Elf64_Shdr.sh_flags",
            )?,
            address: read_u64(
                bytes,
                field(16, "Elf64_Shdr.sh_addr offset")?,
                "Elf64_Shdr.sh_addr",
            )?,
            file_offset: read_u64(
                bytes,
                field(24, "Elf64_Shdr.sh_offset offset")?,
                "Elf64_Shdr.sh_offset",
            )?,
            payload_size: read_u64(
                bytes,
                field(32, "Elf64_Shdr.sh_size offset")?,
                "Elf64_Shdr.sh_size",
            )?,
            link: read_u32(
                bytes,
                field(40, "Elf64_Shdr.sh_link offset")?,
                "Elf64_Shdr.sh_link",
            )?,
            info: read_u32(
                bytes,
                field(44, "Elf64_Shdr.sh_info offset")?,
                "Elf64_Shdr.sh_info",
            )?,
            alignment: read_u64(
                bytes,
                field(48, "Elf64_Shdr.sh_addralign offset")?,
                "Elf64_Shdr.sh_addralign",
            )?,
            entry_size: read_u64(
                bytes,
                field(56, "Elf64_Shdr.sh_entsize offset")?,
                "Elf64_Shdr.sh_entsize",
            )?,
        });
    }
    Ok(rows)
}

/// One decoded `Elf64_Dyn`: the signed eight-byte tag followed by the
/// eight-byte value/address union.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DecodedElfDynamicRow {
    pub(super) tag: i64,
    pub(super) value: u64,
}

/// Decode exactly `expected_count` 16-byte `Elf64_Dyn` records from
/// little-endian bytes, refusing a truncated row or trailing bytes.
pub(super) fn decode_dynamic_rows(
    bytes: &[u8],
    expected_count: usize,
) -> Result<Vec<DecodedElfDynamicRow>, Diagnostic> {
    let expected_size = checked_product(
        expected_count,
        ELF64_DYNAMIC_ROW_SIZE,
        "decoded Elf64_Dyn payload size",
    )?;
    require(
        bytes.len() == expected_size,
        "Elf64_Dyn payload has a truncated row or trailing bytes",
    )?;
    let mut rows = Vec::with_capacity(expected_count);
    for ordinal in 0..expected_count {
        let offset = checked_product(ordinal, ELF64_DYNAMIC_ROW_SIZE, "decoded Elf64_Dyn row")?;
        rows.push(DecodedElfDynamicRow {
            tag: read_i64(bytes, offset, "Elf64_Dyn.d_tag")?,
            value: read_u64(
                bytes,
                checked_sum(offset, ELF64_DYNAMIC_VALUE_OFFSET, "Elf64_Dyn.d_un offset")?,
                "Elf64_Dyn.d_un",
            )?,
        });
    }
    Ok(rows)
}

/// A rejected candidate together with the diagnostic that rejected it: every
/// dynamic-lane section validator builds this same two-field envelope.
pub(crate) struct CandidateValidationError<C> {
    pub(crate) candidate: C,
    pub(crate) diagnostic: Diagnostic,
}

/// Shared validation envelope: run the contents validator, replay the
/// non-authoritative compatibility fingerprint, and on success assemble the
/// validated record from the consumed candidate.
pub(crate) fn validate_candidate<C, T>(
    candidate: C,
    valid: impl Fn(&C) -> Result<(), Diagnostic>,
    replays: impl Fn(&C) -> bool,
    replay_diagnostic: &'static str,
    validated: impl Fn(C) -> T,
) -> Result<T, CandidateValidationError<C>> {
    if let Err(diagnostic) = valid(&candidate) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    if !replays(&candidate) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error(replay_diagnostic),
        });
    }
    Ok(validated(candidate))
}

/// Shared planning scaffold: derive contents (wrapping a derive failure into
/// the caller's planning error), compute the compatibility fingerprint, build
/// the candidate, validate it, and re-wrap a validation rejection the same
/// way. Every dynamic-lane planner runs this same envelope.
#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_candidate<P, C, D, V, E>(
    plan: P,
    derive_contents: impl Fn(&P) -> Result<D, Diagnostic>,
    fingerprint: impl Fn(&P, &D) -> u64,
    into_candidate: impl FnOnce(P, D, u64) -> C,
    validate: impl FnOnce(C) -> Result<V, CandidateValidationError<C>>,
    into_plan: impl FnOnce(C) -> P,
    error: impl Fn(P, Diagnostic) -> E,
) -> Result<V, E> {
    let contents = match derive_contents(&plan) {
        Ok(contents) => contents,
        Err(diagnostic) => return Err(error(plan, diagnostic)),
    };
    let fingerprint = fingerprint(&plan, &contents);
    let candidate = into_candidate(plan, contents, fingerprint);
    match validate(candidate) {
        Ok(validated) => Ok(validated),
        Err(candidate_error) => Err(error(
            into_plan(candidate_error.candidate),
            candidate_error.diagnostic,
        )),
    }
}
