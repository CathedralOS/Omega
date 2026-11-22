//! Lower the exact canonical process-exit invocation as a terminal external transfer.
//!
//! L3 owns recognition of the canonical requirement and publishes one record per
//! call. This module carries that record into a no-successor terminator with the
//! exact requirement and ordered operands. It adds no result, normal-return
//! guarantee or cleanup, and refuses an occurrence whose evidence disagrees.
//! Authority, reach, progress and crossing obligations remain checker judgments.
use super::body::{next_id, site_at};
use super::branches::BlockFlow;
use super::expressions::Expressions;
use crate::{BlockTerminator, CallArgument, EdgeId, ExternalCompletionKind, ExternalExit, LoweringError, MachineBody, OperationResult};
use omega_typed::{CallUse, ExternalCompletion, NodeSemantics, PauseEffects};

/// Grouping preserves the identity of the selected terminal call.
pub(crate) fn recognized<'a>(
    expressions: &Expressions<'a>,
    node: usize,
) -> Option<&'a ExternalCompletion> {
    let file = expressions.tree.resolved().file;
    let id = file.node_id(file.ungroup(node));
    expressions
        .tree
        .forest()
        .external_completions
        .iter()
        .find(|completion| completion.node == id)
}

/// Evaluate the operands in authored formal order, then close the active block
/// with the terminal exit. Nothing after it in the block can execute.
pub(crate) fn lower(
    expressions: &Expressions<'_>,
    node: usize,
    completion: &ExternalCompletion,
    body: &mut MachineBody,
    flow: &mut BlockFlow,
) -> Result<(), LoweringError> {
    let tree = expressions.tree;
    let file = tree.resolved().file;
    let node = file.ungroup(node);
    let site = site_at(file, node);
    let error = |message: &str| LoweringError::new(&site, message);
    if completion.site != site {
        return Err(error("process exit record disagrees with its occurrence"));
    }
    let mut selected = None;
    let mut results = Vec::new();
    for record in tree.semantics_at(node) {
        match record {
            NodeSemantics::Call(call) if call.use_site == CallUse::Execution => {
                if selected.replace(call).is_some() {
                    return Err(error("process exit has multiple executable selections"));
                }
            }
            NodeSemantics::Expression(expression) if expression.static_argument_slot.is_none() => results.push(&expression.ty),
            _ => return Err(error("process exit carries an unsupported typed record")),
        }
    }
    let call = selected.ok_or_else(|| error("process exit has no executable selection"))?;
    if call.node != file.node_id(node)
        || call.site != site
        || call.caller.as_ref() != Some(&expressions.signature.machine)
        || call.machine != completion.requirement
        || !call.type_arguments.is_empty()
        || call.evidence.is_some()
        || call.layout_query.is_some()
        || call.acknowledgements != PauseEffects::default()
        || results.iter().any(|ty| **ty != call.result)
    {
        return Err(error("process exit selection disagrees with its canonical record"));
    }
    let typed = expressions.tree.forest();
    let signature = typed
        .signatures
        .iter()
        .find(|signature| signature.machine == completion.requirement)
        .ok_or_else(|| error("process exit needs its canonical requirement signature"))?;
    if signature.requires || signature.result != call.result || signature.parameters.len() != call.operands.len() {
        return Err(error("process exit disagrees with its canonical requirement contract"));
    }
    let mut arguments = Vec::with_capacity(call.operands.len());
    for (operand, (span, formal)) in call.operands.iter().zip(&signature.parameters) {
        let argument = operand.node.node;
        if argument >= file.nodes.len()
            || operand.node != file.node_id(argument)
            || operand.site != site_at(file, argument)
            || operand.parameter.package != signature.site.package
            || operand.parameter.file != signature.site.file
            || operand.parameter.span != *span
            || operand.callable.is_some()
        {
            return Err(LoweringError::new(&operand.site, "process exit operand disagrees with its formal"));
        }
        if !formal.domains().is_empty() || !expressions.types.can_forward_value(formal) {
            return Err(LoweringError::new(&operand.site, "process exit operand requires a bare scalar carrier"));
        }
        match expressions.lower(argument, formal, body, flow)? {
            OperationResult::Scalar(value) => arguments.push(CallArgument::Scalar(value)),
            _ => return Err(LoweringError::new(&operand.site, "process exit operand requires a bare scalar carrier")),
        }
    }
    let kind = match completion.kind {
        omega_typed::ExternalCompletionKind::ProcessExit => ExternalCompletionKind::ProcessExit,
    };
    let owner = crate::identity::declaration(&completion.owner, &site)?;
    let requirement = crate::identity::declaration(&completion.requirement, &site)?;
    let id = EdgeId(next_id(body.diagnostics.edges.len()));
    body.diagnostics.edges.push(site);
    flow.close(
        body,
        BlockTerminator::ExternalExit(ExternalExit {
            id,
            kind,
            owner,
            requirement,
            arguments,
            // The canonical requirement publishes no `requires` clause.
            requirements: Vec::new(),
        }),
    );
    Ok(())
}
