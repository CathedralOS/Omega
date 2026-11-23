use crate::{Site, ValueType};
use omega_resolved::DeclarationId;

impl ValueType {
    fn concrete_size(&self) -> Option<u64> {
        let Self::StaticInteger { value, .. } = self else { return None };
        u64::try_from(value).ok()
    }
    pub fn with_width(
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
    pub fn array_with_extent(
        element: Self,
        extent: Self,
    ) -> Self {
        match extent.concrete_size() {
            Some(n) => Self::Array(Box::new(element), n),
            None => Self::IndexedArray(Box::new(element), Box::new(extent)),
        }
    }
    pub fn static_size(n: u64) -> Self {
        Self::StaticInteger {
            carrier: Box::new(Self::Integer {
                signed: false,
                width: Some(64),
            }),
            value: n.into(),
        }
    }
    pub fn contains_parameter(&self) -> bool {
        self.components().any(|ty| {
            matches!(
                ty,
                Self::Parameter { .. } | Self::StaticFormula { .. } | Self::PendingStatic(_) | Self::OpenStatic { .. }
            )
        })
    }
    pub fn depends_on_unsupplied_parameter(
        &self,
        declaration: &DeclarationId,
        bindings: &[Option<ValueType>],
    ) -> bool {
        self.components().any(|part| match part {
            Self::Parameter { owner, index } => owner.as_ref() == declaration && bindings.get(*index).is_none_or(Option::is_none),
            Self::StaticFormula { term, .. } => term.depends_on_unsupplied_parameter(declaration, bindings),
            _ => false,
        })
    }
    /// Whether this type or index still depends on a generic parameter or a
    /// runtime argument subject.
    pub fn depends_on_call_input(&self) -> bool {
        self.components().any(|ty| {
            matches!(
                ty,
                Self::Parameter { .. } | Self::StaticFormula { .. } | Self::PendingStatic(_) | Self::OpenStatic { .. } | Self::Subject { .. }
            )
        })
    }
    /// Structural queries use the same children. Binder-sensitive transformations
    /// still control how they enter a binder and what they replace.
    pub fn components(&self) -> impl Iterator<Item = &Self> {
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
                MathematicalValue { subjects, .. } | Tuple(subjects) | Applied(_, subjects) => pending.extend(subjects),
                MachineSelection(selection) => pending.extend(selection.all_arguments()),
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
    pub fn record_parts(&self) -> Option<(&DeclarationId, &[ValueType])> {
        match self {
            Self::Evidence(slot) => Some((&slot.declaration, &slot.trait_arguments)),
            ty => ty.nominal_parts(),
        }
    }
    pub fn nominal_parts(&self) -> Option<(&DeclarationId, &[ValueType])> {
        match self {
            Self::Nominal(id) => Some((id, &[])),
            Self::Applied(id, args) => Some((id, args)),
            _ => None,
        }
    }
    /// Rewrite a type structurally, tracking the number of dependent function binders
    /// around each component. A replacement owns its subtree.
    pub fn rewrite(
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
    pub fn try_map_children<E>(
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
            MachineSelection(selection) => MachineSelection(selection.try_map_arguments(part)?),
            Qualified(t, mut domains) => {
                let carrier = part(*t)?;
                for domain in &mut domains {
                    domain.arguments = std::mem::take(&mut domain.arguments)
                        .into_iter()
                        .map(&mut part)
                        .collect::<Result<_, _>>()?;
                }
                super::domain_application::qualified(carrier, domains)
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
    /// Apply the ordered generic arguments of one exact declaration.
    pub fn substitute(
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
}

impl ValueType {
    pub fn replace_subjects<'a>(
        &self,
        resolve: impl Fn(&Site) -> Option<&'a ValueType>,
    ) -> Self {
        self.rewrite(0, &|ty, _| match ty {
            Self::Subject { declaration, version: None } => resolve(declaration).cloned(),
            _ => None,
        })
    }
}
