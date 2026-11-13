/*
We define the sealed `ObjectArtifact` carrier, its function, attribution,
foreign-call, and byte-literal pool records, stack-demand derivation
(`derive_stack_demand`), installed interrupt stack composition
(`installed_stack`), normalized foreign-call provenance verification, and
boundary/structural replay checks.
*/

mod carriers {
    use crate::HostedReceiverBinding;
    use machine_code::{
        BoundarySettlementRecord, PortEffectRecord, ScalarControlAffineCleanupRecord,
        SemanticCodeAttribution, StructuralReturnRecord,
    };
    use object_file::{ObjectPlan, ObjectSymbolHandle, RelocationPlan};
    use semantic_vocabulary::MachineId;
    use target::NativeTarget;
    use target_operations::{CallSiteOwner, TerminalPsiProvenance};
    use terminal_psi::TerminalPsiIdentity;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ObjectArtifact {
        pub(crate) projected: crate::object::function_fragments::ProjectedObject,
        pub(crate) hosted_receiver: Option<HostedReceiverBinding>,
        pub(crate) physical_entry: Option<crate::PhysicalEntryBinding>,
        pub(crate) retained_entry_storage: ObjectSymbolHandle,
        pub(crate) requires_graph_storage_replay: bool,
        pub(crate) fragment_replay:
            Option<crate::object::function_fragments::replay::FragmentReplay>,
        pub(crate) composition_source: Option<std::sync::Arc<ObjectArtifact>>,
        pub(crate) psi: TerminalPsiIdentity,
        pub(crate) target: NativeTarget,
        pub(crate) x86_feature_profile: Option<target::TargetProfile>,
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
        pub(crate) code_descriptor_pool: Option<
            crate::object::function_fragments::code_descriptors::ObjectCodeDescriptorPool,
        >,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ObjectByteLiteralPool {
        pub symbol: ObjectSymbolHandle,
        pub text_offset: usize,
        pub byte_count: usize,
        pub addresses: Vec<ObjectByteLiteralAddress>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ObjectByteLiteralAddress {
        pub machine: MachineId,
        pub literal: machine_code::ByteLiteralReference,
        pub fields: machine_code::ImageAddressFields,
        pub pool_offset: usize,
        pub byte_count: usize,
    }

    impl ObjectArtifact {
        pub fn observe_invocation_input_sources(
            &self,
            caller: MachineId,
            site: target_operations::CallSiteOwner,
            callee: MachineId,
            origin: &terminal_semantics::descriptor_results::DescriptorSourceOrigin,
        ) -> Result<
            Vec<terminal_semantics::descriptor_results::DescriptorSourceOrigin>,
            crate::FunctionFragmentObjectArtifactError,
        > {
            let body = self.composition_source.as_deref().unwrap_or(self);
            let replay = body.fragment_replay.as_ref().ok_or(
                crate::FunctionFragmentObjectArtifactError::Mismatch(
                    "invocation input observation requires retained native source",
                ),
            )?;
            crate::object::function_fragments::invocation_input_sources(
                &replay.0, caller, site, callee, origin,
            )
        }

        pub fn native_instruction_source_rows(&self) -> Option<NativeInstructionSourceRows<'_>> {
            let source = self.composition_source.as_deref().unwrap_or(self);
            let current = source
                .fragment_replay
                .as_ref()?
                .0
                .source()
                .source()
                .source()
                .source();
            Some(NativeInstructionSourceRows {
                frame_application: source.fragment_replay.as_ref()?.0.source().source(),
                selected: current.selected_plan(),
                physical: current
                    .machine()
                    .machine()
                    .plan()
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
            self.fragment_replay
                .as_ref()?
                .0
                .source()
                .source()
                .source()
                .source()
                .optimized_target()
                .installed_entry_execution()
        }

        pub fn installation_environment_assumptions(
            &self,
        ) -> Vec<&installation_evidence::InstallationEnvironmentAssumption> {
            fn collect<'a>(
                object: &'a ObjectArtifact,
                rows: &mut Vec<&'a installation_evidence::InstallationEnvironmentAssumption>,
            ) {
                let source = object.composition_source.as_deref().unwrap_or(object);
                if let Some(replay) = &source.fragment_replay {
                    for scope in replay.0.lowering_facts().installed_interrupts().scopes() {
                        for assumption in scope.installation_assumptions() {
                            if !rows.contains(&assumption) {
                                rows.push(assumption);
                            }
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

        pub const fn hosted_receiver_binding(&self) -> Option<&HostedReceiverBinding> {
            self.hosted_receiver.as_ref()
        }
        pub const fn psi(&self) -> TerminalPsiIdentity {
            self.psi
        }

        pub const fn target(&self) -> NativeTarget {
            self.target
        }

        pub const fn x86_feature_profile(&self) -> Option<target::TargetProfile> {
            self.x86_feature_profile
        }

        pub fn verify_private_entry_origin(
            &self,
            candidate: &ObjectCompilerPrivateFunction,
        ) -> Result<(), crate::FunctionFragmentObjectArtifactError> {
            crate::entries::private_entry_origin::EntryOrigins::new(self)?.verify(candidate)
        }

        pub const fn entry(&self) -> MachineId {
            self.entry
        }

        pub fn validate_terminal_entry(
            &self,
            artifact: &terminal_codec::CanonicalTerminalArtifact,
        ) -> Result<(), &'static str> {
            let identity = artifact.view().identity();
            let module = artifact.semantic_module();
            if self.psi != identity || module.machine(self.entry).is_none() {
                return Err("native object entry has no exact Terminal body or identity");
            }
            let installed_entry = self
                .fragment_replay
                .as_ref()
                .map(|replay| replay.selected_terminal_entry(artifact, self.entry, self.target))
                .transpose()?
                .flatten();
            let selected_entry = if let Some(entry) = installed_entry {
                entry
            } else {
                match module.entry {
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

        pub fn borrowed_storage_layouts(
            &self,
        ) -> Option<&[runtime_abi::NativeBorrowedStorageLayout]> {
            self.fragment_replay.as_ref().map(|replay| {
                replay
                    .0
                    .source()
                    .source()
                    .source()
                    .source()
                    .optimized_target()
                    .borrowed_storage_layouts()
            })
        }

        pub fn private_functions(&self) -> &[ObjectCompilerPrivateFunction] {
            &self.private_functions
        }

        pub fn terminal_module(&self) -> Option<&terminal_psi::TerminalModule> {
            self.fragment_replay
                .as_ref()
                .map(|replay| replay.terminal_module())
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
        pub frame_application: &'a machine_emission::StagedFunctionFragmentFrameApplication,
        pub selected: &'a selected_instructions::SelectedInstructionPlan,
        pub physical: Vec<(MachineId, selected_instructions::SelectedInstructionId)>,
    }

    impl installation_evidence::ObjectEvidence for ObjectArtifact {
        fn psi(&self) -> TerminalPsiIdentity {
            self.psi
        }

        fn target(&self) -> NativeTarget {
            self.target
        }

        fn text_bytes(&self) -> &[u8] {
            &self.text_bytes
        }

        fn function_text_offset(&self, machine: MachineId) -> Option<usize> {
            self.function_on_machine(machine)
                .map(|function| function.text_offset)
        }
    }

    pub(crate) fn find_object_function(
        functions: &[ObjectFunction],
        machine: MachineId,
    ) -> Option<&ObjectFunction> {
        functions
            .iter()
            .find(|function| function.machine == machine)
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ObjectFunction {
        pub machine: MachineId,
        pub attachment: Option<semantic_vocabulary::StructuralTypeId>,
        pub nominal_cleanup_subject: Option<semantic_vocabulary::StructuralTypeId>,
        pub scalar_abi: Option<target_operations::ScalarFunctionAbi>,
        pub mixed_structural_scalar_abi:
            Option<target_operations::MixedStructuralScalarFunctionAbi>,
        pub structural_call_scalar_return: Option<machine_code::StructuralCallScalarReturnEvidence>,
        pub parameter_abi: Option<machine_code::ParameterFunctionAbiRecord>,
        pub provenance: TerminalPsiProvenance,
        pub symbol: ObjectSymbolHandle,
        pub text_offset: usize,
        pub byte_count: usize,
        pub x86_floating_control: Option<machine_code::X86FloatingControlRecord>,
        pub unit_stack: Option<ObjectUnitStack>,
        pub scalar_stack: Option<ObjectScalarStack>,
        pub unit_call_stacks: Vec<ObjectUnitCallStack>,
        pub scalar_call_stacks: Vec<ObjectScalarCallStack>,
        pub internal_unit_calls: Vec<machine_code::InternalUnitCallRecord>,
        pub internal_unit_scalar_calls: Vec<machine_code::InternalUnitScalarCallRecord>,
        pub installed_provider_unit_scalar_calls:
            Vec<machine_code::InstalledProviderUnitScalarCallRecord>,
        pub unit_scalar_homes: Vec<machine_code::UnitScalarHomeRecord>,
        pub unit_integer_constants: Vec<machine_code::UnitIntegerConstantRecord>,
        pub unit_affine_scalar_records:
            Vec<machine_code::UnitAffineScalarRecordEstablishmentRecord>,
        pub unit_structural_scalar_field_stores:
            Vec<machine_code::UnitStructuralScalarFieldStoreRecord>,
        pub unit_write_only_primitive_stores: Vec<machine_code::UnitWriteOnlyPrimitiveStoreRecord>,
        pub scalar_structural_scalar_field_stores:
            Vec<machine_code::ScalarStructuralScalarFieldStoreRecord>,
        pub unit_parameters: Vec<machine_code::UnitParameterRecord>,
        pub unit_parameter_homes: Vec<machine_code::UnitParameterHomeRecord>,
        pub unit_continuations: Vec<machine_code::UnitContinuationRecord>,
        pub unit_affine_cleanup: Option<machine_code::UnitAffineCleanupRecord>,
        pub scalar_affine_cleanup: Option<machine_code::UnitAffineCleanupRecord>,
        pub scalar_control_affine_cleanups: Vec<ScalarControlAffineCleanupRecord>,
        pub scalar_structural_parameters: Vec<machine_code::UnitParameterRecord>,
        pub scalar_structural_parameter_homes: Vec<machine_code::UnitParameterHomeRecord>,
        pub structural_return: Option<StructuralReturnRecord>,
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
        pub entry_bridge: Option<machine_emission::native_entry_bridge::NativeEntryBridge>,
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
        pub entry_bridge: Option<machine_emission::native_entry_bridge::NativeEntryBridge>,
        pub entry_effects: calling_conventions::StateEffectsEvidence,
        pub inbound: calling_conventions::BoundaryEntryPlan,
        pub inbound_ordinals: Vec<u32>,
    }

    impl ObjectCompilerPrivateFunction {
        pub fn verify_entry_transport(
            &self,
        ) -> Result<(), crate::FunctionFragmentObjectArtifactError> {
            crate::entries::callable_entry_transport::validate_entry_transport(self)
        }

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
    pub struct ObjectForeignCall {
        pub machine: MachineId,
        pub owner: CallSiteOwner,
        pub operation_ordinal: usize,
        pub binding: target_operations::NormalizedForeignCallBinding,
        pub provider_execution: machine_code::ProviderExecutionRecord,
        pub caller_live_bytes: u32,
        pub scalar_arguments: Vec<machine_code::ForeignCallScalarArgumentRecord>,
        pub callback_addresses: Vec<machine_code::CallbackAddressMaterialization>,
        pub scalar_result: Option<machine_code::ForeignCallScalarResultRecord>,
        pub x86_floating_control: Option<machine_code::ForeignCallFloatingControlRecord>,
        pub aarch64_floating_control: Option<machine_code::ForeignCallFloatingControlRecord>,
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

mod errors {
    use semantic_vocabulary::MachineId;
    use target_operations::CallSiteOwner;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ObjectError {
        EmptyPlan,
        NonCanonicalFunctionOrder {
            previous: MachineId,
            current: MachineId,
        },
        EmptyFunction(MachineId),
        InvalidPrivateFunctionIdentity,
        EmptyPrivateFunctionSymbol,
        PrivateFunctionSymbolCollision,
        InvalidPrivateFunctionAbi,
        InvalidPrivateFunctionBody,
        UnsupportedPrivateFunctionBody,
        NonCanonicalInternalCallOrder(MachineId),
        NonCanonicalForeignCallOrder(MachineId),
        NonCanonicalSemanticCodeAttributionOrder(MachineId),
        SemanticCodeAttributionOutsideFunction(MachineId),
        InvalidSemanticCodeAttribution(MachineId),
        NonCanonicalPortEffectOrder(MachineId),
        NonCanonicalBoundarySettlementOrder(MachineId),
        InvalidStructuralReturnEvidence(MachineId),
        StructuralReturnEvidenceConflict(MachineId),
        StructuralReturnBytesMismatch(MachineId),
        UnknownInternalCallTarget {
            caller: MachineId,
            target: MachineId,
        },
        InvalidInternalCallSite {
            caller: MachineId,
            owner: CallSiteOwner,
            offset: usize,
        },
        InvalidForeignCallSite {
            caller: MachineId,
            owner: CallSiteOwner,
            offset: usize,
        },
        InvalidForeignCallProjection {
            caller: MachineId,
            owner: CallSiteOwner,
            reason: &'static str,
        },
        InvalidForeignCallArgument {
            caller: MachineId,
            owner: CallSiteOwner,
            import: Option<String>,
            parameter: Option<u32>,
            reason: &'static str,
            transport: Option<String>,
        },
        InvalidCallbackAddressCustody {
            caller: MachineId,
            owner: CallSiteOwner,
        },
        MissingCallbackPrivateFunction {
            caller: MachineId,
            owner: CallSiteOwner,
        },
        InvalidForeignCallFloatingControl {
            caller: MachineId,
            owner: CallSiteOwner,
        },
        ForeignCallOwnerNotInProvenance {
            caller: MachineId,
            owner: CallSiteOwner,
        },
        DuplicateForeignCallOwner {
            caller: MachineId,
            owner: CallSiteOwner,
        },
        ForeignCallTargetMismatch {
            caller: MachineId,
            owner: CallSiteOwner,
        },
        ForeignStackProviderPlanMismatch {
            caller: MachineId,
            owner: CallSiteOwner,
        },
        UnsupportedForeignStackAlignment {
            caller: MachineId,
            owner: CallSiteOwner,
            admitted_alignment: u64,
            physical_alignment: u32,
        },
        ForeignCallOverlapsInternalCall {
            caller: MachineId,
            offset: usize,
        },
        ForeignLocatorIdentityCollision {
            caller: MachineId,
            owner: CallSiteOwner,
        },
        MissingForeignImportSymbol {
            caller: MachineId,
            owner: CallSiteOwner,
        },
        InvalidInternalUnitCallEvidence(MachineId),
        InvalidInternalUnitScalarCallEvidence(MachineId),
        InvalidUnitScalarFunctionAbi(MachineId),
        InvalidInstalledProviderUnitScalarCallEvidence(MachineId),
        InvalidUnitWriteOnlyPrimitiveStoreEvidence(MachineId),
        InvalidUnitStructuralScalarFieldStoreEvidence(MachineId),
        InvalidScalarStructuralScalarFieldStoreEvidence(MachineId),
        InvalidUnitAffineCleanupEvidence(MachineId),
        InternalCallOperationNotInProvenance {
            caller: MachineId,
            owner: CallSiteOwner,
        },
        DuplicateInternalCallOperation {
            caller: MachineId,
            owner: CallSiteOwner,
        },
        MissingUnitCallStackEvidence {
            caller: MachineId,
            owner: CallSiteOwner,
        },
        UnexpectedUnitCallStackEvidence {
            caller: MachineId,
            owner: CallSiteOwner,
        },
        MissingScalarCallStackEvidence {
            caller: MachineId,
            owner: CallSiteOwner,
        },
        UnexpectedScalarCallStackEvidence {
            caller: MachineId,
            owner: CallSiteOwner,
        },
        InvalidUnitStackAlignment {
            machine: MachineId,
            alignment: u32,
        },
        ConflictingTerminalStackEvidence(MachineId),
        InvalidScalarStackAlignment {
            machine: MachineId,
            alignment: u32,
        },
        InvalidScalarConditionalEvidence {
            machine: MachineId,
            offset: usize,
        },
        ScalarConditionalCallOutsideArm {
            machine: MachineId,
            operation: semantic_vocabulary::OperationId,
            offset: usize,
        },
        UntypedScalarInternalCall {
            machine: MachineId,
            offset: usize,
        },
        InvalidScalarCallStackEvidence {
            caller: MachineId,
            owner: CallSiteOwner,
            offset: usize,
        },
        MisalignedScalarCalleeEntry {
            caller: MachineId,
            owner: CallSiteOwner,
            caller_live_bytes: u32,
        },
        NonCanonicalScalarStackMutationOrder(MachineId),
        InvalidScalarInstructionEncoding {
            machine: MachineId,
            offset: usize,
        },
        NonLinearScalarControlFlow {
            machine: MachineId,
            offset: usize,
        },
        UnclaimedScalarStackMutation {
            machine: MachineId,
            offset: usize,
        },
        UnsupportedScalarStackMutation {
            machine: MachineId,
            offset: usize,
        },
        InvalidScalarStackEvidence {
            machine: MachineId,
            offset: usize,
        },
        MissingBalancedScalarReturn(MachineId),
        ScalarStackArithmeticOverflow(MachineId),
        ScalarStackReleaseExceedsAllocation {
            machine: MachineId,
            offset: usize,
        },
        UnitCallStackArithmeticOverflow {
            caller: MachineId,
            owner: CallSiteOwner,
        },
        InvalidUnitStackEncoding {
            machine: MachineId,
            owner: Option<CallSiteOwner>,
            offset: usize,
        },
        InvalidUnitInstructionEncoding {
            machine: MachineId,
            offset: usize,
        },
        DuplicateUnitStackAdjustment(MachineId),
        UnclaimedUnitStackAdjustment {
            machine: MachineId,
            offset: usize,
        },
        UnclaimedUnitStackMutation {
            machine: MachineId,
            offset: usize,
        },
        MisalignedUnitCalleeEntry {
            caller: MachineId,
            owner: CallSiteOwner,
            caller_live_bytes: u32,
        },
        MissingX86UnitCallStackAdjustment {
            caller: MachineId,
            owner: CallSiteOwner,
        },
        MissingAarch64UnitReturnLink {
            caller: MachineId,
            operation: Option<semantic_vocabulary::OperationId>,
        },
        UnaccountedTerminalStack(MachineId),
        TerminalStackCycle(MachineId),
        PrivateEntryStackArithmeticOverflow(function_identity::MachineFunctionIdentity),
        PhysicalEntryStackArithmeticOverflow(MachineId),
        TerminalStackCompositionOverflow {
            caller: MachineId,
            owner: CallSiteOwner,
        },
        PortEffectOutsideFunction {
            machine: MachineId,
            operation: semantic_vocabulary::OperationId,
        },
        PortEffectOperationNotInProvenance {
            machine: MachineId,
            operation: semantic_vocabulary::OperationId,
        },
        DuplicatePortEffectOperation {
            machine: MachineId,
            operation: semantic_vocabulary::OperationId,
        },
        PortEffectBytesMismatch {
            machine: MachineId,
            operation: semantic_vocabulary::OperationId,
        },
        BoundarySettlementOutsideFunction {
            machine: MachineId,
            operation: semantic_vocabulary::InvocationSite,
        },
        BoundarySettlementOperationNotInProvenance {
            machine: MachineId,
            operation: semantic_vocabulary::InvocationSite,
        },
        DuplicateBoundarySettlementOperation {
            machine: MachineId,
            operation: semantic_vocabulary::InvocationSite,
        },
        InvalidBoundarySettlementArgumentPath {
            machine: MachineId,
            operation: semantic_vocabulary::InvocationSite,
        },
        InvalidCompletionReceiptArgumentIndex {
            machine: MachineId,
            operation: semantic_vocabulary::InvocationSite,
        },
        InvalidCompletionReceiptCustody {
            machine: MachineId,
            operation: semantic_vocabulary::InvocationSite,
        },
        InvalidCompletionProviderCustody {
            machine: MachineId,
            operation: semantic_vocabulary::InvocationSite,
        },
        BoundaryRealizationMismatch {
            machine: MachineId,
            operation: semantic_vocabulary::InvocationSite,
        },
        EntryFunctionMissing(MachineId),
        TextSizeOverflow,
    }

    diagnostics::debug_display!(ObjectError);

    impl std::error::Error for ObjectError {}
}

pub(crate) mod installed_stack {
    use crate::{FunctionFragmentObjectArtifactError as Error, PrivateFunctionObject};
    use installation_evidence::InstalledInterruptStackRequirement;
    mod arrival {
        use crate::FunctionFragmentObjectArtifactError as Error;
        use calling_conventions::EntryStack;
        use installation_evidence::InstalledPhysicalStackRoot;
        use installation_evidence::{
            InstalledArrivalStackDemand, InstalledInterruptStackRequirement,
        };
        use provider_admission::{
            AdmittedInstalledInterruptEntry, AdmittedInstalledInterruptScope,
        };

        struct Entry {
            placement: usize,
            vector: u16,
            domain: Option<u16>,
            bytes: u64,
            alignment: u32,
        }

        pub fn compose(
            scope: &AdmittedInstalledInterruptScope,
            entries: &[&AdmittedInstalledInterruptEntry],
            requirements: &mut [InstalledInterruptStackRequirement],
            physical_entry: Option<InstalledPhysicalStackRoot>,
        ) -> Result<(), Error> {
            if scope.fatal_entries().is_empty() {
                if requirements.iter().any(|row| row.arrival.is_some()) {
                    return Err(Error::Mismatch(
                        "single timer demand has an unrelated arrival composition",
                    ));
                }
                return Ok(());
            }
            let delivery = scope
                .interrupt_delivery_assumption()
                .ok_or(Error::Mismatch(
                    "fatal arrival composition lacks its explicit delivery condition",
                ))?;
            if entries.len() != scope.fatal_entries().len() + 1
                || requirements.len() != entries.len()
            {
                return Err(Error::Mismatch(
                    "fatal arrival composition lost part of its entry roster",
                ));
            }
            let anchor = scope.context_anchor().ok_or(Error::Mismatch(
                "arrival composition has no exact timer anchor",
            ))?;
            let mut inputs = Vec::with_capacity(entries.len());
            for entry in entries {
                let mut matching = requirements
                    .iter()
                    .filter(|row| row.placement_index == entry.placement_index);
                let Some(row) = matching.next() else {
                    return Err(Error::Mismatch("arrival entry has no measured body"));
                };
                if matching.next().is_some()
                    || row.operation != scope.operation()
                    || entry.operation != scope.operation()
                    || row.psi != entry.entry().psi()
                    || row.target != entry.entry().target()
                    || row.entry != entry.entry().machine()
                    || row.fault_exclusion.is_none()
                    || delivery.psi != row.psi
                    || delivery.installation != row.operation
                {
                    return Err(Error::Mismatch(
                        "arrival entry changed its body, installation or fault premises",
                    ));
                }
                let gate =
                    if entry.placement.application_index == scope.acknowledgement_application() {
                        anchor.gate_byte_offset()
                    } else {
                        let mut matching = scope.fatal_entries().iter().filter(|fatal| {
                            fatal.application() == entry.placement.application_index
                                && fatal.machine() == row.entry
                        });
                        let Some(fatal) = matching.next() else {
                            return Err(Error::Mismatch(
                                "arrival entry has no admitted fatal identity",
                            ));
                        };
                        if matching.next().is_some() {
                            return Err(Error::Mismatch("fatal arrival identity is duplicated"));
                        }
                        fatal.gate_byte_offset()
                    };
                if gate % 16 != 0 || gate / 16 > 255 {
                    return Err(Error::Mismatch(
                        "arrival vector is outside its exact x86 gate",
                    ));
                }
                let domain = match entry.entry().inbound().state.stack {
                    EntryStack::Interrupted => None,
                    EntryStack::Dedicated { class } => Some(class),
                    EntryStack::ProviderSelected => {
                        return Err(Error::Mismatch("arrival stack domain is unresolved"));
                    }
                };
                inputs.push(Entry {
                    placement: row.placement_index,
                    vector: (gate / 16) as u16,
                    domain,
                    bytes: row.own_bytes().ok_or(Error::Overflow)?,
                    alignment: row.alignment,
                });
            }
            inputs.sort_unstable_by_key(|entry| entry.placement);
            if inputs
                .windows(2)
                .any(|pair| pair[0].placement == pair[1].placement)
            {
                return Err(Error::Mismatch("arrival placement is duplicated"));
            }
            let mut vectors: Vec<_> = inputs.iter().map(|entry| entry.vector).collect();
            vectors.sort_unstable();
            if vectors.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(Error::Mismatch("arrival vector is duplicated"));
            }
            if !(0..32).all(|vector| vectors.binary_search(&vector).is_ok()) {
                return Err(Error::Mismatch(
                    "fatal installation does not cover every architectural exception destination",
                ));
            }
            let physical_entry = physical_entry
                .filter(|root| root.semantic_entry == delivery.entry)
                .ok_or(Error::Mismatch(
                    "fatal arrival has no matching physical entry stack provision",
                ))?;
            let mut peaks = vec![0; inputs.len()];
            let mut simultaneous = vec![Vec::new(); inputs.len()];
            let mut root_peak = 0;
            for first in 0..inputs.len() {
                let mut history = vec![first];
                histories(
                    &inputs,
                    &mut history,
                    &mut peaks,
                    &mut simultaneous,
                    &mut root_peak,
                )?;
            }
            for row in requirements {
                let index = inputs
                    .binary_search_by_key(&row.placement_index, |entry| entry.placement)
                    .map_err(|_| {
                        Error::Mismatch("arrival requirement is outside its complete roster")
                    })?;
                simultaneous[index].sort_unstable();
                simultaneous[index].dedup();
                row.arrival = Some(InstalledArrivalStackDemand {
                    physical_entry,
                    root_reserve_bytes: root_peak,
                    domain_bytes: peaks[index],
                    simultaneous_placements: std::mem::take(&mut simultaneous[index]),
                });
            }
            Ok(())
        }

        fn histories(
            entries: &[Entry],
            history: &mut Vec<usize>,
            peaks: &mut [u64],
            simultaneous: &mut [Vec<usize>],
            root_peak: &mut u64,
        ) -> Result<(), Error> {
            let mut active = None;
            let mut occupancy: Vec<(Option<u16>, u64)> = vec![(None, 0)];
            for &index in history.iter() {
                let entry = &entries[index];
                if let Some(domain) = entry.domain {
                    active = Some(domain);
                }
                let position = occupancy
                    .iter()
                    .position(|(domain, _)| *domain == active)
                    .unwrap_or_else(|| {
                        occupancy.push((active, 0));
                        occupancy.len() - 1
                    });
                let alignment = u64::from(entry.alignment);
                if alignment == 0 || !alignment.is_power_of_two() {
                    return Err(Error::Mismatch("arrival alignment is invalid"));
                }
                let bytes = occupancy[position]
                    .1
                    .checked_add(alignment - 1)
                    .map(|bytes| bytes & !(alignment - 1))
                    .and_then(|bytes| bytes.checked_add(entry.bytes))
                    .ok_or(Error::Overflow)?;
                occupancy[position].1 = bytes;
            }
            for (domain, bytes) in occupancy {
                if domain.is_none() {
                    *root_peak = (*root_peak).max(bytes);
                }
                for (index, entry) in entries.iter().enumerate() {
                    if entry.domain == domain {
                        peaks[index] = peaks[index].max(bytes);
                    }
                }
            }
            for (position, &left) in history.iter().enumerate() {
                if entries[left].domain.is_none() {
                    continue;
                }
                for &right in &history[position + 1..] {
                    if entries[right].domain.is_none() {
                        continue;
                    }
                    if entries[left].domain == entries[right].domain {
                        return Err(Error::Mismatch(
                            "simultaneous arrivals reload one dedicated stack top",
                        ));
                    }
                    simultaneous[left].push(entries[right].placement);
                    simultaneous[right].push(entries[left].placement);
                }
            }
            for vector in [2, 18] {
                if history.iter().any(|&index| entries[index].vector == vector) {
                    continue;
                }
                if let Some(next) = entries.iter().position(|entry| entry.vector == vector) {
                    history.push(next);
                    histories(entries, history, peaks, simultaneous, root_peak)?;
                    history.pop();
                }
            }
            Ok(())
        }
    }

    mod faults {
        use crate::{FunctionFragmentObjectArtifactError as Error, ObjectArtifact};
        use installation_evidence::InstalledBodyFaultPremises;
        use selected_instructions::{
            MachineEncodedTrapBehavior as Trap, SelectedInstructionKind as Kind,
        };
        use selected_instructions::{
            SelectedInstruction, SelectedInstructionId, SelectedInstructionPlan,
        };
        use semantic_vocabulary::MachineId;
        mod ram {
            use crate::FunctionFragmentObjectArtifactError as Error;
            use abstract_operations::{AbstractOperation as O, AbstractOperationPlan};
            use provider_admission::{
                AdmittedInstalledEntryExecution, AdmittedInstalledInterruptCohort,
            };
            use selected_instructions::SelectedInstructionPlan;
            use semantic_vocabulary::MachineId;
            use target_operations::{CallSiteOwner, NativeCallTarget};

            pub(super) fn validate(
                source: &AbstractOperationPlan,
                selected: &SelectedInstructionPlan,
                cohort: &AdmittedInstalledInterruptCohort,
                execution: &AdmittedInstalledEntryExecution,
                machines: &[MachineId],
            ) -> Result<(), Error> {
                if source.psi != cohort.psi() || source.entry != execution.entry().entry().machine()
                {
                    return Err(Error::Mismatch(
                        "installed RAM closure changed its admitted entry",
                    ));
                }
                let mut bodies: Vec<_> = source.functions.iter().collect();
                bodies.sort_unstable_by_key(|body| body.machine);
                if bodies
                    .windows(2)
                    .any(|pair| pair[0].machine == pair[1].machine)
                {
                    return Err(Error::Mismatch("installed RAM source body is duplicated"));
                }
                let mut helpers: Vec<_> = cohort
                    .helpers()
                    .iter()
                    .filter(|helper| {
                        helper.installation() == execution.scope().operation()
                            && helper.callback_application()
                                == execution.entry().placement.application_index
                    })
                    .collect();
                helpers.sort_unstable_by_key(|helper| helper.operation());
                if helpers
                    .windows(2)
                    .any(|pair| pair[0].operation() == pair[1].operation())
                {
                    return Err(Error::Mismatch(
                        "installed RAM helper identity is duplicated",
                    ));
                }
                for function in &selected.functions {
                    let body = bodies
                        .binary_search_by_key(&function.machine, |body| body.machine)
                        .ok()
                        .map(|index| bodies[index])
                        .ok_or(Error::Mismatch(
                            "installed RAM closure has no exact original body",
                        ))?;
                    let mut calls: Vec<_> = function.calls.iter().collect();
                    calls.sort_unstable_by_key(|call| call.owner);
                    for operation in &body.operations {
                        let fail = |reason| Error::InstalledMemorySource {
                            machine: body.machine,
                            operation: operation.psi_operation(),
                            reason,
                        };
                        let owner = match operation {
                            O::EstablishAccessReference { .. } => {
                                return Err(fail(
                                    "external placed backing needs its own mapped-residence proof",
                                ));
                            }
                            O::BoundaryCall { psi_operation, .. } => {
                                if let Ok(index) = helpers
                                    .binary_search_by_key(psi_operation, |helper| {
                                        helper.operation()
                                    })
                                {
                                    match helpers[index].kind() {
                                        language_core::static_interrupts::StaticInterruptOperation::Context
                                        | language_core::static_interrupts::StaticInterruptOperation::BeginCompletion
                                        | language_core::static_interrupts::StaticInterruptOperation::FinishCompletion => continue,
                                        language_core::static_interrupts::StaticInterruptOperation::Install => {
                                            return Err(fail("nested installation is not a closed Context source"));
                                        }
                                    }
                                }
                                CallSiteOwner::Operation(*psi_operation)
                            }
                            O::ExternalCompletion { psi_edge, .. } => {
                                CallSiteOwner::ExternalCompletion(*psi_edge)
                            }
                            _ => continue,
                        };
                        let first = calls.partition_point(|call| call.owner < owner);
                        let last = calls.partition_point(|call| call.owner <= owner);
                        let [call] = &calls[first..last] else {
                            return Err(fail(
                                "boundary has no unique concrete RAM-preserving body call",
                            ));
                        };
                        if !matches!(call.call.target, NativeCallTarget::Machine(machine)
                            if machines.binary_search(&machine).is_ok())
                        {
                            return Err(fail(
                                "boundary may introduce backing outside the installed body closure",
                            ));
                        }
                    }
                }
                Ok(())
            }
        }

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
            let replay = body.fragment_replay.as_ref().ok_or(Error::Mismatch(
                "installed fault analysis requires the exact retained native pipeline",
            ))?;
            let current = replay.0.source().source().source().source();
            let selected = current.selected_plan();
            let physical = current.machine().machine().plan();
            if selected.psi != body.psi()
                || selected.entry != body.entry()
                || physical.target != body.target()
            {
                return Err(Error::Mismatch(
                    "installed fault analysis changed its body or target",
                ));
            }
            let correlated = correlate_native_instruction_sources(
                selected,
                physical.functions.iter().flat_map(|function| {
                    function
                        .instructions()
                        .map(move |instruction| (function.machine, instruction.instruction))
                }),
            )?;
            let mut machines: Vec<_> = selected
                .functions
                .iter()
                .map(|function| function.machine)
                .collect();
            machines.sort_unstable();
            let mut physical_machines: Vec<_> = physical
                .functions
                .iter()
                .map(|function| function.machine)
                .collect();
            physical_machines.sort_unstable();
            if machines != physical_machines || machines.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(Error::Mismatch(
                    "installed fault analysis changed its exact function roster",
                ));
            }
            ram::validate(
                current.optimized_target().optimized().plan(),
                selected,
                current
                    .optimized_target()
                    .lowering_facts()
                    .installed_interrupts(),
                execution,
                &machines,
            )?;
            crate::observe_native_body_cpu_state(
                body,
                execution.entry().entry().inbound().state.preemption,
            )?
            .require_without_floating_state()?;
            let mut correlated = correlated.into_iter();
            let framed = replay.0.source().source();
            for function in &physical.functions {
                let fragment =
                    framed
                        .fragments()
                        .function(function.machine)
                        .ok_or(Error::Mismatch(
                            "installed fault analysis has no exact framed function",
                        ))?;
                let relative_branches = framed
                    .validate_relative_branch_rows(
                        function.machine,
                        fragment.instructions().filter(|row| row.branch.is_some()),
                    )
                    .map_err(|error| Error::InstalledRelativeBranches {
                        machine: function.machine,
                        source: error,
                    })?;
                for instruction in function.instructions() {
                    let (machine, source) = correlated.next().ok_or(Error::Mismatch(
                        "installed physical instruction has no exact selected source",
                    ))?;
                    let fail = |reason: &str| Error::InstalledFaultSurface {
                        machine,
                        instruction: instruction.instruction,
                        detail: format!(
                            "{reason}; selected={:?}, encoded={:?}",
                            source.kind, instruction.alternative.encoded
                        ),
                    };
                    if machine != function.machine {
                        return Err(fail("physical instruction changed its owning machine"));
                    }
                    if let Some(target) = source.kind.call_target() {
                        let Some(target) = target.machine() else {
                            return Err(fail("dynamic call has no closed fault analysis"));
                        };
                        if machines.binary_search(&target).is_err() {
                            return Err(fail("callee is outside the exact retained body closure"));
                        }
                    }
                    let effects = &instruction.alternative.encoded;
                    match effects.trap {
                        Trap::NeverV1 => {}
                        Trap::MayArchitecturalFaultV1 => {
                            let memory = matches!(
                                source.kind,
                                Kind::CopyBytes
                                    | Kind::ZeroBytes
                                    | Kind::LoadEncodedScalar
                                    | Kind::StoreEncodedScalar
                                    | Kind::ScanByteSequence { .. }
                                    | Kind::CompareByteSequences
                                    | Kind::LoadPacked { .. }
                                    | Kind::StorePacked { .. }
                                    | Kind::Store { .. }
                                    | Kind::Load8Indexed
                                    | Kind::Load8 { .. }
                                    | Kind::Load16 { .. }
                                    | Kind::Load32 { .. }
                                    | Kind::Load64 { .. }
                                    | Kind::Store64 { .. }
                                    | Kind::AtomicAccess { .. }
                                    | Kind::ReturnUnit
                                    | Kind::ReturnScalar
                                    | Kind::ReturnAggregate { .. }
                            );
                            let privileged = match source.kind {
                                Kind::PrivilegedCrash | Kind::PortWrite { .. } => true,
                                Kind::RegisterAccess { operation } => matches!(operation,
                                    terminal_psi::RegisterOperation::ProcessorHalt
                                    | terminal_psi::RegisterOperation::InterruptControl(language_core::inline_assembly::AsmInterruptControlKind::Disable)
                                    | terminal_psi::RegisterOperation::MemoryFence(_)
                                    | terminal_psi::RegisterOperation::PortWrite),
                                _ => false,
                            };
                            let relative = relative_branches
                                .binary_search(&instruction.instruction)
                                .is_ok();
                            if !memory
                                && !privileged
                                && !relative
                                && source.kind.call_target().is_none()
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
            if correlated.next().is_some() {
                return Err(Error::Mismatch(
                    "installed physical closure omits a selected instruction",
                ));
            }
            Ok(Some(InstalledBodyFaultPremises {
                physical_program_identity: physical.identity.bytes(),
            }))
        }

        pub fn correlate_native_instruction_sources(
            selected: &SelectedInstructionPlan,
            physical: impl IntoIterator<Item = (MachineId, SelectedInstructionId)>,
        ) -> Result<Vec<(MachineId, &SelectedInstruction)>, Error> {
            let mut instructions: Vec<_> = selected
                .functions
                .iter()
                .flat_map(|function| {
                    function.blocks.iter().flat_map(move |block| {
                        block
                            .instructions
                            .iter()
                            .chain(std::iter::once(
                                selected_instructions::terminator_instruction(&block.terminator),
                            ))
                            .map(move |instruction| {
                                ((function.machine, instruction.id), instruction)
                            })
                    })
                })
                .collect();
            instructions.sort_unstable_by_key(|row| row.0);
            if instructions.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                return Err(Error::Mismatch(
                    "installed fault analysis has duplicate selected instructions",
                ));
            }
            let mut seen = vec![false; instructions.len()];
            let mut correlated = Vec::with_capacity(instructions.len());
            for owner in physical {
                let index = instructions
                    .binary_search_by_key(&owner, |row| row.0)
                    .map_err(|_| {
                        Error::Mismatch(
                            "installed physical instruction has no exact selected source",
                        )
                    })?;
                if std::mem::replace(&mut seen[index], true) {
                    return Err(Error::Mismatch(
                        "installed physical instruction is duplicated",
                    ));
                }
                let ((machine, _), source) = instructions[index];
                correlated.push((machine, source));
            }
            if seen.iter().any(|present| !present) {
                return Err(Error::Mismatch(
                    "installed physical closure omits a selected instruction",
                ));
            }
            Ok(correlated)
        }
    }

    pub use arrival::compose as compose_installed_arrival_demands;
    pub use faults::correlate_native_instruction_sources;

    pub fn installed_interrupt_stack_requirement(
        private: &PrivateFunctionObject,
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
            body_bytes: body.ceiling_bytes(),
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
            let Some(replay) = &body.fragment_replay else {
                continue;
            };
            let target = replay
                .0
                .source()
                .source()
                .source()
                .source()
                .optimized_target();
            let mut actual_requirements = Vec::new();
            for expected in target.installed_stack_requirements() {
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
                    expected.operation,
                )?);
            }
            for scope in target.lowering_facts().installed_interrupts().scopes() {
                let first =
                    actual_requirements.partition_point(|row| row.operation < scope.operation());
                let last =
                    actual_requirements.partition_point(|row| row.operation <= scope.operation());
                if first == last {
                    continue;
                }
                let entries: Vec<_> = target
                    .lowering_facts()
                    .installed_interrupts()
                    .entries()
                    .iter()
                    .filter(|entry| entry.operation == scope.operation())
                    .collect();
                let proposed_root = target.installed_stack_requirements()[first]
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
                compose_installed_arrival_demands(
                    scope,
                    &entries,
                    &mut actual_requirements[first..last],
                    proposed_root,
                )?;
            }
            if actual_requirements != target.installed_stack_requirements() {
                return Err(Error::Mismatch(
                    "installed stack guard differs from final body and complete arrival demand",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn root_reserve(
        artifact: &crate::ObjectArtifact,
        root: installation_evidence::InstalledPhysicalStackRoot,
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
            let Some(replay) = &body.fragment_replay else {
                continue;
            };
            let target = replay
                .0
                .source()
                .source()
                .source()
                .source()
                .optimized_target();
            for requirement in target.installed_stack_requirements() {
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
            .filter_map(|body| body.fragment_replay.as_ref())
            .any(|replay| {
                replay
                    .0
                    .source()
                    .source()
                    .source()
                    .source()
                    .optimized_target()
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

mod normalized_foreign_calls {
    mod descriptor_input {
        use selected_instructions::{
            SelectedBlock, SelectedFunction, SelectedInstructionKind as Kind,
            SelectedNormalizedForeignCall, VirtualRegisterOrigin,
        };
        use semantic_vocabulary::{IntegerValue, ScalarType};

        pub(super) fn validate(
            function: &SelectedFunction,
            block: &SelectedBlock,
            record: &SelectedNormalizedForeignCall,
        ) -> Option<()> {
            let Some(guard) = record.call.descriptor_input else {
                return Some(());
            };
            let operation = record.invocation_site.operation()?;
            let count = record
                .call
                .scalar_arguments
                .get(guard.count_scalar_argument)?
                .source
                .source_value();
            let call_position = block.instruction_index(record.instruction)?;
            let form = legalized_operations::TrappingForm {
                operation: legalized_operations::TrappingOperation::Subtract,
                carrier: legalized_operations::SaturatingCarrier::from_integer(guard.count_type)?,
            };
            let mut guards = block.instructions[..call_position]
                .iter()
                .enumerate()
                .filter(|(_, instruction)| {
                    instruction.kind == (Kind::TrappingInteger { form })
                        && instruction.provenance.operations.as_slice() == [operation]
                        && instruction.provenance.values.as_slice() == [count]
                });
            let (guard_position, instruction) = guards.next()?;
            if guards.next().is_some() || !instruction.provenance.work.is_empty() {
                return None;
            }
            let [bound, input, output, scratch] = instruction.operands.as_slice() else {
                return None;
            };
            let input = function
                .virtual_registers
                .get(input.virtual_register.0 as usize)?;
            if input.scalar_type != ScalarType::Integer(guard.count_type)
                || !matches!(input.origin,
                    VirtualRegisterOrigin::EntryParameter { source_value, .. }
                    | VirtualRegisterOrigin::BlockParameter { source_value, .. }
                    | VirtualRegisterOrigin::InstructionResult { source_value, .. } if source_value == count)
            {
                return None;
            }
            let bound_register = function
                .virtual_registers
                .get(bound.virtual_register.0 as usize)?;
            let VirtualRegisterOrigin::InstructionScratch {
                instruction: materialize,
                operand: 0,
            } = bound_register.origin
            else {
                return None;
            };
            let materialize_position = block.instruction_index(materialize)?;
            let materialize = block.instructions.get(materialize_position)?;
            if materialize_position >= guard_position
                || materialize.kind
                    != (Kind::MaterializeI64 {
                        value: IntegerValue::Unsigned(u128::from(guard.max_bytes)),
                    })
                || materialize.provenance.operations.as_slice() != [operation]
                || materialize.provenance.values.as_slice() != [count]
                || !materialize.provenance.work.is_empty()
                || materialize.operands.as_slice().first()?.virtual_register
                    != bound.virtual_register
                || materialize.operands.len() != 1
                || bound_register.scalar_type != ScalarType::Integer(guard.count_type)
            {
                return None;
            }
            for (operand, expected) in [(output, 2), (scratch, 3)] {
                if function
                    .virtual_registers
                    .get(operand.virtual_register.0 as usize)?
                    .origin
                    != (VirtualRegisterOrigin::InstructionScratch {
                        instruction: instruction.id,
                        operand: expected,
                    })
                {
                    return None;
                }
            }
            Some(())
        }
    }

    mod floating_control {
        use machine_code::{
            ForeignCallFloatingControlRecord, FunctionTargetFrameLayout, PlacedFunctionFragment,
        };
        use selected_instructions::{
            LocalStorageSlotId, SelectedBlock, SelectedFunction, SelectedNormalizedForeignCall,
        };
        use target::{Architecture, NativeTarget};

        pub(super) struct FloatingControlCustody {
            pub x86: Option<ForeignCallFloatingControlRecord>,
            pub aarch64: Option<ForeignCallFloatingControlRecord>,
        }

        pub(super) fn derive(
            target: NativeTarget,
            function: &SelectedFunction,
            block: &SelectedBlock,
            fragment: &PlacedFunctionFragment,
            frame: &FunctionTargetFrameLayout,
            record: &SelectedNormalizedForeignCall,
        ) -> Result<FloatingControlCustody, &'static str> {
            let operation = record
                .invocation_site
                .operation()
                .ok_or("floating controls have no source operation")?;
            let slot = LocalStorageSlotId::Boundary { operation };
            let transport =
                selected_instructions::calls::normalized_foreign::return_transport::resolve(
                    function, block, record,
                )
                .ok_or("foreign result transport has no exact control restore")?;
            let call_position = transport.call_position;
            let restore_position = transport
                .restore_position
                .ok_or("foreign control restore is absent")?;
            let save_position =
                selected_instructions::calls::normalized_foreign::return_transport::save_position(
                    function,
                    block,
                    record,
                    call_position,
                )
                .ok_or(
                    "foreign control save envelope contains unrelated effects or invalid private transport",
                )?;
            let save = &block.instructions[save_position];
            let restore = block
                .instructions
                .get(restore_position)
                .ok_or("foreign control restore position is outside the block")?;
            let mut homes = function
                .local_storage_slots
                .iter()
                .filter(|home| home.id == slot);
            let home = homes.next().ok_or("foreign control slot is absent")?;
            let mut placements = frame
                .local_storage_slots
                .iter()
                .filter(|home| home.id == slot);
            let placement = placements
                .next()
                .ok_or("foreign control slot has no frame placement")?;
            if homes.next().is_some()
                || placements.next().is_some()
                || home.byte_size != 8
                || home.alignment != 8
                || placement.size_bytes != home.byte_size
                || placement.alignment_bytes != home.alignment
                || placement.frame_offset_bytes < frame.outgoing_abi_area.byte_size
                || !placement.frame_offset_bytes.is_multiple_of(8)
                || placement
                    .frame_offset_bytes
                    .checked_add(8)
                    .ok_or("foreign control frame extent overflows")?
                    > frame.frame_size_bytes
            {
                return Err("foreign control slot extent or alignment does not match the frame");
            }
            let saved_slot_byte_offset = u32::try_from(
                placement
                    .frame_offset_bytes
                    .checked_sub(frame.red_zone_resident_bytes)
                    .ok_or("foreign control slot precedes committed frame")?,
            )
            .map_err(|_| "foreign control slot offset is not addressable")?;
            let (save_offset, save_byte_count) =
                super::fragment_instruction_span(fragment, save.id)
                    .ok_or("foreign control or call has no emitted span")?;
            let (restore_offset, restore_byte_count) =
                super::fragment_instruction_span(fragment, restore.id)
                    .ok_or("foreign control or call has no emitted span")?;
            let (call_offset, call_byte_count) =
                super::fragment_instruction_span(fragment, record.instruction)
                    .ok_or("foreign control or call has no emitted span")?;
            let transported_end = block.instructions[call_position + 1..restore_position]
                .iter()
                .try_fold(
                    call_offset
                        .checked_add(call_byte_count)
                        .ok_or("foreign call emitted extent overflows")?,
                    |end, copy| {
                        let (offset, count) = super::fragment_instruction_span(fragment, copy.id)?;
                        (offset == end).then_some(())?;
                        offset.checked_add(count)
                    },
                )
                .ok_or("foreign result transport bytes are not contiguous before restore")?;
            if save_offset
                .checked_add(save_byte_count)
                .ok_or("foreign control save emitted extent overflows")?
                > call_offset
                || restore_offset != transported_end
            {
                return Err("foreign control save/call/restore emitted spans are out of order");
            }
            let save_offset = usize::try_from(save_offset)
                .map_err(|_| "foreign control emitted span is not addressable")?;
            let save_byte_count = usize::try_from(save_byte_count)
                .map_err(|_| "foreign control emitted span is not addressable")?;
            let restore_offset = usize::try_from(restore_offset)
                .map_err(|_| "foreign control emitted span is not addressable")?;
            let restore_byte_count = usize::try_from(restore_byte_count)
                .map_err(|_| "foreign control emitted span is not addressable")?;
            Ok(match target.architecture {
                Architecture::X86_64 => FloatingControlCustody {
                    x86: Some(ForeignCallFloatingControlRecord {
                        target,
                        saved_slot_byte_offset,
                        save_offset,
                        save_byte_count,
                        restore_offset,
                        restore_byte_count,
                    }),
                    aarch64: None,
                },
                Architecture::Aarch64 => FloatingControlCustody {
                    x86: None,
                    aarch64: Some(ForeignCallFloatingControlRecord {
                        target,
                        saved_slot_byte_offset,
                        save_offset,
                        save_byte_count,
                        restore_offset,
                        restore_byte_count,
                    }),
                },
            })
        }
    }

    use calling_conventions::ValueLocation;
    use semantic_vocabulary::{MachineId, ScalarType};
    use target_operations::CallSiteOwner;

    use super::carriers::{ObjectArtifact, ObjectForeignCall};
    use super::errors::ObjectError;

    pub fn derive_normalized_foreign_call_custody(
        source: &object_file::StagedOptimizedRelocationFreeObjectContainer,
    ) -> Result<Vec<ObjectForeignCall>, ObjectError> {
        let section = source.source().text_section();
        let emissions = source.source().source().source().source();
        let optimized = emissions.optimized_target();
        let selected = emissions.selected_plan();
        let layout = emissions.frame_layout();
        let architecture = source.source().source().fragments().target.architecture;
        let mut custody = Vec::new();
        for function in &selected.functions {
            let fragment =
                caller_fragment(section, function.machine).ok_or(ObjectError::TextSizeOverflow)?;
            if function.normalized_foreign_calls.is_empty() {
                continue;
            }
            let home_offsets = durable_home_offsets(function);
            let mut abstracted_functions = optimized
                .optimized()
                .plan()
                .functions
                .iter()
                .filter(|candidate| candidate.machine == function.machine);
            let abstracted = match (abstracted_functions.next(), abstracted_functions.next()) {
                (Some(abstracted), None) => Some(abstracted),
                _ => None,
            };
            let mut frames = layout
                .functions
                .iter()
                .filter(|frame| frame.machine == function.machine);
            let frame = match (frames.next(), frames.next()) {
                (Some(frame), None) => Some(frame),
                _ => None,
            };
            for (ordinal, record) in function.normalized_foreign_calls.iter().enumerate() {
                let owner = CallSiteOwner::from(record.invocation_site);
                let invalid = |reason| ObjectError::InvalidForeignCallProjection {
                    caller: function.machine,
                    owner,
                    reason,
                };
                if record.call.binding.native_target() != Some(section.target)
                    || !record.call.binding.has_valid_entry_mechanism()
                {
                    return Err(invalid(
                        "binding target or native entry mechanism does not match",
                    ));
                }
                let (block, instruction) = unique_block_instruction(function, record.instruction)
                    .ok_or_else(|| {
                    invalid("selected call instruction is missing or repeated")
                })?;
                descriptor_input::validate(function, block, record).ok_or_else(|| {
                    invalid("descriptor input guard does not match the selected call")
                })?;
                if !foreign_invocation_provenance_matches(
                    &instruction.provenance,
                    record.invocation_site,
                ) {
                    return Err(invalid(
                        "selected call instruction lost its invocation provenance",
                    ));
                }
                if !matches!(
                    instruction.kind,
                    selected_instructions::SelectedInstructionKind::NormalizedForeignCall {
                        boundary, ordinal: actual,
                    } if boundary == record.call.boundary && usize::try_from(actual) == Ok(ordinal)
                ) {
                    return Err(invalid(
                        "selected call boundary or roster ordinal does not match",
                    ));
                }
                if record.invocation_site.operation().is_none()
                    && selected_instructions::calls::normalized_foreign::return_transport::resolve(
                        function, block, record,
                    )
                    .is_none()
                {
                    return Err(invalid("external completion has no exact return transport"));
                }
                let mut fields =
                    section
                        .unresolved_normalized_foreign_calls
                        .iter()
                        .filter(|field| {
                            field.caller == function.machine
                                && field.invocation_site == record.invocation_site
                        });
                let field = match (
                    record.call.binding.symbolic_import(),
                    fields.next(),
                    fields.next(),
                ) {
                    (Some(_), Some(field), None) => {
                        if field.instruction != record.instruction
                            || field.boundary != record.call.boundary
                            || usize::try_from(field.ordinal) != Ok(ordinal)
                            || field.block != block.id
                            || field.state != machine_code::NormalizedForeignCallResolutionState::UnresolvedImportFieldV1
                        {
                            return Err(invalid("placed import field does not match the selected call"));
                        }
                        Some(field)
                    }
                    (None, None, None) => None,
                    _ => {
                        return Err(invalid(
                            "import relocation count does not match the native binding",
                        ));
                    }
                };
                let text_offset = if let Some(field) = field {
                    usize::try_from(field.field_section_offset)
                        .map_err(|_| invalid("import relocation offset is not addressable"))?
                } else {
                    usize::try_from(
                        fragment_instruction_span(fragment, record.instruction)
                            .ok_or_else(|| invalid("native call has no emitted instruction span"))?
                            .0,
                    )
                    .map_err(|_| invalid("native instruction offset is not addressable"))?
                };
                let scalar_arguments = foreign_scalar_argument_custody(
                    function,
                    block,
                    fragment,
                    record,
                    instruction,
                    &home_offsets,
                    function.machine,
                    owner,
                    |operand, register| {
                        operand.fixed_view
                            == emissions
                                .register_environment()
                                .fixed_register_view(register)
                    },
                )?;
                let scalar_result = foreign_scalar_result_custody(
                    function,
                    block,
                    fragment,
                    record,
                    instruction,
                    &home_offsets,
                    function.machine,
                    owner,
                )?;
                let callback_addresses = callback_address_custody(
                    record,
                    field.map_or(&[], |field| field.callback_materializations.as_slice()),
                    function.machine,
                    owner,
                )?;
                let Some(abstracted) = abstracted else {
                    return Err(invalid(
                        "caller is missing or repeated in the abstract operation roster",
                    ));
                };
                let mut ordinals =
                    abstracted
                        .operations
                        .iter()
                        .enumerate()
                        .filter_map(|(index, operation)| match operation {
                            abstract_operations::AbstractOperation::BoundaryCall {
                                psi_operation,
                                ..
                            } if record.invocation_site
                                == semantic_vocabulary::InvocationSite::Operation(
                                    *psi_operation,
                                ) =>
                            {
                                Some(index)
                            }
                            abstract_operations::AbstractOperation::ExternalCompletion {
                                psi_edge,
                                ..
                            } if record.invocation_site
                                == semantic_vocabulary::InvocationSite::ExternalCompletion(
                                    *psi_edge,
                                ) =>
                            {
                                Some(index)
                            }
                            _ => None,
                        });
                let operation_ordinal = match (ordinals.next(), ordinals.next()) {
                    (Some(ordinal), None) => ordinal,
                    (None, _) => {
                        return Err(ObjectError::ForeignCallOwnerNotInProvenance {
                            caller: function.machine,
                            owner,
                        });
                    }
                    _ => {
                        return Err(ObjectError::DuplicateForeignCallOwner {
                            caller: function.machine,
                            owner,
                        });
                    }
                };
                let Some(frame) = frame else {
                    return Err(invalid("caller frame is missing or repeated"));
                };
                if let target_operations::TargetBoundaryResult::Structural(home) =
                    &record.call.result
                {
                    let slot = selected_instructions::LocalStorageSlotId::Structural {
                        operation: record
                            .invocation_site
                            .operation()
                            .ok_or_else(|| invalid("structural result has no source operation"))?,
                        place: home.place(),
                    };
                    let mut homes = frame
                        .local_storage_slots
                        .iter()
                        .filter(|home| home.id == slot);
                    let Some(placed) = homes.next() else {
                        return Err(invalid(
                            "structural result storage is missing from the frame",
                        ));
                    };
                    let shape = home.layout.shape();
                    if homes.next().is_some()
                        || placed.size_bytes != shape.byte_size
                        || placed.alignment_bytes != shape.alignment
                        || placed.frame_offset_bytes < frame.outgoing_abi_area.byte_size
                        || !placed
                            .frame_offset_bytes
                            .is_multiple_of(u64::from(shape.alignment))
                        || placed
                            .frame_offset_bytes
                            .checked_add(u64::from(shape.byte_size))
                            .is_none_or(|end| end > frame.frame_size_bytes)
                    {
                        return Err(invalid(
                            "structural result storage has the wrong frame extent or alignment",
                        ));
                    }
                }
                let committed = frame
                    .frame_size_bytes
                    .checked_sub(frame.red_zone_resident_bytes)
                    .and_then(|extent| {
                        extent.checked_add(
                            if record.call.binding.same_stack_contribution().is_some()
                                && architecture == target::Architecture::X86_64
                            {
                                8
                            } else {
                                0
                            },
                        )
                    })
                    .ok_or(ObjectError::TextSizeOverflow)?;
                let caller_live_bytes =
                    u32::try_from(committed).map_err(|_| ObjectError::TextSizeOverflow)?;
                let controls = if record.call.binding.same_stack_contribution().is_some()
                    && record.invocation_site.operation().is_some()
                {
                    Some(
                        floating_control::derive(
                            section.target,
                            function,
                            block,
                            fragment,
                            frame,
                            record,
                        )
                        .map_err(invalid)?,
                    )
                } else {
                    if !record.call.callbacks.is_empty() {
                        return Err(invalid(
                            "callback addresses require an operation-owned native stack call",
                        ));
                    }
                    None
                };
                custody.push(ObjectForeignCall {
                    machine: function.machine,
                    owner,
                    operation_ordinal,
                    binding: record.call.binding.clone(),
                    provider_execution: record.call.provider_execution.into(),
                    caller_live_bytes,
                    scalar_arguments,
                    callback_addresses,
                    scalar_result,
                    x86_floating_control: controls.as_ref().and_then(|controls| controls.x86),
                    aarch64_floating_control: controls
                        .as_ref()
                        .and_then(|controls| controls.aarch64),
                    text_offset,
                });
            }
        }
        for (index, row) in custody.iter().enumerate() {
            if custody[..index]
                .iter()
                .any(|prior| prior.machine == row.machine && prior.owner == row.owner)
            {
                return Err(ObjectError::DuplicateForeignCallOwner {
                    caller: row.machine,
                    owner: row.owner,
                });
            }
        }
        Ok(custody)
    }

    fn caller_fragment(
        section: &machine_code::RelocationFreeTextSectionPlacement,
        caller: MachineId,
    ) -> Option<&machine_code::PlacedFunctionFragment> {
        let mut functions = section
            .functions
            .iter()
            .filter(|function| function.machine == caller);
        match (functions.next(), functions.next()) {
            (Some(function), None) => Some(function),
            _ => None,
        }
    }

    pub fn fragment_instruction_span(
        fragment: &machine_code::PlacedFunctionFragment,
        instruction: selected_instructions::SelectedInstructionId,
    ) -> Option<(u64, u64)> {
        fragment
            .instruction_span(instruction)
            .map(|span| (span.section_offset, span.byte_count))
    }

    pub fn transported_span_end(
        fragment: &machine_code::PlacedFunctionFragment,
        block: &selected_instructions::SelectedBlock,
        call_position: usize,
        normalization_position: usize,
    ) -> Option<u64> {
        block.instructions[call_position..normalization_position]
            .iter()
            .try_fold(None, |previous, step| {
                let (start, count) = fragment_instruction_span(fragment, step.id)?;
                if previous.is_some_and(|end| end != start) {
                    return None;
                }
                Some(Some(start.checked_add(count)?))
            })
            .flatten()
    }

    pub fn unique_block_instruction(
        function: &selected_instructions::SelectedFunction,
        instruction: selected_instructions::SelectedInstructionId,
    ) -> Option<(
        &selected_instructions::SelectedBlock,
        &selected_instructions::SelectedInstruction,
    )> {
        let mut instructions = function.blocks.iter().flat_map(|block| {
            block
                .instructions
                .iter()
                .filter(move |candidate| candidate.id == instruction)
                .map(move |candidate| (block, candidate))
        });
        let (Some(pair), None) = (instructions.next(), instructions.next()) else {
            return None;
        };
        Some(pair)
    }

    pub fn durable_home_offsets(
        function: &selected_instructions::SelectedFunction,
    ) -> Vec<(semantic_vocabulary::OperationId, u32)> {
        let mut producers = Vec::new();
        for contract in &function.calls {
            if let Some(placement) = &contract.call.result_placement
                && let target_operations::CallSiteOwner::Operation(operation) = contract.owner
            {
                producers.push((contract.instruction, operation, placement.shape.byte_size));
            }
        }
        for record in &function.normalized_foreign_calls {
            if let Some(home) = record.call.result.scalar_home() {
                producers.push((
                    record.instruction,
                    home.defining_operation,
                    home.shape.byte_size,
                ));
            }
            for argument in &record.call.scalar_arguments {
                if let target_operations::TargetUnitScalarArgumentSource::Home(requirement) =
                    argument.source
                {
                    let Some(instruction) = producer_instruction(
                        function,
                        requirement.defining_operation,
                        requirement.source_value,
                    ) else {
                        continue;
                    };
                    producers.push((
                        instruction,
                        requirement.defining_operation,
                        requirement.shape.byte_size,
                    ));
                }
            }
        }
        producers.sort_by_key(|(instruction, _, _)| instruction.0);
        producers.dedup_by_key(|(_, operation, _)| *operation);
        let mut cursor = 0u32;
        let mut offsets = Vec::new();
        for (_, operation, byte_size) in producers {
            let Some(aligned) = cursor.checked_add(7).map(|cursor| cursor & !7) else {
                return Vec::new();
            };
            cursor = aligned;
            offsets.push((operation, cursor));
            let Some(next) = cursor.checked_add(byte_size) else {
                return Vec::new();
            };
            cursor = next;
        }
        offsets
    }

    pub fn producer_instruction(
        function: &selected_instructions::SelectedFunction,
        operation: semantic_vocabulary::OperationId,
        source_value: semantic_vocabulary::ValueId,
    ) -> Option<selected_instructions::SelectedInstructionId> {
        function
            .calls
            .iter()
            .filter_map(|contract| {
                contract
                    .owner
                    .operation()
                    .map(|operation| (operation, contract.instruction))
            })
            .chain(
                function
                    .normalized_foreign_calls
                    .iter()
                    .filter_map(|record| {
                        record
                            .invocation_site
                            .operation()
                            .map(|operation| (operation, record.instruction))
                    }),
            )
            .find(|(candidate, _)| *candidate == operation)
            .map(|(_, instruction)| instruction)
            .or_else(|| scalar_observation_producer(function, operation, source_value))
    }

    pub(crate) fn scalar_observation_producer(
        function: &selected_instructions::SelectedFunction,
        operation: semantic_vocabulary::OperationId,
        source_value: semantic_vocabulary::ValueId,
    ) -> Option<selected_instructions::SelectedInstructionId> {
        let mut producers = function.instructions().filter(|instruction| {
            instruction.provenance.operations.contains(&operation)
                && instruction.operands.iter().any(|operand| {
                    function
                        .virtual_registers
                        .get(operand.virtual_register.0 as usize)
                        .is_some_and(|register| {
                            register.origin
                                == selected_instructions::VirtualRegisterOrigin::InstructionResult {
                                    instruction: instruction.id,
                                    source_value,
                                }
                        })
                })
        });
        let producer = producers.next()?;
        if producers.next().is_some() {
            return None;
        }
        Some(producer.id)
    }

    fn callback_address_custody(
        record: &selected_instructions::SelectedNormalizedForeignCall,
        placed: &[machine_code::PlacedCallbackAddressMaterialization],
        caller: MachineId,
        owner: CallSiteOwner,
    ) -> Result<Vec<machine_code::CallbackAddressMaterialization>, ObjectError> {
        let invalid = || ObjectError::InvalidForeignCallSite {
            caller,
            owner,
            offset: 0,
        };
        let plan = &record.call.binding.boundary_entry_plan.call;
        if record.call.callbacks.len() != placed.len()
            || placed.len() != plan.callback_materializations.len()
        {
            return Err(invalid());
        }
        let mut custody = Vec::with_capacity(placed.len());
        for (callback, placed) in record.call.callbacks.iter().zip(placed.iter()) {
            if plan
                .callback_materializations
                .iter()
                .filter(|row| row.destination == callback.destination)
                .count()
                != 1
            {
                return Err(invalid());
            }
            let destination = match placed.destination.clone() {
                machine_code::CallbackAddressDestination::Register(register) => {
                    machine_code::CallbackAddressDestination::Register(register)
                }
                machine_code::CallbackAddressDestination::OutgoingStack { byte_offset } => {
                    machine_code::CallbackAddressDestination::OutgoingStack { byte_offset }
                }
                machine_code::CallbackAddressDestination::RecordField {
                    root_register,
                    placements,
                } => {
                    if callback.field_placements.as_ref() != Some(&placements) {
                        return Err(invalid());
                    }
                    machine_code::CallbackAddressDestination::RecordField {
                        root_register,
                        placements,
                    }
                }
            };
            let encoding = match placed.encoding {
                machine_code::PlacedCallbackAddressEncoding::X86_64Relative32 {
                    relocation_function_offset,
                    ..
                } => machine_code::CallbackAddressEncoding::X86_64Relative32 {
                    relocation_offset: usize::try_from(relocation_function_offset)
                        .map_err(|_| invalid())?,
                },
                machine_code::PlacedCallbackAddressEncoding::Aarch64PageAddress {
                    page_relocation_function_offset,
                    page_offset_relocation_function_offset,
                    ..
                } => machine_code::CallbackAddressEncoding::Aarch64PageAddress {
                    page_relocation_offset: usize::try_from(page_relocation_function_offset)
                        .map_err(|_| invalid())?,
                    page_offset_relocation_offset: usize::try_from(
                        page_offset_relocation_function_offset,
                    )
                    .map_err(|_| invalid())?,
                },
            };
            custody.push(machine_code::CallbackAddressMaterialization {
                target: callback.clone(),
                destination,
                code_offset: usize::try_from(placed.code_function_offset).map_err(|_| invalid())?,
                byte_count: usize::try_from(placed.code_byte_count).map_err(|_| invalid())?,
                encoding,
            });
        }
        Ok(custody)
    }

    #[allow(clippy::too_many_arguments)]
    fn foreign_scalar_argument_custody<F>(
        function: &selected_instructions::SelectedFunction,
        block: &selected_instructions::SelectedBlock,
        fragment: &machine_code::PlacedFunctionFragment,
        record: &selected_instructions::SelectedNormalizedForeignCall,
        instruction: &selected_instructions::SelectedInstruction,
        home_offsets: &[(semantic_vocabulary::OperationId, u32)],
        caller: MachineId,
        owner: CallSiteOwner,
        fixed_view: F,
    ) -> Result<Vec<machine_code::ForeignCallScalarArgumentRecord>, ObjectError>
    where
        F: Fn(&selected_instructions::SelectedOperand, target_operations::MachineRegister) -> bool,
    {
        let invalid_argument = |reason,
                                argument: Option<
            &target_operations::TargetUnitScalarCallArgument,
        >| {
            ObjectError::InvalidForeignCallArgument {
                caller,
                owner,
                import: record
                    .call
                    .binding
                    .symbolic_import()
                    .map(|(locator, _)| format!("{locator:?}")),
                parameter: argument.map(|argument| argument.parameter_index),
                reason,
                transport: argument.map(|argument| {
                    scalar_argument_transport_context(function, block, fragment, record, argument)
                }),
            }
        };
        let invalid = |reason| invalid_argument(reason, None);
        let plan = &record.call.binding.boundary_entry_plan.call;
        let placements = record.call.binding.operand_placements();
        let parameter_slot_materializations = plan
            .callback_materializations
            .iter()
            .filter(|row| {
                matches!(
                    row.destination,
                    calling_conventions::NativePlace::Parameter(_)
                )
            })
            .count();
        if placements.len()
            != record.call.scalar_arguments.len()
                + record.call.structural_arguments.len()
                + parameter_slot_materializations
        {
            return Err(invalid(
                "argument roster does not match evaluated operand placements",
            ));
        }
        let register_positions =
            selected_instructions::calls::constraints::foreign_argument_registers(&record.call);
        let register_arguments = register_positions.len();
        if instruction.operands.len()
            != register_arguments
                + foreign_result_operand_count(record)
                    .ok_or_else(|| invalid("argument roster has no valid result operand count"))?
        {
            return Err(invalid(
                "selected call operand count does not match argument and result roster",
            ));
        }
        if let target_operations::TargetBoundaryResult::Structural(_) = record.call.result {
            let placement = plan
                .result
                .as_ref()
                .ok_or_else(|| invalid("structural result placement is missing"))?;
            for (location, operand) in placement
                .locations
                .iter()
                .zip(&instruction.operands[register_arguments..])
            {
                let register = location
                    .direct_register()
                    .ok_or_else(|| invalid("structural result location has no direct register"))?;
                if !fixed_view(operand, register) {
                    return Err(invalid(
                        "structural result operand lost its fixed ABI register",
                    ));
                }
            }
        }
        let mut scalar_arguments = Vec::new();
        for (scalar_ordinal, argument) in record.call.scalar_arguments.iter().enumerate() {
            let invalid = |reason| invalid_argument(reason, Some(argument));
            let native_position = usize::try_from(argument.parameter_index)
                .map_err(|_| invalid("parameter index is not addressable"))?;
            let shape = scalar_shape(argument.source.scalar_type())
                .ok_or_else(|| invalid("argument scalar type has no ABI shape"))?;
            if scalar_ordinal.checked_sub(1).is_some_and(|previous| {
                record.call.scalar_arguments[previous].parameter_index >= argument.parameter_index
            }) || placements.get(native_position) != Some(&argument.placement)
                || argument.placement.shape != shape
            {
                return Err(invalid(
                    "argument order, placement or scalar shape differs from evaluated plan",
                ));
            }
            let register_operand = register_positions
                .iter()
                .position(|(position, _)| *position == native_position)
                .unwrap_or(register_arguments);
            let (source, span) = match argument.source {
                source @ (target_operations::TargetUnitScalarArgumentSource::Parameter { .. }
                | target_operations::TargetUnitScalarArgumentSource::BlockParameter(_)
                | target_operations::TargetUnitScalarArgumentSource::BooleanImmediate {
                    ..
                }
                | target_operations::TargetUnitScalarArgumentSource::IeeeFloatImmediate {
                    ..
                }) => (
                    machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall {
                        source_value: source.source_value(),
                        scalar_type: source.scalar_type(),
                        instruction: record.instruction,
                    },
                    fragment_instruction_span(fragment, record.instruction)
                        .ok_or_else(|| invalid("selected argument call has no emitted span"))?,
                ),
                target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
                    defining_operation,
                    source_value,
                    scalar_type,
                    value,
                } => {
                    if semantic_vocabulary::ScalarTerm::integer(scalar_type, value).is_err() {
                        return Err(invalid(
                            "integer immediate is outside its declared scalar type",
                        ));
                    }
                    let span = scalar_immediate_span(
                        function,
                        block,
                        fragment,
                        record,
                        instruction,
                        argument,
                        register_operand,
                        &fixed_view,
                    )
                    .ok_or_else(|| {
                        invalid("integer immediate lost its exact emitted argument transport")
                    })?;
                    (
                        machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                            defining_operation,
                            source_value,
                            scalar_type,
                            value,
                        },
                        span,
                    )
                }
                target_operations::TargetUnitScalarArgumentSource::Home(home) => {
                    if home.shape != argument.placement.shape
                        || !scalar_call_result_producer(function, &home)
                    {
                        return Err(invalid(
                            "scalar home shape or exact producer does not match",
                        ));
                    }
                    let byte_offset = home_offsets
                        .iter()
                        .find(|(operation, _)| *operation == home.defining_operation)
                        .map(|(_, offset)| *offset)
                        .ok_or_else(|| invalid("scalar home has no allocated durable offset"))?;
                    let span = scalar_immediate_span(
                        function,
                        block,
                        fragment,
                        record,
                        instruction,
                        argument,
                        register_operand,
                        &fixed_view,
                    )
                    .ok_or_else(|| invalid("scalar home lost its exact emitted argument transport"))?;
                    (
                        machine_code::InternalUnitScalarArgumentSourceRecord::Home(
                            machine_code::UnitScalarHomeRecord {
                                defining_operation: home.defining_operation,
                                source_value: home.source_value,
                                scalar_type: home.scalar_type,
                                shape: home.shape,
                                byte_offset,
                            },
                        ),
                        span,
                    )
                }
            };
            scalar_arguments.push(machine_code::ForeignCallScalarArgumentRecord {
                parameter_index: argument.parameter_index,
                source,
                placement: argument.placement.clone(),
                code_offset: usize::try_from(span.0).map_err(|_| ObjectError::TextSizeOverflow)?,
                byte_count: usize::try_from(span.1).map_err(|_| ObjectError::TextSizeOverflow)?,
            });
        }
        Ok(scalar_arguments)
    }

    fn scalar_argument_transport_context(
        function: &selected_instructions::SelectedFunction,
        block: &selected_instructions::SelectedBlock,
        fragment: &machine_code::PlacedFunctionFragment,
        record: &selected_instructions::SelectedNormalizedForeignCall,
        argument: &target_operations::TargetUnitScalarCallArgument,
    ) -> String {
        use selected_instructions::{FrameStorageSlotId, SelectedInstructionKind};
        let outgoing = block.instructions.iter().position(|instruction| {
            matches!(
                instruction.kind,
                SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Outgoing(slot), byte_offset: 0,
                } if slot.invocation_site == record.invocation_site
                    && slot.argument_index == argument.parameter_index
            )
        });
        let anchor = outgoing
            .or_else(|| {
                block.instructions.iter().position(|instruction| {
                    instruction
                        .provenance
                        .values
                        .contains(&argument.source.source_value())
                })
            })
            .unwrap_or(0);
        let instructions =
            &block.instructions[anchor..anchor.saturating_add(6).min(block.instructions.len())];
        let mut registers = instructions
            .iter()
            .flat_map(|instruction| {
                instruction
                    .operands
                    .iter()
                    .map(|operand| operand.virtual_register)
            })
            .collect::<Vec<_>>();
        registers.sort_by_key(|register| register.0);
        registers.dedup();
        format!(
            "argument={argument:?}; slot={:?}; found_outgoing={}; steps={:?}; origins={:?}",
            function
                .outgoing_arguments
                .iter()
                .filter(|slot| slot.id.invocation_site == record.invocation_site
                    && slot.id.argument_index == argument.parameter_index)
                .collect::<Vec<_>>(),
            outgoing.is_some(),
            instructions
                .iter()
                .map(|instruction| (
                    instruction.id,
                    &instruction.kind,
                    instruction
                        .operands
                        .iter()
                        .map(|operand| (operand.virtual_register, operand.access))
                        .collect::<Vec<_>>(),
                    &instruction.provenance.values,
                    foreign_invocation_provenance_matches(
                        &instruction.provenance,
                        record.invocation_site
                    ),
                    fragment_instruction_span(fragment, instruction.id),
                ))
                .collect::<Vec<_>>(),
            registers
                .iter()
                .filter_map(|register| function
                    .virtual_register(*register)
                    .map(|value| (*register, value.origin)))
                .collect::<Vec<_>>()
        )
    }

    pub fn structural_result_capture(
        function: &selected_instructions::SelectedFunction,
        block: &selected_instructions::SelectedBlock,
        fragment: &machine_code::PlacedFunctionFragment,
        record: &selected_instructions::SelectedNormalizedForeignCall,
        instruction: &selected_instructions::SelectedInstruction,
    ) -> Option<()> {
        use selected_instructions::{
            FrameStorageSlotId, LocalStorageSlotId, SelectedInstructionKind as Kind,
            SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, VirtualRegisterOrigin,
        };
        let target_operations::TargetBoundaryResult::Structural(home) = &record.call.result else {
            return None;
        };
        let (operation, result) = home.operation_result()?;
        let target_operations::TargetStructuralHomeLayout::Aggregate(shape) = home.layout else {
            return None;
        };
        let placement = record
            .call
            .binding
            .boundary_entry_plan
            .call
            .result
            .as_ref()?;
        let contract = function.structural.as_ref()?;
        if Some(operation) != record.invocation_site.operation()
            || placement.shape.byte_size != shape.byte_size
            || placement.shape.alignment != shape.alignment
            || !contract.structural_places.iter().any(|place| {
                place.id == result.place
                    && place.kind
                        == semantic_vocabulary::StructuralPlaceKind::OperationResult {
                            producer: operation,
                            structural_type: result.structural_type,
                        }
            })
            || !contract
                .structural_types
                .iter()
                .any(|declared| declared.id == result.structural_type)
        {
            return None;
        }
        let slot = LocalStorageSlotId::Structural {
            operation,
            place: result.place,
        };
        let mut slots = function
            .local_storage_slots
            .iter()
            .filter(|candidate| candidate.id == slot);
        let storage = slots.next()?;
        if slots.next().is_some()
            || storage.byte_size != shape.byte_size
            || storage.alignment != shape.alignment
        {
            return None;
        }
        let result_operands = foreign_result_operand_count(record)?;
        let operands = instruction
            .operands
            .get(instruction.operands.len().checked_sub(result_operands)?..)?;
        let transport =
            selected_instructions::calls::normalized_foreign::return_transport::resolve(
                function, block, record,
            )?;
        let call_position = transport.call_position;
        let mut position = transport.normalization_position;
        let address = block.instructions.get(position)?;
        if address.kind
            != (Kind::FrameAddress {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            })
        {
            return None;
        }
        let [pointer] = address.operands.as_slice() else {
            return None;
        };
        let indirect = matches!(
            placement.locations.as_slice(),
            [ValueLocation::Indirect { .. }]
        );
        let origin = if indirect {
            SelectedMemoryAccessOrigin::Operation(operation)
        } else {
            SelectedMemoryAccessOrigin::Block(block.source_block())
        };
        let memory_matches =
            |selected: selected_instructions::SelectedInstructionId, offset, width, role| {
                let mut accesses = function
                    .memory_accesses
                    .iter()
                    .filter(|access| access.instruction == selected);
                let Some(access) = accesses.next() else {
                    return false;
                };
                accesses.next().is_none()
                    && access.origin == origin
                    && access.place == result.place
                    && access.byte_offset == offset
                    && access.byte_count == width
                    && access.role == role
            };
        let transport_matches = |register: selected_instructions::VirtualRegisterId,
                                 selected: selected_instructions::SelectedInstructionId,
                                 offset| {
            function
                .virtual_registers
                .get(register.0 as usize)
                .is_some_and(|value| {
                    value.origin
                        == VirtualRegisterOrigin::AbiTransport {
                            instruction: selected,
                            place: result.place,
                            byte_offset: offset,
                        }
                })
        };
        if !transport_matches(pointer.virtual_register, address.id, 0)
            || !memory_matches(
                address.id,
                0,
                shape.byte_size,
                SelectedMemoryAccessRole::AddressLocal { slot },
            )
        {
            return None;
        }
        let (call_offset, call_bytes) = fragment_instruction_span(fragment, instruction.id)?;
        let (address_offset, address_bytes) = fragment_instruction_span(fragment, address.id)?;
        if address_bytes == 0 || address_offset < call_offset.checked_add(call_bytes)? {
            return None;
        }
        if indirect {
            let [argument] = operands else { return None };
            // Register homes may split the hidden pointer before its fixed argument
            // register: each split is a `CopyI64` whose output keeps this result
            // place's transport identity. Follow those copies back to the one
            // `FrameAddress` that names the result slot.
            let mut register = argument.virtual_register;
            let mut preparation = None;
            // Each copy reads a strictly earlier instruction, so a valid chain
            // visits at most the instructions that precede the call.
            for _ in 0..=call_position {
                let VirtualRegisterOrigin::AbiTransport {
                    instruction: prepared,
                    place,
                    byte_offset: 0,
                } = function.virtual_registers.get(register.0 as usize)?.origin
                else {
                    return None;
                };
                if place != result.place {
                    return None;
                }
                let candidate = block
                    .instructions
                    .iter()
                    .take(call_position)
                    .find(|candidate| candidate.id == prepared)?;
                match (candidate.kind, candidate.operands.as_slice()) {
                    (Kind::CopyI64, [input, output])
                        if output.virtual_register == register
                            && function
                                .memory_accesses
                                .iter()
                                .all(|access| access.instruction != prepared) =>
                    {
                        register = input.virtual_register;
                    }
                    (_, [output]) if output.virtual_register == register => {
                        preparation = Some(candidate);
                        break;
                    }
                    _ => return None,
                }
            }
            let preparation = preparation?;
            let prepared = preparation.id;
            let (prepared_offset, prepared_bytes) = fragment_instruction_span(fragment, prepared)?;
            return (preparation.kind == address.kind
                && memory_matches(
                    prepared,
                    0,
                    shape.byte_size,
                    SelectedMemoryAccessRole::AddressLocal { slot },
                )
                && prepared_bytes != 0
                && prepared_offset.checked_add(prepared_bytes)? <= call_offset)
                .then_some(());
        }
        let mut previous_end = address_offset.checked_add(address_bytes)?;
        position = position.checked_add(1)?;
        if transport.result_registers.len() != placement.locations.len() {
            return None;
        }
        for (location, register) in placement.locations.iter().zip(transport.result_registers) {
            let ValueLocation::Register {
                value_byte_offset,
                byte_size,
                ..
            } = *location
            else {
                return None;
            };
            if value_byte_offset.checked_add(byte_size)? > shape.byte_size {
                return None;
            }
            let mut value = register;
            let mut store = block.instructions.get(position)?;
            if matches!(store.kind, Kind::Float32ToBits | Kind::Float64ToBits) {
                let expected = match byte_size {
                    4 => Kind::Float32ToBits,
                    8 => Kind::Float64ToBits,
                    _ => return None,
                };
                let [input, output] = store.operands.as_slice() else {
                    return None;
                };
                if store.kind != expected
                    || input.virtual_register != value
                    || !transport_matches(output.virtual_register, store.id, value_byte_offset)
                {
                    return None;
                }
                let (offset, width) = fragment_instruction_span(fragment, store.id)?;
                if width == 0 || offset < previous_end {
                    return None;
                }
                previous_end = offset.checked_add(width)?;
                value = output.virtual_register;
                position = position.checked_add(1)?;
                store = block.instructions.get(position)?;
            }
            let actual = match store.kind {
                Kind::Store {
                    byte_offset,
                    byte_size,
                } => (byte_offset, u32::from(byte_size)),
                Kind::StorePacked { byte_offset, width } => {
                    (byte_offset, u32::from(width.byte_size()))
                }
                _ => return None,
            };
            if actual != (value_byte_offset, byte_size)
                || store.operands.first()?.virtual_register != pointer.virtual_register
                || store.operands.get(1)?.virtual_register != value
                || !memory_matches(
                    store.id,
                    value_byte_offset,
                    byte_size,
                    SelectedMemoryAccessRole::WritePlace,
                )
            {
                return None;
            }
            let (offset, width) = fragment_instruction_span(fragment, store.id)?;
            if width == 0 || offset < previous_end {
                return None;
            }
            previous_end = offset.checked_add(width)?;
            position = position.checked_add(1)?;
        }
        Some(())
    }

    pub fn foreign_result_operand_count(
        record: &selected_instructions::SelectedNormalizedForeignCall,
    ) -> Option<usize> {
        match (
            &record.call.result,
            &record.call.binding.boundary_entry_plan.call.result,
        ) {
            (target_operations::TargetBoundaryResult::Unit, None) => Some(0),
            (target_operations::TargetBoundaryResult::Scalar(_), Some(_)) => Some(1),
            (target_operations::TargetBoundaryResult::Structural(_), Some(placement)) => {
                match placement.locations.as_slice() {
                    [
                        ValueLocation::Indirect {
                            pointer: calling_conventions::IndirectPointerLocation::Register(_),
                            ..
                        },
                    ] => Some(1),
                    locations
                        if !locations.is_empty()
                            && locations.iter().all(|location| {
                                matches!(location, ValueLocation::Register { .. })
                            }) =>
                    {
                        Some(locations.len())
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn foreign_scalar_result_custody(
        function: &selected_instructions::SelectedFunction,
        block: &selected_instructions::SelectedBlock,
        fragment: &machine_code::PlacedFunctionFragment,
        record: &selected_instructions::SelectedNormalizedForeignCall,
        instruction: &selected_instructions::SelectedInstruction,
        home_offsets: &[(semantic_vocabulary::OperationId, u32)],
        caller: MachineId,
        owner: CallSiteOwner,
    ) -> Result<Option<machine_code::ForeignCallScalarResultRecord>, ObjectError> {
        let invalid = |reason| ObjectError::InvalidForeignCallArgument {
            caller,
            owner,
            import: record
                .call
                .binding
                .symbolic_import()
                .map(|(locator, _)| format!("{locator:?}")),
            parameter: None,
            reason,
            transport: None,
        };
        let plan = &record.call.binding.boundary_entry_plan.call;
        if matches!(
            record.call.result,
            target_operations::TargetBoundaryResult::Structural(_)
        ) {
            structural_result_capture(function, block, fragment, record, instruction)
                .ok_or_else(|| invalid("structural result lost its exact storage capture"))?;
            return Ok(None);
        }
        let (Some(home), Some(placement)) = (record.call.result.scalar_home(), &plan.result) else {
            if matches!(
                record.call.result,
                target_operations::TargetBoundaryResult::Unit
            ) && plan.result.is_none()
            {
                return Ok(None);
            }
            return Err(invalid(
                "scalar result home and placement disagree about Unit",
            ));
        };
        let shape = scalar_shape(home.scalar_type)
            .ok_or_else(|| invalid("result scalar type has no ABI shape"))?;
        if Some(home.defining_operation) != record.invocation_site.operation()
            || home.shape != shape
            || placement.shape != home.shape
            || !placement.is_complete_scalar_result()
        {
            return Err(invalid(
                "scalar result owner, shape or complete placement differs from evaluated plan",
            ));
        }
        let transport =
            selected_instructions::calls::normalized_foreign::return_transport::resolve(
                function, block, record,
            )
            .ok_or_else(|| invalid("scalar result has no exact selected return transport"))?;
        let normalization = block
            .instructions
            .get(transport.normalization_position)
            .ok_or_else(|| invalid("scalar result normalization instruction is missing"))?;
        let Some(kind) = scalar_result_normalization_kind(
            home.scalar_type,
            matches!(
                placement.locations.as_slice(),
                [ValueLocation::Indirect { .. }]
            ),
        ) else {
            return Err(invalid("scalar result has no supported normalization kind"));
        };
        let [input, output] = normalization.operands.as_slice() else {
            return Err(invalid(
                "scalar result normalization does not have input and output operands",
            ));
        };
        let output_origin = function
            .virtual_registers
            .get(output.virtual_register.0 as usize)
            .map(|register| register.origin);
        if normalization.kind != kind
            || transport.result_registers.as_slice() != [input.virtual_register]
            || normalization.provenance.operations.as_slice()
                != [record
                    .invocation_site
                    .operation()
                    .ok_or_else(|| invalid("scalar result has no operation owner"))?]
            || normalization.provenance.values.as_slice() != [home.source_value]
            || output_origin
                != Some(
                    selected_instructions::VirtualRegisterOrigin::InstructionResult {
                        instruction: normalization.id,
                        source_value: home.source_value,
                    },
                )
        {
            return Err(invalid(
                "scalar result normalization lost its kind, value or source provenance",
            ));
        }
        let Some((offset, byte_count)) = fragment_instruction_span(fragment, normalization.id)
        else {
            return Err(invalid("scalar result normalization has no emitted span"));
        };
        let transported_end = transported_span_end(
            fragment,
            block,
            transport.call_position,
            transport.normalization_position,
        )
        .ok_or_else(|| invalid("scalar return transport spans are not contiguous"))?;
        if transported_end != offset {
            return Err(invalid(
                "scalar result normalization does not immediately follow return transport",
            ));
        }
        let Some(byte_offset) = home_offsets
            .iter()
            .find(|(operation, _)| *operation == home.defining_operation)
            .map(|(_, offset)| *offset)
        else {
            return Err(invalid("scalar result has no allocated durable offset"));
        };
        Ok(Some(machine_code::ForeignCallScalarResultRecord {
            home: machine_code::UnitScalarHomeRecord {
                defining_operation: home.defining_operation,
                source_value: home.source_value,
                scalar_type: home.scalar_type,
                shape: home.shape,
                byte_offset,
            },
            source: placement.clone(),
            code_offset: usize::try_from(offset).map_err(|_| ObjectError::TextSizeOverflow)?,
            byte_count: usize::try_from(byte_count).map_err(|_| ObjectError::TextSizeOverflow)?,
        }))
    }

    fn scalar_result_normalization_kind(
        scalar_type: ScalarType,
        indirect: bool,
    ) -> Option<selected_instructions::SelectedInstructionKind> {
        use selected_instructions::SelectedInstructionKind as Kind;
        use semantic_vocabulary::IntegerSign;
        if indirect && matches!(scalar_type, ScalarType::IeeeFloat(_)) {
            return Some(Kind::CopyI64);
        }
        match scalar_type {
            ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => {
                Some(Kind::Float32ToBits)
            }
            ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => {
                Some(Kind::Float64ToBits)
            }
            ScalarType::Boolean => Some(Kind::ZeroExtendU8),
            ScalarType::Integer(integer) => match (integer.sign(), integer.bits()) {
                (IntegerSign::Unsigned, 8) => Some(Kind::ZeroExtendU8),
                (IntegerSign::Unsigned, 16) => Some(Kind::ZeroExtendU16),
                (IntegerSign::Unsigned, 32) => Some(Kind::ZeroExtendU32),
                (IntegerSign::Unsigned, 64) | (IntegerSign::Signed, 64) => Some(Kind::CopyI64),
                (IntegerSign::Signed, 8) => Some(Kind::SignExtendI8),
                (IntegerSign::Signed, 16) => Some(Kind::SignExtendI16),
                (IntegerSign::Signed, 32) => Some(Kind::SignExtendI32),
                _ => None,
            },
        }
    }

    fn scalar_shape(scalar_type: ScalarType) -> Option<calling_conventions::ValueShape> {
        calling_conventions::ValueShape::for_scalar(scalar_type)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn scalar_immediate_span<F>(
        function: &selected_instructions::SelectedFunction,
        block: &selected_instructions::SelectedBlock,
        fragment: &machine_code::PlacedFunctionFragment,
        record: &selected_instructions::SelectedNormalizedForeignCall,
        instruction: &selected_instructions::SelectedInstruction,
        argument: &target_operations::TargetUnitScalarCallArgument,
        register_operand: usize,
        fixed_view: &F,
    ) -> Option<(u64, u64)>
    where
        F: Fn(&selected_instructions::SelectedOperand, target_operations::MachineRegister) -> bool,
    {
        match argument.placement.locations.as_slice() {
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size,
                },
            ] if *byte_size == argument.placement.shape.byte_size => {
                let operand = instruction.operands.get(register_operand)?;
                if usize::from(operand.operand) != register_operand
                    || !fixed_view(operand, *register)
                {
                    return None;
                }
                let register_value = function
                    .virtual_registers
                    .get(operand.virtual_register.0 as usize)?;
                let selected_instructions::VirtualRegisterOrigin::InstructionResult {
                    instruction: materialization,
                    source_value,
                } = register_value.origin
                else {
                    return None;
                };
                if source_value != argument.source.source_value() {
                    return None;
                }
                fragment_instruction_span(fragment, materialization)
            }
            [
                ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset: 0,
                    byte_size,
                    alignment,
                },
            ] if *byte_size == argument.placement.shape.byte_size => {
                let slot = selected_instructions::OutgoingArgumentSlotId {
                    invocation_site: record.invocation_site,
                    argument_index: argument.parameter_index,
                    role: selected_instructions::OutgoingArgumentSlotRole::Argument,
                };
                let mut outgoing = function
                    .outgoing_arguments
                    .iter()
                    .filter(|outgoing| outgoing.id == slot);
                let (Some(outgoing), None) = (outgoing.next(), outgoing.next()) else {
                    return None;
                };
                if outgoing.abi_stack_byte_offset != *stack_byte_offset
                    || outgoing.byte_size != (*byte_size)
                    || outgoing.alignment != *alignment
                {
                    return None;
                }
                let source_value = argument.source.source_value();
                let stored = u8::try_from(*byte_size).ok()?;
                let address_index = block.instructions.iter().position(|candidate| {
                    matches!(
                        candidate.kind,
                        selected_instructions::SelectedInstructionKind::FrameAddress {
                            slot: selected_instructions::FrameStorageSlotId::Outgoing(candidate),
                            byte_offset: 0,
                        } if candidate == slot
                    ) && foreign_invocation_provenance_matches(
                        &candidate.provenance,
                        record.invocation_site,
                    ) && candidate.provenance.values.as_slice() == [source_value]
                })?;
                let address = &block.instructions[address_index];
                let mut store_index = address_index.checked_add(1)?;
                let next = block.instructions.get(store_index)?;
                let rematerialized = next.operands.first().and_then(|operand| {
                    function
                        .virtual_register(operand.virtual_register)
                        .filter(|value| rematerializes_argument(next, value, argument.source))
                        .map(|value| value.id)
                });
                if rematerialized.is_some() {
                    store_index = store_index.checked_add(1)?;
                }
                let store = block.instructions.get(store_index)?;
                let selected_instructions::SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: stored_bytes,
                } = store.kind
                else {
                    return None;
                };
                if stored_bytes != stored
                    || !foreign_invocation_provenance_matches(
                        &store.provenance,
                        record.invocation_site,
                    )
                    || store.provenance.values.as_slice() != [source_value]
                {
                    return None;
                }
                let address_operand = store.operands.first()?;
                let address_value = function
                    .virtual_registers
                    .get(address_operand.virtual_register.0 as usize)?;
                if address_value.origin
                    != (selected_instructions::VirtualRegisterOrigin::ScalarAbiAddress {
                        instruction: address.id,
                        source_value,
                    })
                {
                    return None;
                }
                if rematerialized.is_some_and(|value| {
                    store
                        .operands
                        .get(1)
                        .map(|operand| operand.virtual_register)
                        != Some(value)
                }) {
                    return None;
                }
                let (address_offset, _) = fragment_instruction_span(fragment, address.id)?;
                let end = transported_span_end(
                    fragment,
                    block,
                    address_index,
                    store_index.checked_add(1)?,
                )?;
                Some((address_offset, end.checked_sub(address_offset)?))
            }
            _ => None,
        }
    }

    fn rematerializes_argument(
        instruction: &selected_instructions::SelectedInstruction,
        register: &selected_instructions::VirtualRegister,
        source: target_operations::TargetUnitScalarArgumentSource,
    ) -> bool {
        use register_model::RegisterOperandAccess;
        use selected_instructions::{SelectedInstructionKind, VirtualRegisterOrigin};
        let target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
            source_value,
            value,
            ..
        } = source
        else {
            return false;
        };
        let [output] = instruction.operands.as_slice() else {
            return false;
        };
        instruction.kind == SelectedInstructionKind::MaterializeI64 { value }
            && output.virtual_register == register.id
            && output.access == RegisterOperandAccess::Def
            && output.class == register.class
            && output.fixed_view.is_none()
            && output.tied_to.is_none()
            && !output.early_clobber
            && register.entry_fixed_view.is_none()
            && register.scalar_type == source.scalar_type()
            && register.origin
                == VirtualRegisterOrigin::InstructionResult {
                    instruction: instruction.id,
                    source_value,
                }
            && instruction.implicit_uses.is_empty()
            && instruction.implicit_defs.is_empty()
            && instruction.clobbers.is_empty()
            && instruction.provenance.operations.is_empty()
            && instruction.provenance.edges.is_empty()
            && instruction.provenance.obligations.is_empty()
            && instruction.provenance.work.is_empty()
            && instruction.provenance.values.as_slice() == [source_value]
    }

    pub fn scalar_call_result_producer(
        function: &selected_instructions::SelectedFunction,
        home: &target_operations::TargetUnitScalarHomeRequirement,
    ) -> bool {
        let mut internal = function.calls.iter().filter(|contract| {
            contract.owner == target_operations::CallSiteOwner::Operation(home.defining_operation)
        });
        let mut foreign = function
            .normalized_foreign_calls
            .iter()
            .filter(|record| record.invocation_site.operation() == Some(home.defining_operation));
        let (producer, result_home) = match (internal.next(), foreign.next()) {
            (Some(contract), None) if internal.next().is_none() => (contract.instruction, None),
            (None, Some(record)) if foreign.next().is_none() => {
                (record.instruction, Some(record.call.result.scalar_home()))
            }
            (None, None) => {
                let Some(producer) = scalar_observation_producer(
                    function,
                    home.defining_operation,
                    home.source_value,
                ) else {
                    return false;
                };
                (producer, None)
            }
            _ => return false,
        };
        if let Some(recorded) = result_home
            && recorded != Some(home)
        {
            return false;
        }
        let Some(producer) = function
            .instructions()
            .find(|instruction| instruction.id == producer)
        else {
            return false;
        };
        producer.operands.iter().any(|operand| {
            function
                .virtual_registers
                .get(operand.virtual_register.0 as usize)
                .is_some_and(|register| {
                    register.origin
                        == selected_instructions::VirtualRegisterOrigin::InstructionResult {
                            instruction: producer.id,
                            source_value: home.source_value,
                        }
                })
        })
    }

    pub(crate) fn image_foreign_calls_match_object(
        artifact: &ObjectArtifact,
        image_rows: &[ObjectForeignCall],
    ) -> bool {
        image_rows == artifact.foreign_calls()
    }

    pub fn foreign_invocation_provenance_matches(
        provenance: &selected_instructions::SelectedInstructionProvenance,
        site: semantic_vocabulary::InvocationSite,
    ) -> bool {
        match site {
            semantic_vocabulary::InvocationSite::InvokeNoNormal(_) => false,
            semantic_vocabulary::InvocationSite::Operation(operation) => {
                provenance.operations.last() == Some(&operation)
                    && provenance.edges.is_empty()
                    && provenance.work_settles_only_claimed_sites()
            }
            semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => {
                provenance.edges.as_slice() == [edge]
                    && provenance.work_settles_only_claimed_sites()
            }
        }
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
                let encoded = match target.architecture {
                    Architecture::X86_64 => {
                        isa_x86_64::encode_linux_write_line_literal(&custody.bytes)
                    }
                    Architecture::Aarch64 => {
                        isa_aarch64::encode_linux_write_line_literal(&custody.bytes)
                    }
                };
                let Ok((encoded, data)) = encoded else {
                    return false;
                };
                let exact_intervals = settlement.byte_count == encoded.len()
                    && settlement.byte_count != 0
                    && custody.code_offset == settlement.code_offset
                    && custody.code_byte_count == data.start
                    && custody.code_byte_count != 0
                    && custody.data_offset == settlement.code_offset.saturating_add(data.start)
                    && custody.data_byte_count == data.len()
                    && custody.data_byte_count == custody.bytes.len().saturating_add(1)
                    && encoded.get(data.clone()).is_some_and(|payload| {
                        payload.strip_suffix(b"\n") == Some(custody.bytes.as_slice())
                    });
                if !exact_intervals {
                    return false;
                }
                function_bytes.is_none_or(|function_bytes| {
                    settlement
                        .code_offset
                        .checked_add(settlement.byte_count)
                        .and_then(|end| function_bytes.get(settlement.code_offset..end))
                        == Some(encoded.as_slice())
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
                                && integer.bits() == 32
                        }
                        terminal_psi::StructuralFieldType::BoundedInteger(bounds) => {
                            let integer = bounds.integer_type();
                            !integer.is_address()
                                && integer.sign() == IntegerSign::Signed
                                && integer.bits() == 32
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
                                id: semantic_vocabulary::StructuralCaseId::new(1).unwrap(),
                                identity: terminal_psi::TerminalIdentityIndex(2),
                                fields: vec![],
                            },
                            terminal_psi::StructuralCaseDeclaration {
                                id: semantic_vocabulary::StructuralCaseId::new(2).unwrap(),
                                identity: terminal_psi::TerminalIdentityIndex(3),
                                fields: vec![terminal_psi::StructuralFieldDeclaration {
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
                            isa_x86_64::decode_x86_64_selected_hosted_exit_process_i32(
                                target, bytes,
                            )
                        }
                        Architecture::Aarch64 => {
                            isa_aarch64::decode_aarch64_selected_hosted_exit_process_i32(
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
                    matches!(scalar_type, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Signed && integer.bits() == 32)
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
                    isa_x86_64::decode_x86_64_selected_hosted_write_byte_i32(target, bytes)
                } else if target == NativeTarget::linux_arm64()
                    || target == NativeTarget::macos_arm64()
                {
                    isa_aarch64::decode_aarch64_selected_hosted_write_byte_i32(target, bytes)
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
                            isa_aarch64::aarch64_hosted_write_byte_i32_byte_count(target, home)
                        })
                    else {
                        return false;
                    };
                    byte_count
                } else {
                    return false;
                };
                matches!(scalar_type, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Signed && integer.bits() == 32)
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

    pub(crate) mod structural {
        pub(crate) mod condition_layout {
            use calling_conventions::ValueShape;
            use semantic_vocabulary::{ScalarType, StructuralTypeId};

            fn checked_align_up(value: u32, alignment: u32) -> Option<u32> {
                value
                    .checked_add(alignment.checked_sub(1)?)
                    .map(|value| value / alignment * alignment)
            }

            fn layout_field(field: &terminal_psi::StructuralFieldDeclaration) -> bool {
                !field.relevance.is_erased()
                    && !matches!(
                        field.field_type,
                        terminal_psi::StructuralFieldType::Erased { .. }
                    )
            }

            fn replay_structural_shape(
                structural_type: StructuralTypeId,
                declarations: &std::collections::BTreeMap<
                    StructuralTypeId,
                    &terminal_psi::StructuralTypeDeclaration,
                >,
                cache: &mut std::collections::BTreeMap<StructuralTypeId, ValueShape>,
                active: &mut std::collections::BTreeSet<StructuralTypeId>,
            ) -> Option<ValueShape> {
                if let Some(shape) = cache.get(&structural_type) {
                    return Some(*shape);
                }
                if !active.insert(structural_type) {
                    return None;
                }
                let declaration = declarations.get(&structural_type)?;
                let shape = match &declaration.shape {
                    terminal_psi::StructuralTypeShape::Reference { referent, .. } => {
                        let scalar_codec = match declarations.get(referent)?.shape {
                            terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar) => {
                                runtime_abi::ScalarStorageCodec::from_scalar(scalar)
                            }
                            _ => None,
                        };
                        ValueShape::integer(if scalar_codec.is_some() { 16 } else { 8 }, 8)
                    }
                    terminal_psi::StructuralTypeShape::AtomicCell(cell) => {
                        replay_structural_field_shape(&(*cell).into(), declarations, cache, active)?
                    }
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean) => {
                        ValueShape::integer(1, 1)
                    }
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                        integer,
                    )) => {
                        let size = integer.bits().div_ceil(8);
                        ValueShape::integer(u32::from(size), size.next_power_of_two().min(8))
                    }
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    )) => ValueShape::float(4),
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    )) => ValueShape::float(8),
                    terminal_psi::StructuralTypeShape::ByteSequence(_)
                    | terminal_psi::StructuralTypeShape::ElementView { .. } => return None,
                    terminal_psi::StructuralTypeShape::Record { fields, .. } => {
                        let mut byte_size = 0_u32;
                        let mut alignment = 1_u16;
                        for field in fields.iter().filter(|field| layout_field(field)) {
                            let field_shape = replay_structural_field_shape(
                                &field.field_type,
                                declarations,
                                cache,
                                active,
                            )?;
                            alignment = alignment.max(field_shape.alignment);
                            byte_size =
                                checked_align_up(byte_size, u32::from(field_shape.alignment))?
                                    .checked_add(field_shape.byte_size)?;
                        }
                        byte_size = checked_align_up(byte_size, u32::from(alignment))?;
                        ValueShape::integer(byte_size, alignment)
                    }
                    terminal_psi::StructuralTypeShape::FixedArray {
                        length: terminal_psi::ArrayExtent::Literal(0),
                        ..
                    } => ValueShape::integer(0, 1),
                    terminal_psi::StructuralTypeShape::FixedArray { element, length } => {
                        let length = length.literal()?;
                        let element =
                            replay_structural_shape(*element, declarations, cache, active)?;
                        let stride =
                            checked_align_up(element.byte_size, u32::from(element.alignment))?;
                        let byte_size = u64::from(stride)
                            .checked_mul(length)
                            .and_then(|size| u32::try_from(size).ok())?;
                        ValueShape::integer(byte_size, element.alignment)
                    }
                    terminal_psi::StructuralTypeShape::Sum { cases } => {
                        replay_sum_layout(&[], cases, declarations, cache, active)?.shape
                    }
                    terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
                        replay_sum_layout(fields, cases, declarations, cache, active)?.shape
                    }
                };
                active.remove(&structural_type);
                cache.insert(structural_type, shape);
                Some(shape)
            }

            fn replay_sum_layout(
                common: &[terminal_psi::StructuralFieldDeclaration],
                cases: &[terminal_psi::StructuralCaseDeclaration],
                declarations: &std::collections::BTreeMap<
                    StructuralTypeId,
                    &terminal_psi::StructuralTypeDeclaration,
                >,
                cache: &mut std::collections::BTreeMap<StructuralTypeId, ValueShape>,
                active: &mut std::collections::BTreeSet<StructuralTypeId>,
            ) -> Option<calling_conventions::ConventionalSumLayout> {
                let common = common
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .map(|field| {
                        replay_structural_field_shape(
                            &field.field_type,
                            declarations,
                            cache,
                            active,
                        )
                    })
                    .collect::<Option<Vec<_>>>()?;
                let payloads = cases
                    .iter()
                    .map(|case| {
                        case.fields
                            .iter()
                            .filter(|field| !field.relevance.is_erased())
                            .map(|field| {
                                replay_structural_field_shape(
                                    &field.field_type,
                                    declarations,
                                    cache,
                                    active,
                                )
                            })
                            .collect::<Option<Vec<_>>>()
                    })
                    .collect::<Option<Vec<_>>>()?;
                calling_conventions::evaluate_conventional_sum_layout(&common, &payloads).ok()
            }

            pub(crate) fn replay_structural_sum_layout(
                structural_type: StructuralTypeId,
                declarations: &[terminal_psi::StructuralTypeDeclaration],
            ) -> Option<calling_conventions::ConventionalSumLayout> {
                let declaration_count = declarations.len();
                let declarations = declarations
                    .iter()
                    .map(|declaration| (declaration.id, declaration))
                    .collect::<std::collections::BTreeMap<_, _>>();
                if declarations.len() != declaration_count {
                    return None;
                }
                let (common, cases) = match &declarations.get(&structural_type)?.shape {
                    terminal_psi::StructuralTypeShape::Sum { cases } => (&[][..], cases),
                    terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
                        (fields.as_slice(), cases)
                    }
                    _ => return None,
                };
                replay_sum_layout(
                    common,
                    cases,
                    &declarations,
                    &mut std::collections::BTreeMap::new(),
                    &mut std::collections::BTreeSet::new(),
                )
            }

            pub(crate) fn replay_structural_value_shape(
                structural_type: StructuralTypeId,
                declarations: &[terminal_psi::StructuralTypeDeclaration],
            ) -> Option<ValueShape> {
                let declarations = declarations
                    .iter()
                    .map(|declaration| (declaration.id, declaration))
                    .collect::<std::collections::BTreeMap<_, _>>();
                let mut cache = std::collections::BTreeMap::new();
                let mut active = std::collections::BTreeSet::new();
                replay_structural_shape(structural_type, &declarations, &mut cache, &mut active)
            }

            pub(crate) fn replay_structural_projection(
                mut structural_type: StructuralTypeId,
                path: &[terminal_psi::StructuralPathSegment],
                declarations: &[terminal_psi::StructuralTypeDeclaration],
            ) -> Option<(StructuralTypeId, ValueShape, u32)> {
                let declaration_count = declarations.len();
                let declarations = declarations
                    .iter()
                    .map(|declaration| (declaration.id, declaration))
                    .collect::<std::collections::BTreeMap<_, _>>();
                if declarations.len() != declaration_count {
                    return None;
                }
                let mut cache = std::collections::BTreeMap::new();
                let mut active = std::collections::BTreeSet::new();
                let mut total_offset = 0_u32;
                let mut selected_shape = None;
                let mut segments = path.iter();
                while let Some(segment) = segments.next() {
                    let (selected_type, shape, local_offset) = match segment {
                        terminal_psi::StructuralPathSegment::AllElements => return None,
                        terminal_psi::StructuralPathSegment::Case(identity) => {
                            let declaration = declarations.get(&structural_type)?;
                            let (common, cases) = match &declaration.shape {
                                terminal_psi::StructuralTypeShape::Sum { cases } => {
                                    (&[][..], cases)
                                }
                                terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
                                    (fields.as_slice(), cases)
                                }
                                _ => return None,
                            };
                            let mut matching_cases = cases
                                .iter()
                                .enumerate()
                                .filter(|(_, case)| case.id == *identity);
                            let (case_position, case) = matching_cases.next()?;
                            if matching_cases.next().is_some() {
                                return None;
                            }
                            let Some(terminal_psi::StructuralPathSegment::Field(field_identity)) =
                                segments.next()
                            else {
                                return None;
                            };
                            let mut matching_fields = case
                                .fields
                                .iter()
                                .filter(|field| !field.relevance.is_erased())
                                .enumerate()
                                .filter(|(_, field)| field.identity == *field_identity);
                            let (field_position, field) = matching_fields.next()?;
                            if matching_fields.next().is_some() {
                                return None;
                            }
                            let selected = field.field_type.as_structural()?;
                            let layout = replay_sum_layout(
                                common,
                                cases,
                                &declarations,
                                &mut cache,
                                &mut active,
                            )?;
                            let field_offset = layout
                                .cases
                                .get(case_position)?
                                .fields
                                .get(field_position)?
                                .byte_offset;
                            let shape = replay_structural_shape(
                                selected,
                                &declarations,
                                &mut cache,
                                &mut active,
                            )?;
                            (selected, shape, field_offset)
                        }
                        terminal_psi::StructuralPathSegment::Referent
                        | terminal_psi::StructuralPathSegment::RepresentationView { .. }
                        | terminal_psi::StructuralPathSegment::FixedRange { .. }
                        | terminal_psi::StructuralPathSegment::RuntimeIndex { .. } => return None,
                        terminal_psi::StructuralPathSegment::Field(identity) => {
                            let declaration = declarations.get(&structural_type)?;
                            let (fields, common_layout) = match &declaration.shape {
                                terminal_psi::StructuralTypeShape::Record { fields, .. } => {
                                    (fields, None)
                                }
                                terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
                                    let layout = replay_sum_layout(
                                        fields,
                                        cases,
                                        &declarations,
                                        &mut cache,
                                        &mut active,
                                    )?;
                                    (fields, Some(layout))
                                }
                                _ => return None,
                            };
                            let mut field_offset = 0_u32;
                            let mut selected = None;
                            for (field_position, field) in fields
                                .iter()
                                .filter(|field| !field.relevance.is_erased())
                                .enumerate()
                            {
                                if !layout_field(field) {
                                    continue;
                                }
                                let shape = replay_structural_field_shape(
                                    &field.field_type,
                                    &declarations,
                                    &mut cache,
                                    &mut active,
                                )?;
                                field_offset = match &common_layout {
                                    Some(layout) => {
                                        layout.common_fields.get(field_position)?.byte_offset
                                    }
                                    None => {
                                        checked_align_up(field_offset, u32::from(shape.alignment))?
                                    }
                                };
                                if field.identity == *identity {
                                    let nested = field.field_type.as_structural()?;
                                    selected = Some((nested, shape, field_offset));
                                    break;
                                }
                                field_offset = field_offset.checked_add(shape.byte_size)?;
                            }
                            selected?
                        }
                        terminal_psi::StructuralPathSegment::FixedIndex(index) => {
                            let declaration = declarations.get(&structural_type)?;
                            let terminal_psi::StructuralTypeShape::FixedArray {
                                element,
                                length: terminal_psi::ArrayExtent::Literal(length),
                            } = declaration.shape
                            else {
                                return None;
                            };
                            if *index >= length {
                                return None;
                            }
                            let shape = replay_structural_shape(
                                element,
                                &declarations,
                                &mut cache,
                                &mut active,
                            )?;
                            let stride =
                                checked_align_up(shape.byte_size, u32::from(shape.alignment))?;
                            let offset = u64::from(stride)
                                .checked_mul(*index)
                                .and_then(|offset| u32::try_from(offset).ok())?;
                            (element, shape, offset)
                        }
                    };
                    total_offset = total_offset.checked_add(local_offset)?;
                    structural_type = selected_type;
                    selected_shape = Some(shape);
                }
                Some((structural_type, selected_shape?, total_offset))
            }

            pub(crate) fn replay_runtime_projection(
                mut structural_type: StructuralTypeId,
                path: &[terminal_psi::StructuralPathSegment],
                declarations: &[terminal_psi::StructuralTypeDeclaration],
            ) -> Option<(StructuralTypeId, ValueShape, u32, Vec<(u64, u32)>)> {
                let mut total_offset = 0_u32;
                let mut selected_shape = None;
                let mut elements = Vec::new();
                let mut segments = path.iter();
                while let Some(segment) = segments.next() {
                    if let terminal_psi::StructuralPathSegment::RuntimeIndex { .. } = segment {
                        let declaration =
                            terminal_psi::find_structural_type(declarations, structural_type)?;
                        let terminal_psi::StructuralTypeShape::FixedArray {
                            element,
                            length: terminal_psi::ArrayExtent::Literal(length),
                        } = declaration.shape
                        else {
                            return None;
                        };
                        let shape = replay_structural_value_shape(element, declarations)?;
                        let stride = checked_align_up(shape.byte_size, u32::from(shape.alignment))?;
                        elements.push((length, stride));
                        structural_type = element;
                        selected_shape = Some(shape);
                    } else {
                        let pair;
                        let projection =
                            if matches!(segment, terminal_psi::StructuralPathSegment::Case(_)) {
                                pair = [segment.clone(), segments.next()?.clone()];
                                pair.as_slice()
                            } else {
                                std::slice::from_ref(segment)
                            };
                        let (selected_type, shape, local_offset) = replay_structural_projection(
                            structural_type,
                            projection,
                            declarations,
                        )?;
                        total_offset = total_offset.checked_add(local_offset)?;
                        structural_type = selected_type;
                        selected_shape = Some(shape);
                    }
                }
                Some((structural_type, selected_shape?, total_offset, elements))
            }

            fn replay_structural_field_shape(
                field_type: &terminal_psi::StructuralFieldType,
                declarations: &std::collections::BTreeMap<
                    StructuralTypeId,
                    &terminal_psi::StructuralTypeDeclaration,
                >,
                cache: &mut std::collections::BTreeMap<StructuralTypeId, ValueShape>,
                active: &mut std::collections::BTreeSet<StructuralTypeId>,
            ) -> Option<ValueShape> {
                match field_type {
                    terminal_psi::StructuralFieldType::RefinedScalar(refinement) => {
                        replay_structural_field_shape(
                            &terminal_psi::StructuralFieldType::Scalar(refinement.carrier()),
                            declarations,
                            cache,
                            active,
                        )
                    }
                    terminal_psi::StructuralFieldType::Atomic(_)
                    | terminal_psi::StructuralFieldType::AtomicBoundedInteger(_) => {
                        replay_structural_field_shape(
                            &terminal_psi::StructuralFieldType::Scalar(
                                field_type.atomic_scalar_type()?,
                            ),
                            declarations,
                            cache,
                            active,
                        )
                    }
                    terminal_psi::StructuralFieldType::Scalar(ScalarType::Boolean) => {
                        Some(ValueShape::integer(1, 1))
                    }
                    terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
                        let size = integer.bits().div_ceil(8);
                        Some(ValueShape::integer(
                            u32::from(size),
                            size.next_power_of_two().min(16),
                        ))
                    }
                    terminal_psi::StructuralFieldType::BoundedInteger(bounds) => {
                        let integer = bounds.integer_type();
                        let size = integer.bits().div_ceil(8);
                        Some(ValueShape::integer(
                            u32::from(size),
                            size.next_power_of_two().min(16),
                        ))
                    }
                    terminal_psi::StructuralFieldType::Scalar(ScalarType::IeeeFloat(
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    )) => Some(ValueShape::float(4)),
                    terminal_psi::StructuralFieldType::Scalar(ScalarType::IeeeFloat(
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    )) => Some(ValueShape::float(8)),
                    terminal_psi::StructuralFieldType::IeeeFloat(
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ) => Some(ValueShape::float(4)),
                    terminal_psi::StructuralFieldType::IeeeFloat(
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ) => Some(ValueShape::float(8)),
                    terminal_psi::StructuralFieldType::ByteSequence(carrier)
                    | terminal_psi::StructuralFieldType::PredicateByteSequence {
                        carrier, ..
                    } => {
                        let byte_size: u32 = match carrier {
                            terminal_psi::ByteSequenceCarrier::BorrowedView { .. } => 16,
                            terminal_psi::ByteSequenceCarrier::BoundedView { .. } => 8,
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity } => {
                                capacity.checked_add(8)?.try_into().ok()?
                            }
                        };
                        Some(ValueShape::integer(byte_size, 8))
                    }
                    terminal_psi::StructuralFieldType::Structural(nested) => {
                        replay_structural_shape(*nested, declarations, cache, active)
                    }
                    terminal_psi::StructuralFieldType::Erased { .. } => None,
                }
            }

            #[cfg(test)]
            mod tests {
                use super::{
                    replay_runtime_projection, replay_structural_projection,
                    replay_structural_value_shape,
                };
                use crate::object::object_artifact::test_support::structural_declaration;
                use calling_conventions::ValueShape;
                use semantic_vocabulary::{
                    IeeeFloatFormat, ObligationId, ScalarType, StructuralCaseId, StructuralFieldId,
                    StructuralTypeId, ValueId,
                };
                use terminal_psi::{
                    BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration,
                    StructuralFieldType, StructuralPathSegment, StructuralTypeShape,
                };

                #[test]
                fn case_projection_replay_preserves_sum_and_mixed_backing_geometry() {
                    let root = StructuralTypeId::new(1).unwrap();
                    let sum = StructuralTypeId::new(2).unwrap();
                    let leaf = StructuralTypeId::new(3).unwrap();
                    let present = StructuralCaseId::new(21).unwrap();
                    let absent = StructuralCaseId::new(22).unwrap();
                    let field = |raw, identity: &str, field_type| StructuralFieldDeclaration {
                        inline_carrier: None,
                        storage: None,
                        id: StructuralFieldId::new(raw).unwrap(),
                        identity: terminal_psi::TerminalIdentityIndex(match identity {
                            "common" => 1,
                            "item" => 2,
                            "value" => 3,
                            _ => 0,
                        }),
                        relevance: BindingRelevance::Relevant,
                        field_type,
                    };
                    let scalar = StructuralFieldType::Scalar(ScalarType::IeeeFloat(
                        IeeeFloatFormat::Binary64,
                    ));
                    for (mixed, stride, payload_offset) in [(false, 16, 8), (true, 24, 16)] {
                        let cases = vec![
                            StructuralCaseDeclaration {
                                id: present,
                                identity: terminal_psi::TerminalIdentityIndex(1),
                                fields: vec![field(
                                    31,
                                    "item",
                                    StructuralFieldType::Structural(leaf),
                                )],
                            },
                            StructuralCaseDeclaration {
                                id: absent,
                                identity: terminal_psi::TerminalIdentityIndex(2),
                                fields: Vec::new(),
                            },
                        ];
                        let mut types = vec![
                            structural_declaration(
                                root,
                                terminal_psi::TerminalIdentityIndex(3),
                                StructuralTypeShape::FixedArray {
                                    element: sum,
                                    length: terminal_psi::ArrayExtent::Literal(2),
                                },
                            ),
                            structural_declaration(
                                sum,
                                terminal_psi::TerminalIdentityIndex(4),
                                if mixed {
                                    StructuralTypeShape::Mixed {
                                        fields: vec![field(33, "common", scalar.clone())],
                                        cases,
                                    }
                                } else {
                                    StructuralTypeShape::Sum { cases }
                                },
                            ),
                            structural_declaration(
                                leaf,
                                terminal_psi::TerminalIdentityIndex(5),
                                StructuralTypeShape::Record {
                                    storage: terminal_psi::StructuralRecordStorage::Packed,
                                    fields: vec![field(32, "value", scalar.clone())],
                                },
                            ),
                        ];
                        let path = vec![
                            StructuralPathSegment::FixedIndex(1),
                            StructuralPathSegment::Case(present),
                            StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(2)),
                        ];
                        assert_eq!(
                            replay_structural_value_shape(root, &types),
                            Some(ValueShape::integer(stride * 2, 8))
                        );
                        assert_eq!(
                            replay_structural_projection(root, &path, &types),
                            Some((leaf, ValueShape::integer(8, 8), stride + payload_offset))
                        );
                        let mut runtime = path.clone();
                        runtime[0] = StructuralPathSegment::RuntimeIndex {
                            index: ValueId::new(41).unwrap(),
                            obligation: ObligationId::new(42).unwrap(),
                        };
                        assert_eq!(
                            replay_runtime_projection(root, &runtime, &types),
                            Some((
                                leaf,
                                ValueShape::integer(8, 8),
                                payload_offset,
                                vec![(2, stride)]
                            ))
                        );
                        for invalid in [
                            vec![
                                StructuralPathSegment::FixedIndex(2),
                                StructuralPathSegment::Case(present),
                                StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(
                                    2,
                                )),
                            ],
                            vec![
                                StructuralPathSegment::FixedIndex(1),
                                StructuralPathSegment::Case(absent),
                                StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(
                                    2,
                                )),
                            ],
                            vec![
                                StructuralPathSegment::FixedIndex(1),
                                StructuralPathSegment::Case(StructuralCaseId::new(99).unwrap()),
                                StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(
                                    2,
                                )),
                            ],
                            vec![
                                StructuralPathSegment::FixedIndex(1),
                                StructuralPathSegment::Case(present),
                            ],
                            vec![
                                StructuralPathSegment::FixedIndex(1),
                                StructuralPathSegment::Case(present),
                                StructuralPathSegment::FixedIndex(0),
                            ],
                            vec![
                                StructuralPathSegment::FixedIndex(1),
                                StructuralPathSegment::Case(present),
                                StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(
                                    7,
                                )),
                            ],
                        ] {
                            assert!(replay_structural_projection(root, &invalid, &types).is_none());
                        }
                        assert!(replay_structural_projection(leaf, &path[1..], &types).is_none());
                        assert!(replay_structural_projection(root, &path[1..], &types).is_none());
                        let mut duplicate = types.clone();
                        duplicate.push(types[1].clone());
                        assert!(replay_structural_projection(root, &path, &duplicate).is_none());
                        let cases = match &mut types[1].shape {
                            StructuralTypeShape::Sum { cases }
                            | StructuralTypeShape::Mixed { cases, .. } => cases,
                            _ => unreachable!(),
                        };
                        cases[0].fields[0].relevance = BindingRelevance::Erased;
                        assert!(replay_structural_projection(root, &path, &types).is_none());
                        assert!(replay_runtime_projection(root, &runtime, &types).is_none());
                    }
                }
            }
        }
    }
}

pub(crate) mod stack_demand {
    use target::{Architecture, NativeTarget};
    use terminal_psi::TerminalPsiIdentity;

    use semantic_vocabulary::MachineId;

    use crate::{ObjectArtifact, ObjectError, ObjectFunction};

    pub fn derive_stack_demand(
        artifact: &ObjectArtifact,
        entry: MachineId,
    ) -> Result<StackDemand, ObjectError> {
        let mut demand = derive_body_stack_demand(artifact, entry)?;
        if let Some(binding) = artifact.physical_entry_binding()
            && binding.entry() == entry
        {
            demand.ceiling_bytes = demand
                .ceiling_bytes
                .checked_add(binding.stack_overhead_bytes())
                .ok_or(ObjectError::PhysicalEntryStackArithmeticOverflow(entry))?;
        }
        Ok(demand)
    }

    pub(crate) fn derive_body_stack_demand(
        artifact: &ObjectArtifact,
        entry: MachineId,
    ) -> Result<StackDemand, ObjectError> {
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
    ) -> Result<StackDemand, ObjectError> {
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
                .ok_or(ObjectError::InvalidPrivateFunctionIdentity)?;
            if active_private.contains(&identity) {
                return Err(ObjectError::TerminalStackCycle(private.body.entry));
            }
            active_private.push(identity);
            let mut demand = derive_namespace_stack_demand(
                &private.body,
                private.body.entry,
                private_functions,
                active_private,
            )?;
            if let Some(bridge) = &private.entry_bridge {
                demand.ceiling_bytes = demand
                    .ceiling_bytes
                    .checked_add(u64::from(bridge.frame_bytes()))
                    .and_then(|bytes| bytes.checked_add(u64::from(bridge.call_stack_bias())))
                    .ok_or(ObjectError::PrivateEntryStackArithmeticOverflow(identity))?;
            }
            active_private.pop();
            private_demands.push((identity, demand));
        }
        let functions = artifact
            .functions
            .iter()
            .map(|function| (function.machine, function))
            .collect::<std::collections::BTreeMap<_, _>>();
        if !functions.contains_key(&entry) {
            return Err(ObjectError::EntryFunctionMissing(entry));
        }
        let stackless_nonreturning = functions
            .values()
            .filter(|function| function.unit_stack.is_none() && function.scalar_stack.is_none())
            .filter(|function| {
                let mut matching = artifact
                    .boundary_settlements
                    .iter()
                    .filter(|row| row.machine == function.machine);
                let Some(row) = matching.next() else {
                    return false;
                };
                matching.next().is_none()
                    && matches!(
                        row.settlement.realization,
                        target_operations::BoundaryRealization::HostedExitProcessI32(_)
                    )
                    && row.settlement.code_offset == 0
                    && row.settlement.byte_count == function.byte_count
            })
            .map(|function| function.machine)
            .collect::<std::collections::BTreeSet<_>>();
        let mut active = std::collections::BTreeSet::new();
        let mut memoized = std::collections::BTreeMap::new();
        let mut contributing_machines = std::collections::BTreeSet::new();
        let mut admitted_contribution_report_identities = std::collections::BTreeSet::new();
        let mut admitted_contribution_commitments = std::collections::BTreeSet::new();
        let ceiling_bytes = derive_terminal_stack_peak(
            entry,
            artifact,
            &functions,
            &mut active,
            &mut memoized,
            &mut contributing_machines,
            &mut admitted_contribution_report_identities,
            &mut admitted_contribution_commitments,
            &stackless_nonreturning,
            &private_demands,
        )?;
        Ok(StackDemand {
            psi: artifact.psi,
            target: artifact.target,
            entry,
            ceiling_bytes,
            stack_alignment: 16,
            contributing_machines,
            admitted_contribution_report_identities,
            admitted_contribution_commitments,
            private_demands,
        })
    }

    fn derive_terminal_stack_peak(
        machine: MachineId,
        artifact: &ObjectArtifact,
        functions: &std::collections::BTreeMap<MachineId, &ObjectFunction>,
        active: &mut std::collections::BTreeSet<MachineId>,
        memoized: &mut std::collections::BTreeMap<MachineId, u64>,
        contributing_machines: &mut std::collections::BTreeSet<MachineId>,
        admitted_contribution_report_identities: &mut std::collections::BTreeSet<
            task_plans::AdmittedStackContributionReportId,
        >,
        admitted_contribution_commitments: &mut std::collections::BTreeSet<
            task_plans::SameStackContributionCommitment,
        >,
        stackless_nonreturning: &std::collections::BTreeSet<MachineId>,
        private_demands: &[(function_identity::MachineFunctionIdentity, StackDemand)],
    ) -> Result<u64, ObjectError> {
        if let Some(peak) = memoized.get(&machine) {
            contributing_machines.insert(machine);
            return Ok(*peak);
        }
        if !active.insert(machine) {
            return Err(ObjectError::TerminalStackCycle(machine));
        }
        contributing_machines.insert(machine);
        let function =
            functions
                .get(&machine)
                .copied()
                .ok_or(ObjectError::UnknownInternalCallTarget {
                    caller: machine,
                    target: machine,
                })?;
        let mut peak = match (function.unit_stack, function.scalar_stack) {
            (Some(_), Some(_)) => {
                return Err(ObjectError::ConflictingTerminalStackEvidence(machine));
            }
            (Some(stack), None) => u64::from(stack.local_peak_bytes),
            (None, Some(stack)) => u64::from(stack.local_peak_bytes),
            (None, None) if stackless_nonreturning.contains(&machine) => 0,
            (None, None) => return Err(ObjectError::UnaccountedTerminalStack(machine)),
        };
        let calls = function
            .unit_call_stacks
            .iter()
            .map(|call| (call.target, call.owner, call.caller_live_bytes))
            .chain(
                function
                    .scalar_call_stacks
                    .iter()
                    .map(|call| (call.target, call.owner, call.caller_live_bytes)),
            );
        for (target, owner, caller_live_bytes) in calls {
            let callee_peak = derive_terminal_stack_peak(
                target,
                artifact,
                functions,
                active,
                memoized,
                contributing_machines,
                admitted_contribution_report_identities,
                admitted_contribution_commitments,
                stackless_nonreturning,
                private_demands,
            )?;
            let composed = u64::from(caller_live_bytes)
                .checked_add(callee_peak)
                .ok_or(ObjectError::TerminalStackCompositionOverflow {
                    caller: machine,
                    owner,
                })?;
            peak = peak.max(composed);
        }
        for call in artifact
            .foreign_calls
            .iter()
            .filter(|call| call.machine == machine)
        {
            let Some(contribution) = call.binding.same_stack_contribution() else {
                peak = peak.max(u64::from(call.caller_live_bytes));
                continue;
            };
            let callback_peak = call
                .callback_addresses
                .iter()
                .filter(|callback| callback.target.registrar_invokes_callback)
                .try_fold(0, |peak, callback| {
                    let (_, demand) = private_demands
                        .iter()
                        .find(|(identity, _)| *identity == callback.target.callback_function)
                        .ok_or(ObjectError::InvalidPrivateFunctionIdentity)?;
                    admitted_contribution_report_identities
                        .extend(&demand.admitted_contribution_report_identities);
                    admitted_contribution_commitments
                        .extend(&demand.admitted_contribution_commitments);
                    Ok::<u64, ObjectError>(peak.max(demand.ceiling_bytes))
                })?;
            let composed = u64::from(call.caller_live_bytes)
                .checked_add(contribution.bytes())
                .and_then(|bytes| bytes.checked_add(callback_peak))
                .ok_or(ObjectError::TerminalStackCompositionOverflow {
                    caller: machine,
                    owner: call.owner,
                })?;
            peak = peak.max(composed);
            admitted_contribution_report_identities.insert(contribution.report_identity());
            admitted_contribution_commitments.insert(contribution.commitment());
        }
        active.remove(&machine);
        memoized.insert(machine, peak);
        Ok(peak)
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct StackDemand {
        pub(crate) psi: TerminalPsiIdentity,
        pub(crate) target: NativeTarget,
        pub(crate) entry: MachineId,
        pub(crate) ceiling_bytes: u64,
        pub(crate) stack_alignment: u32,
        pub(crate) contributing_machines: std::collections::BTreeSet<MachineId>,
        pub(crate) admitted_contribution_report_identities:
            std::collections::BTreeSet<task_plans::AdmittedStackContributionReportId>,
        pub(crate) admitted_contribution_commitments:
            std::collections::BTreeSet<task_plans::SameStackContributionCommitment>,
        pub(crate) private_demands: Vec<(function_identity::MachineFunctionIdentity, StackDemand)>,
    }

    impl StackDemand {
        pub const fn psi(&self) -> TerminalPsiIdentity {
            self.psi
        }

        pub const fn target(&self) -> NativeTarget {
            self.target
        }

        pub const fn entry(&self) -> MachineId {
            self.entry
        }

        pub const fn ceiling_bytes(&self) -> u64 {
            self.ceiling_bytes
        }

        pub const fn stack_alignment(&self) -> u32 {
            self.stack_alignment
        }

        pub const fn contributing_machines(&self) -> &std::collections::BTreeSet<MachineId> {
            &self.contributing_machines
        }

        pub const fn admitted_contribution_report_identities(
            &self,
        ) -> &std::collections::BTreeSet<task_plans::AdmittedStackContributionReportId> {
            &self.admitted_contribution_report_identities
        }

        pub const fn admitted_contribution_commitments(
            &self,
        ) -> &std::collections::BTreeSet<task_plans::SameStackContributionCommitment> {
            &self.admitted_contribution_commitments
        }
    }

    impl installation_evidence::StackDemandEvidence for StackDemand {
        fn psi(&self) -> TerminalPsiIdentity {
            self.psi
        }

        fn architecture(&self) -> Architecture {
            self.target.architecture
        }

        fn entry(&self) -> MachineId {
            self.entry
        }

        fn ceiling_bytes(&self) -> u64 {
            self.ceiling_bytes
        }

        fn stack_alignment(&self) -> u32 {
            self.stack_alignment
        }

        fn contributing_machines(&self) -> &std::collections::BTreeSet<MachineId> {
            &self.contributing_machines
        }

        fn admitted_stack_contribution_report_identities(&self) -> std::collections::BTreeSet<u64> {
            self.admitted_contribution_report_identities
                .iter()
                .map(|identity| identity.normalized_identity())
                .collect()
        }

        fn admitted_stack_contribution_commitments(&self) -> std::collections::BTreeSet<[u8; 32]> {
            self.admitted_contribution_commitments
                .iter()
                .map(|commitment| commitment.as_bytes())
                .collect()
        }
    }
}

pub use carriers::{
    NativeInstructionSourceRows, ObjectArtifact, ObjectBoundarySettlement, ObjectCodeAttribution,
    ObjectForeignCall, ObjectFunction, ObjectPrivateFunctionEntry, ObjectScalarCallStack,
    ObjectUnitCallStack, PrivateFunctionObject, PrivateObjectSymbolBinding,
};
pub(crate) use carriers::{
    ObjectByteLiteralAddress, ObjectByteLiteralPool, ObjectCompilerPrivateFunction,
    ObjectPortEffect, ObjectScalarStack, ObjectUnitStack, find_object_function,
};
pub(crate) use errors::ObjectError;
pub(crate) use normalized_foreign_calls::image_foreign_calls_match_object;
pub use normalized_foreign_calls::{
    derive_normalized_foreign_call_custody, durable_home_offsets,
    foreign_invocation_provenance_matches, foreign_result_operand_count, fragment_instruction_span,
    producer_instruction, scalar_call_result_producer, scalar_immediate_span,
    structural_result_capture, transported_span_end, unique_block_instruction,
};

#[cfg(test)]
pub(crate) mod test_support {
    pub(crate) fn structural_declaration(
        id: semantic_vocabulary::StructuralTypeId,
        identity: terminal_psi::TerminalIdentityIndex,
        shape: terminal_psi::StructuralTypeShape,
    ) -> terminal_psi::StructuralTypeDeclaration {
        terminal_psi::StructuralTypeDeclaration {
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id,
            identity,
            shape,
        }
    }
}
