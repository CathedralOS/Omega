// Canonical public API structure and explicit binder scope validation, plus the shared strict canonical-order predicate (`ordered_by`).

use crate::record::{
    PackagePolicyConformanceShape, PackagePolicyPublicApi, PackagePolicyTypeParameter,
    PackageReviewConstMeaning, PackageReviewNominalIdentity, PackageReviewNominalOwner,
};
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
            match &value.meaning {
                PackageReviewConstMeaning::Evaluated(encoding) => text(encoding)?,
                PackageReviewConstMeaning::TargetDependent(initializer) => {
                    expressions::expression(initializer, &scope(&[], 0), 0)?;
                }
            }
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
        policy_statics: parameters,
        proposition_binders: &[],
        static_offset: 0,
        lifetimes,
        parameters: 0,
        result: false,
        domain_subject: false,
    }
}

#[cfg(test)]
mod constant_dependency_tests {
    use super::*;
    use crate::record::{
        PackageReviewConstShape, PackageReviewContractExpression as Expression,
        PackageReviewLayoutQueryKind, PackageReviewTypeIdentity,
    };

    #[test]
    fn constant_dependency_receiving_rejects_runtime_scope_and_missing_identity() {
        let package = PackageKeyIdentity::from_digest([1; 32]).unwrap();
        let api = |meaning| PackagePolicyPublicApi {
            traits: Vec::new(),
            conformances: Vec::new(),
            domains: Vec::new(),
            propositions: Vec::new(),
            operators: Vec::new(),
            data: Vec::new(),
            consts: vec![PackageReviewConstShape {
                identity: PackageReviewNominalIdentity {
                    owner: PackageReviewNominalOwner::Package(package),
                    path: "RAW_SIZE".to_owned(),
                },
                declared_type: PackageReviewTypeIdentity {
                    canonical: "named(name(u64))".to_owned(),
                },
                meaning,
            }],
        };
        let query = |canonical: &str| Expression::LayoutQuery {
            kind: PackageReviewLayoutQueryKind::Size,
            operand: PackageReviewTypeIdentity {
                canonical: canonical.to_owned(),
            },
        };
        api(PackageReviewConstMeaning::TargetDependent(query(
            "named(name(u8))",
        )))
        .validate_canonical_structure(package)
        .unwrap();
        for expression in [
            Expression::Result,
            Expression::Parameter(0),
            Expression::GenericBinder(0),
            query(""),
            Expression::Nominal(PackageReviewNominalIdentity {
                owner: PackageReviewNominalOwner::Unresolved,
                path: "Main".to_owned(),
            }),
        ] {
            assert!(
                api(PackageReviewConstMeaning::TargetDependent(expression))
                    .validate_canonical_structure(package)
                    .is_err()
            );
        }
        assert!(
            api(PackageReviewConstMeaning::Evaluated(String::new()))
                .validate_canonical_structure(package)
                .is_err()
        );
    }
}
