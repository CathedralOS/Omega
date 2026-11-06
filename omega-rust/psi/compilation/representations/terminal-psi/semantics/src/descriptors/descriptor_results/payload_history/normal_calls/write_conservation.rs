//! A replacement must satisfy the old object's rules before that object can
//! be used again. Keep each performed store and its pre-write payload together;
//! a good replacement at one operation cannot justify another unknown write.

use super::*;

#[derive(Clone)]
pub(super) struct PerformedWritePayload {
    pub(super) machine: MachineId,
    pub(super) position: DescriptorPosition,
    pub(super) source: DescriptorLocation,
    pub(super) destination: DescriptorLocation,
    pub(super) payload: Result<PayloadMembershipContract, &'static str>,
    pub(super) promises: Result<PayloadMembershipContract, &'static str>,
}

#[derive(Clone)]
pub(super) enum PerformedWrite {
    Payload(PerformedWritePayload),
    Primitive(super::primitive_writes::PerformedPrimitiveWrite),
}

impl PerformedWrite {
    pub(super) fn destination(&self) -> &DescriptorLocation {
        match self {
            Self::Payload(write) => &write.destination,
            Self::Primitive(write) => &write.destination,
        }
    }

    pub(super) fn payload(&self) -> Option<&PerformedWritePayload> {
        match self {
            Self::Payload(write) => Some(write),
            Self::Primitive(_) => None,
        }
    }

    pub(super) fn bound(
        self,
        bindings: &BTreeMap<semantic_vocabulary::ValueId, semantic_vocabulary::ScalarTerm>,
    ) -> Result<Self, &'static str> {
        match self {
            Self::Payload(write) => Ok(Self::Payload(write)),
            Self::Primitive(write) => write.bound(bindings).map(Self::Primitive),
        }
    }
}

#[derive(Clone)]
pub(super) struct CallWriteEffect {
    pub(super) query: DescriptorContentsQuery,
    pub(super) performed: Result<PerformedWrite, &'static str>,
}

// This result is local to one call transfer. It carries no scalar snapshot or
// newly inferred duty, only the original roster discharged by every writer.
pub(in super::super) struct ConservedPayload {
    root: PlaceId,
    carrier: StructuralTypeId,
    qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
}

impl ConservedPayload {
    pub(in super::super) fn restore(self, after: &mut State) {
        let Some(current) = after.values.get_mut(&self.root) else {
            return;
        };
        if current.carrier != self.carrier {
            return;
        }
        current.qualifications = self.qualifications;
        current.projected_qualifications = self.projected_qualifications;
        after
            .unavailable_preservation
            .retain(|root| *root != self.root);
    }
}

impl<'module> NormalPayloadContext<'module> {
    pub(in super::super) fn conserved_call_payloads(
        &mut self,
        analysis: &mut DescriptorAnalysis<'_>,
        caller: MachineId,
        position: DescriptorPosition,
        before: &State,
    ) -> Result<Vec<ConservedPayload>, &'static str> {
        let effects = self.call_write_effects(analysis, caller, position)?;
        let mut writes = Vec::new();
        for effect in effects {
            // Every alternative must preserve the supplying reference's whole
            // extent and presentation. An unknown or represented member cannot
            // supply that positive geometry evidence.
            let targets = analysis
                .unchanged_payload_targets(caller, position, &effect.query)
                .ok_or("a call writer has no complete unchanged payload targets")?;
            for target in targets {
                writes.push((target, effect.performed.clone()));
            }
        }
        let mut restored = Vec::new();
        for (subject, required) in &before.values.0 {
            let PayloadSubject::Value(root) = subject else {
                // Current incoming observations are not a caller-owned old
                // roster and cannot become a normal-return restoration proof.
                continue;
            };
            if before.unavailable_preservation.contains(root)
                || (required.qualifications.is_empty()
                    && required.projected_qualifications.is_empty())
            {
                continue;
            }
            let conserved = (|| {
                let mut touched = false;
                for (location, performed) in &writes {
                    if location.root != *root {
                        continue;
                    }
                    touched = true;
                    let performed = performed.as_ref().map_err(|reason| *reason)?;
                    if let PerformedWrite::Primitive(write) = performed {
                        super::primitive_writes::preserves_roster(
                            self.module,
                            required,
                            &location.path,
                            &write.value,
                        )?;
                        continue;
                    }
                    if !location.path.is_empty() {
                        return Err("a writer replaces only part of the original payload");
                    }
                    let performed = performed
                        .payload()
                        .ok_or("the writer is not a whole structural payload replacement")?;
                    let body = descriptor_machine(self.graph, performed.machine)
                        .ok_or("the performed writer lost its machine identity")?;
                    if !body.flow.get(&performed.position).is_some_and(|step| {
                        step.derivations
                            .contains(&(performed.source.clone(), performed.destination.clone()))
                    }) {
                        return Err("the performed writer lost its operation identity");
                    }
                    let observed = performed
                        .payload
                        .as_ref()
                        .map_err(|reason| *reason)?
                        .clone()
                        .presented(
                            &self.module.structural_types,
                            &self.module.structural_domains,
                            required.carrier,
                        )
                        .ok_or("the observed replacement changes the original presentation")?;
                    let promised = performed
                        .promises
                        .as_ref()
                        .map_err(|reason| *reason)?
                        .clone()
                        .presented(
                            &self.module.structural_types,
                            &self.module.structural_domains,
                            required.carrier,
                        )
                        .ok_or("the promised replacement changes the original presentation")?;
                    // Current observations prove the old duties. Independently
                    // preserved source promises may not install additional
                    // duties that this restoration would silently discard.
                    if !contains_roster(&observed, required) {
                        return Err("a replacement does not prove every original payload duty");
                    }
                    if !contains_roster(required, &promised) {
                        return Err("a replacement installs an additional preserved payload duty");
                    }
                }
                Ok(touched)
            })();
            match conserved {
                Ok(true) => restored.push(ConservedPayload {
                    root: *root,
                    carrier: required.carrier,
                    qualifications: required.qualifications.clone(),
                    projected_qualifications: required.projected_qualifications.clone(),
                }),
                Ok(false) => {}
                Err(reason) => {
                    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                        eprintln!(
                            "CHECKSITE normal-call-payload-not-conserved machine={caller:?} position={position:?} root={root:?} reason={reason}"
                        );
                    }
                }
            }
        }
        Ok(restored)
    }

    pub(super) fn direct_call_bindings(
        &self,
        caller: MachineId,
        position: DescriptorPosition,
    ) -> Option<super::super::super::scalar_contracts::TerminalCallBindings<'module>> {
        let machine = self.executable_body(caller).ok()?;
        let operation = machine
            .blocks
            .iter()
            .find(|block| block.id == position.0)?
            .operations
            .get(position.1 as usize)?;
        let expressions =
            super::super::super::payload_predicates::TerminalScalarExpressions::new(machine);
        super::super::super::scalar_contracts::terminal_call_bindings(
            self.module,
            machine,
            operation,
            &expressions,
        )
        .ok()
    }

    pub(super) fn performed_store_before(
        &mut self,
        analysis: &mut DescriptorAnalysis<'_>,
        history: &mut PayloadMembershipHistory<'module>,
        preservation: &mut PayloadMembershipHistory<'module>,
        identity: MachineId,
        position: DescriptorPosition,
    ) -> Result<PerformedWritePayload, &'static str> {
        if history.replay.memberships != PayloadMembershipKind::Observed
            || preservation.replay.memberships != PayloadMembershipKind::Preserved
        {
            return Err("the store proof mixed observation with preserved promises");
        }
        let body = self.executable_body(identity)?;
        let operation = body
            .blocks
            .iter()
            .find(|block| block.id == position.0)
            .and_then(|block| block.operations.get(position.1 as usize))
            .ok_or("the performed writer has no operation")?;
        if !matches!(operation.kind, OperationKind::StoreStructuralField { .. }) {
            return Err("the performed writer is not a structural payload store");
        }
        let step = descriptor_machine(self.graph, identity)
            .and_then(|machine| machine.flow.get(&position))
            .ok_or("the performed writer has no graph operation")?;
        let [(source, destination)] = step.derivations.as_slice() else {
            return Err("the performed store has no single complete derivation");
        };
        let payload = self.performed_store_payload_before(
            analysis,
            history,
            identity,
            position,
            source,
            destination,
        )?;
        let promises = self.performed_store_payload_before(
            analysis,
            preservation,
            identity,
            position,
            source,
            destination,
        )?;
        Ok(PerformedWritePayload {
            machine: identity,
            position,
            source: source.clone(),
            destination: destination.clone(),
            payload,
            promises,
        })
    }

    fn performed_store_payload_before(
        &mut self,
        analysis: &mut DescriptorAnalysis<'_>,
        history: &mut PayloadMembershipHistory<'module>,
        identity: MachineId,
        position: DescriptorPosition,
        source: &DescriptorLocation,
        destination: &DescriptorLocation,
    ) -> Result<Result<PayloadMembershipContract, &'static str>, &'static str> {
        let module = self.module;
        let types = &module.structural_types;
        let domains = &module.structural_domains;
        if history.sources.identity() != identity
            || history.normal.is_some()
            || !std::ptr::eq(history.graph, self.graph)
            || !std::ptr::eq(history.types, types.as_slice())
            || !std::ptr::eq(history.domains, domains.as_slice())
            || !std::ptr::eq(analysis.compatibility_machines(), self.graph)
        {
            return Err("the store observation has a different body, history or graph");
        }
        let machine = descriptor_machine(self.graph, identity)
            .ok_or("the performed store has no body graph")?;
        let step = machine
            .flow
            .get(&position)
            .ok_or("the performed store has no operation cut")?;
        if step
            .derivations
            .iter()
            .filter(|(actual, target)| actual == source && target == destination)
            .count()
            != 1
            || machine.payload_producers.get(&position) == Some(&destination.root)
            || machine.storage_derivations.contains(&(
                position,
                source.clone(),
                destination.clone(),
            ))
        {
            return Err("the performed write has no unique payload-store derivation");
        }
        let source_carrier = *machine
            .place_types
            .get(&source.root)
            .ok_or("the performed store source has no carrier")?;
        let path = descriptor_path(types, source_carrier, &source.path)
            .ok_or("the performed store source has no complete typed path")?;

        // This is the same selected-current implementation used by
        // current_selected_payload_contract_in. Pass the existing external
        // normal-summary context instead of history.normal, so nested calls
        // share recursion and completed summaries. The cut is BEFORE this
        // store; destination annotations are never an input to this query.
        let descriptor = history.descriptor;
        let sources = &history.sources;
        let incoming = history
            .incoming
            .get_or_insert_with(|| descriptor.and_then(|body| incoming_flow(&body.flow)))
            .as_ref();
        let payload = selected_payload_contract_reusing(
            types,
            domains,
            identity,
            descriptor,
            incoming,
            analysis,
            position,
            source.root,
            &path,
            &|place| sources.contract(place),
            history.entry_snapshots.as_deref().unwrap_or(&[]),
            &mut history.replay,
            Some(self),
        );
        // The summary consumes nominal truth only. Callee scalar IDs and
        // their proof scratch must not escape into another activation.
        Ok(payload.map(|contract| PayloadMembershipContract {
            carrier: contract.carrier,
            qualifications: contract.qualifications,
            projected_qualifications: contract.projected_qualifications,
            scalar_observations: Vec::new(),
            scalar_requirements: Vec::new(),
        }))
    }
}

fn contains_roster(
    available: &PayloadMembershipContract,
    required: &PayloadMembershipContract,
) -> bool {
    required
        .qualifications
        .iter()
        .all(|domain| available.qualifications.contains(domain))
        && required
            .projected_qualifications
            .iter()
            .all(|row| available.projected_qualifications.contains(row))
}
