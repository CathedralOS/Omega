//! Citations of logical theorems from machine bodies.
//!
//! A theorem with machine-shaped hypotheses has no executable identity, so a
//! statement that cites it cannot become `OperationKind::Call`. L3 types the
//! citation as an ordinary statement call. It adds one `SupplyHypothesis` per
//! hypothesis binder and the theorem's `CallPrecondition`. This module checks
//! those records against the published `LogicalTheorem` and selects the erased
//! `ApplyTheorem` operation. Forms it does not understand refuse here rather
//! than fall through to executable call selection.

use std::collections::HashMap;
use crate::lowering::applied_tree::LoweringTree;
use crate::{LoweringError, ScalarLiteral, TheoremArgument, TheoremBinder, site_at};
use omega_prepared::{PreparedTrees, PreparedValue};
use omega_resolved::Symbol;
use omega_syntax::NodeKind;
use omega_typed::{CallUse, NodeSemantics, ObligationKind, PauseEffects, TypedSignature, ValueType};

/// A local logical theorem that a body may cite, with its checked signature.
#[derive(Clone, Copy)]
pub(crate) struct TheoremCallee<'a> {
    pub signature: &'a TypedSignature,
    pub theorem: &'a crate::LogicalTheorem,
}

pub(crate) type TheoremCallees<'a> = HashMap<&'a omega_resolved::DeclarationId, TheoremCallee<'a>>;

pub(crate) struct TheoremCitation<I> {
    pub theorem: crate::DeclarationId,
    pub arguments: Vec<TheoremArgument>,
    /// One subject per formal, in formal order.
    pub subjects: Vec<crate::ProofSubject<I>>,
    pub requirement_count: usize,
}

/// The closed value of a theorem subject that names no variable, or `None`
/// for any other subject. It is an integer literal or folded exact
/// expression that L3 landed at `formal`, a Boolean literal, or a frozen
/// constant. Executable citations and theorem proofs both publish it as an
/// erased `ProofSubject::Literal`; neither emits an operation for it.
pub(crate) fn closed_subject(
    prepared: &PreparedTrees<'_>,
    tree: &LoweringTree<'_>,
    node: usize,
    formal: &ValueType,
) -> Result<Option<ScalarLiteral>, LoweringError> {
    let file = tree.resolved().file;
    let outer = node;
    let node = file.ungroup(node);
    let site = site_at(tree, node);
    let refuse = |message: &str| LoweringError::new(&site, message);
    let exact = match file.nodes[node].kind {
        NodeKind::Binary | NodeKind::Unary => super::arithmetic::folded_exact_operator(tree, node, formal, None)?.cloned(),
        NodeKind::Literal if file.nodes[node].children.is_empty() => tree.semantics_at(node).find_map(|row| match row {
            NodeSemantics::Expression(e) => match &e.ty {
                ValueType::Exact {
                    value,
                    fractional_origin: None,
                } if value.is_integer() => Some(value.numer().clone()),
                _ => None,
            },
            _ => None,
        }),
        NodeKind::PathExpression if file.nodes[node].children.is_empty() => {
            return match tree.resolved().symbol(node) {
                Some(Symbol::Builtin(name)) if *formal == ValueType::Bool && matches!(name.as_str(), "true" | "false") => {
                    Ok(Some(ScalarLiteral::Boolean(name == "true")))
                }
                Some(Symbol::Declaration(id))
                    if tree
                        .semantics_at(node)
                        .any(|row| matches!(row, NodeSemantics::Constant(c) if c == id)) =>
                {
                    match prepared.constant(id) {
                        Some(PreparedValue::Bool(value)) if *formal == ValueType::Bool => Ok(Some(ScalarLiteral::Boolean(*value))),
                        Some(PreparedValue::Integer(value)) if formal.contains_integer(value) => Ok(Some(ScalarLiteral::Integer(value.clone()))),
                        _ => Err(refuse("a constant theorem subject has no frozen scalar of its formal's carrier")),
                    }
                }
                _ => Ok(None),
            };
        }
        _ => None,
    };
    let Some(value) = exact else { return Ok(None) };
    // L3 records the landing, on the literal or on its outermost grouping,
    // only after checking that the value fits.
    let landed = [outer, node].into_iter().any(|at| {
        tree.semantics_at(at).any(|row| {
            matches!(row, NodeSemantics::Landing(landing)
                if &landing.ty == formal && landing.value.is_integer() && landing.value.numer() == &value)
        })
    });
    if !landed || !formal.contains_integer(&value) {
        return Err(refuse("a literal theorem subject needs its exact landing at the formal's carrier"));
    }
    Ok(Some(ScalarLiteral::Integer(value)))
}

/// Build the same erased subject for executable and proof citations. Scope
/// resolves each binding to its existing SSA value or its proof input. The
/// postfix table preserves rounding-free fixed-width operations and the
/// formation obligations at each occurrence, including intermediate results.
pub(crate) fn erased_subject<I>(
    prepared: &PreparedTrees<'_>, tree: &LoweringTree<'_>, owner: &omega_resolved::DeclarationId, root: usize, formal: &ValueType,
    binding: impl Fn(usize, &ValueType) -> Option<I>,
) -> Result<crate::ProofSubject<I>, LoweringError> {
    use crate::{ProofIntegerStep as Term, ProofSubject};
    enum Visit<'a> { Node(usize, &'a ValueType), Emit(crate::IntegerOperation, crate::ScalarType, crate::Site) }
    let file = tree.resolved().file;
    let mut work = vec![Visit::Node(root, formal)];
    let mut steps = Vec::new();
    while let Some(visit) = work.pop() {
        let (outer, ty) = match visit {
            Visit::Emit(operation, carrier, site) => { steps.push(Term::Operation { operation, carrier, site }); continue; }
            Visit::Node(node, ty) => (node, ty),
        };
        let node = file.ungroup(outer);
        let site = site_at(tree, node);
        let refuse = || LoweringError::new(&site, "unsupported erased theorem subject");
        if let Some(literal) = closed_subject(prepared, tree, outer, ty)? {
            steps.push(Term::Literal(literal));
            continue;
        }
        if file.nodes[node].kind == NodeKind::PathExpression && matches!(tree.resolved().symbol(node), Some(Symbol::Binding { .. })) {
            if !tree.semantics_at(node).any(|row| matches!(row, NodeSemantics::Expression(e) if e.static_argument_slot.is_none() && &e.ty == ty)) {
                return Err(refuse());
            }
            steps.push(Term::Input(binding(node, ty).ok_or_else(refuse)?));
            continue;
        }
        if !matches!(file.nodes[node].kind, NodeKind::Binary | NodeKind::Unary) { return Err(refuse()); }
        let mut checked = None;
        let mut typed = false;
        let mut obligations = Vec::new();
        for row in tree.semantics_at(node) {
            match row {
                NodeSemantics::Operation(op) if checked.is_none() && op.site == site && op.node == file.node_id(node) && &op.result == ty => checked = Some(op),
                NodeSemantics::Expression(e) if !typed && &e.ty == ty && e.static_argument_slot.is_none() => typed = true,
                NodeSemantics::Obligation(o) => obligations.push(o),
                _ => return Err(refuse()),
            }
        }
        let op = checked.ok_or_else(refuse)?;
        let operands = &file.nodes[node].children;
        if !typed || operands.len() != op.operands.len() || !matches!(ty, ValueType::Integer { width: Some(1..), .. }) { return Err(refuse()); }
        let super::arithmetic::LoweredInteger::Ordinary(operation) = super::arithmetic::lower_integer(op, &obligations, owner)? else { return Err(refuse()); };
        // Portable scalar term forms for checked / wrapping / saturating
        // same-carrier binary arithmetic (encoding.md#scalar-terms).
        if !matches!(operation.primitive, crate::IntegerPrimitive::Add | crate::IntegerPrimitive::Subtract | crate::IntegerPrimitive::Multiply |
            crate::IntegerPrimitive::Divide | crate::IntegerPrimitive::Remainder) { return Err(refuse()); }
        let carrier = crate::lowering::types::scalar_carrier(ty).map_err(|_| refuse())?;
        work.push(Visit::Emit(operation, carrier, site));
        work.extend(operands.iter().zip(&op.operands).rev().map(|(&n, t)| Visit::Node(n, t)));
    }
    if steps.len() == 1 {
        match steps.pop().unwrap() {
            Term::Input(input) => return Ok(ProofSubject::Input(input)),
            Term::Literal(value) => return Ok(ProofSubject::Literal(value)),
            _ => unreachable!("an integer operation follows its operands"),
        }
    }
    Ok(ProofSubject::IntegerTerm(steps))
}

/// `None` when `node` does not cite a local logical theorem.
pub(crate) fn select<I>(
    prepared: &PreparedTrees<'_>,
    tree: &LoweringTree<'_>,
    caller: &TypedSignature,
    node: usize,
    expected: &ValueType,
    callees: &TheoremCallees<'_>,
    binding: impl Fn(usize, &ValueType) -> Option<I>,
) -> Result<Option<TheoremCitation<I>>, LoweringError> {
    let Some((call, callee)) = tree.semantics_at(node).find_map(|record| match record {
        NodeSemantics::Call(call) => callees
            .get(&call.machine)
            .map(|callee| (call, *callee)),
        _ => None,
    }) else {
        return Ok(None);
    };
    let file = tree.resolved().file;
    let site = site_at(tree, node);
    let error = |message: &str| LoweringError::new(&site, message);
    let TheoremCallee { signature, theorem } = callee;
    if call.use_site != CallUse::Execution || call.node != file.node_id(node) || call.site != site || call.caller.as_ref() != Some(&caller.machine) {
        return Err(error("theorem citation disagrees with its owning statement"));
    }
    if call.layout_query.is_some() || call.evidence.is_some() || !call.enclosing_arguments.is_empty() || call.acknowledgements != PauseEffects::default() {
        return Err(error("theorem citation carries an executable selection record"));
    }
    if call.result != ValueType::NoResult || *expected != ValueType::NoResult || signature.machine != call.machine {
        return Err(error("a theorem citation is a statement with no value"));
    }

    let (mut statement, mut precondition, mut supplies) = (false, false, Vec::new());
    for record in tree.semantics_at(node) {
        match record {
            NodeSemantics::Call(other) if other == call => {}
            NodeSemantics::Expression(e) if !statement && e.static_argument_slot.is_none() && e.ty == ValueType::NoResult => statement = true,
            NodeSemantics::Obligation(o) if o.node == file.node_id(node) && o.site == site => match &o.kind {
                ObligationKind::CallPrecondition(machine) if !precondition && machine == &call.machine => precondition = true,
                ObligationKind::SupplyHypothesis {
                    requirement,
                    evidence,
                    substitutions,
                } if substitutions == &call.type_arguments => supplies.push((requirement, evidence)),
                kind => {
                    return Err(LoweringError::new(&o.site, format!("unsupported theorem citation obligation: {kind:?}")));
                }
            },
            _ => return Err(error("unsupported typed record on a theorem citation")),
        }
    }
    if !statement || precondition != signature.requires {
        return Err(error("theorem citation lacks its statement or precondition record"));
    }

    // Each hypothesis binder takes exactly the evidence L3 checked for it.
    if call.type_arguments.len() != theorem.binders.len() {
        return Err(error("theorem citation and binder telescope disagree"));
    }
    let mut arguments = Vec::with_capacity(theorem.binders.len());
    for (binding, binder) in call.type_arguments.iter().zip(&theorem.binders) {
        let TheoremBinder::Hypothesis(index) = *binder else {
            return Err(error(
                "a theorem citation with a mathematical binder argument needs its universe obligations lowered",
            ));
        };
        let hypothesis = &theorem.hypotheses[index];
        let position = supplies
            .iter()
            .position(|(requirement, _)| **requirement == hypothesis.site)
            .ok_or_else(|| error("theorem citation lacks the checked evidence for its hypothesis"))?;
        if supplies.swap_remove(position).1 != binding {
            return Err(error("theorem citation evidence disagrees with its binder argument"));
        }
        let ValueType::ProofCitation(machine) = binding else {
            return Err(error("theorem evidence other than a named machine needs proof-term lowering"));
        };
        arguments.push(TheoremArgument::Citation(crate::identity::declaration(machine, &site)?));
    }
    if !supplies.is_empty() {
        return Err(error("theorem citation supplies evidence for no binder"));
    }

    // Every argument is an erased subject substituted at its exact formal.
    if call.operands.len() != signature.parameters.len() || signature.parameters.len() != theorem.parameters.len() {
        return Err(error("theorem citation and its formals disagree"));
    }
    let mut subjects = Vec::with_capacity(call.operands.len());
    for (applied, (span, formal)) in call.operands.iter().zip(&signature.parameters) {
        let argument = applied.node.node;
        if argument >= file.nodes.len()
            || applied.node != file.node_id(argument)
            || applied.site != site_at(tree, argument)
            || applied.parameter.package != signature.site.package
            || applied.parameter.file != signature.site.file
            || applied.parameter.span != *span
        {
            return Err(error("theorem citation subject disagrees with its argument occurrence or formal"));
        }
        if crate::lowering::types::scalar_carrier(formal).is_err() {
            return Err(LoweringError::new(&applied.site, "a theorem citation subject needs a scalar carrier"));
        }
        if applied.callable.is_some() { return Err(error("unsupported callable theorem subject")); }
        subjects.push(erased_subject(prepared, tree, &caller.machine, argument, formal, &binding)?);
    }
    Ok(Some(TheoremCitation {
        theorem: theorem.declaration.clone(),
        arguments,
        subjects,
        requirement_count: theorem.requires.len(),
    }))
}
