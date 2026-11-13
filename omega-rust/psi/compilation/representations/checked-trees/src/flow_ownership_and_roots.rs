/*
We define `FlowFacts` root storage (`roots`) and ownership permission, claim
join, and computation-custody flow facts (`ownership`) in `CheckedTrees`.
*/

pub(crate) mod roots {

    type Arena<T> = arena::PagedArenaSet<T>;
    use symbols::SymbolHandle;

    use crate::keyed_rows::KeyedRows;

    use crate::flow::{
        FlowBorrowActivationFact, FlowBorrowWeakeningFact, FlowBoundaryEdgeFact, FlowCallFact,
        FlowClaimOutcomeEntryFact, FlowClaimOutcomeMapFact, FlowConstraintRef, FlowExitFact,
        FlowInvalidationFact, FlowPermissionEventFact, FlowSemanticContextRef, FlowStateFact,
        FlowStatementFact,
    };

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowContextFacts {
        pub semantic_context_refs: Arena<FlowSemanticContextRef>,
        pub constraint_refs: Arena<FlowConstraintRef>,
    }

    impl FlowContextFacts {
        pub fn with_roots(
            semantic_context_refs: Arena<FlowSemanticContextRef>,
            constraint_refs: Arena<FlowConstraintRef>,
        ) -> Self {
            Self {
                semantic_context_refs,
                constraint_refs,
            }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowInvalidationFacts {
        pub segments: Arena<facts::PlaceSegment>,
        pub events: Arena<FlowInvalidationFact>,
    }

    impl FlowInvalidationFacts {
        pub fn with_roots(
            segments: Arena<facts::PlaceSegment>,
            events: Arena<FlowInvalidationFact>,
        ) -> Self {
            Self { segments, events }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowBorrowLifetimeFacts {
        pub activations: Arena<FlowBorrowActivationFact>,
        pub weakenings: Arena<FlowBorrowWeakeningFact>,
    }

    impl FlowBorrowLifetimeFacts {
        pub fn with_roots(
            activations: Arena<FlowBorrowActivationFact>,
            weakenings: Arena<FlowBorrowWeakeningFact>,
        ) -> Self {
            Self {
                activations,
                weakenings,
            }
        }
    }

    #[derive(Debug, Clone, Default)]
    pub struct WalkGuard(Option<std::sync::Arc<WalkGuardState>>);

    #[derive(Debug)]
    struct WalkGuardState {
        states: Vec<SymbolHandle>,
        crossed: std::sync::atomic::AtomicBool,
    }

    impl WalkGuard {
        pub fn for_states(mut states: Vec<SymbolHandle>) -> Self {
            states.sort_unstable_by_key(|state| (state.arena_index(), state.generation()));
            Self(Some(std::sync::Arc::new(WalkGuardState {
                states,
                crossed: std::sync::atomic::AtomicBool::new(false),
            })))
        }

        pub fn note(&self, state: SymbolHandle) {
            let Some(guard) = &self.0 else {
                return;
            };
            let owned = guard
                .states
                .binary_search_by_key(&(state.arena_index(), state.generation()), |owned| {
                    (owned.arena_index(), owned.generation())
                })
                .is_ok();
            if !owned {
                guard
                    .crossed
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }

        pub fn crossed(&self) -> bool {
            self.0
                .as_ref()
                .is_some_and(|guard| guard.crossed.load(std::sync::atomic::Ordering::Relaxed))
        }
    }

    impl PartialEq for WalkGuard {
        fn eq(&self, _: &Self) -> bool {
            true
        }
    }

    impl Eq for WalkGuard {}

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowOwnershipFacts {
        pub instruction_effect_receipts: Arena<crate::flow::FlowInstructionEffectReceiptFact>,
        pub instruction_dispositions: Arena<crate::flow::FlowInstructionDispositionFact>,
        pub segments: Arena<facts::PlaceSegment>,
        pub storage_transfers: Arena<crate::flow::FlowStorageTransferFact>,
        pub permissions: Arena<FlowPermissionEventFact>,
        pub borrow_claims: Arena<crate::flow::FlowBorrowClaimFact>,
        pub selected_cleanup_permissions: Arena<FlowPermissionEventFact>,
        pub loan_completions: Arena<crate::flow::FlowLoanCompletionFact>,
        pub claim_outcome_entries: Arena<FlowClaimOutcomeEntryFact>,
        pub claim_outcome_maps: Arena<FlowClaimOutcomeMapFact>,
        pub claim_join_receipts: Arena<crate::flow::FlowClaimJoinReceipt>,
        pub claim_join_alternatives: Arena<crate::flow::FlowClaimJoinAlternative>,
        pub claim_join_exits: Arena<crate::flow::FlowClaimJoinExit>,
        pub claim_join_captures: Arena<crate::flow::FlowClaimJoinCapture>,
        pub owned_selections: Arena<crate::flow::FlowOwnedSelectionReceipt>,
        pub selection_sources: Arena<crate::flow::FlowOwnedSelectionSource>,
        pub selection_transfers: Arena<crate::flow::FlowOwnedSelectionTransfer>,
        pub selection_transfer_claims: Arena<crate::flow::FlowOwnedSelectionClaim>,
        pub selection_place_bindings: Arena<crate::flow::FlowSelectionPlaceBinding>,
        pub owned_transfer_paths: Arena<crate::flow::FlowOwnedTransferPath>,
        pub owned_transfer_path_segments: Arena<crate::flow::CheckedUnitStructuralPathSegment>,
        pub permissions_by_state: KeyedRows<SymbolHandle, FlowPermissionEventFact>,
        pub owned_selections_by_state:
            KeyedRows<SymbolHandle, crate::flow::FlowOwnedSelectionReceipt>,
        pub selection_place_bindings_by_state:
            KeyedRows<SymbolHandle, crate::flow::FlowSelectionPlaceBinding>,
        #[record(skip)]
        pub walk_guard: WalkGuard,
    }

    impl FlowOwnershipFacts {
        pub fn with_roots(
            segments: Arena<facts::PlaceSegment>,
            permissions: Arena<FlowPermissionEventFact>,
            claim_outcome_entries: Arena<FlowClaimOutcomeEntryFact>,
            claim_outcome_maps: Arena<FlowClaimOutcomeMapFact>,
        ) -> Self {
            Self {
                instruction_effect_receipts: Arena::default(),
                instruction_dispositions: Arena::default(),
                segments,
                permissions,
                storage_transfers: Arena::default(),
                borrow_claims: Arena::default(),
                selected_cleanup_permissions: Arena::default(),
                loan_completions: Arena::default(),
                claim_outcome_entries,
                claim_outcome_maps,
                claim_join_receipts: Arena::default(),
                claim_join_alternatives: Arena::default(),
                claim_join_exits: Arena::default(),
                claim_join_captures: Arena::default(),
                owned_selections: Arena::default(),
                selection_sources: Arena::default(),
                selection_transfers: Arena::default(),
                selection_transfer_claims: Arena::default(),
                selection_place_bindings: Arena::default(),
                owned_transfer_paths: Arena::default(),
                owned_transfer_path_segments: Arena::default(),
                permissions_by_state: KeyedRows::default(),
                owned_selections_by_state: KeyedRows::default(),
                selection_place_bindings_by_state: KeyedRows::default(),
                walk_guard: WalkGuard::default(),
            }
        }

        pub fn with_state_permissions<R>(
            &self,
            state: SymbolHandle,
            run: impl FnOnce(&[arena::Handle<FlowPermissionEventFact>]) -> R,
        ) -> R {
            self.walk_guard.note(state);
            self.permissions_by_state.with_rows(
                &self.permissions,
                |event| event.state_symbol,
                &state,
                run,
            )
        }

        pub fn with_state_owned_selections<R>(
            &self,
            state: SymbolHandle,
            run: impl FnOnce(&[arena::Handle<crate::flow::FlowOwnedSelectionReceipt>]) -> R,
        ) -> R {
            self.walk_guard.note(state);
            self.owned_selections_by_state.with_rows(
                &self.owned_selections,
                |receipt| receipt.state,
                &state,
                run,
            )
        }

        pub fn with_state_selection_place_bindings<R>(
            &self,
            state: SymbolHandle,
            run: impl FnOnce(&[arena::Handle<crate::flow::FlowSelectionPlaceBinding>]) -> R,
        ) -> R {
            self.walk_guard.note(state);
            self.selection_place_bindings_by_state.with_rows(
                &self.selection_place_bindings,
                |binding| binding.state,
                &state,
                run,
            )
        }

        pub fn seal_indexes(&mut self) {
            self.permissions_by_state
                .seal(&self.permissions, |event| event.state_symbol);
            self.owned_selections_by_state
                .seal(&self.owned_selections, |receipt| receipt.state);
            self.selection_place_bindings_by_state
                .seal(&self.selection_place_bindings, |binding| binding.state);
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowBoundaryFacts {
        pub edges: Arena<FlowBoundaryEdgeFact>,
    }

    impl FlowBoundaryFacts {
        pub fn with_roots(edges: Arena<FlowBoundaryEdgeFact>) -> Self {
            Self { edges }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowControlFacts {
        pub storage_snapshots: Arena<crate::flow::FlowStorageSnapshot>,
        pub storage_predecessors: Arena<arena::Handle<crate::flow::FlowStorageSnapshot>>,
        pub value_observations: Arena<crate::flow::FlowValueObservationFact>,
        pub descriptor_context_observations: crate::flow::FlowDescriptorObservationTable,
        pub atomic_executions: Arena<crate::flow::FlowAtomicExecutionFact>,
        pub case_accesses: Arena<crate::flow::FlowCaseAccessFact>,
        pub owned_case_selections: Arena<crate::FlowOwnedCaseSelection>,
        pub operator_invocations: Arena<crate::flow::FlowOperatorInvocationFact>,
        pub operands: Arena<crate::flow::FlowOperandFact>,
        pub copied_values: Arena<crate::flow::FlowCopiedValue>,
        pub copied_value_uses: Arena<crate::flow::FlowCopiedValueUse>,
        pub captured_reference_leaves: Arena<crate::flow::FlowCapturedReferenceLeaf>,
        /// Captured occurrences grouped by their actual evaluated expression.
        /// The shared index follows the operand arena's exact mutation revision.
        pub operands_by_expression:
            KeyedRows<typed_trees::expression::ExpressionHandle, crate::flow::FlowOperandFact>,
        pub operand_children: Arena<crate::flow::FlowOperandChild>,
        pub operand_alternatives: Arena<crate::flow::FlowOperandAlternative>,
        pub operand_child_paths: Arena<facts::PlaceSegment>,
        pub operand_origin_writes: Arena<crate::flow::FlowOperandReferent>,
        pub operand_referents: Arena<crate::flow::FlowOperandReferent>,
        pub referent_formations: Arena<crate::flow::FlowReferentFormation>,
        pub operand_referent_segments: Arena<facts::PlaceSegment>,
        pub operand_retentions: Arena<crate::flow::FlowOperandRetention>,
        pub operand_retained_roots: Arena<facts::PlaceRoot>,
        pub operand_retained_leaves: Arena<crate::flow::FlowOperandRetainedLeaf>,
        pub operand_retained_suppliers: Arena<crate::flow::FlowOperandRetainedSupplier>,
        pub operand_stored_sources: Arena<crate::flow::FlowOperandStoredSource>,
        pub operand_reborrow_sources: Arena<crate::flow::FlowOperandReborrowSource>,
        pub output_lifetime_sources: Arena<crate::flow::FlowOutputLifetimeSources>,
        pub operand_retained_regions: Arena<crate::flow::FlowOperandRetainedRegion>,
        pub operand_retained_paths: Arena<facts::PlaceSegment>,
        pub statements: Arena<FlowStatementFact>,
        pub calls: Arena<FlowCallFact>,
        pub call_arguments: Arena<typed_trees::expression::ExpressionHandle>,
        pub call_argument_positions: Arena<u32>,
        pub exits: Arena<FlowExitFact>,
        pub outgoing: Arena<crate::flow::FlowStateOutgoingFact>,
        pub exit_parameter_origins: Arena<crate::flow::FlowExitParameterOrigin>,
        pub states: crate::flow::FlowStateTable,
        pub retired_calls: Vec<crate::flow::RetiredFlowCall>,
    }

    impl FlowControlFacts {
        pub fn with_expression_operands<R>(
            &self,
            expression: typed_trees::expression::ExpressionHandle,
            run: impl FnOnce(&[arena::Handle<crate::flow::FlowOperandFact>]) -> R,
        ) -> R {
            self.operands_by_expression.with_rows(
                &self.operands,
                |operand| operand.expression,
                &expression,
                run,
            )
        }

        pub fn seal_indexes(&mut self) {
            self.operands_by_expression
                .seal(&self.operands, |operand| operand.expression);
        }

        pub fn retained_roots(
            &self,
            retention: arena::Handle<crate::flow::FlowOperandRetention>,
        ) -> Option<&[facts::PlaceRoot]> {
            if !self.operand_retentions.is_valid(retention) {
                return None;
            }
            let profile = self.operand_retentions.get(retention);
            if !profile.stored_projection.is_empty() {
                return None;
            }
            self.operand_retained_roots.span(profile.roots)
        }

        pub fn with_roots(
            statements: Arena<FlowStatementFact>,
            calls: Arena<FlowCallFact>,
            exits: Arena<FlowExitFact>,
            states: Arena<FlowStateFact>,
        ) -> Self {
            Self {
                storage_snapshots: Arena::default(),
                storage_predecessors: Arena::default(),
                statements,
                value_observations: Arena::default(),
                descriptor_context_observations:
                    crate::flow::FlowDescriptorObservationTable::default(),
                atomic_executions: Arena::default(),
                case_accesses: Arena::default(),
                owned_case_selections: Arena::default(),
                operator_invocations: Arena::default(),
                operands: Arena::default(),
                copied_values: Arena::default(),
                copied_value_uses: Arena::default(),
                captured_reference_leaves: Arena::default(),
                operands_by_expression: KeyedRows::default(),
                operand_children: Arena::default(),
                operand_alternatives: Arena::default(),
                operand_child_paths: Arena::default(),
                operand_origin_writes: Arena::default(),
                operand_referents: Arena::default(),
                referent_formations: Arena::default(),
                operand_referent_segments: Arena::default(),
                operand_retentions: Arena::default(),
                operand_retained_roots: Arena::default(),
                operand_retained_leaves: Arena::default(),
                operand_retained_suppliers: Arena::default(),
                operand_stored_sources: Arena::default(),
                operand_reborrow_sources: Arena::default(),
                output_lifetime_sources: Arena::default(),
                operand_retained_regions: Arena::default(),
                operand_retained_paths: Arena::default(),
                calls,
                call_arguments: Arena::default(),
                call_argument_positions: Arena::default(),
                exits,
                outgoing: Arena::default(),
                exit_parameter_origins: Arena::default(),
                states: states.into(),
                retired_calls: Vec::new(),
            }
        }

        pub fn call_argument_evaluation_order(&self, call: &FlowCallFact) -> Vec<usize> {
            if call.argument_evaluation_order.is_empty() {
                return (0..call.authored_arguments.len()).collect();
            }
            self.call_argument_positions
                .span_or_empty(call.argument_evaluation_order)
                .iter()
                .map(|position| *position as usize)
                .collect()
        }

        pub fn call_at(
            &self,
            state: SymbolHandle,
            statement_index: usize,
            call_ordinal: usize,
        ) -> Option<&FlowCallFact> {
            let state = self.state_by_symbol(state)?;
            self.calls.span_or_empty(state.calls).iter().find(|call| {
                call.statement_index == statement_index && call.call_ordinal == call_ordinal
            })
        }

        pub fn call_position(
            &self,
            state: SymbolHandle,
            statement_index: usize,
            call_ordinal: usize,
        ) -> Option<usize> {
            let state = self.state_by_symbol(state)?;
            self.calls
                .span_or_empty(state.calls)
                .iter()
                .position(|call| {
                    call.statement_index == statement_index && call.call_ordinal == call_ordinal
                })
        }

        pub fn is_retired(&self, state: SymbolHandle, call: &FlowCallFact) -> bool {
            self.retired_calls.iter().any(|retired| {
                retired.state_symbol == state
                    && retired.statement_index == call.statement_index
                    && retired.call_ordinal == call.call_ordinal
            })
        }

        pub fn retire_calls(
            &mut self,
            machine: SymbolHandle,
            state: SymbolHandle,
            statement_index: usize,
            expression: typed_trees::expression::ExpressionHandle,
            statement_root: bool,
        ) -> usize {
            let Some(span) = self.state(machine, state).map(|candidate| candidate.calls) else {
                return 0;
            };
            let mut retired = 0;
            for call in self.calls.span_or_empty(span) {
                if call.statement_index != statement_index {
                    continue;
                }
                let matches = if call.authored_expression.is_valid() {
                    call.authored_expression == expression
                } else {
                    statement_root && call.call_ordinal == 0
                };
                if !matches || self.is_retired(state, call) {
                    continue;
                }
                self.retired_calls.push(crate::flow::RetiredFlowCall {
                    state_symbol: state,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                });
                retired += 1;
            }
            retired
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowFacts {
        pub contexts: FlowContextFacts,
        pub invalidations: FlowInvalidationFacts,
        pub borrow_lifetimes: FlowBorrowLifetimeFacts,
        pub ownership: FlowOwnershipFacts,
        pub boundaries: FlowBoundaryFacts,
        pub control: FlowControlFacts,
        pub semantic_dependencies: crate::flow::CheckedSemanticDependencies,
        pub guarded_exits: crate::flow::CheckedGuardedExitPlans,
        pub terminal_machines: crate::flow::CheckedTerminalMachineSelections,
        pub terminal_debug: crate::flow::CheckedTerminalDebugPlans,
        pub terminal_unit_effects: crate::flow::CheckedUnitEffectPlans,
        pub terminal_partial_affine_unit_cleanups:
            crate::flow::CheckedPartialAffineUnitCleanupPlans,
        pub terminal_nominal_affine_unit_cleanups:
            crate::flow::CheckedNominalAffineUnitCleanupPlans,
        pub terminal_structural_control_cleanups: crate::flow::CheckedStructuralControlCleanupPlans,
    }

    impl FlowFacts {
        pub fn with_roots(
            contexts: FlowContextFacts,
            invalidations: FlowInvalidationFacts,
            borrow_lifetimes: FlowBorrowLifetimeFacts,
            ownership: FlowOwnershipFacts,
            boundaries: FlowBoundaryFacts,
            control: FlowControlFacts,
        ) -> Self {
            Self {
                contexts,
                invalidations,
                borrow_lifetimes,
                ownership,
                boundaries,
                control,
                semantic_dependencies: crate::flow::CheckedSemanticDependencies::default(),
                guarded_exits: crate::flow::CheckedGuardedExitPlans::default(),
                terminal_machines: crate::flow::CheckedTerminalMachineSelections::default(),
                terminal_debug: crate::flow::CheckedTerminalDebugPlans::default(),
                terminal_unit_effects: crate::flow::CheckedUnitEffectPlans::default(),
                terminal_partial_affine_unit_cleanups:
                    crate::flow::CheckedPartialAffineUnitCleanupPlans::default(),
                terminal_nominal_affine_unit_cleanups:
                    crate::flow::CheckedNominalAffineUnitCleanupPlans::default(),
                terminal_structural_control_cleanups:
                    crate::flow::CheckedStructuralControlCleanupPlans::default(),
            }
        }
    }

    macro_rules! paged_flow_roots {
    (
        $name:ident {
            arenas: [$($arena:ident),* $(,)?],
            indexes: [$($index:ident),* $(,)?],
            lists: [$($list:ident),* $(,)?] $(,)?
        }
    ) => {
        impl $name {
            pub fn share_pages(&mut self) {
                $(self.$arena.share_pages();)*
            }

            pub fn reset_pages(&mut self) {
                $(self.$arena.reset_retain_capacity();)*
                $(self.$list.clear();)*
            }

            pub fn begin_page(&mut self, generation: u32) {
                let Self {
                    $($arena,)*
                    $($index: _,)*
                    $($list: _,)*
                } = self;
                $($arena.begin_page(generation);)*
            }

            pub fn take_additional_pages(&mut self) -> Self {
                let mut taken = Self::default();
                $(taken.$arena.adopt_additional_pages(self.$arena.take_additional_pages());)*
                $(taken.$list = std::mem::take(&mut self.$list);)*
                taken
            }

            pub fn adopt_additional_pages(&mut self, mut other: Self) {
                $(self.$arena.adopt_additional_pages(other.$arena.take_additional_pages());)*
                $(self.$list.append(&mut other.$list);)*
            }

            pub fn latest_page_generation(&self) -> u32 {
                let mut latest = 1;
                $(latest = latest.max(self.$arena.latest_generation());)*
                latest
            }

            pub fn take_pages_from(&mut self, generation: u32, baseline: &Self) -> Self {
                let mut taken = Self::default();
                $(taken.$arena.adopt_additional_pages(
                    self.$arena
                        .take_pages_from(generation, baseline.$arena.current_generation()),
                );)*
                $(taken.$list = std::mem::take(&mut self.$list);)*
                taken
            }
        }
    };
}

    paged_flow_roots!(FlowContextFacts {
        arenas: [semantic_context_refs, constraint_refs],
        indexes: [],
        lists: [],
    });
    paged_flow_roots!(FlowInvalidationFacts {
        arenas: [segments, events],
        indexes: [],
        lists: [],
    });
    paged_flow_roots!(FlowBorrowLifetimeFacts {
        arenas: [activations, weakenings],
        indexes: [],
        lists: [],
    });
    paged_flow_roots!(FlowOwnershipFacts {
        arenas: [
            instruction_effect_receipts,
            instruction_dispositions,
            segments,
            storage_transfers,
            permissions,
            borrow_claims,
            selected_cleanup_permissions,
            loan_completions,
            claim_outcome_entries,
            claim_outcome_maps,
            claim_join_receipts,
            claim_join_alternatives,
            claim_join_exits,
            claim_join_captures,
            owned_selections,
            selection_sources,
            selection_transfers,
            selection_transfer_claims,
            selection_place_bindings,
            owned_transfer_paths,
            owned_transfer_path_segments,
        ],
        indexes: [
            permissions_by_state,
            owned_selections_by_state,
            selection_place_bindings_by_state,
            walk_guard,
        ],
        lists: [],
    });
    paged_flow_roots!(FlowBoundaryFacts {
        arenas: [edges],
        indexes: [],
        lists: [],
    });
    paged_flow_roots!(FlowControlFacts {
        arenas: [
            storage_snapshots,
            storage_predecessors,
            value_observations,
            descriptor_context_observations,
            atomic_executions,
            case_accesses,
            owned_case_selections,
            operator_invocations,
            operands,
            copied_values,
            copied_value_uses,
            captured_reference_leaves,
            operand_children,
            operand_alternatives,
            operand_child_paths,
            operand_origin_writes,
            operand_referents,
            referent_formations,
            operand_referent_segments,
            operand_retentions,
            operand_retained_roots,
            operand_retained_leaves,
            operand_retained_suppliers,
            operand_stored_sources,
            operand_reborrow_sources,
            output_lifetime_sources,
            operand_retained_regions,
            operand_retained_paths,
            statements,
            calls,
            call_arguments,
            call_argument_positions,
            exits,
            outgoing,
            exit_parameter_origins,
            states,
        ],
        indexes: [operands_by_expression],
        lists: [retired_calls],
    });
}

pub(crate) mod ownership {
    use crate::flow::FlowPermissionEvaluation::Statement;
    use arena::HandleSpan;
    use symbols::SymbolHandle;

    pub(crate) mod computation_custody {

        use crate::CheckedUnitLocalBindingPlan::ByteSequence;
        use crate::CheckedUnitLocalBindingPlan::StructuralValue;
        use crate::{
            CheckedByteSequenceValue, CheckedByteSequenceValuePlan, CheckedCallScalarArgument,
            CheckedConditionalReturnArm, CheckedElementViewRegion, CheckedGuardedExitPlans,
            CheckedScalarBranchDestination, CheckedScalarCaseConstruction,
            CheckedScalarComputationHandle, CheckedScalarComputationKind,
            CheckedScalarComputationStructuralArgument, CheckedScalarDispatchPattern,
            CheckedScalarExpressionRole, CheckedScalarReturnPlan, CheckedScalarStateTerminator,
            CheckedScalarStoreValue, CheckedStoreValue, CheckedStructuralAccess,
            CheckedStructuralRecordFieldValue, CheckedStructuralValueCall,
            CheckedStructuralValueHandle, CheckedStructuralValueKind,
            CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentPlan,
            CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralResultBindingPlan,
            CheckedValueFacts,
        };
        use symbols::SymbolHandle;

        type CallSchedules<'reader, 'calls> = dyn Fn(CheckedStructuralValueHandle) -> Option<&'calls [CheckedStructuralValueCall]>
            + 'reader;

        impl CheckedUnitStructuralArgumentSourcePlan {
            pub fn current_result_binding<'plans>(
                &self,
                operations: impl IntoIterator<
                    Item = &'plans CheckedUnitEffectOperationPlan,
                    IntoIter: DoubleEndedIterator,
                >,
            ) -> Option<u32> {
                let mut source = self.clone();
                let mut prefix = operations.into_iter();
                loop {
                    let symbol = match source {
                        Self::StructuralResult { binding_ordinal } => return Some(binding_ordinal),
                        Self::StructuralLocal { symbol }
                        | Self::StructuralLocalBorrow { symbol, .. } => symbol,
                        _ => return None,
                    };
                    let mut previous = None;
                    while let Some(operation) = prefix.next_back() {
                        if operation.ends_local_occurrence(symbol) {
                            let CheckedUnitEffectOperationPlan::Store(store) = operation else {
                                return None;
                            };
                            let value = store.installed_value()?;
                            if !store.destination.path.is_empty() || !value.path.is_empty() {
                                return None;
                            }
                            previous = Some(value.source.clone());
                            break;
                        }
                        if let Some(result) = operation.statement_structural_result_binding()
                            && result.local.symbol == symbol
                        {
                            return Some(result.binding_ordinal);
                        }
                    }
                    source = previous?;
                }
            }
        }

        impl CheckedUnitEffectOperationPlan {
            pub fn ends_local_occurrence(&self, local: SymbolHandle) -> bool {
                local.is_valid()
                    && matches!(self, Self::Store(store)
                if matches!(store.destination.leaf, crate::CheckedStoreLeaf::OwnedRebind { .. })
                    && store.destination.root == (crate::CheckedStoreRoot::Local { symbol: local }))
            }

            pub fn definite_call_transfers<'calls>(
                &self,
                values: &CheckedValueFacts,
                call_schedules: &CallSchedules<'_, 'calls>,
                producer: &CheckedUnitEffectOperationPlan,
                local: SymbolHandle,
            ) -> usize {
                CallTransfers::for_producer(values, call_schedules, producer, local)
                    .map_or(0, |transfers| transfers.operation(self))
            }
        }

        impl CheckedCallScalarArgument {
            pub fn definite_call_transfers<'calls>(
                &self,
                values: &CheckedValueFacts,
                call_schedules: &CallSchedules<'_, 'calls>,
                producer: &CheckedUnitEffectOperationPlan,
                local: SymbolHandle,
            ) -> usize {
                CallTransfers::for_producer(values, call_schedules, producer, local)
                    .map_or(0, |transfers| transfers.scalar(self))
            }
        }

        #[derive(Clone)]
        struct CallTransfers<'facts, 'calls> {
            values: &'facts CheckedValueFacts,
            call_schedules: &'facts CallSchedules<'facts, 'calls>,
            result: &'facts CheckedUnitStructuralResultBindingPlan,
            local: SymbolHandle,
            expression: typed_trees::expression::ExpressionHandle,
            established_values: Vec<CheckedStructuralValueHandle>,
            completed_computations: Vec<CheckedScalarComputationHandle>,
        }

        impl CheckedScalarReturnPlan {
            pub fn selected_exit_call_transfers<'calls>(
                &self,
                values: &CheckedValueFacts,
                call_schedules: &CallSchedules<'_, 'calls>,
                exits: &CheckedGuardedExitPlans,
                state: SymbolHandle,
                producer: &CheckedUnitEffectOperationPlan,
                local: SymbolHandle,
            ) -> Option<Vec<(CheckedScalarBranchDestination, usize)>> {
                let Self::Exits(completion) = self else {
                    return None;
                };
                CallTransfers::for_producer(values, call_schedules, producer, local)?
                    .scalar_exit_paths(state, exits, &completion.terminator)
            }

            pub fn definite_call_transfers<'calls>(
                &self,
                values: &CheckedValueFacts,
                call_schedules: &CallSchedules<'_, 'calls>,
                exits: &CheckedGuardedExitPlans,
                state: SymbolHandle,
                producer: &CheckedUnitEffectOperationPlan,
                local: SymbolHandle,
            ) -> usize {
                let Self::Exits(completion) = self else {
                    return 0;
                };
                let Some(transfers) =
                    CallTransfers::for_producer(values, call_schedules, producer, local)
                else {
                    return 0;
                };
                transfers.scalar_exits(state, exits, &completion.terminator)
            }
        }

        impl CheckedConditionalReturnArm {
            pub fn definite_call_transfers<'calls>(
                &self,
                values: &CheckedValueFacts,
                call_schedules: &CallSchedules<'_, 'calls>,
                state: SymbolHandle,
                producer: &CheckedUnitEffectOperationPlan,
                local: SymbolHandle,
            ) -> usize {
                let Some(transfers) =
                    CallTransfers::for_producer(values, call_schedules, producer, local)
                else {
                    return 0;
                };
                match self {
                    Self::Scalar {
                        statement_ordinal, ..
                    } => transfers.scalar_at(
                        state,
                        *statement_ordinal,
                        CheckedScalarExpressionRole::Return,
                    ),
                    Self::Structural(operation) => transfers.operation(operation),
                    Self::Completion(plan) => transfers
                        .completion(&plan.expression)
                        .flatten()
                        .unwrap_or(0),
                    Self::EvaluateNoNormal { operations, .. }
                    | Self::EvaluateUnit { operations, .. } => operations
                        .iter()
                        .map(|operation| transfers.operation(operation))
                        .sum(),
                    Self::Call {
                        operands,
                        operation,
                        completions,
                    } => {
                        operands
                            .iter()
                            .map(|operand| transfers.operation(operand))
                            .sum::<usize>()
                            + transfers.operation(operation)
                            + completions
                                .iter()
                                .map(|completion| transfers.operation(completion))
                                .sum::<usize>()
                    }
                    Self::Crash { .. } | Self::Unit { .. } => 0,
                }
            }
        }

        impl<'facts, 'calls> CallTransfers<'facts, 'calls> {
            fn scalar_at(
                &self,
                state: SymbolHandle,
                statement: u32,
                role: CheckedScalarExpressionRole,
            ) -> usize {
                self.values
                    .scalar_computations
                    .root_at(state, statement, role)
                    .map_or(0, |root| self.computation(root.root))
            }

            fn scalar_destination(
                &self,
                state: SymbolHandle,
                destination: &CheckedScalarBranchDestination,
            ) -> Option<usize> {
                match destination {
                    CheckedScalarBranchDestination::Return {
                        statement_ordinal,
                        is_continuation,
                    } => Some(self.scalar_at(
                        state,
                        *statement_ordinal,
                        if *is_continuation {
                            CheckedScalarExpressionRole::ContinuationReturn
                        } else {
                            CheckedScalarExpressionRole::Return
                        },
                    )),
                    CheckedScalarBranchDestination::Jump(_) => Some(0),
                    CheckedScalarBranchDestination::Crash { .. }
                    | CheckedScalarBranchDestination::InvokeNoNormal { .. } => None,
                }
            }

            fn scalar_exits(
                &self,
                state: SymbolHandle,
                exits: &CheckedGuardedExitPlans,
                terminator: &CheckedScalarStateTerminator,
            ) -> usize {
                self.scalar_exit_paths(state, exits, terminator)
                    .and_then(|paths| paths.into_iter().map(|(_, count)| count).min())
                    .unwrap_or(0)
            }

            fn scalar_exit_paths(
                &self,
                state: SymbolHandle,
                exits: &CheckedGuardedExitPlans,
                terminator: &CheckedScalarStateTerminator,
            ) -> Option<Vec<(CheckedScalarBranchDestination, usize)>> {
                let mut paths = Vec::new();
                let mut append = |destination: &CheckedScalarBranchDestination, prefix: usize| {
                    if let Some(transfers) = self.scalar_destination(state, destination) {
                        paths.push((destination.clone(), prefix + transfers));
                    }
                };
                match terminator {
                    CheckedScalarStateTerminator::Return { statement_ordinal } => append(
                        &CheckedScalarBranchDestination::Return {
                            statement_ordinal: *statement_ordinal,
                            is_continuation: false,
                        },
                        0,
                    ),
                    CheckedScalarStateTerminator::Conditional {
                        guard_statement_ordinal,
                        when_true,
                        when_false,
                    } => {
                        let prefix = self.scalar_at(
                            state,
                            *guard_statement_ordinal,
                            CheckedScalarExpressionRole::Guard,
                        );
                        append(when_true, prefix);
                        append(when_false, prefix);
                    }
                    CheckedScalarStateTerminator::Guarded { arms, fallback } => {
                        let mut prefix = 0;
                        for arm in exits.exits.span(*arms)? {
                            prefix += self.scalar_at(
                                state,
                                arm.guard_statement_ordinal,
                                CheckedScalarExpressionRole::Guard,
                            );
                            append(&arm.destination, prefix);
                        }
                        if let Some(fallback) = fallback {
                            append(fallback, prefix);
                        }
                    }
                    CheckedScalarStateTerminator::Jump(edge) => {
                        append(&CheckedScalarBranchDestination::Jump(edge.clone()), 0);
                    }
                    CheckedScalarStateTerminator::Crash { .. } => {}
                }
                Some(paths)
            }

            fn for_producer(
                values: &'facts CheckedValueFacts,
                call_schedules: &'facts CallSchedules<'facts, 'calls>,
                producer: &'facts CheckedUnitEffectOperationPlan,
                local: SymbolHandle,
            ) -> Option<Self> {
                let result = producer.structural_result_binding()?;
                let expression = match producer {
                    CheckedUnitEffectOperationPlan::EstablishLocal(StructuralValue {
                        value,
                        ..
                    }) => values.structural_values.nodes.get(*value).expression,
                    CheckedUnitEffectOperationPlan::Call(call) => match call.source_site() {
                        Some(crate::NominalMachineUseSite::Expression(expression)) => expression,
                        _ => arena::Handle::invalid(),
                    },
                    _ => arena::Handle::invalid(),
                };
                Some(Self {
                    values,
                    call_schedules,
                    result,
                    local,
                    expression,
                    established_values: Vec::new(),
                    completed_computations: Vec::new(),
                })
            }
            fn argument(&self, argument: &CheckedUnitStructuralArgumentPlan) -> usize {
                if !self.source_is_retained(argument) {
                    return 0;
                }
                self.argument_transfer(argument)
                    + self.argument_evaluation(argument)
                    + self.produced(argument)
            }

            fn source_is_retained(&self, argument: &CheckedUnitStructuralArgumentPlan) -> bool {
                match argument.source {
                    CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                        expression,
                        value_dependency: Some(value),
                        ..
                    } => self
                        .values
                        .structural_values
                        .root_for_value_home(value, expression)
                        .is_some(),
                    _ => true,
                }
            }

            fn argument_transfer(&self, argument: &CheckedUnitStructuralArgumentPlan) -> usize {
                let whole_owner = argument.access == CheckedStructuralAccess::Owned
                    && argument.path.is_empty()
                    && argument.type_identity == self.result.type_identity
                    && match argument.source {
                        CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal,
                        } => binding_ordinal == self.result.binding_ordinal,
                        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                            self.local.is_valid() && symbol == self.local
                        }
                        CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                            expression,
                            ..
                        } => self.expression.is_valid() && expression == self.expression,
                        _ => false,
                    };
                usize::from(whole_owner)
            }

            fn produced(&self, argument: &CheckedUnitStructuralArgumentPlan) -> usize {
                let CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                    expression,
                    value_dependency: Some(value),
                    ..
                } = argument.source
                else {
                    return 0;
                };
                let plans = &self.values.structural_values;
                let Some(root) = plans.root_for_value_home(value, expression) else {
                    return 0;
                };
                if matches!(
                    plans.nodes.get(root.root).kind,
                    CheckedStructuralValueKind::Place(_)
                ) {
                    return 0;
                }
                self.constructed(
                    root.root,
                    (self.call_schedules)(root.root).unwrap_or(&[]),
                    argument.access == CheckedStructuralAccess::Owned && argument.path.is_empty(),
                )
            }

            fn constructed(
                &self,
                root: CheckedStructuralValueHandle,
                calls: &[CheckedStructuralValueCall],
                consumes_whole: bool,
            ) -> usize {
                if self.established_values.contains(&root) {
                    return 0;
                }
                let plans = &self.values.structural_values;
                match &plans.nodes.get(root).kind {
                    CheckedStructuralValueKind::QuotientCall { arguments, .. } => arguments
                        .iter()
                        .map(|argument| self.structural(*argument, calls))
                        .sum(),
                    CheckedStructuralValueKind::Place(source) => {
                        if !self.source_is_retained(source) {
                            return 0;
                        }
                        // A Place is an existing value, not another construction. Its
                        // expression may identify this very root; following production
                        // again turns that identity edge into infinite execution.
                        usize::from(consumes_whole) * self.argument_transfer(source)
                            + self.argument_evaluation(source)
                    }
                    crate::record_or_structural_case_pattern!(fields, ..) => plans
                        .record_fields
                        .span_or_empty(*fields)
                        .iter()
                        .map(|field| match field.value {
                            CheckedStructuralRecordFieldValue::Scalar(value) => {
                                self.computation(value)
                            }
                            CheckedStructuralRecordFieldValue::Structural(value) => {
                                self.constructed(value, calls, true)
                            }
                        })
                        .sum(),
                    CheckedStructuralValueKind::FixedArray { elements } => elements
                        .iter()
                        .map(|element| self.constructed(*element, calls, true))
                        .sum(),
                    CheckedStructuralValueKind::Projection {
                        source,
                        path,
                        projection_evaluations,
                        copies_selected_value,
                        ..
                    } => {
                        self.constructed(
                            *source,
                            calls,
                            consumes_whole && path.is_empty() && !copies_selected_value,
                        ) + CheckedUnitStructuralArgumentPlan::projection_computation_operands(
                            path,
                            *projection_evaluations,
                            &self.values.scalar_computations.operands,
                        )
                        .map(|operand| self.computation(operand))
                        .sum::<usize>()
                    }
                    CheckedStructuralValueKind::Dispatch { subject, arms } => {
                        let mut prefix = 0;
                        let alternatives =
                            plans.dispatch_arms.span_or_empty(*arms).iter().map(|arm| {
                                prefix += self.pattern(&arm.pattern);
                                prefix + self.constructed(arm.value, calls, consumes_whole)
                            });
                        self.dispatch_subject(subject) + alternatives.min().unwrap_or(0)
                    }
                    CheckedStructuralValueKind::Reference {
                        source,
                        source_value,
                        ..
                    } => {
                        self.argument_evaluation(source)
                            + if source_value.is_valid() {
                                self.constructed(*source_value, calls, true)
                            } else {
                                0
                            }
                    }
                    _ => self.structural(root, calls),
                }
            }

            fn argument_evaluation(&self, argument: &CheckedUnitStructuralArgumentPlan) -> usize {
                argument
                    .computation_operands(&self.values.scalar_computations.operands)
                    .map(|operand| self.computation(operand))
                    .sum()
            }

            fn scalar(&self, value: &CheckedCallScalarArgument) -> usize {
                match value {
                    CheckedCallScalarArgument::Pure(_) => 0,
                    CheckedCallScalarArgument::Computation(root) => self.computation(*root),
                }
            }

            fn store(&self, value: &CheckedScalarStoreValue) -> usize {
                match value {
                    CheckedScalarStoreValue::Computation(root) => self.computation(*root),
                    CheckedScalarStoreValue::Pure(_)
                    | CheckedScalarStoreValue::ScalarResult { .. } => 0,
                }
            }

            fn case(&self, case: &CheckedScalarCaseConstruction) -> usize {
                self.values
                    .scalar_computations
                    .case_fields
                    .span_or_empty(case.fields)
                    .iter()
                    .map(|field| self.computation(field.value))
                    .sum()
            }

            fn pattern(&self, pattern: &CheckedScalarDispatchPattern) -> usize {
                match pattern {
                    CheckedScalarDispatchPattern::Value(value)
                    | CheckedScalarDispatchPattern::Predicate(value) => self.computation(*value),
                    CheckedScalarDispatchPattern::Wildcard => 0,
                }
            }

            fn dispatch_subject(&self, subject: &crate::CheckedDispatchSubject) -> usize {
                match subject {
                    crate::CheckedDispatchSubject::Scalar(value) => self.computation(*value),
                    crate::CheckedDispatchSubject::Structural { value, place, .. } => {
                        if value.is_valid() {
                            let calls = self
                                .values
                                .structural_values
                                .call_schedule(*value)
                                .map(|(_, calls)| calls)
                                .unwrap_or(&[]);
                            self.constructed(*value, calls, false)
                        } else {
                            self.argument_evaluation(place)
                        }
                    }
                }
            }

            fn completion(
                &self,
                node: &crate::CheckedUnitCompletionExpression,
            ) -> Option<Option<usize>> {
                if !node.normal_continuation {
                    return Some(None);
                }
                if let crate::CheckedUnitCompletionKind::Invocation(operations) = &node.kind {
                    return Some(Some(
                        operations
                            .iter()
                            .map(|operation| self.operation(operation))
                            .sum(),
                    ));
                }
                let subject = node.subject()?;
                let mut scope = self.clone();
                let subject_count = scope.dispatch_subject(&subject);
                match subject {
                    crate::CheckedDispatchSubject::Scalar(value) => {
                        scope.completed_computations.push(value)
                    }
                    crate::CheckedDispatchSubject::Structural { value, .. } if value.is_valid() => {
                        scope.established_values.push(value)
                    }
                    crate::CheckedDispatchSubject::Structural { .. } => {}
                }
                let mut prefix = 0;
                let mut minimum = None;
                for (pattern, child) in node.arms() {
                    if let Some(pattern) = pattern {
                        prefix += scope.computation(pattern);
                        scope.completed_computations.push(pattern);
                    }
                    if let Some(child) = scope.completion(child)? {
                        let count = prefix + child;
                        minimum =
                            Some(minimum.map_or(count, |previous: usize| previous.min(count)));
                    }
                }
                Some(Some(subject_count + minimum?))
            }

            fn computation(&self, root: CheckedScalarComputationHandle) -> usize {
                if self.completed_computations.contains(&root) {
                    return 0;
                }
                let plans = &self.values.scalar_computations;
                match &plans.nodes.get(root).kind {
                    CheckedScalarComputationKind::Call {
                        callee,
                        arguments,
                        structural_arguments,
                        ..
                    } => {
                        callee
                            .evaluated_callee()
                            .map_or(0, |selector| self.computation(selector))
                            + plans
                                .operands
                                .span_or_empty(*arguments)
                                .iter()
                                .map(|argument| self.computation(*argument))
                                .sum::<usize>()
                            + plans
                                .structural_arguments
                                .span_or_empty(*structural_arguments)
                                .iter()
                                .map(|argument| match argument {
                                    CheckedScalarComputationStructuralArgument::Place(argument) => {
                                        self.argument(argument)
                                    }
                                    CheckedScalarComputationStructuralArgument::Case(case) => {
                                        self.case(case)
                                    }
                                })
                                .sum::<usize>()
                    }
                    CheckedScalarComputationKind::WithOperands {
                        operands, operand, ..
                    } => {
                        let mut scope = self.clone();
                        let mut count = 0;
                        for dependency in plans.evaluation_operands.span_or_empty(*operands) {
                            match dependency {
                                crate::CheckedScalarEvaluationOperand::Scalar(value) => {
                                    count += scope.computation(*value);
                                    scope.completed_computations.push(*value);
                                }
                                crate::CheckedScalarEvaluationOperand::Structural(value) => {
                                    count += scope.structural(
                                        *value,
                                        (scope.call_schedules)(*value).unwrap_or(&[]),
                                    );
                                    scope.established_values.push(*value);
                                }
                            }
                        }
                        count + scope.computation(*operand)
                    }
                    CheckedScalarComputationKind::Apply { operands, .. }
                    | CheckedScalarComputationKind::StorageObservation { operands, .. } => plans
                        .operands
                        .span_or_empty(*operands)
                        .iter()
                        .map(|operand| self.computation(*operand))
                        .sum(),
                    CheckedScalarComputationKind::Select {
                        condition,
                        when_true,
                        when_false,
                        ..
                    } => {
                        self.computation(*condition)
                            + self
                                .computation(*when_true)
                                .min(self.computation(*when_false))
                    }
                    CheckedScalarComputationKind::Dispatch { subject, arms, .. } => {
                        let mut prefix = 0;
                        let alternatives =
                            plans.dispatch_arms.span_or_empty(*arms).iter().map(|arm| {
                                prefix += self.pattern(&arm.pattern);
                                prefix + self.computation(arm.value)
                            });
                        self.dispatch_subject(subject) + alternatives.min().unwrap_or(0)
                    }
                    CheckedScalarComputationKind::SelectedComparison { left, right, .. } => {
                        self.computation(*left) + self.computation(*right)
                    }
                    crate::cast_or_qualification_triple_pattern!(operand, ..) => {
                        self.computation(*operand)
                    }
                    CheckedScalarComputationKind::CaseMembership {
                        subject,
                        receiver_value,
                        ..
                    } => {
                        let observation = match subject {
                            CheckedScalarComputationStructuralArgument::Place(argument) => {
                                self.argument_evaluation(argument)
                            }
                            CheckedScalarComputationStructuralArgument::Case(case) => {
                                self.case(case)
                            }
                        };
                        observation
                            + if receiver_value.is_valid() {
                                self.structural(
                                    *receiver_value,
                                    (self.call_schedules)(*receiver_value).unwrap_or(&[]),
                                )
                            } else {
                                0
                            }
                    }
                    CheckedScalarComputationKind::StructuralField {
                        subject,
                        receiver_value,
                        ..
                    } => {
                        self.argument_evaluation(subject)
                            + if receiver_value.is_valid() {
                                self.structural(
                                    *receiver_value,
                                    (self.call_schedules)(*receiver_value).unwrap_or(&[]),
                                )
                            } else {
                                0
                            }
                    }
                    CheckedScalarComputationKind::StorageAddress { subject, .. }
                    | CheckedScalarComputationKind::ByteSequenceLiteralComparison {
                        subject, ..
                    }
                    | CheckedScalarComputationKind::ByteSequencePredicate { subject, .. } => {
                        self.argument_evaluation(subject)
                    }
                    CheckedScalarComputationKind::MachineValue { .. }
                    | CheckedScalarComputationKind::RetainedConstant { .. }
                    | CheckedScalarComputationKind::Value(_) => 0,
                }
            }

            fn structural(
                &self,
                root: CheckedStructuralValueHandle,
                calls: &[CheckedStructuralValueCall],
            ) -> usize {
                if self.established_values.contains(&root) {
                    return 0;
                }
                let plans = &self.values.structural_values;
                match &plans.nodes.get(root).kind {
                    CheckedStructuralValueKind::QuotientCall { arguments, .. } => arguments
                        .iter()
                        .map(|argument| self.structural(*argument, calls))
                        .sum(),
                    CheckedStructuralValueKind::Call { .. } => calls
                        .iter()
                        .find(|call| call.value == root)
                        .map_or(0, |call| {
                            call.operands()
                                .iter()
                                .map(|operation| match operation {
                                    CheckedUnitEffectOperationPlan::EstablishLocal(
                                        StructuralValue { value, calls, .. },
                                    ) => self.constructed(*value, calls, true),
                                    _ => self.operation(operation),
                                })
                                .sum::<usize>()
                                + call
                                    .operation()
                                    .map_or(0, |operation| self.operation(operation))
                        }),
                    CheckedStructuralValueKind::Case(case) => self.case(case),
                    crate::record_or_structural_case_pattern!(fields, ..) => plans
                        .record_fields
                        .span_or_empty(*fields)
                        .iter()
                        .map(|field| match field.value {
                            CheckedStructuralRecordFieldValue::Scalar(value) => {
                                self.computation(value)
                            }
                            CheckedStructuralRecordFieldValue::Structural(value) => {
                                self.structural(value, calls)
                            }
                        })
                        .sum(),
                    CheckedStructuralValueKind::FixedArray { elements } => elements
                        .iter()
                        .map(|element| self.structural(*element, calls))
                        .sum(),
                    CheckedStructuralValueKind::ScalarArray { elements } => elements
                        .iter()
                        .map(|element| self.computation(*element))
                        .sum(),
                    CheckedStructuralValueKind::Scalar { value: element }
                    | CheckedStructuralValueKind::ZeroedScalarArray { element, .. } => {
                        self.computation(*element)
                    }
                    CheckedStructuralValueKind::Projection { source, .. }
                    | CheckedStructuralValueKind::QuotientAdaptation { source, .. } => {
                        self.structural(*source, calls)
                    }
                    CheckedStructuralValueKind::Dispatch { subject, arms } => {
                        let mut prefix = 0;
                        let alternatives =
                            plans.dispatch_arms.span_or_empty(*arms).iter().map(|arm| {
                                prefix += self.pattern(&arm.pattern);
                                prefix + self.structural(arm.value, calls)
                            });
                        self.dispatch_subject(subject) + alternatives.min().unwrap_or(0)
                    }
                    CheckedStructuralValueKind::ViewSubslice {
                        source, start, end, ..
                    } => {
                        self.structural(*source, calls)
                            + start
                                .iter()
                                .chain(end.iter())
                                .map(|value| self.scalar(value))
                                .sum::<usize>()
                    }
                    CheckedStructuralValueKind::BorrowedSliceView { source, region }
                    | CheckedStructuralValueKind::MutableSliceView { source, region } => {
                        self.argument_evaluation(source)
                            + match region {
                                CheckedElementViewRegion::WholeCollection => 0,
                                CheckedElementViewRegion::WholeRepresentation => 0,
                                CheckedElementViewRegion::RepresentationBytes { start, .. } => {
                                    self.computation(*start)
                                }
                            }
                    }
                    CheckedStructuralValueKind::Reference {
                        source,
                        source_value,
                        ..
                    } => {
                        self.argument_evaluation(source)
                            + if source_value.is_valid() {
                                self.structural(*source_value, calls)
                            } else {
                                0
                            }
                    }
                    CheckedStructuralValueKind::Place(source)
                    | CheckedStructuralValueKind::CopiedStructuralPlace { source, .. } => {
                        self.argument_evaluation(source)
                    }
                    CheckedStructuralValueKind::ByteLiteralView { .. }
                    | CheckedStructuralValueKind::NativeCodeLayout { .. }
                    | CheckedStructuralValueKind::ZeroedStructural { .. } => 0,
                }
            }

            fn bytes(
                &self,
                value: &CheckedByteSequenceValuePlan,
                root: arena::Handle<CheckedByteSequenceValue>,
            ) -> usize {
                match value.nodes.get(root) {
                    CheckedByteSequenceValue::Descriptor { source, .. } => {
                        self.argument_evaluation(source)
                    }
                    CheckedByteSequenceValue::Concatenation { left, right, .. } => {
                        self.bytes(value, *left) + self.bytes(value, *right)
                    }
                    CheckedByteSequenceValue::Literal(_)
                    | CheckedByteSequenceValue::CallResult { .. }
                    | CheckedByteSequenceValue::Whole { .. }
                    | CheckedByteSequenceValue::Field { .. } => 0,
                }
            }

            fn operation(&self, operation: &CheckedUnitEffectOperationPlan) -> usize {
                match operation {
                    CheckedUnitEffectOperationPlan::Call(call) => {
                        call.structural_arguments
                            .iter()
                            .map(|argument| self.argument(argument))
                            .sum::<usize>()
                            + call
                                .scalar_arguments
                                .iter()
                                .map(|argument| self.scalar(argument))
                                .sum::<usize>()
                    }
                    CheckedUnitEffectOperationPlan::EstablishLocal(binding) => match binding {
                        crate::CheckedUnitLocalBindingPlan::Scalar { value, .. }
                        | crate::CheckedUnitLocalBindingPlan::Primitive { value, .. } => {
                            self.scalar(value)
                        }
                        crate::CheckedUnitLocalBindingPlan::ScalarArray { elements, .. } => {
                            elements.iter().map(|value| self.scalar(value)).sum()
                        }
                        StructuralValue { value, calls, .. } => self.structural(*value, calls),
                        ByteSequence { value, .. } => self.bytes(value, value.root),
                        crate::CheckedUnitLocalBindingPlan::Structural {
                            initial_value, ..
                        } => initial_value
                            .as_ref()
                            .map_or(0, |value| self.argument_evaluation(value.source())),
                        crate::CheckedUnitLocalBindingPlan::TrivialAffine { .. } => 0,
                    },
                    CheckedUnitEffectOperationPlan::EvaluateStructuralValue {
                        value,
                        calls,
                        ..
                    } => self.structural(*value, calls),
                    CheckedUnitEffectOperationPlan::Store(store) => match &store.value {
                        CheckedStoreValue::Scalar { value, .. } => self.store(value),
                        CheckedStoreValue::Bytes(value) => self.bytes(value, value.root),
                        CheckedStoreValue::Place(value)
                            if store.installed_value().is_some()
                                && matches!(
                                    value.source,
                                    CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
                                ) =>
                        {
                            self.argument(value)
                        }
                        CheckedStoreValue::Place(value) => self.argument_evaluation(value),
                        CheckedStoreValue::CaseLiteral(_) => 0,
                    },
                    CheckedUnitEffectOperationPlan::EstablishReference { source, .. }
                    | CheckedUnitEffectOperationPlan::EstablishViewSubslice { source, .. }
                    | CheckedUnitEffectOperationPlan::MoveStructuralField { source, .. } => {
                        self.argument_evaluation(source)
                    }
                    CheckedUnitEffectOperationPlan::CaptureAssignmentDestination {
                        destination,
                        ..
                    } => self.argument_evaluation(destination),
                    CheckedUnitEffectOperationPlan::GlobalDescriptorTable { storage, .. } => {
                        self.argument_evaluation(storage)
                    }
                    CheckedUnitEffectOperationPlan::RegisterAccess { arguments, .. } => {
                        arguments.iter().map(|argument| self.scalar(argument)).sum()
                    }
                    CheckedUnitEffectOperationPlan::AtomicAccess(atomic) => atomic
                        .event
                        .operands()
                        .iter()
                        .map(|operand| self.scalar(operand))
                        .sum(),
                    CheckedUnitEffectOperationPlan::AtomicFence { argument, .. } => {
                        self.argument(argument)
                    }
                    CheckedUnitEffectOperationPlan::ReleaseReference { .. }
                    | CheckedUnitEffectOperationPlan::ContinuationCleanup { .. }
                    | CheckedUnitEffectOperationPlan::Complete { .. } => 0,
                }
            }
        }

        impl crate::CheckedUnitCompletionPlan {
            pub fn definite_call_transfers<'calls>(
                &self,
                values: &CheckedValueFacts,
                call_schedules: &CallSchedules<'_, 'calls>,
                producer: &CheckedUnitEffectOperationPlan,
                local: SymbolHandle,
            ) -> usize {
                CallTransfers::for_producer(values, call_schedules, producer, local)
                    .and_then(|transfers| transfers.completion(&self.expression))
                    .flatten()
                    .unwrap_or(0)
            }
        }

        #[cfg(test)]
        mod tests {

            use super::*;

            #[test]
            fn place_identity_does_not_reenter_production_or_double_count_transfer() {
                let expression = arena::Handle::from_parts(1, 1);
                let mut argument = CheckedUnitStructuralArgumentPlan {
                    source: CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                        value_dependency: None,
                        expression,
                        logical_root_type_identity: None,
                    },
                    ..Default::default()
                };
                let mut values = CheckedValueFacts::default();
                let root = values
                    .structural_values
                    .nodes
                    .append(crate::CheckedStructuralValue {
                        expression,
                        kind: CheckedStructuralValueKind::Place(argument.clone()),
                        ..Default::default()
                    });
                values
                    .structural_values
                    .roots
                    .append(crate::CheckedStructuralValueRoot {
                        expression,
                        root,
                        ..Default::default()
                    });
                let unretained =
                    values
                        .structural_values
                        .nodes
                        .append(crate::CheckedStructuralValue {
                            expression,
                            ..Default::default()
                        });
                let other_expression = arena::Handle::from_parts(2, 1);
                let wrong_home =
                    values
                        .structural_values
                        .nodes
                        .append(crate::CheckedStructuralValue {
                            expression: other_expression,
                            ..Default::default()
                        });
                values
                    .structural_values
                    .roots
                    .append(crate::CheckedStructuralValueRoot {
                        expression: other_expression,
                        root: wrong_home,
                        ..Default::default()
                    });
                let result = CheckedUnitStructuralResultBindingPlan {
                    statement_index: 0,
                    binding_ordinal: 0,
                    type_identity: Default::default(),
                    multiplicity: language_semantics::Multiplicity::Affine,
                    local: Default::default(),
                };
                let transfers = CallTransfers {
                    values: &values,
                    call_schedules: &|_| None,
                    result: &result,
                    local: SymbolHandle::invalid(),
                    expression,
                    established_values: Vec::new(),
                    completed_computations: Vec::new(),
                };
                assert_eq!(transfers.argument(&argument), 1);
                assert_eq!(transfers.constructed(root, &[], true), 1);
                assert_eq!(transfers.constructed(root, &[], false), 0);
                if let CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                    value_dependency,
                    ..
                } = &mut argument.source
                {
                    *value_dependency = Some(root);
                }
                assert_eq!(transfers.argument(&argument), 1);
                for value in [
                    CheckedStructuralValueHandle::invalid(),
                    unretained,
                    wrong_home,
                ] {
                    if let CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                        value_dependency,
                        ..
                    } = &mut argument.source
                    {
                        *value_dependency = Some(value);
                    }
                    assert_eq!(transfers.argument(&argument), 0);
                }
                if let CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                    value_dependency,
                    ..
                } = &mut argument.source
                {
                    *value_dependency = Some(root);
                }
                argument.access = CheckedStructuralAccess::SharedBorrow;
                assert_eq!(transfers.argument(&argument), 0);
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum FlowStorageTransferSource {
        Statement {
            statement_index: usize,
        },
        Call {
            statement_index: usize,
            call_ordinal: usize,
            target_symbol: SymbolHandle,
        },
    }

    impl Default for FlowStorageTransferSource {
        fn default() -> Self {
            Self::Statement { statement_index: 0 }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowStorageTransferFact {
        pub source: FlowStorageTransferSource,
        pub evaluation: FlowPermissionEvaluation,
        pub expression: typed_trees::expression::ExpressionHandle,
        pub value_type: typed_trees::types::TypeReferenceHandle,
        pub source_arm: arena::Handle<typed_trees::expression::TableMatchArm>,
        pub root: facts::PlaceRoot,
        pub segments: HandleSpan<facts::PlaceSegment>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowBorrowClaimFact {
        pub loan: arena::Handle<crate::BorrowLoanFact>,
        pub claim_identity: language_semantics::PermissionClaimIdentity,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowLoanCompletionFact {
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub source: language_semantics::PermissionEventSource,
        pub root: facts::PlaceRoot,
        pub segments: HandleSpan<facts::PlaceSegment>,
        pub multiplicity: language_semantics::Multiplicity,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub enum FlowPermissionEvaluation {
        #[default]
        Statement,
        TransitionGuard,
        TransitionTarget(typed_trees::statement::TransitionTargetHandle),
        Unknown,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub struct FlowPermissionEventFact {
        pub machine_symbol: SymbolHandle,
        pub state_symbol: SymbolHandle,
        pub source: language_semantics::PermissionEventSource,
        pub evaluation: FlowPermissionEvaluation,
        pub expression: typed_trees::expression::ExpressionHandle,
        pub kind: language_semantics::PermissionEventKind,
        pub multiplicity: language_semantics::Multiplicity,
        pub access: language_semantics::PermissionAccess,
        pub claim_identity: language_semantics::PermissionClaimIdentity,
        pub provenance: language_semantics::PermissionProvenance,
        pub root: facts::PlaceRoot,
        pub segments: HandleSpan<facts::PlaceSegment>,
        pub obligation_live: bool,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowOwnedTransferPath {
        pub event: arena::Handle<FlowPermissionEventFact>,
        pub placement: FlowOwnedTransferPlacement,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum FlowOwnedTransferPlacement {
        #[default]
        Unresolved,
        OtherRoot,
        Outside,
        Within(HandleSpan<crate::flow::CheckedUnitStructuralPathSegment>),
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowOwnedSelectionReceipt {
        pub value: arena::Handle<crate::flow::FlowOperandFact>,
        pub evaluation: crate::flow::FlowPermissionEvaluation,
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub statement_ordinal: u32,
        pub expression: typed_trees::expression::ExpressionHandle,
        pub destination: facts::PlaceRoot,
        pub destination_path: HandleSpan<facts::PlaceSegment>,
        pub type_reference: typed_trees::types::TypeReferenceHandle,
        pub transfers: HandleSpan<FlowOwnedSelectionTransfer>,
        pub sources: HandleSpan<FlowOwnedSelectionSource>,
        pub death: language_semantics::PermissionEventSource,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowOwnedSelectionSource {
        pub symbol: SymbolHandle,
        pub statement_ordinal: u32,
        pub retains_storage: bool,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowOwnedSelectionTransfer {
        pub source_type_reference: typed_trees::types::TypeReferenceHandle,
        pub root: facts::PlaceRoot,
        pub destination_path: HandleSpan<facts::PlaceSegment>,
        pub expression: typed_trees::expression::ExpressionHandle,
        pub source_arm: arena::Handle<typed_trees::expression::TableMatchArm>,
        pub source: arena::Handle<FlowOwnedSelectionSource>,
        pub provenance: language_semantics::PermissionProvenance,
        pub claim_identity: language_semantics::PermissionClaimIdentity,
        pub origin_selection: arena::Handle<FlowOwnedSelectionReceipt>,
        pub path: HandleSpan<facts::PlaceSegment>,
        pub claims: HandleSpan<FlowOwnedSelectionClaim>,
    }

    pub type FlowSelectionPlaceArrival = language_semantics::PermissionArrival<SymbolHandle>;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowSelectionPlaceBinding {
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub statement_ordinal: usize,
        pub evaluation: FlowPermissionEvaluation,
        pub expression: typed_trees::expression::ExpressionHandle,
        pub decision: usize,
        pub root: facts::PlaceRoot,
        pub path: HandleSpan<facts::PlaceSegment>,
        pub selected: FlowSelectionPlaceArrival,
        pub remaining: FlowSelectionPlaceArrival,
        pub destination: FlowSelectionPlaceArrival,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowOwnedSelectionClaim {
        pub path: HandleSpan<facts::PlaceSegment>,
        pub claim_identity: language_semantics::PermissionClaimIdentity,
        pub provenance: language_semantics::PermissionProvenance,
    }

    impl crate::flow::FlowOwnershipFacts {
        pub fn state_exit_cleanup_roots(
            &self,
            machine: SymbolHandle,
            state: SymbolHandle,
        ) -> Option<Vec<&FlowPermissionEventFact>> {
            let mut roots = Vec::<&FlowPermissionEventFact>::new();
            let mut occurrences = Vec::new();
            let handles = self.with_state_permissions(state, |handles| handles.to_vec());
            for handle in handles {
                let event = self.permissions.get(handle);
                if !(event.machine_symbol == machine
                    && event.state_symbol == state
                    && event.source == language_semantics::PermissionEventSource::StateExit
                    && event.kind == language_semantics::PermissionEventKind::AffineDrop)
                {
                    continue;
                }
                let occurrence = (event.root, event.segments, event.evaluation);
                if occurrences.contains(&occurrence) {
                    return None;
                }
                occurrences.push(occurrence);
                if let Some(prior) = roots
                    .iter()
                    .find(|prior| prior.root == event.root && prior.segments == event.segments)
                {
                    let mut same_exit = (**prior).clone();
                    same_exit.evaluation = event.evaluation;
                    if same_exit != *event {
                        return None;
                    }
                } else {
                    roots.push(event);
                }
            }
            Some(roots)
        }

        pub fn owned_selection_for_expression(
            &self,
            state: SymbolHandle,
            statement_ordinal: u32,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> Option<(
            arena::Handle<FlowOwnedSelectionReceipt>,
            &FlowOwnedSelectionReceipt,
        )> {
            self.with_state_owned_selections(state, |handles| {
                let mut matching = handles.iter().copied().filter_map(|handle| {
                    let receipt = self.owned_selections.get(handle);
                    (receipt.state == state
                        && receipt.statement_ordinal == statement_ordinal
                        && receipt.expression == expression)
                        .then_some((handle, receipt))
                });
                let receipt = matching.next()?;
                matching.next().is_none().then_some(receipt)
            })
        }

        pub fn owned_selection_at(
            &self,
            state: SymbolHandle,
            statement_ordinal: u32,
        ) -> Option<(
            arena::Handle<FlowOwnedSelectionReceipt>,
            &FlowOwnedSelectionReceipt,
        )> {
            self.owned_selection_at_evaluation(state, statement_ordinal, Statement)
        }

        pub fn owned_selection_at_evaluation(
            &self,
            state: SymbolHandle,
            statement_ordinal: u32,
            evaluation: crate::flow::FlowPermissionEvaluation,
        ) -> Option<(
            arena::Handle<FlowOwnedSelectionReceipt>,
            &FlowOwnedSelectionReceipt,
        )> {
            self.with_state_owned_selections(state, |handles| {
                let mut matches = handles.iter().copied().filter_map(|handle| {
                    let receipt = self.owned_selections.get(handle);
                    (receipt.state == state
                        && receipt.statement_ordinal == statement_ordinal
                        && receipt.evaluation == evaluation)
                        .then_some((handle, receipt))
                });
                let receipt = matches.next()?;
                matches.next().is_none().then_some(receipt)
            })
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum FlowClaimOutcomeSource {
        #[default]
        Unknown,
        Input {
            parameter_symbol: SymbolHandle,
            segments: HandleSpan<facts::PlaceSegment>,
        },
        Established {
            claim_identity: language_semantics::PermissionClaimIdentity,
            provenance: language_semantics::PermissionProvenance,
        },
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowClaimOutcomeEntryFact {
        pub output_segments: HandleSpan<facts::PlaceSegment>,
        pub source: FlowClaimOutcomeSource,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowClaimOutcomeMapFact {
        pub machine_symbol: SymbolHandle,
        pub state_symbol: SymbolHandle,
        pub entries: HandleSpan<FlowClaimOutcomeEntryFact>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum FlowClaimJoinExitKind {
        #[default]
        FinalExpression,
        TransitionTarget,
        TransitionContinuation,
        CallResult {
            call_ordinal: usize,
        },
        SelectionArm {
            expression: typed_trees::expression::ExpressionHandle,
            arm: arena::Handle<typed_trees::expression::TableMatchArm>,
        },
        SelectionDecision {
            evaluation: FlowPermissionEvaluation,
            expression: typed_trees::expression::ExpressionHandle,
            decision: usize,
            selected: bool,
        },
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowClaimJoinExit {
        pub state_symbol: SymbolHandle,
        pub statement_index: usize,
        pub kind: FlowClaimJoinExitKind,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowClaimJoinAlternative {
        pub captures: HandleSpan<FlowClaimJoinCapture>,
        pub exits: HandleSpan<FlowClaimJoinExit>,
        pub source: FlowClaimJoinAlternativeSource,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowClaimJoinCapture {
        pub operand: arena::Handle<crate::flow::FlowOperandFact>,
        pub path: HandleSpan<facts::PlaceSegment>,
        pub identity: language_semantics::PermissionClaimIdentity,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum FlowClaimJoinAlternativeSource {
        #[default]
        Inactive,
        ExcludedInput {
            source: FlowClaimOutcomeSource,
            operand: arena::Handle<crate::flow::FlowOperandFact>,
            segments: HandleSpan<facts::PlaceSegment>,
        },
        Claim {
            source: FlowClaimOutcomeSource,
            bound_source: FlowClaimOutcomeSource,
        },
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowClaimJoinReceipt {
        pub machine_symbol: SymbolHandle,
        pub state_symbol: SymbolHandle,
        pub statement_index: usize,
        pub call_ordinal: usize,
        pub target_symbol: SymbolHandle,
        pub expression: typed_trees::expression::ExpressionHandle,
        pub result_root: facts::PlaceRoot,
        pub result_segments: HandleSpan<facts::PlaceSegment>,
        pub claim_identity: language_semantics::PermissionClaimIdentity,
        pub alternatives: HandleSpan<FlowClaimJoinAlternative>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowInstructionDispositionFact {
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub source: language_semantics::PermissionEventSource,
        pub evaluation: FlowPermissionEvaluation,
        pub requirement: SymbolHandle,
        pub fact: arena::Handle<typed_trees::domain::ProofFact>,
        pub argument: typed_trees::expression::ExpressionHandle,
        pub event: facts::FactHandle,
        pub evidence: arena::Handle<FlowInstructionEffectReceiptFact>,
        pub root: facts::PlaceRoot,
        pub segments: HandleSpan<facts::PlaceSegment>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct FlowInstructionEffectReceiptFact {
        pub event: facts::FactHandle,
        pub contract: arena::Handle<crate::ContractProofFact>,
        pub observation: facts::ProgramPoint,
        pub children: HandleSpan<FlowInstructionEffectReceiptFact>,
    }
}
