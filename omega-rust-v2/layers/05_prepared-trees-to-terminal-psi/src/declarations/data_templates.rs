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
    if !own
        .into_iter()
        .chain(
            captured
                .iter()
                .filter_map(|id| trees.generic_parameters.get(id)),
        )
        .any(needs_template)
    {
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
                    fields: template_fields(prepared, tree, child, case, &payload)?,
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
    let fields = template_fields(prepared, tree, node, owner, &field_nodes)?;
    if !fields.is_empty() && !cases.is_empty() {
        return Err(LoweringError::new(&site, "typed data mixes record fields and sum cases"));
    }
    let properties = trees
        .data_properties
        .iter()
        .find(|(id, _)| id == owner)
        .map(|(_, p)| p.clone());
    if properties.is_none() && file.child(node, NodeKind::Attributes).is_some() {
        return Err(LoweringError::new(&site, "data has no typed multiplicity and carry attributes"));
    }
    let properties = properties.unwrap_or_default();
    Ok(Some(DataTemplate {
        declaration: crate::identity::declaration(owner, &site)?,
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
        carry: properties.carry,
        site,
    }))
}
fn template_fields(
    prepared: &omega_prepared::PreparedTrees<'_>,
    tree: &TypedTree<'_>,
    node: usize,
    owner: &omega_resolved::DeclarationId,
    field_nodes: &[usize],
) -> Result<Vec<TemplateField>, LoweringError> {
    super::data::checked_fields(prepared.typed(), tree, node, owner, field_nodes)?
        .iter()
        .map(|f| {
            Ok(TemplateField {
                declaration: crate::identity::declaration(&f.declaration, &f.site)?,
                ty: field_type(prepared, &f.ty, &f.site)?,
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
    ty: &ValueType,
    site: &omega_typed::Site,
) -> Result<crate::TypeExpression, LoweringError> {
    let ValueType::Callable(callable) = ty else {
        return type_expression(ty, site);
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
