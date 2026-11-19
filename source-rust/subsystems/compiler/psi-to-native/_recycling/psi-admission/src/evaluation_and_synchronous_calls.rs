/*
Compile-time evaluation admission (`compiler_evaluation`).
*/

pub(crate) mod compiler_evaluation {
    use crate::declaration_selection::require_declaration_selection as require_selection;
    use semantic_vocabulary::compiler_evaluation::{
        CompilerEvaluationDeclaration, CompilerEvaluationEffects, DeclarationCustody,

    };
    use declaration_scope::DeclarationSelectionAuthority;
    use crate::evaluation_requirements::require_compiler_evaluation_floor;
    use terminal_psi::{ProofApplicationScope, ProofDeclarationId, TargetMachineCallable};


    #[derive(Debug, Clone)]
    pub struct AdmittedCompilerEvaluation {
        providers: crate::AdmittedProviderSelections,
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
        pub fn providers(&self) -> &crate::AdmittedProviderSelections {
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
    }

    pub fn admit_compiler_evaluation<'code, 'target>(
        artifact: impl Into<terminal_codec::TerminalExecutionView<'code>>,
        target: impl Into<Option<&'target str>>,
        root: ProofDeclarationId,
        providers: crate::AdmittedProviderSelections,
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
        providers: crate::AdmittedProviderSelections,
        requester: DeclarationCustody,
        authority: Option<&dyn DeclarationSelectionAuthority>,
        source_consent: Option<&dyn InvocationSourceSelection>,
    ) -> Result<AdmittedCompilerEvaluation, String> {
        let artifact = artifact.into();
        let target = target.into();
        if providers.execution_identity() != &artifact.custody_identity() {
            return Err(
                "compiler-evaluation provider installation names a different artifact".into(),
            );
        }
        let scope = &artifact.module().scalar_applications.proof_scope;
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
        let selection_violation = require_invocation_selections(
            artifact.module(),
            &closure,
            root,
            requester,
            authority,
            source_consent.map(|consent| (artifact, consent)),
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

    pub(crate) struct CompilerInvocationScope {
        pub selected: Vec<(ProofDeclarationId, ProofDeclarationId)>,
        pub closure: semantic_vocabulary::proof_applications::ProofApplicationReachability<
            ProofDeclarationId,
        >,
        pub dependencies: Vec<ProofDeclarationId>,
    }

    /// Admit the declarations an execution can select without imposing the
    /// effect-free floor of a constant evaluator. Build has separately admitted
    /// host effects, but still owes the same exact package-custody checks.
    pub fn admit_declaration_selections(
        artifact: terminal_codec::TerminalExecutionView<'_>,
        target: Option<&str>,
        entry: TargetMachineCallable,
        providers: &crate::AdmittedProviderSelections,
        requester: DeclarationCustody,
        authority: &dyn DeclarationSelectionAuthority,
    ) -> Result<(), String> {
        if providers.execution_identity() != &artifact.custody_identity() {
            return Err(
                "declaration admission provider installation names a different artifact".into(),
            );
        }
        let scope = &artifact.module().scalar_applications.proof_scope;
        let root = scope
            .callable_declaration(entry)
            .ok_or("declaration admission entry lost its retained declaration")?;
        let root = scope
            .plan
            .callable_for_target(root, target)
            .ok_or("declaration admission entry has no selected target body")?;
        let invocation = compiler_invocation_scope(scope, target, providers, root)?;
        require_invocation_selections(
            artifact.module(),
            &invocation.closure,
            root,
            requester,
            Some(authority),
            None,
        )
    }

    fn require_scope_selections(
        scope: &ProofApplicationScope,
        closure: &semantic_vocabulary::proof_applications::ProofApplicationReachability<
            ProofDeclarationId,
        >,
        root: ProofDeclarationId,
        requester: DeclarationCustody,
        authority: Option<&dyn DeclarationSelectionAuthority>,
        source_consent: Option<(
            terminal_codec::TerminalExecutionView<'_>,
            &dyn InvocationSourceSelection,
        )>,
    ) -> Result<(), String> {
        require_invocation_selection(
            (requester, None),
            (declaration(scope, root)?.custody, root),
            authority,
            source_consent,
        )?;
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
        closure: &semantic_vocabulary::proof_applications::ProofApplicationReachability<
            ProofDeclarationId,
        >,
        root: ProofDeclarationId,
        requester: DeclarationCustody,
        authority: Option<&dyn DeclarationSelectionAuthority>,
        source_consent: Option<(
            terminal_codec::TerminalExecutionView<'_>,
            &dyn InvocationSourceSelection,
        )>,
    ) -> Result<(), String> {
        let scope = &module.scalar_applications.proof_scope;
        require_scope_selections(scope, closure, root, requester, authority, source_consent)?;
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
                        let (
                            DeclarationCustody::Package(caller),
                            DeclarationCustody::Package(body),
                        ) = (source.custody, target.custody)
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
                                .binary_search_by_key(
                                    &(request.requester, request.owner),
                                    |request| (request.requester, request.owner),
                                )
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
                require_invocation_selection(
                    (source.custody, Some(edge.source.machine())),
                    (target.custody, edge.target.machine()),
                    authority,
                    source_consent,
                )?;
            }
        }
        Ok(())
    }

    // Source call summaries put direct calls before boundary calls. Prepare
    // that same canonical order once, retaining the exact public application;
    // an unrelated call or a global body membership cannot supply this join.
    fn selected_call_request_sources(
        module: &terminal_psi::TerminalModule,
        closure: &semantic_vocabulary::proof_applications::ProofApplicationReachability<
            ProofDeclarationId,
        >,
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
        let scope = &module.scalar_applications.proof_scope;
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
        use semantic_vocabulary::compiler_evaluation::DeclarationAuthorityRequest;
        use semantic_vocabulary::proof_applications::ProofApplicationReachability;
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
                None,
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
        assert!(
            require_compiler_evaluation_floor(&facts.name, &facts.effects, None, None).is_err()
        );
    }

    pub(crate) fn compiler_invocation_scope<'target>(
        scope: &ProofApplicationScope,
        target: impl Into<Option<&'target str>>,
        providers: &crate::AdmittedProviderSelections,
        root: ProofDeclarationId,
    ) -> Result<CompilerInvocationScope, String> {
        let target = target.into();
        let root = match target {
            Some(target) => scope.plan.callable_for_target(root, target),
            None => scope.plan.callable_without_target(root),
        }
        .ok_or("compiler-evaluation declaration has no body in the requested target scope")?;
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
                    return Err(
                        "target-independent evaluation demands a target-selected body".into(),
                    );
                }
            }
        }
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
            && owner.0 == DeclarationCustody::UnownedUser
            && source_consent.is_some_and(|(artifact, consent)| {
                artifact.module().identities.invocation_scope().is_some()
                    && consent.allows_selection(artifact, requester.1, owner.1)
            })
        {
            return Ok(());
        }
        require_selection(requester.0, owner.0, authority)
    }

    #[test]
    fn closed_contract_custody_never_substitutes_for_a_package_edge() {
        struct AllowPackages;
        impl DeclarationSelectionAuthority for AllowPackages {
            fn allows_declaration_selection(
                &self,
                _: semantic_vocabulary::PackageKeyIdentity,
                _: semantic_vocabulary::PackageKeyIdentity,
            ) -> bool {
                true
            }
            fn package_label(&self, _: semantic_vocabulary::PackageKeyIdentity) -> String {
                "package".into()
            }
        }
        let package = DeclarationCustody::Package(
            semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]).unwrap(),
        );
        let contract = DeclarationCustody::ToolchainContract;
        let core = DeclarationCustody::Toolchain;
        for (requester, owner) in [
            (package, contract),
            (contract, package),
            (DeclarationCustody::Missing, contract),
        ] {
            assert!(require_selection(requester, owner, Some(&AllowPackages)).is_err());
        }
        for (requester, owner) in [
            (contract, contract),
            (contract, core),
            (core, contract),
            (package, package),
        ] {
            assert!(require_selection(requester, owner, Some(&AllowPackages)).is_ok());
        }
        let other = DeclarationCustody::Package(
            semantic_vocabulary::PackageKeyIdentity::from_digest([2; 32]).unwrap(),
        );
        assert!(require_selection(package, package, None).is_ok());
        assert!(require_selection(package, other, None).is_err());
        assert!(require_selection(package, other, Some(&AllowPackages)).is_ok());
        assert!(require_selection(DeclarationCustody::Missing, core, None).is_err());
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
}
