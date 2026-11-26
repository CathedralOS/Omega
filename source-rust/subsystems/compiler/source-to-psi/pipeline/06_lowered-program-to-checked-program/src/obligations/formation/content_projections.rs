//! Form owner projections over the same logical field observations used by data clauses.
//! The resulting inequalities are proof obligations, independent of resource custody.
use super::{
    BindingError, Checked, ClauseBindings, Construct, MachineId, PlaceId, Proposition,
    RefusalReason, Scope, TerminalModule, Version,
};
use semantic_vocabulary::{
    ContentAlgebraKind, ContentProjectionExpression, ContentProjectionScalar, IntegerMathTerm,
    IntegerSign, IntegerValue, StructuralTypeId,
};
use terminal_semantics::domain_preservation::content_scalar_path;

pub(super) fn form(
    module: &TerminalModule,
    diagnostic: MachineId,
    activity: &mut crate::ProofSearchActivity,
) -> Checked<()> {
    for domain in &module.types.structural_domains {
        let Some(projection) = &domain.content_projection else {
            continue;
        };
        let expression = &projection.expression;
        let owner = module
            .identities
            .text(domain.identity)
            .unwrap_or("unnamed domain");
        if projection.identity.domain.get() != domain.semantic_domain.get()
            || projection.algebra.parameter.is_empty()
            || !matches!(
                (expression, projection.algebra.kind),
                (
                    ContentProjectionExpression::CountedQuantity(_),
                    ContentAlgebraKind::CountedQuantity
                ) | (
                    ContentProjectionExpression::IntervalSet(_),
                    ContentAlgebraKind::IntervalSet
                )
            )
            || projection.identity.projection_report_fingerprint
                != expression.report_fingerprint(&projection.algebra)
        {
            return Err(crate::refusal(
                RefusalReason::ForeignIdentity,
                diagnostic,
                Construct::Machine,
                &format!(
                    "content projection on {owner} differs from its declared domain or algebra"
                ),
            ));
        }
        let subject = PlaceId::new(1).expect("one is a place");
        let mut scope = Scope::data_subject(module, diagnostic, subject, domain.carrier, activity);
        let check = |scope: &mut Scope<'_, '_>| -> Result<(), BindingError> {
            match expression {
                ContentProjectionExpression::CountedQuantity(value) => {
                    natural(scope, domain.carrier, subject, value)?;
                }
                ContentProjectionExpression::IntervalSet(members) => {
                    let mut previous = None;
                    for (start, end) in members {
                        let start = natural(scope, domain.carrier, subject, start)?;
                        let end = natural(scope, domain.carrier, subject, end)?;
                        if let Some(previous) = previous {
                            scope.prove(&Proposition::IntegerMathLessOrEqual(
                                previous,
                                start.clone(),
                            ))?;
                        }
                        scope.prove(&Proposition::IntegerMathLessOrEqual(start, end.clone()))?;
                        previous = Some(end);
                    }
                }
            }
            Ok(())
        };
        check(&mut scope)
            .map_err(|error| scope.error(error, &format!("content projection on {owner}")))?;
    }
    Ok(())
}

fn natural(
    scope: &mut Scope<'_, '_>,
    carrier: StructuralTypeId,
    subject: PlaceId,
    value: &ContentProjectionScalar,
) -> Result<IntegerMathTerm, BindingError> {
    let term = scalar(scope, carrier, subject, value, 0)?;
    scope.prove(&Proposition::IntegerMathLessOrEqual(
        literal(0),
        term.clone(),
    ))?;
    Ok(term)
}

fn literal(value: u128) -> IntegerMathTerm {
    IntegerMathTerm::literal(IntegerValue::Unsigned(value))
}

fn decimal(value: &str) -> Option<IntegerMathTerm> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    // Each chunk fits u128. The enclosing term is mathematical even for a longer numeral.
    let chunk = |digits: &[u8]| -> Option<u128> { std::str::from_utf8(digits).ok()?.parse().ok() };
    let mut chunks = value.as_bytes().chunks(38);
    let mut result = literal(chunk(chunks.next()?)?);
    for digits in chunks {
        result = IntegerMathTerm::Add(
            Box::new(IntegerMathTerm::Multiply(
                Box::new(result),
                Box::new(literal(10u128.pow(digits.len() as u32))),
            )),
            Box::new(literal(chunk(digits)?)),
        );
    }
    Some(result)
}

fn scalar(
    scope: &mut Scope<'_, '_>,
    carrier: StructuralTypeId,
    subject: PlaceId,
    value: &ContentProjectionScalar,
    depth: usize,
) -> Result<IntegerMathTerm, BindingError> {
    use ContentProjectionScalar as Scalar;
    if depth > 256 {
        return Err("content projection exceeds the supported expression depth".into());
    }
    let operand = |scope: &mut Scope<'_, '_>, value| {
        scalar(scope, carrier, subject, value, depth + 1).map(Box::new)
    };
    Ok(match value {
        Scalar::Natural(value) => {
            decimal(value).ok_or("content projection has a malformed natural literal")?
        }
        Scalar::RuntimeScalarEmbedding(path) => {
            let (path, ty) = content_scalar_path(
                scope.module.types.structural_types.iter(),
                &scope.module.identities,
                carrier,
                path,
            )
            .ok_or("content projection lost its exact structural scalar field")?;
            let integer = ty
                .as_integer()
                .ok_or("content projection embeds a noninteger field")?;
            if !integer.is_address()
                && (integer.sign() != IntegerSign::Unsigned
                    || !matches!(integer.bits(), Some(8 | 16 | 32 | 64)))
            {
                return Err(
                    "content projection embedding needs a u8/u16/u32/u64/address field".into(),
                );
            }
            IntegerMathTerm::embed(scope.field(Version::Current, subject, &path, ty)?)
                .ok_or("content projection field did not form an integer observation")?
        }
        Scalar::SubjectField(_) => {
            return Err("content projection needs a declared proof-natural subject field".into());
        }
        Scalar::Successor(term) => {
            IntegerMathTerm::Add(operand(scope, term)?, Box::new(literal(1)))
        }
        Scalar::Add(a, b) => IntegerMathTerm::Add(operand(scope, a)?, operand(scope, b)?),
        Scalar::Multiply(a, b) => IntegerMathTerm::Multiply(operand(scope, a)?, operand(scope, b)?),
        Scalar::Subtract(a, b) => {
            let left = operand(scope, a)?;
            let right = operand(scope, b)?;
            scope.prove(&Proposition::IntegerMathLessOrEqual(
                (*right).clone(),
                (*left).clone(),
            ))?;
            IntegerMathTerm::Subtract(left, right)
        }
    })
}
