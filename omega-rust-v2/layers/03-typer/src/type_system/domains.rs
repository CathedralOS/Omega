use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{DomainApplication, TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(crate) struct Domains<'a> {
    entries: HashMap<&'a DeclarationId, Domain<'a>>,
    paths: HashMap<(&'a str, &'a str), &'a DeclarationId>,
}
struct Domain<'a> {
    carrier: ValueType,
    indices: usize,
    alias: Vec<&'a DeclarationId>,
}

impl<'a> ResolvedSourceFile<'a> {
    pub fn domain(
        &self,
        n: usize,
    ) -> Result<&'a DeclarationId, TypeError> {
        match self.symbol(n) {
            Some(Symbol::Declaration(id)) if id.kind == NodeKind::Domain => Ok(id),
            _ => Err(TypeError::at(&self.file, n, "expected a domain declaration")),
        }
    }
    pub fn domain_application(
        &self,
        n: usize,
        types: &TypeCatalog<'_>,
    ) -> Result<DomainApplication, TypeError> {
        let id = self.domain(n)?;
        let mut arguments = Vec::new();
        for &section in &self.file.nodes[n].children {
            for (index, &arg) in self.file.nodes[section].children.iter().enumerate() {
                arguments.push(self.argument(arg, types, id, index)?);
            }
        }
        if types.domains.known_carrier(id).is_some() && arguments.len() != types.domains.index_arity(id) {
            return Err(TypeError::at(&self.file, n, "incorrect indexed domain arity"));
        }
        Ok(DomainApplication {
            declaration: id.clone(),
            arguments,
        })
    }
    pub(crate) fn checked_domain_application(
        &self,
        n: usize,
        types: &TypeCatalog<'_>,
    ) -> Result<DomainApplication, TypeError> {
        let app = self.domain_application(n, types)?;
        types
            .check_domain_at(&app, &self.file, n)
            .map_err(|msg| TypeError::at(&self.file, n, msg))?;
        Ok(app)
    }
    pub fn index_domains(
        &self,
        types: &mut TypeCatalog<'a>,
    ) -> Result<(), TypeError> {
        let f = &self.file;
        for (n, _) in f
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.kind == NodeKind::Domain)
        {
            if types.domains.known_carrier(self.declaration(n)?).is_none() {
                self.index_domain(n, types)?;
            }
        }
        Ok(())
    }
    pub(super) fn index_domain(
        &self,
        n: usize,
        types: &mut TypeCatalog<'a>,
    ) -> Result<(), TypeError> {
        let f = &self.file;
        let node = &f.nodes[n];
        let id = self.declaration(n)?;
        let head = f
            .child(n, NodeKind::PathType)
            .ok_or_else(|| TypeError::unsupported(f, n))?;
        let indices = f
            .child(head, NodeKind::TypeArgs)
            .map_or(0, |a| f.nodes[a].children.len());
        let carrier = if indices == 0 {
            self.annotation(head, types)?
        } else {
            match self.symbol(head) {
                Some(Symbol::Builtin(s)) => ValueType::integer(s).ok_or_else(|| TypeError::unsupported(f, head))?,
                Some(Symbol::Declaration(owner)) if owner.kind == NodeKind::Data => types.open_data(owner),
                _ => return Err(TypeError::unsupported(f, head)),
            }
        };
        if indices != 0 && types.arities[id] != indices {
            return Err(TypeError::at(f, head, "indexed domain requires its complete binder tuple"));
        }
        let mut alias = Vec::new();
        for &d in &node.children {
            if d == head || f.nodes[d].kind == NodeKind::GenericParams {
                continue;
            }
            match f.nodes[d].kind {
                NodeKind::PathType => alias.push(self.domain(d)?),
                NodeKind::Requires | NodeKind::DroppedBy | NodeKind::Access => {}
                _ => return Err(TypeError::unsupported(f, d)),
            }
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
        self.entries.get(id).map_or(0, |d| d.indices)
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
            || matches!(declared,ValueType::Parameter{owner,index:0} if owner.as_ref()==id)
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
    ) -> Result<Vec<DomainApplication>, String> {
        let id = &application.declaration;
        if application.arguments.len() != self.index_arity(id) {
            return Err("incorrect indexed domain arity".into());
        }
        let mut atoms = Vec::new();
        // Exit markers distinguish shared subgraphs from a cyclic alias.
        let mut pending = vec![(id, false)];
        let mut active = HashSet::new();
        let mut done = HashSet::new();
        while let Some((id, exit)) = pending.pop() {
            if exit {
                active.remove(id);
                done.insert(id);
                continue;
            }
            if done.contains(id) {
                continue;
            }
            if !active.insert(id) {
                return Err(format!("cyclic domain alias {}", id.path));
            }
            let info = self
                .entries
                .get(id)
                .ok_or_else(|| format!("missing domain declaration {}", id.path))?;
            if !self.admits(id, carrier) {
                return Err(format!("domain {} belongs to {}, found {carrier}", id.path, info.carrier));
            }
            pending.push((id, true));
            if info.alias.is_empty() {
                atoms.push(DomainApplication {
                    declaration: id.clone(),
                    arguments: if id == &application.declaration {
                        application.arguments.clone()
                    } else {
                        Vec::new()
                    },
                });
            } else {
                pending.extend(info.alias.iter().map(|a| (*a, false)));
            }
            if ancestors && let Some(parent) = self.parent_domain(id) {
                pending.push((parent, false));
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
