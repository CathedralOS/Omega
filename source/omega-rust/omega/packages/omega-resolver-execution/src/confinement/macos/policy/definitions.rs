use super::metadata::ConfinedMetadata;
use crate::backend::ResolverExecutionAuthorityRoots;
use crate::network::ResolverExecutionEndpointRoutePolicy;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub(super) fn encode(
    executable: &Path,
    additional_executables: &[PathBuf],
    endpoint_route: Option<&ResolverExecutionEndpointRoutePolicy>,
    roots: ResolverExecutionAuthorityRoots<'_>,
    confined_metadata: Option<&ConfinedMetadata>,
) -> Vec<OsString> {
    let mut definitions = vec![definition_argument("EXECUTABLE_0", executable)];
    for (index, helper) in additional_executables.iter().enumerate() {
        definitions.push(definition_argument(
            &format!("EXECUTABLE_{}", index + 1),
            helper,
        ));
    }
    if let Some(root) = roots.mutable_root {
        definitions.push(definition_argument("MUTABLE_ROOT", root));
    }
    if let Some(root) = roots.inspection_read_root {
        definitions.push(definition_argument("INSPECTION_READ_ROOT", root));
    }
    if let Some(root) = roots.discovery_read_root {
        definitions.push(definition_argument("DISCOVERY_READ_ROOT", root));
    }
    if let Some(metadata) = confined_metadata {
        for (index, path) in metadata.subpaths.iter().enumerate() {
            definitions.push(definition_argument(
                &format!("METADATA_SUBPATH_{index}"),
                path,
            ));
        }
        for (index, path) in metadata.paths.iter().enumerate() {
            definitions.push(definition_argument(&format!("METADATA_PATH_{index}"), path));
        }
    }
    if let Some(route) = endpoint_route {
        definitions.push(OsString::from(format!(
            "BROKER_ENDPOINT=localhost:{}",
            route.broker_endpoint().port()
        )));
    }
    definitions
}

fn definition_argument(name: &str, value: &Path) -> OsString {
    let mut argument = OsString::from(name);
    argument.push("=");
    argument.push(value.as_os_str());
    argument
}
