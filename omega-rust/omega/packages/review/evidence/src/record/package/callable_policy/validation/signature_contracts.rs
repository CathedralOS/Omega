use super::{signature::*, signature_expressions::expression};
use crate::record::*;

pub(in crate::record) fn contract(
    value: &PackageReviewCallableContract,
    scope: &Scope<'_>,
    nesting: usize,
) -> Result {
    if let Some(binding) = &value.binding {
        text(binding)?;
    }
    if let Some(case) = &value.result_case {
        if value.kind != PackageReviewContractKind::Ensures || !scope.result {
            return Err("outcome guarantee has no result scope");
        }
        owned_pair(&case.result_data, &case.result_case)?;
    }
    let clause = Scope {
        result: scope.result && value.kind == PackageReviewContractKind::Ensures,
        ..*scope
    };
    fact(&value.fact, &clause, nesting)
}

pub(in crate::record) fn fact(
    value: &PackageReviewContractFact,
    scope: &Scope<'_>,
    nesting: usize,
) -> Result {
    match value {
        PackageReviewContractFact::Expression(value) => expression(value, scope, nesting)?,
        PackageReviewContractFact::Membership { value, domain } => {
            nominal(domain)?;
            expression(value, scope, nesting)?;
        }
        PackageReviewContractFact::PropositionParameter(application) => {
            let Some(BinderKind::Proposition(signature)) =
                scope.static_kind(application.binder_ordinal)
            else {
                return Err("proposition application does not name a proposition binder");
            };
            if application.arguments.len() != signature.parameters.len() {
                return Err("proposition binder argument count differs from its signature");
            }
            for argument in &application.arguments {
                expression(argument, scope, nesting)?;
            }
        }
        PackageReviewContractFact::Proposition(application) => {
            proposition(application, scope, nesting)?
        }
    }
    Ok(())
}

fn proposition(
    value: &PackageReviewPropositionApplication,
    scope: &Scope<'_>,
    nesting: usize,
) -> Result {
    nominal(&value.declaration)?;
    if value.binders.len() != value.binder_arguments.len()
        || value.parameter_types.len() != value.arguments.len()
    {
        return Err("proposition application differs from its exact telescope");
    }
    for parameter in &value.parameter_types {
        value_type(parameter)?;
    }
    for argument in &value.arguments {
        expression(argument, scope, nesting)?;
    }
    for (binder, argument) in value.binders.iter().zip(&value.binder_arguments) {
        use PackageReviewPropositionBinderArgumentKind as Kind;
        let expected = match &binder.kind {
            PackageReviewPropositionBinderKind::Type => Kind::Type,
            PackageReviewPropositionBinderKind::Const(value) => {
                value_type(value)?;
                Kind::Const
            }
            PackageReviewPropositionBinderKind::Machine => Kind::Machine,
        };
        if argument.kind != expected {
            return Err("proposition argument changes its binder kind");
        }
        use PackageReviewPropositionBinderValue as Value;
        match &argument.value {
            Value::Type(value) if expected == Kind::Type => value_type(value)?,
            Value::Machine(value) if expected == Kind::Machine => nominal(value)?,
            Value::Integer(value) if expected == Kind::Const => text(value)?,
            Value::GenericBinder(ordinal) => {
                let valid = matches!(
                    (expected, scope.static_kind(*ordinal)),
                    (Kind::Type, Some(BinderKind::Type))
                        | (Kind::Const, Some(BinderKind::Const))
                        | (Kind::Machine, Some(BinderKind::Machine))
                );
                if !valid {
                    return Err("proposition generic argument escapes its exact binder kind");
                }
            }
            Value::EvidenceProjection {
                declaring_trait,
                declaring_trait_arguments,
                requirement,
                ..
            } if expected == Kind::Machine => {
                owned_pair(declaring_trait, requirement)?;
                for argument in declaring_trait_arguments {
                    value_type(argument)?;
                }
            }
            _ => return Err("proposition value does not inhabit its binder category"),
        }
    }
    if let PackageReviewPropositionEvidence::Witness(interface) = &value.evidence {
        evidence(interface, scope)?;
    }
    Ok(())
}

pub(in crate::record) fn evidence(
    value: &PackageReviewEvidenceInterface,
    scope: &Scope<'_>,
) -> Result {
    nominal(&value.trait_identity)?;
    lifetimes(&value.lifetime_arguments, scope)?;
    for argument in &value.arguments {
        value_type(argument)?;
    }
    for requirement in &value.requirements {
        owned_pair(&requirement.declaring_trait, &requirement.requirement)?;
        lifetimes(&requirement.declaring_trait_lifetime_arguments, scope)?;
        for argument in &requirement.declaring_trait_arguments {
            value_type(argument)?;
        }
    }
    Ok(())
}
