//! Convert between authored workspace paths and syntax-neutral source paths.

use build_declarations::WorkspaceMemberPath;
use package_source::SourceRelativePath;

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
    use super::*;

    #[test]
    fn boundary_conversion_preserves_canonical_path_bytes() {
        let authored = WorkspaceMemberPath::parse("packages/arithmetic-kernels").unwrap();
        let source = source_relative_path(&authored);
        let recovered = authored_workspace_member_path(&source);

        assert_eq!(source.as_str(), authored.as_str());
        assert_eq!(recovered, authored);
    }
}
