use psi_language_semantics::ServiceReachSummary;
use psi_symbols::SymbolHandle;

use crate::{
    CheckedComposedUnitControlStatePlan, CheckedProviderAttachmentRequirementPlan,
    CheckedScalarExpression, CheckedStructuralControlSuccessorPlan, DynamicConformanceBindingFact,
    MachineContractCommitment,
};

use super::{
    CheckedUnitCallCoordinate, CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralPathSegment,
};

/// Checked dynamic-dispatch custody published by the Unit-effect planner.
/// Direct devirtualization and rebound descriptor/table calls remain distinct
/// lanes without widening `CheckedUnitEffectPlans` again.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedDynamicDispatchPlans {
    /// Exact descriptor movements across ordinary calls, independent of
    /// whether a particular Terminal lowering composes or preserves the call.
    pub transfers: Vec<CheckedDynamicDescriptorTransferPlan>,
    pub direct_scalar_calls: Vec<CheckedDynamicScalarCallPlan>,
    pub rebound_scalar_calls: Vec<CheckedReboundDynamicScalarCallPlan>,
    /// Calls through descriptors stored in local aggregate fields. These stay
    /// separate from direct devirtualization until Terminal Psi explicitly
    /// materializes and reloads the two-word field representation.
    pub stored_scalar_calls: Vec<CheckedStoredDynamicScalarCallPlan>,
    /// Exact terminal Unit-returning calls through a local descriptor. These
    /// remain distinct from scalar-result calls: no result binding, ABI home,
    /// or continuation may be inferred for this lane.
    pub direct_unit_calls: Vec<CheckedDynamicUnitCallPlan>,
    pub rebound_unit_calls: Vec<CheckedReboundDynamicUnitCallPlan>,
}

/// One complete checked scalar call whose descriptor reaches the receiver
/// through an exact local aggregate field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStoredDynamicScalarCallPlan {
    pub storage: crate::DynamicDescriptorStorageFact,
    pub destination_type_identity: String,
    pub destination_field_identity: String,
    pub call: CheckedDynamicScalarCallPlan,
}

/// Checked custody for one terminal Unit-returning call through a local named
/// dynamic value or an exact transparent descriptor-parameter chain. This rung
/// admits no dynamic-call arguments, result discard, realization state
/// contracts, service reach, or realization body operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicUnitCallPlan {
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
    pub caller_multiplicity: psi_language_semantics::Multiplicity,
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
    pub source_multiplicity: psi_language_semantics::Multiplicity,
    pub target_trait: SymbolHandle,
    pub selected_conformance: SymbolHandle,
    pub declaring_trait: SymbolHandle,
    pub requirement: SymbolHandle,
    pub requirement_identity: String,
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub realization_identity: String,
    pub realization_callables: Vec<CheckedDynamicUnitRealizationCallablePlan>,
    pub realization_contract_report_fingerprint: u64,
    pub realization_contract_commitment: MachineContractCommitment,
    pub checked_call_service_reach: ServiceReachSummary,
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

/// One exact operation-free checked Unit callable behind a closed dynamic
/// table row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicUnitRealizationCallablePlan {
    pub declaring_trait: SymbolHandle,
    pub requirement: SymbolHandle,
    pub requirement_identity: String,
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub realization_identity: String,
    pub contract_report_fingerprint: u64,
    pub contract_commitment: MachineContractCommitment,
}

/// A Unit call after exactly one same-interface descriptor reassignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedReboundDynamicUnitCallPlan {
    pub initial: CheckedDynamicSelectionPlan,
    pub latest: CheckedDynamicUnitCallPlan,
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
    /// state. Zero for an owner-local selection; one or two for the bounded
    /// parameter-forwarding lane.
    pub source_predecessor_count: u32,
    /// Complete alternative paths by which a runtime descriptor can reach
    /// this call. A direct selection has one path. A parameter forwarded after
    /// a control-flow join has one path per incoming edge; no representative
    /// selection is allowed to stand in for the joined alternatives.
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
                    && incoming
                        .iter()
                        .all(|candidate| candidate.source_paths.len() == 1)
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
    pub caller_multiplicity: psi_language_semantics::Multiplicity,
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
    pub source_multiplicity: psi_language_semantics::Multiplicity,
    pub target_trait: SymbolHandle,
    pub selected_conformance: SymbolHandle,
    pub declaring_trait: SymbolHandle,
    pub requirement: SymbolHandle,
    pub requirement_identity: String,
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub realization_identity: String,
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
    /// slot retains its exact checked callable and scalar body; retaining only
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
    pub result_type: psi_typed_trees::types::PrimitiveType,
    /// Exact ordered primitive-field mutations performed before the return.
    /// The bounded body shape admits at most three distinct literal stores
    /// through mutable `self`; downstream lowering must not rediscover them
    /// from source.
    pub structural_scalar_field_stores: Vec<CheckedStructuralScalarFieldStorePlan>,
    pub return_expression: CheckedScalarExpression,
    pub contract_report_fingerprint: u64,
    pub contract_commitment: MachineContractCommitment,
}

/// Checked custody for one local named-dynamic scalar call after exactly one
/// same-interface reassignment. This is a separate lane from direct
/// devirtualization: later Terminal lowering must consume both source versions
/// as descriptor/table state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedReboundDynamicScalarCallPlan {
    pub initial: CheckedDynamicSelectionPlan,
    pub latest: CheckedDynamicScalarCallPlan,
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
    pub leaves: Vec<CheckedComposedUnitControlStatePlan>,
    pub provider_attachment_requirements: Vec<CheckedProviderAttachmentRequirementPlan>,
}

/// Checked custody for one literal store into a primitive field below the
/// structural carrier later selected for a direct named-dynamic call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralScalarFieldStorePlan {
    pub statement_index: u32,
    pub destination_parameter_position: u32,
    /// Exact structural path from the destination parameter to the carrier;
    /// the final primitive field is retained separately below.
    pub carrier_path: Vec<CheckedUnitStructuralPathSegment>,
    pub field_identity: String,
    pub primitive_type: psi_typed_trees::types::PrimitiveType,
    pub value: CheckedScalarExpression,
}
