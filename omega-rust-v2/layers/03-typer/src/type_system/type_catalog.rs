//! Generic arguments, aliases, domain carriers and their constraints share one catalog during type formation and specialization.

use super::domains::Domains;
use crate::evaluation::{integer_program, specialized_indices};
use crate::type_system::domains;
use crate::{Access, ValueType};
use omega_parser::NodeKind;
use omega_resolver::DeclarationId;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(crate) struct TypeCatalog<'a> {
    pub(crate) runtime_indices: HashSet<(&'a DeclarationId, usize)>,
    pub specializations: specialized_indices::IndexSpecializations,
    pub operator_tokens: HashSet<&'a str>,
    pub deferred_sites: std::cell::RefCell<HashSet<crate::Site>>,
    pub integer_program: integer_program::IntegerProgram,
    pub formations: std::cell::RefCell<HashMap<DeclarationId, HashMap<crate::Site, ValueType>>>,
    pub open_indices: std::cell::RefCell<HashMap<crate::Site, (usize, ValueType)>>,
    pub mathematical: HashMap<&'a DeclarationId, ValueType>,
    pub evaluator: Option<&'a dyn crate::StaticIntegerEvaluator>,
    pub evaluations: std::cell::RefCell<HashMap<crate::StaticIntegerRequest, (usize, num_bigint::BigInt)>>,
    pub bounds: HashMap<(&'a DeclarationId, usize), crate::Multiplicity>,
    pub multiplicities: HashMap<&'a DeclarationId, crate::Multiplicity>,
    pub witnesses: HashMap<&'a DeclarationId, usize>,
    pub(crate) equations: HashMap<&'a DeclarationId, Vec<(usize, ValueType)>>,
    pub(crate) scopes: HashMap<&'a DeclarationId, Vec<&'a DeclarationId>>,
    pub domains: Domains<'a>,
    pub positive_widths: HashSet<ValueType>,
    pub(crate) lifetimes: HashSet<(&'a DeclarationId, usize)>,
    pub(crate) const_carriers: HashMap<(&'a DeclarationId, usize), ValueType>,
    pub arities: HashMap<&'a DeclarationId, usize>,
    pub(crate) aliases: HashMap<&'a DeclarationId, ValueType>,
}

impl TypeCatalog<'_> {
    pub(crate) fn lifetime_slot(
        &self,
        owner: &DeclarationId,
        index: usize,
    ) -> bool {
        self.lifetimes.contains(&(owner, index))
    }
    pub fn multiplicity(
        &self,
        ty: &ValueType,
    ) -> crate::Multiplicity {
        use crate::Multiplicity::*;
        match ty {
            ValueType::Array(t, _) | ValueType::IndexedArray(t, _) | ValueType::Qualified(t, _) | ValueType::AtLifetime(t, _) => self.multiplicity(t),
            ValueType::Nominal(id) | ValueType::Applied(id, _) => self.multiplicities.get(id.as_ref()).copied().unwrap_or(Affine),
            ValueType::Parameter { owner, index } => self.bounds.get(&(owner, *index)).copied().unwrap_or(Affine),
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
        if let ValueType::StaticInteger { value, .. } = ty
            && value.sign() != num_bigint::Sign::Plus
            && self.positive_widths.contains(&ValueType::Parameter {
                owner: Box::new(id.clone()),
                index,
            })
        {
            return Err("integer width must be positive".into());
        }
        if let Some(bound) = self.bounds.get(&(id, index))
            && self.multiplicity(ty) != *bound
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
        let arity = self.arities[owner];
        if values.len() > arity {
            return Err(format!("excess arguments for {}", owner.path));
        }
        let supplied = values.len();
        let mut slots = values.into_iter().map(Some).collect::<Vec<_>>();
        slots.resize(arity, None);
        if let Some(equations) = self.equations.get(owner) {
            loop {
                let before = slots.iter().filter(|s| s.is_none()).count();
                for (index, pattern) in equations {
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
    /// Compress non-generic transparent chains once, including forward references.
    /// Leave cyclic chains for ordinary normalization to diagnose at their source.
    pub fn flatten_plain_aliases(&mut self) {
        let ids: Vec<_> = self.aliases.keys().copied().collect();
        let mut done = HashSet::new();
        for id in ids {
            let mut next = id;
            let mut path = Vec::new();
            let mut active = HashSet::new();
            while !done.contains(next) && active.insert(next) {
                let Some(ValueType::Nominal(target)) = self.aliases.get(next) else { break };
                let Some((&decl, _)) = self.aliases.get_key_value(target.as_ref()) else {
                    break;
                };
                path.push(next);
                next = decl;
            }
            if !path.contains(&next)
                && let Some(target) = self.aliases.get(next).cloned()
            {
                for &entry in &path {
                    self.aliases.insert(entry, target.clone());
                }
            }
            done.extend(path);
        }
    }
    pub fn formals(
        &self,
        id: &DeclarationId,
    ) -> Vec<ValueType> {
        (0..self.arities[id])
            .map(|index| ValueType::Parameter {
                owner: Box::new(id.clone()),
                index,
            })
            .collect()
    }
    pub fn open_data(
        &self,
        id: &DeclarationId,
    ) -> ValueType {
        let args = self.scopes[id]
            .iter()
            .flat_map(|owner| self.formals(owner))
            .collect::<Vec<_>>();
        if args.is_empty() {
            ValueType::Nominal(Box::new(id.clone()))
        } else {
            ValueType::Applied(Box::new(id.clone()), args)
        }
    }
    pub fn field_type(
        &self,
        ty: &ValueType,
        id: &DeclarationId,
        args: &[ValueType],
    ) -> ValueType {
        let mut result = ty.clone();
        let mut remaining = args;
        for &owner in &self.scopes[id] {
            let (selected, rest) = remaining.split_at(self.arities[owner]);
            result = result.substitute(owner, selected);
            remaining = rest;
        }
        result
    }
    pub fn bound<'b>(
        &'b self,
        owner: &'b DeclarationId,
        index: usize,
    ) -> Option<&'b ValueType> {
        self.const_carriers.get(&(owner, index))
    }
    pub fn parameter_carrier<'b>(
        &'b self,
        ty: &'b ValueType,
    ) -> Option<&'b ValueType> {
        match ty {
            ValueType::Parameter { owner, index } => self.const_carriers.get(&(owner, *index)),
            _ => None,
        }
    }

    pub(crate) fn normalize(
        &self,
        mut ty: ValueType,
        active: &mut HashSet<DeclarationId>,
    ) -> Result<ValueType, String> {
        use ValueType::*;
        let mut entered = Vec::new();
        let result = loop {
            let (id, args) = match ty {
                Callable(mut c) => {
                    c.parameters = c
                        .parameters
                        .into_iter()
                        .map(|p| self.normalize(p, active))
                        .collect::<Result<_, _>>()?;
                    c.result = self.normalize(c.result, active)?;
                    break Ok(Callable(c));
                }
                AtLifetime(t, l) => break Ok(AtLifetime(Box::new(self.normalize(*t, active)?), l)),
                Function(a, b) => break Ok(Function(Box::new(self.normalize(*a, active)?), Box::new(self.normalize(*b, active)?))),
                Qualified(base, domains) => {
                    let base = self.normalize(*base, active)?;
                    let mut atoms = base.domains().to_vec();
                    for id in domains {
                        atoms.extend(self.domains.expanded(&id, base.carrier(), true)?);
                    }
                    break Ok(domains::qualified(base.carrier().clone(), atoms));
                }
                Tuple(items) => {
                    break Ok(Tuple(
                        items
                            .into_iter()
                            .map(|t| self.normalize(t, active))
                            .collect::<Result<_, _>>()?,
                    ));
                }
                IndexedArray(t, n) => break Ok(IndexedArray(Box::new(self.normalize(*t, active)?), n)),
                Reference(a, t) => break Ok(Reference(a, Box::new(self.normalize(*t, active)?))),
                Array(t, n) => break Ok(Array(Box::new(self.normalize(*t, active)?), n)),
                Slice(t) => break Ok(Slice(Box::new(self.normalize(*t, active)?))),
                Applied(id, args) => (
                    id,
                    args.into_iter()
                        .map(|t| self.normalize(t, active))
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                StaticInteger { ref carrier, ref value }
                    if !(Exact {
                        value: num_rational::BigRational::from_integer(value.clone()),
                        fractional_origin: None,
                    })
                    .fits(carrier) =>
                {
                    return Err(format!("static index {value} exceeds {carrier}"));
                }
                Nominal(id) => (id, Vec::new()),
                other => break Ok(other),
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
                let atoms = self.domains.expanded(&app, carrier, true)?;
                break Ok(domains::qualified(carrier.clone(), atoms));
            }
            let arity = self
                .scopes
                .get(id.as_ref())
                .ok_or_else(|| format!("missing declaration for {}", id.path))?
                .iter()
                .map(|owner| self.arities[owner])
                .sum::<usize>();
            if arity != args.len() {
                return Err(format!("{} expects {arity} type arguments, found {}", id.path, args.len()));
            }
            if let Some(target) = self.aliases.get(id.as_ref()) {
                if !active.insert((*id).clone()) {
                    return Err(format!("cyclic type alias {}", id.path));
                }
                ty = self.field_type(target, &id, &args);
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
