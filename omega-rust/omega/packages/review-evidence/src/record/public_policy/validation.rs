//! Canonical public meaning and explicit binder scopes, never compiler proof.

use crate::record::{
    PackagePolicyConformanceShape, PackagePolicyPublicApi, PackagePolicyTypeParameter,
    PackageReviewNominalIdentity, PackageReviewNominalOwner,
};
mod behavior;
mod data;
mod declarations;
pub(in crate::record) mod signatures;

use crate::record::package::callable_policy::validation::{
    signature::{self as shared, *},
    signature_contracts as contracts, signature_expressions as expressions,
};
use semantic_vocabulary::PackageKeyIdentity;

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
        ordered_by(&self.operators, |a, b| a.coordinate.cmp(&b.coordinate))?;
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
    ordered_by(values, |a, b| identity(a).cmp(identity(b)))?;
    for value in values {
        root(identity(value), package)?;
    }
    Ok(())
}

fn root(identity: &PackageReviewNominalIdentity, package: PackageKeyIdentity) -> Result {
    identity.validate_resolved()?;
    if identity.owner != PackageReviewNominalOwner::Package(package) {
        return Err("public declaration is not owned by its baseline package");
    }
    Ok(())
}

/// The shared strict canonical-order predicate: every policy family that
/// keeps a sorted set consumes this one statement of the rule instead of
/// restating it, and the recovery readers agree with the policy writer on it.
/// A set whose canonical key is a projection of the row supplies that key's
/// comparison instead of deriving a sortable shadow type.
pub(in crate::record) fn ordered_by<T>(
    values: &[T],
    compare: impl Fn(&T, &T) -> std::cmp::Ordering,
) -> Result {
    if values
        .windows(2)
        .any(|pair| compare(&pair[0], &pair[1]).is_ge())
    {
        Err("policy set repeats or changes canonical order")
    } else {
        Ok(())
    }
}

pub(in crate::record) fn ordered<T: Ord>(values: &[T]) -> Result {
    ordered_by(values, T::cmp)
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
        result: false,
        domain_subject: false,
    }
}
