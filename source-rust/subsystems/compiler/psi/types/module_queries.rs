use crate::*;
use semantic_vocabulary::{MachineId, Proposition, ServiceId};

/// Physical members hidden while checking guest operations. An opaque value
/// keeps its own identity and ownership throughout the check.
#[must_use = "restore physical storage before publishing the checked module"]
pub struct OpaqueStorageShapes(Vec<OpaqueStorageShape>);

struct OpaqueStorageShape {
    index: usize,
    id: semantic_vocabulary::StructuralTypeId,
    shape: crate::StructuralTypeShape,
    restriction: Option<crate::StructuralFieldType>,
}

impl OpaqueStorageShapes {
    pub fn restore(self, module: &mut TerminalModule) -> Result<(), &'static str> {
        if self.0.iter().any(|saved| {
            module.structural_types.get(saved.index).is_none_or(|row| {
                row.id != saved.id
                    || row.shape != crate::StructuralTypeShape::Opaque
                    || row.scalar_restriction.is_some()
            })
        }) {
            return Err("opaque declaration changed while checking guest operations");
        }
        for saved in self.0 {
            let row = &mut module.structural_types[saved.index];
            row.shape = saved.shape;
            row.scalar_restriction = saved.restriction;
        }
        Ok(())
    }
}

impl TerminalModule {
    /// The wire retains physical storage on both members of a selected pair.
    /// The guest may transport the opaque member, but only the carrier's own
    /// type grants access to its fields. Move the physical shapes out while
    /// checking guest code so every constructor, projection and proof sees
    /// the nominal declaration, including through nested aggregates.
    pub fn take_opaque_storage_shapes(&mut self) -> Result<OpaqueStorageShapes, &'static str> {
        let mut shapes = Vec::with_capacity(self.opaque_representations.len());
        for relation in &self.opaque_representations {
            let index = self
                .structural_types
                .binary_search_by_key(&relation.opaque, |row| row.id)
                .map_err(|_| "opaque representation lost its storage declaration")?;
            let row = &self.structural_types[index];
            if shapes
                .last()
                .is_some_and(|previous: &OpaqueStorageShape| previous.index >= index)
            {
                return Err("opaque representations repeat or reorder a storage declaration");
            }
            if !row.requires.is_empty() || row.quotient_carrier.is_some() {
                return Err(
                    "opaque data cannot supply a visible carrier or authored value predicate",
                );
            }
            shapes.push(OpaqueStorageShape {
                index,
                id: row.id,
                shape: crate::StructuralTypeShape::Opaque,
                restriction: None,
            });
        }
        for saved in &mut shapes {
            let row = &mut self.structural_types[saved.index];
            std::mem::swap(&mut row.shape, &mut saved.shape);
            saved.restriction = row.scalar_restriction.take();
        }
        Ok(OpaqueStorageShapes(shapes))
    }

    pub fn callback_application(&self, index: u32) -> Option<&crate::TerminalCallbackApplication> {
        self.callback_applications
            .binary_search_by_key(&index, |row| row.application_index)
            .ok()
            .map(|ordinal| &self.callback_applications[ordinal])
            .or_else(|| {
                self.installed_interrupt_registrations
                    .iter()
                    .find_map(|registration| {
                        registration
                            .callbacks
                            .binary_search_by_key(&index, |row| row.application_index)
                            .ok()
                            .map(|ordinal| &registration.callbacks[ordinal])
                    })
            })
    }
    /// Resolve the declaration instance attached to this boundary, rather than
    /// looking up its template by name. Closed calls can share a template while
    /// retaining different ordered static arguments.
    pub fn boundary_calling_abi(
        &self,
        boundary: semantic_vocabulary::BoundaryMachineId,
    ) -> Result<&crate::CallingAbiDeclaration, String> {
        // Source publication sorts this roster; the receiving foundation check
        // requires increasing boundary IDs. Reuse that order for each demand.
        let missing = "boundary has no unique Terminal calling declaration";
        let position = self
            .calling_signatures
            .binary_search_by_key(&boundary, |row| row.boundary)
            .map_err(|_| missing.to_owned())?;
        if (position != 0 && self.calling_signatures[position - 1].boundary == boundary)
            || self
                .calling_signatures
                .get(position + 1)
                .is_some_and(|row| row.boundary == boundary)
        {
            return Err(missing.into());
        }
        let declaration = &self.calling_signatures[position];
        self.calling_abi_signature(*declaration.application.as_ref().map_err(Clone::clone)?)
            .ok_or_else(|| "boundary lost its exact declaration ABI".into())
    }

    pub fn calling_abi_identity(
        &self,
        boundary: semantic_vocabulary::BoundaryMachineId,
    ) -> Result<semantic_vocabulary::CallingAbiIdentity, String> {
        self.boundary_calling_abi(boundary)?
            .identity(&self.identities)
    }
    pub fn blocks(&self) -> impl Iterator<Item = &Block> + '_ {
        self.machines.iter().flat_map(|machine| &machine.blocks)
    }

    /// Whether any block can end the whole process (`ProcessExit`) instead of
    /// returning to its caller. A native host sees such an end only as an exit
    /// status, which a normal return can also produce.
    pub fn can_end_the_process(&self) -> bool {
        self.blocks().any(|block| {
            matches!(
                block.terminator,
                crate::Terminator::ExternalCompletion { .. }
            )
        })
    }

    pub fn operations(&self) -> impl Iterator<Item = &Operation> + '_ {
        self.machines
            .iter()
            .flat_map(|machine| machine.operations())
    }

    pub fn machine(&self, id: MachineId) -> Option<&TerminalMachine> {
        self.machines.iter().find(|machine| machine.id == id)
    }

    pub fn declaration_contract(
        &self,
        id: semantic_vocabulary::ContractId,
    ) -> Option<&crate::MachineContract> {
        self.declaration_contracts
            .binary_search_by_key(&id, |contract| contract.id)
            .ok()
            .map(|position| &self.declaration_contracts[position])
    }

    /// Read the public contract of this call without changing the body being
    /// executed. The selected association is checked with the module's other
    /// conformance records; finding a header here grants no permission.
    pub fn invocation_machine_contract(
        &self,
        callee: MachineId,
        required: Option<semantic_vocabulary::ContractId>,
    ) -> Option<&crate::MachineContract> {
        let body = self.machine(callee)?;
        let Some(required) = required else {
            return Some(&body.contract);
        };
        let contract = self.declaration_contract(required)?;
        let selected = contract.selected_application.as_ref()?;
        (selected.realization == callee).then_some(contract)
    }

    pub fn invocation_requires(
        &self,
        callee: crate::TargetMachineCallable,
        required: Option<semantic_vocabulary::ContractId>,
    ) -> Option<&[Proposition]> {
        match callee {
            crate::TargetMachineCallable::Machine(callee) => {
                Some(&self.invocation_machine_contract(callee, required)?.requires)
            }
            crate::TargetMachineCallable::Boundary(_) if required.is_some() => None,
            crate::TargetMachineCallable::Boundary(_) => self.callable_requires(callee),
        }
    }

    /// Ground layout prerequisites belong to the actual retained call and
    /// can be reconstructed from its declared contract during independent readback.
    pub fn layout_call_conditions(
        &self,
        machine: &TerminalMachine,
    ) -> Result<Vec<(semantic_vocabulary::ObligationId, ScalarPredicateCondition)>, &'static str>
    {
        if self.scalar_applications.declarations().is_empty() {
            return Ok(Vec::new());
        }
        let owner = self
            .scalar_applications
            .proof_scope
            .callable_declaration(crate::TargetMachineCallable::Machine(machine.id));
        let calls = machine
            .module_invocations()
            .map(|call| {
                let inputs = call.inputs.declared();
                (
                    inputs.callee,
                    inputs.required_contract,
                    inputs.requirement_obligations,
                )
            })
            .chain(machine.boundary_invocations().map(|call| {
                (
                    crate::TargetMachineCallable::Boundary(call.boundary),
                    None,
                    call.requirement_obligations,
                )
            }));
        let mut conditions = Vec::new();
        for (callee, required, obligations) in calls {
            let Some(clauses) = self.invocation_requires(callee, required) else {
                continue;
            };
            for (clause, obligation) in clauses.iter().zip(obligations) {
                let condition = self.scalar_applications.layout_predicate(
                    owner,
                    || {
                        format!(
                            "layout-dependent call requirement {} of {callee:?}",
                            obligation.get()
                        )
                    },
                    clause,
                )?;
                if let Some(condition) = condition {
                    conditions.push((*obligation, condition));
                }
            }
        }
        conditions.sort_unstable_by_key(|(obligation, _)| *obligation);
        if conditions.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err("layout call requirements reuse an obligation identity");
        }
        Ok(conditions)
    }

    pub fn invocation_ensures(
        &self,
        callee: crate::TargetMachineCallable,
        required: Option<semantic_vocabulary::ContractId>,
    ) -> Option<impl Iterator<Item = &Proposition>> {
        let (contract, boundary) = match callee {
            crate::TargetMachineCallable::Machine(callee) => (
                Some(self.invocation_machine_contract(callee, required)?),
                None,
            ),
            crate::TargetMachineCallable::Boundary(_) if required.is_some() => return None,
            crate::TargetMachineCallable::Boundary(boundary) => {
                (None, Some(self.boundary_machine(boundary)?))
            }
        };
        Some(
            contract
                .into_iter()
                .flat_map(|contract| contract.ensures.iter().map(|clause| &clause.proposition))
                .chain(
                    boundary
                        .into_iter()
                        .flat_map(|boundary| &boundary.scalar_ensures),
                ),
        )
    }

    pub fn invocation_structural_parameters(
        &self,
        callee: crate::TargetMachineCallable,
        required: Option<semantic_vocabulary::ContractId>,
    ) -> Option<&[StructuralParameterDeclaration]> {
        match callee {
            crate::TargetMachineCallable::Machine(callee) => Some(
                &self
                    .invocation_machine_contract(callee, required)?
                    .structural_parameters,
            ),
            crate::TargetMachineCallable::Boundary(_) if required.is_some() => None,
            crate::TargetMachineCallable::Boundary(_) => {
                self.callable_structural_parameters(callee)
            }
        }
    }

    /// Read the written contract of the exact declaration a call names. A
    /// boundary contributes its interface contract, never a fabricated body
    /// or a body-derived result summary. Provider admission remains separate.
    pub fn callable_requires(
        &self,
        callable: crate::TargetMachineCallable,
    ) -> Option<&[Proposition]> {
        match callable {
            crate::TargetMachineCallable::Machine(machine) => {
                Some(&self.machine(machine)?.contract.requires)
            }
            crate::TargetMachineCallable::Boundary(boundary) => {
                Some(&self.boundary_machine(boundary)?.scalar_requires)
            }
        }
    }

    pub fn callable_ensures(
        &self,
        callable: crate::TargetMachineCallable,
    ) -> Option<impl Iterator<Item = &Proposition>> {
        let (machine, boundary) = match callable {
            crate::TargetMachineCallable::Machine(machine) => (Some(self.machine(machine)?), None),
            crate::TargetMachineCallable::Boundary(boundary) => {
                (None, Some(self.boundary_machine(boundary)?))
            }
        };
        Some(
            machine
                .into_iter()
                .flat_map(|machine| {
                    machine
                        .contract
                        .ensures
                        .iter()
                        .map(|clause| &clause.proposition)
                })
                .chain(
                    boundary
                        .into_iter()
                        .flat_map(|boundary| &boundary.scalar_ensures),
                ),
        )
    }

    pub fn callable_structural_parameters(
        &self,
        callable: crate::TargetMachineCallable,
    ) -> Option<&[StructuralParameterDeclaration]> {
        match callable {
            crate::TargetMachineCallable::Machine(machine) => {
                Some(&self.machine(machine)?.structural_parameters)
            }
            crate::TargetMachineCallable::Boundary(boundary) => {
                Some(&self.boundary_machine(boundary)?.structural_parameters)
            }
        }
    }

    pub fn calling_abi_signature(
        &self,
        id: semantic_vocabulary::CallingSignatureId,
    ) -> Option<&crate::CallingAbiDeclaration> {
        self.calling_abi_signatures
            .binary_search_by_key(&id, |row| row.id)
            .ok()
            .map(|position| &self.calling_abi_signatures[position])
    }

    pub fn unoptimized_machine_behavior(
        &self,
        id: MachineId,
    ) -> Option<&crate::UnoptimizedMachineBehavior> {
        self.unoptimized_behavior
            .iter()
            .find(|body| body.machine == id)
    }

    pub fn service(&self, id: ServiceId) -> Option<&ServiceDeclaration> {
        self.services.iter().find(|service| service.id == id)
    }

    pub fn structural_type(
        &self,
        id: semantic_vocabulary::StructuralTypeId,
    ) -> Option<&StructuralTypeDeclaration> {
        find_structural_type(&self.structural_types, id)
    }

    pub fn foreign_reference_descriptor(
        &self,
        referent: semantic_vocabulary::StructuralTypeId,
    ) -> Option<bool> {
        if let Some(declaration) = self.structural_type(referent) {
            return Some(matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(_) | StructuralTypeShape::ElementView { .. }
            ));
        }
        self.structural_referents
            .iter()
            .find(|row| row.id == referent)
            .map(|row| row.foreign_descriptor)
    }

    pub fn structural_domain(
        &self,
        id: semantic_vocabulary::StructuralDomainId,
    ) -> Option<&StructuralDomainDeclaration> {
        self.structural_domains
            .iter()
            .find(|declaration| declaration.id == id)
    }

    pub fn boundary_machine(
        &self,
        id: semantic_vocabulary::BoundaryMachineId,
    ) -> Option<&BoundaryMachineDeclaration> {
        self.boundary_machines
            .iter()
            .find(|declaration| declaration.id == id)
    }

    /// Exact requirements whose selected service rows remain to be supplied.
    /// Keep the declaration's bound distinct from an ordinary pinned row.
    pub fn installation_reach_requirements(&self) -> Vec<semantic_vocabulary::BoundaryMachineId> {
        self.boundary_machines
            .iter()
            .filter(|boundary| boundary.service_reach_is_installation_bound)
            .map(|boundary| boundary.id)
            .collect()
    }

    pub fn for_entry(entry: MachineId) -> Self {
        Self {
            entry: Some(TargetMachineCallable::Machine(entry)),
            ..Self::declarations()
        }
    }

    pub fn declarations() -> Self {
        Self {
            scalar_applications: Default::default(),
            folded_constants: Default::default(),
            indexed_theorems: Vec::new(),
            mathematical_carriers: Vec::new(),
            identity_formations: TerminalIdentityFormations::default(),
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: None,
            identities: Default::default(),
            scalar_qualifications: Default::default(),
            structural_types: Vec::new(),
            structural_referents: Vec::new(),
            structural_domains: Vec::new(),
            structural_qualification_changes: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            access_view_inputs: Vec::new(),
            access_view_plans: Vec::new(),
            access_lifecycle_plans: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: Vec::new(),
            calling_signatures: Vec::new(),
            calling_abi_signatures: Vec::new(),
            calling_policy_requests: Vec::new(),
            service_schemas: Vec::new(),
            program_entry_contracts: Vec::new(),
            provider_selections: Vec::new(),
            opaque_representations: Vec::new(),
            unoptimized_behavior: Vec::new(),
            external_providers: Vec::new(),
            provider_candidates: Vec::new(),
            provider_instance_requirements: Vec::new(),
            boundary_application_candidates: Vec::new(),
            callback_applications: Vec::new(),
            installed_interrupt_registrations: Vec::new(),
            dynamic_dispatch: TerminalDynamicDispatchCatalog::default(),
            callables: crate::TerminalCallableCatalog::default(),
            quotients: Default::default(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            scalar_predicate_definitions: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            scalar_block_invariants: Vec::new(),
            operation_crash_contracts: Vec::new(),
            closed_conformance_applications: Vec::new(),
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            declaration_contracts: Vec::new(),
            machines: Vec::new(),
            target_layout_applications: Vec::new(),
            native_layouts: Default::default(),
            boundary_application_demands: Vec::new(),
            task_activation_demands: TerminalTaskActivationDemands::default(),
        }
    }
}
