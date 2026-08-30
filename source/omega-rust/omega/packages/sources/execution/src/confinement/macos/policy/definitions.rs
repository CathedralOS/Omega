use crate::backend::ResolverExecutionAuthorityRoots;
use std::ffi::OsString;
use std::path::Path;

pub(super) fn encode(
    executable: &Path,
    roots: ResolverExecutionAuthorityRoots<'_>,
) -> Vec<OsString> {
    let mut definitions = vec![definition_argument("EXECUTABLE_0", executable)];
    if let Some(root) = roots.mutable_root {
        definitions.push(definition_argument("MUTABLE_ROOT", root));
    }
    definitions
}

fn definition_argument(name: &str, value: &Path) -> OsString {
    let mut argument = OsString::from(name);
    argument.push("=");
    argument.push(value.as_os_str());
    argument
}
