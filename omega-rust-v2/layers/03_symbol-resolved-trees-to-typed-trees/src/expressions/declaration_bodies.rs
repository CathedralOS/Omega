use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::expressions::body_typer::{BodyTyper, ExpressionContext};
use crate::{ObligationKind, TypeError, TypedStage, ValueType};
use omega_parser::NodeKind;

pub(crate) fn check_static_argument(
    unit: &ResolvedSourceFile<'_>,
    catalog: &DeclarationCatalog<'_>,
    node_index: usize,
    request: &crate::StaticValueRequest,
    evaluation: &crate::evaluation::check_static_indices::IndexEvaluation,
    output: &mut TypedStage,
) -> Result<(), TypeError> {
    let mut body = BodyTyper::new(unit, catalog, output);
    body.context = ExpressionContext::StaticArgument;
    let expected = &request.carrier;
    body.caller = unit
        .file
        .ancestors(node_index)
        .find_map(|p| {
            unit.declaration_at(p)
                .filter(|d| matches!(d.kind, NodeKind::Machine | NodeKind::AsmMachine))
        })
        .cloned();
    let start = body.output.calls.len();
    let value = body.type_expression_with_expected_type(node_index, Some(expected))?;
    if body.output.calls[start..].iter().any(|c| c.evidence.is_some())
        && !matches!(
            evaluation,
            crate::evaluation::check_static_indices::IndexEvaluation::Specialized(_) | crate::evaluation::check_static_indices::IndexEvaluation::Pending
        )
        && !catalog.types.indices.has_supplied_value(request)
    {
        return Err(TypeError::at(
            unit.file,
            node_index,
            "static operator requires evaluation with its selected evidence",
        ));
    }
    body.check_destination(node_index, &value, expected)
}
pub(crate) fn infer_constant(
    unit: &ResolvedSourceFile<'_>,
    catalog: &DeclarationCatalog<'_>,
    expr: usize,
    output: &mut TypedStage,
) -> Result<ValueType, TypeError> {
    let mut body = BodyTyper::new(unit, catalog, output);
    body.context = ExpressionContext::StaticArgument;
    let value = body.type_expression(expr)?;
    if value.holds_anonymous_number() || value == ValueType::NoResult {
        return Err(TypeError::at(unit.file, expr, "constant requires a concrete value type"));
    }
    Ok(value)
}
pub(crate) fn check_fragment(
    unit: &ResolvedSourceFile<'_>,
    catalog: &DeclarationCatalog<'_>,
    output: &mut TypedStage,
) -> Result<(), TypeError> {
    let mut body = BodyTyper::new(unit, catalog, output);
    body.check_block(0, Default::default(), false).map(|_| ())
}
pub(crate) fn check_callable(
    unit: &ResolvedSourceFile<'_>,
    catalog: &DeclarationCatalog<'_>,
    node_index: usize,
    output: &mut TypedStage,
) -> Result<(), TypeError> {
    let source = unit.file;
    let ValueType::Callable(c) = unit.annotation(node_index, &catalog.types)? else {
        return Err(TypeError::unsupported(source, node_index));
    };
    let mut body = BodyTyper {
        locals: source
            .parameters(node_index)
            .map(|(name, _)| name.span)
            .zip(c.parameters)
            .collect(),
        subject: Some(c.result),
        ..BodyTyper::new(unit, catalog, output)
    };
    for &clause in &source.nodes[node_index].children {
        body.check_contract_clause(clause, node_index)?;
    }
    Ok(())
}
pub(crate) fn check_declaration(
    unit: &ResolvedSourceFile<'_>,
    catalog: &DeclarationCatalog<'_>,
    node_index: usize,
    output: &mut TypedStage,
) -> Result<(), TypeError> {
    let source = unit.file;
    let id = unit.declaration(node_index)?;
    let signature = catalog.signatures.get(id);
    let subject = match source.nodes[node_index].kind {
        NodeKind::Machine | NodeKind::AsmMachine => signature.map(|s| s.result.clone()),
        NodeKind::Domain => Some(catalog.types.domains.carrier(id).clone()),
        NodeKind::Data => Some(catalog.types.open_data(id)),
        _ => None,
    };
    let mut body = BodyTyper {
        locals: signature
            .into_iter()
            .flat_map(|s| s.parameters.iter().cloned())
            .collect(),
        caller: matches!(id.kind, NodeKind::Machine | NodeKind::AsmMachine).then(|| id.clone()),
        subject,
        ..BodyTyper::new(unit, catalog, output)
    };
    if let Some(binding) = source.math_binding(node_index) {
        body.context = ExpressionContext::Proof;
        let mut result = catalog.types.mathematical[id].clone();
        for (name, annotation) in source.parameters(node_index) {
            let ValueType::Function(input, out) = result else {
                return Err(TypeError::unsupported(source, annotation));
            };
            body.locals.insert(name.span, *input);
            result = out.apply_subject(&ValueType::Subject {
                declaration: unit.binder_site(name.span),
                version: None,
            });
        }
        if let Some(term) = binding.body {
            let value = body.type_expression_with_expected_type(term, Some(&result))?;
            if matches!(result, ValueType::Universe { strict: true, .. }) && value == ValueType::Bool {
                body.record_obligation(term, ObligationKind::MathematicalFact { sort: result });
            } else {
                body.check_destination(term, &value, &result)?;
            }
        }
        return Ok(());
    }
    if source.nodes[node_index].kind == NodeKind::AsmMachine {
        body.assembly(node_index)?;
    }
    if source.nodes[node_index].kind == NodeKind::Const {
        let expr = *source.nodes[node_index]
            .children
            .last()
            .ok_or_else(|| TypeError::unsupported(source, node_index))?;
        let ty = &catalog.constants[id];
        body.context = ExpressionContext::StaticArgument;
        let value = body.type_expression_with_expected_type(expr, Some(ty))?;
        return body.check_destination(expr, &value, ty);
    }
    for &part in &source.nodes[node_index].children {
        match source.nodes[part].kind {
            NodeKind::Requires | NodeKind::Ensures | NodeKind::Crashes | NodeKind::Hypothesis => body.check_contract_clause(part, node_index)?,
            NodeKind::Terminates => {
                body.context = ExpressionContext::ContractValue;
                body.ranking(part, id)?;
            }
            NodeKind::Invokes => body.invocation(part, id)?,
            NodeKind::Body => {
                body.context = ExpressionContext::Execution;
                let result = &signature
                    .ok_or_else(|| TypeError::unsupported(source, node_index))?
                    .result;
                body.check_block(part, Some(result).into(), true)?;
            }
            _ => {}
        }
    }
    Ok(())
}
