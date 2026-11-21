use super::type_expression::type_expression;
use crate::lowering::applied_tree::LoweringTree;
use crate::{LoweringError, MathematicalBody, MathematicalDeclaration, MathematicalStep, TypeExpression, site_at};
use omega_resolved::{DeclarationId, Symbol};
use omega_syntax::NodeKind;
use omega_typed::{NodeSemantics, ObligationKind, Site, TypedTree, TypedTrees, ValueType};

pub(crate) fn lower(
    trees: &TypedTrees<'_>,
    tree: &TypedTree<'_>,
    node: usize,
    interfaces: &std::collections::HashMap<&DeclarationId, (&ValueType, &[DeclarationId])>,
) -> Result<MathematicalDeclaration, LoweringError> {
    let resolved = tree.resolved();
    let file = resolved.file;
    let site = site_at(tree, node);
    let error = |message| LoweringError::new(&site, message);
    let owner = resolved
        .declaration_at(node)
        .ok_or_else(|| error("mathematical binding has no declaration identity"))?;
    let binding = file
        .math_binding(node)
        .ok_or_else(|| error("mathematical declaration has no binder"))?;
    let &(signature, dependencies) = interfaces
        .get(owner)
        .ok_or_else(|| error("mathematical declaration has no checked interface"))?;
    let generic_bounds = generic_bounds(trees.generic_parameters.get(owner), file.generic_parameters(node).count(), &site)?;
    let formals = &file.nodes[binding.parameters].children;
    let body = match (binding.body, file.nodes[node].has_boundary_modifier()) {
        (Some(term), false) => MathematicalBody::Definition(lower_term(&LoweringTree::plain(tree), term, TermScope::Parameters(formals), None)?),
        (None, true) if dependencies.contains(owner) => MathematicalBody::BoundaryAssumption,
        _ => return Err(error("mathematical declaration lacks its definition or boundary assumption identity")),
    };
    Ok(MathematicalDeclaration {
        declaration: crate::identity::declaration(owner, &site)?,
        public: resolved.is_exported(node),
        signature: type_expression(signature, &site)?,
        generic_bounds,
        parameter_count: formals.len(),
        body,
        assumptions: portable_ids(dependencies, &site)?,
        site,
    })
}

pub(super) fn generic_bounds(
    binders: Option<&omega_typed::GenericParameters>,
    written_count: usize,
    site: &Site,
) -> Result<Vec<TypeExpression>, LoweringError> {
    let error = |reason| LoweringError::new(site, reason);
    let mut generic_bounds = Vec::new();
    if let Some(binders) = binders {
        if binders.witness_count != 0 || !binders.equations.is_empty() {
            return Err(error("mathematical telescope has unsupported witnesses or equations"));
        }
        for parameter in &binders.parameters {
            let Some(bound) = &parameter.carrier else {
                return Err(error("mathematical parameter needs its checked bound"));
            };
            if parameter.kind != NodeKind::KindedParam || parameter.multiplicity.is_some() {
                return Err(error("unsupported mathematical generic binder"));
            }
            generic_bounds.push(type_expression(bound, site)?);
        }
    }
    if generic_bounds.len() != written_count {
        return Err(error("mathematical declaration has an incomplete checked telescope"));
    }
    Ok(generic_bounds)
}

fn portable_ids(
    ids: &[DeclarationId],
    site: &Site,
) -> Result<Vec<omega_psi::DeclarationId>, LoweringError> {
    ids.iter()
        .map(|id| crate::identity::declaration(id, site))
        .collect()
}

pub(super) enum TermScope<'a> {
    Parameters(&'a [usize]),
    Hypothesis {
        theorem: &'a [usize],
        local: &'a [usize],
        slot: usize,
        /// Only in a conclusion over a mathematical result.
        result: Option<&'a ValueType>,
    },
    Data {
        owner: &'a DeclarationId,
        binders: &'a [usize],
        fields: &'a [omega_typed::TypedField],
    },
}

pub(super) fn lower_term(
    tree: &LoweringTree<'_>,
    root: usize,
    scope: TermScope<'_>,
    guarantee: Option<&DeclarationId>,
) -> Result<Vec<MathematicalStep>, LoweringError> {
    enum Work {
        Visit(usize),
        Callee(usize),
        Emit(MathematicalStep),
    }
    let resolved = tree.resolved();
    let file = resolved.file;
    let mut work = vec![Work::Visit(root)];
    let mut terms = Vec::new();
    while let Some(item) = work.pop() {
        let is_callee = matches!(item, Work::Callee(_));
        let node = match item {
            Work::Visit(node) | Work::Callee(node) => node,
            Work::Emit(term) => {
                terms.push(term);
                continue;
            }
        };
        let site = site_at(tree, node);
        let error = |reason| LoweringError::new(&site, reason);
        let syntax = &file.nodes[node];
        let mut ty = None;
        let mut op = None;
        let mut application = None;
        let mut dependencies = Vec::new();
        let mut sort = None;
        let mut guarantee_obligation = false;
        let mut literal = None;
        for row in tree.semantics_at(node) {
            match row {
                NodeSemantics::Expression(e) if ty.is_none() && e.static_argument_slot.is_none() => ty = Some(&e.ty),
                NodeSemantics::Landing(l) if l.value.is_integer() && l.ty.contains_integer(l.value.numer()) && literal.is_none() => {
                    literal = Some((l.value.numer(), &l.ty))
                }
                NodeSemantics::Operation(operation) if op.is_none() => op = Some(operation),
                NodeSemantics::Obligation(o) => match &o.kind {
                    ObligationKind::ProveGuarantee { machine, predicate }
                        if node == root && guarantee == Some(machine) && predicate == &site && !guarantee_obligation =>
                    {
                        guarantee_obligation = true
                    }
                    ObligationKind::MathematicalApplication {
                        definition,
                        generic_arguments,
                        argument_count,
                    } if application.is_none() => application = Some((definition, generic_arguments, *argument_count)),
                    ObligationKind::AdmitMathematicalAssumption(id) => dependencies.push(crate::identity::declaration(id, &site)?),
                    ObligationKind::MathematicalFact { sort: result } if sort.is_none() => sort = Some(result),
                    ObligationKind::RefineMathematicalCitation { machine, expected } => {
                        return Err(LoweringError::new(
                            &site,
                            format!("Psi cannot cite machine `{}` as evidence of `{expected}` in a mathematical term", machine.path),
                        ));
                    }
                    ObligationKind::UniverseMembership { .. } => {
                        return Err(LoweringError::new(
                            &site,
                            "Psi cannot carry a universe membership check on a mathematical application",
                        ));
                    }
                    _ => return Err(error("unsupported obligation in mathematical definition")),
                },
                _ => return Err(error("unsupported selection in mathematical definition")),
            }
        }
        if guarantee_obligation != (node == root && guarantee.is_some()) {
            return Err(error("mathematical guarantee lacks its typed proof obligation"));
        }
        if literal.is_some() && syntax.kind != NodeKind::Literal {
            return Err(error("unsupported mathematical numeric landing"));
        }
        if ty.is_none() && !(is_callee && syntax.kind == NodeKind::PathExpression) {
            return Err(error("mathematical term has no checked type"));
        }
        if let Some(sort) = sort {
            work.push(Work::Emit(MathematicalStep::FormProposition(type_expression(sort, &site)?)));
        }
        match (syntax.kind, syntax.children.as_slice(), op, application) {
            (NodeKind::Call, [callee, args], None, Some((definition, generic_arguments, argument_count)))
                if file.nodes[*args].kind == NodeKind::Args && file.nodes[*args].children.len() == argument_count =>
            {
                if let Some(id) = definition
                    && !matches!(resolved.symbol(*callee),Some(Symbol::Declaration(actual)) if actual==id)
                {
                    return Err(error("mathematical application disagrees with its selected declaration"));
                }
                let generic_arguments = generic_arguments
                    .iter()
                    .map(|ty| type_expression(ty, &site))
                    .collect::<Result<Vec<_>, _>>()?;
                work.push(Work::Emit(MathematicalStep::Apply {
                    generic_arguments,
                    argument_count,
                    assumptions: dependencies,
                }));
                work.extend(
                    file.nodes[*args]
                        .children
                        .iter()
                        .rev()
                        .copied()
                        .map(Work::Visit),
                );
                work.push(Work::Callee(*callee));
            }
            (NodeKind::Binary | NodeKind::Unary, operands, Some(op), None)
                if dependencies.is_empty() && operands.len() == op.operands.len() && ty == Some(&op.result) =>
            {
                let operator =
                    super::contracts::predicate_operator(&op.token, &op.operands).ok_or_else(|| error("unsupported primitive in mathematical definition"))?;
                work.push(Work::Emit(MathematicalStep::Predicate(operator)));
                work.extend(operands.iter().rev().copied().map(Work::Visit));
            }
            (NodeKind::Paren, [inner], None, None) if dependencies.is_empty() => work.push(Work::Visit(*inner)),
            (NodeKind::Literal, [], None, None) if dependencies.is_empty() => {
                let (value, carrier) = match (literal, ty) {
                    (Some((value, ty)), _) => (
                        value,
                        Some(crate::lowering::types::scalar_carrier(ty).map_err(|err| LoweringError::new(&site, err.to_string()))?),
                    ),
                    (
                        None,
                        Some(ValueType::Exact {
                            value,
                            fractional_origin: None,
                        }),
                    ) if value.is_integer() => (value.numer(), None),
                    _ => return Err(error("unsupported mathematical literal")),
                };
                terms.push(MathematicalStep::Integer { value: value.clone(), carrier });
            }
            (NodeKind::PathExpression, children, None, None) if dependencies.is_empty() && (children.is_empty() || is_callee) => {
                let step = match resolved.symbol(node) {
                    Some(Symbol::Binding { binder_node, .. }) => match &scope {
                        TermScope::Parameters(parameters) => MathematicalStep::Parameter(
                            parameters
                                .iter()
                                .position(|p| p == binder_node)
                                .ok_or_else(|| error("mathematical term refers to an unsupported binding"))?,
                        ),
                        TermScope::Hypothesis { theorem, local, slot, .. } => {
                            if let Some(index) = local.iter().position(|p| p == binder_node) {
                                MathematicalStep::HypothesisParameter { hypothesis: *slot, index }
                            } else {
                                MathematicalStep::Parameter(
                                    theorem
                                        .iter()
                                        .position(|p| p == binder_node)
                                        .ok_or_else(|| error("mathematical term refers to an unsupported binding"))?,
                                )
                            }
                        }
                        TermScope::Data { owner, binders, .. } => MathematicalStep::GenericParameter {
                            owner: crate::identity::declaration(owner, &site)?,
                            index: binders
                                .iter()
                                .position(|p| p == binder_node)
                                .ok_or_else(|| error("mathematical term refers to an unsupported data binder"))?,
                        },
                    },
                    Some(Symbol::Builtin(name)) if name == "result"
                        && let TermScope::Hypothesis { slot, result: Some(carrier), .. } = &scope
                        && ty == Some(*carrier) => MathematicalStep::HypothesisResult(*slot),
                    Some(Symbol::Declaration(id)) if matches!(&scope, TermScope::Data{fields,..} if fields.iter().any(|field|&field.declaration==id)) => {
                        MathematicalStep::SelfField(crate::identity::declaration(id, &site)?)
                    }
                    Some(Symbol::Declaration(id)) if id.kind == NodeKind::MathLet && id.package == site.package => {
                        MathematicalStep::Declaration(crate::identity::declaration(id, &site)?)
                    }
                    Some(Symbol::Builtin(name)) if ty == Some(&ValueType::Bool) && matches!(name.as_str(), "true" | "false") => {
                        MathematicalStep::Boolean(name == "true")
                    }
                    _ => return Err(error("mathematical term has no supported subject")),
                };
                terms.push(step);
            }
            _ => return Err(error("unsupported checked mathematical term")),
        }
    }
    Ok(terms)
}
