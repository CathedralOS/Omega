//! The execution engine: the live execution state, executable machines,
//! suspended calls, runtime descriptors and claims, and the interpreter loop.
//! `resume_with_effect_handler` charges and dispatches each operation to its
//! `execute_*` method (`integer_operations`, `boolean_float_operations`,
//! `call_operations`, `structural_operations` and the earlier owners), then
//! settles the block terminator through its `settle_*` method (`terminators`).

use crate::TerminalStructuralScalarFieldValue;
use crate::terminal_interpreter::byte_sequence_binding::ByteSequenceBinding;
use crate::terminal_interpreter::byte_sequence_view::ByteSequenceView;
use crate::terminal_interpreter::case_membership::StructuralCaseContents;
use crate::terminal_interpreter::custody::{
    bind_affine_frontier, bind_arguments, bind_entry_claims, bind_structural_arguments,
    bind_structural_primitive_values,
};
use crate::terminal_interpreter::primitive_storage;
use crate::terminal_interpreter::results::meter_status;
use crate::terminal_interpreter::scalar_array::TerminalScalarArrayValue;
use crate::terminal_interpreter::structural_scalar_fields;
use crate::terminal_interpreter::values::{
    StructuralByteSequenceRuntimeField, StructuralRuntimePlace, StructuralScalarRuntimeField,
};
use crate::terminal_interpreter::{
    AdmittedProviderInstallation, TerminalArtifactInterpretError, TerminalCrash, TerminalEffect,
    TerminalEffectHandler, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalInterpretError, TerminalScalarCaseValue, TerminalScalarValue, TerminalStructuralInputs,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, MachineId, OperationId, PlaceId, StructuralTypeId, ValueId,
};
use std::collections::{BTreeMap, BTreeSet};
use terminal_fuel::TerminalFuelMeter;
use terminal_psi::{
    Block, BoundaryMachineDeclaration, EntryClaim, NominalAffineCleanup, OperationKind,
    StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralResultClaimTransfer,
    StructuralTypeDeclaration, TerminalMachineResult, Terminator,
};

/// What one settled terminator tells the interpreter loop to do next.
pub(super) enum TerminatorFlow {
    /// The block transferred control; dispatch from the new current block.
    Continue,
    /// Execution stops here with this status.
    Yield(TerminalExecutionStatus),
}

/// What one executed operation tells the interpreter loop to do next.
pub(super) enum OperationFlow {
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
    pub(crate) structural_types: BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    // One immutable code owner survives suspension. A resume or call borrows
    // this owner independently of mutable runtime storage; frames retain only
    // machine identity. There is no per-machine or per-operation Arc allocation.
    pub(crate) machines: std::sync::Arc<BTreeMap<MachineId, ExecutableMachine>>,
    pub(crate) dynamic_scalar_calls: BTreeMap<(MachineId, u32), (MachineId, StructuralArgument)>,
    pub(crate) dynamic_descriptor_templates:
        BTreeMap<(MachineId, u32), RuntimeDynamicDescriptorTemplate>,
    pub(crate) dynamic_selection_templates:
        BTreeMap<(MachineId, u32), RuntimeDynamicDescriptorTemplate>,
    pub(crate) dynamic_descriptor_arguments:
        BTreeMap<(MachineId, OperationId), Vec<terminal_psi::TerminalDynamicDescriptorArgument>>,
    pub(crate) dynamic_parameters: BTreeMap<u32, RuntimeDynamicDescriptor>,
    pub(crate) boundary_machines: BTreeMap<BoundaryMachineId, BoundaryMachineDeclaration>,
    pub(crate) provider_candidates: BTreeSet<BoundaryMachineId>,
    pub(crate) provider_installation: BTreeMap<BoundaryMachineId, MachineId>,
    pub(crate) values: BTreeMap<ValueId, TerminalScalarValue>,
    pub(crate) structural_values: BTreeMap<PlaceId, TerminalStructuralValue>,
    /// A reference carrier owns this descriptor, never the referent's backing.
    /// Carrier identities survive structural call/return moves across frames.
    pub(crate) reference_referents: BTreeMap<StructuralRuntimePlace, TerminalStructuralValue>,
    /// Mutable primitive contents live outside call frames. Machine-local
    /// place maps are only views into this stable logical storage arena.
    pub(crate) structural_primitive_storage: BTreeMap<StructuralRuntimePlace, TerminalScalarValue>,
    pub(crate) structural_primitive_entry_places: BTreeMap<u32, StructuralRuntimePlace>,
    pub(crate) local_structural_identities: primitive_storage::LocalStructuralIdentities,
    /// Scalar leaves written below aggregate structural values. Keys use the
    /// invocation-independent opaque identity and resolved parent path, so a
    /// projected call observes the same field without native layout claims.
    pub(crate) structural_scalar_fields:
        BTreeMap<StructuralScalarRuntimeField, TerminalScalarValue>,
    /// Owned byte contents are keyed by referent identity, not a callee-local
    /// parameter. Immutable backing implements a logical copy of the live prefix.
    pub(crate) structural_byte_sequence_fields:
        BTreeMap<StructuralByteSequenceRuntimeField, ByteSequenceView>,
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
    /// Exact claim-free affine ownership frontier. Opaque structural storage is
    /// root-addressed, so projected moves must be represented here rather than
    /// by unsoundly deleting their containing root.
    pub(crate) live_affine_frontier: BTreeSet<StructuralAffineDiscard>,
    pub(crate) live_claims: BTreeMap<ClaimId, LiveClaim>,
    pub(crate) current_machine: MachineId,
    pub(crate) current: BlockId,
    pub(crate) next_operation: usize,
    pub(crate) call_stack: Vec<SuspendedCall>,
    pub(crate) result: Option<TerminalExecutionResult>,
    pub(crate) crash: Option<TerminalCrash>,
    pub(crate) effects: Vec<TerminalEffect>,
}

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
    /// Decode, verify and start one artifact with its scalar arguments and
    /// every structural input in one record. This is the only start entry.
    /// Decode and independently verify an artifact, then bind its initialized
    /// entry contents before committing any operation or custody transfer.
    pub fn start_artifact(
        semantic_bytes: &[u8],
        proof_bytes: &[u8],
        profile: &proof_admission::AdmissionProfile,
        scalar_arguments: &[TerminalScalarValue],
        structural_inputs: TerminalStructuralInputs<'_>,
    ) -> Result<Self, TerminalArtifactInterpretError> {
        let module = terminal_codec::decode_module(semantic_bytes)
            .map_err(TerminalArtifactInterpretError::SemanticDecode)?;
        // Only the subject-sealed proof section enters interpretation: the
        // seal must name this module's reconstructed identity, so a proof
        // produced for another source/model/profile cannot be replayed here.
        let proof = terminal_codec::decode_proof_section_for(&module, proof_bytes)
            .map_err(TerminalArtifactInterpretError::ProofDecode)?;
        let _verified =
            terminal_verifier::verify_module_for_interpretation(&module, &proof, profile)
                .map_err(TerminalArtifactInterpretError::Verification)?;
        let converted_boolean_fields;
        let scalar_fields = if structural_inputs.boolean_fields.is_empty() {
            structural_inputs.scalar_fields
        } else {
            converted_boolean_fields = structural_inputs
                .boolean_fields
                .iter()
                .map(TerminalStructuralScalarFieldValue::from)
                .collect::<Vec<_>>();
            &converted_boolean_fields
        };
        let mut execution = Self::start_verified_module(
            module,
            scalar_arguments,
            structural_inputs.arguments,
            scalar_fields,
            structural_inputs.primitive_values,
            None,
        )
        .map_err(TerminalArtifactInterpretError::Execution)
        .map_err(|error| match error {
            TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::StructuralScalarFieldArgumentInvalid {
                    argument_index,
                    field,
                },
            ) if !structural_inputs.boolean_fields.is_empty() => {
                TerminalArtifactInterpretError::Execution(
                    TerminalInterpretError::StructuralBooleanFieldArgumentInvalid {
                        argument_index,
                        field,
                    },
                )
            }
            other => other,
        })?;
        execution
            .bind_structural_cases(structural_inputs.cases)
            .map_err(TerminalArtifactInterpretError::Execution)?;
        if !structural_inputs.byte_arrays.is_empty() {
            execution
                .bind_byte_arrays(structural_inputs.byte_arrays)
                .map_err(TerminalArtifactInterpretError::Execution)?;
        }
        Ok(execution)
    }

    /// Begin execution with one explicit provider installation previously
    /// admitted against these exact semantic/proof sections. Fixed-array inputs
    /// supply exact initialized backing; an installed provider grants no storage.
    pub fn start_installed_artifact(
        semantic_bytes: &[u8],
        proof_bytes: &[u8],
        profile: &proof_admission::AdmissionProfile,
        scalar_arguments: &[TerminalScalarValue],
        structural_inputs: TerminalStructuralInputs<'_>,
        installation: &AdmittedProviderInstallation,
    ) -> Result<Self, TerminalArtifactInterpretError> {
        let module = terminal_codec::decode_module(semantic_bytes)
            .map_err(TerminalArtifactInterpretError::SemanticDecode)?;
        // Same sealed-section requirement as the other artifact starts: an
        // admitted installation is bound to this exact Terminal-Psi identity.
        let proof = terminal_codec::decode_proof_section_for(&module, proof_bytes)
            .map_err(TerminalArtifactInterpretError::ProofDecode)?;
        let _verified = terminal_verifier::verify_module(&module, &proof, profile)
            .map_err(TerminalArtifactInterpretError::Verification)?;
        let mut execution = Self::start_verified_module(
            module,
            scalar_arguments,
            structural_inputs.arguments,
            structural_inputs.scalar_fields,
            structural_inputs.primitive_values,
            Some(installation),
        )
        .map_err(TerminalArtifactInterpretError::Execution)?;
        if !structural_inputs.cases.is_empty() {
            execution
                .bind_structural_cases(structural_inputs.cases)
                .map_err(TerminalArtifactInterpretError::Execution)?;
        }
        execution
            .bind_byte_arrays(structural_inputs.byte_arrays)
            .map_err(TerminalArtifactInterpretError::Execution)?;
        Ok(execution)
    }

    pub(crate) fn start_verified_module(
        mut module: terminal_psi::TerminalModule,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[TerminalStructuralValue],
        structural_scalar_field_arguments: &[TerminalStructuralScalarFieldValue],
        structural_primitive_value_arguments: &[TerminalStructuralPrimitiveValue],
        installation: Option<&AdmittedProviderInstallation>,
    ) -> Result<Self, TerminalInterpretError> {
        // A nonempty placed-view roster declares direct entry inputs whose
        // referent interpretation cannot lend: no structural or scalar input
        // channel carries the placement each row names. Execution must fail
        // closed rather than start the entry machine with a declared input
        // silently unbound.
        if !module.placed_view_inputs.is_empty() {
            return Err(TerminalInterpretError::PlacedViewInputsRequireCustody);
        }
        let terminal_psi = terminal_codec::terminal_psi_identity(&module)
            .map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)?;
        if installation.is_some_and(|installation| installation.terminal_psi != terminal_psi) {
            return Err(TerminalInterpretError::ProviderInstallationIdentityMismatch);
        }
        let local_structural_identities =
            primitive_storage::LocalStructuralIdentities::new(&module, structural_arguments);
        let machines = std::mem::take(&mut module.machines)
            .into_iter()
            .map(|machine| {
                (
                    machine.id,
                    ExecutableMachine {
                        parameters: machine.parameters,
                        structural_parameters: machine.structural_parameters,
                        structural_places: machine.structural_places,
                        entry_claims: machine.entry_claims,
                        content_entry_claims: machine.content_entry_claims,
                        result: machine.result,
                        entry: machine.entry,
                        blocks: machine
                            .blocks
                            .into_iter()
                            .map(|block| (block.id, block))
                            .collect(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut dynamic_scalar_calls = module
            .dynamic_dispatch
            .indirect_dispatches
            .iter()
            .map(|dispatch| {
                let descriptor = module
                    .dynamic_dispatch
                    .rebound_descriptors
                    .iter()
                    .find(|descriptor| {
                        descriptor.owner == dispatch.owner
                            && descriptor.ordinal == dispatch.descriptor_ordinal
                    })
                    .expect("verified indirect dispatch has one descriptor");
                let selection = module
                    .dynamic_dispatch
                    .selections
                    .iter()
                    .find(|selection| {
                        selection.owner == descriptor.owner
                            && selection.ordinal == descriptor.rebound_selection_ordinal
                    })
                    .expect("verified descriptor has one latest selection");
                (
                    (dispatch.owner, dispatch.descriptor_ordinal),
                    (dispatch.realization, selection.source.clone()),
                )
            })
            .collect::<BTreeMap<_, _>>();
        for dispatch in &module.dynamic_dispatch.stored_dispatches {
            let descriptor = module
                .dynamic_dispatch
                .stored_descriptors
                .iter()
                .find(|descriptor| {
                    descriptor.owner == dispatch.owner
                        && descriptor.ordinal == dispatch.descriptor_ordinal
                })
                .expect("verified stored dispatch has one descriptor");
            let selection = module
                .dynamic_dispatch
                .selections
                .iter()
                .find(|selection| {
                    selection.owner == descriptor.owner
                        && selection.ordinal == descriptor.selection_ordinal
                })
                .expect("verified stored descriptor has one selection");
            assert!(
                dynamic_scalar_calls
                    .insert(
                        (dispatch.owner, dispatch.descriptor_ordinal),
                        (dispatch.realization, selection.source.clone()),
                    )
                    .is_none(),
                "verified dynamic descriptor coordinates must be disjoint"
            );
        }
        let dynamic_selection_templates = module
            .dynamic_dispatch
            .selections
            .iter()
            .map(|selection| {
                let application = module
                    .closed_conformance_applications
                    .iter()
                    .find(|application| {
                        application.owner == selection.owner
                            && application.report_fingerprint
                                == selection.conformance_application_report_fingerprint
                            && application.commitment
                                == selection.conformance_application_commitment
                    })
                    .expect("verified selection has one conformance application");
                let callables = application
                    .rows
                    .iter()
                    .map(|row| {
                        row.realization_callable_identity.as_ref().map(|identity| {
                            application
                                .realization_callables
                                .iter()
                                .find(|callable| callable.source_callable_identity == *identity)
                                .map(|callable| callable.machine)
                                .expect("verified dynamic row has one callable")
                        })
                    })
                    .collect();
                (
                    (selection.owner, selection.ordinal),
                    RuntimeDynamicDescriptorTemplate {
                        source: selection.source.clone(),
                        callables,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut dynamic_descriptor_templates = module
            .dynamic_dispatch
            .rebound_descriptors
            .iter()
            .map(|descriptor| {
                let template = dynamic_selection_templates
                    .get(&(descriptor.owner, descriptor.rebound_selection_ordinal))
                    .expect("verified descriptor has one latest selection")
                    .clone();
                ((descriptor.owner, descriptor.ordinal), template)
            })
            .collect::<BTreeMap<_, _>>();
        for descriptor in &module.dynamic_dispatch.stored_descriptors {
            let template = dynamic_selection_templates
                .get(&(descriptor.owner, descriptor.selection_ordinal))
                .expect("verified stored descriptor has one selection")
                .clone();
            assert!(
                dynamic_descriptor_templates
                    .insert((descriptor.owner, descriptor.ordinal), template)
                    .is_none(),
                "verified dynamic descriptor coordinates must be disjoint"
            );
        }
        let mut dynamic_descriptor_arguments = BTreeMap::<
            (MachineId, OperationId),
            Vec<terminal_psi::TerminalDynamicDescriptorArgument>,
        >::new();
        for argument in &module.dynamic_dispatch.arguments {
            dynamic_descriptor_arguments
                .entry((argument.owner, argument.operation))
                .or_default()
                .push(argument.clone());
        }
        let boundary_machines = module
            .boundary_machines
            .iter()
            .cloned()
            .map(|boundary| (boundary.id, boundary))
            .collect::<BTreeMap<_, _>>();
        let structural_types = module
            .structural_types
            .iter()
            .cloned()
            .map(|declaration| (declaration.id, declaration))
            .collect();
        let machine = machines
            .get(&module.entry)
            .ok_or(TerminalInterpretError::VerifiedEntryMachineMissing)?;
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
                &structural_types,
                &structural_values,
                structural_primitive_value_arguments,
            )?;
        let structural_scalar_fields = structural_scalar_fields::bind(
            machine,
            &structural_types,
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
        let current = machine.entry;
        Ok(Self {
            structural_types,
            machines: std::sync::Arc::new(machines),
            dynamic_scalar_calls,
            dynamic_descriptor_templates,
            dynamic_selection_templates,
            dynamic_descriptor_arguments,
            dynamic_parameters: BTreeMap::new(),
            boundary_machines,
            provider_candidates: module
                .provider_candidates
                .iter()
                .map(|candidate| candidate.boundary)
                .collect(),
            provider_installation: installation
                .map(|installation| installation.installed.clone())
                .unwrap_or_default(),
            values,
            structural_values,
            reference_referents: BTreeMap::new(),
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
            live_affine_frontier,
            live_claims,
            current_machine: module.entry,
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
        if let Some(result) = &self.result {
            return Ok(TerminalExecutionStatus::Complete(result.clone()));
        }
        if let Some(crash) = &self.crash {
            return Ok(TerminalExecutionStatus::Crashed(crash.clone()));
        }

        let machines = std::sync::Arc::clone(&self.machines);
        loop {
            while let Some(operation) = machines
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
                let flow = match operation.kind {
                    OperationKind::EstablishReference { ref source } => {
                        self.establish_reference(operation, source)?;
                        OperationFlow::Advance
                    }
                    OperationKind::ReleaseReference { source } => {
                        self.release_reference(operation, source)?;
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
                    OperationKind::EstablishByteSequenceLiteral { .. } => {
                        self.execute_establish_byte_sequence_literal(operation)?
                    }
                    OperationKind::EstablishTrivialAffineLocal { .. } => {
                        self.execute_establish_trivial_affine_local(operation)?
                    }
                    OperationKind::EstablishRecord { .. } => {
                        self.execute_establish_record(operation)?
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
                    OperationKind::Call { .. } => self.execute_call(operation, &machines)?,
                    OperationKind::WriteOnlyPrimitiveStore {
                        destination,
                        value,
                        ref path,
                    } => {
                        self.execute_primitive_store(operation, destination, value, path)?;
                        OperationFlow::Advance
                    }
                    OperationKind::WriteOnlyIndexedPrimitiveStore {
                        destination,
                        ref path,
                        index,
                        value,
                        ..
                    } => {
                        self.execute_indexed_primitive_store(
                            operation,
                            destination,
                            path,
                            index,
                            value,
                        )?;
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
                    OperationKind::StructuralByteSequenceFieldStore { .. } => {
                        self.execute_structural_byte_sequence_field_store(operation)?;
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
                    OperationKind::IntegerConstant { .. } => {
                        self.execute_integer_constant(operation)?
                    }
                    OperationKind::IeeeFloatConstant { .. } => {
                        self.execute_ieee_float_constant(operation)?
                    }
                    OperationKind::IeeeFloatCompare { .. } => {
                        self.execute_ieee_float_compare(operation)?
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
                    OperationKind::BooleanStructuralField { .. } => {
                        self.execute_boolean_structural_field(operation)?
                    }
                    OperationKind::IntegerStructuralField { .. } => {
                        self.execute_integer_structural_field(operation)?
                    }
                    OperationKind::ByteSequenceSubslice { .. } => {
                        self.execute_byte_sequence_subslice(operation)?;
                        OperationFlow::Advance
                    }
                    OperationKind::ByteSequenceWrite { .. } => {
                        self.execute_byte_sequence_write(operation)?;
                        OperationFlow::Advance
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
                };
                match flow {
                    OperationFlow::Advance => {}
                    OperationFlow::Redispatch => continue,
                    OperationFlow::Yield(status) => return Ok(status),
                }
                self.next_operation += 1;
            }
            let terminator = &machines
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
                TerminatorFlow::Yield(status) => return Ok(status),
            }
        }
    }
}
