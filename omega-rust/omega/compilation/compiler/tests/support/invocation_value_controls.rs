//! Source-produced mutable values distinguish a call edge from an operation
//! with the same numeric ID. These queries grant no runtime or entry authority.

use semantic_vocabulary::{InvocationSite, OperationId};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{OperationKind, StructuralArgument, TerminalModule, Terminator};
use terminal_semantics::descriptor_results::{
    ValueIdentityAnalysis, ValueIdentitySeed, terminal_reference_graph_from,
};

pub(super) fn inspect(artifact: &CanonicalTerminalArtifact) -> Result<(), String> {
    let module = artifact.semantic_module();
    let mut candidates = module.machines.iter().filter_map(|body| {
        let [first, second] = body.structural_parameters.as_slice() else {
            return None;
        };
        if first.structural_type != second.structural_type {
            return None;
        }
        let write = body.operations().find(|operation| {
            matches!(operation.kind, OperationKind::StructuralScalarFieldStore { destination, .. }
                if destination == first.place)
        })?;
        let mut calls = body.blocks.iter().filter_map(|block| {
            let Terminator::InvokeNoNormal {
                edge,
                callee,
                structural_arguments,
                ..
            } = &block.terminator
            else {
                return None;
            };
            (structural_arguments.len() == 2).then_some((*edge, *callee))
        });
        let (Some((edge, callee)), None) = (calls.next(), calls.next()) else {
            return None;
        };
        Some((
            body.id,
            write.id,
            edge,
            callee,
            [
                StructuralArgument {
                    place: first.place,
                    path: Vec::new(),
                    access: first.access,
                },
                StructuralArgument {
                    place: second.place,
                    path: Vec::new(),
                    access: second.access,
                },
            ],
        ))
    });
    let (Some((body, write, edge, callee, actuals)), None) = (candidates.next(), candidates.next())
    else {
        return Err(
            "invocation identity controls need one source-owned write followed by a no-normal call"
                .into(),
        );
    };
    check(module, body, write, edge, callee, &actuals)?;

    // Change only operation IDs, keeping the source operation order, operands,
    // values, types and control edges. No forged Unit call or accepted artifact
    // is constructed. Rebuild the non-authorizing graph from the changed input.
    let alias = OperationId::new(edge.get()).ok_or("source invocation edge has no identity")?;
    let mut relabeled = module.clone();
    for machine in &mut relabeled.machines {
        for operation in machine
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
        {
            if operation.id == write {
                operation.id = alias;
            } else if operation.id == alias {
                operation.id = write;
            }
        }
    }
    check(&relabeled, body, alias, edge, callee, &actuals)
}

fn check(
    module: &TerminalModule,
    body: semantic_vocabulary::MachineId,
    write: OperationId,
    edge: semantic_vocabulary::EdgeId,
    callee: semantic_vocabulary::MachineId,
    actuals: &[StructuralArgument; 2],
) -> Result<(), String> {
    let graph = terminal_reference_graph_from(module, [body, callee])
        .map_err(|error| format!("source invocation reference graph: {error:?}"))?;
    let mut identity = ValueIdentityAnalysis::new(module, &graph, body)?;
    let entry = ValueIdentitySeed::Parameter(0);
    let call = InvocationSite::InvokeNoNormal(edge);
    identity.require(entry, write, &actuals[0])?;
    require_changed(identity.require_at_invocation(entry, call, &actuals[0]))?;
    identity.require_at_invocation(ValueIdentitySeed::Parameter(1), call, &actuals[1])?;
    require_changed(identity.require_at_invocation(entry, call, &actuals[1]))?;
    // Repeat in the opposite success/failure order through the same cache.
    require_changed(identity.require_at_invocation(entry, call, &actuals[0]))?;
    identity.require(entry, write, &actuals[0])?;
    let absent = module
        .machines
        .iter()
        .flat_map(|body| &body.blocks)
        .map(|block| block.terminator.edge().get())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .and_then(semantic_vocabulary::EdgeId::new)
        .ok_or("source invocation edge namespace is exhausted")?;
    match identity.require_at_invocation(entry, InvocationSite::InvokeNoNormal(absent), &actuals[0])
    {
        Err("value identity observation is absent") => Ok(()),
        other => Err(format!(
            "missing invocation site did not refuse for its exact reason: {other:?}"
        )),
    }
}

fn require_changed(result: Result<(), &'static str>) -> Result<(), String> {
    match result {
        Err(
            "actual value is not the specified unchanged value; transformations and ordinary call returns supply no identity",
        ) => Ok(()),
        other => Err(format!(
            "changed source value did not refuse for its exact identity reason: {other:?}"
        )),
    }
}
