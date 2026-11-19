use crate::{NameError, StageSource};
use omega_lexer::SourceSpan;
use omega_parser::IndexedSyntax;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TreeNodeId {
    pub file: usize,
    pub node: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct File<'a> {
    pub(crate) ordinal: usize,
    pub package: &'a str,
    pub path: &'a str,
    pub namespace: String,
    syntax: IndexedSyntax<'a>,
}
impl<'a> std::ops::Deref for File<'a> {
    type Target = IndexedSyntax<'a>;
    fn deref(&self) -> &Self::Target {
        &self.syntax
    }
}

impl<'a> File<'a> {
    pub(crate) fn arm_has_subject(
        &self,
        arm: usize,
    ) -> bool {
        let dispatch = self.nodes[arm].parent.expect("arm belongs to dispatch");
        self.nodes[self.nodes[dispatch].children[0]].kind != omega_parser::NodeKind::Arm
    }
    /// Node IDs index this tree within the ordered forest issued by L2.
    pub fn node_id(
        &self,
        node: usize,
    ) -> TreeNodeId {
        TreeNodeId { file: self.ordinal, node }
    }
    pub fn read(input: &StageSource<'a, '_>) -> Result<Self, NameError> {
        let source = &input.source;
        let bad = || NameError::new(input.package, source.logical_path, SourceSpan::new(0, 0), "inconsistent parsed source");
        let syntax = source.tree.index(source.text, source.tokens).ok_or_else(bad)?;
        let namespace = source
            .logical_path
            .rsplit_once('/')
            .map_or(String::new(), |(dir, _)| dir.replace('/', "::"));
        Ok(Self {
            ordinal: 0,
            package: input.package,
            path: source.logical_path,
            namespace,
            syntax,
        })
    }
    pub fn error(
        &self,
        span: SourceSpan,
        reason: impl Into<String>,
    ) -> NameError {
        NameError::new(self.package, self.path, span, reason)
    }
}
