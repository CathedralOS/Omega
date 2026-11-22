use crate::lowering::applied_tree::LoweringTree;
use crate::{DeclarationTerm, LoweringError, ScalarType, TypeExpression, TypeExpressionStep};
use crate::{TemplateBinder, site_at};
use omega_syntax::NodeKind;
use omega_typed::TypedTree;
use omega_typed::{Site, ValueType};

pub(super) fn type_expression(
    ty: &ValueType,
    site: &Site,
) -> Result<TypeExpression, LoweringError> {
    expression(ty, site, false)
}

pub(super) fn indexed_expression(ty: &ValueType, site: &Site) -> Result<TypeExpression, LoweringError> {
    expression(ty, site, true)
}

fn expression(ty: &ValueType, site: &Site, indexed: bool) -> Result<TypeExpression, LoweringError> {
    use TypeExpressionStep as Term;
    enum Work<'a> {
        Visit(&'a ValueType),
        Emit(Term),
    }
    let mut pending = vec![Work::Visit(ty)];
    let mut result = Vec::new();
    while let Some(item) = pending.pop() {
        let step = match item {
            Work::Emit(step) => step,
            Work::Visit(ty) => match ty {
                ValueType::IndexedInteger { signed, width } if indexed => {
                    pending.extend([Work::Emit(Term::IndexedInteger { signed: *signed }), Work::Visit(width)]);
                    continue;
                }
                ValueType::StaticInteger { carrier, value } if indexed => Term::StaticInteger {
                    carrier: crate::lowering::types::scalar_carrier(carrier).map_err(|e| LoweringError::new(site, e.to_string()))?, value: value.clone(),
                },
                ValueType::StaticFormula { carrier, term: omega_typed::IntegerIndex::Add(inputs) } if indexed => {
                    let carrier = crate::lowering::types::scalar_carrier(carrier).map_err(|e| LoweringError::new(site, e.to_string()))?;
                    for input in inputs {
                        result.push(match input {
                            omega_typed::IntegerIndex::Binder { owner, index } => Term::GenericParameter { owner: crate::identity::declaration(owner, site)?, index: *index },
                            omega_typed::IntegerIndex::Value(n) => Term::StaticInteger { carrier, value: n.clone() },
                            _ => return Err(LoweringError::new(site, "unsupported operand of a static index sum")),
                        });
                    }
                    Term::IndexSum { carrier, operands: inputs.len() }
                }
                ValueType::LevelKind => Term::LevelKind,
                ValueType::Level(n) => Term::Level(n.clone()),
                ValueType::LevelOf(inner) => {
                    pending.extend([Work::Emit(Term::LevelOf), Work::Visit(inner)]);
                    continue;
                }
                ValueType::Bool => Term::Scalar(ScalarType::Boolean),
                ValueType::Integer {
                    signed,
                    width: Some(width @ 1..),
                } => Term::Scalar(ScalarType::Integer {
                    signed: *signed,
                    width: *width,
                }),
                ValueType::IndexedArray(element, extent) => {
                    pending.extend([
                        Work::Emit(Term::IndexedArray),
                        Work::Visit(extent),
                        Work::Visit(element),
                    ]);
                    continue;
                }
                ValueType::Qualified(base, members) => {
                    let mut identities = Vec::with_capacity(members.len());
                    for member in members {
                        identities.push((crate::identity::declaration(&member.declaration, site)?, member.arguments.len()));
                    }
                    pending.push(Work::Emit(Term::Qualified(identities)));
                    for member in members.iter().rev() {
                        pending.extend(member.arguments.iter().rev().map(Work::Visit));
                    }
                    pending.push(Work::Visit(base));
                    continue;
                }
                ValueType::Nominal(declaration) => Term::Named {
                    declaration: crate::identity::declaration(declaration, site)?,
                    argument_count: 0,
                },
                ValueType::Applied(declaration, arguments) => {
                    pending.push(Work::Emit(Term::Named {
                        declaration: crate::identity::declaration(declaration, site)?,
                        argument_count: arguments.len(),
                    }));
                    pending.extend(arguments.iter().rev().map(Work::Visit));
                    continue;
                }
                ValueType::Witness { owner, index, opening: None } => Term::Witness {
                    owner: crate::identity::declaration(owner, site)?,
                    index: *index,
                },
                ValueType::Reference(access, inner) => {
                    let access = match access {
                        omega_typed::Access::Shared => crate::TemplateAccess::Shared,
                        omega_typed::Access::Mut => crate::TemplateAccess::Mut,
                        omega_typed::Access::Write => crate::TemplateAccess::Write,
                    };
                    pending.extend([Work::Emit(Term::Reference(access)), Work::Visit(inner)]);
                    continue;
                }
                ValueType::AtLifetime(inner, lifetime) => {
                    pending.extend([
                        Work::Emit(Term::AtLifetime),
                        Work::Visit(lifetime),
                        Work::Visit(inner),
                    ]);
                    continue;
                }
                ValueType::Slice(inner) => {
                    pending.extend([Work::Emit(Term::Slice), Work::Visit(inner)]);
                    continue;
                }
                ValueType::Array(inner, size) => {
                    pending.extend([Work::Emit(Term::Array(*size)), Work::Visit(inner)]);
                    continue;
                }
                ValueType::BoundSubject(slot) => Term::BoundSubject(*slot),
                ValueType::Parameter { owner, index } => Term::GenericParameter {
                    owner: crate::identity::declaration(owner, site)?,
                    index: *index,
                },
                ValueType::Universe { strict, level } => {
                    pending.extend([
                        Work::Emit(Term::Universe { strict: *strict }),
                        Work::Visit(level),
                    ]);
                    continue;
                }
                ValueType::Squash(inner) => {
                    pending.extend([Work::Emit(Term::Squash), Work::Visit(inner)]);
                    continue;
                }
                ValueType::Function(input, output) => {
                    pending.extend([
                        Work::Emit(Term::Function),
                        Work::Visit(output),
                        Work::Visit(input),
                    ]);
                    continue;
                }
                other => return Err(LoweringError::new(site, format!("unsupported type in a declaration interface {other}"))),
            },
        };
        result.push(step);
    }
    Ok(TypeExpression(result))
}

/// Retain a normalized type term or a closed static value with its carrier.
/// A declaration application preserves its ordered type and static arguments.
pub(super) fn declaration_term(
    ty: &ValueType,
    site: &Site,
) -> Result<DeclarationTerm, LoweringError> {
    match ty {
        ValueType::Applied(declaration, arguments) if arguments.iter().any(has_static_argument) => Ok(DeclarationTerm::Applied {
            declaration: crate::identity::declaration(declaration, site)?,
            arguments: arguments
                .iter()
                .map(|argument| declaration_term(argument, site))
                .collect::<Result<_, _>>()?,
        }),
        ValueType::StaticInteger { carrier, value } => match carrier.as_ref() {
            ValueType::Integer {
                signed,
                width: Some(width @ 1..),
            } => Ok(DeclarationTerm::StaticInteger {
                carrier: ScalarType::Integer {
                    signed: *signed,
                    width: *width,
                },
                value: value.clone(),
            }),
            _ => Err(LoweringError::new(site, "static argument requires a fixed-width integer carrier")),
        },
        ValueType::StaticValue {
            carrier,
            value: omega_typed::StaticValue::Bool(value),
        } if **carrier == ValueType::Bool => Ok(DeclarationTerm::StaticBoolean(*value)),
        ValueType::StaticValue { .. }
        | ValueType::StaticFormula { .. }
        | ValueType::OpenStatic { .. }
        | ValueType::PendingStatic(_)
        | ValueType::InvalidStatic(_)
        | ValueType::Exact { .. } => Err(LoweringError::new(
            site,
            format!("declaration argument {ty} requires an exact canonical static value"),
        )),
        ty => type_expression(ty, site).map(DeclarationTerm::Type),
    }
}

fn has_static_argument(ty: &ValueType) -> bool {
    match ty {
        ValueType::StaticInteger { .. }
        | ValueType::StaticValue { .. }
        | ValueType::StaticFormula { .. }
        | ValueType::OpenStatic { .. }
        | ValueType::PendingStatic(_)
        | ValueType::InvalidStatic(_)
        | ValueType::Exact { .. } => true,
        ValueType::Applied(_, arguments) => arguments.iter().any(has_static_argument),
        _ => false,
    }
}

pub(super) fn template_binders(
    params: Option<&omega_typed::GenericParameters>,
    site: &omega_typed::Site,
) -> Result<(Vec<TemplateBinder>, usize), LoweringError> {
    let Some(params) = params else {
        return Ok((vec![], 0));
    };
    let binders = params
        .parameters
        .iter()
        .map(|param| template_binder(param, site))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((binders, params.witness_count))
}

pub(super) fn template_binder(p: &omega_typed::GenericParameter, site: &Site) -> Result<TemplateBinder, LoweringError> {
    match (p.kind, p.carrier.as_deref(), p.multiplicity) {
        (NodeKind::TypeParam, None, multiplicity) => Ok(TemplateBinder::Type { multiplicity }),
        (NodeKind::LifetimeParam, None, None) => Ok(TemplateBinder::Lifetime),
        (NodeKind::KindedParam, Some(bound), None) => Ok(TemplateBinder::Kinded(type_expression(bound, site)?)),
        (NodeKind::ConstParam, Some(bound), None) => Ok(TemplateBinder::Const(type_expression(bound, site)?)),
        _ => Err(LoweringError::new(site, "unsupported checked generic binder")),
    }
}

pub(crate) fn callable_interface(
    prepared: &omega_prepared::PreparedTrees<'_>,
    tree: &TypedTree<'_>,
    node: usize,
    callable: &omega_typed::CallableType,
) -> Result<crate::TemplateCallable, LoweringError> {
    let site = &callable.contract;
    let file = tree.resolved().file;
    if callable.requirement.is_some() || file.nodes[node].kind != NodeKind::MachineType || &site_at(tree, node) != site {
        return Err(LoweringError::new(site, "callable requires its checked structural interface"));
    }
    let mut requires = Vec::new();
    let mut ensures = Vec::new();
    let owner = super::contracts::ClauseOwner::Callable(node, callable);
    let input = LoweringTree::plain(tree);
    for &member in &file.nodes[node].children {
        let clauses = match file.nodes[member].kind {
            NodeKind::Requires => &mut requires,
            NodeKind::Ensures => &mut ensures,
            // Routes, including guards over these formals, are lowered below.
            NodeKind::Params | NodeKind::Result | NodeKind::Reaches | NodeKind::Crashes | NodeKind::Blocks | NodeKind::Suspends | NodeKind::Terminates => {
                continue;
            }
            _ => return Err(LoweringError::new(&site_at(tree, member), "unsupported stored callable declaration member")),
        };
        clauses.push(super::contracts::lower_clause(prepared, &input, owner, member)?);
    }
    if (requires.is_empty(), ensures.is_empty()) != (!callable.requires, !callable.has_guarantee) {
        return Err(LoweringError::new(site, "stored callable predicates disagree with its checked interface"));
    }
    let parameters = callable
        .parameters
        .iter()
        .map(|ty| type_expression(ty, site))
        .collect::<Result<_, _>>()?;
    let result = (callable.result != ValueType::NoResult)
        .then(|| type_expression(&callable.result, site))
        .transpose()?;
    let reach = callable
        .reach
        .as_ref()
        .map(|row| {
            row.iter()
                .map(|s| {
                    Ok((
                        crate::identity::declaration(&s.service, site)?,
                        s.arguments
                            .iter()
                            .map(|arg| type_expression(arg, site))
                            .collect::<Result<Vec<_>, _>>()?,
                    ))
                })
                .collect::<Result<Vec<_>, LoweringError>>()
        })
        .transpose()?;
    let crash_routes = super::crash_routes::lower_routes(prepared, &input, owner, node, site, callable.crash_ceiling.as_deref())?;
    Ok(crate::TemplateCallable {
        parameters,
        result,
        result_lifetime_input: super::callables::result_lifetime_input(&callable.result, callable.parameters.iter()),
        requires,
        ensures,
        reach,
        crash_ceiling: callable.crash_ceiling.clone(),
        crash_routes,
        pause: callable.pause,
        terminates: callable.terminates,
        site: callable.contract.clone(),
    })
}
