use omega_psi::{LoweredPackage, MachineTemplate, Parameter, ParameterType, TemplateOperand, TypeExpression, TypeExpressionStep, TypeId, TypeShape};

/// Follow a template's field operand from its formal's published record through
/// each named field to the copied value, using published `data` rows only.
/// Returns the value's lane and type, the form a requirement formal states.
/// The operand's own lane and type expression must describe that same value.
pub fn template_field_argument(
    package: &LoweredPackage,
    template: &MachineTemplate,
    operand: &TemplateOperand,
) -> Result<ParameterType, &'static str> {
    let TemplateOperand::RecordField {
        formal,
        carrier,
        field,
        argument,
    } = operand
    else {
        return Err("not a field operand");
    };
    let formal = template
        .signature
        .parameters
        .get(*formal)
        .ok_or("no such formal")?;
    let [
        TypeExpressionStep::Named {
            declaration,
            argument_count: 0,
        },
    ] = formal.0.as_slice()
    else {
        return Err("formal is not a closed record");
    };
    let mut record = package
        .data
        .iter()
        .find(|d| &d.declaration == declaration)
        .ok_or("formal names no published record")?;
    let mut ty = None;
    for step in carrier.iter().chain([field]) {
        if ty.is_some() {
            let Some(TypeShape::Nominal(id)) = ty.and_then(|t| package.types.get(t)) else {
                return Err("carrier step does not reach a record");
            };
            record = package.data.get(id.get() - 1).ok_or("unpublished record")?;
        }
        ty = Some(
            record
                .fields
                .iter()
                .find(|f| &f.declaration == step)
                .ok_or("field does not belong to the reached record")?
                .carrier,
        );
    }
    let ty = ty.ok_or("empty path")?;
    let (scalar, expression) = match argument {
        Parameter::Scalar(e) => (true, e),
        Parameter::Structural(e) => (false, e),
    };
    let shape = package.types.get(ty).ok_or("unpublished type")?;
    if scalar != matches!(shape, TypeShape::PrimitiveScalar(_)) {
        return Err("published lane disagrees with the field's type");
    }
    if !describes(package, expression, ty) {
        return Err("published type disagrees with the field's type");
    }
    Ok(if scalar { Parameter::Scalar(ty) } else { Parameter::Structural(ty) })
}

/// A postfix type expression names exactly this published type.
fn describes(
    package: &LoweredPackage,
    expression: &TypeExpression,
    ty: TypeId,
) -> bool {
    fn step<'a>(
        package: &LoweredPackage,
        steps: &'a [TypeExpressionStep],
        ty: TypeId,
    ) -> Option<&'a [TypeExpressionStep]> {
        let (last, rest) = steps.split_last()?;
        match (last, package.types.get(ty)?) {
            (TypeExpressionStep::Scalar(s), TypeShape::PrimitiveScalar(t)) if s == t => Some(rest),
            (TypeExpressionStep::Array(n), TypeShape::FixedArray { element, length }) if n == length => step(package, rest, *element),
            (
                TypeExpressionStep::Named {
                    declaration,
                    argument_count: 0,
                },
                TypeShape::Nominal(id),
            ) => (package.data.get(id.get() - 1)?.declaration == *declaration).then_some(rest),
            _ => None,
        }
    }
    step(package, &expression.0, ty).is_some_and(<[_]>::is_empty)
}
