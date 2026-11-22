pub(super) mod predicate_calls;

use crate::lowering::applied_tree::LoweringTree;
use num_bigint::BigInt;
use omega_prepared::{PreparedTrees, PreparedValue};
use omega_program_contracts::CollectionPrimitive;
use omega_resolved::{DeclarationId, Symbol};
use omega_syntax::NodeKind;
use omega_typed::{Access, NodeSemantics, ObligationKind, Projection, TypedSignature, ValueType};

use crate::{LoweringError, site_at};

use crate::{CallRequirement, CallSite, ContractClause, PredicateOperator, PredicateStep};
pub(crate) fn call_requirements(
    site: CallSite,
    count: usize,
) -> Vec<CallRequirement> {
    (0..count)
        .map(|clause| CallRequirement { site, clause })
        .collect()
}

#[derive(Clone, Copy)]
pub(crate) enum ClauseOwner<'a> {
    Machine(&'a TypedSignature),
    Callable(usize, &'a omega_typed::CallableType),
    Subject(&'a ValueType),
    Template(&'a DeclarationId),
    Domain(usize, &'a ValueType, &'a [Option<&'a ValueType>]),
    IndexedData(usize, &'a DeclarationId, &'a [&'a ValueType]),
    Requirement(&'a TypedSignature, &'a [&'a ValueType]),
    /// A proof-only theorem's clauses, or with a scope the clauses of that hypothesis.
    Theorem(&'a TypedSignature, &'a [super::hypotheses::Bound<'a>], Option<usize>),
}

pub(crate) fn lower_clause(
    prepared: &PreparedTrees<'_>,
    tree: &LoweringTree<'_>,
    owner: ClauseOwner<'_>,
    clause: usize,
) -> Result<ContractClause, LoweringError> {
    let file = tree.resolved().file;
    let site = site_at(tree, clause);
    let node = &file.nodes[clause];
    let [root] = *node.children.as_slice() else {
        return Err(LoweringError::new(&site, "contract clause has no single predicate"));
    };
    if tree.semantics_at(clause).next().is_some() {
        return Err(LoweringError::new(&site, "unsupported contract clause record"));
    }
    lower_predicate(prepared, tree, owner, root, site, PredicateRole::clause(node.kind))
}

/// Which typed proposition a predicate root carries. A crash guard is a total
/// entry fact owing exactly its `TotalCrashGuard` record; it cannot name `result`.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum PredicateRole {
    Requirement,
    Guarantee,
    CrashGuard,
}

impl PredicateRole {
    fn clause(kind: NodeKind) -> Self {
        if kind == NodeKind::Ensures { Self::Guarantee } else { Self::Requirement }
    }
}

pub(crate) fn lower_predicate(
    prepared: &PreparedTrees<'_>,
    tree: &LoweringTree<'_>,
    owner: ClauseOwner<'_>,
    root: usize,
    site: omega_typed::Site,
    role: PredicateRole,
) -> Result<ContractClause, LoweringError> {
    let file = tree.resolved().file;
    let signature = match owner {
        ClauseOwner::Machine(signature) | ClauseOwner::Requirement(signature, _) | ClauseOwner::Theorem(signature, ..) => Some(signature),
        ClauseOwner::Callable(..) | ClauseOwner::Domain(..) | ClauseOwner::Subject(_) | ClauseOwner::Template(_) | ClauseOwner::IndexedData(..) => None,
    };
    let node_is_guarantee = role == PredicateRole::Guarantee;
    let guarantee = signature.filter(|_| node_is_guarantee && super::hypotheses::proves_guarantee(owner));
    let hypothesis_result = match (role, owner) {
        (PredicateRole::Guarantee, ClauseOwner::Theorem(_, bounds, Some(slot))) => bounds.get(slot).and_then(|bound| bound.result()).map(|ty| (slot, ty)),
        _ => None,
    };
    let mut predicate = Vec::new();
    let mut work = vec![Step::Visit {
        node: root,
        ty: &ValueType::Bool,
        inherited: None,
    }];
    while let Some(step) = work.pop() {
        let (node, ty, inherited) = match step {
            Step::Emit(term) => {
                predicate.push(term);
                continue;
            }
            Step::Visit { node, ty, inherited } => (node, ty, inherited),
        };
        let current = &file.nodes[node];
        let site = site_at(tree, node);
        let unsupported = |reason| LoweringError::new(&site, reason);
        // A float literal or anonymous number landed in `f32` or `f64` is the
        // term preparation rounded; the predicate never rounds it again.
        if inherited.is_none()
            && let Some(bits) = crate::machines::arithmetic::folded_float(tree, prepared, node, ty)?
        {
            predicate.push(PredicateStep::IeeeFloat(bits));
            continue;
        }
        let selected_operator = matches!(current.kind, NodeKind::Binary | NodeKind::Unary)
            && tree.semantics_at(node).any(|row| matches!(row, NodeSemantics::Call(call) if call.use_site == omega_typed::CallUse::Fact));
        if !selected_operator && matches!(current.kind, NodeKind::Binary | NodeKind::Unary)
            && let Some(value) = crate::machines::arithmetic::folded_exact_operator(tree, node, ty, inherited)?
        {
            predicate.push(PredicateStep::Integer(value.clone()));
            continue;
        }
        let mathematical_call = current.kind == NodeKind::Call
            && tree.semantics_at(node).any(|row| {
                matches!(row, NodeSemantics::Obligation(o) if matches!(o.kind, ObligationKind::MathematicalApplication { .. }))
            });
        let squash_fact = node == root && current.kind == NodeKind::PathExpression
            && matches!(tree.resolved().symbol(node), Some(Symbol::Builtin(name)) if name == "core\\Squash")
            && super::hypotheses::strict_fact(owner, tree, node);
        if mathematical_call || squash_fact {
            let signature = signature.ok_or_else(|| unsupported("mathematical contract application requires machine formals"))?;
            let params = file
                .child(signature.node.node, NodeKind::Params)
                .ok_or_else(|| unsupported("mathematical contract application lacks its formals"))?;
            if !tree
                .semantics_at(node)
                .any(|row| matches!(row,NodeSemantics::Expression(e) if &e.ty==ty))
                && !(node == root && super::hypotheses::strict_fact(owner, tree, node))
            {
                return Err(unsupported("mathematical contract application has an incompatible result"));
            }
            let formals = &file.nodes[params].children;
            let scope = match owner {
                ClauseOwner::Theorem(_, bounds, Some(slot)) => super::mathematics::TermScope::Hypothesis {
                    theorem: formals,
                    bounds,
                    slot,
                    result: hypothesis_result.map(|(_, ty)| ty),
                },
                _ => super::mathematics::TermScope::Parameters(formals),
            };
            let term = super::mathematics::lower_term(tree, node, scope, guarantee.filter(|_| node == root).map(|sig| &sig.machine))?;
            predicate.push(PredicateStep::MathematicalTerm(term));
            continue;
        }
        if selected_operator || current.kind == NodeKind::Call {
            let proved = guarantee.filter(|_| node == root).map(|sig| &sig.machine);
            let projection = if selected_operator { None } else { super::hypotheses::guarantee(tree, owner, node, ty, proved)? };
            let application = match projection {
                None if tree.semantics_at(node).any(|row| matches!(row, NodeSemantics::Call(c) if c.use_site == omega_typed::CallUse::Fact)) => {
                    Some(predicate_calls::application(prepared, tree, owner, node, ty, proved, role == PredicateRole::CrashGuard && node == root)?)
                }
                hypothesis => hypothesis,
            };
            if let Some((step, operands)) = application {
                work.push(Step::Emit(step));
                for (node, ty) in operands.into_iter().rev() {
                    work.push(Step::Visit { node, ty, inherited: None });
                }
                continue;
            }
        }
        if current.kind == NodeKind::Index {
            index_terms(prepared, tree, node, ty, inherited, guarantee.filter(|_| node == root).map(|sig| &sig.machine), &mut work)?;
            continue;
        }
        let closed_integer = match (owner, tree.resolved().symbol(node)) {
            (ClauseOwner::Machine(_), Some(Symbol::Binding { kind: NodeKind::ConstParam, binder_node, .. })) => tree.integer_binding(*binder_node),
            _ => None,
        };
        let static_slot = match (owner, tree.resolved().symbol(node)) {
            (
                ClauseOwner::Domain(scope, _, _),
                Some(Symbol::Binding {
                    kind: NodeKind::ConstParam,
                    binder_node,
                    ..
                }),
            ) => file
                .generic_parameters(scope)
                .position(|(_, binder, _)| binder == *binder_node),
            _ => closed_integer.map(|(slot, _)| slot),
        };
        let semantic = records(
            tree,
            node,
            ty,
            guarantee.filter(|_| node == root).map(|sig| &sig.machine),
            static_slot,
            role == PredicateRole::CrashGuard && node == root,
        )?;
        if inherited
            .zip(semantic.landing)
            .is_some_and(|(outer, inner)| outer != inner)
        {
            return Err(unsupported("contract literal has conflicting integer landings"));
        }
        let landing = semantic.landing.or(inherited);

        match (current.kind, current.children.as_slice()) {
            (NodeKind::Binary, [subject, _]) if let Some(test) = semantic.membership => {
                let domain = &test.domain.declaration;
                let declaration = crate::identity::declaration(domain, &site)?;
                let typed = prepared.typed();
                // A domain test names a packaged, parameterless, non-alias domain
                // whose carrier is exactly the tested subject's type, on a scalar
                // (`volume in u8::Audible`) or record (`track in Audio::Audible`).
                // Anything else is refused rather than published unchecked.
                if domain.kind != NodeKind::Case
                    && (typed.domain_carriers.get(domain) != Some(&test.subject)
                        || typed.generic_parameters.get(domain).is_some_and(|g| !g.parameters.is_empty())
                        || typed.domain_alias_members.contains_key(domain))
                {
                    return Err(unsupported("membership needs its packaged domain and exact subject"));
                }
                work.push(Step::Emit(if domain.kind == NodeKind::Case {
                    PredicateStep::IsCase(declaration)
                } else {
                    PredicateStep::InDomain(omega_psi::DomainAtom {
                        declaration,
                        arguments: test
                            .domain
                            .arguments
                            .iter()
                            .map(|argument| super::type_expression::declaration_term(argument, &site))
                            .collect::<Result<_, _>>()?,
                    })
                }));
                work.push(Step::Visit {
                    node: *subject,
                    ty: &test.subject,
                    inherited: None,
                });
            }
            (NodeKind::Binary | NodeKind::Unary, operands) if semantic.constant.is_none() && landing.is_none() => {
                let Some(operation) = semantic.operation else {
                    return Err(unsupported("contract predicate has no selected primitive"));
                };
                if ty == &ValueType::Bool && let Some(value) = prepared.exact_comparison(tree, node) {
                    predicate.push(PredicateStep::Boolean(value));
                    continue;
                }
                if operation.token == "type ==" && matches!(owner, ClauseOwner::Template(_)) {
                    let [left, right] = operation.operands.as_slice() else {
                        return Err(unsupported("type equality requires two checked type operands"));
                    };
                    predicate.push(PredicateStep::TypesEqual {
                        left: super::type_expression::type_expression(left, &site)?,
                        right: super::type_expression::type_expression(right, &site)?,
                    });
                    continue;
                }
                if let Some((comparison, reversed)) = ieee_comparison(&operation.token, &operation.operands).filter(|_| operands.len() == 2) {
                    // The proposition is typed, not an executable operation:
                    // `a > b` is `LessThan` of `b` then `a`, identical under IEEE.
                    work.push(Step::Emit(comparison));
                    let mut ordered: Vec<_> = operands.iter().zip(&operation.operands).collect();
                    if reversed {
                        ordered.reverse();
                    }
                    for (&operand, carrier) in ordered.into_iter().rev() {
                        work.push(Step::Visit {
                            node: operand,
                            ty: carrier,
                            inherited: None,
                        });
                    }
                    continue;
                }
                if operands.len() != operation.operands.len() {
                    return Err(unsupported("contract predicate operator has the wrong operand count"));
                }
                let term = match semantic.float_arithmetic {
                    Some((operation, format)) => PredicateStep::IeeeArithmetic {
                        operation,
                        semantics: crate::FloatSemantics::Ieee754NearestEvenV1,
                        format,
                    },
                    None => PredicateStep::Apply(predicate_operator(&operation.token, &operation.operands)
                        .ok_or_else(|| unsupported("unsupported contract predicate operator"))?),
                };
                work.push(Step::Emit(term));
                for (&operand, carrier) in operands.iter().zip(&operation.operands).rev() {
                    work.push(Step::Visit {
                        node: operand,
                        ty: carrier,
                        inherited: None,
                    });
                }
            }
            (NodeKind::FieldAccess, [base]) if semantic.constant.is_none() && semantic.field.is_none() && landing.is_none() && semantic.operation.is_some() => {
                // Element views keep their own element-count step: their `.length`
                // is never a byte count or a fixed extent.
                let (receiver, length) = semantic
                    .operation
                    .and_then(|operation| {
                        crate::machines::access::collection_length(operation)
                            .map(|(receiver, length)| {
                                (
                                    receiver,
                                    match length {
                                        crate::machines::access::CollectionLength::Fixed(_) => PredicateStep::FixedArrayLength,
                                        crate::machines::access::CollectionLength::Bytes => PredicateStep::ByteSequenceLength,
                                    },
                                )
                            })
                            .or_else(|| {
                                crate::machines::access::element_view_length(operation).map(|receiver| (receiver, PredicateStep::ElementViewLength))
                            })
                    })
                    .ok_or_else(|| unsupported("unsupported contract predicate expression"))?;
                work.push(Step::Emit(length));
                work.push(Step::Visit {
                    node: *base,
                    ty: receiver,
                    inherited: None,
                });
            }
            (NodeKind::FieldAccess, [base]) if semantic.operation.is_none() && semantic.constant.is_none() && landing.is_none() => {
                let field = semantic
                    .field
                    .ok_or_else(|| unsupported("contract field has no selected declaration"))?;
                let receiver = tree
                    .semantics_at(*base)
                    .find_map(|record| match record {
                        NodeSemantics::Expression(expression) if expression.ty.nominal_parts().is_some() => Some(&expression.ty),
                        _ => None,
                    })
                    .ok_or_else(|| unsupported("contract field requires a nominal value receiver"))?;
                work.push(Step::Emit(field_segment(prepared, tree, node, *base, receiver, field, ty)?));
                work.push(Step::Visit {
                    node: *base,
                    ty: receiver,
                    inherited: None,
                });
            }
            (NodeKind::Cast, [operand, ..])
                if semantic.constant.is_none()
                    && landing.is_none()
                    && let Some(operation) = semantic.operation =>
            {
                // A cast denotes its operand's value: `-D` discards no fact and `=D`
                // keeps the domain. Removing or keeping domains on the same carrier
                // therefore reads the operand's own term. Adding a domain would owe
                // its establishment at this occurrence, and a carrier change owes
                // an exact conversion; a predicate step can state neither.
                let [source] = operation.operands.as_slice() else {
                    return Err(unsupported("contract predicate cast requires one source value"));
                };
                let target = &operation.result;
                if operation.token != "as"
                    || matches!(source, ValueType::Reference(..))
                    || source.carrier() != target.carrier()
                    || target
                        .domains()
                        .iter()
                        .any(|atom| !source.domains().contains(atom))
                {
                    return Err(unsupported("a contract predicate cast can only keep or remove domains on the same carrier"));
                }
                work.push(Step::Visit {
                    node: *operand,
                    ty: source,
                    inherited: None,
                });
            }
            (NodeKind::Paren, [inner]) if semantic.operation.is_none() && semantic.constant.is_none() => {
                work.push(Step::Visit {
                    node: *inner,
                    ty,
                    inherited: landing,
                });
            }
            (NodeKind::Literal, []) if semantic.operation.is_none() && semantic.constant.is_none() => {
                let literal = match ty {
                    ValueType::Exact {
                        value,
                        fractional_origin: None,
                    } if value.is_integer() => value.numer(),
                    _ => landing.ok_or_else(|| unsupported("contract literal has no integer landing"))?,
                };
                predicate.push(PredicateStep::Integer(literal.clone()));
            }
            (NodeKind::PathExpression, []) if semantic.operation.is_none() && landing.is_none() => {
                let resolved = tree.resolved();
                let term = match resolved.symbol(node) {
                    Some(Symbol::Binding { kind: NodeKind::ConstParam, .. }) if semantic.constant.is_none()
                        && let Some((_, ValueType::StaticInteger { carrier, value })) = closed_integer && carrier.as_ref() == ty => {
                        PredicateStep::Integer(value.clone())
                    }
                    Some(Symbol::Binding { kind: NodeKind::Param, .. }) if semantic.constant.is_none() && matches!(owner, ClauseOwner::Theorem(..)) => {
                        super::hypotheses::parameter(owner, tree, node, ty).ok_or_else(|| unsupported("contract binding is not a formal of this theorem or hypothesis"))?
                    }
                    Some(Symbol::Binding {
                        span, kind: NodeKind::Param, ..
                    }) if semantic.constant.is_none() => {
                        let formal = match owner {
                            ClauseOwner::Callable(node, callable) => file
                                .parameters(node)
                                .zip(&callable.parameters)
                                .position(|((name, _), carrier)| name.span == *span && carrier == ty),
                            _ => signature.and_then(|sig| {
                                sig.parameters
                                    .iter()
                                    .position(|(candidate, carrier)| candidate == span && carrier == ty)
                            }),
                        }
                        .filter(|_| {
                            is_contract_operand(ty)
                                || crate::lowering::types::byte_view_access(ty).is_some()
                                || crate::lowering::types::element_view_access(ty).is_some()
                        })
                        .ok_or_else(|| unsupported("contract binding is not a supported formal of this machine"))?;
                        PredicateStep::Formal(formal)
                    }
                    Some(Symbol::Binding {
                        binder_node,
                        kind: NodeKind::ConstParam | NodeKind::KindedParam,
                        ..
                    }) if matches!(owner, ClauseOwner::Domain(..)) && semantic.constant.is_none() => {
                        let ClauseOwner::Domain(node, _, indices) = owner else { unreachable!() };
                        let position = file
                            .generic_parameters(node)
                            .position(|(_, binder, _)| binder == *binder_node)
                            .filter(|&index| indices.get(index) == Some(&Some(ty)))
                            .ok_or_else(|| unsupported("domain index disagrees with its declared carrier"))?;
                        PredicateStep::ValueIndex(position)
                    }
                    Some(Symbol::Binding {
                        binder_node,
                        kind: NodeKind::KindedParam,
                        ..
                    }) if semantic.constant.is_none() => {
                        let (node, indices) = match owner {
                            ClauseOwner::Requirement(signature, indices) => (signature.node.node, indices),
                            ClauseOwner::IndexedData(node, _, indices) => (node, indices),
                            _ => return Err(unsupported("value index has no declaration owner")),
                        };
                        let position = file
                            .generic_parameters(node)
                            .position(|(_, binder, _)| binder == *binder_node)
                            .filter(|&index| indices.get(index) == Some(&ty))
                            .ok_or_else(|| unsupported("predicate value index disagrees with its declared carrier"))?;
                        PredicateStep::ValueIndex(position)
                    }
                    Some(Symbol::Declaration(field)) if field.kind == NodeKind::Field && semantic.constant.is_none() => {
                        let subject = match owner {
                            ClauseOwner::IndexedData(_, owner, _) | ClauseOwner::Template(owner) => Some(owner),
                            ClauseOwner::Subject(ty) => ty.nominal_parts().map(|(owner, _)| owner),
                            _ => None,
                        };
                        if !subject
                            .and_then(|id| prepared.typed().record_fields.get(id))
                            .is_some_and(|fields| fields.iter().any(|f| &f.declaration == field && &f.ty == ty))
                        {
                            return Err(unsupported("contract field does not belong to this data declaration"));
                        }
                        predicate.push(PredicateStep::SelfValue);
                        PredicateStep::Field(crate::identity::declaration(field, &site)?)
                    }
                    Some(Symbol::Builtin(name)) if name == "result" && semantic.constant.is_none()
                        && let Some((slot, _)) = hypothesis_result.filter(|(_, carrier)| *carrier == ty) =>
                    {
                        PredicateStep::MathematicalTerm(vec![crate::MathematicalStep::HypothesisResult(slot)])
                    }
                    Some(Symbol::Builtin(name))
                        if name == "result"
                            && (guarantee.is_some_and(|sig| &sig.result == ty)
                                || (node_is_guarantee && matches!(owner,ClauseOwner::Callable(_,c) if &c.result==ty)))
                            && semantic.constant.is_none()
                            && is_contract_operand(ty) =>
                    {
                        PredicateStep::Result
                    }
                    Some(Symbol::Builtin(name))
                        if name == "self"
                            && (matches!(owner,ClauseOwner::Subject(carrier) | ClauseOwner::Domain(_,carrier,_) if carrier == ty)
                                || matches!(owner,ClauseOwner::Template(id) | ClauseOwner::IndexedData(_,id,_) if ty.nominal_parts().is_some_and(|(o,_)|o==id)))
                            && semantic.constant.is_none() =>
                    {
                        PredicateStep::SelfValue
                    }
                    Some(Symbol::Builtin(name)) if ty == &ValueType::Bool && matches!(name.as_str(), "true" | "false") && semantic.constant.is_none() => {
                        PredicateStep::Boolean(name == "true")
                    }
                    Some(Symbol::Declaration(constant)) if semantic.constant == Some(constant) => match prepared.constant(constant) {
                        Some(PreparedValue::Bool(value)) if ty == &ValueType::Bool => PredicateStep::Boolean(*value),
                        Some(PreparedValue::Integer(value)) if ty.contains_integer(value) => PredicateStep::Integer(value.clone()),
                        Some(PreparedValue::Float {
                            value: omega_prepared::PreparedFloat::Binary32(bits),
                            ..
                        }) if ty == &ValueType::Float32 => PredicateStep::IeeeFloat(crate::IeeeFloatBits::Binary32(*bits)),
                        Some(PreparedValue::Float {
                            value: omega_prepared::PreparedFloat::Binary64(bits),
                            ..
                        }) if ty == &ValueType::Float64 => PredicateStep::IeeeFloat(crate::IeeeFloatBits::Binary64(*bits)),
                        // A NaN computed without exact bits has no term to state.
                        Some(PreparedValue::Float {
                            value: omega_prepared::PreparedFloat::UnspecifiedNaN(_),
                            ..
                        }) => return Err(unsupported("an unspecified NaN constant has no exact bits to materialize")),
                        _ => return Err(unsupported("contract constant has no matching frozen scalar")),
                    },
                    _ => return Err(unsupported("unsupported contract subject")),
                };
                predicate.push(term);
            }
            _ => return Err(unsupported("unsupported contract predicate expression")),
        }
    }

    Ok(ContractClause { predicate, site })
}

/// The typed place step of a contract selection must name this exact receiver
/// node, without a reference crossing, and the expected projection.
fn selects_from(
    tree: &LoweringTree<'_>,
    step: &omega_typed::PlaceStep,
    base: usize,
    projection: &Projection,
) -> bool {
    let file = tree.resolved().file;
    (step.base == file.node_id(base) || step.base == file.node_id(file.ungroup(base))) && step.through.is_none() && step.projection.as_ref() == Some(projection)
}

/// Classify one selected field by the typed roster that owns it. Record fields
/// stay single segments; a payload field also retains its owning case. The
/// case is a path segment only and never stands for a membership test.
pub(super) fn field_segment(
    prepared: &PreparedTrees<'_>,
    tree: &LoweringTree<'_>,
    node: usize,
    base: usize,
    receiver: &ValueType,
    field: &DeclarationId,
    leaf: &ValueType,
) -> Result<PredicateStep, LoweringError> {
    let file = tree.resolved().file;
    let site = site_at(tree, node);
    let refuse = |message: &str| LoweringError::new(&site, message);
    let typed = prepared.typed();
    // The clause can belong to a library file, such as the toolchain's
    // `BoundaryReach`. Its place steps are that forest's, keyed by its nodes.
    let (_, path) = tree.forest().place_path(file.node_id(node));
    let projection = Projection::Field(Box::new(field.clone()));
    if path
        .last()
        .is_none_or(|step| !selects_from(tree, step, base, &projection))
    {
        return Err(refuse("contract field disagrees with its typed place projection"));
    }
    let Some((owner, _)) = receiver.nominal_parts() else {
        return Err(refuse("contract field requires a nominal value receiver"));
    };
    let (declarations, roster) = std::iter::once(typed).chain(typed.library)
        .find_map(|forest| forest.record_fields.get(owner).map(|fields| (forest, fields)))
        .ok_or_else(|| refuse("contract field receiver has no typed field roster"))?;
    // The declaration identifies each slot. A concrete application can give
    // that slot a different carrier from the template, e.g. Pair<u8>::left.
    let shape = typed.static_data_shapes.get(receiver).or_else(|| declarations.static_data_shapes.get(receiver));
    let matches_leaf = |fields: &[omega_typed::TypedField], index: usize, case| {
        match shape {
            Some(shape) => shape.active_fields(case).and_then(|fields| fields.get(index)) == Some(leaf),
            None => fields.get(index).is_some_and(|field| &field.ty == leaf),
        }
    };
    if let Some(index) = roster.iter().position(|row| &row.declaration == field) {
        if !matches_leaf(roster, index, None) {
            return Err(refuse("contract field disagrees with its declared carrier"));
        }
        return Ok(PredicateStep::Field(crate::identity::declaration(field, &site)?));
    }
    if !roster.is_empty() {
        return Err(refuse("contract field does not belong to its receiver"));
    }
    // A payload field belongs to exactly one case roster. Its case must be a
    // declared case of this receiver by the typed owner relation, never by
    // spelling; the active-case guard is not consulted.
    let mut owners = declarations
        .record_fields
        .iter()
        .filter(|(case, rows)| case.kind == NodeKind::Case && rows.iter().any(|row| &row.declaration == field));
    let (Some((case, payload)), None) = (owners.next(), owners.next()) else {
        return Err(refuse("contract payload field has no unique declared case"));
    };
    let listed = shape.is_none_or(|shape| shape.cases.iter().any(|(id, _)| id == case));
    if !listed || declarations.case_owners.get(case) != Some(owner) {
        return Err(refuse("contract payload case does not belong to its receiver"));
    }
    if payload
        .iter()
        .position(|row| &row.declaration == field)
        .is_none_or(|index| !matches_leaf(payload, index, Some(case)))
    {
        return Err(refuse("contract payload field disagrees with its declared carrier"));
    }
    Ok(PredicateStep::CasePayload {
        case: crate::identity::declaration(case, &site)?,
        field: crate::identity::declaration(field, &site)?,
    })
}

/// An element of an owned fixed array. Both forms require the typed core
/// selection, its shared borrow and the indexing precondition. A frozen
/// selector becomes a `FixedIndex` path segment that keeps the exact index even
/// past the declared extent: the lowered-form checker owns the read's bounds
/// precondition and compares the index with the extent in the receiver's Psi
/// type, so lowering never redirects or drops an out-of-bounds selector. A
/// runtime selector is never a path segment; it becomes a `FixedArrayRead` of
/// the receiver and selector terms whose contract the checker instantiates.
fn index_terms<'a>(
    prepared: &PreparedTrees<'_>,
    tree: &'a LoweringTree<'_>,
    node: usize,
    element: &ValueType,
    inherited: Option<&BigInt>,
    guarantee: Option<&DeclarationId>,
    work: &mut Vec<Step<'a>>,
) -> Result<(), LoweringError> {
    let file = tree.resolved().file;
    let site = site_at(tree, node);
    let refuse = |message: &str| LoweringError::new(&site, message);
    if inherited.is_some() {
        return Err(refuse("contract index element has an integer landing"));
    }
    let [base, selector] = *file.nodes[node].children.as_slice() else {
        return Err(refuse("contract index has no exact receiver and selector"));
    };
    let receiver = tree
        .semantics_at(base)
        .find_map(|record| match record {
            NodeSemantics::Expression(expression) if expression.static_argument_slot.is_none() => Some(&expression.ty),
            _ => None,
        })
        .ok_or_else(|| refuse("contract index has no typed receiver"))?;
    let ValueType::Array(inner, _) = receiver else {
        return Err(refuse("contract index requires an owned fixed-array receiver"));
    };
    if inner.as_ref() != element {
        return Err(refuse("contract index element disagrees with its receiver"));
    }
    let base_site = site_at(tree, base);
    let mut access = None;
    let mut precondition = None;
    let mut borrow = false;
    let mut typed = false;
    let mut obligation = false;
    for record in tree.semantics_at(node) {
        match record {
            NodeSemantics::CollectionAccess(selected)
                if access.is_none()
                    && selected.node == file.node_id(node)
                    && selected.site == site
                    && selected.primitive == CollectionPrimitive::Read
                    && &selected.element == element
                    && &selected.result == element
                    && selected.selector_carriers == [crate::machines::access::INDEX_TYPE] =>
            {
                access = Some(selected)
            }
            NodeSemantics::Obligation(row) if row.node == file.node_id(node) && row.site == site => match &row.kind {
                ObligationKind::CallPrecondition(machine) if precondition.is_none() => precondition = Some(machine),
                ObligationKind::Borrow { access: Access::Shared, place } if !borrow && place == &base_site => borrow = true,
                ObligationKind::ProveGuarantee { machine, predicate } if !obligation && Some(machine) == guarantee && predicate == &site => obligation = true,
                _ => return Err(refuse("contract index has an unsupported obligation")),
            },
            NodeSemantics::Expression(expression) if expression.static_argument_slot.is_none() && &expression.ty == element => typed = true,
            _ => return Err(refuse("unsupported typed contract index record")),
        }
    }
    let Some(selected) = access.filter(|s| typed && borrow && obligation == guarantee.is_some() && precondition == Some(&s.machine)) else {
        return Err(refuse("contract index has no complete typed read and access obligation"));
    };
    let (_, path) = tree.forest().place_path(file.node_id(node));
    let projection = Projection::Index(file.node_id(selector));
    if path
        .last()
        .is_none_or(|step| !selects_from(tree, step, base, &projection))
    {
        return Err(refuse("contract index disagrees with its typed place projection"));
    }
    if file.nodes[selector].kind == NodeKind::Literal || tree.semantics_at(selector).any(|row| matches!(row, NodeSemantics::Constant(_))) {
        let value = fixed_selector(prepared, tree, selector)?;
        let index = u64::try_from(value).map_err(|_| refuse("contract index has no u64 selector value"))?;
        work.push(Step::Emit(PredicateStep::FixedIndex(index)));
    } else {
        work.push(Step::Emit(PredicateStep::FixedArrayRead {
            callee: crate::identity::declaration(&selected.machine, &site)?,
            site,
        }));
        work.push(Step::Visit { node: selector, ty: &selected.selector_carriers[0], inherited: None });
    }
    work.push(Step::Visit { node: base, ty: receiver, inherited: None });
    Ok(())
}

/// A literal or frozen constant selector. Every value record on the selector
/// must agree; a computed or runtime selector forms no fixed index.
fn fixed_selector<'a>(
    prepared: &'a PreparedTrees<'_>,
    tree: &'a LoweringTree<'_>,
    selector: usize,
) -> Result<&'a BigInt, LoweringError> {
    let file = tree.resolved().file;
    let site = site_at(tree, selector);
    let refuse = || LoweringError::new(&site, "contract index requires a fixed selector");
    let kind = file.nodes[selector].kind;
    if !matches!(kind, NodeKind::Literal | NodeKind::PathExpression) {
        return Err(refuse());
    }
    let mut typed = false;
    let mut values: Vec<&BigInt> = Vec::new();
    for record in tree.semantics_at(selector) {
        match record {
            NodeSemantics::Expression(expression) if expression.static_argument_slot.is_none() => match &expression.ty {
                ty if ty == &crate::machines::access::INDEX_TYPE => typed = true,
                ValueType::Exact {
                    value,
                    fractional_origin: None,
                } if kind == NodeKind::Literal && value.is_integer() => {
                    typed = true;
                    values.push(value.numer());
                }
                _ => return Err(refuse()),
            },
            NodeSemantics::Landing(landing) if landing.ty == crate::machines::access::INDEX_TYPE && landing.value.is_integer() => {
                values.push(landing.value.numer())
            }
            NodeSemantics::Constant(constant)
                if kind == NodeKind::PathExpression && matches!(tree.resolved().symbol(selector), Some(Symbol::Declaration(id)) if id == constant) =>
            {
                match prepared.constant(constant) {
                    Some(PreparedValue::Integer(value)) => values.push(value),
                    _ => return Err(refuse()),
                }
            }
            _ => return Err(refuse()),
        }
    }
    match values.split_first() {
        Some((first, rest)) if typed && rest.iter().all(|value| value == first) => Ok(first),
        _ => Err(refuse()),
    }
}

enum Step<'a> {
    Visit {
        node: usize,
        ty: &'a ValueType,
        inherited: Option<&'a BigInt>,
    },
    Emit(PredicateStep),
}

#[derive(Default)]
struct Records<'a> {
    /// A case test or a domain test such as `track in Audio::Audible`.
    membership: Option<&'a omega_typed::TypedMembership>,
    field: Option<&'a DeclarationId>,
    operation: Option<&'a omega_typed::TypedOperation>,
    float_arithmetic: Option<(crate::FloatOperation, crate::IeeeFloatFormat)>,
    constant: Option<&'a DeclarationId>,
    landing: Option<&'a BigInt>,
}

fn records<'a>(
    tree: &'a LoweringTree<'_>,
    node: usize,
    carrier: &ValueType,
    guarantee: Option<&DeclarationId>,
    static_slot: Option<usize>,
    crash_guard: bool,
) -> Result<Records<'a>, LoweringError> {
    let site = site_at(tree, node);
    let kind = tree.resolved().file.nodes[node].kind;
    let mut result = Records::default();
    let mut typed = false;
    let mut obligation = false;
    let mut total_guard = false;
    let mut float_contract = None;
    let mut unsupported = false;
    for record in tree.semantics_at(node) {
        match record {
            NodeSemantics::Expression(e)
                if e.static_argument_slot == static_slot
                    && (&e.ty == carrier
                        || (matches!(carrier, ValueType::Integer { .. })
                            && matches!(kind, NodeKind::Paren | NodeKind::Literal)
                            && is_exact_integer(&e.ty))) =>
            {
                typed = true;
            }
            NodeSemantics::Membership(test)
                if kind == NodeKind::Binary
                    && carrier == &ValueType::Bool
                    && match test.domain.declaration.kind {
                        NodeKind::Case => test.domain.arguments.is_empty(),
                        kind => kind == NodeKind::Domain,
                    }
                    && result.membership.is_none() =>
            {
                result.membership = Some(test)
            }
            NodeSemantics::Field(field) if kind == NodeKind::FieldAccess && result.field.is_none() => result.field = Some(field),
            NodeSemantics::Operation(op) if result.operation.is_none() => result.operation = Some(op),
            NodeSemantics::Constant(constant) if result.constant.is_none() => result.constant = Some(constant),
            NodeSemantics::Landing(landing)
                if &landing.ty == carrier
                    && landing.value.is_integer()
                    && carrier.contains_integer(landing.value.numer())
                    && result
                        .landing
                        .is_none_or(|previous| previous == landing.value.numer()) =>
            {
                result.landing = Some(landing.value.numer());
            }
            NodeSemantics::Obligation(row)
                if !obligation
                    && matches!(
                        &row.kind,
                        ObligationKind::ProveGuarantee { machine, predicate } if Some(machine) == guarantee && predicate == &site
                    ) =>
            {
                obligation = true;
            }
            NodeSemantics::Obligation(row)
                if matches!(carrier, ValueType::Float32 | ValueType::Float64)
                    && kind == NodeKind::Binary && float_contract.is_none()
                    && matches!(row.kind, ObligationKind::OperationContract { .. }) => float_contract = Some(row),
            NodeSemantics::Obligation(row) if crash_guard && !total_guard && matches!(row.kind, ObligationKind::TotalCrashGuard) && row.site == site => {
                total_guard = true;
            }
            NodeSemantics::Obligation(row) if matches!(row.kind, ObligationKind::FormSquash { .. }) => {
                return Err(LoweringError::new(&site, "contract squash requires a strict theorem clause"));
            }
            NodeSemantics::Call(call) => {
                return Err(LoweringError::new(
                    &site,
                    format!("unsupported machine call `{}` in a Psi contract predicate", call.machine.path),
                ));
            }
            // A specific unsupported construct can follow its expression type at the same site.
            _ => unsupported = true,
        }
    }
    if unsupported {
        return Err(LoweringError::new(&site, "unsupported typed contract predicate record"));
    }
    if !typed || obligation != guarantee.is_some() || total_guard != crash_guard {
        return Err(LoweringError::new(&site, "contract predicate has incomplete typing or guarantee records"));
    }
    if result.operation.is_some_and(|op| &op.result != carrier) {
        return Err(LoweringError::new(&site, "unsupported typed contract predicate record"));
    }
    if let Some(operation) = result.operation
        && matches!(kind, NodeKind::Binary | NodeKind::Unary)
        && let Ok(crate::ScalarType::IeeeFloat(format)) = crate::lowering::types::stored_scalar(carrier)
    {
        let primitive = crate::machines::arithmetic::lower_float(operation, float_contract.as_slice())?;
        result.float_arithmetic = Some((primitive, format));
    } else if float_contract.is_some() {
        return Err(LoweringError::new(&site, "float predicate formation lacks its selected operation"));
    }
    Ok(result)
}

/// A qualified scalar formal or result names its own value, as its entry and
/// result domain predicates already do. Typing admits it only where a
/// qualified operand is accepted, such as a cast that removes its domains.
fn is_contract_operand(ty: &ValueType) -> bool {
    crate::lowering::types::uses_structural_place(ty) || crate::lowering::types::stored_scalar(ty.carrier()).is_ok()
}

/// The builtin IEEE comparison typing selected for two unqualified operands of
/// one format, and whether its canonical kind reads them in reverse order. A
/// user operator is a call, never this; integer order laws never apply here.
fn ieee_comparison(
    token: &str,
    operands: &[ValueType],
) -> Option<(PredicateStep, bool)> {
    use crate::IeeeComparisonKind::*;
    let format = match operands {
        [ValueType::Float32, ValueType::Float32] => crate::IeeeFloatFormat::Binary32,
        [ValueType::Float64, ValueType::Float64] => crate::IeeeFloatFormat::Binary64,
        _ => return None,
    };
    let (kind, reversed) = match token {
        "==" => (Equal, false),
        "!=" => (NotEqual, false),
        "<" => (LessThan, false),
        "<=" => (LessOrEqual, false),
        ">" => (LessThan, true),
        ">=" => (LessOrEqual, true),
        _ => return None,
    };
    Some((PredicateStep::IeeeComparison { kind, format }, reversed))
}

fn is_scalar(ty: &ValueType) -> bool {
    matches!(ty, ValueType::Bool | ValueType::Integer { width: Some(1..), .. })
}

fn is_exact_integer(ty: &ValueType) -> bool {
    matches!(ty, ValueType::Exact { value, fractional_origin: None } if value.is_integer())
}

pub(super) fn predicate_operator(
    token: &str,
    operands: &[ValueType],
) -> Option<PredicateOperator> {
    use PredicateOperator::*;
    let boolean = |ty: &ValueType| ty == &ValueType::Bool;
    let integer = |ty: &ValueType| !boolean(ty) && (is_scalar(ty) || is_exact_integer(ty));
    match (token, operands) {
        ("!", [value]) if boolean(value) => Some(Not),
        ("&&", [left, right]) if boolean(left) && boolean(right) => Some(And),
        ("||", [left, right]) if boolean(left) && boolean(right) => Some(Or),
        ("==" | "!=", [left, right]) if boolean(left) && boolean(right) || integer(left) && integer(right) => {
            Some(if token == "==" { Equal } else { NotEqual })
        }
        ("<", [left, right]) if integer(left) && integer(right) => Some(LessThan),
        ("<=", [left, right]) if integer(left) && integer(right) => Some(LessOrEqual),
        (">", [left, right]) if integer(left) && integer(right) => Some(GreaterThan),
        (">=", [left, right]) if integer(left) && integer(right) => Some(GreaterOrEqual),
        _ => None,
    }
}
