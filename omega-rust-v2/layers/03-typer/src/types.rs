use super::domains::Domains;
use super::{Access, TypeError, Unit, ValueType};
use omega_lexer::SourceSpan;
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct Types<'a> {
    pub open_indices: std::cell::RefCell<HashMap<super::Site, (usize, ValueType)>>,
    pub evaluator: Option<&'a dyn super::StaticIntegerEvaluator>,
    pub evaluations: std::cell::RefCell<HashMap<super::StaticIntegerRequest, (usize, num_bigint::BigInt)>>,
    pub bounds: HashMap<(&'a DeclarationId, usize), crate::Multiplicity>,
    pub multiplicities: HashMap<&'a DeclarationId, crate::Multiplicity>,
    pub witnesses: HashMap<&'a DeclarationId, usize>,
    equations: HashMap<&'a DeclarationId, Vec<(usize, ValueType)>>,
    scopes: HashMap<&'a DeclarationId, Vec<&'a DeclarationId>>,
    pub domains: Domains<'a>,
    pub positive_widths: HashSet<ValueType>,
    lifetimes: HashSet<(&'a DeclarationId, usize)>,
    pub(super) const_carriers: HashMap<(&'a DeclarationId, usize), ValueType>,
    pub arities: HashMap<&'a DeclarationId, usize>,
    aliases: HashMap<&'a DeclarationId, ValueType>,
}

impl<'a> Unit<'a> {
    pub fn symbol(
        &self,
        n: usize,
    ) -> Option<&'a Symbol> {
        let mut words = self.file.nodes[n].words();
        let first = words.next()?;
        let last = words.next_back().unwrap_or(first);
        self.names
            .get(&SourceSpan::new(first.span.get_start_offset(), last.span.get_end_offset()))
            .copied()
    }
    pub fn access(
        &self,
        n: usize,
    ) -> Access {
        match self.file.nodes[n].words().next().map(|w| w.text) {
            Some("mut") => Access::Mut,
            Some("write") => Access::Write,
            _ => Access::Shared,
        }
    }
    pub fn index_types(
        &mut self,
        types: &mut Types<'a>,
    ) -> Result<(), TypeError> {
        let f = &self.file;
        for &id in self.definitions.values() {
            if matches!(
                id.kind,
                NodeKind::Machine | NodeKind::AsmMachine | NodeKind::MathLet | NodeKind::Data | NodeKind::Trait | NodeKind::Conformance | NodeKind::Domain
            ) {
                types.arities.insert(id, 0);
            }
        }
        for (_, node) in f
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.kind == NodeKind::GenericParams)
        {
            let owner = self.declaration(node.parent.unwrap())?;
            let mut index = 0;
            for &p in &node.children {
                if f.nodes[p].kind == NodeKind::SomeParam {
                    if owner.kind != NodeKind::Data {
                        return Err(TypeError::unsupported(f, p));
                    }
                    let name = f.nodes[p]
                        .words()
                        .nth(1)
                        .ok_or_else(|| TypeError::unsupported(f, p))?;
                    let witness = types.witnesses.entry(owner).or_default();
                    self.parameters.insert(
                        name.span,
                        ValueType::Witness {
                            owner: Box::new(owner.clone()),
                            index: *witness,
                            opening: None,
                        },
                    );
                    *witness += 1;
                    continue;
                }
                let name = match f.nodes[p].kind {
                    NodeKind::TypeParam => {
                        for &attr in &f.nodes[p].children {
                            let prop = crate::properties::read(self, attr)?;
                            if prop.carry.is_some() {
                                return Err(TypeError::unsupported(f, attr));
                            }
                            types.bounds.insert((owner, index), prop.multiplicity);
                        }
                        f.nodes[p].words().next()
                    }
                    NodeKind::KindedParam => f.nodes[p].words().next(),
                    NodeKind::ConstParam => f.nodes[p].words().nth(1),
                    NodeKind::LifetimeParam => {
                        types.lifetimes.insert((owner, index));
                        f.nodes[p].words().next()
                    }
                    _ => return Err(TypeError::unsupported(f, p)),
                }
                .ok_or_else(|| TypeError::unsupported(f, p))?;
                self.parameters.insert(
                    name.span,
                    ValueType::Parameter {
                        owner: Box::new(owner.clone()),
                        index,
                    },
                );
                index += 1;
            }
            types.arities.insert(owner, index);
        }
        Ok(())
    }
    pub fn application_segment(
        &self,
        path: usize,
        section: usize,
    ) -> Option<&'a Symbol> {
        let node = &self.file.nodes[path];
        let before = self.file.span(section).get_start_offset();
        let mut prefix = node.words().take_while(|w| w.span.get_start_offset() < before);
        let first = prefix.next()?;
        let last = prefix.last().unwrap_or(first);
        self.names
            .get(&SourceSpan::new(first.span.get_start_offset(), last.span.get_end_offset()))
            .copied()
    }
    pub fn index_scopes(
        &self,
        types: &mut Types<'a>,
    ) -> Result<(), TypeError> {
        let f = &self.file;
        for (n, _) in f
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, d)| d.kind == NodeKind::Data)
        {
            let id = self.declaration(n)?;
            let mut chain = f
                .enclosing_contexts(n)
                .filter(|&a| f.nodes[a].kind == NodeKind::Data)
                .map(|a| self.declaration(a))
                .collect::<Result<Vec<_>, _>>()?;
            chain.reverse();
            chain.push(id);
            types.scopes.insert(id, chain);
            if let Some(attr) = f.child(n, NodeKind::Attributes) {
                types
                    .multiplicities
                    .insert(id, crate::properties::read(self, attr)?.multiplicity);
            }
        }
        Ok(())
    }
    pub fn data_application(
        &self,
        n: usize,
        id: &DeclarationId,
        types: &Types<'_>,
        candidate: Option<&DeclarationId>,
    ) -> Result<ValueType, TypeError> {
        let f = &self.file;
        let node = &f.nodes[n];
        let mut groups = HashMap::new();
        for &section in &node.children {
            if !matches!(f.nodes[section].kind, NodeKind::TypeArgs | NodeKind::ExpressionArgs) {
                return Err(TypeError::unsupported(f, section));
            }
            let before = f.span(section).get_start_offset();
            let mut prefix = node.words().take_while(|w| w.span.get_start_offset() < before);
            let first = prefix
                .next()
                .ok_or_else(|| TypeError::unsupported(f, section))?;
            let last = prefix.last().unwrap_or(first);
            let span = SourceSpan::new(first.span.get_start_offset(), last.span.get_end_offset());
            let Some(Symbol::Declaration(owner)) = self.names.get(&span).copied() else {
                return Err(TypeError::at(f, section, "unresolved type application owner"));
            };
            let values = f.nodes[section]
                .children
                .iter()
                .enumerate()
                .map(|(i, &a)| self.raw_argument(a, types, owner, i, candidate))
                .collect::<Result<Vec<_>, _>>()?;
            let values = types
                .solve_arguments(owner, values)
                .map_err(|m| TypeError::at(f, section, m))?;
            groups.insert(owner, values);
        }
        let mut args = Vec::new();
        for &owner in &types.scopes[id] {
            if let Some(selected) = groups.remove(owner) {
                for (index, arg) in selected.iter().enumerate() {
                    types
                        .validate_bound(owner, index, arg)
                        .map_err(|m| TypeError::at(f, n, m))?;
                }
                args.extend(selected);
            } else if owner != id
                && f.enclosing_contexts(n)
                    .filter_map(|i| self.definitions.get(&f.span(i)))
                    .any(|&d| d == owner)
            {
                args.extend(types.formals(owner));
            } else if types.arities[owner] != 0 {
                return Err(TypeError::at(
                    f,
                    n,
                    format!("{} expects {} type arguments, found 0", owner.path, types.arities[owner]),
                ));
            }
        }
        if !groups.is_empty() {
            return Err(TypeError::at(f, n, "type arguments do not belong to the selected carrier"));
        }
        Ok(if args.is_empty() {
            ValueType::Nominal(Box::new(id.clone()))
        } else {
            ValueType::Applied(Box::new(id.clone()), args)
        })
    }
    pub fn index_const_carriers(
        &self,
        types: &mut Types<'a>,
    ) -> Result<(), TypeError> {
        for (n, node) in self
            .file
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| matches!(node.kind, NodeKind::ConstParam | NodeKind::KindedParam))
        {
            if node
                .parent
                .is_some_and(|p| self.file.nodes[p].kind == NodeKind::Fragment)
            {
                continue;
            }
            let telescope = node
                .parent
                .ok_or_else(|| TypeError::unsupported(&self.file, n))?;
            let owner = self.declaration(
                self.file.nodes[telescope]
                    .parent
                    .ok_or_else(|| TypeError::unsupported(&self.file, n))?,
            )?;
            let index = self.file.nodes[telescope]
                .children
                .iter()
                .take_while(|&&i| i != n)
                .filter(|&&i| self.file.nodes[i].kind != NodeKind::SomeParam)
                .count();
            let annotation = self.single_child(n)?;
            if matches!(self.symbol(annotation),Some(Symbol::Declaration(id)) if id.kind==NodeKind::Machine)
                || self.file.nodes[annotation].kind == NodeKind::PathType && self.file.nodes[annotation].path() == "machine"
            {
                continue;
            }
            let carrier = if let Some(Symbol::Declaration(schema)) = self.symbol(annotation)
                && schema.kind == NodeKind::Trait
            {
                let args = self
                    .file
                    .child(annotation, NodeKind::TypeArgs)
                    .into_iter()
                    .flat_map(|a| self.file.nodes[a].children.iter().copied())
                    .enumerate()
                    .map(|(i, arg)| self.argument(arg, types, schema, i))
                    .collect::<Result<Vec<_>, _>>()?;
                if args.len() != types.arities[schema] {
                    return Err(TypeError::at(&self.file, annotation, "incorrect evidence application arity"));
                }
                ValueType::Evidence(Box::new(crate::RequirementSlot {
                    declaration: schema.clone(),
                    trait_arguments: args,
                }))
            } else {
                self.raw_type(annotation, None, types)?
            };
            if !matches!(carrier, ValueType::Integer { .. } | ValueType::Callable(_) | ValueType::Evidence(_)) {
                return Err(TypeError::unsupported(&self.file, n));
            }
            types.const_carriers.insert((owner, index), carrier);
        }
        Ok(())
    }
    pub fn argument(
        &self,
        n: usize,
        types: &Types<'_>,
        owner: &DeclarationId,
        index: usize,
    ) -> Result<ValueType, TypeError> {
        let arg = self.raw_argument(n, types, owner, index, Some(owner))?;
        types
            .normalize(arg, &mut HashSet::new())
            .map_err(|m| TypeError::at(&self.file, n, m))
    }
    fn raw_argument(
        &self,
        n: usize,
        types: &Types<'_>,
        owner: &DeclarationId,
        index: usize,
        candidate: Option<&DeclarationId>,
    ) -> Result<ValueType, TypeError> {
        if types.lifetimes.contains(&(owner, index)) {
            if self.file.nodes[n].kind != NodeKind::Lifetime {
                return Err(TypeError::at(&self.file, n, "expected a lifetime argument"));
            }
            return self.raw_type(n, candidate, types);
        }
        if self.file.nodes[n].kind == NodeKind::Lifetime {
            return Err(TypeError::at(&self.file, n, "unexpected lifetime argument"));
        }
        let Some(carrier) = types.const_carriers.get(&(owner, index)) else {
            return self.raw_type(n, candidate, types);
        };
        let symbol = match self.symbol(n) {
            Some(Symbol::ByOverload(options)) => options
                .iter()
                .find(|o| Some(&o.candidate) == candidate)
                .and_then(|o| o.target.as_deref()),
            other => other,
        };
        if matches!(carrier, ValueType::Evidence(_))
            && let Some(Symbol::Declaration(id)) = symbol
            && id.kind == NodeKind::Conformance
        {
            return Ok(ValueType::Implementation(Box::new(id.clone())));
        }
        if matches!(carrier, ValueType::Callable(_))
            && let Some(Symbol::Declaration(id)) = symbol
            && id.kind == NodeKind::Machine
        {
            return Ok(ValueType::MachineSelection(Box::new(id.clone())));
        }
        if let Some(Symbol::Binding {
            span,
            kind: NodeKind::ConstParam | NodeKind::KindedParam,
        }) = symbol
        {
            let value = self
                .parameters
                .get(span)
                .ok_or_else(|| TypeError::unsupported(&self.file, n))?;
            if types.parameter_carrier(value) != Some(carrier)
                && !matches!(
                    (types.parameter_carrier(value), carrier),
                    (Some(ValueType::Callable(_)), ValueType::Callable(_)) | (Some(ValueType::Evidence(_)), ValueType::Evidence(_))
                )
            {
                return Err(TypeError::at(&self.file, n, "const parameter carrier mismatch"));
            }
            return Ok(value.clone());
        }
        if let Some(term) = self.open_index(types, n, carrier)? {
            return Ok(term);
        }
        let (value, suffix) = self.closed_number(types, n, carrier)?;
        if suffix.as_ref().is_some_and(|t| t != carrier)
            || !value.fits(carrier)
            || matches!(
                value,
                ValueType::Exact {
                    fractional_origin: Some(_),
                    ..
                }
            )
        {
            return Err(TypeError::at(&self.file, n, format!("const argument {value} does not land in {carrier}")));
        }
        let ValueType::Exact { value, .. } = value else {
            return Err(TypeError::unsupported(&self.file, n));
        };
        Ok(ValueType::StaticInteger {
            carrier: Box::new(carrier.clone()),
            value: value.to_integer(),
        })
    }
    pub(super) fn closed_number(
        &self,
        types: &Types<'_>,
        n: usize,
        carrier: &ValueType,
    ) -> Result<(ValueType, Option<ValueType>), TypeError> {
        let f = &self.file;
        let needs_evaluation = (n..f.nodes[n].end)
            .any(|i| matches!(f.nodes[i].kind, NodeKind::Call | NodeKind::PathExpression | NodeKind::PathType | NodeKind::Cast))
            || f.nodes[n].kind != NodeKind::Literal
                && (n..f.nodes[n].end).any(|i| f.nodes[i].kind == NodeKind::Literal && super::numeric::literal(f, i).is_ok_and(|(_, s)| s.is_some()));
        if !needs_evaluation {
            return super::numeric::static_number(f, n);
        }
        if (n..f.nodes[n].end).any(|i| matches!(self.symbol(i), Some(Symbol::Binding { .. }))) {
            return Err(TypeError::at(f, n, "static expression needs specialization before evaluation"));
        }
        let request = super::StaticIntegerRequest {
            expression: super::Site::at(f, n),
            carrier: carrier.clone(),
        };
        let cached = types.evaluations.borrow().get(&request).cloned();
        let value = if let Some((_, value)) = cached {
            value
        } else {
            let evaluator = types
                .evaluator
                .ok_or_else(|| TypeError::at(f, n, format!("static expression requires evaluation as {carrier}")))?;
            let value = evaluator
                .evaluate(&request)
                .map_err(|m| TypeError::at(f, n, m))?;
            types
                .evaluations
                .borrow_mut()
                .insert(request, (n, value.clone()));
            value
        };
        Ok((
            ValueType::Exact {
                value: num_rational::BigRational::from_integer(value),
                fractional_origin: None,
            },
            Some(carrier.clone()),
        ))
    }
    fn static_size(
        &self,
        types: &Types<'_>,
        n: usize,
    ) -> Result<u64, TypeError> {
        let (value, suffix) = self.closed_number(types, n, &ValueType::integer("u64").unwrap())?;
        super::numeric::array_length(&self.file, n, value, suffix)
    }
    pub fn index_equations(
        &self,
        types: &mut Types<'a>,
    ) -> Result<(), TypeError> {
        let f = &self.file;
        for (n, node) in f
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, d)| d.kind == NodeKind::Data)
        {
            let owner = self.declaration(n)?;
            for &clause in &node.children {
                if f.nodes[clause].kind != NodeKind::Requires {
                    continue;
                }
                let term = self.single_child(clause)?;
                let predicate = &f.nodes[term];
                if predicate.kind != NodeKind::Binary || !predicate.leaves.iter().any(|l| l.text == "==") {
                    continue;
                }
                let [l, r] = *predicate.children.as_slice() else {
                    continue;
                };
                let binding = |n| match self.symbol(n) {
                    Some(Symbol::Binding {
                        span,
                        kind: NodeKind::TypeParam,
                    }) => match self.parameters.get(span) {
                        Some(ValueType::Parameter { owner: o, index }) if o.as_ref() == owner => Some(*index),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some((i, pattern)) = binding(l)
                    .map(|i| (i, r))
                    .or_else(|| binding(r).map(|i| (i, l)))
                {
                    let pattern = self.annotation(pattern, types)?;
                    types.equations.entry(owner).or_default().push((i, pattern));
                }
            }
        }
        Ok(())
    }
    pub fn index_aliases(
        &self,
        types: &mut Types<'a>,
    ) -> Result<(), TypeError> {
        let f = &self.file;
        for (n, node) in f
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.kind == NodeKind::Data && node.leaves.iter().any(|w| w.text == "="))
        {
            let ty = *node
                .children
                .last()
                .ok_or_else(|| TypeError::unsupported(f, n))?;
            types
                .aliases
                .insert(self.declaration(n)?, self.raw_type(ty, None, types)?);
        }
        Ok(())
    }
    pub fn annotation(
        &self,
        n: usize,
        types: &Types<'_>,
    ) -> Result<ValueType, TypeError> {
        self.annotation_for(n, types, None)
    }
    pub fn annotation_for(
        &self,
        n: usize,
        types: &Types<'_>,
        candidate: Option<&DeclarationId>,
    ) -> Result<ValueType, TypeError> {
        let raw = self.raw_type(n, candidate, types)?;
        if let ValueType::Callable(c) = &raw
            && !c.closed_runtime
        {
            return Err(TypeError::at(&self.file, n, "runtime callable requires explicit reach and crash ceilings"));
        }
        types
            .normalize(raw, &mut HashSet::new())
            .map_err(|reason| TypeError::at(&self.file, n, reason))
    }
    pub(super) fn raw_type(
        &self,
        n: usize,
        candidate: Option<&DeclarationId>,
        types: &Types<'_>,
    ) -> Result<ValueType, TypeError> {
        use NodeKind::*;
        let f = &self.file;
        let node = &f.nodes[n];
        match node.kind {
            FunctionType => {
                let [input, out] = *node.children.as_slice() else {
                    return Err(TypeError::unsupported(f, n));
                };
                Ok(ValueType::Function(
                    Box::new(self.raw_type(input, candidate, types)?),
                    Box::new(self.raw_type(out, candidate, types)?),
                ))
            }
            MachineType => {
                let mut params = Vec::new();
                let mut result = ValueType::NoResult;
                let mut closed = [false; 2];
                for &part in &node.children {
                    match f.nodes[part].kind {
                        Params => {
                            for &p in &f.nodes[part].children {
                                params.push(self.raw_type(self.single_child(p)?, candidate, types)?);
                            }
                        }
                        Result => result = self.raw_type(self.single_child(part)?, candidate, types)?,
                        Reaches => closed[0] = true,
                        Crashes => closed[1] = true,
                        Requires | Ensures | Blocks | Suspends => {}
                        Terminates if f.nodes[part].children.is_empty() => {}
                        _ => return Err(TypeError::unsupported(f, part)),
                    }
                }
                Ok(ValueType::Callable(Box::new(super::CallableType {
                    requirement: None,
                    parameters: params,
                    result,
                    contract: super::Site::at(f, n),
                    closed_runtime: closed == [true, true],
                    pause: super::effects::Envelope::read(self, n)?.declared(),
                    has_guarantee: f.child(n, Ensures).is_some(),
                    reach: super::effects::reach(self, types, n)?,
                    terminates: f.child(n, Terminates).is_some(),
                    crash_ceiling: super::effects::crashes(self, n)?,
                })))
            }
            Lifetime => {
                let Some(Symbol::Binding { span, kind: LifetimeParam }) = self.symbol(n) else {
                    return Err(TypeError::at(f, n, "unresolved lifetime"));
                };
                self.parameters
                    .get(span)
                    .cloned()
                    .ok_or_else(|| TypeError::unsupported(f, n))
            }
            ReferenceType if f.child(n, Lifetime).is_some() => {
                let [region, target] = *node.children.as_slice() else {
                    return Err(TypeError::unsupported(f, n));
                };
                let reference = ValueType::Reference(self.access(n), Box::new(self.raw_type(target, candidate, types)?));
                Ok(ValueType::AtLifetime(Box::new(reference), Box::new(self.raw_type(region, candidate, types)?)))
            }
            QualifiedType => {
                let (base, atoms) = node
                    .children
                    .split_first()
                    .ok_or_else(|| TypeError::unsupported(f, n))?;
                let domains = atoms
                    .iter()
                    .map(|&d| self.domain_application(d, types))
                    .collect::<std::result::Result<_, _>>()?;
                Ok(ValueType::Qualified(Box::new(self.raw_type(*base, candidate, types)?), domains))
            }
            ReferenceType | SliceType => {
                let inner = Box::new(self.raw_type(self.single_child(n)?, candidate, types)?);
                Ok(if node.kind == ReferenceType {
                    ValueType::Reference(self.access(n), inner)
                } else {
                    ValueType::Slice(inner)
                })
            }
            ArrayType => {
                let [element, count] = *node.children.as_slice() else {
                    return Err(TypeError::unsupported(f, n));
                };
                if let Some(Symbol::Binding { span, kind: ConstParam }) = self.symbol(count) {
                    let parameter = self
                        .parameters
                        .get(span)
                        .ok_or_else(|| TypeError::unsupported(f, count))?;
                    if types.parameter_carrier(parameter) != ValueType::integer("u64").as_ref() {
                        return Err(TypeError::at(f, count, "array extent needs a static u64"));
                    }
                    return Ok(ValueType::IndexedArray(
                        Box::new(self.raw_type(element, candidate, types)?),
                        Box::new(parameter.clone()),
                    ));
                }
                if let Some(term) = self.open_index(types, count, &ValueType::integer("u64").unwrap())? {
                    return Ok(ValueType::IndexedArray(Box::new(self.raw_type(element, candidate, types)?), Box::new(term)));
                }
                let size = self.static_size(types, count)?;
                Ok(ValueType::Array(Box::new(self.raw_type(element, candidate, types)?), size))
            }
            PathType | PathExpression => {
                let symbol = match self.symbol(n) {
                    Some(Symbol::ByOverload(options)) => options
                        .iter()
                        .find(|o| Some(&o.candidate) == candidate)
                        .and_then(|o| o.target.as_deref()),
                    other => other,
                };
                if let Some(Symbol::Builtin(name)) = symbol
                    && matches!(name.as_str(), "u" | "i")
                    && node.children.len() == 1
                {
                    let arg = self.single_child(node.children[0])?;
                    if let Some(Symbol::Binding {
                        span,
                        kind: ConstParam | KindedParam,
                    }) = self.symbol(arg)
                    {
                        let width = self
                            .parameters
                            .get(span)
                            .ok_or_else(|| TypeError::unsupported(f, arg))?;
                        if types.parameter_carrier(width) != ValueType::integer("u64").as_ref() {
                            return Err(TypeError::at(f, arg, "integer width requires a static u64"));
                        }
                        return Ok(ValueType::IndexedInteger {
                            signed: name == "i",
                            width: Box::new(width.clone()),
                        });
                    }
                    let required = ValueType::integer("u64").unwrap();
                    if let Some(width) = self.open_index(types, arg, &required)? {
                        return Ok(ValueType::IndexedInteger {
                            signed: name == "i",
                            width: Box::new(width),
                        });
                    }
                    let (number, suffix) = self.closed_number(types, arg, &required)?;
                    if suffix.as_ref().is_some_and(|actual| actual != &required) {
                        return Err(TypeError::at(f, arg, "integer width requires a static u64"));
                    }
                    let width = super::numeric::array_length(f, arg, number, suffix)?;
                    if width == 0 {
                        return Err(TypeError::at(f, arg, "integer width must be positive"));
                    }
                    return Ok(ValueType::Integer {
                        signed: name == "i",
                        width: Some(width),
                    });
                }
                if let Some(Symbol::Declaration(id)) = symbol
                    && id.kind == Data
                {
                    return self.data_application(n, id, types, candidate);
                }
                if let Some(Symbol::Declaration(id)) = symbol
                    && id.kind == Domain
                {
                    let args = node
                        .children
                        .iter()
                        .flat_map(|&a| f.nodes[a].children.iter().copied())
                        .enumerate()
                        .map(|(i, a)| self.raw_argument(a, types, id, i, candidate))
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    if let Some(carrier) = types.domains.known_carrier(id) {
                        return Ok(ValueType::Qualified(
                            Box::new(carrier.clone()),
                            vec![super::DomainApplication {
                                declaration: id.clone(),
                                arguments: args,
                            }],
                        ));
                    }
                    return Ok(ValueType::Applied(Box::new(id.clone()), args));
                }
                let mut args = Vec::new();
                for &section in &node.children {
                    if !matches!(f.nodes[section].kind, TypeArgs | ExpressionArgs) {
                        return Err(TypeError::unsupported(f, section));
                    }
                    for (i, &a) in f.nodes[section].children.iter().enumerate() {
                        args.push(match symbol {
                            Some(Symbol::Declaration(owner)) => self.raw_argument(a, types, owner, i, candidate)?,
                            _ => self.raw_type(a, candidate, types)?,
                        });
                    }
                }
                match symbol {
                    Some(Symbol::Declaration(id)) if id.kind == Data => Ok(if args.is_empty() {
                        ValueType::Nominal(Box::new(id.clone()))
                    } else {
                        ValueType::Applied(Box::new(id.clone()), args)
                    }),
                    Some(Symbol::Binding {
                        span,
                        kind: TypeParam | SomeParam,
                    }) if args.is_empty() => self
                        .parameters
                        .get(span)
                        .cloned()
                        .ok_or_else(|| TypeError::at(f, n, "unknown type parameter")),
                    Some(Symbol::Builtin(name)) if args.is_empty() => match name.as_str() {
                        "addr" => Ok(ValueType::Address),
                        "bool" => Ok(ValueType::Bool),
                        "f32" => Ok(ValueType::Float32),
                        "f64" => Ok(ValueType::Float64),
                        _ if name.starts_with(['u', 'i']) => {
                            ValueType::integer(name).ok_or_else(|| TypeError::at(f, n, format!("invalid integer carrier {name}")))
                        }
                        _ => Err(TypeError::unsupported(f, n)),
                    },
                    _ => Err(TypeError::at(f, n, "expected a type declaration or type parameter")),
                }
            }
            _ => Err(TypeError::unsupported(f, n)),
        }
    }
}
impl Types<'_> {
    pub(super) fn lifetime_slot(
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
    fn solve_arguments(
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

    fn normalize(
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
                    break Ok(super::domains::qualified(base.carrier().clone(), atoms));
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
                let app = super::DomainApplication {
                    declaration: *id,
                    arguments: args,
                };
                let atoms = self.domains.expanded(&app, carrier, true)?;
                break Ok(super::domains::qualified(carrier.clone(), atoms));
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
impl ValueType {
    pub(super) fn validate_closed_indices(&self) -> Result<(), String> {
        use ValueType::*;
        let mut pending = vec![self];
        while let Some(t) = pending.pop() {
            match t {
                StaticInteger { carrier, value }
                    if !(Exact {
                        value: num_rational::BigRational::from_integer(value.clone()),
                        fractional_origin: None,
                    })
                    .fits(carrier) =>
                {
                    return Err(format!("static index {value} exceeds {carrier}"));
                }
                IndexedInteger { width, .. } => {
                    if let StaticInteger { value, .. } = width.as_ref()
                        && value.sign() != num_bigint::Sign::Plus
                    {
                        return Err("integer width must be positive".into());
                    }
                    pending.push(width);
                }
                Applied(_, v) | Tuple(v) => pending.extend(v),
                Array(t, _) | Slice(t) | Reference(_, t) => pending.push(t),
                Function(a, b) | AtLifetime(a, b) | IndexedArray(a, b) => {
                    pending.push(a);
                    pending.push(b);
                }
                Qualified(t, d) => {
                    pending.push(t);
                    for atom in d {
                        pending.extend(&atom.arguments);
                    }
                }
                Callable(c) => {
                    pending.push(&c.result);
                    pending.extend(&c.parameters);
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn static_size(n: u64) -> Self {
        Self::StaticInteger {
            carrier: Box::new(Self::integer("u64").unwrap()),
            value: n.into(),
        }
    }
    pub(super) fn contains_parameter(&self) -> bool {
        match self {
            Self::Parameter { .. } | Self::StaticFormula { .. } => true,
            Self::Array(t, _) | Self::Reference(_, t) | Self::Slice(t) | Self::IndexedInteger { width: t, .. } => t.contains_parameter(),
            Self::Qualified(t, ds) => {
                t.contains_parameter()
                    || ds
                        .iter()
                        .any(|d| d.arguments.iter().any(Self::contains_parameter))
            }
            Self::IndexedArray(a, b) | Self::Function(a, b) => a.contains_parameter() || b.contains_parameter(),
            Self::Tuple(items) => items.iter().any(Self::contains_parameter),
            Self::Applied(_, args) => args.iter().any(Self::contains_parameter),
            Self::AtLifetime(t, r) => t.contains_parameter() || r.contains_parameter(),
            _ => false,
        }
    }
    pub(super) fn nominal_parts(&self) -> Option<(&DeclarationId, &[ValueType])> {
        match self {
            Self::Nominal(id) => Some((id, &[])),
            Self::Applied(id, args) => Some((id, args)),
            _ => None,
        }
    }
    pub(super) fn substitute(
        &self,
        owner: &DeclarationId,
        args: &[ValueType],
    ) -> Self {
        use ValueType::*;
        match self {
            Parameter { owner: b, index } if b.as_ref() == owner => args[*index].clone(),
            Evidence(slot) => Evidence(Box::new(crate::RequirementSlot {
                declaration: slot.declaration.clone(),
                trait_arguments: slot
                    .trait_arguments
                    .iter()
                    .map(|t| t.substitute(owner, args))
                    .collect(),
            })),
            StaticFormula { carrier, term } => term.substitute(owner, args).typed(*carrier.clone()),
            Tuple(items) => Tuple(items.iter().map(|t| t.substitute(owner, args)).collect()),
            IndexedInteger { signed, width } => {
                let width = width.substitute(owner, args);
                if let StaticInteger { value, .. } = &width
                    && let Ok(n) = u64::try_from(value)
                    && n > 0
                {
                    Integer {
                        signed: *signed,
                        width: Some(n),
                    }
                } else {
                    IndexedInteger {
                        signed: *signed,
                        width: Box::new(width),
                    }
                }
            }
            Function(a, b) => Function(Box::new(a.substitute(owner, args)), Box::new(b.substitute(owner, args))),
            Callable(c) => {
                let mut c = c.clone();
                if let Some(row) = &mut c.reach {
                    for service in row {
                        service.arguments = service
                            .arguments
                            .iter()
                            .map(|t| t.substitute(owner, args))
                            .collect();
                    }
                }
                c.parameters = c.parameters.iter().map(|p| p.substitute(owner, args)).collect();
                c.result = c.result.substitute(owner, args);
                if let Some(slot) = &mut c.requirement {
                    slot.trait_arguments = slot
                        .trait_arguments
                        .iter()
                        .map(|t| t.substitute(owner, args))
                        .collect();
                }
                Callable(c)
            }
            AtLifetime(t, l) => AtLifetime(Box::new(t.substitute(owner, args)), Box::new(l.substitute(owner, args))),
            Qualified(t, d) => crate::domains::qualified(t.substitute(owner, args), d.iter().map(|d| d.substitute(owner, args)).collect()),
            IndexedArray(t, index) => {
                let element = Box::new(t.substitute(owner, args));
                let index = index.substitute(owner, args);
                if let StaticInteger { value, .. } = &index
                    && let Ok(n) = u64::try_from(value)
                {
                    Array(element, n)
                } else {
                    IndexedArray(element, Box::new(index))
                }
            }
            Applied(id, inner) => Applied(id.clone(), inner.iter().map(|t| t.substitute(owner, args)).collect()),
            Array(t, n) => Array(Box::new(t.substitute(owner, args)), *n),
            Slice(t) => Slice(Box::new(t.substitute(owner, args))),
            Reference(a, t) => Reference(a.clone(), Box::new(t.substitute(owner, args))),
            other => other.clone(),
        }
    }
    pub(super) fn infer(
        &self,
        actual: &ValueType,
        owner: &DeclarationId,
        args: &mut [Option<ValueType>],
    ) -> bool {
        use ValueType::*;
        match (self, actual) {
            (Parameter { owner: b, index }, actual) if b.as_ref() == owner => match &args[*index] {
                Some(bound) => bound.admits(actual),
                None if matches!(actual, Exact { .. }) => true,
                None => {
                    args[*index] = Some(actual.clone());
                    true
                }
            },
            (Tuple(left), Tuple(right)) => left.len() == right.len() && left.iter().zip(right).all(|(a, b)| a.infer(b, owner, args)),
            (IndexedInteger { signed: a, width }, Integer { signed: b, width: Some(n) }) if a == b => width.infer(&Self::static_size(*n), owner, args),
            (IndexedInteger { signed: a, width: x }, IndexedInteger { signed: b, width: y }) if a == b => x.infer(y, owner, args),
            (IndexedArray(t, index), Array(v, n)) => t.infer(v, owner, args) && index.infer(&Self::static_size(*n), owner, args),
            (IndexedArray(t, index), IndexedArray(v, n)) => t.infer(v, owner, args) && index.infer(n, owner, args),
            (Function(a, b), Function(c, d)) => a.infer(c, owner, args) && b.infer(d, owner, args),
            (AtLifetime(t, l), AtLifetime(v, r)) => t.infer(v, owner, args) && l.infer(r, owner, args),
            (Qualified(t, a), Qualified(v, b)) => {
                a.len() == b.len()
                    && t.infer(v, owner, args)
                    && a.iter().zip(b).all(|(x, y)| {
                        x.declaration == y.declaration
                            && x.arguments.len() == y.arguments.len()
                            && x.arguments
                                .iter()
                                .zip(&y.arguments)
                                .all(|(i, j)| i.infer(j, owner, args))
                    })
            }
            (Applied(id, a), Applied(id2, b)) => id == id2 && a.len() == b.len() && a.iter().zip(b).all(|(t, v)| t.infer(v, owner, args)),
            (Reference(a, t), Reference(b, v)) => a == b && t.infer(v, owner, args),
            (Array(t, n), Array(v, m)) => n == m && t.infer(v, owner, args),
            (Slice(t), Slice(v)) => t.infer(v, owner, args),
            _ => self.admits(actual),
        }
    }
}
pub(super) fn same_operands(
    left: &[(SourceSpan, ValueType)],
    left_owner: &DeclarationId,
    right: &[(SourceSpan, ValueType)],
    right_owner: &DeclarationId,
) -> bool {
    let mut mapping = HashMap::new();
    let mut used = HashSet::new();
    if left.len() != right.len() {
        return false;
    }
    let mut work = left
        .iter()
        .zip(right)
        .map(|((_, l), (_, r))| (l, r))
        .collect::<Vec<_>>();
    while let Some((l, r)) = work.pop() {
        use ValueType::*;
        match (l, r) {
            (Parameter { owner: a, index: i }, Parameter { owner: b, index: j }) if a.as_ref() == left_owner && b.as_ref() == right_owner => {
                if let Some(previous) = mapping.get(i) {
                    if previous != j {
                        return false;
                    }
                } else {
                    if !used.insert(j) {
                        return false;
                    }
                    mapping.insert(*i, *j);
                }
            }
            (AtLifetime(a, l), AtLifetime(b, r)) => {
                work.push((a, b));
                work.push((l, r));
            }
            (Qualified(a, d), Qualified(b, e)) if d.len() == e.len() => {
                work.push((a, b));
                for (l, r) in d.iter().zip(e) {
                    if l.declaration != r.declaration || l.arguments.len() != r.arguments.len() {
                        return false;
                    }
                    work.extend(l.arguments.iter().zip(&r.arguments));
                }
            }
            (StaticFormula { term: a, carrier: x }, StaticFormula { term: b, carrier: y }) if x == y && a.renamed(left_owner, right_owner) == *b => {}
            (Tuple(a), Tuple(b)) if a.len() == b.len() => work.extend(a.iter().zip(b)),
            (IndexedInteger { signed: a, width: x }, IndexedInteger { signed: b, width: y }) if a == b => work.push((x, y)),
            (IndexedArray(a, x), IndexedArray(b, y)) | (Function(a, x), Function(b, y)) => {
                work.push((a, b));
                work.push((x, y));
            }
            (Applied(a, x), Applied(b, y)) if a == b && x.len() == y.len() => work.extend(x.iter().zip(y)),
            (Reference(a, x), Reference(b, y)) if a == b => work.push((x, y)),
            (Array(a, x), Array(b, y)) if x == y => work.push((a, b)),
            (Slice(a), Slice(b)) => work.push((a, b)),
            _ if l == r => {}
            _ => return false,
        }
    }
    true
}
