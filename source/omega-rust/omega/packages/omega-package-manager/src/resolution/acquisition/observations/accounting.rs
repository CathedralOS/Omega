use crate::resolution::acquisition::SourceResolveError;
use crate::resolution::acquisition::limits::{
    GIT_CAPTURED_OUTPUT_ABSOLUTE_LIMIT, GIT_CAPTURED_OUTPUT_FIXED_ALLOWANCE,
    GIT_NETWORK_TRANSFER_ABSOLUTE_LIMIT, GIT_NETWORK_TRANSFER_FIXED_ALLOWANCE, LocalSourceLimits,
};
use omega_resolver_execution::{
    ResolverExecutionEndpointOutcome, ResolverExecutionPolicyObservation,
};

use super::GitCommandExecutionObservation;

/// Compiler-owned cumulative stdout/stderr accounting for one Git resolution.
///
/// This observation covers only bytes captured by the parent process. It does
/// not measure network transfer, object-store allocation, or descendant
/// aggregate resources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCapturedOutputObservation {
    pub(in crate::resolution::acquisition) ceiling: u64,
    pub(in crate::resolution::acquisition) observed: u64,
}

impl GitCapturedOutputObservation {
    pub const fn ceiling(&self) -> u64 {
        self.ceiling
    }

    pub const fn observed(&self) -> u64 {
        self.observed
    }
}

/// Compiler-owned bidirectional accounting for bytes accepted by the endpoint
/// broker across one Git resolution. CONNECT framing and DNS traffic are not
/// included. This does not claim that every platform prevents direct helper
/// egress around the broker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitNetworkTransferObservation {
    pub(in crate::resolution::acquisition) ceiling: u64,
    pub(in crate::resolution::acquisition) uploaded: u64,
    pub(in crate::resolution::acquisition) downloaded: u64,
}

impl GitNetworkTransferObservation {
    pub const fn ceiling(&self) -> u64 {
        self.ceiling
    }

    pub const fn uploaded(&self) -> u64 {
        self.uploaded
    }

    pub const fn downloaded(&self) -> u64 {
        self.downloaded
    }

    pub const fn observed(&self) -> u64 {
        self.uploaded + self.downloaded
    }
}

pub(in crate::resolution::acquisition) fn git_resolution_captured_output_ceiling(
    limits: LocalSourceLimits,
) -> u64 {
    limits
        .max_bytes
        .saturating_add(GIT_CAPTURED_OUTPUT_FIXED_ALLOWANCE)
        .min(GIT_CAPTURED_OUTPUT_ABSOLUTE_LIMIT)
}

pub(in crate::resolution::acquisition) fn git_resolution_network_transfer_ceiling(
    limits: LocalSourceLimits,
) -> u64 {
    limits
        .max_bytes
        .saturating_add(GIT_NETWORK_TRANSFER_FIXED_ALLOWANCE)
        .min(GIT_NETWORK_TRANSFER_ABSOLUTE_LIMIT)
}

pub(in crate::resolution::acquisition) fn git_network_transfer_observation(
    policies: &[ResolverExecutionPolicyObservation],
    commands: &[GitCommandExecutionObservation],
    ceiling: u64,
) -> Result<GitNetworkTransferObservation, SourceResolveError> {
    for policy in policies {
        if let Some(route) = policy.endpoint_route()
            && route.transfer_byte_ceiling() != ceiling
        {
            return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                message: "Git endpoint route carries a different transfer ceiling".to_owned(),
            });
        }
    }
    let mut uploaded = 0_u64;
    let mut downloaded = 0_u64;
    for endpoint in commands
        .iter()
        .filter_map(|command| command.endpoint_observation.as_ref())
    {
        for event in endpoint.events() {
            if event.outcome() == ResolverExecutionEndpointOutcome::TransferCeilingReached {
                return Err(SourceResolveError::GitResolutionNetworkTransferCeiling { ceiling });
            }
            uploaded = uploaded
                .checked_add(event.uploaded_bytes())
                .ok_or_else(|| SourceResolveError::GitExecutionBoundaryInvalid {
                    message: "Git upload accounting overflowed".to_owned(),
                })?;
            downloaded = downloaded
                .checked_add(event.downloaded_bytes())
                .ok_or_else(|| SourceResolveError::GitExecutionBoundaryInvalid {
                    message: "Git download accounting overflowed".to_owned(),
                })?;
        }
    }
    let observed = uploaded.checked_add(downloaded).ok_or_else(|| {
        SourceResolveError::GitExecutionBoundaryInvalid {
            message: "Git network-transfer accounting overflowed".to_owned(),
        }
    })?;
    if observed > ceiling {
        return Err(SourceResolveError::GitExecutionBoundaryInvalid {
            message: "Git network-transfer observations exceed their compiler ceiling".to_owned(),
        });
    }
    Ok(GitNetworkTransferObservation {
        ceiling,
        uploaded,
        downloaded,
    })
}

pub(in crate::resolution::acquisition) fn git_captured_output_observation(
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
