use super::{Catalog, Context, DomainCarrier, NameError, NameUse, Symbol, callable, qualify, usage};
use crate::syntax::File;
use omega_lexer::SourceSpan;
use omega_parser::NodeKind;

#[derive(Clone, Copy)]
pub(super) struct Attachment {
    pub slot: usize,
    // Declaration slot and ordinal of the binder whose type role permits this attachment.
    type_binder: Option<(usize, usize)>,
}

impl Context<'_> {
    pub(super) fn carrier_binders(
        &self,
        name: &str,
        node: usize,
    ) -> impl Iterator<Item = &super::Local> {
        use NodeKind::{ConstParam, KindedParam, SomeParam, TypeParam};
        self.locals
            .get(name)
            .into_iter()
            .flatten()
            .rev()
            .filter(move |l| l.static_binding && l.start <= node && node < l.end && matches!(l.kind, TypeParam | SomeParam | KindedParam | ConstParam))
    }
}

impl Catalog<'_> {
    pub(super) fn index_trait_parents(
        &mut self,
        contexts: &[Context<'_>],
        exports: &[&super::InterfaceExport],
        source_count: usize,
    ) -> Result<std::collections::BTreeMap<usize, Vec<usize>>, NameError> {
        let mut direct = std::collections::BTreeMap::new();
        for (offset, export) in exports
            .iter()
            .enumerate()
            .filter(|(_, export)| !export.trait_parents.is_empty())
        {
            let error = |reason| NameError::new(&export.id.package, &export.id.path, SourceSpan::new(0, 0), reason);
            if export.id.kind != NodeKind::Trait {
                return Err(error("only a trait can have parents in a compiled interface"));
            }
            let mut parents = Vec::with_capacity(export.trait_parents.len());
            for parent in &export.trait_parents {
                let slots = self.index.get(&(parent.package.clone(), parent.path.clone()));
                let slot = slots
                    .into_iter()
                    .flatten()
                    .copied()
                    .find(|&i| {
                        let entry = &self.entries[i];
                        entry.public && entry.id.kind == NodeKind::Trait && entry.id == *parent
                    })
                    .ok_or_else(|| error("unknown parent trait in compiled interface"))?;
                parents.push(slot);
            }
            direct.insert(source_count + offset, parents);
        }
        for (slot, entry) in self.entries[..source_count]
            .iter()
            .enumerate()
            .filter(|(_, e)| e.id.kind == NodeKind::Trait)
        {
            let (file_number, node) = entry.source.expect("source trait");
            let file = &self.files[file_number];
            let mut parents = vec![];
            for &base in file.nodes[node]
                .children
                .iter()
                .filter(|&&n| file.nodes[n].kind == NodeKind::PathType)
            {
                let name = file.nodes[base].path();
                let ids = self.lookup(file, &contexts[file_number], base, &name)?;
                let mut traits = ids
                    .into_iter()
                    .filter(|&i| self.entries[i].id.kind == NodeKind::Trait);
                let Some(parent) = traits.next() else {
                    return Err(file.error(file.span(base), format!("unresolved trait {name}")));
                };
                if traits.any(|i| i != parent) {
                    return Err(file.error(file.span(base), format!("ambiguous trait {name}")));
                }
                parents.push(parent);
            }
            if !parents.is_empty() {
                direct.insert(slot, parents);
            }
        }
        let mut children: Vec<_> = direct.keys().copied().collect();
        children.sort_by_key(|&i| (&self.entries[i].id.package, &self.entries[i].id.path));
        for child in children {
            let entry = &self.entries[child];
            let mut reached = std::collections::BTreeSet::new();
            let mut todo = direct[&child].clone();
            while let Some(parent) = todo.pop() {
                if parent == child {
                    return Err(match entry.source {
                        Some((f, n)) => self.files[f].error(self.files[f].span(n), "cyclic trait inheritance"),
                        None => NameError::new(&entry.id.package, &entry.id.path, SourceSpan::new(0, 0), "cyclic trait inheritance"),
                    });
                }
                if reached.insert(parent)
                    && let Some(next) = direct.get(&parent)
                {
                    todo.extend(next.iter().copied());
                }
            }
            let names = reached
                .into_iter()
                .map(|i| {
                    let id = &self.entries[i].id;
                    (id.package.clone(), id.path.clone())
                })
                .collect();
            self.trait_parents
                .insert((entry.id.package.clone(), entry.id.path.clone()), names);
        }
        Ok(direct)
    }
    pub(super) fn require_carrier(
        &self,
        file: &File<'_>,
        domain: usize,
        node: usize,
        span: SourceSpan,
        checks: &mut Vec<super::CarrierVisibility>,
    ) -> Result<(), NameError> {
        let mut owner = domain;
        let mut visited = std::collections::HashSet::new();
        let mut binders = Vec::new();
        while let Some(carrier) = self.carriers.get(&owner) {
            let id = match carrier {
                DomainCarrier::Named(id) => id,
                DomainCarrier::Bound { position, alternative } => {
                    binders.push((self.entries[owner].id.clone(), *position));
                    alternative
                }
                _ => break,
            };
            let slot = self
                .index
                .get(&(id.package.clone(), id.path.clone()))
                .into_iter()
                .flatten()
                .copied()
                .find(|&i| self.entries[i].id == *id);
            let accessible = slot.filter(|&i| {
                id.package == file.package
                    || self.entries[i].public
                        && self
                            .edges
                            .iter()
                            .any(|(&(from, _), &to)| from == file.package && to == id.package)
            });
            let Some(slot) = accessible else {
                let error = file.error(span, format!("no authority to name carrier {}::{}", id.package, id.path));
                if binders.is_empty() {
                    return Err(error);
                }
                checks.push(super::CarrierVisibility {
                    site: file.node_id(node),
                    domain: self.entries[domain].id.clone(),
                    binders,
                    error,
                });
                break;
            };
            if id.kind != NodeKind::Domain || !visited.insert(slot) {
                break;
            }
            owner = slot;
        }
        Ok(())
    }
    /// Imports and fully qualified paths start at a requester-local dependency
    /// alias or at the requester's package root, never at the caller's directory.
    /// An alias that also names a root module or declaration of the requester
    /// makes two modules visible for one path, so the path is refused instead of
    /// letting the caller's directory choose between them.
    fn address(
        &self,
        file: &File<'_>,
        path: &str,
        span: SourceSpan,
    ) -> Result<(String, String), NameError> {
        let (root, rest) = path.split_once("::").unwrap_or((path, ""));
        let Some(&package) = self.edges.get(&(file.package, root)) else {
            return Ok((file.package.into(), path.into()));
        };
        let own = (file.package.to_owned(), root.to_owned());
        if self.modules.contains(&own) || !self.visible(file, &own).is_empty() {
            return Err(file.error(span, format!("competing module paths {root}: package {package} and {}::{root}", file.package)));
        }
        Ok((package.into(), rest.into()))
    }
    fn direct(
        &self,
        file: &File<'_>,
        address: &(String, String),
    ) -> Vec<usize> {
        self.index
            .get(address)
            .into_iter()
            .flatten()
            .copied()
            .filter(|&i| {
                let e = &self.entries[i];
                e.lexical_owner.is_none() && (e.id.package == file.package || e.public)
            })
            .collect()
    }
    pub(super) fn visible(
        &self,
        file: &File<'_>,
        address: &(String, String),
    ) -> Vec<usize> {
        let mut slots = self.direct(file, address);
        self.inherited_members(file, address, &mut slots);
        slots
    }
    pub(super) fn inherited_members(
        &self,
        file: &File<'_>,
        address: &(String, String),
        slots: &mut Vec<usize>,
    ) {
        let Some((schema, member)) = address.1.rsplit_once("::") else { return };
        let schema = (address.0.clone(), schema.to_owned());
        let Some(parents) = self.trait_parents.get(&schema) else { return };
        if !self
            .direct(file, &schema)
            .iter()
            .any(|&slot| self.entries[slot].id.kind == NodeKind::Trait)
        {
            return;
        }
        for (package, parent) in parents {
            let address = (package.clone(), qualify(parent, member));
            slots.extend(
                self.direct(file, &address)
                    .into_iter()
                    .filter(|&i| callable(self.entries[i].id.kind)),
            );
        }
    }
    pub(super) fn collect_imports(
        &self,
        file: &File<'_>,
        ctx: &mut Context<'_>,
    ) -> Result<(), NameError> {
        for node in file.nodes.iter().filter(|node| node.kind == NodeKind::Use) {
            let head = node.children[0];
            let path = file.nodes[head].path();
            let address = self.address(file, &path, file.span(head))?;
            let alias = path.rsplit("::").next().unwrap();
            let site = ctx.imports.len();
            let slots = self.direct(file, &address);
            if self.modules.contains(&address) && slots.is_empty() {
                ctx.open_modules.insert(address.clone());
            }
            ctx.heads.insert(head);
            ctx.by_alias.entry(alias.to_owned()).or_default().push(site);
            ctx.imports.push(super::Import { node: head, address, slots });
        }
        Ok(())
    }

    pub(super) fn bind_imports(
        &self,
        file: &File<'_>,
        ctx: &mut Context<'_>,
        uses: &mut Vec<NameUse>,
        visibility: &mut Vec<super::CarrierVisibility>,
    ) -> Result<(), NameError> {
        let mut expose = |slot: usize, origin| {
            let id = &self.entries[slot].id;
            let leaf = id.path.rsplit("::").next().unwrap();
            ctx.domains
                .entry(leaf.to_owned())
                .or_default()
                .insert(slot, origin);
        };
        let own = (file.package.into(), file.namespace.clone());
        for &slot in self.domains_by_module.get(&own).into_iter().flatten() {
            expose(slot, None);
        }
        for import in &ctx.imports {
            let span = file.span(import.node);
            let (package, path) = &import.address;
            let target = if ctx.open_modules.contains(&import.address) {
                for &slot in self
                    .domains_by_module
                    .get(&import.address)
                    .into_iter()
                    .flatten()
                {
                    if self.entries[slot].public || package == file.package {
                        expose(slot, Some(span));
                    }
                }
                Symbol::Module {
                    package: package.clone(),
                    path: path.clone(),
                }
            } else {
                if import.slots.is_empty() {
                    return Err(file.error(span, format!("unavailable import {}", file.nodes[import.node].path())));
                }
                let target = self.symbol(file, span, &import.slots)?;
                for &slot in &import.slots {
                    if self.carriers.contains_key(&slot) {
                        expose(slot, Some(span));
                    }
                    self.require_carrier(file, slot, import.node, span, visibility)?;
                }
                target
            };
            uses.push(usage(file, import.node, super::NameRole::Reference, span, target));
        }
        Ok(())
    }

    pub(super) fn symbol(
        &self,
        file: &File<'_>,
        span: SourceSpan,
        slots: &[usize],
    ) -> Result<Symbol, NameError> {
        let mut ids: Vec<_> = slots.iter().map(|&i| self.entries[i].id.clone()).collect();
        ids.sort_unstable();
        ids.dedup();
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
    /// In `i32 in Positive::Small`, Positive names an attached domain and
    /// Small names its child. Start at the longest directly visible qualifier
    /// and follow the remaining segments by owner identity. At each step the
    /// caller must have authority to name that attachment.
    pub(super) fn attached_domains(
        &self,
        file: &File<'_>,
        ctx: &Context<'_>,
        node: usize,
        path: &str,
        membership_target: bool,
    ) -> Result<Vec<Attachment>, NameError> {
        let parent = file.nodes[node].parent.map(|p| &file.nodes[p]);
        // A membership test names its domain without a carrier, even when it is written as a
        // qualified generic argument under a value binder.
        let implicit_carrier = parent
            .filter(|p| !membership_target && p.kind == NodeKind::QualifiedType && p.children.first() != Some(&node))
            .map(|p| file.nodes[p.children[0]].path());
        let domain_position = implicit_carrier.is_some() || parent.is_some_and(|p| p.kind == NodeKind::CastEntry) || membership_target;
        if !domain_position && !path.contains("::") {
            return Ok(Vec::new());
        }

        fn split(name: &str) -> (Option<&str>, &str) {
            name.rsplit_once("::")
                .map_or((None, name), |(prefix, leaf)| (Some(prefix), leaf))
        }
        let (mut qualifier, leaf) = split(path);
        let mut unresolved_parents = Vec::new();
        let roots = loop {
            let Some(prefix) = qualifier else {
                qualifier = implicit_carrier.as_deref();
                break match qualifier {
                    Some(name) => self.lookup(file, ctx, node, name)?,
                    None => Vec::new(),
                };
            };
            let direct = self.lookup(file, ctx, node, prefix)?;
            if !domain_position || super::builtin(prefix) || !direct.is_empty() || ctx.carrier_binders(prefix, node).next().is_some() {
                break direct;
            }
            let (outer, segment) = split(prefix);
            unresolved_parents.push(segment);
            qualifier = outer;
        };
        let mut owners: Vec<_> = roots
            .into_iter()
            .map(|slot| Attachment { slot, type_binder: None })
            .collect();
        for segment in unresolved_parents.into_iter().rev().chain([leaf]) {
            let generic = qualifier.is_some_and(|name| {
                super::builtin(name)
                    || ctx.carrier_binders(name, node).next().is_some()
                    || owners
                        .iter()
                        .any(|owner| self.entries[owner.slot].id.kind == NodeKind::Data)
            });
            owners = ctx
                .domains
                .get(segment)
                .into_iter()
                .flat_map(|sites| sites.keys())
                .filter_map(|&slot| {
                    let carrier = &self.carriers[&slot];
                    let named = match carrier {
                        DomainCarrier::Named(owner) | DomainCarrier::Bound { alternative: owner, .. } => owners
                            .iter()
                            .find(|parent| self.entries[parent.slot].id == *owner),
                        _ => None,
                    };
                    let type_binder = match (qualifier, carrier, named) {
                        (None, _, _) => None,
                        (_, _, Some(parent)) => parent.type_binder,
                        (Some(name), DomainCarrier::Builtin(carrier), _) if name == carrier => None,
                        (_, DomainCarrier::Generic, _) if generic => None,
                        (_, DomainCarrier::Bound { position, .. }, _) if generic => Some((slot, *position)),
                        _ => return None,
                    };
                    Some(Attachment { slot, type_binder })
                })
                .collect();
            if owners.is_empty() {
                break;
            }
            qualifier = Some(segment);
        }
        Ok(owners)
    }

    /// A generic domain gets its carrier from this qualifier. An inherited
    /// requirement gets its parent trait arguments from the named child trait.
    pub(super) fn application_qualifier(
        &self,
        file: &File<'_>,
        ctx: &Context<'_>,
        n: usize,
        path: &str,
        candidates: &[usize],
    ) -> Result<Option<NameUse>, NameError> {
        let Some((prefix, _)) = path.rsplit_once("::") else { return Ok(None) };
        let generic = candidates
            .iter()
            .any(|slot| matches!(self.carriers.get(slot), Some(DomainCarrier::Generic | DomainCarrier::Bound { .. })));
        if !generic && !candidates.iter().any(|&s| callable(self.entries[s].id.kind)) {
            return Ok(None);
        }
        let span = file.nodes[n].qualifier_span().expect("qualified path");
        let target = if generic && super::builtin(prefix) {
            Symbol::Builtin(prefix.to_owned())
        } else if generic {
            let mut names: Vec<_> = ctx
                .carrier_binders(prefix, n)
                .map(super::Local::symbol)
                .collect();
            let owners: Vec<_> = self
                .lookup(file, ctx, n, prefix)?
                .into_iter()
                .filter(|&i| self.entries[i].id.kind == NodeKind::Data)
                .collect();
            if !owners.is_empty() {
                names.push(self.symbol(file, span, &owners)?);
            }
            match names.len() {
                0 => return Ok(None),
                1 => names.pop().unwrap(),
                _ => Symbol::ScopedNames {
                    context: super::NameContext::Type,
                    names,
                },
            }
        } else {
            let owners = self.lookup(file, ctx, n, prefix)?;
            let [owner] = owners.as_slice() else { return Ok(None) };
            let id = &self.entries[*owner].id;
            if id.kind != NodeKind::Trait {
                return Ok(None);
            }
            Symbol::Declaration(id.clone())
        };
        Ok(Some(usage(file, n, super::NameRole::Qualifier, span, target)))
    }
    pub(super) fn occurrence_symbol(
        &self,
        file: &File<'_>,
        ctx: &Context<'_>,
        span: SourceSpan,
        slots: &[usize],
        attachments: &[Attachment],
    ) -> Result<Symbol, NameError> {
        self.symbol(file, span, slots).or_else(|mut error| {
            for slot in slots.iter().copied().collect::<std::collections::BTreeSet<_>>() {
                let Some(carrier) = self.carriers.get(&slot) else { continue };
                let owner = &self.entries[slot].id;
                let carrier = match carrier {
                    DomainCarrier::Named(id) => format!("{}::{}", id.package, id.path),
                    DomainCarrier::Builtin(name) => name.clone(),
                    DomainCarrier::Generic | DomainCarrier::Bound { .. } => "generic carrier".into(),
                };
                error
                    .message
                    .push_str(&format!(" ({}::{} on {carrier}", owner.package, owner.path));
                let leaf = owner.path.rsplit("::").next().unwrap();
                if let Some(Some(import)) = ctx.domains.get(leaf).and_then(|sites| sites.get(&slot)) {
                    error
                        .message
                        .push_str(&format!(", import {}@{}..{}", file.path, import.get_start_offset(), import.get_end_offset()));
                }
                error.message.push(')');
            }
            if slots
                .iter()
                .all(|&i| self.entries[i].id.kind == NodeKind::Domain)
                && attachments.iter().any(|a| a.type_binder.is_some())
            {
                let candidates = slots
                    .iter()
                    .map(|&slot| super::DomainCandidate {
                        declaration: self.entries[slot].id.clone(),
                        type_binder: attachments
                            .iter()
                            .find(|a| a.slot == slot)
                            .and_then(|a| a.type_binder)
                            .map(|(owner, index)| (self.entries[owner].id.clone(), index)),
                    })
                    .collect();
                Ok(Symbol::DomainCandidates {
                    candidates,
                    ambiguity: error.message,
                })
            } else {
                Err(error)
            }
        })
    }
    pub(super) fn has_static_root(
        &self,
        file: &File<'_>,
        ctx: &Context<'_>,
        n: usize,
        root: &str,
    ) -> Result<bool, NameError> {
        use NodeKind::{Conformance, Data, Domain, Trait};
        Ok(super::builtin(root)
            || self.edges.contains_key(&(file.package, root))
            || ctx.by_alias.get(root).is_some_and(|sites| {
                sites
                    .iter()
                    .any(|&i| ctx.open_modules.contains(&ctx.imports[i].address))
            })
            || self.modules.contains(&(file.package.into(), root.into()))
            || self
                .lookup(file, ctx, n, root)?
                .iter()
                .any(|&i| matches!(self.entries[i].id.kind, Data | Domain | Trait | Conformance)))
    }

    pub(super) fn lookup(
        &self,
        file: &File<'_>,
        ctx: &Context<'_>,
        n: usize,
        path: &str,
    ) -> Result<Vec<usize>, NameError> {
        let (root, rest) = path
            .split_once("::")
            .map_or((path, None), |(root, rest)| (root, Some(rest)));
        // An import cannot resolve away a conflict between a package alias and a root name.
        let dependency_address = if rest.is_some() && self.edges.contains_key(&(file.package, root)) {
            Some(self.address(file, path, file.span(n))?)
        } else {
            None
        };
        for owner in file.ancestors(n) {
            let Some(&entry) = ctx.entries.get(&owner) else { continue };
            let address = (file.package.into(), qualify(&self.entries[entry].id.path, path));
            if let Some(slots) = self.index.get(&address) {
                let visible: Vec<_> = slots
                    .iter()
                    .copied()
                    .filter(|&i| {
                        let e = &self.entries[i];
                        e.lexical_owner.is_none() || (e.source.is_some_and(|(origin, _)| origin == file.ordinal) && e.lexical_owner == Some(owner))
                    })
                    .collect();
                if !visible.is_empty() {
                    return Ok(visible);
                }
            }
        }
        // A qualified path can name a declaration of the caller's own namespace,
        // as in `Shape::Circle` or `i32::Positive`, but it never descends into a
        // child directory: written in game/, `combat::score` names the root
        // module combat, as `use combat::score` does. Root module and dependency
        // addresses are considered after the file's imports.
        let local = match path.split_once("::") {
            Some((root, _))
                if self
                    .modules
                    .contains(&(file.package.into(), qualify(&file.namespace, root)))
                    || (file.namespace.is_empty() && self.edges.contains_key(&(file.package, root))) =>
            {
                Vec::new()
            }
            _ => self.visible(file, &(file.package.into(), qualify(&file.namespace, path))),
        };
        let mut preferred: Vec<_> = local
            .iter()
            .copied()
            .filter(|&i| {
                self.entries[i]
                    .source
                    .is_some_and(|(origin, _)| origin == file.ordinal)
            })
            .collect();
        let mut imported = Vec::new();
        for &site in ctx.by_alias.get(root).into_iter().flatten() {
            let address = &ctx.imports[site].address;
            if ctx.open_modules.contains(address) {
                if let Some(rest) = rest {
                    preferred.extend(self.visible(file, &(address.0.clone(), qualify(&address.1, rest))));
                }
            } else {
                imported.extend_from_slice(&ctx.imports[site].slots);
            }
        }
        // Opening a module exposes its direct declarations under their own names.
        for (package, module) in &ctx.open_modules {
            imported.extend(self.direct(file, &(package.clone(), qualify(module, root))));
        }
        if let Some(rest) = rest {
            for slot in imported {
                let id = &self.entries[slot].id;
                preferred.extend(self.visible(file, &(id.package.clone(), qualify(&id.path, rest))));
            }
        } else {
            preferred.extend(imported);
        }
        if !preferred.is_empty() {
            return Ok(preferred);
        }
        if !local.is_empty() || rest.is_none() {
            return Ok(local);
        }
        let address = dependency_address.unwrap_or_else(|| (file.package.into(), path.into()));
        Ok(self.visible(file, &address))
    }
}
