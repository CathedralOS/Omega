//! Restricted build-host request recovery under the shared reader budget.

use super::super::selected_providers::target;
use super::Error;
use crate::encoding::recovery::policy::reader::Reader;
use crate::record::PackagePolicyEvaluationSponsorLimits;
use crate::record::PackagePolicyFilesystemSponsorLimits;
use crate::record::PackagePolicyRestrictedBuildBounds;
use crate::record::PackagePolicyRestrictedBuildGrant;
use crate::record::PackagePolicyRestrictedBuildGrantRoot;
use crate::record::PackagePolicyRestrictedBuildOperation;
use crate::record::PackagePolicyRestrictedBuildRequest;

/// operation tag + two grant sequence counts + bounds (two option tags, one
/// output sequence count, artifact flag) + two profile option tags.
const REQUEST_MINIMUM_BYTES: usize = 1 + 8 + 8 + 11 + 1 + 1;

/// root tag + narrowed flag + captured flag (`other` adds its own u32).
const GRANT_MINIMUM_BYTES: usize = 3;

/// length-prefixed byte string contributes its usize length alone.
const REQUIRED_OUTPUT_MINIMUM_BYTES: usize = 8;

pub(super) fn request(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyRestrictedBuildRequest, Error> {
    Ok(PackagePolicyRestrictedBuildRequest {
        operation: match reader.byte()? {
            0 => PackagePolicyRestrictedBuildOperation::ScopedFilesystemExecution,
            1 => PackagePolicyRestrictedBuildOperation::UnscopedFilesystemExecution,
            _ => return Err(Error::InvalidTag),
        },
        read_grants: reader.sequence(GRANT_MINIMUM_BYTES, grant)?,
        write_grants: reader.sequence(GRANT_MINIMUM_BYTES, grant)?,
        bounds: bounds(reader)?,
        build_execution_profile: reader.option(target)?,
        selected_target_profile: reader.option(target)?,
    })
}

fn grant(reader: &mut Reader<'_>) -> Result<PackagePolicyRestrictedBuildGrant, Error> {
    Ok(PackagePolicyRestrictedBuildGrant {
        root: match reader.byte()? {
            0 => PackagePolicyRestrictedBuildGrantRoot::SourceInventory,
            1 => PackagePolicyRestrictedBuildGrantRoot::StagedOutput,
            2 => PackagePolicyRestrictedBuildGrantRoot::Other(reader.u32()?),
            _ => return Err(Error::InvalidTag),
        },
        narrowed: reader.boolean()?,
        captured: reader.boolean()?,
    })
}

fn bounds(reader: &mut Reader<'_>) -> Result<PackagePolicyRestrictedBuildBounds, Error> {
    Ok(PackagePolicyRestrictedBuildBounds {
        filesystem_sponsor_limits: reader.option(|reader| {
            Ok(PackagePolicyFilesystemSponsorLimits {
                maximum_entries: reader.u64()?,
                maximum_total_logical_bytes: reader.u64()?,
                maximum_object_extent: reader.u64()?,
            })
        })?,
        evaluation_sponsor_limits: reader.option(|reader| {
            Ok(PackagePolicyEvaluationSponsorLimits {
                maximum_fuel_units: reader.u64()?,
                maximum_build_log_bytes: reader.u64()?,
                maximum_filesystem_operation_attempts: reader.u64()?,
                maximum_live_filesystem_handles: reader.u64()?,
                maximum_live_cells: reader.u64()?,
                maximum_live_text_bytes: reader.u64()?,
                maximum_result_cells: reader.u64()?,
                maximum_result_text_bytes: reader.u64()?,
            })
        })?,
        required_outputs: reader.sequence(REQUIRED_OUTPUT_MINIMUM_BYTES, Reader::bytes)?,
        artifact_only: reader.boolean()?,
    })
}

pub(super) fn requests(
    reader: &mut Reader<'_>,
) -> Result<Vec<PackagePolicyRestrictedBuildRequest>, Error> {
    reader.sequence(REQUEST_MINIMUM_BYTES, request)
}
