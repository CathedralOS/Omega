use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::expressions::body_typer::BodyTyper;
use crate::{ObligationKind, Site, TypeError, TypedStage, ValueType};
use omega_parser::NodeKind;
use omega_resolver::Symbol;

pub(crate) fn check_static_argument(
    unit: &ResolvedSourceFile<'_>,
    catalog: &DeclarationCatalog<'_>,
    node_index: usize,
    expected: &ValueType,
    output: &mut TypedStage,
) -> Result<(), TypeError> {
    let mut body = BodyTyper::new(unit, catalog, output);
    body.static_argument = true;
    body.caller = unit
        .file
        .enclosing_contexts(node_index)
        .find_map(|p| {
            unit.definitions
                .get(&unit.file.span(p))
                .filter(|d| matches!(d.kind, NodeKind::Machine | NodeKind::AsmMachine))
        })
        .map(|d| (**d).clone());
    let start = body.output.calls.len();
    let value = if unit.file.nodes[node_index].kind == NodeKind::PathType
        && let Some(Symbol::Declaration(id)) = unit.symbol(node_index)
        && id.kind == NodeKind::Const
    {
        catalog.constants[id].clone()
    } else {
        body.type_expression_with_expected_type(node_index, Some(expected))?
    };
    if body.output.calls[start..].iter().any(|c| c.evidence.is_some())
        && !catalog
            .types
            .deferred_sites
            .borrow()
            .contains(&Site::at(&unit.file, node_index))
        && catalog.types.evaluator.is_none()
    {
        return Err(TypeError::at(
            &unit.file,
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
    let value = body.type_expression(expr)?;
    if matches!(value, ValueType::Exact { .. } | ValueType::NoResult) {
        return Err(TypeError::at(&unit.file, expr, "constant requires a concrete value type"));
    }
    Ok(value)
}
pub(crate) fn check_fragment(
    unit: &ResolvedSourceFile<'_>,
    catalog: &DeclarationCatalog<'_>,
    output: &mut TypedStage,
) -> Result<(), TypeError> {
    let mut body = BodyTyper::new(unit, catalog, output);
    body.check_block(0, None).map(|_| ())
}
pub(crate) fn check_callable(
    unit: &ResolvedSourceFile<'_>,
    catalog: &DeclarationCatalog<'_>,
    node_index: usize,
    output: &mut TypedStage,
) -> Result<(), TypeError> {
    let source = &unit.file;
    let ValueType::Callable(c) = unit.raw_type(node_index, None, &catalog.types)? else {
        return Err(TypeError::unsupported(source, node_index));
    };
    let params = source
        .child(node_index, NodeKind::Params)
        .ok_or_else(|| TypeError::unsupported(source, node_index))?;
    let locals = source.nodes[params]
        .children
        .iter()
        .zip(&c.parameters)
        .map(|(&p, t)| {
            source.nodes[p]
                .words()
                .next()
                .map(|name| (name.span, t.clone()))
                .ok_or_else(|| TypeError::unsupported(source, p))
        })
        .collect::<Result<_, _>>()?;
    let mut body = BodyTyper {
        locals,
        subject: Some(c.result.clone()),
        ..BodyTyper::new(unit, catalog, output)
    };
    for &clause in &source.nodes[node_index].children {
        if source.nodes[clause].kind == NodeKind::Crashes {
            body.crash_route(clause, node_index)?;
        }
        if matches!(source.nodes[clause].kind, NodeKind::Requires | NodeKind::Ensures) {
            let expr = unit.single_child(clause)?;
            let ty = body.type_expression_with_expected_type(expr, Some(&ValueType::Bool))?;
            body.check_destination(expr, &ty, &ValueType::Bool)?;
        }
    }
    Ok(())
}
pub(crate) fn check_declaration(
    unit: &ResolvedSourceFile<'_>,
    catalog: &DeclarationCatalog<'_>,
    node_index: usize,
    output: &mut TypedStage,
) -> Result<(), TypeError> {
    let source = &unit.file;
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
            .flat_map(|s| s.params.iter().cloned())
            .collect(),
        caller: matches!(id.kind, NodeKind::Machine | NodeKind::AsmMachine).then(|| id.clone()),
        subject,
        ..BodyTyper::new(unit, catalog, output)
    };
    if source.nodes[node_index].kind == NodeKind::MathLet {
        let mut result = catalog.types.mathematical[id].clone();
        if let Some(params) = source.child(node_index, NodeKind::Params) {
            for &p in &source.nodes[params].children {
                let ValueType::Function(input, out) = result else {
                    return Err(TypeError::unsupported(source, p));
                };
                let name = source.nodes[p]
                    .words()
                    .next()
                    .ok_or_else(|| TypeError::unsupported(source, p))?;
                body.locals.insert(name.span, *input);
                result = out.apply_subject(&ValueType::Subject {
                    declaration: unit.binder_site(name.span),
                    version: None,
                });
            }
        }
        if source.nodes[node_index].leaves.iter().any(|l| l.text == "=") {
            let term = *source.nodes[node_index].children.last().unwrap();
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
        let value = body.type_expression_with_expected_type(expr, Some(ty))?;
        return body.check_destination(expr, &value, ty);
    }
    for &part in &source.nodes[node_index].children {
        match source.nodes[part].kind {
            NodeKind::Requires | NodeKind::Ensures => {
                let expr = unit.single_child(part)?;
                body.check_fact(expr)?;
                if source.nodes[part].kind == NodeKind::Ensures && signature.is_some() {
                    body.record_obligation(
                        expr,
                        ObligationKind::ProveGuarantee {
                            machine: id.clone(),
                            predicate: Site::at(source, expr),
                        },
                    );
                }
            }
            NodeKind::Hypothesis => body.check_hypothesis(part)?,
            NodeKind::Crashes => body.crash_route(part, node_index)?,
            NodeKind::Terminates => body.ranking(part, id)?,
            NodeKind::Invokes => body.invocation(part, id)?,
            NodeKind::Body => {
                let result = &signature
                    .ok_or_else(|| TypeError::unsupported(source, node_index))?
                    .result;
                body.check_block(part, Some(result))?;
            }
            _ => {}
        }
    }
    Ok(())
}
