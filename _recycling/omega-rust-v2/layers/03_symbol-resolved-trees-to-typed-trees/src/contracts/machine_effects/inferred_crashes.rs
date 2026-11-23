//! Publish an empty crash row for the route-free fragment of private checked
//! bodies. This is structural effect inference, not guard disproof. An
//! unclassified operation or call leaves the row unknown, never empty.
use super::super::effect_propagation::ExecutionCalls;
use crate::declarations::{declaration_catalog::DeclarationCatalog, resolved_source_file::ResolvedSourceFile};
use crate::typing::preparation_dependencies::PreparationDependencies;
use crate::{Multiplicity, TypedStage};
use omega_resolved::{DeclarationId, Symbol};
use omega_syntax::NodeKind;
use std::collections::{HashMap, HashSet};

pub(super) fn empty_rows<'a>(
    units: &[ResolvedSourceFile<'_>],
    catalog: &DeclarationCatalog<'a>,
    graph: &ExecutionCalls,
    dependencies: &PreparationDependencies<'_>,
    stage: &TypedStage,
) -> HashSet<&'a DeclarationId> {
    // Only execution contributes here: type arguments of a checked constructor
    // may contain their own, separately demanded, static calls.
    let direct: HashMap<_, _> = graph
        .direct
        .iter()
        .map(|&i| (stage.calls[i].node, &stage.calls[i]))
        .collect();
    let indirect: HashMap<_, _> = graph
        .indirect
        .iter()
        .map(|&i| (stage.indirect_calls[i].node, &stage.indirect_calls[i]))
        .collect();
    let expressions: HashMap<_, _> = stage.expressions.iter().map(|e| (e.node, &e.ty)).collect();
    let mut empty: HashSet<_> = catalog
        .signatures
        .iter()
        .filter_map(|(&id, signature)| (signature.crash_ceiling.as_deref() == Some(&[])).then_some(id))
        .collect();
    for row in &stage.machine_effects {
        if row.crash_ceiling.as_deref() == Some(&[])
            && let Some((&id, _)) = catalog.signatures.get_key_value(&row.machine)
        {
            empty.insert(id);
        }
    }
    let mut users = HashMap::<&DeclarationId, Vec<&DeclarationId>>::new();
    let mut missing = HashMap::new();
    let mut ready = Vec::new();
    for &id in &catalog.local_machines {
        let signature = &catalog.signatures[id];
        if id.kind != NodeKind::Machine
            || signature.crash_ceiling.is_some()
            || !dependencies.declaration_is_ready(id)
            || signature
                .parameters
                .iter()
                .any(|(_, ty)| catalog.types.multiplicity(ty) != Multiplicity::Copy)
        {
            continue;
        }
        let unit = &units[signature.node.file];
        let file = unit.file;
        let Some(body) = file.child(signature.node.node, NodeKind::Body) else {
            continue;
        };
        let mut callees = Vec::new();
        let mut nodes = vec![body];
        let mut supported = true;
        while let Some(n) = nodes.pop() {
            let node = &file.nodes[n];
            let key = file.node_id(n);
            match node.kind {
                NodeKind::Body | NodeKind::Tail | NodeKind::Paren | NodeKind::Tuple => nodes.extend(&node.children),
                NodeKind::Literal => (),
                NodeKind::PathExpression
                    if matches!(
                        unit.symbol(n),
                        Some(Symbol::Binding {
                            kind: NodeKind::Param | NodeKind::ConstParam,
                            ..
                        })
                    ) =>
                {
                }
                NodeKind::PathExpression if matches!(unit.symbol(n), Some(Symbol::Builtin(name)) if name == "true" || name == "false") => (),
                NodeKind::RecordLiteral
                    if expressions
                        .get(&key)
                        .is_some_and(|ty| catalog.types.multiplicity(ty) == Multiplicity::Copy) =>
                {
                    nodes.extend(node.children.iter().skip(1)); // the head is a type application
                }
                NodeKind::FieldInit if node.children.len() == 1 => nodes.extend(&node.children),
                NodeKind::Call
                    if let Some(call) = direct.get(&key)
                        && call.evidence.is_none() =>
                {
                    if !empty.contains(&call.machine) {
                        callees.push(&call.machine);
                    }
                    nodes.extend(call.operands.iter().map(|operand| operand.node.node));
                }
                NodeKind::Call
                    if let Some(call) = indirect.get(&key)
                        && call.contract.crash_ceiling.as_deref() == Some(&[])
                        && let [callee, _] = node.children.as_slice() =>
                {
                    // The callable ceiling covers invocation, not evaluation of the callee.
                    nodes.push(*callee);
                    nodes.extend(call.operands.iter().map(|operand| operand.node.node));
                }
                _ => {
                    supported = false;
                    break;
                }
            }
        }
        if supported {
            if callees.is_empty() {
                ready.push(id);
            } else {
                missing.insert(id, callees.len());
                for callee in callees {
                    users.entry(callee).or_default().push(id);
                }
            }
        }
    }
    while let Some(id) = ready.pop() {
        empty.insert(id);
        for &user in users.get(id).into_iter().flatten() {
            let count = missing.get_mut(user).unwrap();
            *count -= 1;
            if *count == 0 {
                ready.push(user);
            }
        }
    }
    empty
}
