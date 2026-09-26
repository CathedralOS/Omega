use language_semantics::ServiceReachSummary;
use symbols::SymbolHandle;

use crate::{
    CheckedComposedUnitControlStatePlan, CheckedProviderAttachmentRequirementPlan,
    CheckedScalarExpression, CheckedStructuralControlSuccessorPlan,
    CheckedUnitStructuralArgumentPlan, DynamicConformanceBindingFact, MachineContractCommitment,
};

use super::{
    CheckedUnitCallCoordinate, CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralPathSegment,
};

/// Checked dynamic-dispatch custody published by the Unit-effect planner.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedDynamicDispatchPlans {
    /// Exact descriptor movements across ordinary calls, independent of
    /// whether a particular Terminal lowering composes or preserves the call.
    pub transfers: Vec<CheckedDynamicDescriptorTransferPlan>,
    /// One row per checked dynamic dispatch, whichever binding carries the
    /// descriptor to the call and whichever result the call returns.
    pub calls: Vec<CheckedDynamicDispatchPlan>,
}

impl CheckedDynamicDispatchPlans {
    /// Every dispatch plan whose calling machine is `machine`.
    pub fn for_caller(
        &self,
        machine: SymbolHandle,
    ) -> impl Iterator<Item = &CheckedDynamicDispatchPlan> {
        self.calls
            .iter()
            .filter(move |plan| plan.caller_machine() == machine)
    }

    /// Every scalar-result call plan across every binding, both branches of
    /// a joined binding included.
    pub fn scalar_calls(&self) -> impl Iterator<Item = &CheckedDynamicScalarCallPlan> {
        self.calls
            .iter()
            .flat_map(CheckedDynamicDispatchPlan::scalar_calls)
    }
}

/// One checked dynamic dispatch. The variant names the result the dispatched
/// call returns; the binding inside names how the selected descriptor reaches
/// that call. The result wraps the binding because a joined binding owns two
/// branch calls that must return the same result: no plan can pair a scalar
/// branch with a Unit branch, and no result binding, ABI home, or
/// continuation can be inferred for a Unit call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedDynamicDispatchPlan {
    Scalar(CheckedDynamicBinding<CheckedDynamicScalarCallPlan>),
    Unit(CheckedDynamicBinding<CheckedDynamicUnitCallPlan>),
}

impl CheckedDynamicDispatchPlan {
    pub fn caller_machine(&self) -> SymbolHandle {
        match self {
            Self::Scalar(binding) => binding.first_call().caller_machine,
            Self::Unit(binding) => binding.first_call().caller_machine,
        }
    }

    pub fn binding_kind(&self) -> CheckedDynamicBindingKind {
        match self {
            Self::Scalar(binding) => binding.kind(),
            Self::Unit(binding) => binding.kind(),
        }
    }

    /// The scalar-result call plans this dispatch performs: none for a Unit
    /// dispatch, one for a same-state binding, one per branch for a join.
    pub fn scalar_calls(&self) -> impl Iterator<Item = &CheckedDynamicScalarCallPlan> {
        match self {
            Self::Scalar(binding) => Some(binding.calls()),
            Self::Unit(_) => None,
        }
        .into_iter()
        .flatten()
    }

    /// The Unit-result call plans this dispatch performs, mirroring
    /// [`Self::scalar_calls`].
    pub fn unit_calls(&self) -> impl Iterator<Item = &CheckedDynamicUnitCallPlan> {
        match self {
            Self::Scalar(_) => None,
            Self::Unit(binding) => Some(binding.calls()),
        }
        .into_iter()
        .flatten()
    }

    /// The realization machine every performed call selects.
    pub fn realization_machines(&self) -> impl Iterator<Item = SymbolHandle> + '_ {
        let scalar = self.scalar_calls().map(|call| call.realization_machine);
        let unit = self.unit_calls().map(|call| call.realization_machine);
        scalar.chain(unit)
    }

    /// Every structural type identity the dispatch names: the caller
    /// attachment, each call's source type, and a rebound binding's initial
    /// source type.
    pub fn type_identities(&self) -> impl Iterator<Item = &str> {
        let initial = match self {
            Self::Scalar(binding) => binding.initial_selection(),
            Self::Unit(binding) => binding.initial_selection(),
        }
        .map(|initial| initial.type_identity.as_str());
        let scalar = self.scalar_calls().flat_map(|call| {
            [
                call.caller_attachment_type_identity.as_str(),
                call.source_type_identity.as_str(),
            ]
        });
        let unit = self.unit_calls().flat_map(|call| {
            [
                call.caller_attachment_type_identity.as_str(),
                call.source_type_identity.as_str(),
            ]
        });
        initial.into_iter().chain(scalar).chain(unit)
    }
}

/// How one selected descriptor reaches the dispatched call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedDynamicBinding<Call> {
    /// The call follows one selection in the same state.
    Direct(Call),
    /// The call follows exactly one same-interface reassignment; both source
    /// versions remain descriptor/table state for the later indirect call.
    Rebound {
        initial: CheckedDynamicSelectionPlan,
        latest: Call,
    },
    /// The descriptor reaches the receiver through an exact local aggregate
    /// field, which Terminal Psi materializes and reloads explicitly.
    Stored {
        descriptor: CheckedDynamicStoredDescriptorPlan,
        call: Call,
    },
    /// The first source-level runtime descriptor phi: two branch-local exact
    /// selections entering one shared descriptor parameter. Each branch
    /// retains its complete direct call while the control plan owns the one
    /// checked Boolean split that selects between them.
    Joined {
        control: CheckedDynamicJoinControlPlan,
        when_true: CheckedDynamicJoinBranchPlan<Call>,
        when_false: CheckedDynamicJoinBranchPlan<Call>,
    },
}

impl<Call> CheckedDynamicBinding<Call> {
    pub fn kind(&self) -> CheckedDynamicBindingKind {
        match self {
            Self::Direct(_) => CheckedDynamicBindingKind::Direct,
            Self::Rebound { .. } => CheckedDynamicBindingKind::Rebound,
            Self::Stored { .. } => CheckedDynamicBindingKind::Stored,
            Self::Joined { .. } => CheckedDynamicBindingKind::Joined,
        }
    }

    /// The one call, or the `when_true` branch call of a join.
    pub fn first_call(&self) -> &Call {
        match self {
            Self::Direct(call) | Self::Stored { call, .. } => call,
            Self::Rebound { latest, .. } => latest,
            Self::Joined { when_true, .. } => &when_true.call,
        }
    }

    /// Every call this binding dispatches, `when_true` before `when_false`.
    pub fn calls(&self) -> impl Iterator<Item = &Call> {
        let (first, second) = match self {
            Self::Direct(call) | Self::Stored { call, .. } => (call, None),
            Self::Rebound { latest, .. } => (latest, None),
            Self::Joined {
                when_true,
                when_false,
                ..
            } => (&when_true.call, Some(&when_false.call)),
        };
        std::iter::once(first).chain(second)
    }

    pub fn initial_selection(&self) -> Option<&CheckedDynamicSelectionPlan> {
        match self {
            Self::Rebound { initial, .. } => Some(initial),
            Self::Direct(_) | Self::Stored { .. } | Self::Joined { .. } => None,
        }
    }
}

/// The binding kind alone, for policy that does not need the calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedDynamicBindingKind {
    Direct,
    Rebound,
    Stored,
    Joined,
}

/// The exact local aggregate field a stored descriptor travels through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicStoredDescriptorPlan {
    pub storage: crate::DynamicDescriptorStorageFact,
    pub destination_type_identity: String,
    pub destination_field_identity: String,
}

/// The caller-owned Boolean split of a joined binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicJoinControlPlan {
    pub entry_state: SymbolHandle,
    pub caller_attachment_type_identity: String,
    pub scalar_parameters: Vec<crate::CheckedStructuralScalarParameterPlan>,
    pub guard: CheckedScalarExpression,
}

/// One branch of a joined binding: the successor the split enters and the
/// complete direct call that branch state performs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicJoinBranchPlan<Call> {
    pub successor: CheckedStructuralControlSuccessorPlan,
    pub call: Call,
}

/// Checked custody for one terminal Unit-returning call through a local named
/// dynamic value or an exact descriptor-parameter chain. This rung admits no
/// dynamic-call arguments, result discard, realization state contracts,
/// service reach, or realization body operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicUnitCallPlan {
    /// Complete ordered bodies of the parameter helpers, outermost first.
    pub forwarding_helpers: Vec<CheckedDynamicUnitHelperPlan>,
    /// Exact authored route by which the selected descriptor reaches this
    /// Unit dispatch. Forwarding retains both the outer ordinary-call
    /// coordinate and the helper's parameter-slot call coordinate.
    pub origin: CheckedDynamicUnitCallOrigin,
    /// Exact parameter-sourced transfers before the final dispatching helper,
    /// ordered from the root caller toward that helper. Empty for local and
    /// one-hop forwarded calls.
    pub forwarding_transfers: Vec<CheckedDynamicDescriptorTransferPlan>,
    pub caller_machine: SymbolHandle,
    pub caller_state: SymbolHandle,
    pub caller_attachment_type_identity: String,
    pub caller_multiplicity: language_semantics::Multiplicity,
    pub caller_parameter_access: super::CheckedStructuralAccess,
    pub caller_contract_report_fingerprint: u64,
    pub caller_contract_commitment: MachineContractCommitment,
    pub caller_service_reach: ServiceReachSummary,
    pub coordinate: CheckedUnitCallCoordinate,
    pub receiver_binding: SymbolHandle,
    pub selection: DynamicConformanceBindingFact,
    pub source_parameter_position: u32,
    pub source_access: super::CheckedStructuralAccess,
    pub source_field: SymbolHandle,
    pub source_path: Vec<CheckedUnitStructuralPathSegment>,
    pub source_type_identity: String,
    pub source_multiplicity: language_semantics::Multiplicity,
    pub target_trait: SymbolHandle,
    pub selected_conformance: SymbolHandle,
    pub declaring_trait: SymbolHandle,
    pub requirement: SymbolHandle,
    pub requirement_identity: String,
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub realization_identity: String,
    /// The finite-family value tuple this call selects: canonical const
    /// identities in the requirement's const/value binder declaration order.
    /// `realization_machine` and `realization_state` then name the tuple's
    /// specialization instance. Empty on a nongeneric requirement.
    pub family_tuple: Box<[String]>,
    pub realization_callables: Vec<CheckedDynamicRealizationCallablePlan>,
    pub realization_contract_report_fingerprint: u64,
    pub realization_contract_commitment: MachineContractCommitment,
    pub checked_call_service_reach: ServiceReachSummary,
}

/// Ordered pure locals around one helper's Unit call. The call binds nothing,
/// so the locals alone occupy the helper's scalar namespace, and the helper
/// returns Unit after its last statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicUnitHelperPlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    /// The statement that performs the helper's call.
    pub call_statement_index: u32,
    /// Every other statement's binding and checked initializer, in order.
    pub scalar_locals: Vec<(CheckedUnitScalarResultBindingPlan, CheckedScalarExpression)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedDynamicUnitCallOrigin {
    Local,
    Forwarded {
        machine: SymbolHandle,
        state: SymbolHandle,
        coordinate: CheckedUnitCallCoordinate,
        parameter: SymbolHandle,
    },
}

/// One checked call argument that transfers an already-selected dynamic
/// descriptor into one exact bare dynamic parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicDescriptorTransferPlan {
    pub caller_machine: SymbolHandle,
    pub caller_state: SymbolHandle,
    pub coordinate: CheckedUnitCallCoordinate,
    pub target_machine: SymbolHandle,
    pub target_state: SymbolHandle,
    /// Dense among the target's non-self runtime parameters.
    pub parameter_position: u32,
    pub parameter: SymbolHandle,
    pub target_trait: SymbolHandle,
    pub source_binding: SymbolHandle,
    /// Whether this call materializes an owner-local selection or forwards an
    /// already-received descriptor parameter.
    pub source: CheckedDynamicDescriptorTransferSource,
    /// Independently counted syntactic calls entering the source parameter's
    /// state. Zero for an owner-local selection; one for a transparent
    /// forwarding step, or two for the first bounded control-flow join.
    pub source_predecessor_count: u32,
    /// Complete alternative paths by which a runtime descriptor can reach
    /// this call. A direct selection has one path. A parameter forwarded after
    /// a control-flow join has one path per incoming edge, and subsequent
    /// transparent hops retain that complete set; no representative selection
    /// is allowed to stand in for the joined alternatives.
    pub source_paths: Vec<CheckedDynamicDescriptorTransferPath>,
}

/// One exact root selection and the ordered ordinary-call edges that carry it
/// to a descriptor parameter. Keeping the paths distinct preserves both
/// same-conformance/different-referent joins and different-conformance joins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicDescriptorTransferPath {
    pub selection: DynamicConformanceBindingFact,
    pub edges: Vec<CheckedDynamicDescriptorTransferEdge>,
}

/// Expression-table-free identity of one descriptor-carrying ordinary-call
/// edge. This is a projection of the owning transfer, not a second authority
/// row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicDescriptorTransferEdge {
    pub caller_machine: SymbolHandle,
    pub caller_state: SymbolHandle,
    pub coordinate: CheckedUnitCallCoordinate,
    pub target_machine: SymbolHandle,
    pub target_state: SymbolHandle,
    pub parameter_position: u32,
    pub parameter: SymbolHandle,
    pub target_trait: SymbolHandle,
    pub source_binding: SymbolHandle,
    pub source: CheckedDynamicDescriptorTransferSource,
    pub source_predecessor_count: u32,
}

impl CheckedDynamicDescriptorTransferEdge {
    pub fn canonical_order_key(&self) -> [u32; 20] {
        let (source_kind, source_position) = match self.source {
            CheckedDynamicDescriptorTransferSource::Selection => (0, 0),
            CheckedDynamicDescriptorTransferSource::Parameter { parameter_position } => {
                (1, parameter_position)
            }
        };
        [
            self.caller_machine.arena_index(),
            self.caller_machine.generation(),
            self.caller_state.arena_index(),
            self.caller_state.generation(),
            self.coordinate.statement_index,
            self.coordinate.call_ordinal,
            self.target_machine.arena_index(),
            self.target_machine.generation(),
            self.target_state.arena_index(),
            self.target_state.generation(),
            self.parameter_position,
            self.parameter.arena_index(),
            self.parameter.generation(),
            self.target_trait.arena_index(),
            self.target_trait.generation(),
            self.source_binding.arena_index(),
            self.source_binding.generation(),
            source_kind,
            source_position,
            self.source_predecessor_count,
        ]
    }
}

impl CheckedDynamicDescriptorTransferPlan {
    pub fn edge(&self) -> CheckedDynamicDescriptorTransferEdge {
        CheckedDynamicDescriptorTransferEdge {
            caller_machine: self.caller_machine,
            caller_state: self.caller_state,
            coordinate: self.coordinate,
            target_machine: self.target_machine,
            target_state: self.target_state,
            parameter_position: self.parameter_position,
            parameter: self.parameter,
            target_trait: self.target_trait,
            source_binding: self.source_binding,
            source: self.source,
            source_predecessor_count: self.source_predecessor_count,
        }
    }

    /// The concrete root is available only when no control-flow join exists.
    pub fn sole_selection(&self) -> Option<&DynamicConformanceBindingFact> {
        let [path] = self.source_paths.as_slice() else {
            return None;
        };
        Some(&path.selection)
    }

    /// Replays the complete incoming transfer graph for this row. This does
    /// not discover selections or edges: every expected path must already be
    /// present in the supplied checked roster.
    pub fn has_complete_source_custody(
        &self,
        transfers: &[CheckedDynamicDescriptorTransferPlan],
    ) -> bool {
        self.has_complete_source_custody_inner(transfers, &mut Vec::new())
    }

    fn has_complete_source_custody_inner(
        &self,
        transfers: &[CheckedDynamicDescriptorTransferPlan],
        visiting: &mut Vec<CheckedDynamicDescriptorTransferEdge>,
    ) -> bool {
        let edge = self.edge();
        if visiting.contains(&edge) || self.source_paths.is_empty() {
            return false;
        }
        visiting.push(edge.clone());
        let valid = match self.source {
            CheckedDynamicDescriptorTransferSource::Selection => {
                let [path] = self.source_paths.as_slice() else {
                    visiting.pop();
                    return false;
                };
                self.source_predecessor_count == 0
                    && path.edges == [edge]
                    && path.selection.machine == self.caller_machine
                    && path.selection.state == self.caller_state
                    && path.selection.binding == self.source_binding
                    && path.selection.target_trait == self.target_trait
                    && path.selection.statement_index < self.coordinate.statement_index as usize
            }
            CheckedDynamicDescriptorTransferSource::Parameter { .. } => {
                let mut incoming = transfers
                    .iter()
                    .filter(|candidate| {
                        candidate.target_machine == self.caller_machine
                            && candidate.target_state == self.caller_state
                            && candidate.parameter == self.source_binding
                            && candidate.target_trait == self.target_trait
                    })
                    .collect::<Vec<_>>();
                incoming.sort_by_key(|candidate| candidate.edge().canonical_order_key());
                let mut expected_paths = Vec::new();
                let incoming_valid = usize::try_from(self.source_predecessor_count).ok()
                    == Some(incoming.len())
                    && matches!(incoming.len(), 1 | 2)
                    && (incoming.len() == 1
                        || incoming
                            .iter()
                            .all(|candidate| candidate.source_paths.len() == 1))
                    && incoming.iter().all(|candidate| {
                        if !candidate.has_complete_source_custody_inner(transfers, visiting) {
                            return false;
                        }
                        for path in &candidate.source_paths {
                            let mut path = path.clone();
                            path.edges.push(edge.clone());
                            expected_paths.push(path);
                        }
                        true
                    });
                incoming_valid && self.source_paths == expected_paths
            }
        };
        visiting.pop();
        valid
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedDynamicDescriptorTransferSource {
    Selection,
    Parameter {
        /// Dense among the source state's non-self runtime parameters.
        parameter_position: u32,
    },
}

/// Shared checked custody for the selected call version of one local named
/// dynamic scalar call. The containing direct or rebound catalog supplies its
/// dispatch semantics; Terminal lowering must consume this row rather than
/// repeat conformance discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicScalarCallPlan {
    /// Complete ordered scalar bodies of the parameter helpers, outermost first.
    pub forwarding_helpers: Vec<CheckedDynamicScalarHelperPlan>,
    /// Exact authored route by which the selected descriptor reaches this
    /// scalar dispatch. The surrounding stored-call wrapper owns aggregate
    /// lineage; forwarded routes retain every transparent parameter transfer.
    pub origin: CheckedDynamicScalarCallOrigin,
    /// Exact parameter-sourced transfers before the final dispatching helper,
    /// ordered from the root caller toward that helper. Empty for local and
    /// one-hop forwarded calls.
    pub forwarding_transfers: Vec<CheckedDynamicDescriptorTransferPlan>,
    pub caller_machine: SymbolHandle,
    pub caller_state: SymbolHandle,
    pub caller_attachment_type_identity: String,
    pub caller_multiplicity: language_semantics::Multiplicity,
    pub caller_parameter_access: super::CheckedStructuralAccess,
    pub caller_contract_report_fingerprint: u64,
    pub caller_contract_commitment: MachineContractCommitment,
    pub caller_service_reach: ServiceReachSummary,
    pub coordinate: CheckedUnitCallCoordinate,
    pub result_binding: SymbolHandle,
    pub result: CheckedUnitScalarResultBindingPlan,
    pub receiver_binding: SymbolHandle,
    /// The exact latest selection preceding `coordinate`. This projection is
    /// source-handle-free and retains the complete selected row roster for
    /// downstream replay.
    pub selection: DynamicConformanceBindingFact,
    /// Exact field below the caller attachment selected as concrete `self`.
    pub source_parameter_position: u32,
    pub source_access: super::CheckedStructuralAccess,
    pub source_field: SymbolHandle,
    pub source_path: Vec<CheckedUnitStructuralPathSegment>,
    pub source_type_identity: String,
    pub source_multiplicity: language_semantics::Multiplicity,
    pub target_trait: SymbolHandle,
    pub selected_conformance: SymbolHandle,
    pub declaring_trait: SymbolHandle,
    pub requirement: SymbolHandle,
    pub requirement_identity: String,
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub realization_identity: String,
    /// The finite-family value tuple this call selects: canonical const
    /// identities in the requirement's const/value binder declaration order
    /// (`named(integer-const(16))` spellings). `realization_machine` and
    /// `realization_state` then name the tuple's specialization instance.
    /// Empty on a nongeneric requirement, which matches every call to it.
    pub family_tuple: Box<[String]>,
    /// Exact source-independent body of the selected realization's sole
    /// scalar return. The current structural-scalar lane cannot represent an
    /// unrestricted borrowed `self`, so downstream lowering consumes this
    /// expression directly instead of reopening typed source.
    pub realization_return_expression: CheckedScalarExpression,
    /// Exact ordered primitive-field mutations performed by the selected
    /// realization immediately before its scalar return. These are realization
    /// custody, not the independent caller-side pre-selection store below.
    pub realization_structural_scalar_field_stores: Vec<CheckedStructuralScalarFieldStorePlan>,
    /// Complete closed realization roster for the selected conformance. A
    /// rebound dynamic descriptor is materializable only when every table
    /// slot retains its exact checked callable and body; retaining only
    /// the currently selected row would make the later indirect table a
    /// producer assertion rather than a reconstruction.
    pub realization_callables: Vec<CheckedDynamicRealizationCallablePlan>,
    /// Compact report coordinate; authority uses the adjacent commitment.
    pub realization_contract_report_fingerprint: u64,
    pub realization_contract_commitment: MachineContractCommitment,
    pub checked_call_service_reach: ServiceReachSummary,
    /// Exact caller-side store immediately preceding the selected dynamic
    /// binding, when the bounded three-statement structural-field shape was
    /// admitted. Ordinary direct calls retain `None`.
    pub caller_structural_scalar_field_store: Option<CheckedStructuralScalarFieldStorePlan>,
    /// Exact checked control suffix when this result immediately selects two
    /// Unit effect leaves. The dynamic call remains in this plan; this suffix
    /// begins at the authored guard and therefore cannot be lowered as an
    /// independent machine or silently discarded.
    pub unit_continuation: Option<CheckedDynamicUnitContinuationPlan>,
}

/// Scalar sequencing around one exact descriptor call, with ordinary completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicScalarHelperPlan {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub call_result: CheckedUnitScalarResultBindingPlan,
    pub scalar_locals: Vec<(CheckedUnitScalarResultBindingPlan, CheckedScalarExpression)>,
    pub scalar_control: crate::CheckedUnitScalarControlPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedDynamicScalarCallOrigin {
    Local,
    Forwarded {
        machine: SymbolHandle,
        state: SymbolHandle,
        coordinate: CheckedUnitCallCoordinate,
        parameter: SymbolHandle,
    },
}

/// One exact checked callable behind a closed dynamic-conformance table row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicRealizationCallablePlan {
    pub declaring_trait: SymbolHandle,
    pub requirement: SymbolHandle,
    pub requirement_identity: String,
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub realization_identity: String,
    /// The finite-family value tuple this callable realizes: one roster tuple
    /// per entry for a generic requirement, empty for a nongeneric one.
    pub family_tuple: Box<[String]>,
    pub body: CheckedDynamicRealizationBodyPlan,
    pub contract_report_fingerprint: u64,
    pub contract_commitment: MachineContractCommitment,
}

/// Result and body belong to each callable, not to the enclosing trait table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedDynamicRealizationBodyPlan {
    /// An operation-free checked body with no result carrier.
    Unit,
    Scalar {
        result_type: typed_trees::types::PrimitiveType,
        /// Ordered checked mutations; lowering does not rediscover source bodies.
        structural_scalar_field_stores: Vec<CheckedStructuralScalarFieldStorePlan>,
        return_expression: CheckedScalarExpression,
    },
}

/// Source-normalized custody for one version of a local named-dynamic
/// selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicSelectionPlan {
    pub fact: DynamicConformanceBindingFact,
    pub field: SymbolHandle,
    pub path: Vec<CheckedUnitStructuralPathSegment>,
    pub type_identity: String,
}

/// One named-dynamic scalar result consumed by an immediate binary control
/// split whose leaves each perform one checked Unit effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicUnitContinuationPlan {
    pub guard: CheckedScalarExpression,
    pub when_true: CheckedStructuralControlSuccessorPlan,
    pub when_false: CheckedStructuralControlSuccessorPlan,
    /// The one no-code affine local discarded when a stored descriptor's
    /// owning record dies at both outgoing edges. This remains checked-side
    /// custody: Terminal retains the descriptor store/reload rather than
    /// fabricating an unrelated structural place.
    pub trivial_affine_local_discard: Option<SymbolHandle>,
    pub leaves: Vec<CheckedComposedUnitControlStatePlan>,
    pub provider_attachment_requirements: Vec<CheckedProviderAttachmentRequirementPlan>,
}

/// Evaluated source of one scalar field replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedStructuralScalarFieldStoreValue {
    Pure(CheckedScalarExpression),
    Computation(crate::CheckedScalarComputationHandle),
    /// The already-defined SSA result of the scalar call this same statement
    /// performs, named by its dense position in the consuming plan's scalar
    /// namespace -- the namespace `CheckedScalarExpression::Local` indexes,
    /// scalar parameters first and then each established scalar result in
    /// order. The authored source has no local binding to name, so no
    /// `AssignmentValue` scalar-expression row exists for it; the receiving
    /// lowerer reconstructs the value from the call operation it already
    /// emitted for this statement instead of from an authored expression.
    ScalarResult {
        position: u32,
    },
}

impl CheckedStructuralScalarFieldStoreValue {
    pub fn as_pure(&self) -> Option<&CheckedScalarExpression> {
        match self {
            Self::Pure(expression) => Some(expression),
            Self::Computation(_) | Self::ScalarResult { .. } => None,
        }
    }
}

/// Exact source root for a field write. Parameter positions are authored state
/// positions, not dense structural-parameter indexes; locals keep symbol identity
/// so ordered lowering can resolve their current home after copies or moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedStructuralScalarFieldStoreDestination {
    Parameter { position: u32 },
    Local { symbol: SymbolHandle },
}

impl CheckedStructuralScalarFieldStoreDestination {
    /// Parameter-only consumers reject local storage instead of inventing an index.
    pub fn parameter_position(self) -> Option<u32> {
        match self {
            Self::Parameter { position } => Some(position),
            Self::Local { .. } => None,
        }
    }
}

/// Checked custody for one replacement of an exact primitive field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralScalarFieldStorePlan {
    pub statement_index: u32,
    pub destination: CheckedStructuralScalarFieldStoreDestination,
    /// Exact structural path from the destination root to the carrier;
    /// the final primitive field is retained separately below.
    pub carrier_path: Vec<CheckedUnitStructuralPathSegment>,
    pub field_identity: String,
    pub primitive_type: typed_trees::types::PrimitiveType,
    pub value: CheckedStructuralScalarFieldStoreValue,
}

/// Checked custody for one replacement of an exact unrestricted-sum field.
/// The destination carries the same root, path and write-frame custody a
/// scalar field store proves; the stored value is a whole owned place of the
/// field's declared sum type, so replacement copies every bit and moves
/// nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralCaseFieldStorePlan {
    pub statement_index: u32,
    pub destination: CheckedStructuralScalarFieldStoreDestination,
    /// Exact structural path from the destination root to the carrier;
    /// the final sum-typed field is retained separately below.
    pub carrier_path: Vec<CheckedUnitStructuralPathSegment>,
    pub field_identity: String,
    /// Whole owned place supplying the stored value; its checked type is the
    /// field's declared unrestricted sum type.
    pub value: CheckedUnitStructuralArgumentPlan,
}
