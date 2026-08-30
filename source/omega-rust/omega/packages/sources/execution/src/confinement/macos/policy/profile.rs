use super::MACOS_NULL_DEVICE;
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
) -> String {
    let mut encoded = "(version 1) (deny default) ".to_owned();
    if phase.permits_descendant_processes() {
        encoded.push_str("(allow process-fork) ");
    }
    encoded.push_str("(allow signal) ");
    // Git and its transport helpers consume the invoking user's ordinary host
    // configuration, include files, credential helpers, agents, identities,
    // known-host policy, and proxies. Their read locations are intentionally
    // not a compiler-owned closed set. Seatbelt still confines writes,
    // executable paths, descendants, resources, and network endpoints.
    encoded.push_str("(allow file-read*) ");
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
