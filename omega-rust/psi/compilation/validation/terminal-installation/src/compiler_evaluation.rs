//! Invocation admission over the checked facts retained by Psi.
//!
//! Selection and installation supply exact provider edges. This owner applies
//! the same compiler-execution floor as Early Build, without recovering Source
//! or treating runtime success, available bodies, or fuel as a proof.

use semantic_vocabulary::compiler_evaluation::{
    CompilerEvaluationDeclaration, CompilerEvaluationEffects, DeclarationCustody,
    DeclarationSelectionAuthority, require_compiler_evaluation_floor,
};
use terminal_psi::{ProofApplicationScope, ProofDeclarationId, TargetMachineCallable};

mod failures;
pub(crate) use failures::require_root_failure_admission;

pub struct AdmittedCompilerEvaluation {
    providers: crate::AdmittedProviderSelections,
    artifact: terminal_psi::TerminalPsiIdentity,
    target: String,
    declaration: ProofDeclarationId,
}

impl AdmittedCompilerEvaluation {
    pub fn providers(&self) -> &crate::AdmittedProviderSelections {
        &self.providers
    }
    pub fn require_invocation(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        target: &str,
        entry: TargetMachineCallable,
    ) -> Result<(), String> {
        let declaration = artifact
            .module()
            .scalar_applications
            .proof_scope
            .callable_declaration(entry);
        if self.artifact != artifact.identity()
            || self.target != target
            || declaration != Some(self.declaration)
        {
            return Err(
                "compiler-evaluation admission names a different artifact, target or invocation"
                    .into(),
            );
        }
        Ok(())
    }
}

pub fn admit_compiler_evaluation(
    artifact: terminal_codec::TerminalArtifactView<'_>,
    target: &str,
    root: ProofDeclarationId,
    providers: crate::AdmittedProviderSelections,
    requester: DeclarationCustody,
    authority: Option<&dyn DeclarationSelectionAuthority>,
) -> Result<AdmittedCompilerEvaluation, String> {
    if providers.terminal_psi() != artifact.identity() {
        return Err("compiler-evaluation provider installation names a different artifact".into());
    }
    let scope = &artifact.module().scalar_applications.proof_scope;
    let root = scope
        .plan
        .callable_for_target(root, target)
        .ok_or("compiler-evaluation declaration has no selected target body")?;
    let root_facts = declaration(scope, root)?;
    if root_facts.entry_requires {
        return Err(format!(
            "machine `{}` has an authored `requires` premise; no checked invocation proof for that premise",
            root_facts.name
        ));
    }
    require_root_failure_admission(artifact.module(), root)?;
    let invocation = compiler_invocation_scope(scope, target, &providers, root)?;
    let CompilerInvocationScope {
        selected,
        closure,
        dependencies,
    } = invocation;
    for binding in providers.assumed_bindings() {
        for requirement in
            std::iter::once(binding.required).chain(binding.supply_path.iter().copied())
        {
            let declaration = scope
                .callable_declaration(TargetMachineCallable::Boundary(requirement))
                .ok_or("compiler-evaluation assumed requirement lost its declaration")?;
            if closure.calls.contains(&declaration) || closure.machines.contains(&declaration) {
                return Err("assumed binding contracts are runtime-only and cannot supply compiler evaluation".into());
            }
        }
    }
    for (_, owner) in scope.pending_evaluation_conditions.iter() {
        if owner.0 == 0 || dependencies.contains(owner) {
            return Err("constant invocation retains a pending target proof condition in its demanded closure; it cannot establish its own pending target condition".into());
        }
    }
    let mut effects = CompilerEvaluationEffects::default();
    let mut closure_violation = None;
    let mut selection_violation = authority
        .and_then(|authority| require_selection(requester, root_facts.custody, authority).err());
    for &machine in &closure.machines {
        let facts = declaration(scope, machine)?;
        for service in &facts.effects.services {
            if !effects.services.contains(service) {
                effects.services.push(service.clone());
            }
        }
        effects.may_suspend |= facts.effects.may_suspend;
        effects.may_block |= facts.effects.may_block;
        if closure_violation.is_none() {
            closure_violation = facts
                .static_application
                .clone()
                .or_else(|| facts.linear_carrier.clone());
        }
        // A selected requirement keeps its source effect ceiling but does not
        // pretend to have a checked body. Its exact provider owes the floor.
        if !facts.local_terminates
            && !selected
                .iter()
                .any(|(requirement, _)| *requirement == machine)
            && closure_violation.is_none()
        {
            closure_violation = Some(format!(
                "machine call path `{}` has no ordinary checked `Terminates` guarantee",
                facts.name
            ));
        }
        if let Some(authority) = authority {
            let requests = facts.selections.as_ref().map_err(Clone::clone)?;
            for request in requests {
                if let Err(reason) = require_selection(
                    DeclarationCustody::Package(request.requester),
                    DeclarationCustody::Package(request.owner),
                    authority,
                ) && selection_violation.is_none()
                {
                    selection_violation = Some(reason);
                }
            }
        }
    }
    for edge in &closure.invocations {
        if !edge.provider
            && !edge.static_delegation
            && let Some(authority) = authority
            && let (Ok(source), Ok(target)) = (
                declaration(scope, edge.source.machine()),
                declaration(scope, edge.target.machine()),
            )
            && let Err(reason) = require_selection(source.custody, target.custody, authority)
            && selection_violation.is_none()
        {
            selection_violation = Some(reason);
        }
        // Only recurrent edges owe component evidence. Exact application
        // equality retains static callback tuples in this cycle test.
        let mut pending = vec![edge.target.clone()];
        let mut visited = Vec::new();
        let mut recurrent = false;
        while let Some(application) = pending.pop() {
            if application == edge.source {
                recurrent = true;
                break;
            }
            if visited.contains(&application) {
                continue;
            }
            visited.push(application.clone());
            pending.extend(
                closure
                    .invocations
                    .iter()
                    .filter(|next| next.source == application)
                    .map(|next| next.target.clone()),
            );
        }
        if recurrent && !covered_recursive_edge(scope, edge) {
            closure_violation = Some("recursive machine-call cycle has no ordinary termination proof for its exact selected application edges".into());
            break;
        }
    }
    require_compiler_evaluation_floor(
        &root_facts.name,
        &effects,
        closure_violation.as_deref(),
        selection_violation.as_deref(),
    )?;
    Ok(AdmittedCompilerEvaluation {
        providers,
        artifact: artifact.identity(),
        target: target.to_owned(),
        declaration: root,
    })
}

/// Include state-level preconditions when admitting a constant invocation.
/// Use the same selected calls for effect checks and prerequisite checks.
pub(crate) struct CompilerInvocationScope {
    pub selected: Vec<(ProofDeclarationId, ProofDeclarationId)>,
    pub closure:
        semantic_vocabulary::proof_applications::ProofApplicationReachability<ProofDeclarationId>,
    pub dependencies: Vec<ProofDeclarationId>,
}

pub(crate) fn compiler_invocation_scope(
    scope: &ProofApplicationScope,
    target: &str,
    providers: &crate::AdmittedProviderSelections,
    root: ProofDeclarationId,
) -> Result<CompilerInvocationScope, String> {
    let root = scope
        .plan
        .callable_for_target(root, target)
        .ok_or("compiler-evaluation declaration has no selected target body")?;
    let mut selected = Vec::new();
    for (requirement, body) in providers.providers().chain(
        providers
            .assumed_bindings()
            .iter()
            .map(|binding| (binding.required, binding.implementation)),
    ) {
        selected.push((
            scope
                .callable_declaration(TargetMachineCallable::Boundary(requirement))
                .ok_or("compiler-evaluation provider requirement lost its declaration")?,
            scope
                .callable_declaration(TargetMachineCallable::Machine(body))
                .ok_or("compiler-evaluation selected body lost its declaration")?,
        ));
    }
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
    dependencies.extend(scope.plan.declaration_closure(&dependencies.clone()));
    Ok(CompilerInvocationScope {
        selected,
        closure,
        dependencies,
    })
}

fn header(
    scope: &ProofApplicationScope,
    declaration: ProofDeclarationId,
) -> Result<&terminal_psi::ProofCallableHeader, String> {
    let mut headers = scope
        .callable_headers
        .iter()
        .filter_map(|(_, header)| (header.declaration == declaration).then_some(header));
    let header = headers
        .next()
        .ok_or("compiler-evaluation declaration has no retained checked facts")?;
    if headers.next().is_some() {
        return Err("compiler-evaluation declaration has competing checked facts".into());
    }
    Ok(header)
}

fn declaration(
    scope: &ProofApplicationScope,
    declaration: ProofDeclarationId,
) -> Result<&CompilerEvaluationDeclaration, String> {
    header(scope, declaration)?
        .compiler_evaluation
        .as_ref()
        .map_err(Clone::clone)
}

fn require_selection(
    requester: DeclarationCustody,
    owner: DeclarationCustody,
    authority: &dyn DeclarationSelectionAuthority,
) -> Result<(), String> {
    match (requester, owner) {
        (DeclarationCustody::Toolchain, _) | (_, DeclarationCustody::Toolchain) => Ok(()),
        (DeclarationCustody::Package(requester), DeclarationCustody::Package(owner))
            if authority.allows_declaration_selection(requester, owner) =>
        {
            Ok(())
        }
        (DeclarationCustody::Package(requester), DeclarationCustody::Package(owner)) => {
            Err(format!(
                "build-time invocation selects package {} from package {} without direct dependency authority",
                authority.package_label(owner),
                authority.package_label(requester)
            ))
        }
        (DeclarationCustody::UnownedUser, _) | (_, DeclarationCustody::UnownedUser) => {
            Err("build-time invocation has user source without reconciled package custody".into())
        }
        _ => Err("build-time invocation lacks compiler-owned source/package provenance".into()),
    }
}

fn covered_recursive_edge(
    scope: &ProofApplicationScope,
    edge: &semantic_vocabulary::proof_applications::ProofInvocationEdge<ProofDeclarationId>,
) -> bool {
    let Some(ordinal) = edge
        .call_ordinal
        .and_then(|ordinal| u32::try_from(ordinal).ok())
    else {
        return false;
    };
    !edge.provider
        && scope.recursive_components.iter().any(|(_, component)| {
            let members = scope.recursive_members.span_or_empty(component.members);
            members.contains(&edge.source.machine())
                && members.contains(&edge.target.machine())
                && scope
                    .recursive_calls
                    .span_or_empty(component.calls)
                    .iter()
                    .any(|call| {
                        call.caller == edge.source.machine()
                            && call.callee == edge.target.machine()
                            && call.call_ordinal == ordinal
                    })
        })
}
