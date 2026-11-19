use crate::ValueType;
use crate::type_system::domains;
use omega_resolver::DeclarationId;

impl ValueType {
    pub(crate) fn validate_closed_indices(&self) -> Result<(), String> {
        use ValueType::*;
        for t in self.components() {
            match t {
                StaticFormula {
                    term: crate::IntegerIndex::Invalid(message),
                    ..
                } => return Err(message.clone()),
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
                }
                _ => {}
            }
        }
        Ok(())
    }
    pub(crate) fn static_size(n: u64) -> Self {
        Self::StaticInteger {
            carrier: Box::new(Self::integer("u64").unwrap()),
            value: n.into(),
        }
    }
    pub(crate) fn contains_parameter(&self) -> bool {
        self.components()
            .any(|ty| matches!(ty, Self::Parameter { .. } | Self::StaticFormula { .. }))
    }
    pub(crate) fn depends_on_call_input(&self) -> bool {
        self.components()
            .any(|ty| matches!(ty, Self::Parameter { .. } | Self::StaticFormula { .. } | Self::Subject { .. }))
    }
    /// Structural queries use the same children. Binder-sensitive transformations
    /// still control how they enter a binder and what they replace.
    fn components(&self) -> impl Iterator<Item = &Self> {
        let mut pending = Vec::new();
        let mut root = Some(self);
        std::iter::from_fn(move || {
            let ty = root.take().or_else(|| pending.pop())?;
            use ValueType::*;
            match ty {
                Squash(t)
                | LevelOf(t)
                | Universe { level: t, .. }
                | IndexedInteger { width: t, .. }
                | Reference(_, t)
                | Array(t, _)
                | Slice(t)
                | StaticInteger { carrier: t, .. }
                | StaticFormula { carrier: t, .. } => pending.push(t.as_ref()),
                Function(a, b) | AtLifetime(a, b) | IndexedArray(a, b) => {
                    pending.push(a.as_ref());
                    pending.push(b.as_ref());
                }
                MathematicalValue { subjects, .. } | Tuple(subjects) | Applied(_, subjects) => pending.extend(subjects),
                FamilyApplication { family, subjects } => {
                    pending.push(family.as_ref());
                    pending.extend(subjects);
                }
                Qualified(carrier, domains) => {
                    pending.push(carrier.as_ref());
                    pending.extend(domains.iter().flat_map(|domain| &domain.arguments));
                }
                Evidence(slot) => pending.extend(&slot.trait_arguments),
                Callable(callable) => {
                    if let Some(slot) = &callable.requirement {
                        pending.extend(&slot.trait_arguments);
                    }
                    if let Some(reach) = &callable.reach {
                        pending.extend(reach.iter().flat_map(|service| &service.arguments));
                    }
                    pending.push(&callable.result);
                    pending.extend(&callable.parameters);
                }
                Bool
                | LevelKind
                | Subject { .. }
                | BoundSubject(_)
                | MathematicalDefinition(_)
                | Level(_)
                | Integer { .. }
                | Address
                | Float32
                | Float64
                | Nominal(_)
                | Exact { .. }
                | Witness { .. }
                | MachineSelection(_)
                | LogicalRequirement(_)
                | ProofCitation(_)
                | ProofTerm(_)
                | Implementation(_)
                | Parameter { .. }
                | NoResult => {}
            }
            Some(ty)
        })
    }
    pub(crate) fn nominal_parts(&self) -> Option<(&DeclarationId, &[ValueType])> {
        match self {
            Self::Nominal(id) => Some((id, &[])),
            Self::Applied(id, args) => Some((id, args)),
            _ => None,
        }
    }
    pub(crate) fn substitute(
        &self,
        owner: &DeclarationId,
        args: &[ValueType],
    ) -> Self {
        use ValueType::*;
        match self {
            MathematicalValue { definition, subjects } => MathematicalValue {
                definition: definition.clone(),
                subjects: subjects.iter().map(|s| s.substitute(owner, args)).collect(),
            },
            FamilyApplication { family, subjects } => FamilyApplication {
                family: Box::new(family.substitute(owner, args)),
                subjects: subjects.iter().map(|s| s.substitute(owner, args)).collect(),
            },
            Squash(t) => Squash(Box::new(t.substitute(owner, args))),
            LevelOf(t) => LevelOf(Box::new(t.substitute(owner, args))),
            Universe { strict, level } => Universe {
                strict: *strict,
                level: Box::new(level.substitute(owner, args)),
            },
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
            Qualified(t, d) => domains::qualified(t.substitute(owner, args), d.iter().map(|d| d.substitute(owner, args)).collect()),
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
    pub(crate) fn infer(
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
            (Universe { strict: a, level: x }, Universe { strict: b, level: y }) => a == b && x.infer(y, owner, args),
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
