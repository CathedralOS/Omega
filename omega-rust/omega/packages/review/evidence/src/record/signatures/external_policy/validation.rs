use crate::record::package::callable_policy::validation::signature::{
    Result, nominal, operator, owned_pair, text, value_type,
};
use crate::record::public_policy::validation::{
    scope,
    signatures::{bounds, parameters},
};
use crate::record::*;

impl PackagePolicyExternalExecutableSupply {
    pub(crate) fn validate_canonical_structure(&self) -> Result {
        nominal(&self.callable)?;
        signature(&self.signature)?;
        match &self.requirement {
            PackagePolicyExternalRequirement::Trait(value) => {
                owned_pair(&value.trait_identity, &value.requirement_identity)?;
                if value
                    .trait_lifetime_arguments
                    .iter()
                    .any(|ordinal| *ordinal as usize >= self.signature.lifetime_parameter_count)
                {
                    return Err("external requirement lifetime escapes the callable telescope");
                }
                let mut seen = Vec::new();
                let normalized = value
                    .trait_lifetime_arguments
                    .iter()
                    .map(|ordinal| {
                        let index = match seen.iter().position(|prior| prior == ordinal) {
                            Some(index) => index,
                            None => {
                                seen.push(*ordinal);
                                seen.len() - 1
                            }
                        };
                        u32::try_from(index).map_err(|_| "external lifetime partition overflows")
                    })
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                if value.requirement_lifetime_partition != normalized {
                    return Err(
                        "external requirement lifetime partition differs from its actual arguments",
                    );
                }
                for argument in &value.arguments {
                    value_type(argument)?;
                }
            }
            PackagePolicyExternalRequirement::Operator { coordinate, .. } => operator(coordinate)?,
            PackagePolicyExternalRequirement::TopLevelRequirement {
                identity,
                signature: value,
                ..
            } => {
                nominal(identity)?;
                signature(value)?;
            }
        }
        if let Some(alias) = self.requirement.alias() {
            text(alias)?;
        }
        if let PackagePolicyExternalBinding::NormalizedImport { producer, .. }
        | PackagePolicyExternalBinding::NormalizedSyscall { producer, .. } = &self.binding
        {
            nominal(producer.declaration())?;
            text(producer.callable_identity())?;
            if let Some(package) = producer.package()
                && producer.declaration().owner() != PackageReviewNominalOwner::Package(package)
            {
                return Err("external producer package differs from its exact declaration");
            }
        }
        Ok(())
    }
}

fn signature(value: &PackagePolicyExternalCallableSignature) -> Result {
    let mut scope = scope(&value.static_parameters, value.lifetime_parameter_count);
    scope.parameters = value.parameters.len();
    let receivers = value
        .parameters
        .iter()
        .filter(|parameter| parameter.is_self)
        .count();
    if receivers > 1 {
        return Err("external signature repeats its receiver");
    }
    scope.has_self = receivers == 1;
    scope.nonself_parameters = value.parameters.len() - receivers;
    scope.result = value.return_type.is_some();
    parameters(&scope, 0)?;
    bounds(&value.conformance_bounds, &scope)?;
    for parameter in &value.parameters {
        value_type(&parameter.type_identity)?;
    }
    if let Some(value) = &value.return_type {
        value_type(value)?;
    }
    Ok(())
}
