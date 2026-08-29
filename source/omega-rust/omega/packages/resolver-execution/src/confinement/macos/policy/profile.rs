use super::metadata::ConfinedMetadata;
use super::{MACOS_NULL_DEVICE, MACOS_TLS_CONFIGURATION_ALIAS_ROOT, MACOS_TLS_CONFIGURATION_ROOT};
use crate::model::{ResolverExecutionNetworkTransport, ResolverExecutionPhase};
use std::path::PathBuf;

const MACOS_DIRECTORY_LOOKUP_SERVICE: &str = "com.apple.system.opendirectoryd.libinfo";
const MACOS_HOSTNAME_SYSCTL: &str = "kern.hostname";
const MACOS_RUST_RUNTIME_PAGE_SIZE_SYSCTL: &str = "hw.pagesize_compat";

pub(super) fn encode(
    additional_executables: &[PathBuf],
    phase: ResolverExecutionPhase,
    network_transport: Option<ResolverExecutionNetworkTransport>,
    has_endpoint_route: bool,
    confined_metadata: Option<&ConfinedMetadata>,
) -> String {
    let mut encoded = "(version 1) (deny default) ".to_owned();
    if phase.permits_descendant_processes() {
        encoded.push_str("(allow process-fork) ");
    }
    encoded.push_str("(allow signal) ");
    encode_read_policy(
        &mut encoded,
        additional_executables,
        phase,
        network_transport,
        confined_metadata,
    );
    encode_execution_policy(&mut encoded, additional_executables);
    if has_endpoint_route {
        encoded.push_str(" (allow network-outbound (remote tcp (param \"BROKER_ENDPOINT\")))");
    }
    if network_transport == Some(ResolverExecutionNetworkTransport::Ssh) {
        encoded.push_str(&format!(
            " (allow mach-lookup (global-name \"{MACOS_DIRECTORY_LOOKUP_SERVICE}\")) \
             (allow sysctl-read (sysctl-name \"{MACOS_HOSTNAME_SYSCTL}\")) \
             (allow sysctl-read (sysctl-name \"{MACOS_RUST_RUNTIME_PAGE_SIZE_SYSCTL}\"))"
        ));
    }
    if phase.requires_mutable_root() {
        encoded.push_str(" (allow file-write* (subpath (param \"MUTABLE_ROOT\")))");
    }
    encoded
}

fn encode_read_policy(
    encoded: &mut String,
    additional_executables: &[PathBuf],
    phase: ResolverExecutionPhase,
    network_transport: Option<ResolverExecutionNetworkTransport>,
    confined_metadata: Option<&ConfinedMetadata>,
) {
    let confines_content_reads = matches!(
        phase,
        ResolverExecutionPhase::RepositoryInitialization
            | ResolverExecutionPhase::RepositoryInspection
    ) || (phase == ResolverExecutionPhase::Fetch
        && network_transport == Some(ResolverExecutionNetworkTransport::Https))
        || (phase == ResolverExecutionPhase::TransportDiscovery
            && network_transport == Some(ResolverExecutionNetworkTransport::Https));
    if !confines_content_reads {
        encoded.push_str("(allow file-read*) ");
        return;
    }

    let read_root_parameter = match phase {
        ResolverExecutionPhase::TransportDiscovery => "DISCOVERY_READ_ROOT",
        ResolverExecutionPhase::RepositoryInspection => "INSPECTION_READ_ROOT",
        ResolverExecutionPhase::RepositoryInitialization | ResolverExecutionPhase::Fetch => {
            "MUTABLE_ROOT"
        }
    };
    if let Some(metadata) = confined_metadata {
        encoded.push_str("(allow file-read-metadata file-test-existence (subpath (param \"");
        encoded.push_str(metadata.root_parameter);
        encoded.push_str("\"))");
        if metadata.includes_tls_root {
            encoded.push_str(&format!(
                " (subpath \"{MACOS_TLS_CONFIGURATION_ROOT}\") \
                 (subpath \"{MACOS_TLS_CONFIGURATION_ALIAS_ROOT}\")"
            ));
        }
        for index in 0..metadata.subpaths.len() {
            encoded.push_str(&format!(" (subpath (param \"METADATA_SUBPATH_{index}\"))"));
        }
        for index in 0..metadata.paths.len() {
            encoded.push_str(&format!(" (literal (param \"METADATA_PATH_{index}\"))"));
        }
        encoded.push_str(") ");
    } else {
        encoded.push_str("(allow file-read-metadata) ");
    }
    encoded.push_str("(allow file-read-data (subpath (param \"");
    encoded.push_str(read_root_parameter);
    encoded.push_str("\")) (literal (param \"EXECUTABLE_0\"))");
    for index in 0..additional_executables.len() {
        encoded.push_str(&format!(" (literal (param \"EXECUTABLE_{}\"))", index + 1));
    }
    if matches!(
        phase,
        ResolverExecutionPhase::TransportDiscovery | ResolverExecutionPhase::Fetch
    ) && network_transport == Some(ResolverExecutionNetworkTransport::Https)
    {
        encoded.push_str(&format!(" (subpath \"{MACOS_TLS_CONFIGURATION_ROOT}\")"));
    }
    encoded.push_str(&format!(
        " (literal \"{}\") (literal \"{MACOS_NULL_DEVICE}\")) ",
        std::path::MAIN_SEPARATOR
    ));
}

fn encode_execution_policy(encoded: &mut String, additional_executables: &[PathBuf]) {
    encoded.push_str(&format!(
        "(allow file-test-existence file-write-data (literal \"{MACOS_NULL_DEVICE}\")) \
         (allow process-exec (literal (param \"EXECUTABLE_0\"))"
    ));
    for index in 0..additional_executables.len() {
        encoded.push_str(&format!(" (literal (param \"EXECUTABLE_{}\"))", index + 1));
    }
    encoded.push(')');
}
