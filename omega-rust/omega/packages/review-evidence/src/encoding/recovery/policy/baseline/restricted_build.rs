// Restricted build-host request recovery under the shared reader budget.
// Minimum-encoded-byte constants bound sequence reservation before element decoding: `REQUEST_MINIMUM_BYTES` (operation tag + two grant sequence counts + bounds + two profile option tags), `GRANT_MINIMUM_BYTES` (root tag + narrowed flag + captured flag), and `REQUIRED_OUTPUT_MINIMUM_BYTES` (length-prefixed byte string length).

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

const REQUEST_MINIMUM_BYTES: usize = 1 + 8 + 8 + 11 + 1 + 1;
const GRANT_MINIMUM_BYTES: usize = 3;
const REQUIRED_OUTPUT_MINIMUM_BYTES: usize = 8;

pub(in crate::encoding::recovery) fn request(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyRestrictedBuildRequest, Error> {
    Ok(PackagePolicyRestrictedBuildRequest {
        operation: super::super::tag!(reader => {
            0 => PackagePolicyRestrictedBuildOperation::ScopedFilesystemExecution,
            1 => PackagePolicyRestrictedBuildOperation::UnscopedFilesystemExecution,
        }),
        read_grants: reader.sequence(GRANT_MINIMUM_BYTES, grant)?,
        write_grants: reader.sequence(GRANT_MINIMUM_BYTES, grant)?,
        bounds: bounds(reader)?,
        build_execution_profile: reader.option(target)?,
        selected_target_profile: reader.option(target)?,
    })
}

pub(in crate::encoding::recovery) fn grant(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyRestrictedBuildGrant, Error> {
    Ok(PackagePolicyRestrictedBuildGrant {
        root: super::super::tag!(reader => {
            0 => PackagePolicyRestrictedBuildGrantRoot::SourceInventory,
            1 => PackagePolicyRestrictedBuildGrantRoot::StagedOutput,
            2 => PackagePolicyRestrictedBuildGrantRoot::Other(reader.u32()?),
        }),
        narrowed: reader.boolean()?,
        captured: reader.boolean()?,
    })
}

pub(in crate::encoding::recovery) fn bounds(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyRestrictedBuildBounds, Error> {
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
                work_budget: reader.u64()?,
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

pub(in crate::encoding::recovery) fn requests(
    reader: &mut Reader<'_>,
) -> Result<Vec<PackagePolicyRestrictedBuildRequest>, Error> {
    reader.sequence(REQUEST_MINIMUM_BYTES, request)
}
