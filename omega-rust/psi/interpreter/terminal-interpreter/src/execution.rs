//! The execution engine: the live execution state, executable machines,
//! suspended calls, runtime descriptors and claims, and the interpreter loop.
//! `resume_with_effect_handler` charges and dispatches each operation to its
//! `execute_*` method (`integer_operations`, `boolean_float_operations`,
//! `call_operations`, `structural_operations` and the earlier owners), then
//! settles the block terminator through its `settle_*` method (`terminators`).

use crate::byte_sequences::ByteSequenceView;
use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::custody::{
    bind_affine_frontier, bind_arguments, bind_entry_claims, bind_structural_arguments,
    bind_structural_primitive_values,
};
use crate::effects::{
    AdmittedAssumedBinding, AdmittedProviderInstallation, TerminalEffect, TerminalEffectHandler,
};
use crate::errors::TerminalInterpretError;
use crate::results::meter_status;
use crate::results::{TerminalCrash, TerminalExecutionResult, TerminalExecutionStatus};
use crate::scalar_array::TerminalScalarArrayValue;
use crate::structural_identities::LocalStructuralIdentities;
use crate::structural_inputs::case_membership::StructuralCaseContents;
use crate::structural_inputs::placed_views::{
    PlacedViewOccurrence, TerminalPlacedViewEstablishment, establish_placed_view_inputs,
};
use crate::structural_inputs::scalar_fields;
use crate::structural_inputs::scalar_fields::TerminalStructuralScalarFieldValue;
use crate::values::{
    StructuralByteSequenceRuntimeField, StructuralRuntimePlace, StructuralScalarRuntimeField,
};
use crate::values::{
    TerminalScalarCaseValue, TerminalScalarValue, TerminalStructuralPrimitiveValue,
    TerminalStructuralValue,
};
#[cfg(test)]
use semantic_vocabulary::StructuralTypeId;
use semantic_vocabulary::{BlockId, BoundaryMachineId, ClaimId, MachineId, PlaceId, ValueId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_fuel::TerminalFuelMeter;
#[cfg(test)]
use terminal_psi::StructuralTypeDeclaration;
use terminal_psi::{
    Block, EntryClaim, NominalAffineCleanup, OperationKind, StructuralAffineDiscard,
    StructuralArgument, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralResultClaimTransfer,
    TerminalMachineResult, TerminalPlacedViewInput, Terminator,
};

/// What one settled terminator tells the interpreter loop to do next.
pub(crate) enum TerminatorFlow {
    /// The block transferred control; dispatch from the new current block.
    Continue,
    /// Execution stops here with this status.
    Yield(TerminalExecutionStatus),
}

/// What one executed operation tells the interpreter loop to do next.
pub(crate) enum OperationFlow {
    /// The operation completed; advance to the next operation.
    Advance,
    /// A call replaced the current frame; dispatch from the callee's entry.
    Redispatch,
    /// Execution stops here with this status.
    Yield(TerminalExecutionStatus),
}

/// Resumable execution state created from canonical terminal-Psi artifact
/// sections.
///
/// Fuel exhaustion never advances `next_operation` or the current terminator,
/// so a sponsor can replenish the same meter and resume without replaying
/// semantic work or charging it twice.
pub struct TerminalExecution {
    // One immutable program owner survives suspension and is shared verbatim
    // across every independent instance admitted from the same module. A
    // resume or call borrows this owner independently of mutable runtime
    // storage; frames retain only machine identity. There is no per-machine
    // or per-operation Arc allocation.
    pub(crate) program: std::sync::Arc<crate::program::AdmittedTerminalProgram>,
    pub(crate) dynamic_parameters: BTreeMap<u32, RuntimeDynamicDescriptor>,
    pub(crate) provider_installation: BTreeMap<BoundaryMachineId, MachineId>,
    /// Root-authorized assumed binding contracts admitted with the provider
    /// installation. They are conditional evidence on this execution, not
    /// checked redirects: call custody keeps consulting only
    /// `provider_candidates` and `provider_installation`, so a direct call of
    /// either requirement gains no assumed facts.
    pub(crate) assumed_bindings: Vec<AdmittedAssumedBinding>,
    pub(crate) values: BTreeMap<ValueId, TerminalScalarValue>,
    pub(crate) structural_values: BTreeMap<PlaceId, TerminalStructuralValue>,
    /// A reference carrier owns this descriptor, never the referent's backing.
    /// Carrier identities survive structural call/return moves across frames.
    pub(crate) reference_referents: BTreeMap<StructuralRuntimePlace, TerminalStructuralValue>,
    /// Mutable primitive contents live outside call frames. Machine-local
    /// place maps are only views into this stable logical storage arena.
    pub(crate) structural_primitive_storage: BTreeMap<StructuralRuntimePlace, TerminalScalarValue>,
    pub(crate) structural_primitive_entry_places: BTreeMap<u32, StructuralRuntimePlace>,
    pub(crate) local_structural_identities: LocalStructuralIdentities,
    /// Scalar leaves written below aggregate structural values. Keys use the
    /// invocation-independent opaque identity and resolved parent path, so a
    /// projected call observes the same field without native layout claims.
    pub(crate) structural_scalar_fields:
        BTreeMap<StructuralScalarRuntimeField, TerminalScalarValue>,
    /// Owned byte contents are keyed by referent identity, not a callee-local
    /// parameter. Immutable backing implements a logical copy of the live prefix.
    pub(crate) structural_byte_sequence_fields:
        BTreeMap<StructuralByteSequenceRuntimeField, ByteSequenceView>,
    /// Stored element descriptors retain their original backing and window.
    /// Their field homes survive borrowed calls independently of frame-local IDs.
    pub(crate) structural_element_view_fields:
        BTreeMap<StructuralRuntimePlace, crate::element_views::ElementView>,
    pub(crate) structural_byte_arrays: BTreeMap<StructuralRuntimePlace, ByteSequenceView>,
    /// Entry-provided discriminators remain attached to the original referent
    /// across projected and forwarded calls, independent of machine-local IDs.
    pub(crate) structural_cases: BTreeMap<StructuralRuntimePlace, StructuralCaseContents>,
    pub(crate) scalar_case_values: BTreeMap<PlaceId, TerminalScalarCaseValue>,
    pub(crate) scalar_array_values: BTreeMap<PlaceId, TerminalScalarArrayValue>,
    /// Frame-local immutable descriptors or exact boundary-introduced mutable
    /// field loans, rebound to callee parameters. Opaque identities alone do
    /// not supply either byte contents or permission to mutate a field.
    pub(crate) byte_sequence_values: BTreeMap<PlaceId, ByteSequenceBinding>,
    /// Shared immutable element views rebound to callee parameters, parallel
    /// to `byte_sequence_values` for borrowed `&[T]` storage.
    pub(crate) element_view_values: BTreeMap<PlaceId, crate::element_views::ElementView>,
    /// Exact claim-free affine ownership frontier. Opaque structural storage is
    /// root-addressed, so projected moves must be represented here rather than
    /// by unsoundly deleting their containing root.
    pub(crate) live_affine_frontier: BTreeSet<StructuralAffineDiscard>,
    /// Live placed-view occurrences: each direct-entry roster row's lent
    /// referent, bound for the entry invocation's duration. Retirement at
    /// completion releases the loans without reminting custody.
    pub(crate) placed_view_occurrences: BTreeMap<TerminalPlacedViewInput, PlacedViewOccurrence>,
    pub(crate) live_claims: BTreeMap<ClaimId, LiveClaim>,
    pub(crate) current_machine: MachineId,
    pub(crate) current: BlockId,
    pub(crate) next_operation: usize,
    pub(crate) call_stack: Vec<SuspendedCall>,
    pub(crate) result: Option<TerminalExecutionResult>,
    pub(crate) crash: Option<TerminalCrash>,
    pub(crate) effects: Vec<TerminalEffect>,
}

#[derive(Clone)]
pub(crate) struct ExecutableMachine {
    pub(crate) parameters: Vec<terminal_psi::ValueDeclaration>,
    pub(crate) structural_parameters: Vec<StructuralParameterDeclaration>,
    pub(crate) structural_places: Vec<terminal_psi::StructuralPlaceDeclaration>,
    pub(crate) entry_claims: Vec<EntryClaim>,
    pub(crate) content_entry_claims: Vec<terminal_psi::ContentEntryClaim>,
    pub(crate) result: TerminalMachineResult,
    pub(crate) entry: BlockId,
    pub(crate) blocks: BTreeMap<BlockId, Block>,
}

pub(crate) struct SuspendedCall {
    pub(crate) values: BTreeMap<ValueId, TerminalScalarValue>,
    pub(crate) structural_values: BTreeMap<PlaceId, TerminalStructuralValue>,
    pub(crate) scalar_case_values: BTreeMap<PlaceId, TerminalScalarCaseValue>,
    pub(crate) scalar_array_values: BTreeMap<PlaceId, TerminalScalarArrayValue>,
    pub(crate) byte_sequence_values: BTreeMap<PlaceId, ByteSequenceBinding>,
    pub(crate) element_view_values: BTreeMap<PlaceId, crate::element_views::ElementView>,
    pub(crate) live_affine_frontier: BTreeSet<StructuralAffineDiscard>,
    pub(crate) live_claims: BTreeMap<ClaimId, LiveClaim>,
    pub(crate) dynamic_parameters: BTreeMap<u32, RuntimeDynamicDescriptor>,
    pub(crate) current_machine: MachineId,
    pub(crate) current: BlockId,
    pub(crate) next_operation: usize,
    pub(crate) result: SuspendedCallResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeDynamicDescriptorTemplate {
    pub(crate) source: StructuralArgument,
    /// Positional callable table parallel to the selection's conformance
    /// application rows. A lane that materializes only its selected
    /// realization leaves the other roster rows unbound evidence, so entries
    /// are sparse; verified descriptor-capable selections bind every row.
    pub(crate) callables: Vec<Option<MachineId>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeDynamicDescriptor {
    pub(crate) source: TerminalStructuralValue,
    /// See [`RuntimeDynamicDescriptorTemplate::callables`].
    pub(crate) callables: Vec<Option<MachineId>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiveClaim {
    pub(crate) place: Option<PlaceId>,
    pub(crate) path: Vec<StructuralPathSegment>,
    pub(crate) multiplicity: Option<StructuralMultiplicity>,
}

pub(crate) enum SuspendedCallResult {
    Scalar(ValueId),
    Unit,
    Structural {
        result: StructuralOperationResult,
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
        expected_reference_backings: Vec<TerminalStructuralValue>,
    },
    NominalCleanups {
        completed: (NominalAffineCleanup, TerminalStructuralValue),
        remaining: Vec<(NominalAffineCleanup, TerminalStructuralValue)>,
        final_result: Option<TerminalScalarValue>,
    },
}

impl TerminalExecution {
    /// Admit a freshly verified module and start its first execution. Unit
    /// tests that construct `TerminalModule`s by hand enter through here;
    /// the public starts admit through `TerminalProgram` instead.
    #[cfg(test)]
    pub(crate) fn start_verified_module(
        module: terminal_psi::TerminalModule,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[TerminalStructuralValue],
        structural_scalar_field_arguments: &[TerminalStructuralScalarFieldValue],
        structural_primitive_value_arguments: &[TerminalStructuralPrimitiveValue],
        placed_view_establishments: &[TerminalPlacedViewEstablishment],
        installation: Option<&AdmittedProviderInstallation>,
    ) -> Result<Self, TerminalInterpretError> {
        Self::start_with_program(
            std::sync::Arc::new(crate::program::AdmittedTerminalProgram::admit(module)?),
            scalar_arguments,
            structural_arguments,
            structural_scalar_field_arguments,
            structural_primitive_value_arguments,
            placed_view_establishments,
            installation,
        )
    }

    /// Start one instance on an already-admitted program: bind this
    /// invocation's inputs and custody, commit no operation. The program is
    /// data — no decode or verification repeats here.
    pub(crate) fn start_with_program(
        program: std::sync::Arc<crate::program::AdmittedTerminalProgram>,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[TerminalStructuralValue],
        structural_scalar_field_arguments: &[TerminalStructuralScalarFieldValue],
        structural_primitive_value_arguments: &[TerminalStructuralPrimitiveValue],
        placed_view_establishments: &[TerminalPlacedViewEstablishment],
        installation: Option<&AdmittedProviderInstallation>,
    ) -> Result<Self, TerminalInterpretError> {
        // A nonempty placed-view roster declares direct entry inputs whose
        // referent custody must be established beside the ordinary inputs: one
        // exact establishment per entry row lends the qualified backing for
        // the invocation's duration. A row left unsupplied, a supply that
        // answers no declared row, and any roster row on a non-entry machine
        // all fail closed rather than start the entry machine with a declared
        // input unbound.
        let placed_view_occurrences = establish_placed_view_inputs(
            &program.catalogs,
            placed_view_establishments,
            structural_arguments,
        )?;
        if installation.is_some_and(|installation| installation.terminal_psi != program.identity) {
            return Err(TerminalInterpretError::ProviderInstallationIdentityMismatch);
        }
        let local_structural_identities = LocalStructuralIdentities::new(
            program
                .machines
                .values()
                .flat_map(|machine| machine.structural_places.iter()),
            structural_arguments,
        );
        let machine = program
            .machines
            .get(&program.catalogs.entry)
            .ok_or(TerminalInterpretError::VerifiedEntryMachineMissing)?;
        let current = machine.entry;
        let current_machine = program.catalogs.entry;
        if machine
            .parameters
            .iter()
            .any(|parameter| !parameter.qualifications.is_empty())
        {
            return Err(TerminalInterpretError::ScalarEntryQualificationUnsupported);
        }
        let values = bind_arguments(&machine.parameters, scalar_arguments)?;
        let structural_values =
            bind_structural_arguments(&machine.structural_parameters, structural_arguments)?;
        let (structural_primitive_storage, structural_primitive_entry_places) =
            bind_structural_primitive_values(
                machine,
                &program.structural_types,
                &structural_values,
                structural_primitive_value_arguments,
            )?;
        let structural_scalar_fields = scalar_fields::bind(
            machine,
            &program.structural_types,
            &structural_values,
            structural_scalar_field_arguments,
        )?;
        let live_affine_frontier =
            bind_affine_frontier(&machine.structural_parameters, &structural_values)?;
        let live_claims = bind_entry_claims(
            &machine.entry_claims,
            &machine.content_entry_claims,
            &machine.structural_parameters,
            &structural_values,
        )?;
        Ok(Self {
            program,
            dynamic_parameters: BTreeMap::new(),
            provider_installation: installation
                .map(|installation| installation.installed.clone())
                .unwrap_or_default(),
            assumed_bindings: installation
                .map(|installation| installation.assumed_bindings.clone())
                .unwrap_or_default(),
            values,
            structural_values,
            reference_referents: BTreeMap::new(),
            structural_element_view_fields: BTreeMap::new(),
            structural_primitive_storage,
            structural_primitive_entry_places,
            local_structural_identities,
            structural_scalar_fields,
            structural_byte_sequence_fields: BTreeMap::new(),
            structural_byte_arrays: BTreeMap::new(),
            structural_cases: BTreeMap::new(),
            scalar_case_values: BTreeMap::new(),
            scalar_array_values: BTreeMap::new(),
            byte_sequence_values: BTreeMap::new(),
            element_view_values: BTreeMap::new(),
            live_affine_frontier,
            live_claims,
            placed_view_occurrences,
            current_machine,
            current,
            next_operation: 0,
            call_stack: Vec::new(),
            result: None,
            crash: None,
            effects: Vec::new(),
        })
    }

    pub fn effects(&self) -> &[TerminalEffect] {
        &self.effects
    }

    /// Whether the entry invocation has reached a terminal outcome — a
    /// committed result or a crash. An unfinished execution still holds
    /// custody; it is not reclaimable by `TerminalInstance::close`.
    pub(crate) fn is_finished(&self) -> bool {
        self.result.is_some() || self.crash.is_some()
    }

    /// The committed result once the entry invocation has produced one.
    pub(crate) fn result(&self) -> Option<&TerminalExecutionResult> {
        self.result.as_ref()
    }

    /// Test hook: apply one corruption to a structural type in both retained
    /// copies — the lookup map the interpreter loop reads and the slim
    /// catalog module placed-view validation consults. Only sound while this
    /// execution is the program's sole owner.
    #[cfg(test)]
    pub(crate) fn mutate_structural_type(
        &mut self,
        id: StructuralTypeId,
        mutation: impl Fn(&mut StructuralTypeDeclaration),
    ) {
        let program =
            std::sync::Arc::get_mut(&mut self.program).expect("test mutates before sharing");
        let mut matched = false;
        if let Some(declaration) = program.structural_types.get_mut(&id) {
            mutation(declaration);
            matched = true;
        }
        if let Some(declaration) = program
            .catalogs
            .structural_types
            .iter_mut()
            .find(|declaration| declaration.id == id)
        {
            mutation(declaration);
            matched = true;
        }
        assert!(matched, "mutate_structural_type: unknown structural type");
    }

    /// Test hook: insert or replace one structural type in both retained
    /// copies — the lookup map and the slim catalog module. Only sound while
    /// this execution is the program's sole owner.
    #[cfg(test)]
    pub(crate) fn insert_structural_type(&mut self, declaration: StructuralTypeDeclaration) {
        let program =
            std::sync::Arc::get_mut(&mut self.program).expect("test mutates before sharing");
        if let Some(existing) = program
            .catalogs
            .structural_types
            .iter_mut()
            .find(|existing| existing.id == declaration.id)
        {
            *existing = declaration.clone();
        } else {
            program.catalogs.structural_types.push(declaration.clone());
        }
        program.structural_types.insert(declaration.id, declaration);
    }

    /// The root-authorized assumed binding contracts admitted with this
    /// execution's provider installation, in authored policy order. These
    /// are conditional evidence retained across the run: the artifact's
    /// guarantees hold under each recorded assumption, and the
    /// assumption's falsity invalidates them without producing any
    /// unconditional claim.
    pub fn assumed_bindings(&self) -> &[AdmittedAssumedBinding] {
        &self.assumed_bindings
    }

    /// Final values of direct primitive structural entry arguments, ordered by
    /// their dense structural-argument positions.
    pub fn structural_primitive_values(&self) -> Vec<TerminalStructuralPrimitiveValue> {
        self.final_structural_primitive_values()
    }

    pub(crate) fn final_structural_primitive_values(
        &self,
    ) -> Vec<TerminalStructuralPrimitiveValue> {
        self.structural_primitive_entry_places
            .iter()
            .filter_map(|(argument_index, place)| {
                self.structural_primitive_storage
                    .get(place)
                    .copied()
                    .map(|value| TerminalStructuralPrimitiveValue {
                        argument_index: *argument_index,
                        value,
                    })
            })
            .collect()
    }

    pub fn live_claim_frontier(&self) -> impl Iterator<Item = ClaimId> + '_ {
        self.live_claims.keys().copied()
    }

    /// Exact live affine structural paths, ordered canonically. This is
    /// semantic ownership state, not a runtime object-layout bitmap.
    pub fn live_affine_frontier(&self) -> impl Iterator<Item = &StructuralAffineDiscard> + '_ {
        self.live_affine_frontier.iter()
    }

    pub fn resume(
        &mut self,
        meter: &mut TerminalFuelMeter,
        handler: &mut impl TerminalEffectHandler,
    ) -> Result<TerminalExecutionStatus, TerminalInterpretError> {
        if self.result.is_some() {
            self.retire_placed_view_occurrences();
            let result = self.result.clone().expect("checked above");
            return Ok(TerminalExecutionStatus::Complete(result));
        }
        if let Some(crash) = &self.crash {
            return Ok(TerminalExecutionStatus::Crashed(crash.clone()));
        }

        let program = std::sync::Arc::clone(&self.program);
        loop {
            while let Some(operation) = program
                .machines
                .get(&self.current_machine)
                .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(
                    self.current_machine,
                ))?
                .blocks
                .get(&self.current)
                .ok_or(TerminalInterpretError::VerifiedBlockMissing)?
                .operations
                .get(self.next_operation)
            {
                if let Err(error) = meter.charge_operation(operation) {
                    return meter_status(error);
                }
                let selected;
                let operation = if operation.kind.runtime_indexes().is_empty() {
                    operation
                } else {
                    selected = self.select_runtime_elements(operation)?;
                    &selected
                };
                self.validate_case_projections(operation)?;
                let flow = match operation.kind {
                    OperationKind::EstablishReference { ref source } => {
                        self.establish_reference(operation, source)?;
                        OperationFlow::Advance
                    }
                    OperationKind::ReleaseReference { source } => {
                        self.release_reference(operation, source)?;
                        OperationFlow::Advance
                    }
                    OperationKind::EstablishZeroedStructural { .. } => {
                        self.execute_establish_zeroed_structural(operation)?;
                        OperationFlow::Advance
                    }
                    OperationKind::EstablishStructuralLocal { .. } => {
                        self.execute_establish_structural_local(operation)?;
                        OperationFlow::Advance
                    }
                    OperationKind::EstablishScalarArray { ref elements } => {
                        self.execute_scalar_array_establishment(operation, elements)?;
                        OperationFlow::Advance
                    }
                    OperationKind::EstablishPrimitiveLocal { value } => {
                        self.execute_primitive_establishment(operation, value)?;
                        OperationFlow::Advance
                    }
                    OperationKind::PrimitiveScalarRead { source, ref path } => {
                        self.execute_primitive_read(operation, source, path)?;
                        OperationFlow::Advance
                    }
                    OperationKind::StoreDynamicDescriptor { .. } => {
                        self.execute_store_dynamic_descriptor(operation)?
                    }
                    OperationKind::EstablishScalarCase { .. } => {
                        self.execute_establish_scalar_case(operation)?
                    }
                    OperationKind::EstablishByteSequenceConcatenation { .. } => {
                        self.execute_byte_sequence_concatenation(operation)?
                    }
                    OperationKind::EstablishByteSequenceLiteral { .. } => {
                        self.execute_establish_byte_sequence_literal(operation)?
                    }
                    OperationKind::EstablishTrivialAffineLocal { .. } => {
                        self.execute_establish_trivial_affine_local(operation)?
                    }
                    OperationKind::EstablishRecord { .. } => {
                        self.execute_establish_record(operation)?
                    }
                    OperationKind::EstablishStructuralCase { .. } => {
                        self.execute_establish_structural_case(operation)?
                    }
                    OperationKind::CallUnit { .. } => self.execute_call_unit(operation)?,
                    OperationKind::CallStructuralScalar { .. } => {
                        self.execute_call_structural_scalar(operation)?
                    }
                    OperationKind::CallDynamicScalar { .. } => {
                        self.execute_call_dynamic_scalar(operation)?
                    }
                    OperationKind::CallDynamicParameterScalar { .. } => {
                        self.execute_call_dynamic_parameter_scalar(operation)?
                    }
                    OperationKind::CallDynamicUnit { .. } => {
                        self.execute_call_dynamic_unit(operation)?
                    }
                    OperationKind::CallDynamicParameterUnit { .. } => {
                        self.execute_call_dynamic_parameter_unit(operation)?
                    }
                    OperationKind::CallStructural { .. } => {
                        self.execute_call_structural(operation)?
                    }
                    OperationKind::CallStructuralWithScalarArguments { .. } => {
                        self.execute_call_structural_with_scalar_arguments(operation)?
                    }
                    OperationKind::BoundaryCall { .. } => {
                        self.execute_boundary_call(operation, handler)?
                    }
                    OperationKind::PortWrite { .. } => {
                        self.execute_port_write(operation, handler)?
                    }
                    OperationKind::Call { .. } => self.execute_call(operation, &program)?,
                    OperationKind::WriteOnlyPrimitiveStore {
                        destination,
                        value,
                        ref path,
                    } => {
                        self.execute_primitive_store(operation, destination, value, path)?;
                        OperationFlow::Advance
                    }
                    OperationKind::StructuralScalarFieldStore { .. } => {
                        self.execute_structural_scalar_field_store(operation)?
                    }
                    OperationKind::MoveStructuralField { .. } => {
                        self.execute_move_structural_field(operation)?;
                        OperationFlow::Advance
                    }
                    OperationKind::StoreStructuralField { .. } => {
                        self.execute_store_structural_field(operation)?;
                        OperationFlow::Advance
                    }
                    OperationKind::ByteSequenceStore { .. } => {
                        self.execute_byte_sequence_store(operation)?;
                        OperationFlow::Advance
                    }
                    OperationKind::StructuralByteSequenceFieldStore { .. } => {
                        self.execute_structural_byte_sequence_field_store(operation)?;
                        OperationFlow::Advance
                    }
                    OperationKind::StructuralByteViewFieldStore { .. } => {
                        self.execute_structural_byte_view_field_store(operation)?;
                        OperationFlow::Advance
                    }
                    OperationKind::StructuralByteSequenceFieldLength { .. } => {
                        self.execute_structural_byte_sequence_field_length(operation)?;
                        OperationFlow::Advance
                    }
                    OperationKind::StructuralByteSequenceFieldByteStore { .. } => {
                        self.execute_structural_byte_sequence_field_byte_store(operation)?;
                        OperationFlow::Advance
                    }
                    OperationKind::StructuralByteSequenceFieldRead { .. } => {
                        self.execute_structural_byte_sequence_field_read(operation)?;
                        OperationFlow::Advance
                    }
                    OperationKind::IntegerConstant { .. } => {
                        self.execute_integer_constant(operation)?
                    }
                    OperationKind::IeeeFloatConstant { .. } => {
                        self.execute_ieee_float_constant(operation)?
                    }
                    OperationKind::IeeeFloatCompare { .. } => {
                        self.execute_ieee_float_compare(operation)?
                    }
                    OperationKind::ScalarRepresentationView { .. } => {
                        self.execute_scalar_representation_view(operation)?
                    }
                    OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. } => {
                        self.execute_nearest_ieee_float_fused_multiply_add(operation)?
                    }
                    OperationKind::BooleanConstant { .. } => {
                        self.execute_boolean_constant(operation)?
                    }
                    OperationKind::StructuralCaseMembership { .. } => {
                        self.execute_structural_case_membership(operation)?
                    }
                    OperationKind::StructuralLeafCopy { .. }
                    | OperationKind::StructuralCaseLeafCopy { .. } => {
                        self.execute_structural_leaf_copy(operation)?;
                        OperationFlow::Advance
                    }
                    OperationKind::BooleanStructuralField { .. } => {
                        self.execute_boolean_structural_field(operation)?
                    }
                    OperationKind::IntegerStructuralField { .. } => {
                        self.execute_integer_structural_field(operation)?
                    }
                    OperationKind::EstablishElementView { .. }
                    | OperationKind::EstablishMutableElementView { .. } => {
                        self.execute_establish_element_view(operation)?
                    }
                    OperationKind::ElementViewLength { .. } => {
                        self.execute_element_view_length(operation)?
                    }
                    OperationKind::ElementViewRead { .. } => {
                        self.execute_element_view_read(operation)?
                    }
                    OperationKind::ElementViewWrite { .. } => {
                        self.execute_element_view_write(operation)?
                    }
                    OperationKind::ElementViewSubslice { .. } => {
                        self.execute_element_view_subslice(operation)?;
                        OperationFlow::Advance
                    }
                    OperationKind::ByteSequenceSubslice { .. } => {
                        self.execute_byte_sequence_subslice(operation)?;
                        OperationFlow::Advance
                    }
                    OperationKind::ByteSequenceWrite { .. } => {
                        self.execute_byte_sequence_write(operation)?;
                        OperationFlow::Advance
                    }
                    OperationKind::ByteSequencePredicate { .. } => {
                        self.execute_byte_sequence_predicate(operation)?
                    }
                    OperationKind::ByteSequenceEqual { .. } => {
                        self.execute_byte_sequence_equal(operation)?
                    }
                    OperationKind::ByteSequenceRead { .. } => {
                        self.execute_byte_sequence_read(operation)?
                    }
                    OperationKind::ByteSequenceLength { .. } => {
                        self.execute_byte_sequence_length(operation)?
                    }
                    OperationKind::BooleanNot { .. } => self.execute_boolean_not(operation)?,
                    OperationKind::BooleanEqual { .. } => self.execute_boolean_equal(operation)?,
                    OperationKind::IntegerEqual { .. } => self.execute_integer_equal(operation)?,
                    OperationKind::IntegerLessThan { .. }
                    | OperationKind::IntegerLessOrEqual { .. } => {
                        self.execute_integer_less_than(operation)?
                    }
                    OperationKind::IntegerBitwiseNot { .. } => {
                        self.execute_integer_bitwise_not(operation)?
                    }
                    OperationKind::IntegerWiden { .. } => self.execute_integer_widen(operation)?,
                    OperationKind::IntegerExactCast { .. } => {
                        self.execute_integer_exact_cast(operation)?
                    }
                    OperationKind::IntegerBitwiseAnd { .. }
                    | OperationKind::IntegerBitwiseOr { .. }
                    | OperationKind::IntegerBitwiseXor { .. } => {
                        self.execute_integer_bitwise_and(operation)?
                    }
                    OperationKind::WrappingIntegerShiftLeft { .. }
                    | OperationKind::WrappingIntegerShiftRight { .. }
                    | OperationKind::ExactIntegerShiftLeft { .. }
                    | OperationKind::ExactIntegerShiftRight { .. } => {
                        self.execute_wrapping_integer_shift_left(operation)?
                    }
                    OperationKind::ExactIntegerAdd { .. }
                    | OperationKind::WrappingIntegerAdd { .. }
                    | OperationKind::ExactIntegerSubtract { .. }
                    | OperationKind::WrappingIntegerSubtract { .. }
                    | OperationKind::ExactIntegerMultiply { .. }
                    | OperationKind::ExactIntegerDivide { .. }
                    | OperationKind::ExactIntegerRemainder { .. }
                    | OperationKind::WrappingIntegerDivide { .. }
                    | OperationKind::WrappingIntegerRemainder { .. }
                    | OperationKind::SaturatingIntegerDivide { .. }
                    | OperationKind::SaturatingIntegerRemainder { .. }
                    | OperationKind::WrappingIntegerMultiply { .. } => {
                        self.execute_exact_integer_add(operation)?
                    }
                    OperationKind::SaturatingIntegerAdd { .. } => {
                        self.execute_saturating_integer_add(operation)?
                    }
                    OperationKind::SaturatingIntegerSubtract { .. } => {
                        self.execute_saturating_integer_subtract(operation)?
                    }
                    OperationKind::SaturatingIntegerMultiply { .. } => {
                        self.execute_saturating_integer_multiply(operation)?
                    }
                    OperationKind::TrappingInteger { .. } => {
                        self.execute_trapping_integer(operation)?
                    }
                    OperationKind::FloatResultPolicy { .. } => {
                        self.execute_float_result_policy(operation)?
                    }
                    OperationKind::AtomicAccess {
                        place,
                        ref path,
                        field,
                        event,
                    } => {
                        self.execute_atomic_access(operation, place, path, field, event)?;
                        OperationFlow::Advance
                    }
                };
                match flow {
                    OperationFlow::Advance => {}
                    OperationFlow::Redispatch => continue,
                    OperationFlow::Yield(status) => {
                        if matches!(status, TerminalExecutionStatus::Complete(_)) {
                            self.retire_placed_view_occurrences();
                        }
                        return Ok(status);
                    }
                }
                self.next_operation += 1;
            }
            let terminator = &program
                .machines
                .get(&self.current_machine)
                .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(
                    self.current_machine,
                ))?
                .blocks
                .get(&self.current)
                .ok_or(TerminalInterpretError::VerifiedBlockMissing)?
                .terminator;
            let flow = match &terminator {
                Terminator::ReturnUnitNominalAffine { .. } => {
                    self.settle_return_unit_nominal_affine(terminator, meter)?
                }
                Terminator::ReturnUnitPartialAffine { .. } => {
                    self.settle_return_unit_partial_affine(terminator, meter)?
                }
                Terminator::Jump { .. } => self.settle_jump(terminator, meter)?,
                Terminator::Conditional { .. } => self.settle_conditional(terminator, meter)?,
                Terminator::StructuralCase { .. } => {
                    self.settle_structural_case(terminator, meter)?
                }
                Terminator::Return { .. } => self.settle_return(terminator, meter)?,
                Terminator::ReturnUnit { .. } => self.settle_return_unit(terminator, meter)?,
                Terminator::ReturnStructural { .. } => {
                    self.settle_return_structural(terminator, meter)?
                }
                Terminator::Crash { .. } => self.settle_crash(terminator, meter)?,
            };
            match flow {
                TerminatorFlow::Continue => {}
                TerminatorFlow::Yield(status) => {
                    if matches!(status, TerminalExecutionStatus::Complete(_)) {
                        self.retire_placed_view_occurrences();
                    }
                    return Ok(status);
                }
            }
        }
    }

    /// Release the entry invocation's placed-view loans. The provider's
    /// qualified backing is returned untouched — custody is lent, never
    /// reminted — so a crash leaves occurrences recorded rather than retired.
    fn retire_placed_view_occurrences(&mut self) {
        self.placed_view_occurrences.clear();
    }
}
