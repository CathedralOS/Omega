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
    let (lifetime_count, generic_bounds) = generic_bounds(trees.generic_parameters.get(owner), file.generic_parameters(node).count(), &site)?;
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
        lifetime_count,
        generic_bounds,
        parameter_count: formals.len(),
        body,
        assumptions: portable_ids(dependencies, &site)?,
        site,
    })
}

/// The leading lifetime binders of a mathematical telescope, counted, and
/// the checked bound of each binder after them.
///
/// A formal's type can apply a record at a lifetime, as
/// `pick<'r>(x: Link<'r, u8, bool>)` does, and the application needs a binder
/// of its own declaration to name. A lifetime has no bound and no erased
/// value. It keeps its telescope position, so the signature's
/// `GenericParameter` indices stay those the source wrote. Only a leading
/// lifetime is counted: after any other binder it would shift the bounds
/// that follow it, so it is refused like every other unsupported binder.
pub(super) fn generic_bounds(
    binders: Option<&omega_typed::GenericParameters>,
    written_count: usize,
    site: &Site,
) -> Result<(usize, Vec<TypeExpression>), LoweringError> {
    let error = |reason| LoweringError::new(site, reason);
    let mut lifetime_count = 0;
    let mut generic_bounds = Vec::new();
    if let Some(binders) = binders {
        if binders.witness_count != 0 || !binders.equations.is_empty() {
            return Err(error("mathematical telescope has unsupported witnesses or equations"));
        }
        for parameter in &binders.parameters {
            if parameter.kind == NodeKind::LifetimeParam && parameter.carrier.is_none() && parameter.multiplicity.is_none() && generic_bounds.is_empty() {
                lifetime_count += 1;
                continue;
            }
            let Some(bound) = &parameter.carrier else {
                return Err(error("mathematical parameter needs its checked bound"));
            };
            if parameter.kind != NodeKind::KindedParam || parameter.multiplicity.is_some() {
                return Err(error("unsupported mathematical generic binder"));
            }
            generic_bounds.push(type_expression(bound, site)?);
        }
    }
    if lifetime_count + generic_bounds.len() != written_count {
        return Err(error("mathematical declaration has an incomplete checked telescope"));
    }
    Ok((lifetime_count, generic_bounds))
}

fn portable_ids(
    ids: &[DeclarationId],
    site: &Site,
) -> Result<Vec<omega_psi::DeclarationId>, LoweringError> {
    ids.iter()
        .map(|id| crate::identity::declaration(id, site))
        .collect()
}

/// Psi represents the question by the selected declaration's telescope and Apply's
/// ordered actuals. Check that this projection accounts for the prepared rows;
/// do not decide whether a subject inhabits a universe here.
fn check_application_memberships(
    tree: &LoweringTree<'_>,
    definition: Option<&DeclarationId>,
    arguments: &[ValueType],
    memberships: &[(&ValueType, &ValueType)],
    site: &Site,
) -> Result<(), LoweringError> {
    let mut rows = memberships.iter().copied();
    let parameters = definition.and_then(|id| tree.forest().generic_parameters.get(id));
    if arguments.len() != parameters.map_or(0, |p| p.parameters.len()) {
        return Err(LoweringError::new(site, "mathematical application disagrees with its generic telescope"));
    }
    if let (Some(id), Some(parameters)) = (definition, parameters) {
        for (argument, parameter) in arguments.iter().zip(&parameters.parameters) {
            if let Some(bound @ ValueType::Universe { .. }) = parameter.carrier.as_deref() {
                let instantiated = bound.at_machine_application(id, arguments, &[], &[]);
                if rows.next() != Some((argument, &instantiated)) {
                    return Err(LoweringError::new(site, "universe membership disagrees with the mathematical argument or its bound"));
                }
            }
        }
    }
    if rows.next().is_some() {
        return Err(LoweringError::new(site, "universe membership has no corresponding mathematical binder"));
    }
    Ok(())
}

pub(super) enum TermScope<'a> {
    Parameters(&'a [usize]),
    Hypothesis {
        theorem: &'a [usize],
        bounds: &'a [super::hypotheses::Bound<'a>],
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

/// A plain declaration identifies the contract only when the local package
/// emits it without import, member selection or specialization.
fn local_citation_target(tree: &TypedTree<'_>, machine: &DeclarationId) -> bool {
    let forest = tree.forest();
    let Some(sig) = forest.signatures.iter().find(|sig| sig.machine == *machine) else { return false };
    let Some(target) = forest.trees().nth(sig.node.file) else { return false };
    let unit = target.resolved();
    let n = sig.node.node;
    unit.declaration_at(n) == Some(machine)
        && unit.file.nodes[0].children.contains(&n)
        && unit.file.generic_parameters(n).next().is_none()
        && unit.file.child(n, NodeKind::Body).is_some()
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
        let mut squash = None;
        let mut citation = None;
        let mut memberships = Vec::new();
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
                    ObligationKind::FormSquash { source, level } if squash.is_none() && o.site == site => squash = Some((source, level)),
                    ObligationKind::MathematicalFact { sort: result } if sort.is_none() => sort = Some(result),
                    ObligationKind::RefineMathematicalCitation { machine, expected } if citation.is_none() && o.site == site => {
                        citation = Some((machine, expected));
                    }
                    ObligationKind::UniverseMembership { subject, sort } if syntax.kind == NodeKind::Call => memberships.push((subject, sort)),
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
        if let Some((source, level)) = squash {
            if is_callee
                || syntax.kind != NodeKind::PathExpression
                || !matches!(resolved.symbol(node), Some(Symbol::Builtin(name)) if name == "core\\Squash")
                || !matches!(level, ValueType::LevelOf(ty) if ty.as_ref() == source)
                || !matches!(ty, Some(ValueType::Universe { strict: true, level: at }) if at.as_ref() == level)
                || sort.is_some_and(|sort| Some(sort) != ty)
                || op.is_some()
                || application.is_some()
                || !dependencies.is_empty()
            {
                return Err(error("squash formation disagrees with its checked type or level"));
            }
            terms.push(MathematicalStep::Squash(type_expression(source, &site)?));
            continue;
        }
        if let Some((machine, expected)) = citation {
            if syntax.kind != NodeKind::PathExpression || !syntax.children.is_empty() || is_callee
                || op.is_some() || application.is_some() || !dependencies.is_empty() || ty != Some(expected)
                || machine.kind != NodeKind::Machine || !matches!(resolved.symbol(node), Some(Symbol::Declaration(id)) if id == machine)
            {
                return Err(error("mathematical citation disagrees with its selected machine or checked interface"));
            }
            if machine.package != site.package || !local_citation_target(tree, machine) {
                return Err(error("mathematical citation needs a local non-template machine contract"));
            }
            terms.push(MathematicalStep::RefineCitation {
                machine: crate::identity::declaration(machine, &site)?,
                expected: type_expression(expected, &site)?,
            });
            continue;
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
                check_application_memberships(tree, definition.as_ref(), generic_arguments, &memberships, &site)?;
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
                    Some(Symbol::Binding { binder_node, span, .. }) => match &scope {
                        TermScope::Parameters(parameters) => MathematicalStep::Parameter(
                            parameters
                                .iter()
                                .position(|p| p == binder_node)
                                .ok_or_else(|| error("mathematical term refers to an unsupported binding"))?,
                        ),
                        TermScope::Hypothesis { theorem, bounds, slot, .. } => {
                            if let Some((hypothesis, index, _)) = super::hypotheses::scoped_parameter(bounds, Some(*slot), span) {
                                MathematicalStep::HypothesisParameter { hypothesis, index }
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
