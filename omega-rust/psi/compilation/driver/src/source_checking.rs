//! Prepare and check one complete typed program under fresh invocation authority.
//!
//! Product and Build consumers supply admission data, not a preparation/checking
//! sequence. Retained language judgments are matched here against their exact
//! preparation inputs; package authority is admitted even when work is reused.
//! Product root selection consumes the successful result, so it cannot hide an
//! invalid authored body. Host effects and provider dispatch are not inputs.

use diagnostics::Diagnostic;
use representation_selections::OpaqueRepresentationSelection;
use std::sync::Arc;

pub use typed_trees_to_checked_trees::CheckedTreeProductSelection;

/// A complete language judgment beside the source owner that produced its input.
/// Bare typed requests retain no resolver origin and cannot supply an authored
/// extension merely because their final trees match another compilation.
#[derive(Debug, Clone)]
pub struct RetainedCheck {
    judgment: typed_trees_to_checked_trees::RetainedCheck,
    pub(crate) source: Option<Arc<crate::PreparedSource>>,
}

impl RetainedCheck {
    /// Retention identity includes the source origin, not only the checked
    /// result. An originless request may reuse that language judgment, but
    /// cannot replace a source-backed continuation in the invocation store.
    pub fn same_retained_check(&self, other: &Self) -> bool {
        self.judgment.same_judgment(&other.judgment)
            && match (&self.source, &other.source) {
                (None, None) => true,
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                _ => false,
            }
    }
}

/// Checking consumes typed work or the coupled source owner that produced it.
/// Only source completion can construct the latter; callers cannot pair a
/// resolver snapshot with independently supplied typed trees.
pub struct SourceCheckInput {
    owner: CheckInputOwner,
}

enum CheckInputOwner {
    Typed(typed_trees::TypedTrees),
    Source(Arc<crate::PreparedSource>),
}

impl From<typed_trees::TypedTrees> for SourceCheckInput {
    fn from(program: typed_trees::TypedTrees) -> Self {
        Self {
            owner: CheckInputOwner::Typed(program),
        }
    }
}

impl SourceCheckInput {
    pub(crate) fn from_source(source: Arc<crate::PreparedSource>) -> Self {
        Self {
            owner: CheckInputOwner::Source(source),
        }
    }

    pub fn typed(&self) -> &typed_trees::TypedTrees {
        match &self.owner {
            CheckInputOwner::Typed(program) => program,
            CheckInputOwner::Source(source) => source.typed(),
        }
    }

    fn into_checking_parts(
        self,
        retain: bool,
    ) -> (
        typed_trees::TypedTrees,
        Vec<build_time_evaluation::PreCheckEvaluation>,
        Option<Arc<crate::PreparedSource>>,
    ) {
        match self.owner {
            CheckInputOwner::Typed(program) => (program, Vec::new(), None),
            CheckInputOwner::Source(source) => {
                let origin = retain.then(|| source.clone());
                let (program, recipes) = Arc::unwrap_or_clone(source).into_parts();
                (program, recipes, origin)
            }
        }
    }
}

/// Evaluated source permissions and representation facts for this invocation.
/// These are not provider selections or execution authority.
#[derive(Default)]
pub struct SourceAdmission<'request> {
    pub completion: validation::source_completion::SourceCompletionAdmission<'request>,
    pub opaque_properties: &'request [validation::OpaqueDataPropertyReceipt],
    pub opaque_representations: &'request [OpaqueRepresentationSelection],
    pub asm_authority: Option<validation::AsmAuthorityAdmission>,
    pub wire_demands: &'request [language_semantics::wire::WireCompatibilityDemand],
    pub declarations: Option<&'request dyn validation::DeclarationSelectionAuthority>,
}

/// Consume a complete typed program. Previous judgments belong to the same
/// invocation; the first is the incremental predecessor, followed by siblings.
pub struct CheckRequest<'request> {
    pub program: SourceCheckInput,
    pub admission: SourceAdmission<'request>,
    pub previous: &'request [&'request RetainedCheck],
    pub retain: bool,
}

/// Deferred work and the exact authored program whose occurrences it names.
/// Checking creates this pair before executable preparation; a target closure
/// resumes a private copy without reconstructing or replacing its source.
#[derive(Clone)]
pub struct DeferredSourceEvaluation {
    source: Arc<typed_trees::TypedTrees>,
    continuations: Vec<build_time_evaluation::PreCheckEvaluation>,
}

impl DeferredSourceEvaluation {
    pub fn authored(&self) -> &typed_trees::TypedTrees {
        &self.source
    }

    pub fn resume(
        &self,
        evaluate: impl FnOnce(
            &mut typed_trees::TypedTrees,
            Vec<build_time_evaluation::PreCheckEvaluation>,
        ) -> Result<(), Vec<Diagnostic>>,
    ) -> Result<typed_trees::TypedTrees, Vec<Diagnostic>> {
        let mut source = (*self.source).clone();
        evaluate(&mut source, self.continuations.clone())?;
        Ok(source)
    }
}

/// Successful language judgment, before optional whole-product root selection.
pub struct CheckedSource {
    pub program: Arc<checked_trees::CheckedTrees>,
    pub retained: Option<RetainedCheck>,
    pub reused: bool,
    pub deferred_evaluation: Option<DeferredSourceEvaluation>,
}

/// Shared source product with the exact optional root-selection evidence.
#[derive(Clone)]
pub struct SelectedCheckedSource {
    pub program: Arc<checked_trees::CheckedTrees>,
    pub provider_defaults:
        Arc<[build_time_evaluation::provider_declarations::TargetProviderDefaults]>,
    pub retained: Option<RetainedCheck>,
    pub selection: Option<CheckedTreeProductSelection>,
    pub deferred_evaluation: Option<DeferredSourceEvaluation>,
}

impl CheckRequest<'_> {
    pub fn check(self) -> Result<CheckedSource, Vec<Diagnostic>> {
        let _profile = phase_profile::guard("05");
        let (mut program, deferred_recipes, source) = self.program.into_checking_parts(self.retain);
        let deferred_evaluation =
            (!deferred_recipes.is_empty()).then(|| DeferredSourceEvaluation {
                source: Arc::new(program.clone()),
                continuations: deferred_recipes,
            });
        phase_profile::scope("05.a.source_completion", || {
            validation::source_completion::prepare_source_completion(
                &mut program,
                self.admission.completion,
            )
        })?;
        let mut preparation = typed_trees_to_typed_trees::PreparationRequest::settled()
            .with_opaque_property_receipts(self.admission.opaque_properties)
            .with_opaque_representation_selections(self.admission.opaque_representations)
            .with_wire_compatibility_demands(self.admission.wire_demands);
        if let Some(authority) = self.admission.asm_authority {
            preparation = preparation.with_asm_authority(authority);
        }
        let checking = typed_trees_to_checked_trees::CheckingRequest::new()
            .with_declaration_authority(self.admission.declarations);
        let admission = preparation.admission();
        let matching = phase_profile::scope("05.b.retained_match", || {
            self.previous.iter().copied().find(|prior| {
                prior
                    .judgment
                    .judges_preparation_input(&program, &admission)
            })
        });
        let outcome = if let Some(prior) = matching {
            phase_profile::count("05.b.retained_match.hit", 1);
            let outcome = typed_trees_to_checked_trees::reuse_retained_check(
                &prior.judgment,
                &checking,
                self.retain,
            )?;
            build_time_evaluation::admit_placement_lifecycle_plans(
                &outcome.checked.typed,
                self.admission.declarations,
            )?;
            outcome
        } else {
            let retained_input = phase_profile::scope("05.b.retained_input_clone", || {
                self.retain.then(|| (Arc::new(program.clone()), admission))
            });
            let (program, preparation) = phase_profile::scope("05.c.prepare_typed_trees", || {
                typed_trees_to_typed_trees::prepare_typed_trees(program, &preparation)
            })?;
            // Whole-value lifecycle demands become concrete through the same
            // specialization as ordinary calls. Add geometry only: preparation's
            // executable bodies and mathematical judgments remain unchanged.
            let program = phase_profile::scope("05.c.placement_lifecycle_plans", || {
                build_time_evaluation::retain_placement_lifecycle_plans(
                    program,
                    self.admission.declarations,
                )
            })?;
            let prior_dependency_bodies: Vec<_> = self
                .previous
                .iter()
                .flat_map(|prior| prior.judgment.strict_dependency_bodies().iter().cloned())
                .collect();
            phase_profile::scope("05.d.lower_typed_trees", || {
                typed_trees_to_checked_trees::lower_typed_trees_reusing(
                    program,
                    preparation,
                    retained_input,
                    &checking,
                    &prior_dependency_bodies,
                    self.retain,
                )
            })?
        };
        Ok(CheckedSource {
            program: outcome.checked,
            retained: outcome
                .retained
                .map(|judgment| RetainedCheck { judgment, source }),
            reused: outcome.took_prior,
            deferred_evaluation,
        })
    }
}

impl CheckedSource {
    /// Select all authored Product roots after checking, or retain the complete
    /// checked program when no pruning was requested. An explicit empty root set
    /// remains an invalid request, not permission to skip checking or selection.
    pub fn select_product(
        self,
        roots: Option<Vec<symbols::SymbolHandle>>,
    ) -> Result<SelectedCheckedSource, Vec<Diagnostic>> {
        // Defaults describe every authored target, including declarations root
        // pruning may remove. Decode them from the checked source before pruning;
        // realization later chooses a row without reading declaration bodies.
        let provider_defaults =
            build_time_evaluation::provider_declarations::retain_provider_defaults(
                build_time_evaluation::provider_declarations::ProviderPlanningContext::new(
                    &self.program.typed,
                    source::DependencyScope::Product,
                ),
            )?;
        let (program, selection) =
            typed_trees_to_checked_trees::select_checked_tree_product(self.program, roots)?;
        Ok(SelectedCheckedSource {
            program,
            provider_defaults: provider_defaults.into(),
            retained: self.retained,
            selection,
            deferred_evaluation: self.deferred_evaluation,
        })
    }
}

/// Invocation-owned Product selections over already-shared checked programs.
/// Root pruning is a transform, not a new judgment. Equal roots of the same
/// immutable input retain its one output rather than cloning and pruning it
/// once per realization. Authority admission happens before this operation.
#[derive(Default)]
pub struct ProductSelections {
    entries: Vec<SelectedProduct>,
}

struct SelectedProduct {
    input: Arc<checked_trees::CheckedTrees>,
    roots: Option<Vec<symbols::SymbolHandle>>,
    program: Arc<checked_trees::CheckedTrees>,
    provider_defaults: Arc<[build_time_evaluation::provider_declarations::TargetProviderDefaults]>,
    selection: Option<CheckedTreeProductSelection>,
}

impl ProductSelections {
    pub fn select(
        &mut self,
        checked: CheckedSource,
        roots: Option<Vec<symbols::SymbolHandle>>,
    ) -> Result<SelectedCheckedSource, Vec<Diagnostic>> {
        if let Some(prior) = self
            .entries
            .iter()
            .find(|prior| Arc::ptr_eq(&prior.input, &checked.program) && prior.roots == roots)
        {
            return Ok(SelectedCheckedSource {
                program: prior.program.clone(),
                provider_defaults: prior.provider_defaults.clone(),
                selection: prior.selection.clone(),
                retained: checked.retained,
                deferred_evaluation: checked.deferred_evaluation,
            });
        }
        let input = checked.program.clone();
        let output = checked.select_product(roots.clone())?;
        self.entries.push(SelectedProduct {
            input,
            roots,
            program: output.program.clone(),
            provider_defaults: output.provider_defaults.clone(),
            selection: output.selection.clone(),
        });
        Ok(output)
    }
}
