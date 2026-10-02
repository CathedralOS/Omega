//! Unit effect plans: the effect machines and every effect operation a
//! Unit-effect body performs.

use crate::checked_trees::flow::terminal::structural_return::CheckedUnitStructuralReturnPlan;
use crate::checked_trees::flow::terminal::{
    CheckedBoundaryMachinePlan, CheckedByteSequenceStorePlan, CheckedByteSequenceWritePlan,
    CheckedCallScalarArgument, CheckedComposedUnitControlMachinePlan, CheckedElementViewWritePlan,
    CheckedProviderAttachmentRequirementPlan, CheckedScalarStateTerminator,
    CheckedStructuralByteSequenceFieldByteStorePlan, CheckedStructuralScalarParameterPlan,
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitCallCoordinate, CheckedUnitCallPlan,
    CheckedUnitEntryClaimPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralDomainPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralResultBindingPlan,
    CheckedUnitStructuralTypePlan,
};
use crate::checked_trees::values::CheckedErasedProofParameterPlan;
use crate::{
    CheckedStructuralCaseFieldStorePlan, CheckedStructuralCaseLiteralFieldStorePlan,
    CheckedStructuralScalarFieldStorePlan,
};
use language_semantics::{SemanticDomainId, ServiceReachPlan, ServiceReachSummary};
use symbols::SymbolHandle;
use typed_trees::types::PrimitiveType;

/// Source-handle-free plans for the first general structural/Unit terminal
/// slice. These rows are assembled only after ownership and carry checking
/// have recorded their authoritative facts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedUnitEffectPlans {
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    pub structural_domains: Vec<CheckedUnitStructuralDomainPlan>,
    pub boundary_machines: Vec<CheckedBoundaryMachinePlan>,
    pub machines: Vec<CheckedUnitEffectMachinePlan>,
    /// Dynamic dispatch custody is grouped separately so direct and rebound
    /// descriptor/table lanes can evolve without repeatedly widening this
    /// general Unit-plan record.
    pub dynamic_dispatch: crate::CheckedDynamicDispatchPlans,
    /// Multi-state Unit machines whose exact control and effect rows were
    /// admitted as one atomic executable plan.
    pub composed_machines: Vec<CheckedComposedUnitControlMachinePlan>,
    /// One row per checked-body machine that has neither an ordinary nor a
    /// composed plan, naming the stage that omitted it and, for closure
    /// pruning, the direct dependency that was unavailable. Rows are
    /// diagnostic evidence for the producer that later reports a missing
    /// transitive plan; they grant nothing and select no fallback body.
    pub omissions: Vec<CheckedUnitPlanOmission>,
}

/// Why one checked-body machine has no Unit plan. `machine` never appears in
/// `machines` or `composed_machines` of the same record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedUnitPlanOmission {
    pub machine: SymbolHandle,
    pub stage: CheckedUnitPlanOmissionStage,
}

/// The planning stage at which a machine left the Unit plan roster. Closure
/// stages name the direct dependency that was unavailable; following
/// `UnavailableCallee` rows through the same record reaches the machine whose
/// own body failed local construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedUnitPlanOmissionStage {
    /// No ordinary or composed builder admitted the machine's own body.
    /// `phase` names the last phase the ordinary builder (single-state
    /// bodies) or the general state-graph builder (multi-state bodies)
    /// entered before it gave up, `state_index` the state and
    /// `statement_index` the statement it was planning, when known.
    LocalConstruction {
        phase: &'static str,
        state_index: Option<u32>,
        statement_index: Option<u32>,
    },
    /// The body was admitted, then dropped while receiver retention was
    /// propagated along the call graph (the rebuilt caller no longer
    /// retained `self`, or a receiver operand could not be rejoined).
    ReceiverReconciliation,
    /// Two builders described the same entry and neither survived the
    /// overlap resolution.
    CompetingCandidates,
    /// A composed body used an operation outside the composed vocabulary.
    ComposedVocabulary,
    /// A Unit, structural, or ordinary scalar call targets a machine that has
    /// no admitted body in the roster.
    UnavailableCallee { target: SymbolHandle },
    /// A boundary call targets a machine with no boundary plan.
    MissingBoundaryTarget { target: SymbolHandle },
    /// A scalar call has neither a registered scalar target nor an ordinary
    /// body to borrow.
    UnavailableScalarTarget { target: SymbolHandle },
}

impl CheckedUnitEffectPlans {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&CheckedUnitEffectMachinePlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }

    pub fn boundary_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedBoundaryMachinePlan> {
        self.boundary_machines
            .iter()
            .find(|plan| plan.machine == machine)
    }

    pub fn composed_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedComposedUnitControlMachinePlan> {
        self.composed_machines
            .iter()
            .find(|plan| plan.machine == machine)
    }

    /// The omission row for a machine that has no plan in this record.
    pub fn omission_for_machine(&self, machine: SymbolHandle) -> Option<&CheckedUnitPlanOmission> {
        self.omissions.iter().find(|row| row.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedUnitEffectOperationPlan {
    EstablishReference {
        result: CheckedUnitStructuralResultBindingPlan,
        source: CheckedUnitStructuralArgumentPlan,
    },
    ReleaseReference {
        statement_index: u32,
        binding_ordinal: u32,
        loan: arena::Handle<crate::BorrowLoanFact>,
    },
    /// Establish the byte value declared by one local initializer.
    /// The binding retains its declared borrowed or owned carrier. Only the
    /// authored byte-domain qualifications constrain later writes; incidental
    /// predicates of the initial octets do not become declared invariants.
    EstablishByteSequence {
        result: CheckedUnitStructuralResultBindingPlan,
        value: crate::CheckedByteSequenceValuePlan,
        qualifications: Vec<SemanticDomainId>,
    },
    /// Bind the statement's immutable view local to an exclusive range of an
    /// established view. `source` is the same `ByteSequenceSubslice` or
    /// `ElementViewSubslice` argument plan a call argument carries, with its
    /// endpoints keyed at the `LocalBinding` subslice site. The result is a
    /// shared, unrestricted view in the structural result namespace: it owns
    /// no storage, so its end is the end of the loan, never a disposal.
    EstablishViewSubslice {
        result: CheckedUnitStructuralResultBindingPlan,
        source: CheckedUnitStructuralArgumentPlan,
    },
    /// Establish a fresh structural expression in the shared result namespace.
    /// Selected arms transfer one new owner into the expression continuation.
    EstablishStructuralValue {
        result: CheckedUnitStructuralResultBindingPlan,
        value: crate::CheckedStructuralValueHandle,
        calls: Vec<CheckedStructuralValueCall>,
        /// `Some` when the value is an inline operand of a call statement,
        /// naming the consuming call coordinate and authored formal position;
        /// absent for statement-level constructions such as local bindings.
        operand_source: Option<crate::CheckedArrayConstructionSource>,
        discard_result_on_return: bool,
    },
    /// Construct an unrestricted primitive fixed array in authored row-major
    /// leaf order. Empty dimensions remain in the exact structural result type.
    EstablishScalarArray {
        source: crate::CheckedArrayConstructionSource,
        result: CheckedUnitStructuralResultBindingPlan,
        elements: Vec<CheckedCallScalarArgument>,
    },
    /// Establish initialized mutable storage at its authored declaration.
    /// Later reads and borrows name the symbol, never the initializer value.
    /// The initializer is the `StorageInitializer` value checking retained:
    /// a pure expression, or the computation rooted there when it evaluates
    /// calls or ordered selections (`let mut stored: i32 = echo(a);`).
    EstablishPrimitiveLocal {
        statement_index: u32,
        symbol: SymbolHandle,
        type_identity: String,
        primitive_type: PrimitiveType,
        value: CheckedCallScalarArgument,
    },
    /// Normal edge after the named call completes. Cleanup belongs to this
    /// continuation, not the caller's final return or a synthetic source call.
    CallContinuationCleanup {
        coordinate: CheckedUnitCallCoordinate,
        affine_discards: Vec<CheckedUnitPartialAffineDiscardPlan>,
    },
    EstablishTrivialAffineLocal {
        statement_index: u32,
        declaration_ordinal: u32,
        type_identity: String,
    },
    /// Establish one declared structural local's home at its authored
    /// declaration, publishing the binding that member operations and
    /// borrows rejoin by ordinal. The declaration supplies the home each
    /// member operation establishes into; which members reads may rely on
    /// is the custody ledger's question, not this root's.
    EstablishStructuralLocal {
        statement_index: u32,
        declaration_ordinal: u32,
        symbol: SymbolHandle,
        result: CheckedUnitStructuralResultBindingPlan,
        discard_result_on_return: bool,
    },
    /// Establish one immutable primitive local from a retained pure expression
    /// or selective computation. The result coordinate names the
    /// same dense scalar namespace used by result-bearing calls; later
    /// consumers may therefore refer to either kind without reconstructing a
    /// source expression.
    EstablishScalarLocal {
        result: CheckedUnitScalarResultBindingPlan,
        value: CheckedCallScalarArgument,
    },
    /// One ordinary or boundary call with a Unit, scalar or structural
    /// result. Selected operators keep their own operand lanes.
    Call(CheckedUnitCallPlan),
    PortWrite {
        coordinate: CheckedUnitCallCoordinate,
        port: u16,
        value: u8,
        service_reach: ServiceReachSummary,
    },
    /// Replace one unrestricted primitive through an exclusive borrowed
    /// parameter (by authored position) or an initialized mutable local.
    /// Primitive store custody stays separate from immutable scalar bindings.
    /// A nonempty declared path selects
    /// a primitive leaf without inventing a scalar field identity: fields,
    /// literal elements, and runtime elements (`RuntimeIndex`, whose selector
    /// is evaluated before the value) compose in one path. Evaluate the
    /// retained scalar expression or computation against current storage
    /// before committing the write.
    WriteOnlyPrimitiveStore {
        statement_index: u32,
        destination: crate::CheckedStoreRoot,
        path: Vec<CheckedUnitStructuralPathSegment>,
        value: crate::CheckedCallScalarArgument,
    },
    /// Replace one relevant primitive field through an exact common-field
    /// path, optionally followed by one literal fixed-array index, below an
    /// exclusive structural parameter. This shares the same checked
    /// path/value custody as dynamic realization stores, but is an ordinary
    /// attached-Unit effect and therefore carries no dispatch authority. The
    /// bounded result form reads only the exact fixed-integer local produced
    /// by the immediately preceding ordinary scalar call or selected
    /// boundary-operator realization.
    StructuralScalarFieldStore(CheckedStructuralScalarFieldStorePlan),
    /// Replace one relevant unrestricted-sum field through an exact common-field
    /// path below an exclusive structural parameter. The stored value is a whole
    /// owned place of the field's declared sum type: an unrestricted sum carries
    /// no erased or payload custody, so the write copies whole and consumes
    /// nothing.
    StructuralCaseFieldStore(CheckedStructuralCaseFieldStorePlan),
    /// Replace one sum field through the same exact path with an authored
    /// case literal: the store itself establishes the construction into the
    /// field's storage — tag, authored scalar operands and zero-initialized
    /// unnamed commons — copying nothing that was ever a held place.
    StructuralCaseLiteralFieldStore(CheckedStructuralCaseLiteralFieldStorePlan),
    ByteSequenceStore(CheckedByteSequenceStorePlan),
    /// One normalized atomic event on an exact primitive leaf, joining the
    /// source carrier's placeholder local and carrier assignment into one
    /// operation (see `atomic_plans.rs`).
    AtomicAccess(crate::CheckedAtomicAccessPlan),
    StructuralByteSequenceFieldByteStore(CheckedStructuralByteSequenceFieldByteStorePlan),
    ByteSequenceWrite(CheckedByteSequenceWritePlan),
    /// Write one runtime-selected element through a mutable `&mut [T]` view
    /// (a borrowed slice parameter or a locally bound `&mut` view local):
    /// `view[index] = element`. The live-length observation composes with
    /// the write exactly as the byte-view write's does.
    ElementViewWrite(CheckedElementViewWritePlan),
    /// Replace one primitive leaf re-covered by a `&mut`/`&write`
    /// representation view — one row for the authored `view = v`, however
    /// many footprint bytes the view spans at lowering.
    RepresentationViewStore(crate::CheckedRepresentationViewStorePlan),
    /// Move one whole structural field out of exclusive borrowed storage into
    /// a fresh owned binding, opening a repair window the body must close.
    /// The source is a parameter-rooted exact field chain; the result binding
    /// names the authored local that receives the moved value.
    MoveStructuralField {
        result: CheckedUnitStructuralResultBindingPlan,
        source: CheckedUnitStructuralArgumentPlan,
    },
    /// Restore one open borrowed window by storing a whole owned structural
    /// binding into the exact absent place. The checked multiplicity gate
    /// proved the destination is an open hole and the value carries its type.
    StoreStructuralField {
        statement_index: u32,
        destination: CheckedUnitStructuralArgumentPlan,
        /// Whole owned place carrying the hole's declared type.
        value: CheckedUnitStructuralArgumentPlan,
    },
    /// Finish the body after cleanup, yielding its retained structural/scalar
    /// binding or scalar control result when present, and Unit otherwise.
    Complete {
        statement_index: u32,
        /// Exact local declaration coordinates cleaned before parameters, in
        /// reverse declaration order.
        trivial_affine_local_discard_ordinals: Vec<u32>,
        /// Owned affine structural parameters discarded on this return edge,
        /// in reverse declaration order.
        trivial_affine_discards: Vec<u32>,
    },
}

/// Operand preparation belongs to its consuming call and runs when that call
/// is evaluated. Operation slots use an arena span; scratch vectors are consumed
/// at publication.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedOperandSequence {
    operations: arena::Arena<OperandOperation>,
    order: arena::HandleSpan<OperandOperation>,
}

// The arena's zero slot denotes absence. Published spans contain only occupied
// operations, and the private fields keep that construction invariant local.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct OperandOperation(Option<crate::CheckedUnitEffectOperationPlan>);

impl CheckedOperandSequence {
    pub fn new(
        operations: impl IntoIterator<Item = crate::CheckedUnitEffectOperationPlan>,
    ) -> Self {
        let mut sequence = Self::default();
        for operation in operations {
            sequence
                .operations
                .append_to_span(&mut sequence.order, OperandOperation(Some(operation)));
        }
        sequence
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &crate::CheckedUnitEffectOperationPlan> {
        self.operations
            .span_or_empty(self.order)
            .iter()
            .map(|operation| {
                operation
                    .0
                    .as_ref()
                    .expect("published operand span contains operations")
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralValueCall {
    pub value: crate::CheckedStructuralValueHandle,
    operands: CheckedOperandSequence,
    operation: CheckedUnitEffectOperationPlan,
}

impl CheckedStructuralValueCall {
    pub fn new(
        value: crate::CheckedStructuralValueHandle,
        operation: CheckedUnitEffectOperationPlan,
    ) -> Option<Self> {
        Self::with_operands(value, CheckedOperandSequence::default(), operation)
    }

    pub fn with_operands(
        value: crate::CheckedStructuralValueHandle,
        operands: CheckedOperandSequence,
        operation: CheckedUnitEffectOperationPlan,
    ) -> Option<Self> {
        // Only an ordinary structural call produces a value operand; a
        // boundary result keeps its own statement-level binding.
        operation
            .call()
            .is_some_and(|call| !call.is_boundary() && call.structural_result().is_some())
            .then_some(Self {
                value,
                operands,
                operation,
            })
    }

    pub fn operands(&self) -> &CheckedOperandSequence {
        &self.operands
    }

    pub fn operation(&self) -> &CheckedUnitEffectOperationPlan {
        &self.operation
    }
}

impl CheckedUnitEffectOperationPlan {
    /// The binding this operation adds to the shared structural namespace.
    /// Literal declarations and constructed/called values use the same dense
    /// ordinal space; their distinct storage and ownership remain on the variant.
    pub fn structural_result_binding(&self) -> Option<&CheckedUnitStructuralResultBindingPlan> {
        match self {
            Self::EstablishReference { result, .. }
            | Self::EstablishByteSequence { result, .. }
            | Self::EstablishViewSubslice { result, .. }
            | Self::EstablishStructuralValue { result, .. }
            | Self::EstablishScalarArray { result, .. }
            | Self::EstablishStructuralLocal { result, .. }
            | Self::MoveStructuralField { result, .. } => Some(result),
            Self::Call(call) => call.structural_result(),
            _ => None,
        }
    }

    /// The authored assignment a store-family row writes: a primitive,
    /// projected scalar, whole-sum, byte-sequence or indexed byte field store,
    /// or a byte- or element-view write. One assignment may own several rows
    /// of one family (each member of a record literal, each element of an
    /// array literal). Repair stores into an open borrowed window and atomic
    /// events keep their own statement custody and answer `None`, as does
    /// every operation outside the store family.
    pub fn store_statement_index(&self) -> Option<u32> {
        match self {
            Self::WriteOnlyPrimitiveStore {
                statement_index, ..
            } => Some(*statement_index),
            Self::StructuralScalarFieldStore(store) => Some(store.statement_index),
            Self::StructuralCaseFieldStore(store) => Some(store.statement_index),
            Self::StructuralCaseLiteralFieldStore(store) => Some(store.statement_index),
            Self::ByteSequenceStore(store) => Some(store.statement_index),
            Self::StructuralByteSequenceFieldByteStore(store) => Some(store.statement_index),
            Self::ByteSequenceWrite(write) => Some(write.statement_index),
            Self::ElementViewWrite(write) => Some(write.statement_index),
            Self::RepresentationViewStore(store) => Some(store.statement_index),
            _ => None,
        }
    }

    /// The authored structural arguments of an ordinary call — Unit, scalar,
    /// structural or boundary — in argument order; empty for every other
    /// operation. Selected-operator calls keep their own operand lanes.
    pub fn call_structural_arguments(&self) -> &[CheckedUnitStructuralArgumentPlan] {
        self.call()
            .map(|call| call.structural_arguments.as_slice())
            .unwrap_or_default()
    }

    /// The ordinary or boundary call this operation performs; `None` for
    /// every other operation. Selected operators keep their own operand lanes.
    pub fn call(&self) -> Option<&CheckedUnitCallPlan> {
        match self {
            Self::Call(call) => Some(call),
            _ => None,
        }
    }

    pub fn call_mut(&mut self) -> Option<&mut CheckedUnitCallPlan> {
        match self {
            Self::Call(call) => Some(call),
            _ => None,
        }
    }

    /// The structural result binding of a call or a constructed value, the
    /// producers that own an automatic return discard, with that flag. The
    /// other structural producers answer `None`.
    pub fn discardable_structural_result(
        &self,
    ) -> Option<(&CheckedUnitStructuralResultBindingPlan, bool)> {
        match self {
            Self::EstablishStructuralValue {
                result,
                discard_result_on_return,
                ..
            }
            | Self::EstablishStructuralLocal {
                result,
                discard_result_on_return,
                ..
            } => Some((result, *discard_result_on_return)),
            Self::Call(call) => call
                .structural_result()
                .zip(call.discard_result_on_return()),
            _ => None,
        }
    }

    /// [`Self::discardable_structural_result`] with its flag writable.
    pub fn discardable_structural_result_mut(
        &mut self,
    ) -> Option<(&mut CheckedUnitStructuralResultBindingPlan, &mut bool)> {
        match self {
            Self::EstablishStructuralValue {
                result,
                discard_result_on_return,
                ..
            }
            | Self::EstablishStructuralLocal {
                result,
                discard_result_on_return,
                ..
            } => Some((result, discard_result_on_return)),
            Self::Call(call) => call.structural_result_mut(),
            _ => None,
        }
    }

    /// Enumerate retained operation dependencies for signature/catalog readers.
    /// This is not execution order: structural expressions evaluate their call
    /// operands in their own authored field and selected-arm order.
    pub fn with_value_calls(&self) -> impl Iterator<Item = &Self> {
        let mut pending = vec![self];
        std::iter::from_fn(move || {
            let operation = pending.pop()?;
            if let Self::EstablishStructuralValue { calls, .. } = operation {
                for call in calls.iter().rev() {
                    pending.push(call.operation());
                    pending.extend(call.operands().iter().rev());
                }
            }
            Some(operation)
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitScalarControlPlan {
    pub primitive_type: PrimitiveType,
    /// Exact authored conditional and return coordinates after the ordered body.
    pub terminator: CheckedScalarStateTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitEffectMachinePlan {
    /// Final ordinary scalar binding, mutually exclusive with the other completions.
    pub scalar_result: Option<CheckedUnitScalarResultBindingPlan>,
    /// Scalar control completion, mutually exclusive with either result binding.
    pub scalar_control: Option<CheckedUnitScalarControlPlan>,
    /// Whole structural value returned after ordinary sequencing, with its
    /// actual source and multiplicity retained by the completion plan.
    pub structural_result: Option<CheckedUnitStructuralReturnPlan>,
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    /// Exact attached data carrier, or `None` for an ordinary free machine.
    pub attachment_type_identity: Option<String>,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    /// Primitive parameters in authored order after removing structural
    /// parameters into their independent custody namespace.
    pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    /// Proof-only erased scalar formals in authored order, retaining their
    /// authored parameter positions. They own no runtime argument lane.
    pub erased_scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    /// Erased formals whose carriers are proof-only and admit no scalar
    /// lane. The contract term lane carries their semantic type identities.
    pub erased_proof_parameters: Vec<CheckedErasedProofParameterPlan>,
    /// Exact boundary requirements replacing one authored provider-backed
    /// attachment field. Empty means no attachment specialization occurred.
    pub provider_attachment_requirements: Vec<CheckedProviderAttachmentRequirementPlan>,
    /// Dense source-order declarations for the bounded empty-record affine
    /// local prefix.
    pub trivial_affine_locals: Vec<CheckedTrivialAffineStructuralLocalPlan>,
    pub entry_claims: Vec<CheckedUnitEntryClaimPlan>,
    /// Canonical sorted domains from `QualificationFacts`.
    pub body_qualifications: Vec<SemanticDomainId>,
    pub contract_report_fingerprint: u64,
    pub contract_commitment: crate::MachineContractCommitment,
    pub contract_service_reach: ServiceReachPlan,
    pub service_reach: ServiceReachSummary,
    pub operations: Vec<CheckedUnitEffectOperationPlan>,
}
