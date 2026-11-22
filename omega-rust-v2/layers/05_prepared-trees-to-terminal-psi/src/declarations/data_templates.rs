use super::mathematics::{TermScope, lower_term};
use super::type_expression::{callable_interface, template_binders, type_expression};
use crate::lowering::applied_tree::LoweringTree;
use crate::{ContractClause, DataTemplate, LoweringError, PredicateStep, TemplateCase, TemplateField, site_at};
use omega_syntax::NodeKind;
use omega_typed::{TypedTree, ValueType};

pub(crate) fn read(
    prepared: &omega_prepared::PreparedTrees<'_>,
    tree: &TypedTree<'_>,
    node: usize,
    nested: &mut Vec<usize>,
    regions: &super::regions::RegionData<'_>,
) -> Result<Option<DataTemplate>, LoweringError> {
    let trees = prepared.typed();
    let resolved = tree.resolved();
    let file = resolved.file;
    let site = site_at(tree, node);
    let Some(owner) = resolved.declaration_at(node) else { return Ok(None) };
    if trees.type_alias_targets.contains_key(owner) {
        return Ok(None);
    }
    let captured = trees
        .enclosing_data_parameters
        .get(owner)
        .map_or(&[][..], Vec::as_slice);
    let own = trees.generic_parameters.get(owner);
    let scopes = || own.into_iter().chain(captured.iter().filter_map(|id| trees.generic_parameters.get(id)));
    // A const-indexed record can declare lifetimes before its indices, as
    // `Link<'r, const N: u64>` does. They select no schema: the template keeps
    // each as a `Lifetime` binder at its position, and a field names it
    // through `AtLifetime` or as a lifetime argument of the record it refers to.
    let leading_lifetimes = regions.indexed_lifetimes(owner).map_or(0, |(lifetimes, _)| lifetimes);
    let const_indices = |scope: &omega_typed::GenericParameters, lifetimes: usize| {
        scope.equations.is_empty() && scope.witness_count == 0 && scope.parameters[lifetimes..].iter().all(|p| {
            p.kind == NodeKind::ConstParam && matches!(p.carrier.as_deref(), Some(ValueType::Integer { width: Some(1..), .. }))
        })
    };
    let indexed = scopes().any(|scope| !scope.parameters.is_empty())
        && own.is_none_or(|scope| const_indices(scope, leading_lifetimes))
        && captured.iter().filter_map(|id| trees.generic_parameters.get(id)).all(|scope| const_indices(scope, 0));
    // A concrete data declaration records a region held by a direct reference
    // field, or passed on to a record `regions` lists. A region in any other
    // carrier belongs in the field's type expression.
    let nested_lifetime = !regions.contains(owner) && own.is_some_and(|scope| !scope.parameters.is_empty() && scope.parameters.iter().all(|p| p.kind == NodeKind::LifetimeParam)
        && std::iter::once(owner)
            .chain(file.nodes[node].children.iter().filter(|&&member| file.nodes[member].kind == NodeKind::Case)
                .filter_map(|&member| resolved.declaration_at(member)))
            .filter_map(|id| trees.record_fields.get(id))
            .flat_map(|fields| fields.iter())
            .any(|field| {
                use super::regions::Stored;
                !matches!(super::regions::stored(&field.ty, owner, scope.parameters.len(), regions), Some(Stored::Plain | Stored::Reference { .. }))
            }));
    if !indexed && !nested_lifetime && !scopes().any(needs_template) {
        return Ok(None);
    }
    let fields = trees
        .record_fields
        .get(owner)
        .map_or(&[][..], |fields| fields.as_slice());
    let generic_nodes: Vec<_> = file
        .generic_parameters(node)
        .filter(|(_, _, kind)| *kind != NodeKind::SomeParam)
        .map(|(_, binder, _)| binder)
        .collect();
    let (parameters, witness_count) = template_binders(own, &site)?;
    if generic_nodes.len() != parameters.len() {
        return Err(LoweringError::new(&site, "dependent data has incomplete checked binders"));
    }
    let enclosing_parameters = captured
        .iter()
        .map(|id| crate::identity::declaration(id, &site))
        .collect::<Result<Vec<_>, _>>()?;
    let equations = own
        .into_iter()
        .flat_map(|p| &p.equations)
        .map(|(index, ty)| Ok((*index, type_expression(ty, &site)?)))
        .collect::<Result<_, LoweringError>>()?;
    let mut field_nodes = Vec::new();
    let mut cases = Vec::new();
    let mut requires = Vec::new();
    for &child in &file.nodes[node].children {
        match file.nodes[child].kind {
            NodeKind::GenericParams | NodeKind::RetiredNumber | NodeKind::Attributes => {}
            NodeKind::Data => nested.push(child),
            NodeKind::Field => {
                super::data::field_members(tree, child, nested)?;
                field_nodes.push(child);
            }
            NodeKind::Case => {
                let case = resolved
                    .declaration_at(child)
                    .ok_or_else(|| LoweringError::new(&site_at(tree, child), "case has no selected declaration"))?;
                let mut payload = Vec::new();
                for &member in &file.nodes[child].children {
                    match file.nodes[member].kind {
                        NodeKind::CaseField => {
                            super::data::field_members(tree, member, nested)?;
                            payload.push(member);
                        }
                        NodeKind::StableNumber | NodeKind::RetiredNumber => {}
                        _ => return Err(LoweringError::new(&site_at(tree, member), "unsupported dependent case metadata")),
                    }
                }
                cases.push(TemplateCase {
                    declaration: crate::identity::declaration(case, &site_at(tree, child))?,
                    fields: template_fields(prepared, tree, child, case, &payload, indexed)?,
                    numbering: trees.schema_numbering.get(case).cloned(),
                    site: site_at(tree, child),
                });
            }
            NodeKind::Requires => {
                let [term] = *file.nodes[child].children.as_slice() else {
                    return Err(LoweringError::new(&site_at(tree, child), "dependent data clause needs one predicate"));
                };
                if !tree
                    .semantics_at(term)
                    .any(|row| matches!(row,omega_typed::NodeSemantics::Expression(e) if matches!(e.ty,ValueType::Bool|ValueType::Universe{strict:true,..})))
                {
                    return Err(LoweringError::new(&site_at(tree, term), "dependent data predicate needs a checked proposition"));
                }
                if tree
                    .semantics_at(term)
                    .any(|row| matches!(row, omega_typed::NodeSemantics::Expression(e) if e.ty == ValueType::Bool))
                {
                    requires.push(super::contracts::lower_clause(
                        prepared,
                        &LoweringTree::plain(tree),
                        super::contracts::ClauseOwner::Template(owner),
                        child,
                    )?);
                    continue;
                }
                let steps = lower_term(
                    &LoweringTree::plain(tree),
                    term,
                    TermScope::Data {
                        owner,
                        binders: &generic_nodes,
                        fields,
                    },
                    None,
                )?;
                requires.push(ContractClause {
                    predicate: vec![PredicateStep::MathematicalTerm(steps)],
                    site: site_at(tree, child),
                });
            }
            other => {
                return Err(LoweringError::new(
                    &site_at(tree, child),
                    format!("unsupported member of dependent data template: {other:?}"),
                ));
            }
        }
    }
    let fields = template_fields(prepared, tree, node, owner, &field_nodes, indexed)?;
    if !fields.is_empty() && !cases.is_empty() {
        return Err(LoweringError::new(&site, "typed data mixes record fields and sum cases"));
    }
    let properties = trees
        .data_properties
        .iter()
        .find(|(id, _)| id == owner)
        .map(|(_, p)| p.clone());
    if properties.is_none() && file.child(node, NodeKind::Attributes).is_some() {
        return Err(LoweringError::new(&site, "data has no typed multiplicity"));
    }
    let properties = properties.unwrap_or_default();
    Ok(Some(DataTemplate {
        declaration: crate::identity::declaration(owner, &site)?,
        formation: if indexed {
            super::data_formation::read(tree, node, owner)?
        } else { vec![] },
        parameters,
        equations,
        witness_count,
        enclosing_parameters,
        public: resolved.is_exported(node),
        fields,
        cases,
        requires,
        numbering: trees.schema_numbering.get(owner).cloned(),
        multiplicity: properties.multiplicity,
        site,
    }))
}
fn template_fields(
    prepared: &omega_prepared::PreparedTrees<'_>,
    tree: &TypedTree<'_>,
    node: usize,
    owner: &omega_resolved::DeclarationId,
    field_nodes: &[usize],
    indexed: bool,
) -> Result<Vec<TemplateField>, LoweringError> {
    super::data::checked_fields(prepared.typed(), tree, node, owner, field_nodes)?
        .iter()
        .map(|f| {
            Ok(TemplateField {
                declaration: crate::identity::declaration(&f.declaration, &f.site)?,
                ty: field_type(prepared, tree, &f.ty, &f.site, indexed)?,
                site: f.site.clone(),
            })
        })
        .collect()
}

fn needs_template(params: &omega_typed::GenericParameters) -> bool {
    params.witness_count != 0
        || params.parameters.iter().any(|p| {
            p.kind == NodeKind::TypeParam
                || matches!(
                    p.carrier.as_deref(),
                    Some(ValueType::Universe { .. } | ValueType::Function(..) | ValueType::LevelKind)
                )
        })
}
fn field_type(
    prepared: &omega_prepared::PreparedTrees<'_>,
    tree: &TypedTree<'_>,
    ty: &ValueType,
    site: &omega_typed::Site,
    indexed: bool,
) -> Result<crate::TypeExpression, LoweringError> {
    // A deferred field extent names the checked static expression, not an
    // answer at one representative application. Preserve that expression for
    // the template; the concrete schema below it uses L3's resumed shape.
    if indexed && let ValueType::IndexedArray(element, extent) = ty
        && let ValueType::StaticFormula { term: omega_typed::IntegerIndex::Deferred { template, arguments }, .. } = extent.as_ref()
    {
        let request = template.request();
        let mut terms = type_expression(element, site)?.0;
        let mut work = vec![(request.node, false)];
        while let Some((id, emit)) = work.pop() {
            if id.node >= tree.resolved().file.nodes.len() || tree.resolved().file.node_id(id.node) != id { return Err(LoweringError::new(site, "field extent needs its owning expression")); }
            let mut calls = tree.semantics_at(id.node).filter_map(|row| match row { omega_typed::NodeSemantics::Call(c) => Some(c), _ => None });
            if let Some(call) = calls.next() {
                if calls.next().is_some() || call.machine.package != site.package || call.operands.iter().any(|op| op.node.file != id.file || op.node.node <= id.node || op.node.node >= tree.resolved().file.nodes[id.node].end) || !call.type_arguments.is_empty() || !call.enclosing_arguments.is_empty() || call.evidence.is_some() || call.layout_query.is_some() {
                    return Err(LoweringError::new(site, "static field call requires its complete selected application"));
                }
                if emit {
                    terms.push(crate::TypeExpressionStep::StaticCall { machine: crate::identity::declaration(&call.machine, &call.site)?, operands: call.operands.len(), site: call.site.clone() });
                } else {
                    work.push((id, true));
                    work.extend(call.operands.iter().rev().map(|op| (op.node, false)));
                }
            } else {
                let binder = tree.semantics_at(id.node).find_map(|row| match row {
                    omega_typed::NodeSemantics::Expression(e) => e.static_argument_slot.and_then(|i| arguments.get(i)), _ => None,
                }).ok_or_else(|| LoweringError::new(site, "static field operand requires its checked owner argument"))?;
                terms.extend(type_expression(binder, site)?.0);
            }
        }
        terms.push(crate::TypeExpressionStep::IndexedArray);
        return Ok(crate::TypeExpression(terms));
    }
    let ValueType::Callable(callable) = ty else {
        return if indexed { super::type_expression::indexed_expression(ty, site) } else { type_expression(ty, site) };
    };
    if !callable.closed_runtime {
        return Err(LoweringError::new(site, "stored callable requires a closed structural interface"));
    }
    // The field names an alias; its signature owns the contract clauses.
    let (origin, node) = prepared
        .typed()
        .callable_origin(&callable.contract)
        .ok_or_else(|| LoweringError::new(site, "stored callable lacks its checked structural contract"))?;
    let interface = callable_interface(prepared, &origin, node, callable)?;
    Ok(crate::TypeExpression(vec![crate::TypeExpressionStep::Callable(Box::new(interface))]))
}
