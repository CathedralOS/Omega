//! Checked arithmetic and little-endian reads for the dynamic lane, each
//! failing with a diagnostic that names the context it was checking.
use diagnostics::Diagnostic;

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

pub(super) fn read_u64(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<u64, Diagnostic> {
    Ok(u64::from_le_bytes(read_array::<8>(bytes, offset, context)?))
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

pub(super) fn read_i64(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<i64, Diagnostic> {
    Ok(i64::from_le_bytes(read_array::<8>(bytes, offset, context)?))
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
