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

/// Verify all direct moves of this formal and its published normal-return
/// complement against the schema. A copy, dropped residual, or wrong order
/// cannot supply the same ownership listing.
pub fn moved_formal_fields<'a>(package: &'a LoweredPackage, template: &'a MachineTemplate, formal: usize) -> Option<(Vec<&'a str>, Vec<&'a str>)> {
    let [TypeExpressionStep::Named { declaration, argument_count: 0 }] = template.signature.parameters.get(formal)?.0.as_slice() else { return None };
    let record = package.data.iter().find(|r| r.declaration == *declaration)?;
    if record.multiplicity != omega_psi::Multiplicity::Affine || record.cleanup_target.is_some() { return None; }
    let mut moved = Vec::new();
    for call in template.body.iter().filter_map(|step| step.call()) {
        for operand in &call.operands {
            if let TemplateOperand::MoveField { formal: root, field, ty } = operand && *root == formal {
                let entry = record.fields.iter().find(|f| f.declaration == *field)?;
                let TypeShape::Nominal(id) = package.types.get(entry.carrier)? else { return None };
                let selected = package.data.iter().find(|d| d.id == *id)?;
                if selected.multiplicity != omega_psi::Multiplicity::Affine { return None; }
                if !describes(package, ty, entry.carrier) || moved.contains(&entry.declaration.path.as_str()) { return None; }
                moved.push(entry.declaration.path.as_str());
            }
        }
    }
    if moved.is_empty() { return None; }
    let mut actual = template.discarded_fields.iter().filter(|row| row.formal == formal);
    let mut residual = Vec::new();
    for field in record.fields.iter().rev().filter(|f| !moved.contains(&f.declaration.path.as_str())) {
        let row = actual.next()?;
        if row.field != field.declaration || !describes(package, &row.ty, field.carrier) { return None; }
        residual.push(field.declaration.path.as_str());
    }
    actual.next().is_none().then_some((moved,residual))
}
