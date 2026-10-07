//! Invocation-owned memos over the immutable program being checked.
//!
//! Stage 04 borrows its prepared typed program in CheckingContext at the frozen
//! boundary. Read-only checking inputs carry that context through every query;
//! neither a thread's current scope nor a program address finds it. Queries on
//! bare TypedTrees remain one-shot, so mutation passes cannot borrow frozen
//! evidence accidentally. Memos are not durable IR or semantic identity.

use crate::declarations::symbols::CallerSiteCaches;
use crate::machine_calls::calls::CallFrameCaches;
use crate::program_scope::data_definitions::DataFieldIndex;
use crate::proof_contracts::contract_entailment::structural_judgment::{
    EntryMachines, LicenseCandidates,
};
use crate::value_custody::content_conservation::ContentConservationSourcePlan;
use language_semantics::declaration_selection::AuthoredDeclarationSelections;
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

    /// Source preparation reads resolution data. A checked input or a context
    /// carrying finalized selections reads checked choices over the same trees.
    fn authored_declaration_selections(&self) -> &AuthoredDeclarationSelections {
        self.typed_program().authored_declaration_selections()
    }

    #[doc(hidden)]
    fn checking_memos(&self) -> Option<&CheckingMemos>;
}

impl Deref for dyn CheckingInput + '_ {
    type Target = TypedTrees;

    fn deref(&self) -> &TypedTrees {
        self.typed_program()
    }
}

// Local proof-plan equality observes the typed program, never invocation scratch.
impl PartialEq for dyn CheckingInput + '_ {
    fn eq(&self, other: &Self) -> bool {
        self.typed_program() == other.typed_program()
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
    memos: CheckingMemos,
}

impl<'program> CheckingContext<'program> {
    pub fn new(program: &'program TypedTrees) -> Self {
        Self {
            program,
            finalized_selections: None,
            memos: CheckingMemos::default(),
        }
    }

    /// Borrow checked selections for final visibility and cleanup judgments
    /// without replacing or cloning the immutable typed program or its memos.
    pub fn with_authored_declaration_selections(
        mut self,
        selections: &'program AuthoredDeclarationSelections,
    ) -> Self {
        self.finalized_selections = Some(selections);
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
}

impl Deref for CheckingProjection<'_> {
    type Target = TypedTrees;

    fn deref(&self) -> &TypedTrees {
        self.typed_program()
    }
}

impl CheckingInput for CheckingProjection<'_> {
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
}

/// Opaque invocation scratch. Only checking owns these fields; callers carry
/// it through CheckingInput rather than opening or sharing arbitrary slots.
#[doc(hidden)]
#[derive(Default)]
pub struct CheckingMemos {
    pub(crate) call_frames: Arc<CallFrameCaches>,
    proof_only: OnceLock<Arc<ProofOnlyClassification>>,
    pub(crate) license_candidates: OnceLock<Arc<LicenseCandidates>>,
    pub(crate) entry_machines: OnceLock<Arc<EntryMachines>>,
    pub(crate) caller_sites: OnceLock<Arc<CallerSiteCaches>>,
    pub(crate) content_conservation: OnceLock<Arc<Vec<ContentConservationSourcePlan>>>,
    pub(crate) binary_meanings:
        Mutex<symbols::SymbolKeyMap<(u32, u32, typed_trees::expression::ExpressionHandle), bool>>,
    pub(crate) data_fields: OnceLock<Arc<DataFieldIndex>>,
    pub(crate) contract_occurrences:
        OnceLock<Arc<crate::proof_contracts::contract_results::ContractOccurrenceIndex>>,
    pub(crate) bound_maps:
        OnceLock<Arc<crate::proof_contracts::immutable_integer_bounds::BoundMaps>>,
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
    frozen_memo(
        program,
        |memos| &memos.proof_only,
        || typed_trees::proof_only::classify(program),
    )
}
