//! Read the proof declarations an evaluation entry demands for a particular dispatch selection.
use terminal_psi::{ProofApplicationScope, ProofDeclarationId};

pub struct CompilerInvocationScope {
    pub selected: Vec<(ProofDeclarationId, ProofDeclarationId)>,
    pub closure: proof_applications::ProofApplicationReachability<ProofDeclarationId>,
    pub dependencies: Vec<ProofDeclarationId>,
}

pub fn compiler_invocation_scope<'target>(
    scope: &ProofApplicationScope,
    target: impl Into<Option<&'target str>>,
    providers: impl IntoIterator<
        Item = (
            semantic_vocabulary::BoundaryMachineId,
            semantic_vocabulary::MachineId,
        ),
    >,
    root: ProofDeclarationId,
) -> Result<CompilerInvocationScope, String> {
    let target = target.into();
    let root = match target {
        Some(target) => scope.plan.callable_for_target(root, target),
        None => scope.plan.callable_without_target(root),
    }
    .ok_or("compiler-evaluation declaration has no body in the requested target scope")?;
    let selected = crate::scalar_conditions::proof_provider_edges(scope, providers)?;
    let closure = scope
        .applications()
        .invocation_reachability(target, &selected, &[root]);
    let mut dependencies = closure.machines.clone();
    dependencies.extend(closure.calls.iter().copied());
    for (_, machine) in scope.plan.machines.iter() {
        if closure.machines.contains(&machine.machine) {
            dependencies.extend(
                scope
                    .plan
                    .symbols
                    .span_or_empty(machine.states)
                    .iter()
                    .copied(),
            );
        }
    }
    let retained = scope.plan.declaration_closure(&dependencies);
    dependencies.extend(retained);
    if target.is_none() {
        // A tagged callee can occur below an untagged root. Traversal leaves
        // it in the demanded call roster rather than silently selecting it.
        let demanded = dependencies
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        for (_, machine) in scope.plan.machines.iter() {
            if (demanded.contains(&machine.machine)
                || scope
                    .plan
                    .symbols
                    .span_or_empty(machine.states)
                    .iter()
                    .any(|state| demanded.contains(state)))
                && (machine.authored_target.is_some()
                    || machine.source_target.is_some()
                    || machine.family_interface)
            {
                return Err("target-independent evaluation demands a target-selected body".into());
            }
        }
    }
    Ok(CompilerInvocationScope {
        selected,
        closure,
        dependencies,
    })
}
