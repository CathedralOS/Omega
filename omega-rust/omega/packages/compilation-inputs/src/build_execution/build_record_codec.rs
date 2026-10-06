//! Wire codec for the build-execution records a retained compiled review
//! carries: the evaluation usage the sponsor observed, the restricted build
//! requests the authority projection issued, and the observation summary the
//! activation produced.
//!
//! These values were produced by checked machinery before they entered a
//! retained review record, so recovery validates shape (enum tags, nonzero
//! identities, canonical paths) without re-running admission. Anything
//! malformed or unexpected fails the decode and the caller treats the record
//! as absent.

use crate::review_record_encoding::{
    declaration_kind_tag, encode_optional, recover_declaration_kind, recover_optional,
};
use filesystem_authority::observation_codec::{ObservationCursor as Cursor, push_framed};
use filesystem_authority::{
    FilesystemSponsorLimits, canonical_filesystem_metadata_path_is_canonical, observation_codec,
};

use evaluation_accounting::{
    BuildEvaluationSponsorLimits, BuildEvaluationUsage, EvaluationMeasurementIdentity,
    TerminalStorageScheduleIdentity,
};
use semantic_vocabulary::FuelScheduleIdentity;

// Untagged legacy usage had no way to distinguish two engines' schedule 1.
const EVALUATION_USAGE_MAGIC: &[u8; 4] = b"BEU\x01";

use super::observations::{
    BuildActivation, BuildCanonicalSourceMetadataIdentity, BuildCapturedSourceInventory,
    BuildFilesystemAuthorizedPath, BuildFilesystemOperationAttempt, BuildFilesystemRoot,
    BuildIncludedSourceHandoff, BuildNamedInputInventory, BuildObservationSummary,
    BuildRequiredOutputSettlement,
};
use super::restricted_requests::{
    RestrictedBuildBounds, RestrictedBuildGrant, RestrictedBuildGrantRoot,
    RestrictedBuildOperation, RestrictedBuildRequest,
};
use build_output::{
    BuildStagedOutputEntryInput, BuildStagedOutputEntryKind, BuildStagedOutputTree,
    BuildStagedOutputTreeCommitment,
};
use semantic_vocabulary::PackageKeyIdentity;

/// Encode the evaluation-usage record a retained review carries.
pub fn encode_build_evaluation_usage(usage: &BuildEvaluationUsage, out: &mut Vec<u8>) {
    out.extend_from_slice(EVALUATION_USAGE_MAGIC);
    match usage.measurement {
        EvaluationMeasurementIdentity::CheckedTree => out.push(0),
        EvaluationMeasurementIdentity::TerminalPsi {
            fuel_schedule,
            storage_schedule,
        } => {
            out.push(1);
            out.extend_from_slice(&fuel_schedule.marker().to_le_bytes());
            out.extend_from_slice(&storage_schedule.marker().to_le_bytes());
        }
    }
    out.extend_from_slice(&usage.usage_schema_version.to_le_bytes());
    out.extend_from_slice(&usage.step_schedule_marker.to_le_bytes());
    out.extend_from_slice(&usage.invocation_fuel_ceiling.to_le_bytes());
    encode_optional(usage.sponsor_schema_version.as_ref(), out, |value, out| {
        out.extend_from_slice(&value.to_le_bytes())
    });
    for value in [
        usage.session_fuel_ceiling,
        usage.session_build_log_byte_ceiling,
        usage.session_filesystem_attempt_ceiling,
        usage.session_live_filesystem_handle_ceiling,
        usage.session_live_storage_cell_ceiling,
        usage.session_live_payload_byte_ceiling,
        usage.session_result_storage_cell_ceiling,
        usage.session_result_payload_byte_ceiling,
    ] {
        encode_optional(value.as_ref(), out, |value, out| {
            out.extend_from_slice(&value.to_le_bytes())
        });
    }
    for value in [
        usage.session_peak_live_filesystem_handles,
        usage.session_peak_live_storage_cells,
        usage.session_peak_live_payload_bytes,
        usage.fuel_units,
        usage.build_log_bytes,
        usage.filesystem_operation_attempts,
        usage.peak_live_storage_cells,
        usage.peak_live_payload_bytes,
        usage.result_storage_cells,
        usage.result_payload_bytes,
    ] {
        out.extend_from_slice(&value.to_le_bytes());
    }
}

/// Recover a retained evaluation-usage record.
pub fn recover_build_evaluation_usage(cursor: &mut Cursor) -> Option<BuildEvaluationUsage> {
    if cursor.bytes(4)? != EVALUATION_USAGE_MAGIC {
        return None;
    }
    let measurement = match cursor.u8()? {
        0 => EvaluationMeasurementIdentity::CheckedTree,
        1 => EvaluationMeasurementIdentity::TerminalPsi {
            fuel_schedule: FuelScheduleIdentity::new(cursor.u32()?)?,
            storage_schedule: TerminalStorageScheduleIdentity::new(cursor.u32()?)?,
        },
        _ => return None,
    };
    let usage_schema_version = cursor.u32()?;
    let step_schedule_marker = cursor.u32()?;
    let invocation_fuel_ceiling = cursor.u64()?;
    let sponsor_schema_version = recover_optional(cursor, Cursor::u32)?;
    let session_fuel_ceiling = recover_optional(cursor, Cursor::u64)?;
    let session_build_log_byte_ceiling = recover_optional(cursor, Cursor::u64)?;
    let session_filesystem_attempt_ceiling = recover_optional(cursor, Cursor::u64)?;
    let session_live_filesystem_handle_ceiling = recover_optional(cursor, Cursor::u64)?;
    let session_live_storage_cell_ceiling = recover_optional(cursor, Cursor::u64)?;
    let session_live_payload_byte_ceiling = recover_optional(cursor, Cursor::u64)?;
    let session_result_storage_cell_ceiling = recover_optional(cursor, Cursor::u64)?;
    let session_result_payload_byte_ceiling = recover_optional(cursor, Cursor::u64)?;
    let usage = BuildEvaluationUsage {
        measurement,
        usage_schema_version,
        step_schedule_marker,
        invocation_fuel_ceiling,
        sponsor_schema_version,
        session_fuel_ceiling,
        session_build_log_byte_ceiling,
        session_filesystem_attempt_ceiling,
        session_live_filesystem_handle_ceiling,
        session_live_storage_cell_ceiling,
        session_live_payload_byte_ceiling,
        session_result_storage_cell_ceiling,
        session_result_payload_byte_ceiling,
        session_peak_live_filesystem_handles: cursor.u64()?,
        session_peak_live_storage_cells: cursor.u64()?,
        session_peak_live_payload_bytes: cursor.u64()?,
        fuel_units: cursor.u64()?,
        build_log_bytes: cursor.u64()?,
        filesystem_operation_attempts: cursor.u64()?,
        peak_live_storage_cells: cursor.u64()?,
        peak_live_payload_bytes: cursor.u64()?,
        result_storage_cells: cursor.u64()?,
        result_payload_bytes: cursor.u64()?,
    };
    usage.validate().ok()?;
    Some(usage)
}

fn encode_source_metadata_identity(
    identity: &BuildCanonicalSourceMetadataIdentity,
    out: &mut Vec<u8>,
) {
    out.extend_from_slice(&identity.policy_version().to_le_bytes());
    out.extend_from_slice(&identity.source_content_commitment());
}

fn recover_source_metadata_identity(
    cursor: &mut Cursor,
) -> Option<BuildCanonicalSourceMetadataIdentity> {
    let policy_version = cursor.u32()?;
    let commitment: [u8; 32] = cursor.bytes(32)?.try_into().ok()?;
    Some(BuildCanonicalSourceMetadataIdentity::new(
        policy_version,
        commitment,
    ))
}

fn encode_captured_inventory(inventory: &BuildCapturedSourceInventory, out: &mut Vec<u8>) {
    out.extend_from_slice(&inventory.entry_count().to_le_bytes());
    out.extend_from_slice(&inventory.file_bytes().to_le_bytes());
    encode_source_metadata_identity(&inventory.source_metadata_identity(), out);
}

fn recover_captured_inventory(cursor: &mut Cursor) -> Option<BuildCapturedSourceInventory> {
    let entry_count = cursor.u64()?;
    let file_bytes = cursor.u64()?;
    let source_metadata_identity = recover_source_metadata_identity(cursor)?;
    Some(BuildCapturedSourceInventory {
        entry_count,
        file_bytes,
        source_metadata_identity,
    })
}

fn encode_staged_output_tree(tree: &BuildStagedOutputTree, out: &mut Vec<u8>) {
    let commitment = tree.commitment();
    out.extend_from_slice(&commitment.digest());
    out.extend_from_slice(&commitment.entry_count().to_le_bytes());
    out.extend_from_slice(&commitment.file_bytes().to_le_bytes());
    let entries: Vec<_> = tree.entries().collect();
    out.extend_from_slice(&(entries.len() as u64).to_le_bytes());
    for entry in entries {
        push_framed(out, entry.relative_path());
        match entry.kind() {
            BuildStagedOutputEntryKind::Directory => out.push(0),
            BuildStagedOutputEntryKind::File { bytes, executable } => {
                out.push(1);
                push_framed(out, bytes);
                out.push(executable as u8);
            }
            BuildStagedOutputEntryKind::Symlink { target } => {
                out.push(2);
                push_framed(out, target);
            }
        }
    }
}

fn recover_staged_output_tree(cursor: &mut Cursor) -> Option<BuildStagedOutputTree> {
    let digest: [u8; 32] = cursor.bytes(32)?.try_into().ok()?;
    let entry_count = cursor.u64()?;
    let file_bytes = cursor.u64()?;
    let commitment =
        BuildStagedOutputTreeCommitment::from_retained_parts(digest, entry_count, file_bytes);
    let entries = cursor_count(cursor)?;
    let mut inputs = Vec::with_capacity(entries);
    for _ in 0..entries {
        let relative_path = cursor.framed()?.to_vec();
        let input = match cursor.u8()? {
            0 => BuildStagedOutputEntryInput::Directory,
            1 => {
                let bytes = cursor.framed()?.to_vec();
                let executable = match cursor.u8()? {
                    0 => false,
                    1 => true,
                    _ => return None,
                };
                BuildStagedOutputEntryInput::File {
                    bytes: bytes.into(),
                    executable,
                }
            }
            2 => BuildStagedOutputEntryInput::Symlink {
                target: cursor.framed()?.to_vec(),
            },
            _ => return None,
        };
        inputs.push((relative_path, input));
    }
    BuildStagedOutputTree::from_retained_parts(&commitment, inputs).ok()
}

fn encode_attempt(attempt: &BuildFilesystemOperationAttempt, out: &mut Vec<u8>) {
    out.extend_from_slice(&attempt.operation_tag().to_le_bytes());
    out.push(observation_codec::encode_observation_provider(
        attempt.provider(),
    ));
    observation_codec::encode_operation_result(attempt.result(), out);
    out.extend_from_slice(&attempt.post_error().to_le_bytes());
    let authorized = attempt.authorized_paths();
    out.extend_from_slice(&(authorized.len() as u64).to_le_bytes());
    for path in authorized {
        out.push(path.operand_ordinal());
        out.push(observation_codec::encode_grant_access(path.access()));
        match path.root() {
            BuildFilesystemRoot::Source => out.push(0),
            BuildFilesystemRoot::Output => out.push(1),
            BuildFilesystemRoot::NamedInput(identity) => {
                out.push(2);
                out.extend_from_slice(&identity.to_le_bytes());
            }
        }
        push_framed(out, path.relative_path());
    }
    let inputs = attempt.logical_handle_inputs();
    out.extend_from_slice(&(inputs.len() as u64).to_le_bytes());
    for input in inputs {
        observation_codec::encode_logical_handle_input(*input, out);
    }
    match attempt.logical_handle_output() {
        Some(output) => {
            out.push(1);
            observation_codec::encode_logical_handle_output(output, out);
        }
        None => out.push(0),
    }
    let retired = attempt.retired_logical_handles();
    out.extend_from_slice(&(retired.len() as u64).to_le_bytes());
    for identity in retired {
        out.extend_from_slice(&identity.get().to_le_bytes());
    }
    let refusals = attempt.grant_refusals();
    out.extend_from_slice(&(refusals.len() as u64).to_le_bytes());
    for refusal in refusals {
        observation_codec::encode_grant_refusal(refusal, out);
    }
}

fn recover_attempt(cursor: &mut Cursor) -> Option<BuildFilesystemOperationAttempt> {
    let operation_tag = cursor.u16()?;
    let provider = observation_codec::recover_observation_provider(cursor.u8()?)?;
    let result = observation_codec::recover_operation_result(cursor)?;
    let post_error = cursor.i32()?;
    let authorized_count = cursor_count(cursor)?;
    let mut authorized_paths = Vec::with_capacity(authorized_count);
    for _ in 0..authorized_count {
        let operand_ordinal = cursor.u8()?;
        let access = observation_codec::recover_grant_access(cursor.u8()?)?;
        let root = match cursor.u8()? {
            0 => BuildFilesystemRoot::Source,
            1 => BuildFilesystemRoot::Output,
            2 => BuildFilesystemRoot::NamedInput(cursor.u32()?),
            _ => return None,
        };
        let relative_path = cursor.framed()?.to_vec();
        if !canonical_filesystem_metadata_path_is_canonical(&relative_path, false) {
            return None;
        }
        authorized_paths.push(BuildFilesystemAuthorizedPath {
            operand_ordinal,
            access,
            root,
            relative_path,
        });
    }
    let input_count = cursor_count(cursor)?;
    let mut logical_handle_inputs = Vec::with_capacity(input_count);
    for _ in 0..input_count {
        logical_handle_inputs.push(observation_codec::recover_logical_handle_input(cursor)?);
    }
    let logical_handle_output = match cursor.u8()? {
        0 => None,
        1 => Some(observation_codec::recover_logical_handle_output(cursor)?),
        _ => return None,
    };
    let retired_count = cursor_count(cursor)?;
    let mut retired_logical_handles = Vec::with_capacity(retired_count);
    for _ in 0..retired_count {
        retired_logical_handles.push(filesystem_authority::FilesystemLogicalHandleIdentity::new(
            cursor.u64()?,
        )?);
    }
    let refusal_count = cursor_count(cursor)?;
    let mut grant_refusals = Vec::with_capacity(refusal_count);
    for _ in 0..refusal_count {
        grant_refusals.push(observation_codec::recover_grant_refusal(cursor)?);
    }
    Some(BuildFilesystemOperationAttempt {
        operation_tag,
        provider,
        result,
        post_error,
        authorized_paths,
        logical_handle_inputs,
        logical_handle_output,
        retired_logical_handles,
        grant_refusals,
    })
}

/// Encode the observation summary a retained review carries.
pub fn encode_observation_summary(summary: &BuildObservationSummary, out: &mut Vec<u8>) {
    out.extend_from_slice(&summary.schema_version().to_le_bytes());
    out.push(summary.filesystem_host_observed() as u8);
    out.extend_from_slice(&summary.filesystem_operation_schema_version().to_le_bytes());
    let attempts = summary.filesystem_operation_attempts();
    out.extend_from_slice(&(attempts.len() as u64).to_le_bytes());
    for attempt in attempts {
        encode_attempt(attempt, out);
    }
    match summary.canonical_source_metadata_identity() {
        Some(identity) => {
            out.push(1);
            encode_source_metadata_identity(&identity, out);
        }
        None => out.push(0),
    }
    let activation = summary.activation();
    match activation.root_package_identity() {
        Some(identity) => {
            out.push(1);
            out.extend_from_slice(&identity.digest());
        }
        None => out.push(0),
    }
    encode_optional(activation.root_role().as_ref(), out, |role, out| {
        out.push(declaration_kind_tag(*role))
    });
    match activation.build_execution_profile() {
        Some(profile) => {
            out.push(1);
            out.push(crate::target_profile_tags::target_profile_tag(profile));
        }
        None => out.push(0),
    }
    match summary.captured_source_inventory() {
        Some(inventory) => {
            out.push(1);
            encode_captured_inventory(&inventory, out);
        }
        None => out.push(0),
    }
    let named = summary.named_input_inventories();
    out.extend_from_slice(&(named.len() as u64).to_le_bytes());
    for inventory in named {
        push_framed(out, inventory.name());
        out.extend_from_slice(&inventory.root_identity().to_le_bytes());
        encode_captured_inventory(&inventory.inventory(), out);
    }
    let handoffs = summary.included_source_handoffs();
    out.extend_from_slice(&(handoffs.len() as u64).to_le_bytes());
    for handoff in handoffs {
        push_framed(out, handoff.relative_path());
        out.extend_from_slice(&handoff.filesystem_attempt_ordinal().to_le_bytes());
    }
    let settlements = summary.required_output_settlements();
    out.extend_from_slice(&(settlements.len() as u64).to_le_bytes());
    for settlement in settlements {
        push_framed(out, settlement.relative_path());
        out.extend_from_slice(&settlement.sealed_attempt_ordinal().to_le_bytes());
    }
    match summary.staged_output_tree() {
        Some(tree) => {
            out.push(1);
            encode_staged_output_tree(tree, out);
        }
        None => out.push(0),
    }
    push_framed(out, summary.build_log());
}

/// Recover a retained observation summary. A malformed record yields `None`
/// so the caller treats the cache entry as absent.
pub fn recover_observation_summary(cursor: &mut Cursor) -> Option<BuildObservationSummary> {
    let schema_version = cursor.u32()?;
    let filesystem_host_observed = match cursor.u8()? {
        0 => false,
        1 => true,
        _ => return None,
    };
    let filesystem_operation_schema_version = cursor.u32()?;
    let attempt_count = cursor_count(cursor)?;
    let mut filesystem_operation_attempts = Vec::with_capacity(attempt_count);
    for _ in 0..attempt_count {
        filesystem_operation_attempts.push(recover_attempt(cursor)?);
    }
    let canonical_source_metadata_identity = match cursor.u8()? {
        0 => None,
        1 => Some(recover_source_metadata_identity(cursor)?),
        _ => return None,
    };
    let root_package_identity = match cursor.u8()? {
        0 => None,
        1 => {
            let digest: [u8; 32] = cursor.bytes(32)?.try_into().ok()?;
            Some(PackageKeyIdentity::from_digest(digest)?)
        }
        _ => return None,
    };
    let root_role = recover_optional(cursor, recover_declaration_kind)?;
    let build_execution_profile = match cursor.u8()? {
        0 => None,
        1 => Some(crate::target_profile_tags::recover_target_profile_tag(
            cursor.u8()?,
        )?),
        _ => return None,
    };
    let captured_source_inventory = match cursor.u8()? {
        0 => None,
        1 => Some(recover_captured_inventory(cursor)?),
        _ => return None,
    };
    let named_count = cursor_count(cursor)?;
    let mut named_input_inventories = Vec::with_capacity(named_count);
    for _ in 0..named_count {
        let name = cursor.framed()?.to_vec();
        let root_identity = cursor.u32()?;
        let inventory = recover_captured_inventory(cursor)?;
        named_input_inventories.push(BuildNamedInputInventory {
            name,
            root_identity,
            inventory,
        });
    }
    let handoff_count = cursor_count(cursor)?;
    let mut included_source_handoffs = Vec::with_capacity(handoff_count);
    for _ in 0..handoff_count {
        let relative_path = cursor.framed()?.to_vec();
        if !canonical_filesystem_metadata_path_is_canonical(&relative_path, false) {
            return None;
        }
        let filesystem_attempt_ordinal = cursor.u64()?;
        included_source_handoffs.push(BuildIncludedSourceHandoff {
            relative_path,
            filesystem_attempt_ordinal,
        });
    }
    let settlement_count = cursor_count(cursor)?;
    let mut required_output_settlements = Vec::with_capacity(settlement_count);
    for _ in 0..settlement_count {
        let relative_path = cursor.framed()?.to_vec();
        if !canonical_filesystem_metadata_path_is_canonical(&relative_path, false) {
            return None;
        }
        let sealed_attempt_ordinal = cursor.u64()?;
        required_output_settlements.push(BuildRequiredOutputSettlement {
            relative_path,
            sealed_attempt_ordinal,
        });
    }
    let staged_output_tree = match cursor.u8()? {
        0 => None,
        1 => Some(recover_staged_output_tree(cursor)?),
        _ => return None,
    };
    let build_log = cursor.framed()?.to_vec();
    Some(BuildObservationSummary {
        schema_version,
        filesystem_host_observed,
        filesystem_operation_schema_version,
        filesystem_operation_attempts,
        canonical_source_metadata_identity,
        activation: BuildActivation {
            root_package_identity,
            root_role,
            build_execution_profile,
        },
        captured_source_inventory,
        named_input_inventories,
        included_source_handoffs,
        required_output_settlements,
        staged_output_tree,
        build_log,
    })
}

fn encode_sponsor_limits(limits: &FilesystemSponsorLimits, out: &mut Vec<u8>) {
    out.extend_from_slice(&limits.maximum_entries.to_le_bytes());
    out.extend_from_slice(&limits.maximum_total_logical_bytes.to_le_bytes());
    out.extend_from_slice(&limits.maximum_object_extent.to_le_bytes());
}

fn recover_sponsor_limits(cursor: &mut Cursor) -> Option<FilesystemSponsorLimits> {
    Some(FilesystemSponsorLimits {
        maximum_entries: cursor.u64()?,
        maximum_total_logical_bytes: cursor.u64()?,
        maximum_object_extent: cursor.u64()?,
    })
}

fn encode_evaluation_sponsor_limits(limits: &BuildEvaluationSponsorLimits, out: &mut Vec<u8>) {
    for value in [
        limits.maximum_fuel_units(),
        limits.maximum_build_log_bytes(),
        limits.maximum_filesystem_operation_attempts(),
        limits.maximum_live_filesystem_handles(),
        limits.maximum_live_storage_cells(),
        limits.maximum_live_payload_bytes(),
        limits.maximum_result_storage_cells(),
        limits.maximum_result_payload_bytes(),
    ] {
        out.extend_from_slice(&value.to_le_bytes());
    }
}

fn recover_evaluation_sponsor_limits(cursor: &mut Cursor) -> Option<BuildEvaluationSponsorLimits> {
    BuildEvaluationSponsorLimits::new(
        cursor.u64()?,
        cursor.u64()?,
        cursor.u64()?,
        cursor.u64()?,
        cursor.u64()?,
        cursor.u64()?,
        cursor.u64()?,
        cursor.u64()?,
    )
}

fn encode_grant(grant: &RestrictedBuildGrant, out: &mut Vec<u8>) {
    match grant.root() {
        RestrictedBuildGrantRoot::SourceInventory => out.push(0),
        RestrictedBuildGrantRoot::StagedOutput => out.push(1),
        RestrictedBuildGrantRoot::Other(identity) => {
            out.push(2);
            out.extend_from_slice(&identity.to_le_bytes());
        }
    }
    out.push(grant.narrowed() as u8);
    match grant.captured() {
        Some(inventory) => {
            out.push(1);
            encode_captured_inventory(&inventory, out);
        }
        None => out.push(0),
    }
}

fn recover_grant(cursor: &mut Cursor) -> Option<RestrictedBuildGrant> {
    let root = match cursor.u8()? {
        0 => RestrictedBuildGrantRoot::SourceInventory,
        1 => RestrictedBuildGrantRoot::StagedOutput,
        2 => RestrictedBuildGrantRoot::Other(cursor.u32()?),
        _ => return None,
    };
    let narrowed = match cursor.u8()? {
        0 => false,
        1 => true,
        _ => return None,
    };
    let captured = match cursor.u8()? {
        0 => None,
        1 => Some(recover_captured_inventory(cursor)?),
        _ => return None,
    };
    Some(RestrictedBuildGrant::from_retained(
        root, narrowed, captured,
    ))
}

/// Encode one restricted build request.
pub fn encode_restricted_build_request(request: &RestrictedBuildRequest, out: &mut Vec<u8>) {
    out.push(match request.operation() {
        RestrictedBuildOperation::ScopedFilesystemExecution => 0,
        RestrictedBuildOperation::UnscopedFilesystemExecution => 1,
    });
    let reads = request.read_grants();
    out.extend_from_slice(&(reads.len() as u64).to_le_bytes());
    for grant in reads {
        encode_grant(grant, out);
    }
    let writes = request.write_grants();
    out.extend_from_slice(&(writes.len() as u64).to_le_bytes());
    for grant in writes {
        encode_grant(grant, out);
    }
    let bounds = request.bounds();
    match bounds.filesystem_sponsor_limits() {
        Some(limits) => {
            out.push(1);
            encode_sponsor_limits(&limits, out);
        }
        None => out.push(0),
    }
    match bounds.evaluation_sponsor_limits() {
        Some(limits) => {
            out.push(1);
            encode_evaluation_sponsor_limits(&limits, out);
        }
        None => out.push(0),
    }
    let required = bounds.required_outputs();
    out.extend_from_slice(&(required.len() as u64).to_le_bytes());
    for output in required {
        push_framed(out, output);
    }
    out.push(bounds.artifact_only() as u8);
    match request.build_execution_profile() {
        Some(profile) => {
            out.push(1);
            out.push(crate::target_profile_tags::target_profile_tag(profile));
        }
        None => out.push(0),
    }
    match request.selected_target_profile() {
        Some(profile) => {
            out.push(1);
            out.push(crate::target_profile_tags::target_profile_tag(profile));
        }
        None => out.push(0),
    }
}

/// Recover one restricted build request.
pub fn recover_restricted_build_request(cursor: &mut Cursor) -> Option<RestrictedBuildRequest> {
    let operation = match cursor.u8()? {
        0 => RestrictedBuildOperation::ScopedFilesystemExecution,
        1 => RestrictedBuildOperation::UnscopedFilesystemExecution,
        _ => return None,
    };
    let read_count = cursor_count(cursor)?;
    let mut read_grants = Vec::with_capacity(read_count);
    for _ in 0..read_count {
        read_grants.push(recover_grant(cursor)?);
    }
    let write_count = cursor_count(cursor)?;
    let mut write_grants = Vec::with_capacity(write_count);
    for _ in 0..write_count {
        write_grants.push(recover_grant(cursor)?);
    }
    let filesystem_sponsor_limits = match cursor.u8()? {
        0 => None,
        1 => Some(recover_sponsor_limits(cursor)?),
        _ => return None,
    };
    let evaluation_sponsor_limits = match cursor.u8()? {
        0 => None,
        1 => Some(recover_evaluation_sponsor_limits(cursor)?),
        _ => return None,
    };
    let required_count = cursor_count(cursor)?;
    let mut required_outputs = Vec::with_capacity(required_count);
    for _ in 0..required_count {
        let output = cursor.framed()?.to_vec();
        if !canonical_filesystem_metadata_path_is_canonical(&output, false) {
            return None;
        }
        required_outputs.push(output);
    }
    let artifact_only = match cursor.u8()? {
        0 => false,
        1 => true,
        _ => return None,
    };
    let build_execution_profile = match cursor.u8()? {
        0 => None,
        1 => Some(crate::target_profile_tags::recover_target_profile_tag(
            cursor.u8()?,
        )?),
        _ => return None,
    };
    let selected_target_profile = match cursor.u8()? {
        0 => None,
        1 => Some(crate::target_profile_tags::recover_target_profile_tag(
            cursor.u8()?,
        )?),
        _ => return None,
    };
    Some(RestrictedBuildRequest::from_retained(
        operation,
        read_grants,
        write_grants,
        RestrictedBuildBounds::from_retained(
            filesystem_sponsor_limits,
            evaluation_sponsor_limits,
            required_outputs,
            artifact_only,
        ),
        build_execution_profile,
        selected_target_profile,
    ))
}

fn cursor_count(cursor: &mut Cursor) -> Option<usize> {
    usize::try_from(cursor.u64()?).ok()
}

#[cfg(test)]
mod evaluation_usage_controls {
    use super::*;
    use evaluation_accounting::BuildEvaluationSponsor;

    fn measured(identity: EvaluationMeasurementIdentity) -> BuildEvaluationUsage {
        let limits = BuildEvaluationSponsorLimits::new(10, 8, 2, 2, 4, 8, 4, 8).unwrap();
        let sponsor = BuildEvaluationSponsor::new_with_measurement(limits, identity).unwrap();
        let account = sponsor.begin_invocation(identity, 10).unwrap();
        let _lease = account.reserve_site(1, 2, 4).unwrap();
        account.charge_result_custody(2, 4).unwrap();
        *account.snapshot().unwrap().record()
    }

    fn terminal() -> EvaluationMeasurementIdentity {
        EvaluationMeasurementIdentity::TerminalPsi {
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
        }
    }

    fn encoded(usage: &BuildEvaluationUsage) -> Vec<u8> {
        let mut bytes = Vec::new();
        encode_build_evaluation_usage(usage, &mut bytes);
        bytes
    }

    #[test]
    fn versioned_measurements_round_trip_without_engine_substitution() {
        for identity in [EvaluationMeasurementIdentity::CheckedTree, terminal()] {
            let usage = measured(identity);
            let bytes = encoded(&usage);
            let mut cursor = Cursor::new(&bytes);
            assert_eq!(recover_build_evaluation_usage(&mut cursor), Some(usage));
            assert!(cursor.exhausted());
        }
        assert_ne!(
            encoded(&measured(EvaluationMeasurementIdentity::CheckedTree)),
            encoded(&measured(terminal()))
        );
    }

    #[test]
    fn untagged_prior_usage_and_unknown_engine_tags_are_rejected() {
        let mut bytes = encoded(&measured(EvaluationMeasurementIdentity::CheckedTree));
        assert!(recover_build_evaluation_usage(&mut Cursor::new(&bytes[5..])).is_none());
        bytes[4] = 2;
        assert!(recover_build_evaluation_usage(&mut Cursor::new(&bytes)).is_none());
    }

    #[test]
    fn unsupported_storage_and_fuel_schedules_are_rejected() {
        for (offset, value) in [(5, 0_u32), (5, 2), (9, 0), (9, 2)] {
            let mut bytes = encoded(&measured(terminal()));
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(recover_build_evaluation_usage(&mut Cursor::new(&bytes)).is_none());
        }
    }

    #[test]
    fn schema_schedule_and_sponsor_flags_cannot_default() {
        let base = encoded(&measured(EvaluationMeasurementIdentity::CheckedTree));
        let mut wrong_schema = base.clone();
        wrong_schema[5..9].copy_from_slice(&8_u32.to_le_bytes());
        assert!(recover_build_evaluation_usage(&mut Cursor::new(&wrong_schema)).is_none());
        let mut wrong_schedule = base.clone();
        wrong_schedule[9..13].copy_from_slice(&2_u32.to_le_bytes());
        assert!(recover_build_evaluation_usage(&mut Cursor::new(&wrong_schedule)).is_none());
        let mut invalid_flag = base.clone();
        invalid_flag[21] = 2;
        assert!(recover_build_evaluation_usage(&mut Cursor::new(&invalid_flag)).is_none());
        let mut wrong_sponsor_schema = base;
        wrong_sponsor_schema[22..26].copy_from_slice(&0_u32.to_le_bytes());
        assert!(recover_build_evaluation_usage(&mut Cursor::new(&wrong_sponsor_schema)).is_none());
    }

    #[test]
    fn counters_exceeding_declared_allowances_are_not_recovered() {
        let mut usage = measured(terminal());
        usage.fuel_units = usage.invocation_fuel_ceiling + 1;
        let bytes = encoded(&usage);
        assert!(recover_build_evaluation_usage(&mut Cursor::new(&bytes)).is_none());
    }
}
