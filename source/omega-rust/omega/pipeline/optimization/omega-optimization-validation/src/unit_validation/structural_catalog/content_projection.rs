use super::*;

pub(crate) fn validate_content_projection_scalar(
    value: &ContentProjectionScalar,
    carrier: StructuralTypeId,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    depth: usize,
) -> bool {
    if depth > 256 {
        return false;
    }
    match value {
        ContentProjectionScalar::SubjectField(path)
        | ContentProjectionScalar::RuntimeScalarEmbedding(path) => {
            if path.is_empty() || path.iter().any(String::is_empty) {
                return false;
            }
            let mut current = carrier;
            for (index, segment) in path.iter().enumerate() {
                let Some(declaration) = types.get(&current) else {
                    return false;
                };
                let psi_terminal::StructuralTypeShape::Record { fields } = &declaration.shape
                else {
                    return false;
                };
                let Some(field) = fields.iter().find(|field| field.identity == *segment) else {
                    return false;
                };
                let last = index + 1 == path.len();
                match (&field.field_type, last) {
                    (psi_terminal::StructuralFieldType::Structural(next), false) => {
                        current = *next;
                    }
                    (psi_terminal::StructuralFieldType::Scalar(_), true) => {}
                    _ => return false,
                }
            }
            true
        }
        ContentProjectionScalar::Natural(value) => {
            !value.is_empty()
                && value.bytes().all(|byte| byte.is_ascii_digit())
                && (value == "0" || !value.starts_with('0'))
        }
        ContentProjectionScalar::Successor(inner) => {
            validate_content_projection_scalar(inner, carrier, types, depth + 1)
        }
        ContentProjectionScalar::Add(left, right)
        | ContentProjectionScalar::Subtract(left, right)
        | ContentProjectionScalar::Multiply(left, right) => {
            validate_content_projection_scalar(left, carrier, types, depth + 1)
                && validate_content_projection_scalar(right, carrier, types, depth + 1)
        }
    }
}

pub(crate) fn validate_content_projection_expression(
    expression: &ContentProjectionExpression,
    carrier: StructuralTypeId,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    match expression {
        ContentProjectionExpression::IntervalSet(members) => members.iter().all(|(start, end)| {
            validate_content_projection_scalar(start, carrier, types, 0)
                && validate_content_projection_scalar(end, carrier, types, 0)
        }),
        ContentProjectionExpression::CountedQuantity(magnitude) => {
            validate_content_projection_scalar(magnitude, carrier, types, 0)
        }
    }
}

pub(crate) fn validate_structural_content_projection(
    semantic_domain: psi_core::DomainSemanticId,
    carrier: StructuralTypeId,
    projection: &psi_terminal::StructuralContentProjection,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    let shape_matches_algebra = matches!(
        (&projection.expression, projection.algebra.kind),
        (
            ContentProjectionExpression::IntervalSet(_),
            psi_core::ContentAlgebraKind::IntervalSet
        ) | (
            ContentProjectionExpression::CountedQuantity(_),
            psi_core::ContentAlgebraKind::CountedQuantity
        )
    );
    projection.identity.domain.get() == semantic_domain.get()
        && projection.identity.projection_fingerprint != 0
        && !projection.algebra.parameter.is_empty()
        && shape_matches_algebra
        && validate_content_projection_expression(&projection.expression, carrier, types)
        && psi_language_semantics::content::terminal_projection_fingerprint(
            &projection.algebra,
            &projection.expression,
        ) == projection.identity.projection_fingerprint
}
