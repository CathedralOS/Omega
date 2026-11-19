use super::{Carrier, Catalog, Context, NameError, NameUse, Symbol, callable, qualify, usage};
use crate::syntax::File;
use omega_lexer::SourceSpan;
use omega_parser::NodeKind;

impl Catalog<'_> {
    pub(super) fn index_trait_parents(
        &mut self,
        contexts: &[Context<'_>],
    ) -> Result<(), NameError> {
        let mut direct = std::collections::BTreeMap::new();
        for entry in self.entries.iter().filter(|e| e.id.kind == NodeKind::Trait) {
            let file = &self.files[entry.file];
            let mut parents = vec![];
            for &base in file.nodes[entry.node]
                .children
                .iter()
                .filter(|&&n| file.nodes[n].kind == NodeKind::PathType)
            {
                let name = file.nodes[base].path();
                let ids = self.lookup(file, entry.file, &contexts[entry.file], base, &name);
                let mut traits = ids
                    .iter()
                    .map(|&i| &self.entries[i])
                    .filter(|e| e.id.kind == NodeKind::Trait);
                let Some(parent) = traits.next() else {
                    return Err(file.error(file.span(base), format!("unresolved trait {name}")));
                };
                if traits.any(|e| e.id != parent.id) {
                    return Err(file.error(file.span(base), format!("ambiguous trait {name}")));
                }
                let boundary = |e: &super::Entry| {
                    self.files[e.file].nodes[e.node]
                        .words()
                        .take_while(|w| w.text != "trait")
                        .any(|w| w.text == "boundary")
                };
                if boundary(parent) && !boundary(entry) {
                    return Err(file.error(file.span(base), "ordinary trait inherits a boundary trait"));
                }
                parents.push((parent.id.package.clone(), parent.id.path.clone()));
            }
            direct.insert((entry.id.package.clone(), entry.id.path.clone()), parents);
        }
        for (key, parents) in &direct {
            let mut reached = std::collections::BTreeSet::new();
            let mut todo = parents.clone();
            while let Some(parent) = todo.pop() {
                if &parent == key {
                    let e = &self.entries[self.index[key][0]];
                    return Err(self.files[e.file].error(self.files[e.file].span(e.node), "cyclic trait inheritance"));
                }
                if reached.insert(parent.clone())
                    && let Some(next) = direct.get(&parent)
                {
                    todo.extend(next.iter().cloned());
                }
            }
            self.trait_parents.insert(key.clone(), reached);
        }
        Ok(())
    }
    pub(super) fn require_carrier(
        &self,
        file: &File<'_>,
        domain: usize,
        span: SourceSpan,
    ) -> Result<(), NameError> {
        if let Some(Carrier::Named(slot)) = self.carriers.get(&domain) {
            let carrier = &self.entries[*slot];
            if carrier.id.package != file.package
                && (!carrier.public
                    || !self
                        .edges
                        .iter()
                        .any(|(&(from, _), &to)| from == file.package && to == carrier.id.package))
            {
                return Err(file.error(span, format!("no authority to name carrier {}::{}", carrier.id.package, carrier.id.path)));
            }
        }
        Ok(())
    }
    fn address(
        &self,
        file: &File<'_>,
        path: &str,
    ) -> (String, String) {
        let (root, rest) = path.split_once("::").unwrap_or((path, ""));
        match self.edges.get(&(file.package, root)) {
            Some(&package) => (package.into(), rest.into()),
            None => (file.package.into(), path.into()),
        }
    }
    pub(super) fn visible(
        &self,
        file: &File<'_>,
        address: &(String, String),
    ) -> Vec<usize> {
        let mut slots: Vec<_> = self
            .index
            .get(address)
            .into_iter()
            .flatten()
            .copied()
            .filter(|&i| {
                let e = &self.entries[i];
                e.lexical_owner.is_none() && (e.id.package == file.package || e.public)
            })
            .collect();
        if let Some((schema, member)) = address.1.rsplit_once("::")
            && let Some(parents) = self.trait_parents.get(&(address.0.clone(), schema.into()))
        {
            // The selected child carries its parents' requirements under their original owners.
            let child = self
                .index
                .get(&(address.0.clone(), schema.into()))
                .into_iter()
                .flatten()
                .any(|&i| self.entries[i].public || file.package == address.0);
            if child {
                for (p, t) in parents {
                    slots.extend(
                        self.index
                            .get(&(p.clone(), qualify(t, member)))
                            .into_iter()
                            .flatten()
                            .copied()
                            .filter(|&i| callable(self.entries[i].id.kind) && (file.package == *p || self.entries[i].public)),
                    );
                }
            }
        }
        slots
    }
    pub(super) fn imports(
        &self,
        file: &File<'_>,
        ctx: &mut Context<'_>,
        uses: &mut Vec<NameUse>,
        collecting_trait_parents: bool,
    ) -> Result<(), NameError> {
        for (n, _) in file
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.kind == NodeKind::Use)
        {
            let Some(h) = file.child(n, NodeKind::Path) else { continue };
            ctx.heads.insert(h);
            let path = file.nodes[h].path();
            let address = self.address(file, &path);
            let span = file.span(h);
            let alias = path.rsplit("::").next().unwrap_or(&path);
            let slots = self.visible(file, &address);
            let target = if !slots.is_empty() {
                ctx.exposed_domains.extend(
                    slots
                        .iter()
                        .copied()
                        .filter(|&i| self.entries[i].id.kind == NodeKind::Domain)
                        .map(|i| (i, span)),
                );
                ctx.imports.entry(alias.into()).or_default().extend(&slots);
                self.symbol(file, span, &slots)?
            } else if self.modules.contains(&address) {
                ctx.modules
                    .entry(alias.into())
                    .or_default()
                    .push(address.clone());
                // A module exposes its direct declarations. Its own imports and descendants stay private to it.
                for ((p, name), members) in self.index {
                    if *p != address.0 {
                        continue;
                    }
                    let (owner, leaf) = name.rsplit_once("::").unwrap_or(("", name));
                    ctx.exposed_domains.extend(
                        members
                            .iter()
                            .copied()
                            .filter(|&i| {
                                let e = &self.entries[i];
                                e.id.kind == NodeKind::Domain && self.files[e.file].namespace == address.1 && (e.public || p == file.package)
                            })
                            .map(|i| (i, span)),
                    );
                    if owner == address.1 {
                        ctx.imports
                            .entry(leaf.into())
                            .or_default()
                            .extend(members.iter().copied().filter(|&i| {
                                let e = &self.entries[i];
                                e.lexical_owner.is_none() && (e.public || p == file.package)
                            }));
                    }
                }
                Symbol::Module {
                    package: address.0,
                    path: address.1,
                }
            } else if collecting_trait_parents
                && address.1.rsplit_once("::").is_some_and(|(owner, _)| {
                    self.visible(file, &(address.0.clone(), owner.into()))
                        .iter()
                        .any(|&i| self.entries[i].id.kind == NodeKind::Trait)
                })
            {
                // This member can originate on a parent whose own file imports its base.
                // The final pass checks the address against the expanded requirement set.
                continue;
            } else {
                return Err(file.error(span, format!("unavailable import {path}")));
            };
            uses.push(usage(file, span, target));
        }
        Ok(())
    }
    pub(super) fn symbol(
        &self,
        file: &File<'_>,
        span: SourceSpan,
        slots: &[usize],
    ) -> Result<Symbol, NameError> {
        let mut unique = slots.to_vec();
        unique.sort_unstable();
        unique.dedup();
        let ids: Vec<_> = unique.into_iter().map(|i| self.entries[i].id.clone()).collect();
        match ids.as_slice() {
            [id] => Ok(Symbol::Declaration(id.clone())),
            [first, ..]
                if ids
                    .iter()
                    .all(|id| callable(id.kind) && id.package == first.package && id.path == first.path) =>
            {
                Ok(Symbol::Candidates(ids))
            }
            _ => Err(file.error(
                span,
                format!(
                    "competing declarations: {}",
                    ids.iter()
                        .map(|id| format!("{}::{}", id.package, id.path))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )),
        }
    }
    pub(super) fn occurrence_symbol(
        &self,
        file: &File<'_>,
        ctx: &Context<'_>,
        span: SourceSpan,
        slots: &[usize],
    ) -> Result<Symbol, NameError> {
        self.symbol(file, span, slots).map_err(|mut error| {
            for slot in slots.iter().copied().collect::<std::collections::BTreeSet<_>>() {
                let Some(carrier) = self.carriers.get(&slot) else { continue };
                let owner = &self.entries[slot].id;
                let carrier = match carrier {
                    Carrier::Named(i) => {
                        let id = &self.entries[*i].id;
                        format!("{}::{}", id.package, id.path)
                    }
                    Carrier::Builtin(name) => name.clone(),
                    Carrier::Generic => "generic carrier".into(),
                };
                error
                    .message
                    .push_str(&format!(" ({}::{} on {carrier}", owner.package, owner.path));
                if let Some(import) = ctx.exposed_domains.get(&slot) {
                    error
                        .message
                        .push_str(&format!(", import {}@{}..{}", file.path, import.get_start_offset(), import.get_end_offset()));
                }
                error.message.push(')');
            }
            error
        })
    }
    pub(super) fn lookup(
        &self,
        file: &File<'_>,
        file_number: usize,
        ctx: &Context<'_>,
        n: usize,
        path: &str,
    ) -> Vec<usize> {
        for owner in file.enclosing_contexts(n) {
            let Some(&entry) = ctx.entries.get(&owner) else { continue };
            let address = (file.package.into(), qualify(&self.entries[entry].id.path, path));
            if let Some(slots) = self.index.get(&address) {
                let visible: Vec<_> = slots
                    .iter()
                    .copied()
                    .filter(|&i| {
                        let e = &self.entries[i];
                        e.lexical_owner.is_none() || (e.file == file_number && e.lexical_owner == Some(owner))
                    })
                    .collect();
                if !visible.is_empty() {
                    return visible;
                }
            }
        }
        let local = self.visible(file, &(file.package.into(), qualify(&file.namespace, path)));
        let mut preferred: Vec<_> = local
            .iter()
            .copied()
            .filter(|&i| self.entries[i].file == file_number)
            .collect();
        if let Some(imported) = ctx.imports.get(path) {
            preferred.extend(imported);
        }
        if let Some((root, rest)) = path.split_once("::") {
            if let Some(modules) = ctx.modules.get(root) {
                for (p, base) in modules {
                    preferred.extend(self.visible(file, &(p.clone(), qualify(base, rest))));
                }
            }
            if let Some(imported) = ctx.imports.get(root) {
                for &i in imported {
                    let id = &self.entries[i].id;
                    preferred.extend(self.visible(file, &(id.package.clone(), qualify(&id.path, rest))));
                }
            }
        }
        if !preferred.is_empty() {
            return preferred;
        }
        if !local.is_empty() {
            return local;
        }
        if path.contains("::") {
            self.visible(file, &self.address(file, path))
        } else {
            vec![]
        }
    }
}
