//! Exact Linux System-V `u32` countdown in the canonical incoming `edi` home.

use psi_diagnostics::Diagnostic;

pub const X86_64_RANKED_U32_COUNTDOWN_BYTE_COUNT: usize = 21;
pub const X86_64_RANKED_U32_PREHEADER_BRANCH_OFFSET: usize = 0;
pub const X86_64_RANKED_U32_PREHEADER_BRANCH_BYTE_COUNT: usize = 5;
pub const X86_64_RANKED_U32_HEADER_OFFSET: usize = 5;
pub const X86_64_RANKED_U32_COMPARE_OFFSET: usize = 5;
pub const X86_64_RANKED_U32_COMPARE_BYTE_COUNT: usize = 2;
pub const X86_64_RANKED_U32_EXIT_BRANCH_OFFSET: usize = 7;
pub const X86_64_RANKED_U32_EXIT_BRANCH_BYTE_COUNT: usize = 6;
pub const X86_64_RANKED_U32_POSITIVE_PATH_OFFSET: usize = 13;
pub const X86_64_RANKED_U32_DECREMENT_OFFSET: usize = 13;
pub const X86_64_RANKED_U32_DECREMENT_BYTE_COUNT: usize = 2;
pub const X86_64_RANKED_U32_BACKWARD_BRANCH_OFFSET: usize = 15;
pub const X86_64_RANKED_U32_BACKWARD_BRANCH_BYTE_COUNT: usize = 5;
pub const X86_64_RANKED_U32_EXIT_OFFSET: usize = 20;
pub const X86_64_RANKED_U32_RETURN_OFFSET: usize = 20;
pub const X86_64_RANKED_U32_RETURN_BYTE_COUNT: usize = 1;

/// Target-owned byte layout recovered by independently validating the exact
/// ranked countdown encoding. Its private construction prevents unvalidated
/// bytes from acquiring layout evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64RankedU32CountdownLayout {
    _private: (),
}

impl X86_64RankedU32CountdownLayout {
    pub const fn preheader_branch(self) -> (usize, usize) {
        (
            X86_64_RANKED_U32_PREHEADER_BRANCH_OFFSET,
            X86_64_RANKED_U32_PREHEADER_BRANCH_BYTE_COUNT,
        )
    }

    pub const fn header_offset(self) -> usize {
        X86_64_RANKED_U32_HEADER_OFFSET
    }

    pub const fn compare(self) -> (usize, usize) {
        (
            X86_64_RANKED_U32_COMPARE_OFFSET,
            X86_64_RANKED_U32_COMPARE_BYTE_COUNT,
        )
    }

    pub const fn positive_path_offset(self) -> usize {
        X86_64_RANKED_U32_POSITIVE_PATH_OFFSET
    }

    pub const fn decrement(self) -> (usize, usize) {
        (
            X86_64_RANKED_U32_DECREMENT_OFFSET,
            X86_64_RANKED_U32_DECREMENT_BYTE_COUNT,
        )
    }

    pub const fn backward_branch(self) -> (usize, usize) {
        (
            X86_64_RANKED_U32_BACKWARD_BRANCH_OFFSET,
            X86_64_RANKED_U32_BACKWARD_BRANCH_BYTE_COUNT,
        )
    }

    pub const fn exit_offset(self) -> usize {
        X86_64_RANKED_U32_EXIT_OFFSET
    }

    pub const fn return_instruction(self) -> (usize, usize) {
        (
            X86_64_RANKED_U32_RETURN_OFFSET,
            X86_64_RANKED_U32_RETURN_BYTE_COUNT,
        )
    }
}

/// Opaque evidence that one byte slice is exactly the canonical Linux
/// System-V ranked `u32` countdown and that all three branches target their
/// required sites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64RankedU32CountdownEncoding {
    bytes: [u8; X86_64_RANKED_U32_COUNTDOWN_BYTE_COUNT],
    layout: X86_64RankedU32CountdownLayout,
}

impl ValidatedX86_64RankedU32CountdownEncoding {
    pub const fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn layout(&self) -> X86_64RankedU32CountdownLayout {
        self.layout
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64RankedU32CountdownEncodingError {
    WrongByteCount,
    MalformedInstruction,
    PreheaderTargetMismatch,
    ExitTargetMismatch,
    BackedgeTargetMismatch,
}

impl std::fmt::Display for X86_64RankedU32CountdownEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid x86-64 ranked u32 countdown: {self:?}")
    }
}

impl std::error::Error for X86_64RankedU32CountdownEncodingError {}

/// Emit one fixed-width, relocation-free countdown body.
///
/// The entry branch makes the preheader a distinct executable site. The
/// backward branch targets the header at byte 5, so it cannot replay the
/// preheader site. Near branches are intentional: later fuel rebasing can
/// preserve one stable source encoding while changing their displacements.
pub const fn encode_ranked_u32_countdown_in_edi() -> [u8; X86_64_RANKED_U32_COUNTDOWN_BYTE_COUNT] {
    [
        0xe9, 0x00, 0x00, 0x00, 0x00, // jmp header
        0x85, 0xff, // test edi, edi
        0x0f, 0x84, 0x07, 0x00, 0x00, 0x00, // je exit
        0xff, 0xcf, // dec edi
        0xe9, 0xf1, 0xff, 0xff, 0xff, // jmp header
        0xc3, // ret
    ]
}

/// Independently decode and validate the exact countdown. This deliberately
/// does not compare against [`encode_ranked_u32_countdown_in_edi`]: opcode,
/// register, width, and branch-target checks are replayed from the bytes.
pub fn validate_x86_64_ranked_u32_countdown_in_edi(
    bytes: &[u8],
) -> Result<ValidatedX86_64RankedU32CountdownEncoding, X86_64RankedU32CountdownEncodingError> {
    use X86_64RankedU32CountdownEncodingError as Error;

    let bytes: &[u8; X86_64_RANKED_U32_COUNTDOWN_BYTE_COUNT] =
        bytes.try_into().map_err(|_| Error::WrongByteCount)?;
    if bytes[0] != 0xe9
        || bytes[5..7] != [0x85, 0xff]
        || bytes[7..9] != [0x0f, 0x84]
        || bytes[13..15] != [0xff, 0xcf]
        || bytes[15] != 0xe9
        || bytes[20] != 0xc3
    {
        return Err(Error::MalformedInstruction);
    }
    let relative_target = |field: std::ops::Range<usize>, next_instruction: i64| {
        let displacement = i32::from_le_bytes(
            bytes[field]
                .try_into()
                .expect("a ranked x86-64 rel32 field is four bytes"),
        );
        next_instruction + i64::from(displacement)
    };
    if relative_target(1..5, 5) != X86_64_RANKED_U32_HEADER_OFFSET as i64 {
        return Err(Error::PreheaderTargetMismatch);
    }
    if relative_target(9..13, 13) != X86_64_RANKED_U32_EXIT_OFFSET as i64 {
        return Err(Error::ExitTargetMismatch);
    }
    if relative_target(16..20, 20) != X86_64_RANKED_U32_HEADER_OFFSET as i64 {
        return Err(Error::BackedgeTargetMismatch);
    }
    Ok(ValidatedX86_64RankedU32CountdownEncoding {
        bytes: *bytes,
        layout: X86_64RankedU32CountdownLayout { _private: () },
    })
}

/// Rebase only the three canonical relative fields after a caller expands the
/// semantic layout with independently owned instrumentation.
pub fn rebase_ranked_u32_countdown_branches(
    bytes: &mut [u8],
    preheader_offset: usize,
    header_offset: usize,
    exit_branch_offset: usize,
    exit_offset: usize,
    backward_branch_offset: usize,
) -> Result<(), Diagnostic> {
    patch_rel32(bytes, preheader_offset, 5, 1, header_offset, &[0xe9])?;
    patch_rel32(bytes, exit_branch_offset, 6, 2, exit_offset, &[0x0f, 0x84])?;
    patch_rel32(bytes, backward_branch_offset, 5, 1, header_offset, &[0xe9])
}

fn patch_rel32(
    bytes: &mut [u8],
    instruction_offset: usize,
    instruction_size: usize,
    immediate_offset: usize,
    target_offset: usize,
    opcode: &[u8],
) -> Result<(), Diagnostic> {
    if bytes.get(instruction_offset..instruction_offset + opcode.len()) != Some(opcode) {
        return Err(Diagnostic::error("ranked x86-64 branch opcode drifted"));
    }
    let origin = instruction_offset
        .checked_add(instruction_size)
        .ok_or_else(|| Diagnostic::error("ranked x86-64 branch origin overflowed"))?;
    let distance = isize::try_from(target_offset)
        .ok()
        .and_then(|target| isize::try_from(origin).ok().map(|origin| target - origin))
        .and_then(|distance| i32::try_from(distance).ok())
        .ok_or_else(|| Diagnostic::error("ranked x86-64 branch is out of rel32 range"))?;
    let immediate = instruction_offset
        .checked_add(immediate_offset)
        .ok_or_else(|| Diagnostic::error("ranked x86-64 branch field overflowed"))?;
    bytes
        .get_mut(immediate..immediate + 4)
        .ok_or_else(|| Diagnostic::error("ranked x86-64 branch field is outside code"))?
        .copy_from_slice(&distance.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranked_countdown_branches_target_header_and_exit() {
        let bytes = encode_ranked_u32_countdown_in_edi();
        let preheader = i32::from_le_bytes(bytes[1..5].try_into().unwrap());
        assert_eq!(
            5_i64 + i64::from(preheader),
            X86_64_RANKED_U32_HEADER_OFFSET as i64
        );
        let exit = i32::from_le_bytes(bytes[9..13].try_into().unwrap());
        assert_eq!(
            13_i64 + i64::from(exit),
            X86_64_RANKED_U32_EXIT_OFFSET as i64
        );
        let backedge = i32::from_le_bytes(bytes[16..20].try_into().unwrap());
        assert_eq!(
            20_i64 + i64::from(backedge),
            X86_64_RANKED_U32_HEADER_OFFSET as i64
        );
        let validated = validate_x86_64_ranked_u32_countdown_in_edi(&bytes).unwrap();
        assert_eq!(validated.bytes(), bytes);
        assert_eq!(validated.layout().header_offset(), 5);
    }

    #[test]
    fn decoder_rejects_instruction_and_register_mutations() {
        for index in [0, 5, 6, 7, 8, 13, 14, 15, 20] {
            let mut bytes = encode_ranked_u32_countdown_in_edi();
            bytes[index] ^= 1;
            assert_eq!(
                validate_x86_64_ranked_u32_countdown_in_edi(&bytes),
                Err(X86_64RankedU32CountdownEncodingError::MalformedInstruction)
            );
        }
    }

    #[test]
    fn decoder_rejects_each_wrong_branch_target() {
        let mut preheader = encode_ranked_u32_countdown_in_edi();
        preheader[1..5].copy_from_slice(&1_i32.to_le_bytes());
        assert_eq!(
            validate_x86_64_ranked_u32_countdown_in_edi(&preheader),
            Err(X86_64RankedU32CountdownEncodingError::PreheaderTargetMismatch)
        );

        let mut exit = encode_ranked_u32_countdown_in_edi();
        exit[9..13].copy_from_slice(&6_i32.to_le_bytes());
        assert_eq!(
            validate_x86_64_ranked_u32_countdown_in_edi(&exit),
            Err(X86_64RankedU32CountdownEncodingError::ExitTargetMismatch)
        );

        let mut backedge_to_preheader = encode_ranked_u32_countdown_in_edi();
        backedge_to_preheader[16..20].copy_from_slice(&(-20_i32).to_le_bytes());
        assert_eq!(
            validate_x86_64_ranked_u32_countdown_in_edi(&backedge_to_preheader),
            Err(X86_64RankedU32CountdownEncodingError::BackedgeTargetMismatch)
        );
    }

    #[test]
    fn decoder_rejects_truncated_and_trailing_bytes() {
        let bytes = encode_ranked_u32_countdown_in_edi();
        assert_eq!(
            validate_x86_64_ranked_u32_countdown_in_edi(&bytes[..20]),
            Err(X86_64RankedU32CountdownEncodingError::WrongByteCount)
        );
        let mut trailing = bytes.to_vec();
        trailing.push(0x90);
        assert_eq!(
            validate_x86_64_ranked_u32_countdown_in_edi(&trailing),
            Err(X86_64RankedU32CountdownEncodingError::WrongByteCount)
        );
    }
}
