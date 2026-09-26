//! Restricted build-host request meaning, in grant order.
//!
//! The captured-inventory presence flag is retained; the inventory's entry
//! counts and identity are not — they bind source content, and folding them
//! into the row would demand fresh acceptance on every source edit even when
//! the requested authority is unchanged.

use super::super::encoder::Encoder;
use crate::encoding::PackageReviewEncodingError;
use crate::record::PackagePolicyEvaluationSponsorLimits;
use crate::record::PackagePolicyFilesystemSponsorLimits;
use crate::record::PackagePolicyRestrictedBuildBounds;
use crate::record::PackagePolicyRestrictedBuildGrant;
use crate::record::PackagePolicyRestrictedBuildGrantRoot;
use crate::record::PackagePolicyRestrictedBuildOperation;
use crate::record::PackagePolicyRestrictedBuildRequest;
use target::TargetProfile;

/// The operation's closed-vocabulary spelling and tag; the row key reuses it
/// so a request's identity and its retained value agree.
pub(in crate::encoding::encode) fn operation(
    operation: PackagePolicyRestrictedBuildOperation,
) -> (&'static str, u8) {
    match operation {
        PackagePolicyRestrictedBuildOperation::ScopedFilesystemExecution => {
            ("scoped_filesystem_execution", 0)
        }
        PackagePolicyRestrictedBuildOperation::UnscopedFilesystemExecution => {
            ("unscoped_filesystem_execution", 1)
        }
    }
}

pub(in crate::encoding::encode) fn request(
    encoder: &mut Encoder,
    request: &PackagePolicyRestrictedBuildRequest,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("operation", |encoder| {
        let (name, tag) = operation(request.operation);
        encoder.tag(name, tag);
        Ok(())
    })?;
    encoder.field("read_grants", |encoder| {
        encoder.sequence(&request.read_grants, grant)
    })?;
    encoder.field("write_grants", |encoder| {
        encoder.sequence(&request.write_grants, grant)
    })?;
    encoder.field("bounds", |encoder| bounds(encoder, &request.bounds))?;
    encoder.field("build_execution_profile", |encoder| {
        profile(encoder, request.build_execution_profile)
    })?;
    encoder.field("selected_target_profile", |encoder| {
        profile(encoder, request.selected_target_profile)
    })
}

fn grant(
    encoder: &mut Encoder,
    grant: &PackagePolicyRestrictedBuildGrant,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("root", |encoder| {
        match grant.root {
            PackagePolicyRestrictedBuildGrantRoot::SourceInventory => {
                encoder.tag("source_inventory", 0)
            }
            PackagePolicyRestrictedBuildGrantRoot::StagedOutput => encoder.tag("staged_output", 1),
            PackagePolicyRestrictedBuildGrantRoot::Other(identity) => {
                encoder.tag("other", 2);
                encoder.field("identity", |encoder| {
                    encoder.u32(identity);
                    Ok(())
                })?;
            }
        }
        Ok(())
    })?;
    encoder.field("narrowed", |encoder| {
        encoder.boolean(grant.narrowed);
        Ok(())
    })?;
    encoder.field("captured", |encoder| {
        encoder.boolean(grant.captured);
        Ok(())
    })
}

fn bounds(
    encoder: &mut Encoder,
    bounds: &PackagePolicyRestrictedBuildBounds,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("filesystem_sponsor_limits", |encoder| {
        encoder.option(
            bounds.filesystem_sponsor_limits.as_ref(),
            filesystem_sponsor_limits,
        )
    })?;
    encoder.field("evaluation_sponsor_limits", |encoder| {
        encoder.option(
            bounds.evaluation_sponsor_limits.as_ref(),
            evaluation_sponsor_limits,
        )
    })?;
    encoder.field("required_outputs", |encoder| {
        encoder.sequence(&bounds.required_outputs, |encoder, output| {
            encoder.bytes(output)
        })
    })?;
    encoder.field("artifact_only", |encoder| {
        encoder.boolean(bounds.artifact_only);
        Ok(())
    })
}

fn filesystem_sponsor_limits(
    encoder: &mut Encoder,
    limits: &PackagePolicyFilesystemSponsorLimits,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("maximum_entries", |encoder| {
        encoder.u64(limits.maximum_entries);
        Ok(())
    })?;
    encoder.field("maximum_total_logical_bytes", |encoder| {
        encoder.u64(limits.maximum_total_logical_bytes);
        Ok(())
    })?;
    encoder.field("maximum_object_extent", |encoder| {
        encoder.u64(limits.maximum_object_extent);
        Ok(())
    })
}

fn evaluation_sponsor_limits(
    encoder: &mut Encoder,
    limits: &PackagePolicyEvaluationSponsorLimits,
) -> Result<(), PackageReviewEncodingError> {
    for (name, value) in [
        ("maximum_fuel_units", limits.maximum_fuel_units),
        ("maximum_build_log_bytes", limits.maximum_build_log_bytes),
        (
            "maximum_filesystem_operation_attempts",
            limits.maximum_filesystem_operation_attempts,
        ),
        (
            "maximum_live_filesystem_handles",
            limits.maximum_live_filesystem_handles,
        ),
        ("maximum_live_cells", limits.maximum_live_cells),
        ("maximum_live_text_bytes", limits.maximum_live_text_bytes),
        ("maximum_result_cells", limits.maximum_result_cells),
        (
            "maximum_result_text_bytes",
            limits.maximum_result_text_bytes,
        ),
    ] {
        encoder.field(name, |encoder| {
            encoder.u64(value);
            Ok(())
        })?;
    }
    Ok(())
}

fn profile(
    encoder: &mut Encoder,
    profile: Option<TargetProfile>,
) -> Result<(), PackageReviewEncodingError> {
    encoder.option(profile.as_ref(), |encoder, profile| {
        encoder.string(profile.identity().as_str())
    })
}
