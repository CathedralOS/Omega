//! Structural signature scope; no compiler replay or canonical-type parsing.

use super::{
    signature_contracts as contracts, signature_expressions as expressions,
    signature_parameters as parameters,
};
use crate::record::{
    PackagePolicyCallable, PackagePolicyTypeParameter, PackagePolicyTypeParameterKind,
    PackageReviewContractExpression, PackageReviewNominalIdentity, PackageReviewOperatorCoordinate,
    PackageReviewPropositionBinder, PackageReviewPropositionBinderKind,
    PackageReviewPropositionParameterSignature, PackageReviewTypeIdentity,
    PackageReviewTypeParameter, PackageReviewTypeParameterKind,
};

pub(in crate::record) type Result = std::result::Result<(), &'static str>;

#[derive(Clone, Copy)]
pub(in crate::record) enum BinderKind<'a> {
    Type,
    Implementation,
    Const,
    Machine,
    Proposition(&'a PackageReviewPropositionParameterSignature),
}

#[derive(Clone, Copy)]
pub(in crate::record) struct Scope<'a> {
    pub outer: Option<&'a Scope<'a>>,
    pub statics: &'a [PackageReviewTypeParameter],
    pub policy_statics: &'a [PackagePolicyTypeParameter],
    pub proposition_binders: &'a [PackageReviewPropositionBinder],
    pub static_offset: usize,
    pub lifetimes: usize,
    pub parameters: usize,
    pub result: bool,
    pub domain_subject: bool,
}
impl Scope<'_> {
    pub(crate) fn static_kind(&self, ordinal: u32) -> Option<BinderKind<'_>> {
        let ordinal = ordinal as usize;
        if ordinal < self.static_offset {
            self.outer?.static_kind(ordinal as u32)
        } else {
            let local = ordinal - self.static_offset;
            if !self.policy_statics.is_empty() {
                return self
                    .policy_statics
                    .get(local)
                    .map(|parameter| match &parameter.kind {
                        PackagePolicyTypeParameterKind::Type => BinderKind::Type,
                        PackagePolicyTypeParameterKind::Implementation => {
                            BinderKind::Implementation
                        }
                        PackagePolicyTypeParameterKind::Const(_)
                        | PackagePolicyTypeParameterKind::Value(_) => BinderKind::Const,
                        PackagePolicyTypeParameterKind::Machine(_) => BinderKind::Machine,
                        PackagePolicyTypeParameterKind::Proposition(signature) => {
                            BinderKind::Proposition(signature)
                        }
                    });
            }
            if !self.proposition_binders.is_empty() {
                return self
                    .proposition_binders
                    .get(local)
                    .map(|binder| match &binder.kind {
                        PackageReviewPropositionBinderKind::Type => BinderKind::Type,
                        PackageReviewPropositionBinderKind::Const(_) => BinderKind::Const,
                        PackageReviewPropositionBinderKind::Machine => BinderKind::Machine,
                    });
            }
            self.statics
                .get(local)
                .map(|parameter| match &parameter.kind {
                    PackageReviewTypeParameterKind::Type => BinderKind::Type,
                    PackageReviewTypeParameterKind::Implementation => BinderKind::Implementation,
                    PackageReviewTypeParameterKind::Const(_)
                    | PackageReviewTypeParameterKind::Value(_) => BinderKind::Const,
                    PackageReviewTypeParameterKind::Machine(_) => BinderKind::Machine,
                    PackageReviewTypeParameterKind::Proposition(signature) => {
                        BinderKind::Proposition(signature)
                    }
                })
        }
    }
    pub fn static_count(&self) -> usize {
        self.static_offset
            + self.statics.len()
            + self.policy_statics.len()
            + self.proposition_binders.len()
    }
}

fn scope(callable: &PackagePolicyCallable) -> Scope<'_> {
    Scope {
        outer: None,
        statics: &[],
        policy_statics: &callable.type_parameters,
        proposition_binders: &[],
        static_offset: 0,
        lifetimes: callable.lifetime_parameter_count,
        parameters: callable.parameters.len(),
        result: callable.return_type.is_some(),
        domain_subject: false,
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
            .any(|prior| prior.name == parameter.name)
        {
            return Err("callable signature repeats a formal name");
        }
    }
    if let Some(result) = &callable.return_type {
        value_type(result)?;
    }
    parameters::parameters(&scope, 0)?;
    let mut next_evidence = 0;
    for bound in &callable.conformance_bounds {
        if bound.binder_ordinal != next_evidence {
            return Err("conformance evidence binders are not declaration ordered");
        }
        next_evidence += 1;
        bound.trait_identity.validate_resolved()?;
        lifetimes(&bound.trait_lifetime_arguments, &scope)?;
        for argument in &bound.arguments {
            value_type(argument)?;
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
    for realization in &callable.requirement_realizations {
        realization.requirement_identity.validate_resolved()?;
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

pub(in crate::record) fn text(value: &str) -> Result {
    if value.is_empty() {
        Err("empty signature identity or name")
    } else {
        Ok(())
    }
}
pub(in crate::record) fn value_type(value: &PackageReviewTypeIdentity) -> Result {
    text(&value.canonical)
}
pub(in crate::record) fn owned_pair(
    owner: &PackageReviewNominalIdentity,
    member: &PackageReviewNominalIdentity,
) -> Result {
    owner.validate_resolved()?;
    member.validate_resolved()?;
    if owner.owner != member.owner {
        Err("signature member changes its declaring package owner")
    } else {
        Ok(())
    }
}
pub(in crate::record) fn operator(value: &PackageReviewOperatorCoordinate) -> Result {
    value.identity.validate_resolved()?;
    // Empty result dispatch is exact meaning for operand-directed operators.
    text(&value.parameter_dispatch)
}
pub(in crate::record) fn depth(depth: usize) -> Result {
    // Zero-based depth of recursive wire owners: machine contracts,
    // contract expressions, and static arguments. Nonrecursive signature,
    // fact, proposition, and behavior wrappers do not consume another level.
    // This agrees with Reader::nested and the policy writer at the frontier.
    if depth >= 128 {
        Err("signature structure exceeds bounded nesting")
    } else {
        Ok(())
    }
}
pub(in crate::record) fn lifetimes(ordinals: &[u32], scope: &Scope<'_>) -> Result {
    if ordinals
        .iter()
        .any(|ordinal| *ordinal as usize >= scope.lifetimes)
    {
        Err("signature lifetime argument escapes its containing telescope")
    } else {
        Ok(())
    }
}
