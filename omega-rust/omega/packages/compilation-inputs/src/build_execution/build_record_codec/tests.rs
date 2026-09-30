//! Pins the retained build-record encodings to their exact bytes. Persisted
//! compiled reviews frame these records verbatim, so a failure here is a
//! record format change, not a pin to refresh.

use super::{
    encode_build_evaluation_usage, encode_observation_summary, encode_restricted_build_request,
    recover_build_evaluation_usage, recover_observation_summary, recover_restricted_build_request,
};
use crate::build_execution::observations::{
    BuildActivation, BuildCanonicalSourceMetadataIdentity, BuildCapturedSourceInventory,
    BuildIncludedSourceHandoff, BuildNamedInputInventory, BuildObservationSummary,
    BuildRequiredOutputSettlement,
};
use crate::build_execution::restricted_requests::{
    RestrictedBuildBounds, RestrictedBuildGrant, RestrictedBuildGrantRoot,
    RestrictedBuildOperation, RestrictedBuildRequest,
};
use checked_interpreter::observation_codec::ObservationCursor;
use checked_interpreter::{
    BuildEvaluationSponsorLimits, BuildEvaluationUsage, FilesystemSponsorLimits,
};
use target::TargetProfile;

const PINNED_USAGE: &str = concat!(
    "0300000004000000050000000000000001060000000107000000000000000001090000000000000000010b0000000000",
    "0000010c0000000000000000010e000000000000000f0000000000000010000000000000001100000000000000120000",
    "000000000013000000000000001400000000000000150000000000000016000000000000001700000000000000180000",
    "0000000000",
);
const PINNED_SUMMARY: &str = concat!(
    "520000000105000000000000000000000001020000003131313131313131313131313131313131313131313131313131",
    "313131313131013232323232323232323232323232323232323232323232323232323232323232010101030133000000",
    "000000009700000000000000020000003333333333333333333333333333333333333333333333333333333333333333",
    "010000000000000006000000000000006173736574730400000034000000000000009800000000000000020000003434",
    "343434343434343434343434343434343434343434343434343434343434010000000000000012000000000000006765",
    "6e6572617465642f6c6561662e6f6d670600000000000000010000000000000007000000000000006f75742f61707007",
    "0000000000000001a683e8d11b724e05d9c486c951588f531266baa78a34ba16e0755c803b35934c0200000000000000",
    "0600000000000000020000000000000003000000000000006f75740007000000000000006f75742f6170700106000000",
    "0000000062696e6172790109000000000000006275696c64206c6f67",
);
const PINNED_REQUEST: &str = concat!(
    "010200000000000000000101350000000000000099000000000000000200000035353535353535353535353535353535",
    "353535353535353535353535353535350209000000000001000000000000000100000109000000000000000010000000",
    "000000000200000000000001010000000000000002000000000000000300000000000000040000000000000005000000",
    "00000000060000000000000007000000000000000800000000000000010000000000000007000000000000006f75742f",
    "6170700101020103",
);

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn usage() -> BuildEvaluationUsage {
    BuildEvaluationUsage {
        usage_schema_version: 3,
        step_schedule_marker: 4,
        invocation_fuel_ceiling: 5,
        sponsor_schema_version: Some(6),
        session_fuel_ceiling: Some(7),
        session_build_log_byte_ceiling: None,
        session_filesystem_attempt_ceiling: Some(9),
        session_live_filesystem_handle_ceiling: None,
        session_live_cell_ceiling: Some(11),
        session_live_text_byte_ceiling: Some(12),
        session_result_cell_ceiling: None,
        session_result_text_byte_ceiling: Some(14),
        session_peak_live_filesystem_handles: 15,
        session_peak_live_cells: 16,
        session_peak_live_text_bytes: 17,
        fuel_units: 18,
        build_log_bytes: 19,
        filesystem_operation_attempts: 20,
        peak_live_cells: 21,
        peak_live_text_bytes: 22,
        result_cells: 23,
        result_text_bytes: 24,
    }
}

fn inventory(marker: u8) -> BuildCapturedSourceInventory {
    BuildCapturedSourceInventory {
        entry_count: u64::from(marker),
        file_bytes: 100 + u64::from(marker),
        source_metadata_identity: BuildCanonicalSourceMetadataIdentity::new(2, [marker; 32]),
    }
}

fn summary() -> BuildObservationSummary {
    BuildObservationSummary {
        schema_version: 82,
        filesystem_host_observed: true,
        filesystem_operation_schema_version: 5,
        filesystem_operation_attempts: Vec::new(),
        canonical_source_metadata_identity: Some(BuildCanonicalSourceMetadataIdentity::new(
            2, [0x31; 32],
        )),
        activation: BuildActivation {
            root_package_identity: semantic_vocabulary::PackageKeyIdentity::from_digest([0x32; 32]),
            root_role: Some(crate::BuildDeclarationKind::Application),
            build_execution_profile: Some(TargetProfile::MacosArm64),
        },
        captured_source_inventory: Some(inventory(0x33)),
        named_input_inventories: vec![BuildNamedInputInventory {
            name: b"assets".to_vec(),
            root_identity: 4,
            inventory: inventory(0x34),
        }],
        included_source_handoffs: vec![BuildIncludedSourceHandoff {
            relative_path: b"generated/leaf.omg".to_vec(),
            filesystem_attempt_ordinal: 6,
        }],
        required_output_settlements: vec![BuildRequiredOutputSettlement {
            relative_path: b"out/app".to_vec(),
            sealed_attempt_ordinal: 7,
        }],
        staged_output_tree: Some(
            build_output::from_entries(&[
                build_output::OutputTreeEntry::directory(b"out"),
                build_output::OutputTreeEntry::regular_file(b"out/app", b"binary", true),
            ])
            .expect("canonical staged output tree"),
        ),
        build_log: b"build log".to_vec(),
    }
}

fn request() -> RestrictedBuildRequest {
    RestrictedBuildRequest::from_retained(
        RestrictedBuildOperation::UnscopedFilesystemExecution,
        vec![
            RestrictedBuildGrant::from_retained(
                RestrictedBuildGrantRoot::SourceInventory,
                true,
                Some(inventory(0x35)),
            ),
            RestrictedBuildGrant::from_retained(RestrictedBuildGrantRoot::Other(9), false, None),
        ],
        vec![RestrictedBuildGrant::from_retained(
            RestrictedBuildGrantRoot::StagedOutput,
            false,
            None,
        )],
        RestrictedBuildBounds::from_retained(
            Some(FilesystemSponsorLimits {
                maximum_entries: 9,
                maximum_total_logical_bytes: 4096,
                maximum_object_extent: 512,
            }),
            BuildEvaluationSponsorLimits::new(1, 2, 3, 4, 5, 6, 7, 8),
            vec![b"out/app".to_vec()],
            true,
        ),
        Some(TargetProfile::LinuxX64),
        Some(TargetProfile::MacosArm64),
    )
}

#[test]
fn build_evaluation_usage_keeps_its_exact_bytes() {
    let mut encoded = Vec::new();
    encode_build_evaluation_usage(&usage(), &mut encoded);
    assert_eq!(hex(&encoded), PINNED_USAGE);
    let mut cursor = ObservationCursor::new(&encoded);
    assert_eq!(recover_build_evaluation_usage(&mut cursor), Some(usage()));
    assert!(cursor.exhausted());
}

#[test]
fn observation_summary_keeps_its_exact_bytes() {
    let mut encoded = Vec::new();
    encode_observation_summary(&summary(), &mut encoded);
    assert_eq!(hex(&encoded), PINNED_SUMMARY);
    let mut cursor = ObservationCursor::new(&encoded);
    assert_eq!(recover_observation_summary(&mut cursor), Some(summary()));
    assert!(cursor.exhausted());
}

#[test]
fn restricted_build_request_keeps_its_exact_bytes() {
    let mut encoded = Vec::new();
    encode_restricted_build_request(&request(), &mut encoded);
    assert_eq!(hex(&encoded), PINNED_REQUEST);
    let mut cursor = ObservationCursor::new(&encoded);
    assert_eq!(
        recover_restricted_build_request(&mut cursor),
        Some(request())
    );
    assert!(cursor.exhausted());
}
