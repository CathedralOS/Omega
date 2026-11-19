use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{DomainApplication, TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol};
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
        match name {
            Symbol::Declaration(id) => Ok(id),
            Symbol::DomainCandidates { candidates, ambiguity } => {
                let mut names = candidates.iter().filter(|c| {
                    c.type_binder
                        .as_ref()
                        .is_none_or(|binder| self.binder_names_type(binder))
                });
                match (names.next(), names.next()) {
                    (Some(candidate), None) => Ok(&candidate.declaration),
                    (None, _) => Err("no visible domain on the written carrier"),
                    _ => Err(ambiguity),
                }
            }
            _ => Err("expected an exact declaration"),
        }
    }
}

impl<'a> ResolvedSourceFile<'a> {
    pub(crate) fn domain_name(
        &self,
        node: usize,
        types: &TypeCatalog<'_>,
    ) -> Result<&'a DeclarationId, TypeError> {
        let name = self
            .name_in_role(node, true)
            .filter(|name| name.names_domain())
            .ok_or_else(|| TypeError::at(self.file, node, "expected a domain declaration"))?;
        types
            .selected_declaration(name)
            .map_err(|message| TypeError::at(self.file, node, message))
    }
    pub(crate) fn domain_reference(
        &self,
        n: usize,
        subject: Option<&ValueType>,
        types: &TypeCatalog<'_>,
    ) -> Result<(&'a DeclarationId, Option<ValueType>), TypeError> {
        let written = match self
            .qualifier(n)
            .map(|q| self.select_name(q.visible_names(), true))
        {
            Some(Some(Symbol::Declaration(id))) if id.kind == NodeKind::Data => Some(self.data_application(n, id, types)?),
            Some(Some(Symbol::Declaration(id))) if id.kind == NodeKind::Domain => None,
            Some(Some(Symbol::Binding { span, .. })) => Some(
                self.parameters
                    .get(span)
                    .cloned()
                    .ok_or_else(|| TypeError::unsupported(self.file, n))?,
            ),
            Some(Some(Symbol::Builtin(name))) => Some(ValueType::builtin_scalar(name).ok_or_else(|| TypeError::unsupported(self.file, n))?),
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
                return Err(TypeError::at(self.file, n, format!("domain qualifier names {required}, found {actual}")));
            }
        }
        let id = self.domain_name(n, types)?;
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
            let Some(Symbol::Declaration(owner)) = self.application_name(section).map(|name| &name.target) else {
                continue;
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
    pub(super) fn index_domain(
        &self,
        n: usize,
        head: usize,
        types: &mut TypeCatalog<'a>,
    ) -> Result<(), TypeError> {
        let f = self.file;
        let node = &f.nodes[n];
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
        types.domains.entries.insert(
            id,
            Domain {
                parent,
                carrier,
                indices,
                alias,
            },
        );
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
pub(crate) fn qualified(
    carrier: ValueType,
    mut atoms: Vec<DomainApplication>,
) -> ValueType {
    atoms.sort_by_cached_key(|a| (a.declaration.package.clone(), a.declaration.path.clone(), format!("{:?}", a.arguments)));
    atoms.dedup();
    if atoms.is_empty() {
        carrier
    } else {
        ValueType::Qualified(Box::new(carrier), atoms)
    }
}
impl ValueType {
    pub(crate) fn can_infer_result_domains(
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
    pub fn domains(&self) -> &[DomainApplication] {
        match self {
            Self::Qualified(_, atoms) => atoms,
            _ => &[],
        }
    }
    pub fn carrier(&self) -> &Self {
        match self {
            Self::Qualified(base, _) => base,
            _ => self,
        }
    }
}
