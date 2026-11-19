use crate::{NameError, StageSource};
use omega_lexer::{SourceSpan, TokenKind};
use omega_parser::{NodeKind, TreeEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Leaf<'a> {
    pub text: &'a str,
    pub kind: TokenKind,
    pub span: SourceSpan,
}

/// Nodes are numbered in preorder. The half-open subtree interval supports scope
/// checks without recursive walks, including for long left-associated expressions.
#[derive(Debug, PartialEq, Eq)]
pub struct Node<'a> {
    pub kind: NodeKind,
    pub parent: Option<usize>,
    enclosing_context: Option<usize>,
    pub end: usize,
    pub children: Vec<usize>,
    pub leaves: Vec<Leaf<'a>>,
    pub first_token: usize,
    pub end_token: usize,
}

impl<'a> Node<'a> {
    pub fn words(&self) -> impl DoubleEndedIterator<Item = Leaf<'a>> + '_ {
        self.leaves
            .iter()
            .copied()
            .filter(|leaf| leaf.kind == TokenKind::Word)
    }
    pub fn path(&self) -> String {
        self.words()
            .map(|leaf| leaf.text)
            .collect::<Vec<_>>()
            .join("::")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
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
    pub nodes: Vec<Node<'a>>,
    pub tokens: Vec<Leaf<'a>>,
}

impl<'a> File<'a> {
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
        let mut nodes: Vec<Node<'a>> = Vec::new();
        let mut tokens = Vec::new();
        let mut token_indices = source.tree.get_meaningful_token_indices().iter();
        let mut stack = Vec::<usize>::new();
        for event in source.tree.get_events() {
            match *event {
                TreeEvent::Open(kind) => {
                    let index = nodes.len();
                    let parent = stack.last().copied();
                    let enclosing_context = parent.and_then(|p| {
                        use NodeKind::*;
                        match nodes[p].kind {
                            Machine | AsmMachine | MathLet | Trait | Domain | Data | Conformance | MachineType | FunctionType | Hypothesis | Let | Arm
                            | Field | Case | CaseField | Const | Requires | Ensures | Body | Access | AsmBlock | SelectionRow => Some(p),
                            _ => nodes[p].enclosing_context,
                        }
                    });
                    if let Some(owner) = parent {
                        nodes[owner].children.push(index);
                    }
                    nodes.push(Node {
                        kind,
                        parent,
                        enclosing_context,
                        end: 0,
                        children: vec![],
                        leaves: vec![],
                        first_token: tokens.len(),
                        end_token: 0,
                    });
                    stack.push(index);
                }
                TreeEvent::Token => {
                    let token = token_indices
                        .next()
                        .and_then(|&i| source.tokens.get(i as usize))
                        .ok_or_else(bad)?;
                    let span = token.get_span();
                    let text = source
                        .text
                        .get(span.get_start_offset()..span.get_end_offset())
                        .ok_or_else(bad)?;
                    let leaf = Leaf {
                        text,
                        span,
                        kind: token.get_kind(),
                    };
                    nodes[*stack.last().ok_or_else(bad)?].leaves.push(leaf);
                    tokens.push(leaf);
                }
                TreeEvent::Close => {
                    let index = stack.pop().ok_or_else(bad)?;
                    nodes[index].end = nodes.len();
                    nodes[index].end_token = tokens.len();
                }
            }
        }
        let namespace = source
            .logical_path
            .rsplit_once('/')
            .map_or(String::new(), |(dir, _)| dir.replace('/', "::"));
        Ok(Self {
            ordinal: 0,
            package: input.package,
            path: source.logical_path,
            namespace,
            nodes,
            tokens,
        })
    }
    /// Skip expression-only ancestors. A flat chain of binary expressions has one binding scope.
    pub fn enclosing_contexts(
        &self,
        n: usize,
    ) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(self.nodes[n].enclosing_context, |&p| self.nodes[p].enclosing_context)
    }
    pub fn child(
        &self,
        n: usize,
        kind: NodeKind,
    ) -> Option<usize> {
        self.nodes[n]
            .children
            .iter()
            .copied()
            .find(|&i| self.nodes[i].kind == kind)
    }
    pub fn span(
        &self,
        n: usize,
    ) -> SourceSpan {
        let node = &self.nodes[n];
        match self.tokens.get(node.first_token..node.end_token) {
            Some([first, .., last]) => SourceSpan::new(first.span.get_start_offset(), last.span.get_end_offset()),
            Some([leaf]) => leaf.span,
            _ => SourceSpan::new(0, 0),
        }
    }
    pub fn error(
        &self,
        span: SourceSpan,
        reason: impl Into<String>,
    ) -> NameError {
        NameError::new(self.package, self.path, span, reason)
    }
}
