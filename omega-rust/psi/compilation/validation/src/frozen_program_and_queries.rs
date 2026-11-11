/*
Immutable checking input context and memo tables (`frozen_program`) alongside
shared read-only program queries (`queries`: `execution_projection`,
`expression_traversal`, `layout_checking`, `product_queries`, `substitutions`,
`type_applications`, and `type_regions`).
*/

pub mod frozen_program {
    use crate::declarations::symbols::CallerSiteCaches;
    use crate::machine_calls::calls::CallFrameCaches;
    use crate::machine_calls::effect_inference::PreparedEffectPlans;
    use crate::proof_contracts::contract_entailment::structural_judgment::{
        CaseGuaranteeEligibility, EntryMachines, LicenseCandidates,
    };
    use crate::value_custody::content_conservation::ContentConservationInputs;
    use language_semantics::declaration_selection::AuthoredDeclarationSelections;
    use representation_selections::OpaqueRepresentationSelection;
    use std::fmt;
    use std::ops::Deref;
    use std::sync::{Arc, Mutex, OnceLock};
    use typed_trees::TypedTrees;
    use typed_trees::proof_only::ProofOnlyClassification;

    pub(crate) mod queries {
        use crate::CheckingInput;
        use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
        use symbols::SymbolHandle;

        #[cfg(test)]
        pub(crate) mod tests {
            use super::CheckingQuery;
            use crate::{CheckingContext, CheckingInput, CheckingProjection};
            use symbols::SymbolHandle;
            use typed_trees::TypedTrees;

            #[test]
            fn numeric_query_ancestry_is_exact_and_request_local() {
                let program = TypedTrees::default();
                let context = CheckingContext::new(&program);
                let machine = SymbolHandle::from_parts(1, 1);
                let first = SymbolHandle::from_parts(2, 1);
                let second = SymbolHandle::from_parts(3, 1);
                let query = CheckingQuery::numeric_observation(&context, machine, first).unwrap();
                let descendant =
                    CheckingQuery::numeric_observation(&query, machine, second).unwrap();
                assert!(CheckingQuery::numeric_observation(&descendant, machine, first).is_none());
                assert!(CheckingQuery::numeric_observation(&descendant, machine, second).is_none());
                assert!(
                    CheckingQuery::numeric_observation(
                        &descendant,
                        machine,
                        SymbolHandle::from_parts(first.arena_index(), first.generation() + 1),
                    )
                    .is_some()
                );

                assert!(CheckingQuery::numeric_observation(&query, machine, second).is_some());
                assert!(CheckingQuery::numeric_observation(&context, machine, first).is_some());
                let independent = TypedTrees::default();
                assert!(CheckingQuery::numeric_observation(&independent, machine, first).is_some());
                let one_shot =
                    CheckingQuery::numeric_observation(&program, machine, first).unwrap();
                assert!(one_shot.checking_memos().is_none());
                assert!(CheckingQuery::numeric_observation(&one_shot, machine, first).is_none());
                assert!(
                    CheckingQuery::numeric_observation(
                        &descendant,
                        SymbolHandle::from_parts(machine.arena_index(), machine.generation() + 1),
                        first,
                    )
                    .is_some()
                );
                std::thread::scope(|scope| {
                    assert!(
                        scope
                            .spawn(|| {
                                CheckingQuery::numeric_observation(&context, machine, first)
                                    .is_some()
                            })
                            .join()
                            .unwrap()
                    );
                });
            }

            #[test]
            fn only_queries_inside_a_cut_ancestor_record_the_cut() {
                let program = TypedTrees::default();
                let context = CheckingContext::new(&program);
                let machine = SymbolHandle::from_parts(1, 1);
                let first = SymbolHandle::from_parts(2, 1);
                let second = SymbolHandle::from_parts(3, 1);
                let third = SymbolHandle::from_parts(4, 1);
                let query = CheckingQuery::numeric_observation(&context, machine, first).unwrap();
                let descendant =
                    CheckingQuery::numeric_observation(&query, machine, second).unwrap();

                assert!(CheckingQuery::numeric_observation(&descendant, machine, second).is_none());
                assert!(!descendant.met_enclosing_cut());

                assert!(CheckingQuery::numeric_observation(&descendant, machine, third).is_some());
                assert!(!descendant.met_enclosing_cut());

                let nested =
                    CheckingQuery::numeric_observation(&descendant, machine, third).unwrap();
                assert!(CheckingQuery::numeric_observation(&nested, machine, first).is_none());
                assert!(nested.met_enclosing_cut());
                assert!(descendant.met_enclosing_cut());
                assert!(!query.met_enclosing_cut());

                let population = CheckingQuery::operand_fit_population(&context, machine).unwrap();
                let numeric =
                    CheckingQuery::numeric_observation(&population, machine, first).unwrap();
                assert!(CheckingQuery::operand_fit_population(&numeric, machine).is_none());
                assert!(numeric.met_enclosing_cut());
                assert!(!population.met_enclosing_cut());
            }

            #[test]
            fn numeric_query_views_preserve_inputs_and_follow_projection_custody() {
                let program = TypedTrees::default();
                let context = CheckingContext::new(&program).with_pending_layout_geometry(true);
                let machine = SymbolHandle::from_parts(1, 1);
                let state = SymbolHandle::from_parts(2, 1);
                let population = CheckingQuery::operand_fit_population(&context, machine).unwrap();
                let query =
                    CheckingQuery::numeric_observation(&population, machine, state).unwrap();
                assert!(query.defers_pending_layout_geometry());
                assert!(std::ptr::eq(context.typed_program(), query.typed_program()));
                query
                    .checking_memos()
                    .unwrap()
                    .shared_proof_walks
                    .lock()
                    .unwrap()
                    .insert(machine, std::sync::Arc::default());
                assert!(
                    context
                        .checking_memos()
                        .unwrap()
                        .shared_proof_walks
                        .lock()
                        .unwrap()
                        .get(&machine)
                        .is_none()
                );
                assert!(std::sync::Arc::ptr_eq(
                    &context.proof_only_classification(),
                    &query.proof_only_classification(),
                ));

                let layout = crate::queries::layout_checking::LayoutCheckingContext::new(
                    &query,
                    Some("linux_x86_64"),
                );
                assert_eq!(layout.layout_target(), Some("linux_x86_64"));
                assert!(CheckingQuery::operand_fit_population(&layout, machine).is_none());
                assert!(CheckingQuery::numeric_observation(&layout, machine, state).is_none());
                let mut projection = CheckingProjection::new(&query);
                assert!(CheckingQuery::operand_fit_population(&projection, machine).is_none());
                assert!(CheckingQuery::numeric_observation(&projection, machine, state).is_none());
                projection.program_mut();
                assert!(projection.checking_memos().is_none());
                assert!(CheckingQuery::operand_fit_population(&projection, machine).is_some());
                assert!(CheckingQuery::numeric_observation(&projection, machine, state).is_some());
            }

            #[test]
            fn operand_fit_population_ancestry_is_exact_and_composes_with_numeric_queries() {
                let program = TypedTrees::default();
                let context = CheckingContext::new(&program);
                let machine = SymbolHandle::from_parts(1, 1);
                let other = SymbolHandle::from_parts(2, 1);
                let state = SymbolHandle::from_parts(3, 1);
                let population = CheckingQuery::operand_fit_population(&context, machine).unwrap();
                let numeric =
                    CheckingQuery::numeric_observation(&population, machine, state).unwrap();
                assert!(CheckingQuery::operand_fit_population(&numeric, machine).is_none());
                let descendant = CheckingQuery::operand_fit_population(&numeric, other).unwrap();
                assert!(CheckingQuery::operand_fit_population(&descendant, machine).is_none());
                assert!(CheckingQuery::operand_fit_population(&descendant, other).is_none());
                assert!(CheckingQuery::numeric_observation(&descendant, machine, state).is_none());
                assert!(CheckingQuery::operand_fit_population(&population, other).is_some());
                assert!(CheckingQuery::operand_fit_population(&context, machine).is_some());
                assert!(
                    CheckingQuery::operand_fit_population(
                        &descendant,
                        SymbolHandle::from_parts(machine.arena_index(), machine.generation() + 1),
                    )
                    .is_some()
                );
                let independent = TypedTrees::default();
                assert!(CheckingQuery::operand_fit_population(&independent, machine).is_some());
                let one_shot = CheckingQuery::operand_fit_population(&program, machine).unwrap();
                assert!(one_shot.checking_memos().is_none());
                assert!(CheckingQuery::operand_fit_population(&one_shot, machine).is_none());
                std::thread::scope(|scope| {
                    assert!(
                        scope
                            .spawn(|| CheckingQuery::operand_fit_population(&context, machine)
                                .is_some())
                            .join()
                            .unwrap()
                    );
                });
            }

            #[test]
            fn completed_case_population_ancestry_follows_the_request_and_projection_owner() {
                let program = TypedTrees::default();
                let context = CheckingContext::new(&program);
                let machine = SymbolHandle::from_parts(1, 1);
                let other = SymbolHandle::from_parts(2, 1);
                let state = SymbolHandle::from_parts(3, 1);
                let population =
                    CheckingQuery::completed_case_population(&context, machine).unwrap();
                let fits = CheckingQuery::operand_fit_population(&population, machine).unwrap();
                let numeric = CheckingQuery::numeric_observation(&fits, machine, state).unwrap();
                assert!(CheckingQuery::completed_case_population(&numeric, machine).is_none());
                assert!(CheckingQuery::completed_case_population(&numeric, other).is_some());
                assert!(
                    CheckingQuery::completed_case_population(
                        &numeric,
                        SymbolHandle::from_parts(machine.arena_index(), machine.generation() + 1),
                    )
                    .is_some()
                );
                assert!(CheckingQuery::completed_case_population(&context, machine).is_some());
                let independent = TypedTrees::default();
                assert!(CheckingQuery::completed_case_population(&independent, machine).is_some());
                let layout = crate::queries::layout_checking::LayoutCheckingContext::new(
                    &numeric,
                    Some("linux_x86_64"),
                );
                assert!(CheckingQuery::completed_case_population(&layout, machine).is_none());
                let mut projection = CheckingProjection::new(&layout);
                assert!(CheckingQuery::completed_case_population(&projection, machine).is_none());
                projection.program_mut();
                assert!(projection.checking_memos().is_none());
                assert!(CheckingQuery::completed_case_population(&projection, machine).is_some());
            }

            #[test]
            fn only_cut_free_answers_under_a_query_reach_the_context() {
                let program = TypedTrees::default();
                let context = CheckingContext::new(&program);
                let machine = SymbolHandle::from_parts(1, 1);
                let state = SymbolHandle::from_parts(2, 1);
                let other = SymbolHandle::from_parts(3, 1);
                let shared = context.checking_memos().unwrap();
                let fresh = || CheckingQuery::operand_fit_population(&context, machine).unwrap();

                let population = fresh();
                let query =
                    CheckingQuery::numeric_observation(&population, machine, state).unwrap();
                let guard = crate::frozen_program::CutGuard::start(&query);
                assert!(!query.has_active_numeric_observation(machine, other));
                assert!(std::ptr::eq(guard.memos().unwrap(), shared));

                let query =
                    CheckingQuery::numeric_observation(&population, machine, state).unwrap();
                let guard = crate::frozen_program::CutGuard::start(&query);
                assert!(query.has_active_numeric_observation(machine, state));
                assert!(std::ptr::eq(
                    guard.memos().unwrap(),
                    query.checking_memos().unwrap()
                ));
                let query =
                    CheckingQuery::numeric_observation(&population, machine, state).unwrap();
                let guard = crate::frozen_program::CutGuard::start(&query);
                assert!(CheckingQuery::operand_fit_population(&query, machine).is_none());
                assert!(std::ptr::eq(
                    guard.memos().unwrap(),
                    query.checking_memos().unwrap()
                ));

                let population = fresh();
                let query =
                    CheckingQuery::numeric_observation(&population, machine, state).unwrap();
                assert!(query.has_active_numeric_observation(machine, state));
                query
                    .checking_memos()
                    .unwrap()
                    .shared_proof_walks
                    .lock()
                    .unwrap()
                    .insert(machine, std::sync::Arc::default());
                let guard = crate::frozen_program::CutGuard::start(&query);
                assert!(
                    query
                        .checking_memos()
                        .unwrap()
                        .shared_proof_walks
                        .lock()
                        .unwrap()
                        .contains_key(&machine)
                );
                assert!(std::ptr::eq(
                    guard.memos().unwrap(),
                    query.checking_memos().unwrap()
                ));

                let population = fresh();
                let query =
                    CheckingQuery::numeric_observation(&population, machine, state).unwrap();
                let guard = crate::frozen_program::CutGuard::start(&query);
                let nested = CheckingQuery::numeric_observation(&query, machine, other).unwrap();
                assert!(nested.has_active_numeric_observation(machine, other));
                assert!(std::ptr::eq(guard.memos().unwrap(), shared));
                assert!(nested.has_active_operand_fit_population(machine));
                assert!(std::ptr::eq(
                    guard.memos().unwrap(),
                    query.checking_memos().unwrap()
                ));

                assert_eq!(crate::frozen_program::readable_memos(&context).count(), 1);
                assert_eq!(crate::frozen_program::readable_memos(&query).count(), 2);
            }
        }

        #[derive(Clone, Copy, Debug)]
        pub(crate) enum QueryKind {
            Numeric {
                machine: SymbolHandle,
                state: SymbolHandle,
            },
            OperandFits {
                machine: SymbolHandle,
            },
            CompletedCases {
                machine: SymbolHandle,
            },
            RecastFormation {
                expression: typed_trees::expression::ExpressionHandle,
            },
        }

        pub(crate) struct CheckingQuery<'input> {
            input: &'input dyn CheckingInput,
            query: QueryKind,
            memos: Option<crate::CheckingMemos>,
            met_enclosing_cut: AtomicBool,
            cut_reads: AtomicU64,
        }

        impl std::fmt::Debug for CheckingQuery<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("CheckingQuery")
                    .field("input", &self.input)
                    .field("query", &self.query)
                    .finish_non_exhaustive()
            }
        }

        impl<'input> CheckingQuery<'input> {
            pub(crate) fn recast_formation(
                input: &'input dyn CheckingInput,
                expression: typed_trees::expression::ExpressionHandle,
            ) -> Option<Self> {
                if input.has_active_recast_formation(expression, 0) {
                    return None;
                }
                Some(Self::new(input, QueryKind::RecastFormation { expression }))
            }

            pub(crate) fn numeric_observation(
                input: &'input dyn CheckingInput,
                machine: SymbolHandle,
                state: SymbolHandle,
            ) -> Option<Self> {
                if input.has_active_numeric_observation(machine, state) {
                    return None;
                }
                Some(Self::new(input, QueryKind::Numeric { machine, state }))
            }

            pub(crate) fn operand_fit_population(
                input: &'input dyn CheckingInput,
                machine: SymbolHandle,
            ) -> Option<Self> {
                if input.has_active_operand_fit_population(machine) {
                    return None;
                }
                Some(Self::new(input, QueryKind::OperandFits { machine }))
            }

            pub(crate) fn completed_case_population(
                input: &'input dyn CheckingInput,
                machine: SymbolHandle,
            ) -> Option<Self> {
                if input.has_active_completed_case_population(machine) {
                    return None;
                }
                Some(Self::new(input, QueryKind::CompletedCases { machine }))
            }

            fn new(input: &'input dyn CheckingInput, query: QueryKind) -> Self {
                Self {
                    input,
                    query,
                    memos: input
                        .checking_memos()
                        .map(crate::CheckingMemos::query_scratch),
                    met_enclosing_cut: AtomicBool::new(false),
                    cut_reads: AtomicU64::new(0),
                }
            }

            pub(crate) fn met_enclosing_cut(&self) -> bool {
                self.met_enclosing_cut.load(Ordering::Relaxed)
            }

            fn enclosing_cut(&self, enclosing: bool) -> bool {
                if enclosing {
                    self.met_enclosing_cut.store(true, Ordering::Relaxed);
                }
                enclosing
            }

            fn counted(&self, active: bool) -> bool {
                if active {
                    self.cut_reads.fetch_add(1, Ordering::Relaxed);
                }
                active
            }
        }

        impl CheckingInput for CheckingQuery<'_> {
            fn has_active_recast_formation(
                &self,
                expression: typed_trees::expression::ExpressionHandle,
                depth: usize,
            ) -> bool {
                let formation = matches!(self.query, QueryKind::RecastFormation { .. });
                self.counted(
                    matches!(self.query, QueryKind::RecastFormation { expression: active }
                if active == expression)
                        || self.enclosing_cut(self.input.has_active_recast_formation(
                            expression,
                            depth + usize::from(formation),
                        )),
                )
            }

            fn external_invocation_roots(&self) -> Option<&[symbols::SymbolHandle]> {
                self.input.external_invocation_roots()
            }

            fn typed_program(&self) -> &typed_trees::TypedTrees {
                self.input.typed_program()
            }

            fn has_active_numeric_observation(
                &self,
                machine: SymbolHandle,
                state: SymbolHandle,
            ) -> bool {
                self.counted(
                    matches!(self.query, QueryKind::Numeric { machine: active, state: active_state }
            if active == machine && active_state == state)
                        || self.enclosing_cut(
                            self.input.has_active_numeric_observation(machine, state),
                        ),
                )
            }

            fn has_active_operand_fit_population(&self, machine: SymbolHandle) -> bool {
                self.counted(
            matches!(self.query, QueryKind::OperandFits { machine: active } if active == machine)
                || self.enclosing_cut(self.input.has_active_operand_fit_population(machine)),
        )
            }

            fn has_active_completed_case_population(&self, machine: SymbolHandle) -> bool {
                self.counted(
            matches!(self.query, QueryKind::CompletedCases { machine: active } if active == machine)
                || self.enclosing_cut(self.input.has_active_completed_case_population(machine)),
        )
            }

            fn defers_pending_layout_geometry(&self) -> bool {
                self.input.defers_pending_layout_geometry()
            }

            fn retained_borrow_custodies(&self) -> &[checked_trees::RetainedBorrowCustodyFact] {
                self.input.retained_borrow_custodies()
            }

            fn layout_target(&self) -> Option<&str> {
                self.input.layout_target()
            }

            fn opaque_representation_selections(
                &self,
            ) -> &[representation_selections::OpaqueRepresentationSelection] {
                self.input.opaque_representation_selections()
            }

            fn authored_declaration_selections(
                &self,
            ) -> &language_semantics::declaration_selection::AuthoredDeclarationSelections
            {
                self.input.authored_declaration_selections()
            }

            fn checking_memos(&self) -> Option<&crate::CheckingMemos> {
                self.memos.as_ref()
            }

            fn cut_reads(&self) -> u64 {
                self.cut_reads.load(Ordering::Relaxed) + self.input.cut_reads()
            }

            fn shared_memos(&self) -> Option<&crate::CheckingMemos> {
                self.input.shared_memos()
            }

            fn proof_only_classification(
                &self,
            ) -> std::sync::Arc<typed_trees::proof_only::ProofOnlyClassification> {
                self.input.proof_only_classification()
            }

            fn call_frame_tables(
                &self,
            ) -> Option<std::sync::Arc<crate::machine_calls::calls::CallFrameTables<'_>>>
            {
                self.input.call_frame_tables()
            }

            fn prepared_effect_plans(&self) -> Option<&crate::PreparedEffectPlans> {
                self.input.prepared_effect_plans()
            }

            fn storage_graph(&self) -> Option<&checked_trees::FlowControlFacts> {
                self.input.storage_graph()
            }

            fn expression_interval(
                &self,
                expression: typed_trees::expression::ExpressionHandle,
            ) -> Option<crate::ExpressionInterval> {
                self.input.expression_interval(expression)
            }

            fn prepared_mathematics(
                &self,
            ) -> Option<crate::mathematical_applications::PreparedMathematicalView<'_>>
            {
                self.input.prepared_mathematics()
            }
        }
    }
    pub(crate) use queries::CheckingQuery;

    mod sealed {
        pub trait Sealed {}
        impl Sealed for typed_trees::TypedTrees {}
        impl Sealed for checked_trees::CheckedTrees {}
        impl Sealed for super::CheckingContext<'_> {}
        impl Sealed for super::CheckingProjection<'_> {}
        impl Sealed for crate::queries::layout_checking::LayoutCheckingContext<'_> {}
        impl Sealed for typed_trees::FrozenTypedTrees {}
        impl Sealed for super::queries::CheckingQuery<'_> {}
    }

    pub trait CheckingInput: sealed::Sealed + fmt::Debug + Send + Sync {
        fn typed_program(&self) -> &TypedTrees;

        fn external_invocation_roots(&self) -> Option<&[symbols::SymbolHandle]> {
            None
        }

        #[doc(hidden)]
        fn has_active_numeric_observation(
            &self,
            _machine: symbols::SymbolHandle,
            _state: symbols::SymbolHandle,
        ) -> bool {
            false
        }

        #[doc(hidden)]
        fn has_active_operand_fit_population(&self, _machine: symbols::SymbolHandle) -> bool {
            false
        }

        #[doc(hidden)]
        fn has_active_completed_case_population(&self, _machine: symbols::SymbolHandle) -> bool {
            false
        }

        #[doc(hidden)]
        fn has_active_recast_formation(
            &self,
            _expression: typed_trees::expression::ExpressionHandle,
            depth: usize,
        ) -> bool {
            depth >= 64
        }

        fn defers_pending_layout_geometry(&self) -> bool {
            false
        }

        fn retained_borrow_custodies(&self) -> &[checked_trees::RetainedBorrowCustodyFact] {
            &[]
        }

        fn layout_target(&self) -> Option<&str> {
            None
        }

        fn opaque_representation_selections(&self) -> &[OpaqueRepresentationSelection] {
            &[]
        }

        fn authored_declaration_selections(&self) -> &AuthoredDeclarationSelections {
            self.typed_program().authored_declaration_selections()
        }

        #[doc(hidden)]
        fn checking_memos(&self) -> Option<&CheckingMemos>;

        #[doc(hidden)]
        fn cut_reads(&self) -> u64;

        #[doc(hidden)]
        fn shared_memos(&self) -> Option<&CheckingMemos>;

        fn proof_only_classification(&self) -> Arc<ProofOnlyClassification> {
            let compute = || Arc::new(typed_trees::proof_only::classify(self.typed_program()));
            match self.checking_memos() {
                Some(memos) => memos.proof_only.get_or_init(compute).clone(),
                None => compute(),
            }
        }

        fn call_frame_tables(
            &self,
        ) -> Option<Arc<crate::machine_calls::calls::CallFrameTables<'_>>> {
            None
        }

        fn prepared_effect_plans(&self) -> Option<&PreparedEffectPlans> {
            None
        }

        fn storage_graph(&self) -> Option<&checked_trees::FlowControlFacts> {
            None
        }

        fn expression_interval(
            &self,
            _expression: typed_trees::expression::ExpressionHandle,
        ) -> Option<crate::ExpressionInterval> {
            None
        }

        fn prepared_mathematics(
            &self,
        ) -> Option<crate::mathematical_applications::PreparedMathematicalView<'_>> {
            None
        }
    }

    impl Deref for dyn CheckingInput + '_ {
        type Target = TypedTrees;

        #[inline]
        fn deref(&self) -> &TypedTrees {
            self.typed_program()
        }
    }

    impl PartialEq for dyn CheckingInput + '_ {
        fn eq(&self, other: &Self) -> bool {
            self.external_invocation_roots() == other.external_invocation_roots()
                && self.layout_target() == other.layout_target()
                && self.defers_pending_layout_geometry() == other.defers_pending_layout_geometry()
                && self.typed_program() == other.typed_program()
        }
    }

    impl Eq for dyn CheckingInput + '_ {}

    pub(crate) fn expression_table(
        program: &dyn CheckingInput,
    ) -> &typed_trees::expression::ExpressionTable {
        &program.expression_table
    }

    impl CheckingInput for TypedTrees {
        fn typed_program(&self) -> &TypedTrees {
            self
        }

        fn checking_memos(&self) -> Option<&CheckingMemos> {
            None
        }

        fn cut_reads(&self) -> u64 {
            0
        }

        fn shared_memos(&self) -> Option<&CheckingMemos> {
            None
        }
    }

    pub(crate) fn once_lock_rows<T>(
        symbols: impl Iterator<Item = symbols::SymbolHandle>,
    ) -> Vec<(symbols::SymbolHandle, OnceLock<T>)> {
        let mut symbols: Vec<_> = symbols.collect();
        symbols.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
        symbols.dedup();
        symbols
            .into_iter()
            .map(|symbol| (symbol, OnceLock::new()))
            .collect()
    }

    impl CheckingInput for typed_trees::FrozenTypedTrees {
        fn typed_program(&self) -> &TypedTrees {
            self.as_ref()
        }

        fn checking_memos(&self) -> Option<&CheckingMemos> {
            None
        }

        fn cut_reads(&self) -> u64 {
            0
        }

        fn shared_memos(&self) -> Option<&CheckingMemos> {
            None
        }
    }

    impl CheckingInput for checked_trees::CheckedTrees {
        fn storage_graph(&self) -> Option<&checked_trees::FlowControlFacts> {
            Some(&self.facts.flow.control)
        }

        fn retained_borrow_custodies(&self) -> &[checked_trees::RetainedBorrowCustodyFact] {
            &self.facts.qualifications.content.retained_borrow_custodies
        }

        fn typed_program(&self) -> &TypedTrees {
            &self.typed
        }

        fn authored_declaration_selections(&self) -> &AuthoredDeclarationSelections {
            self.facts.authored_declaration_selections()
        }

        fn checking_memos(&self) -> Option<&CheckingMemos> {
            None
        }

        fn cut_reads(&self) -> u64 {
            0
        }

        fn shared_memos(&self) -> Option<&CheckingMemos> {
            None
        }
    }

    pub struct CheckingContext<'program> {
        defer_layout_geometry: bool,
        program: &'program TypedTrees,
        external_roots: Option<&'program [symbols::SymbolHandle]>,
        retained_borrows: Arc<[checked_trees::RetainedBorrowCustodyFact]>,
        finalized_selections: Option<&'program AuthoredDeclarationSelections>,
        prepared_effects: Option<&'program PreparedEffectPlans>,
        storage_graph: Option<Arc<checked_trees::FlowFacts>>,
        expression_intervals: OnceLock<Arc<[crate::ExpressionInterval]>>,
        prepared_mathematics:
            Option<crate::mathematical_applications::PreparedMathematicalView<'program>>,
        opaque_selections: Option<&'program [OpaqueRepresentationSelection]>,
        memos: Arc<CheckingMemos>,
        call_frame_tables:
            OnceLock<Option<Arc<crate::machine_calls::calls::CallFrameTables<'program>>>>,
    }

    impl<'program> CheckingContext<'program> {
        fn reset_context_sensitive_memos(&mut self) {
            let memos = std::mem::take(&mut self.memos);
            self.memos = Arc::new(Arc::try_unwrap(memos).map_or_else(
                |shared| shared.clone_typed_tree_memos(),
                CheckingMemos::retain_typed_tree_memos,
            ));
        }

        pub fn with_retained_borrow_custodies(
            mut self,
            rows: Arc<[checked_trees::RetainedBorrowCustodyFact]>,
        ) -> Self {
            self.retained_borrows = rows;
            self.memos = Arc::new(CheckingMemos::default());
            self.call_frame_tables = OnceLock::new();
            self
        }

        pub fn with_pending_layout_geometry(mut self, defer: bool) -> Self {
            if self.defer_layout_geometry != defer {
                self.defer_layout_geometry = defer;
                self.reset_context_sensitive_memos();
            }
            self
        }

        pub fn new(program: &'program TypedTrees) -> Self {
            Self {
                defer_layout_geometry: false,
                program,
                external_roots: None,
                retained_borrows: Arc::from([]),
                finalized_selections: None,
                prepared_effects: None,
                storage_graph: None,
                expression_intervals: OnceLock::new(),
                prepared_mathematics: None,
                opaque_selections: None,
                memos: Arc::new(CheckingMemos::default()),
                call_frame_tables: OnceLock::new(),
            }
        }

        pub fn with_external_invocation_roots(
            mut self,
            roots: &'program [symbols::SymbolHandle],
        ) -> Self {
            if self.external_roots != Some(roots) {
                self.external_roots = Some(roots);
                self.memos = Arc::new(CheckingMemos::default());
                self.call_frame_tables = OnceLock::new();
            }
            self
        }

        pub fn with_authored_declaration_selections(
            mut self,
            selections: &'program AuthoredDeclarationSelections,
        ) -> Self {
            let changed = self.authored_declaration_selections() != selections;
            self.finalized_selections = Some(selections);
            if changed {
                self.reset_context_sensitive_memos();
            }
            self
        }

        pub fn clone_with_storage_graph(
            &self,
            graph: Arc<checked_trees::FlowFacts>,
        ) -> CheckingContext<'program> {
            CheckingContext {
                defer_layout_geometry: self.defer_layout_geometry,
                program: self.program,
                external_roots: self.external_roots,
                retained_borrows: self.retained_borrows.clone(),
                finalized_selections: self.finalized_selections,
                prepared_effects: self.prepared_effects,
                storage_graph: Some(graph),
                expression_intervals: self.expression_intervals.clone(),
                prepared_mathematics: self.prepared_mathematics,
                opaque_selections: self.opaque_selections,
                memos: Arc::new(self.memos.clone_typed_tree_memos()),
                call_frame_tables: self.call_frame_tables.clone(),
            }
        }

        pub fn clone_with_expression_intervals(
            &self,
            intervals: Arc<[crate::ExpressionInterval]>,
        ) -> CheckingContext<'program> {
            CheckingContext {
                defer_layout_geometry: self.defer_layout_geometry,
                program: self.program,
                external_roots: self.external_roots,
                retained_borrows: self.retained_borrows.clone(),
                finalized_selections: self.finalized_selections,
                prepared_effects: self.prepared_effects,
                storage_graph: self.storage_graph.clone(),
                expression_intervals: OnceLock::from(intervals),
                prepared_mathematics: self.prepared_mathematics,
                opaque_selections: self.opaque_selections,
                memos: Arc::new(self.memos.clone_typed_tree_memos()),
                call_frame_tables: self.call_frame_tables.clone(),
            }
        }

        pub fn with_prepared_effect_plans(mut self, plans: &'program PreparedEffectPlans) -> Self {
            self.prepared_effects = Some(plans);
            if !self.memos.matches_effect_plans(plans) {
                self.reset_context_sensitive_memos();
            }
            self
        }

        pub fn clone_with_prepared_effect_plans<'context>(
            &self,
            plans: &'context PreparedEffectPlans,
        ) -> CheckingContext<'context>
        where
            'program: 'context,
        {
            let call_frame_tables = OnceLock::new();
            if let Some(tables) = self.call_frame_tables.get() {
                let _ = call_frame_tables.set(tables.clone());
            }
            let memos = if self.memos.matches_effect_plans(plans) {
                self.memos.clone()
            } else {
                Arc::new(self.memos.clone_typed_tree_memos())
            };
            CheckingContext {
                defer_layout_geometry: self.defer_layout_geometry,
                program: self.program,
                external_roots: self.external_roots,
                retained_borrows: self.retained_borrows.clone(),
                finalized_selections: self.finalized_selections,
                prepared_effects: Some(plans),
                storage_graph: self.storage_graph.clone(),
                expression_intervals: self.expression_intervals.clone(),
                prepared_mathematics: self.prepared_mathematics,
                opaque_selections: self.opaque_selections,
                memos,
                call_frame_tables,
            }
        }

        pub fn with_prepared_mathematics(
            mut self,
            mathematics: Option<
                crate::mathematical_applications::PreparedMathematicalView<'program>,
            >,
        ) -> Self {
            self.prepared_mathematics = mathematics;
            self.reset_context_sensitive_memos();
            self
        }

        pub fn with_opaque_representation_selections(
            mut self,
            selections: &'program [OpaqueRepresentationSelection],
        ) -> Self {
            self.opaque_selections = Some(selections);
            self.reset_context_sensitive_memos();
            self
        }
    }

    impl fmt::Debug for CheckingContext<'_> {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("CheckingContext")
                .field("program", &self.program)
                .finish_non_exhaustive()
        }
    }

    impl Deref for CheckingContext<'_> {
        type Target = TypedTrees;

        fn deref(&self) -> &TypedTrees {
            self.program
        }
    }

    impl CheckingInput for CheckingContext<'_> {
        fn external_invocation_roots(&self) -> Option<&[symbols::SymbolHandle]> {
            self.external_roots
        }

        fn defers_pending_layout_geometry(&self) -> bool {
            self.defer_layout_geometry
        }
        fn retained_borrow_custodies(&self) -> &[checked_trees::RetainedBorrowCustodyFact] {
            &self.retained_borrows
        }

        fn typed_program(&self) -> &TypedTrees {
            self.program
        }

        fn authored_declaration_selections(&self) -> &AuthoredDeclarationSelections {
            self.finalized_selections
                .unwrap_or_else(|| self.program.authored_declaration_selections())
        }

        fn checking_memos(&self) -> Option<&CheckingMemos> {
            Some(&self.memos)
        }

        fn cut_reads(&self) -> u64 {
            0
        }

        fn shared_memos(&self) -> Option<&CheckingMemos> {
            Some(&self.memos)
        }

        fn call_frame_tables(
            &self,
        ) -> Option<Arc<crate::machine_calls::calls::CallFrameTables<'_>>> {
            self.call_frame_tables
                .get_or_init(|| {
                    let mut tables =
                        crate::machine_calls::calls::CallFrameTables::build(self.program)?;
                    tables.symbols.caller_sites = crate::frozen_program::frozen_memo(
                        self,
                        |memos| &memos.caller_sites,
                        crate::declarations::symbols::CallerSiteCaches::default,
                    );
                    Some(Arc::new(tables))
                })
                .clone()
        }

        fn prepared_effect_plans(&self) -> Option<&PreparedEffectPlans> {
            self.prepared_effects
        }

        fn storage_graph(&self) -> Option<&checked_trees::FlowControlFacts> {
            self.storage_graph.as_ref().map(|graph| &graph.control)
        }

        fn expression_interval(
            &self,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> Option<crate::ExpressionInterval> {
            crate::expression_interval(self.expression_intervals.get()?, expression).copied()
        }

        fn prepared_mathematics(
            &self,
        ) -> Option<crate::mathematical_applications::PreparedMathematicalView<'_>> {
            self.prepared_mathematics
        }

        fn opaque_representation_selections(&self) -> &[OpaqueRepresentationSelection] {
            self.opaque_selections.unwrap_or(&[])
        }
    }

    #[derive(Debug)]
    pub struct CheckingProjection<'input> {
        original: &'input dyn CheckingInput,
        projected: Option<TypedTrees>,
    }

    impl<'input> CheckingProjection<'input> {
        pub fn new(original: &'input dyn CheckingInput) -> Self {
            Self {
                original,
                projected: None,
            }
        }

        pub fn program_mut(&mut self) -> &mut TypedTrees {
            self.projected.get_or_insert_with(|| {
                let mut projected = self.original.typed_program().clone();
                projected.retain_authored_declaration_selections(
                    self.original.authored_declaration_selections().clone(),
                );
                projected
            })
        }

        fn memo_source(&self) -> Option<&'input dyn CheckingInput> {
            self.projected.is_none().then_some(self.original)
        }
    }

    impl Deref for CheckingProjection<'_> {
        type Target = TypedTrees;

        fn deref(&self) -> &TypedTrees {
            self.typed_program()
        }
    }

    impl CheckingInput for CheckingProjection<'_> {
        fn external_invocation_roots(&self) -> Option<&[symbols::SymbolHandle]> {
            self.original.external_invocation_roots()
        }

        fn has_active_recast_formation(
            &self,
            expression: typed_trees::expression::ExpressionHandle,
            depth: usize,
        ) -> bool {
            self.memo_source().map_or(depth >= 64, |input| {
                input.has_active_recast_formation(expression, depth)
            })
        }

        fn has_active_completed_case_population(&self, machine: symbols::SymbolHandle) -> bool {
            self.memo_source()
                .is_some_and(|input| input.has_active_completed_case_population(machine))
        }

        fn has_active_operand_fit_population(&self, machine: symbols::SymbolHandle) -> bool {
            self.memo_source()
                .is_some_and(|input| input.has_active_operand_fit_population(machine))
        }

        fn has_active_numeric_observation(
            &self,
            machine: symbols::SymbolHandle,
            state: symbols::SymbolHandle,
        ) -> bool {
            self.memo_source()
                .is_some_and(|input| input.has_active_numeric_observation(machine, state))
        }

        fn defers_pending_layout_geometry(&self) -> bool {
            self.original.defers_pending_layout_geometry()
        }
        fn retained_borrow_custodies(&self) -> &[checked_trees::RetainedBorrowCustodyFact] {
            self.original.retained_borrow_custodies()
        }

        fn proof_only_classification(&self) -> Arc<ProofOnlyClassification> {
            match &self.projected {
                Some(projected) => projected.proof_only_classification(),
                None => self.original.proof_only_classification(),
            }
        }

        fn layout_target(&self) -> Option<&str> {
            self.original.layout_target()
        }

        fn typed_program(&self) -> &TypedTrees {
            self.projected
                .as_ref()
                .unwrap_or_else(|| self.original.typed_program())
        }

        fn authored_declaration_selections(&self) -> &AuthoredDeclarationSelections {
            match &self.projected {
                Some(projected) => projected.authored_declaration_selections(),
                None => self.original.authored_declaration_selections(),
            }
        }

        fn checking_memos(&self) -> Option<&CheckingMemos> {
            self.memo_source()?.checking_memos()
        }

        fn cut_reads(&self) -> u64 {
            self.memo_source().map_or(0, CheckingInput::cut_reads)
        }

        fn shared_memos(&self) -> Option<&CheckingMemos> {
            self.memo_source()?.shared_memos()
        }

        fn prepared_effect_plans(&self) -> Option<&PreparedEffectPlans> {
            self.memo_source()?.prepared_effect_plans()
        }

        fn storage_graph(&self) -> Option<&checked_trees::FlowControlFacts> {
            self.memo_source()?.storage_graph()
        }

        fn expression_interval(
            &self,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> Option<crate::ExpressionInterval> {
            self.memo_source()?.expression_interval(expression)
        }

        fn call_frame_tables(
            &self,
        ) -> Option<Arc<crate::machine_calls::calls::CallFrameTables<'_>>> {
            self.memo_source()?.call_frame_tables()
        }

        fn prepared_mathematics(
            &self,
        ) -> Option<crate::mathematical_applications::PreparedMathematicalView<'_>> {
            self.memo_source()?.prepared_mathematics()
        }

        fn opaque_representation_selections(&self) -> &[OpaqueRepresentationSelection] {
            self.original.opaque_representation_selections()
        }
    }

    #[doc(hidden)]
    #[derive(Default)]
    pub struct CheckingMemos {
        /// Naming and match-prefix associations depend only on frozen typed
        /// tables, never changing flow or numeric facts.
        pub(crate) projection_syntax:
            OnceLock<Arc<crate::value_custody::places::ProjectionSyntaxQueries>>,
    pub(crate) case_guarantee_eligibility: OnceLock<Arc<CaseGuaranteeEligibility>>,
    pub(crate) provider_applications: OnceLock<Arc<Vec<crate::ProviderApplicationAssociation>>>,
    pub(crate) indexed_logical_machines: OnceLock<Arc<Vec<symbols::SymbolHandle>>>,
    pub(crate) structural_descent_components:
        OnceLock<Arc<crate::machine_calls::call_cycles::StructuralDescentComponents>>,
    pub(crate) requirement_satisfiers:
        OnceLock<Arc<crate::machine_calls::denotational_calls::RequirementSatisfierIndex>>,
    pub(crate) erased_initializers: OnceLock<Arc<Vec<typed_trees::expression::ExpressionHandle>>>,
    pub(crate) mathematical_expression_nodes:
        OnceLock<Arc<Vec<typed_trees::expression::ExpressionHandle>>>,
    pub(crate) owned_field_domains:
        OnceLock<crate::value_custody::field_domains::OwnedFieldDomainQueries>,
    pub(crate) declared_type_domains:
        OnceLock<crate::value_custody::field_domains::DeclaredTypeDomainQueries>,
    pub(crate) claim_frontiers:
        OnceLock<crate::value_custody::type_properties::ClaimFrontierQueries>,
    pub(crate) establishment: OnceLock<crate::value_custody::type_properties::EstablishmentQueries>,
    pub(crate) cleanup_types: OnceLock<crate::value_custody::cleanup::CleanupTypeQueries>,
    pub(crate) call_destinations:
        OnceLock<crate::machine_calls::calls::signature_queries::CallDestinationQueries>,
    pub(crate) operational: OnceLock<Arc<flow_effects::OperationalPlan>>,
    pub(crate) service_reaches: OnceLock<(
        flow_effects::OperationalPlan,
        flow_effects::ServiceReachInferencePlan,
    )>,
    pub(crate) call_frames: Arc<CallFrameCaches>,
    pub(crate) numeric_observations:
        crate::proof_contracts::contract_entailment::numeric_observations::NumericObservationTable,
    pub(crate) operand_fit_outcomes: Mutex<
        symbols::SymbolKeyMap<
            (
                symbols::SymbolHandle,
                typed_trees::expression::ExpressionHandle,
            ),
            Option<crate::proof_contracts::contract_entailment::OperandFitOutcome>,
        >,
    >,
    pub(crate) operand_fit_populated:
        Mutex<symbols::SymbolKeyMap<symbols::SymbolHandle, ()>>,
    pub(crate) completed_case_outcomes: Mutex<
        symbols::SymbolKeyMap<
            (
                symbols::SymbolHandle,
                symbols::SymbolHandle,
                usize,
                typed_trees::expression::ExpressionHandle,
                typed_trees::types::TypeReferenceHandle,
            ),
            Option<symbols::SymbolHandle>,
        >,
    >,
    pub(crate) completed_case_populated:
        Mutex<symbols::SymbolKeyMap<symbols::SymbolHandle, ()>>,
    proof_only: OnceLock<Arc<ProofOnlyClassification>>,
    pub(crate) license_candidates: OnceLock<Arc<LicenseCandidates>>,
    pub(crate) entry_machines: OnceLock<Arc<EntryMachines>>,
    pub(crate) caller_sites: OnceLock<Arc<CallerSiteCaches>>,
    pub(crate) content_conservation: OnceLock<Arc<ContentConservationInputs>>,
    pub(crate) admitted_dependency_bodies: Mutex<symbols::SymbolKeyMap<symbols::SymbolHandle, ()>>,
    pub(crate) shared_proof_walks: Mutex<
        symbols::SymbolKeyMap<
            symbols::SymbolHandle,
            Arc<crate::proof_contracts::contract_entailment::shared_proof_walks::SharedProofWalks>,
        >,
    >,
    pub(crate) building_shared_proof_walks: Mutex<
        symbols::SymbolKeyMap<
            crate::proof_contracts::contract_entailment::shared_proof_walks::BuildingWalk,
            (),
        >,
    >,
    pub(crate) normal_return_intervals: Mutex<
        symbols::SymbolKeyMap<
            (
                symbols::SymbolHandle,
                symbols::SymbolHandle,
                u8,
                Vec<(symbols::SymbolHandle, crate::proof_contracts::arithmetic_domains::Interval)>,
            ),
            Option<(
                crate::proof_contracts::arithmetic_domains::Interval,
                Option<checked_trees::IntegerRangeRecipe>,
            )>,
        >,
    >,
    pub(crate) completed_collection_lengths: Mutex<
        symbols::SymbolKeyMap<
            (
                symbols::SymbolHandle,
                symbols::SymbolHandle,
                typed_trees::expression::ExpressionHandle,
            ),
            u64,
        >,
    >,
    pub(crate) active_collection_lengths: Mutex<
        symbols::SymbolKeyMap<
            (
                symbols::SymbolHandle,
                symbols::SymbolHandle,
                typed_trees::expression::ExpressionHandle,
                std::thread::ThreadId,
            ),
            (),
        >,
    >,
    pub(crate) binary_meanings:
        Mutex<symbols::SymbolKeyMap<(u32, u32, typed_trees::expression::ExpressionHandle), bool>>,
    pub(crate) bound_subtree_meanings: Mutex<
        symbols::SymbolKeyMap<(u32, u32, typed_trees::expression::ExpressionHandle), Option<u8>>,
    >,
    pub(crate) expression_result_types: std::sync::RwLock<
        symbols::SymbolKeyMap<
            (u32, u32, typed_trees::expression::ExpressionHandle),
            Option<typed_trees::types::TypeReferenceHandle>,
        >,
    >,
    /// A parameter telescope's principal results depend on its owner,
    /// formal roster and exact supplying application, including phantom
    /// operands of nullary calls. Equal complete contexts share one dense id.
    /// Proof intake rebuilds the same roster for every observed value; without
    /// the id, a deep `||`/`&&` predicate retypes each subtree at every node.
    pub(crate) parameter_telescopes: Mutex<
        symbols::SymbolKeyMap<
            (
                symbols::SymbolHandle,
                Vec<(
                    symbols::SymbolHandle,
                    typed_trees::types::TypeReferenceHandle,
                )>,
                Option<typed_trees::trait_definition::RetainedTraitScope>,
            ),
            u32,
        >,
    >,
    pub(crate) parameter_result_types: std::sync::RwLock<
        symbols::SymbolKeyMap<
            (u32, typed_trees::expression::ExpressionHandle),
            Option<typed_trees::types::TypeReferenceHandle>,
        >,
    >,
    pub(crate) effective_result_references: Mutex<
        std::collections::HashMap<
            String,
            Option<typed_trees::types::TypeReferenceHandle>,
        >,
    >,
    pub(crate) ordered_integer_meanings: Mutex<
        symbols::SymbolKeyMap<
            (u32, u32, typed_trees::expression::ExpressionHandle),
            Option<(
                typed_trees::types::PrimitiveType,
                numerics::arithmetic::ArithmeticDomain,
            )>,
        >,
    >,
    pub(crate) parameter_requires_intervals: Mutex<
        symbols::SymbolKeyMap<
            (u32, u32, symbols::SymbolHandle, bool),
            crate::proof_contracts::arithmetic_domains::Interval,
        >,
    >,
    pub(crate) case_reference_owners:
        Mutex<symbols::SymbolKeyMap<typed_trees::expression::ExpressionHandle, Option<usize>>>,
    pub(crate) checked_value_call_candidates: Mutex<
        symbols::SymbolKeyMap<
            (symbols::SymbolHandle, symbols::SymbolHandle),
            Result<(), &'static str>,
        >,
    >,
    pub(crate) nominal_cleanup_owners: Mutex<symbols::SymbolMap<Option<symbols::SymbolHandle>>>,
    pub(crate) selected_application_carriers: Mutex<
        symbols::SymbolKeyMap<
            (
                symbols::SymbolHandle,
                symbols::SymbolHandle,
                typed_trees::types::TypeReferenceHandle,
                usize,
            ),
            SelectedCarrierVerdict,
        >,
    >,
    pub(crate) contract_occurrences:
        OnceLock<Arc<crate::proof_contracts::contract_results::ContractOccurrenceIndex>>,
    pub(crate) bound_maps:
        OnceLock<Arc<crate::proof_contracts::immutable_integer_bounds::BoundMaps>>,
    pub(crate) incoming_guard_environments: Mutex<
        symbols::SymbolMap<
            Arc<
                Vec<(
                    symbols::SymbolHandle,
                    crate::proof_contracts::arithmetic_domains::ValueEnvironment,
                )>,
            >,
        >,
    >,
    pub(crate) returned_field_projections: Mutex<
        symbols::SymbolMap<
            Arc<(
                Vec<(
                    Vec<facts::PlaceSegment>,
                    crate::proof_contracts::contract_entailment::structural_judgment::StructuralTerm,
                )>,
                Vec<symbols::SymbolHandle>,
            )>,
        >,
    >,
    pub(crate) declared_float_field_bounds: Mutex<
        symbols::SymbolKeyMap<
            typed_trees::types::TypeReferenceHandle,
            Vec<crate::DeclaredFloatFieldBound>,
        >,
    >,
    pub(crate) trapping_receipt_demand: OnceLock<Arc<crate::proof_contracts::arithmetic_domains::TrappingReceiptDemand>>,
    pub(crate) private_arrival_index: OnceLock<
        Arc<
            crate::proof_contracts::contract_entailment::structural_prefix::ProgramPrivateArrivalIndex,
        >,
    >,
    pub(crate) private_closed_arrivals: Mutex<
        symbols::SymbolKeyMap<
            (symbols::SymbolHandle, symbols::SymbolHandle),
            Result<
                std::sync::Arc<
                    crate::proof_contracts::contract_entailment::structural_prefix::ClosedArrivals,
                >,
                &'static str,
            >,
        >,
    >,
    pub(crate) state_prefix_reachability_edges:
        Mutex<symbols::SymbolKeyMap<symbols::SymbolHandle, Option<Vec<symbols::SymbolHandle>>>>,
    pub(crate) state_prefix_cycle_membership:
        Mutex<symbols::SymbolKeyMap<symbols::SymbolHandle, bool>>,
    pub(crate) normal_completion_possible:
        Mutex<symbols::SymbolKeyMap<symbols::SymbolHandle, Option<bool>>>,
    }

    impl CheckingMemos {
        fn matches_effect_plans(&self, plans: &PreparedEffectPlans) -> bool {
            self.operational
                .get()
                .is_some_and(|operational| **operational == plans.operational)
                && self
                    .service_reaches
                    .get()
                    .is_some_and(|(operational, service_reaches)| {
                        operational == &plans.operational
                            && service_reaches == &plans.service_reaches
                    })
        }

        fn retain_typed_tree_memos(self) -> Self {
            Self {
                projection_syntax: self.projection_syntax,
                requirement_satisfiers: self.requirement_satisfiers,
                erased_initializers: self.erased_initializers,
                mathematical_expression_nodes: self.mathematical_expression_nodes,
                owned_field_domains: self.owned_field_domains,
                declared_type_domains: self.declared_type_domains,
                claim_frontiers: self.claim_frontiers,
                establishment: self.establishment,
                call_destinations: self.call_destinations,
                proof_only: self.proof_only,
                license_candidates: self.license_candidates,
                entry_machines: self.entry_machines,
                caller_sites: self.caller_sites,
                content_conservation: self.content_conservation,
                case_reference_owners: self.case_reference_owners,
                nominal_cleanup_owners: self.nominal_cleanup_owners,
                contract_occurrences: self.contract_occurrences,
                bound_maps: self.bound_maps,
                declared_float_field_bounds: self.declared_float_field_bounds,
                private_arrival_index: self.private_arrival_index,
                private_closed_arrivals: self.private_closed_arrivals,
                state_prefix_reachability_edges: self.state_prefix_reachability_edges,
                state_prefix_cycle_membership: self.state_prefix_cycle_membership,
                normal_completion_possible: self.normal_completion_possible,
                admitted_dependency_bodies: self.admitted_dependency_bodies,
                ..Self::default()
            }
        }

        pub(crate) fn query_scratch(&self) -> Self {
            Self {
                caller_sites: OnceLock::new(),
                ..self.clone_typed_tree_memos()
            }
        }

        fn clone_typed_tree_memos(&self) -> Self {
            Self {
                projection_syntax: self.projection_syntax.clone(),
                requirement_satisfiers: self.requirement_satisfiers.clone(),
                erased_initializers: self.erased_initializers.clone(),
                mathematical_expression_nodes: self.mathematical_expression_nodes.clone(),
                proof_only: self.proof_only.clone(),
                license_candidates: self.license_candidates.clone(),
                entry_machines: self.entry_machines.clone(),
                caller_sites: self.caller_sites.clone(),
                content_conservation: self.content_conservation.clone(),
                case_reference_owners: Mutex::new(
                    self.case_reference_owners
                        .lock()
                        .map(|guard| guard.clone())
                        .unwrap_or_default(),
                ),
                nominal_cleanup_owners: Mutex::new(
                    self.nominal_cleanup_owners
                        .lock()
                        .map(|guard| guard.clone())
                        .unwrap_or_default(),
                ),
                contract_occurrences: self.contract_occurrences.clone(),
                bound_maps: self.bound_maps.clone(),
                declared_float_field_bounds: Mutex::new(
                    self.declared_float_field_bounds
                        .lock()
                        .map(|guard| guard.clone())
                        .unwrap_or_default(),
                ),
                trapping_receipt_demand: self.trapping_receipt_demand.clone(),
                private_arrival_index: self.private_arrival_index.clone(),
                private_closed_arrivals: Mutex::new(
                    self.private_closed_arrivals
                        .lock()
                        .map(|guard| guard.clone())
                        .unwrap_or_default(),
                ),
                state_prefix_reachability_edges: Mutex::new(
                    self.state_prefix_reachability_edges
                        .lock()
                        .map(|guard| guard.clone())
                        .unwrap_or_default(),
                ),
                state_prefix_cycle_membership: Mutex::new(
                    self.state_prefix_cycle_membership
                        .lock()
                        .map(|guard| guard.clone())
                        .unwrap_or_default(),
                ),
                normal_completion_possible: Mutex::new(
                    self.normal_completion_possible
                        .lock()
                        .map(|guard| guard.clone())
                        .unwrap_or_default(),
                ),
                admitted_dependency_bodies: Mutex::new(
                    self.admitted_dependency_bodies
                        .lock()
                        .map(|guard| guard.clone())
                        .unwrap_or_default(),
                ),
                ..Self::default()
            }
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) enum SelectedCarrierVerdict {
        Concrete(symbols::SymbolHandle, symbols::SymbolHandle),
        Operator(symbols::SymbolHandle),
        NoCarrier,
    }

    pub(crate) fn frozen_program_memos(program: &dyn CheckingInput) -> Option<&CheckingMemos> {
        program.checking_memos()
    }

    pub(crate) struct CutGuard<'a> {
        program: &'a dyn CheckingInput,
    }

    impl<'a> CutGuard<'a> {
        pub(crate) fn start(program: &'a dyn CheckingInput) -> Self {
            Self { program }
        }

        pub(crate) fn memos(&self) -> Option<&'a CheckingMemos> {
            if self.program.cut_reads() == 0 {
                self.program.shared_memos()
            } else {
                self.program.checking_memos()
            }
        }
    }

    pub(crate) fn readable_memos(
        program: &dyn CheckingInput,
    ) -> impl Iterator<Item = &CheckingMemos> {
        let shared = program.shared_memos();
        let own = program
            .checking_memos()
            .filter(|own| !shared.is_some_and(|shared| std::ptr::eq(shared, *own)));
        shared.into_iter().chain(own)
    }

    pub(crate) fn frozen_memo<T>(
        program: &dyn CheckingInput,
        slot: impl FnOnce(&CheckingMemos) -> &OnceLock<Arc<T>>,
        compute: impl FnOnce() -> T,
    ) -> Arc<T> {
        match program.checking_memos() {
            Some(memos) => slot(memos).get_or_init(|| Arc::new(compute())).clone(),
            None => Arc::new(compute()),
        }
    }

    pub fn proof_only_classification(program: &dyn CheckingInput) -> Arc<ProofOnlyClassification> {
        program.proof_only_classification()
    }

    #[cfg(test)]
    mod cleanup_query_tests {
        use super::*;
        use crate::value_custody::cleanup::has_no_owned_cleanup_obligations;
        use typed_trees::types::TypeReferenceNode;

        #[test]
        fn cleanup_queries_reset_for_unique_and_shared_context_substitutions() {
            let mut program = TypedTrees::default();
            let reference = program.type_reference_table.insert(TypeReferenceNode::Unit);
            for share_previous in [false, true] {
                let context = CheckingContext::new(&program);
                let answer = has_no_owned_cleanup_obligations(&context, reference);
                assert!(context.memos.cleanup_types.get().is_some());
                let previous = share_previous.then(|| context.memos.clone());

                let substituted = context.with_prepared_mathematics(None);
                assert!(substituted.memos.cleanup_types.get().is_none());
                assert_eq!(
                    has_no_owned_cleanup_obligations(&substituted, reference),
                    answer
                );
                if let Some(previous) = previous {
                    assert!(previous.cleanup_types.get().is_some());
                }
            }
        }

        #[test]
        fn cleanup_queries_do_not_cross_borrow_contexts_or_bare_inputs() {
            let mut program = TypedTrees::default();
            let reference = program.type_reference_table.insert(TypeReferenceNode::Unit);
            assert!(program.checking_memos().is_none());
            let uncached = has_no_owned_cleanup_obligations(&program, reference);
            let context = CheckingContext::new(&program);
            assert!(context.memos.cleanup_types.get().is_none());
            assert_eq!(
                has_no_owned_cleanup_obligations(&context, reference),
                uncached
            );

            let substituted = context.with_retained_borrow_custodies(Arc::from([]));
            assert!(substituted.memos.cleanup_types.get().is_none());
            assert_eq!(
                has_no_owned_cleanup_obligations(&substituted, reference),
                uncached
            );
        }
    }
}

pub mod queries {
    pub mod execution_projection {
        use crate::frozen_program::expression_table;
        use std::collections::HashSet;
        use std::sync::Arc;

        use crate::compiler_operations::static_argument_role;
        use diagnostics::Diagnostic;
        use source::{DependencyScope, SourceOrigin};
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;
        use typed_trees::machine::Machine;
        use typed_trees::references::{
            DeclarationSelection, StaticArgumentClassifier, collect_program_references_with_roles,
            collect_selected_program_references,
        };

        /*
            Project the program a Build activation checks and executes
            (wiki/spec/build/validation_subjects.md#orchestration). Its roots are
            the Build entry, the declarations the activation selects, and every
            Build-scope declaration authored in a subject source; everything
            else enters only as their closure. Executing a dependency's Build
            (the toolchain core Build that settles provider defaults on every
            compile, for example) therefore checks what that Build reaches and
            never promotes the dependency's other Build-scope declarations to
            subjects.
        */
        pub fn project_build_program(
            original: &dyn crate::CheckingInput,
            entry: SymbolHandle,
            selected_declarations: &[SymbolHandle],
            is_subject: impl Fn(&source::SourceFile) -> bool,
        ) -> Result<TypedTrees, Vec<Diagnostic>> {
            let instances = instantiated_declarations(original);
            let is_generated = |symbol| is_derived_declaration(original, &instances, symbol);
            let in_build_context = |symbol| {
                original
                    .symbols
                    .symbol_provenance_source_span(symbol)
                    .and_then(|span| original.symbols.source_file(span))
                    .is_some_and(|file| {
                        file.origin == SourceOrigin::Toolchain
                            || file.dependency_scope == DependencyScope::Build
                    })
            };
            if !original.machine(entry).is_some() || !in_build_context(entry) {
                return Err(vec![Diagnostic::error(
                    "selected Build entry has no exact Build-context declaration custody",
                )]);
            }
            if let Some(symbol) = selected_declarations
                .iter()
                .find(|symbol| !in_build_context(**symbol))
            {
                return Err(vec![Diagnostic::error(format!(
                    "selected Build dependency `{}` is outside the Build source context",
                    original.symbols.display_path(*symbol, "::"),
                ))]);
            }
            let selected =
                DeclarationSelection::from_symbols(selected_declarations.iter().copied());
            let is_build_scope = |symbol| {
                symbol == entry
                    || original
                        .symbols
                        .symbol_provenance_source_span(symbol)
                        .and_then(|span| original.symbols.source_file(span))
                        .is_some_and(|file| file.dependency_scope == DependencyScope::Build)
            };
            let is_subject_build_declaration = |symbol| {
                symbol == entry
                    || original
                        .symbols
                        .symbol_provenance_source_span(symbol)
                        .and_then(|span| original.symbols.source_file(span))
                        .is_some_and(|file| {
                            file.dependency_scope == DependencyScope::Build && is_subject(file)
                        })
            };
            let generated_methods = original
                .machines()
                .iter()
                .filter(|machine| {
                    machine.attached_data_symbol.is_valid()
                        && (machine.generic_data_template.is_valid()
                            || !is_build_scope(machine.symbol))
                })
                .map(|machine| (machine.attached_data_symbol, machine.symbol))
                .collect::<Vec<_>>();
            let descriptors = build_descriptor_declarations(original);
            let mut program = project_declarations(
                original,
                |symbol| {
                    if selected.contains(symbol) {
                        return true;
                    }
                    is_subject_build_declaration(symbol)
                        && !is_generated(symbol)
                        && (original
                            .machine(symbol)
                            .is_some_and(|machine| machine.body_is_present)
                            || !descriptors.contains(&symbol))
                },
                |symbol| {
                    is_generated(symbol)
                        || in_build_context(symbol)
                        || original
                            .symbols
                            .symbol_provenance_source_span(symbol)
                            .is_none()
                },
                &generated_methods,
                Some(static_argument_role),
            );
            let typed = original.typed_program();
            program.product_declaration_frontier = Some(
                typed
                    .product_declaration_frontier
                    .as_ref()
                    .map(Arc::clone)
                    .unwrap_or_else(|| Arc::new(typed.clone())),
            );
            Ok(program)
        }

        pub fn project_source_program(
            original: &TypedTrees,
            sources: &source::SourceMap,
        ) -> TypedTrees {
            let instances = instantiated_declarations(original);
            let derived = |symbol| is_derived_declaration(original, &instances, symbol);
            let generated_methods = original
                .machines()
                .iter()
                .filter(|machine| machine.generic_data_template.is_valid())
                .map(|machine| (machine.attached_data_symbol, machine.symbol))
                .collect::<Vec<_>>();
            let mut program = project_declarations(
                original,
                |symbol| {
                    !derived(symbol)
                        && original
                            .symbols
                            .symbol_provenance_source_span(symbol)
                            .is_none_or(|span| sources.source_is_selected(span.source_id))
                },
                |symbol| {
                    derived(symbol)
                        || original
                            .symbols
                            .symbol_provenance_source_span(symbol)
                            .is_none()
                },
                &generated_methods,
                None,
            );
            let references = typed_trees::references::collect_program_references(&program);
            let mut excluded = std::mem::take(&mut program.excluded_authored_selection_occurrences)
                .into_occurrences();
            excluded.extend(
                original
                    .authored_declaration_selections()
                    .iter()
                    .filter_map(|selection| {
                        let span = selection.source_span();
                        (span.span.start != span.span.end
                            && !sources.source_is_selected(span.source_id)
                            && !references
                                .authored_selection_occurrences
                                .contains(&selection.occurrence_id()))
                        .then_some(selection.occurrence_id())
                    }),
            );
            let expressions = expression_table(original);
            program.expression_table.retain_expression_roots(|handle| {
                references.expressions.contains(&handle)
                    || expressions
                        .authored_selection_occurrences(handle)
                        .any(|occurrence| !excluded.contains(&occurrence))
            });
            program.excluded_authored_selection_occurrences = excluded.into();
            program
        }

        /*
            Narrow a product program to its validation subjects before checking
            (wiki/spec/build/validation_subjects.md). What stays: every
            declaration authored in a subject source, the declarations the Build
            demands (product roots and its selections), and whatever those
            reference. A dependency declaration nothing reaches is removed
            rather than left unchecked, so no later stage can rely on it.

            Dispatch chooses among conformances and methods after this point, so
            every conformance of a reached trait and every method of a reached
            data type stays. Operator resolution likewise chooses among every
            operator and token-bound machine (a primitive `==` on `f32` selects
            `Float::equal` by its token, not by a name the walker sees), so those
            stay as candidates too.
        */
        pub fn project_subject_program(
            original: &TypedTrees,
            is_subject: impl Fn(&source::SourceFile) -> bool,
            demanded: &[SymbolHandle],
        ) -> TypedTrees {
            let instances = instantiated_declarations(original);
            let mut demanded = demanded.iter().copied().collect::<HashSet<_>>();
            demanded.extend(
                original
                    .operators()
                    .iter()
                    .chain(original.machine_token_bindings())
                    .map(|operator| operator.symbol)
                    .filter(|symbol| symbol.is_valid()),
            );
            let domains = original
                .tables
                .domain_definitions
                .span_or_empty(original.roots.domain_definitions);
            // ponytail: keeps all conformances of a reached trait, not only the
            // selected ones. Narrow it once selection is known before checking.
            let children = original
                .machines()
                .iter()
                .filter(|machine| machine.attached_data_symbol.is_valid())
                .map(|machine| (machine.attached_data_symbol, machine.symbol))
                .chain(
                    original
                        .conformances()
                        .iter()
                        .map(|conformance| (conformance.trait_symbol, conformance.symbol)),
                )
                // A value's qualification names a domain of its data type
                // without a reference the walker follows, so a reached data
                // type keeps its domains.
                .chain(
                    domains
                        .iter()
                        .map(|domain| (domain.subject_symbol, domain.symbol)),
                )
                // A machine owned by a domain, such as its content projection
                // `Granted::content`, names that domain only by its path in the
                // same package; the checker finds it the same way. A reached
                // domain keeps those machines.
                .chain(
                    original
                        .machines()
                        .iter()
                        .filter(|machine| !machine.attached_data_symbol.is_valid())
                        .filter_map(|machine| {
                            let owner = machine.attached_data.as_ref()?.as_str();
                            Some((machine, owner.rsplit("::").next().unwrap_or(owner)))
                        })
                        .flat_map(|(machine, owner)| {
                            domains
                                .iter()
                                .filter(move |domain| {
                                    let name = domain.name.as_str();
                                    name.rsplit("::").next().unwrap_or(name) == owner
                                        && original.symbols.same_symbol_source_package(
                                            machine.symbol,
                                            domain.symbol,
                                        )
                                })
                                .map(move |domain| (domain.symbol, machine.symbol))
                        }),
                )
                .collect::<Vec<_>>();
            project_declarations(
                original,
                |symbol| {
                    demanded.contains(&symbol)
                        || (!is_derived_declaration(original, &instances, symbol)
                            && original
                                .symbols
                                .symbol_provenance_source_span(symbol)
                                .and_then(|span| original.symbols.source_file(span))
                                .is_none_or(&is_subject))
                },
                |_| true,
                &children,
                None,
            )
        }

        fn instantiated_declarations(program: &dyn crate::CheckingInput) -> HashSet<SymbolHandle> {
            let mut instances = program
                .data_definitions()
                .iter()
                .filter(|data| data.generic_instance.is_some())
                .map(|data| data.symbol)
                .collect::<HashSet<_>>();
            instances.extend(
                program
                    .machines()
                    .iter()
                    .filter(|machine| machine.generic_data_template.is_valid())
                    .map(|machine| machine.symbol),
            );
            instances.extend(
                program
                    .machine_specializations
                    .iter()
                    .map(|specialization| specialization.instance),
            );
            instances
        }

        fn is_derived_declaration(
            program: &dyn crate::CheckingInput,
            instances: &HashSet<SymbolHandle>,
            mut symbol: SymbolHandle,
        ) -> bool {
            while symbol.is_valid() {
                let declaration = program.symbols.get(symbol);
                if instances.contains(&symbol) || declaration.generated_from.is_valid() {
                    return true;
                }
                symbol = declaration.parent;
            }
            false
        }

        fn build_descriptor_declarations(
            program: &dyn crate::CheckingInput,
        ) -> HashSet<SymbolHandle> {
            let references = collect_program_references_with_roles(
                program,
                &DeclarationSelection::all(),
                Some(static_argument_role),
            );
            let mut pending = references
                .descriptor_symbols
                .into_iter()
                .collect::<Vec<_>>();
            pending.extend(
                program
                    .conformances()
                    .iter()
                    .map(|conformance| conformance.symbol),
            );
            let mut descriptors = HashSet::new();
            while !pending.is_empty() {
                let mut roots = Vec::new();
                for mut symbol in pending.drain(..) {
                    while symbol.is_valid() && descriptors.insert(symbol) {
                        roots.push(symbol);
                        symbol = program.symbols.get(symbol).parent;
                    }
                }
                if roots.is_empty() {
                    break;
                }
                pending.extend(
                    collect_selected_program_references(
                        program,
                        &DeclarationSelection::from_symbols(roots),
                    )
                    .symbols,
                );
            }
            descriptors
        }

        pub fn project_selected_program(
            original: &dyn crate::CheckingInput,
            selection: &typed_trees::references::DeclarationSelection,
        ) -> TypedTrees {
            if selection.is_all() {
                return original.typed_program().clone();
            }
            project_declarations(
                original,
                |symbol| selection.contains(symbol),
                |symbol| {
                    original
                        .symbols
                        .symbol_provenance_source_span(symbol)
                        .is_none()
                },
                &[],
                None,
            )
        }

        fn project_declarations(
            original: &dyn crate::CheckingInput,
            selected_root: impl Fn(SymbolHandle) -> bool,
            retain_reference: impl Fn(SymbolHandle) -> bool,
            declaration_children: &[(SymbolHandle, SymbolHandle)],
            classify_static_arguments: Option<StaticArgumentClassifier>,
        ) -> TypedTrees {
            let mut program = original.typed_program().clone();
            program.identity = Default::default();

            let mut synthesized = HashSet::new();
            let mut removed_declarations = HashSet::new();
            loop {
                let selected = |symbol| selected_root(symbol) || synthesized.contains(&symbol);
                removed_declarations.clear();
                macro_rules! project_roots {
                    ($field:ident, $table:ident) => {{
                        let declarations =
                            original.tables.$table.span_or_empty(original.roots.$field);
                        removed_declarations.extend(
                            declarations
                                .iter()
                                .filter(|declaration| !selected(declaration.symbol))
                                .map(|declaration| declaration.symbol),
                        );
                        let roots = declarations
                            .iter()
                            .filter(|declaration| selected(declaration.symbol))
                            .cloned()
                            .collect::<Vec<_>>();
                        program.roots.$field = program.tables.$table.insert_many(roots);
                    }};
                }
                project_roots!(const_declarations, const_declarations);
                project_roots!(data_definitions, data_definitions);
                project_roots!(type_aliases, type_aliases);
                project_roots!(domain_definitions, domain_definitions);
                project_roots!(measures, measures);
                project_roots!(operators, operators);
                project_roots!(machine_token_bindings, operators);
                project_roots!(propositions, propositions);
                project_roots!(mathematical_definitions, mathematical_definitions);
                project_roots!(traits, traits);
                project_roots!(conformances, conformances);
                project_roots!(wire_schemas, wire_schemas);
                let machines = original
                    .machines()
                    .iter()
                    .filter(|machine| selected(machine.symbol))
                    .cloned()
                    .collect::<Vec<_>>();
                program.roots.machines = program.tables.machines.insert_many(machines);

                let references = collect_program_references_with_roles(
                    original.typed_program(),
                    &DeclarationSelection::from_program(&program),
                    classify_static_arguments,
                );
                let before = synthesized.len();
                for mut symbol in references.symbols {
                    while symbol.is_valid() && retain_reference(symbol) {
                        synthesized.insert(symbol);
                        symbol = original.symbols.get(symbol).parent;
                    }
                }
                synthesized.extend(references.descriptor_symbols.into_iter().filter(|symbol| {
                    symbol.is_valid()
                        && original.symbols.get(*symbol).kind == symbols::SymbolKind::TypeAlias
                }));
                for &(owner, child) in declaration_children {
                    if selected_root(owner) || synthesized.contains(&owner) {
                        synthesized.insert(child);
                    }
                }
                for declaration in original.const_declarations() {
                    if !original
                        .tables
                        .machines
                        .is_valid(declaration.evaluation_entry)
                    {
                        continue;
                    }
                    let entry = original
                        .tables
                        .machines
                        .get(declaration.evaluation_entry)
                        .symbol;
                    if selected_root(entry) || synthesized.contains(&entry) {
                        synthesized.insert(declaration.symbol);
                    }
                }
                if synthesized.len() == before {
                    break;
                }
            }
            let retained = program
                .machines()
                .iter()
                .map(|machine| machine.symbol)
                .collect::<HashSet<_>>();
            let removed = original
                .machines()
                .iter()
                .map(|machine| machine.symbol)
                .filter(|symbol| !retained.contains(symbol))
                .collect::<HashSet<_>>();
            removed_declarations.extend(removed.iter().copied());
            retain_machine_roster(&mut program, &retained, &removed);
            apply_declaration_sidecar_pruning(&mut program, &removed_declarations);
            let original_references = typed_trees::references::collect_program_references(original);
            let selected_references = typed_trees::references::collect_program_references(&program);
            let mut excluded_occurrences =
                std::mem::take(&mut program.excluded_authored_selection_occurrences)
                    .into_occurrences();
            excluded_occurrences.retain(|occurrence| {
                !selected_references
                    .authored_selection_occurrences
                    .contains(occurrence)
            });
            excluded_occurrences.extend(
                original_references
                    .authored_selection_occurrences
                    .difference(&selected_references.authored_selection_occurrences)
                    .copied(),
            );
            program.excluded_authored_selection_occurrences = excluded_occurrences.into();
            let expressions = selected_references.expressions;
            let excluded = &program.excluded_authored_selection_occurrences;
            let orphan_obligations = expression_table(original)
                .iter_expressions()
                .map(|(handle, _)| handle)
                .filter(|handle| {
                    !original_references.expressions.contains(handle)
                        && expression_table(original)
                            .authored_selection_occurrences(*handle)
                            .any(|occurrence| !excluded.contains(occurrence))
                })
                .collect::<HashSet<_>>();
            program.expression_table.retain_expression_roots(|handle| {
                expressions.contains(&handle) || orphan_obligations.contains(&handle)
            });
            program.target_machine_families.retain_mut(|family| {
                family
                    .members
                    .retain(|member| retained.contains(&member.machine));
                !family.members.is_empty()
            });
            program
        }

        pub fn descriptor_selection_occurrences(
            program: &dyn crate::CheckingInput,
        ) -> HashSet<typed_trees::AuthoredDeclarationSelectionOccurrenceId> {
            let mut occurrences = product_request_operand_occurrences(program);
            occurrences.extend(
                program
                    .authored_declaration_selections()
                    .iter()
                    .filter(|selection| !program.checks_authored_selection(selection))
                    .map(|selection| selection.occurrence_id()),
            );
            let materialized = materialized_product_operand_occurrences(program);
            occurrences.retain(|occurrence| !materialized.contains(occurrence));
            occurrences
        }

        pub fn retain_machine_roster(
            program: &mut TypedTrees,
            retained: &HashSet<SymbolHandle>,
            pruned: &HashSet<SymbolHandle>,
        ) {
            let retained_machines: Vec<Machine> = program
                .machines()
                .iter()
                .filter(|machine| retained.contains(&machine.symbol))
                .cloned()
                .collect();
            program.roots.machines = program.tables.machines.insert_many(retained_machines);

            program
                .machine_specializations
                .retain(|row| !pruned.contains(&row.instance));
            let designated = program
                .tables
                .designated_static_type_arguments
                .iter()
                .filter(|(_, row)| !pruned.contains(&row.owner))
                .map(|(_, row)| row.clone())
                .collect::<Vec<_>>();
            let mut arguments = arena::Arena::default();
            arguments.insert_many(designated);
            program.tables.designated_static_type_arguments = arguments;
            let captures = program
                .captured_machine_applications
                .iter()
                .filter(|(_, row)| !pruned.contains(&row.instance))
                .map(|(_, row)| row.clone())
                .collect::<Vec<_>>();
            let mut applications = arena::Arena::default();
            applications.insert_many(captures);
            program.captured_machine_applications = applications;
            program.retain_live_trait_implementation_applications(pruned);
            program
                .authored_service_reach_rows
                .retain(|row| !pruned.contains(&row.owner));
            program
                .evidence_forwardings
                .retain(|row| !pruned.contains(&row.machine_symbol));
            program
                .proof_output_calls
                .retain(|row| !pruned.contains(&row.machine_symbol));
            program
                .ranking_expression_custody
                .retain(|row| !pruned.contains(&row.machine));
        }

        fn apply_declaration_sidecar_pruning(
            program: &mut TypedTrees,
            removed: &HashSet<SymbolHandle>,
        ) {
            let symbols = &program.symbols;
            let owner_removed = |mut symbol: SymbolHandle| {
                while symbol.is_valid() {
                    if removed.contains(&symbol) {
                        return true;
                    }
                    symbol = symbols.get(symbol).parent;
                }
                false
            };
            program
                .plan_laid_layouts
                .retain(|row| !owner_removed(row.data_symbol));
            program
                .access_view_plans
                .retain(|row| !owner_removed(row.data_symbol));
            let lifecycle_plans = program
                .access_lifecycle_plans
                .iter()
                .filter(|(_, row)| match row {
                    typed_trees::typed_trees::AccessLifecyclePlan::Absent => false,
                    typed_trees::typed_trees::AccessLifecyclePlan::Evaluated {
                        callable, ..
                    } => {
                        !owner_removed(*callable)
                            && !program
                                .closed_requirement_application(*callable)
                                .is_some_and(|application| {
                                    owner_removed(application.declaring_trait)
                                        || owner_removed(application.template)
                                })
                            && !program
                                .machine_specialization(*callable)
                                .is_some_and(|application| owner_removed(application.template))
                    }
                })
                .map(|(_, row)| row.clone())
                .collect::<Vec<_>>();
            let mut lifecycle_catalog = arena::Arena::default();
            lifecycle_catalog.insert_many(lifecycle_plans);
            program.access_lifecycle_plans = lifecycle_catalog;
            program
                .wire_schema_plans
                .retain(|row| !owner_removed(row.schema));
            program
                .authored_service_reach_rows
                .retain(|row| !owner_removed(row.owner));
            let completions = program
                .boundary_completion_declarations
                .iter()
                .filter(|(_, row)| !owner_removed(row.owner))
                .map(|(_, row)| row.clone())
                .collect::<Vec<_>>();
            let mut declarations = arena::Arena::default();
            declarations.insert_many(completions);
            program.boundary_completion_declarations = declarations;
            program.remove_trait_declaration_applications(|declaration| {
                removed.contains(&declaration)
            });
            program.retain_live_trait_implementation_applications(removed);
            program.retain_live_trait_callable_applications();
        }

        pub fn product_request_operand_occurrences(
            program: &dyn crate::CheckingInput,
        ) -> HashSet<typed_trees::AuthoredDeclarationSelectionOccurrenceId> {
            use language_semantics::declaration_selection::AuthoredDeclarationSelectionKind as Kind;
            let mut occurrences = HashSet::new();
            for (expression, node) in expression_table(program).iter_expressions() {
                let typed_trees::expression::ExpressionNode::Call(call) = node else {
                    continue;
                };
                let Some(request) =
                    expression_table(program).build_selection_request(call.build_selection)
                else {
                    continue;
                };
                for occurrence in
                    expression_table(program).authored_selection_occurrences(expression)
                {
                    let Some(selection) = program.authored_declaration_selections().get(occurrence)
                    else {
                        continue;
                    };
                    let span = program
                        .symbols
                        .rejoin_product_occurrence(selection.source_span());
                    if request.designated_operands().iter().any(|operand| {
                        let operand_span = program
                            .symbols
                            .rejoin_product_occurrence(operand.source_span);
                        let declaration_path = matches!(
                            selection.kind(),
                            Kind::StaticArgument | Kind::StaticPathSegment | Kind::Conformance
                        ) && span.source_id == operand_span.source_id
                            && span.span.start >= operand_span.span.start
                            && span.span.end <= operand_span.span.end;
                        let captured_expression =
                            operand.application.as_ref().is_some_and(|graph| {
                                graph.selections.iter().any(|(_, captured)| {
                                    program
                                        .symbols
                                        .rejoin_product_occurrence(captured.source_span)
                                        == span
                                        && captured.kind == Some(selection.kind())
                                        && captured.exposure == selection.exposure()
                                })
                            });
                        declaration_path || captured_expression
                    }) {
                        occurrences.insert(occurrence);
                    }
                }
            }
            let materialized = materialized_product_operand_occurrences(program);
            occurrences.retain(|occurrence| !materialized.contains(occurrence));
            occurrences
        }

        fn materialized_product_operand_occurrences(
            program: &dyn crate::CheckingInput,
        ) -> HashSet<typed_trees::AuthoredDeclarationSelectionOccurrenceId> {
            #[derive(Default)]
            struct Selections {
                expressions: HashSet<typed_trees::expression::ExpressionHandle>,
                occurrences: HashSet<typed_trees::AuthoredDeclarationSelectionOccurrenceId>,
            }
            impl Extend<SymbolHandle> for Selections {
                fn extend<Symbols: IntoIterator<Item = SymbolHandle>>(&mut self, _: Symbols) {}
            }
            impl typed_trees::references::TypedReferences for Selections {
                fn expression(
                    &mut self,
                    handle: typed_trees::expression::ExpressionHandle,
                ) -> bool {
                    self.expressions.insert(handle)
                }
                fn authored_selection(
                    &mut self,
                    occurrence: typed_trees::AuthoredDeclarationSelectionOccurrenceId,
                ) {
                    self.occurrences.insert(occurrence);
                }
            }
            let mut selections = Selections::default();
            let typed = program.typed_program();
            let mut owners = Vec::new();
            for handle in typed.live_product_operand_applications() {
                let application = typed.product_operand_applications.get(handle);
                let owner = expression_table(typed)
                    .static_machine_application(application.supplying_application)
                    .lexical_owner;
                if !owners.contains(&owner) {
                    owners.push(owner);
                }
            }
            for owner in owners {
                typed_trees::references::visit_product_operand_inputs(
                    typed,
                    &mut selections,
                    owner,
                );
            }
            selections.occurrences
        }

        #[cfg(test)]
        mod lifecycle_projection_tests {
            use super::*;
            use typed_trees::typed_trees::{AccessLifecycleOperation, AccessLifecyclePlan};

            #[test]
            fn lifecycle_projection_removes_known_owners_but_preserves_orphan_obligations() {
                let mut symbols = symbols::SymbolTableBuilder::default();
                let owner = symbols.insert_root(
                    symbols::SymbolKind::Trait,
                    symbols::SymbolNameRef::Static("Owner"),
                );
                let requirement = symbols
                    .insert_children(
                        owner,
                        [(
                            symbols::SymbolKind::Machine,
                            symbols::SymbolNameRef::Static("initialize"),
                        )],
                    )
                    .handle_at(0);
                let orphan = symbols.insert_root(
                    symbols::SymbolKind::Machine,
                    symbols::SymbolNameRef::Static("Orphan"),
                );
                let mut program = TypedTrees {
                    symbols: symbols.finish(),
                    ..Default::default()
                };
                let layout = layout_plans::LayoutPlanReport {
                    schema_report_fingerprint: 1,
                    entries: Vec::new(),
                    offsets: Some(Vec::new()),
                    size: Some(0),
                    align: 1,
                };
                let access = access_plans::AccessFields::inaccessible(&layout).unwrap();
                let placement = access_plans::validate_access_plan(access_plans::AccessPlan {
                    layout,
                    access,
                    reach: Default::default(),
                })
                .unwrap();
                let invocations = program
                    .access_policy_invocations
                    .insert_many([source::SourceSpan::default()]);
                for requirement in [requirement, orphan] {
                    program
                        .access_lifecycle_plans
                        .append(AccessLifecyclePlan::Evaluated {
                            callable: requirement,
                            operation: AccessLifecycleOperation::Initialize,
                            policy: Default::default(),
                            schema: Default::default(),
                            policy_machine: Default::default(),
                            invocations,
                            target: None,
                            placement: placement.clone(),
                            storage: Default::default(),
                            profile_carrier: None,
                        });
                }
                apply_declaration_sidecar_pruning(&mut program, &HashSet::from([owner]));
                let retained = program
                    .access_lifecycle_plans
                    .iter()
                    .filter_map(|(_, row)| match row {
                        AccessLifecyclePlan::Evaluated { callable, .. } => Some(*callable),
                        AccessLifecyclePlan::Absent => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(retained, vec![orphan]);
                assert_eq!(
                    program
                        .access_policy_invocations
                        .span_or_empty(invocations)
                        .len(),
                    1
                );
            }
        }
    }
    pub mod expression_traversal {
        use crate::frozen_program::expression_table;
        use language_core::ReferenceAccess;
        use std::collections::HashSet;
        use typed_trees::domain::ProofFact;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableMatchExpression};
        use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

        pub fn inert_value_tree_with_operations(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            mut allow_operation: impl FnMut(ExpressionHandle) -> bool,
        ) -> bool {
            if !expression.is_valid() {
                return false;
            }
            let mut expressions = Vec::new();
            collect_expression_tree(program, expression, &mut expressions);
            expressions.iter().all(|expression| {
                matches!(
                    expression_table(program).expression(*expression),
                    ExpressionNode::Name(_)
                        | ExpressionNode::Integer(_)
                        | ExpressionNode::Boolean(_)
                        | ExpressionNode::Float(_)
                        | ExpressionNode::String(_)
                        | ExpressionNode::StructLiteral(_)
                        | ExpressionNode::ArrayLiteral(_)
                        | ExpressionNode::Borrow(_)
                        | ExpressionNode::Member(_)
                        | ExpressionNode::Match(_)
                ) || allow_operation(*expression)
            })
        }

        pub fn expression_has_inert_prefix(
            program: &dyn crate::CheckingInput,
            patterns: &[ExpressionHandle],
            root: ExpressionHandle,
            endpoint: ExpressionHandle,
        ) -> bool {
            struct Prefix {
                effect_seen: bool,
                found: bool,
                stable: bool,
            }
            fn visit(
                program: &dyn crate::CheckingInput,
                expression: ExpressionHandle,
                endpoint: ExpressionHandle,
                depth: usize,
                prefix: &mut Prefix,
            ) -> bool {
                if !expression.is_valid() {
                    return true;
                }
                if depth >= 64 {
                    return false;
                }
                let node = expression_table(program).expression(expression);
                let mut complete = true;
                executable_children(program, node, |child| {
                    complete &= visit(program, child, endpoint, depth + 1, prefix);
                });
                if matches!(node, ExpressionNode::Call(_) | ExpressionNode::Atomic(_)) {
                    prefix.effect_seen = true;
                }
                if expression == endpoint {
                    prefix.found = true;
                    prefix.stable &= !prefix.effect_seen;
                }
                complete
            }
            let mut prefix = Prefix {
                effect_seen: false,
                found: false,
                stable: true,
            };
            let mut complete = true;
            for pattern in patterns {
                complete &= visit(program, *pattern, endpoint, 0, &mut prefix);
            }
            if prefix.found {
                return false;
            }
            complete &= visit(program, root, endpoint, 0, &mut prefix);
            complete && prefix.found && prefix.stable
        }

        pub fn expressions_before_occurrence(
            program: &dyn crate::CheckingInput,
            roots: &[ExpressionHandle],
            endpoint: ExpressionHandle,
        ) -> Option<Vec<ExpressionHandle>> {
            fn visit(
                program: &dyn crate::CheckingInput,
                expression: ExpressionHandle,
                endpoint: ExpressionHandle,
                depth: usize,
                occurrences: &mut usize,
                prefix: &mut Vec<ExpressionHandle>,
                captured: &mut Vec<ExpressionHandle>,
            ) -> Option<()> {
                if !expression.is_valid() {
                    return Some(());
                }
                if depth >= 64 {
                    return None;
                }
                if expression == endpoint {
                    *occurrences += 1;
                    if *occurrences == 1 {
                        captured.clone_from(prefix);
                    }
                    return Some(());
                }
                let start = prefix.len();
                let node = expression_table(program).expression(expression);
                if let ExpressionNode::Match(dispatch) = node {
                    visit(
                        program,
                        dispatch.subject,
                        endpoint,
                        depth + 1,
                        occurrences,
                        prefix,
                        captured,
                    )?;
                    for arm in expression_table(program).match_arms(dispatch.arms) {
                        for pattern in expression_table(program).match_pattern_operands(arm.pattern)
                        {
                            visit(
                                program,
                                pattern,
                                endpoint,
                                depth + 1,
                                occurrences,
                                prefix,
                                captured,
                            )?;
                        }
                        let patterns_end = prefix.len();
                        visit(
                            program,
                            arm.value,
                            endpoint,
                            depth + 1,
                            occurrences,
                            prefix,
                            captured,
                        )?;
                        prefix.truncate(patterns_end);
                    }
                } else {
                    let mut completed = Some(());
                    executable_children(program, node, |child| {
                        if visit(
                            program,
                            child,
                            endpoint,
                            depth + 1,
                            occurrences,
                            prefix,
                            captured,
                        )
                        .is_none()
                        {
                            completed = None;
                        }
                    });
                    completed?;
                }
                prefix.truncate(start);
                prefix.push(expression);
                Some(())
            }
            let mut occurrences = 0;
            let mut prefix = Vec::new();
            let mut captured = Vec::new();
            for root in roots {
                visit(
                    program,
                    *root,
                    endpoint,
                    0,
                    &mut occurrences,
                    &mut prefix,
                    &mut captured,
                )?;
            }
            (occurrences == 1).then_some(captured)
        }

        pub fn children(
            program: &dyn crate::CheckingInput,
            node: &ExpressionNode,
            mut child: impl FnMut(ExpressionHandle),
        ) {
            match node {
                ExpressionNode::Match(dispatch) => {
                    for expression in match_children(program, *dispatch) {
                        child(expression);
                    }
                }
                ExpressionNode::Binary(binary) => {
                    child(binary.left);
                    child(binary.right);
                }
                ExpressionNode::Unary(unary) => child(unary.operand),
                ExpressionNode::Borrow(borrow) => child(borrow.target),
                ExpressionNode::Cast(cast) => child(cast.value),
                ExpressionNode::Atomic(atomic) => {
                    if let Some(once) =
                        crate::value_custody::atomic_operations::atomic_once_operands(
                            program, atomic,
                        )
                    {
                        child(once.place);
                        for operand in once.operands {
                            child(operand);
                        }
                    } else {
                        child(atomic.value);
                        child(atomic.result);
                    }
                }
                ExpressionNode::ArrayLiteral(elements) => {
                    for element in expression_table(program).expression_handles(*elements) {
                        child(*element);
                    }
                }
                ExpressionNode::Call(call) => {
                    child(call.receiver);
                    for argument in expression_table(program).expression_handles(call.arguments) {
                        child(*argument);
                    }
                }
                ExpressionNode::Indexed(indexed) => {
                    child(indexed.collection);
                    child(indexed.index);
                }
                ExpressionNode::Member(member) => child(member.receiver),
                ExpressionNode::Range(range) => {
                    child(range.start);
                    child(range.end);
                }
                ExpressionNode::StructLiteral(literal) => {
                    for field in expression_table(program).struct_fields(literal.fields) {
                        child(field.value);
                    }
                }
                ExpressionNode::TypeExpression(_)
                | ExpressionNode::SavedSubject(_)
                | typed_trees::literal_expression_pattern!() => {}
            }
        }

        pub(crate) fn executable_children(
            program: &dyn crate::CheckingInput,
            node: &ExpressionNode,
            mut child: impl FnMut(ExpressionHandle),
        ) {
            if let ExpressionNode::Atomic(atomic) = node {
                if let Some(once) =
                    crate::value_custody::atomic_operations::atomic_once_operands(program, atomic)
                {
                    child(once.place);
                    for operand in once.operands {
                        child(operand);
                    }
                } else {
                    child(atomic.value);
                }
            } else {
                children(program, node, child);
            }
        }

        pub fn executable_subtree_any(
            program: &dyn crate::CheckingInput,
            root: ExpressionHandle,
            edges: typed_trees::SubtreeEdges,
            mut visit: impl FnMut(ExpressionHandle, &ExpressionNode) -> typed_trees::SubtreeVisit,
        ) -> bool {
            program.subtree_any(root, edges, |expression, node| {
                match (visit(expression, node), node) {
                    (typed_trees::SubtreeVisit::Descend, ExpressionNode::Atomic(atomic)) => {
                        match crate::value_custody::atomic_operations::atomic_once_operands(
                            program, atomic,
                        ) {
                            Some(once) => typed_trees::SubtreeVisit::InsteadEach(
                                std::iter::once(once.place).chain(once.operands).collect(),
                            ),
                            None => typed_trees::SubtreeVisit::Descend,
                        }
                    }
                    (visit, _) => visit,
                }
            })
        }

        pub fn visit_post_order(
            program: &dyn crate::CheckingInput,
            root: ExpressionHandle,
            visited: &mut Vec<ExpressionHandle>,
            mut arm: impl FnMut(ExpressionHandle, &ExpressionNode),
        ) {
            fn inner(
                program: &dyn crate::CheckingInput,
                expression: ExpressionHandle,
                visited: &mut Vec<ExpressionHandle>,
                arm: &mut dyn FnMut(ExpressionHandle, &ExpressionNode),
            ) {
                if !expression.is_valid() || visited.contains(&expression) {
                    return;
                }
                visited.push(expression);
                let node = expression_table(program).expression(expression);
                children(program, node, |child| inner(program, child, visited, arm));
                arm(expression, node);
            }
            inner(program, root, visited, &mut arm);
        }

        pub fn visit_pre_order(
            program: &dyn crate::CheckingInput,
            pending: Vec<ExpressionHandle>,
            mut arm: impl FnMut(ExpressionHandle, &ExpressionNode) -> bool,
        ) -> bool {
            let mut pending = pending;
            let mut visited = Vec::new();
            while let Some(expression) = pending.pop() {
                if !expression_table(program).expression_is_valid(expression)
                    || visited.contains(&expression)
                {
                    continue;
                }
                visited.push(expression);
                let node = expression_table(program).expression(expression);
                if arm(expression, node) {
                    return true;
                }
                children(program, node, |child| pending.push(child));
            }
            false
        }

        pub fn projection_climb(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            max_depth: usize,
        ) -> Option<(Vec<ExpressionHandle>, ExpressionHandle)> {
            let mut projections = Vec::new();
            let mut cursor = expression;
            loop {
                if !expression_table(program).expression_is_valid(cursor)
                    || projections.contains(&cursor)
                    || projections.len() >= max_depth
                {
                    return None;
                }
                cursor = match expression_table(program).expression(cursor) {
                    ExpressionNode::Member(member) if member.case_variant.is_none() => {
                        projections.push(cursor);
                        member.receiver
                    }
                    ExpressionNode::Indexed(indexed) => {
                        projections.push(cursor);
                        indexed.collection
                    }
                    _ => break,
                };
            }
            Some((projections, cursor))
        }

        pub fn single_member_argument_symbol(
            program: &dyn crate::CheckingInput,
            argument: ExpressionHandle,
        ) -> Option<symbols::SymbolHandle> {
            let named = match expression_table(program).expression(argument) {
                ExpressionNode::Borrow(borrow) if borrow.access == ReferenceAccess::Shared => {
                    borrow.target
                }
                _ => argument,
            };
            let path = expression_table(program).name_path_node(named)?;
            (path.symbol.is_valid()
                && expression_table(program)
                    .name_path_members(path.members)
                    .len()
                    == 1)
                .then_some(path.symbol)
        }

        pub fn name_path_last_member(
            program: &dyn crate::CheckingInput,
            value: ExpressionHandle,
        ) -> Option<&str> {
            let path = expression_table(program).name_path_node(value)?;
            expression_table(program)
                .name_path_members(path.members)
                .last()
                .map(|member| member.as_str())
        }

        pub fn unwrapped(
            program: &dyn crate::CheckingInput,
            mut expression: ExpressionHandle,
        ) -> ExpressionHandle {
            while let ExpressionNode::Atomic(atomic) =
                expression_table(program).expression(expression)
            {
                expression = atomic.value;
            }
            expression
        }

        pub fn push_children_reversed(
            program: &dyn crate::CheckingInput,
            node: &ExpressionNode,
            pending: &mut Vec<ExpressionHandle>,
        ) {
            let mut edges = Vec::new();
            children(program, node, |child| edges.push(child));
            pending.extend(edges.into_iter().rev());
        }

        pub(crate) fn match_children(
            program: &dyn crate::CheckingInput,
            dispatch: TableMatchExpression,
        ) -> impl Iterator<Item = ExpressionHandle> + '_ {
            std::iter::once(dispatch.subject).chain(
                expression_table(program)
                    .match_arms(dispatch.arms)
                    .iter()
                    .flat_map(|arm| {
                        expression_table(program)
                            .match_pattern_operands(arm.pattern)
                            .chain(std::iter::once(arm.value))
                    }),
            )
        }

        pub(crate) fn collect_expression_nodes<Hashing: std::hash::BuildHasher>(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            nodes: &mut Vec<ExpressionHandle>,
            seen: &mut HashSet<ExpressionHandle, Hashing>,
        ) {
            if !expression.is_valid() || !seen.insert(expression) {
                return;
            }
            nodes.push(expression);
            children(
                program,
                expression_table(program).expression(expression),
                |child| {
                    collect_expression_nodes(program, child, nodes, seen);
                },
            );
        }

        pub fn single_segment_name_symbol(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<symbols::SymbolHandle> {
            let path = expression_table(program).name_path_node(expression)?;
            is_single_segment_name_path(program, path).then_some(path.symbol)
        }

        pub fn is_single_segment_name_path(
            program: &dyn crate::CheckingInput,
            path: &typed_trees::expression::TableNamePath,
        ) -> bool {
            expression_table(program).name_path_is_single_member(path)
        }

        pub fn record_unresolved_subject_source_obligations(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            prior_statements: &[typed_trees::statement::StatementNode],
            symbols: &crate::TopLevelSymbols<'_>,
            expression: ExpressionHandle,
            diagnostics: &mut Vec<diagnostics::Diagnostic>,
        ) {
            let mut names = Vec::new();
            collect_expression_tree(program, expression, &mut names);
            for name in names {
                crate::machine_calls::calls::report_unresolved_name_use(
                    program,
                    machine,
                    state,
                    prior_statements,
                    symbols,
                    name,
                    diagnostics,
                );
            }
            if let Err(mut construction) = crate::validate_value_construction(
                program,
                machine,
                state,
                expression,
                &mut Vec::new(),
            ) {
                diagnostics.append(&mut construction);
            }
            crate::value_custody::expression_types::validate_float_expression_formats(
                program,
                state.symbol,
                expression,
                &|operand| {
                    crate::expression_result_type_reference(program, machine, state, operand)
                },
                diagnostics,
            );
        }

        pub fn collect_expression_tree(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            handles: &mut Vec<ExpressionHandle>,
        ) {
            collect_expression_trees(program, std::iter::once(expression), handles);
        }

        pub fn collect_expression_trees(
            program: &dyn crate::CheckingInput,
            roots: impl IntoIterator<Item = ExpressionHandle>,
            handles: &mut Vec<ExpressionHandle>,
        ) {
            let mut roots = roots.into_iter();
            let Some(first) = roots.next() else {
                return;
            };
            let mut seen: HashSet<_, symbols::BuildSymbolHasher> =
                handles.iter().copied().collect();
            collect_expression_nodes(program, first, handles, &mut seen);
            for expression in roots {
                collect_expression_nodes(program, expression, handles, &mut seen);
            }
        }

        pub fn collect_statement_expression_trees(
            program: &dyn crate::CheckingInput,
            statement: &typed_trees::statement::StatementNode,
            handles: &mut Vec<ExpressionHandle>,
        ) {
            match statement {
                typed_trees::statement::StatementNode::Assembly(_) => {}
                typed_trees::statement::StatementNode::AssemblyFact(fact) => {
                    collect_expression_trees(
                        program,
                        program.tables.proof_fact_expression_roots(fact.fact),
                        handles,
                    );
                }
                typed_trees::statement::StatementNode::Assignment(assignment) => {
                    collect_expression_trees(
                        program,
                        [assignment.target, assignment.value],
                        handles,
                    );
                }
                typed_trees::statement::StatementNode::Call(call) => {
                    collect_expression_trees(
                        program,
                        program
                            .statement_table
                            .expression_handles(call.arguments)
                            .iter()
                            .copied(),
                        handles,
                    );
                }
                typed_trees::statement::StatementNode::Expression(expression) => {
                    collect_expression_tree(program, *expression, handles)
                }
                typed_trees::statement::StatementNode::LocalData(local) => {
                    collect_expression_tree(program, local.initial_value, handles)
                }
                typed_trees::statement::StatementNode::Transition(transition) => {
                    let guard = match transition.guard {
                        typed_trees::statement::TransitionGuardNode::When(guard) => Some(guard),
                        _ => None,
                    };
                    collect_expression_trees(
                        program,
                        guard
                            .into_iter()
                            .chain(transition_target_expression_roots(
                                program,
                                transition.target,
                            ))
                            .chain(transition_target_expression_roots(
                                program,
                                transition.continuation,
                            )),
                        handles,
                    );
                }
            }
        }

        pub fn collect_type_expressions(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
            visited: &mut Vec<TypeReferenceHandle>,
            expressions: &mut Vec<ExpressionHandle>,
        ) {
            if !reference.is_valid() || visited.contains(&reference) {
                return;
            }
            visited.push(reference);
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Reference { referee, .. } => {
                    collect_type_expressions(program, *referee, visited, expressions);
                }
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => {
                    collect_type_expressions(program, *element_type, visited, expressions);
                }
                TypeReferenceNode::Generic { arguments, .. } => {
                    for argument in program
                        .type_reference_table
                        .type_reference_handles(*arguments)
                    {
                        collect_type_expressions(program, *argument, visited, expressions);
                    }
                }
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    collect_type_expressions(program, *base_type, visited, expressions);
                    for constraint in program.type_reference_table.constraints(*constraints) {
                        match constraint {
                            TypeConstraintNode::Domain(domain) => {
                                for argument in &domain.arguments {
                                    collect_type_expressions(
                                        program,
                                        *argument,
                                        visited,
                                        expressions,
                                    );
                                }
                            }
                            TypeConstraintNode::Named(_)
                            | TypeConstraintNode::ArithmeticDomain(_) => {}
                        }
                    }
                }
                TypeReferenceNode::ConstExpression(expression) => {
                    collect_expression_tree(program, *expression, expressions);
                }
                TypeReferenceNode::Named { .. }
                | TypeReferenceNode::DynamicTrait { .. }
                | TypeReferenceNode::Unit => {}
            }
        }

        pub fn collect_transition_target_expression_trees(
            program: &dyn crate::CheckingInput,
            target: typed_trees::statement::TransitionTargetHandle,
            handles: &mut Vec<ExpressionHandle>,
        ) {
            collect_expression_trees(
                program,
                transition_target_expression_roots(program, target),
                handles,
            );
        }

        fn transition_target_expression_roots(
            program: &dyn crate::CheckingInput,
            target: typed_trees::statement::TransitionTargetHandle,
        ) -> impl Iterator<Item = ExpressionHandle> + '_ {
            let roots = if !target.is_valid() {
                &[]
            } else {
                match program.statement_table.transition_target(target) {
                    typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
                        program.statement_table.expression_handles(*arguments)
                    }
                    typed_trees::statement::TransitionTargetNode::Value(expression) => {
                        std::slice::from_ref(expression)
                    }
                    typed_trees::statement::TransitionTargetNode::Terminal => &[],
                }
            };
            roots.iter().copied()
        }

        pub fn append_fact_expression_roots(
            program: &dyn crate::CheckingInput,
            fact: &ProofFact,
            expressions: &mut Vec<ExpressionHandle>,
        ) {
            match fact {
                ProofFact::Expression(expression) => expressions.push(*expression),
                ProofFact::Membership(membership) => expressions.push(membership.value),
                ProofFact::Proposition(application) => expressions.extend(
                    expression_table(program)
                        .expression_handles(application.arguments)
                        .iter()
                        .copied(),
                ),
            }
        }

        pub fn collect_contract_facts(
            program: &dyn crate::CheckingInput,
            facts: arena::HandleSpan<ProofFact>,
            handles: &mut Vec<ExpressionHandle>,
        ) {
            collect_proof_fact_expression_trees(
                program,
                program.proof_facts.span_or_empty(facts),
                handles,
            );
        }

        pub fn collect_proof_fact_expression_trees<'program>(
            program: &'program dyn crate::CheckingInput,
            facts: impl IntoIterator<Item = &'program ProofFact>,
            handles: &mut Vec<ExpressionHandle>,
        ) {
            collect_expression_trees(
                program,
                facts.into_iter().flat_map(|fact| {
                    let roots = match fact {
                        ProofFact::Expression(expression) => std::slice::from_ref(expression),
                        ProofFact::Membership(membership) => {
                            std::slice::from_ref(&membership.value)
                        }
                        ProofFact::Proposition(application) => {
                            expression_table(program).expression_handles(application.arguments)
                        }
                    };
                    roots.iter().copied()
                }),
                handles,
            );
        }

        #[cfg(test)]
        mod collection_tests {
            use super::*;
            use typed_trees::{
                TypedTrees, domain::ProofMembershipFact, proposition::PropositionApplication,
            };

            #[test]
            fn batched_roots_keep_seed_order_full_generations_and_shared_children() {
                let mut program = TypedTrees::default();
                let first = program
                    .expression_table
                    .insert(ExpressionNode::Boolean(false));
                let shared = program
                    .expression_table
                    .insert(ExpressionNode::Boolean(true));
                let last = program
                    .expression_table
                    .insert(ExpressionNode::Boolean(false));
                let left_children = program
                    .expression_table
                    .insert_expression_handles([first, shared]);
                let left = program
                    .expression_table
                    .insert(ExpressionNode::ArrayLiteral(left_children));
                let right_children = program
                    .expression_table
                    .insert_expression_handles([shared, last]);
                let right = program
                    .expression_table
                    .insert(ExpressionNode::ArrayLiteral(right_children));
                let other_generation = ExpressionHandle::from_parts(
                    right.arena_index(),
                    right.generation().wrapping_add(1),
                );
                let seed = vec![first, first, other_generation];
                let mut nodes = seed.clone();
                collect_expression_trees(
                    &program,
                    [left, right, left, ExpressionHandle::invalid()],
                    &mut nodes,
                );
                let mut expected = seed;
                expected.extend([left, shared, right, last]);
                assert_eq!(nodes, expected);
            }

            #[test]
            fn proof_fact_roots_keep_order_and_independent_lists() {
                let mut program = TypedTrees::default();
                let first = program
                    .expression_table
                    .insert(ExpressionNode::Boolean(false));
                let second = program
                    .expression_table
                    .insert(ExpressionNode::Boolean(true));
                let third = program
                    .expression_table
                    .insert(ExpressionNode::Boolean(false));
                let arguments = program
                    .expression_table
                    .insert_expression_handles([third, first]);
                let facts = [
                    ProofFact::Expression(first),
                    ProofFact::Membership(ProofMembershipFact {
                        value: second,
                        ..Default::default()
                    }),
                    ProofFact::Proposition(PropositionApplication {
                        proposition: symbols::SymbolHandle::invalid(),
                        name: typed_trees::name::Identifier::from("Predicate"),
                        binder_arguments: Vec::new().into(),
                        arguments,
                    }),
                ];
                let mut first_scope = Vec::new();
                let mut second_scope = Vec::new();
                collect_proof_fact_expression_trees(&program, &facts, &mut first_scope);
                collect_proof_fact_expression_trees(&program, &facts, &mut second_scope);
                assert_eq!(first_scope, [first, second, third]);
                assert_eq!(second_scope, first_scope);
            }
        }
    }
    pub(crate) mod layout_checking {
        use crate::mathematical_applications::PreparedMathematicalView;
        use crate::{CheckingInput, CheckingMemos, PreparedEffectPlans};
        use language_semantics::declaration_selection::AuthoredDeclarationSelections;
        use representation_selections::OpaqueRepresentationSelection;
        use std::sync::Arc;
        use typed_trees::proof_only::ProofOnlyClassification;
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
        use typed_trees::{PlanLaidLayout, PlanLaidRealizations, TypedTrees};

        #[derive(Debug)]
        pub struct LayoutCheckingContext<'a> {
            input: &'a dyn CheckingInput,
            target: Option<&'a str>,
        }

        impl<'a> LayoutCheckingContext<'a> {
            pub fn new(input: &'a dyn CheckingInput, target: Option<&'a str>) -> Self {
                Self { input, target }
            }
        }

        impl CheckingInput for LayoutCheckingContext<'_> {
            fn has_active_recast_formation(
                &self,
                expression: typed_trees::expression::ExpressionHandle,
                depth: usize,
            ) -> bool {
                self.input.has_active_recast_formation(expression, depth)
            }

            fn external_invocation_roots(&self) -> Option<&[symbols::SymbolHandle]> {
                self.input.external_invocation_roots()
            }

            fn has_active_completed_case_population(&self, machine: symbols::SymbolHandle) -> bool {
                self.input.has_active_completed_case_population(machine)
            }

            fn has_active_operand_fit_population(&self, machine: symbols::SymbolHandle) -> bool {
                self.input.has_active_operand_fit_population(machine)
            }

            fn has_active_numeric_observation(
                &self,
                machine: symbols::SymbolHandle,
                state: symbols::SymbolHandle,
            ) -> bool {
                self.input.has_active_numeric_observation(machine, state)
            }

            fn defers_pending_layout_geometry(&self) -> bool {
                self.input.defers_pending_layout_geometry()
            }
            fn retained_borrow_custodies(&self) -> &[checked_trees::RetainedBorrowCustodyFact] {
                self.input.retained_borrow_custodies()
            }

            fn typed_program(&self) -> &TypedTrees {
                self.input.typed_program()
            }
            fn layout_target(&self) -> Option<&str> {
                self.target
            }
            fn authored_declaration_selections(&self) -> &AuthoredDeclarationSelections {
                self.input.authored_declaration_selections()
            }
            fn checking_memos(&self) -> Option<&CheckingMemos> {
                None
            }

            fn cut_reads(&self) -> u64 {
                self.input.cut_reads()
            }

            fn shared_memos(&self) -> Option<&CheckingMemos> {
                None
            }
            fn proof_only_classification(&self) -> Arc<ProofOnlyClassification> {
                self.input.proof_only_classification()
            }
            fn prepared_effect_plans(&self) -> Option<&PreparedEffectPlans> {
                self.input.prepared_effect_plans()
            }
            fn storage_graph(&self) -> Option<&checked_trees::FlowControlFacts> {
                self.input.storage_graph()
            }

            fn expression_interval(
                &self,
                expression: typed_trees::expression::ExpressionHandle,
            ) -> Option<crate::ExpressionInterval> {
                self.input.expression_interval(expression)
            }
            fn prepared_mathematics(&self) -> Option<PreparedMathematicalView<'_>> {
                self.input.prepared_mathematics()
            }
            fn opaque_representation_selections(&self) -> &[OpaqueRepresentationSelection] {
                self.input.opaque_representation_selections()
            }
        }

        pub(crate) fn layout_family_is_complete(
            program: &dyn CheckingInput,
            layout: &PlanLaidLayout,
        ) -> bool {
            let PlanLaidRealizations::TargetInputs(span) = layout.realizations else {
                return true;
            };
            let Some(rows) = program.plan_laid_target_layouts.span(span) else {
                return false;
            };
            rows.len() == layout.target_inputs.len()
                && rows.iter().zip(&layout.target_inputs).enumerate().all(
                    |(index, (row, input))| {
                        row.target_facts == *input
                            && row.machine == layout.policy_plan_machine_symbol
                            && row.target.as_str() == row.target_facts.profile
                            && row.target_facts.validate().is_ok()
                            && !rows[..index]
                                .iter()
                                .any(|previous| previous.target == row.target)
                    },
                )
        }

        pub(crate) fn layout_checking_targets(
            program: &dyn CheckingInput,
            roots: impl IntoIterator<Item = TypeReferenceHandle>,
        ) -> Result<Vec<Option<&str>>, ()> {
            let alternatives = discover_layout_checking_targets(program, roots)?;
            if let Some(target) = program.layout_target() {
                return (alternatives == [None] || alternatives.contains(&Some(target)))
                    .then_some(vec![Some(target)])
                    .ok_or(());
            }
            Ok(alternatives)
        }

        fn discover_layout_checking_targets(
            program: &dyn CheckingInput,
            roots: impl IntoIterator<Item = TypeReferenceHandle>,
        ) -> Result<Vec<Option<&str>>, ()> {
            let mut pending: Vec<_> = roots.into_iter().map(|root| (root, true)).collect();
            let mut visited = Vec::new();
            let mut targets: Option<Vec<Option<&str>>> = None;
            while let Some((reference, root_referent)) = pending.pop() {
                if !reference.is_valid() || visited.contains(&(reference, root_referent)) {
                    continue;
                }
                if !program
                    .type_reference_table
                    .contains_type_reference(reference)
                {
                    return Err(());
                }
                visited.push((reference, root_referent));
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Reference { referee, .. } => {
                        if root_referent {
                            pending.push((*referee, true));
                        }
                    }
                    TypeReferenceNode::Constrained { base_type, .. } => {
                        pending.push((*base_type, root_referent));
                    }
                    TypeReferenceNode::FixedArray { element_type, .. }
                    | TypeReferenceNode::Slice { element_type } => {
                        pending.push((*element_type, false))
                    }
                    _ => {
                        if program.layout_storage_arguments(reference).is_some()
                            && program.plan_laid_layout_for_type(reference).is_none()
                        {
                            return Err(());
                        }
                        let Some(data) = crate::value_custody::places::data_definition_for_type(
                            program, reference,
                        ) else {
                            continue;
                        };
                        if let Some(layout) = program.plan_laid_layout_for_type(reference) {
                            if !layout_family_is_complete(program, layout) {
                                return Err(());
                            }
                            if matches!(layout.realizations, PlanLaidRealizations::TargetInputs(_))
                            {
                                let alternatives = layout
                                    .target_layouts(program.typed_program())
                                    .iter()
                                    .map(|row| Some(row.target.as_str()))
                                    .collect::<Vec<_>>();
                                match &mut targets {
                                    Some(targets) => {
                                        targets.retain(|target| alternatives.contains(target))
                                    }
                                    None => targets = Some(alternatives),
                                }
                            }
                        }
                        pending.extend(
                            program
                                .data_member_fields(program.data_members(data))
                                .filter(|field| !field.relevance.is_erased())
                                .map(|field| (field.type_reference, false)),
                        );
                    }
                }
            }
            let targets = targets.unwrap_or_else(|| vec![None]);
            if targets.is_empty() {
                Err(())
            } else {
                Ok(targets)
            }
        }

        pub fn pending_layout_geometry(
            program: &dyn CheckingInput,
            roots: impl IntoIterator<Item = TypeReferenceHandle>,
        ) -> bool {
            let mut work = roots.into_iter().collect::<Vec<_>>();
            let mut visited = Vec::new();
            while let Some(reference) = work.pop() {
                if !reference.is_valid() {
                    continue;
                }
                let Some(index) = program.normalized_type_identity(reference).storage_index()
                else {
                    continue;
                };
                if visited.len() <= index {
                    visited.resize(index + 1, false);
                }
                if std::mem::replace(&mut visited[index], true) {
                    continue;
                }
                if let Some(storage) = program.layout_storage_arguments(reference) {
                    if program
                        .plan_laid_layout_for_type(storage.application)
                        .is_none()
                    {
                        return true;
                    }
                    work.push(storage.semantic_type);
                    continue;
                }
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Constrained { base_type, .. }
                    | TypeReferenceNode::Reference {
                        referee: base_type, ..
                    }
                    | TypeReferenceNode::FixedArray {
                        element_type: base_type,
                        ..
                    }
                    | TypeReferenceNode::Slice {
                        element_type: base_type,
                    } => work.push(*base_type),
                    TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                        if let Some(data) = program.closed_data_definition(reference) {
                            work.extend(
                                program
                                    .live_data_fields(program.data_members(data))
                                    .map(|field| field.type_reference),
                            );
                        }
                    }
                    _ => {}
                }
            }
            false
        }
    }
    pub mod product_queries {
        use language_semantics::declaration_selection::{
            DescribedProductEntry, DescribedProductProvider, DescribedProductSchema,
        };
        use semantic_vocabulary::ProgramEntrySourceCatalog;
        use source::SourceSpan;
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;
        use typed_trees::product_operand::ProductEntryDeclaration;

        pub(crate) struct ProductScope<'path> {
            occurrence: SourceSpan,
            path: &'path str,
            qualified: bool,
            dependency: Option<semantic_vocabulary::PackageKeyIdentity>,
        }

        impl<'path> ProductScope<'path> {
            fn new(program: &TypedTrees, occurrence: SourceSpan, name: &'path str) -> Self {
                let qualified = name.split_once("::");
                let dependency = qualified.and_then(|(alias, _)| {
                    program.symbols.product_dependency_target(occurrence, alias)
                });
                Self {
                    occurrence,
                    path: match (qualified, dependency) {
                        (Some((_, path)), Some(_)) => path,
                        _ => name,
                    },
                    qualified: qualified.is_some(),
                    dependency,
                }
            }

            fn contains(
                &self,
                program: &TypedTrees,
                symbol: SymbolHandle,
                is_public: bool,
            ) -> bool {
                let matches_path = if self.qualified {
                    program.symbols.display_path(symbol, "::") == self.path
                } else {
                    program.symbols.name(symbol) == self.path
                };
                let authorized = match self.dependency {
                    Some(package) => {
                        is_public
                            && program.symbols.symbol_product_package_identity(symbol)
                                == Some(package)
                    }
                    None => program
                        .symbols
                        .symbol_source_span(symbol)
                        .is_some_and(|span| {
                            program
                                .symbols
                                .same_product_package_instance(self.occurrence, span)
                        }),
                };
                matches_path
                    && authorized
                    && program
                        .symbols
                        .source_reference_can_describe_product_symbol(self.occurrence, symbol)
            }
        }

        pub fn describe_schema(
            program: &TypedTrees,
            occurrence: SourceSpan,
            path: &[u8],
        ) -> Result<DescribedProductSchema, String> {
            let selected = select_declaration(
                program,
                occurrence,
                path,
                "schema",
                "data",
                program
                    .data_definitions()
                    .iter()
                    .map(|definition| (definition.symbol, definition.is_public)),
            )?;
            Ok(DescribedProductSchema {
                declaration: symbols::RetainedDeclaration::capture(&program.symbols, selected)
                    .map_err(str::to_owned)?,
                canonical_path: program.symbols.display_path(selected, "::"),
            })
        }

        pub fn describe_provider(
            program: &TypedTrees,
            occurrence: SourceSpan,
            path: &[u8],
        ) -> Result<DescribedProductProvider, String> {
            let selected = select_declaration(
                program,
                occurrence,
                path,
                "provider",
                "provider",
                program
                    .conformances()
                    .iter()
                    .map(|implementation| (implementation.symbol, implementation.is_public)),
            )?;
            Ok(DescribedProductProvider {
                declaration: symbols::RetainedDeclaration::capture(&program.symbols, selected)
                    .map_err(str::to_owned)?,
                canonical_path: program.symbols.display_path(selected, "::"),
            })
        }

        fn select_declaration(
            program: &TypedTrees,
            occurrence: SourceSpan,
            path: &[u8],
            role: &str,
            declaration_kind: &str,
            declarations: impl Iterator<Item = (SymbolHandle, bool)>,
        ) -> Result<SymbolHandle, String> {
            let name = std::str::from_utf8(path)
                .map_err(|_| format!("product {role} path must be canonical UTF-8"))?;
            if name.is_empty() {
                return Err(format!(
                    "product {role} selection requires a non-empty path"
                ));
            }
            let scope = ProductScope::new(program, occurrence, name);
            let mut selected = None;
            let mut inaccessible = false;
            for (symbol, is_public) in declarations {
                let canonical = program.symbols.display_path(symbol, "::");
                let leaf = program.symbols.name(symbol);
                let probe = name.split_once("::").map_or(name, |(_, path)| path);
                inaccessible |= leaf == probe || canonical == probe;
                if !scope.contains(program, symbol, is_public) {
                    continue;
                }
                if selected.replace(symbol).is_some() {
                    return Err(format!(
                        "product {role} `{name}` is ambiguous within its package"
                    ));
                }
            }
            selected.ok_or_else(|| if inaccessible {
        format!("product {role} `{name}` is not a {declaration_kind} declaration visible from this build occurrence's package")
            } else {
        format!("product {role} `{name}` names no {declaration_kind} declaration in this build occurrence's package")
            })
        }

        pub fn describe_entry(
            program: &TypedTrees,
            occurrence: SourceSpan,
            path: &[u8],
            slot: &[u8],
            slots: &dyn ProgramEntrySourceCatalog,
        ) -> Result<DescribedProductEntry, String> {
            let name = std::str::from_utf8(path)
                .map_err(|_| "product entry path must be canonical UTF-8".to_owned())?;
            let slot = std::str::from_utf8(slot)
                .map_err(|_| "product entry slot must be canonical UTF-8".to_owned())?;
            if name.is_empty() || slot.is_empty() {
                return Err("product entry selection requires a non-empty path and slot".to_owned());
            }
            let scope = ProductScope::new(program, occurrence, name);
            let mut visible = false;
            let mut selected = None;
            let mut rejection = None;
            let mut slot_contract = None;
            for machine in program.machines() {
                if !scope.contains(program, machine.symbol, machine.is_public) {
                    continue;
                }
                visible = true;
                let declaration = ProductEntryDeclaration::from_machine(program, machine);
                let compatibility =
                    match slot_contract.get_or_insert_with(|| slots.source_contract(slot)) {
                        Ok(contract) => crate::product_entries::validate_source_compatibility(
                            program,
                            declaration,
                            *contract,
                        )
                        .map_err(|diagnostics| {
                            diagnostics
                                .into_iter()
                                .map(|diagnostic| diagnostic.message)
                                .collect::<Vec<_>>()
                                .join("; ")
                        }),
                        Err(reason) => Err(reason.clone()),
                    };
                match compatibility {
                    Ok(()) => {
                        if selected.is_some() {
                            return Err(format!(
                                "product entry `{name}` is ambiguous: more than one visible declaration is compatible with slot `{slot}`"
                            ));
                        }
                        selected = Some(machine);
                    }
                    Err(reason) => {
                        rejection.get_or_insert(reason);
                    }
                }
            }
            if !visible {
                let probe = name.split_once("::").map_or(name, |(_, path)| path);
                let inaccessible = program.machines().iter().any(|machine| {
                    machine.name.as_str() == probe
                        || program.symbols.display_path(machine.symbol, "::") == probe
                });
                return Err(if inaccessible {
                    format!(
                        "product entry `{name}` is not a product declaration visible from this build occurrence's package"
                    )
                } else {
                    format!(
                        "product entry `{name}` names no machine in this build occurrence's package"
                    )
                });
            }
            let machine = selected.ok_or_else(|| {
                format!(
                    "product entry `{name}` is incompatible with slot `{slot}`: {}",
                    rejection.as_deref().unwrap_or("no compatible declaration"),
                )
            })?;
            Ok(DescribedProductEntry {
                declaration: symbols::RetainedDeclaration::capture(
                    &program.symbols,
                    machine.symbol,
                )
                .map_err(str::to_owned)?,
                machine_name: machine.name.as_str().to_owned(),
                slot: slot.to_owned(),
            })
        }
    }
    pub mod substitutions {
        use symbols::SymbolHandle;

        pub fn actual_for<T: Clone>(
            substitutions: &[(SymbolHandle, T)],
            symbol: SymbolHandle,
        ) -> Option<T> {
            substitutions
                .iter()
                .rev()
                .find(|(parameter, _)| *parameter == symbol)
                .map(|(_, actual)| actual.clone())
        }
    }
    pub(crate) mod type_applications {
        use symbols::SymbolHandle;
        use typed_trees::data::{DataDefinition, TypeParameterKind};
        use typed_trees::type_identity::TypeIdentityRequest;
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        pub(crate) type TypeBindings = Vec<(SymbolHandle, TypeReferenceHandle)>;

        pub struct DeclarationScopes {
            scopes: Vec<DeclarationScope>,
        }

        pub(crate) struct DeclarationScope {
            bindings: TypeBindings,
            starts: Vec<usize>,
            parent: usize,
        }

        impl Default for DeclarationScopes {
            fn default() -> Self {
                Self {
                    scopes: vec![DeclarationScope {
                        bindings: Vec::new(),
                        starts: Vec::new(),
                        parent: 0,
                    }],
                }
            }
        }

        impl DeclarationScopes {
            pub(crate) fn from_bindings(bindings: TypeBindings) -> (Self, usize) {
                let mut scopes = Self::default();
                if bindings.is_empty() {
                    return (scopes, 0);
                }
                scopes.scopes.push(DeclarationScope {
                    bindings,
                    starts: vec![0],
                    parent: 0,
                });
                (scopes, 1)
            }

            pub fn head(
                &self,
                program: &dyn crate::CheckingInput,
                mut reference: TypeReferenceHandle,
                mut scope: usize,
            ) -> (TypeReferenceHandle, usize) {
                loop {
                    if !reference.is_valid() {
                        return (reference, scope);
                    }
                    let TypeReferenceNode::Named { symbol, .. } =
                        program.type_reference_table.type_reference(reference)
                    else {
                        return (reference, scope);
                    };
                    let mut owner = scope;
                    loop {
                        let environment = &self.scopes[owner];
                        let Some(start) = environment.starts.last().copied() else {
                            return (reference, scope);
                        };
                        if let Some((_, actual)) = environment.bindings[start..]
                            .iter()
                            .find(|(formal, _)| formal == symbol)
                        {
                            reference = *actual;
                            scope = environment.parent;
                            break;
                        }
                        owner = environment.parent;
                    }
                }
            }

            pub fn type_identity(
                &self,
                program: &dyn crate::CheckingInput,
                reference: TypeReferenceHandle,
                scope: usize,
            ) -> typed_trees::type_identity::TypeIdentityId {
                let environment = &self.scopes[scope];
                program.scoped_type_identity(
                    TypeIdentityRequest {
                        substitutions: &environment.bindings,
                        ..TypeIdentityRequest::ordinary(reference)
                    },
                    &environment.starts,
                )
            }

            pub fn is_strict_argument(
                &self,
                program: &dyn crate::CheckingInput,
                parent: TypeReferenceHandle,
                parent_scope: usize,
                selected: typed_trees::type_identity::TypeIdentityId,
            ) -> bool {
                let (parent, parent_scope) = self.head(program, parent, parent_scope);
                let TypeReferenceNode::Generic { arguments, .. } =
                    program.type_reference_table.type_reference(parent)
                else {
                    return false;
                };
                let mut pending = program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .map(|reference| (*reference, parent_scope))
                    .collect::<Vec<_>>();
                let mut visited = Vec::new();
                while let Some((reference, scope)) = pending.pop() {
                    let (reference, scope) = self.head(program, reference, scope);
                    if visited.contains(&(reference, scope)) {
                        continue;
                    }
                    visited.push((reference, scope));
                    if self.type_identity(program, reference, scope) == selected {
                        return true;
                    }
                    match program.type_reference_table.type_reference(reference) {
                        TypeReferenceNode::Generic { arguments, .. } => pending.extend(
                            program
                                .type_reference_table
                                .type_reference_handles(*arguments)
                                .iter()
                                .map(|reference| (*reference, scope)),
                        ),
                        TypeReferenceNode::Constrained { base_type, .. } => {
                            pending.push((*base_type, scope))
                        }
                        TypeReferenceNode::FixedArray { element_type, .. } => {
                            pending.push((*element_type, scope))
                        }
                        _ => {}
                    }
                }
                false
            }

            pub fn definition<'program>(
                &mut self,
                program: &'program dyn crate::CheckingInput,
                reference: TypeReferenceHandle,
                scope: usize,
            ) -> Option<(&'program DataDefinition, usize)> {
                if let Some(storage) = program.layout_storage_arguments(reference) {
                    let (semantic, scope) = self.head(program, storage.semantic_type, scope);
                    return self.definition(program, semantic, scope);
                }
                if let TypeReferenceNode::Constrained { base_type, .. } =
                    program.type_reference_table.type_reference(reference)
                {
                    return self.definition(program, *base_type, scope);
                }
                if let Some(definition) = program.closed_data_definition(reference) {
                    return Some((definition, scope));
                }
                let TypeReferenceNode::Generic {
                    base_symbol,
                    lifetime_arguments,
                    arguments,
                    ..
                } = program.type_reference_table.type_reference(reference)
                else {
                    return None;
                };
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(*arguments);
                let definition = unique_data_definition(program, *base_symbol)?;
                let parameters = program.data_type_parameters(definition);
                if parameters.len() != arguments.len()
                    || lifetime_arguments.len() != definition.lifetime_parameters.len()
                    || parameters.iter().any(|parameter| {
                        !parameter.symbol.is_valid()
                            || !matches!(parameter.kind, TypeParameterKind::Type)
                    })
                    || arguments.iter().any(|argument| {
                        !program
                            .type_reference_table
                            .contains_type_reference(*argument)
                    })
                {
                    return None;
                }
                let parent = scope;
                let mut bindings = self.scopes[parent].bindings.clone();
                let mut starts = self.scopes[parent].starts.clone();
                starts.push(bindings.len());
                bindings.extend(
                    parameters
                        .iter()
                        .map(|parameter| parameter.symbol)
                        .zip(arguments.iter().copied()),
                );
                let scope = self.scopes.len();
                self.scopes.push(DeclarationScope {
                    bindings,
                    starts,
                    parent,
                });
                Some((definition, scope))
            }
        }

        pub(crate) fn substituted_head(
            program: &dyn crate::CheckingInput,
            mut reference: TypeReferenceHandle,
            bindings: &[(SymbolHandle, TypeReferenceHandle)],
        ) -> TypeReferenceHandle {
            for _ in 0..=bindings.len() {
                if !reference.is_valid() {
                    return reference;
                }
                let TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(reference)
                else {
                    return reference;
                };
                let Some(bound) = crate::queries::substitutions::actual_for(bindings, *symbol)
                else {
                    return reference;
                };
                reference = bound;
            }
            reference
        }

        pub(crate) fn unique_data_definition(
            program: &dyn crate::CheckingInput,
            symbol: SymbolHandle,
        ) -> Option<&DataDefinition> {
            if !symbol.is_valid() {
                return None;
            }
            let mut definitions = program
                .data_definitions()
                .iter()
                .filter(|definition| definition.symbol == symbol);
            let definition = definitions.next()?;
            definitions.next().is_none().then_some(definition)
        }

        pub(crate) fn push_generic_application_bindings<'program>(
            program: &'program dyn crate::CheckingInput,
            base_symbol: SymbolHandle,
            arguments: &[TypeReferenceHandle],
            bindings: &mut TypeBindings,
        ) -> Option<&'program DataDefinition> {
            let definition = unique_data_definition(program, base_symbol)?;
            let parameters = program.data_type_parameters(definition);
            if parameters.len() != arguments.len()
                || parameters.iter().any(|parameter| {
                    !parameter.symbol.is_valid()
                        || !matches!(parameter.kind, TypeParameterKind::Type)
                })
            {
                return None;
            }
            let resolved: Vec<TypeReferenceHandle> = arguments
                .iter()
                .map(|argument| substituted_head(program, *argument, bindings))
                .collect();
            bindings.extend(
                parameters
                    .iter()
                    .map(|parameter| parameter.symbol)
                    .zip(resolved),
            );
            Some(definition)
        }
    }
    pub(crate) mod type_regions {
        use typed_trees::name::Identifier;
        use typed_trees::types::{
            DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
        };

        pub(crate) trait TypeRegionScope: Copy {
            fn erased_identity(
                self,
                program: &dyn crate::CheckingInput,
                reference: TypeReferenceHandle,
            ) -> typed_trees::type_identity::NormalizedTypeIdentity {
                program.normalized_type_identity(reference)
            }

            fn enter_operand(
                self,
                _program: &dyn crate::CheckingInput,
                _owner: symbols::SymbolHandle,
            ) -> Self {
                self
            }

            fn at_lifetime(
                self,
                program: &dyn crate::CheckingInput,
                reference: TypeReferenceHandle,
                slot: usize,
                spelling: Option<&Identifier>,
            ) -> Option<Self> {
                Some(
                    match retained_lifetime_identity(program, reference, slot, spelling).ok()? {
                        Some(region) => self.enter_lifetime(program, region),
                        None => self,
                    },
                )
            }

            fn enter_lifetime(
                self,
                program: &dyn crate::CheckingInput,
                region: typed_trees::type_identity::LifetimeParameterIdentity,
            ) -> Self {
                self.enter_operand(program, region.declaration)
            }

            fn named_alias(
                self,
                _program: &dyn crate::CheckingInput,
                _symbol: symbols::SymbolHandle,
                _reference: TypeReferenceHandle,
            ) -> Option<ReferenceStep<Self>> {
                None
            }

            fn resolve(
                mut self,
                program: &dyn crate::CheckingInput,
                mut reference: TypeReferenceHandle,
            ) -> Option<(TypeReferenceHandle, Self)> {
                let mut visited = Vec::new();
                loop {
                    if !reference.is_valid() {
                        return Some((reference, self));
                    }
                    if visited.contains(&reference)
                        || !program
                            .type_reference_table
                            .contains_type_reference(reference)
                    {
                        return None;
                    }
                    visited.push(reference);
                    let owner = program
                        .type_reference_table
                        .operand_lifetime_owner(reference);
                    if owner.is_valid() {
                        self = self.enter_operand(program, owner);
                    }
                    let TypeReferenceNode::Named { symbol, .. } =
                        program.type_reference_table.type_reference(reference)
                    else {
                        return Some((reference, self));
                    };
                    match self.named_alias(program, *symbol, reference) {
                        Some(ReferenceStep::Refuse) => return None,
                        Some(ReferenceStep::Stop) => return Some((reference, self)),
                        Some(ReferenceStep::Chase(scope, next)) => {
                            self = scope;
                            reference = next;
                            continue;
                        }
                        None => {}
                    }
                    if let Some(application) = program
                        .data_definition_by_symbol(*symbol)
                        .and_then(|definition| definition.generic_instance)
                    {
                        reference = application;
                        continue;
                    }
                    return Some((reference, self));
                }
            }
        }

        pub(crate) fn retained_lifetime_identity(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
            slot: usize,
            spelling: Option<&Identifier>,
        ) -> Result<Option<typed_trees::type_identity::LifetimeParameterIdentity>, &'static str>
        {
            let regions = program.type_reference_table.operand_lifetimes(reference);
            if regions.is_empty() {
                return Ok(None);
            }
            let expected = match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Reference {
                    lifetime: Some(_), ..
                } => 1,
                TypeReferenceNode::Generic {
                    lifetime_arguments, ..
                } => lifetime_arguments.len(),
                _ => 0,
            };
            if regions.len() != expected {
                return Err("retained lifetime slots do not match the type's telescope");
            }
            let region = *regions
                .get(slot)
                .ok_or("retained lifetime slot is absent")?;
            if !region.declaration.is_valid() {
                return Ok(None);
            }
            if typed_trees::type_identity::lifetime_parameter_name(program.typed_program(), region)
                != spelling
            {
                return Err("retained lifetime slot lost its exact declaration ordinal");
            }
            Ok(Some(region))
        }

        pub(crate) enum ReferenceStep<Scope> {
            Chase(Scope, TypeReferenceHandle),
            Stop,
            Refuse,
        }

        #[derive(Clone, Copy, PartialEq, Eq)]
        pub(crate) enum TypeRegionRelationship {
            CallableRefinement,
            ExactApplication,
        }

        pub(crate) fn type_reference_regions_match<
            Scope: TypeRegionScope,
            State: Clone + PartialEq,
        >(
            program: &dyn crate::CheckingInput,
            actual: TypeReferenceHandle,
            required: TypeReferenceHandle,
            actual_scope: Scope,
            required_scope: Scope,
            state: &mut State,
            lifetime_matches: &mut impl FnMut(
                &mut State,
                Scope,
                Scope,
                TypeReferenceHandle,
                Option<&Identifier>,
                Option<&Identifier>,
            ) -> bool,
        ) -> bool {
            let mut applications = vec![state.clone()];
            if !type_reference_region_applications(
                program,
                actual,
                required,
                actual_scope,
                required_scope,
                TypeRegionRelationship::CallableRefinement,
                &mut applications,
                lifetime_matches,
            ) {
                return false;
            }
            let [application] = applications.as_slice() else {
                return false;
            };
            *state = application.clone();
            true
        }

        pub(crate) fn type_reference_region_applications<
            Scope: TypeRegionScope,
            State: Clone + PartialEq,
        >(
            program: &dyn crate::CheckingInput,
            actual: TypeReferenceHandle,
            required: TypeReferenceHandle,
            actual_scope: Scope,
            required_scope: Scope,
            relationship: TypeRegionRelationship,
            applications: &mut Vec<State>,
            lifetime_matches: &mut impl FnMut(
                &mut State,
                Scope,
                Scope,
                TypeReferenceHandle,
                Option<&Identifier>,
                Option<&Identifier>,
            ) -> bool,
        ) -> bool {
            if applications.is_empty() {
                return false;
            }
            if !actual.is_valid() || !required.is_valid() {
                return actual.is_valid() == required.is_valid();
            }
            let Some((actual, actual_scope, actual_domains)) =
                type_structure(program, actual, actual_scope)
            else {
                return false;
            };
            let Some((required, required_scope, required_domains)) =
                type_structure(program, required, required_scope)
            else {
                return false;
            };
            macro_rules! region_child {
                ($actual:expr, $required:expr $(,)?) => {
                    type_reference_region_applications(
                        program,
                        $actual,
                        $required,
                        actual_scope,
                        required_scope,
                        relationship,
                        applications,
                        lifetime_matches,
                    )
                };
            }
            if !domain_region_applications(
                program,
                &actual_domains,
                &required_domains,
                DomainCoverage::Required,
                relationship,
                applications,
                lifetime_matches,
            ) || (relationship == TypeRegionRelationship::ExactApplication
                && !domain_region_applications(
                    program,
                    &actual_domains,
                    &required_domains,
                    DomainCoverage::Actual,
                    relationship,
                    applications,
                    lifetime_matches,
                ))
            {
                return false;
            }
            match (
                program.type_reference_table.type_reference(actual),
                program.type_reference_table.type_reference(required),
            ) {
                (
                    TypeReferenceNode::Reference {
                        referee: actual_referee,
                        lifetime: actual_lifetime,
                        ..
                    },
                    TypeReferenceNode::Reference {
                        referee: required_referee,
                        lifetime: required_lifetime,
                        ..
                    },
                ) => {
                    let Some(actual_region_scope) =
                        actual_scope.at_lifetime(program, actual, 0, actual_lifetime.as_ref())
                    else {
                        return false;
                    };
                    let Some(required_region_scope) = required_scope.at_lifetime(
                        program,
                        required,
                        0,
                        required_lifetime.as_ref(),
                    ) else {
                        return false;
                    };
                    applications.retain_mut(|state| {
                        lifetime_matches(
                            state,
                            actual_region_scope,
                            required_region_scope,
                            actual,
                            actual_lifetime.as_ref(),
                            required_lifetime.as_ref(),
                        )
                    });
                    region_child!(*actual_referee, *required_referee)
                }
                (
                    TypeReferenceNode::FixedArray {
                        element_type: actual_element,
                        ..
                    },
                    TypeReferenceNode::FixedArray {
                        element_type: required_element,
                        ..
                    },
                )
                | (
                    TypeReferenceNode::Slice {
                        element_type: actual_element,
                    },
                    TypeReferenceNode::Slice {
                        element_type: required_element,
                    },
                ) => region_child!(*actual_element, *required_element),
                (
                    TypeReferenceNode::Generic {
                        lifetime_arguments: actual_lifetimes,
                        arguments: actual_arguments,
                        ..
                    },
                    TypeReferenceNode::Generic {
                        lifetime_arguments: required_lifetimes,
                        arguments: required_arguments,
                        ..
                    },
                ) => {
                    actual_lifetimes.len() == required_lifetimes.len()
                        && actual_lifetimes
                            .iter()
                            .zip(required_lifetimes)
                            .enumerate()
                            .all(|(slot, (actual_lifetime, required_lifetime))| {
                                let Some(actual_region_scope) = actual_scope.at_lifetime(
                                    program,
                                    actual,
                                    slot,
                                    Some(actual_lifetime),
                                ) else {
                                    return false;
                                };
                                let Some(required_region_scope) = required_scope.at_lifetime(
                                    program,
                                    required,
                                    slot,
                                    Some(required_lifetime),
                                ) else {
                                    return false;
                                };
                                applications.retain_mut(|state| {
                                    lifetime_matches(
                                        state,
                                        actual_region_scope,
                                        required_region_scope,
                                        actual,
                                        Some(actual_lifetime),
                                        Some(required_lifetime),
                                    )
                                });
                                !applications.is_empty()
                            })
                        && actual_arguments.count() == required_arguments.count()
                        && program
                            .type_reference_table
                            .type_reference_handles(*actual_arguments)
                            .len()
                            == actual_arguments.len()
                        && program
                            .type_reference_table
                            .type_reference_handles(*required_arguments)
                            .len()
                            == required_arguments.len()
                        && program
                            .type_reference_table
                            .type_reference_handles(*actual_arguments)
                            .iter()
                            .zip(
                                program
                                    .type_reference_table
                                    .type_reference_handles(*required_arguments),
                            )
                            .all(|(actual_argument, required_argument)| {
                                region_child!(*actual_argument, *required_argument)
                            })
                }
                (
                    TypeReferenceNode::DynamicTrait {
                        arguments: actual, ..
                    },
                    TypeReferenceNode::DynamicTrait {
                        arguments: required,
                        ..
                    },
                ) => {
                    actual.len() == required.len()
                        && program
                            .type_reference_table
                            .type_reference_handles(*actual)
                            .len()
                            == actual.len()
                        && program
                            .type_reference_table
                            .type_reference_handles(*required)
                            .len()
                            == required.len()
                        && program
                            .type_reference_table
                            .type_reference_handles(*actual)
                            .iter()
                            .zip(
                                program
                                    .type_reference_table
                                    .type_reference_handles(*required),
                            )
                            .all(|(actual, required)| region_child!(*actual, *required))
                }
                _ => true,
            }
        }

        #[derive(Clone, Copy)]
        pub(crate) enum DomainCoverage {
            Required,
            Actual,
        }

        fn domain_region_applications<Scope: TypeRegionScope, State: Clone + PartialEq>(
            program: &dyn crate::CheckingInput,
            actual_domains: &[(&DomainConstraint, Scope)],
            required_domains: &[(&DomainConstraint, Scope)],
            coverage: DomainCoverage,
            relationship: TypeRegionRelationship,
            applications: &mut Vec<State>,
            lifetime_matches: &mut impl FnMut(
                &mut State,
                Scope,
                Scope,
                TypeReferenceHandle,
                Option<&Identifier>,
                Option<&Identifier>,
            ) -> bool,
        ) -> bool {
            let (obligations, candidates) = match coverage {
                DomainCoverage::Required => (required_domains, actual_domains),
                DomainCoverage::Actual => (actual_domains, required_domains),
            };
            for (obligation, obligation_scope) in obligations {
                let corresponding = candidates.iter().filter(|(candidate, candidate_scope)| {
                    candidate.subject == obligation.subject
                        && candidate.symbol == obligation.symbol
                        && candidate.arguments.len() == obligation.arguments.len()
                        && candidate.arguments.iter().zip(&obligation.arguments).all(
                            |(candidate, obligation)| {
                                candidate_scope.erased_identity(program, *candidate)
                                    == obligation_scope.erased_identity(program, *obligation)
                            },
                        )
                });
                let mut found = false;
                let mut alternatives = Vec::new();
                for (candidate, candidate_scope) in corresponding {
                    found = true;
                    let (actual, actual_scope, required, required_scope) = match coverage {
                        DomainCoverage::Required => {
                            (*candidate, *candidate_scope, *obligation, *obligation_scope)
                        }
                        DomainCoverage::Actual => {
                            (*obligation, *obligation_scope, *candidate, *candidate_scope)
                        }
                    };
                    let mut candidate_applications = applications.clone();
                    if actual
                        .arguments
                        .iter()
                        .zip(&required.arguments)
                        .all(|(actual, required)| {
                            type_reference_region_applications(
                                program,
                                *actual,
                                *required,
                                actual_scope,
                                required_scope,
                                relationship,
                                &mut candidate_applications,
                                lifetime_matches,
                            )
                        })
                    {
                        for application in candidate_applications {
                            if !alternatives.contains(&application) {
                                alternatives.push(application);
                            }
                        }
                    }
                }
                if !found && relationship == TypeRegionRelationship::CallableRefinement {
                    continue;
                }
                *applications = alternatives;
                if applications.is_empty() {
                    return false;
                }
            }
            true
        }

        fn type_structure<Scope: TypeRegionScope>(
            program: &dyn crate::CheckingInput,
            mut reference: TypeReferenceHandle,
            mut scope: Scope,
        ) -> Option<(TypeReferenceHandle, Scope, Vec<(&DomainConstraint, Scope)>)> {
            let mut domains = Vec::new();
            let mut visited = Vec::new();
            loop {
                let (resolved, resolved_scope) = scope.resolve(program, reference)?;
                reference = resolved;
                scope = resolved_scope;
                if visited.contains(&reference) {
                    return None;
                }
                visited.push(reference);
                let TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } = program.type_reference_table.type_reference(reference)
                else {
                    return Some((reference, scope, domains));
                };
                for constraint in program.type_reference_table.constraints(*constraints) {
                    if let TypeConstraintNode::Domain(domain) = constraint {
                        domains.push((domain, scope));
                    }
                }
                reference = *base_type;
            }
        }
    }
}
