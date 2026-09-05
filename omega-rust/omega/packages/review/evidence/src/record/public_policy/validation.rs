//! Canonical public meaning and explicit binder scopes, never compiler proof.

mod behavior;
mod data;
mod declarations;
pub(in crate::record) mod signatures;
#[cfg(test)]
mod tests;

use crate::record::package::callable_policy::validation::{
    signature::{self as shared, *},
    signature_contracts as contracts, signature_expressions as expressions,
};
use crate::record::*;
use psi_core::PackageKeyIdentity;

pub(in crate::record) fn validate_conformance_shape(
    value: &PackagePolicyConformanceShape,
) -> Result {
    declarations::conformance_scope(value)
}

impl PackagePolicyPublicApi {
    pub(crate) fn validate_canonical_structure(&self, package: PackageKeyIdentity) -> Result {
        roots(&self.traits, package, |value| &value.identity)?;
        roots(&self.conformances, package, |value| &value.identity)?;
        roots(&self.domains, package, |value| &value.identity)?;
        roots(&self.propositions, package, |value| &value.identity)?;
        roots(&self.consts, package, |value| &value.identity)?;
        roots(&self.data, package, |value| &value.identity)?;
        if self
            .operators
            .windows(2)
            .any(|pair| pair[0].coordinate >= pair[1].coordinate)
        {
            return Err("public operators repeat or change canonical coordinate order");
        }
        for value in &self.operators {
            root(&value.coordinate.identity, package)?;
            shared::operator(&value.coordinate)?;
            declarations::operator(value)?;
        }
        for value in &self.traits {
            declarations::trait_shape(value, self)?;
        }
        for value in &self.conformances {
            declarations::conformance(value, self)?;
        }
        for value in &self.domains {
            declarations::domain(value)?;
        }
        for value in &self.propositions {
            declarations::proposition(value, self)?;
        }
        for value in &self.consts {
            value_type(&value.declared_type)?;
            text(&value.canonical_value_encoding)?;
        }
        for value in &self.data {
            data::validate(value)?;
        }
        Ok(())
    }
}

fn roots<T>(
    values: &[T],
    package: PackageKeyIdentity,
    identity: impl Fn(&T) -> &PackageReviewNominalIdentity,
) -> Result {
    if values
        .windows(2)
        .any(|pair| identity(&pair[0]) >= identity(&pair[1]))
    {
        return Err("public declarations repeat or change canonical identity order");
    }
    for value in values {
        root(identity(value), package)?;
    }
    Ok(())
}

fn root(identity: &PackageReviewNominalIdentity, package: PackageKeyIdentity) -> Result {
    nominal(identity)?;
    if identity.owner != PackageReviewNominalOwner::Package(package) {
        return Err("public declaration is not owned by its baseline package");
    }
    Ok(())
}

fn ordered<T: Ord>(values: &[T]) -> Result {
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        Err("public policy set repeats or changes canonical order")
    } else {
        Ok(())
    }
}

pub(in crate::record) fn scope(
    parameters: &[PackagePolicyTypeParameter],
    lifetimes: usize,
) -> Scope<'_> {
    Scope {
        outer: None,
        statics: &[],
        policy_statics: parameters,
        proposition_binders: &[],
        static_offset: 0,
        lifetimes,
        parameters: 0,
        nonself_parameters: 0,
        has_self: false,
        result: false,
        domain_subject: false,
    }
}
