//! A public helper may replace a stored pointer while saved users still need
//! the original data and its guarantees. Capture those guarantees before the
//! call and keep them on that original data after a normal return.
//! These are Observed facts on exact captured subjects. They identify no new
//! pointer target, clear no unavailable exclusive roster and retain no old
//! scalar value or equation.
use super::*;

pub(in super::super) struct CapturedNormalFact {
    location: DescriptorLocation,
    carrier: StructuralTypeId,
    domain: semantic_vocabulary::StructuralDomainId,
    query: DescriptorContentsQuery,
    subject_carrier: StructuralTypeId,
}

impl CapturedNormalFact {
    pub(in super::super) fn publish(self, types: &[StructuralTypeDeclaration], after: &mut State) {
        let Some(contract) = after.values.get_mut(&self.location.root) else {
            return;
        };
        if contract.carrier != self.carrier {
            return;
        }
        let Some(path) = descriptor_path(types, contract.carrier, &self.location.path) else {
            return;
        };
        if path.is_empty() {
            contract.qualifications.push(self.domain);
            contract.qualifications.sort();
            contract.qualifications.dedup();
        } else {
            contract
                .projected_qualifications
                .push(terminal_psi::StructuralPathQualification {
                    path,
                    domain: self.domain,
                });
            contract.projected_qualifications.sort();
            contract.projected_qualifications.dedup();
        }
    }
}

impl NormalPayloadContext<'_> {
    pub(in super::super) fn captured_provider_facts(
        &self,
        analysis: &mut DescriptorAnalysis<'_>,
        caller: MachineId,
        position: DescriptorPosition,
        before: &State,
    ) -> Result<Vec<CapturedNormalFact>, &'static str> {
        let mut bodies = self.module.machines.iter().filter(|body| body.id == caller);
        let body = bodies.next().ok_or("a public normal fact has no caller")?;
        if bodies.next().is_some() {
            return Err("a public normal fact has ambiguous callers");
        }
        let operation = body
            .blocks
            .iter()
            .find(|block| block.id == position.0)
            .and_then(|block| block.operations.get(position.1 as usize))
            .ok_or("a public normal fact has no performed operation")?;
        let OperationKind::BoundaryCall {
            boundary,
            structural_arguments,
            arguments,
            ..
        } = &operation.kind
        else {
            return Ok(Vec::new());
        };
        let mut declarations = self
            .module
            .boundary_machines
            .iter()
            .filter(|row| row.id == *boundary);
        let declaration = declarations
            .next()
            .ok_or("the public normal contract is absent")?;
        if declarations.next().is_some()
            || declaration.role != terminal_psi::BoundaryDeclarationRole::Callable
            || declaration.completion != terminal_psi::BoundaryCompletion::NormalReturn
        {
            return Err("the public normal contract is ambiguous or has no callable normal return");
        }
        let machine = descriptor_machine(self.graph, caller)
            .ok_or("the public normal call graph is absent")?;
        let call = machine
            .calls
            .get(&position)
            .ok_or("the public normal invocation is absent")?;
        if call.operation != operation.id
            || structural_arguments.len() != declaration.structural_parameters.len()
            || arguments.len() != declaration.scalar_parameters.len()
            || call.argument_contents.len() != structural_arguments.len()
            || call.arguments.len() != structural_arguments.len()
        {
            return Err("the public normal contract and performed invocation disagree");
        }
        let expressions =
            super::super::super::payload_predicates::TerminalScalarExpressions::new(body);
        if arguments
            .iter()
            .zip(&declaration.scalar_parameters)
            .any(|(actual, expected)| {
                expressions
                    .expression(*actual)
                    .is_none_or(|value| value.scalar_type() != *expected)
            })
        {
            return Err("the public normal scalar arguments disagree with its signature");
        }
        let bindings = declaration
            .structural_parameters
            .iter()
            .zip(structural_arguments)
            .enumerate()
            .map(|(index, (formal, actual))| {
                super::super::super::scalar_contracts::terminal_structural_argument_binding(
                    &self.module.structural_types,
                    body,
                    formal,
                    actual,
                    index,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        if bindings.iter().any(|binding| {
            machine.place_types.get(&binding.actual.place) != Some(&binding.root_type)
                || call.arguments.get(binding.position) != Some(&binding.location)
                || call.argument_contents.get(binding.position) != Some(&binding.contents)
        }) {
            return Err("the public normal signature and captured actual coordinates disagree");
        }
        let mut facts = Vec::new();
        for demand in self
            .module
            .boundary_lender_preservation
            .iter()
            .filter(|demand| demand.machine == caller && demand.operation == operation.id)
        {
            let row = &demand.requirement;
            if !declaration.normal_qualifications.contains(row)
                || !row.path.contains(&StructuralPathSegment::Referent)
            {
                return Err("the requested lender duty is not an exact public normal row");
            }
            let index = usize::try_from(row.argument_index)
                .map_err(|_| "the public normal argument exceeds usize")?;
            let formal = declaration
                .structural_parameters
                .get(index)
                .ok_or("the public normal row has no formal")?;
            let actual = structural_arguments
                .get(index)
                .ok_or("the public normal row has no actual")?;
            if formal.access != StructuralAccess::MutableBorrow || actual.access != formal.access {
                return Err("the public normal row has no matching mutable argument");
            }
            // Universal and inactive-case subjects need their own capture
            // witnesses. A missing exact subject is unavailable, not empty.
            if row.path.iter().any(|step| {
                matches!(
                    step,
                    StructuralPathSegment::AllElements | StructuralPathSegment::Case(_)
                )
            }) {
                return Err(
                    "the requested lender subject needs an unavailable case or element capture",
                );
            }
            let mut domains = self
                .module
                .structural_domains
                .iter()
                .filter(|domain| domain.id == row.domain);
            let domain = domains.next().ok_or("the public normal domain is absent")?;
            if domains.next().is_some()
                || qualification_path_carrier(
                    self.module.structural_types.iter(),
                    formal.structural_type,
                    &row.path,
                ) != Some(domain.carrier)
            {
                return Err("the public normal domain has no unique matching carrier");
            }
            let suffix = descriptor_contents_query(
                &self.module.structural_types,
                formal.place,
                formal.structural_type,
                &row.path,
                None,
            )
            .ok_or("the public normal row has no typed subject")?;
            let binding = &bindings[index];
            let Some(mut query) = binding.contents.clone() else {
                return Err("the requested lender has no captured actual contents");
            };
            let root_type = binding.root_type;
            query.steps.extend(suffix.steps);
            query
                .validate(&self.module.structural_types, root_type)
                .ok_or("the captured public subject has no complete typed path")?;
            let Some(targets) = analysis.payload_targets(caller, position, &query) else {
                return Err("the requested lender has no complete captured target set");
            };
            let [DescriptorPayloadTarget::Storage(location)] = targets.as_slice() else {
                return Err("the requested lender has no unique concrete subject");
            };
            if location.path.iter().any(|step| {
                matches!(
                    step,
                    DescriptorPathSegment::Referent | DescriptorPathSegment::Case(_)
                )
            }) {
                return Err("the requested lender has no exact static subject path");
            }
            let Some(original) = before.contract(location.root, None) else {
                return Err("the requested lender has no pre-call payload contract");
            };
            let Some(selected) = select_payload_memberships_with_kind(
                &self.module.structural_types,
                &self.module.structural_domains,
                &original,
                &location.path,
                PayloadMembershipKind::Preserved,
            ) else {
                return Err("the requested lender has no selected pre-call contract");
            };
            if selected.carrier != domain.carrier || !selected.qualifications.contains(&row.domain)
            {
                return Err("the requested lender lacks the exact prior membership");
            }
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                eprintln!(
                    "CHECKSITE public-normal-fact-captured machine={caller:?} position={position:?} subject={location:?} domain={:?}",
                    row.domain
                );
            }
            facts.push(CapturedNormalFact {
                location: location.clone(),
                carrier: original.carrier,
                domain: row.domain,
                query,
                subject_carrier: domain.carrier,
            });
        }
        Ok(facts)
    }

    // Replacing a stored pointer need not change its old target. Establish
    // that separation from every write in the selected body, using the
    // caller's captured arguments. A public declaration or a possible target
    // alone supplies neither an exact write footprint nor this proof.
    pub(in super::super) fn require_provider_preservation(
        &mut self,
        analysis: &mut DescriptorAnalysis<'_>,
        caller: MachineId,
        position: DescriptorPosition,
        before: &State,
        implementation: Option<MachineId>,
    ) -> Result<(), &'static str> {
        let required = self.captured_provider_facts(analysis, caller, position, before)?;
        if required.is_empty() {
            return Ok(());
        }
        let implementation = implementation.ok_or(
            "captured lender preservation requires an independently checked execution body",
        )?;
        let caller_graph = descriptor_machine(self.graph, caller)
            .ok_or("captured lender preservation has no caller graph")?;
        let call = caller_graph
            .calls
            .get(&position)
            .ok_or("captured lender preservation has no performed invocation")?;
        let body = self.executable_body(implementation)?;
        let operation = self
            .executable_body(caller)?
            .blocks
            .iter()
            .find(|block| block.id == position.0)
            .and_then(|block| block.operations.get(position.1 as usize))
            .ok_or("the provider preservation call has no operation")?;
        let OperationKind::BoundaryCall {
            boundary,
            arguments,
            ..
        } = &operation.kind
        else {
            return Err("provider preservation requires a boundary call");
        };
        let declaration =
            terminal_psi::find_boundary_machine(&self.module.boundary_machines, *boundary)
                .ok_or("provider preservation has no public declaration")?;
        if body.structural_parameters.len() != call.argument_contents.len()
            || body.parameters.len() != arguments.len()
            || !body
                .parameters
                .iter()
                .map(|parameter| parameter.scalar_type)
                .eq(declaration.scalar_parameters.iter().copied())
            || body.structural_parameters.len() != declaration.structural_parameters.len()
            || body
                .structural_parameters
                .iter()
                .zip(&declaration.structural_parameters)
                .any(|(actual, expected)| {
                    actual.structural_type != expected.structural_type
                        || actual.access != expected.access
                        || actual.multiplicity != expected.multiplicity
                })
        {
            return Err("the selected provider and captured argument roster disagree");
        }
        let expressions = super::super::super::payload_predicates::TerminalScalarExpressions::new(
            self.executable_body(caller)?,
        );
        let mut scalars = BTreeMap::new();
        for (formal, actual) in body.parameters.iter().zip(arguments) {
            let value = expressions
                .expression(*actual)
                .filter(|value| value.scalar_type() == formal.scalar_type)
                .ok_or("the selected provider scalar writer has no matching actual value")?;
            if scalars.insert(formal.id, value).is_some() {
                return Err("the selected provider scalar formals are ambiguous");
            }
        }
        let mut writes = Vec::new();
        for effect in self.body_writes(self.graph, analysis, implementation)? {
            let mut query = call
                .argument_contents
                .get(effect.parameter)
                .and_then(Option::as_ref)
                .ok_or("a selected provider write has no captured actual")?
                .clone();
            query.steps.extend(effect.steps);
            query
                .validate(
                    &self.module.structural_types,
                    *caller_graph
                        .place_types
                        .get(&query.root)
                        .ok_or("the selected provider actual has no carrier")?,
                )
                .ok_or("the selected provider write has no complete typed projection")?;
            let performed = effect.performed.and_then(|write| write.bound(&scalars));
            let targets = analysis
                .unchanged_payload_targets(caller, position, &query)
                .ok_or("the selected provider write has no complete unchanged footprint")?;
            writes.extend(
                targets
                    .into_iter()
                    .map(|target| (target, performed.clone())),
            );
        }
        for fact in required {
            // A predicate on an inline payload has no separately backed
            // subject which this storage-only separation could overlook.
            if !plain_owned_shape(self.module.structural_types.iter(), fact.subject_carrier) {
                return Err("the captured lender contains separately backed payloads");
            }
            if analysis.exact_payload_target(caller, position, &fact.query)
                != Some(fact.location.clone())
            {
                return Err("the captured lender has no exact unchanged storage presentation");
            }
            let original = before
                .contract(fact.location.root, None)
                .ok_or("the captured lender lost its current pre-call contract")?;
            let selected = select_payload_memberships_with_kind(
                &self.module.structural_types,
                &self.module.structural_domains,
                &original,
                &fact.location.path,
                PayloadMembershipKind::Observed,
            )
            .ok_or("the captured lender has no current selected domain subject")?;
            for (written, performed) in &writes {
                if written.root != fact.location.root
                    || !written
                        .path
                        .iter()
                        .zip(&fact.location.path)
                        .all(|(left, right)| left == right)
                {
                    continue;
                }
                let relative = written
                    .path
                    .strip_prefix(fact.location.path.as_slice())
                    .ok_or(
                        "a selected provider write replaces an ancestor of the captured lender",
                    )?;
                let super::write_conservation::PerformedWrite::Primitive(write) =
                    performed.as_ref().map_err(|reason| *reason)?
                else {
                    return Err("a selected provider write may change the captured lender");
                };
                super::primitive_writes::preserves_domain(
                    self.module, &selected, fact.domain, relative, &write.value,
                ).inspect_err(|reason| {
                    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                        eprintln!("CHECKSITE primitive-lender-not-preserved writer={:?} position={:?} subject={:?} reason={reason}", write.machine, write.position, fact.location);
                    }
                })?;
            }
        }
        Ok(())
    }
}

/// A boundary may replace a stored pointer while the old lender still has a
/// standing domain duty. Check the selected body's complete writes against
/// the subjects captured at each call. An absent body supplies no such proof.
/// This establishes no new domain and returns no serialized proof receipt.
pub fn require_boundary_lender_preservation(
    module: &TerminalModule,
    boundary: semantic_vocabulary::BoundaryMachineId,
    implementation: Option<MachineId>,
) -> Result<(), &'static str> {
    terminal_psi::find_boundary_machine(&module.boundary_machines, boundary)
        .ok_or("lender preservation has no boundary declaration")?;
    super::reference_forwarding::require(module, boundary, implementation)?;
    if module.boundary_lender_preservation.is_empty() {
        return Ok(());
    }
    let callers = module
        .machines
        .iter()
        .filter(|machine| {
            machine.operations().any(|operation| {
                matches!(operation.kind, OperationKind::BoundaryCall { boundary: selected, .. }
                    if selected == boundary)
                    && module.boundary_lender_preservation.iter().any(|demand| {
                        demand.machine == machine.id && demand.operation == operation.id
                    })
            })
        })
        .collect::<Vec<_>>();
    if callers.is_empty() {
        return Ok(());
    }
    let graph = super::super::super::terminal_graph_closure(
        module,
        callers.iter().map(|body| body.id).chain(implementation),
        DescriptorSlotKind::BorrowedStorage,
    )
    .ok_or("lender preservation has no complete borrowed-storage invocation graph")?;
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    let mut normal = NormalPayloadContext::new(module, &graph);
    for caller in callers {
        let mut history = PayloadMembershipHistory::for_module(module, &graph, caller);
        let feasible = analysis.flow_feasibility(caller.id);
        for block in &caller.blocks {
            for (ordinal, operation) in block.operations.iter().enumerate() {
                let OperationKind::BoundaryCall {
                    boundary: selected,
                    structural_arguments,
                    ..
                } = &operation.kind
                else {
                    continue;
                };
                if *selected != boundary {
                    continue;
                }
                let position = (
                    block.id,
                    u32::try_from(ordinal).map_err(|_| "the lender call position exceeds u32")?,
                );
                match feasible.get(&position) {
                    Some(FlowFeasibility::ProvedImpossible) => continue,
                    Some(FlowFeasibility::Possible) => {}
                    _ => return Err("the lender call has unknown reachability"),
                }
                let first = structural_arguments
                    .first()
                    .ok_or("the lender call has no structural argument")?;
                // The replay can reach this cut even when the queried carrier
                // itself has no surviving contract. Consume the actual state,
                // never a declaration or a later descriptor generation.
                let _ = history.current_payload_contract_in(&mut analysis, position, first.place);
                let before = history
                    .replay
                    .cached
                    .as_ref()
                    .and_then(|cached| cached.snapshots.get(&position))
                    .ok_or("the lender call has no reached pre-call payload state")?;
                normal.require_provider_preservation(
                    &mut analysis,
                    caller.id,
                    position,
                    before,
                    implementation,
                )?;
            }
        }
    }
    Ok(())
}
