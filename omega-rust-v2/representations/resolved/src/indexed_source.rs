use crate::{NameError, NameErrorKind};
use omega_syntax::IndexedSyntax;
use omega_tokens::SourceSpan;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TreeNodeId {
    pub file: usize,
    pub node: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct IndexedSource<'a> {
    ordinal: usize,
    pub package: &'a str,
    pub path: &'a str,
    pub namespace: String,
    syntax: IndexedSyntax<'a>,
}
impl<'a> std::ops::Deref for IndexedSource<'a> {
    type Target = IndexedSyntax<'a>;
    fn deref(&self) -> &Self::Target {
        &self.syntax
    }
}

impl<'a> IndexedSource<'a> {
    pub fn new(
        ordinal: usize,
        package: &'a str,
        path: &'a str,
        namespace: String,
        syntax: IndexedSyntax<'a>,
    ) -> Self {
        Self {
            ordinal,
            package,
            path,
            namespace,
            syntax,
        }
    }

    pub fn ordinal(&self) -> usize {
        self.ordinal
    }

    /// Node IDs index this tree within the ordered forest issued by L2.
    pub fn node_id(
        &self,
        node: usize,
    ) -> TreeNodeId {
        TreeNodeId { file: self.ordinal, node }
    }
    pub fn error(
        &self,
        kind: NameErrorKind,
        span: SourceSpan,
        reason: impl Into<String>,
    ) -> NameError {
        NameError::new(kind, self.package, self.path, span, reason)
    }
}
