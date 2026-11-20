use super::{Catalog, Context, DeclarationId, DomainCarrier, NameError, NameErrorKind, NameUse, Symbol, callable, module_child, qualify, usage};
use crate::syntax::File;
use omega_syntax::NodeKind;
use omega_tokens::SourceSpan;

#[derive(Clone, Copy)]
pub(super) struct Attachment {
    pub slot: usize,
    // Declaration slot and ordinal of the binder whose type role permits this attachment.
    type_binder: Option<(usize, usize)>,
    // Matched by integer family under an applied carrier. Typing compares the normalized carriers.
    pub carrier_deferred: bool,
}

/// Bare `u` and `i` describe fixed-width carriers only when supplied a width.
fn fixed_integer_family(name: &str, applied: bool) -> Option<u8> {
    let (&first, rest) = name.as_bytes().split_first()?;
    (matches!(first, b'u' | b'i') && (applied || !rest.is_empty()) && rest.iter().all(u8::is_ascii_digit)).then_some(first)
}

impl Context<'_> {
    fn opened_modules(&self, alias: &str) -> impl Iterator<Item = &(String, String)> {
        self.by_alias.get(alias).into_iter().flatten()
            .map(|&site| &self.imports[site].address)
            .filter(|module| self.open_modules.contains(*module))
    }

    pub(super) fn unresolved_name(&self, file: &File<'_>, span: SourceSpan, path: &str) -> NameError {
        // Lookup has already rejected competing module paths. Name the unique
        // imported module that the user's spelling actually selected.
        let mut message = format!("unresolved name {path}");
        if let Some((alias, _)) = path.split_once('\\')
            && let Some((package, module)) = self.opened_modules(alias).next()
        {
            let module = if module.is_empty() { "<root>" } else { module };
            message.push_str(&format!(" in imported module {module} of package {package}"));
        }
        file.error(NameErrorKind::UnresolvedName, span, message)
    }

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
            let error = |kind, reason| NameError::new(kind, &export.id.package, &export.id.path, SourceSpan::new(0, 0), reason);
            if export.id.kind != NodeKind::Trait {
                return Err(error(NameErrorKind::CompiledParentsOnNonTrait, "only a trait can have parents in a compiled interface"));
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
                    .ok_or_else(|| error(NameErrorKind::UnknownCompiledParentTrait, "unknown parent trait in compiled interface"))?;
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
                    return Err(file.error(NameErrorKind::UnresolvedParentTrait, file.span(base), format!("unresolved trait {name}")));
                };
                if traits.any(|i| i != parent) {
                    return Err(file.error(NameErrorKind::AmbiguousParentTrait, file.span(base), format!("ambiguous trait {name}")));
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
                        Some((f, n)) => self.files[f].error(NameErrorKind::CyclicTraitInheritance, self.files[f].span(n), "cyclic trait inheritance"),
                        None => NameError::new(
                            NameErrorKind::CyclicCompiledTraitInheritance,
                            &entry.id.package,
                            &entry.id.path,
                            SourceSpan::new(0, 0),
                            "cyclic trait inheritance",
                        ),
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
            let slot = self.declaration_slot(id);
            let accessible = slot.filter(|&i| {
                id.package == file.package
                    || self.entries[i].public
                        && self
                            .edges
                            .iter()
                            .any(|(&(from, _), &to)| from == file.package && to == id.package)
            });
            let Some(slot) = accessible else {
                let error = file.error(
                    NameErrorKind::CarrierWithoutAuthority,
                    span,
                    format!("no authority to name carrier {}\\{}", id.package, id.path),
                );
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

    fn declaration_slot(
        &self,
        id: &DeclarationId,
    ) -> Option<usize> {
        self.index
            .get(&(id.package.clone(), id.path.clone()))
            .into_iter()
            .flatten()
            .copied()
            .find(|&slot| self.entries[slot].id == *id)
    }

    pub(super) fn has_domain_ancestor(
        &self,
        domain: usize,
        ancestor: usize,
    ) -> bool {
        let mut current = domain;
        let mut visited = std::collections::HashSet::new();
        while visited.insert(current) {
            let parent = match self.carriers.get(&current) {
                Some(DomainCarrier::Named(parent) | DomainCarrier::Bound { alternative: parent, .. }) if parent.kind == NodeKind::Domain => parent,
                _ => return false,
            };
            if *parent == self.entries[ancestor].id {
                return true;
            }
            let Some(slot) = self.declaration_slot(parent) else { return false };
            current = slot;
        }
        false
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
        let (root, rest) = path.split_once('\\').unwrap_or((path, ""));
        let Some(&package) = self.edges.get(&(file.package, root)) else {
            return Ok((file.package.into(), path.into()));
        };
        let own = (file.package.to_owned(), root.to_owned());
        if self.modules.contains(&own) || rest.is_empty() && !self.visible(file, &own).is_empty() {
            return Err(file.error(
                NameErrorKind::CompetingModulePaths,
                span,
                format!("competing module paths {root}: package {package} and {}\\{root}", file.package),
            ));
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
            let alias = path.rsplit([':', '\\']).next().unwrap();
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

    pub(super) fn index_visible_domains(
        &self,
        file: &File<'_>,
        ctx: &mut Context<'_>,
    ) {
        let mut expose = |slot: usize, origin| {
            let id = &self.entries[slot].id;
            let leaf = id.path.rsplit([':', '\\']).next().unwrap();
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
            if ctx.open_modules.contains(&import.address) {
                for &slot in self
                    .domains_by_module
                    .get(&import.address)
                    .into_iter()
                    .flatten()
                {
                    if self.entries[slot].public || import.address.0 == file.package {
                        expose(slot, Some(span));
                    }
                }
            } else {
                for &slot in &import.slots {
                    if self.carriers.contains_key(&slot) {
                        expose(slot, Some(span));
                    }
                }
            }
        }
    }

    pub(super) fn bind_imports(
        &self,
        file: &File<'_>,
        ctx: &Context<'_>,
        uses: &mut Vec<NameUse>,
        visibility: &mut Vec<super::CarrierVisibility>,
    ) -> Result<(), NameError> {
        for import in &ctx.imports {
            let span = file.span(import.node);
            let (package, path) = &import.address;
            let target = if ctx.open_modules.contains(&import.address) {
                Symbol::Module {
                    package: package.clone(),
                    path: path.clone(),
                }
            } else {
                if import.slots.is_empty() {
                    return Err(file.error(
                        NameErrorKind::UnavailableImport,
                        span,
                        format!("unavailable import {}", file.nodes[import.node].path()),
                    ));
                }
                let target = self.symbol(file, span, &import.slots)?;
                for &slot in &import.slots {
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
                NameErrorKind::CompetingDeclarations,
                span,
                format!(
                    "competing declarations: {}",
                    ids.iter()
                        .map(|id| format!("{}\\{}", id.package, id.path))
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
        let implicit_subject = parent
            .filter(|p| !membership_target && p.kind == NodeKind::QualifiedType && p.children.first() != Some(&node))
            .map(|p| p.children[0]);
        let implicit_carrier = implicit_subject.map(|subject| file.nodes[subject].path());
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
        let mut implicit = false;
        let roots = loop {
            let Some(prefix) = qualifier else {
                qualifier = implicit_carrier.as_deref();
                implicit = true;
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
        // `u<8>` and `u8` name one carrier, but only typing evaluates a width. An applied
        // integer root therefore admits every visible domain of that leaf on a carrier of
        // the same integer family, and typing selects by the normalized carrier.
        let mut integer_root = qualifier.filter(|&name| super::builtin(name)).and_then(|name| {
            let applied = match implicit_subject.filter(|_| implicit) {
                Some(subject) => file.nodes[subject]
                    .children
                    .iter()
                    .any(|&section| matches!(file.nodes[section].kind, NodeKind::TypeArgs | NodeKind::ExpressionArgs)),
                None => file.nodes[node]
                    .children
                    .iter()
                    .any(|&section| file.application_prefix(section).is_some_and(|(prefix, _)| prefix == name)),
            };
            fixed_integer_family(name, applied).map(|family| (family, applied))
        });
        let mut owners: Vec<_> = roots
            .into_iter()
            .map(|slot| Attachment {
                slot,
                type_binder: None,
                carrier_deferred: false,
            })
            .collect();
        for segment in unresolved_parents.into_iter().rev().chain([leaf]) {
            // Only the first segment is attached to the written carrier root.
            let root = integer_root.take();
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
                    let applied = !self.entries[slot].id.domain_carrier_arguments.is_empty();
                    let named = match carrier {
                        DomainCarrier::Named(owner) | DomainCarrier::Bound { alternative: owner, .. } => owners
                            .iter()
                            .find(|parent| self.entries[parent.slot].id == *owner),
                        _ => None,
                    };
                    let (type_binder, carrier_deferred) = match (qualifier, carrier, named) {
                        (None, _, _) => (None, false),
                        (_, _, Some(parent)) => (parent.type_binder, parent.carrier_deferred),
                        (Some(_), DomainCarrier::Builtin(name), _)
                            if root.is_some_and(|(family, used_arguments)| {
                                (used_arguments || applied) && fixed_integer_family(name, applied) == Some(family)
                            }) => (None, true),
                        (Some(written), DomainCarrier::Builtin(name), _) if !applied && written == name => (None, false),
                        (_, DomainCarrier::Generic, _) if generic => (None, false),
                        (_, DomainCarrier::Bound { position, .. }, _) if generic => (Some((slot, *position)), false),
                        _ => return None,
                    };
                    Some(Attachment {
                        slot,
                        type_binder,
                        carrier_deferred,
                    })
                })
                .collect();
            if owners.is_empty() {
                break;
            }
            qualifier = Some(segment);
        }
        Ok(owners)
    }

    /// The root of a domain path can name a builtin carrier or a static binder
    /// rather than a declaration. Arguments written on that root apply to it.
    pub(super) fn carrier_application_owner(
        &self,
        ctx: &Context<'_>,
        n: usize,
        prefix: &str,
    ) -> Option<Symbol> {
        if super::builtin(prefix) {
            return Some(Symbol::Builtin(prefix.to_owned()));
        }
        let mut names: Vec<_> = ctx
            .carrier_binders(prefix, n)
            .map(super::Local::symbol)
            .collect();
        match names.len() {
            0 => None,
            1 => names.pop(),
            _ => Some(Symbol::ScopedNames {
                context: super::NameContext::Type,
                names,
            }),
        }
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
        if super::builtin(prefix)
            && candidates
                .iter()
                .any(|&slot| !self.entries[slot].id.domain_carrier_arguments.is_empty())
        {
            let span = file.nodes[n].qualifier_span().expect("qualified path");
            return Ok(Some(usage(file, n, super::NameRole::Qualifier, span, Symbol::Builtin(prefix.into()))));
        }
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
            // Source and compiled declarations can occupy different catalog slots.
            // Report the competing attachments in the same order as their identities.
            let mut slots = slots.to_vec();
            slots.sort_unstable_by_key(|&slot| &self.entries[slot].id);
            slots.dedup();
            for &slot in &slots {
                let Some(carrier) = self.carriers.get(&slot) else { continue };
                let owner = &self.entries[slot].id;
                let carrier = match carrier {
                    DomainCarrier::Named(id) => format!("{}\\{}", id.package, id.path),
                    DomainCarrier::Builtin(name) => name.clone(),
                    DomainCarrier::Generic | DomainCarrier::Bound { .. } => "generic carrier".into(),
                };
                error
                    .message
                    .push_str(&format!(" ({}\\{} on {carrier}", owner.package, owner.path));
                let leaf = owner.path.rsplit([':', '\\']).next().unwrap();
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
                && attachments.iter().any(|a| a.type_binder.is_some() || a.carrier_deferred)
            {
                let candidates = slots
                    .iter()
                    .map(|&slot| {
                        let attachment = attachments.iter().find(|a| a.slot == slot);
                        super::DomainCandidate {
                            declaration: self.entries[slot].id.clone(),
                            type_binder: attachment
                                .and_then(|a| a.type_binder)
                                .map(|(owner, index)| (self.entries[owner].id.clone(), index)),
                            carrier_deferred: attachment.is_some_and(|a| a.carrier_deferred),
                        }
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
    /// Decide eligibility from the written use before consulting the member name.
    pub(super) fn has_static_owner(
        &self,
        file: &File<'_>,
        ctx: &Context<'_>,
        node: usize,
        name: &str,
        span: SourceSpan,
        call: bool,
    ) -> Result<bool, NameError> {
        use NodeKind::{Conformance, Data, Domain, Trait};
        let visible = self.lookup(file, ctx, node, name)?;
        let domains = ctx
            .domains
            .get(name)
            .into_iter()
            .flat_map(|sites| sites.keys().copied());
        let mut owners = visible.into_iter().chain(domains).filter(|&slot| {
            let kind = self.entries[slot].id.kind;
            matches!(kind, Trait | Conformance | Domain) || !call && kind == Data
        });
        let first = owners.next();
        if first.is_some_and(|first| owners.any(|other| self.entries[first].id != self.entries[other].id)) {
            return Err(file.error(NameErrorKind::AmbiguousStaticRoot, span, format!("ambiguous static root {name}")));
        }
        Ok(first.is_some() || !call && super::builtin(name))
    }

    pub(super) fn lookup(
        &self,
        file: &File<'_>,
        ctx: &Context<'_>,
        n: usize,
        path: &str,
    ) -> Result<Vec<usize>, NameError> {
        if let Some((root, rest)) = path.split_once('\\') {
            let address = self.address(file, path, file.span(n))?;
            let mut modules = ctx.opened_modules(root);
            if let Some(module) = modules.next() {
                if modules.any(|other| other != module) {
                    let span = file.nodes[n]
                        .words()
                        .next()
                        .map_or(file.span(n), |word| word.span);
                    return Err(file.error(NameErrorKind::CompetingModulePaths, span, format!("competing module paths {root}")));
                }
                return Ok(self.visible(file, &(module.0.clone(), module_child(&module.1, rest))));
            }
            return Ok(self.visible(file, &address));
        }
        let (root, rest) = path
            .split_once("::")
            .map_or((path, None), |(root, rest)| (root, Some(rest)));
        for owner in file.ancestors(n) {
            let Some(&entry) = ctx.entries.get(&owner) else { continue };
            let address = (file.package.into(), qualify(&self.entries[entry].id.path, path));
            if let Some(slots) = self.index.get(&address) {
                let visible: Vec<_> = slots
                    .iter()
                    .copied()
                    .filter(|&i| {
                        let e = &self.entries[i];
                        e.lexical_owner.is_none() || (e.source.is_some_and(|(origin, _)| origin == file.ordinal()) && e.lexical_owner == Some(owner))
                    })
                    .collect();
                // Cases share their owner's member namespace, but only `Shape::Circle`
                // names a case. A bare spelling that finds nothing except cases at this
                // owner keeps searching outward, so `case Circle(c:Circle)` can name an
                // outer `Circle`. Any other member with that spelling still decides here.
                let bare_cases_only = rest.is_none()
                    && visible
                        .iter()
                        .all(|&i| self.entries[i].id.kind == NodeKind::Case);
                if !visible.is_empty() && !bare_cases_only {
                    return Ok(visible);
                }
            }
        }
        let local = self.visible(file, &(file.package.into(), module_child(&file.namespace, path)));
        let mut preferred: Vec<_> = local
            .iter()
            .copied()
            .filter(|&i| {
                self.entries[i]
                    .source
                    .is_some_and(|(origin, _)| origin == file.ordinal())
            })
            .collect();
        let mut imported = Vec::new();
        for &site in ctx.by_alias.get(root).into_iter().flatten() {
            let address = &ctx.imports[site].address;
            if !ctx.open_modules.contains(address) {
                imported.extend_from_slice(&ctx.imports[site].slots);
            }
        }
        // Opening a module exposes its direct declarations under their own names.
        for (package, module) in &ctx.open_modules {
            imported.extend(self.direct(file, &(package.clone(), module_child(module, root))));
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
        let address = (file.package.into(), path.into());
        Ok(self.visible(file, &address))
    }
}
