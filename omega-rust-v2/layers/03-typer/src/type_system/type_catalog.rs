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
    type_universe_memberships: std::cell::RefCell<HashMap<crate::Site, Vec<(ValueType, ValueType)>>>,
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
        source: &omega_resolver::IndexedSource<'_>,
        node: usize,
    ) -> Result<ValueType, String> {
        let mut memberships = Vec::new();
        let normalized = self.normalize(ty, &mut HashSet::new(), &mut memberships)?;
        self.retain_universe_memberships(source, node, memberships);
        Ok(normalized)
    }

    fn retain_universe_memberships(
        &self,
        source: &omega_resolver::IndexedSource<'_>,
        node: usize,
        memberships: Vec<(ValueType, ValueType)>,
    ) {
        if !memberships.is_empty() {
            let mut by_site = self.type_universe_memberships.borrow_mut();
            let rows = by_site.entry(crate::Site::at(source, node)).or_default();
            for row in memberships {
                if !rows.contains(&row) {
                    rows.push(row);
                }
            }
        }
    }
    fn check_universe_arguments(
        &self,
        owner: &DeclarationId,
        arguments: &[ValueType],
        instantiate: impl Fn(&ValueType) -> ValueType,
        memberships: &mut Vec<(ValueType, ValueType)>,
    ) -> Result<(), String> {
        for (index, subject) in arguments.iter().enumerate() {
            if let Some(bound @ ValueType::Universe { .. }) = self.bound(owner, index) {
                let sort = instantiate(bound);
                Self::check_universe_member(subject, &sort)?;
                memberships.push((subject.clone(), sort));
            }
        }
        Ok(())
    }
    fn check_domain_universes(
        &self,
        app: &crate::DomainApplication,
        memberships: &mut Vec<(ValueType, ValueType)>,
    ) -> Result<(), String> {
        self.check_universe_arguments(
            &app.declaration,
            &app.arguments,
            |bound| bound.substitute(&app.declaration, &app.arguments),
            memberships,
        )
    }
    pub(crate) fn check_domain_at(
        &self,
        app: &crate::DomainApplication,
        source: &omega_resolver::IndexedSource<'_>,
        node: usize,
    ) -> Result<(), String> {
        let mut memberships = Vec::new();
        self.check_domain_universes(app, &mut memberships)?;
        self.retain_universe_memberships(source, node, memberships);
        Ok(())
    }
    pub(crate) fn record_type_universe_memberships(
        &mut self,
        output: &mut crate::TypedStage,
    ) {
        let mut groups: Vec<_> = self.type_universe_memberships.get_mut().drain().collect();
        groups.sort_unstable_by(|(a, _), (b, _)| {
            (&a.package, &a.file, a.span.get_start_offset(), a.span.get_end_offset()).cmp(&(
                &b.package,
                &b.file,
                b.span.get_start_offset(),
                b.span.get_end_offset(),
            ))
        });
        for (site, rows) in groups {
            output
                .obligations
                .extend(rows.into_iter().map(|(subject, sort)| crate::TypingObligation {
                    site: site.clone(),
                    kind: crate::ObligationKind::UniverseMembership { subject, sort },
                }));
        }
    }

    fn normalize(
        &self,
        mut ty: ValueType,
        active: &mut HashSet<DeclarationId>,
        memberships: &mut Vec<(ValueType, ValueType)>,
    ) -> Result<ValueType, String> {
        use ValueType::*;
        let mut entered = Vec::new();
        let result = loop {
            let (id, args) = match ty {
                Callable(mut c) => {
                    c.parameters = c
                        .parameters
                        .into_iter()
                        .map(|p| self.normalize(p, active, memberships))
                        .collect::<Result<_, _>>()?;
                    c.result = self.normalize(c.result, active, memberships)?;
                    break Ok(Callable(c));
                }
                AtLifetime(t, l) => break Ok(AtLifetime(Box::new(self.normalize(*t, active, memberships)?), l)),
                Function(a, b) => {
                    break Ok(Function(
                        Box::new(self.normalize(*a, active, memberships)?),
                        Box::new(self.normalize(*b, active, memberships)?),
                    ));
                }
                Qualified(base, domains) => {
                    let base = self.normalize(*base, active, memberships)?;
                    let mut atoms = base.domains().to_vec();
                    for id in domains {
                        self.check_domain_universes(&id, memberships)?;
                        atoms.extend(self.domains.expanded(&id, base.carrier(), true)?);
                    }
                    break Ok(domains::qualified(base.carrier().clone(), atoms));
                }
                Tuple(items) => {
                    break Ok(Tuple(
                        items
                            .into_iter()
                            .map(|t| self.normalize(t, active, memberships))
                            .collect::<Result<_, _>>()?,
                    ));
                }
                IndexedArray(t, n) => break Ok(IndexedArray(Box::new(self.normalize(*t, active, memberships)?), n)),
                Reference(a, t) => break Ok(Reference(a, Box::new(self.normalize(*t, active, memberships)?))),
                Array(t, n) => break Ok(Array(Box::new(self.normalize(*t, active, memberships)?), n)),
                Slice(t) => break Ok(Slice(Box::new(self.normalize(*t, active, memberships)?))),
                Applied(id, args) => (
                    id,
                    args.into_iter()
                        .map(|t| self.normalize(t, active, memberships))
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
                .map(|owner| self.arities[owner])
                .sum::<usize>();
            if arity != args.len() {
                return Err(format!("{} expects {arity} type arguments, found {}", id.path, args.len()));
            }
            let mut remaining = args.as_slice();
            for &owner in &self.scopes[id.as_ref()] {
                let (selected, rest) = remaining.split_at(self.arities[owner]);
                self.check_universe_arguments(owner, selected, |bound| self.field_type(bound, &id, &args), memberships)?;
                remaining = rest;
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
