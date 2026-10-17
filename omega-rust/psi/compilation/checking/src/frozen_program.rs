//! Invocation-owned memos over the immutable program being checked.
//!
//! Stage 04 borrows its prepared typed program in CheckingContext at the frozen
//! boundary. Read-only checking inputs carry that context through every query;
//! neither a thread's current scope nor a program address finds it. Queries on
//! bare TypedTrees remain one-shot, so mutation passes cannot borrow frozen
//! evidence accidentally. Memos are not durable IR or semantic identity.

use crate::declarations::data_queries::DataFieldIndex;
use crate::declarations::symbols::CallerSiteCaches;
use crate::machine_calls::calls::CallFrameCaches;
use crate::machine_calls::effect_inference::PreparedEffectPlans;
use crate::proof_contracts::contract_entailment::structural_judgment::{
    EntryMachines, LicenseCandidates,
};
use crate::value_custody::content_conservation::ContentConservationSourcePlan;
use language_semantics::declaration_selection::AuthoredDeclarationSelections;
use representation_selections::OpaqueRepresentationSelection;
use std::fmt;
use std::ops::Deref;
use std::sync::{Arc, Mutex, OnceLock};
use typed_trees::TypedTrees;
use typed_trees::proof_only::ProofOnlyClassification;

mod sealed {
    pub trait Sealed {}
    impl Sealed for typed_trees::TypedTrees {}
    impl Sealed for checked_trees::CheckedTrees {}
    impl Sealed for super::CheckingContext<'_> {}
    impl Sealed for super::CheckingProjection<'_> {}
    impl Sealed for crate::layout_checking::LayoutCheckingContext<'_> {}
    impl Sealed for typed_trees::FrozenTypedTrees {}
}

/// Read-only source checking input. A prepared invocation owns reusable memos;
/// a bare typed program supplies no frozen memo state.
///
/// The closed implementations prevent substituting another program under an
/// invocation's memos. Dereferencing exposes typed queries, not mutation or
/// permission to retain this scratch in a compilation product.
pub trait CheckingInput: sealed::Sealed + fmt::Debug + Send + Sync {
    fn typed_program(&self) -> &TypedTrees;

    /// One universally checked layout alternative, never a deployment choice.
    /// Bare inputs cannot supply concrete geometry for a target family.
    fn layout_target(&self) -> Option<&str> {
        None
    }

    /// The authoritative build's exact opaque-representation applications for
    /// this invocation. A bare input selects no carrier, so a boundary-opaque
    /// value still cannot claim storage against it.
    fn opaque_representation_selections(&self) -> &[OpaqueRepresentationSelection] {
        &[]
    }

    /// Source preparation reads resolution data. A checked input or a context
    /// carrying finalized selections reads checked choices over the same trees.
    fn authored_declaration_selections(&self) -> &AuthoredDeclarationSelections {
        self.typed_program().authored_declaration_selections()
    }

    #[doc(hidden)]
    fn checking_memos(&self) -> Option<&CheckingMemos>;

    /// Structural classification observes only these immutable typed trees,
    /// unlike geometry-sensitive checking memos. Layout views can share this
    /// query without sharing judgments for another target's representation.
    fn proof_only_classification(&self) -> Arc<ProofOnlyClassification> {
        let compute = || Arc::new(typed_trees::proof_only::classify(self.typed_program()));
        match self.checking_memos() {
            Some(memos) => memos.proof_only.get_or_init(compute).clone(),
            None => compute(),
        }
    }

    /// Program-fixed call-frame symbol tables shared by every resolver over
    /// this input. Inputs without invocation state supply none, and each
    /// resolver builds its own.
    fn call_frame_tables(&self) -> Option<Arc<crate::machine_calls::calls::CallFrameTables<'_>>> {
        None
    }

    /// Cleanup-complete effects are an explicit input to the later checking
    /// phase. Source preparation has no such input and infers source effects.
    fn prepared_effect_plans(&self) -> Option<&PreparedEffectPlans> {
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

// Local proof-plan equality observes the typed program, never invocation scratch.
impl PartialEq for dyn CheckingInput + '_ {
    fn eq(&self, other: &Self) -> bool {
        self.layout_target() == other.layout_target()
            && self.typed_program() == other.typed_program()
    }
}

impl Eq for dyn CheckingInput + '_ {}

impl CheckingInput for TypedTrees {
    fn typed_program(&self) -> &TypedTrees {
        self
    }

    fn checking_memos(&self) -> Option<&CheckingMemos> {
        None
    }
}

/// One sorted, deduplicated `(symbol, OnceLock)` memo row set — the row
/// table shape every query memo in this crate builds over its symbol list.
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
}

impl CheckingInput for checked_trees::CheckedTrees {
    fn typed_program(&self) -> &TypedTrees {
        &self.typed
    }

    fn authored_declaration_selections(&self) -> &AuthoredDeclarationSelections {
        self.facts.authored_declaration_selections()
    }

    fn checking_memos(&self) -> Option<&CheckingMemos> {
        None
    }
}

/// Borrows one immutable program and owns invocation-local query state. Moving
/// or nesting contexts preserves memos without installing ambient state.
pub struct CheckingContext<'program> {
    program: &'program TypedTrees,
    finalized_selections: Option<&'program AuthoredDeclarationSelections>,
    prepared_effects: Option<&'program PreparedEffectPlans>,
    prepared_mathematics:
        Option<crate::mathematical_applications::PreparedMathematicalView<'program>>,
    opaque_selections: Option<&'program [OpaqueRepresentationSelection]>,
    memos: CheckingMemos,
    call_frame_tables:
        OnceLock<Option<Arc<crate::machine_calls::calls::CallFrameTables<'program>>>>,
}

impl<'program> CheckingContext<'program> {
    pub fn new(program: &'program TypedTrees) -> Self {
        Self {
            program,
            finalized_selections: None,
            prepared_effects: None,
            prepared_mathematics: None,
            opaque_selections: None,
            memos: CheckingMemos::default(),
            call_frame_tables: OnceLock::new(),
        }
    }

    /// Borrow checked selections for final visibility and cleanup judgments
    /// without replacing or cloning the immutable typed program. Selection
    /// authority is an input to meaning queries, so replacing it invalidates
    /// any prior memo even though the source tables remain equal.
    pub fn with_authored_declaration_selections(
        mut self,
        selections: &'program AuthoredDeclarationSelections,
    ) -> Self {
        self.finalized_selections = Some(selections);
        self.memos = CheckingMemos::default();
        self.call_frame_tables = OnceLock::new();
        self
    }

    /// Switch to the ownership-complete execution graph. Effects participate
    /// in later judgments, so no source-only memo survives this phase change.
    pub fn with_prepared_effect_plans(mut self, plans: &'program PreparedEffectPlans) -> Self {
        self.prepared_effects = Some(plans);
        self.memos = CheckingMemos::default();
        self.call_frame_tables = OnceLock::new();
        self
    }

    /// Mathematical applications were formed and reduced before this phase.
    /// Their immutable arena and whole-value captures share the invocation's
    /// lifetime, not a global cache or a later source reconstruction.
    pub fn with_prepared_mathematics(
        mut self,
        mathematics: Option<crate::mathematical_applications::PreparedMathematicalView<'program>>,
    ) -> Self {
        self.prepared_mathematics = mathematics;
        self.memos = CheckingMemos::default();
        self.call_frame_tables = OnceLock::new();
        self
    }

    /// Attach the build's validated opaque applications: admission evidence a
    /// boundary-opaque value's storage judgment reads, not typed-program data.
    /// Meaning changes with the admitted roster, so no memo survives it.
    pub fn with_opaque_representation_selections(
        mut self,
        selections: &'program [OpaqueRepresentationSelection],
    ) -> Self {
        self.opaque_selections = Some(selections);
        self.memos = CheckingMemos::default();
        self.call_frame_tables = OnceLock::new();
        self
    }

    /// End checking without attaching scratch to the durable typed program.
    pub fn into_program(self) -> &'program TypedTrees {
        self.program
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

    fn call_frame_tables(&self) -> Option<Arc<crate::machine_calls::calls::CallFrameTables<'_>>> {
        self.call_frame_tables
            .get_or_init(|| {
                // Built from the typed program so the tables live as long as
                // it does; the caller-site caches rejoin this context's memos.
                let mut tables = crate::machine_calls::calls::CallFrameTables::build(self.program)?;
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

    fn prepared_mathematics(
        &self,
    ) -> Option<crate::mathematical_applications::PreparedMathematicalView<'_>> {
        self.prepared_mathematics
    }

    fn opaque_representation_selections(&self) -> &[OpaqueRepresentationSelection] {
        self.opaque_selections.unwrap_or(&[])
    }
}

/// A private elaboration that retains its source context until mutation is
/// requested. A bare typed-tree Cow loses that association even on unchanged
/// reads. Mutation clones only the program; frozen memos cannot describe it.
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
            // The private mutation copy retains the input's authoritative
            // selections, which may live outside its original typed tables.
            projected.retain_authored_declaration_selections(
                self.original.authored_declaration_selections().clone(),
            );
            projected
        })
    }

    /// Transfer private elaboration storage, retaining authoritative selections
    /// even when an unchanged projection must become independently owned.
    pub fn into_program(self) -> TypedTrees {
        self.projected.unwrap_or_else(|| {
            let mut projected = self.original.typed_program().clone();
            projected.retain_authored_declaration_selections(
                self.original.authored_declaration_selections().clone(),
            );
            projected
        })
    }
}

impl Deref for CheckingProjection<'_> {
    type Target = TypedTrees;

    fn deref(&self) -> &TypedTrees {
        self.typed_program()
    }
}

impl CheckingInput for CheckingProjection<'_> {
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
        match self.projected {
            Some(_) => None,
            None => self.original.checking_memos(),
        }
    }

    fn prepared_effect_plans(&self) -> Option<&PreparedEffectPlans> {
        match self.projected {
            Some(_) => None,
            None => self.original.prepared_effect_plans(),
        }
    }

    fn prepared_mathematics(
        &self,
    ) -> Option<crate::mathematical_applications::PreparedMathematicalView<'_>> {
        match self.projected {
            Some(_) => None,
            None => self.original.prepared_mathematics(),
        }
    }

    fn opaque_representation_selections(&self) -> &[OpaqueRepresentationSelection] {
        self.original.opaque_representation_selections()
    }
}

/// Opaque invocation scratch. Only checking owns these fields; callers carry
/// it through CheckingInput rather than opening or sharing arbitrary slots.
#[doc(hidden)]
#[derive(Default)]
pub struct CheckingMemos {
    pub(crate) owned_field_domains:
        OnceLock<crate::value_custody::field_domains::OwnedFieldDomainQueries>,
    pub(crate) claim_frontiers:
        OnceLock<crate::value_custody::type_properties::ClaimFrontierQueries>,
    pub(crate) establishment: OnceLock<crate::value_custody::type_properties::EstablishmentQueries>,
    pub(crate) call_destinations:
        OnceLock<crate::machine_calls::calls::signature_queries::CallDestinationQueries>,
    pub(crate) operational: OnceLock<Arc<flow_effects::OperationalPlan>>,
    pub(crate) service_reaches: OnceLock<(
        flow_effects::OperationalPlan,
        flow_effects::ServiceReachInferencePlan,
    )>,
    pub(crate) call_frames: Arc<CallFrameCaches>,
    proof_only: OnceLock<Arc<ProofOnlyClassification>>,
    pub(crate) license_candidates: OnceLock<Arc<LicenseCandidates>>,
    pub(crate) entry_machines: OnceLock<Arc<EntryMachines>>,
    pub(crate) caller_sites: OnceLock<Arc<CallerSiteCaches>>,
    pub(crate) content_conservation: OnceLock<Arc<Vec<ContentConservationSourcePlan>>>,
    /// Exact ordered roots to a complete target roster. Rows are retained
    /// as positions in this immutable program's target-layout storage, not
    /// borrowed strings or a selected deployment target.
    pub(crate) layout_target_rosters: Mutex<
        symbols::SymbolKeyMap<
            Vec<typed_trees::types::TypeReferenceHandle>,
            Result<Vec<Option<usize>>, ()>,
        >,
    >,
    /// Concrete body inference consumes exact captured integer intervals, not
    /// caller places. Recipe-bearing arguments use ordinary inference. Retain
    /// the complete result recipe and negative results; immutable actuals and
    /// the exact stored machine/state own this invocation-specific judgment.
    pub(crate) normal_return_intervals: Mutex<
        symbols::SymbolKeyMap<
            (
                symbols::SymbolHandle,
                symbols::SymbolHandle,
                u8,
                Vec<(symbols::SymbolHandle, Option<i64>, Option<i64>)>,
            ),
            Option<(
                crate::proof_contracts::arithmetic_domains::Interval,
                Option<checked_trees::IntegerRangeRecipe>,
            )>,
        >,
    >,
    /// Closed metadata at one exact completed call occurrence. These queries
    /// consume their own immutable state prefix, never an external engine roster.
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
    /// Recursive proof queries may revisit that occurrence. This invocation-
    /// owned active set prevents same-thread recursion without treating another
    /// worker's pending computation as a negative result or holding its lock.
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
    /// Whole bound-expression meaning, including the remaining traversal
    /// budget. Guard DAGs reuse children; caching only each binary's local
    /// meaning still re-expands those children for every enclosing guard.
    /// Depth is part of the key so a shallow success cannot waive the
    /// existing recursion limit when reused inside a deeper expression.
    pub(crate) bound_subtree_meanings: Mutex<
        symbols::SymbolKeyMap<(u32, u32, typed_trees::expression::ExpressionHandle, u8), bool>,
    >,
    /// Exact case-reference paths depend only on this immutable input's
    /// expression, symbol and data tables. Retain the declaration position,
    /// including a negative verdict, without merging equal source spellings.
    pub(crate) case_reference_owners:
        Mutex<symbols::SymbolKeyMap<typed_trees::expression::ExpressionHandle, Option<usize>>>,
    /// One denotational candidacy verdict per selected (machine, state); the
    /// answer is a pure function of the program's call plans, so repeated
    /// consultations across case-arm leaves share the closure walks.
    pub(crate) checked_value_call_candidates: Mutex<
        symbols::SymbolKeyMap<
            (symbols::SymbolHandle, symbols::SymbolHandle),
            Result<(), &'static str>,
        >,
    >,
    /// One owner-designated cleanup role per exact machine symbol; the chain
    /// walk through specialization templates is program-fixed, so cleanup
    /// consumers that probe every machine share each declaration's walk once.
    pub(crate) nominal_cleanup_owners: Mutex<symbols::SymbolMap<Option<symbols::SymbolHandle>>>,
    /// Effective owner -> cleanup machine candidates, built once: a machine
    /// binds under its direct cleanup owner and under the generic base of
    /// that owner's origin, so consumers stop scanning every machine.
    pub(crate) nominal_cleanup_machine_index:
        OnceLock<Arc<symbols::SymbolKeyMap<symbols::SymbolHandle, Vec<symbols::SymbolHandle>>>>,
    /// One selected-application carrier verdict per (caller machine,
    /// application target, declared result, arity); each consultation
    /// re-derives program-fixed ownership, parameter eligibility, callee
    /// independence and carrier matching, which case-arm leaf judging
    /// repeats for the same applications.
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
    pub(crate) data_fields: OnceLock<Arc<DataFieldIndex>>,
    pub(crate) contract_occurrences:
        OnceLock<Arc<crate::proof_contracts::contract_results::ContractOccurrenceIndex>>,
    pub(crate) bound_maps:
        OnceLock<Arc<crate::proof_contracts::immutable_integer_bounds::BoundMaps>>,
    /// Each machine's seeded per-state incoming guard environments, under
    /// the program's own call-frame resolver. A state query previously
    /// rebuilt the whole machine's environments to keep one, once per
    /// observed expression.
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
}

/// A rebuildable selected-application carrier verdict: the owner and declared
/// result are re-derived from the program tables these symbols name, so the
/// memo stores no `'program` borrows.
#[derive(Clone, Copy)]
pub(crate) enum SelectedCarrierVerdict {
    Concrete(symbols::SymbolHandle, symbols::SymbolHandle),
    Operator(symbols::SymbolHandle),
    NoCarrier,
}

/// The reusable memos explicitly carried by a frozen checking input.
pub(crate) fn frozen_program_memos(program: &dyn CheckingInput) -> Option<&CheckingMemos> {
    program.checking_memos()
}

/// Compute once in the input's invocation, or one-shot for a bare typed program.
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

/// Proof-only classification, shared by consumers of this checking context.
pub fn proof_only_classification(program: &dyn CheckingInput) -> Arc<ProofOnlyClassification> {
    program.proof_only_classification()
}
