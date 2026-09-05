//! One exact executable binding and full surface per external callable.

use super::*;

impl PackagePolicyBaseline {
    pub(super) fn validate_external_associations(&self) -> Result<(), &'static str> {
        // The checked external owner admits one exact realization edge per
        // machine. Full-row ordering alone would permit conflicting bindings.
        if self
            .external_supplies
            .windows(2)
            .any(|pair| pair[0].callable() == pair[1].callable())
        {
            return Err("external policy repeats one exact callable coordinate");
        }
        for supply in &self.external_supplies {
            supply.validate_canonical_structure()?;
            let callable = self
                .callables
                .callables
                .iter()
                .find(|callable| &callable.identity == supply.callable())
                .ok_or("external policy has no exact retained callable")?;
            if callable.supply != PackageReviewCallableSupply::ExternalRealization
                || !matches!(
                    callable.role,
                    PackagePolicyCallableRole::Public | PackagePolicyCallableRole::PrivateExternal
                )
            {
                return Err("external policy has no public or private external callable surface");
            }
            let signature = supply.signature();
            if signature.lifetime_parameter_count() != callable.lifetime_parameter_count
                || signature.static_parameters() != callable.type_parameters
                || signature.conformance_bounds() != callable.conformance_bounds
                || signature.return_type() != callable.return_type.as_ref()
                || signature.parameters().len() != callable.parameters.len()
                || signature.parameters().iter().zip(&callable.parameters).any(
                    |(external, formal)| {
                        external.type_identity() != &formal.type_identity
                            || external.is_const() != formal.is_const
                            || external.is_mutable() != formal.is_mutable
                            || external.is_self() != formal.is_self
                    },
                )
            {
                return Err("external policy signature differs from its retained callable surface");
            }
            match supply.requirement() {
                PackagePolicyExternalRequirement::Trait(required)
                    if callable.conformances.as_slice() == std::slice::from_ref(required)
                        && callable.operator_realizations.is_empty() => {}
                PackagePolicyExternalRequirement::Operator { coordinate, alias }
                    if callable.conformances.is_empty()
                        && callable.operator_realizations.len() == 1
                        && callable.operator_realizations[0].coordinate() == coordinate
                        && callable.operator_realizations[0].alias() == alias.as_deref() => {}
                PackagePolicyExternalRequirement::TopLevelRequirement { .. }
                    if callable.conformances.is_empty()
                        && callable.operator_realizations.is_empty() => {}
                _ => {
                    return Err(
                        "external policy requirement differs from its retained callable realization",
                    );
                }
            }
        }
        for callable in &self.callables.callables {
            if callable.supply == PackageReviewCallableSupply::ExternalRealization
                && !self
                    .external_supplies
                    .iter()
                    .any(|supply| supply.callable() == &callable.identity)
            {
                return Err("external callable omits its exact executable supply policy");
            }
        }
        Ok(())
    }
}
