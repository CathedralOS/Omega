use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{DomainApplication, TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(crate) struct Domains<'a> {
    entries: HashMap<&'a DeclarationId, Domain>,
    paths: HashMap<(&'a str, &'a str), &'a DeclarationId>,
}
struct Domain {
    carrier: ValueType,
    indices: Vec<usize>,
    alias: Vec<DomainApplication>,
}

impl<'a> ResolvedSourceFile<'a> {
    pub fn domain(
        &self,
        n: usize,
    ) -> Result<&'a DeclarationId, TypeError> {
        match self.symbol(n) {
            Some(Symbol::Declaration(id)) if id.kind == NodeKind::Domain => Ok(id),
            _ => Err(TypeError::at(self.file, n, "expected a domain declaration")),
        }
    }
    pub(crate) fn explicit_domain_carrier(
        &self,
        n: usize,
        declared: &ValueType,
        types: &TypeCatalog<'_>,
        candidate: Option<&DeclarationId>,
    ) -> Result<ValueType, TypeError> {
        if !matches!(declared, ValueType::Parameter { .. }) {
            return Ok(declared.clone());
        }
        if self.file.nodes[n].words().count() < 2 {
            return Ok(declared.clone());
        }
        match self.qualifier(n) {
            Some(Symbol::Declaration(id)) if id.kind == NodeKind::Data => self.data_application(n, id, types, candidate),
            Some(Symbol::Binding { span, .. }) => self
                .parameters
                .get(span)
                .cloned()
                .ok_or_else(|| TypeError::unsupported(self.file, n)),
            Some(Symbol::Builtin(name)) => match name.as_str() {
                "bool" => Ok(ValueType::Bool),
                "addr" => Ok(ValueType::Address),
                "f32" => Ok(ValueType::Float32),
                "f64" => Ok(ValueType::Float64),
                _ => ValueType::integer(name).ok_or_else(|| TypeError::unsupported(self.file, n)),
            },
            _ => Err(TypeError::at(self.file, n, "qualified generic domain requires a resolved carrier")),
        }
    }
    fn check_explicit_domain_carrier(
        &self,
        n: usize,
        subject: &ValueType,
        types: &TypeCatalog<'_>,
    ) -> Result<(), TypeError> {
        let id = self.domain(n)?;
        let Some(declared) = types.domains.known_carrier(id) else { return Ok(()) };
        if !matches!(declared,ValueType::Parameter { owner,.. } if owner.as_ref()==id) || self.qualifier(n).is_none() {
            return Ok(());
        }
        let explicit = self.explicit_domain_carrier(n, declared, types, None)?;
        let normalized = |ty| {
            types
                .normalize_at(ty, self.file, n)
                .map_err(|msg| TypeError::at(self.file, n, msg))
        };
        let required = normalized(explicit)?;
        let actual = normalized(subject.clone())?;
        if required.carrier() != actual.carrier() {
            return Err(TypeError::at(self.file, n, format!("domain qualifier names {required}, found {actual}")));
        }
        Ok(())
    }
    pub fn domain_application(
        &self,
        n: usize,
        carrier: &ValueType,
        types: &TypeCatalog<'_>,
    ) -> Result<DomainApplication, TypeError> {
        self.check_explicit_domain_carrier(n, carrier, types)?;
        let id = self.domain(n)?;
        let mut arguments = Vec::new();
        let last = self.file.nodes[n]
            .words()
            .next_back()
            .unwrap()
            .span
            .get_end_offset();
        for &section in self.file.nodes[n]
            .children
            .iter()
            .filter(|&&s| self.file.span(s).get_start_offset() >= last)
        {
            for (index, &arg) in self.file.nodes[section].children.iter().enumerate() {
                arguments.push(self.argument(arg, types, id, types.domains.index_binder(id, index))?);
            }
        }
        if types.domains.known_carrier(id).is_some() && arguments.len() != types.domains.index_arity(id) {
            return Err(TypeError::at(self.file, n, "incorrect indexed domain arity"));
        }
        Ok(DomainApplication {
            declaration: id.clone(),
            arguments,
        })
    }
    pub(super) fn index_domain(
        &self,
        n: usize,
        types: &mut TypeCatalog<'a>,
    ) -> Result<(), TypeError> {
        let f = self.file;
        let node = &f.nodes[n];
        let id = self.declaration(n)?;
        let head = f
            .child(n, NodeKind::PathType)
            .ok_or_else(|| TypeError::unsupported(f, n))?;
        let mut indices = Vec::new();
        if let Some(section) = f.child(head, NodeKind::TypeArgs) {
            for &arg in &f.nodes[section].children {
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
        }
        let carrier = if indices.is_empty() {
            self.annotation(head, types)?
        } else {
            match self.symbol(head) {
                Some(Symbol::Builtin(s)) => ValueType::integer(s).ok_or_else(|| TypeError::unsupported(f, head))?,
                Some(Symbol::Declaration(owner)) if owner.kind == NodeKind::Data => types.open_data(owner),
                Some(Symbol::Binding {
                    span,
                    kind: NodeKind::TypeParam,
                }) => self
                    .parameters
                    .get(span)
                    .cloned()
                    .ok_or_else(|| TypeError::unsupported(f, head))?,
                _ => return Err(TypeError::unsupported(f, head)),
            }
        };
        let carrier_binder = match &carrier {
            ValueType::Parameter { owner, index } if owner.as_ref() == id => Some(*index),
            _ => None,
        };
        let complete = types.arities[id] == indices.len() + usize::from(carrier_binder.is_some()) && !carrier_binder.is_some_and(|i| indices.contains(&i));
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
        types
            .domains
            .entries
            .insert(id, Domain { carrier, indices, alias });
        types.domains.paths.insert((&id.package, &id.path), id);
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
        if !matches!(&domain.carrier, ValueType::Parameter { owner, .. } if owner.as_ref() == &app.declaration)
            && domain.indices.iter().copied().eq(0..app.arguments.len())
        {
            return std::borrow::Cow::Borrowed(&app.arguments);
        }
        let generic_carrier = matches!(&domain.carrier, ValueType::Parameter { owner, .. } if owner.as_ref() == &app.declaration);
        let mut args = vec![carrier.clone(); app.arguments.len() + usize::from(generic_carrier)];
        for (&binder, value) in domain.indices.iter().zip(&app.arguments) {
            args[binder] = value.clone();
        }
        std::borrow::Cow::Owned(args)
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
    fn parent_domain(
        &self,
        id: &DeclarationId,
    ) -> Option<&DeclarationId> {
        let (parent, _) = id.path.rsplit_once("::")?;
        self.paths.get(&(id.package.as_str(), parent)).copied()
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
            if ancestors && let Some(parent) = self.parent_domain(id) {
                pending.push((
                    DomainApplication {
                        declaration: parent.clone(),
                        arguments: Vec::new(),
                    },
                    false,
                ));
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
