//! The execution engine: the live execution state, borrowed Terminal machines,
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
    AdmittedAssumedBinding, TerminalEffect, TerminalEffectHandler, TerminalRuntimeSupplies,
};
use crate::errors::{TerminalInterpretError, malformed};
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
    TerminalScalarValue, TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, IntegerType, IntegerValue, MachineId, PlaceId, ValueId,
};
use std::collections::{BTreeMap, BTreeSet};
use terminal_fuel::TerminalFuelMeter;
use terminal_psi::{
    NominalAffineCleanup, OperationKind, StructuralAffineDiscard, StructuralMultiplicity,
    StructuralOperationResult, StructuralPathSegment, TerminalPlacedViewInput, Terminator,
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
pub struct TerminalExecution<'artifact> {
    // One immutable program owner survives suspension and is shared verbatim
    // across every independent instance admitted from the same module. A
    // resume or call borrows this owner independently of mutable runtime
    // storage; frames retain only machine identity. There is no per-machine
    // or per-operation Arc allocation.
    pub(crate) program: std::sync::Arc<crate::program::AdmittedTerminalProgram<'artifact>>,
    /// One immutable selected route per boundary. Catalog alternatives are
    /// never a substitute for a checked installation or explicit host choice.
    pub(crate) boundary_supplies:
        BTreeMap<BoundaryMachineId, crate::effects::AdmittedBoundarySupply>,
    /// Exact family catalog selection, never provider or boundary issuance.
    pub(crate) target_machines: Option<terminal_installation::AdmittedTargetMachineSelections>,
    /// Root-authorized assumed binding contracts admitted with the provider
    /// installation. They are conditional evidence on this execution, not
    /// checked redirects: call custody keeps consulting only
    /// `provider_candidates` and `boundary_supplies`, so a direct call of
    /// either requirement gains no assumed facts.
    pub(crate) assumed_bindings: Vec<AdmittedAssumedBinding>,
    pub(crate) values: BTreeMap<ValueId, TerminalScalarValue>,
    pub(crate) structural_values: BTreeMap<PlaceId, TerminalStructuralValue>,
    pub(crate) private_entry_ram: Vec<crate::reference::placed_ram::PrivateEntryRamAllocation>,
    pub(crate) aggregate_argument_backings: BTreeMap<PlaceId, crate::reference::AggregateBacking>,
    /// A reference carrier owns this descriptor, never the referent's backing.
    /// Carrier identities survive structural call/return moves across frames.
    pub(crate) reference_referents:
        BTreeMap<StructuralRuntimePlace, crate::reference::ReferenceReferent>,
    /// Mutable primitive contents live outside call frames. Machine-local
    /// place maps are only views into this stable logical storage arena.
    pub(crate) structural_primitive_storage: BTreeMap<StructuralRuntimePlace, TerminalScalarValue>,
    pub(crate) structural_primitive_entry_places: BTreeMap<u32, StructuralRuntimePlace>,
    pub(crate) local_structural_identities: LocalStructuralIdentities,
    /// Amortized scalar-only storage reclamation, never an ownership frontier.
    pub(crate) next_scalar_collection_identity: u64,
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
    /// Completed byte payloads addressed by exact typed storage home. Fixed
    /// byte arrays and stored shared descriptors reuse the backing representation;
    /// each observing operation separately authenticates its declared carrier.
    pub(crate) structural_byte_storage: BTreeMap<StructuralRuntimePlace, ByteSequenceView>,
    /// Entry-provided discriminators remain attached to the original referent
    /// across projected and forwarded calls, independent of machine-local IDs.
    pub(crate) structural_cases: BTreeMap<StructuralRuntimePlace, StructuralCaseContents>,
    pub(crate) scalar_array_values: BTreeMap<PlaceId, TerminalScalarArrayValue>,
    /// Frame-local immutable descriptors or exact boundary-introduced mutable
    /// field loans, rebound to callee parameters. Opaque identities alone do
    /// not supply either byte contents or permission to mutate a field.
    pub(crate) byte_sequence_values: BTreeMap<PlaceId, ByteSequenceBinding>,
    pub(crate) primitive_loan_values: BTreeMap<PlaceId, crate::primitive_storage::PrimitiveLoan>,
    /// Shared immutable element views rebound to callee parameters, parallel
    /// to `byte_sequence_values` for borrowed `&[T]` storage.
    pub(crate) element_view_values: BTreeMap<PlaceId, crate::element_views::ElementView>,
    /// Exact claim-free affine ownership frontier. Opaque structural storage is
    /// root-addressed, so projected moves must be represented here rather than
    /// by unsoundly deleting their containing root.
    pub(crate) live_affine_frontier: BTreeSet<StructuralAffineDiscard>,
    /// Current frame's placed-view presentations, retaining the entry's original
    /// backing. Suspended frames preserve their presentations across a helper;
    /// completion returns the same loans without reminting custody.
    pub(crate) placed_view_occurrences: BTreeMap<TerminalPlacedViewInput, PlacedViewOccurrence>,
    pub(crate) live_claims: BTreeMap<ClaimId, LiveClaim>,
    pub(crate) current_machine: MachineId,
    pub(crate) current: BlockId,
    pub(crate) next_operation: usize,
    pub(crate) call_stack: Vec<SuspendedCall>,
    pub(crate) result: Option<TerminalExecutionResult>,
    pub(crate) crash: Option<TerminalCrash>,
    pub(crate) external_completion: Option<crate::results::TerminalExternalCompletion>,
    pub(crate) external_demands: Vec<crate::capabilities::TerminalExternalCompletionDemand>,
    pub(crate) external_bindings:
        BTreeMap<BoundaryMachineId, crate::instance::TerminalProcessExitAdmission>,
    pub(crate) process_domain: Option<std::sync::Arc<crate::instance::SimulatedProcessDomain>>,
    /// Host-established erased occurrence custody, separate from initialized
    /// data storage and retained through abandonment without discharge.
    pub(crate) established_erased_fields:
        Vec<crate::structural_inputs::TerminalEstablishedErasedField>,
    pub(crate) effects: Vec<TerminalEffect>,
}

pub(crate) struct SuspendedCall {
    pub(crate) placed_view_occurrences: BTreeMap<TerminalPlacedViewInput, PlacedViewOccurrence>,
    pub(crate) loan_completions: Vec<terminal_psi::LoanCompletion>,
    pub(crate) values: BTreeMap<ValueId, TerminalScalarValue>,
    pub(crate) structural_values: BTreeMap<PlaceId, TerminalStructuralValue>,
    pub(crate) aggregate_argument_backings: BTreeMap<PlaceId, crate::reference::AggregateBacking>,
    pub(crate) scalar_array_values: BTreeMap<PlaceId, TerminalScalarArrayValue>,
    pub(crate) byte_sequence_values: BTreeMap<PlaceId, ByteSequenceBinding>,
    pub(crate) primitive_loan_values: BTreeMap<PlaceId, crate::primitive_storage::PrimitiveLoan>,
    pub(crate) element_view_values: BTreeMap<PlaceId, crate::element_views::ElementView>,
    pub(crate) live_affine_frontier: BTreeSet<StructuralAffineDiscard>,
    pub(crate) live_claims: BTreeMap<ClaimId, LiveClaim>,
    pub(crate) current_machine: MachineId,
    pub(crate) current: BlockId,
    pub(crate) next_operation: usize,
    pub(crate) result: SuspendedCallResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiveClaim {
    pub(crate) place: Option<PlaceId>,
    pub(crate) path: Vec<StructuralPathSegment>,
    pub(crate) multiplicity: Option<StructuralMultiplicity>,
}

/// Where the claims a structural call result binds come from on return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResultClaimOrigin {
    /// An in-module callee's returning edge holds them at the result paths.
    CalleeReturn,
    /// An exact member returns authentic custody under the weaker family head.
    TargetFamilyReturn {
        head: BoundaryMachineId,
        member: MachineId,
    },
    /// A checked provider returns actual custody through an admitted boundary route.
    CheckedProvider,
}

impl ResultClaimOrigin {
    /// Whether `result` carries exactly the domains its callee returns: the
    /// callee's declared result domains plus, for a checked boundary return, each
    /// domain the operation establishes through its admitted route. An
    /// ordinary callee return establishes nothing beyond its declaration.
    pub(crate) fn result_qualifications_match(
        self,
        result: &terminal_psi::StructuralOperationResult,
        declared: &[semantic_vocabulary::StructuralDomainId],
    ) -> bool {
        if matches!(self, Self::TargetFamilyReturn { .. }) {
            return result
                .qualifications
                .iter()
                .all(|domain| declared.contains(domain));
        }
        let mut expected = declared.to_vec();
        if self == Self::CheckedProvider {
            for establishment in &result.qualification_establishments {
                if !expected.contains(&establishment.domain) {
                    expected.push(establishment.domain);
                }
            }
            expected.sort();
        }
        result.qualifications == expected
    }

    pub(crate) fn result_projected_qualifications_match(
        self,
        result: &terminal_psi::StructuralOperationResult,
        declared: &[terminal_psi::StructuralPathQualification],
    ) -> bool {
        if matches!(
            self,
            Self::TargetFamilyReturn { .. } | Self::CheckedProvider
        ) {
            result
                .projected_qualifications
                .iter()
                .all(|row| declared.contains(row))
        } else {
            result.projected_qualifications == declared
        }
    }
}

pub(crate) enum SuspendedCallResult {
    Scalar(ValueId),
    Unit,
    Structural {
        result: StructuralOperationResult,
        claim_origin: ResultClaimOrigin,
        expected_reference_backings: Vec<Vec<crate::reference::ReferenceBacking>>,
    },
    NominalCleanups {
        completed: (NominalAffineCleanup, TerminalStructuralValue),
        remaining: std::vec::IntoIter<terminal_psi::TerminalAffineCleanupAction>,
        continuation: CleanupContinuation,
    },
}

/// Captured edge output survives every nominal hook without re-evaluation.
pub(crate) enum CleanupContinuation {
    Return(CompletedReturn),
    Successor {
        target: BlockId,
        bindings: crate::terminators::block_bindings::BlockBindings,
    },
}

/// The result is materialized once, before normal cleanup. It remains detached
/// from hook frames until the original caller can receive it.
pub(crate) struct CompletedReturn {
    pub(crate) value: TerminalExecutionResult,
    pub(crate) destination: Option<StructuralOperationResult>,
    pub(crate) rebound_claims: Option<BTreeMap<ClaimId, LiveClaim>>,
    pub(crate) byte_sequence: Option<ByteSequenceBinding>,
    pub(crate) element_view: Option<crate::element_views::ElementView>,
}

impl CompletedReturn {
    pub(crate) fn plain(value: TerminalExecutionResult) -> Self {
        Self {
            value,
            destination: None,
            rebound_claims: None,
            byte_sequence: None,
            element_view: None,
        }
    }
}

/// Dispatch rows of the interpreter loop: `advance` rows run the operation's
/// `execute_*` method and continue the block; `flow` rows take the method's
/// own flow decision. Literal arms (operations whose dispatch passes fields)
/// ride in the `literal` block ahead of the generated rows.
macro_rules! dispatch_operations {
    (
        $receiver:ident, $operation:ident, $kind:expr;
        literal { $( $pat:pat => $body:block )* }
        dispatch { $( $($variant:ident)|+ => $tail:ident $method:ident ),* $(,)? }
    ) => {
        match $kind {
            $( $pat => $body )*
            $(
                $( OperationKind::$variant { .. } )|+ => {
                    dispatch_operations!(@body $receiver, $operation, $tail, $method)
                }
            )*
        }
    };
    ( @body $receiver:ident, $operation:ident, advance, $method:ident ) => {
        {
            $receiver.$method($operation)?;
            OperationFlow::Advance
        }
    };
    ( @body $receiver:ident, $operation:ident, flow, $method:ident ) => {
        {
            $receiver.$method($operation)?
        }
    };
}

impl<'artifact> TerminalExecution<'artifact> {
    /// Start one instance on an already-admitted program: bind this
    /// invocation's inputs and custody, commit no operation. The program is
    /// data — no decode or verification repeats here.
    pub(crate) fn start_with_program(
        program: std::sync::Arc<crate::program::AdmittedTerminalProgram<'artifact>>,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[TerminalStructuralValue],
        structural_scalar_field_arguments: &[TerminalStructuralScalarFieldValue],
        structural_primitive_value_arguments: &[TerminalStructuralPrimitiveValue],
        placed_view_establishments: &[TerminalPlacedViewEstablishment],
        private_entry_ram: &[terminal_installation::CheckedPrivateEntryRamRecipe],
        zero_initialized_arguments: &[u32],
        established_erased_fields: &[crate::structural_inputs::TerminalEstablishedErasedField],
        installation: Option<&TerminalRuntimeSupplies>,
    ) -> Result<Self, TerminalInterpretError> {
        let current_machine = program.resolve_entry(installation)?;
        // A nonempty placed-view roster declares direct entry inputs whose
        // referent custody must be established beside the ordinary inputs: one
        // exact establishment per entry row lends the qualified backing for
        // the invocation's duration. A row left unsupplied, a supply that
        // answers no declared row both fail closed. Ordinary calls reborrow
        // the same established backing; another retained target's roster does
        // not describe this invocation's host input.
        let placed_view_occurrences = establish_placed_view_inputs(
            &program.catalogs,
            current_machine,
            placed_view_establishments,
            structural_arguments,
        )?;
        // Descriptor homes and their separately established backing are both
        // live identities. Local allocations must not collide with either.
        let identity_roots = structural_arguments
            .iter()
            .chain(
                placed_view_establishments
                    .iter()
                    .map(|establishment| &establishment.referent),
            )
            .cloned()
            .collect::<Vec<_>>();
        let local_structural_identities = LocalStructuralIdentities::new(
            program
                .machines()
                .values()
                .flat_map(|machine| machine.structural_places.iter()),
            &identity_roots,
        );
        let machine = program
            .machines()
            .get(&current_machine)
            .ok_or(TerminalInterpretError::VerifiedEntryMachineMissing)?;
        let current = machine.entry;
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
        let live_affine_frontier = bind_affine_frontier(
            &machine.structural_parameters,
            &structural_values,
            &program.structural_types(),
        )?;
        let live_claims = bind_entry_claims(
            &machine.entry_claims,
            &machine.content_entry_claims,
            &machine.structural_parameters,
            &structural_values,
        )?;
        let mut execution = Self {
            program: std::sync::Arc::clone(&program),
            boundary_supplies: installation
                .map(|installation| installation.supplies.clone())
                .unwrap_or_default(),
            target_machines: installation
                .and_then(|installation| installation.target_machines.clone()),
            assumed_bindings: installation
                .map(|installation| installation.assumed_bindings.clone())
                .unwrap_or_default(),
            values,
            structural_values,
            aggregate_argument_backings: BTreeMap::new(),
            reference_referents: BTreeMap::new(),
            structural_element_view_fields: BTreeMap::new(),
            structural_primitive_storage: BTreeMap::new(),
            structural_primitive_entry_places: BTreeMap::new(),
            local_structural_identities,
            next_scalar_collection_identity: 1024,
            structural_scalar_fields: BTreeMap::new(),
            private_entry_ram: Vec::new(),
            structural_byte_sequence_fields: BTreeMap::new(),
            structural_byte_storage: BTreeMap::new(),
            structural_cases: BTreeMap::new(),
            scalar_array_values: BTreeMap::new(),
            byte_sequence_values: BTreeMap::new(),
            primitive_loan_values: BTreeMap::new(),
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
            external_completion: None,
            external_demands: Vec::new(),
            external_bindings: BTreeMap::new(),
            process_domain: None,
            established_erased_fields: established_erased_fields.to_vec(),
            effects: Vec::new(),
        };
        for argument_index in zero_initialized_arguments {
            let parameter = machine
                .structural_parameters
                .get(*argument_index as usize)
                .ok_or(TerminalInterpretError::ZeroInitializedArgumentInvalid {
                    argument_index: *argument_index,
                })?;
            let value = crate::custody::verified_structural_place(
                &execution.structural_values,
                parameter.place,
            )
            .cloned()?;
            // Host-supplied erased establishments arrive as identity text;
            // resolve every segment against this module's sealed table —
            // anything the producer never interned is malformed input, not
            // a fresh identity.
            let identities = &program.catalogs.identities;
            let established_fields = established_erased_fields
                .iter()
                .filter(|field| field.argument_index == *argument_index)
                .map(|field| -> Result<_, TerminalInterpretError> {
                    let field_path = field
                        .field_path
                        .iter()
                        .map(|segment| identities.index_of(segment))
                        .collect::<Option<Vec<_>>>()
                        .ok_or_else(malformed)?;
                    let carrier_type_identity = identities
                        .index_of(&field.carrier_type_identity)
                        .ok_or_else(malformed)?;
                    Ok(terminal_semantics::EstablishedErasedStructuralField {
                        field_path,
                        carrier_type_identity,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            execution.materialize_zeroed_structural_storage_with_established_fields(
                &value,
                &established_fields,
            )?;
        }
        let primitive_storage = std::mem::take(&mut execution.structural_primitive_storage);
        let (storage, entry_places) = bind_structural_primitive_values(
            machine,
            &program.structural_types(),
            &execution.structural_values,
            structural_primitive_value_arguments,
            primitive_storage,
            zero_initialized_arguments,
        )?;
        execution.structural_primitive_storage = storage;
        execution.structural_primitive_entry_places = entry_places;
        let scalar_fields = std::mem::take(&mut execution.structural_scalar_fields);
        execution.structural_scalar_fields = scalar_fields::bind(
            machine,
            &program.structural_types(),
            &execution.structural_values,
            structural_scalar_field_arguments,
            scalar_fields,
            zero_initialized_arguments,
        )?;
        execution.establish_private_entry_ram(private_entry_ram, placed_view_establishments)?;
        Ok(execution)
    }

    pub fn effects(&self) -> &[TerminalEffect] {
        &self.effects
    }

    /// Whether the entry invocation has reached a terminal outcome — a
    /// committed result or a crash. An unfinished execution still holds
    /// custody; it is not reclaimable by `TerminalInstance::close`.
    pub(crate) fn is_finished(&self) -> bool {
        self.result.is_some() || self.crash.is_some() || self.external_completion.is_some()
    }

    /// The committed result once the entry invocation has produced one.
    pub(crate) fn result(&self) -> Option<&TerminalExecutionResult> {
        self.result.as_ref()
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

    /// The scalar bound to `value`, or `VerifiedValueMissing` when the verified
    /// artifact references a value the interpreter never produced.
    pub(crate) fn scalar_value(
        &self,
        value: ValueId,
    ) -> Result<TerminalScalarValue, TerminalInterpretError> {
        self.values
            .get(&value)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(value))
    }

    /// The unsigned integer bound to `value`, malformed when its declared
    /// scalar type differs from `expected_type` or it is not an unsigned
    /// integer at all.
    pub(crate) fn unsigned_integer_value(
        &self,
        value: ValueId,
        expected_type: IntegerType,
    ) -> Result<u64, TerminalInterpretError> {
        let TerminalScalarValue::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(value),
        } = self.scalar_value(value)?
        else {
            return Err(malformed());
        };
        if scalar_type != expected_type {
            return Err(malformed());
        }
        u64::try_from(value).map_err(|_| malformed())
    }

    /// Every scalar bound to `arguments`, in order.
    pub(crate) fn scalar_values_at(
        &self,
        arguments: &[ValueId],
    ) -> Result<Vec<TerminalScalarValue>, TerminalInterpretError> {
        arguments
            .iter()
            .map(|argument| self.scalar_value(*argument))
            .collect()
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

    /// Exact live affine structural paths, ordered canonically. This is
    /// semantic ownership state, not a runtime object-layout bitmap.
    pub fn live_affine_frontier(&self) -> impl Iterator<Item = &StructuralAffineDiscard> + '_ {
        self.live_affine_frontier.iter()
    }

    /// The live affine discard covering `place`, when the frontier holds one.
    pub fn live_affine_discard_at(&self, place: PlaceId) -> Option<&StructuralAffineDiscard> {
        self.live_affine_frontier
            .iter()
            .find(|entry| entry.place == place)
    }

    pub fn resume(
        &mut self,
        meter: &mut TerminalFuelMeter,
        handler: &mut impl TerminalEffectHandler,
    ) -> Result<TerminalExecutionStatus, TerminalInterpretError> {
        self.resume_operations(meter, handler).map_err(|error| {
            // Report the execution cursor that declined the operation, including
            // failures reached in a callee. A missing operation means the block
            // terminator failed; source-free execution retains canonical IDs.
            // Preserve the cause for every refusal, not only malformed rows:
            // a missing home needs its consuming operation or cleanup edge.
            let operation = self
                .program
                .machines()
                .get(&self.current_machine)
                .and_then(|machine| machine.block(&self.current))
                .and_then(|block| block.operations.get(self.next_operation))
                .map(|operation| operation.id);
            TerminalInterpretError::ExecutionSite {
                machine: self.current_machine,
                block: self.current,
                operation,
                cause: Box::new(error),
            }
        })
    }

    fn resume_operations(
        &mut self,
        meter: &mut TerminalFuelMeter,
        handler: &mut impl TerminalEffectHandler,
    ) -> Result<TerminalExecutionStatus, TerminalInterpretError> {
        if let Some(completion) = &self.external_completion {
            return Ok(TerminalExecutionStatus::ExternalCompletion(
                completion.clone(),
            ));
        }
        if self.result.is_some() {
            self.retire_placed_view_occurrences();
            let result = self.result.clone().expect("checked above");
            return Ok(TerminalExecutionStatus::Complete(result));
        }
        if let Some(crash) = &self.crash {
            return Ok(TerminalExecutionStatus::Crashed(crash.clone()));
        }

        let missing = self
            .external_demands
            .iter()
            .copied()
            .filter(|demand| {
                !self
                    .external_bindings
                    .get(&demand.boundary)
                    .is_some_and(|admission| {
                        admission.answers(
                            self.program.identity,
                            self.process_domain.as_ref(),
                            *demand,
                        )
                    })
            })
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(TerminalInterpretError::UnboundExternalCompletions { missing });
        }

        let program = std::sync::Arc::clone(&self.program);
        loop {
            // A settled block boundary has no transaction-local constructor
            // payload. Suspended nominal cleanups are excluded by reclamation.
            self.reclaim_orphan_scalar_records();
            while let Some(operation) = program
                .verified_machine(self.current_machine)?
                .block(&self.current)
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
                let flow = dispatch_operations!(self, operation, operation.kind;
                                    literal {
                                    OperationKind::EstablishPlacedReference { ref source } => {
                                        self.establish_placed_reference(operation, source)?;
                                        OperationFlow::Advance
                                    }
                                    OperationKind::EstablishReference {
                                        ref source,
                                        storage,
                                    } => {
                                        self.establish_reference(operation, source, storage)?;
                                        OperationFlow::Advance
                                    }
                                    OperationKind::ReleaseReference { source, ref path } => {
                                        self.release_reference(operation, source, path)?;
                                        OperationFlow::Advance
                                    }
                                    OperationKind::EstablishScalarArray { ref elements } => {
                                        self.execute_scalar_array_establishment(operation, elements)?;
                                        OperationFlow::Advance
                                    }
                                    OperationKind::EstablishPrimitiveLocal { value, .. } => {
                                        self.execute_primitive_establishment(operation, value)?;
                                        OperationFlow::Advance
                                    }
                                    OperationKind::PrimitiveScalarRead { source, ref path } => {
                                        self.execute_primitive_read(operation, source, path)?;
                                        OperationFlow::Advance
                                    }
                                    OperationKind::BoundaryCall { .. } => {
                                        self.execute_boundary_call(operation, handler)?
                                    }
                                    OperationKind::PortWrite { .. } => {
                                        self.execute_port_write(operation, handler)?
                                    }
                                    OperationKind::Call { .. } => {
                                        self.execute_call(operation, &program)?
                                    }
                                    OperationKind::SelectImplementation { table } => {
                                        self.execute_select_implementation(operation, table, &program)?
                                    }
                                    OperationKind::CallDynamic { .. } => {
                                        self.execute_call_dynamic(operation, &program)?
                                    }
                                    OperationKind::WriteOnlyPrimitiveStore {
                                        destination,
                                        value,
                                        ref path,
                                        ..
                                    } => {
                                        self.execute_primitive_store(operation, destination, value, path)?;
                                        OperationFlow::Advance
                                    }
                                    OperationKind::AtomicFence { .. } => {
                                        // The core executes one ordered event at a time. A fence
                                        // consumes this normal operation step without observing
                                        // storage or manufacturing synchronization evidence.
                                        OperationFlow::Advance
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
                                        }
                dispatch {
                                    EstablishZeroedStructural => advance execute_establish_zeroed_structural,
                                    EstablishStructuralLocal => advance execute_establish_structural_local,
                                    EstablishScalarCase => flow execute_establish_scalar_case,
                                    EstablishByteSequenceConcatenation => flow execute_byte_sequence_concatenation,
                                    EstablishByteSequenceLiteral => flow execute_establish_byte_sequence_literal,
                                    EstablishTrivialAffineLocal => flow execute_establish_trivial_affine_local,
                                    EstablishStructuralArray => flow execute_establish_structural_array,
                                    EstablishRecord => flow execute_establish_record,
                                    EstablishStructuralCase => flow execute_establish_structural_case,
                                    CallStructuralScalar => flow execute_call_structural_scalar,
                                    CallStructural => flow execute_call_structural,
                                    CallStructuralWithScalarArguments => flow execute_call_structural_with_scalar_arguments,
                                    CallUnit => flow execute_call_unit,
                                    StructuralScalarFieldStore => flow execute_structural_scalar_field_store,
                                    MoveStructuralField => advance execute_move_structural_field,
                                    StoreStructuralField => advance execute_store_structural_field,
                                    ByteSequenceStore => advance execute_byte_sequence_store,
                                    StructuralByteSequenceFieldStore => advance execute_structural_byte_sequence_field_store,
                                    StructuralByteViewFieldStore => advance execute_structural_byte_view_field_store,
                                    StructuralByteSequenceFieldLength => advance execute_structural_byte_sequence_field_length,
                                    StructuralByteSequenceFieldByteStore => advance execute_structural_byte_sequence_field_byte_store,
                                    StructuralByteSequenceFieldRead => advance execute_structural_byte_sequence_field_read,
                                    IntegerConstant => flow execute_integer_constant,
                                    IeeeFloatConstant => flow execute_ieee_float_constant,
                                    IeeeFloatCompare => flow execute_ieee_float_compare,
                                    ScalarRepresentationView => flow execute_scalar_representation_view,
                                    NearestIeeeFloatFusedMultiplyAdd => flow execute_nearest_ieee_float_fused_multiply_add,
                                    BooleanConstant => flow execute_boolean_constant,
                                    StructuralCaseMembership => flow execute_structural_case_membership,
                                    StructuralLeafCopy | StructuralCaseLeafCopy => advance execute_structural_leaf_copy,
                                    BooleanStructuralField => flow execute_boolean_structural_field,
                                    ScalarStructuralField => flow execute_scalar_structural_field,
                                    BooleanNot => flow execute_boolean_not,
                                    BooleanEqual => flow execute_boolean_equal,
                                    EstablishElementView | EstablishMutableElementView => flow execute_establish_element_view,
                                    ElementViewLength => flow execute_element_view_length,
                                    ElementViewRead => flow execute_element_view_read,
                                    ElementViewWrite => flow execute_element_view_write,
                                    ElementViewSubslice => advance execute_element_view_subslice,
                                    ByteSequenceSubslice => advance execute_byte_sequence_subslice,
                                    ByteSequenceWrite => advance execute_byte_sequence_write,
                                    ByteSequencePredicate => flow execute_byte_sequence_predicate,
                                    ByteSequenceEqual => flow execute_byte_sequence_equal,
                                    ByteSequenceRead => flow execute_byte_sequence_read,
                                    ByteSequenceLength => flow execute_byte_sequence_length,
                                    IntegerLessThan | IntegerLessOrEqual => flow execute_integer_less_than,
                                    IntegerEqual => flow execute_integer_equal,
                                    IntegerBitwiseNot => flow execute_integer_bitwise_not,
                                    IntegerExactCast => flow execute_integer_exact_cast,
                                    IntegerWiden => flow execute_integer_widen,
                                    IntegerBitwiseAnd | IntegerBitwiseOr | IntegerBitwiseXor => flow execute_integer_bitwise_and,
                                    WrappingIntegerShiftLeft | WrappingIntegerShiftRight | ExactIntegerShiftLeft | ExactIntegerShiftRight => flow execute_wrapping_integer_shift_left,
                                    ExactIntegerAdd | WrappingIntegerAdd | ExactIntegerSubtract | WrappingIntegerSubtract | ExactIntegerMultiply | ExactIntegerDivide | ExactIntegerRemainder | WrappingIntegerDivide | WrappingIntegerRemainder | SaturatingIntegerDivide | SaturatingIntegerRemainder | WrappingIntegerMultiply => flow execute_exact_integer_add,
                                    SaturatingIntegerAdd => flow execute_saturating_integer_add,
                                    SaturatingIntegerSubtract => flow execute_saturating_integer_subtract,
                                    SaturatingIntegerMultiply => flow execute_saturating_integer_multiply,
                                    TrappingInteger => flow execute_trapping_integer,
                                    FloatResultPolicy => flow execute_float_result_policy
                                    }
                                );
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
                .verified_machine(self.current_machine)?
                .block(&self.current)
                .ok_or(TerminalInterpretError::VerifiedBlockMissing)?
                .terminator;
            let flow = match &terminator {
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
                Terminator::ExternalCompletion { .. } => {
                    self.settle_external_completion(terminator, meter)?
                }
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
