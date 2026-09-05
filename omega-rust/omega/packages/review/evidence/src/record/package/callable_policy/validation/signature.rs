//! Structural signature scope; no compiler replay or canonical-type parsing.

use super::{
    signature_contracts as contracts, signature_expressions as expressions,
    signature_parameters as parameters,
};
use crate::record::*;

pub(super) type Result = std::result::Result<(), &'static str>;

#[derive(Clone, Copy)]
pub(super) struct Scope<'a> {
    pub outer: Option<&'a Scope<'a>>,
    pub statics: &'a [PackageReviewTypeParameter],
    pub static_offset: usize,
    pub lifetimes: usize,
    pub parameters: usize,
    pub nonself_parameters: usize,
    pub has_self: bool,
    pub result: bool,
}
impl Scope<'_> {
    pub fn static_kind(&self, ordinal: u32) -> Option<&PackageReviewTypeParameterKind> {
        let ordinal = ordinal as usize;
        if ordinal < self.static_offset {
            self.outer?.static_kind(ordinal as u32)
        } else {
            self.statics
                .get(ordinal - self.static_offset)
                .map(|parameter| &parameter.kind)
        }
    }
    pub fn static_count(&self) -> usize {
        self.static_offset + self.statics.len()
    }
}

fn scope(callable: &PackagePolicyCallable) -> Scope<'_> {
    Scope {
        outer: None,
        statics: &callable.type_parameters,
        static_offset: 0,
        lifetimes: callable.lifetime_parameter_count,
        parameters: callable.parameters.len(),
        nonself_parameters: callable
            .parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count(),
        has_self: callable
            .parameters
            .iter()
            .any(|parameter| parameter.is_self),
        result: callable.return_type.is_some(),
    }
}

pub(super) fn validate(callable: &PackagePolicyCallable) -> Result {
    let scope = scope(callable);
    u32::try_from(scope.lifetimes).map_err(|_| "callable lifetime count exceeds portable width")?;
    for (index, parameter) in callable.parameters.iter().enumerate() {
        text(&parameter.name)?;
        value_type(&parameter.type_identity)?;
        if callable.parameters[..index]
            .iter()
            .any(|prior| prior.name == parameter.name || (prior.is_self && parameter.is_self))
        {
            return Err("callable signature repeats a formal name or receiver");
        }
    }
    if let Some(result) = &callable.return_type {
        value_type(result)?;
    }
    parameters::parameters(&scope, 0)?;
    let mut next_evidence = 0;
    for bound in &callable.conformance_bounds {
        if let Some(ordinal) = bound.binder_ordinal {
            if ordinal != next_evidence {
                return Err("conformance evidence binders are not declaration ordered");
            }
            next_evidence += 1;
        }
        if !matches!(
            scope.static_kind(bound.subject_parameter),
            Some(PackageReviewTypeParameterKind::Type)
        ) {
            return Err("conformance bound subject is not a declared type parameter");
        }
        nominal(&bound.trait_identity)?;
        lifetimes(&bound.trait_lifetime_arguments, &scope)?;
        for argument in &bound.arguments {
            value_type(argument)?;
        }
        match (&bound.selected_conformance, &bound.selected_subject) {
            (Some(selected), Some(subject)) => {
                nominal(selected)?;
                lifetimes(&bound.selected_lifetime_arguments, &scope)?;
                expressions::static_argument(subject, &scope, 0)?;
                for argument in &bound.selected_arguments {
                    expressions::static_argument(argument, &scope, 0)?;
                }
            }
            (None, None)
                if bound.selected_arguments.is_empty()
                    && bound.selected_lifetime_arguments.is_empty() => {}
            _ => return Err("conformance selection has incomplete structural coordinates"),
        }
    }
    for contract in &callable.contracts {
        contracts::contract(contract, &scope, 0)?;
    }
    for conformance in &callable.conformances {
        owned_pair(
            &conformance.trait_identity,
            &conformance.requirement_identity,
        )?;
        if let Some(alias) = &conformance.alias {
            text(alias)?;
        }
        for argument in &conformance.arguments {
            value_type(argument)?;
        }
        lifetimes(&conformance.trait_lifetime_arguments, &scope)?;
        if conformance.requirement_lifetime_partition.len()
            != conformance.trait_lifetime_arguments.len()
        {
            return Err("conformance lifetime partition loses an actual selected lifetime");
        }
        let mut next = 0;
        for (index, argument) in conformance.trait_lifetime_arguments.iter().enumerate() {
            let expected = if let Some(prior) = conformance.trait_lifetime_arguments[..index]
                .iter()
                .position(|prior| prior == argument)
            {
                conformance.requirement_lifetime_partition[prior]
            } else {
                let ordinal = next;
                next += 1;
                ordinal
            };
            if conformance.requirement_lifetime_partition[index] != expected {
                return Err(
                    "requirement lifetime partition differs from its actual selected lifetimes",
                );
            }
        }
    }
    for realization in &callable.operator_realizations {
        operator(&realization.coordinate)?;
        if let Some(alias) = &realization.alias {
            text(alias)?;
        }
    }
    Ok(())
}

pub(super) fn expression(
    value: &PackageReviewContractExpression,
    callable: &PackagePolicyCallable,
) -> Result {
    let mut scope = scope(callable);
    scope.result = false;
    expressions::expression(value, &scope, 0)
}

pub(super) fn text(value: &str) -> Result {
    if value.is_empty() {
        Err("empty signature identity or name")
    } else {
        Ok(())
    }
}
pub(super) fn value_type(value: &PackageReviewTypeIdentity) -> Result {
    text(&value.canonical)
}
pub(super) fn nominal(value: &PackageReviewNominalIdentity) -> Result {
    text(&value.path)?;
    if value.owner == PackageReviewNominalOwner::Unresolved {
        Err("signature nominal has no exact owner")
    } else {
        Ok(())
    }
}
pub(super) fn owned_pair(
    owner: &PackageReviewNominalIdentity,
    member: &PackageReviewNominalIdentity,
) -> Result {
    nominal(owner)?;
    nominal(member)?;
    if owner.owner != member.owner {
        Err("signature member changes its declaring package owner")
    } else {
        Ok(())
    }
}
pub(super) fn operator(value: &PackageReviewOperatorCoordinate) -> Result {
    nominal(&value.identity)?;
    // Empty result dispatch is exact meaning for operand-directed operators.
    text(&value.parameter_dispatch)
}
pub(super) fn depth(depth: usize) -> Result {
    if depth >= 128 {
        Err("signature structure exceeds bounded nesting")
    } else {
        Ok(())
    }
}
pub(super) fn lifetimes(ordinals: &[u32], scope: &Scope<'_>) -> Result {
    if ordinals
        .iter()
        .any(|ordinal| *ordinal as usize >= scope.lifetimes)
    {
        Err("signature lifetime argument escapes its containing telescope")
    } else {
        Ok(())
    }
}
