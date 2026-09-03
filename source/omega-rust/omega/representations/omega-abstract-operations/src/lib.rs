#![forbid(unsafe_code)]

//! Source-independent Omega realization requirements lowered from terminal Psi.
//!
//! This small representation is the replacement seed for the legacy
//! source-shaped abstract-operation plan. It deliberately carries stable Psi
//! provenance and scalar semantics, but no syntax tree, arena handle,
//! `ExpressionHandle`, source statement, target register, or storage choice.

use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, EdgeId, FuelScheduleIdentity, IeeeFloatFormat,
    IeeeFloatValue, IntegerType, IntegerValue, MachineId, ObligationId, OperationId, PlaceId,
    Proposition, ScalarType, ServiceId, StructuralCaseId, StructuralTypeId, ValueId,
};
use psi_terminal::{
    BoundaryMachineDeclaration, ClaimTransfer, ClosedConformanceApplication, CompletionReceipt,
    ContentEntryClaim, CrashCause, CrashRouteBucket, EntryClaim, OutcomeSpecificCallEvidence,
    ProviderCandidateConformance, StructuralArgument, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralResultClaimTransfer, StructuralResultDeclaration,
    StructuralTypeDeclaration, TerminalAffineCleanupAction, TerminalDynamicConformanceSelection,
    TerminalDynamicDescriptorArgument, TerminalDynamicDescriptorParameter,
    TerminalIndirectDynamicDispatch, TerminalModule, TerminalParameterDynamicDispatch,
    TerminalPsiIdentity, TerminalRankedScc, TerminalReboundDynamicDescriptor,
    TerminalStoredDynamicDescriptor, TerminalStoredDynamicDispatch,
};

/// Exact caller claim source needed to replay boundary-completion custody after
/// the verified module is discarded. Content-bearing sources retain their full
/// entry-version subject and owner-unique projection/algebra catalog rather
/// than collapsing to a generic whole-root claim identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompletionClaimSource {
    pub claim: ClaimId,
    /// Ordinary structural claim source, when this claim participates in the
    /// whole-value frontier.
    pub entry: Option<EntryClaim>,
    /// Exact content subject and projection/algebra catalog, when this claim
    /// also participates in content conservation.
    pub content: Option<ContentEntryClaim>,
}

impl CompletionClaimSource {
    pub const fn claim(&self) -> ClaimId {
        self.claim
    }

    pub fn input(&self) -> PlaceId {
        match &self.entry {
            Some(source) => source.input,
            None => match &self.content {
                Some(source) => source.input.root,
                None => unreachable!(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractOperationPlan {
    pub psi: TerminalPsiIdentity,
    pub entry: MachineId,
    /// Concrete target-neutral carrier shapes retained for Omega-owned layout
    /// and ABI selection. These rows contain no source handles or target
    /// offsets.
    pub structural_types: Vec<StructuralTypeDeclaration>,
    /// Exact bodyless boundary declarations available to Unit operations.
    pub boundary_machines: Vec<BoundaryMachineDeclaration>,
    /// Complete verifier-approved checked provider catalog. Target/provider
    /// installation selects from these exact terminal IDs without changing
    /// terminal-Psi semantic identity.
    pub provider_candidates: Vec<ProviderCandidateConformance>,
    pub functions: Vec<AbstractFunction>,
}

/// Native-ranked admission kept beside, rather than inside, the ordinary
/// abstract-operation plan. Existing acyclic plan and function constructors
/// therefore remain unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedNativeAbstractOperationPlan {
    pub plan: AbstractOperationPlan,
    pub countdown: RankedU32CountdownCustody,
}

/// Exact source-free custody for the first native-ranked control slice.
///
/// The SCC owns rank and covered-edge meaning. `graph` retains the remaining
/// concrete operation/edge coordinates needed for later lowering to replay
/// that meaning without recognizing source syntax. Canonical semantic/proof
/// bytes and complete relevant frontier projections are data, not authority:
/// the object boundary re-runs native and fixed-fuel verification over them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedU32CountdownCustody {
    pub semantic_replay: TerminalModule,
    pub proof_replay: Vec<u8>,
    pub ranked_scc: TerminalRankedScc,
    pub fixed_fuel: RankedFixedEntryFuel,
    pub graph: RankedU32CountdownGraph,
    pub structural_frontiers: RankedMachineStructuralFrontiers,
}

/// Exact public projection of the independently derived ranked entry theorem.
/// This record is intentionally constructible data; consumers acquire
/// authority only by deriving the semantic certificate again and comparing
/// every field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedFixedEntryFuel {
    pub terminal_psi: TerminalPsiIdentity,
    pub schedule: FuelScheduleIdentity,
    pub entry: MachineId,
    pub relevant_preconditions: Vec<Proposition>,
    pub ceiling_units: u64,
}

impl RankedFixedEntryFuel {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn schedule(&self) -> FuelScheduleIdentity {
        self.schedule
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub fn relevant_preconditions(&self) -> &[Proposition] {
        &self.relevant_preconditions
    }

    pub const fn ceiling_units(&self) -> u64 {
        self.ceiling_units
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedLiveClaim {
    pub claim: ClaimId,
    pub input: Option<PlaceId>,
    pub path: Vec<StructuralPathSegment>,
    pub multiplicity: Option<StructuralMultiplicity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankedOwnedStructuralPlace {
    pub place: PlaceId,
    pub multiplicity: StructuralMultiplicity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedPartialStructuralCustody {
    pub place: PlaceId,
    pub moved_paths: Vec<Vec<StructuralPathSegment>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedStructuralOwnershipFrontier {
    pub claims: Vec<RankedLiveClaim>,
    pub owned_places: Vec<RankedOwnedStructuralPlace>,
    pub partial_custody: Vec<RankedPartialStructuralCustody>,
}

impl RankedStructuralOwnershipFrontier {
    pub fn claims(&self) -> &[RankedLiveClaim] {
        &self.claims
    }

    pub fn owned_places(&self) -> &[RankedOwnedStructuralPlace] {
        &self.owned_places
    }

    pub fn partial_custody(&self) -> &[RankedPartialStructuralCustody] {
        &self.partial_custody
    }
}

/// The two ownership snapshots relevant to the one admitted ranked backedge.
/// Coordinates are retained explicitly so unrelated block/edge queries fail
/// closed instead of aliasing either snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedMachineStructuralFrontiers {
    pub machine: MachineId,
    pub header: BlockId,
    pub backedge: EdgeId,
    pub header_entry: RankedStructuralOwnershipFrontier,
    pub backedge_exit: RankedStructuralOwnershipFrontier,
}

impl RankedMachineStructuralFrontiers {
    pub fn block_entry(&self, block: BlockId) -> Option<&RankedStructuralOwnershipFrontier> {
        (block == self.header).then_some(&self.header_entry)
    }

    pub fn edge_exit(&self, edge: EdgeId) -> Option<&RankedStructuralOwnershipFrontier> {
        (edge == self.backedge).then_some(&self.backedge_exit)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankedU32CountdownGraph {
    pub entry: BlockId,
    pub preheader_edge: EdgeId,
    pub initial_value: ValueId,
    pub zero_operation: OperationId,
    pub zero_value: ValueId,
    pub compare_operation: OperationId,
    pub false_exit_edge: EdgeId,
    pub done_block: BlockId,
    pub one_operation: OperationId,
    pub one_value: ValueId,
    pub subtract_operation: OperationId,
    pub subtract_obligation: ObligationId,
    pub return_edge: EdgeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractFunction {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
    pub entry: BlockId,
    /// Runtime values supplied by the caller, in declared terminal-Psi order.
    pub parameters: Vec<AbstractParameter>,
    pub structural_parameters: Vec<StructuralParameterDeclaration>,
    pub result: AbstractFunctionResult,
    /// Generic live claims supplied by the caller/root installation.
    pub entry_claims: Vec<EntryClaim>,
    /// Exact verified service ceiling retained for realization and audit.
    pub published_service_ceiling: Vec<ServiceId>,
    /// Canonical block starts in `operations`. This keeps conditional targets
    /// source-independent without flattening away control-flow identity.
    pub block_entries: Vec<AbstractBlockEntry>,
    /// Operations in canonical block order. Straight-line functions retain
    /// their historical executable order.
    pub operations: Vec<AbstractOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractBlockEntry {
    pub block: BlockId,
    /// Scalar parameters in canonical Terminal-Psi declaration order. This is
    /// retained independently of incoming bindings so entry and otherwise
    /// unreferenced declarations cannot disappear during lowering.
    pub parameters: Vec<AbstractParameter>,
    pub operation_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbstractParameter {
    pub value: ValueId,
    pub scalar_type: ScalarType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbstractResult {
    pub value: ValueId,
    pub scalar_type: ScalarType,
}

/// Exact target-neutral custody for one rebound dynamic invocation.
///
/// The two selections retain the initializer and latest runtime source. The
/// descriptor retains their version relation. `initial_application` preserves
/// the initializer's exact conformance even when the rebound changes it;
/// `application` is the latest complete canonical private-table map, while
/// `dispatch` identifies the sole row permitted at this call site. Realization
/// machines are table content, not statically addressed call targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractReboundDynamicDispatch {
    pub initial: TerminalDynamicConformanceSelection,
    pub rebound: TerminalDynamicConformanceSelection,
    pub descriptor: TerminalReboundDynamicDescriptor,
    pub initial_application: ClosedConformanceApplication,
    pub application: ClosedConformanceApplication,
    pub dispatch: TerminalIndirectDynamicDispatch,
}

/// Exact target-neutral custody for one descriptor stored in an aggregate
/// field. Establishment and dispatch remain separate operations, but both
/// retain this same source selection and closed application instead of
/// reconstructing either from a descriptor ordinal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractStoredDynamicDescriptor {
    pub selection: TerminalDynamicConformanceSelection,
    pub descriptor: TerminalStoredDynamicDescriptor,
    pub application: ClosedConformanceApplication,
}

impl AbstractStoredDynamicDescriptor {
    pub fn has_complete_custody(&self, owner: MachineId, operation: OperationId) -> bool {
        self.descriptor.owner == owner
            && self.descriptor.establishment_operation == operation
            && self.selection.owner == owner
            && self.selection.ordinal == self.descriptor.selection_ordinal
            && self.application.owner == owner
            && !self.descriptor.aggregate_type_identity.is_empty()
            && !self.descriptor.field_identity.is_empty()
            && self.selection.conformance_application_report_fingerprint
                == self.application.report_fingerprint
            && self.selection.conformance_application_commitment == self.application.commitment
            && self.application.report_fingerprint
                == psi_terminal::closed_conformance_application_report_fingerprint(
                    &self.application,
                )
            && self.application.commitment
                == psi_terminal::closed_conformance_application_commitment(&self.application)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractStoredDynamicDispatch {
    pub stored: AbstractStoredDynamicDescriptor,
    pub dispatch: TerminalStoredDynamicDispatch,
}

impl AbstractStoredDynamicDispatch {
    pub fn has_complete_custody(&self, owner: MachineId, operation: OperationId) -> bool {
        self.stored
            .has_complete_custody(owner, self.stored.descriptor.establishment_operation)
            && self.dispatch.owner == owner
            && self.dispatch.operation == operation
            && self.dispatch.descriptor_ordinal == self.stored.descriptor.ordinal
            && self
                .stored
                .application
                .rows
                .iter()
                .filter(|row| {
                    row.declaring_trait_identity == self.dispatch.declaring_trait_identity
                        && row.public_requirement_identity
                            == self.dispatch.public_requirement_identity
                        && row.requirement_identity == self.dispatch.requirement_identity
                        && row.realization_identity == self.dispatch.realization_identity
                        && row.realization_callable_identity.as_deref()
                            == Some(self.dispatch.realization_callable_identity.as_str())
                })
                .count()
                == 1
            && self
                .stored
                .application
                .realization_callables
                .iter()
                .filter(|callable| {
                    callable.source_callable_identity == self.dispatch.realization_callable_identity
                        && callable.machine == self.dispatch.realization
                })
                .count()
                == 1
    }
}

/// One target-neutral dynamic descriptor argument after the Terminal catalog
/// has been independently rejoined. The target parameter is retained beside
/// the exact source custody so later ABI selection never has to recover either
/// side from owner-local ordinals alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractDynamicDescriptorArgument {
    pub argument: TerminalDynamicDescriptorArgument,
    pub target: TerminalDynamicDescriptorParameter,
    pub source: AbstractDynamicDescriptorSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractDynamicDescriptorSource {
    Selection {
        selection: TerminalDynamicConformanceSelection,
        application: ClosedConformanceApplication,
    },
    Rebound {
        initial: TerminalDynamicConformanceSelection,
        rebound: TerminalDynamicConformanceSelection,
        descriptor: TerminalReboundDynamicDescriptor,
        initial_application: ClosedConformanceApplication,
        application: ClosedConformanceApplication,
    },
    Parameter(TerminalDynamicDescriptorParameter),
}

impl AbstractDynamicDescriptorArgument {
    /// Replay one caller-to-callee existential interface join without treating
    /// either repeated ordinal as authority.
    pub fn has_complete_custody(
        &self,
        caller: MachineId,
        operation: OperationId,
        callee: MachineId,
    ) -> bool {
        if self.argument.owner != caller
            || self.argument.operation != operation
            || self.argument.parameter_ordinal != self.target.ordinal
            || self.target.owner != callee
        {
            return false;
        }
        let interfaces_match = |source: &TerminalDynamicDescriptorParameter| {
            source.trait_identity == self.target.trait_identity
                && source.access == self.target.access
                && source.requirements == self.target.requirements
        };
        match (&self.argument.source, &self.source) {
            (
                psi_terminal::TerminalDynamicDescriptorSource::Selection { ordinal },
                AbstractDynamicDescriptorSource::Selection {
                    selection,
                    application,
                },
            ) => {
                selection.owner == caller
                    && selection.ordinal == *ordinal
                    && application.owner == caller
                    && selection.conformance_application_report_fingerprint
                        == application.report_fingerprint
                    && selection.conformance_application_commitment == application.commitment
                    && application.report_fingerprint != 0
                    && !application.commitment.is_zero()
                    && application.report_fingerprint
                        == psi_terminal::closed_conformance_application_report_fingerprint(
                            application,
                        )
                    && application.commitment
                        == psi_terminal::closed_conformance_application_commitment(application)
                    && selection.source.access == self.target.access
                    && application.trait_identity == self.target.trait_identity
                    && application.rows.len() == self.target.requirements.len()
                    && application.rows.iter().zip(&self.target.requirements).all(
                        |(row, requirement)| {
                            row.declaring_trait_identity == requirement.declaring_trait_identity
                                && row.public_requirement_identity
                                    == requirement.public_requirement_identity
                                && row
                                    .realization_callable_identity
                                    .as_ref()
                                    .and_then(|identity| {
                                        application.realization_callables.iter().find(|callable| {
                                            callable.source_callable_identity == *identity
                                        })
                                    })
                                    .is_some_and(|callable| callable.result == requirement.result)
                        },
                    )
            }
            (
                psi_terminal::TerminalDynamicDescriptorSource::Parameter { ordinal },
                AbstractDynamicDescriptorSource::Parameter(source),
            ) => source.owner == caller && source.ordinal == *ordinal && interfaces_match(source),
            (
                psi_terminal::TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal },
                AbstractDynamicDescriptorSource::Rebound {
                    initial,
                    rebound,
                    descriptor,
                    initial_application,
                    application,
                },
            ) => {
                descriptor.owner == caller
                    && descriptor.ordinal == *ordinal
                    && initial.owner == caller
                    && rebound.owner == caller
                    && descriptor.initial_selection_ordinal == initial.ordinal
                    && descriptor.rebound_selection_ordinal == rebound.ordinal
                    && initial.conformance_application_report_fingerprint
                        == initial_application.report_fingerprint
                    && rebound.conformance_application_report_fingerprint
                        == application.report_fingerprint
                    && initial.conformance_application_commitment == initial_application.commitment
                    && rebound.conformance_application_commitment == application.commitment
                    && initial_application.owner == caller
                    && application.owner == caller
                    && initial_application.report_fingerprint != 0
                    && !initial_application.commitment.is_zero()
                    && initial_application.report_fingerprint
                        == psi_terminal::closed_conformance_application_report_fingerprint(
                            initial_application,
                        )
                    && initial_application.commitment
                        == psi_terminal::closed_conformance_application_commitment(
                            initial_application,
                        )
                    && application.report_fingerprint != 0
                    && !application.commitment.is_zero()
                    && application.report_fingerprint
                        == psi_terminal::closed_conformance_application_report_fingerprint(
                            application,
                        )
                    && application.commitment
                        == psi_terminal::closed_conformance_application_commitment(application)
                    && initial.source.access == self.target.access
                    && rebound.source.access == self.target.access
                    && initial_application.trait_identity == application.trait_identity
                    && initial_application.trait_lifetime_arguments
                        == application.trait_lifetime_arguments
                    && initial_application.trait_arguments == application.trait_arguments
                    && initial_application.telescope == application.telescope
                    && initial_application.rows.len() == application.rows.len()
                    && initial_application.rows.iter().zip(&application.rows).all(
                        |(initial, rebound)| {
                            initial.declaring_trait_identity == rebound.declaring_trait_identity
                                && initial.public_requirement_identity
                                    == rebound.public_requirement_identity
                                && initial.requirement_identity == rebound.requirement_identity
                        },
                    )
                    && application.trait_identity == self.target.trait_identity
                    && application.rows.len() == self.target.requirements.len()
                    && application.rows.iter().zip(&self.target.requirements).all(
                        |(row, requirement)| {
                            row.declaring_trait_identity == requirement.declaring_trait_identity
                                && row.public_requirement_identity
                                    == requirement.public_requirement_identity
                                && row
                                    .realization_callable_identity
                                    .as_ref()
                                    .and_then(|identity| {
                                        application.realization_callables.iter().find(|callable| {
                                            callable.source_callable_identity == *identity
                                        })
                                    })
                                    .is_some_and(|callable| callable.result == requirement.result)
                        },
                    )
            }
            _ => false,
        }
    }
}

/// One call through a descriptor received by the current function.
/// The closed parameter interface supplies the public slot and result shape;
/// the concrete table and instance remain runtime inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractParameterDynamicDispatch {
    pub parameter: TerminalDynamicDescriptorParameter,
    pub dispatch: TerminalParameterDynamicDispatch,
}

impl AbstractReboundDynamicDispatch {
    /// Replay the complete selected-table join without trusting the repeated
    /// call-site row or compact report coordinate as authority.
    pub fn has_complete_application_custody(
        &self,
        owner: MachineId,
        operation: OperationId,
    ) -> bool {
        self.initial.owner == owner
            && self.rebound.owner == owner
            && self.descriptor.owner == owner
            && self.initial_application.owner == owner
            && self.application.owner == owner
            && self.dispatch.owner == owner
            && self.descriptor.initial_selection_ordinal == self.initial.ordinal
            && self.descriptor.rebound_selection_ordinal == self.rebound.ordinal
            && self.dispatch.operation == operation
            && self.dispatch.descriptor_ordinal == self.descriptor.ordinal
            && self.initial_application.report_fingerprint != 0
            && !self.initial_application.commitment.is_zero()
            && self.initial_application.report_fingerprint
                == psi_terminal::closed_conformance_application_report_fingerprint(
                    &self.initial_application,
                )
            && self.initial_application.commitment
                == psi_terminal::closed_conformance_application_commitment(
                    &self.initial_application,
                )
            && self.application.report_fingerprint != 0
            && !self.application.commitment.is_zero()
            && self.application.report_fingerprint
                == psi_terminal::closed_conformance_application_report_fingerprint(
                    &self.application,
                )
            && self.application.commitment
                == psi_terminal::closed_conformance_application_commitment(&self.application)
            && self.initial.conformance_application_report_fingerprint
                == self.initial_application.report_fingerprint
            && self.initial.conformance_application_commitment
                == self.initial_application.commitment
            && self.rebound.conformance_application_report_fingerprint
                == self.application.report_fingerprint
            && self.rebound.conformance_application_commitment == self.application.commitment
            && self.initial_application.trait_identity == self.application.trait_identity
            && self.initial_application.trait_lifetime_arguments
                == self.application.trait_lifetime_arguments
            && self.initial_application.trait_arguments == self.application.trait_arguments
            && self.initial_application.telescope == self.application.telescope
            && self.initial_application.rows.len() == self.application.rows.len()
            && self
                .initial_application
                .rows
                .iter()
                .zip(&self.application.rows)
                .all(|(initial, rebound)| {
                    initial.declaring_trait_identity == rebound.declaring_trait_identity
                        && initial.public_requirement_identity
                            == rebound.public_requirement_identity
                        && initial.requirement_identity == rebound.requirement_identity
                })
            && self
                .application
                .rows
                .iter()
                .filter(|row| {
                    row.declaring_trait_identity == self.dispatch.declaring_trait_identity
                        && row.public_requirement_identity
                            == self.dispatch.public_requirement_identity
                        && row.requirement_identity == self.dispatch.requirement_identity
                        && row.realization_identity == self.dispatch.realization_identity
                        && row.realization_callable_identity.as_deref()
                            == Some(self.dispatch.realization_callable_identity.as_str())
                })
                .count()
                == 1
            && self
                .application
                .realization_callables
                .iter()
                .filter(|callable| {
                    callable.source_callable_identity == self.dispatch.realization_callable_identity
                        && callable.machine == self.dispatch.realization
                })
                .count()
                == 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractFunctionResult {
    Unit,
    Scalar(AbstractResult),
    Structural(StructuralResultDeclaration),
}

impl AbstractFunctionResult {
    pub const fn scalar(&self) -> Option<AbstractResult> {
        match self {
            Self::Unit => None,
            Self::Scalar(result) => Some(*result),
            Self::Structural(_) => None,
        }
    }

    pub const fn structural(&self) -> Option<&StructuralResultDeclaration> {
        match self {
            Self::Structural(result) => Some(result),
            Self::Unit | Self::Scalar(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractOperation {
    /// Zero-code declaration of one existential descriptor in the current
    /// function's runtime interface. Keeping the complete Terminal row in the
    /// entry block prevents an unused parameter from disappearing before a
    /// receiving lowerer selects its physical `{data, table}` ABI.
    DynamicDescriptorParameter {
        parameter: TerminalDynamicDescriptorParameter,
    },
    /// Establish a selected `{instance, table}` descriptor in one exact
    /// aggregate field. This remains target-neutral; later stages choose the
    /// physical two-word local and field offsets.
    StoreDynamicDescriptor {
        psi_operation: OperationId,
        stored: AbstractStoredDynamicDescriptor,
    },
    /// One verifier-approved non-observing replacement through an exact
    /// whole-root write-only structural parameter. The complete parameter row
    /// keeps access, multiplicity, nominal type, and signature position from
    /// being reconstructed from physical ABI shape; `value` retains the exact
    /// preceding scalar definition and type. Target lowering must not realize
    /// this event without a separate physical address/width/store model.
    WriteOnlyPrimitiveStore {
        psi_operation: OperationId,
        destination: StructuralParameterDeclaration,
        value: AbstractResult,
    },
    /// One verifier-approved scalar replacement at an exact field beneath a
    /// structural parameter root. The complete parameter row retains root
    /// authority, `path` and `field` retain the selected structural location,
    /// and `value` rejoins the exact typed dominating scalar definition.
    StructuralScalarFieldStore {
        psi_operation: OperationId,
        destination: StructuralParameterDeclaration,
        path: Vec<StructuralPathSegment>,
        field: psi_core::StructuralFieldId,
        value: AbstractResult,
    },
    /// Establish one exact payloadless case of a declared structural sum.
    /// Target realization remains deliberately separate from retention in the
    /// optimizer's target-neutral semantic vocabulary.
    EstablishPayloadlessCase {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        result_case: StructuralCaseId,
    },
    /// Establish one exact immutable byte payload in a verifier-declared
    /// borrowed-view place. The bytes remain semantic data until target
    /// realization chooses their physical code/data placement.
    EstablishByteSequenceLiteral {
        psi_operation: OperationId,
        place: StructuralPlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
        bytes: Vec<u8>,
    },
    EstablishTrivialAffineLocal {
        psi_operation: OperationId,
        place: StructuralPlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
    },
    /// Atomically establish one complete owned-affine record from its exact
    /// fixed-width scalar field. The operation-result place remains semantic
    /// custody; target lowering must assign a physical home before use.
    EstablishAffineScalarRecord {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        field: psi_core::StructuralFieldId,
        value: IntegerValue,
    },
    CallUnit {
        psi_operation: OperationId,
        callee: MachineId,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one Unit-result machine while forwarding exact existential
    /// descriptor arguments into its declared dynamic parameter interface.
    CallUnitWithDynamicArguments {
        psi_operation: OperationId,
        callee: MachineId,
        structural_arguments: Vec<StructuralArgument>,
        dynamic_arguments: Vec<AbstractDynamicDescriptorArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    CallStructuralScalar {
        psi_operation: OperationId,
        result: AbstractResult,
        callee: MachineId,
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    CallStructuralScalarWithDynamicArguments {
        psi_operation: OperationId,
        result: AbstractResult,
        callee: MachineId,
        structural_arguments: Vec<StructuralArgument>,
        dynamic_arguments: Vec<AbstractDynamicDescriptorArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one scalar-result requirement through an exact rebound dynamic
    /// descriptor. Target realization must materialize the two-word
    /// `{instance, table}` carrier and call through the selected private table;
    /// it may not replace this operation with a direct call to `realization`.
    CallDynamicScalar {
        psi_operation: OperationId,
        result: AbstractResult,
        dynamic_dispatch: AbstractReboundDynamicDispatch,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one scalar requirement by reloading a descriptor previously
    /// established in an aggregate field.
    CallStoredDynamicScalar {
        psi_operation: OperationId,
        result: AbstractResult,
        dynamic_dispatch: AbstractStoredDynamicDispatch,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    CallDynamicParameterScalar {
        psi_operation: OperationId,
        result: AbstractResult,
        dynamic_dispatch: AbstractParameterDynamicDispatch,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one Unit-result requirement through the same exact rebound
    /// descriptor carrier as a scalar dynamic call. Result shape is a property
    /// of the selected callable row, not of descriptor custody.
    CallDynamicUnit {
        psi_operation: OperationId,
        dynamic_dispatch: AbstractReboundDynamicDispatch,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one Unit-result requirement through a descriptor received by the
    /// current function. The closed parameter interface supplies the result
    /// shape and the concrete instance/table pair remains a runtime input.
    CallDynamicParameterUnit {
        psi_operation: OperationId,
        dynamic_dispatch: AbstractParameterDynamicDispatch,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One verifier-approved structural-result call. The result place and
    /// returned-claim correspondence remain semantic custody; target lowering
    /// may realize only a deliberately bounded ABI subset.
    CallStructural {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        callee: MachineId,
        /// Runtime scalar arguments in exact Terminal call order. The
        /// established structural-only lane carries an empty row.
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
        selected_evidence: Vec<OutcomeSpecificCallEvidence>,
    },
    BoundaryCall {
        psi_operation: OperationId,
        result: Option<AbstractResult>,
        boundary: BoundaryMachineId,
        /// Runtime scalar arguments in the exact terminal-Psi call order.
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
    PortWrite {
        psi_operation: OperationId,
        service: ServiceId,
        port: u16,
        value: u8,
    },
    Call {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: ScalarType,
        callee: MachineId,
        arguments: Vec<ValueId>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    IntegerConstant {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: ScalarType,
        value: IntegerValue,
    },
    IeeeFloatConstant {
        psi_operation: OperationId,
        result: ValueId,
        value: IeeeFloatValue,
    },
    NearestIeeeFloatFusedMultiplyAdd {
        psi_operation: OperationId,
        result: ValueId,
        format: IeeeFloatFormat,
        left: ValueId,
        right: ValueId,
        addend: ValueId,
    },
    BooleanConstant {
        psi_operation: OperationId,
        result: ValueId,
        value: bool,
    },
    BooleanStructuralField {
        psi_operation: OperationId,
        result: ValueId,
        source: PlaceId,
        field: psi_core::StructuralFieldId,
    },
    /// Read one exact relevant integer field through the complete verified
    /// structural parameter declaration. The result retains both its value
    /// identity and integer type; no source or field identity is recovered
    /// from a declaration name downstream.
    IntegerStructuralField {
        psi_operation: OperationId,
        result: AbstractResult,
        source: StructuralParameterDeclaration,
        field: psi_core::StructuralFieldId,
    },
    BooleanNot {
        psi_operation: OperationId,
        result: ValueId,
        operand: ValueId,
    },
    BooleanEqual {
        psi_operation: OperationId,
        result: ValueId,
        left: ValueId,
        right: ValueId,
    },
    IntegerEqual {
        psi_operation: OperationId,
        result: ValueId,
        left: ValueId,
        right: ValueId,
    },
    IntegerLessThan {
        psi_operation: OperationId,
        result: ValueId,
        left: ValueId,
        right: ValueId,
    },
    IntegerLessOrEqual {
        psi_operation: OperationId,
        result: ValueId,
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseNot {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        operand: ValueId,
    },
    IntegerWiden {
        psi_operation: OperationId,
        result: ValueId,
        source_type: IntegerType,
        target_type: IntegerType,
        operand: ValueId,
    },
    IntegerExactCast {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        source_type: IntegerType,
        target_type: IntegerType,
        operand: ValueId,
    },
    IntegerBitwiseAnd {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseOr {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseXor {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerShiftLeft {
        psi_operation: OperationId,
        result: ValueId,
        value_type: IntegerType,
        count_type: IntegerType,
        value: ValueId,
        count: ValueId,
    },
    WrappingIntegerShiftRight {
        psi_operation: OperationId,
        result: ValueId,
        value_type: IntegerType,
        count_type: IntegerType,
        value: ValueId,
        count: ValueId,
    },
    ExactIntegerShiftLeft {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        value_type: IntegerType,
        count_type: IntegerType,
        value: ValueId,
        count: ValueId,
    },
    ExactIntegerShiftRight {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        value_type: IntegerType,
        count_type: IntegerType,
        value: ValueId,
        count: ValueId,
    },
    WrappingIntegerAdd {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    /// Exact mathematical addition admitted only after Psi verifies the
    /// operation's overflow obligation. Target realization may use the same
    /// modular instruction as wrapping addition, but the semantic operation
    /// identity remains distinct for optimization and audit.
    ExactIntegerAdd {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerAdd {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerSubtract {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    /// Exact mathematical subtraction with a verifier-discharged range
    /// obligation. It must not be reclassified as wrapping arithmetic merely
    /// because both lower to the same native instruction on admitted inputs.
    ExactIntegerSubtract {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerSubtract {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerMultiply {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    /// Exact mathematical multiplication with a verifier-discharged range
    /// obligation, retained separately from modular multiplication.
    ExactIntegerMultiply {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    ExactIntegerDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    ExactIntegerRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerDivide {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerRemainder {
        psi_operation: OperationId,
        obligation: psi_core::ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerMultiply {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    Jump {
        psi_edge: EdgeId,
        target: BlockId,
        bindings: Vec<ValueBinding>,
        /// Exact Terminal-Psi edge cleanup order. These no-ABI affine
        /// discards still participate in ownership semantics and therefore
        /// cannot be reconstructed from the target block alone.
        trivial_affine_discards: Vec<PlaceId>,
    },
    Conditional {
        condition: ValueId,
        when_true: AbstractSuccessor,
        when_false: AbstractSuccessor,
    },
    Return {
        psi_edge: EdgeId,
        result: ValueId,
        value: ValueId,
        scalar_type: ScalarType,
        /// Exact cleanup execution order retained from verified Psi. The
        /// scalar result is materialized before these actions execute.
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    ReturnUnit {
        psi_edge: EdgeId,
        /// Exact cleanup execution order retained from verified Psi.
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    /// Transfer one verified structural root and its complete live claim set
    /// into the function's declared structural result. Omega realization must
    /// preserve this custody metadata even though claim identities add no ABI
    /// fragments of their own.
    ReturnStructural {
        psi_edge: EdgeId,
        source: PlaceId,
        returned_claims: Vec<ClaimId>,
        /// Exact typed no-ABI local declarations established before this
        /// return. They remain distinct from caller-supplied parameters.
        trivial_affine_locals: Vec<(
            OperationId,
            StructuralPlaceDeclaration,
            StructuralTypeDeclaration,
        )>,
        trivial_affine_discards: Vec<PlaceId>,
    },
    /// A verified no-successor terminal. The audit-only site guard and frontier
    /// remain attached at the Omega boundary even though native realization
    /// only needs the closed cause and edge identity.
    Crash {
        psi_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<psi_terminal::CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractSuccessor {
    pub psi_edge: EdgeId,
    pub target: BlockId,
    pub bindings: Vec<ValueBinding>,
    /// Exact Terminal-Psi cleanup order for this conditional edge.
    pub trivial_affine_discards: Vec<PlaceId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueBinding {
    pub parameter: ValueId,
    pub argument: ValueId,
    pub scalar_type: ScalarType,
}
