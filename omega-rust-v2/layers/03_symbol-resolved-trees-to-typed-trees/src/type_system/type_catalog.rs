//! Generic arguments, aliases, domain carriers and their constraints share one catalog during type formation and specialization.

use super::domains::Domains;
use crate::evaluation::{check_static_indices::IndexEvaluations, primitive_indices::LiteralConstants};
use crate::type_system::domains;
use crate::{Access, ValueType};
use omega_parser::NodeKind;
use omega_resolver::DeclarationId;
use std::collections::{HashMap, HashSet};

use super::formation::{Formations, TypeRequirement};
use crate::declarations::resolved_source_file::ResolvedSourceFile;

#[derive(Default)]
pub(crate) struct TypeCatalog<'a> {
    pub(crate) indices: IndexEvaluations<'a>,
    pub(crate) formations: Formations,
    pub(crate) declarations: DeclaredTypes<'a>,
}

impl<'a> std::ops::Deref for TypeCatalog<'a> {
    type Target = DeclaredTypes<'a>;
    fn deref(&self) -> &Self::Target {
        &self.declarations
    }
}
impl std::ops::DerefMut for TypeCatalog<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.declarations
    }
}

#[derive(Clone, Default)]
pub(crate) struct DeclaredTypes<'a> {
    pub(crate) generics: HashMap<&'a DeclarationId, GenericParameters>,
    pub trait_operator_tokens: HashMap<&'a DeclarationId, HashSet<&'a str>>,
    pub literal_constants: LiteralConstants,
    pub mathematical: HashMap<&'a DeclarationId, ValueType>,
    pub data_properties: HashMap<&'a DeclarationId, crate::DataProperties>,
    pub(crate) scopes: HashMap<&'a DeclarationId, Vec<&'a DeclarationId>>,
    pub domains: Domains<'a>,
    pub service_traits: HashSet<&'a DeclarationId>,
    pub(crate) aliases: HashMap<&'a DeclarationId, ValueType>,
}

/// A specialization supplies the public parameters in order. Existential witnesses are private to the data value.
#[derive(Clone, Default)]
pub(crate) struct GenericParameters {
    pub parameters: Vec<GenericParameter>,
    pub witness_count: usize,
    pub equations: Vec<(usize, ValueType)>,
}

#[derive(Clone)]
pub(crate) struct GenericParameter {
    pub kind: NodeKind,
    pub multiplicity: Option<crate::Multiplicity>,
    pub carrier: Option<Box<ValueType>>,
}

impl TypeCatalog<'_> {
    fn parameter(
        &self,
        owner: &DeclarationId,
        index: usize,
    ) -> Option<&GenericParameter> {
        self.generics.get(owner)?.parameters.get(index)
    }
    pub(crate) fn arity(
        &self,
        owner: &DeclarationId,
    ) -> usize {
        self.generics[owner].parameters.len()
    }

    pub(crate) fn witness_count(
        &self,
        owner: &DeclarationId,
    ) -> usize {
        self.generics.get(owner).map_or(0, |g| g.witness_count)
    }

    pub(crate) fn admits_runtime_index(
        &self,
        owner: &DeclarationId,
        index: usize,
    ) -> bool {
        self.parameter(owner, index)
            .is_some_and(|p| p.kind == NodeKind::KindedParam && matches!(p.carrier.as_deref(), Some(ValueType::Integer { .. })))
    }

    pub(crate) fn set_carrier(
        &mut self,
        owner: &DeclarationId,
        index: usize,
        carrier: ValueType,
    ) -> Option<Box<ValueType>> {
        self.generics
            .get_mut(owner)
            .expect("indexed declaration")
            .parameters[index]
            .carrier
            .replace(Box::new(carrier))
    }

    pub(crate) fn lifetime_slot(
        &self,
        owner: &DeclarationId,
        index: usize,
    ) -> bool {
        self.parameter(owner, index)
            .is_some_and(|p| p.kind == NodeKind::LifetimeParam)
    }
    pub fn multiplicity(
        &self,
        ty: &ValueType,
    ) -> crate::Multiplicity {
        use crate::Multiplicity::*;
        match ty {
            ValueType::Array(t, _) | ValueType::IndexedArray(t, _) | ValueType::Qualified(t, _) | ValueType::AtLifetime(t, _) => self.multiplicity(t),
            ValueType::Tuple(parts) => parts.iter().map(|t| self.multiplicity(t)).max().unwrap_or(Copy),
            ValueType::Nominal(id) | ValueType::Applied(id, _) => self
                .data_properties
                .get(id.as_ref())
                .map_or(Affine, |p| p.multiplicity),
            ValueType::Parameter { owner, index } => self
                .parameter(owner, *index)
                .and_then(|p| p.multiplicity)
                .unwrap_or(Affine),
            ValueType::Reference(Access::Mut | Access::Write, _) | ValueType::Witness { .. } => Affine,
            _ => Copy,
        }
    }
    pub fn validate_bound(
        &self,
        id: &DeclarationId,
        index: usize,
        ty: &ValueType,
    ) -> Result<(), String> {
        if let Some(bound) = self.parameter(id, index).and_then(|p| p.multiplicity)
            && self.multiplicity(ty) != bound
        {
            return Err(format!("type argument {ty} does not satisfy {bound:?} for {}", id.path));
        }
        Ok(())
    }
    pub(crate) fn solve_arguments(
        &self,
        owner: &DeclarationId,
        values: Vec<ValueType>,
    ) -> Result<Vec<ValueType>, String> {
        let arity = self.arity(owner);
        if values.len() > arity {
            return Err(format!("excess arguments for {}", owner.path));
        }
        let supplied = values.len();
        let mut slots = values.into_iter().map(Some).collect::<Vec<_>>();
        slots.resize(arity, None);
        if !self.generics[owner].equations.is_empty() {
            loop {
                let before = slots.iter().filter(|s| s.is_none()).count();
                for (index, pattern) in &self.generics[owner].equations {
                    if let Some(actual) = slots[*index].clone()
                        && !pattern.infer(&actual, owner, &mut slots)
                    {
                        return Err(format!("conflicting type equation for {}", owner.path));
                    }
                }
                if before == slots.iter().filter(|s| s.is_none()).count() {
                    break;
                }
            }
        }
        slots
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| format!("{} expects {arity} arguments, found {supplied} with incomplete inference", owner.path))
    }
    pub fn formals(
        &self,
        id: &DeclarationId,
    ) -> Vec<ValueType> {
        (0..self.arity(id))
            .map(|index| ValueType::Parameter {
                owner: Box::new(id.clone()),
                index,
            })
            .collect()
    }
    pub(crate) fn specialization_formals(
        &self,
        id: &DeclarationId,
    ) -> Vec<ValueType> {
        self.specialization_scopes(id)
            .flat_map(|owner| self.formals(owner))
            .collect()
    }
    pub(crate) fn specialization_scopes<'b>(
        &'b self,
        owner: &'b DeclarationId,
    ) -> impl Iterator<Item = &'b DeclarationId> {
        let scopes = self.scopes.get(owner).map(Vec::as_slice);
        scopes
            .into_iter()
            .flatten()
            .copied()
            .chain(scopes.is_none().then_some(owner))
    }
    pub(crate) fn specialization_slot(
        &self,
        owner: &DeclarationId,
        binder_owner: &DeclarationId,
        index: usize,
    ) -> Option<usize> {
        let scopes = self
            .scopes
            .get(owner)
            .map_or(std::slice::from_ref(&owner), Vec::as_slice);
        let mut offset = 0;
        for &scope in scopes {
            if scope == binder_owner {
                return (index < self.arity(scope)).then_some(offset + index);
            }
            offset += self.arity(scope);
        }
        None
    }

    pub fn open_data(
        &self,
        id: &DeclarationId,
    ) -> ValueType {
        let args = self.specialization_formals(id);
        if args.is_empty() {
            ValueType::Nominal(Box::new(id.clone()))
        } else {
            ValueType::Applied(Box::new(id.clone()), args)
        }
    }
    pub(crate) fn scoped_arguments<'b, T>(
        &'b self,
        id: &'b DeclarationId,
        mut arguments: &'b [T],
    ) -> impl Iterator<Item = (&'b DeclarationId, &'b [T])> {
        let scopes = self.scopes.get(id);
        let single = scopes.is_none().then_some((id, arguments));
        scopes
            .into_iter()
            .flatten()
            .map(move |&owner| {
                let (selected, rest) = arguments.split_at(self.arity(owner));
                arguments = rest;
                (owner, selected)
            })
            .chain(single)
    }

    pub fn apply_type_arguments(
        &self,
        ty: &ValueType,
        id: &DeclarationId,
        args: &[ValueType],
    ) -> ValueType {
        self.scoped_arguments(id, args)
            .fold(ty.clone(), |field, (owner, selected)| field.substitute(owner, selected))
    }
    pub fn bound<'b>(
        &'b self,
        owner: &'b DeclarationId,
        index: usize,
    ) -> Option<&'b ValueType> {
        self.parameter(owner, index)?.carrier.as_deref()
    }
    pub fn parameter_carrier<'b>(
        &'b self,
        ty: &'b ValueType,
    ) -> Option<&'b ValueType> {
        match ty {
            ValueType::Parameter { owner, index } => self.bound(owner, *index),
            _ => None,
        }
    }

    // A universe itself can only inhabit the ordinary universe at its successor.
    // Other subjects retain a membership obligation for the proof checker.
    pub(crate) fn check_universe_member(
        subject: &ValueType,
        sort: &ValueType,
    ) -> Result<(), String> {
        if let (ValueType::Universe { level, .. }, ValueType::Universe { strict, level: outer }) = (subject, sort)
            && (*strict || matches!((level.as_ref(),outer.as_ref()),(ValueType::Level(actual),ValueType::Level(required)) if actual+1u8 != *required))
        {
            return Err("universe argument requires its exact successor universe".into());
        }
        Ok(())
    }

    pub(crate) fn normalize_at(
        &self,
        ty: ValueType,
        unit: &ResolvedSourceFile<'_>,
        node: usize,
    ) -> Result<ValueType, String> {
        let mut requirements = Vec::new();
        let normalized = self.normalize(ty, &mut HashSet::new(), &mut requirements)?;
        self.indices
            .retain_demands_at(unit.file.node_id(node), &normalized);
        self.formations.retain(unit, node, requirements);
        Ok(normalized)
    }

    pub(super) fn require_type_application(
        &self,
        owner: &DeclarationId,
        arguments: &[ValueType],
        requirements: &mut Vec<TypeRequirement>,
    ) -> Result<(), String> {
        if !arguments.is_empty() {
            requirements.push(TypeRequirement::Application(owner.clone(), arguments.to_vec()));
            for (scope, values) in self.scoped_arguments(owner, arguments) {
                self.check_universe_arguments(
                    scope,
                    values,
                    |bound| self.apply_type_arguments(bound, owner, arguments),
                    |subject, sort| requirements.push(TypeRequirement::Universe(subject, sort)),
                )?;
            }
        }
        Ok(())
    }
    pub(crate) fn check_universe_arguments(
        &self,
        owner: &DeclarationId,
        arguments: &[ValueType],
        instantiate: impl Fn(&ValueType) -> ValueType,
        mut record: impl FnMut(ValueType, ValueType),
    ) -> Result<(), String> {
        for (index, subject) in arguments.iter().enumerate() {
            if let Some(bound @ ValueType::Universe { .. }) = self.bound(owner, index) {
                let sort = instantiate(bound);
                Self::check_universe_member(subject, &sort)?;
                record(subject.clone(), sort);
            }
        }
        Ok(())
    }
    fn expand_domain(
        &self,
        app: &crate::DomainApplication,
        carrier: &ValueType,
        ancestors: bool,
        active: &mut HashSet<DeclarationId>,
        requirements: &mut Vec<TypeRequirement>,
    ) -> Result<Vec<crate::DomainApplication>, String> {
        self.domains.expanded(app, carrier, ancestors, |applied| {
            applied.arguments = std::mem::take(&mut applied.arguments)
                .into_iter()
                .map(|argument| self.normalize(argument, active, requirements))
                .collect::<Result<_, _>>()?;
            let arguments = self.domains.bind_arguments(applied, carrier);
            self.require_type_application(&applied.declaration, &arguments, requirements)
        })
    }
    pub(crate) fn expanded_domain_at(
        &self,
        app: &crate::DomainApplication,
        carrier: &ValueType,
        ancestors: bool,
        unit: &ResolvedSourceFile<'_>,
        node: usize,
    ) -> Result<Vec<crate::DomainApplication>, String> {
        let mut requirements = Vec::new();
        let atoms = self.expand_domain(app, carrier, ancestors, &mut HashSet::new(), &mut requirements)?;
        self.formations.retain(unit, node, requirements);
        Ok(atoms)
    }
    fn normalize(
        &self,
        mut ty: ValueType,
        active: &mut HashSet<DeclarationId>,
        requirements: &mut Vec<TypeRequirement>,
    ) -> Result<ValueType, String> {
        use ValueType::*;
        let mut entered = Vec::new();
        let result = loop {
            ty.validate_index()?;
            let (id, args) = match ty {
                Qualified(base, domains) => {
                    let base = self.normalize(*base, active, requirements)?;
                    let mut atoms = base.domains().to_vec();
                    for id in domains {
                        atoms.extend(self.expand_domain(&id, base.carrier(), true, active, requirements)?);
                    }
                    break Ok(domains::qualified(base.carrier().clone(), atoms));
                }
                Applied(id, args) => (
                    id,
                    args.into_iter()
                        .map(|t| self.normalize(t, active, requirements))
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                Nominal(id) => (id, Vec::new()),
                other => break other.try_map_children(0, |part, _| self.normalize(part, active, requirements)),
            };
            if id.kind == NodeKind::Domain {
                let carrier = self
                    .domains
                    .known_carrier(&id)
                    .ok_or_else(|| format!("unresolved domain carrier {}", id.path))?;
                let app = crate::DomainApplication {
                    declaration: *id,
                    arguments: args,
                };
                // A child's declared carrier includes its parent qualification.
                // Normalize both domain spellings through the same qualified type.
                ty = Qualified(Box::new(carrier.clone()), vec![app]);
                continue;
            }
            let arity = self
                .scopes
                .get(id.as_ref())
                .ok_or_else(|| format!("missing declaration for {}", id.path))?
                .iter()
                .map(|owner| self.arity(owner))
                .sum::<usize>();
            if arity != args.len() {
                return Err(format!("{} expects {arity} type arguments, found {}", id.path, args.len()));
            }
            self.require_type_application(&id, &args, requirements)?;
            if let Some(target) = self.aliases.get(id.as_ref()) {
                if !active.insert((*id).clone()) {
                    return Err(format!("cyclic type alias {}", id.path));
                }
                ty = self.apply_type_arguments(target, &id, &args);
                entered.push(id);
                continue;
            }
            break Ok(if args.is_empty() { Nominal(id) } else { Applied(id, args) });
        };
        for id in entered {
            active.remove(id.as_ref());
        }
        result
    }
}
