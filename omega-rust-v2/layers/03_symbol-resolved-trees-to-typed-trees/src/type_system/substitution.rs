use crate::ValueType;
use omega_resolver::DeclarationId;

impl ValueType {
    pub(crate) fn validate_closed_indices(&self) -> Result<(), String> {
        self.components().try_for_each(Self::validate_index)
    }
    pub(super) fn validate_index(&self) -> Result<(), String> {
        use ValueType::*;
        match self {
            StaticFormula {
                term: crate::IntegerIndex::Invalid(message),
                ..
            }
            | InvalidStatic(message) => return Err(message.clone()),
            StaticInteger { carrier, value } if !carrier.contains_integer(value) => {
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
        Ok(())
    }
    fn concrete_size(&self) -> Option<u64> {
        let Self::StaticInteger { value, .. } = self else { return None };
        u64::try_from(value).ok()
    }
    pub(super) fn with_width(
        signed: bool,
        width: Self,
    ) -> Self {
        match width.concrete_size() {
            Some(n) if n > 0 => Self::Integer { signed, width: Some(n) },
            _ => Self::IndexedInteger {
                signed,
                width: Box::new(width),
            },
        }
    }
    pub(super) fn array_with_extent(
        element: Self,
        extent: Self,
    ) -> Self {
        match extent.concrete_size() {
            Some(n) => Self::Array(Box::new(element), n),
            None => Self::IndexedArray(Box::new(element), Box::new(extent)),
        }
    }
    pub(crate) fn static_size(n: u64) -> Self {
        Self::StaticInteger {
            carrier: Box::new(Self::integer("u64").unwrap()),
            value: n.into(),
        }
    }
    pub(crate) fn contains_parameter(&self) -> bool {
        self.components().any(|ty| {
            matches!(
                ty,
                Self::Parameter { .. } | Self::StaticFormula { .. } | Self::PendingStatic(_) | Self::OpenStatic { .. }
            )
        })
    }
    pub(crate) fn depends_on_unsupplied_parameter(
        &self,
        declaration: &DeclarationId,
        supplied: usize,
    ) -> bool {
        self.components().any(|part| match part {
            Self::Parameter { owner, index } => owner.as_ref() == declaration && *index >= supplied,
            Self::StaticFormula { term, .. } => term.depends_on_unsupplied_parameter(declaration, supplied),
            _ => false,
        })
    }
    pub(crate) fn depends_on_call_input(&self) -> bool {
        self.components().any(|ty| {
            matches!(
                ty,
                Self::Parameter { .. } | Self::StaticFormula { .. } | Self::PendingStatic(_) | Self::OpenStatic { .. } | Self::Subject { .. }
            )
        })
    }
    /// Structural queries use the same children. Binder-sensitive transformations
    /// still control how they enter a binder and what they replace.
    pub(crate) fn components(&self) -> impl Iterator<Item = &Self> {
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
                | StaticFormula { carrier: t, .. }
                | StaticValue { carrier: t, .. } => pending.push(t.as_ref()),
                PendingStatic(demand) => pending.push(&demand.request().carrier),
                OpenStatic { arguments, .. } => pending.extend(arguments),
                Function(a, b) | AtLifetime(a, b) | IndexedArray(a, b) => {
                    pending.push(a.as_ref());
                    pending.push(b.as_ref());
                }
                MathematicalValue { subjects, .. } | Tuple(subjects) | Applied(_, subjects) | MachineSelection(_, subjects) => pending.extend(subjects),
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
                | InferredLifetime { .. }
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
                | LogicalRequirement(_)
                | ProofCitation(_)
                | ProofTerm(_)
                | Implementation(_)
                | Parameter { .. }
                | InvalidStatic(_)
                | NoResult => {}
            }
            Some(ty)
        })
    }
    pub(crate) fn record_parts(&self) -> Option<(&DeclarationId, &[ValueType])> {
        match self {
            Self::Evidence(slot) => Some((&slot.declaration, &slot.trait_arguments)),
            ty => ty.nominal_parts(),
        }
    }
    pub(crate) fn nominal_parts(&self) -> Option<(&DeclarationId, &[ValueType])> {
        match self {
            Self::Nominal(id) => Some((id, &[])),
            Self::Applied(id, args) => Some((id, args)),
            _ => None,
        }
    }
    /// Rewrite a type structurally, tracking the number of dependent function binders
    /// around each component. A replacement owns its subtree.
    pub(crate) fn rewrite(
        &self,
        depth: usize,
        replace: &impl Fn(&Self, usize) -> Option<Self>,
    ) -> Self {
        self.clone().rewrite_owned(depth, replace)
    }

    fn rewrite_owned(
        self,
        depth: usize,
        replace: &impl Fn(&Self, usize) -> Option<Self>,
    ) -> Self {
        if let Some(ty) = replace(&self, depth) {
            return ty;
        }
        let Ok(ty) = self.try_map_children(depth, |child, depth| Ok::<_, std::convert::Infallible>(child.rewrite_owned(depth, replace)));
        ty
    }

    /// Map immediate children, entering a dependent binder only at its codomain.
    /// Domain argument substitution may merge or reorder qualified atoms.
    pub(super) fn try_map_children<E>(
        self,
        depth: usize,
        mut map: impl FnMut(Self, usize) -> Result<Self, E>,
    ) -> Result<Self, E> {
        use ValueType::*;
        let mut part = |ty| map(ty, depth);
        Ok(match self {
            Function(a, b) => Function(Box::new(part(*a)?), Box::new(map(*b, depth + 1)?)),
            MathematicalValue { definition, subjects } => MathematicalValue {
                definition,
                subjects: subjects.into_iter().map(part).collect::<Result<_, _>>()?,
            },
            FamilyApplication { family, subjects } => FamilyApplication {
                family: Box::new(part(*family)?),
                subjects: subjects.into_iter().map(part).collect::<Result<_, _>>()?,
            },
            Applied(id, args) => Applied(id, args.into_iter().map(part).collect::<Result<_, _>>()?),
            MachineSelection(id, args) => MachineSelection(id, args.into_iter().map(part).collect::<Result<_, _>>()?),
            Qualified(t, mut domains) => {
                let carrier = part(*t)?;
                for domain in &mut domains {
                    domain.arguments = std::mem::take(&mut domain.arguments)
                        .into_iter()
                        .map(&mut part)
                        .collect::<Result<_, _>>()?;
                }
                crate::type_system::domains::qualified(carrier, domains)
            }
            Tuple(ts) => Tuple(ts.into_iter().map(part).collect::<Result<_, _>>()?),
            Reference(a, t) => Reference(a, Box::new(part(*t)?)),
            Array(t, n) => Array(Box::new(part(*t)?), n),
            IndexedArray(t, n) => Self::array_with_extent(part(*t)?, part(*n)?),
            IndexedInteger { signed, width } => Self::with_width(signed, part(*width)?),
            Squash(t) => Squash(Box::new(part(*t)?)),
            LevelOf(t) => LevelOf(Box::new(part(*t)?)),
            Slice(t) => Slice(Box::new(part(*t)?)),
            AtLifetime(t, l) => AtLifetime(Box::new(part(*t)?), Box::new(part(*l)?)),
            Callable(mut c) => {
                c.parameters = c
                    .parameters
                    .into_iter()
                    .map(&mut part)
                    .collect::<Result<_, _>>()?;
                c.result = part(c.result)?;
                for service in c.reach.iter_mut().flatten() {
                    service.arguments = std::mem::take(&mut service.arguments)
                        .into_iter()
                        .map(&mut part)
                        .collect::<Result<_, _>>()?;
                }
                if let Some(slot) = &mut c.requirement {
                    slot.trait_arguments = std::mem::take(&mut slot.trait_arguments)
                        .into_iter()
                        .map(part)
                        .collect::<Result<_, _>>()?;
                }
                Callable(c)
            }
            Evidence(mut slot) => {
                slot.trait_arguments = slot
                    .trait_arguments
                    .into_iter()
                    .map(part)
                    .collect::<Result<_, _>>()?;
                Evidence(slot)
            }
            Universe { strict, level } => Universe {
                strict,
                level: Box::new(part(*level)?),
            },
            StaticInteger { carrier, value } => StaticInteger {
                carrier: Box::new(part(*carrier)?),
                value,
            },
            StaticValue { carrier, value } => StaticValue {
                carrier: Box::new(part(*carrier)?),
                value,
            },
            OpenStatic { template, arguments } => template.apply_value(arguments.into_iter().map(part).collect::<Result<_, _>>()?),
            StaticFormula { carrier, term } => StaticFormula {
                carrier: Box::new(part(*carrier)?),
                term,
            },
            other => other,
        })
    }
    pub(crate) fn substitute(
        &self,
        owner: &DeclarationId,
        args: &[ValueType],
    ) -> Self {
        use ValueType::*;
        self.rewrite(0, &|ty, _| {
            Some(match ty {
                Parameter { owner: b, index } if b.as_ref() == owner => args[*index].clone(),
                StaticFormula { carrier, term } => term
                    .substitute(owner, args)
                    .typed(carrier.substitute(owner, args)),
                OpenStatic { template, arguments } => template.apply_value(arguments.iter().map(|t| t.substitute(owner, args)).collect()),
                _ => return None,
            })
        })
    }
    pub(crate) fn infer(
        &self,
        actual: &ValueType,
        owner: &DeclarationId,
        args: &mut [Option<ValueType>],
    ) -> bool {
        self.infer_with(actual, owner, args, &mut |_, _| false)
    }

    pub(crate) fn infer_with(
        &self,
        actual: &ValueType,
        owner: &DeclarationId,
        args: &mut [Option<ValueType>],
        compare_pending: &mut impl FnMut(&ValueType, &ValueType) -> bool,
    ) -> bool {
        use ValueType::*;
        match (self, actual) {
            (Parameter { owner: b, index }, actual) if b.as_ref() == owner => match &args[*index] {
                Some(bound) => bound.admits(actual) || compare_pending(bound, actual),
                None if matches!(actual, Exact { .. }) => true,
                None => {
                    args[*index] = Some(actual.clone());
                    true
                }
            },
            (Universe { strict: a, level: x }, Universe { strict: b, level: y }) => a == b && x.infer_with(y, owner, args, compare_pending),
            (Tuple(left), Tuple(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|(a, b)| a.infer_with(b, owner, args, compare_pending))
            }
            (IndexedInteger { signed: a, width }, Integer { signed: b, width: Some(n) }) if a == b => {
                width.infer_with(&Self::static_size(*n), owner, args, compare_pending)
            }
            (IndexedInteger { signed: a, width: x }, IndexedInteger { signed: b, width: y }) if a == b => x.infer_with(y, owner, args, compare_pending),
            (IndexedArray(t, index), Array(v, n)) => {
                t.infer_with(v, owner, args, compare_pending) && index.infer_with(&Self::static_size(*n), owner, args, compare_pending)
            }
            (IndexedArray(t, index), IndexedArray(v, n)) => t.infer_with(v, owner, args, compare_pending) && index.infer_with(n, owner, args, compare_pending),
            (Function(a, b), Function(c, d)) => a.infer_with(c, owner, args, compare_pending) && b.infer_with(d, owner, args, compare_pending),
            (AtLifetime(t, l), AtLifetime(v, r)) => t.infer_with(v, owner, args, compare_pending) && l.infer_with(r, owner, args, compare_pending),
            (Qualified(t, a), Qualified(v, b)) => {
                a.len() == b.len()
                    && t.infer_with(v, owner, args, compare_pending)
                    && a.iter().zip(b).all(|(x, y)| {
                        x.declaration == y.declaration
                            && x.arguments.len() == y.arguments.len()
                            && x.arguments
                                .iter()
                                .zip(&y.arguments)
                                .all(|(i, j)| i.infer_with(j, owner, args, compare_pending))
                    })
            }
            (Applied(id, a), Applied(id2, b)) => {
                id == id2
                    && a.len() == b.len()
                    && a.iter()
                        .zip(b)
                        .all(|(t, v)| t.infer_with(v, owner, args, compare_pending))
            }
            (Reference(a, t), Reference(b, v)) => a == b && t.infer_with(v, owner, args, compare_pending),
            (Array(t, n), Array(v, m)) => n == m && t.infer_with(v, owner, args, compare_pending),
            (Slice(t), Slice(v)) => t.infer_with(v, owner, args, compare_pending),
            _ => self.admits(actual) || compare_pending(self, actual),
        }
    }
}

/// Pair each field component with the value that supplies its type.
pub(crate) fn field_components<'a>(
    ty: &'a ValueType,
    actual: &'a ValueType,
) -> impl Iterator<Item = (&'a ValueType, &'a ValueType)> {
    use ValueType::*;
    let mut pending = vec![(ty, actual)];
    std::iter::from_fn(move || {
        let (template, value) = pending.pop()?;
        match (template, value) {
            (Applied(a, x), Applied(b, y)) if a == b && x.len() == y.len() => pending.extend(x.iter().zip(y)),
            (Qualified(x, left), Qualified(y, right)) => {
                pending.push((x, y));
                for domain in left {
                    let mut matches = right
                        .iter()
                        .filter(|candidate| candidate.declaration == domain.declaration);
                    if let (Some(actual), None) = (matches.next(), matches.next()) {
                        pending.extend(domain.arguments.iter().zip(&actual.arguments));
                    }
                }
            }
            (IndexedArray(x, n), IndexedArray(y, m)) if n == m => pending.push((x, y)),
            (Function(x, a), Function(y, b)) => {
                pending.push((x, y));
                pending.push((a, b));
            }
            (Reference(a, x), Reference(b, y)) if a == b => pending.push((x, y)),
            (AtLifetime(x, _), AtLifetime(y, _)) => pending.push((x, y)),
            (AtLifetime(x, _), Reference(..)) => pending.push((x, value)),
            (Tuple(xs), Tuple(ys)) if xs.len() == ys.len() => pending.extend(xs.iter().zip(ys)),
            (Array(x, a), Array(y, b)) if a == b => pending.push((x, y)),
            (Slice(x), Slice(y)) => pending.push((x, y)),
            (Callable(x), Callable(y)) => {
                pending.extend(x.parameters.iter().zip(&y.parameters));
                pending.push((&x.result, &y.result));
            }
            _ => {}
        }
        Some((template, value))
    })
}
