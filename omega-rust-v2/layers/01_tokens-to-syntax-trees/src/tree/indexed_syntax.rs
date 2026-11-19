//! Random access to the parser event stream for consumers that need subtree and token ranges.
use crate::{NodeKind, SyntaxTree, TreeEvent};
use omega_lexer::{SourceSpan, Token, TokenKind};

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
    pub end: usize,
    pub children: Vec<usize>,
    pub leaves: Vec<Leaf<'a>>,
    pub first_token: usize,
    pub end_token: usize,
}

impl<'a> Node<'a> {
    /// The flag as written. The meaning of a bare clause depends on its declaration.
    pub fn pause_flag(&self) -> Option<PauseFlag> {
        if !matches!(self.kind, NodeKind::Blocks | NodeKind::Suspends) || !self.children.is_empty() {
            return None;
        }
        match self.leaves.as_slice() {
            [_] => Some(PauseFlag::Omitted),
            [_, flag] => match flag.text {
                "true" => Some(PauseFlag::Explicit(true)),
                "false" => Some(PauseFlag::Explicit(false)),
                _ => None,
            },
            _ => None,
        }
    }

    /// Only the clause's own separators distinguish an order from a bound.
    /// Words and punctuation inside the expressions belong to their child nodes.
    pub fn termination_clause(&self) -> Option<TerminationClause> {
        if self.kind != NodeKind::Terminates {
            return None;
        }
        let (separators, measure, remaining) = match (self.leaves.as_slice(), self.children.as_slice()) {
            ([_], []) => return Some(TerminationClause::Bare),
            ([_, by, separators @ ..], [measure, remaining @ ..]) if by.text == "by" => (separators, *measure, remaining),
            _ => return None,
        };
        let (order, bound) = match (separators, remaining) {
            ([], []) => (None, None),
            ([arrow], [order]) if arrow.text == "->" => (Some(*order), None),
            ([in_word], [bound]) if in_word.text == "in" => (None, Some(*bound)),
            ([arrow, in_word], [order, bound]) if arrow.text == "->" && in_word.text == "in" => (Some(*order), Some(*bound)),
            _ => return None,
        };
        Some(TerminationClause::By { measure, order, bound })
    }

    pub fn words(&self) -> impl DoubleEndedIterator<Item = Leaf<'a>> + '_ {
        self.leaves
            .iter()
            .copied()
            .filter(|leaf| leaf.kind == TokenKind::Word)
    }
    /// A contextual word can also be the name of a declaration.
    pub fn conformance_name(&self) -> Option<Leaf<'a>> {
        if self.kind != NodeKind::Conformance {
            return None;
        }
        self.words()
            .rev()
            .skip_while(|word| word.text != "satisfies")
            .nth(1)
    }
    pub fn has_public_modifier(&self) -> bool {
        use NodeKind::*;
        matches!(self.kind, Const | Data | Domain | Trait | Conformance | Machine | AsmMachine)
            && self
                .leaves
                .first()
                .is_some_and(|leaf| leaf.text == "public" && Some(*leaf) != self.conformance_name())
    }
    pub fn has_boundary_modifier(&self) -> bool {
        matches!(self.kind, NodeKind::Trait | NodeKind::Machine | NodeKind::MathLet)
            && self
                .words()
                .nth(usize::from(self.has_public_modifier()))
                .is_some_and(|leaf| leaf.text == "boundary")
    }
    /// The name introduced by a parameter or hypothesis, at its original source span.
    pub fn binder_name(&self) -> Option<Leaf<'a>> {
        use NodeKind::*;
        match self.kind {
            ConstParam | MachineParam | SomeParam => self.words().nth(1),
            Hypothesis => self.words().nth(2),
            Param | TypeParam | KindedParam | LifetimeParam => self.words().next(),
            _ => None,
        }
    }
    /// Register names written in an assembly clause, in source order.
    pub fn register_names(&self) -> impl Iterator<Item = Leaf<'a>> + '_ {
        let names = match self.leaves.as_slice() {
            [_, name] if self.kind == NodeKind::Clobbers && name.text == "none" => &[][..],
            [_, names @ ..] if matches!(self.kind, NodeKind::Registers | NodeKind::Clobbers) => names,
            _ => &[],
        };
        names
            .iter()
            .copied()
            .filter(|leaf| leaf.kind == TokenKind::Word)
    }

    /// The mnemonic as the target spells it, including an optional dotted suffix.
    pub fn instruction_name(&self) -> Option<(String, SourceSpan)> {
        if self.kind != NodeKind::Instruction {
            return None;
        }
        let first = self.leaves.first()?;
        if let [_, dot, suffix, ..] = self.leaves.as_slice()
            && dot.text == "."
        {
            return Some((
                format!("{}.{}", first.text, suffix.text),
                SourceSpan::new(first.span.get_start_offset(), suffix.span.get_end_offset()),
            ));
        }
        Some((first.text.to_owned(), first.span))
    }

    /// Source range through the name before the last path segment.
    pub fn qualifier_span(&self) -> Option<SourceSpan> {
        let first = self.words().next()?;
        let qualifier = self.words().rev().nth(1)?;
        Some(SourceSpan::new(first.span.get_start_offset(), qualifier.span.get_end_offset()))
    }

    pub fn path(&self) -> String {
        self.words()
            .map(|leaf| leaf.text)
            .collect::<Vec<_>>()
            .join("::")
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct IndexedSyntax<'a> {
    source: &'a str,
    pub nodes: Vec<Node<'a>>,
    pub tokens: Vec<Leaf<'a>>,
}
impl SyntaxTree {
    /// Borrow spellings from the source and index this tree in preorder.
    /// Returns None if its events or token spans cannot be mapped to the supplied inputs.
    pub fn index<'a>(
        &self,
        source: &'a str,
        lexed: &[Token],
    ) -> Option<IndexedSyntax<'a>> {
        let mut nodes: Vec<Node<'a>> = Vec::new();
        let mut tokens = Vec::new();
        let mut token_indices = self.get_meaningful_token_indices().iter();
        let mut stack = Vec::<usize>::new();
        for event in self.get_events() {
            match *event {
                TreeEvent::Open(kind) => {
                    let index = nodes.len();
                    let parent = stack.last().copied();
                    if let Some(owner) = parent {
                        nodes[owner].children.push(index);
                    }
                    nodes.push(Node {
                        kind,
                        parent,
                        end: 0,
                        children: vec![],
                        leaves: vec![],
                        first_token: tokens.len(),
                        end_token: 0,
                    });
                    stack.push(index);
                }
                TreeEvent::Token => {
                    let token = token_indices.next().and_then(|&i| lexed.get(i as usize))?;
                    let span = token.get_span();
                    let text = source.get(span.get_start_offset()..span.get_end_offset())?;
                    let leaf = Leaf {
                        text,
                        span,
                        kind: token.get_kind(),
                    };
                    nodes[*stack.last()?].leaves.push(leaf);
                    tokens.push(leaf);
                }
                TreeEvent::Close => {
                    let index = stack.pop()?;
                    nodes[index].end = nodes.len();
                    nodes[index].end_token = tokens.len();
                }
            }
        }
        (stack.is_empty() && token_indices.next().is_none()).then_some(IndexedSyntax { source, nodes, tokens })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseFlag {
    Omitted,
    Explicit(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminationClause {
    Bare,
    By {
        measure: usize,
        order: Option<usize>,
        bound: Option<usize>,
    },
}

/// Written range endpoints in this syntax tree. An omitted endpoint has no node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RangeBounds {
    pub start: Option<usize>,
    pub end: Option<usize>,
    pub inclusive: bool,
}

/// The parameter list, result annotation and optional defining expression of a mathematical let.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MathBinding {
    pub parameters: usize,
    pub result: usize,
    pub body: Option<usize>,
}

/// Spelling and byte range of an operator occurrence in the source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyntaxOperator<'a> {
    pub token: &'a str,
    pub span: SourceSpan,
}
impl<'a> IndexedSyntax<'a> {
    /// Walk from the immediate parent to the root in grammatical nesting order.
    pub fn ancestors(
        &self,
        n: usize,
    ) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(self.nodes[n].parent, |&p| self.nodes[p].parent)
    }
    pub fn pattern_owner(
        &self,
        node: usize,
    ) -> usize {
        self.ancestors(node)
            .find(|&p| matches!(self.nodes[p].kind, NodeKind::Let | NodeKind::Arm))
            .expect("a pattern belongs to a let or dispatch arm")
    }
    /// Written binders in the declaration's generic parameter list.
    pub fn generic_parameters(
        &self,
        owner: usize,
    ) -> impl Iterator<Item = (Leaf<'a>, usize, NodeKind)> + '_ {
        self.child(owner, NodeKind::GenericParams)
            .into_iter()
            .flat_map(|g| &self.nodes[g].children)
            .map(|&node| (self.nodes[node].binder_name().expect("named generic parameter"), node, self.nodes[node].kind))
    }

    /// The path node for a qualified declaration head, or the word introducing a simple name.
    pub fn declaration_name(
        &self,
        node: usize,
    ) -> Option<(Option<usize>, String, SourceSpan)> {
        use NodeKind::*;
        let syntax = &self.nodes[node];
        let simple = match syntax.kind {
            Machine | AsmMachine | Const | Domain => {
                let path = self.child(node, if syntax.kind == Domain { PathType } else { Path })?;
                return Some((Some(path), self.nodes[path].path(), self.span(path)));
            }
            Field | CaseField => syntax.words().next(),
            Conformance => syntax.conformance_name(),
            kind => {
                let keyword = match kind {
                    Data => "data",
                    Trait => "trait",
                    MathLet => "let",
                    Case => "case",
                    _ => return None,
                };
                syntax.words().skip_while(|w| w.text != keyword).nth(1)
            }
        }?;
        Some((None, simple.text.to_owned(), simple.span))
    }

    /// Runtime parameter names and annotations in their declared order.
    pub fn parameters(
        &self,
        owner: usize,
    ) -> impl Iterator<Item = (Leaf<'a>, usize)> + '_ {
        self.child(owner, NodeKind::Params)
            .into_iter()
            .flat_map(|list| &self.nodes[list].children)
            .map(|&p| {
                let param = &self.nodes[p];
                (param.binder_name().expect("named parameter"), param.children[0])
            })
    }

    pub fn result_annotation(
        &self,
        owner: usize,
    ) -> Option<usize> {
        self.child(owner, NodeKind::Result)
            .map(|result| self.nodes[result].children[0])
    }

    pub fn math_binding(
        &self,
        index: usize,
    ) -> Option<MathBinding> {
        let node = &self.nodes[index];
        if node.kind != NodeKind::MathLet {
            return None;
        }
        let start = usize::from(self.nodes[node.children[0]].kind == NodeKind::GenericParams);
        let parts = &node.children[start..];
        Some(MathBinding {
            parameters: parts[0],
            result: parts[1],
            body: parts.get(2).copied(),
        })
    }
    pub fn operator(
        &self,
        n: usize,
    ) -> Option<SyntaxOperator<'a>> {
        let node = self.nodes.get(n)?;
        if !matches!(node.kind, NodeKind::Binary | NodeKind::Unary | NodeKind::Index | NodeKind::Range) {
            return None;
        }
        let span = SourceSpan::new(node.leaves.first()?.span.get_start_offset(), node.leaves.last()?.span.get_end_offset());
        let token = if node.kind == NodeKind::Index {
            "[]"
        } else {
            self.source
                .get(span.get_start_offset()..span.get_end_offset())?
        };
        Some(SyntaxOperator { token, span })
    }

    pub fn range_bounds(
        &self,
        index: usize,
    ) -> Option<RangeBounds> {
        let node = &self.nodes[index];
        if node.kind != NodeKind::Range {
            return None;
        }
        Some(RangeBounds {
            start: node
                .children
                .first()
                .copied()
                .filter(|&n| self.nodes[n].first_token == node.first_token),
            end: node
                .children
                .last()
                .copied()
                .filter(|&n| self.nodes[n].end_token == node.end_token),
            inclusive: node.leaves[0].text == "..=",
        })
    }

    /// The argument list on the last segment of a path, as in the Tag<2> of Box<u8>::Tag<2>.
    pub fn final_segment_arguments(
        &self,
        path: usize,
    ) -> Option<usize> {
        let node = &self.nodes[path];
        let &last = node.children.last()?;
        let section = &self.nodes[last];
        (matches!(section.kind, NodeKind::TypeArgs | NodeKind::ExpressionArgs) && section.end_token == node.end_token).then_some(last)
    }

    /// For an application on a nonterminal path segment, return the qualified
    /// name through that segment. Arguments and trivia are excluded from the
    /// spelling; the span still points into the original source.
    pub fn application_prefix(
        &self,
        section: usize,
    ) -> Option<(String, SourceSpan)> {
        let arguments = &self.nodes[section];
        if !matches!(arguments.kind, NodeKind::TypeArgs | NodeKind::ExpressionArgs) {
            return None;
        }
        let path = &self.nodes[arguments.parent?];
        if arguments.end_token == path.end_token {
            return None;
        }
        let start = self.tokens[arguments.first_token].span.get_start_offset();
        let mut words = path
            .words()
            .take_while(|word| word.span.get_start_offset() < start);
        let first = words.next()?;
        let mut prefix = first.text.to_owned();
        let mut end = first.span;
        for word in words {
            prefix.push_str("::");
            prefix.push_str(word.text);
            end = word.span;
        }
        Some((prefix, SourceSpan::new(first.span.get_start_offset(), end.get_end_offset())))
    }

    /// Argument lists on the referenced path. A domain head's final segment introduces its own indices.
    pub fn reference_sections(
        &self,
        path: usize,
    ) -> &[usize] {
        let node = &self.nodes[path];
        let sections = node.children.as_slice();
        if node
            .parent
            .is_some_and(|p| self.nodes[p].kind == NodeKind::Domain && self.child(p, NodeKind::PathType) == Some(path))
            && self.final_segment_arguments(path).is_some()
        {
            &sections[..sections.len() - 1]
        } else {
            sections
        }
    }

    /// The name at the head of a generic argument and its written domain qualifiers.
    pub fn argument_head(
        &self,
        argument: usize,
    ) -> (usize, &[usize]) {
        let node = &self.nodes[argument];
        match node.children.split_first() {
            Some((&head, domains)) if node.kind == NodeKind::QualifiedType => (head, domains),
            _ => (argument, &[]),
        }
    }

    /// The argument section and slot that receive this node or its qualified head.
    pub fn argument_position(
        &self,
        node: usize,
    ) -> Option<(usize, usize)> {
        let parent = self.nodes.get(node)?.parent?;
        let argument = if self.argument_head(parent).0 == node { parent } else { node };
        let section = self.nodes[argument].parent?;
        let list = &self.nodes[section];
        if !matches!(list.kind, NodeKind::TypeArgs | NodeKind::ExpressionArgs) {
            return None;
        }
        Some((section, list.children.binary_search(&argument).ok()?))
    }

    /// The expression inside any grouping parentheses around this node.
    pub fn ungroup(
        &self,
        mut node: usize,
    ) -> usize {
        while self.nodes[node].kind == NodeKind::Paren {
            let [inner] = *self.nodes[node].children.as_slice() else { break };
            node = inner;
        }
        node
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
}
