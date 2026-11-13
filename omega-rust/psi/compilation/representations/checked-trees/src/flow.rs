/*
We record per-state control-flow facts (`FlowFacts`), borrow lifetimes, boundary
edges, call origins, descriptor observations, operand referents, state rows, and
store plans in `CheckedTrees`.
*/

pub(crate) use crate::flow_ownership_and_roots::roots;

pub(crate) mod borrow_lifetimes {
    use arena::Handle;

    use crate::BorrowLoanFact;

    use super::FlowInvalidationSource;

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum FlowBorrowWeakeningReason {
        #[default]
        LastUseExpired,
        StateExit,
        LocalReassigned,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowBorrowActivationFact {
        pub source: FlowInvalidationSource,
        pub loan: Handle<BorrowLoanFact>,
        pub formation_value: Handle<super::FlowOperandFact>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowBorrowWeakeningFact {
        pub source: FlowInvalidationSource,
        pub loan: Handle<BorrowLoanFact>,
        pub reason: FlowBorrowWeakeningReason,
    }
}
pub(crate) mod boundaries {
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowBoundaryEdgeFact {
        pub statement_index: usize,
        pub call_ordinal: usize,
        pub receiver_symbol: SymbolHandle,
        pub target_symbol: SymbolHandle,
        pub boundary_trait_symbol: SymbolHandle,
        pub boundary_signature_symbol: SymbolHandle,
    }
}
pub(crate) mod call_origins {
    use arena::Handle;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedCallOrigin {
        Statement(Handle<super::FlowCallFact>),
        Declaration {
            application: crate::CheckedRankApplicationFormationHandle,
            call: Handle<crate::CheckedDeclarationCall>,
        },
    }

    impl CheckedCallOrigin {
        pub fn declaration(
            self,
            plan: &crate::CheckedRankArrivalPlan,
            machine: symbols::SymbolHandle,
            state: symbols::SymbolHandle,
        ) -> Result<
            (
                &crate::CheckedRankApplicationFormation,
                &crate::CheckedDeclarationCall,
            ),
            &'static str,
        > {
            let Self::Declaration { application, call } = self else {
                return Err("call has statement custody, not declaration-application custody");
            };
            if !plan.application_formations.is_valid(application)
                || !plan.application_calls.is_valid(call)
            {
                return Err("declaration call has an invalid application or call handle");
            }
            let application = plan.application_formations.get(application);
            if application.machine != machine
                || application.state != state
                || application.machine_body.is_empty()
                || !application.machine_body.start().is_valid()
                || plan
                    .bindings
                    .span(application.bindings)
                    .is_none_or(|bindings| bindings.is_empty() && application.input_home.is_none())
                || plan
                    .application_calls
                    .span(application.application.calls)
                    .is_none()
                || !application
                    .application
                    .calls
                    .handles()
                    .any(|candidate| candidate == call)
            {
                return Err(
                    "declaration call changed its owning body, state or complete input roster",
                );
            }
            if let Some(input) = application.input_home {
                if !input.formal.is_valid()
                    || !input.actual_type.is_valid()
                    || !plan.coordinates.is_valid(input.actual)
                {
                    return Err("declaration call has an invalid whole-value input receipt");
                }
                let coordinate = plan.coordinates.get(input.actual);
                if !coordinate.parameter.is_valid()
                    || plan.path_segments.span(coordinate.path).is_none()
                    || plan.entry_indexes.span(coordinate.entry_indexes).is_none()
                {
                    return Err("declaration call changed its whole-value input coordinate");
                }
            }
            let call = plan.application_calls.get(call);
            if !call.expression.is_valid()
                || !call.authored_target.is_valid()
                || !call.target_machine.is_valid()
                || !call.target_state.is_valid()
                || call.target_contract.is_none()
            {
                return Err("declaration call has no complete selected target contract binding");
            }
            Ok((application, call))
        }

        pub fn is_valid(self) -> bool {
            match self {
                Self::Statement(call) => call.is_valid(),
                Self::Declaration { application, call } => {
                    application.is_valid() && call.is_valid()
                }
            }
        }

        pub fn statement(self) -> Option<Handle<super::FlowCallFact>> {
            match self {
                Self::Statement(call) => Some(call),
                Self::Declaration { .. } => None,
            }
        }
    }

    impl From<Handle<super::FlowCallFact>> for CheckedCallOrigin {
        fn from(call: Handle<super::FlowCallFact>) -> Self {
            Self::Statement(call)
        }
    }
}
pub(crate) mod contexts {
    use arena::Handle;

    use crate::{
        BorrowArgumentAccessFact, BorrowCallFact, BorrowLoanFact, BorrowWritableRootFact,
        StateBorrowFact,
    };

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowSemanticContextRef {
        pub context: facts::FactContextHandle,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum FlowConstraintKind {
        #[default]
        Unknown,
        SemanticContext {
            context: facts::FactContextHandle,
        },
        BorrowState {
            state: Handle<StateBorrowFact>,
        },
        BorrowCall {
            call: Handle<BorrowCallFact>,
        },
        BorrowWritableRoot {
            root: Handle<BorrowWritableRootFact>,
        },
        BorrowAccess {
            access: Handle<BorrowArgumentAccessFact>,
        },
        BorrowLoan {
            loan: Handle<BorrowLoanFact>,
        },
        BorrowIncomingReference {
            identity: Handle<crate::IncomingReferenceIdentity>,
        },
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowConstraintRef {
        pub kind: FlowConstraintKind,
    }

    #[macro_export]
    macro_rules! borrow_resource_pattern {
    ($($fields:tt)*) => {
        $crate::FlowConstraintKind::BorrowAccess { $($fields)* }
            | $crate::FlowConstraintKind::BorrowCall { $($fields)* }
            | $crate::FlowConstraintKind::BorrowLoan { $($fields)* }
            | $crate::FlowConstraintKind::BorrowIncomingReference { $($fields)* }
            | $crate::FlowConstraintKind::BorrowState { $($fields)* }
            | $crate::FlowConstraintKind::BorrowWritableRoot { $($fields)* }
    };
}

    #[macro_export]
    macro_rules! nonloan_constraint_pattern {
    ($($fields:tt)*) => {
        $crate::FlowConstraintKind::BorrowAccess { $($fields)* }
            | $crate::FlowConstraintKind::BorrowCall { $($fields)* }
            | $crate::FlowConstraintKind::BorrowState { $($fields)* }
            | $crate::FlowConstraintKind::BorrowWritableRoot { $($fields)* }
            | $crate::FlowConstraintKind::SemanticContext { $($fields)* }
            | $crate::FlowConstraintKind::BorrowIncomingReference { $($fields)* }
    };
}
}
pub(crate) mod control {

    use arena::HandleSpan;
    use language_semantics::{BlockingSummary, ServiceReachSummary, SuspensionSummary};
    use symbols::SymbolHandle;

    use crate::{BorrowArgumentAccessFact, BorrowWritableRootFact, ContractProofFactRef};

    use super::{
        FlowBorrowActivationFact, FlowBorrowWeakeningFact, FlowBoundaryEdgeFact, FlowConstraintRef,
        FlowInvalidationFact, FlowSemanticContextRef,
    };

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowOperatorInvocationFact {
        pub operator_use: arena::Handle<crate::CheckedOperatorUseFact>,
        pub named_use: arena::Handle<crate::CheckedNamedOperatorUseFact>,
        pub operands: HandleSpan<super::FlowOperandFact>,
        pub requires_constraints: HandleSpan<FlowConstraintRef>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowCallFact {
        pub statement_index: usize,
        pub call_ordinal: usize,
        pub invocation_storage: arena::Handle<super::FlowStorageSnapshot>,
        pub completed_storage: arena::Handle<super::FlowStorageSnapshot>,
        pub operands: HandleSpan<super::FlowOperandFact>,
        pub authored_expression: typed_trees::expression::ExpressionHandle,
        pub authored_site: FlowCallSite,
        pub authored_target: SymbolHandle,
        pub authored_arguments: HandleSpan<typed_trees::expression::ExpressionHandle>,
        pub argument_evaluation_order: HandleSpan<u32>,
        pub authored_receiver: typed_trees::expression::ExpressionHandle,
        pub authored_result: FlowCallResult,
        pub authored_closing_invocation: bool,
        pub authored_dispatch_role: FlowCallDispatchRole,
        pub authored_selected: bool,
        pub receiver_symbol: SymbolHandle,
        pub target_symbol: SymbolHandle,
        pub static_requirement_dispatch: Option<crate::StaticRequirementDispatchFact>,
        pub has_receiver: bool,
        pub accesses: HandleSpan<BorrowArgumentAccessFact>,
        pub entry_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
        pub entry_constraints: HandleSpan<FlowConstraintRef>,
        pub requires_contexts: HandleSpan<FlowSemanticContextRef>,
        pub requires_constraints: HandleSpan<FlowConstraintRef>,
        pub exit_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
        pub exit_constraints: HandleSpan<FlowConstraintRef>,
        pub invalidations: HandleSpan<FlowInvalidationFact>,
        pub normal_return_writes: HandleSpan<super::FlowOperandReferent>,
        pub normal_return_writes_complete: bool,
        pub normal_return_referents: HandleSpan<super::FlowOperandReferent>,
        pub boundary_edges: HandleSpan<FlowBoundaryEdgeFact>,
        pub requires: HandleSpan<ContractProofFactRef>,
        pub ensures: HandleSpan<ContractProofFactRef>,
        pub service_reach: ServiceReachSummary,
        pub suspension: SuspensionSummary,
        pub blocking: BlockingSummary,
        pub operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
        pub authored_source_span: Option<source::SourceSpan>,
        pub authored_source_custody_valid: bool,
    }

    impl FlowCallFact {
        pub fn argument_operands(&self) -> HandleSpan<super::FlowOperandFact> {
            if !self.authored_receiver.is_valid() || self.operands.is_empty() {
                return self.operands;
            }
            HandleSpan::from_parts_or_empty(self.operands.handle_at(1), self.operands.count() - 1)
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum FlowCallSite {
        #[default]
        Unbound,
        Statement,
        Expression,
        NamedTransition,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum FlowCallResult {
        #[default]
        Operand,
        Discarded,
        ImmutableLocal,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum FlowCallDispatchRole {
        #[default]
        Ordinary,
        SubjectEvaluation,
        SavedSubjectRead,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct RetiredFlowCall {
        pub state_symbol: SymbolHandle,
        pub statement_index: usize,
        pub call_ordinal: usize,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowStatementFact {
        pub statement_index: usize,
        pub storage_snapshot: arena::Handle<super::FlowStorageSnapshot>,
        pub value_observations: HandleSpan<super::FlowValueObservationFact>,
        pub operator_invocations: HandleSpan<FlowOperatorInvocationFact>,
        /// Nested case decisions produced by this actual statement evaluation.
        pub owned_case_selections: HandleSpan<super::FlowOwnedCaseSelection>,
        /// Completed RHS after destination evaluation, before storage. Zero for
        /// statements without a stored value; children retain their own epochs.
        pub stored_value: arena::Handle<super::FlowOperandFact>,
        pub store_target: arena::Handle<super::FlowOperandFact>,
        pub stored_destination: facts::PlaceHandle,
        pub completed_writes: HandleSpan<super::FlowOperandReferent>,
        pub completed_unknown_write: bool,
        pub completed_content_writes: HandleSpan<super::FlowOperandReferent>,
        pub completed_content_unknown_write: bool,
        pub entry_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
        pub exit_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
        pub entry_constraints: HandleSpan<FlowConstraintRef>,
        pub pre_store_constraints: HandleSpan<FlowConstraintRef>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowStateOutgoingFact {
        pub machine_symbol: SymbolHandle,
        pub state_symbol: SymbolHandle,
        pub statement_index: usize,
        pub transition_target: typed_trees::statement::TransitionTargetHandle,
        pub returned_value: arena::Handle<super::FlowOperandFact>,
        pub completed_storage: arena::Handle<super::FlowStorageSnapshot>,
        pub cannot_return_normally: bool,
        pub operand_entry_constraints: HandleSpan<FlowConstraintRef>,
        pub copied_payloads: HandleSpan<super::FlowOperandFact>,
        pub constraints: HandleSpan<FlowConstraintRef>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowExitFact {
        pub machine_symbol: SymbolHandle,
        pub state_symbol: SymbolHandle,
        pub statement_index: usize,
        pub transition_target: typed_trees::statement::TransitionTargetHandle,
        pub is_continuation: bool,
        pub returned_value: arena::Handle<super::FlowOperandFact>,
        pub completed_storage: arena::Handle<super::FlowStorageSnapshot>,
        pub parameter_origins: HandleSpan<FlowExitParameterOrigin>,
        pub entry_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
        pub entry_constraints: HandleSpan<FlowConstraintRef>,
        pub ensures_contexts: HandleSpan<FlowSemanticContextRef>,
        pub ensures_constraints: HandleSpan<FlowConstraintRef>,
        pub ensures: HandleSpan<ContractProofFactRef>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowExitParameterOrigin {
        pub contract: arena::Handle<typed_trees::domain::ProofFact>,
        pub entry_parameter: SymbolHandle,
        pub state_parameter: SymbolHandle,
        pub retained_referent_type: typed_trees::types::TypeReferenceHandle,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowStateFact {
        pub machine_symbol: SymbolHandle,
        pub state_symbol: SymbolHandle,
        pub storage_transfers: HandleSpan<super::FlowStorageTransferFact>,
        pub writable_roots: HandleSpan<BorrowWritableRootFact>,
        pub mutable_parameter_count: usize,
        pub entry_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
        pub entry_constraints: HandleSpan<FlowConstraintRef>,
        pub invalidations: HandleSpan<FlowInvalidationFact>,
        pub borrow_activations: HandleSpan<FlowBorrowActivationFact>,
        pub borrow_weakenings: HandleSpan<FlowBorrowWeakeningFact>,
        pub boundary_edges: HandleSpan<FlowBoundaryEdgeFact>,
        pub statements: HandleSpan<FlowStatementFact>,
        pub case_accesses: HandleSpan<super::FlowCaseAccessFact>,
        pub calls: HandleSpan<FlowCallFact>,
        pub exits: HandleSpan<FlowExitFact>,
        pub outgoing: HandleSpan<FlowStateOutgoingFact>,
        pub service_reach: ServiceReachSummary,
        pub suspension: SuspensionSummary,
        pub blocking: BlockingSummary,
    }
}
pub use call_origins::CheckedCallOrigin;
pub(crate) mod descriptor_observations {
    use super::FlowDescriptorContextObservation;
    use arena::{Handle, PagedArenaSet};
    use facts::FactContextHandle;

    #[derive(Debug, Clone, Default, compiler_storage::Record)]
    struct ContextObservations {
        context: FactContextHandle,
        rows: Vec<Handle<FlowDescriptorContextObservation>>,
    }

    #[derive(Debug, Clone, Default, compiler_storage::Record)]
    pub struct FlowDescriptorObservationTable {
        rows: PagedArenaSet<FlowDescriptorContextObservation>,
        by_context: Vec<ContextObservations>,
    }

    impl PartialEq for FlowDescriptorObservationTable {
        fn eq(&self, other: &Self) -> bool {
            self.rows == other.rows
        }
    }
    impl Eq for FlowDescriptorObservationTable {}

    fn key(context: FactContextHandle) -> (u32, u32) {
        (context.generation(), context.arena_index())
    }

    impl FlowDescriptorObservationTable {
        fn index(
            &mut self,
            context: FactContextHandle,
            row: Handle<FlowDescriptorContextObservation>,
        ) {
            let position = match self
                .by_context
                .binary_search_by_key(&key(context), |batch| key(batch.context))
            {
                Ok(position) => position,
                Err(position) => {
                    self.by_context.insert(
                        position,
                        ContextObservations {
                            context,
                            rows: Vec::new(),
                        },
                    );
                    position
                }
            };
            self.by_context[position].rows.push(row);
        }

        fn rebuild_index(&mut self) {
            self.by_context.clear();
            let mut rows = self
                .rows
                .iter()
                .map(|(handle, row)| (row.context, handle))
                .collect::<Vec<_>>();
            rows.sort_by_key(|(context, row)| (key(*context), row.generation(), row.arena_index()));
            for (context, row) in rows {
                if self
                    .by_context
                    .last()
                    .is_none_or(|batch| batch.context != context)
                {
                    self.by_context.push(ContextObservations {
                        context,
                        rows: Vec::new(),
                    });
                }
                self.by_context
                    .last_mut()
                    .expect("context batch appended above")
                    .rows
                    .push(row);
            }
        }

        pub fn append(
            &mut self,
            row: FlowDescriptorContextObservation,
        ) -> Handle<FlowDescriptorContextObservation> {
            let context = row.context;
            let handle = self.rows.append(row);
            self.index(context, handle);
            handle
        }

        pub fn for_context(
            &self,
            context: FactContextHandle,
        ) -> impl Iterator<Item = &FlowDescriptorContextObservation> {
            let position = self
                .by_context
                .binary_search_by_key(&key(context), |batch| key(batch.context))
                .ok();
            position
                .into_iter()
                .flat_map(|position| self.by_context[position].rows.iter())
                .map(|handle| self.rows.get(*handle))
        }

        pub fn reset_retain_capacity(&mut self) {
            self.rows.reset_retain_capacity();
            self.by_context.clear();
        }

        pub fn begin_page(&mut self, generation: u32) {
            self.rows.begin_page(generation);
        }

        pub fn take_additional_pages(&mut self) -> Self {
            let mut taken = Self {
                rows: self.rows.take_additional_pages(),
                by_context: Vec::new(),
            };
            self.rebuild_index();
            taken.rebuild_index();
            taken
        }

        pub fn adopt_additional_pages(&mut self, other: Self) {
            self.rows.adopt_additional_pages(other.rows);
            self.rebuild_index();
        }

        pub fn share_pages(&mut self) {
            self.rows.share_pages();
        }

        pub fn latest_generation(&self) -> u32 {
            self.rows.latest_generation()
        }

        pub fn current_generation(&self) -> u32 {
            self.rows.current_generation()
        }

        pub fn take_pages_from(&mut self, generation: u32, restored_current: u32) -> Self {
            let mut taken = Self {
                rows: self.rows.take_pages_from(generation, restored_current),
                by_context: Vec::new(),
            };
            self.rebuild_index();
            taken.rebuild_index();
            taken
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn observation(context: FactContextHandle, index: u32) -> FlowDescriptorContextObservation {
            FlowDescriptorContextObservation {
                context,
                observation: Handle::from_parts(index, 1),
            }
        }

        #[test]
        fn descriptor_context_batches_preserve_all_observations_and_generations() {
            let context = FactContextHandle::from_parts(5, 1);
            let replacement = FactContextHandle::from_parts(5, 2);
            let unrelated = FactContextHandle::from_parts(6, 1);
            let mut table = FlowDescriptorObservationTable::default();
            table.append(observation(context, 1));
            table.append(observation(unrelated, 2));
            table.append(observation(context, 3));
            table.append(observation(replacement, 4));
            assert_eq!(
                table
                    .for_context(context)
                    .map(|row| row.observation.arena_index())
                    .collect::<Vec<_>>(),
                [1, 3]
            );
            assert_eq!(
                table
                    .for_context(replacement)
                    .map(|row| row.observation.arena_index())
                    .collect::<Vec<_>>(),
                [4]
            );
            assert_eq!(table.for_context(unrelated).count(), 1);
            assert_eq!(table.for_context(FactContextHandle::invalid()).count(), 0);
        }

        #[test]
        fn descriptor_context_batches_follow_page_transfer_and_reset() {
            let context = FactContextHandle::from_parts(5, 1);
            let mut source = FlowDescriptorObservationTable::default();
            source.append(observation(context, 1));
            source.begin_page(2);
            source.append(observation(context, 2));
            let taken = source.take_additional_pages();
            assert_eq!(
                source
                    .for_context(context)
                    .map(|row| row.observation.arena_index())
                    .collect::<Vec<_>>(),
                [1]
            );
            assert_eq!(
                taken
                    .for_context(context)
                    .map(|row| row.observation.arena_index())
                    .collect::<Vec<_>>(),
                [2]
            );
            source.adopt_additional_pages(taken);
            assert_eq!(
                source
                    .for_context(context)
                    .map(|row| row.observation.arena_index())
                    .collect::<Vec<_>>(),
                [1, 2]
            );
            source.reset_retain_capacity();
            assert_eq!(source.for_context(context).count(), 0);
            source.append(observation(context, 3));
            assert_eq!(
                source
                    .for_context(context)
                    .map(|row| row.observation.arena_index())
                    .collect::<Vec<_>>(),
                [3]
            );
        }
        #[test]
        fn descriptor_context_batches_keep_disjoint_pages_and_clones() {
            let old_context = FactContextHandle::from_parts(5, 1);
            let new_context = FactContextHandle::from_parts(6, 1);
            let mut table = FlowDescriptorObservationTable::default();
            table.begin_page(2);
            table.append(observation(old_context, 1));
            let retained = table.clone();
            table.begin_page(3);
            assert_eq!(table.for_context(old_context).count(), 1);
            table.append(observation(new_context, 2));
            assert_eq!(table.for_context(old_context).count(), 1);
            assert_eq!(table.for_context(new_context).count(), 1);
            assert_eq!(retained.for_context(old_context).count(), 1);
            assert_eq!(retained.for_context(new_context).count(), 0);
        }
    }
}
pub(crate) mod invalidations {
    use arena::HandleSpan;
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum FlowInvalidationSource {
        Statement {
            statement_index: usize,
        },
        Call {
            statement_index: usize,
            call_ordinal: usize,
            target_symbol: SymbolHandle,
        },
    }

    impl Default for FlowInvalidationSource {
        fn default() -> Self {
            Self::Statement { statement_index: 0 }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowInvalidationFact {
        pub source: FlowInvalidationSource,
        pub context: facts::FactContextHandle,
        pub fact: facts::FactHandle,
        pub mutated_root: facts::PlaceRoot,
        pub mutated_segments: HandleSpan<facts::PlaceSegment>,
        pub dependency_segments: HandleSpan<facts::PlaceSegment>,
    }
}
pub(crate) mod operand {

    use arena::{Handle, HandleSpan};
    use typed_trees::expression::ExpressionHandle;

    use super::FlowConstraintRef;

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowCaseAccessFact {
        pub machine_symbol: symbols::SymbolHandle,
        pub state_symbol: symbols::SymbolHandle,
        pub expression: ExpressionHandle,
        pub evaluation: super::FlowPermissionEvaluation,
        pub statement_index: usize,
        pub constraints: HandleSpan<FlowConstraintRef>,
        pub places: HandleSpan<FlowOperandReferent>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowOperandReferent {
        pub formation: Handle<FlowReferentFormation>,
        pub root: facts::PlaceRoot,
        pub segments: HandleSpan<facts::PlaceSegment>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowReferentFormation {
        pub source: Handle<FlowOperandReferent>,
        pub evidence: FlowReferentFormationEvidence,
        pub owner: Handle<FlowOperandFact>,
        pub path: HandleSpan<facts::PlaceSegment>,
        pub root: facts::PlaceRoot,
        pub segments: HandleSpan<facts::PlaceSegment>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum FlowReferentFormationEvidence {
        #[default]
        Unknown,
        Access(Handle<FlowCaseAccessFact>),
        Return(Handle<super::FlowCallFact>),
    }

    impl FlowReferentFormation {
        pub fn source_access(&self) -> Handle<FlowCaseAccessFact> {
            match self.evidence {
                FlowReferentFormationEvidence::Access(access) => access,
                FlowReferentFormationEvidence::Unknown
                | FlowReferentFormationEvidence::Return(_) => Handle::invalid(),
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowOperandRetention {
        pub reborrow_sources: HandleSpan<FlowOperandReborrowSource>,
        pub possible_storage: HandleSpan<FlowOperandReferent>,
        pub enclosing_storage: Handle<FlowOperandFact>,
        pub enclosing_regions: HandleSpan<FlowOperandRetainedRegion>,
        pub descriptor: Handle<FlowOperandFact>,
        pub descriptor_sources: HandleSpan<FlowOperandRetainedSupplier>,
        pub immutable_image: bool,
        pub roots: HandleSpan<facts::PlaceRoot>,
        pub lifetime_sources: Option<HandleSpan<FlowOperandRetainedSupplier>>,
        pub suppliers: HandleSpan<FlowOperandRetainedSupplier>,
        pub alternatives: HandleSpan<FlowOperandRetention>,
        pub leaves: HandleSpan<FlowOperandRetainedLeaf>,
        pub stored_value: Handle<FlowOperandFact>,
        pub stored_projection: HandleSpan<facts::PlaceSegment>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowOperandRetainedRegion {
        pub operand: Handle<FlowOperandFact>,
        pub projection: HandleSpan<facts::PlaceSegment>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum ViewReturnInputSource {
        Referent,
        Stored,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowOperandLifetimeContract {
        pub state_symbol: symbols::SymbolHandle,
        pub statement_index: usize,
        pub invocation: ExpressionHandle,
        pub call_ordinal: usize,
        pub output_parameter: Option<usize>,
        pub input: Handle<FlowOperandFact>,
        pub parameter_index: usize,
        pub source: ViewReturnInputSource,
        pub source_type: typed_trees::types::TypeReferenceHandle,
        pub source_path: HandleSpan<facts::PlaceSegment>,
        pub owner_path: HandleSpan<facts::PlaceSegment>,
        pub stored_sources: HandleSpan<FlowOperandStoredSource>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum FlowOperandStoredSource {
        Value {
            path: HandleSpan<facts::PlaceSegment>,
            operand: Handle<FlowOperandFact>,
            reference: typed_trees::types::TypeReferenceHandle,
            projection: HandleSpan<facts::PlaceSegment>,
            slot: Option<FlowOperandReferent>,
        },
        Incoming {
            path: HandleSpan<facts::PlaceSegment>,
            slot: FlowOperandReferent,
        },
    }

    impl Default for FlowOperandStoredSource {
        fn default() -> Self {
            Self::Value {
                path: HandleSpan::empty(),
                operand: Handle::invalid(),
                reference: Handle::invalid(),
                projection: HandleSpan::empty(),
                slot: None,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowOutputLifetimeSources {
        pub output: facts::InvocationOutputIdentity,
        pub owner_path: HandleSpan<facts::PlaceSegment>,
        pub sources: Option<HandleSpan<FlowOperandRetainedSupplier>>,
    }

    impl Default for FlowOutputLifetimeSources {
        fn default() -> Self {
            Self {
                output: facts::InvocationOutputIdentity {
                    state_symbol: Handle::invalid(),
                    statement_index: 0,
                    call_ordinal: 0,
                    parameter_symbol: Handle::invalid(),
                },
                owner_path: HandleSpan::empty(),
                sources: None,
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowOperandRetainedSupplier {
        pub operand: Handle<FlowOperandFact>,
        pub projection: HandleSpan<facts::PlaceSegment>,
        pub contract: Option<FlowOperandLifetimeContract>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowOperandReborrowSource {
        pub operand: Handle<FlowOperandFact>,
        pub source_path: HandleSpan<facts::PlaceSegment>,
        pub projection: HandleSpan<facts::PlaceSegment>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowOperandRetainedLeaf {
        pub path: HandleSpan<facts::PlaceSegment>,
        pub retention: Handle<FlowOperandRetention>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum FlowDescriptorPermission {
        #[default]
        Unavailable,
        Loaded {
            loan: Handle<crate::BorrowLoanFact>,
            read: Handle<FlowValueObservationFact>,
        },
        SuppliedOutput {
            loan: Handle<crate::BorrowLoanFact>,
        },
    }

    impl FlowDescriptorPermission {
        pub fn loaded_loan(self) -> Handle<crate::BorrowLoanFact> {
            match self {
                Self::Loaded { loan, .. } => loan,
                Self::Unavailable | Self::SuppliedOutput { .. } => Handle::invalid(),
            }
        }

        pub fn loaded_read(self) -> Handle<FlowValueObservationFact> {
            match self {
                Self::Loaded { read, .. } => read,
                Self::Unavailable | Self::SuppliedOutput { .. } => Handle::invalid(),
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum FlowOperandAddressProjection {
        Place(HandleSpan<facts::PlaceSegment>),
        View(HandleSpan<facts::PlaceSegment>),
    }

    impl FlowOperandAddressProjection {
        pub fn segments(self) -> HandleSpan<facts::PlaceSegment> {
            match self {
                Self::Place(segments) | Self::View(segments) => segments,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowOperandFact {
        pub descriptor_permission: FlowDescriptorPermission,
        pub expression: ExpressionHandle,
        pub state_symbol: symbols::SymbolHandle,
        pub statement_index: usize,
        pub evaluation: super::FlowPermissionEvaluation,
        pub type_reference: typed_trees::types::TypeReferenceHandle,
        pub completion: FlowOperandCompletion,
        pub backing: Handle<FlowOperandFact>,
        pub address_projection: Option<FlowOperandAddressProjection>,
        pub evaluated_inputs: HandleSpan<FlowOperandFact>,
        pub conditional_inputs_from: Option<usize>,
        pub value_source: Handle<FlowOperandFact>,
        pub entry_origin_writes: HandleSpan<FlowOperandReferent>,
        pub entry_origin_unknown_write: bool,
        pub storage_snapshot: Handle<super::FlowStorageSnapshot>,
        pub entry_binding_writes: HandleSpan<FlowOperandReferent>,
        pub entry_binding_unknown_write: bool,
        pub case_access: Handle<FlowCaseAccessFact>,
        pub constraints: HandleSpan<FlowConstraintRef>,
        pub referents: HandleSpan<FlowOperandReferent>,
        pub retention: Handle<FlowOperandRetention>,
        pub reference_leaves: HandleSpan<FlowCapturedReferenceLeaf>,
    }

    impl Default for FlowOperandFact {
        fn default() -> Self {
            Self {
                descriptor_permission: Default::default(),
                expression: Default::default(),
                state_symbol: Default::default(),
                statement_index: 0,
                evaluation: super::FlowPermissionEvaluation::Unknown,
                type_reference: Default::default(),
                completion: Default::default(),
                backing: Default::default(),
                address_projection: None,
                evaluated_inputs: HandleSpan::empty(),
                conditional_inputs_from: None,
                value_source: Default::default(),
                entry_origin_writes: HandleSpan::empty(),
                entry_origin_unknown_write: false,
                storage_snapshot: Default::default(),
                entry_binding_writes: HandleSpan::empty(),
                entry_binding_unknown_write: false,
                case_access: Default::default(),
                constraints: HandleSpan::empty(),
                referents: HandleSpan::empty(),
                retention: Default::default(),
                reference_leaves: HandleSpan::empty(),
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum FlowOperandCompletion {
        #[default]
        Observation,
        IncomingContents {
            root: facts::PlaceRoot,
            path: HandleSpan<facts::PlaceSegment>,
        },
        InvocationResult {
            invocation: Handle<super::FlowCallFact>,
            result_type: typed_trees::types::TypeReferenceHandle,
            path: HandleSpan<facts::PlaceSegment>,
        },
        Constructed(HandleSpan<FlowOperandChild>),
        Selected(HandleSpan<FlowOperandAlternative>),
        Copied {
            copy: Handle<FlowCopiedValue>,
            path: HandleSpan<facts::PlaceSegment>,
        },
    }

    impl FlowOperandFact {
        pub fn constructed_children(&self) -> HandleSpan<FlowOperandChild> {
            match self.completion {
                FlowOperandCompletion::Constructed(children) => children,
                FlowOperandCompletion::Observation
                | FlowOperandCompletion::IncomingContents { .. }
                | FlowOperandCompletion::InvocationResult { .. }
                | FlowOperandCompletion::Selected(_)
                | FlowOperandCompletion::Copied { .. } => HandleSpan::empty(),
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowOperandAlternative {
        pub prior_evaluation: Handle<FlowOperandAlternative>,
        pub evaluation_inputs: HandleSpan<FlowOperandFact>,
        pub source_arm: Handle<typed_trees::expression::TableMatchArm>,
        pub value: Handle<FlowOperandFact>,
    }

    /// One checked case decision on the unchanged owned home of a completed
    /// subject. The statement owns these rows in evaluation order. They introduce
    /// no transfer or disposal; cleanup still meets the actual branch outcomes.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowOwnedCaseSelection {
        pub machine_symbol: symbols::SymbolHandle,
        pub state_symbol: symbols::SymbolHandle,
        pub statement_index: usize,
        pub statement: Handle<super::FlowStatementFact>,
        pub expression: ExpressionHandle,
        pub source_arm: Handle<typed_trees::expression::TableMatchArm>,
        pub evaluation: super::FlowPermissionEvaluation,
        pub subject: Handle<FlowOperandFact>,
        pub root: facts::PlaceRoot,
        pub path: HandleSpan<facts::PlaceSegment>,
        pub variant: symbols::SymbolHandle,
        /// Only a total case pattern's failure excludes that case. A failed guard
        /// or payload equality may still have observed the same active case.
        pub miss_excludes_case: bool,
        pub captured_storage: Handle<super::FlowStorageSnapshot>,
        pub selected_storage: Handle<super::FlowStorageSnapshot>,
    }

    /// One authored child installed at its resolved destination in a completed
    /// record, case, or array. Rows are in evaluation order; paths name declaration
    /// identities and are stored in `FlowControlFacts::operand_child_paths`.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowOperandChild {
        pub parent: ExpressionHandle,
        pub path: HandleSpan<facts::PlaceSegment>,
        pub value: Handle<FlowOperandFact>,
        pub type_reference: typed_trees::types::TypeReferenceHandle,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowValueObservationFact {
        pub expression: ExpressionHandle,
        pub evaluation: super::FlowPermissionEvaluation,
        pub receiver: Handle<FlowOperandFact>,
        pub result: Handle<FlowOperandFact>,
        pub places: HandleSpan<FlowOperandReferent>,
        pub origin_writes: HandleSpan<FlowOperandReferent>,
        pub origin_unknown_write: bool,
        pub semantic_contexts: HandleSpan<super::FlowSemanticContextRef>,
        pub constraints: HandleSpan<FlowConstraintRef>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowDescriptorContextObservation {
        pub context: facts::FactContextHandle,
        pub observation: Handle<FlowValueObservationFact>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowAtomicExecutionFact {
        pub state_symbol: symbols::SymbolHandle,
        pub statement_index: usize,
        pub expression: ExpressionHandle,
        pub operands: HandleSpan<FlowOperandFact>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowCopiedValue {
        pub source: Handle<FlowOperandFact>,
        pub references: HandleSpan<FlowCapturedReferenceLeaf>,
        pub uses: HandleSpan<FlowCopiedValueUse>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowCapturedReferenceLeaf {
        pub path: HandleSpan<facts::PlaceSegment>,
        pub type_reference: typed_trees::types::TypeReferenceHandle,
        pub referents: HandleSpan<FlowOperandReferent>,
        pub retention: Handle<FlowOperandRetention>,
        pub constraints: HandleSpan<FlowConstraintRef>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowCopiedValueUse {
        pub expression: ExpressionHandle,
        pub evaluation: super::FlowPermissionEvaluation,
    }
}
pub(crate) mod storage {
    use super::{FlowOperandFact, FlowOperandReferent};
    use arena::{Handle, HandleSpan};

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowStorageSnapshot {
        pub previous: Handle<FlowStorageSnapshot>,
        pub operation: FlowStorageOperation,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum FlowStorageOperation {
        #[default]
        Unknown,
        Writes {
            bindings: HandleSpan<FlowOperandReferent>,
            contents: HandleSpan<FlowOperandReferent>,
            bindings_complete: bool,
            contents_complete: bool,
        },
        Store {
            destination: FlowOperandReferent,
            value: Handle<FlowOperandFact>,
            projection: HandleSpan<facts::PlaceSegment>,
        },
        PreserveBinding {
            destination: FlowOperandReferent,
            source: Handle<FlowStorageSnapshot>,
        },
        Join {
            predecessors: HandleSpan<Handle<FlowStorageSnapshot>>,
        },
    }
}
pub(crate) use crate::flow_ownership_and_roots::ownership;
pub use storage::{FlowStorageOperation, FlowStorageSnapshot};
pub(crate) mod semantic_dependencies {
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, compiler_storage::Record)]
    pub enum CheckedSemanticDependencyExposure {
        PrivateImplementation,
        PublicInterface,
    }

    pub use semantic_vocabulary::SemanticDependencyKind as CheckedSemanticDependencyKind;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedSemanticDependency {
        pub consumer_machine: SymbolHandle,
        pub dependency: SymbolHandle,
        pub exposure: CheckedSemanticDependencyExposure,
        pub kind: CheckedSemanticDependencyKind,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedSemanticDependencies {
        pub rows: Vec<CheckedSemanticDependency>,
    }

    impl CheckedSemanticDependencies {
        pub fn iter(&self) -> impl ExactSizeIterator<Item = &CheckedSemanticDependency> {
            self.rows.iter()
        }
    }
}
pub(crate) mod state_rows {
    use super::{FlowControlFacts, FlowStateFact};
    use crate::keyed_rows::{DurableRows, KeyedRows};
    use arena::Handle;
    use std::sync::OnceLock;
    use symbols::SymbolHandle;
    type Arena<T> = arena::PagedArenaSet<T>;

    #[derive(Default, compiler_storage::Record)]
    pub struct FlowStateTable {
        arena: Arena<FlowStateFact>,
        machine_rows: KeyedRows<SymbolHandle, FlowStateFact>,
        state_rows: KeyedRows<SymbolHandle, FlowStateFact>,
        #[record(skip)]
        call_rows: KeyedRows<u32, FlowStateFact>,
        #[record(skip)]
        revision: OnceLock<u64>,
    }

    impl Clone for FlowStateTable {
        fn clone(&self) -> Self {
            Self {
                arena: self.arena.clone(),
                machine_rows: self.machine_rows.clone(),
                state_rows: self.state_rows.clone(),
                call_rows: self.call_rows.clone(),
                revision: OnceLock::new(),
            }
        }
    }

    impl std::fmt::Debug for FlowStateTable {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("FlowStateTable")
                .field("arena", &self.arena)
                .field("machine_rows", &self.machine_rows)
                .field("state_rows", &self.state_rows)
                .finish()
        }
    }

    impl PartialEq for FlowStateTable {
        fn eq(&self, other: &Self) -> bool {
            self.arena == other.arena
        }
    }

    impl Eq for FlowStateTable {}

    impl DurableRows for FlowStateTable {
        type Row = FlowStateFact;

        fn revision(&self) -> u64 {
            *self.revision.get_or_init(|| self.arena.revision())
        }

        fn rows(&self) -> impl Iterator<Item = (Handle<FlowStateFact>, &FlowStateFact)> {
            self.arena.iter()
        }

        fn append_epoch(&self) -> Option<(u64, usize)> {
            self.arena.append_epoch()
        }

        fn for_each_row_from(
            &self,
            start: usize,
            visit: impl FnMut(Handle<FlowStateFact>, &FlowStateFact),
        ) {
            self.arena.for_each_from(start, visit);
        }
    }

    impl From<Arena<FlowStateFact>> for FlowStateTable {
        fn from(arena: Arena<FlowStateFact>) -> Self {
            Self {
                arena,
                machine_rows: KeyedRows::default(),
                state_rows: KeyedRows::default(),
                call_rows: KeyedRows::default(),
                revision: OnceLock::new(),
            }
        }
    }

    impl std::ops::Deref for FlowStateTable {
        type Target = Arena<FlowStateFact>;

        fn deref(&self) -> &Self::Target {
            &self.arena
        }
    }

    fn grouping_keys(row: &FlowStateFact) -> (SymbolHandle, SymbolHandle, u32) {
        (
            row.machine_symbol,
            row.state_symbol,
            row.calls.start().generation(),
        )
    }

    impl FlowStateTable {
        pub fn append(&mut self, row: FlowStateFact) -> Handle<FlowStateFact> {
            self.revision.take();
            self.arena.append(row)
        }

        pub fn update_row(
            &mut self,
            handle: Handle<FlowStateFact>,
            edit: impl FnOnce(&mut FlowStateFact),
        ) {
            let row = self.arena.get_mut(handle);
            let keys = grouping_keys(row);
            edit(row);
            if grouping_keys(row) != keys {
                self.revision.take();
                self.machine_rows.clear();
                self.state_rows.clear();
                self.call_rows.clear();
            }
        }
    }

    impl std::ops::DerefMut for FlowStateTable {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.revision.take();
            self.machine_rows.clear();
            self.state_rows.clear();
            self.call_rows.clear();
            &mut self.arena
        }
    }

    impl FlowControlFacts {
        pub fn call_owners(&self, call: Handle<super::FlowCallFact>) -> Vec<&FlowStateFact> {
            if !self.calls.is_valid(call) {
                return Vec::new();
            }
            self.states.call_rows.with_rows(
                &self.states,
                |state| state.calls.start().generation(),
                &call.generation(),
                |rows| {
                    rows.iter()
                        .map(|owner| self.states.get(*owner))
                        .filter(|state| {
                            if self.calls.span(state.calls).is_none() {
                                return false;
                            }
                            let Some(offset) = call
                                .arena_index()
                                .checked_sub(state.calls.start().arena_index())
                            else {
                                return false;
                            };
                            state.calls.handles().nth(offset as usize) == Some(call)
                        })
                        .collect()
                },
            )
        }

        fn with_machine_rows<Result>(
            &self,
            machine: SymbolHandle,
            read: impl FnOnce(&[Handle<FlowStateFact>]) -> Result,
        ) -> Result {
            self.states.machine_rows.with_rows(
                &self.states,
                |state| state.machine_symbol,
                &machine,
                read,
            )
        }

        pub fn state_by_symbol(&self, state: SymbolHandle) -> Option<&FlowStateFact> {
            self.states.state_rows.with_rows(
                &self.states,
                |row| row.state_symbol,
                &state,
                |rows| rows.first().map(|handle| self.states.get(*handle)),
            )
        }

        pub fn unique_state_by_symbol(&self, state: SymbolHandle) -> Option<&FlowStateFact> {
            self.states.state_rows.with_rows(
                &self.states,
                |row| row.state_symbol,
                &state,
                |rows| match rows {
                    [handle] => Some(self.states.get(*handle)),
                    _ => None,
                },
            )
        }

        pub fn state(&self, machine: SymbolHandle, state: SymbolHandle) -> Option<&FlowStateFact> {
            self.with_machine_rows(machine, |rows| {
                rows.iter().find_map(|handle| {
                    let row = self.states.get(*handle);
                    (row.state_symbol == state).then_some(row)
                })
            })
        }

        pub fn unique_state(
            &self,
            machine: SymbolHandle,
            state: SymbolHandle,
        ) -> Option<&FlowStateFact> {
            self.with_machine_rows(machine, |rows| {
                let mut matching = rows.iter().filter_map(|handle| {
                    let row = self.states.get(*handle);
                    (row.state_symbol == state).then_some(row)
                });
                let first = matching.next()?;
                matching.next().is_none().then_some(first)
            })
        }

        pub fn machine_states(&self, machine: SymbolHandle) -> Vec<&FlowStateFact> {
            self.with_machine_rows(machine, |rows| {
                rows.iter().map(|handle| self.states.get(*handle)).collect()
            })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn unique_state_symbol_keeps_duplicates_and_generations_distinct() {
            let mut control = FlowControlFacts::default();
            let state = SymbolHandle::from_parts(7, 1);
            let other_generation = SymbolHandle::from_parts(7, 2);
            assert!(control.unique_state_by_symbol(state).is_none());
            control.states.insert(FlowStateFact {
                state_symbol: state,
                ..FlowStateFact::default()
            });
            assert_eq!(
                control
                    .unique_state_by_symbol(state)
                    .map(|row| row.state_symbol),
                Some(state)
            );
            assert!(control.unique_state_by_symbol(other_generation).is_none());
            control.states.insert(FlowStateFact {
                state_symbol: other_generation,
                ..FlowStateFact::default()
            });
            assert_eq!(
                control
                    .unique_state_by_symbol(other_generation)
                    .map(|row| row.state_symbol),
                Some(other_generation)
            );
            control.states.insert(FlowStateFact {
                state_symbol: state,
                ..FlowStateFact::default()
            });
            assert!(control.unique_state_by_symbol(state).is_none());
            assert!(control.unique_state_by_symbol(other_generation).is_some());
        }

        fn owner_matches_scan(
            control: &FlowControlFacts,
            call: Handle<super::super::FlowCallFact>,
        ) {
            let expected = if control.calls.is_valid(call) {
                control
                    .states
                    .iter()
                    .filter_map(|(_, state)| {
                        control
                            .calls
                            .span_or_empty(state.calls)
                            .iter()
                            .any(|row| std::ptr::eq(row, control.calls.get(call)))
                            .then_some(state)
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            let actual = control.call_owners(call);
            assert_eq!(actual.len(), expected.len());
            assert!(
                actual
                    .iter()
                    .zip(expected)
                    .all(|(actual, expected)| std::ptr::eq(*actual, expected))
            );
        }

        #[test]
        fn call_page_rows_keep_duplicate_owners_and_follow_state_edits() {
            let mut control = FlowControlFacts::default();
            let first = control.calls.append(Default::default());
            control.calls.begin_page(11);
            let second = control.calls.append(Default::default());
            let first_span = arena::HandleSpan::from_parts(first, 1);
            let second_span = arena::HandleSpan::from_parts(second, 1);
            let owner = control.states.append(FlowStateFact {
                calls: first_span,
                ..Default::default()
            });
            owner_matches_scan(&control, first);
            owner_matches_scan(&control, second);
            control.states.append(FlowStateFact {
                calls: first_span,
                ..Default::default()
            });
            assert_eq!(control.call_owners(first).len(), 2);
            control.states.get_mut(owner).calls = second_span;
            owner_matches_scan(&control, first);
            owner_matches_scan(&control, second);
            control.states.get_mut(owner).calls = arena::HandleSpan::from_parts(second, 20);
            owner_matches_scan(&control, second);
            let cloned = control.clone();
            owner_matches_scan(&cloned, first);
            owner_matches_scan(&cloned, second);
            let mut original = Arena::default();
            original.append(FlowStateFact {
                calls: first_span,
                ..Default::default()
            });
            let mut replacement = Arena::default();
            replacement.append(FlowStateFact {
                calls: second_span,
                ..Default::default()
            });
            assert_eq!(original.revision(), replacement.revision());
            *control.states = original;
            owner_matches_scan(&control, first);
            *control.states = replacement;
            owner_matches_scan(&control, first);
            owner_matches_scan(&control, second);
            let limits = compiler_storage::RecordLimits::default();
            let before = compiler_storage::encode(&control.states, limits).unwrap();
            owner_matches_scan(&control, second);
            assert_eq!(
                compiler_storage::encode(&control.states, limits).unwrap(),
                before
            );
            let states = compiler_storage::decode(&before, limits).unwrap();
            control.states = states;
            owner_matches_scan(&control, second);
        }

        #[test]
        fn machine_rows_preserve_roster_selection_and_mutation_visibility() {
            let machine = SymbolHandle::from_parts(7, 1);
            let other_machine = SymbolHandle::from_parts(2, 1);
            let replacement_machine = SymbolHandle::from_parts(7, 2);
            let state = SymbolHandle::from_parts(3, 1);
            let other_state = SymbolHandle::from_parts(1, 1);
            let replacement_state = SymbolHandle::from_parts(3, 2);
            let assert_roster = |facts: &FlowControlFacts| {
                for owner in [
                    machine,
                    other_machine,
                    replacement_machine,
                    SymbolHandle::invalid(),
                ] {
                    let expected: Vec<_> = facts
                        .states
                        .iter()
                        .filter_map(|(_, row)| (row.machine_symbol == owner).then_some(row))
                        .collect();
                    assert_eq!(facts.machine_states(owner), expected);
                    for symbol in [
                        state,
                        other_state,
                        replacement_state,
                        SymbolHandle::invalid(),
                    ] {
                        let matching: Vec<_> = expected
                            .iter()
                            .copied()
                            .filter(|row| row.state_symbol == symbol)
                            .collect();
                        assert_eq!(facts.state(owner, symbol), matching.first().copied());
                        assert_eq!(
                            facts.unique_state(owner, symbol),
                            match matching.as_slice() {
                                [only] => Some(*only),
                                _ => None,
                            }
                        );
                    }
                }
                assert_eq!(
                    facts.states.revision.get().copied(),
                    Some(facts.states.arena.revision())
                );
            };
            let mut facts = FlowControlFacts::default();
            assert_roster(&facts);
            let first = facts.states.insert(FlowStateFact {
                machine_symbol: machine,
                state_symbol: state,
                mutable_parameter_count: 1,
                ..FlowStateFact::default()
            });
            assert_roster(&facts);
            for (owner, symbol, marker) in [
                (other_machine, state, 2),
                (machine, other_state, 3),
                (machine, state, 4),
                (replacement_machine, state, 5),
                (machine, replacement_state, 6),
            ] {
                facts.states.insert(FlowStateFact {
                    machine_symbol: owner,
                    state_symbol: symbol,
                    mutable_parameter_count: marker,
                    ..FlowStateFact::default()
                });
                assert_roster(&facts);
            }
            let snapshot = facts.clone();
            assert!(snapshot.states.revision.get().is_none());
            assert_eq!(facts, snapshot);
            facts.states.get_mut(first).machine_symbol = other_machine;
            assert!(facts.states.revision.get().is_none());
            assert_roster(&facts);
            assert_roster(&snapshot);
            let mut replacement = Arena::default();
            replacement.insert(FlowStateFact {
                machine_symbol: replacement_machine,
                state_symbol: replacement_state,
                ..FlowStateFact::default()
            });
            let mut equal_revision = FlowControlFacts::default();
            equal_revision.states.insert(FlowStateFact {
                machine_symbol: machine,
                state_symbol: state,
                ..FlowStateFact::default()
            });
            assert_roster(&equal_revision);
            assert_eq!(equal_revision.states.revision(), replacement.revision());
            *equal_revision.states = replacement;
            assert!(equal_revision.states.revision.get().is_none());
            assert_roster(&equal_revision);
            facts.states.insert(FlowStateFact {
                machine_symbol: replacement_machine,
                state_symbol: replacement_state,
                ..FlowStateFact::default()
            });
            assert_roster(&facts);
            facts.states.reset_retain_capacity();
            assert_roster(&facts);
            assert_roster(&snapshot);
        }

        #[test]
        fn unkeyed_row_edit_keeps_groupings_and_keyed_edit_rebuilds_them() {
            let machine = SymbolHandle::from_parts(7, 1);
            let other_machine = SymbolHandle::from_parts(2, 1);
            let state = SymbolHandle::from_parts(3, 1);
            let mut facts = FlowControlFacts::default();
            let first = facts.states.insert(FlowStateFact {
                machine_symbol: machine,
                state_symbol: state,
                ..FlowStateFact::default()
            });
            assert!(facts.state(machine, state).is_some());
            let roster = facts.states.revision.get().copied();
            facts
                .states
                .update_row(first, |row| row.mutable_parameter_count = 9);
            assert_eq!(facts.states.revision.get().copied(), roster);
            assert_eq!(
                facts
                    .state(machine, state)
                    .map(|row| row.mutable_parameter_count),
                Some(9)
            );
            let later = SymbolHandle::from_parts(4, 1);
            facts.states.append(FlowStateFact {
                machine_symbol: machine,
                state_symbol: later,
                ..FlowStateFact::default()
            });
            assert_eq!(
                facts
                    .machine_states(machine)
                    .iter()
                    .map(|row| row.state_symbol)
                    .collect::<Vec<_>>(),
                [state, later]
            );
            assert!(facts.state_by_symbol(later).is_some());
            facts
                .states
                .update_row(first, |row| row.machine_symbol = other_machine);
            assert!(facts.state(machine, state).is_none());
            assert!(facts.state(machine, later).is_some());
            assert_eq!(
                facts
                    .state(other_machine, state)
                    .map(|row| row.mutable_parameter_count),
                Some(9)
            );
        }
    }
}
pub use state_rows::FlowStateTable;

pub(crate) mod store_plans {
    use symbols::SymbolHandle;
    use typed_trees::types::PrimitiveType;

    use crate::{
        CheckedByteSequenceValuePlan, CheckedScalarCaseFieldPlan, CheckedScalarExpression,
        CheckedScalarExpressionRole, CheckedUnitStructuralArgumentPlan,
        CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    };

    use super::CheckedUnitStructuralPathSegment;
    use crate::CheckedMemberIdentity;
    use typed_trees::type_identity::TypeIdentityId;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedStorePlan {
        pub statement_index: u32,
        pub captured_selection: Vec<CheckedUnitStructuralPathSegment>,
        pub destination: CheckedStoreDestination,
        pub value: CheckedStoreValue,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedStoreDestination {
        pub root: CheckedStoreRoot,
        pub path: Vec<CheckedUnitStructuralPathSegment>,
        pub leaf: CheckedStoreLeaf,
    }

    impl CheckedStorePlan {
        pub fn installed_value(&self) -> Option<&CheckedUnitStructuralArgumentPlan> {
            match (&self.destination.leaf, &self.value) {
                (
                    CheckedStoreLeaf::StructuralStore { .. } | CheckedStoreLeaf::OwnedRebind { .. },
                    CheckedStoreValue::Place(value),
                ) => Some(value),
                _ => None,
            }
        }

        pub fn window_place(
            &self,
            structural_parameters: &[CheckedUnitStructuralParameterPlan],
        ) -> Option<CheckedUnitStructuralArgumentPlan> {
            let (CheckedStoreLeaf::StructuralStore { type_identity, .. }
            | CheckedStoreLeaf::OwnedRebind { type_identity }) = &self.destination.leaf
            else {
                return None;
            };
            let source = match self.destination.root {
                CheckedStoreRoot::Parameter { position } => {
                    CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: u32::try_from(
                            structural_parameters
                                .iter()
                                .position(|parameter| parameter.position == position)?,
                        )
                        .ok()?,
                    }
                }
                CheckedStoreRoot::Local { symbol } => {
                    CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
                }
            };
            Some(CheckedUnitStructuralArgumentPlan {
                projection_evaluations: Default::default(),
                source,
                path: self.destination.path.clone(),
                type_identity: *type_identity,
                access: crate::CheckedStructuralAccess::Owned,
            })
        }
    }

    impl CheckedStoreDestination {
        pub fn field(
            &self,
        ) -> Option<(&[CheckedUnitStructuralPathSegment], &CheckedMemberIdentity)> {
            match self.path.split_last()? {
                (CheckedUnitStructuralPathSegment::Field(identity), carrier) => {
                    Some((carrier, identity))
                }
                _ => None,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedPrimitiveReceivingRange {
        pub primitive: PrimitiveType,
        pub integer: semantic_vocabulary::BoundedIntegerType,
        pub decision: arena::Handle<crate::CheckedIntegerAssignmentDecision>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedStoreLeaf {
        Primitive {
            primitive_type: PrimitiveType,
            preserves_byte_memberships: bool,
            receiving_range: Option<CheckedPrimitiveReceivingRange>,
        },
        ScalarField {
            primitive_type: PrimitiveType,
            range_decision: arena::Handle<crate::CheckedIntegerAssignmentDecision>,
        },
        ByteElement {
            index: CheckedScalarExpression,
            decisions: CheckedIndexedWriteDecisions,
        },
        ViewElement {
            index: CheckedScalarExpression,
            element_path: Vec<CheckedUnitStructuralPathSegment>,
            element: PrimitiveType,
            range_decision: arena::Handle<crate::CheckedIntegerAssignmentDecision>,
            decisions: CheckedIndexedWriteDecisions,
        },
        RepresentationView {
            view: PrimitiveType,
        },
        ByteContents,
        ByteDescriptor,
        SumField,
        StructuralStore {
            type_identity: TypeIdentityId,
            multiplicity: language_semantics::Multiplicity,
            opening_binding: Option<u32>,
        },
        OwnedRebind {
            type_identity: TypeIdentityId,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedStoreValue {
        Scalar {
            role: CheckedScalarExpressionRole,
            value: CheckedScalarStoreValue,
        },
        Bytes(CheckedByteSequenceValuePlan),
        Place(CheckedUnitStructuralArgumentPlan),
        CaseLiteral(CheckedCaseLiteralValuePlan),
    }

    impl CheckedStoreValue {
        pub fn assigned(value: CheckedScalarStoreValue) -> Self {
            Self::Scalar {
                role: CheckedScalarExpressionRole::AssignmentValue,
                value,
            }
        }

        pub fn scalar(&self) -> Option<(CheckedScalarExpressionRole, &CheckedScalarStoreValue)> {
            match self {
                Self::Scalar { role, value } => Some((*role, value)),
                Self::Bytes(_) | Self::Place(_) | Self::CaseLiteral(_) => None,
            }
        }

        pub fn bytes(&self) -> Option<&CheckedByteSequenceValuePlan> {
            match self {
                Self::Bytes(value) => Some(value),
                Self::Scalar { .. } | Self::Place(_) | Self::CaseLiteral(_) => None,
            }
        }

        pub fn bytes_mut(&mut self) -> Option<&mut CheckedByteSequenceValuePlan> {
            match self {
                Self::Bytes(value) => Some(value),
                Self::Scalar { .. } | Self::Place(_) | Self::CaseLiteral(_) => None,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedCaseLiteralValuePlan {
        pub literal: typed_trees::expression::ExpressionHandle,
        pub case_identity: CheckedMemberIdentity,
        pub type_identity: TypeIdentityId,
        pub fields: Vec<CheckedScalarCaseFieldPlan>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
    pub struct CheckedIndexedWriteDecisions {
        pub index_in_bounds: bool,
        pub predicates_preserved: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedScalarStoreValue {
        Pure(CheckedScalarExpression),
        Computation(crate::CheckedScalarComputationHandle),
        ScalarResult { position: u32 },
    }

    impl CheckedScalarStoreValue {
        pub fn as_pure(&self) -> Option<&CheckedScalarExpression> {
            match self {
                Self::Pure(expression) => Some(expression),
                Self::Computation(_) | Self::ScalarResult { .. } => None,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedStoreRoot {
        Parameter { position: u32 },
        Local { symbol: SymbolHandle },
    }

    impl CheckedStoreRoot {
        pub fn parameter_position(self) -> Option<u32> {
            match self {
                Self::Parameter { position } => Some(position),
                Self::Local { .. } => None,
            }
        }
    }
}
pub(crate) use crate::flow_terminal as terminal;

pub(crate) mod queries {
    pub(crate) mod constraints {
        use arena::{Handle, HandleSpan};

        use crate::{BorrowCallFact, BorrowLoanFact, BorrowWritableRootFact};

        use super::super::{FlowConstraintKind, FlowConstraintRef, FlowContextFacts, FlowFacts};
        use crate::FlowFactsRef;

        impl FlowContextFacts {
            pub fn semantic_constraint_contexts(
                &self,
                constraints: HandleSpan<FlowConstraintRef>,
            ) -> impl Iterator<Item = facts::FactContextHandle> + '_ {
                self.constraint_refs
                    .span_or_empty(constraints)
                    .iter()
                    .filter_map(|constraint| match constraint.kind {
                        FlowConstraintKind::SemanticContext { context } => Some(context),
                        FlowConstraintKind::Unknown | crate::borrow_resource_pattern!(..) => None,
                    })
            }
        }

        impl FlowFacts {
            pub fn constraints(
                &self,
                constraints: HandleSpan<FlowConstraintRef>,
            ) -> &[FlowConstraintRef] {
                self.view().constraints(constraints)
            }

            pub fn semantic_constraint_contexts(
                &self,
                constraints: HandleSpan<FlowConstraintRef>,
            ) -> impl Iterator<Item = facts::FactContextHandle> + '_ {
                self.view().semantic_constraint_contexts(constraints)
            }

            pub fn borrow_call_constraints(
                &self,
                constraints: HandleSpan<FlowConstraintRef>,
            ) -> impl Iterator<Item = Handle<BorrowCallFact>> + '_ {
                self.view().borrow_call_constraints(constraints)
            }

            pub fn borrow_writable_root_constraints(
                &self,
                constraints: HandleSpan<FlowConstraintRef>,
            ) -> impl Iterator<Item = Handle<BorrowWritableRootFact>> + '_ {
                self.view().borrow_writable_root_constraints(constraints)
            }

            pub fn incoming_reference_constraints(
                &self,
                constraints: HandleSpan<FlowConstraintRef>,
            ) -> impl Iterator<Item = Handle<crate::IncomingReferenceIdentity>> + '_ {
                self.view().incoming_reference_constraints(constraints)
            }

            pub fn borrow_loan_constraints(
                &self,
                constraints: HandleSpan<FlowConstraintRef>,
            ) -> impl Iterator<Item = Handle<BorrowLoanFact>> + '_ {
                self.view().borrow_loan_constraints(constraints)
            }
        }

        impl<'f> FlowFactsRef<'f> {
            pub fn constraints(
                self,
                constraints: HandleSpan<FlowConstraintRef>,
            ) -> &'f [FlowConstraintRef] {
                self.contexts.constraint_refs.span_or_empty(constraints)
            }

            pub fn semantic_constraint_contexts(
                self,
                constraints: HandleSpan<FlowConstraintRef>,
            ) -> impl Iterator<Item = facts::FactContextHandle> + 'f {
                self.contexts.semantic_constraint_contexts(constraints)
            }

            pub fn borrow_call_constraints(
                self,
                constraints: HandleSpan<FlowConstraintRef>,
            ) -> impl Iterator<Item = Handle<BorrowCallFact>> + 'f {
                self.constraints(constraints)
                    .iter()
                    .filter_map(|constraint| match constraint.kind {
                        FlowConstraintKind::BorrowCall { call } => Some(call),
                        _ => None,
                    })
            }

            pub fn borrow_writable_root_constraints(
                self,
                constraints: HandleSpan<FlowConstraintRef>,
            ) -> impl Iterator<Item = Handle<BorrowWritableRootFact>> + 'f {
                self.constraints(constraints)
                    .iter()
                    .filter_map(|constraint| match constraint.kind {
                        FlowConstraintKind::BorrowWritableRoot { root } => Some(root),
                        _ => None,
                    })
            }

            pub fn incoming_reference_constraints(
                self,
                constraints: HandleSpan<FlowConstraintRef>,
            ) -> impl Iterator<Item = Handle<crate::IncomingReferenceIdentity>> + 'f {
                self.constraints(constraints)
                    .iter()
                    .filter_map(|constraint| {
                        if let FlowConstraintKind::BorrowIncomingReference { identity } =
                            constraint.kind
                        {
                            Some(identity)
                        } else {
                            None
                        }
                    })
            }

            pub fn borrow_loan_constraints(
                self,
                constraints: HandleSpan<FlowConstraintRef>,
            ) -> impl Iterator<Item = Handle<BorrowLoanFact>> + 'f {
                self.constraints(constraints)
                    .iter()
                    .filter_map(|constraint| match constraint.kind {
                        FlowConstraintKind::BorrowLoan { loan } => Some(loan),
                        FlowConstraintKind::Unknown | crate::nonloan_constraint_pattern!(..) => {
                            None
                        }
                    })
            }
        }
    }
    pub(crate) mod state {
        use arena::HandleSpan;
        use symbols::SymbolHandle;

        use super::super::{
            FlowCallFact, FlowConstraintRef, FlowFacts, FlowInvalidationFact,
            FlowInvalidationSource, FlowStateFact, FlowStatementFact,
        };
        use crate::FlowFactsRef;

        impl FlowFacts {
            pub fn state_statement(
                &self,
                state: &FlowStateFact,
                statement_index: usize,
            ) -> Option<&FlowStatementFact> {
                self.view().state_statement(state, statement_index)
            }

            pub fn state_call(
                &self,
                state: &FlowStateFact,
                statement_index: usize,
                call_ordinal: usize,
                target_symbol: SymbolHandle,
                receiver_symbol: SymbolHandle,
            ) -> Option<&FlowCallFact> {
                self.view().state_call(
                    state,
                    statement_index,
                    call_ordinal,
                    target_symbol,
                    receiver_symbol,
                )
            }

            pub fn state_call_entry_constraints(
                &self,
                state: &FlowStateFact,
                statement_index: usize,
                call_ordinal: usize,
                target_symbol: SymbolHandle,
                receiver_symbol: SymbolHandle,
            ) -> HandleSpan<FlowConstraintRef> {
                self.view().state_call_entry_constraints(
                    state,
                    statement_index,
                    call_ordinal,
                    target_symbol,
                    receiver_symbol,
                )
            }

            pub fn state_call_entry_semantic_contexts(
                &self,
                state: &FlowStateFact,
                statement_index: usize,
                call_ordinal: usize,
                target_symbol: SymbolHandle,
                receiver_symbol: SymbolHandle,
            ) -> impl Iterator<Item = facts::FactContextHandle> + '_ {
                self.view().state_call_entry_semantic_contexts(
                    state,
                    statement_index,
                    call_ordinal,
                    target_symbol,
                    receiver_symbol,
                )
            }

            pub fn state_call_prior_invalidations<'a>(
                &'a self,
                state: &'a FlowStateFact,
                call: &'a FlowCallFact,
            ) -> impl Iterator<Item = &'a FlowInvalidationFact> + 'a {
                self.view().state_call_prior_invalidations(state, call)
            }
        }

        impl<'f> FlowFactsRef<'f> {
            pub fn state_statement(
                self,
                state: &FlowStateFact,
                statement_index: usize,
            ) -> Option<&'f FlowStatementFact> {
                self.control
                    .statements
                    .span_or_empty(state.statements)
                    .iter()
                    .find(|statement| statement.statement_index == statement_index)
            }

            pub fn state_call(
                self,
                state: &FlowStateFact,
                statement_index: usize,
                call_ordinal: usize,
                target_symbol: SymbolHandle,
                receiver_symbol: SymbolHandle,
            ) -> Option<&'f FlowCallFact> {
                self.control
                    .calls
                    .span_or_empty(state.calls)
                    .iter()
                    .find(|call| {
                        call.statement_index == statement_index
                            && call.call_ordinal == call_ordinal
                            && call.target_symbol == target_symbol
                            && call.receiver_symbol == receiver_symbol
                    })
            }

            pub fn state_call_entry_constraints(
                self,
                state: &FlowStateFact,
                statement_index: usize,
                call_ordinal: usize,
                target_symbol: SymbolHandle,
                receiver_symbol: SymbolHandle,
            ) -> HandleSpan<FlowConstraintRef> {
                self.state_call(
                    state,
                    statement_index,
                    call_ordinal,
                    target_symbol,
                    receiver_symbol,
                )
                .map(|call| call.entry_constraints)
                .or_else(|| {
                    self.state_statement(state, statement_index)
                        .map(|statement| statement.entry_constraints)
                })
                .unwrap_or(state.entry_constraints)
            }

            pub fn state_call_entry_semantic_contexts(
                self,
                state: &FlowStateFact,
                statement_index: usize,
                call_ordinal: usize,
                target_symbol: SymbolHandle,
                receiver_symbol: SymbolHandle,
            ) -> impl Iterator<Item = facts::FactContextHandle> + use<'f> {
                self.semantic_constraint_contexts(self.state_call_entry_constraints(
                    state,
                    statement_index,
                    call_ordinal,
                    target_symbol,
                    receiver_symbol,
                ))
            }

            pub fn state_call_prior_invalidations<'a>(
                self,
                state: &'a FlowStateFact,
                call: &'a FlowCallFact,
            ) -> impl Iterator<Item = &'f FlowInvalidationFact> + 'a
            where
                'f: 'a,
            {
                self.invalidations
                    .events
                    .span_or_empty(state.invalidations)
                    .iter()
                    .filter(move |invalidation| match invalidation.source {
                        FlowInvalidationSource::Statement { statement_index } => {
                            statement_index < call.statement_index
                        }
                        FlowInvalidationSource::Call {
                            statement_index,
                            call_ordinal,
                            ..
                        } => {
                            statement_index < call.statement_index
                                || (statement_index == call.statement_index
                                    && call_ordinal < call.call_ordinal)
                        }
                    })
            }
        }
    }
}

pub use self::terminal::{
    CheckedAffineConstructionElementPlan, CheckedAtomicAccessPlan, CheckedAtomicEvent,
    CheckedAtomicReadModifyWrite, CheckedBoundaryExecutionPlan, CheckedBoundaryLifetimeDependency,
    CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan,
    CheckedBoundaryReferenceDestination, CheckedBoundaryReferenceOutput,
    CheckedBoundaryScalarDomainRequirementPlan, CheckedBoundaryScalarGuarantee,
    CheckedByteSequenceCarrier, CheckedByteSequenceValue, CheckedByteSequenceValuePlan,
    CheckedCallScalarArgument, CheckedCallingParameter, CheckedCallingSignaturePlan,
    CheckedCallingSignatureStorage, CheckedCleanupCaseCondition, CheckedCleanupCaseReachability,
    CheckedCleanupControlPlans, CheckedCleanupPartitionArm, CheckedCleanupPartitionLeaf,
    CheckedCleanupPartitionNode, CheckedCleanupPartitionPlan, CheckedCleanupPartitionRef,
    CheckedCleanupPartitionStep, CheckedClosedSumCaseOutcomePlan, CheckedClosedSumCasePlan,
    CheckedClosedSumPayloadTransferPlan, CheckedClosedSumSubjectKind,
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
    CheckedComposedUnitControlTerminatorPlan, CheckedConditionalReturnArm,
    CheckedControlResultPlan, CheckedDeclarationRequirementSet, CheckedDynamicClaimPath,
    CheckedDynamicSlotPlan, CheckedExtent, CheckedGuardedExitPlans, CheckedGuardedJumpArmPlan,
    CheckedGuardedJumpFallbackPlan, CheckedGuardedJumpPlan, CheckedGuardedJumpReturnPlan,
    CheckedGuardedReturnValue, CheckedInitializedLocal, CheckedIntegerRankObservation,
    CheckedLayoutObservation, CheckedMemberIdentity, CheckedNaturalRankFunction,
    CheckedNaturalRankMeasure, CheckedNominalAffineUnitCleanupMachinePlan,
    CheckedNominalAffineUnitCleanupPlans, CheckedNormalCleanupAction, CheckedNormalCompletion,
    CheckedNormalExitCleanupPlan, CheckedNormalExitCleanups, CheckedNormalResultArm,
    CheckedNormalResultCompletion, CheckedOpaqueRepresentation, CheckedOperandSequence,
    CheckedPartialAffineUnitCleanupMachinePlan, CheckedPartialAffineUnitCleanupPlans,
    CheckedPlaceLoanPlan, CheckedPlaceLoanRoot, CheckedRankEdgeCoordinate,
    CheckedReferenceOriginProjection, CheckedReferenceOriginRoot, CheckedReferenceOriginSourcePlan,
    CheckedReferenceOwnerCleanup, CheckedReferenceResultSourcePlan, CheckedReferenceStorage,
    CheckedReturnedReferencePlan, CheckedRuntimeIndex, CheckedScalarBranchDestination,
    CheckedScalarCaseFieldPlan, CheckedScalarGuardedExit, CheckedScalarGuardedTail,
    CheckedScalarReturnCopies, CheckedScalarReturnPlan, CheckedScalarStateTerminator,
    CheckedSelectedPayloadCopies, CheckedSelectedPayloadCopy, CheckedStateContractRequirement,
    CheckedStateNaturalRank, CheckedStructuralAccess, CheckedStructuralCallCustodyPlan,
    CheckedStructuralControlCleanupPlans, CheckedStructuralControlEdgeCleanupPlan,
    CheckedStructuralControlStateCleanupPlan, CheckedStructuralControlSuccessorPlan,
    CheckedStructuralControlTransferPlan, CheckedStructuralControlTransferSourcePlan,
    CheckedStructuralInvocation, CheckedStructuralLocalInitialization,
    CheckedStructuralPathQualification, CheckedStructuralResultClaimPlan,
    CheckedStructuralResultPlan, CheckedStructuralReturnCompletion,
    CheckedStructuralScalarArgumentPlan, CheckedStructuralScalarArgumentSourcePlan,
    CheckedStructuralScalarParameterPlan, CheckedStructuralValueCall, CheckedSuccessorCompletion,
    CheckedTerminalDebugPlans, CheckedTerminalMachineDebugPlan, CheckedTerminalMachineSelection,
    CheckedTerminalMachineSelections, CheckedTerminalSignatureEligibility,
    CheckedTerminalStateDebugPlan, CheckedTrivialAffineStructuralLocalPlan,
    CheckedUnitCallCoordinate, CheckedUnitCallParts, CheckedUnitCallPlan, CheckedUnitCallResult,
    CheckedUnitCallRoute, CheckedUnitCallTarget, CheckedUnitCallTransfers,
    CheckedUnitCallableEntry, CheckedUnitClaimTransferPlan, CheckedUnitCleanupBoundary,
    CheckedUnitCompletionExpression, CheckedUnitCompletionKind, CheckedUnitCompletionPlan,
    CheckedUnitCompletionSelection, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEffectPlans, CheckedUnitEntryClaimPlan, CheckedUnitLocalBindingPlan,
    CheckedUnitNominalAffineCallerRequirementPlan, CheckedUnitNominalAffineCleanupPlan,
    CheckedUnitNominalAffineCleanupRequirementPlan, CheckedUnitParameterStoragePlan,
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitPlan, CheckedUnitPlanGap,
    CheckedUnitPlanGapReason, CheckedUnitRecordStorage, CheckedUnitScalarCompletionArm,
    CheckedUnitScalarCompletionCondition, CheckedUnitScalarControlPlan,
    CheckedUnitScalarResultBindingPlan, CheckedUnitSelectedReferenceRetirementPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralCasePlan, CheckedUnitStructuralCompletionArm,
    CheckedUnitStructuralDomainPlan, CheckedUnitStructuralDomainRequirementPlan,
    CheckedUnitStructuralFieldPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralResultBindingPlan, CheckedUnitStructuralReturnPlan,
    CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape, CheckedValueCompletionPlan,
    ScalarReturnPath, checked_structural_type_by_identity, is_borrowed_view,
    is_bounded_nominal_cleanup_record, is_partial_affine_field_type,
};
pub use borrow_lifetimes::{
    FlowBorrowActivationFact, FlowBorrowWeakeningFact, FlowBorrowWeakeningReason,
};
pub use boundaries::FlowBoundaryEdgeFact;
pub use contexts::{FlowConstraintKind, FlowConstraintRef, FlowSemanticContextRef};
pub use control::{
    FlowCallDispatchRole, FlowCallFact, FlowCallResult, FlowCallSite, FlowExitFact,
    FlowExitParameterOrigin, FlowOperatorInvocationFact, FlowStateFact, FlowStateOutgoingFact,
    FlowStatementFact, RetiredFlowCall,
};
pub use descriptor_observations::FlowDescriptorObservationTable;
pub use invalidations::{FlowInvalidationFact, FlowInvalidationSource};
pub use operand::{
    FlowAtomicExecutionFact, FlowCapturedReferenceLeaf, FlowCaseAccessFact, FlowCopiedValue,
    FlowCopiedValueUse, FlowDescriptorContextObservation, FlowDescriptorPermission,
    FlowOperandAddressProjection, FlowOperandAlternative, FlowOperandChild, FlowOperandCompletion,
    FlowOperandFact, FlowOperandLifetimeContract, FlowOperandReborrowSource, FlowOperandReferent,
    FlowOperandRetainedLeaf, FlowOperandRetainedRegion, FlowOperandRetainedSupplier,
    FlowOperandRetention, FlowOperandStoredSource, FlowOutputLifetimeSources,
    FlowOwnedCaseSelection, FlowReferentFormation, FlowReferentFormationEvidence,
    FlowValueObservationFact, ViewReturnInputSource,
};
pub use ownership::{
    FlowBorrowClaimFact, FlowClaimJoinAlternative, FlowClaimJoinAlternativeSource,
    FlowClaimJoinCapture, FlowClaimJoinExit, FlowClaimJoinExitKind, FlowClaimJoinReceipt,
    FlowClaimOutcomeEntryFact, FlowClaimOutcomeMapFact, FlowClaimOutcomeSource,
    FlowInstructionDispositionFact, FlowInstructionEffectReceiptFact, FlowLoanCompletionFact,
    FlowOwnedSelectionClaim, FlowOwnedSelectionReceipt, FlowOwnedSelectionSource,
    FlowOwnedSelectionTransfer, FlowOwnedTransferPath, FlowOwnedTransferPlacement,
    FlowPermissionEvaluation, FlowPermissionEventFact, FlowSelectionPlaceArrival,
    FlowSelectionPlaceBinding, FlowStorageTransferFact, FlowStorageTransferSource,
};
pub use roots::{
    FlowBorrowLifetimeFacts, FlowBoundaryFacts, FlowContextFacts, FlowControlFacts, FlowFacts,
    FlowInvalidationFacts, FlowOwnershipFacts, WalkGuard,
};
pub use semantic_dependencies::{
    CheckedSemanticDependencies, CheckedSemanticDependency, CheckedSemanticDependencyExposure,
    CheckedSemanticDependencyKind,
};
pub use store_plans::{
    CheckedCaseLiteralValuePlan, CheckedIndexedWriteDecisions, CheckedPrimitiveReceivingRange,
    CheckedScalarStoreValue, CheckedStoreDestination, CheckedStoreLeaf, CheckedStorePlan,
    CheckedStoreRoot, CheckedStoreValue,
};
