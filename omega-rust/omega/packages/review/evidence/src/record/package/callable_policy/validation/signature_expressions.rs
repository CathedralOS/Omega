use super::signature::*;
use crate::record::*;

pub(super) fn expression(
    value: &PackageReviewContractExpression,
    scope: &Scope<'_>,
    nesting: usize,
) -> Result {
    depth(nesting)?;
    use PackageReviewContractExpression as Expression;
    match value {
        Expression::Boolean(_) | Expression::Float(_) | Expression::ByteSequence(_) => {}
        Expression::Integer(value) => text(value)?,
        Expression::DomainSubject => {
            return Err("callable expression has no domain-subject binder");
        }
        Expression::Parameter(ordinal) => {
            if *ordinal as usize >= scope.parameters {
                return Err("contract parameter escapes its value telescope");
            }
        }
        Expression::Result => {
            if !scope.result {
                return Err("contract result has no result value in this scope");
            }
        }
        Expression::GenericBinder(ordinal) => {
            if scope.static_kind(*ordinal).is_none() {
                return Err("contract generic binder escapes its static telescope");
            }
        }
        Expression::Nominal(value) => nominal(value)?,
        Expression::ZeroValue(value) => value_type(value)?,
        Expression::Array(elements) => {
            for element in elements {
                expression(element, scope, nesting + 1)?;
            }
        }
        Expression::Constructor { data, case, fields } => {
            nominal(data)?;
            if let Some(case) = case {
                owned_pair(data, case)?;
            }
            for (index, field) in fields.iter().enumerate() {
                owned_pair(data, &field.field)?;
                if fields[..index]
                    .iter()
                    .any(|prior| prior.field == field.field)
                {
                    return Err("constructor repeats an exact field");
                }
                expression(&field.value, scope, nesting + 1)?;
            }
        }
        Expression::Indexed {
            meaning,
            collection,
            index,
        } => {
            meaning_check(meaning)?;
            expression(collection, scope, nesting + 1)?;
            expression(index, scope, nesting + 1)?;
        }
        Expression::Range { start, end, .. } => {
            for value in [start, end].into_iter().flatten() {
                expression(value, scope, nesting + 1)?;
            }
        }
        Expression::Reference { target, .. } => expression(target, scope, nesting + 1)?,
        Expression::AtomicLoad { value, .. } => expression(value, scope, nesting + 1)?,
        Expression::CollectionLength { collection } => expression(collection, scope, nesting + 1)?,
        Expression::Member {
            receiver,
            member,
            case_variant,
        } => {
            expression(receiver, scope, nesting + 1)?;
            nominal(member)?;
            if let Some(case) = case_variant {
                owned_pair(case, member)?;
            }
        }
        Expression::Cast {
            value,
            target,
            semantic_domain,
            semantic_domain_arguments,
            ..
        } => {
            expression(value, scope, nesting + 1)?;
            value_type(target)?;
            if let Some(domain) = semantic_domain {
                nominal(domain)?;
            }
            for argument in semantic_domain_arguments {
                value_type(argument)?;
            }
        }
        Expression::Call {
            receiver,
            target,
            static_arguments,
            evidence_arguments,
            arguments,
        } => {
            if let Some(receiver) = receiver {
                expression(receiver, scope, nesting + 1)?;
            }
            if let PackageReviewContractCallTarget::Nominal(target) = target {
                nominal(target)?;
            }
            for argument in static_arguments {
                static_argument(argument, scope, nesting + 1)?;
            }
            for argument in arguments {
                expression(argument, scope, nesting + 1)?;
            }
            for evidence in evidence_arguments {
                nominal(&evidence.source.owner)?;
                nominal(&evidence.parameter.owner)?;
            }
        }
        Expression::Binary {
            meaning,
            left,
            right,
            ..
        } => {
            meaning_check(meaning)?;
            expression(left, scope, nesting + 1)?;
            expression(right, scope, nesting + 1)?;
        }
        Expression::Unary { operand, .. } => expression(operand, scope, nesting + 1)?,
    }
    Ok(())
}

fn meaning_check(meaning: &PackageReviewContractOperatorMeaning) -> Result {
    if let PackageReviewContractOperatorMeaning::Declared(coordinate) = meaning {
        operator(coordinate)?;
    }
    Ok(())
}

pub(super) fn static_argument(
    value: &PackageReviewContractStaticArgument,
    scope: &Scope<'_>,
    nesting: usize,
) -> Result {
    depth(nesting)?;
    use PackageReviewContractStaticArgument as Argument;
    match value {
        Argument::Type(value) => value_type(value)?,
        Argument::GenericTypeBinder(ordinal) => {
            if !matches!(
                scope.static_kind(*ordinal),
                Some(PackageReviewTypeParameterKind::Type)
            ) {
                return Err("type argument does not name a type binder");
            }
        }
        Argument::GenericConstBinder(ordinal) => {
            if !matches!(
                scope.static_kind(*ordinal),
                Some(PackageReviewTypeParameterKind::Const(_))
            ) {
                return Err("const argument does not name a const binder");
            }
        }
        Argument::GenericMachineBinder(ordinal) => {
            if !matches!(
                scope.static_kind(*ordinal),
                Some(PackageReviewTypeParameterKind::Machine(_))
            ) {
                return Err("machine argument does not name a machine binder");
            }
        }
        Argument::GenericType {
            base,
            lifetime_arguments,
            arguments,
        } => {
            value_type(base)?;
            lifetimes(lifetime_arguments, scope)?;
            for argument in arguments {
                static_argument(argument, scope, nesting + 1)?;
            }
        }
        Argument::ConstInteger(value) => text(value)?,
        Argument::ConstBoolean(_) => {}
        Argument::ConstStructured {
            declared_type,
            canonical_value_encoding,
        } => {
            value_type(declared_type)?;
            text(canonical_value_encoding)?;
        }
        Argument::ConcreteMachine(value) => nominal(value)?,
        Argument::ConformanceApplication {
            declaration,
            arguments,
            subject,
            trait_identity,
            trait_arguments,
        } => {
            nominal(declaration)?;
            nominal(trait_identity)?;
            for argument in arguments {
                static_argument(argument, scope, nesting + 1)?;
            }
            static_argument(subject, scope, nesting + 1)?;
            for argument in trait_arguments {
                value_type(argument)?;
            }
        }
    }
    Ok(())
}
