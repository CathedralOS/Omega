use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_error::SourceTypeError;
use crate::type_relations::TypeRelations;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{DomainApplication, TypeError, ValueType};
use omega_resolved::{DeclarationId, IndexedSource, Symbol};
use omega_syntax::NodeKind;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Default)]
pub(crate) struct Domains<'a> {
    entries: HashMap<&'a DeclarationId, Domain>,
}
#[derive(Clone)]
struct Domain {
    parent: Option<DomainApplication>,
    carrier: ValueType,
    indices: Vec<usize>,
    /// The head binds the declaration's complete binder tuple, as an alias requires.
    complete: bool,
    alias: Vec<DomainApplication>,
}

impl TypeCatalog<'_> {
    pub(crate) fn binder_names_type(
        &self,
        binder: &(DeclarationId, usize),
    ) -> bool {
        matches!(self.bound(&binder.0, binder.1), Some(ValueType::Universe { .. }))
    }

    pub(crate) fn selected_declaration<'a>(
        &self,
        name: &'a Symbol,
    ) -> Result<&'a DeclarationId, &'a str> {
        self.selected_domain(name, None)
            .map(|selection| selection.expect("selection without a carrier has no width comparison"))
    }

    /// Machine heads are plain paths (no type arguments). For `u8::Ready::add`,
    /// use the written `u8`, not an operand or the machine's module-qualified ID.
    pub(crate) fn selected_machine_home<'a>(
        &self,
        name: &'a Symbol,
        file: &IndexedSource<'_>,
        machine: usize,
    ) -> Result<Option<&'a DeclarationId>, &'a str> {
        let mut carrier = None;
        if matches!(name, Symbol::DomainCandidates { candidates, .. } if candidates.iter().any(|c| c.carrier_deferred))
            && let Some(head) = file.child(machine, NodeKind::Path)
            && let Some((owner, _)) = file.nodes[head].path().rsplit_once("::")
            && let Some((root, _)) = owner.split_once("::")
            && let Some(integer @ ValueType::Integer { width: Some(_), .. }) = ValueType::integer(root)
        {
            carrier = Some(integer);
        }
        self.selected_domain(name, carrier.as_ref())
    }

    /// Select among visible domains. A carrier-deferred candidate, found through an applied
    /// integer root such as u<8>, matches only when its normalized declared carrier is
    /// the normalized written or subject carrier. Without that carrier it still competes.
    fn selected_domain<'a>(
        &self,
        name: &'a Symbol,
        carrier: Option<&ValueType>,
    ) -> Result<Option<&'a DeclarationId>, &'a str> {
        match name {
            Symbol::Declaration(id) => Ok(Some(id)),
            Symbol::DomainCandidates { candidates, ambiguity } => {
                let mut matched = None;
                let mut waiting = false;
                for candidate in candidates {
                    if candidate.type_binder.as_ref().is_some_and(|binder| !self.binder_names_type(binder)) {
                        continue;
                    }
                    if candidate.carrier_deferred && let Some(subject) = carrier {
                        let Some(declared) = self.domains.known_carrier(&candidate.declaration) else {
                            waiting |= self.indices.declaration_is_pending(&candidate.declaration);
                            continue;
                        };
                        if declared.carrier() != subject.carrier() {
                            if self.indices.can_wait_for_equality(declared.carrier(), subject.carrier()) {
                                self.indices.retain_demands(declared);
                                self.indices.retain_demands(subject);
                                waiting = true;
                            }
                            continue;
                        }
                    }
                    if matched.replace(&candidate.declaration).is_some() {
                        return Err(ambiguity);
                    }
                }
                if waiting { return Ok(None); }
                matched.map(Some).ok_or("no visible domain on the written carrier")
            }
            _ => Err("expected an exact declaration"),
        }
    }
}

impl<'a> ResolvedSourceFile<'a> {
    /// Ordinary machines don't publish an operator home, but their explicit domain
    /// qualifier must select one declaration before their signature is available.
    pub(crate) fn check_machine_qualifier(&self, machine: usize, types: &TypeCatalog<'_>) -> Result<(), TypeError> {
        if let Some(head) = self.file.child(machine, NodeKind::Path)
            && let Some(name) = self.names(head).find(|name| name.role == omega_resolved::NameRole::Qualifier)
            && matches!(name.target, Symbol::DomainCandidates { .. })
        {
            types.selected_machine_home(&name.target, self.file, machine)
                .map_err(|message| TypeError::at(self.file, head, message).with_span(name.span))?
                .ok_or_else(|| types.indices.wait(self.file, head))?;
        }
        Ok(())
    }

    pub(crate) fn domain_name(
        &self,
        node: usize,
        types: &TypeCatalog<'_>,
    ) -> Result<&'a DeclarationId, TypeError> {
        self.domain_name_on(node, None, types)
    }
    fn domain_name_on(
        &self,
        node: usize,
        carrier: Option<&ValueType>,
        types: &TypeCatalog<'_>,
    ) -> Result<&'a DeclarationId, TypeError> {
        let name = self
            .name_in_role(node, true)
            .filter(|name| name.names_domain())
            .ok_or_else(|| TypeError::at(self.file, node, "expected a domain declaration"))?;
        types
            .selected_domain(name, carrier)
            .map_err(|message| TypeError::at(self.file, node, message))?
            .ok_or_else(|| types.indices.wait(self.file, node))
    }
    pub(crate) fn domain_reference(
        &self,
        n: usize,
        subject: Option<&ValueType>,
        types: &TypeCatalog<'_>,
    ) -> Result<(&'a DeclarationId, Option<ValueType>), TypeError> {
        // An applied parent domain can sit between the written carrier and the
        // selected domain. Its indices have a different owner from the carrier's arguments.
        let carrier_sections: Vec<_> = self
            .file
            .reference_sections(n)
            .iter()
            .copied()
            .filter(|&section| {
                let selected = self.application_name(section)
                    .and_then(|name| self.select_name(name.target.visible_names(), true));
                matches!(selected, Some(Symbol::Builtin(_) | Symbol::Binding { .. }))
                    || matches!(selected, Some(Symbol::Declaration(id)) if id.kind == NodeKind::Data)
            })
            .collect();
        let root = carrier_sections.last().map(|&section| {
            self.application_name(section)
                .and_then(|name| self.select_name(name.target.visible_names(), true))
        });
        let written = match self
            .qualifier(n)
            .map(|q| self.select_name(q.visible_names(), true))
        {
            Some(Some(Symbol::Declaration(id))) if id.kind == NodeKind::Domain => root,
            // A parent domain found by integer family, as the Mark of u<8>::Mark<3>::Child.
            Some(Some(Symbol::DomainCandidates { candidates, .. })) if candidates.iter().any(|c| c.carrier_deferred) => root,
            None => root,
            qualifier => qualifier,
        };
        let written = match written {
            Some(Some(Symbol::Declaration(id))) if id.kind == NodeKind::Data => Some(self.data_application(n, id, types)?),
            Some(Some(Symbol::Declaration(id))) if id.kind == NodeKind::Domain => None,
            Some(Some(Symbol::Binding { .. })) if !carrier_sections.is_empty() => {
                return Err(TypeError::at(self.file, carrier_sections[0], "generic arguments require an applicable type declaration"));
            }
            Some(Some(Symbol::Binding { span, .. })) => Some(
                self.parameters
                    .get(span)
                    .cloned()
                    .ok_or_else(|| TypeError::unsupported(self.file, n))?,
            ),
            Some(Some(Symbol::Builtin(name))) => Some(self.builtin_carrier(n, name, &carrier_sections, types)?),
            None => None,
            _ => return Err(TypeError::at(self.file, n, "qualified generic domain requires a resolved carrier")),
        };
        let normalize = |ty: &ValueType| {
            types
                .normalize_at(ty.clone(), self, n)
                .map_err(|m| TypeError::at(self.file, n, m))
        };
        let written = written.as_ref().map(normalize).transpose()?;
        if let (Some(required), Some(actual)) = (&written, subject) {
            let actual = normalize(actual)?;
            if required.carrier() != actual.carrier() {
                if !types.indices.can_wait_for_equality(required.carrier(), actual.carrier()) {
                    return Err(TypeError::at(self.file, n, format!("domain qualifier names {required}, found {actual}")));
                }
                types.indices.retain_demands(required);
                types.indices.retain_demands(&actual);
                return Err(types.indices.wait(self.file, n));
            }
        }
        // Candidates found through an applied integer root are selected by the
        // normalized carrier: the written one, or else the subject's.
        let deferred = self
            .name_in_role(n, true)
            .is_some_and(|name| matches!(name, Symbol::DomainCandidates { candidates, .. } if candidates.iter().any(|c| c.carrier_deferred)));
        let selection_carrier = match (&written, subject) {
            (Some(written), _) => Some(written.clone()),
            (None, Some(subject)) if deferred => Some(normalize(subject)?),
            _ => None,
        };
        let id = self.domain_name_on(n, selection_carrier.as_ref(), types)?;
        if let Some(declared) = types.domains.known_carrier(id) {
            if let Some(use_carrier) = written.as_ref().or(subject)
                && types.indices.can_wait_for_equality(declared.carrier(), use_carrier.carrier())
            {
                types.indices.retain_demands(declared);
                types.indices.retain_demands(use_carrier);
                return Err(types.indices.wait(self.file, n));
            } else if let Some(written) = &written
                && !types.domains.admits(id, written)
            {
                return Err(TypeError::at(self.file, n, "no visible domain on the written carrier"));
            }
        }
        if deferred {
            self.check_signature_selection(n, id)?;
        }
        Ok((id, written))
    }

    pub fn domain_application(
        &self,
        n: usize,
        carrier: &ValueType,
        types: &TypeCatalog<'_>,
    ) -> Result<DomainApplication, TypeError> {
        let (id, _) = self.domain_reference(n, Some(carrier), types)?;
        self.indexed_domain(n, id, carrier, types)
    }

    pub(super) fn indexed_domain(
        &self,
        n: usize,
        id: &DeclarationId,
        carrier: &ValueType,
        types: &TypeCatalog<'_>,
    ) -> Result<DomainApplication, TypeError> {
        if types.indices.declaration_is_pending(id) {
            return Err(types.indices.wait(self.file, n));
        }
        let mut arguments = Vec::new();
        for (index, &arg) in self.arguments_for(n, id).iter().enumerate() {
            let binder = types.domains.index_binder(id, index);
            arguments.push(self.argument(arg, types, id, binder, types.bound(id, binder))?);
        }
        if types.domains.known_carrier(id).is_some() && arguments.len() != types.domains.index_arity(id) {
            return Err(TypeError::at(self.file, n, "incorrect indexed domain arity"));
        }
        let application = DomainApplication {
            declaration: id.clone(),
            arguments,
        };
        let mut ancestor = std::borrow::Cow::Borrowed(&application);
        for &section in self.file.reference_sections(n).iter().rev() {
            let owner = match self.application_name(section).map(|name| &name.target) {
                Some(Symbol::Declaration(owner)) => owner,
                // Parents found by integer family: the selected domain decides which one is meant.
                Some(Symbol::DomainCandidates { candidates, .. }) if candidates.iter().any(|c| c.carrier_deferred) => {
                    let owner = candidates
                        .iter()
                        .map(|c| &c.declaration)
                        .find(|&owner| owner == id || types.domains.is_ancestor(owner, id))
                        .ok_or_else(|| TypeError::at(self.file, section, "qualifier does not name a parent domain"))?;
                    self.check_signature_selection(n, owner)?;
                    owner
                }
                _ => continue,
            };
            if owner.kind != NodeKind::Domain || owner == id {
                continue;
            }
            loop {
                let Some(parent) = types.domains.applied_parent(&ancestor, carrier) else {
                    return Err(TypeError::at(self.file, section, "qualifier does not name a parent domain"));
                };
                ancestor = std::borrow::Cow::Owned(parent);
                if &ancestor.declaration == owner {
                    break;
                }
            }
            let supplied = &self.file.nodes[section].children;
            if supplied.len() != ancestor.arguments.len() {
                return Err(TypeError::at(self.file, section, "incorrect parent domain arity"));
            }
            for (i, (&arg, expected)) in supplied.iter().zip(&ancestor.arguments).enumerate() {
                let binder = types.domains.index_binder(owner, i);
                let actual = self.argument(arg, types, owner, binder, types.bound(owner, binder))?;
                let expected = types
                    .normalize_at(expected.clone(), self, arg)
                    .map_err(|msg| TypeError::at(self.file, arg, msg))?;
                if actual != expected {
                    if types.indices.can_wait_for_equality(&actual, &expected) {
                        types.indices.retain_demands(&actual);
                        types.indices.retain_demands(&expected);
                        return Err(types.indices.wait(self.file, n));
                    }
                    return Err(TypeError::at(
                        self.file,
                        arg,
                        format!("parent domain {} expects {expected}, found {actual}", owner.path),
                    ));
                }
            }
        }
        Ok(application)
    }
    /// Form the domain's identity: its carrier, parent and index binders. Selection of a
    /// carrier-deferred candidate needs only this, so it precedes alias expansion.
    pub(super) fn index_domain_head(
        &self,
        n: usize,
        head: usize,
        types: &mut TypeCatalog<'a>,
    ) -> Result<(), TypeError> {
        let f = self.file;
        let id = self.declaration(n)?;
        let mut indices = Vec::new();
        for &arg in self.arguments_for(head, id) {
            let Some(Symbol::Binding { span, .. }) = self.symbol(arg) else {
                return Err(TypeError::at(f, arg, "domain head index must name its binder"));
            };
            let Some(ValueType::Parameter { owner, index }) = self.parameters.get(span) else {
                return Err(TypeError::at(f, arg, "domain head index must name its binder"));
            };
            if owner.as_ref() != id || indices.contains(index) {
                return Err(TypeError::at(f, arg, "domain head index must name a distinct binder"));
            }
            indices.push(*index);
        }
        let raw_carrier = self.raw_type(head, types)?;
        // A domain head names its parent before aliases and ancestors expand.
        let parent = raw_carrier.domains().first().cloned();
        let carrier = types
            .normalize_at(raw_carrier, self, head)
            .map_err(|message| TypeError::at(f, head, message))?;
        let carrier_binder = match &carrier {
            ValueType::Parameter { owner, index } if owner.as_ref() == id => Some(*index),
            _ => None,
        };
        let complete = types.arity(id) == indices.len() + usize::from(carrier_binder.is_some()) && !carrier_binder.is_some_and(|i| indices.contains(&i));
        if !indices.is_empty() && !complete {
            return Err(TypeError::at(f, head, "indexed domain requires its complete binder tuple"));
        }
        types.domains.entries.insert(
            id,
            Domain {
                parent,
                carrier,
                indices,
                complete,
                alias: Vec::new(),
            },
        );
        Ok(())
    }

    /// Expand the alias terms against the carrier formed by `index_domain_head`.
    pub(super) fn index_domain_alias(
        &self,
        n: usize,
        head: usize,
        types: &mut TypeCatalog<'a>,
    ) -> Result<(), TypeError> {
        let f = self.file;
        let node = &f.nodes[n];
        let id = self.declaration(n)?;
        let (carrier, complete) = types
            .domains
            .entries
            .get(id)
            .map(|domain| (domain.carrier.clone(), domain.complete))
            .ok_or_else(|| TypeError::at(f, head, format!("missing domain declaration {}", id.path)))?;
        let mut alias = Vec::new();
        for &d in &node.children {
            if d == head || f.nodes[d].kind == NodeKind::GenericParams {
                continue;
            }
            match f.nodes[d].kind {
                NodeKind::PathType => alias.push(self.domain_application(d, &carrier, types)?),
                NodeKind::Requires | NodeKind::DroppedBy | NodeKind::Access => {}
                _ => return Err(TypeError::unsupported(f, d)),
            }
        }
        if !alias.is_empty() && !complete {
            return Err(TypeError::at(f, head, "indexed domain alias requires its complete binder tuple"));
        }
        types
            .domains
            .entries
            .get_mut(id)
            .ok_or_else(|| TypeError::at(f, head, format!("missing domain declaration {}", id.path)))?
            .alias = alias;
        Ok(())
    }
}
impl Domains<'_> {
    pub fn index_arity(
        &self,
        id: &DeclarationId,
    ) -> usize {
        self.entries.get(id).map_or(0, |d| d.indices.len())
    }
    pub fn index_binder(
        &self,
        id: &DeclarationId,
        position: usize,
    ) -> usize {
        self.entries
            .get(id)
            .and_then(|d| d.indices.get(position))
            .copied()
            .unwrap_or(position)
    }
    pub fn bind_arguments<'b>(
        &self,
        app: &'b DomainApplication,
        carrier: &ValueType,
    ) -> std::borrow::Cow<'b, [ValueType]> {
        let domain = &self.entries[&app.declaration];
        let generic_carrier = matches!(&domain.carrier, ValueType::Parameter { owner, .. } if owner.as_ref() == &app.declaration);
        if !generic_carrier && domain.indices.iter().copied().eq(0..app.arguments.len()) {
            return std::borrow::Cow::Borrowed(&app.arguments);
        }
        let mut args = vec![carrier.clone(); app.arguments.len() + usize::from(generic_carrier)];
        for (&binder, value) in domain.indices.iter().zip(&app.arguments) {
            args[binder] = value.clone();
        }
        std::borrow::Cow::Owned(args)
    }
    fn applied_parent(
        &self,
        child: &DomainApplication,
        carrier: &ValueType,
    ) -> Option<DomainApplication> {
        let parent = self.entries.get(&child.declaration)?.parent.as_ref()?;
        Some(parent.substitute(&child.declaration, &self.bind_arguments(child, carrier)))
    }
    pub(crate) fn alias_members(
        &self,
        id: &DeclarationId,
    ) -> &[DomainApplication] {
        &self.entries[id].alias
    }
    pub fn known_carrier(
        &self,
        id: &DeclarationId,
    ) -> Option<&ValueType> {
        self.entries.get(id).map(|d| &d.carrier)
    }
    pub fn carrier(
        &self,
        id: &DeclarationId,
    ) -> &ValueType {
        &self.entries[id].carrier
    }
    pub fn admits(
        &self,
        id: &DeclarationId,
        carrier: &ValueType,
    ) -> bool {
        let declared = self.carrier(id);
        declared == carrier
            || declared == carrier.carrier()
            || (self.parent_domain(id).is_some() && declared.carrier() == carrier.carrier())
            || matches!(declared,ValueType::Parameter{owner,..} if owner.as_ref()==id)
    }
    pub(crate) fn is_ancestor(
        &self,
        ancestor: &DeclarationId,
        child: &DeclarationId,
    ) -> bool {
        std::iter::successors(self.parent_domain(child), |id| self.parent_domain(id)).any(|id| id == ancestor)
    }
    fn parent_domain(
        &self,
        id: &DeclarationId,
    ) -> Option<&DeclarationId> {
        self.entries
            .get(id)?
            .parent
            .as_ref()
            .map(|app| &app.declaration)
    }
    pub fn expanded(
        &self,
        application: &DomainApplication,
        carrier: &ValueType,
        ancestors: bool,
        mut normalize: impl FnMut(&mut DomainApplication) -> Result<(), String>,
    ) -> Result<Vec<DomainApplication>, String> {
        let mut atoms = Vec::new();
        // Exit markers distinguish shared subgraphs from a cyclic alias.
        let mut pending = vec![(application.clone(), false)];
        let mut active = HashSet::new();
        let mut done = HashSet::new();
        while let Some((mut app, exit)) = pending.pop() {
            if exit {
                active.remove(&app.declaration);
                done.insert(app);
                continue;
            }
            let id = &app.declaration;
            if app.arguments.len() != self.index_arity(id) {
                return Err("incorrect indexed domain arity".into());
            }
            normalize(&mut app)?;
            if done.contains(&app) {
                continue;
            }
            let id = &app.declaration;
            if !active.insert(id.clone()) {
                return Err(format!("cyclic domain alias {}", id.path));
            }
            let info = self
                .entries
                .get(id)
                .ok_or_else(|| format!("missing domain declaration {}", id.path))?;
            if !self.admits(id, carrier) {
                return Err(format!("domain {} belongs to {}, found {carrier}", id.path, info.carrier));
            }
            pending.push((app.clone(), true));
            if ancestors && let Some(parent) = self.applied_parent(&app, carrier) {
                pending.push((parent, false));
            }
            if info.alias.is_empty() {
                atoms.push(app);
            } else {
                let arguments = self.bind_arguments(&app, carrier);
                pending.extend(
                    info.alias
                        .iter()
                        .map(|term| (term.substitute(id, &arguments), false)),
                );
            }
        }
        Ok(atoms)
    }
}
pub(crate) trait InferResultDomains: Sized {
    fn can_infer_result_domains(
        &self,
        owner: &DeclarationId,
        destination: &Self,
    ) -> bool;
}

impl InferResultDomains for ValueType {
    fn can_infer_result_domains(
        &self,
        owner: &DeclarationId,
        destination: &Self,
    ) -> bool {
        if matches!(self, Self::Parameter { owner: binder, .. } if binder.as_ref() == owner) {
            return true;
        }
        let domains = self.domains();
        // Qualification sorts its applications. Substitution can merge two indices
        // of one domain, but cannot change the identity of that domain.
        fn declarations(domains: &[DomainApplication]) -> impl Iterator<Item = &DeclarationId> {
            domains
                .chunk_by(|a, b| a.declaration == b.declaration)
                .map(|group| &group[0].declaration)
        }
        domains
            .iter()
            .any(|d| d.arguments.iter().any(Self::depends_on_call_input))
            && declarations(domains).eq(declarations(destination.domains()))
    }
}
