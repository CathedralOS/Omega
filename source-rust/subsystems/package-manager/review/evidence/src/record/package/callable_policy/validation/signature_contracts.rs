// Structural validation of callable contract clauses, facts, propositions, and evidence interfaces.

use super::{signature::*, signature_expressions::expression};
use crate::record::{
    PackageReviewCallableContract, PackageReviewContractFact, PackageReviewContractKind,
    PackageReviewEvidenceInterface, PackageReviewPropositionApplication,
    PackageReviewPropositionBinderKind, PackageReviewPropositionEvidence,
};

use std::collections::BTreeMap;

type RequiresWitnesses<'a> = BTreeMap<u32, &'a PackageReviewEvidenceInterface>;

pub(in crate::record) fn roster(
    clauses: &[PackageReviewCallableContract],
    scope: &Scope<'_>,
    nesting: usize,
) -> Result {
    let mut witnesses = RequiresWitnesses::new();
    for clause in clauses {
        let Some(position) = clause.evidence_lane_position else {
            continue;
        };
        if clause.kind == PackageReviewContractKind::Requires {
            let PackageReviewContractFact::Proposition(application) = &clause.fact else {
                return Err("requires evidence lane has no proposition witness");
            };
            let PackageReviewPropositionEvidence::Witness(interface) = &application.evidence else {
                return Err("requires evidence lane has no proposition witness");
            };
            if witnesses.insert(position, interface).is_some() {
                return Err("callable repeats a requires evidence lane position");
            }
        }
    }
    for clause in clauses {
        contract(clause, scope, nesting, &witnesses)?;
    }
    Ok(())
}

fn contract(
    value: &PackageReviewCallableContract,
    scope: &Scope<'_>,
    nesting: usize,
    witnesses: &RequiresWitnesses,
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
    scoped_fact(&value.fact, &clause, nesting, witnesses)
}

pub(in crate::record) fn fact(
    value: &PackageReviewContractFact,
    scope: &Scope<'_>,
    nesting: usize,
) -> Result {
    scoped_fact(value, scope, nesting, &RequiresWitnesses::new())
}

fn scoped_fact(
    value: &PackageReviewContractFact,
    scope: &Scope<'_>,
    nesting: usize,
    witnesses: &RequiresWitnesses,
) -> Result {
    match value {
        PackageReviewContractFact::Expression(value) => expression(value, scope, nesting)?,
        PackageReviewContractFact::Membership {
            value,
            domain,
            application,
        } => {
            domain.validate_resolved()?;
            text(application)?;
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
            proposition(application, scope, nesting, witnesses)?
        }
    }
    Ok(())
}

fn proposition(
    value: &PackageReviewPropositionApplication,
    scope: &Scope<'_>,
    nesting: usize,
    witnesses: &RequiresWitnesses,
) -> Result {
    value.declaration.validate_resolved()?;
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
        use crate::record::PackageReviewPropositionBinderArgumentKind as Kind;
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
        use crate::record::PackageReviewPropositionBinderValue as Value;
        match &argument.value {
            Value::Type(value) if expected == Kind::Type => value_type(value)?,
            Value::Machine(value) if expected == Kind::Machine => value.validate_resolved()?,
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
                source_kind,
                source_lane_position,
            } if expected == Kind::Machine => {
                if *source_kind != PackageReviewContractKind::Requires {
                    return Err("machine projection must name a requires evidence lane");
                }
                let interface = witnesses
                    .get(source_lane_position)
                    .ok_or("machine projection names an unknown requires evidence lane")?;
                let mut members = interface.requirements.iter().filter(|member| {
                    member.declaring_trait == *declaring_trait && member.requirement == *requirement
                });
                let (Some(member), None) = (members.next(), members.next()) else {
                    return Err("machine projection requires one exact witness member");
                };
                if member.declaring_trait_arguments != *declaring_trait_arguments {
                    return Err("machine projection changes the witness member trait arguments");
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
    value.trait_identity.validate_resolved()?;
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
