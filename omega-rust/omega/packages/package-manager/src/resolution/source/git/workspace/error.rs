use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GitWorkspaceSelectionLimit {
    DeclarationBytes,
    TotalDeclarationBytes,
}

impl GitWorkspaceSelectionLimit {
    const fn description(self) -> &'static str {
        match self {
            Self::DeclarationBytes => "per-declaration byte",
            Self::TotalDeclarationBytes => "aggregate declaration byte",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GitWorkspaceSelectionError {
    ResourceLimit {
        limit: GitWorkspaceSelectionLimit,
        maximum: usize,
        observed: usize,
    },
    DeclarationEvidenceChanged,
}

impl fmt::Display for GitWorkspaceSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimit {
                limit,
                maximum,
                observed,
            } => write!(
                formatter,
                "Git workspace selection exceeded its {} ceiling of {maximum} (observed {observed})",
                limit.description()
            ),
            Self::DeclarationEvidenceChanged => formatter.write_str(
                "authenticated Git workspace declaration evidence changed during replay",
            ),
        }
    }
}
impl std::error::Error for GitWorkspaceSelectionError {}
