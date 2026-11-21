//! Citations of logical theorems from machine bodies.
//!
//! A theorem with machine-shaped hypotheses has no executable identity, so a
//! statement that cites it cannot become `OperationKind::Call`. L3 types the
//! citation as an ordinary statement call. It adds one `SupplyHypothesis` per
//! hypothesis binder and the theorem's `CallPrecondition`. This module checks
//! those records against the published `LogicalTheorem` and selects the erased
//! `ApplyTheorem` operation. Forms it does not understand refuse here rather
//! than fall through to executable call selection.

use super::calls::Callees;
use crate::lowering::applied_tree::LoweringTree;
use crate::{LoweringError, TheoremArgument, TheoremBinder, site_at};
use omega_resolved::Symbol;
use omega_syntax::NodeKind;
use omega_typed::{CallUse, NodeSemantics, ObligationKind, PauseEffects, TypedSignature, ValueType};

/// A local logical theorem that a body may cite, with its checked signature.
#[derive(Clone, Copy)]
pub(crate) struct TheoremCallee<'a> {
    pub signature: &'a TypedSignature,
    pub theorem: &'a crate::LogicalTheorem,
}

pub(crate) struct TheoremCitation<'a> {
    pub theorem: crate::DeclarationId,
    pub arguments: Vec<TheoremArgument>,
    /// Subject occurrences with their formal carriers, in formal order.
    pub subjects: Vec<(usize, &'a ValueType)>,
    pub requirement_count: usize,
}

/// `None` when `node` does not cite a local logical theorem.
pub(crate) fn select<'a>(
    tree: &LoweringTree<'_>,
    caller: &TypedSignature,
    node: usize,
    expected: &ValueType,
    callees: &Callees<'a>,
) -> Result<Option<TheoremCitation<'a>>, LoweringError> {
    let Some((call, callee)) = tree.semantics_at(node).find_map(|record| match record {
        NodeSemantics::Call(call) => callees
            .theorems
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

    // A subject is named, never computed. Computing an operand would make the
    // erased citation demand a runtime value.
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
        if applied.callable.is_some()
            || file.nodes[argument].kind != NodeKind::PathExpression
            || !matches!(tree.resolved().symbol(argument), Some(Symbol::Binding { .. }))
        {
            return Err(LoweringError::new(
                &applied.site,
                "a theorem citation subject other than a named binding needs erased subject-term lowering",
            ));
        }
        if crate::lowering::types::scalar_carrier(formal).is_err() {
            return Err(LoweringError::new(&applied.site, "a theorem citation subject needs a scalar carrier"));
        }
        subjects.push((argument, formal));
    }
    Ok(Some(TheoremCitation {
        theorem: theorem.declaration.clone(),
        arguments,
        subjects,
        requirement_count: theorem.requires.len(),
    }))
}
