//! Convert between authored workspace paths and syntax-neutral source paths.

use build_declarations::WorkspaceMemberPath;
use package_source::SourceRelativePath;

/// Resolve an authored Path inside its registered workspace, never the host.
pub(crate) fn normalize_member_path(
    requester_member: Option<&str>,
    location: &str,
) -> Result<WorkspaceMemberPath, String> {
    if location.is_empty()
        || location.starts_with('/')
        || location.ends_with('/')
        || location.contains('\\')
        || location.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err("path must be a portable relative location".into());
    }
    let mut components = requester_member
        .map(|member| member.split('/').map(str::to_owned).collect::<Vec<_>>())
        .unwrap_or_default();
    for component in location.split('/') {
        match component {
            "" => return Err("path contains an empty component".into()),
            "." => {}
            ".." => {
                if components.pop().is_none() {
                    return Err("path escapes its registered workspace".into());
                }
            }
            component => components.push(component.to_owned()),
        }
    }
    if components.is_empty() {
        return Err("path resolves to the workspace root".into());
    }
    WorkspaceMemberPath::parse(components.join("/")).map_err(|error| error.to_string())
}

pub(crate) fn source_relative_path(member_path: &WorkspaceMemberPath) -> SourceRelativePath {
    SourceRelativePath::parse(member_path.as_str())
        .expect("authored workspace paths satisfy source-relative path invariants")
}

pub(crate) fn authored_workspace_member_path(
    source_path: &SourceRelativePath,
) -> WorkspaceMemberPath {
    WorkspaceMemberPath::parse(source_path.as_str())
        .expect("source-relative paths satisfy authored workspace path invariants")
}

#[cfg(test)]
mod tests {
    use super::normalize_member_path;

    #[test]
    fn sibling_navigation_stays_inside_registered_workspace() {
        assert_eq!(
            normalize_member_path(Some("packages/first"), "../second")
                .unwrap()
                .as_str(),
            "packages/second"
        );
        assert_eq!(
            normalize_member_path(None, "packages/./second")
                .unwrap()
                .as_str(),
            "packages/second"
        );
        assert!(
            normalize_member_path(Some("first"), "../../outside")
                .unwrap_err()
                .contains("escapes its registered workspace")
        );
        assert!(
            normalize_member_path(Some("first"), "..")
                .unwrap_err()
                .contains("workspace root")
        );
    }
}
