//! L2 retains its indexed syntax and binds each file's occurrences to its
//! declarations or candidates. Only text and file identities borrow from the caller.
use crate::{DeclarationId, IndexedSource, NameRole, NameUse, ResolvedStage, Symbol};
use std::{collections::BTreeMap, ops::Deref, sync::Arc};

#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct TreeBindings {
    pub(crate) uses: Vec<NameUse>,
    pub(crate) exports: Vec<usize>,
    pub(crate) definitions: BTreeMap<usize, usize>,
    pub(crate) applications: BTreeMap<usize, usize>,
    pub(crate) patterns: BTreeMap<usize, omega_lexer::SourceSpan>,
    pub(crate) private_signature_candidates: BTreeMap<usize, Vec<DeclarationId>>,
}

#[derive(Debug, Eq, PartialEq)]
struct Forest<'text> {
    files: Vec<(IndexedSource<'text>, TreeBindings)>,
    resolution: ResolvedStage,
}

/// A clone shares the immutable trees when typing under another environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolResolvedTrees<'text>(Arc<Forest<'text>>);

impl<'text> SymbolResolvedTrees<'text> {
    pub fn compiled_interface(&self) -> crate::CompiledInterface {
        let mut interface = crate::CompiledInterface::default();
        for tree in self.trees() {
            interface
                .modules
                .insert((tree.file.package.into(), tree.file.namespace.clone()));
            interface.declarations.extend(tree.exports());
        }
        interface
    }
    pub(crate) fn new(
        files: Vec<IndexedSource<'text>>,
        bindings: Vec<TreeBindings>,
        resolution: ResolvedStage,
    ) -> Self {
        let files = files.into_iter().zip(bindings).collect();
        Self(Arc::new(Forest { files, resolution }))
    }

    pub fn uses(&self) -> impl Iterator<Item = &NameUse> {
        self.trees().flat_map(|tree| tree.occurrences())
    }

    pub fn trees(&self) -> impl ExactSizeIterator<Item = SymbolResolvedTree<'_>> {
        self.0.files.iter().map(|(file, bindings)| SymbolResolvedTree {
            file,
            bindings,
            resolution: &self.0.resolution,
        })
    }
}

impl Deref for SymbolResolvedTrees<'_> {
    type Target = ResolvedStage;
    fn deref(&self) -> &Self::Target {
        &self.0.resolution
    }
}

/// Each lookup resolves an occurrence within this tree's file.
#[derive(Clone, Copy)]
pub struct SymbolResolvedTree<'a> {
    pub file: &'a IndexedSource<'a>,
    bindings: &'a TreeBindings,
    resolution: &'a ResolvedStage,
}

impl<'a> SymbolResolvedTree<'a> {
    /// Selecting a declaration after typing still requires authority in the author's package.
    pub fn can_select_from(
        &self,
        package: &str,
    ) -> bool {
        self.file.package == package
            || self
                .resolution
                .direct_dependencies
                .get(self.file.package)
                .is_some_and(|dependencies| dependencies.contains(package))
    }

    /// Resolved occurrences in this file, in syntax order.
    pub fn occurrences(self) -> impl ExactSizeIterator<Item = &'a NameUse> + 'a {
        self.bindings.uses.iter()
    }
    /// A field shorthand has both a member occurrence and a value reference.
    pub fn names(
        &self,
        node: usize,
    ) -> impl Iterator<Item = &'a NameUse> {
        let uses = &self.bindings.uses;
        let first = uses.partition_point(|name| name.node.node < node);
        uses[first..]
            .iter()
            .take_while(move |name| name.node.node == node)
    }
    pub fn symbol(
        &self,
        node: usize,
    ) -> Option<&'a Symbol> {
        self.names(node)
            .find(|name| name.role == NameRole::Reference)
            .map(|name| &name.target)
    }
    /// Private overloads retained for selection at a public signature occurrence.
    pub fn private_signature_candidates(
        &self,
        node: usize,
    ) -> &'a [DeclarationId] {
        self.bindings
            .private_signature_candidates
            .get(&node)
            .map_or(&[], Vec::as_slice)
    }
    pub fn pattern_binding(
        &self,
        node: usize,
    ) -> Option<omega_lexer::SourceSpan> {
        self.bindings.patterns.get(&node).copied()
    }
    pub fn qualifier(
        &self,
        node: usize,
    ) -> Option<&'a Symbol> {
        self.names(node)
            .filter(|name| name.role == NameRole::Qualifier)
            .last()
            .map(|name| &name.target)
    }
    pub fn member(
        &self,
        node: usize,
    ) -> Option<&'a str> {
        self.names(node).find_map(|name| match &name.target {
            Symbol::Member(member) => Some(member.as_str()),
            _ => None,
        })
    }
    pub fn declaration_at(
        &self,
        node: usize,
    ) -> Option<&'a DeclarationId> {
        self.bindings
            .definitions
            .get(&node)
            .map(|&slot| &self.resolution.declarations[slot])
    }
    pub fn declaration_nodes(self) -> impl Iterator<Item = (usize, &'a DeclarationId)> + 'a {
        self.bindings
            .definitions
            .iter()
            .map(move |(&node, &i)| (node, &self.resolution.declarations[i]))
    }
    /// Names introduced by this file that another package may import.
    pub fn exports(self) -> impl Iterator<Item = crate::InterfaceExport> + 'a {
        self.bindings
            .exports
            .iter()
            .map(move |slot| crate::InterfaceExport {
                id: self.resolution.declarations[*slot].clone(),
                module: self.file.namespace.clone(),
                domain_carrier: self.resolution.domain_carriers.get(slot).cloned(),
                trait_parents: self
                    .resolution
                    .direct_trait_parents
                    .get(slot)
                    .into_iter()
                    .flatten()
                    .map(|&parent| self.resolution.declarations[parent].clone())
                    .collect(),
            })
    }
    pub fn definitions(self) -> impl Iterator<Item = (omega_lexer::SourceSpan, &'a DeclarationId)> + 'a {
        self.declaration_nodes()
            .map(move |(node, id)| (self.file.span(node), id))
    }
    /// The arguments bound to this declaration on a path with one or more applied segments.
    pub fn arguments_for(
        &self,
        path: usize,
        declaration: &DeclarationId,
    ) -> &'a [usize] {
        self.file.nodes[path]
            .children
            .iter()
            .find_map(|&section| {
                let name = self.application_name(section)?;
                let selected = name.target.visible_names().iter().any(|symbol| match symbol {
                    Symbol::DomainCandidates { candidates, .. } => candidates.iter().any(|c| &c.declaration == declaration),
                    _ => symbol.declaration_candidates().contains(declaration),
                });
                selected.then_some(self.file.nodes[section].children.as_slice())
            })
            .unwrap_or(&[])
    }

    /// Each written argument list and the name it applies to, in syntax order.
    pub fn applications(self) -> impl ExactSizeIterator<Item = (usize, &'a NameUse)> + 'a {
        self.bindings
            .applications
            .iter()
            .map(|(&section, &name)| (section, &self.bindings.uses[name]))
    }

    pub fn application_name(
        &self,
        section: usize,
    ) -> Option<&'a NameUse> {
        self.bindings
            .applications
            .get(&section)
            .map(|&slot| &self.bindings.uses[slot])
    }
}
