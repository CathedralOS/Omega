/*
We define the sealed `ObjectArtifact` carrier, its function, attribution,
foreign-call, and byte-literal pool records, stack-demand derivation
(`derive_stack_demand`), installed interrupt stack composition
(`installed_stack`), normalized foreign-call provenance verification, and
boundary/structural replay checks.
*/

mod carriers {
    use machine_code::{BoundarySettlementRecord, PortEffectRecord, SemanticCodeAttribution};
    use object_file::{ObjectByteLiteralPool, ObjectForeignCall};
    use object_file::{ObjectPlan, ObjectSymbolHandle, RelocationPlan};
    use semantic_vocabulary::MachineId;
    use target::NativeTarget;
    use target_operations::{CallSiteOwner, TerminalPsiProvenance};
    use terminal_psi::TerminalPsiIdentity;
    use verified_optimization_input::hosted_receiver::AdmittedHostedReceiver;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ObjectArtifact {
        pub(crate) emission_input: std::sync::Arc<crate::object_input::ObjectEmissionInput>,
        pub(crate) image_data: crate::function_fragments::image_data::ObjectImageData,
        pub(crate) foreign_call_plans:
            std::sync::Arc<machine_code::foreign_call_plans::PhysicalForeignCallPlans>,
        pub(crate) imports: std::sync::Arc<machine_code::import_code::EncodedImportProgram>,
        pub(crate) windows_internet:
            Option<std::sync::Arc<machine_code::WindowsInternetProviderCode>>,
        pub(crate) projected: crate::object::function_fragments::ProjectedObject,
        pub(crate) hosted_entry: Option<crate::hosted_program::EncodedHostedProgramEntry>,
        pub(crate) physical_entry: Option<crate::PhysicalEntryBinding>,
        pub(crate) retained_entry_storage: ObjectSymbolHandle,
        pub(crate) requires_graph_storage_replay: bool,
        pub(crate) fragment_replay:
            Option<crate::object::function_fragments::replay::FragmentReplay>,
        pub(crate) composition_source: Option<std::sync::Arc<ObjectArtifact>>,
        pub(crate) psi: TerminalPsiIdentity,
        pub(crate) target: NativeTarget,
        pub(crate) profile: Option<target::TargetProfile>,
        pub(crate) entry: MachineId,
        pub(crate) object: ObjectPlan,
        pub(crate) relocations: RelocationPlan,
        pub(crate) text_bytes: Vec<u8>,
        pub(crate) data_bytes: Vec<u8>,
        pub(crate) functions: Vec<ObjectFunction>,
        pub(crate) private_functions: Vec<ObjectCompilerPrivateFunction>,
        pub(crate) semantic_code_attribution: Vec<ObjectCodeAttribution>,
        pub(crate) port_effects: Vec<ObjectPortEffect>,
        pub(crate) boundary_settlements: Vec<ObjectBoundarySettlement>,
        pub(crate) foreign_calls: Vec<ObjectForeignCall>,
        pub(crate) byte_literal_pool: Option<ObjectByteLiteralPool>,
        pub(crate) native_code_layout_pool:
            Option<crate::object::function_fragments::native_code_layouts::NativeCodeLayoutPool>,
        pub(crate) code_descriptor_pool:
            Option<crate::object::function_fragments::code_descriptors::ObjectCodeDescriptorPool>,
    }

    impl ObjectArtifact {
        pub fn publication(&self) -> &object_file::ObjectPublication {
            self.emission_input.publication()
        }

        pub fn emission_facts(
            &self,
        ) -> &verified_optimization_input::publication::SemanticEmissionFacts {
            &self.emission_input.emission_facts().semantic
        }

        pub(crate) fn physical_facts(&self) -> &physical_program::PhysicalEmissionFacts {
            &self.emission_input.emission_facts().physical
        }

        pub fn foreign_call_plans(
            &self,
        ) -> &std::sync::Arc<machine_code::foreign_call_plans::PhysicalForeignCallPlans> {
            &self.foreign_call_plans
        }
        pub fn call_origins(
            &self,
        ) -> &std::sync::Arc<machine_code::call_origins::PhysicalCallOrigins> {
            &self.physical_facts().calls
        }

        pub(crate) fn has_free_unit_entry(&self) -> Result<bool, diagnostics::Diagnostic> {
            let (function, _) = self.entry_facts()?;
            Ok(matches!(
                function.result,
                abstract_operations::AbstractFunctionResult::Unit
            ) && function.entry_claims.is_empty()
                && function.parameters.is_empty()
                && function.structural_parameters.is_empty())
        }

        pub fn entry_call_plan(&self) -> Option<&calling_conventions::CallPlan> {
            self.physical_facts()
                .contracts
                .function_abi(self.entry)
                .map(|abi| &abi.call_plan)
        }

        pub(crate) fn entry_facts(
            &self,
        ) -> Result<
            (
                &verified_optimization_input::publication::EmissionFunctionFacts,
                &legalized_operations::LegalizedFunctionAbi,
            ),
            diagnostics::Diagnostic,
        > {
            let absent = || {
                diagnostics::Diagnostic::error("entry has no current function or calling contract")
            };
            let function = self
                .emission_facts()
                .program
                .functions
                .iter()
                .find(|row| row.machine == self.entry)
                .ok_or_else(absent)?;
            let abi = self
                .physical_facts()
                .contracts
                .function_abi(self.entry)
                .ok_or_else(absent)?;
            Ok((function, abi))
        }

        /// Audit inputs for the root body before any private functions were composed.
        pub fn retained_body_replay(&self) -> Option<&crate::FragmentReplay> {
            self.composition_source
                .as_deref()
                .unwrap_or(self)
                .fragment_replay
                .as_ref()
        }

        /// Borrows the retained producer for the root object, including after
        /// private-body composition. Private bodies retain their own producers.
        pub fn retained_fragment_source(
            &self,
        ) -> Option<&object_file::StagedOptimizedRelocationFreeObjectContainer> {
            self.retained_body_replay()
                .map(crate::FragmentReplay::fragment_source)
        }

        pub fn native_instruction_source_rows(&self) -> Option<NativeInstructionSourceRows<'_>> {
            let physical = &self.retained_body_replay()?.original_encoding()?.physical;
            Some(NativeInstructionSourceRows {
                selected: physical,
                physical: physical
                    .machine
                    .functions
                    .iter()
                    .flat_map(|function| {
                        function
                            .instructions()
                            .map(move |instruction| (function.machine, instruction.instruction))
                    })
                    .collect(),
            })
        }

        pub(crate) fn installed_entry_execution(
            &self,
        ) -> Option<&provider_admission::AdmittedInstalledEntryExecution> {
            self.emission_input
                .installation_authority()
                .installed_entry_execution()
        }

        pub fn installation_environment_assumptions(
            &self,
        ) -> Vec<&installation_conditions::InstallationEnvironmentAssumption> {
            fn collect<'a>(
                object: &'a ObjectArtifact,
                rows: &mut Vec<&'a installation_conditions::InstallationEnvironmentAssumption>,
            ) {
                let source = object.composition_source.as_deref().unwrap_or(object);
                for scope in source
                    .emission_input
                    .installation_authority()
                    .installed_interrupts()
                    .scopes()
                {
                    for assumption in scope.installation_assumptions() {
                        if !rows.contains(&assumption) {
                            rows.push(assumption);
                        }
                    }
                }
                for private in &object.private_functions {
                    collect(&private.body, rows);
                }
            }
            let mut rows = Vec::new();
            collect(self, &mut rows);
            rows.sort_unstable_by_key(|row| (row.entry(), row.installation(), row.condition_tag()));
            rows
        }

        pub const fn physical_entry_binding(&self) -> Option<&crate::PhysicalEntryBinding> {
            self.physical_entry.as_ref()
        }

        pub const fn byte_literal_pool(&self) -> Option<&ObjectByteLiteralPool> {
            self.byte_literal_pool.as_ref()
        }

        pub const fn hosted_receiver_binding(&self) -> Option<&AdmittedHostedReceiver> {
            match &self.hosted_entry {
                Some(entry) => entry.context().receiver(),
                None => None,
            }
        }
        pub const fn psi(&self) -> TerminalPsiIdentity {
            self.psi
        }

        pub const fn target(&self) -> NativeTarget {
            self.target
        }

        /// Returns the retained deployment, never inferred from machine geometry.
        pub const fn profile(&self) -> Option<target::TargetProfile> {
            self.profile
        }

        pub const fn entry(&self) -> MachineId {
            self.entry
        }

        /// Rejoins the functions selected for native execution with this object.
        /// Objects without retained admission return no scope; their consumers
        /// must validate the full module or refuse, rather than assume no work.
        pub fn native_execution_scope(
            &self,
            identity: terminal_psi::TerminalPsiIdentity,
            selected_provider_closure: [u8; 32],
        ) -> Result<Option<&native_execution_reach::ExecutionReach>, &'static str> {
            let facts = self.emission_facts().admission.entry_facts();
            facts.selected_terminal_entry(identity, self.entry, self.target)?;
            facts.native_execution_scope_for(
                identity,
                self.target,
                self.entry,
                selected_provider_closure,
            )
        }

        pub fn validate_terminal_entry(
            &self,
            identity: terminal_psi::TerminalPsiIdentity,
        ) -> Result<(), &'static str> {
            let entry = self.emission_facts().admission.entry_facts();
            if self.psi != identity || !entry.has_entry_body(self.entry) {
                return Err("native object entry has no exact Terminal body or identity");
            }
            let installed_entry =
                entry.selected_terminal_entry(identity, self.entry, self.target)?;
            let selected_entry = if let Some(entry) = installed_entry {
                entry
            } else {
                match entry.original_entry() {
                    Some(terminal_psi::TargetMachineCallable::Machine(machine)) => machine,
                    Some(terminal_psi::TargetMachineCallable::Boundary(_)) | None => {
                        return Err("native entry requires retained deployment selection");
                    }
                }
            };
            if selected_entry != self.entry {
                return Err("native object entry substituted the exact installed Terminal member");
            }
            Ok(())
        }

        pub const fn object(&self) -> &ObjectPlan {
            &self.object
        }

        pub const fn relocations(&self) -> &RelocationPlan {
            &self.relocations
        }

        pub fn text_bytes(&self) -> &[u8] {
            &self.text_bytes
        }

        pub fn data_bytes(&self) -> &[u8] {
            &self.data_bytes
        }

        pub fn functions(&self) -> &[ObjectFunction] {
            &self.functions
        }

        pub fn function_on_machine(&self, machine: MachineId) -> Option<&ObjectFunction> {
            self.functions
                .iter()
                .find(|function| function.machine == machine)
        }

        pub fn borrowed_storage_layouts(&self) -> &runtime_abi::NativeBorrowedStorageLayouts {
            &self.physical_facts().contracts.borrowed_storage_layouts
        }

        pub fn private_functions(&self) -> &[ObjectCompilerPrivateFunction] {
            &self.private_functions
        }

        pub(crate) fn entry_function(&self) -> &ObjectFunction {
            self.function_on_machine(self.entry)
                .expect("artifact construction requires one entry function")
        }

        pub fn boundary_settlements(&self) -> &[ObjectBoundarySettlement] {
            &self.boundary_settlements
        }

        pub fn foreign_calls(&self) -> &[ObjectForeignCall] {
            &self.foreign_calls
        }

        pub fn port_effects(&self) -> &[ObjectPortEffect] {
            &self.port_effects
        }

        pub fn semantic_code_attribution(&self) -> &[ObjectCodeAttribution] {
            &self.semantic_code_attribution
        }
    }

    pub struct NativeInstructionSourceRows<'a> {
        pub selected: &'a post_allocation_machine::PostAllocationMachineProgram,
        pub physical: Vec<(MachineId, selected_instructions::SelectedInstructionId)>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ObjectFunction {
        pub location: object_file::ObjectFunctionLocation,
        pub attachment: Option<semantic_vocabulary::StructuralTypeId>,
        pub scalar_abi: Option<target_operations::ScalarFunctionAbi>,
        pub mixed_structural_scalar_abi:
            Option<target_operations::MixedStructuralScalarFunctionAbi>,
        pub parameter_abi: Option<machine_code::ParameterFunctionAbiRecord>,
        pub provenance: TerminalPsiProvenance,
        pub unit_stack: Option<ObjectUnitStack>,
        pub scalar_stack: Option<ObjectScalarStack>,
        pub unit_call_stacks: Vec<ObjectUnitCallStack>,
        pub scalar_call_stacks: Vec<ObjectScalarCallStack>,
        pub internal_unit_calls: Vec<machine_code::InternalUnitCallRecord>,
        pub unit_parameters: Vec<machine_code::UnitParameterRecord>,
        pub unit_parameter_homes: Vec<machine_code::UnitParameterHomeRecord>,
        pub scalar_structural_parameters: Vec<machine_code::UnitParameterRecord>,
        pub scalar_structural_parameter_homes: Vec<machine_code::UnitParameterHomeRecord>,
    }

    impl std::ops::Deref for ObjectFunction {
        type Target = object_file::ObjectFunctionLocation;

        fn deref(&self) -> &Self::Target {
            &self.location
        }
    }
    impl AsRef<object_file::ObjectFunctionLocation> for ObjectFunction {
        fn as_ref(&self) -> &object_file::ObjectFunctionLocation {
            &self.location
        }
    }
    impl ObjectFunction {
        pub fn call_arguments(
            &self,
        ) -> impl Iterator<Item = &machine_code::InternalUnitCallArgumentRecord> + '_ {
            machine_code::unit_call_arguments(&self.internal_unit_calls)
        }
    }

    impl ObjectFunction {
        pub fn scalar_structural_parameter(
            &self,
            place: semantic_vocabulary::PlaceId,
        ) -> Option<&machine_code::UnitParameterRecord> {
            self.scalar_structural_parameters
                .iter()
                .find(|parameter| parameter.place == place)
        }

        pub fn bytes<'artifact>(&self, artifact: &'artifact ObjectArtifact) -> &'artifact [u8] {
            &artifact.text_bytes[self.text_offset..self.text_offset + self.byte_count]
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ObjectCompilerPrivateFunction {
        pub identity: function_identity::MachineFunctionIdentity,
        pub source_psi: TerminalPsiIdentity,
        pub function: ObjectPrivateFunctionEntry,
        pub body: std::sync::Arc<ObjectArtifact>,
        pub entry_bridge: Option<crate::NativeEntryBridge>,
        pub entry_effects: calling_conventions::StateEffectsEvidence,
        pub inbound: calling_conventions::BoundaryEntryPlan,
        pub inbound_ordinals: Vec<u32>,
        pub text_offset: usize,
        pub data_offset: usize,
        pub symbol_bindings: arena::Arena<PrivateObjectSymbolBinding>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ObjectPrivateFunctionEntry {
        pub machine: MachineId,
        pub scalar_abi: Option<target_operations::ScalarFunctionAbi>,
        pub symbol: ObjectSymbolHandle,
        pub text_offset: usize,
        pub byte_count: usize,
    }

    impl ObjectPrivateFunctionEntry {
        pub(crate) fn from_function(
            function: &ObjectFunction,
            symbol: ObjectSymbolHandle,
            text_offset: usize,
        ) -> Self {
            Self {
                machine: function.machine,
                scalar_abi: function.scalar_abi.clone(),
                symbol,
                text_offset,
                byte_count: function.byte_count,
            }
        }

        pub fn bytes<'artifact>(&self, artifact: &'artifact ObjectArtifact) -> &'artifact [u8] {
            &artifact.text_bytes[self.text_offset..self.text_offset + self.byte_count]
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct PrivateObjectSymbolBinding {
        pub local: ObjectSymbolHandle,
        pub linked: ObjectSymbolHandle,
    }

    pub struct PrivateFunctionObject {
        pub identity: function_identity::MachineFunctionIdentity,
        pub body: std::sync::Arc<ObjectArtifact>,
        pub entry_bridge: Option<crate::NativeEntryBridge>,
        pub entry_effects: calling_conventions::StateEffectsEvidence,
        pub inbound: calling_conventions::BoundaryEntryPlan,
        pub inbound_ordinals: Vec<u32>,
    }

    impl ObjectCompilerPrivateFunction {
        pub fn bytes<'artifact>(&self, artifact: &'artifact ObjectArtifact) -> &'artifact [u8] {
            self.function.bytes(artifact)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ObjectUnitCallStack {
        pub owner: CallSiteOwner,
        pub target: MachineId,
        pub text_offset: usize,
        pub active_frame_bytes: u32,
        pub transient_bytes: u32,
        pub caller_live_bytes: u32,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ObjectScalarCallStack {
        pub owner: CallSiteOwner,
        pub target: MachineId,
        pub text_offset: usize,
        pub caller_live_bytes: u32,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ObjectUnitStack {
        pub frame_bytes: u32,
        pub local_peak_bytes: u32,
        pub stack_alignment: u32,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ObjectScalarStack {
        pub local_peak_bytes: u32,
        pub stack_alignment: u32,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ObjectBoundarySettlement {
        pub machine: MachineId,
        pub settlement: BoundarySettlementRecord,
        pub text_offset: usize,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ObjectPortEffect {
        pub machine: MachineId,
        pub effect: PortEffectRecord,
        pub text_offset: usize,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ObjectCodeAttribution {
        pub machine: MachineId,
        pub attribution: SemanticCodeAttribution,
        pub text_offset: usize,
    }
}

pub(crate) mod installed_stack {
    use crate::{FunctionFragmentObjectArtifactError as Error, PrivateFunctionObject};
    use installation_conditions::InstalledInterruptStackRequirement;
    mod faults {
        use crate::{FunctionFragmentObjectArtifactError as Error, ObjectArtifact};
        use installation_conditions::InstalledBodyFaultPremises;
        use selected_instructions::MachineEncodedTrapBehavior as Trap;

        pub(super) fn derive(
            body: &ObjectArtifact,
        ) -> Result<Option<InstalledBodyFaultPremises>, Error> {
            let execution = body.installed_entry_execution().ok_or(Error::Mismatch(
                "installed body stack demand has no exact execution domain",
            ))?;
            if execution.scope().fatal_entries().is_empty() {
                return Ok(None);
            }
            if execution.scope().interrupt_delivery_assumption().is_none() {
                return Err(Error::Mismatch(
                    "installed fatal closure lacks its delivery condition",
                ));
            }
            if !body.foreign_calls.is_empty() || !body.private_functions.is_empty() {
                return Err(Error::Mismatch(
                    "installed quiet closure has an unresolved foreign or private execution dependency",
                ));
            }
            let current = body.emission_input.readback();
            let origins = body
                .emission_input
                .emission_facts()
                .physical
                .faults
                .as_ref()
                .map_err(|detail| Error::PhysicalSourceObservation(detail.clone()))?;
            if origins.psi != body.psi()
                || origins.entry != body.entry()
                || origins.target != body.target()
            {
                return Err(Error::Mismatch(
                    "installed fault analysis changed its body or target",
                ));
            }
            let mut machines: Vec<_> = origins
                .functions
                .iter()
                .map(|function| function.machine)
                .collect();
            machines.sort_unstable();
            if machines.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(Error::Mismatch(
                    "installed fault analysis changed its exact function roster",
                ));
            }
            installation_admission::ram::validate(
                &body.emission_facts().program,
                body.call_origins(),
                body.emission_input
                    .installation_authority()
                    .installed_interrupts(),
                execution,
                &machines,
            )?;
            crate::observe_native_body_cpu_state(
                body,
                execution.entry().entry().inbound().state.preemption,
            )?
            .require_without_floating_state()?;
            let mut encoded = current.instruction_effects.iter();
            for function in &origins.functions {
                let mut placements = body
                    .functions()
                    .iter()
                    .filter(|placed| placed.machine == function.machine);
                let placed = placements.next().ok_or(Error::Mismatch(
                    "installed fault analysis has no exact placed function",
                ))?;
                if placements.next().is_some() {
                    return Err(Error::Mismatch(
                        "installed function has multiple object placements",
                    ));
                }
                let function_bytes = placed
                    .text_offset
                    .checked_add(placed.byte_count)
                    .and_then(|end| body.text_bytes().get(placed.text_offset..end))
                    .ok_or(Error::Mismatch(
                        "installed function bytes exceed the object text",
                    ))?;
                let relative_branches = body
                    .emission_input
                    .publication()
                    .body()
                    .check_installed_branches(function.machine, function_bytes)
                    .map_err(|source| Error::InstalledRelativeBranches {
                        machine: function.machine,
                        source,
                    })?;
                for instruction in &function.instructions {
                    let machine = function.machine;
                    let row = encoded
                        .next()
                        .filter(|row| {
                            row.machine == machine && row.instruction == instruction.instruction
                        })
                        .ok_or(Error::Mismatch(
                            "installed encoding differs from physical instruction order",
                        ))?;
                    let effects = row.effects().ok_or(Error::Mismatch(
                        "installed instruction lacks encoded effects",
                    ))?;
                    let fail = |reason: &str| Error::InstalledFaultSurface {
                        machine,
                        instruction: instruction.instruction,
                        detail: format!(
                            "{reason}; selected={}, encoded={effects:?}",
                            instruction.description
                        ),
                    };
                    if let machine_code::fault_origins::FaultOperation::Call(target) =
                        instruction.operation
                    {
                        let Some(target) = target.machine() else {
                            return Err(fail("dynamic call has no closed fault analysis"));
                        };
                        if machines.binary_search(&target).is_err() {
                            return Err(fail("callee is outside the exact retained body closure"));
                        }
                    }
                    match effects.trap {
                        Trap::NeverV1 => {}
                        Trap::MayArchitecturalFaultV1 => {
                            use machine_code::fault_origins::FaultOperation;
                            let memory = instruction.operation == FaultOperation::MemoryOrStack;
                            let privileged = instruction.operation == FaultOperation::Privileged;
                            let relative = relative_branches.contains(instruction.instruction);
                            if !memory
                                && !privileged
                                && !relative
                                && !matches!(instruction.operation, FaultOperation::Call(_))
                            {
                                return Err(fail(
                                    "architectural fault source is not discharged by checked RAM/stack or CPL0",
                                ));
                            }
                        }
                        _ => {
                            return Err(fail(
                                "instruction retains an architectural trap or hosted failure route",
                            ));
                        }
                    }
                }
            }
            if encoded.next().is_some() {
                return Err(Error::Mismatch(
                    "installed physical closure omits a selected instruction",
                ));
            }
            Ok(Some(InstalledBodyFaultPremises {
                physical_program_identity: origins.physical_identity,
            }))
        }
    }

    impl From<installation_admission::arrival::ArrivalCompositionError> for Error {
        fn from(error: installation_admission::arrival::ArrivalCompositionError) -> Self {
            match error {
                installation_admission::arrival::ArrivalCompositionError::Mismatch(reason) => {
                    Self::Mismatch(reason)
                }
                installation_admission::arrival::ArrivalCompositionError::Overflow => {
                    Self::Overflow
                }
            }
        }
    }

    pub fn installed_interrupt_stack_requirement(
        private: &PrivateFunctionObject,
        installation_machine: semantic_vocabulary::MachineId,
        operation: semantic_vocabulary::OperationId,
    ) -> Result<InstalledInterruptStackRequirement, Error> {
        let placement_index =
            private
                .identity
                .callback_thunk_placement_index()
                .ok_or(Error::Mismatch(
                    "installed stack demand has no private placement",
                ))?;
        if function_identity::MachineFunctionIdentity::callback_thunk(
            private.body.psi(),
            placement_index,
        ) != Some(private.identity)
            || private.body.composition_source.is_some()
            || private.body.physical_entry.is_some()
        {
            return Err(Error::Mismatch(
                "installed stack demand changed its original private body",
            ));
        }
        let bridge = private
            .entry_bridge
            .as_ref()
            .filter(|bridge| bridge.installed_interrupt().is_some())
            .ok_or(Error::Mismatch(
                "installed stack demand has no admitted arrival bridge",
            ))?;
        let body =
            super::stack_demand::derive_body_stack_demand(&private.body, private.body.entry())
                .map_err(Error::PrivateFunctions)?;
        let requirement = InstalledInterruptStackRequirement {
            installation_machine,
            arrival: None,
            fault_exclusion: faults::derive(&private.body)?,
            psi: private.body.psi(),
            target: private.body.target(),
            operation,
            placement_index,
            entry: private.body.entry(),
            entry_plan: bridge
                .installed_interrupt()
                .ok_or(Error::Mismatch("installed entry plan is missing"))?,
            body_bytes: body.ceiling_bytes().ok_or(Error::Mismatch(
                "installed stack requires a finite demand without hosted foreign calls",
            ))?,
            bridge_bytes: bridge.frame_bytes(),
            call_stack_bias: bridge.call_stack_bias(),
            alignment: body
                .stack_alignment()
                .max(u32::from(bridge.request().internal.stack_alignment)),
        };
        requirement.required_bytes().ok_or(Error::Overflow)?;
        Ok(requirement)
    }

    pub(crate) fn validate_installed_stack_requirements(
        root: &crate::ObjectArtifact,
        private_objects: &[PrivateFunctionObject],
    ) -> Result<(), Error> {
        let key = |identity: function_identity::MachineFunctionIdentity| {
            (
                identity.caller().vocabulary_marker.get(),
                *identity.caller().program_fingerprint.as_bytes(),
                identity.callback_thunk_placement_index(),
            )
        };
        let mut objects: Vec<_> = private_objects.iter().collect();
        objects.sort_unstable_by_key(|row| key(row.identity));
        if objects
            .windows(2)
            .any(|pair| key(pair[0].identity) == key(pair[1].identity))
        {
            return Err(Error::Mismatch("final private identity is duplicated"));
        }
        for body in std::iter::once(root).chain(private_objects.iter().map(|row| row.body.as_ref()))
        {
            let authority = body.emission_input.installation_authority();
            let mut actual_requirements = Vec::new();
            for expected in authority.installed_stack_requirements() {
                let identity = function_identity::MachineFunctionIdentity::callback_thunk(
                    expected.psi,
                    expected.placement_index,
                )
                .ok_or(Error::Mismatch(
                    "installed stack requirement has invalid private identity",
                ))?;
                let private = objects
                    .binary_search_by_key(&key(identity), |row| key(row.identity))
                    .ok()
                    .and_then(|position| objects.get(position))
                    .copied()
                    .ok_or(Error::Mismatch(
                        "installed stack guard has no final private body",
                    ))?;
                actual_requirements.push(installed_interrupt_stack_requirement(
                    private,
                    expected.installation_machine,
                    expected.operation,
                )?);
            }
            for scope in authority.installed_interrupts().scopes() {
                let first = actual_requirements.partition_point(|row| {
                    (row.installation_machine, row.operation) < (scope.machine(), scope.operation())
                });
                let last = actual_requirements.partition_point(|row| {
                    (row.installation_machine, row.operation)
                        <= (scope.machine(), scope.operation())
                });
                if first == last {
                    continue;
                }
                let entries: Vec<_> = authority
                    .installed_interrupts()
                    .entries()
                    .iter()
                    .filter(|entry| {
                        entry.machine == scope.machine() && entry.operation == scope.operation()
                    })
                    .collect();
                let proposed_root = authority.installed_stack_requirements()[first]
                    .arrival
                    .as_ref()
                    .map(|arrival| arrival.physical_entry);
                if proposed_root
                    .is_some_and(|entry| !std::ptr::eq(body, root) || entry.adapter != root.entry())
                {
                    return Err(Error::Mismatch(
                        "private installation cannot borrow the application's physical entry stack",
                    ));
                }
                installation_admission::arrival::compose(
                    scope,
                    &entries,
                    &mut actual_requirements[first..last],
                    proposed_root,
                )?;
            }
            if actual_requirements != authority.installed_stack_requirements() {
                return Err(Error::Mismatch(
                    "installed stack guard differs from final body and complete arrival demand",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn root_reserve(
        artifact: &crate::ObjectArtifact,
        root: installation_conditions::InstalledPhysicalStackRoot,
    ) -> Result<u64, Error> {
        if artifact.entry() != root.adapter {
            return Err(Error::Mismatch(
                "installed root stack has a different physical adapter",
            ));
        }
        let source = artifact.composition_source.as_deref().unwrap_or(artifact);
        let mut reserve = 0;
        for body in std::iter::once(source).chain(
            artifact
                .private_functions
                .iter()
                .map(|entry| entry.body.as_ref()),
        ) {
            for requirement in body
                .emission_input
                .installation_authority()
                .installed_stack_requirements()
            {
                let Some(arrival) = &requirement.arrival else {
                    continue;
                };
                let expected = arrival.physical_entry;
                if expected != root
                    || !std::ptr::eq(body, source)
                    || requirement.psi != artifact.psi()
                    || requirement.target != artifact.target()
                {
                    return Err(Error::Mismatch(
                        "arrival root demand differs from the actual physical entry binding",
                    ));
                }
                reserve = reserve.max(arrival.root_reserve_bytes);
            }
        }
        Ok(reserve)
    }

    pub(crate) fn requires_root_provision(artifact: &crate::ObjectArtifact) -> bool {
        let source = artifact.composition_source.as_deref().unwrap_or(artifact);
        std::iter::once(source)
            .chain(
                artifact
                    .private_functions
                    .iter()
                    .map(|entry| entry.body.as_ref()),
            )
            .any(|body| {
                body.emission_input
                    .installation_authority()
                    .installed_stack_requirements()
                    .iter()
                    .any(|row| row.arrival.is_some())
            })
    }

    pub fn require_installed_root_provision(artifact: &crate::ObjectArtifact) -> Result<(), Error> {
        if requires_root_provision(artifact) && artifact.physical_entry_binding().is_none() {
            return Err(Error::Mismatch(
                "interrupt installation has no matching physical-entry stack provision",
            ));
        }
        Ok(())
    }
}

pub(crate) mod replay {
    pub(crate) mod boundary {
        pub(crate) mod byte_sequence_custody {
            use machine_code::BoundarySettlementRecord;
            use target::{Architecture, NativeTarget, ObjectFormat};
            use target_operations::BoundaryRealization;

            pub fn linux_write_line_custody_is_exact(
                target: NativeTarget,
                settlement: &BoundarySettlementRecord,
                function_bytes: Option<&[u8]>,
            ) -> bool {
                let BoundaryRealization::LinuxWriteLine(_) = settlement.realization else {
                    return false;
                };
                let [custody] = settlement.byte_sequence_arguments.as_slice() else {
                    return false;
                };
                if target.object_format != ObjectFormat::Elf
                    || !matches!(
                        target.architecture,
                        Architecture::X86_64 | Architecture::Aarch64
                    )
                    || !settlement.scalar_arguments.is_empty()
                    || !settlement.runtime_scalar_arguments.is_empty()
                    || settlement.arguments.as_slice() != [custody.argument.clone()]
                    || !custody.argument.path.is_empty()
                    || !custody
                        .structural_type
                        .shape
                        .is_borrowed_view_byte_sequence()
                    || !settlement.native_result.is_unit()
                {
                    return false;
                }
                let layout = match target.architecture {
                    Architecture::X86_64 => target_x86_64::write_line::layout(custody.bytes.len()),
                    Architecture::Aarch64 => {
                        target_aarch64::write_line::layout(custody.bytes.len())
                    }
                };
                let Some((data, total)) = layout else {
                    return false;
                };
                if settlement.byte_count != total
                    || custody.code_offset != settlement.code_offset
                    || custody.code_byte_count != data.start
                    || settlement.code_offset.checked_add(data.start) != Some(custody.data_offset)
                    || custody.data_byte_count != data.len()
                {
                    return false;
                }
                function_bytes.is_none_or(|function| {
                    let Some(bytes) = settlement
                        .code_offset
                        .checked_add(total)
                        .and_then(|end| function.get(settlement.code_offset..end))
                    else {
                        return false;
                    };
                    let literal = match target.architecture {
                        Architecture::X86_64 => target_x86_64::write_line::decode(bytes),
                        Architecture::Aarch64 => target_aarch64::write_line::decode(bytes),
                    };
                    literal == Some(custody.bytes.as_slice())
                })
            }
        }

        pub(crate) mod completion_receipts {
            use std::collections::BTreeSet;

            use machine_code::{BoundarySettlementRecord, derive_completion_provider_custody};
            use semantic_vocabulary::{ClaimId, ContentPlaceSegment, ContentPlaceVersion};
            use target_operations::CompletionClaimSource;
            use terminal_psi::{CompletionReceipt, StructuralArgument, StructuralPathSegment};

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) enum CompletionCustodyError {
                ArgumentPath,
                ReceiptArgumentIndex,
                ReceiptCustody,
                ProviderCustody,
            }

            pub(crate) fn validate_completion_custody(
                settlement: &BoundarySettlementRecord,
                identities: &terminal_psi::TerminalIdentityTable,
            ) -> Result<(), CompletionCustodyError> {
                if settlement.arguments.iter().any(|argument| {
                    argument.path.iter().any(
                        |segment| matches!(segment, StructuralPathSegment::Field(identity) if *identity == terminal_psi::TerminalIdentityIndex::ABSENT),
                    )
                }) {
                    return Err(CompletionCustodyError::ArgumentPath);
                }
                if settlement.completion_receipts.iter().any(|receipt| {
                    usize::try_from(receipt.argument_index)
                        .map_or(true, |index| index >= settlement.arguments.len())
                }) {
                    return Err(CompletionCustodyError::ReceiptArgumentIndex);
                }
                if !completion_receipts_have_exact_custody(
                    identities,
                    &settlement.arguments,
                    &settlement.completion_claim_sources,
                    &settlement.completion_receipts,
                ) {
                    return Err(CompletionCustodyError::ReceiptCustody);
                }
                let execution_realization_pair_is_exact =
                    match (settlement.execution, settlement.realization) {
                        (
                            machine_code::BoundaryExecutionRecord::AdmittedProvider(_),
                            target_operations::BoundaryRealization::HostedWriteByteI32(_),
                        ) => true,
                        (
                            machine_code::BoundaryExecutionRecord::CompilerBuiltin(
                                target_operations::CompilerBuiltinExecution::HostedExitProcessI32,
                            ),
                            target_operations::BoundaryRealization::HostedExitProcessI32(_),
                        )
                        | (
                            machine_code::BoundaryExecutionRecord::CompilerBuiltin(
                                target_operations::CompilerBuiltinExecution::HostedWriteByteI32,
                            ),
                            target_operations::BoundaryRealization::HostedWriteByteI32(_),
                        )
                        | (
                            machine_code::BoundaryExecutionRecord::CompilerBuiltin(
                                target_operations::CompilerBuiltinExecution::HostedReadByte,
                            ),
                            target_operations::BoundaryRealization::HostedReadByte(_),
                        ) => true,
                        (machine_code::BoundaryExecutionRecord::CompilerBuiltin(_), _)
                        | (machine_code::BoundaryExecutionRecord::NativeBinding(_), _)
                        | (
                            _,
                            target_operations::BoundaryRealization::HostedExitProcessI32(_)
                            | target_operations::BoundaryRealization::HostedReadByte(_),
                        ) => false,
                        _ => true,
                    };
                if !execution_realization_pair_is_exact {
                    return Err(CompletionCustodyError::ProviderCustody);
                }
                if derive_completion_provider_custody(
                    settlement.execution,
                    &settlement.completion_claim_sources,
                    &settlement.completion_receipts,
                )
                .is_none_or(|expected| expected != settlement.completion_provider_custody)
                {
                    return Err(CompletionCustodyError::ProviderCustody);
                }
                Ok(())
            }

            pub fn completion_receipts_have_exact_custody(
                identities: &terminal_psi::TerminalIdentityTable,
                arguments: &[StructuralArgument],
                sources: &[CompletionClaimSource],
                receipts: &[CompletionReceipt],
            ) -> bool {
                let mut source_claims = BTreeSet::<ClaimId>::new();
                if sources.windows(2).any(|pair| pair[0] >= pair[1])
                    || sources.iter().any(|source| {
                        !source_claims.insert(source.claim())
                            || !claim_source_is_canonical(source, identities)
                    })
                {
                    return false;
                }

                let expected = arguments
                    .iter()
                    .enumerate()
                    .flat_map(|(index, argument)| {
                        sources.iter().filter_map(move |source| {
                            let argument_index = u32::try_from(index).ok()?;
                            (source.input() == argument.place
                                && match &source.entry {
                                    Some(source) => {
                                        argument.path.is_empty() || source.path == argument.path
                                    }
                                    None => true,
                                })
                            .then_some((argument_index, source.claim()))
                        })
                    })
                    .collect::<BTreeSet<_>>();
                let actual = receipts
                    .iter()
                    .map(|receipt| (receipt.argument_index, receipt.claim))
                    .collect::<BTreeSet<_>>();
                let mut receipt_claims = BTreeSet::<ClaimId>::new();
                receipts.windows(2).all(|pair| pair[0] < pair[1])
                    && receipts
                        .iter()
                        .all(|receipt| receipt_claims.insert(receipt.claim))
                    && actual == expected
            }

            fn claim_source_is_canonical(
                source: &CompletionClaimSource,
                identities: &terminal_psi::TerminalIdentityTable,
            ) -> bool {
                let entry_is_canonical = source.entry.as_ref().is_none_or(|entry| {
                    entry.claim == source.claim
                        && entry.path.iter().all(|segment| {
                        !matches!(segment, StructuralPathSegment::Field(identity) if *identity == terminal_psi::TerminalIdentityIndex::ABSENT)
                    })
                });
                let content_is_canonical = source.content.as_ref().is_none_or(|content| {
                    content.claim == source.claim
                        && content.input.version == ContentPlaceVersion::Entry
                        && !content.projections.is_empty()
                        && !content
                            .projections
                            .windows(2)
                            .any(|pair| pair[0] >= pair[1])
                        && content.input.segments.iter().all(|segment| {
                            !matches!(
                                segment,
                                ContentPlaceSegment::Case(identity)
                                    | ContentPlaceSegment::Field(identity)
                                    if identity.is_empty()
                            )
                        })
                        && content.projections.iter().all(|projection| {
                            projection.projection.projection_report_fingerprint != 0
                                && !projection.algebra.parameter.is_empty()
                        })
                });
                let paired_sources_match = match (&source.entry, &source.content) {
                    (Some(entry), Some(content)) => {
                        entry.input == content.input.root
                            && entry.path.len() == content.input.segments.len()
                            && entry.path.iter().zip(&content.input.segments).all(
                                |(entry, content)| match (entry, content) {
                                    (
                                        StructuralPathSegment::Field(entry),
                                        ContentPlaceSegment::Field(content),
                                    ) => identities.text(*entry) == Some(content.as_str()),
                                    (
                                        StructuralPathSegment::FixedIndex(entry),
                                        ContentPlaceSegment::FixedIndex(content),
                                    ) => entry == content,
                                    _ => false,
                                },
                            )
                    }
                    _ => true,
                };
                (source.entry.is_some() || source.content.is_some())
                    && entry_is_canonical
                    && content_is_canonical
                    && paired_sources_match
            }

            #[cfg(test)]
            mod tests {
                use super::{
                    BoundarySettlementRecord, ClaimId, CompletionClaimSource,
                    CompletionCustodyError, CompletionReceipt, ContentPlaceSegment,
                    ContentPlaceVersion, StructuralArgument, StructuralPathSegment,
                    completion_receipts_have_exact_custody, derive_completion_provider_custody,
                    validate_completion_custody,
                };

                fn settlement(
                    arguments: Vec<StructuralArgument>,
                    completion_claim_sources: Vec<CompletionClaimSource>,
                    completion_receipts: Vec<CompletionReceipt>,
                ) -> BoundarySettlementRecord {
                    let provider_execution =
                        machine_code::ProviderExecutionRecord::new(1, 2, 3, 4, 5)
                            .expect("provider execution");
                    let completion_provider_custody = derive_completion_provider_custody(
                        machine_code::BoundaryExecutionRecord::AdmittedProvider(provider_execution),
                        &completion_claim_sources,
                        &completion_receipts,
                    )
                    .unwrap_or_default();
                    BoundarySettlementRecord {
                        invocation_site: semantic_vocabulary::OperationId::new(1)
                            .expect("operation")
                            .into(),
                        boundary: semantic_vocabulary::BoundaryMachineId::new(1).expect("boundary"),
                        execution: machine_code::BoundaryExecutionRecord::AdmittedProvider(
                            provider_execution,
                        ),
                        realization: target_operations::BoundaryRealization::MetadataOnlyPort(
                            target_operations::MetadataOnlyPortRealization {
                                effect_operation: semantic_vocabulary::OperationId::new(2)
                                    .expect("effect operation"),
                                service: semantic_vocabulary::ServiceId::new(1).expect("service"),
                                port: 0x20,
                                value: 0x20,
                            },
                        ),
                        scalar_arguments: Vec::new(),
                        runtime_scalar_arguments: Vec::new(),
                        arguments,
                        byte_sequence_arguments: Vec::new(),
                        completion_claim_sources,
                        completion_receipts,
                        completion_provider_custody,
                        native_result: machine_code::BoundaryResultRecord::Unit,
                        operation_ordinal: 1,
                        code_offset: 0,
                        byte_count: 0,
                    }
                }

                fn receipt(claim: u64, argument_index: u32) -> CompletionReceipt {
                    CompletionReceipt {
                        claim: ClaimId::new(claim).expect("claim"),
                        argument_index,
                        path: Vec::new(),
                    }
                }

                fn argument(place: u64) -> StructuralArgument {
                    StructuralArgument {
                        access: terminal_psi::StructuralAccess::Owned,
                        place: semantic_vocabulary::PlaceId::new(place).expect("place"),
                        path: Vec::new(),
                    }
                }

                fn source(claim: u64, place: u64) -> CompletionClaimSource {
                    let claim = ClaimId::new(claim).expect("claim");
                    CompletionClaimSource {
                        claim,
                        entry: Some(terminal_psi::EntryClaim {
                            claim,
                            input: semantic_vocabulary::PlaceId::new(place).expect("place"),
                            path: Vec::new(),
                        }),
                        content: None,
                    }
                }

                #[test]
                fn exact_completion_receipt_custody_replays_the_claim_catalog() {
                    assert!(completion_receipts_have_exact_custody(
                        &Default::default(),
                        &[],
                        &[],
                        &[]
                    ));
                    assert!(completion_receipts_have_exact_custody(
                        &Default::default(),
                        &[argument(1)],
                        &[source(1, 1)],
                        &[receipt(1, 0)],
                    ));
                    assert!(completion_receipts_have_exact_custody(
                        &Default::default(),
                        &[argument(1), argument(2),],
                        &[source(1, 2), source(2, 1),],
                        &[receipt(1, 1), receipt(2, 0)]
                    ));

                    assert!(!completion_receipts_have_exact_custody(
                        &Default::default(),
                        &[argument(1), argument(2),],
                        &[source(1, 2), source(2, 1)],
                        &[receipt(2, 0), receipt(1, 1)]
                    ));
                    assert!(!completion_receipts_have_exact_custody(
                        &Default::default(),
                        &[argument(1), argument(2)],
                        &[source(1, 1)],
                        &[receipt(1, 0), receipt(1, 1)],
                    ));
                    assert!(!completion_receipts_have_exact_custody(
                        &Default::default(),
                        &[argument(1)],
                        &[source(1, 1)],
                        &[receipt(1, 0), receipt(1, 0)],
                    ));
                    assert!(!completion_receipts_have_exact_custody(
                        &Default::default(),
                        &[argument(1)],
                        &[source(1, 1)],
                        &[],
                    ));

                    let projected = StructuralArgument {
                        access: terminal_psi::StructuralAccess::Owned,
                        place: semantic_vocabulary::PlaceId::new(1).expect("place"),
                        path: vec![StructuralPathSegment::FixedIndex(2)],
                    };
                    let mut entry_projection = source(1, 1);
                    entry_projection.entry.as_mut().unwrap().path =
                        vec![StructuralPathSegment::FixedIndex(1)];
                    assert!(!completion_receipts_have_exact_custody(
                        &Default::default(),
                        std::slice::from_ref(&projected),
                        &[entry_projection],
                        &[receipt(1, 0)],
                    ));
                    let claim = ClaimId::new(1).expect("claim");
                    let content_root = CompletionClaimSource {
                        claim,
                        entry: None,
                        content: Some(terminal_psi::ContentEntryClaim {
                            claim,
                            input: semantic_vocabulary::ContentStructuralPlace {
                                version: ContentPlaceVersion::Entry,
                                root: semantic_vocabulary::PlaceId::new(1).expect("place"),
                                segments: vec![ContentPlaceSegment::FixedIndex(1)],
                            },
                            projections: vec![terminal_psi::ClaimContentProjection {
                                projection: semantic_vocabulary::ContentProjectionIdentity {
                                    domain: semantic_vocabulary::ContentDomainId::new(1)
                                        .expect("domain"),
                                    projection_report_fingerprint: 7,
                                },
                                algebra: semantic_vocabulary::ContentAlgebra {
                                    kind: semantic_vocabulary::ContentAlgebraKind::CountedQuantity,
                                    parameter: "Bytes".into(),
                                },
                            }],
                        }),
                    };
                    assert!(completion_receipts_have_exact_custody(
                        &Default::default(),
                        &[projected],
                        std::slice::from_ref(&content_root),
                        &[receipt(1, 0)],
                    ));

                    let mut combined = content_root;
                    combined.entry = Some(terminal_psi::EntryClaim {
                        claim,
                        input: semantic_vocabulary::PlaceId::new(1).expect("place"),
                        path: vec![StructuralPathSegment::FixedIndex(1)],
                    });
                    assert!(completion_receipts_have_exact_custody(
                        &Default::default(),
                        &[argument(1)],
                        std::slice::from_ref(&combined),
                        &[receipt(1, 0)],
                    ));
                    combined.content.as_mut().unwrap().input.segments[0] =
                        ContentPlaceSegment::FixedIndex(2);
                    assert!(!completion_receipts_have_exact_custody(
                        &Default::default(),
                        &[argument(1)],
                        &[combined],
                        &[receipt(1, 0)],
                    ));
                }

                #[test]
                fn complete_custody_replay_preserves_validation_precedence() {
                    let valid =
                        settlement(vec![argument(1)], vec![source(1, 1)], vec![receipt(1, 0)]);
                    assert_eq!(
                        validate_completion_custody(&valid, &Default::default()),
                        Ok(())
                    );

                    let mut invalid_argument = valid.clone();
                    invalid_argument.arguments[0].path = vec![StructuralPathSegment::Field(
                        terminal_psi::TerminalIdentityIndex(0),
                    )];
                    invalid_argument.completion_receipts[0].argument_index = 2;
                    assert_eq!(
                        validate_completion_custody(&invalid_argument, &Default::default()),
                        Err(CompletionCustodyError::ArgumentPath)
                    );

                    let mut invalid_index = valid.clone();
                    invalid_index.completion_receipts[0].argument_index = 2;
                    assert_eq!(
                        validate_completion_custody(&invalid_index, &Default::default()),
                        Err(CompletionCustodyError::ReceiptArgumentIndex)
                    );

                    let mut invalid_receipt = valid.clone();
                    invalid_receipt.completion_receipts[0].claim =
                        ClaimId::new(2).expect("other claim");
                    assert_eq!(
                        validate_completion_custody(&invalid_receipt, &Default::default()),
                        Err(CompletionCustodyError::ReceiptCustody)
                    );

                    let mut invalid_provider = valid;
                    invalid_provider.completion_provider_custody[0]
                        .provider_execution
                        .provider_plan_report_identity = 9;
                    assert_eq!(
                        validate_completion_custody(&invalid_provider, &Default::default()),
                        Err(CompletionCustodyError::ProviderCustody)
                    );

                    let mut role_substitution = settlement(Vec::new(), Vec::new(), Vec::new());
                    role_substitution.execution =
                        machine_code::BoundaryExecutionRecord::CompilerBuiltin(
                            target_operations::CompilerBuiltinExecution::HostedExitProcessI32,
                        );
                    assert_eq!(
                        validate_completion_custody(&role_substitution, &Default::default()),
                        Err(CompletionCustodyError::ProviderCustody)
                    );

                    let mut provider_output = settlement(Vec::new(), Vec::new(), Vec::new());
                    provider_output.realization =
                        target_operations::BoundaryRealization::HostedWriteByteI32(
                            Default::default(),
                        );
                    assert_eq!(
                        validate_completion_custody(&provider_output, &Default::default()),
                        Ok(())
                    );

                    let mut reverse_role_substitution = provider_output;
                    reverse_role_substitution.realization =
                        target_operations::BoundaryRealization::HostedExitProcessI32(
                            Default::default(),
                        );
                    assert_eq!(
                        validate_completion_custody(
                            &reverse_role_substitution,
                            &Default::default()
                        ),
                        Err(CompletionCustodyError::ProviderCustody)
                    );
                }
            }
        }

        pub(crate) mod result_placement {
            #[cfg(test)]
            use crate::object::object_artifact::test_support::structural_declaration;
            use semantic_vocabulary::{IntegerSign, ScalarType};

            pub(crate) fn hosted_read_byte_declaration_is_valid(
                declaration: &terminal_psi::StructuralTypeDeclaration,
            ) -> bool {
                let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
                    return false;
                };
                let [empty, byte] = cases.as_slice() else {
                    return false;
                };
                let [field] = byte.fields.as_slice() else {
                    return false;
                };
                empty.fields.is_empty()
                    && declaration.identity != terminal_psi::TerminalIdentityIndex::ABSENT
                    && empty.identity != terminal_psi::TerminalIdentityIndex::ABSENT
                    && byte.identity != terminal_psi::TerminalIdentityIndex::ABSENT
                    && field.identity != terminal_psi::TerminalIdentityIndex::ABSENT
                    && empty.id < byte.id
                    && empty.identity != byte.identity
                    && !field.relevance.is_erased()
                    && match field.field_type {
                        terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
                            !integer.is_address()
                                && integer.sign() == IntegerSign::Signed
                                && integer.bits() == Some(32)
                        }
                        terminal_psi::StructuralFieldType::BoundedInteger(bounds) => {
                            let integer = bounds.integer_type();
                            !integer.is_address()
                                && integer.sign() == IntegerSign::Signed
                                && integer.bits() == Some(32)
                                && bounds.contains(semantic_vocabulary::IntegerValue::Signed(0))
                                && bounds.contains(semantic_vocabulary::IntegerValue::Signed(255))
                        }
                        _ => false,
                    }
            }

            #[cfg(test)]
            pub(crate) fn test_byte_read_declaration(
                id: semantic_vocabulary::StructuralTypeId,
            ) -> terminal_psi::StructuralTypeDeclaration {
                structural_declaration(
                    id,
                    terminal_psi::TerminalIdentityIndex(1),
                    terminal_psi::StructuralTypeShape::Sum {
                        cases: vec![
                            terminal_psi::StructuralCaseDeclaration {
                                declaration_identity: Default::default(),
                                id: semantic_vocabulary::StructuralCaseId::new(1).unwrap(),
                                identity: terminal_psi::TerminalIdentityIndex(2),
                                fields: vec![],
                            },
                            terminal_psi::StructuralCaseDeclaration {
                                declaration_identity: Default::default(),
                                id: semantic_vocabulary::StructuralCaseId::new(2).unwrap(),
                                identity: terminal_psi::TerminalIdentityIndex(3),
                                fields: vec![terminal_psi::StructuralFieldDeclaration {
                                    declaration_identity: Default::default(),
                                    inline_carrier: None,
                                    storage: None,
                                    id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                                    identity: terminal_psi::TerminalIdentityIndex(4),
                                    relevance: terminal_psi::BindingRelevance::Relevant,
                                    field_type: terminal_psi::StructuralFieldType::BoundedInteger(
                                        semantic_vocabulary::BoundedIntegerType::new(
                                            semantic_vocabulary::IntegerType::new(
                                                IntegerSign::Signed,
                                                32,
                                            )
                                            .unwrap(),
                                            semantic_vocabulary::IntegerValue::Signed(0),
                                            semantic_vocabulary::IntegerValue::Signed(255),
                                        )
                                        .unwrap(),
                                    ),
                                }],
                            },
                        ],
                    },
                )
            }

            #[cfg(test)]
            mod tests {
                use super::{hosted_read_byte_declaration_is_valid, test_byte_read_declaration};
                use semantic_vocabulary::StructuralTypeId;

                #[test]
                fn read_byte_declaration_requires_a_known_signed_32_field() {
                    use semantic_vocabulary::{
                        IntegerSign, IntegerType, MachineId, ScalarType, ValueId, WidthParameter,
                    };
                    let mut declaration =
                        test_byte_read_declaration(StructuralTypeId::new(3).unwrap());
                    assert!(hosted_read_byte_declaration_is_valid(&declaration));
                    let terminal_psi::StructuralTypeShape::Sum { cases } = &mut declaration.shape
                    else {
                        unreachable!()
                    };
                    cases[1].fields[0].field_type = terminal_psi::StructuralFieldType::Scalar(
                        ScalarType::Integer(IntegerType::parameterized(
                            IntegerSign::Signed,
                            WidthParameter {
                                owner: MachineId::new(1).unwrap(),
                                value: ValueId::new(1).unwrap(),
                            },
                        )),
                    );
                    assert!(!hosted_read_byte_declaration_is_valid(&declaration));
                }

                #[test]
                fn read_byte_declaration_replay_rejects_noncanonical_case_and_field_custody() {
                    let original = test_byte_read_declaration(StructuralTypeId::new(3).unwrap());
                    for mutation in 0..8 {
                        let mut changed = original.clone();
                        let terminal_psi::StructuralTypeShape::Sum { cases } = &mut changed.shape
                        else {
                            unreachable!()
                        };
                        match mutation {
                            0 => changed.identity = terminal_psi::TerminalIdentityIndex(0),
                            1 => cases[0].identity = terminal_psi::TerminalIdentityIndex(0),
                            2 => cases[1].identity = terminal_psi::TerminalIdentityIndex(0),
                            3 => {
                                cases[1].fields[0].identity = terminal_psi::TerminalIdentityIndex(0)
                            }
                            4 => cases[1].id = cases[0].id,
                            5 => {
                                cases[0].id = semantic_vocabulary::StructuralCaseId::new(3).unwrap()
                            }
                            6 => cases[1].identity = cases[0].identity,
                            7 => {
                                cases[1].fields[0].relevance =
                                    terminal_psi::BindingRelevance::Erased
                            }
                            _ => unreachable!(),
                        }
                        assert!(
                            !hosted_read_byte_declaration_is_valid(&changed),
                            "mutation {mutation}"
                        );
                    }
                }
            }
        }

        pub(crate) mod runtime_scalar_custody {
            use calling_conventions::{MachineRegister, ValueLocation, ValueShape};
            use machine_code::{
                BoundaryExecutionRecord, BoundarySettlementRecord,
                InternalUnitScalarArgumentSourceRecord,
            };
            use semantic_vocabulary::{IntegerSign, ScalarType};
            use target::{Architecture, NativeTarget};
            use target_operations::CompilerBuiltinExecution;

            pub(crate) mod process_exit {
                use super::{
                    Architecture, BoundaryExecutionRecord, BoundarySettlementRecord,
                    CompilerBuiltinExecution, IntegerSign, InternalUnitScalarArgumentSourceRecord,
                    MachineRegister, NativeTarget, ScalarType, ValueLocation,
                };
                pub(crate) fn decode(
                    target: NativeTarget,
                    bytes: &[u8],
                ) -> Option<MachineRegister> {
                    match target.architecture {
                        Architecture::X86_64 => {
                            target_x86_64::inspection::decode_x86_64_selected_hosted_exit_process_i32(
                                target, bytes,
                            )
                        }
                        Architecture::Aarch64 => {
                            target_aarch64::inspection::decode_aarch64_selected_hosted_exit_process_i32(
                                target, bytes,
                            )
                        }
                    }
                }

                pub(crate) fn shape_is_exact(
                    target: NativeTarget,
                    settlement: &BoundarySettlementRecord,
                ) -> bool {
                    let Some(argument) = super::single_i32_register_argument(settlement, None)
                    else {
                        return false;
                    };
                    let InternalUnitScalarArgumentSourceRecord::SelectedProcessExit {
                        scalar_type,
                        ..
                    } = argument.source
                    else {
                        return false;
                    };
                    let byte_count = if target == NativeTarget::linux_x64()
                        || target == NativeTarget::macos_x64()
                    {
                        12
                    } else if target == NativeTarget::linux_arm64()
                        || target == NativeTarget::macos_arm64()
                    {
                        16
                    } else {
                        return false;
                    };
                    matches!(scalar_type, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Signed && integer.bits() == Some(32))
                        && settlement.execution
                            == BoundaryExecutionRecord::CompilerBuiltin(
                                CompilerBuiltinExecution::HostedExitProcessI32,
                            )
                        && matches!(
                            settlement.realization,
                            target_operations::BoundaryRealization::HostedExitProcessI32(_)
                        )
                        && settlement.byte_count == byte_count
                        && super::settlement_carries_only_runtime_scalars(settlement)
                }

                pub(crate) fn bytes_are_exact(
                    target: NativeTarget,
                    settlement: &BoundarySettlementRecord,
                    function_bytes: &[u8],
                ) -> bool {
                    if !shape_is_exact(target, settlement) {
                        return false;
                    }
                    let Some(end) = settlement.code_offset.checked_add(settlement.byte_count)
                    else {
                        return false;
                    };
                    let Some(bytes) = function_bytes.get(settlement.code_offset..end) else {
                        return false;
                    };
                    let Some(register) = decode(target, bytes) else {
                        return false;
                    };
                    settlement.runtime_scalar_arguments[0]
                        .placement
                        .locations
                        .as_slice()
                        == [ValueLocation::Register {
                            register,
                            value_byte_offset: 0,
                            byte_size: 4,
                        }]
                }
            }

            pub(crate) fn decode_selected_byte_output(
                target: NativeTarget,
                bytes: &[u8],
            ) -> Option<(MachineRegister, u32)> {
                if target == NativeTarget::linux_x64() || target == NativeTarget::macos_x64() {
                    target_x86_64::inspection::decode_x86_64_selected_hosted_write_byte_i32(
                        target, bytes,
                    )
                } else if target == NativeTarget::linux_arm64()
                    || target == NativeTarget::macos_arm64()
                {
                    target_aarch64::inspection::decode_aarch64_selected_hosted_write_byte_i32(
                        target, bytes,
                    )
                } else {
                    None
                }
            }

            pub(crate) fn selected_byte_output_bytes_are_exact(
                target: NativeTarget,
                settlement: &BoundarySettlementRecord,
                function_bytes: &[u8],
            ) -> bool {
                let [argument] = settlement.runtime_scalar_arguments.as_slice() else {
                    return false;
                };
                let InternalUnitScalarArgumentSourceRecord::SelectedBoundary {
                    scratch_byte_offset,
                    ..
                } = argument.source
                else {
                    return false;
                };
                let Some(end) = settlement.code_offset.checked_add(settlement.byte_count) else {
                    return false;
                };
                let Some(bytes) = function_bytes.get(settlement.code_offset..end) else {
                    return false;
                };
                let Some((register, offset)) = decode_selected_byte_output(target, bytes) else {
                    return false;
                };
                selected_byte_output_shape_is_exact(target, settlement)
                    && scratch_byte_offset == u64::from(offset)
                    && argument.placement.locations.as_slice()
                        == [ValueLocation::Register {
                            register,
                            value_byte_offset: 0,
                            byte_size: 4,
                        }]
            }

            pub(crate) fn single_i32_register_argument(
                settlement: &BoundarySettlementRecord,
                expected_register: Option<MachineRegister>,
            ) -> Option<&machine_code::ForeignCallScalarArgumentRecord> {
                let [argument] = settlement.runtime_scalar_arguments.as_slice() else {
                    return None;
                };
                (argument.parameter_index == 0
                    && argument.code_offset == settlement.code_offset
                    && argument.byte_count == settlement.byte_count
                    && argument.placement.shape == ValueShape::integer(4, 4)
                    && matches!(
                        argument.placement.locations.as_slice(),
                        [ValueLocation::Register {
                            register,
                            value_byte_offset: 0,
                            byte_size: 4,
                        }] if expected_register.is_none_or(|expected| *register == expected)
                    ))
                .then_some(argument)
            }

            pub(crate) fn hosted_i32_argument(
                source: InternalUnitScalarArgumentSourceRecord,
                register: MachineRegister,
                code_offset: usize,
                byte_count: usize,
            ) -> machine_code::ForeignCallScalarArgumentRecord {
                machine_code::ForeignCallScalarArgumentRecord {
                    parameter_index: 0,
                    source,
                    placement: calling_conventions::ValuePlacement {
                        shape: ValueShape::integer(4, 4),
                        locations: vec![ValueLocation::Register {
                            register,
                            value_byte_offset: 0,
                            byte_size: 4,
                        }],
                    },
                    code_offset,
                    byte_count,
                }
            }

            pub(crate) fn settlement_carries_only_runtime_scalars(
                settlement: &BoundarySettlementRecord,
            ) -> bool {
                settlement.native_result.is_unit()
                    && settlement.scalar_arguments.is_empty()
                    && settlement.arguments.is_empty()
                    && settlement.byte_sequence_arguments.is_empty()
                    && settlement.completion_claim_sources.is_empty()
                    && settlement.completion_receipts.is_empty()
                    && settlement.completion_provider_custody.is_empty()
            }

            pub(crate) fn selected_byte_output_shape_is_exact(
                target: NativeTarget,
                settlement: &BoundarySettlementRecord,
            ) -> bool {
                let Some(argument) = single_i32_register_argument(settlement, None) else {
                    return false;
                };
                let InternalUnitScalarArgumentSourceRecord::SelectedBoundary {
                    scalar_type,
                    scratch_byte_offset,
                    ..
                } = argument.source
                else {
                    return false;
                };
                let byte_count = if target == NativeTarget::linux_x64() {
                    40
                } else if target == NativeTarget::macos_x64() {
                    42
                } else if target == NativeTarget::linux_arm64()
                    || target == NativeTarget::macos_arm64()
                {
                    let Some(byte_count) =
                        u32::try_from(scratch_byte_offset).ok().and_then(|home| {
                            target_aarch64::inspection::aarch64_hosted_write_byte_i32_byte_count(
                                target, home,
                            )
                        })
                    else {
                        return false;
                    };
                    byte_count
                } else {
                    return false;
                };
                matches!(scalar_type, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Signed && integer.bits() == Some(32))
                    && settlement.byte_count == byte_count
                    && matches!(
                        settlement.execution,
                        BoundaryExecutionRecord::AdmittedProvider(_)
                            | BoundaryExecutionRecord::CompilerBuiltin(
                                CompilerBuiltinExecution::HostedWriteByteI32
                            )
                    )
                    && matches!(
                        settlement.realization,
                        target_operations::BoundaryRealization::HostedWriteByteI32(_)
                    )
                    && settlement_carries_only_runtime_scalars(settlement)
            }

            #[cfg(test)]
            mod tests {
                use super::{MachineRegister, NativeTarget, decode_selected_byte_output};

                #[test]
                fn selected_hosted_output_decoder_binds_the_exact_operating_system() {
                    let linux = [
                        0x3900_03e9_u32,
                        0xd280_0020,
                        0x9100_03e1,
                        0xd280_0022,
                        0xd280_0808,
                        0xd400_0001,
                        0xf100_001f,
                        0x5400_004c,
                        0xd420_0000,
                    ];
                    let macos = [
                        0x3900_03e9_u32,
                        0xd280_0020,
                        0x9100_03e1,
                        0xd280_0022,
                        0xd280_0090,
                        0xd400_1001,
                        0x5400_0062,
                        0xf100_041f,
                        0x5400_0040,
                        0xd420_0000,
                    ];
                    for (target, words) in [
                        (NativeTarget::linux_arm64(), linux.as_slice()),
                        (NativeTarget::macos_arm64(), macos.as_slice()),
                    ] {
                        let bytes = words
                            .iter()
                            .flat_map(|word| word.to_le_bytes())
                            .collect::<Vec<_>>();
                        assert_eq!(
                            decode_selected_byte_output(target, &bytes),
                            Some((MachineRegister::Aarch64X(9), 0))
                        );
                        for wrong in [
                            NativeTarget::linux_x64(),
                            NativeTarget::windows_x64(),
                            if target == NativeTarget::linux_arm64() {
                                NativeTarget::macos_arm64()
                            } else {
                                NativeTarget::linux_arm64()
                            },
                        ] {
                            assert_eq!(
                                decode_selected_byte_output(wrong, &bytes),
                                None,
                                "wrong target {wrong:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

pub(crate) mod stack_demand {
    pub use machine_code::physical_stack::StackDemand;
    use machine_code::physical_stack::{
        ForeignStackExtent, LocalStackExtent, PhysicalForeignStackCall, PhysicalStackCall,
        PhysicalStackFunction, PhysicalStackProgram, derive_physical_stack_demand,
    };

    use semantic_vocabulary::MachineId;

    use crate::ObjectArtifact;
    use machine_code::physical_stack::StackDemandError;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ObjectStackError {
        Body(StackDemandError),
        PhysicalEntryStackArithmeticOverflow(MachineId),
        PrivateEntryStackArithmeticOverflow(function_identity::MachineFunctionIdentity),
    }
    diagnostics::debug_display!(ObjectStackError);
    impl std::error::Error for ObjectStackError {}

    pub fn derive_stack_demand(
        artifact: &ObjectArtifact,
        entry: MachineId,
    ) -> Result<StackDemand, ObjectStackError> {
        let mut demand = derive_body_stack_demand(artifact, entry)?;
        if let Some(binding) = artifact.physical_entry_binding()
            && binding.entry() == entry
        {
            let alignment = demand.stack_alignment();
            demand = demand
                .with_entry_cost(binding.stack_overhead_bytes(), alignment)
                .ok_or(ObjectStackError::PhysicalEntryStackArithmeticOverflow(
                    entry,
                ))?;
        }
        Ok(demand)
    }

    pub(crate) fn derive_body_stack_demand(
        artifact: &ObjectArtifact,
        entry: MachineId,
    ) -> Result<StackDemand, ObjectStackError> {
        derive_namespace_stack_demand(
            artifact,
            entry,
            &artifact.private_functions,
            &mut Vec::new(),
        )
    }

    fn derive_namespace_stack_demand(
        artifact: &ObjectArtifact,
        entry: MachineId,
        private_functions: &[crate::ObjectCompilerPrivateFunction],
        active_private: &mut Vec<function_identity::MachineFunctionIdentity>,
    ) -> Result<StackDemand, ObjectStackError> {
        let mut private_demands = Vec::new();
        for callback in artifact
            .foreign_calls
            .iter()
            .flat_map(|call| &call.callback_addresses)
            .filter(|callback| callback.target.registrar_invokes_callback)
        {
            let identity = callback.target.callback_function;
            if private_demands.iter().any(|(known, _)| *known == identity) {
                continue;
            }
            let private = private_functions
                .iter()
                .find(|private| private.identity == identity)
                .ok_or(ObjectStackError::Body(
                    StackDemandError::InvalidPrivateFunctionIdentity,
                ))?;
            if active_private.contains(&identity) {
                return Err(ObjectStackError::Body(
                    StackDemandError::TerminalStackCycle(private.body.entry),
                ));
            }
            active_private.push(identity);
            let mut demand = derive_namespace_stack_demand(
                &private.body,
                private.body.entry,
                private_functions,
                active_private,
            )?;
            if let Some(bridge) = &private.entry_bridge {
                let alignment = demand.stack_alignment();
                demand = demand
                    .with_entry_cost(
                        u64::from(bridge.frame_bytes()) + u64::from(bridge.call_stack_bias()),
                        alignment,
                    )
                    .ok_or(ObjectStackError::PrivateEntryStackArithmeticOverflow(
                        identity,
                    ))?;
            }
            active_private.pop();
            private_demands.push((identity, demand));
        }
        derive_physical_stack_demand(&artifact.physical_facts().stack, entry, private_demands)
            .map_err(ObjectStackError::Body)
    }

    pub(crate) fn validate_carried_stack_facts(artifact: &ObjectArtifact) -> bool {
        let canonical = |mut program: PhysicalStackProgram| {
            program
                .functions
                .sort_unstable_by_key(|function| function.machine);
            for function in &mut program.functions {
                function
                    .calls
                    .sort_unstable_by_key(|call| (call.owner, call.target, call.caller_live_bytes));
                function.foreign_calls.sort_by_key(|call| call.owner);
            }
            program
        };
        canonical(artifact.physical_facts().stack.clone())
            == canonical(physical_stack_facts(artifact))
    }

    fn physical_stack_facts(artifact: &ObjectArtifact) -> PhysicalStackProgram {
        PhysicalStackProgram {
            psi: artifact.psi, target: artifact.target,
            functions: artifact.functions.iter().map(|function| {
                let local = match (function.unit_stack, function.scalar_stack) {
                    (Some(_), Some(_)) => LocalStackExtent::Conflicting,
                    (Some(stack), None) => LocalStackExtent::Finite(u64::from(stack.local_peak_bytes)),
                    (None, Some(stack)) => LocalStackExtent::Finite(u64::from(stack.local_peak_bytes)),
                    (None, None) => {
                        let mut matching = artifact.boundary_settlements.iter().filter(|row| row.machine == function.machine);
                        let stackless = matching.next().is_some_and(|row| matching.next().is_none()
                            && matches!(row.settlement.realization, target_operations::BoundaryRealization::HostedExitProcessI32(_))
                            && row.settlement.code_offset == 0 && row.settlement.byte_count == function.byte_count);
                        if stackless { LocalStackExtent::Finite(0) } else { LocalStackExtent::Unaccounted }
                    }
                };
                PhysicalStackFunction {
                    machine: function.machine, local,
                    calls: function.unit_call_stacks.iter().map(|call| PhysicalStackCall {
                        target: call.target, owner: call.owner, caller_live_bytes: call.caller_live_bytes,
                    }).chain(function.scalar_call_stacks.iter().map(|call| PhysicalStackCall {
                        target: call.target, owner: call.owner, caller_live_bytes: call.caller_live_bytes,
                    })).collect(),
                    foreign_calls: artifact.foreign_calls.iter().filter(|call| call.machine == function.machine).map(|call| {
                        let extent = ForeignStackExtent::from_binding(&call.binding);
                        PhysicalForeignStackCall {
                            owner: call.owner, caller_live_bytes: call.caller_live_bytes, extent,
                            invoked_callbacks: call.callback_addresses.iter().filter(|callback| callback.target.registrar_invokes_callback)
                                .map(|callback| callback.target.callback_function).collect(),
                        }
                    }).collect(),
                }
            }).collect(),
        }
    }
}

pub use carriers::{
    NativeInstructionSourceRows, ObjectArtifact, ObjectBoundarySettlement, ObjectCodeAttribution,
    ObjectFunction, ObjectPrivateFunctionEntry, ObjectScalarCallStack, ObjectUnitCallStack,
    PrivateFunctionObject, PrivateObjectSymbolBinding,
};
pub(crate) use carriers::{
    ObjectCompilerPrivateFunction, ObjectPortEffect, ObjectScalarStack, ObjectUnitStack,
};
pub use object_file::{
    ObjectForeignCall, derive_normalized_foreign_call_custody, fragment_instruction_span,
    place_foreign_call, resolve_instruction_sequence,
};

#[cfg(test)]
pub(crate) mod test_support {
    pub(crate) fn structural_declaration(
        id: semantic_vocabulary::StructuralTypeId,
        identity: terminal_psi::TerminalIdentityIndex,
        shape: terminal_psi::StructuralTypeShape,
    ) -> terminal_psi::StructuralTypeDeclaration {
        terminal_psi::StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            scalar_restriction: None,
            id,
            identity,
            shape,
        }
    }
}
