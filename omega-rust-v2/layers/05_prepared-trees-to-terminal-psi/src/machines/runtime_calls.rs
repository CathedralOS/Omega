//! Runtime callable descriptors: invocation through a callable parameter,
//! forwarding of the actual descriptor to a direct callee, and refinement of a
//! descriptor into a destination contract. The callable contract owns the call
//! shape; no representative static callee is substituted or reconstructed.
use super::body::site_at;
use super::expressions::{EmitOperation, Expressions, OperationOutput};
use crate::{CallTarget, LoweringError, MachineBody, OperationKind, OperationResult, TypeShape, ValueId};
use omega_resolved::Symbol;
use omega_syntax::NodeKind;
use omega_typed::{CallUse, InvocationTarget, NodeSemantics, ObligationKind, PauseEffects, TypedSignature, ValueType};

/// An indirect call through the current descriptor of a callable formal.
pub(super) struct RuntimeCall<'a> {
    pub callee: CallTarget,
    /// Argument nodes in evaluation order with their contract formals.
    pub operands: Vec<(usize, &'a ValueType)>,
}

/// The current scalar descriptor bound at `node`, which must name `binder_span`
/// when the caller demands an exact formal identity.
fn bound_descriptor(
    expressions: &Expressions<'_>,
    node: usize,
    binder_span: Option<crate::SourceSpan>,
) -> Result<ValueId, LoweringError> {
    let tree = expressions.tree;
    let file = tree.resolved().file;
    let error = || LoweringError::new(&site_at(file, node), "callable descriptor requires its exact bound value");
    if file.nodes[node].kind != NodeKind::PathExpression || !file.nodes[node].children.is_empty() {
        return Err(error());
    }
    let Some(Symbol::Binding { binder_node, span, .. }) = tree.resolved().symbol(node) else {
        return Err(error());
    };
    if binder_span.is_some_and(|expected| expected != *span) {
        return Err(error());
    }
    match expressions
        .bindings
        .iter()
        .rev()
        .find(|(binder, _)| binder == binder_node)
    {
        Some((_, OperationResult::Scalar(value))) => Ok(*value),
        _ => Err(error()),
    }
}

fn contract_of(
    expressions: &Expressions<'_>,
    value: ValueId,
    body: &MachineBody,
) -> Option<crate::CallableContractId> {
    match expressions.types.get(body.values[value.index()].ty) {
        Some(TypeShape::Callable(contract)) => Some(*contract),
        _ => None,
    }
}

/// Lower a descriptor value landing in `carrier`. A typed refinement between
/// two contracts becomes an explicit Psi refinement of the same descriptor.
pub(super) fn descriptor_value(
    expressions: &Expressions<'_>,
    node: usize,
    carrier: &ValueType,
    body: &mut MachineBody,
) -> Result<OperationResult, LoweringError> {
    let tree = expressions.tree;
    let file = tree.resolved().file;
    let site = site_at(file, node);
    let error = |message: &str| LoweringError::new(&site, message);
    let ValueType::Callable(destination) = carrier else {
        return Err(error("callable descriptor requires a callable carrier"));
    };
    let target = expressions
        .types
        .id_for(carrier)
        .map_err(|e| error(&e.to_string()))?;
    let contract = expressions
        .types
        .callable_contract(destination)
        .map_err(|e| error(&e.to_string()))?;
    let mut refinement = None;
    let mut typed = Vec::new();
    let mut current = node;
    loop {
        for record in tree.semantics_at(current) {
            match record {
                NodeSemantics::Expression(e) if e.static_argument_slot.is_none() => match &e.ty {
                    ValueType::Callable(callable) => typed.push(callable.as_ref()),
                    _ => return Err(error("callable descriptor has a non-callable typed carrier")),
                },
                NodeSemantics::Obligation(o) if o.node == file.node_id(current) && refinement.is_none() => match &o.kind {
                    ObligationKind::RefineCallableValue { source, destination: to } if *to == destination.contract => refinement = Some(source),
                    other => {
                        return Err(LoweringError::new(&o.site, format!("unsupported callable descriptor obligation: {other:?}")));
                    }
                },
                _ => return Err(error("callable descriptor has an unsupported typed record")),
            }
        }
        match (file.nodes[current].kind, file.nodes[current].children.as_slice()) {
            (NodeKind::Paren, [inner]) => current = *inner,
            _ => break,
        }
    }
    if typed.is_empty()
        || typed
            .iter()
            .any(|c| *c != destination.as_ref() && refinement.is_none_or(|source| *source != c.contract))
    {
        return Err(error("callable descriptor type disagrees with its landing contract"));
    }
    let value = bound_descriptor(expressions, current, None)?;
    let actual = contract_of(expressions, value, body).ok_or_else(|| error("bound value is not a callable descriptor"))?;
    let Some(source) = refinement else {
        if body.values[value.index()].ty != target {
            return Err(error("callable descriptor contract disagrees with its destination"));
        }
        return Ok(OperationResult::Scalar(value));
    };
    if expressions.types.callable_contracts()[actual.index()].contract != *source {
        return Err(error("callable refinement source disagrees with the bound descriptor"));
    }
    Ok(body.push_operation(
        OperationOutput::Scalar(target),
        OperationKind::RefineCallableDescriptor { source: value, contract },
        site,
    ))
}

/// Select an executable indirect call at `node`, when typing recorded one.
/// Only a call through the caller's own callable formal is admitted: the
/// descriptor is that formal's current value and the contract is its type.
pub(super) fn select_runtime_call<'a>(
    expressions: &Expressions<'a>,
    node: usize,
    expected: &ValueType,
    body: &MachineBody,
) -> Result<Option<RuntimeCall<'a>>, LoweringError> {
    let tree: &'a crate::lowering::applied_tree::LoweringTree<'a> = expressions.tree;
    let caller: &TypedSignature = expressions.signature;
    let file = tree.resolved().file;
    let site = site_at(file, node);
    let error = |message: &str| LoweringError::new(&site, message);
    let mut selected = None;
    for record in tree.semantics_at(node) {
        if let NodeSemantics::IndirectCall(call) = record
            && selected.replace(call).is_some()
        {
            return Err(error("call has multiple indirect selections"));
        }
    }
    // Without an indirect selection the node belongs to direct call lowering.
    let Some(call) = selected else { return Ok(None) };
    let mut typed = false;
    for record in tree.semantics_at(node) {
        match record {
            NodeSemantics::IndirectCall(_) => {}
            NodeSemantics::Expression(e) if e.static_argument_slot.is_none() && &e.ty == expected => typed = true,
            NodeSemantics::Obligation(o) => {
                return Err(LoweringError::new(&o.site, format!("unsupported runtime call obligation: {:?}", o.kind)));
            }
            _ => return Err(error("runtime call has an unsupported typed record")),
        }
    }
    if call.use_site != CallUse::Execution || call.node != file.node_id(node) || call.site != site || call.caller.as_ref() != Some(&caller.machine) {
        return Err(error("indirect call disagrees with its owning occurrence"));
    }
    if call.acknowledgements != PauseEffects::default() || call.contract.pause != PauseEffects::default() {
        return Err(error("runtime call requires pause lowering"));
    }
    if !typed || call.contract.result != *expected {
        return Err(error("runtime call result disagrees with its normal continuation"));
    }
    let contract = expressions
        .types
        .callable_contract(&call.contract)
        .map_err(|e| error(&e.to_string()))?;
    let (NodeKind::Call, [callee, args]) = (file.nodes[node].kind, file.nodes[node].children.as_slice()) else {
        return Err(error("runtime call requires a callee and an argument list"));
    };
    if file.nodes[*args].kind != NodeKind::Args || call.callee != site_at(file, *callee) {
        return Err(error("runtime call disagrees with its callee occurrence"));
    }
    for part in *callee..file.nodes[*callee].end {
        for record in tree.semantics_at(part) {
            match record {
                NodeSemantics::Expression(e) if e.static_argument_slot.is_none() && matches!(&e.ty, ValueType::Callable(c) if **c == call.contract) => {}
                _ => return Err(error("runtime callee has an unsupported typed record")),
            }
        }
    }
    let callee_node = file.ungroup(*callee);
    let binder = match &call.entry {
        InvocationTarget::RuntimeParameter { owner, index } if owner == &caller.machine => {
            let Some((span, ValueType::Callable(formal))) = caller.parameters.get(*index) else {
                return Err(error("runtime call names no callable formal"));
            };
            if **formal != call.contract {
                return Err(error("runtime call disagrees with its callable formal"));
            };
            Some(*span)
        }
        InvocationTarget::StoredCallable(stored) if *stored == site_at(file, callee_node) => None,
        _ => return Err(error("runtime call lacks its exact selected descriptor occurrence")),
    };
    let descriptor = bound_descriptor(expressions, callee_node, binder)?;
    if contract_of(expressions, descriptor, body) != Some(contract) {
        return Err(error("runtime call descriptor disagrees with its callable contract"));
    }
    let arguments = &file.nodes[*args].children;
    if call.operands.len() != call.contract.parameters.len() || arguments.len() != call.operands.len() {
        return Err(error("runtime call operands disagree with the callable contract"));
    }
    let mut operands = Vec::with_capacity(arguments.len());
    for ((operand, &argument), formal) in call
        .operands
        .iter()
        .zip(arguments)
        .zip(&call.contract.parameters)
    {
        if operand.node != file.node_id(argument) || operand.site != site_at(file, argument) {
            return Err(error("runtime call operand disagrees with its argument occurrence"));
        }
        if operand.callable.is_some() || !crate::lowering::types::supports_value_copy(formal) || crate::lowering::types::is_shared_byte_view(formal) {
            return Err(LoweringError::new(&operand.site, "runtime call operand requires unsupported carrier lowering"));
        }
        operands.push((argument, formal));
    }
    Ok(Some(RuntimeCall {
        callee: CallTarget::Runtime { descriptor, contract },
        operands,
    }))
}

/// Check that the source operand supplies the descriptor it actually carries.
pub(super) fn check_forwarded_operand(
    tree: &crate::lowering::applied_tree::LoweringTree<'_>,
    caller: &TypedSignature,
    applied: &omega_typed::AppliedOperand,
) -> Result<(), LoweringError> {
    let argument = applied.node.node;
    let file = tree.resolved().file;
    let target = applied
        .callable
        .as_ref()
        .ok_or_else(|| LoweringError::new(&applied.site, "callable formal lacks the selected runtime descriptor"))?;
    let exact = match target {
        InvocationTarget::RuntimeParameter { owner, index } if owner == &caller.machine => caller
            .parameters
            .get(*index)
            .filter(|(_, ty)| matches!(ty, ValueType::Callable(_)))
            .is_some_and(|(binder, _)| matches!(tree.resolved().symbol(file.ungroup(argument)), Some(Symbol::Binding { span, .. }) if span == binder)),
        InvocationTarget::StoredCallable(stored) => *stored == site_at(file, file.ungroup(argument)),
        _ => false,
    };
    if !exact {
        return Err(LoweringError::new(
            &applied.site,
            "forwarded descriptor requires the actual current descriptor; construction from a named machine is unsupported",
        ));
    }
    Ok(())
}
