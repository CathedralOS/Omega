//! A helper may install a new pointer whose complete duties are known.
//! The public guarantees are only a lower bound. A conditional unchanged-value
//! proof supplies an upper bound only when every actual captured source has
//! exactly the public source roster. The two bounds must agree. Ordinary
//! helpers replay with this invocation's actual entries, never a body-only
//! cached postcondition or a private empty annotation.

use super::*;
use terminal_psi::{BoundaryReferenceDestination, BoundaryReferenceSource};

pub(in super::super) struct CompletedOutput {
    root: PlaceId,
    contract: PayloadMembershipContract,
}

impl CompletedOutput {
    pub(in super::super) fn publish(self, after: &mut State) {
        after.values.insert(self.root, self.contract);
        after
            .unavailable_preservation
            .retain(|root| *root != self.root);
    }
}

impl NormalPayloadContext<'_> {
    pub(in super::super) fn completed_call_outputs(
        &mut self,
        analysis: &mut DescriptorAnalysis<'_>,
        caller: MachineId,
        position: DescriptorPosition,
        before: &State,
    ) -> Result<Vec<CompletedOutput>, &'static str> {
        let body = self.executable_body(caller)?;
        if self.module.boundary_reference_forwarding.is_empty() {
            return Ok(Vec::new());
        }
        // Uninstantiated entry declarations promise at least these duties.
        // Only a closed entry or a call-bound replay can supply their ceiling.
        if !body.structural_parameters.is_empty() && !self.entry_is_complete(caller) {
            return Err("the completed output has no complete actual entry roster");
        }
        let operation = body
            .blocks
            .iter()
            .find(|block| block.id == position.0)
            .and_then(|block| block.operations.get(position.1 as usize))
            .ok_or("the completed output has no exact call operation")?;
        if let OperationKind::BoundaryCall {
            boundary,
            structural_arguments,
            ..
        } = &operation.kind
        {
            return self.completed_provider_outputs(
                analysis,
                body,
                position,
                before,
                operation,
                *boundary,
                structural_arguments,
            );
        }
        let expressions =
            super::super::super::payload_predicates::TerminalScalarExpressions::new(body);
        let bindings = super::super::super::scalar_contracts::terminal_call_bindings(
            self.module,
            body,
            operation,
            &expressions,
        )?;
        if !self.begin_completion(bindings.callee.id)? {
            return Err("the call-bound output replay has a recursive dependency");
        }
        let result = (|| {
            let callee = self.executable_body(bindings.callee.id)?;
            let descriptor = descriptor_machine(self.graph, callee.id)
                .ok_or("the call-bound body has no current descriptor graph")?;
            let feasible = analysis.flow_feasibility(callee.id);
            for (position, step) in &descriptor.flow {
                if !descriptor.calls.contains_key(position)
                    && (step.may_escape || descriptor.unresolved_contents_writes.contains(position))
                    && feasible.get(position) != Some(&FlowFeasibility::ProvedImpossible)
                {
                    return Err(
                        "the call-bound body has an uncertain non-call value or write effect",
                    );
                }
            }
            let mut contracts = callee
                .structural_places
                .iter()
                .map(|place| place.id)
                .filter_map(|place| {
                    terminal_payload_contract(callee, place).map(|contract| (place, contract))
                })
                .collect::<Vec<_>>();
            let mut outputs = Vec::new();
            for parameter in &bindings.parameters {
                let actual = self.actual_roster(before, parameter)?;
                let contract = contracts
                    .iter_mut()
                    .find(|(place, _)| *place == parameter.formal)
                    .ok_or("the call-bound formal has no declaration")?;
                if !reference_forwarding::covers_roster(&actual, &contract.1) {
                    return Err("the call-bound input lacks the helper's entry duties");
                }
                contract.1 = actual;
                if matches!(
                    parameter.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                ) {
                    outputs.push((
                        parameter.formal,
                        self.output_home(analysis, caller, position, parameter)?,
                    ));
                }
            }
            // The scratch declarations and summary context belong to this call.
            // Different actual rosters cannot reuse a post keyed only by body.
            let mut history = PayloadMembershipHistory::for_current_contracts(
                &self.module.structural_types,
                &self.module.structural_domains,
                self.graph,
                callee.id,
                &callee.structural_parameters,
                &contracts,
            )?;
            if outputs.is_empty() {
                return Ok(Vec::new());
            }
            history.replay.memberships = PayloadMembershipKind::Preserved;
            let mut normal = self.for_complete_entry(callee.id)?;
            let roots = outputs
                .iter()
                .map(|(formal, _)| *formal)
                .collect::<Vec<_>>();
            let exits =
                history.preserved_normal_exit_rosters_in(analysis, &roots, Some(&mut normal))?;
            let mut result = Vec::new();
            for (formal, home) in outputs {
                let mut complete = None;
                for exit in &exits {
                    let (_, contract) = exit
                        .contracts
                        .iter()
                        .find(|(root, _)| *root == formal)
                        .ok_or("the call-bound exit lost its output")?;
                    let contract = roster_only(contract.clone());
                    if complete.as_ref().is_some_and(|previous| {
                        !reference_forwarding::same_roster(previous, &contract)
                    }) {
                        return Err("the call-bound exits have different complete output rosters");
                    }
                    complete = Some(contract);
                }
                result.push(CompletedOutput {
                    root: home.root,
                    contract: complete.ok_or("the call-bound output has no normal exit")?,
                });
            }
            Ok(result)
        })();
        self.end_completion(bindings.callee.id)?;
        result
    }

    fn actual_roster(
        &self,
        before: &State,
        binding: &super::super::super::scalar_contracts::TerminalCallParameterBinding,
    ) -> Result<PayloadMembershipContract, &'static str> {
        let location = binding
            .location
            .as_ref()
            .ok_or("the captured actual has no exact value path")?;
        let root = before
            .contract(location.root, None)
            .ok_or("the captured actual has no complete current duty roster")?;
        select_payload_memberships_with_kind(
            &self.module.structural_types,
            &self.module.structural_domains,
            &root,
            &location.path,
            PayloadMembershipKind::Preserved,
        )
        .and_then(|contract| {
            contract.presented(
                &self.module.structural_types,
                &self.module.structural_domains,
                binding.carrier,
            )
        })
        .map(roster_only)
        .ok_or("the captured actual has no unchanged typed duty roster")
    }

    fn output_home(
        &self,
        analysis: &mut DescriptorAnalysis<'_>,
        caller: MachineId,
        position: DescriptorPosition,
        binding: &super::super::super::scalar_contracts::TerminalCallParameterBinding,
    ) -> Result<DescriptorLocation, &'static str> {
        let query = binding
            .contents
            .as_ref()
            .ok_or("the output has no typed actual contents")?;
        let location =
            if let Some(location) = analysis.exact_payload_target(caller, position, query) {
                location
            } else {
                // A call-bound formal names that borrowed storage word itself.
                // It is not an address for any reference stored inside the word.
                let location = query
                    .exact_location()
                    .ok_or("the output is not a static whole slot")?;
                let body = self.executable_body(caller)?;
                if !self.entry_is_complete(caller)
                    || !location.path.is_empty()
                    || !body.structural_parameters.iter().any(|formal| {
                        formal.place == location.root
                            && formal.structural_type == binding.carrier
                            && matches!(
                                formal.access,
                                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                            )
                    })
                {
                    return Err("the output has no unchanged whole receiving storage");
                }
                location
            };
        if !location.path.is_empty() {
            return Err("a projected output cannot close its entire owner's duty roster");
        }
        if descriptor_machine(self.graph, caller)
            .and_then(|machine| machine.place_types.get(&location.root))
            != Some(&binding.carrier)
        {
            return Err("the output's whole home differs from its receiving carrier");
        }
        Ok(location)
    }

    #[allow(clippy::too_many_arguments)]
    fn completed_provider_outputs(
        &self,
        analysis: &mut DescriptorAnalysis<'_>,
        body: &TerminalMachine,
        position: DescriptorPosition,
        before: &State,
        operation: &Operation,
        boundary: semantic_vocabulary::BoundaryMachineId,
        arguments: &[StructuralArgument],
    ) -> Result<Vec<CompletedOutput>, &'static str> {
        let declaration = self
            .module
            .boundary_machine(boundary)
            .ok_or("the output has no public declaration")?;
        if declaration.role != terminal_psi::BoundaryDeclarationRole::Callable
            || declaration.completion != terminal_psi::BoundaryCompletion::NormalReturn
            || arguments.len() != declaration.structural_parameters.len()
        {
            return Err("the completed output has no complete callable normal signature");
        }
        if declaration.reference_outputs.len() != 1
            || declaration
                .structural_parameters
                .iter()
                .filter(|formal| {
                    matches!(
                        formal.access,
                        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                    )
                })
                .count()
                != 1
        {
            return Err("the completed output has competing possible writers");
        }
        let mut demands = self
            .module
            .boundary_reference_forwarding
            .iter()
            .filter(|demand| {
                demand.machine == body.id && demand.operation == operation.id && demand.output == 0
            });
        let demand = demands
            .next()
            .ok_or("the new output lacks its conditional unchanged-value requirement")?;
        if demands.next().is_some() || demand.sources.is_empty() {
            return Err("the new output has an ambiguous or empty forwarding requirement");
        }
        let output = &declaration.reference_outputs[0];
        let BoundaryReferenceDestination::Parameter(index) = output.destination;
        if !output.path.is_empty() {
            return Err("the new output is not a whole reference word");
        }
        let formal = declaration
            .structural_parameters
            .get(index as usize)
            .ok_or("the output formal is absent")?;
        if !matches!(
            find_structural_type(&self.module.structural_types, formal.structural_type)
                .map(|row| &row.shape),
            Some(StructuralTypeShape::Reference { .. })
        ) {
            return Err("the new output formal is not a reference word");
        }
        let binding = |index: usize| -> Result<_, &'static str> {
            let formal = declaration
                .structural_parameters
                .get(index)
                .ok_or("the forwarding formal is absent")?;
            let actual = arguments
                .get(index)
                .ok_or("the forwarding actual is absent")?;
            let binding =
                super::super::super::scalar_contracts::terminal_structural_argument_binding(
                    &self.module.structural_types,
                    body,
                    formal,
                    actual,
                    index,
                )?;
            let call = descriptor_machine(self.graph, body.id)
                .and_then(|machine| machine.calls.get(&position))
                .ok_or("the completed output has no retained invocation")?;
            if call.operation != operation.id
                || call.arguments.get(index) != Some(&binding.location)
                || call.argument_contents.get(index) != Some(&binding.contents)
            {
                return Err("the completed output's actual tuple differs from its invocation");
            }
            Ok(binding)
        };
        let destination = binding(index as usize)?;
        let home = self.output_home(analysis, body.id, position, &destination)?;
        let mut lower = PayloadMembershipContract {
            carrier: formal.structural_type,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            scalar_observations: Vec::new(),
            scalar_requirements: Vec::new(),
        };
        for row in declaration
            .normal_qualifications
            .iter()
            .filter(|row| row.argument_index == index)
        {
            if row.path.is_empty() {
                lower.qualifications.push(row.domain);
            } else {
                lower
                    .projected_qualifications
                    .push(terminal_psi::StructuralPathQualification {
                        path: row.path.clone(),
                        domain: row.domain,
                    });
            }
        }
        let lower = roster_only(lower);
        for source in &demand.sources {
            let (index, path) = match source {
                BoundaryReferenceSource::Reference { argument_index } => {
                    (*argument_index as usize, None)
                }
                BoundaryReferenceSource::Stored {
                    argument_index,
                    path,
                } => (*argument_index as usize, Some(path.as_slice())),
            };
            let binding = binding(index)?;
            let actual = self.actual_roster(before, &binding)?;
            let public = &declaration.structural_parameters[index];
            let mut public = PayloadMembershipContract {
                carrier: public.structural_type,
                qualifications: public.qualifications.clone(),
                projected_qualifications: public.projected_qualifications.clone(),
                scalar_observations: Vec::new(),
                scalar_requirements: Vec::new(),
            };
            for row in declaration
                .requires
                .iter()
                .filter(|row| row.argument_index as usize == index)
            {
                if row.path.is_empty() {
                    public.qualifications.push(row.domain);
                } else {
                    public.projected_qualifications.push(
                        terminal_psi::StructuralPathQualification {
                            path: row.path.clone(),
                            domain: row.domain,
                        },
                    );
                }
            }
            let stored = path
                .map(|path| {
                    crate::descriptors::descriptor_lifetimes::descriptor_location(
                        &self.module.structural_types,
                        binding.formal,
                        binding.carrier,
                        path,
                        None,
                    )
                    .ok_or("the forwarding stored source has no typed path")
                })
                .transpose()?;
            let actual = if path.is_none()
                && binding.location.as_ref().is_some_and(|location| {
                    location.path.last() == Some(&DescriptorPathSegment::Referent)
                }) {
                // The actual already is a reference value. Keep duties on its
                // pointer word as well as those below Referent; rebuilding it
                // only from the pointee would silently erase extra duties.
                let location = binding
                    .location
                    .as_ref()
                    .expect("the reference location was checked");
                let root = before
                    .contract(location.root, None)
                    .ok_or("the captured reference value has no complete roster")?;
                select_payload_memberships_with_kind(
                    &self.module.structural_types,
                    &self.module.structural_domains,
                    &root,
                    &location.path[..location.path.len() - 1],
                    PayloadMembershipKind::Preserved,
                )
                .and_then(|contract| {
                    contract.presented(
                        &self.module.structural_types,
                        &self.module.structural_domains,
                        formal.structural_type,
                    )
                })
                .ok_or("the captured reference value has no unchanged presentation")?
            } else {
                reference_forwarding::captured_roster(
                    self.module,
                    &actual,
                    stored.as_ref().map(|path| path.path.as_slice()),
                    formal.structural_type,
                )?
            };
            let public = reference_forwarding::captured_roster(
                self.module,
                &public,
                stored.as_ref().map(|path| path.path.as_slice()),
                formal.structural_type,
            )?;
            let actual = roster_only(actual);
            if !reference_forwarding::same_roster(&actual, &roster_only(public))
                || !reference_forwarding::same_roster(&actual, &lower)
            {
                return Err(
                    "the actual source, public source and completed output have different duty rosters",
                );
            }
        }
        Ok(vec![CompletedOutput {
            root: home.root,
            contract: lower,
        }])
    }
}

fn roster_only(mut contract: PayloadMembershipContract) -> PayloadMembershipContract {
    reference_forwarding::canonicalize_roster(&mut contract);
    contract.scalar_observations.clear();
    contract.scalar_requirements.clear();
    contract
}
