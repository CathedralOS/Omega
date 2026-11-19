//! L3 borrows L2's resolved tree and keeps parameter types and logical hypotheses for this file.

use crate::{Access, Site, TypeError, ValueType};
use omega_lexer::SourceSpan;
use omega_resolver::{DeclarationId, NameContext, Symbol, SymbolResolvedTree};
use std::collections::{BTreeMap, HashMap, HashSet};

pub(crate) type ParameterSlot<'a> = (&'a DeclarationId, usize);

pub(crate) struct ResolvedSourceFile<'a> {
    pub(crate) tree: SymbolResolvedTree<'a>,
    /// Inferred slots are keyed by binder node. Authored slots retain the requirement annotation.
    pub(crate) inferred_callbacks: BTreeMap<usize, ParameterSlot<'a>>,
    pub(crate) nominal_callbacks: Vec<(usize, ParameterSlot<'a>)>,
    pub(crate) universe_binders: HashSet<usize>,
    pub(crate) parameters: HashMap<SourceSpan, ValueType>,
    pub(crate) hypotheses: HashMap<SourceSpan, Site>,
}
impl<'a> std::ops::Deref for ResolvedSourceFile<'a> {
    type Target = SymbolResolvedTree<'a>;
    fn deref(&self) -> &Self::Target {
        &self.tree
    }
}

impl<'a> ResolvedSourceFile<'a> {
    pub(crate) fn require_selection_authority(
        &self,
        node: usize,
        declaration: &DeclarationId,
    ) -> Result<(), TypeError> {
        if self.can_select_from(&declaration.package) {
            Ok(())
        } else {
            Err(TypeError::at(
                self.file,
                node,
                format!(
                    "selecting {}::{} requires a direct dependency on {}",
                    declaration.package, declaration.path, declaration.package
                ),
            ))
        }
    }

    pub(crate) fn symbol(
        &self,
        node: usize,
    ) -> Option<&'a Symbol> {
        match self.tree.symbol(node)? {
            Symbol::ScopedNames {
                context: NameContext::Type,
                names,
            } => self.select_name(names, true),
            Symbol::ScopedNames {
                context: NameContext::Value,
                names,
            } => self.select_name(names, false),
            symbol => Some(symbol),
        }
    }

    pub(crate) fn check_signature_selection(
        &self,
        node: usize,
        declaration: &DeclarationId,
    ) -> Result<(), TypeError> {
        if self.private_signature_candidates(node).contains(declaration) {
            return Err(TypeError::at(
                self.file,
                node,
                format!("public signature exposes private declaration {}", self.file.nodes[node].path()),
            ));
        }
        Ok(())
    }

    pub(crate) fn names_type(
        &self,
        symbol: &Symbol,
    ) -> bool {
        use omega_parser::NodeKind::*;
        match symbol {
            Symbol::Builtin(name) => omega_resolver::is_builtin_type_name(name),
            Symbol::Declaration(id) => matches!(id.kind, Data | Trait | Domain),
            Symbol::DomainCandidates { .. } => true,
            Symbol::Binding {
                kind: TypeParam | SomeParam, ..
            } => true,
            Symbol::Binding {
                kind: ConstParam | KindedParam,
                binder_node,
                ..
            } => self.universe_binders.contains(binder_node),
            _ => false,
        }
    }

    pub(crate) fn name_in_role(
        &self,
        node: usize,
        type_name: bool,
    ) -> Option<&'a Symbol> {
        match self.symbol(node)? {
            Symbol::ScopedNames { names, .. } => self.select_name(names, type_name),
            name => Some(name),
        }
    }

    pub(crate) fn select_name(
        &self,
        names: &'a [Symbol],
        type_name: bool,
    ) -> Option<&'a Symbol> {
        names.iter().find(|name| self.names_type(name) == type_name)
    }

    pub(crate) fn equality_operands(
        &self,
        node: usize,
    ) -> Result<Option<(usize, usize)>, TypeError> {
        if self.file.operator(node).is_none_or(|op| op.token != "==") {
            return Ok(None);
        }
        let [left, right] = *self.file.nodes[node].children.as_slice() else {
            return Err(TypeError::unsupported(self.file, node));
        };
        let (left, right) = (self.file.ungroup(left), self.file.ungroup(right));
        let is_type = |operand| self.symbol(operand).is_some_and(|name| self.names_type(name));
        match (is_type(left), is_type(right)) {
            (true, true) => Ok(Some((left, right))),
            (false, false) => Ok(None),
            _ => Err(TypeError::at(self.file, node, "type equality mixes a type and a value")),
        }
    }

    pub(crate) fn declaration(
        &self,
        node_index: usize,
    ) -> Result<&'a DeclarationId, TypeError> {
        self.declaration_at(node_index)
            .ok_or_else(|| TypeError::at(self.file, node_index, "missing L2 definition"))
    }
    pub(crate) fn single_child(
        &self,
        node_index: usize,
    ) -> Result<usize, TypeError> {
        match self.file.nodes[node_index].children.as_slice() {
            [child] => Ok(*child),
            _ => Err(TypeError::unsupported(self.file, node_index)),
        }
    }
}

impl<'a> ResolvedSourceFile<'a> {
    pub(crate) fn parameter_annotation(
        &self,
        binder_node: usize,
    ) -> Option<usize> {
        let binder = &self.file.nodes[binder_node];
        if binder.kind != omega_parser::NodeKind::Param {
            return None;
        }
        binder.children.first().copied()
    }
    pub(crate) fn operator_token(
        &self,
        node: usize,
    ) -> Result<&'a str, TypeError> {
        self.file
            .operator(node)
            .map(|op| op.token)
            .ok_or_else(|| TypeError::at(self.file, node, "missing syntax operator"))
    }
    pub(crate) fn member_name(
        &self,
        node: usize,
    ) -> Result<&'a str, TypeError> {
        self.member(node)
            .ok_or_else(|| TypeError::at(self.file, node, "missing resolved member"))
    }
    pub fn access(
        &self,
        node_index: usize,
    ) -> Access {
        match self.file.nodes[node_index].words().next().map(|w| w.text) {
            Some("mut") => Access::Mut,
            Some("write") => Access::Write,
            _ => Access::Shared,
        }
    }
}
