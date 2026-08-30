use super::execution::GitCommandExecutionObservation;
use crate::SourceResolveError;
use crate::limits::{
    GIT_CAPTURED_OUTPUT_ABSOLUTE_LIMIT, GIT_CAPTURED_OUTPUT_FIXED_ALLOWANCE, LocalSourceLimits,
};

/// Compiler-owned cumulative stdout/stderr accounting for one Git resolution.
///
/// This observation covers only bytes captured by the parent process. It does
/// not measure network transfer, object-store allocation, or descendant
/// aggregate resources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCapturedOutputObservation {
    pub(crate) ceiling: u64,
    pub(crate) observed: u64,
}

impl GitCapturedOutputObservation {
    pub const fn ceiling(&self) -> u64 {
        self.ceiling
    }

    pub const fn observed(&self) -> u64 {
        self.observed
    }
}

pub(crate) fn git_resolution_captured_output_ceiling(limits: LocalSourceLimits) -> u64 {
    limits
        .max_bytes
        .saturating_add(GIT_CAPTURED_OUTPUT_FIXED_ALLOWANCE)
        .min(GIT_CAPTURED_OUTPUT_ABSOLUTE_LIMIT)
}

pub(crate) fn git_captured_output_observation(
    commands: &[GitCommandExecutionObservation],
    ceiling: u64,
) -> Result<GitCapturedOutputObservation, SourceResolveError> {
    let observed = commands.iter().try_fold(0_u64, |total, command| {
        total
            .checked_add(command.stdout_length)
            .and_then(|total| total.checked_add(command.stderr_length))
            .ok_or_else(|| SourceResolveError::GitExecutionBoundaryInvalid {
                message: "Git captured-output accounting overflowed".to_owned(),
            })
    })?;
    if observed > ceiling {
        return Err(SourceResolveError::GitExecutionBoundaryInvalid {
            message: "Git captured-output observations exceed their compiler ceiling".to_owned(),
        });
    }
    Ok(GitCapturedOutputObservation { ceiling, observed })
}
