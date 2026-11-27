//! Admit a published declaration for compiler evaluation using its exact artifact,
//! target, provider selections, and the requester's package permissions.
//! The interpreter consumes the receipt when it starts that invocation.
use declaration_scope::DeclarationSelectionAuthority;
use declaration_scope::require_declaration_selection as require_selection;
use psi_provider_selections::AdmittedProviderSelections;
use semantic_vocabulary::compiler_evaluation::{
    CompilerEvaluationDeclaration, CompilerEvaluationEffects, DeclarationCustody,
};
use terminal_psi::{ProofApplicationScope, ProofDeclarationId, TargetMachineCallable};
use terminal_semantics::evaluation_scope::{CompilerInvocationScope, compiler_invocation_scope};

#[derive(Debug, Clone)]
pub struct AdmittedCompilerEvaluation {
    providers: AdmittedProviderSelections,
    artifact: terminal_codec::TerminalExecutionIdentity,
    target: Option<String>,
    declaration: ProofDeclarationId,
    requester: DeclarationCustody,
}

impl AdmittedCompilerEvaluation {
    pub const fn requester(&self) -> DeclarationCustody {
        self.requester
    }
    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }
    pub fn providers(&self) -> &AdmittedProviderSelections {
        &self.providers
    }
    pub fn require_invocation<'code, 'target>(
        &self,
        artifact: impl Into<terminal_codec::TerminalExecutionView<'code>>,
        target: impl Into<Option<&'target str>>,
        entry: TargetMachineCallable,
    ) -> Result<(), String> {
        let target = target.into();
        let artifact = artifact.into();
        let declaration = artifact
            .module()
            .scalars
            .scalar_applications
            .proof_scope
            .callable_declaration(entry);
        if self.artifact != artifact.custody_identity()
            || self.target.as_deref() != target
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

/// Consent to select unowned source inside this exact private invocation.
/// This is not a declaration owner, package edge or execution admission.
pub trait InvocationSourceSelection {
    fn allows_selection(
        &self,
        artifact: terminal_codec::TerminalExecutionView<'_>,
        requester: Option<ProofDeclarationId>,
        declaration: ProofDeclarationId,
    ) -> bool;

    fn allows_supplied_library(
        &self,
        _artifact: terminal_codec::TerminalExecutionView<'_>,
        _requester: Option<ProofDeclarationId>,
        _declaration: ProofDeclarationId,
    ) -> bool {
        false
    }
}

pub fn admit_compiler_evaluation<'code, 'target>(
    artifact: impl Into<terminal_codec::TerminalExecutionView<'code>>,
    target: impl Into<Option<&'target str>>,
    root: ProofDeclarationId,
    providers: AdmittedProviderSelections,
    requester: DeclarationCustody,
    authority: Option<&dyn DeclarationSelectionAuthority>,
) -> Result<AdmittedCompilerEvaluation, String> {
    admit_compiler_evaluation_with_source_consent(
        artifact, target, root, providers, requester, authority, None,
    )
}

pub fn admit_compiler_evaluation_with_source_consent<'code, 'target>(
    artifact: impl Into<terminal_codec::TerminalExecutionView<'code>>,
    target: impl Into<Option<&'target str>>,
    root: ProofDeclarationId,
    providers: AdmittedProviderSelections,
    requester: DeclarationCustody,
    authority: Option<&dyn DeclarationSelectionAuthority>,
    source_consent: Option<&dyn InvocationSourceSelection>,
) -> Result<AdmittedCompilerEvaluation, String> {
    let artifact = artifact.into();
    let target = target.into();
    if providers.execution_identity() != &artifact.custody_identity() {
        return Err("compiler-evaluation provider installation names a different artifact".into());
    }
    let scope = &artifact.module().scalars.scalar_applications.proof_scope;
    let root = match target {
        Some(target) => scope.plan.callable_for_target(root, target),
        None => scope.plan.callable_without_target(root),
    }
    .ok_or("compiler-evaluation declaration has no body in the requested target scope")?;
    let root_facts = evaluation_declaration(scope, root)?;
    if root_facts.entry_requires {
        return Err(format!(
            "machine `{}` has an authored `requires` premise; no checked invocation proof for that premise",
            root_facts.name
        ));
    }
    terminal_semantics::evaluation_crashes::require_crash_free_evaluation(artifact.module(), root)?;
    let invocation = compiler_invocation_scope(scope, target, providers.providers(), root)?;
    let CompilerInvocationScope {
        selected,
        closure,
        dependencies,
    } = invocation;
    for (_, owner) in scope.pending_evaluation_conditions.iter() {
        if owner.0 == 0 || dependencies.contains(owner) {
            return Err("constant invocation retains a pending target proof condition in its demanded closure; it cannot establish its own pending target condition".into());
        }
    }
    let mut effects = CompilerEvaluationEffects::default();
    let mut closure_violation = None;
    let selection_violation = require_invocation_selections(
        artifact.module(),
        &closure,
        root,
        requester,
        authority,
        |(requester, owner, _)| {
            require_invocation_selection(
                requester,
                owner,
                authority,
                source_consent.map(|consent| (artifact, consent)),
            )
        },
    )
    .err();
    for &machine in &closure.machines {
        let facts = evaluation_declaration(scope, machine)?;
        for service in &facts.effects.services {
            if !effects.services.contains(service) {
                effects.services.push(service.clone());
            }
        }
        effects.may_suspend |= facts.effects.may_suspend;
        effects.may_block |= facts.effects.may_block;
        if closure_violation.is_none() {
            closure_violation = facts.linear_carrier.clone();
        }
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
    }
    for edge in &closure.invocations {
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
        artifact: artifact.custody_identity(),
        target: target.map(str::to_owned),
        declaration: root,
        requester,
    })
}

/// Admit the declarations an execution can select without imposing the
/// effect-free floor of a constant evaluator. Build has separately admitted
/// host effects, but still owes the same exact package-custody checks.
pub fn admit_declaration_selections(
    artifact: terminal_codec::TerminalExecutionView<'_>,
    target: Option<&str>,
    entry: TargetMachineCallable,
    providers: &AdmittedProviderSelections,
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
    authority: Option<&dyn DeclarationSelectionAuthority>,
) -> Result<CapturedBuildSelections, String> {
    if providers.execution_identity() != &artifact.custody_identity() {
        return Err(
            "declaration admission provider installation names a different artifact".into(),
        );
    }
    let scope = &artifact.module().scalars.scalar_applications.proof_scope;
    let root = scope
        .callable_declaration(entry)
        .ok_or("declaration admission entry lost its retained declaration")?;
    let root = scope
        .plan
        .callable_for_target(root, target)
        .ok_or("declaration admission entry has no selected target body")?;
    let invocation = compiler_invocation_scope(scope, target, providers.providers(), root)?;
    let requester = match package {
        Some(key) => DeclarationCustody::Package(key),
        None => match declaration(scope, root)?.custody {
            custody @ (DeclarationCustody::UnownedUser
            | DeclarationCustody::Toolchain
            | DeclarationCustody::ToolchainContract) => custody,
            _ => return Err("Build entry has no admitted package or captured source".into()),
        },
    };
    let mut captured = CapturedBuildSelections::default();
    let mut requests = Vec::new();
    require_invocation_selections(
        artifact.module(),
        &invocation.closure,
        root,
        requester,
        authority,
        |(from, to, ordinal)| {
            if package.is_none()
                && from.0 == DeclarationCustody::UnownedUser
                && matches!(
                    to.0,
                    DeclarationCustody::UnownedUser
                        | DeclarationCustody::Toolchain
                        | DeclarationCustody::Package(_)
                )
            {
                if let Some(caller) = from.1 {
                    requests.push((caller, ordinal, to.1));
                } else if to.1 == root && to.0 == DeclarationCustody::UnownedUser {
                    captured.entry = Some(entry);
                } else {
                    return Err("captured Build root changed its declaration".into());
                }
                Ok(())
            } else {
                require_selection(from.0, to.0, authority)
            }
        },
    )?;
    captured.calls = build_calls(artifact.module(), requests)?;
    Ok(captured)
}

/// The source companion must discharge each selection before execution.
#[must_use]
#[derive(Default)]
pub struct CapturedBuildSelections {
    pub entry: Option<TargetMachineCallable>,
    pub calls: Vec<CapturedCall>,
}

type DeclarationSelection = (
    (DeclarationCustody, Option<ProofDeclarationId>),
    (DeclarationCustody, ProofDeclarationId),
    Option<usize>,
);

fn require_scope_selections(
    scope: &ProofApplicationScope,
    closure: &proof_applications::ProofApplicationReachability<ProofDeclarationId>,
    root: ProofDeclarationId,
    requester: DeclarationCustody,
    authority: Option<&dyn DeclarationSelectionAuthority>,
    mut select: impl FnMut(DeclarationSelection) -> Result<(), String>,
) -> Result<(), String> {
    select((
        (requester, None),
        (declaration(scope, root)?.custody, root),
        None,
    ))?;
    for &machine in &closure.machines {
        let facts = declaration(scope, machine)?;
        for request in facts.selections.as_ref().map_err(Clone::clone)? {
            require_selection(
                DeclarationCustody::Package(request.requester),
                DeclarationCustody::Package(request.owner),
                authority,
            )?;
        }
    }
    Ok(())
}

fn require_invocation_selections(
    module: &terminal_psi::TerminalModule,
    closure: &proof_applications::ProofApplicationReachability<ProofDeclarationId>,
    root: ProofDeclarationId,
    requester: DeclarationCustody,
    authority: Option<&dyn DeclarationSelectionAuthority>,
    mut select: impl FnMut(DeclarationSelection) -> Result<(), String>,
) -> Result<(), String> {
    let scope = &module.scalars.scalar_applications.proof_scope;
    require_scope_selections(scope, closure, root, requester, authority, &mut select)?;
    let selected = selected_call_request_sources(module, closure)?;
    for edge in &closure.invocations {
        if !edge.provider && !edge.static_delegation {
            let source = declaration(scope, edge.source.machine())?;
            let target = declaration(scope, edge.target.machine())?;
            if let Some(ordinal) = edge.call_ordinal {
                let key = (edge.source.machine(), ordinal);
                if let Ok(position) = selected
                    .binary_search_by_key(&key, |(caller, ordinal, _, _)| (*caller, *ordinal))
                {
                    let (_, _, callee, supplier) = selected[position];
                    if callee != edge.target.machine() {
                        return Err("selected invocation names another canonical call".into());
                    }
                    let (DeclarationCustody::Package(caller), DeclarationCustody::Package(body)) =
                        (source.custody, target.custody)
                    else {
                        return Err("selected invocation lost its exact package custody".into());
                    };
                    let requests = source.selections.as_ref().map_err(Clone::clone)?;
                    for request in [
                        semantic_vocabulary::compiler_evaluation::DeclarationAuthorityRequest {
                            requester: caller,
                            owner: supplier,
                        },
                        semantic_vocabulary::compiler_evaluation::DeclarationAuthorityRequest {
                            requester: supplier,
                            owner: body,
                        },
                    ] {
                        if requests
                            .binary_search_by_key(&(request.requester, request.owner), |request| {
                                (request.requester, request.owner)
                            })
                            .is_err()
                        {
                            return Err("selected invocation lost its supplier's admitted source-selection requests".into());
                        }
                    }
                    // These exact requests were admitted above. The source
                    // chose the map; its supplier chose this body. Do not
                    // charge that physical edge as a new authored choice.
                    continue;
                }
            }
            select((
                (source.custody, Some(edge.source.machine())),
                (target.custody, edge.target.machine()),
                edge.call_ordinal,
            ))?;
        }
    }
    Ok(())
}

// For a graphless Build, resolve each actually required selection to its
// physical operation. Equal declarations mentioned at different sites do
// not confer one another's capture consent.
fn build_calls(
    module: &terminal_psi::TerminalModule,
    mut requests: Vec<(ProofDeclarationId, Option<usize>, ProofDeclarationId)>,
) -> Result<Vec<CapturedCall>, String> {
    use semantic_vocabulary::{CallableInvocationSite, InvocationSite};
    if requests.is_empty() {
        return Ok(Vec::new());
    }
    let scope = &module.scalars.scalar_applications.proof_scope;
    let mut callables = scope
        .callables
        .iter()
        .filter_map(|(_, row)| match row {
            terminal_psi::ProofCallableAssociation::Callable {
                declaration,
                callable,
            } => Some((*declaration, *callable)),
            _ => None,
        })
        .collect::<Vec<_>>();
    callables.sort_unstable_by_key(|&(declaration, _)| declaration);
    let callable = |id| {
        let first = callables.partition_point(|&(declaration, _)| declaration < id);
        let &(declaration, callable) = callables.get(first)?;
        (declaration == id && callables.get(first + 1).is_none_or(|row| row.0 != id))
            .then_some(callable)
    };
    requests.sort_unstable();
    requests.dedup();
    let mut result = Vec::with_capacity(requests.len());
    for group in requests.chunk_by(|a, b| a.0 == b.0) {
        let Some(TargetMachineCallable::Machine(caller)) = callable(group[0].0) else {
            return Err("captured Build source has no unique calling machine".into());
        };
        let machine = module
            .machine(caller)
            .ok_or("captured Build caller has no body")?;
        let sites = machine
            .module_invocations()
            .map(|call| {
                (
                    call.site,
                    TargetMachineCallable::Machine(call.inputs.callee),
                )
            })
            .chain(
                machine
                    .boundary_invocations()
                    .map(|call| (call.site, TargetMachineCallable::Boundary(call.boundary))),
            )
            .collect::<Vec<_>>();
        for &(_, ordinal, declaration) in group {
            let target = callable(declaration)
                .ok_or("captured Build call has no unique target declaration")?;
            let site = if let Some(ordinal) = ordinal {
                let &(site, selected) = sites
                    .get(ordinal)
                    .ok_or("captured Build call has no emitted occurrence")?;
                if selected != target {
                    return Err("captured Build call selects a different declaration at its emitted occurrence".into());
                }
                Some(match site {
                    InvocationSite::Cleanup {
                        edge,
                        action_ordinal,
                    } => CallableInvocationSite::CleanupAction {
                        edge,
                        action_ordinal,
                    },
                    _ => CallableInvocationSite::Invocation(site),
                })
            } else {
                // The proof closure lists cleanup and machine-value targets
                // without an ordinal. Discharge every matching cleanup action
                // and require a descriptor if this is also a dynamic target.
                let target_is_dynamic = scope.plan.machines.iter().any(|(_, row)| {
                    row.machine == group[0].0
                        && scope
                            .plan
                            .symbols
                            .span_or_empty(row.dynamic_realizations)
                            .contains(&declaration)
                });
                let mut cleanup = false;
                for &(ref site, selected) in &sites {
                    if selected == target
                        && let InvocationSite::Cleanup {
                            edge,
                            action_ordinal,
                        } = *site
                    {
                        result.push((
                            caller,
                            Some(CallableInvocationSite::CleanupAction {
                                edge,
                                action_ordinal,
                            }),
                            target,
                        ));
                        cleanup = true;
                    }
                }
                if !target_is_dynamic && cleanup {
                    continue;
                }
                if !target_is_dynamic {
                    return Err(
                        "captured Build selection has neither a cleanup action nor a machine value"
                            .into(),
                    );
                }
                None
            };
            result.push((caller, site, target));
        }
    }
    Ok(result)
}

type CapturedCall = (
    semantic_vocabulary::MachineId,
    Option<semantic_vocabulary::CallableInvocationSite>,
    TargetMachineCallable,
);

// Source call summaries put direct calls before boundary calls. Prepare
// that same canonical order once, retaining the exact public application;
// an unrelated call or a global body membership cannot supply this join.
fn selected_call_request_sources(
    module: &terminal_psi::TerminalModule,
    closure: &proof_applications::ProofApplicationReachability<ProofDeclarationId>,
) -> Result<
    Vec<(
        ProofDeclarationId,
        usize,
        ProofDeclarationId,
        semantic_vocabulary::PackageKeyIdentity,
    )>,
    String,
> {
    use terminal_semantics::closed_conformances::{
        declaration_package, dispatch_application, dispatch_applications,
    };
    let scope = &module.scalars.scalar_applications.proof_scope;
    let mut declarations = scope
        .callables
        .iter()
        .filter_map(|(_, row)| match row {
            terminal_psi::ProofCallableAssociation::Callable {
                declaration,
                callable: TargetMachineCallable::Machine(machine),
            } => Some((*machine, *declaration)),
            _ => None,
        })
        .collect::<Vec<_>>();
    declarations.sort_unstable();
    if declarations.windows(2).any(|rows| rows[0].0 == rows[1].0) {
        return Err("selected invocation has ambiguous emitted declaration custody".into());
    }
    let mut bodies = declarations
        .iter()
        .map(|(machine, declaration)| (*declaration, *machine))
        .collect::<Vec<_>>();
    bodies.sort_unstable();
    if bodies.windows(2).any(|rows| rows[0].0 == rows[1].0) {
        return Err("selected invocation has competing body associations".into());
    }
    let mut custodies = closure
        .machines
        .iter()
        .map(|&declaration_id| {
            declaration(scope, declaration_id).map(|facts| (declaration_id, facts.custody))
        })
        .collect::<Result<Vec<_>, _>>()?;
    custodies.sort_unstable_by_key(|(declaration, _)| *declaration);
    let custody = |declaration| {
        custodies
            .binary_search_by_key(&declaration, |(declared, _)| *declared)
            .map(|position| custodies[position].1)
            .map_err(|_| "selected invocation has no reached declaration custody")
    };
    let applications = dispatch_applications(module);
    let mut selected = Vec::new();
    for &caller in &closure.machines {
        let position = bodies
            .binary_search_by_key(&caller, |(declaration, _)| *declaration)
            .map_err(|_| "selected invocation lost its canonical caller")?;
        let machine = module
            .machine(bodies[position].1)
            .ok_or("selected invocation lost its caller body")?;
        let before = selected.len();
        for (ordinal, call) in machine.module_invocations().enumerate() {
            let Some(required) = call.inputs.required_contract else {
                continue;
            };
            let public = module
                .invocation_machine_contract(call.inputs.callee, Some(required))
                .ok_or("selected invocation lost its exact public/body association")?;
            let dispatch = public
                .selected_application
                .as_ref()
                .ok_or("selected invocation lost its closed application")?;
            let application =
                dispatch_application(&applications, machine.id, dispatch, call.inputs.callee)
                    .map_err(str::to_owned)?;
            let position = declarations
                .binary_search_by_key(&call.inputs.callee, |(machine, _)| *machine)
                .map_err(|_| "selected invocation lost its callee declaration")?;
            let target = declarations[position].1;
            if let (DeclarationCustody::Package(source), DeclarationCustody::Package(body)) =
                (custody(caller)?, custody(target)?)
                && source != body
            {
                selected.push((caller, ordinal, target, application.declaration_identity));
            }
        }
        if selected.len() != before {
            let requests = declaration(scope, caller)?
                .selections
                .as_ref()
                .map_err(Clone::clone)?;
            if requests.windows(2).any(|pair| {
                (pair[0].requester, pair[0].owner) >= (pair[1].requester, pair[1].owner)
            }) {
                return Err("selected invocation requests are not in canonical order".into());
            }
        }
    }
    let mut identities = selected
        .iter()
        .map(|(_, _, _, identity)| *identity)
        .collect::<Vec<_>>();
    identities.sort_unstable();
    identities.dedup();
    let packages = identities
        .into_iter()
        .map(|identity| {
            declaration_package(module, identity)
                .map(|package| (identity, package))
                .map_err(str::to_owned)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut requests = Vec::new();
    for (caller, ordinal, target, identity) in selected {
        let position = packages
            .binary_search_by_key(&identity, |(identity, _)| *identity)
            .map_err(|_| "selected invocation lost its declaration package")?;
        if let Some(package) = packages[position].1 {
            let source = custody(caller)?;
            let body = custody(target)?;
            if source != DeclarationCustody::Package(package)
                && body != DeclarationCustody::Package(package)
            {
                requests.push((caller, ordinal, target, package));
            }
        }
    }
    requests.sort_unstable();
    Ok(requests)
}

#[test]
fn declaration_admission_preserves_authority_without_a_pure_effect_floor() {
    use proof_applications::ProofApplicationReachability;
    use semantic_vocabulary::compiler_evaluation::DeclarationAuthorityRequest;
    let root = ProofDeclarationId(1);
    let package = semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]).unwrap();
    let other = semantic_vocabulary::PackageKeyIdentity::from_digest([2; 32]).unwrap();
    let facts = CompilerEvaluationDeclaration {
        name: "build".into(),
        custody: DeclarationCustody::Package(package),
        effects: CompilerEvaluationEffects {
            services: vec!["explicit host service".into()],
            may_suspend: true,
            may_block: false,
        },
        local_terminates: false,
        entry_requires: false,
        linear_carrier: None,
        invocation_refusal: None,
        selections: Ok(Vec::new()),
    };
    let closure = ProofApplicationReachability {
        machines: vec![root],
        calls: Vec::new(),
        invocations: Vec::new(),
    };
    // This model has no invocation edges: it checks root custody and
    // source requests. Actual call associations use the source control.
    let scope_for = |facts: Result<CompilerEvaluationDeclaration, String>| {
        let mut scope = ProofApplicationScope::default();
        scope
            .callable_headers
            .insert(terminal_psi::ProofCallableHeader {
                declaration: root,
                compiler_evaluation: facts,
                ..Default::default()
            });
        scope
    };
    let check = |facts| {
        require_scope_selections(
            &scope_for(facts),
            &closure,
            root,
            DeclarationCustody::Package(package),
            None,
            |(from, to, _)| require_invocation_selection(from, to, None, None),
        )
    };
    assert!(check(Ok(facts.clone())).is_ok());
    let mut host_call = facts.clone();
    let reason = "proof scope needs the actual selected boundary correspondence";
    host_call.invocation_refusal = Some(reason.into());
    assert!(check(Ok(host_call.clone())).is_ok());
    assert_eq!(
        evaluation_declaration(&scope_for(Ok(host_call)), root).unwrap_err(),
        reason,
    );
    let mut foreign = facts.clone();
    foreign.selections = Ok(vec![DeclarationAuthorityRequest {
        requester: package,
        owner: other,
    }]);
    assert!(
        check(Ok(foreign))
            .unwrap_err()
            .contains("dependency selection authority")
    );
    let mut missing = facts.clone();
    missing.custody = DeclarationCustody::Missing;
    assert!(check(Ok(missing)).unwrap_err().contains("provenance"));
    assert_eq!(
        check(Err("lost declaration facts".into())).unwrap_err(),
        "lost declaration facts"
    );
    assert!(require_compiler_evaluation_floor(&facts.name, &facts.effects, None, None).is_err());
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

// Build checks source selection after its own provider admission.
// Pure evaluation must also carry an eligible executable closure.
fn evaluation_declaration(
    scope: &ProofApplicationScope,
    declaration_id: ProofDeclarationId,
) -> Result<&CompilerEvaluationDeclaration, String> {
    let facts = declaration(scope, declaration_id)?;
    if let Some(reason) = &facts.invocation_refusal {
        return Err(reason.clone());
    }
    Ok(facts)
}

fn require_invocation_selection(
    requester: (DeclarationCustody, Option<ProofDeclarationId>),
    owner: (DeclarationCustody, ProofDeclarationId),
    authority: Option<&dyn DeclarationSelectionAuthority>,
    source_consent: Option<(
        terminal_codec::TerminalExecutionView<'_>,
        &dyn InvocationSourceSelection,
    )>,
) -> Result<(), String> {
    if requester.0 == DeclarationCustody::UnownedUser
        && source_consent.is_some_and(|(artifact, consent)| {
            artifact.module().identities.invocation_scope().is_some()
                && match owner.0 {
                    DeclarationCustody::UnownedUser => {
                        consent.allows_selection(artifact, requester.1, owner.1)
                    }
                    DeclarationCustody::Package(_) => {
                        consent.allows_supplied_library(artifact, requester.1, owner.1)
                    }
                    _ => false,
                }
        })
    {
        return Ok(());
    }
    require_selection(requester.0, owner.0, authority)
}

fn covered_recursive_edge(
    scope: &ProofApplicationScope,
    edge: &proof_applications::ProofInvocationEdge<ProofDeclarationId>,
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

fn require_compiler_evaluation_floor(
    machine: &str,
    effects: &CompilerEvaluationEffects,
    closure_violation: Option<&str>,
    selection_violation: Option<&str>,
) -> Result<(), String> {
    let mut violations = Vec::new();
    if !effects.services.is_empty() {
        violations.push(format!("service reach [{}]", effects.services.join(", ")));
    }
    if effects.may_suspend {
        violations.push("may suspend".to_owned());
    }
    if effects.may_block {
        violations.push("may block".to_owned());
    }
    violations.extend(closure_violation.map(str::to_owned));
    violations.extend(selection_violation.map(str::to_owned));
    if violations.is_empty() {
        return Ok(());
    }
    Err(format!(
        "machine `{machine}` is not build-time admissible: {}; build-time evaluation requires empty service reach, no possible suspension or blocking, ordinary checked termination, no unadmitted linear runtime carrier, and admitted declaration-selection authority across the complete call closure",
        violations.join("; ")
    ))
}
